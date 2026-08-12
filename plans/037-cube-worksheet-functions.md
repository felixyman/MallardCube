# Plan 037: CUBE worksheet function support — CUBEVALUE, CUBEMEMBER, CUBESET

## Status

- **Priority**: P2 (feature completeness)
- **Effort**: S (was XS; strtomember handling pushed it up)
- **Risk**: LOW
- **Depends on**: none
- **Category**: compatibility
- **Status**: DONE

## Why this matters

CUBE functions (`CUBEVALUE`, `CUBEMEMBER`, `CUBESET`, `CUBERANKEDMEMBER`,
`CUBESETCOUNT`) let analysts build free-form Excel reports against SSAS data
without a PivotTable. These are worksheet formulas — not PivotTable fields —
and are heavily used in finance and operations teams for formatted, print-ready
reports where the PivotTable layout system is too rigid.

Every SSAS Tabular model supports these out of the box. MallardCube already has
the hard parts (XMLA DISCOVER handshake, MDX execution, member enumeration).
The missing pieces are three minor DISCOVER rowsets that Excel's CUBE function
connection workflow requires before it will accept the server as a valid data
source.

## Current state

All CUBE function operations map to existing MallardCube handlers:

| CUBE function | XMLA request | MallardCube handler |
|---|---|---|
| CUBEVALUE | `Execute` with MDX slicer query | `SlicerAllAndMeasure` semantic path |
| CUBEMEMBER | `MDSCHEMA_MEMBERS` with filter | `get_members_response_with_backend` |
| CUBESET | `MDSCHEMA_MEMBERS` with TREE_OP | Same, with `tree_op` parameter |
| CUBESETCOUNT | Client-side (Excel) | No server involvement |
| CUBERANKEDMEMBER | Client-side (Excel) | No server involvement |

Three DISCOVER rowsets are listed in `schema_rowsets.rs` (so they appear in
`DISCOVER_SCHEMA_ROWSETS` output) but have no parser routing or handler:

| Rowset | Line in schema_rowsets | Parser routing | Handler |
|---|---|---|---|
| `DISCOVER_ENUMERATORS` | 347 | Missing | Missing |
| `DISCOVER_KEYWORDS` | 353 | Missing | Missing |
| `DISCOVER_DATASOURCES` | 325 | Missing | Missing |

Without these, Excel's CUBE function connection workflow rejects the server
as an incomplete XMLA provider — even though all the actual data operations
work.

## Design

### Template for each handler

All three follow the same pattern: return a thin XML rowset with one or a few
hardcoded rows describing the server's capabilities. No per-model data, no
database queries.

```rust
// In src/xmla/discover/ (new or existing file)

pub fn get_enumerators_response() -> String {
    let rows = r#"
          <row>
            <EnumName>MDSCHEMA_CUBES</EnumName>
            <EnumDescription>Cube schema rowset</EnumDescription>
            <EnumType>1</EnumType>
          </row>
          <row>
            <EnumName>DBSCHEMA_CATALOGS</EnumName>
            ...
          </row>"#;
    // One row per supported schema/discover request type
    crate::response::discover_rowset_envelope("", ENUMERATOR_FIELDS, rows)
}
```

### DISCOVER_ENUMERATORS

Returns one `<row>` per schema/discover type the server supports. The field
list mirrors the standard SSAS enumerator schema. We return rows for every
DISCOVER and DBSCHEMA/MDSCHEMA type we currently handle.

### DISCOVER_KEYWORDS

Returns one `<row>` per reserved keyword in the XMLA specification. This
is a static list — all SSAS instances return the same keywords. We return
a minimal set covering the keywords present in our MDX parser (SELECT,
FROM, WHERE, NON, EMPTY, etc.).

### DISCOVER_DATASOURCES

Returns one `<row>` for the local server instance. Standard SSAS returns
the machine name and instance name. We return `localhost` or the binding
address.

### Parser changes

Add three variants to `XmlaRequest`:

```rust
// In src/xmla/parser.rs
XmlaRequest::DiscoverEnumerators,
XmlaRequest::DiscoverKeywords,
XmlaRequest::DiscoverDatasources,
```

Add routing in the `match parsed_request_type.as_str()` block:

```rust
"DISCOVER_ENUMERATORS" => return XmlaRequest::DiscoverEnumerators,
"DISCOVER_KEYWORDS" => return XmlaRequest::DiscoverKeywords,
"DISCOVER_DATASOURCES" => return XmlaRequest::DiscoverDatasources,
```

### Route_request changes

Add three arms in `src/main.rs`:

```rust
XmlaRequest::DiscoverEnumerators => {
    let resp = enumerators::get_enumerators_response();
    xmla_proxy::xmla_trace::trace_request("DiscoverEnumerators", body, &resp, None, None);
    resp
}
// ... same pattern for Keywords and Datasources
```

### XMLA trace support

Add the three new request types to the trace replay's recognized list in
`src/tools/trace_replay.rs`:

```rust
| "DISCOVER_ENUMERATORS" | "DISCOVER_KEYWORDS" | "DISCOVER_DATASOURCES"
```

## Scope

**In scope:**
- `DiscoverEnumerators` / `DiscoverKeywords` / `DiscoverDatasources` request
  parsing and routing
- Three rowset handler functions returning standard SSAS-compatible XML
- Trace replay recognition of the three new request types

**Out of scope:**
- CUBE function Excel testing (requires Windows VM — test manually after deploy)
- Dynamic keyword list (static list is sufficient)
- `DISCOVER_INSTANCES` (not required by CUBE functions in practice)
- `DISCOVER_CSDL_METADATA` content (we return an empty rowset stub; CUBE
  functions don't need CSDL)

## Test plan

- Verify that the three new request types parse correctly (add a unit test
  to `xmla::parser` tests)
- Verify that each handler returns valid XML with `<row>` elements
- Verify trace replay processes entries containing the new request types
  without error
- Manual: connect Excel via ODC, confirm `=CUBEMEMBER("connection", ...)`
  resolves member captions; `=CUBEVALUE("connection", ...)` returns values

## Done criteria

- [ ] `DISCOVER_ENUMERATORS` request returns enumerated schema list
- [ ] `DISCOVER_KEYWORDS` request returns keyword list
- [ ] `DISCOVER_DATASOURCES` request returns at least one data source
- [ ] All three parse and route correctly (parser test passes)
- [ ] Trace replay recognizes all three request types
- [ ] Existing test suite unaffected (324 tests green)
