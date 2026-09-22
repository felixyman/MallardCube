---
name: ssas-reference-oracle
description: >
  Use the Windows VM's real SQL Server 2025 Analysis Services (tabular) as the
  reference engine for Excel-facing metadata questions: deploy/process a small
  model, query MDSCHEMA_* rowsets via ADODB DMVs, connect Excel to it, and read
  the pivot cache definition Excel writes. Use when the proxy's Excel behaviour
  needs a ground truth (date filters, level types, member properties, cache
  metadata) or when a metadata question cannot be answered from docs. Triggers:
  SSAS, Analysis Services, reference engine, oracle, MDSCHEMA, DMV, TMSL,
  memberValueDatatype, date filters, msmdsrv, tabular model.
---

# SSAS reference oracle (Windows VM)

The VM has a **real SSAS 2025 tabular instance** plus SQL Server 2025, SSMS 22
and Tabular Editor 2.28. Deploy a tiny model, then use it to answer "what does a
real engine/Excel do here?" — this settled the Excel date-filter question that
docs alone could not.

## Environment facts (do not rediscover)

- Service `MSSQLServerOLAPService` (msmdsrv), server **`localhost`**, edition
  `StandardDeveloper64`, default compatibility level **1700**.
- For a **tabular** model the cube name is **`Model`** (not the database name):
  MDX `FROM [Model]`, and Excel's connection `CommandText` = `Model`.
- SQL engine on the same box with database **`SSASModels`** (source tables
  `dbo.DateDim` 2020–2026, `dbo.FactSales`, `dbo.ExcelProfiler`).
- Tooling: SSMS 22, Tabular Editor `C:\Program Files (x86)\Tabular Editor\TabularEditor.exe`,
  TOM/AMO assemblies in `C:\Program Files\Microsoft SQL Server\170\DTS\Binn\`
  (`Microsoft.AnalysisServices.Tabular.dll`, `Microsoft.AnalysisServices.dll`).
- Everything is driven through the `windows` MCP server's PowerShell tool
  (see the `windows-mcp-desktop` skill). Run it in session 1.
- **SQL auth is disabled** on the engine. Use Windows auth + the SSAS service
  account for processing:
  ```sql
  IF NOT EXISTS (SELECT 1 FROM sys.server_principals WHERE name='NT Service\MSSQLServerOLAPService')
    CREATE LOGIN [NT Service\MSSQLServerOLAPService] FROM WINDOWS;
  USE SSASModels;
  CREATE USER [NT Service\MSSQLServerOLAPService] FOR LOGIN [NT Service\MSSQLServerOLAPService];
  ALTER ROLE db_datareader ADD MEMBER [NT Service\MSSQLServerOLAPService];
  ```
  Data source: `Integrated Security=SSPI` + `"impersonationMode": "impersonateServiceAccount"`.

## Deploy + process a model (the recipe that works)

Deploy with **TMSL `createOrReplace` through AMO's `Server.Execute`** — TOM's
`SaveChanges` cannot push structural changes on this box (see gotchas).

```powershell
Add-Type -Path "C:\Program Files\Microsoft SQL Server\170\DTS\Binn\Microsoft.AnalysisServices.dll"
$svr = New-Object Microsoft.AnalysisServices.Server
$svr.Connect("localhost")

$tmsl = '{ "createOrReplace": {
    "object": { "database": "MallardRef" },          # name reference only
    "database": {
      "name": "MallardRef", "compatibilityLevel": 1600,
      "model": {
        "culture": "en-US",
        "dataSources": [ { "name": "Sql", "provider": "System.Data.SqlClient",
          "connectionString": "Data Source=localhost;Initial Catalog=SSASModels;Integrated Security=SSPI;TrustServerCertificate=True",
          "impersonationMode": "impersonateServiceAccount" } ],
        "tables": [ { "name": "DateDim", "dataCategory": "Time",
          "columns": [ {"name":"FullDate","dataType":"dateTime","sourceColumn":"FullDate","formatString":"Short Date"},
                       {"name":"Year","dataType":"int64","sourceColumn":"Year"} ],
          "partitions": [ { "name":"DateDim", "source": {"type":"query","dataSource":"Sql",
            "query":"SELECT FullDate, Year FROM dbo.DateDim"} } ] } ],
        "relationships": []
      } } } }'

