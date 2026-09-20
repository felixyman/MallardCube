#!/usr/bin/env bash
# A/B: a row-level-security query against a large fact, with and without
# rollups (plan 043). Needs a bench database: run scripts/bench.sh first.
#
# Usage: BENCH_DIR=/home/felix/mallardcube-bench bash scripts/rls-rollup-ab.sh
#
# Prints wall-clock seconds for the same role-filtered query three times per
# mode, plus the oracle (raw SQL) so the numbers can be trusted.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BENCH_DIR="${BENCH_DIR:-/tmp/mallardcube-bench}"
DB="$BENCH_DIR/sales_large.duckdb"
CFG="$BENCH_DIR/rls-proxy-config.json"
AGG="$BENCH_DIR/rls-agg.duckdb"
URL="http://127.0.0.1:8080/xmla"

if [ ! -f "$DB" ]; then
  echo "bench database not found: $DB (run scripts/bench.sh first)" >&2
  exit 1
fi

# A project over the bench DB with one role that filters the fact by territory.
python3 - "$REPO_ROOT/projects/project3/proxy-config.json" "$CFG" "$DB" <<'PY'
import json, sys
src, cfg, db = sys.argv[1], sys.argv[2], sys.argv[3]
d = json.load(open(src))
d["db_path"] = db
d["auth"] = {"trusted_proxy": True}
d["roles"] = [{
    "name": "NorthOnly",
    "model_permission": "read",
    "members": [{"member_name": "user1", "member_type": "user"}],
    "table_permissions": [{
        "table": "sales_fact",
        "filter_expression": "territory = 'North'",
    }],
}]
json.dump(d, open(cfg, "w"), indent=2)
PY

query_time() {
  curl -s -m 900 -o /tmp/opencode/rls-response.xml -w '%{time_total}' -X POST "$URL" \
    -H "Content-Type: text/xml" -H "X-User: user1" \
    -d '<?xml version="1.0"?><Envelope xmlns="http://schemas.xmlsoap.org/soap/envelope/"><Body><Execute xmlns="urn:schemas-microsoft-com:xml-analysis"><Command><Statement>SELECT FROM [Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE</Statement></Command><Properties/></Execute></Body></Envelope>'
}

start_server() { # $1 = aggregation sidecar or empty
  pkill -x mallard 2>/dev/null || true
  sleep 1
  local agg_env=""
  [ -n "$1" ] && agg_env="MALLARDCUBE_AGG_CACHE=$1"
  local started_at
  started_at=$(date +%s)
  PROXY_CONFIG="$CFG" BIND_ADDRESS=0.0.0.0:8080 MALLARDCUBE_RESULT_CACHE=0 \
    setsid nohup env $agg_env "$REPO_ROOT/target/release/mallard" serve \
    > "$BENCH_DIR/rls.log" 2>&1 < /dev/null &
  for i in $(seq 1 900); do
    if curl -s -m 2 -o /dev/null "$URL" 2>/dev/null; then
      echo "   (startup $(( $(date +%s) - started_at ))s)"
      return 0
    fi
    [ "$i" = "30" ] && echo "   (still starting — rollup build scans the fact)"
    sleep 1
  done
  echo "server did not become ready; see $BENCH_DIR/rls.log" >&2
  exit 1
}

run_mode() { # $1 = label, $2 = agg sidecar or empty
  start_server "$2"
  printf '%-28s' "$1:"
  for _ in 1 2 3; do
    printf ' %ss' "$(query_time)"
  done
  printf '  value=%s\n' "$(grep -oE '<Value[^>]*>[0-9.]+</Value>' /tmp/opencode/rls-response.xml | head -1 | grep -oE '[0-9.]+')"
  pkill -x mallard 2>/dev/null || true
}

echo "== RLS query A/B ($(basename "$DB")) =="
run_mode "fact path (no rollups)" ""
rm -f "$AGG"
run_mode "rollups (plan 043)" "$AGG"

echo "== oracle =="
duckdb -readonly -noheader -list "$DB" \
  "SELECT SUM(revenue) FROM sales_fact WHERE territory = 'North'" 2>/dev/null
