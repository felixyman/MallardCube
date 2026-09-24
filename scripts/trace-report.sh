#!/usr/bin/env bash
# Summarise an xmla-trace.jsonl: request kinds, faulted responses, and the
# restriction names each rowset was asked with. Read-only.
#
# Usage:
#   bash scripts/trace-report.sh [trace.jsonl]     default: xmla-trace.jsonl
set -u

trace="${1:-xmla-trace.jsonl}"
if [ ! -f "$trace" ]; then
    echo "no such trace: $trace" >&2
    exit 1
fi

python3 - "$trace" <<'PY'
import collections
import json
import re
import sys

path = sys.argv[1]
records = [json.loads(line) for line in open(path) if line.strip()]
kinds = collections.Counter(record.get("request_kind") or "?" for record in records)
faults = sum(1 for record in records if "<faultstring>" in (record.get("response_xml") or ""))
faults_by_kind = collections.Counter(
    record.get("request_kind") or "?"
    for record in records
    if "<faultstring>" in (record.get("response_xml") or "")
)


def restriction_names(block):
    """Names at depth 1 of a RestrictionList; nested <Value> children are
    values, not names (Excel's DISCOVER_PROPERTIES form)."""
    depth = 0
    out = []
    for match in re.finditer(r"<(/?)([A-Za-z_][A-Za-z0-9_]*)([^>]*)>", block):
        closing, tag, attrs = match.group(1), match.group(2), match.group(3)
        self_closing = attrs.rstrip().endswith("/")
        if not closing and depth == 0:
            if tag != "RestrictionList":
                out.append(tag)
            if not self_closing:
                depth = 1
        elif not closing and depth >= 1:
            if not self_closing:
                depth += 1
        elif closing:
            depth = max(0, depth - 1)
    return out


pairs = collections.Counter()
for record in records:
    xml = record.get("request_xml") or ""
    request_type = re.search(r"<RequestType>([A-Z_]+)</RequestType>", xml)
    if not request_type:
        continue
    for block in re.findall(r"<RestrictionList>(.*?)</RestrictionList>", xml, re.S):
        for name in restriction_names(block):
            pairs[(request_type.group(1), name)] += 1

nested_form = sum(
    1
    for record in records
    if re.search(r"<restriction[ >/]", record.get("request_xml") or "")
)
print(f"{path}: {len(records)} requests, {faults} faulted responses")
if nested_form:
    print(f"nested <restriction> requests: {nested_form} (the reference rejects this form)")
print("kinds:", dict(kinds.most_common()))
if faults_by_kind:
    print("faults by kind:", dict(faults_by_kind.most_common()))
print(f"restriction names ({len(pairs)} distinct rowset/name pairs):")
for (rowset, name), count in sorted(pairs.items()):
    print(f"  {rowset:32s} {name:28s} {count}")
PY