# AMO wraps the string in <Execute><Command>, so pass the inner <Statement>
$res = $svr.Execute('<Statement xmlns="urn:schemas-microsoft-com:xml-analysis">' + $tmsl + '</Statement>')
foreach ($m in $res.Messages) { "deploy msg: " + $m.Description }

# Process (same path)
$refresh = '{"refresh":{"type":"full","objects":[{"database":"MallardRef"}]}}'
$res2 = $svr.Execute('<Statement xmlns="urn:schemas-microsoft-com:xml-analysis">' + $refresh + '</Statement>')
foreach ($m in $res2.Messages) { "refresh msg: " + $m.Description }
```

Verify processing (do NOT trust `$db.State` — it can still read `Unprocessed`):

```powershell
$conn = New-Object -ComObject ADODB.Connection
$conn.Open('Provider=MSOLAP;Data Source=localhost;Initial Catalog=MallardRef;')
$rs = $conn.Execute('SELECT * FROM $SYSTEM.DISCOVER_STORAGE_TABLES')
while (-not $rs.EOF) { $rs.Fields.Item('TABLE_ID').Value + ' rows=' + $rs.Fields.Item('ROWS_COUNT').Value; $rs.MoveNext() }
```

A deployed reference model already exists: **`MallardRef`** (DateDim 2557 dates,
FactSales 6 rows, a `Calendar` user hierarchy Year→Quarter→Month→FullDate, a
`Revenue` measure).

## Mirror model: the proxy's model as a real tabular model

`MallardDemo` is a tabular model that mirrors the proxy's demo surface, so the
same Excel gesture can be run against both and the requests diffed. Build
recipe (all on the VM):

1. Extract the proxy's demo DuckDB (`/tmp/mallardcube-demo-<pid>-0.duckdb`,
   newest) and `COPY` `sales_fact` + `date_dim` to CSV; gzip and fetch them to
   the VM (serve them on the Linux host's port 8080 while the proxy is stopped,
   then restart the proxy).
2. Load into `SSASModels` with tables named exactly like the Excel-visible
   dimensions: `Sales` (fact), `Category`, `Territory`, `Channel`, `Segment`,
   `[Date]` (`date_key`, `[Full Date]`, `Year`, `Quarter`, `Month`) — plus
   distinct-value tables for the flat dimensions.
3. Deploy TMSL `createOrReplace` (see the recipe above) with:
   - a `Calendar` user hierarchy on `Date`: Year → Quarter → Month → Full Date;
   - helper columns (`Year`/`Quarter`/`Month`/`date_key`/fact keys) `isHidden`,
     which keeps them out of Excel's field list while still usable as levels
     and relationship keys;
   - measures `Revenue`, `Units`, `Revenue YTD/QTD/MTD` (`TOTAL*TD`),
     `Revenue Prior Year` (`SAMEPERIODLASTYEAR`), `formatString` `#,##0.00`;
   - the `Date` table `dataCategory: "Time"` for time intelligence.
   Watch out: tabular names are case-insensitive, so a fact column `revenue`
   collides with the measure `Revenue` — name the column `revenue_amt`.
4. Connect Excel through the plain-XML relay
   (`http://127.0.0.1:8090/OLAP/msmdpump.dll`, catalog `MallardDemo`).

**Capture what Excel sends to the reference** with the logging relay on the VM:
`python C:\Users\Public\Documents\parity\relay.py` listens on `127.0.0.1:8095`
and forwards to the pump on `127.0.0.1:8090`, writing `NNNN_req.xml` /
`NNNN_resp.xml` under `C:\Users\Public\Documents\relay\`. Connect Excel to
`http://127.0.0.1:8095/OLAP/msmdpump.dll` and the exact MDX (including the
`DrilldownMember(CrossJoin(...))` shape Excel uses for two row fields) is in the
log. Compare engines on the **raw cellset XML** (`AdomdCommand.ExecuteXmlReader`
for the reference, the HTTP response for the proxy): ADOMD's reader flattens
rows, and the reference writes unformatted doubles in `<Value>` where the proxy
writes plain integers — compare numerically, and compare axis member counts.

