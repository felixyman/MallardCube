#!/usr/bin/env bash
# Server-side smoke test for the MallardCube SSAS proxy.
# Asserts known demo values from projects/project3 (cube [Sales]).
#
# Usage:
#   bash scripts/proxy-smoke.sh [proxy_url]     run assertions against a running proxy
#   bash scripts/proxy-smoke.sh serve           start the proxy (0.0.0.0:8080, trace on),
#                                               wait until ready, leave it running
#
# Exit code: 0 if all assertions pass, 1 otherwise.

set -u

PROXY="${1:-http://127.0.0.1:8080/xmla}"

# serve: start the proxy correctly (0.0.0.0 so the Windows VM can reach it) and
# wait for readiness. Exits 0 once the proxy answers.
if [ "${1:-}" = "serve" ]; then
  cd "$(dirname "$0")/.." || exit 1
  cargo build >/dev/null 2>&1 || { echo "build failed"; exit 1; }
  pkill -f 'target/debug/mallard' 2>/dev/null
  pkill -f 'target/debug/xmla_proxy' 2>/dev/null
  sleep 1
  BIND_ADDRESS=0.0.0.0:8080 XMLA_TRACE=1 setsid nohup cargo run > /tmp/opencode/proxy.log 2>&1 < /dev/null &
  for _ in $(seq 1 30); do
    code=$(curl -s -m 2 -o /dev/null -w '%{http_code}' -X POST http://127.0.0.1:8080/xmla \
      -H "Content-Type: text/xml" \
      -d '<?xml version="1.0"?><Envelope xmlns="http://schemas.xmlsoap.org/soap/envelope/"><Body><Discover xmlns="urn:schemas-microsoft-com:xml-analysis"><RequestType>DISCOVER_DATASOURCES</RequestType><Restrictions/><Properties/></Discover></Body></Envelope>')
    [ "$code" = "200" ] && { echo "proxy ready on 0.0.0.0:8080 (trace: xmla-trace.jsonl)"; exit 0; }
    sleep 1
  done
  echo "proxy failed to become ready; see /tmp/opencode/proxy.log"
  exit 1
fi

fail=0
pass=0

check_eq() {
  local label="$1" expected="$2" actual="$3"
  if [ "$actual" = "$expected" ]; then
    echo "  PASS  $label = $actual"
    pass=$((pass + 1))
  else
    echo "  FAIL  $label: expected $expected, got ${actual:-<none>}"
    fail=$((fail + 1))
  fi
}

# Send an MDX Execute and print the first <Value> numeric content.
mdx_value() {
  local mdx="$1"
  local esc="${mdx//&/&amp;}"
  curl -s -m 30 -X POST "$PROXY" -H "Content-Type: text/xml" \
    -d "<?xml version=\"1.0\"?><Envelope xmlns=\"http://schemas.xmlsoap.org/soap/envelope/\"><Body><Execute xmlns=\"urn:schemas-microsoft-com:xml-analysis\"><Command><Statement>${esc}</Statement></Command><Properties/></Execute></Body></Envelope>" \
    | grep -oE '<Value[^>]*>[0-9]+</Value>' \
    | head -1 \
    | grep -oE '[0-9]+'
}

# Health: proxy must answer DISCOVER_DATASOURCES with HTTP 200.
echo "== proxy: $PROXY =="
code=$(curl -s -m 5 -o /dev/null -w '%{http_code}' -X POST "$PROXY" \
  -H "Content-Type: text/xml" \
  -d '<?xml version="1.0"?><Envelope xmlns="http://schemas.xmlsoap.org/soap/envelope/"><Body><Discover xmlns="urn:schemas-microsoft-com:xml-analysis"><RequestType>DISCOVER_DATASOURCES</RequestType><Restrictions/><Properties/></Discover></Body></Envelope>')
if [ "$code" = "200" ]; then
  echo "  PASS  proxy reachable (HTTP 200)"
  pass=$((pass + 1))
else
  echo "  FAIL  proxy not reachable (HTTP ${code:-<none>}). Is it running? See /tmp/opencode/proxy.log"
  exit 1
fi

echo "== measures =="
check_eq "Total Revenue" "521586767" "$(mdx_value 'SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales]')"
check_eq "Units total"   "4931640"   "$(mdx_value 'SELECT {[Measures].[Units]} ON COLUMNS FROM [Sales]')"
check_eq "Electronics (tuple on axis)" "24719896" "$(mdx_value 'SELECT {([Measures].[Revenue],[Category].[Category].&[Electronics])} ON 0 FROM [Sales]')"

echo "== time intelligence (flag filters) =="
for pair in "YTD:RevenueYTD:ytd_flag" "QTD:RevenueQTD:qtd_flag" "MTD:RevenueMTD:mtd_flag"; do
  label="${pair%%:*}"; rest="${pair#*:}"; meas="${rest%%:*}"; flag="${rest#*:}"
  val="$(mdx_value "SELECT {[Measures].[$meas]} ON COLUMNS FROM [Sales]")"
  if [ -n "$val" ] && [ "$val" -gt 0 ] 2>/dev/null && [ "$val" -lt 521586767 ] 2>/dev/null; then
    echo "  PASS  Revenue$label ($flag) = $val (strict subset of total)"
    pass=$((pass + 1))
  else
    echo "  FAIL  Revenue$label expected 0 < x < 521586767, got ${val:-<none>}"
    fail=$((fail + 1))
  fi
done

echo "== grouped by category =="
body=$(curl -s -m 30 -X POST "$PROXY" -H "Content-Type: text/xml" \
  -d "<?xml version=\"1.0\"?><Envelope xmlns=\"http://schemas.xmlsoap.org/soap/envelope/\"><Body><Execute xmlns=\"urn:schemas-microsoft-com:xml-analysis\"><Command><Statement>SELECT [Category].[Category].Members ON ROWS, {[Measures].[Revenue]} ON COLUMNS FROM [Sales]</Statement></Command><Properties/></Execute></Body></Envelope>")
catcount=$(echo "$body" | grep -oE '<Caption>[^<]+</Caption>' | grep -vcE 'All|Revenue')
check_eq "Category member count" "20" "$catcount"

echo
if [ "$fail" -eq 0 ]; then
  echo "SMOKE OK: $pass/$((pass + fail)) passed"
  exit 0
else
  echo "SMOKE FAILED: $fail/$((pass + fail)) failed"
  exit 1
fi
