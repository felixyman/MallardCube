#!/usr/bin/env python3
"""Contract conformance gate (plan 057-A).

Two jobs:

1. Every ``contracts/*/contract.yaml`` fixture matches ``schema/contract-0.1.json``
   and validates in the Rust validator.
2. A small conformance corpus: each case declares the verdict both gates must
   give. The validator is the enforcement, so the schema must never refuse a
   contract the validator accepts (schema-refused => validator-refused); cases
   only the validator can see (duplicate mapping keys, cross-block rules) are
   marked ``any`` on the schema side.

Usage:  python3 scripts/contract_check.py [--binary target/release/mallard]
Exit:   0 when every fixture and case passes, 1 otherwise (each violation named).
"""

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCHEMA = ROOT / "schema" / "contract-0.1.json"
CONTRACTS = ROOT / "contracts"

VALID_MINIMAL = """\
contract_version: "0.1.0"
model: { name: demo }
grain:
  - { table: fact_orders, key: order_id }
dimensions:
  - { id: Date, table: dim_date, key: date_key, attribute: full_date, date_role: true,
      levels: [ { name: Full Date, column: full_date } ] }
relationships:
  - { fact: fact_orders, dimension: Date, columns: [order_date_key, date_key] }
measures:
  - { id: Revenue, source: { table: fact_orders, column: revenue }, aggregation: sum }
provenance: { source_system: manual, generator: test/0.1.0 }
"""

