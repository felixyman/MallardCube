/// M/Power Query expression parser — extracts structured source connection info
/// from Tabular partition M expressions.
///
/// This is a pure parser module. It has no knowledge of DuckDB, SQL generation,
/// or output files. Returns `None` when the expression can't be parsed (fail safe).
///
/// # Patterns supported
///
/// - Sql.Database("server", "db") + table reference → SQL Server
/// - Sql.Database("server", "db", [Query="SELECT ..."]) → SQL Server with native query
/// - Web.Contents(url, [RelativePath="path.csv"]) + Csv.Document → CSV from web
/// - File.Contents("path.csv") + Csv.Document → CSV from local file
/// - PostgreSQL.Database("server", "db") + table reference → PostgreSQL
/// - MySQL.Database("server", "db") + table reference → MySQL
/// - Named source reference (#"provider;name") + table reference → Unknown kind
use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A data source connection extracted from an M expression.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SourceConnection {
    pub kind: SourceKind,
    pub server: Option<String>,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub table: Option<String>,
    pub native_query: Option<String>,
    pub file_path: Option<String>,     // for CSV/Excel sources
    pub url: Option<String>,           // for Web.Contents sources
    pub relative_path: Option<String>, // for Web.Contents with RelativePath
}

/// The type of data source.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum SourceKind {
    SqlServer,
    Postgres,
    MySQL,
    CSV,
    Excel,
    Web,
    Unknown,
}

// ---------------------------------------------------------------------------
// Lazy regex statics
// ---------------------------------------------------------------------------

fn sql_db_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"Sql\.Database\s*\(\s*"([^"]+)"\s*,\s*"([^"]+)""#).expect("sql_db_re")
    })
}

fn native_query_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"\[Query\s*=\s*"((?:[^"\\]|\\.)*)"\]"#).expect("native_query_re")
    })
}

fn table_ref_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Match either Schema before Item or Item before Schema
        Regex::new(
            r#"\{[^}]*(?:Schema\s*=\s*"([^"]+)"[^}]*Item\s*=\s*"([^"]+)"|Item\s*=\s*"([^"]+)"[^}]*Schema\s*=\s*"([^"]+)")[^}]*\}"#,
        )
        .expect("table_ref_re")
    })
}

fn web_contents_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"Web\.Contents\s*\(\s*([^,]+?)\s*,\s*\[RelativePath\s*=\s*"([^"]+)""#)
            .expect("web_contents_re")
    })
}

fn file_contents_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"File\.Contents\s*\(\s*"([^"]+)""#).expect("file_contents_re"))
}

fn pg_db_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"PostgreSQL\.Database\s*\(\s*"([^"]+)"\s*,\s*"([^"]+)""#).expect("pg_db_re")
    })
}

fn mysql_db_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"MySQL\.Database\s*\(\s*"([^"]+)"\s*,\s*"([^"]+)""#).expect("mysql_db_re")
    })
}

// ---------------------------------------------------------------------------
// Main extraction function
// ---------------------------------------------------------------------------

/// Extract source connection info from an M expression.
///
/// Returns `None` if the expression can't be parsed (fail safe — never guess).
/// Returns `Some(SourceConnection)` if a recognizable pattern is found.
pub fn extract_source(m_expression: &str) -> Option<SourceConnection> {
    let expr = m_expression.trim();
    if expr.is_empty() {
        return None;
    }

    // --- Pattern 1 & 2: Sql.Database (SQL Server) ---
    if let Some(conn) = try_sql_server(expr) {
        return Some(conn);
    }

    // --- Pattern 3 & 4: CSV sources (Web.Contents or File.Contents + Csv.Document) ---
    if let Some(conn) = try_csv(expr) {
        return Some(conn);
    }

    // --- Pattern 5: PostgreSQL ---
    if let Some(conn) = try_postgres(expr) {
        return Some(conn);
    }

    // --- Pattern 6: MySQL ---
    if let Some(conn) = try_mysql(expr) {
        return Some(conn);
    }

    // --- Pattern 7: Named source reference ---
    if let Some(conn) = try_named_source(expr) {
        return Some(conn);
    }

    None
}

