#!/usr/bin/env python3
"""Validate every contract fixture against the checked-in JSON Schema (plan 057-A).

Usage:  python3 scripts/contract_check.py
Exit:   0 when every contracts/*/contract.yaml matches schema/contract-0.1.json,
        1 otherwise (with each violation named).
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCHEMA = ROOT / "schema" / "contract-0.1.json"
CONTRACTS = ROOT / "contracts"


def main() -> int:
    try:
        import jsonschema
        import yaml
    except ImportError as error:  # pragma: no cover - CI installs them
        print(f"contract check needs jsonschema and pyyaml: {error}")
        return 1

    schema = json.loads(SCHEMA.read_text())
    # A schema that does not compile would silently pass everything.
    jsonschema.Draft202012Validator.check_schema(schema)
    validator = jsonschema.Draft202012Validator(schema)

    fixtures = sorted(CONTRACTS.glob("*/contract.yaml"))
    if not fixtures:
        print("no contract fixtures found under contracts/")
        return 1

    failed = 0
    for fixture in fixtures:
        contract = yaml.safe_load(fixture.read_text())
        errors = sorted(validator.iter_errors(contract), key=lambda e: list(e.path))
        if errors:
            failed += 1
            print(f"FAIL {fixture.relative_to(ROOT)}")
            for error in errors[:10]:
                where = ".".join(str(part) for part in error.path) or "(root)"
                print(f"     {where}: {error.message}")
        else:
            print(f"PASS {fixture.relative_to(ROOT)}")

    print()
    if failed:
        print(f"CONTRACT FAILED: {failed} of {len(fixtures)} fixture(s) invalid")
        return 1
    print(f"CONTRACT OK: {len(fixtures)} fixture(s) match {SCHEMA.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