# (name, yaml, schema verdict, validator verdict); "any" = this gate cannot see
# the rule. Every schema "refuse" must be a validator "refuse" too.
CASES = [
    ("valid minimal", VALID_MINIMAL, "accept", "accept"),
    (
        "unknown core field",
        VALID_MINIMAL.replace(
            "model: { name: demo }", 'model: { name: demo, sql: "SELECT 1" }'
        ),
        "refuse",
        "refuse",
    ),
    (
        # PyYAML is last-wins and the schema is structurally blind to
        # duplicates; only the validator's document walk sees them.
        "duplicate mapping key",
        VALID_MINIMAL.replace("aggregation: sum", "aggregation: sum\n    aggregation: max"),
        "any",
        "refuse",
    ),
    ("newer version", VALID_MINIMAL.replace('"0.1.0"', '"0.2.0"'), "refuse", "refuse"),
    ("malformed version", VALID_MINIMAL.replace('"0.1.0"', '"0.1"'), "refuse", "refuse"),
    (
        "many_to_many",
        VALID_MINIMAL.replace(
            "columns: [order_date_key, date_key]",
            "columns: [order_date_key, date_key], cardinality: many_to_many",
        ),
        "refuse",
        "refuse",
    ),
    (
        "empty expression",
        VALID_MINIMAL.replace("aggregation: sum", 'aggregation: sum, expression: ""'),
        "refuse",
        "refuse",
    ),
    (
        "empty source reference",
        VALID_MINIMAL.replace("column: revenue", 'reference: ""'),
        "refuse",
        "refuse",
    ),
    (
        "empty valid_grain",
        VALID_MINIMAL.replace("aggregation: sum", "aggregation: sum, valid_grain: []"),
        "refuse",
        "refuse",
    ),
    (
        "empty serving id",
        VALID_MINIMAL.replace("key: order_id", 'key: order_id, id: ""'),
        "refuse",
        "refuse",
    ),
    (
        "empty measure group",
        VALID_MINIMAL.replace("key: order_id", 'key: order_id, measure_group: ""'),
        "refuse",
        "refuse",
    ),
    (
        "sum without a source",
        VALID_MINIMAL.replace("column: revenue", 'column: ""'),
        "refuse",
        "refuse",
    ),
    (
        "ratio without an expression",
        VALID_MINIMAL.replace("column: revenue }, aggregation: sum", "}, aggregation: ratio"),
        "refuse",
        "refuse",
    ),
    (
        "additive valid_grain",
        VALID_MINIMAL.replace("aggregation: sum", "aggregation: sum, valid_grain: [order_id]"),
        "refuse",
        "refuse",
    ),
    (
        "window aggregation without a window",
        VALID_MINIMAL.replace("aggregation: sum", "aggregation: time_window"),
        "refuse",
        "refuse",
    ),
    (
        # Cross-block rule the schema cannot express.
        "window without a flag catalogue",
        VALID_MINIMAL.replace(
            "aggregation: sum",
            "aggregation: sum, time_window: { dimension: Date, flag: ytd_flag }",
        ),
        "any",
        "refuse",
    ),
    (
        "short join pair",
        VALID_MINIMAL.replace(
            "columns: [order_date_key, date_key]", "columns: [order_date_key]"
        ),
        "refuse",
        "refuse",
    ),
    (
        "empty grain",
        VALID_MINIMAL.replace(
            "grain:\n  - { table: fact_orders, key: order_id }", "grain: []"
        ),
        "refuse",
        "refuse",
    ),
    (
        # The schema cannot tie a date role's leaf level to its attribute.
        "date leaf mismatch",
        VALID_MINIMAL.replace("attribute: full_date", "attribute: date_key"),
        "any",
        "refuse",
    ),
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary",
        default=None,
        help="mallard binary for the validator side (default: target/release/mallard)",
    )
    args = parser.parse_args()

    try:
        import jsonschema
        import yaml
    except ImportError as error:  # pragma: no cover - CI installs them
        print(f"contract check needs jsonschema and pyyaml: {error}")
        return 1

    schema = json.loads(SCHEMA.read_text())
    # A schema that does not compile would silently pass everything.
    jsonschema.Draft202012Validator.check_schema(schema)
    schema_validator = jsonschema.Draft202012Validator(schema)

    binary = Path(args.binary) if args.binary else ROOT / "target" / "release" / "mallard"
    if not binary.exists():
        binary = None
        print(f"note: {args.binary or 'target/release/mallard'} not found; validator checks skipped")

    def validator_verdict(text: str) -> str:
        with tempfile.NamedTemporaryFile(
            "w", suffix=".yaml", delete=False, encoding="utf-8"
        ) as handle:
            handle.write(text)
            path = handle.name
        result = subprocess.run(
            [str(binary), "contract", "validate", path], capture_output=True, text=True
        )
        Path(path).unlink(missing_ok=True)
        if result.returncode == 0:
            return "accept"
        if result.returncode == 1:
            return "refuse"
        raise RuntimeError(f"validator crashed on {path}: {result.stderr.strip()}")

    def schema_verdict(text: str) -> str:
        try:
            document = yaml.safe_load(text)
        except yaml.YAMLError:
            return "refuse"
        return "refuse" if list(schema_validator.iter_errors(document)) else "accept"

    failed = 0
    fixtures = sorted(CONTRACTS.glob("*/contract.yaml"))
    if not fixtures:
        print("no contract fixtures found under contracts/")
        return 1
    for fixture in fixtures:
        contract = yaml.safe_load(fixture.read_text())
        errors = sorted(schema_validator.iter_errors(contract), key=lambda e: list(e.path))
        if errors:
            failed += 1
            print(f"FAIL {fixture.relative_to(ROOT)}")
            for error in errors[:10]:
                where = ".".join(str(part) for part in error.path) or "(root)"
                print(f"     {where}: {error.message}")
            if len(errors) > 10:
                print(f"     ... and {len(errors) - 10} more")
        elif binary is not None and validator_verdict(fixture.read_text()) != "accept":
            failed += 1
            print(f"FAIL {fixture.relative_to(ROOT)}: the validator refuses a schema-valid fixture")
        else:
            print(f"PASS {fixture.relative_to(ROOT)}")

    for name, text, schema_expect, validator_expect in CASES:
        from_schema = schema_verdict(text)
        from_validator = validator_verdict(text) if binary is not None else validator_expect
        if schema_expect != "any" and from_schema != schema_expect:
            failed += 1
            print(f"FAIL case '{name}': schema expected {schema_expect}, got {from_schema}")
        if binary is not None and from_validator != validator_expect:
            failed += 1
            print(
                f"FAIL case '{name}': validator expected {validator_expect}, got {from_validator}"
            )
        if from_schema == "refuse" and from_validator == "accept":
            failed += 1
            print(f"FAIL case '{name}': schema-refused but validator-accepted (weaker gate)")

    print()
    if failed:
        print(f"CONTRACT FAILED: {failed} finding(s)")
        return 1
    gates = f"schema + validator ({binary})" if binary is not None else "schema only"
    print(
        f"CONTRACT OK: {len(fixtures)} fixture(s) match {SCHEMA.name}, "
        f"{len(CASES)} corpus case(s) agree ({gates})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