Verified: `Revenue` totals 521586767 on both engines, and the pivot renders the
same 20 categories. Diffing Excel's requests showed the proxy's cell-property
advertisement was short (no `LANGUAGE`/`FONT_FLAGS`), which made Excel ask for
four cell properties where the mirror is asked for six — a reminder that Excel
adapts its request to `MDSCHEMA_PROPERTIES`.

## Query the metadata rowsets (DMVs)

```powershell
$conn = New-Object -ComObject ADODB.Connection
$conn.Open('Provider=MSOLAP;Data Source=localhost;Initial Catalog=MallardRef;')
$rs = $conn.Execute('SELECT * FROM $SYSTEM.MDSCHEMA_PROPERTIES')   # single quotes!
```

Use single-quoted strings — `"$SYSTEM..."` interpolates to nothing in PowerShell.
Read columns defensively (`try { $rs.Fields.Item('DATA_TYPE').Value } catch {}`).

**DMVs are not Discover requests.** A `SELECT * FROM $SYSTEM.MDSCHEMA_PROPERTIES`
returns every row and filters client-side, so it cannot show what the engine
answers to a *restricted* request. To reproduce Excel's requests exactly, use
ADOMD with an `AdomdRestrictionCollection` (this is how the `PROPERTY_TYPE`
semantics were measured):

```powershell
Add-Type -Path "C:\Program Files\Microsoft.NET\ADOMD.NET\170\Microsoft.AnalysisServices.AdomdClient.dll"
$conn = New-Object Microsoft.AnalysisServices.AdomdClient.AdomdConnection("Data Source=localhost;Initial Catalog=MallardDemo")
$conn.Open()
$rc = New-Object Microsoft.AnalysisServices.AdomdClient.AdomdRestrictionCollection
$rc.Add('CUBE_NAME', 'Model')
$rc.Add('HIERARCHY_UNIQUE_NAME', '[Category].[Category]')
$rc.Add('PROPERTY_TYPE', 1)          # integer, as Excel sends it
$ds = $conn.GetSchemaDataSet('MDSCHEMA_PROPERTIES', $rc)   # -> row count 0
```

The relay at `http://127.0.0.1:8090/OLAP/msmdpump.dll` answers `Discover` with
an empty rowset (it only forwards `Execute`), so use ADOMD for metadata
questions.

## Connect Excel to the reference model

```powershell
$conn = $wb.Connections.Add2('Ref', 'ref',
  'OLEDB;Provider=MSOLAP.8;Integrated Security=SSPI;Persist Security Info=True;Initial Catalog=MallardRef;Data Source=localhost;MDX Compatibility=1;Safety Options=2;MDX Missing Member Mode=Error;Update Isolation Level=2',
  'Model', 1, $false, $false)          # CommandType MUST be 1 (xlCmdCube)
$pc = $wb.PivotCaches().Create(2, $conn)   # 2 = xlExternal
$pt = $pc.CreatePivotTable($ws.Range('A3'), 'RefPivot')
$pt.CubeFields('[DateDim].[FullDate]').Orientation = 1   # AddFields is refused for OLAP
$pt.CubeFields('[Measures].[Revenue]').Orientation = 4
```

## Read what Excel decided (pivot cache definition)

```powershell
$wb.SaveAs("C:\Users\Public\Documents\probe.xlsx", 51)   # 51 = xlsx; SaveCopyAs may save ODF!
Expand-Archive probe.zip -DestinationPath probe_x -Force
$xml = Get-Content "probe_x\xl\pivotCache\pivotCacheDefinition1.xml" -Raw
[regex]::Matches($xml, '<cacheHierarchy [^>]*>') | ForEach-Object { $_.Value }
```

`cacheHierarchy` attributes that matter: `time`, `attribute`, `keyAttribute`,
`memberValueDatatype`, `hidden`.

## Verified metadata rules (real SSAS 2025 tabular, 2026-09-21)

