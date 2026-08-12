# Cellset (mddataset) Reference

This documents the exact cellset XML format that Excel's MSOLAP client accepts,
derived from trial/error against the [MS-SSAS] spec, Section 4.13.

## Namespace

The ExecuteResponse wraps a `<root>` in the mddataset namespace:

```xml
<root xmlns="urn:schemas-microsoft-com:xml-analysis:mddataset"
      xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
      xmlns:xsd="http://www.w3.org/2001/XMLSchema">
```

**Do NOT use `rowset` namespace** — that is for tabular/flat results only.
Cellset responses MUST use `mddataset`.

## Required child elements of `<root>`

In order:
1. `<xsd:schema>` — inline schema (can be minimal, see below)
2. `<OlapInfo>` — metadata about axes and cells
3. `<Axes>` — axis tuple data
4. `<CellData>` — cell values

## Minimal xsd:schema

```xml
<xsd:schema targetNamespace="urn:schemas-microsoft-com:xml-analysis:mddataset"
             elementFormDefault="qualified"
             xmlns="urn:schemas-microsoft-com:xml-analysis:mddataset">
  <xsd:element name="root">
    <xsd:complexType>
      <xsd:sequence>
        <xsd:any namespace="http://www.w3.org/2001/XMLSchema"
                 processContents="strict" minOccurs="0"/>
        <xsd:element name="OlapInfo" minOccurs="0"/>
        <xsd:element name="Axes" minOccurs="0"/>
        <xsd:element name="CellData" minOccurs="0"/>
      </xsd:sequence>
    </xsd:complexType>
  </xsd:element>
</xsd:schema>
```

Critical: `<xsd:schema>` must have `xmlns="urn:...mddataset"` (default namespace).
The schema's own `xsd:` prefix comes from `http://www.w3.org/2001/XMLSchema`.

## OlapInfo structure

### CubeInfo
```xml
<CubeInfo>
  <Cube>
    <CubeName>Model</CubeName>
  </Cube>
</CubeInfo>
```

### AxesInfo — HierarchyInfo property declarations

**This was the hardest part.** The `<HierarchyInfo>` children are NOT member data.
They are **property declarations** that tell MSOLAP what properties will appear
inside each `<Member>` element later. Each declaration has `name` and `type` attributes.

**Every property that appears as a child element of `<Member>` MUST be pre-declared
here.** Undeclared member children cause `MDDSAxis::MoveToHierProperty` crash.

Standard 5 (always present):

| Element tag | `name` attribute |
|---|---|
| `<UName>` | `[Hier].[MEMBER_UNIQUE_NAME]` |
| `<Caption>` | `[Hier].[MEMBER_CAPTION]` |
| `<LName>` | `[Hier].[LEVEL_UNIQUE_NAME]` |
| `<LNum>` | `[Hier].[LEVEL_NUMBER]` |
| `<DisplayInfo>` | `[Hier].[DISPLAY_INFO]` |

Qualified name format: `[HierarchyUniqueName].[PROPERTY_SUFFIX]`

Example:
```xml
<HierarchyInfo name="[Produktkategori].[Produktkategori]">
  <UName name="[Produktkategori].[Produktkategori].[MEMBER_UNIQUE_NAME]" type="xsd:string"/>
  <Caption name="[Produktkategori].[Produktkategori].[MEMBER_CAPTION]" type="xsd:string"/>
  <LName name="[Produktkategori].[Produktkategori].[LEVEL_UNIQUE_NAME]" type="xsd:string"/>
  <LNum name="[Produktkategori].[Produktkategori].[LEVEL_NUMBER]" type="xsd:int"/>
  <DisplayInfo name="[Produktkategori].[Produktkategori].[DISPLAY_INFO]" type="xsd:unsignedInt"/>
  <!-- additional dimension properties below -->
  <PARENT_UNIQUE_NAME name="[Produktkategori].[Produktkategori].[PARENT_UNIQUE_NAME]" type="xsd:string"/>
  ...
</HierarchyInfo>
```

