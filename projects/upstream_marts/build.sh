#!/usr/bin/env bash
# Build the upstream demo database from the SQL files (plan 044).
#
#   bash projects/upstream_marts/build.sh
#   PROXY_CONFIG=projects/upstream_marts/proxy-config.yaml cargo run
set -euo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"
DB="$DIR/data/upstream_marts.duckdb"

command -v duckdb >/dev/null || { echo "duckdb CLI required" >&2; exit 1; }

mkdir -p "$DIR/data"
rm -f "$DB"

for file in schema seed marts; do
    echo "==> upstream/$file.sql"
    duckdb "$DB" < "$DIR/upstream/$file.sql"
done

echo "built $DB"
