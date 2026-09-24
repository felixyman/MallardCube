#!/usr/bin/env bash
# The reviewer's own proxy, for a review agent that must not have broad shell
# permissions. Project, port and log path are fixed; the agent only needs
# `bash scripts/review-proxy.sh start|status|stop` (covered by the
# `bash scripts/*` rule in .opencode/agents/reviewer.md).
#
# Usage:
#   bash scripts/review-proxy.sh start     start on 0.0.0.0:8099, or report that it is already up
#   bash scripts/review-proxy.sh status    is it serving?
#   bash scripts/review-proxy.sh stop      stop the proxy listening on 8099
#
# The demo project is deliberate: reviews compare proxy behaviour against the
# documented demo values, and it needs no external data file.

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PORT=8099
LOG=/tmp/opencode/review-proxy.log
CONFIG="$REPO_ROOT/projects/project3/proxy-config.json"

BINARY="$REPO_ROOT/target/release/mallard"
[ -x "$BINARY" ] || BINARY="$REPO_ROOT/target/debug/mallard"

serving() {
  curl -s -m 2 "http://127.0.0.1:$PORT/status" >/dev/null 2>&1
}

case "${1:-}" in
  start)
    if serving; then
      echo "review proxy already serving on $PORT"
      exit 0
    fi
    if [ ! -x "$BINARY" ]; then
      echo "review proxy: no binary at target/{release,debug}/mallard — build first" >&2
      exit 1
    fi
    mkdir -p "$(dirname "$LOG")"
    cd "$REPO_ROOT" || exit 1
    PROXY_CONFIG="$CONFIG" BIND_ADDRESS="0.0.0.0:$PORT" \
      setsid nohup "$BINARY" serve > "$LOG" 2>&1 < /dev/null &
    for _ in $(seq 1 30); do
      if serving; then
        echo "review proxy ready on $PORT (binary $BINARY, log $LOG)"
        exit 0
      fi
      sleep 0.5
    done
    echo "review proxy did not become ready; see $LOG" >&2
    tail -5 "$LOG" >&2 2>/dev/null
    exit 1
    ;;
  stop)
    pid="$(ss -ltnp 2>/dev/null | grep ":$PORT " | grep -oP 'pid=\K[0-9]+' | head -1)"
    if [ -n "$pid" ]; then
      kill "$pid" 2>/dev/null && echo "stopped review proxy (pid $pid)"
    else
      echo "no review proxy listening on $PORT"
    fi
    ;;
  status)
    if serving; then
      echo "review proxy serving on $PORT"
    else
      echo "review proxy not serving on $PORT" >&2
      exit 1
    fi
    ;;
  *)
    echo "usage: $0 start|status|stop" >&2
    exit 2
    ;;
esac