**Rules for dimension property declarations:**
- Do NOT duplicate qualified names. UName already declares `[MEMBER_UNIQUE_NAME]`,
  so don't also declare `<MEMBER_UNIQUE_NAME>` with the same qualified name.
  The unique name is already available via `<UName>` in the Member element.
- Same applies: Caption covers MEMBER_CAPTION, LNum covers LEVEL_NUMBER,
  LName covers LEVEL_UNIQUE_NAME.
- All other intrinsic member properties (MEMBER_NAME, MEMBER_KEY, MEMBER_TYPE,
  MEMBER_VALUE, PARENT_LEVEL, PARENT_COUNT, CHILDREN_CARDINALITY, PARENT_UNIQUE_NAME,
  HIERARCHY_UNIQUE_NAME) each get their own declaration with unique qualified names.

### CellInfo
```xml
<CellInfo>
  <Value name="VALUE"/>
  <FmtValue name="FORMATTED_VALUE" type="xsd:string"/>
  <FormatString name="FORMAT_STRING" type="xsd:string"/>
  <BackColor name="BACK_COLOR" type="xsd:string"/>
  <ForeColor name="FORE_COLOR" type="xsd:string"/>
</CellInfo>
```

Each `<Cell>` child element needs a corresponding `<CellInfo>` declaration.
Note: cell property declarations use bare property names (VALUE, FORMAT_STRING),
NOT hierarchy-qualified names.

## Axes structure

For a query with one hierarchy on columns and no explicit rows axis:

```xml
<Axes>
  <!-- Axis0: the hierarchy members from the MDX ON COLUMNS clause -->
  <Axis name="Axis0">
    <Tuples>
      <Tuple>
        <Member Hierarchy="[HierUniqueName]">
          <UName>...</UName>      <!-- member unique name -->
          <Caption>...</Caption>  <!-- display caption -->
          <LName>...</LName>      <!-- level unique name -->
          <LNum>0</LNum>          <!-- 0-based level number -->
          <DisplayInfo>3</DisplayInfo> <!-- bitmask: 3=LNum+DisplayInfo present -->
          <!-- dimension properties declared in HierarchyInfo -->
          <PARENT_UNIQUE_NAME>...</PARENT_UNIQUE_NAME>
          ...
        </Member>
      </Tuple>
    </Tuples>
  </Axis>

  <!-- SlicerAxis: default member of every dimension NOT on a query axis -->
  <Axis name="SlicerAxis">
    <Tuples>
      <Tuple>
        <Member Hierarchy="[Measures]">
          <UName>[Measures].[Total Försäljning]</UName>
          <Caption>Total Försäljning (SEK)</Caption>
          <LName>[Measures].[MeasuresLevel]</LName>
          <LNum>0</LNum>
          <DisplayInfo>3</DisplayInfo>
        </Member>
      </Tuple>
    </Tuples>
  </Axis>
</Axes>
```

### SlicerAxis requirement

**There MUST be a SlicerAxis.** It contains the default member of every dimension
NOT referenced on any query axis. For a query with only one axis (e.g.,
`SELECT ... ON COLUMNS FROM [Model]`), the SlicerAxis contains the Measures
dimension's default member (the default measure).

Without SlicerAxis, the cell data won't be properly mapped.

### `<Tuples>` vs `NormTupleSet` (decision record)

The SlicerAxis (and every axis) is emitted in the plain `<Tuples>` format above.
Excel's MSOLAP client accepts this. We deliberately do **not** use the
`NormTupleSet` optimized format for the SlicerAxis:

- `NormTupleSet` was introduced once (commit `8fdaa53`) to mirror real SSAS
  Tabular output, but it regressed both CUBEVALUE and PivotTable execution
  (Excel raised "rowset store ... null value" and stopped rendering cells).