// ---------------------------------------------------------------------------
// Pattern matchers
// ---------------------------------------------------------------------------

/// Sql.Database("server", "db") with optional native query and table ref.
fn try_sql_server(expr: &str) -> Option<SourceConnection> {
    let caps = sql_db_re().captures(expr)?;
    let server = caps[1].to_string();
    let database = caps[2].to_string();

    // Check for native query: [Query="SELECT ..."]
    let native_query = native_query_re().captures(expr).map(|c| c[1].to_string());

    // Extract schema + table from {[Schema="...",Item="..."]}[Data]
    let (schema, table) = extract_table_ref(expr);

    Some(SourceConnection {
        kind: SourceKind::SqlServer,
        server: Some(server),
        database: Some(database),
        schema,
        table,
        native_query,
        file_path: None,
        url: None,
        relative_path: None,
    })
}

/// CSV sources: Web.Contents or File.Contents + Csv.Document
fn try_csv(expr: &str) -> Option<SourceConnection> {
    // Must contain Csv.Document to be treated as CSV
    if !expr.contains("Csv.Document") {
        return None;
    }

    // Pattern 3: Web.Contents(url, [RelativePath = "..."])
    if let Some(caps) = web_contents_re().captures(expr) {
        let url_str = caps[1].trim();
        // Strip surrounding quotes if present
        let url_clean = url_str
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(url_str)
            .to_string();
        let relative_path = caps[2].to_string();
        // If url looks like a parameter reference (#"[...]") we can't resolve it
        let url = if url_str.starts_with('#') {
            None
        } else {
            Some(url_clean)
        };
        return Some(SourceConnection {
            kind: SourceKind::CSV,
            server: None,
            database: None,
            schema: None,
            table: None,
            native_query: None,
            file_path: None,
            url,
            relative_path: Some(relative_path),
        });
    }

    // Pattern 4: File.Contents("path")
    if let Some(caps) = file_contents_re().captures(expr) {
        return Some(SourceConnection {
            kind: SourceKind::CSV,
            server: None,
            database: None,
            schema: None,
            table: None,
            native_query: None,
            file_path: Some(caps[1].to_string()),
            url: None,
            relative_path: None,
        });
    }

    None
}

/// PostgreSQL.Database("server", "db") + table ref.
fn try_postgres(expr: &str) -> Option<SourceConnection> {
    let caps = pg_db_re().captures(expr)?;
    let (schema, table) = extract_table_ref(expr);

    Some(SourceConnection {
        kind: SourceKind::Postgres,
        server: Some(caps[1].to_string()),
        database: Some(caps[2].to_string()),
        schema,
        table,
        native_query: None,
        file_path: None,
        url: None,
        relative_path: None,
    })
}

/// MySQL.Database("server", "db") + table ref.
fn try_mysql(expr: &str) -> Option<SourceConnection> {
    let caps = mysql_db_re().captures(expr)?;
    let (schema, table) = extract_table_ref(expr);

    Some(SourceConnection {
        kind: SourceKind::MySQL,
        server: Some(caps[1].to_string()),
        database: Some(caps[2].to_string()),
        schema,
        table,
        native_query: None,
        file_path: None,
        url: None,
        relative_path: None,
    })
}

