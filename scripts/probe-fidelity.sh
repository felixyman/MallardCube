#!/usr/bin/env bash
# Fidelity probes for the XMLA surface: the requests that used to come back as
# plausible-but-wrong answers instead of faults (plan 051).
#
# Usage:
#   bash scripts/probe-fidelity.sh [proxy_url]     default: http://127.0.0.1:8080/xmla
#
# Every probe asserts what the reference (SSAS) would do, not merely that the
# proxy answered:
#
#   * a CDATA-wrapped statement must run (SOAP clients emit CDATA by default)
#   * unparsable request text must fault, never blank the statement or — worse —
#     drop a Discover restriction and return the *unrestricted* rowset
#   * both nested <restriction> forms must be honoured like the flat form
#   * an Execute whose <Statement> was present but unreadable must fault
#   * an Execute with an empty Statement (or none) is a valid empty success
#
# Exit code: 0 when every probe behaves, 1 otherwise.

set -u

URL="${1:-http://127.0.0.1:8080/xmla}"
CATALOG="SALES_ANALYTICS"
CUBE="Sales"

pass=0
fail=0

post() {
  curl -s -m 30 -X POST "$URL" -H 'Content-Type: text/xml' -d "$1"
}

report() { # label ok detail
  if [ "$2" = "1" ]; then
    pass=$((pass + 1))
    echo "  PASS  $1"
  else
    fail=$((fail + 1))
    echo "  FAIL  $1${3:+ ($3)}"
  fi
}

envelope() { # statement
  cat <<EOF
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><Execute xmlns="urn:schemas-microsoft-com:xml-analysis"><Command><Statement>$1</Statement></Command><Properties><PropertyList><Catalog>${CATALOG}</Catalog></PropertyList></Properties></Execute></soap:Body></soap:Envelope>
EOF
}

discover() { # restriction list body
  cat <<EOF
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><Discover xmlns="urn:schemas-microsoft-com:xml-analysis"><RequestType>MDSCHEMA_MEMBERS</RequestType><Restrictions>$1</Restrictions><Properties><PropertyList><Catalog>${CATALOG}</Catalog></PropertyList></Properties></Discover></soap:Body></soap:Envelope>
EOF
}

echo "== probe-fidelity against $URL"

# --- a statement must run, however its text is wrapped -----------------------
MDX="SELECT {[Measures].[Revenue]} ON COLUMNS FROM [${CUBE}] CELL PROPERTIES VALUE"

out="$(post "$(envelope "$MDX")")"
report "plain statement runs" "$([[ "$out" == *"<Cell "* && "$out" == *"521586767"* ]] && echo 1 || echo 0)" "no cell / wrong value"

out="$(post "$(envelope "<![CDATA[${MDX}]]>")")"
report "CDATA statement runs" "$([[ "$out" == *"<Cell "* && "$out" == *"521586767"* ]] && echo 1 || echo 0)" "$(head -c 80 <<<"$out")"

# --- text we cannot read must fault, not vanish ------------------------------
out="$(post "$(envelope "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [${CUBE}] &foo; CELL PROPERTIES VALUE")")"
report "unparsable entity in a statement faults" "$([[ "$out" == *"faultstring"* ]] && echo 1 || echo 0)" "answered without a fault"

# --- an empty statement is the reference's empty success ---------------------
# MSOLAP's session-begin request carries exactly `<Statement/>`; faulting it (as
# a review once asked) breaks every real connection. Verified against SSAS 2025
# on 2026-09-24: empty, self-closing and whitespace-only statements all answer
# with an empty ExecuteResponse.
out="$(post "$(envelope "<Statement></Statement>")")"
report "empty Statement is the reference's empty success" "$([[ "$out" == *"ExecuteResponse"* && "$out" != *"faultstring"* ]] && echo 1 || echo 0)" "did not answer like the reference"

out="$(post '<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><Execute xmlns="urn:schemas-microsoft-com:xml-analysis"><Command><Statement/></Command><Properties><PropertyList><Catalog>SALES_ANALYTICS</Catalog></PropertyList></Properties></Execute></soap:Body></soap:Envelope>')"
report "self-closing <Statement/> is the reference's empty success" "$([[ "$out" == *"ExecuteResponse"* && "$out" != *"faultstring"* ]] && echo 1 || echo 0)" "did not answer like the reference"

out="$(post "$(envelope "   ")")"
report "whitespace-only Statement is the reference's empty success" "$([[ "$out" == *"ExecuteResponse"* && "$out" != *"faultstring"* ]] && echo 1 || echo 0)" "did not answer like the reference"

# --- no Statement at all stays a valid empty success -------------------------
out="$(post '<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><Execute xmlns="urn:schemas-microsoft-com:xml-analysis"><Command/></Execute></soap:Body></soap:Envelope>')"
report "Execute without a Statement stays empty success" "$([[ "$out" != *"faultstring"* ]] && echo 1 || echo 0)" "faulted a legitimate empty Execute"

# --- Discover restrictions: every form, and never a silent widening ----------
restricted_rows() { grep -o '<row>' <<<"$1" | wc -l; }

flat="<RestrictionList><CATALOG_NAME>${CATALOG}</CATALOG_NAME><CUBE_NAME>${CUBE}</CUBE_NAME><DIMENSION_UNIQUE_NAME>[Category]</DIMENSION_UNIQUE_NAME></RestrictionList>"
out="$(post "$(discover "$flat")")"
baseline="$(restricted_rows "$out")"
report "flat restriction filters (${baseline} rows)" "$([[ "$baseline" -gt 0 && "$baseline" -lt 100 ]] && echo 1 || echo 0)" "expected the Category members only"

out="$(post "$(discover "<RestrictionList><CATALOG_NAME>${CATALOG}</CATALOG_NAME><CUBE_NAME>${CUBE}</CUBE_NAME><DIMENSION_UNIQUE_NAME>[Category]&foo;</DIMENSION_UNIQUE_NAME></RestrictionList>")")"
rows="$(restricted_rows "$out")"
report "unparsable entity in a restriction faults instead of widening" \
  "$([[ "$out" == *"faultstring"* || "$rows" -eq "$baseline" ]] && echo 1 || echo 0)" \
  "returned ${rows} rows (unrestricted is larger)"

nested="<restriction><CATALOG_NAME>${CATALOG}</CATALOG_NAME><CUBE_NAME>${CUBE}</CUBE_NAME><DIMENSION_UNIQUE_NAME>[Category]</DIMENSION_UNIQUE_NAME></restriction>"
out="$(post "$(discover "$nested")")"
rows="$(restricted_rows "$out")"
report "nested <restriction><NAME> form is honoured" "$([[ "$rows" -eq "$baseline" ]] && echo 1 || echo 0)" "returned ${rows} rows, expected ${baseline}"

columns="<restriction><column>CATALOG_NAME</column><value>${CATALOG}</value></restriction><restriction><column>CUBE_NAME</column><value>${CUBE}</value></restriction><restriction><column>DIMENSION_UNIQUE_NAME</column><value>[Category]</value></restriction>"
out="$(post "$(discover "$columns")")"
rows="$(restricted_rows "$out")"
report "nested <column>/<value> form is honoured" "$([[ "$rows" -eq "$baseline" ]] && echo 1 || echo 0)" "returned ${rows} rows, expected ${baseline}"

echo
if [ "$fail" -eq 0 ]; then
  echo "FIDELITY OK: ${pass}/${pass} probes passed"
  exit 0
fi
echo "FIDELITY FAILED: ${fail} of $((pass + fail)) probes failed"
exit 1
