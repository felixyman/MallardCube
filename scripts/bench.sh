#!/usr/bin/env bash
# Load-test MallardCube against a 5M-row DuckDB database and report latency /
# throughput. Establishes a repeatable "is the proxy the bottleneck?" baseline.
#
# Usage:
#   bash scripts/bench.sh [trace.jsonl] [--concurrency 8] [--iterations 400]
#
# Workload: a captured XMLA trace (see `XMLA_TRACE=1` + an Excel session, or
# reuse the repo's `xmla-trace.jsonl`). Without a trace there is nothing to
# replay, so the script refuses to run.
#
# Requirements: duckdb CLI, python3, cargo.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BENCH_DIR="${BENCH_DIR:-/tmp/mallardcube-bench}"
DB="$BENCH_DIR/sales_large.duckdb"
CFG="$BENCH_DIR/proxy-config.json"
URL="http://127.0.0.1:8080/xmla"

TRACE="${1:-}"
CONC="${CONC:-8}"
ITERS="${ITERS:-400}"
if [ -n "$TRACE" ] && [ ! -f "$TRACE" ]; then
  echo "trace not found: $TRACE" >&2
  exit 1
fi
if [ -z "$TRACE" ]; then
  for candidate in "$REPO_ROOT/scripts/bench-workload.jsonl" "$REPO_ROOT/xmla-trace.jsonl"; do
    [ -f "$candidate" ] && TRACE="$candidate" && break
  done
fi
if [ -z "$TRACE" ]; then
  echo "no workload trace found; capture one with XMLA_TRACE=1 and pass it as an argument" >&2
  exit 1
fi

for cmd in duckdb python3 cargo; do
  command -v "$cmd" >/dev/null || { echo "missing dependency: $cmd" >&2; exit 1; }
done

echo "==> workload: $TRACE"

# 1. Generate the benchmark DB (idempotent).
if [ ! -f "$DB" ]; then
  echo "==> generating 5M-row database at $DB"
  mkdir -p "$BENCH_DIR"
  duckdb "$DB" < "$REPO_ROOT/scripts/bench_gen.sql"
else
  echo "==> reusing existing database at $DB"
fi

# 2. Point a project3 clone at the benchmark DB.
python3 - "$REPO_ROOT/projects/project3/proxy-config.json" "$CFG" "$DB" <<'PY'
import json, sys
src, cfg, db = sys.argv[1], sys.argv[2], sys.argv[3]
d = json.load(open(src))
d["db_path"] = db
json.dump(d, open(cfg, "w"), indent=2)
PY

# 3. Build + start the release proxy (tracing off = production path).
echo "==> building release binary"
(cd "$REPO_ROOT" && cargo build --release >/dev/null 2>&1)
pkill -x mallard 2>/dev/null || true
sleep 1
echo "==> starting proxy on 0.0.0.0:8080"
PROXY_CONFIG="$CFG" BIND_ADDRESS=0.0.0.0:8080 \
  setsid nohup "$REPO_ROOT/target/release/mallard" serve \
  > "$BENCH_DIR/proxy.log" 2>&1 < /dev/null &
for _ in $(seq 1 30); do
  curl -s -m 2 -o /dev/null "$URL" 2>/dev/null && break
  sleep 1
done

run() {
  local kind="$1" concurrency="$2" iterations="$3"
  echo ""
  echo "===== $kind | concurrency $concurrency ====="
  "$REPO_ROOT/target/release/mallard" load-replay "$TRACE" \
    --url "$URL" --rewrite-session-ids \
    --kind "$kind" --concurrency "$concurrency" --iterations "$iterations" --warmup 20 2>&1 \
    | grep -E "throughput|p50|p90|p95|p99|error_rate"
}

run execute 1 300
run execute "$CONC" "$ITERS"
run discover "$CONC" "$ITERS"

echo ""
echo "==> stopping proxy"
pkill -x mallard 2>/dev/null || true
