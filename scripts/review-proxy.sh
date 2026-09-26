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

listener_pid() {
  ss -ltnp 2>/dev/null | grep ":$PORT " | grep -oP 'pid=\K[0-9]+' | head -1
}

# The port could be held by something that is not ours: never reuse or kill a
# process the wrapper did not start (plan 051 review).
owns_port() {
  local pid
  pid="$(listener_pid)"
  [ -n "$pid" ] && tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null | grep -q mallard
}

case "${1:-}" in
  start)
    if serving; then
      if owns_port; then
        echo "review proxy already serving on $PORT"
        exit 0
      fi
      echo "port $PORT is served by something that is not the review proxy — refusing to reuse it" >&2
      exit 1
    fi
    if [ ! -x "$BINARY" ]; then
      echo "review proxy: no binary at target/{release,debug}/mallard — build first" >&2
      exit 1
    fi
    # A stale binary reviews the wrong code: say so loudly rather than serving
    # a verdict on uncommitted work (plan 051 review).
    stale="$(find "$REPO_ROOT/src" -name '*.rs' -newer "$BINARY" -print -quit 2>/dev/null)"
    if [ -n "$stale" ]; then
      echo "WARNING: $BINARY is older than $stale — build first (cargo build --release), or this review tests stale code" >&2
    fi
    mkdir -p "$(dirname "$LOG")"
    cd "$REPO_ROOT" || exit 1
    PROXY_CONFIG="$CONFIG" BIND_ADDRESS="0.0.0.0:$PORT" MALLARDCUBE_ALLOW_ANONYMOUS=1 \
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
    if ! owns_port; then
      echo "no review proxy (mallard) listening on $PORT — nothing stopped"
      exit 0
    fi
    pid="$(listener_pid)"
    kill "$pid" 2>/dev/null && echo "stopped review proxy (pid $pid)"
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