- The plain `<Tuples>` SlicerAxis is the verified-working shape; keep it unless a
  concrete Excel trace proves `NormTupleSet` is required.

### Member element rules

1. `Hierarchy` attribute: value is the hierarchy unique name (e.g., `[Produktkategori].[Produktkategori]`)
2. Standard 5 children: UName, Caption, LName, LNum, DisplayInfo
3. Dimension property children: ONLY those declared in HierarchyInfo
4. **Child element order must match the HierarchyInfo declaration order.**
5. No `xsi:nil` or `xsi:type` on member children (they're plain text).

### DisplayInfo values

A bitmask integer:
- `0` = no extra info
- `3` = LNum is present (bit 0) + DisplayInfo is present (bit 1) — standard for regular members
- `5` = parent information present (common for All members)
- `131072` = some server-specific flags (seen on Measures members in real SSAS)

## CellData

```xml
<CellData>
  <Cell CellOrdinal="0">
    <Value xsi:type="xsd:double">1250000.5</Value>
    <FmtValue>1,250,000.50 SEK</FmtValue>
    <FormatString>#,##0.00 SEK</FormatString>
    <BackColor></BackColor>
    <ForeColor></ForeColor>
  </Cell>
</CellData>
```

### Cell rules

1. `CellOrdinal` attribute is **required** (type `xsd:unsignedInt`). Zero-based.
2. `<Value>` uses `xsi:type` for data type declaration (e.g., `xsd:double`, `xsd:int`, `xsd:decimal`).
   This IS required — earlier attempts to omit `xsi:type` caused `PFBaseString::Clear` crashes.
3. `<BackColor>` / `<ForeColor>` — use empty elements (`<BackColor></BackColor>`), NOT
   `xsi:nil="true"`. The nil attribute caused `PFBaseString::Clear` crashes.
4. Cell count = number of axis tuples (Axis0 tuples × axis without any tuples = 1 cell
   for a 1-member × 1-slicer result).
5. `<FmtValue>` is the formatted display string.

## Common crash errors and their causes

| Error | Cause |
|---|---|
| `pfshstring.cpp PFBaseString::Clear` | `xsi:nil="true"` on cell properties, missing `xsi:type` on `<Value>`, or missing `<xsd:schema>` |
| `mddsaxis.cpp MDDSAxis::MoveToHierProperty` | Member element has child elements not declared in the corresponding HierarchyInfo |
| `Empty result` / no render | Missing SlicerAxis, or HierarchyInfo uses data format instead of property declarations |

## Flat rowset vs cellset detection

When routing an Execute response, route to the correct format:

- MDX contains `DIMENSION PROPERTIES` or `CELL PROPERTIES` → **cellset** (mddataset)
- MDX starts with `SELECT ... FROM` without those → may work as flat rowset
- DAX `EVALUATE` or `DEFINE` → **flat rowset** (rowset namespace)
- Simple `SELECT FROM [Cube]` without axis clauses → flat rowset (rowset namespace)

## Summary recipe for generating a cellset from data

To generate a cellset response for N members on a single axis:

1. Build `<OlapInfo>` with CubeInfo, AxesInfo (one HierarchyInfo per hierarchy on each axis + SlicerAxis), and CellInfo
2. HierarchyInfo: declare standard 5 + any dimension properties requested, with unique qualified names
3. Axes: build one `<Tuple>` per member on Axis0, one `<Tuple>` with one `<Member>` on SlicerAxis
4. CellData: one `<Cell>` per Axis0 tuple, with the measure value and requested cell properties
5. Wrap in `<ExecuteResponse>` → `<return>` → `<root xmlns="...mddataset">`
6. Wrap in SOAP envelope via `wrap_in_soap_envelope()`

All XML element names are case-sensitive. The English character encoding works
with both ASCII and UTF-8 (Swedish characters like `ö` in `Försäljning` are fine).
