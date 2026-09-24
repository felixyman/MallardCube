#!/usr/bin/env python3
"""Replay the parity catalog (parity/catalog.json) against a running proxy.

Each case's expected observations were recorded from the real SSAS 2025
tabular mirror on the VM. This is the deterministic half of "capture on the
VM, assert everywhere": no Excel, no desktop, no VM needed to check the
Excel-facing shapes that unit tests do not cover.

Usage:
  python3 scripts/parity_check.py [url]     # default http://127.0.0.1:8080/xmla

Exit code 0 when every case matches, 1 otherwise. Add a case whenever a review
or a VM session finds a proxy/reference difference — record the *mirror's*
value, not the proxy's.
"""

from __future__ import annotations

import json
import re
import sys
import urllib.error
import urllib.request
from html import unescape
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CATALOG = ROOT / "parity" / "catalog.json"


def soap_discover(catalog: str, request_type: str, restrictions: dict[str, str]) -> str:
    items = "".join(f"<{key}>{value}</{key}>" for key, value in restrictions.items())
    return (
        '<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body>'
        '<Discover xmlns="urn:schemas-microsoft-com:xml-analysis">'
        f"<RequestType>{request_type}</RequestType>"
        f"<Restrictions><RestrictionList>{items}</RestrictionList></Restrictions>"
        f"<Properties><PropertyList><Catalog>{catalog}</Catalog></PropertyList></Properties>"
        "</Discover></soap:Body></soap:Envelope>"
    )


def soap_execute(catalog: str, mdx: str) -> str:
    return (
        '<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body>'
        '<Execute xmlns="urn:schemas-microsoft-com:xml-analysis">'
        f"<Command><Statement>{mdx}</Statement></Command>"
        f"<Properties><PropertyList><Catalog>{catalog}</Catalog></PropertyList></Properties>"
        "</Execute></soap:Body></soap:Envelope>"
    )


def post(url: str, body: str) -> str:
    request = urllib.request.Request(
        url, data=body.encode(), headers={"Content-Type": "text/xml"}
    )
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            return response.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as error:
        # A SOAP fault can arrive with a 4xx/5xx status; read the body so an
        # expected-fault case can still compare the message.
        return error.read().decode("utf-8", "replace")


def tag(block: str, name: str) -> str | None:
    match = re.search(rf"<{name}>(.*?)</{name}>", block, re.S)
    return unescape(match.group(1)) if match else None


def axis_unames(xml: str, axis: str) -> list[str]:
    start = xml.find(f'<Axis name="{axis}">')
    if start < 0:
        return []
    end = xml.find("</Axis>", start)
    block = xml[start : end if end >= 0 else len(xml)]
    return [unescape(name) for name in re.findall(r"<Member[^>]*>.*?<UName>(.*?)</UName>", block, re.S)]


def observe_discover(xml: str, case: dict) -> dict:
    rows = re.findall(r"<row>(.*?)</row>", xml, re.S)
    observed: dict = {"row_count": len(rows)}
    for alias, column in case.get("columns", {}).items():
        values = [tag(row, column) for row in rows]
        values = [value for value in values if value is not None]
        observed[f"{alias}_first"] = values[0] if values else None
        observed[f"{alias}_last"] = values[-1] if values else None
        observed[f"{alias}_sorted"] = sorted(values)
    for alias, column in case.get("single", {}).items():
        observed[alias] = tag(rows[0], column) if rows else None
    return observed


def observe_execute(xml: str) -> dict:
    # Only the CellData block: OlapInfo's CellInfo carries self-closing
    # `<Value name="..."/>` elements that would otherwise swallow the match.
    cell_data = re.search(r"<CellData>(.*?)</CellData>", xml, re.S)
    cells = cell_data.group(1) if cell_data else ""
    observed: dict = {
        "cell_count": len(re.findall(r"<Cell[ >]", cells)),
        "cell_values": [
            unescape(value).strip()
            for value in re.findall(r"<Value[^>]*>(.*?)</Value>", cells, re.S)
        ],
    }
    for index in (0, 1):
        members = axis_unames(xml, f"Axis{index}")
        if members:
            observed[f"axis{index}_member_count"] = len(members)
            observed[f"axis{index}_first_member"] = members[0]
            observed[f"axis{index}_last_member"] = members[-1]
    return observed


def normalise(value):
    """Numbers compare numerically, so 521586767 and 521586767.0 are equal."""
    if isinstance(value, str):
        try:
            return round(float(value), 6)
        except ValueError:
            return value
    if isinstance(value, list):
        return [normalise(item) for item in value]
    return value


def compare(expected: dict, observed: dict) -> list[tuple[str, object, object]]:
    mismatches = []
    for key, want in expected.items():
        got = observed.get(key)
        if normalise(want) != normalise(got):
            mismatches.append((key, want, got))
    return mismatches


def main() -> int:
    url = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:8080/xmla"
    catalog = json.loads(CATALOG.read_text())
    catalog_name = catalog["catalog"]
    cube = catalog["cube"]

    passed = failed = known = 0
    for case in catalog["cases"]:
        if not case.get("expect"):
            failed += 1
            print(f"FAIL {case['id']}")
            print("     case has no expectations — a vacuous pass is not allowed")
            continue
        request = case["request"]
        if case["kind"] == "discover":
            restrictions = {
                key: value.replace("{catalog}", catalog_name).replace("{cube}", cube)
                for key, value in request.get("restrictions", {}).items()
            }
            body = soap_discover(catalog_name, request["type"], restrictions)
        else:
            body = soap_execute(catalog_name, request["mdx"].replace("{cube}", cube))

        try:
            xml = post(url, body)
        except Exception as error:  # transport failures are a failed case, not a crash
            observed = {"error": str(error)}
        else:
            fault = case["expect"].get("fault")
            if fault:
                message = re.search(r"<faultstring[^>]*>(.*?)</faultstring>", xml, re.S)
                text = unescape(message.group(1)) if message else ""
                observed = {"fault": text if isinstance(fault, str) else bool(message)}
            else:
                observed = (
                    observe_discover(xml, case)
                    if case["kind"] == "discover"
                    else observe_execute(xml)
                )

        if "error" in observed:
            # A transport failure is never a documented gap: the proxy was not
            # even reachable, so nothing was observed.
            failed += 1
            print(f"FAIL {case['id']}")
            print(f"     transport: {observed['error']}")
            continue

        mismatches = compare(case["expect"], observed)
        if mismatches and case.get("known_gap"):
            # `expect` is always the reference's value. A mismatch on a case
            # that documents a known gap is reported, not failed — and when a
            # gap is closed the case matches and says so, so it gets promoted.
            known += 1
            print(f"KNOWN {case['id']}")
            for key, want, got in mismatches:
                print(f"     {key}: reference {want!r}, proxy {got!r}")
            print(f"     GAP: {case['known_gap']}")
        elif mismatches:
            failed += 1
            print(f"FAIL {case['id']}")
            for key, want, got in mismatches:
                print(f"     {key}: expected {want!r}, got {got!r}")
        else:
            passed += 1
            print(f"PASS {case['id']}")
            if case.get("known_gap"):
                print(
                    "     NOTE: this known gap no longer reproduces — "
                    "drop known_gap from the case"
                )

    total = passed + failed
    suffix = f" ({known} known gap{'s' if known != 1 else ''})" if known else ""
    print(
        f"\nPARITY {'OK' if failed == 0 else 'FAILED'}: {passed}/{total} cases matched{suffix}"
    )
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