| Rowset / attribute | Real engine value |
|---|---|
| `MDSCHEMA_DIMENSIONS.DIMENSION_TYPE` | 1 = Time, 2 = Measures, 3 = Other; `DEFAULT_HIERARCHY` points at the user hierarchy |
| `MDSCHEMA_HIERARCHIES.HIERARCHY_ORIGIN` | 1 = user hierarchy, 2 = attribute hierarchy, 6 = Measures (attribute + key attribute bits) |
| `MDSCHEMA_LEVELS.LEVEL_TYPE` | 1 for `(All)` levels, **0 for every other level** (tabular does not use time level types) |
| `MDSCHEMA_LEVELS.LEVEL_DBTYPE` | 7 = date, 20 = int64, 130 = string, 3 for `(All)` |
| `MDSCHEMA_LEVELS.LEVEL_ORIGIN` | 1 = user hierarchy levels, 2 = attribute hierarchy levels, 6 = Measures |
| `MDSCHEMA_PROPERTIES` MEMBER_VALUE | one row **per level**, `PROPERTY_TYPE=5`, `DATA_TYPE` = 130 `(All)` / 20 int / 7 date / 130 string |
| `MDSCHEMA_MEASURES.DATA_TYPE` | measure data type (e.g. 20 for int64 measures) |

**The Excel date-filter rule (root cause of a long investigation):**

