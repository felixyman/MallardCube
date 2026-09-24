#!/usr/bin/env bash
# Replay an xmla-trace.jsonl against a running proxy and compare the
# fault/no-fault split with the recorded responses. Read-only.
#
# Usage:
#   bash scripts/trace-replay.sh [url] [trace.jsonl]
# Defaults: http://127.0.0.1:8099/xmla and xmla-trace.jsonl
set -u

url="${1:-http://127.0.0.1:8099/xmla}"
trace="${2:-xmla-trace.jsonl}"
if [ ! -f "$trace" ]; then
    echo "no such trace: $trace" >&2
    exit 1
fi

python3 - "$url" "$trace" <<'PY'
import collections
import json
import sys
import urllib.request

url, path = sys.argv[1], sys.argv[2]
counts = collections.Counter()
interesting = []

for line in open(path):
    line = line.strip()
    if not line:
        continue
    record = json.loads(line)
    body = record.get("request_xml") or ""
    if not body:
        continue
    recorded_fault = "<faultstring>" in (record.get("response_xml") or "")
    request = urllib.request.Request(
        url, data=body.encode(), headers={"Content-Type": "text/xml"}
    )
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            text = response.read().decode("utf-8", "replace")
    except Exception as error:
        counts["transport error"] += 1
        interesting.append(("transport", str(error)[:100], body[:140]))
        continue

    now_fault = "<faultstring>" in text
    if "not recognized by the server" in text:
        counts["contract fault"] += 1
        interesting.append(("contract", text[:140], body[:140]))
    elif now_fault and not recorded_fault:
        counts["new fault"] += 1
        interesting.append(("new", text[:140], body[:140]))
    elif recorded_fault and not now_fault:
        counts["fault disappeared"] += 1
    elif now_fault:
        counts["fault (as recorded)"] += 1
    else:
        counts["ok"] += 1

print(f"replayed {sum(counts.values())} requests against {url}")
print(dict(counts))
for kind, response, request in interesting[:10]:
    print(f"--- {kind}: {request}")
    print(f"    {response}")
PY
