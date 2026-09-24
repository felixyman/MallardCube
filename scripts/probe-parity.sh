#!/usr/bin/env bash
# Replay the parity catalog (parity/catalog.json) against a running proxy.
#
# Usage:
#   bash scripts/probe-parity.sh [proxy_url]   default http://127.0.0.1:8080/xmla
#
# Every case's expected values were recorded from the real SSAS 2025 tabular
# mirror on the VM (see parity/README.md). Deterministic and VM-free: this is
# the regression gate for Excel-facing shapes that unit tests do not cover.
# Add a case whenever a review or VM session finds a proxy/reference
# difference — record the mirror's value, not the proxy's.
set -u

here="$(cd "$(dirname "$0")" && pwd)"
exec python3 "$here/parity_check.py" "$@"