- Excel stores `memberValueDatatype` **per attribute hierarchy** (single-level
  column/hierarchy), read from that level's `MEMBER_VALUE` `DATA_TYPE`
  (`7` = date/time — the OOXML note's value).
- A **user hierarchy** gets `time="1"` (because the dimension is Time) but **no
  `memberValueDatatype`** — so its levels never offer Date Filters.
- Verified in the UI: the date column (`[DateDim].[FullDate]`, a single-level
  attribute hierarchy, `memberValueDatatype="7"`) shows **Date Filters…** in the
  Filter menu; a user hierarchy's Year level does not.
- Consequence for MallardCube: to offer Date Filters, expose the date role's
  full-date level as its **own single-level attribute hierarchy** with
  `MEMBER_VALUE` DATA_TYPE=7, and emit `MEMBER_VALUE` rows for all levels
  (`(All)`=130, period levels=int, date level=7). See plan 048.

**Three more rules that gate the same feature (2026-09-22):**

- **`DISCOVER_SCHEMA_ROWSETS` must honour the `SchemaName` restriction.** Excel
  asks for one rowset's entry to learn its restrictions; answering with the
  whole list makes it miss `HIERARCHY_VISIBILITY` and take an older metadata
  path. The reference answers with exactly one row.
- **`MDSCHEMA_CUBES.PREFERRED_QUERY_PATTERNS=3`** (tabular) gates whether Excel
  ever asks for the key attribute's `MEMBER_VALUE` at all. With `0` its trace
  shows only `PROPERTY_TYPE=2` cell-property requests and the pivot cache gets
  no `memberValueDatatype`.
- **Excel reads `MDSCHEMA_PROPERTIES` rows positionally against the schema.**
  Emit the elements in the reference's order — `… LEVEL_UNIQUE_NAME,
  PROPERTY_TYPE, PROPERTY_NAME, PROPERTY_CAPTION, DATA_TYPE, PROPERTY_ORIGIN,
  PROPERTY_IS_VISIBLE` — and use the reference's column list in the schema.
  A `PROPERTY_NAME`-before-`PROPERTY_TYPE` order made Excel read `5` as
  `DATA_TYPE` and stamp `memberValueDatatype="5"` on every hierarchy. `KEY0` and
  `NAME` rows carry the same key type as `MEMBER_VALUE` (`(All)` KEY0=3, NAME=130;
  int levels 20; date 7; string 130), and member-value rows are sorted by
  hierarchy with `[Measures]` last.
- **`MDSCHEMA_PROPERTIES` honours the `PROPERTY_TYPE` restriction, and the
  reference has no `PROPERTY_TYPE=1` rows at all** (measured 2026-09-22 on the
  mirror tabular model, via `GetSchemaDataSet` with an
  `AdomdRestrictionCollection`). Excel sends
  `<CUBE_NAME>…</CUBE_NAME><HIERARCHY_UNIQUE_NAME>…</HIERARCHY_UNIQUE_NAME><PROPERTY_TYPE>1</PROPERTY_TYPE>`
  while it builds pivot cache fields; the reference answers **empty**, which
  Excel reads as `(No Properties Retrieved)`. Answering with the hierarchy's
  own `PROPERTY_TYPE=5` rows instead makes Excel write `memberPropertyField="1"`
  cache fields (`…KEY0`, `…MEMBER_VALUE`) and ask for them in every pivot MDX.
  Full measured semantics: `PROPERTY_TYPE=1/3/4` → empty; `2` → the 12 cell
  properties **only when the request names no cube or hierarchy** (cube-scoped
  → empty); `5` → the hierarchy rows; no type + cube/hierarchy → hierarchy
  rows; no type + no cube → 12 cell properties + the hierarchy rows.
  Restriction-free queries (`GetSchemaDataSet('MDSCHEMA_PROPERTIES', $null)`)
  are *not* the same as Excel's Discover requests — always compare with the
  same restrictions.
- **The attribute hierarchy's axis members** are `(All)` first, then
  `[DateDim].[FullDate].&[2024-01-15T00:00:00]` with `LName
  =[DateDim].[FullDate].[FullDate]`, `LNum=1` and
  `PARENT_UNIQUE_NAME=[DateDim].[FullDate].[All]`. Excel places axis members by
  the field's hierarchy and level numbers: rendering the dates in the *user*
  hierarchy's namespace (level 4) leaves the field empty, and dropping the
  hierarchy name from `DrilldownLevel({[DateDim].[FullDate].[All]})` makes it
  list the user hierarchy's years instead.

- **Excel reads member elements positionally.** A cellset member must carry the
  standard five in the reference's order — `UName`, `Caption`, `LName`, `LNum`,
  `DisplayInfo` — followed by the *requested* dimension properties.
  `CHILDREN_CARDINALITY` is declared and emitted **only when the query asks for
  it**; shipping it unrequested shifts `PARENT_UNIQUE_NAME` by one and Excel
  crashes on multi-level expansion (`Expand to Month` / `Expand to Full Date`),
  while single-level expansion still looks fine (plan 048).
- **Compound member UNames** in the reference name the *top* level and then one
  key segment each: `[DateDim].[Calendar].[Year].&[2024].&[1]` for a quarter,
  `…&[1].&[1]` for a month. MallardCube names the member's own level
  (`[Date].[Calendar].[Quarter].&[2024]&[1]`); Excel round-trips either, and the
  difference is still open as a parity item.

**Excel's Date Filters (captured verbatim, 2026-09-22):**

```sql
SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Full Date].[All]},,,INCLUDE_CALC_MEMBERS)})
  DIMENSION PROPERTIES PARENT_UNIQUE_NAME,[Date].[Full Date].[Full Date]KEY0,[Date].[Full Date].[Full Date]MEMBER_VALUE
  ON COLUMNS
  FROM (SELECT Filter([Date].[Full Date].Levels(1).AllMembers,
                      ([Date].[Full Date].CurrentMember.MemberValue = CDate("2026-09-23")))
        ON COLUMNS FROM [Sales])
  WHERE ([Measures].[Revenue])
```

- The filter is a **subquery** over the key attribute hierarchy, with
  `Levels(1)` for its single level; the operator follows the menu item
  (`=`, `<>`, `<`, `<=`, `>`, `>=`). Excel computes the period items
  (`Today`, `This Month`, …) client-side and sends a plain `CDate`.
- Driving the dialog: its buttons are **drawn by Office** (no child HWNDs), the
  date field's text is not committed until the **calendar picker** sets it, and
  OK stays disabled until then. Click the picker's `Today` button, then OK.
  `WM_COMMAND`/`SendMessage` do not reach it; synthetic clicks do once the
  dialog is the foreground window.

## Caveat: tabular ≠ multidimensional

This instance is **tabular**. Multidimensional SSAS date filters key off the
dimension's **key attribute hierarchy** and work "independent of which hierarchy
is filtered" (MS: "OLE DB for OLAP properties used by Excel"), so the exact
hierarchy shape to emulate depends on which flavour the proxy presents.
