//! Shared types for Tabular model representation.
//! Used by both the converter and inventory tools, and by folder/BIM/TMDL parsers.

use serde::Serialize;

/// Parsed Tabular model (format-agnostic).
/// Produced by `parse_folder::parse_model()` or `parse_bim::parse_model()`.
#[derive(Debug, Clone, Serialize)]
pub struct TabularModel {
    pub name: String,
    pub compatibility_level: i64,
    pub tables: Vec<TableInfo>,
    pub relationships: Vec<RelInfo>,
    pub roles: Vec<RoleInfo>,
    pub data_sources: Vec<DataSourceInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableInfo {
    pub name: String,
    pub ssas_name: String,
    pub description: String,
    pub columns: Vec<ColumnInfo>,
    pub measures: Vec<MeasureInfo>,
    pub partitions: Vec<PartitionInfo>,
    pub hierarchies: Vec<String>,
}

impl TableInfo {
    pub fn is_m_partition(&self) -> bool {
        self.partitions.iter().any(|p| p.is_m)
    }

    pub fn is_calculated(&self) -> bool {
        self.partitions
            .iter()
            .any(|p| p.source_type == "calculated")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub source_column: String,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeasureInfo {
    pub name: String,
    pub expression: String, // flattened DAX expression (NOT Vec<String>)
    pub display_folder: String,
    pub classification: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartitionInfo {
    pub name: String,
    pub source_type: String,
    pub is_m: bool,
    pub query: Option<String>,
    pub data_source_name: Option<String>,
    pub mode: Option<String>,
    pub schema: Option<String>,
    pub database: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataSourceInfo {
    pub name: String,
    pub provider: String,
    pub server: String,
    pub database: String,
    pub connection_string: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelInfo {
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleMemberInfo {
    pub member_name: String,
    #[serde(default)]
    pub member_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TablePermissionInfo {
    pub table: String,
    #[serde(default)]
    pub filter_expression: String,
    #[serde(default)]
    pub dax_filter: Option<String>,
    pub metadata_permission: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleInfo {
    pub name: String,
    pub description: String,
    pub model_permission: String,
    #[serde(default)]
    pub members: Vec<RoleMemberInfo>,
    #[serde(default)]
    pub table_permissions: Vec<TablePermissionInfo>,
}

// ---- Shared utility functions ----

/// Normalize Tabular Editor DAX whitespace: `CALCULATE (` → `CALCULATE(`.
/// Most exporters add spaces before/after parentheses that our classifiers
/// and expression lowerlers don't expect.
pub fn normalize_dax(s: &str) -> String {
    let s = s.trim();
    let s = if let Some(idx) = s.find("//") {
        &s[..idx]
    } else {
        s
    };
    s.trim()
        .trim_start_matches('=')
        .trim()
        .replace(" (", "(")
        .replace("( ", "(")
        .replace(" )", ")")
}

/// Classify a DAX expression into: simple, time_ytd, time_prior_year, sql_fallback, calculated_table, manual.
/// This is the FULL version from the converter (with time intelligence and measure arithmetic detection).
pub fn classify_dax(expr: &str) -> String {
    let expr = normalize_dax(expr);
    let upper = expr.to_uppercase();
    // Time intelligence: emit structured date-flag measures instead of sql_fallback
    if upper.contains("TOTALYTD") || upper.contains("DATESYTD") {
        return "time_ytd".into();
    }
    if upper.contains("SAMEPERIODLASTYEAR") {
        return "time_prior_year".into();
    }
    if upper.contains("ALLSELECTED")
        || upper.contains("ISONORAFTER")
        || (upper.contains("CALCULATE(") && upper.contains("FILTER("))
    {
        return "sql_fallback".into();
    }
    if upper.contains("ALL(") || upper.contains("ALLEXCEPT") || upper.contains("KEEPFILTERS") {
        return "sql_fallback".into();
    }
    if upper.contains("SUMX(")
        || upper.contains("AVERAGEX(")
        || upper.contains("MAXX(")
        || upper.contains("RANKX(")
    {
        return "sql_fallback".into();
    }
    if upper.contains("TODAY()")
        || upper.contains("NOW()")
        || upper.contains("UTCNOW()")
        || upper.contains("SAMEPERIODLASTYEAR")
    {
        return "sql_fallback".into();
    }
    if upper.contains("CALCULATE(") {
        if !upper.contains("ALL(") && !upper.contains("FILTER(") && !upper.contains("KEEPFILTERS") {
            return "simple".into();
        }
        return "sql_fallback".into();
    }
    if upper.contains("MEDIAN(") || upper.contains("PERCENTILE(") {
        return "sql_fallback".into();
    }
    let trimmed = expr.trim();
    if trimmed.parse::<f64>().is_ok() || trimmed.starts_with('"') {
        return "simple".into();
    }
    if upper.contains("SUM(")
        || upper.contains("COUNT(")
        || upper.contains("COUNTA(")
        || upper.contains("COUNTROWS(")
        || upper.contains("DISTINCTCOUNT(")
        || upper.contains("MIN(")
        || upper.contains("MAX(")
        || upper.contains("AVERAGE(")
    {
        return "simple".into();
    }
    if upper.contains("DIVIDE(") {
        if upper.contains("CALCULATE(")
            && !upper.contains("ALLSELECTED")
            && !upper.contains("ISONORAFTER")
            && !upper.contains("ALL(")
            && !upper.contains("FILTER(")
        {
            return "simple".into();
        }
        return "simple".into();
    }
    if upper.contains("DATATABLE(") {
        return "calculated_table".into();
    }
    // Arithmetic between measure/column references: [A] - [B], [A] * [B], etc.
    if expr.contains('[')
        && expr.contains(']')
        && (expr.contains("- [")
            || expr.contains("+ [")
            || expr.contains("* [")
            || expr.contains("/ ["))
    {
        return "sql_fallback".into();
    }
    "manual".into()
}

/// Flatten a JSON array of string fragments into a single DAX expression string.
pub fn flatten_json_array(arr: &serde_json::Value) -> String {
    match arr {
        serde_json::Value::Array(vals) => vals
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .collect::<Vec<_>>()
            .join(" "),
        serde_json::Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

/// Convert SSAS name to identifier: replace spaces and dashes with underscores, uppercase.
pub fn ssas_name_to_id(name: &str) -> String {
    name.replace([' ', '-'], "_").to_uppercase()
}

/// Convert a name to a lowercase, underscore-separated SQL identifier.
pub fn normalize_ident(name: &str) -> String {
    name.to_lowercase().replace([' ', '-'], "_")
}

/// Map BIM/SSAS data types to DuckDB types.
pub fn duckdb_type(bim_type: &str) -> &str {
    match bim_type {
        "int64" => "BIGINT",
        "double" => "DOUBLE",
        "string" => "VARCHAR",
        "dateTime" => "TIMESTAMP",
        "boolean" => "BOOLEAN",
        _ => "VARCHAR",
    }
}

/// Supported Tabular Editor source formats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TabularFormat {
    Bim,
    Folder,
    Tmdl,
}

/// Detect the source format from the given path.
///
/// - `.bim` file → `Bim`
/// - Directory with `database.tmdl` and `tables/` → `Tmdl`
/// - Directory (any) → `Folder`
/// - Otherwise → `None`
pub fn detect_format(path: &std::path::Path) -> Option<TabularFormat> {
    if path.is_file() && path.extension().is_some_and(|e| e == "bim") {
        Some(TabularFormat::Bim)
    } else if path.is_dir() && path.join("database.tmdl").exists() && path.join("tables").is_dir() {
        Some(TabularFormat::Tmdl)
    } else if path.is_dir() {
        Some(TabularFormat::Folder)
    } else {
        None
    }
}

/// Parse an ADO.NET connection string like "data source=server;initial catalog=db;user id=sa"
/// into a map. Keys are lowercased. Handles multiple aliases.
pub fn parse_ado_connection_string(conn_str: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for part in conn_str.split(';') {
        let part = part.trim();
        if let Some((key, val)) = part.split_once('=') {
            map.insert(key.trim().to_lowercase(), val.trim().to_string());
        }
    }
    map
}

/// Extract server from ADO.NET connection string map.
/// Handles aliases: "data source", "server", "addr", "address"
pub fn ado_server(map: &std::collections::HashMap<String, String>) -> String {
    map.get("data source")
        .or_else(|| map.get("server"))
        .or_else(|| map.get("addr"))
        .or_else(|| map.get("address"))
        .cloned()
        .unwrap_or_default()
}

/// Extract database from ADO.NET connection string map.
/// Handles aliases: "initial catalog", "database"
pub fn ado_database(map: &std::collections::HashMap<String, String>) -> String {
    map.get("initial catalog")
        .or_else(|| map.get("database"))
        .cloned()
        .unwrap_or_default()
}