/// Named source reference: #"provider;name" + optional table ref.
/// We can't resolve the named reference without data source lookup,
/// so kind=Unknown, but we still extract schema+table if present.
fn try_named_source(expr: &str) -> Option<SourceConnection> {
    // Must have a #"..." reference and a table ref to be a named source pattern
    if !expr.contains('#') {
        return None;
    }
    let (schema, table) = extract_table_ref(expr);
    table.as_ref()?;
    Some(SourceConnection {
        kind: SourceKind::Unknown,
        server: None,
        database: None,
        schema,
        table,
        native_query: None,
        file_path: None,
        url: None,
        relative_path: None,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract schema and table from `{[Schema="...",Item="..."]}[Data]` patterns.
///
/// Handles both `Schema="x",Item="y"` and `Item="y",Schema="x"` orderings.
fn extract_table_ref(expr: &str) -> (Option<String>, Option<String>) {
    if let Some(caps) = table_ref_re().captures(expr) {
        // Alternation groups: if Schema first → caps[1], caps[2]; if Item first → caps[3], caps[4]
        if caps.get(1).is_some() {
            (Some(caps[1].to_string()), Some(caps[2].to_string()))
        } else {
            (Some(caps[4].to_string()), Some(caps[3].to_string()))
        }
    } else {
        (None, None)
    }
}

/// Check if an M expression contains complex transformations beyond simple connect+select.
/// Returns true if we detect Table.SelectRows, Table.Combine, Table.Join, etc.
pub fn is_complex_m(m_expression: &str) -> bool {
    let upper = m_expression.to_uppercase();
    upper.contains("TABLE.SELECTROWS")
        || upper.contains("TABLE.COMBINE")
        || upper.contains("TABLE.JOIN")
        || upper.contains("TABLE.MERGE")
        || upper.contains("TABLE.ADDCOLUMN")
        || upper.contains("TABLE.GROUP")
        || upper.contains("TABLE.SORT")
        || upper.contains("TABLE.RENAMECOLUMNS")
        || upper.contains("TABLE.EXPANDTABLECOLUMN")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // Pattern 1: Sql.Database + table ref
    // ---------------------------------------------------------------

    #[test]
    fn test_sql_server_direct() {
        let m = r#"let
    Source = Sql.Database("server01", "AdventureWorksDW"),
    dbo_Sales = Source{[Schema="dbo",Item="FactInternetSales"]}[Data]
in
    dbo_Sales"#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::SqlServer);
        assert_eq!(conn.server.as_deref(), Some("server01"));
        assert_eq!(conn.database.as_deref(), Some("AdventureWorksDW"));
        assert_eq!(conn.schema.as_deref(), Some("dbo"));
        assert_eq!(conn.table.as_deref(), Some("FactInternetSales"));
        assert_eq!(conn.native_query, None);
        assert_eq!(conn.file_path, None);
        assert_eq!(conn.url, None);
        assert_eq!(conn.relative_path, None);
    }

    #[test]
    fn test_sql_server_direct_without_table_ref() {
        let m = r#"let
    Source = Sql.Database("server01", "AdventureWorksDW")
in
    Source"#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::SqlServer);
        assert_eq!(conn.server.as_deref(), Some("server01"));
        assert_eq!(conn.database.as_deref(), Some("AdventureWorksDW"));
        assert_eq!(conn.schema, None);
        assert_eq!(conn.table, None);
    }

    // ---------------------------------------------------------------
    // Pattern 2: Sql.Database with native query
    // ---------------------------------------------------------------

    #[test]
    fn test_sql_server_native_query() {
        let m = r#"let
    Source = Sql.Database("server01", "AdventureWorksDW", [Query="SELECT * FROM DimDate"])
in
    Source"#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::SqlServer);
        assert_eq!(conn.server.as_deref(), Some("server01"));
        assert_eq!(conn.database.as_deref(), Some("AdventureWorksDW"));
        assert_eq!(conn.native_query.as_deref(), Some("SELECT * FROM DimDate"));
    }

    #[test]
    fn test_sql_server_native_query_with_escaped_quotes() {
        let m = r#"let
    Source = Sql.Database("s", "d", [Query="SELECT * FROM \"Table\""])
in
    Source"#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::SqlServer);
        assert_eq!(
            conn.native_query.as_deref(),
            Some(r#"SELECT * FROM \"Table\""#)
        );
    }

    // ---------------------------------------------------------------
    // Pattern 3: CSV via Web.Contents + Csv.Document (Contoso style)
    // ---------------------------------------------------------------

    #[test]
    fn test_csv_web_contents() {
        let m = r#"let
    Source = Web.Contents(#"[SourceUrl]", [RelativePath = "pbi-tools/contoso-sales-model/main/data/Sales.csv"]),
    Csv = Csv.Document(Source, [QuoteStyle=QuoteStyle.Csv]),
    #"Promoted Headers" = Table.PromoteHeaders(Csv, [PromoteAllScalars=true])
in
    #"Promoted Headers""#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::CSV);
        assert_eq!(conn.url, None); // parameter reference, not resolvable
        assert_eq!(
            conn.relative_path.as_deref(),
            Some("pbi-tools/contoso-sales-model/main/data/Sales.csv")
        );
        assert_eq!(conn.server, None);
        assert_eq!(conn.database, None);
    }

    #[test]
    fn test_csv_web_contents_with_url() {
        let m = r#"let
    Source = Web.Contents("https://example.com/data", [RelativePath = "sales.csv"]),
    Csv = Csv.Document(Source)
in
    Csv"#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::CSV);
        assert_eq!(conn.url.as_deref(), Some("https://example.com/data"));
        assert_eq!(conn.relative_path.as_deref(), Some("sales.csv"));
    }

    // ---------------------------------------------------------------
    // Pattern 4: CSV via File.Contents + Csv.Document
    // ---------------------------------------------------------------

    #[test]
    fn test_csv_file_contents() {
        let m = r#"let
    Source = File.Contents("C:\data\sales.csv"),
    Csv = Csv.Document(Source)
in
    Csv"#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::CSV);
        assert_eq!(conn.file_path.as_deref(), Some("C:\\data\\sales.csv"));
    }

    #[test]
    fn test_csv_file_contents_unix_path() {
        let m = r#"let
    Source = File.Contents("/data/sales.csv"),
    Csv = Csv.Document(Source)
in
    Csv"#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::CSV);
        assert_eq!(conn.file_path.as_deref(), Some("/data/sales.csv"));
    }

    // ---------------------------------------------------------------
    // Pattern 5: PostgreSQL
    // ---------------------------------------------------------------

    #[test]
    fn test_postgres() {
        let m = r#"let
    Source = PostgreSQL.Database("pg-server", "sales_db"),
    Table = Source{[Schema="public",Item="orders"]}[Data]
in
    Table"#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::Postgres);
        assert_eq!(conn.server.as_deref(), Some("pg-server"));
        assert_eq!(conn.database.as_deref(), Some("sales_db"));
        assert_eq!(conn.schema.as_deref(), Some("public"));
        assert_eq!(conn.table.as_deref(), Some("orders"));
    }

    // ---------------------------------------------------------------
    // Pattern 6: MySQL
    // ---------------------------------------------------------------

    #[test]
    fn test_mysql() {
        let m = r#"let
    Source = MySQL.Database("mysql-host", "inventory"),
    Table = Source{[Schema="inventory",Item="products"]}[Data]
in
    Table"#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::MySQL);
        assert_eq!(conn.server.as_deref(), Some("mysql-host"));
        assert_eq!(conn.database.as_deref(), Some("inventory"));
        assert_eq!(conn.schema.as_deref(), Some("inventory"));
        assert_eq!(conn.table.as_deref(), Some("products"));
    }

    // ---------------------------------------------------------------
    // Pattern 7: Named source reference
    // ---------------------------------------------------------------

    #[test]
    fn test_named_source_reference() {
        let m = r#"let
    Source = #"SQL/sqlserver database windows net;Contoso",
    dbo_Sales = Source{[Schema="dbo",Item="Sales"]}[Data]
in
    dbo_Sales"#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::Unknown);
        assert_eq!(conn.server, None);
        assert_eq!(conn.database, None);
        assert_eq!(conn.schema.as_deref(), Some("dbo"));
        assert_eq!(conn.table.as_deref(), Some("Sales"));
    }

    // ---------------------------------------------------------------
    // Edge cases
    // ---------------------------------------------------------------

    #[test]
    fn test_empty_expression() {
        assert_eq!(extract_source(""), None);
        assert_eq!(extract_source("   "), None);
    }

    #[test]
    fn test_unrecognized_expression() {
        assert_eq!(extract_source("this is not an m expression"), None);
        assert_eq!(extract_source("just some random text with Sql."), None);
    }

    #[test]
    fn test_calculated_table_expression() {
        // Info table uses #table(type table [...], {...}) — not extractable
        let m = r#"let
    Source = #table(type table [Label=text, Timestamp=datetime, Text=text],
    {
        { "Data Updated", DateTimeZone.RemoveZone(DateTimeZone.UtcNow()), null }
    })
in
    Source"#;
        assert_eq!(extract_source(m), None);
    }

    // ---------------------------------------------------------------
    // Complex M detection
    // ---------------------------------------------------------------

    #[test]
    fn test_complex_m_detection() {
        let m = r#"let
    Source = Sql.Database("s", "d"),
    Filtered = Table.SelectRows(Source, each [Date] > #date(2020,1,1))
in
    Filtered"#;
        assert!(is_complex_m(m));
    }

    #[test]
    fn test_simple_m_not_complex() {
        let m = r#"let
    Source = Sql.Database("s", "d"),
    Table = Source{[Schema="dbo",Item="t"]}[Data]
in
    Table"#;
        assert!(!is_complex_m(m));
    }

    #[test]
    fn test_contoso_sales_pattern() {
        // Sales has both Web.Contents + Csv.Document + Table.SelectRows (complex)
        let m = r#"let
    Source = Web.Contents(#"[SourceUrl]", [RelativePath = "pbi-tools/contoso-sales-model/main/data/Sales.csv"]),
    Csv = Csv.Document(Source),
    #"Promoted Headers" = Table.PromoteHeaders(Csv, [PromoteAllScalars=true]),
    #"Changed Type" = Table.TransformColumnTypes(#"Promoted Headers",{{"StoreKey", Int64.Type}, {"ProductKey", Int64.Type}}),
    #"Parsed Date" = Table.TransformColumns(#"Changed Type",{{"Delivery Date", each Date.From(DateTimeZone.From(_)), type date}, {"Order Date", each Date.From(DateTimeZone.From(_)), type date}}),
    Filtered_Date = if #"[FilterDate]" = null then #"Parsed Date" else Table.SelectRows(#"Parsed Date", each [Order Date] >= #"[FilterDate]")
in
    Filtered_Date"#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::CSV);
        assert_eq!(conn.url, None); // parameter reference
        assert_eq!(
            conn.relative_path.as_deref(),
            Some("pbi-tools/contoso-sales-model/main/data/Sales.csv")
        );
        assert!(is_complex_m(m));
    }

    #[test]
    fn test_contoso_customer_pattern() {
        // Customer from Contoso BIM — complete with type changes and date parsing
        let m = r#"let
    Source = Web.Contents(#"[SourceUrl]", [RelativePath = "pbi-tools/contoso-sales-model/main/data/Customer.csv"]),
    Csv = Csv.Document(Source, [QuoteStyle=QuoteStyle.Csv]),
    #"Promoted Headers" = Table.PromoteHeaders(Csv, [PromoteAllScalars=true]),
    #"Changed Type" = Table.TransformColumnTypes(#"Promoted Headers",{{"CustomerKey", Int64.Type}, {"Customer Code", type text}, {"Title", type text}, {"Name", type text}, {"Marital Status", type text}, {"Gender", type text}, {"Yearly Income", type number}, {"Total Children", Int64.Type}, {"Children At Home", Int64.Type}, {"Education", type text}, {"Occupation", type text}, {"House Ownership", type text}, {"Cars Owned", Int64.Type}, {"Continent", type text}, {"City", type text}, {"State", type text}, {"CountryRegion", type text}, {"Address Line 1", type text}, {"Address Line 2", type text}, {"Phone", type text}, {"Customer Type", type text}, {"Company Name", type text}}),
    #"Parsed Date" = Table.TransformColumns(#"Changed Type",{{"Birth Date", each Date.From(DateTimeZone.From(_)), type date}, {"Date First Purchase", each Date.From(DateTimeZone.From(_)), type date}})
in
    #"Parsed Date""#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::CSV);
        assert_eq!(conn.url, None);
        assert_eq!(
            conn.relative_path.as_deref(),
            Some("pbi-tools/contoso-sales-model/main/data/Customer.csv")
        );
        // Customer has Table.TransformColumns (not in complex list) + TransformColumnTypes
        // is_complex_m should be false since we don't flag TransformColumns/TransformColumnTypes
        assert!(!is_complex_m(m));
    }

    // ---------------------------------------------------------------
    // Multi-line / whitespace variations
    // ---------------------------------------------------------------

    #[test]
    fn test_sql_server_extra_whitespace() {
        let m = r#"let
    Source = Sql.Database(  "server01"  ,  "AdventureWorksDW"  )
in
    Source"#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::SqlServer);
        assert_eq!(conn.server.as_deref(), Some("server01"));
        assert_eq!(conn.database.as_deref(), Some("AdventureWorksDW"));
    }

    // ---------------------------------------------------------------
    // extract_table_ref as standalone helper validation
    // ---------------------------------------------------------------

    #[test]
    fn test_extract_table_ref_variations() {
        // Standard pattern
        let (s, t) = extract_table_ref(r#"Source{[Schema="dbo",Item="Sales"]}[Data]"#);
        assert_eq!(s.as_deref(), Some("dbo"));
        assert_eq!(t.as_deref(), Some("Sales"));

        // Reversed order (Item before Schema)
        let (s, t) = extract_table_ref(r#"Source{[Item="Sales",Schema="dbo"]}[Data]"#);
        assert_eq!(s.as_deref(), Some("dbo"));
        assert_eq!(t.as_deref(), Some("Sales"));

        // No match
        let (s, t) = extract_table_ref("no table ref here");
        assert_eq!(s, None);
        assert_eq!(t, None);
    }

    // ---------------------------------------------------------------
    // Csv.Document without Web.Contents or File.Contents
    // ---------------------------------------------------------------

    #[test]
    fn test_csv_without_known_source_function() {
        let m = r#"let
    Source = SomeOtherFunction("path"),
    Csv = Csv.Document(Source)
in
    Csv"#;
        assert_eq!(extract_source(m), None);
    }

    // ---------------------------------------------------------------
    // is_complex_m with various patterns
    // ---------------------------------------------------------------

    #[test]
    fn test_complex_m_table_combine() {
        let m = "Table.Combine({Source, Other})";
        assert!(is_complex_m(m));
    }

    #[test]
    fn test_complex_m_table_merge() {
        let m = "Table.Merge(Source, Other, {\"key\"})";
        assert!(is_complex_m(m));
    }

    #[test]
    fn test_complex_m_empty_string() {
        assert!(!is_complex_m(""));
    }

    #[test]
    fn test_complex_m_case_insensitive() {
        assert!(is_complex_m("table.selectrows"));
        assert!(is_complex_m("Table.SelectRows"));
        assert!(is_complex_m("TABLE.SELECTROWS"));
    }

    // ---------------------------------------------------------------
    // Sql.Database with both table ref and native query
    // ---------------------------------------------------------------

    #[test]
    fn test_sql_server_with_table_ref_and_native_query() {
        let m = r#"let
    Source = Sql.Database("s", "d", [Query="SELECT * FROM sys.tables"]),
    Filtered = Source{[Schema="dbo",Item="Users"]}[Data]
in
    Filtered"#;
        let conn = extract_source(m).expect("should extract source");
        assert_eq!(conn.kind, SourceKind::SqlServer);
        assert_eq!(conn.server.as_deref(), Some("s"));
        assert_eq!(conn.database.as_deref(), Some("d"));
        assert_eq!(
            conn.native_query.as_deref(),
            Some("SELECT * FROM sys.tables")
        );
        assert_eq!(conn.schema.as_deref(), Some("dbo"));
        assert_eq!(conn.table.as_deref(), Some("Users"));
    }

    // ---------------------------------------------------------------
    // Named source without table ref should not match
    // ---------------------------------------------------------------

    #[test]
    fn test_named_source_without_table_ref() {
        let m = r#"let
    Source = #"SQL/sqlserver database windows net;Contoso"
in
    Source"#;
        assert_eq!(extract_source(m), None);
    }
}
