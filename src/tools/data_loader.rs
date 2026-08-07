//! Data loading support for Tabular Editor models.
//!
//! This module provides dummy data generation for testing when the real data source
//! is inaccessible. It produces a DuckDB SQL script that populates all tables
//! with synthetic data using `generate_series` and per-column expressions.
//!
//! ## Dummy data generation strategy
//!
//! - Fact tables get `fact_rows` rows, dimension tables get `dim_rows` rows.
//! - Date tables are skipped (use `seed_date_dim.sql` instead).
//! - Calculated tables are skipped (computed by DAX).
//! - FK columns reference parent table row ranges for referential integrity.
//! - Hidden columns are excluded.
//! - Tables with no visible columns are skipped with a comment.

use super::m_query::{extract_source, is_complex_m, SourceConnection, SourceKind};
use super::tabular_model::*;
use std::collections::{HashMap, HashSet};

/// Resolve a relationship column reference (display name) to the column's
/// `source_column` name. Relationship fromColumn/toColumn in Tabular models
/// use the column display name, but schema.sql uses `source_column`.
/// Falls back to the original name if no matching column is found.
fn resolve_source_column(tables: &[TableInfo], table_name: &str, col_name: &str) -> String {
    let table_malloy = malloy_name(table_name);
    for t in tables {
        if malloy_name(&t.name) == table_malloy {
            for c in &t.columns {
                if c.name == col_name || malloy_name(&c.name) == malloy_name(col_name) {
                    return malloy_name(&c.source_column);
                }
            }
        }
    }
    // Fallback: use the relationship column name directly
    malloy_name(col_name)
}

/// Render a DuckDB SQL script that populates all tables with dummy data.
///
/// - Fact tables get `fact_rows` rows (default 10000)
/// - Dimension tables get `dim_rows` rows (default 1000)
/// - Date tables are skipped (use `seed_date_dim.sql` instead)
/// - FK columns reference parent table row ranges for referential integrity
/// - Calculated tables and hidden columns are skipped
pub fn render_dummy_data_script(
    model: &TabularModel,
    fact_table_names: &[String],
    date_role_names: &[String],
    fact_rows: usize,
    dim_rows: usize,
) -> String {
    let mut out = String::new();

    // ── Header ──
    out.push_str("-- Dummy data generation script\n");
    out.push_str(&format!("-- Generated from Tabular Editor model: {}\n", model.name));
    out.push_str(&format!(
        "-- Fact tables: {} rows, Dimension tables: {} rows\n",
        fact_rows, dim_rows
    ));
    out.push_str("-- Date tables: use seed_date_dim.sql instead\n");
    out.push('\n');

    // Normalise input table-name lists for case/spacing-insensitive matching.
    let fact_names: Vec<String> = fact_table_names.iter().map(|n| malloy_name(n)).collect();
    let date_names: Vec<String> = date_role_names.iter().map(|n| malloy_name(n)).collect();

    // ── FK map: (from_table_malloy, from_column_malloy) → (to_table_malloy, to_column_malloy) ──
    // Relationship fromColumn/toColumn use display names; resolve to source_column
    // to match the schema.sql column identifiers.
    let mut fk_map: HashMap<(String, String), (String, String)> = HashMap::new();
    for rel in &model.relationships {
        let from_table_malloy = malloy_name(&rel.from_table);
        let from_col_malloy = resolve_source_column(&model.tables, &rel.from_table, &rel.from_column);
        let to_table_malloy = malloy_name(&rel.to_table);
        let to_col_malloy = resolve_source_column(&model.tables, &rel.to_table, &rel.to_column);
        fk_map.insert(
            (from_table_malloy, from_col_malloy),
            (to_table_malloy, to_col_malloy),
        );
    }

    // ── Row-count map ──
    let mut row_counts: HashMap<String, usize> = HashMap::new();
    for table in &model.tables {
        let name = malloy_name(&table.name);
        let count = if fact_names.contains(&name) {
            fact_rows
        } else {
            dim_rows
        };
        row_counts.insert(name, count);
    }
    // Ensure every table referenced in a relationship has an entry
    // (even if it wouldn't generate an INSERT, e.g. date tables).
    for rel in &model.relationships {
        let from = malloy_name(&rel.from_table);
        let to = malloy_name(&rel.to_table);
        row_counts.entry(from).or_insert(dim_rows);
        row_counts.entry(to).or_insert(dim_rows);
    }

    // ── Per-table generation ──
    for table in &model.tables {
        let table_name_malloy = malloy_name(&table.name);

        // Skip date tables (populated by seed_date_dim.sql)
        if date_names.contains(&table_name_malloy) {
            out.push_str(&format!(
                "-- Date table '{}' is populated by seed_date_dim.sql\n",
                table.name
            ));
            out.push('\n');
            continue;
        }

        // Skip calculated tables / calculation groups (computed by DAX at query time)
        if table.is_calculated() || is_calc_group(table) {
            out.push_str(&format!(
                "-- Calculated table '{}' is computed by DAX, not loaded\n",
                table.name
            ));
            out.push('\n');
            continue;
        }

        // All columns get dummy data (including hidden, for FK join integrity)
        let columns: Vec<&ColumnInfo> = table.columns.iter().collect();

        if columns.is_empty() {
            out.push_str(&format!(
                "-- Table '{}' has no columns, skipping\n",
                table.name
            ));
            out.push('\n');
            continue;
        }

        let row_count = row_counts
            .get(&table_name_malloy)
            .copied()
            .unwrap_or(dim_rows);
        let is_fact = fact_names.contains(&table_name_malloy);
        let table_type = if is_fact { "fact" } else { "dimension" };

        out.push_str(&format!(
            "-- Table: {} ({}, {} rows)\n",
            table_name_malloy, table_type, row_count
        ));

        // Column name list (malloy-normalised, matches schema.sql identifiers)
        let col_names: Vec<String> = columns.iter().map(|c| malloy_name(&c.source_column)).collect();
        out.push_str(&format!(
            "INSERT INTO {} ({})\nSELECT\n",
            table_name_malloy,
            col_names.join(", ")
        ));

        // Build per-column SELECT expressions
        let mut exprs: Vec<String> = Vec::new();
        for col in &columns {
            let col_malloy = malloy_name(&col.source_column);
            let fk_key = (table_name_malloy.clone(), col_malloy.clone());

            let expr = if let Some((to_table, _to_col)) = fk_map.get(&fk_key) {
                // Check the FK column's DuckDB type first — if it's TIMESTAMP or DATE,
                // generate a date expression instead of an integer modulo.
                let db_type = duckdb_type(&col.data_type);
                let val = match db_type {
                    "TIMESTAMP" => {
                        "TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day'"
                            .to_string()
                    }
                    "DATE" => {
                        "DATE '2020-01-01' + (i % 365) * INTERVAL '1 day'".to_string()
                    }
                    _ if date_names.contains(to_table) => {
                        // ── FK to date table: generate date_key-style values ──
                        "strftime(DATE '2020-01-01' + (i % 4018) * INTERVAL '1 day', '%Y%m%d')::INTEGER"
                            .to_string()
                    }
                    _ => {
                        // ── FK column: modulo parent row count ──
                        let parent_count = row_counts.get(to_table.as_str()).copied().unwrap_or(dim_rows);
                        format!("(i % {}) + 1", parent_count)
                    }
                };
                format!("    {} AS {}", val, col_malloy)
            } else {
                // ── Regular column: expression based on DuckDB type ──
                let db_type = duckdb_type(&col.data_type);
                let val = match db_type {
                    "BIGINT" => "i".to_string(),
                    "DOUBLE" => {
                        "round((random() * 1000)::DECIMAL(10,2), 2)".to_string()
                    }
                    "VARCHAR" => {
                        "'Item_' || lpad(i::VARCHAR, 4, '0')".to_string()
                    }
                    "TIMESTAMP" => {
                        "TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day'"
                            .to_string()
                    }
                    "BOOLEAN" => "i % 2 = 0".to_string(),
                    // DuckDB does not have a native DATE mapping from duckdb_type()
                    // but handle safely if it ever appears.
                    "DATE" => "DATE '2020-01-01' + ((i * 7 + 3) % 365)".to_string(),
                    _ => "NULL".to_string(),
                };
                format!("    {} AS {}", val, col_malloy)
            };
            exprs.push(expr);
        }

        out.push_str(&exprs.join(",\n"));
        out.push_str(&format!("\nFROM generate_series(1, {}) t(i);\n\n", row_count));
    }

    out
}

// ============================================================================
// Real data loading (load from partition sources)
// ============================================================================

/// Render a DuckDB SQL script that loads real data from the Tabular model's
/// partition sources.
///
/// For each table with a resolvable data source:
/// - `"query"` type partitions: use the SQL query directly
/// - `"m"` type partitions: extract source via `m_query::extract_source()`
/// - `"calculated"` / `"calculationGroup"` type partitions: skip
/// - Unresolvable sources: emit a comment with manual load instructions
///
/// Always fails safe — never generates potentially wrong SQL.
pub fn render_load_script(
    model: &TabularModel,
    fact_table_names: &[String],
    date_role_names: &[String],
) -> String {
    let mut out = String::new();

    // ── Header ──
    out.push_str("-- Data loading script\n");
    out.push_str(&format!("-- Generated from Tabular Editor model: {}\n", model.name));
    out.push_str("-- Requires DuckDB extensions for external database sources\n");
    out.push('\n');

    // Normalise date-role names for case/spacing-insensitive matching.
    let date_names: Vec<String> = date_role_names.iter().map(|n| malloy_name(n)).collect();

    // Data source lookup by name.
    let data_source_map: HashMap<&str, &DataSourceInfo> =
        model.data_sources.iter().map(|ds| (ds.name.as_str(), ds)).collect();

    // Track ATTACH statements already emitted (deduplication).
    let mut attach_emitted: HashSet<String> = HashSet::new();
    let mut attach_counter: usize = 0;
    let mut alias_map: HashMap<String, String> = HashMap::new();

    // Emit ATTACH header once if there's at least one SQL Server source.
    let needs_sqlserver_header = model.data_sources.iter().any(|ds| {
        ds.provider.contains("SqlClient") || ds.provider.contains("OLEDB")
    });
    if needs_sqlserver_header {
        out.push_str("-- SQL Server sources detected\n");
        out.push_str("-- Requires: INSTALL sqlserver_scanner; LOAD sqlserver_scanner;\n");
        out.push_str("-- If extension unavailable, export to CSV and use: INSERT INTO t SELECT * FROM read_csv_auto('file.csv');\n");
        out.push('\n');
    }

    // ── Per-table processing ──
    for table in &model.tables {
        let table_name_malloy = malloy_name(&table.name);

        // Skip date tables (populated by seed_date_dim.sql)
        if date_names.contains(&table_name_malloy) {
            out.push_str(&format!(
                "-- Date table '{}' is populated by seed_date_dim.sql\n",
                table.name
            ));
            out.push('\n');
            continue;
        }

        // Skip calculated tables / calculation groups (computed by DAX)
        if table.is_calculated() || is_calc_group(table) {
            out.push_str(&format!(
                "-- Calculated table '{}' is computed by DAX, not loaded\n",
                table.name
            ));
            out.push('\n');
            continue;
        }

        // Find the best partition (one with a query/expression, or first)
        let partition = table
            .partitions
            .iter()
            .find(|p| p.query.is_some())
            .or_else(|| table.partitions.first());

        let Some(partition) = partition else {
            out.push_str(&format!(
                "-- No partition source found for table '{}', manual load required\n",
                table.name
            ));
            out.push('\n');
            continue;
        };

        match partition.source_type.as_str() {
            "query" => render_query_partition(
                &mut out,
                table,
                partition,
                &data_source_map,
                &mut attach_emitted,
                &mut attach_counter,
                &mut alias_map,
            ),
            "m" => render_m_partition(
                &mut out,
                table,
                partition,
                &data_source_map,
                &mut attach_emitted,
                &mut attach_counter,
                &mut alias_map,
            ),
            "calculated" => {
                out.push_str(&format!(
                    "-- Calculated table '{}' is computed by DAX, not loaded\n",
                    table.name
                ));
            }
            "calculationGroup" => {
                out.push_str(&format!(
                    "-- Calculated table '{}' is computed by DAX, not loaded\n",
                    table.name
                ));
            }
            other => {
                out.push_str(&format!(
                    "-- Unrecognized partition type '{}' for table '{}', manual load required\n",
                    other,
                    table.name
                ));
            }
        }
        out.push('\n');
    }

    out
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Check if a table is a calculation group.
fn is_calc_group(table: &TableInfo) -> bool {
    table.partitions.iter().any(|p| p.source_type == "calculationGroup")
}

/// Render SQL for a "query" type partition (direct SQL from the BIM source).
fn render_query_partition(
    out: &mut String,
    table: &TableInfo,
    partition: &PartitionInfo,
    data_source_map: &HashMap<&str, &DataSourceInfo>,
    attach_emitted: &mut HashSet<String>,
    attach_counter: &mut usize,
    alias_map: &mut HashMap<String, String>,
) {
    let table_name_malloy = malloy_name(&table.name);
    let sql = partition.query.as_deref().unwrap_or("");

    if sql.is_empty() {
        out.push_str(&format!(
            "-- Empty query for table '{}', manual load required\n",
            table.name
        ));
        return;
    }

    let ds_name = partition.data_source_name.as_deref();
    let ds = ds_name.and_then(|name| data_source_map.get(name)).copied();

    if let Some(ds) = ds {
        // Deduplicate ATTACH by data source name
        let dn = ds_name.unwrap_or("");
        let attach_str = translate_to_duckdb_attach(ds);
        if !attach_emitted.contains(dn) {
            *attach_counter += 1;
            let alias = format!("src_{}", attach_counter);
            out.push_str(&format!(
                "ATTACH '{}' AS {} (TYPE mssql);\n",
                attach_str, alias
            ));
            attach_emitted.insert(dn.to_string());
            alias_map.insert(dn.to_string(), alias);
        }
        let alias = alias_map.get(dn).map(|s| s.as_str()).unwrap_or("src_?");
        let schema = partition.schema.as_deref().unwrap_or("dbo");

        // Bug 7: warn about missing credentials (Windows Auth not supported by DuckDB mssql scanner)
        if !attach_str.contains("User=") {
            out.push_str("-- WARNING: No credentials in connection string. Windows Auth (Integrated Security) may not work with DuckDB's mssql scanner.\n");
            out.push_str("-- Consider using SQL auth: add User= and Password= to the connection string.\n");
        }

        out.push_str(&format!(
            "-- Table: {} (query partition)\n",
            table_name_malloy
        ));
        out.push_str(&format!(
            "-- Raw SQL (SQL Server dialect, cannot run in DuckDB directly):\n\
             -- {}\n\
             -- Rewrite using the ATTACH alias, e.g.:\n\
             -- INSERT INTO {} SELECT * FROM {}.{}.<source_table>;\n",
            sql, table_name_malloy, alias, schema
        ));
    } else {
        // No DataSourceInfo available (e.g. BIM-originated model with no data sources)
        out.push_str(&format!(
            "-- Table: {} (query partition)\n",
            table_name_malloy
        ));
        if let (Some(_schema), Some(_database)) = (&partition.schema, &partition.database) {
            out.push_str(&format!(
                "-- TODO: Data source '{}' connection info not available. Load manually.\n",
                ds_name.unwrap_or("(unknown)")
            ));
            out.push_str("-- Export to CSV and use:\n");
            out.push_str(&format!(
                "-- INSERT INTO {} SELECT * FROM read_csv_auto('path/table.csv');\n",
                table_name_malloy
            ));
        } else {
            out.push_str(&format!(
                "-- TODO: Data source '{}' connection info not available. Load manually.\n",
                ds_name.unwrap_or("(unknown)")
            ));
            out.push_str(&format!(
                "-- INSERT INTO {} SELECT * FROM read_csv_auto('path/table.csv');\n",
                table_name_malloy
            ));
        }
    }
}

/// Render SQL/instructions for an "m" (Power Query / M) type partition.
fn render_m_partition(
    out: &mut String,
    table: &TableInfo,
    partition: &PartitionInfo,
    data_source_map: &HashMap<&str, &DataSourceInfo>,
    attach_emitted: &mut HashSet<String>,
    attach_counter: &mut usize,
    alias_map: &mut HashMap<String, String>,
) {
    let table_name_malloy = malloy_name(&table.name);
    let m_expr = partition.query.as_deref().unwrap_or("");

    if m_expr.is_empty() {
        out.push_str(&format!(
            "-- Empty M expression for table '{}', manual load required\n",
            table.name
        ));
        return;
    }

    let conn = extract_source(m_expr);

    match conn {
        Some(sc) => render_m_connection(out, table_name_malloy, &sc, m_expr, partition, data_source_map, attach_emitted, attach_counter, alias_map),
        None => {
            out.push_str(&format!(
                "-- Table: {} (M partition, unrecognized)\n",
                table_name_malloy
            ));
            if is_complex_m(m_expr) {
                out.push_str("-- Complex M expression, manual load required\n");
            } else {
                out.push_str("-- Unrecognized M expression, manual load required\n");
            }
            out.push_str(&format!(
                "-- M expression (first 200 chars): {}\n",
                truncate_m(m_expr, 200)
            ));
        }
    }
}

/// Map a resolved `SourceConnection` to DuckDB SQL / instructions.
fn render_m_connection(
    out: &mut String,
    table_name_malloy: String,
    conn: &SourceConnection,
    _original_m: &str,
    partition: &PartitionInfo,
    data_source_map: &HashMap<&str, &DataSourceInfo>,
    attach_emitted: &mut HashSet<String>,
    attach_counter: &mut usize,
    alias_map: &mut HashMap<String, String>,
) {
    match conn.kind {
        SourceKind::SqlServer => {
            let server = conn.server.as_deref().unwrap_or("(unknown)");
            let database = conn.database.as_deref().unwrap_or("(unknown)");
            let schema = conn.schema.as_deref().unwrap_or("dbo");
            let table_n = conn.table.as_deref().unwrap_or("(unknown)");

            out.push_str(&format!(
                "-- Table: {} (M partition, SQL Server)\n",
                table_name_malloy
            ));
            out.push_str("-- Requires: INSTALL sqlserver_scanner; LOAD sqlserver_scanner;\n");
            out.push_str("-- If extension unavailable, export to CSV and use: INSERT INTO t SELECT * FROM read_csv_auto('file.csv');\n");

            // Try to enrich ATTACH with credentials from DataSourceInfo
            let attach_str = if let Some(ds_name) = &partition.data_source_name {
                if let Some(ds) = data_source_map.get(ds_name.as_str()) {
                    translate_to_duckdb_attach(ds)
                } else {
                    format!("Server={};Database={}", server, database)
                }
            } else {
                format!("Server={};Database={}", server, database)
            };

            // Deduplicate ATTACH: use data_source_name if available, otherwise server:database
            let dedup_key = if let Some(ds_name) = &partition.data_source_name {
                ds_name.clone()
            } else {
                format!("{}:{}", server, database)
            };
            if !attach_emitted.contains(&dedup_key) {
                *attach_counter += 1;
                let alias = format!("src_{}", attach_counter);
                out.push_str(&format!(
                    "ATTACH '{}' AS {} (TYPE mssql);\n",
                    attach_str, alias
                ));
                attach_emitted.insert(dedup_key.clone());
                alias_map.insert(dedup_key.clone(), alias);
            }
            let alias = alias_map.get(&dedup_key).map(|s| s.as_str()).unwrap_or("src_?");

            // Bug 7: warn about missing credentials (Windows Auth not supported by DuckDB mssql scanner)
            if !attach_str.contains("User=") {
                out.push_str("-- WARNING: No credentials in connection string. Windows Auth (Integrated Security) may not work with DuckDB's mssql scanner.\n");
                out.push_str("-- Consider using SQL auth: add User= and Password= to the connection string.\n");
            }

            out.push_str(&format!(
                "INSERT INTO {} SELECT * FROM {}.{}.\"{}\";\n",
                table_name_malloy, alias, schema, table_n
            ));
        }
        SourceKind::CSV => {
            out.push_str(&format!(
                "-- Table: {} (M partition, CSV source)\n",
                table_name_malloy
            ));

            if let Some(path) = &conn.file_path {
                out.push_str(&format!(
                    "INSERT INTO {} SELECT * FROM read_csv_auto('{}');\n",
                    table_name_malloy, path
                ));
            } else if let Some(rel_path) = &conn.relative_path {
                out.push_str(&format!("-- CSV source: relative path '{}'\n", rel_path));
                if let Some(url) = &conn.url {
                    out.push_str(&format!("-- URL: {}\n", url));
                } else {
                    out.push_str("-- The CSV is served from a Web.Contents source with a parameterized URL that cannot be resolved.\n");
                }
                let filename = rel_path.rsplit('/').next().unwrap_or(rel_path);
                out.push_str("-- If you have a local copy of the CSV files, use:\n");
                out.push_str(&format!(
                    "-- INSERT INTO {} SELECT * FROM read_csv_auto('path/to/{}');\n",
                    table_name_malloy, filename
                ));
            }
        }
        SourceKind::Postgres => {
            out.push_str(&format!(
                "-- Table: {} (M partition, PostgreSQL source)\n",
                table_name_malloy
            ));
            let server = conn.server.as_deref().unwrap_or("(unknown)");
            let database = conn.database.as_deref().unwrap_or("(unknown)");
            out.push_str("-- Requires: INSTALL postgres_scanner; LOAD postgres_scanner;\n");
            out.push_str(&format!(
                "-- ATTACH 'host={} dbname={}' AS src (TYPE postgres);\n",
                server, database
            ));
        }
        SourceKind::MySQL => {
            out.push_str(&format!(
                "-- Table: {} (M partition, MySQL source)\n",
                table_name_malloy
            ));
            let server = conn.server.as_deref().unwrap_or("(unknown)");
            let database = conn.database.as_deref().unwrap_or("(unknown)");
            out.push_str("-- Requires: INSTALL mysql_scanner; LOAD mysql_scanner;\n");
            out.push_str(&format!(
                "-- ATTACH 'host={} database={}' AS src (TYPE mysql);\n",
                server, database
            ));
        }
        SourceKind::Web => {
            out.push_str(&format!(
                "-- Table: {} (M partition, Web source)\n",
                table_name_malloy
            ));
            out.push_str(&format!(
                "-- URL: {}\n",
                conn.url.as_deref().unwrap_or("(unknown)")
            ));
            if let Some(rel_path) = &conn.relative_path {
                out.push_str(&format!("-- Relative path: {}\n", rel_path));
            }
            out.push_str("-- Manual download required.\n");
        }
        SourceKind::Excel => {
            out.push_str(&format!(
                "-- Table: {} (M partition, Excel source)\n",
                table_name_malloy
            ));
            if let Some(path) = &conn.file_path {
                out.push_str(&format!("-- File: {}\n", path));
            }
            out.push_str("-- Manual conversion to CSV required.\n");
        }
        SourceKind::Unknown => {
            out.push_str(&format!(
                "-- Table: {} (M partition, unknown source type)\n",
                table_name_malloy
            ));
            if let Some(schema) = &conn.schema {
                out.push_str(&format!(
                    "-- Schema: {}, Table: {}\n",
                    schema,
                    conn.table.as_deref().unwrap_or("?")
                ));
            }
            out.push_str("-- Manual load required.\n");
        }
    }
}

/// Translate an ADO.NET connection string to DuckDB ATTACH format.
pub fn translate_to_duckdb_attach(ds: &DataSourceInfo) -> String {
    let map = parse_ado_connection_string(&ds.connection_string);
    let server = ado_server(&map);
    let database = ado_database(&map);
    let user = map.get("user id").or_else(|| map.get("uid")).cloned();
    let password = map.get("password").or_else(|| map.get("pwd")).cloned();

    let mut conn = format!("Server={};Database={}", server, database);
    if let Some(u) = &user {
        conn.push_str(&format!(";User={}", u));
    }
    if let Some(p) = &password {
        conn.push_str(&format!(";Password={}", p));
    }
    conn
}

/// Truncate an M expression to `max_chars` for display in comments.
/// Uses char-based truncation to avoid multi-byte UTF-8 panic.
fn truncate_m(expr: &str, max_chars: usize) -> String {
    let one_line = expr.replace('\n', " ").replace('\r', "");
    if one_line.chars().count() > max_chars {
        let truncated: String = one_line.chars().take(max_chars).collect();
        format!("{}...", truncated)
    } else {
        one_line
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::parse_folder;

    // ── Helpers ───────────────────────────────────────────────────────────

    /// Minimal two-table model: Sales (fact) + Products (dimension) with one FK.
    fn make_minimal_model() -> TabularModel {
        TabularModel {
            name: "TestModel".into(),
            compatibility_level: 1200,
            tables: vec![
                TableInfo {
                    name: "Sales".into(),
                    ssas_name: "Sales".into(),
                    description: String::new(),
                    columns: vec![
                        ColumnInfo {
                            name: "Transaction ID".into(),
                            data_type: "int64".into(),
                            source_column: "salesid".into(),
                            is_hidden: false,
                        },
                        ColumnInfo {
                            name: "Product ID".into(),
                            data_type: "int64".into(),
                            source_column: "productid".into(),
                            is_hidden: false,
                        },
                        ColumnInfo {
                            name: "Unit Price".into(),
                            data_type: "double".into(),
                            source_column: "unitprice".into(),
                            is_hidden: false,
                        },
                    ],
                    measures: vec![],
                    partitions: vec![PartitionInfo {
                        name: "vw_sales".into(),
                        source_type: "query".into(),
                        is_m: false,
                        query: Some("SELECT * FROM [dbo].[vw_sales]".into()),
                        data_source_name: None,
                        mode: None,
                        schema: None,
                        database: None,
                    }],
                    hierarchies: vec![],
                },
                TableInfo {
                    name: "Products".into(),
                    ssas_name: "Products".into(),
                    description: String::new(),
                    columns: vec![
                        ColumnInfo {
                            name: "Product ID".into(),
                            data_type: "int64".into(),
                            source_column: "productid".into(),
                            is_hidden: false,
                        },
                        ColumnInfo {
                            name: "Product Name".into(),
                            data_type: "string".into(),
                            source_column: "name".into(),
                            is_hidden: false,
                        },
                    ],
                    measures: vec![],
                    partitions: vec![PartitionInfo {
                        name: "vw_products".into(),
                        source_type: "query".into(),
                        is_m: false,
                        query: None,
                        data_source_name: None,
                        mode: None,
                        schema: None,
                        database: None,
                    }],
                    hierarchies: vec![],
                },
            ],
            relationships: vec![RelInfo {
                from_table: "Sales".into(),
                from_column: "productid".into(),
                to_table: "Products".into(),
                to_column: "productid".into(),
            }],
            roles: vec![],
            data_sources: vec![],
        }
    }

    /// Adds a Dates table to the minimal model.
    /// Also gives Products a query so load script tests work.
    fn model_with_dates() -> TabularModel {
        let mut model = make_minimal_model();
        // Give Products a query so the load script can process it
        model.tables[1].partitions[0].query = Some("SELECT * FROM [dbo].[vw_products]".into());
        model.tables.push(TableInfo {
            name: "Dates".into(),
            ssas_name: "Dates".into(),
            description: String::new(),
            columns: vec![
                ColumnInfo {
                    name: "Date Key".into(),
                    data_type: "int64".into(),
                    source_column: "datekey".into(),
                    is_hidden: false,
                },
                ColumnInfo {
                    name: "Calendar Date".into(),
                    data_type: "dateTime".into(),
                    source_column: "fulldate".into(),
                    is_hidden: false,
                },
            ],
            measures: vec![],
            partitions: vec![PartitionInfo {
                name: "vw_dates".into(),
                source_type: "query".into(),
                is_m: false,
                query: None,
                data_source_name: None,
                mode: None,
                schema: None,
                database: None,
            }],
            hierarchies: vec![],
        });
        model
    }

    /// Adds a calculated table (State) to the minimal model.
    /// Also gives Products a query so load script tests work.
    fn model_with_calculated() -> TabularModel {
        let mut model = make_minimal_model();
        // Give Products a query so the load script can process it
        model.tables[1].partitions[0].query = Some("SELECT * FROM [dbo].[vw_products]".into());
        model.tables.push(TableInfo {
            name: "State".into(),
            ssas_name: "State".into(),
            description: String::new(),
            columns: vec![],
            measures: vec![],
            partitions: vec![PartitionInfo {
                name: "State".into(),
                source_type: "calculated".into(),
                is_m: false,
                query: Some("DATATABLE(...)".into()),
                data_source_name: None,
                mode: Some("import".into()),
                schema: None,
                database: None,
            }],
            hierarchies: vec![],
        });
        model
    }

    /// Model with one table containing one column of each supported type.
    fn model_all_types() -> TabularModel {
        TabularModel {
            name: "TypeTest".into(),
            compatibility_level: 1200,
            tables: vec![TableInfo {
                name: "AllTypes".into(),
                ssas_name: "AllTypes".into(),
                description: String::new(),
                columns: vec![
                    ColumnInfo {
                        name: "Big Int Col".into(),
                        data_type: "int64".into(),
                        source_column: "bigintcol".into(),
                        is_hidden: false,
                    },
                    ColumnInfo {
                        name: "Double Col".into(),
                        data_type: "double".into(),
                        source_column: "doublecol".into(),
                        is_hidden: false,
                    },
                    ColumnInfo {
                        name: "Varchar Col".into(),
                        data_type: "string".into(),
                        source_column: "varcharcol".into(),
                        is_hidden: false,
                    },
                    ColumnInfo {
                        name: "Timestamp Col".into(),
                        data_type: "dateTime".into(),
                        source_column: "tscol".into(),
                        is_hidden: false,
                    },
                    ColumnInfo {
                        name: "Bool Col".into(),
                        data_type: "boolean".into(),
                        source_column: "boolcol".into(),
                        is_hidden: false,
                    },
                ],
                measures: vec![],
                partitions: vec![PartitionInfo {
                    name: "part".into(),
                    source_type: "query".into(),
                    is_m: false,
                    query: None,
                    data_source_name: None,
                    mode: None,
                    schema: None,
                    database: None,
                }],
                hierarchies: vec![],
            }],
            relationships: vec![],
            roles: vec![],
            data_sources: vec![],
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────

    #[test]
    fn test_render_dummy_data_basic() {
        let model = make_minimal_model();
        let result = render_dummy_data_script(&model, &["Sales".into()], &[], 10000, 1000);

        // Header
        assert!(result.contains("Dummy data generation script"));
        assert!(result.contains("TestModel"));
        assert!(result.contains("Fact tables: 10000 rows, Dimension tables: 1000 rows"));

        // INSERT INTO for both tables (sales fact, products dimension)
        assert!(result.contains("INSERT INTO sales"));
        assert!(result.contains("INSERT INTO products"));

        // Row counts
        assert!(result.contains("generate_series(1, 10000)"));
        assert!(result.contains("generate_series(1, 1000)"));

        // Column names in INSERT lists (use source_column to match schema.sql)
        assert!(result.contains("salesid"));
        assert!(result.contains("productid"));
        assert!(result.contains("unitprice"));
        assert!(result.contains("name"));
    }

    #[test]
    fn test_fk_generation() {
        let model = make_minimal_model();
        let result = render_dummy_data_script(&model, &["Sales".into()], &[], 10000, 1000);

        // Product ID in Sales is a FK to Products (1000 rows)
        assert!(result.contains("(i % 1000) + 1 AS productid"));

        // Transaction ID is NOT a FK — gets `i` (BIGINT)
        assert!(result.contains("i AS salesid"));
    }

    #[test]
    fn test_date_table_skipped() {
        let model = model_with_dates();
        let result = render_dummy_data_script(
            &model,
            &["Sales".into()],
            &["Dates".into()],
            10000,
            1000,
        );

        // Dates table should be skipped with a comment
        assert!(result.contains("is populated by seed_date_dim.sql"));
        assert!(!result.contains("INSERT INTO dates"));

        // Sales and Products should still be present
        assert!(result.contains("INSERT INTO sales"));
        assert!(result.contains("INSERT INTO products"));
    }

    #[test]
    fn test_calculated_table_skipped() {
        let model = model_with_calculated();
        let result = render_dummy_data_script(&model, &["Sales".into()], &[], 10000, 1000);

        // State table should be skipped with a comment
        assert!(result.contains("is computed by DAX, not loaded"));
        assert!(!result.contains("INSERT INTO state"));
        assert!(!result.contains("insert into state"));

        // Sales and Products should still be present
        assert!(result.contains("INSERT INTO sales"));
        assert!(result.contains("INSERT INTO products"));
    }

    #[test]
    fn test_column_type_mapping() {
        let model = model_all_types();
        let result = render_dummy_data_script(&model, &[], &[], 100, 100);

        // BIGINT
        assert!(result.contains("i AS bigintcol"));

        // DOUBLE
        assert!(result.contains(
            "round((random() * 1000)::DECIMAL(10,2), 2) AS doublecol"
        ));

        // VARCHAR
        assert!(result.contains(
            "'Item_' || lpad(i::VARCHAR, 4, '0') AS varcharcol"
        ));

        // TIMESTAMP
        assert!(result.contains(
            "TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day' AS tscol"
        ));

        // BOOLEAN
        assert!(result.contains("i % 2 = 0 AS boolcol"));
    }

    #[test]
    fn test_fact_vs_dim_row_counts() {
        // Use a slightly asymmetric count to verify the distinction.
        let model = make_minimal_model();
        let result = render_dummy_data_script(&model, &["Sales".into()], &[], 7777, 333);

        // Sales is a fact table → 7777 rows
        assert!(result.contains("generate_series(1, 7777)"));
        // Products is a dimension → 333 rows
        assert!(result.contains("generate_series(1, 333)"));

        // Verify labels in comments
        assert!(result.contains("(fact, 7777 rows)"));
        assert!(result.contains("(dimension, 333 rows)"));
    }

    #[test]
    fn test_hidden_columns_skipped() {
        let model = TabularModel {
            name: "HiddenTest".into(),
            compatibility_level: 1200,
            tables: vec![TableInfo {
                name: "Employees".into(),
                ssas_name: "Employees".into(),
                description: String::new(),
                columns: vec![
                    ColumnInfo {
                        name: "Employee ID".into(),
                        data_type: "int64".into(),
                        source_column: "empid".into(),
                        is_hidden: false,
                    },
                    ColumnInfo {
                        name: "Salary".into(),
                        data_type: "double".into(),
                        source_column: "salary".into(),
                        is_hidden: true,
                    },
                ],
                measures: vec![],
                partitions: vec![PartitionInfo {
                    name: "part".into(),
                    source_type: "query".into(),
                    is_m: false,
                    query: None,
                    data_source_name: None,
                    mode: None,
                    schema: None,
                    database: None,
                }],
                hierarchies: vec![],
            }],
            relationships: vec![],
            roles: vec![],
            data_sources: vec![],
        };

        let result = render_dummy_data_script(&model, &[], &[], 100, 100);
        // Employee ID should be present (source_column: empid)
        assert!(result.contains("empid"));
        // Salary (hidden) should now BE present (included in dummy data for FK integrity)
        assert!(result.contains("salary"));
    }

    #[test]
    fn test_empty_visible_columns_skipped() {
        let model = TabularModel {
            name: "EmptyTest".into(),
            compatibility_level: 1200,
            tables: vec![TableInfo {
                name: "EmptyTable".into(),
                ssas_name: "EmptyTable".into(),
                description: String::new(),
                columns: vec![ColumnInfo {
                    name: "HiddenCol".into(),
                    data_type: "int64".into(),
                    source_column: "hc".into(),
                    is_hidden: true,
                }],
                measures: vec![],
                partitions: vec![PartitionInfo {
                    name: "part".into(),
                    source_type: "query".into(),
                    is_m: false,
                    query: None,
                    data_source_name: None,
                    mode: None,
                    schema: None,
                    database: None,
                }],
                hierarchies: vec![],
            }],
            relationships: vec![],
            roles: vec![],
            data_sources: vec![],
        };

        let result = render_dummy_data_script(&model, &[], &[], 100, 100);
        // Table with only hidden columns now generates data (hidden columns included)
        assert!(!result.contains("has no columns, skipping"));
        assert!(result.contains("INSERT INTO emptytable"));
        assert!(result.contains("hc")); // hidden column now included
    }

    #[test]
    fn test_unknown_type_defaults_to_varchar() {
        let model = TabularModel {
            name: "UnknownType".into(),
            compatibility_level: 1200,
            tables: vec![TableInfo {
                name: "Weird".into(),
                ssas_name: "Weird".into(),
                description: String::new(),
                columns: vec![ColumnInfo {
                    name: "Unknown Col".into(),
                    data_type: "someUnknownType".into(),
                    source_column: "uc".into(),
                    is_hidden: false,
                }],
                measures: vec![],
                partitions: vec![PartitionInfo {
                    name: "part".into(),
                    source_type: "query".into(),
                    is_m: false,
                    query: None,
                    data_source_name: None,
                    mode: None,
                    schema: None,
                    database: None,
                }],
                hierarchies: vec![],
            }],
            relationships: vec![],
            roles: vec![],
            data_sources: vec![],
        };

        let result = render_dummy_data_script(&model, &[], &[], 100, 100);
        // duckdb_type returns VARCHAR for unknown BIM types
        assert!(result.contains("Item_"));
        assert!(result.contains("uc"));
    }

    #[test]
    fn test_retail_fixture_dummy_data() {
        let (model, _warnings) = parse_folder::parse_model("data/retailanalytics_tabular");

        let result = render_dummy_data_script(
            &model,
            &["Sales".into()],
            &["Dates".into()],
            10000,
            1000,
        );

        // Must contain INSERT INTO for all non-date, non-calculated tables
        assert!(result.contains("INSERT INTO sales"));
        assert!(result.contains("INSERT INTO products"));
        assert!(result.contains("INSERT INTO customers"));
        assert!(result.contains("INSERT INTO stores"));
        assert!(result.contains("INSERT INTO promotions"));

        // Must NOT contain INSERT INTO for the date table
        assert!(!result.contains("INSERT INTO dates"));

        // Must NOT contain INSERT INTO for the calculated table (DAX)
        assert!(!result.contains("INSERT INTO dax"));

        // Must have skip comments
        assert!(result.contains("is populated by seed_date_dim.sql"));
        assert!(result.contains("is computed by DAX, not loaded"));

        // Row counts
        assert!(result.contains("generate_series(1, 10000)"));
        assert!(result.contains("generate_series(1, 1000)"));

        // Verify FK expressions reference correct parent counts
        // Sales.Product ID → Products (1000 dim rows)
        assert!(result.contains("(i % 1000) + 1 AS productid"));

        // Sales.Store ID → Stores (1000 dim rows)
        assert!(result.contains("(i % 1000) + 1 AS storeid"));

        // Sales.Customer ID → Customers (1000 dim rows)
        assert!(result.contains("(i % 1000) + 1 AS customerid"));

        // Sales.Promotion ID → Promotions (1000 dim rows)
        assert!(result.contains("(i % 1000) + 1 AS promoid"));

        // Sales.Date Key → Dates is a date table, generates date_key-style values
        assert!(result.contains("strftime(DATE '2020-01-01' + (i % 4018) * INTERVAL '1 day', '%Y%m%d')::INTEGER AS datekey"));
    }

    // ── Load script tests ─────────────────────────────────────────────────

    /// Helper: model with a query partition and a shared data source.
    fn make_model_with_data_source() -> TabularModel {
        let mut model = make_minimal_model();
        model.data_sources.push(DataSourceInfo {
            name: "MySource".into(),
            provider: "System.Data.SqlClient".into(),
            server: "DESKTOP-SRV\\MSSQLSERVER".into(),
            database: "retaildb".into(),
            connection_string: "data source=DESKTOP-SRV\\MSSQLSERVER;initial catalog=retaildb;user id=sa".into(),
        });
        // Wire up Sales partition to use the data source
        model.tables[0].partitions[0].data_source_name = Some("MySource".into());
        // Also wire Products partition
        model.tables[1].partitions[0].query = Some("SELECT * FROM [dbo].[vw_products]".into());
        model.tables[1].partitions[0].data_source_name = Some("MySource".into());
        model
    }

    #[test]
    fn test_render_load_script_query_partition() {
        let model = make_model_with_data_source();
        let result = render_load_script(&model, &["Sales".into()], &[]);

        // Header
        assert!(result.contains("Data loading script"));
        assert!(result.contains("Requires DuckDB extensions"));

        // Should have one ATTACH (shared data source)
        assert!(result.contains("ATTACH"));
        assert!(result.contains("System.Data.SqlClient") || result.contains("src_1"));
        assert!(result.contains("Server=DESKTOP-SRV\\MSSQLSERVER;Database=retaildb;User=sa"));

        // Should have INSERT for both tables
        assert!(result.contains("INSERT INTO sales"));
        assert!(result.contains("INSERT INTO products"));

        // Should have the SQL query
        assert!(result.contains("SELECT * FROM [dbo].[vw_sales]"));
        assert!(result.contains("SELECT * FROM [dbo].[vw_products]"));

        // Query partition label
        assert!(result.contains("query partition"));
    }

    #[test]
    fn test_render_load_script_m_partition_sql_server() {
        let model = TabularModel {
            name: "MTest".into(),
            compatibility_level: 1200,
            tables: vec![TableInfo {
                name: "Orders".into(),
                ssas_name: "Orders".into(),
                description: String::new(),
                columns: vec![ColumnInfo {
                    name: "Order ID".into(),
                    data_type: "int64".into(),
                    source_column: "orderid".into(),
                    is_hidden: false,
                }],
                measures: vec![],
                partitions: vec![PartitionInfo {
                    name: "m_part".into(),
                    source_type: "m".into(),
                    is_m: true,
                    query: Some(r#"let
    Source = Sql.Database("pg01", "sales_db"),
    Table = Source{[Schema="public",Item="orders"]}[Data]
in
    Table"#.into()),
                    data_source_name: None,
                    mode: None,
                    schema: None,
                    database: None,
                }],
                hierarchies: vec![],
            }],
            relationships: vec![],
            roles: vec![],
            data_sources: vec![],
        };

        let result = render_load_script(&model, &[], &[]);

        assert!(result.contains("M partition, SQL Server"));
        assert!(result.contains("INSTALL sqlserver_scanner"));
        assert!(result.contains("ATTACH"));
        assert!(result.contains("Server=pg01;Database=sales_db"));
        assert!(result.contains("INSERT INTO orders"));
        assert!(result.contains("src_1.public.\"orders\""));
    }

    #[test]
    fn test_render_load_script_m_partition_csv() {
        let model = TabularModel {
            name: "CSVTest".into(),
            compatibility_level: 1200,
            tables: vec![TableInfo {
                name: "Customer".into(),
                ssas_name: "Customer".into(),
                description: String::new(),
                columns: vec![ColumnInfo {
                    name: "CustomerKey".into(),
                    data_type: "int64".into(),
                    source_column: "customerkey".into(),
                    is_hidden: false,
                }],
                measures: vec![],
                partitions: vec![PartitionInfo {
                    name: "csv_part".into(),
                    source_type: "m".into(),
                    is_m: true,
                    query: Some(r#"let
    Source = Web.Contents(#"[SourceUrl]", [RelativePath = "pbi-tools/contoso-sales-model/main/data/Customer.csv"]),
    Csv = Csv.Document(Source, [QuoteStyle=QuoteStyle.Csv]),
    #"Promoted Headers" = Table.PromoteHeaders(Csv, [PromoteAllScalars=true])
in
    #"Promoted Headers""#.into()),
                    data_source_name: None,
                    mode: None,
                    schema: None,
                    database: None,
                }],
                hierarchies: vec![],
            }],
            relationships: vec![],
            roles: vec![],
            data_sources: vec![],
        };

        let result = render_load_script(&model, &[], &[]);

        assert!(result.contains("M partition, CSV source"));
        assert!(result.contains("CSV source: relative path"));
        assert!(result.contains("Customer.csv"));
        assert!(result.contains("parameterized URL"));
        assert!(result.contains("read_csv_auto"));
    }

    #[test]
    fn test_render_load_script_m_partition_unrecognized() {
        let model = TabularModel {
            name: "UnrecTest".into(),
            compatibility_level: 1200,
            tables: vec![TableInfo {
                name: "Unknown".into(),
                ssas_name: "Unknown".into(),
                description: String::new(),
                columns: vec![],
                measures: vec![],
                partitions: vec![PartitionInfo {
                    name: "weird".into(),
                    source_type: "m".into(),
                    is_m: true,
                    query: Some("let Source = SomeCustomFunction(\"path\") in Source".into()),
                    data_source_name: None,
                    mode: None,
                    schema: None,
                    database: None,
                }],
                hierarchies: vec![],
            }],
            relationships: vec![],
            roles: vec![],
            data_sources: vec![],
        };

        let result = render_load_script(&model, &[], &[]);

        assert!(result.contains("M partition, unrecognized"));
        assert!(result.contains("Unrecognized M expression, manual load required"));
        assert!(result.contains("M expression (first 200 chars)"));
    }

    #[test]
    fn test_render_load_script_calculated_skipped() {
        let model = model_with_calculated();
        let result = render_load_script(&model, &["Sales".into()], &[]);

        assert!(result.contains("is computed by DAX, not loaded"));
        assert!(!result.contains("INSERT INTO state"));
        assert!(result.contains("INSERT INTO sales"));
        assert!(result.contains("INSERT INTO products"));
    }

    #[test]
    fn test_render_load_script_date_table_skipped() {
        let model = model_with_dates();
        let result = render_load_script(&model, &["Sales".into()], &["Dates".into()]);

        assert!(result.contains("is populated by seed_date_dim.sql"));
        assert!(!result.contains("INSERT INTO dates"));
        assert!(result.contains("INSERT INTO sales"));
    }

    #[test]
    fn test_attach_deduplication() {
        let model = make_model_with_data_source();
        let result = render_load_script(&model, &["Sales".into()], &[]);

        // Count ATTACH statements (not word occurrences in comments)
        let attach_count = result.matches("ATTACH '").count();
        assert_eq!(attach_count, 1, "Two tables sharing one data source should emit one ATTACH");

        // Both tables should have INSERT
        assert!(result.contains("INSERT INTO sales"));
        assert!(result.contains("INSERT INTO products"));
    }

    #[test]
    fn test_connection_string_translation() {
        let ds = DataSourceInfo {
            name: "Test".into(),
            provider: "System.Data.SqlClient".into(),
            server: "MY-SERVER".into(),
            database: "MyDB".into(),
            connection_string: "data source=MY-SERVER;initial catalog=MyDB;user id=admin;password=secret123".into(),
        };

        let attach = translate_to_duckdb_attach(&ds);
        assert!(attach.contains("Server=MY-SERVER"));
        assert!(attach.contains("Database=MyDB"));
        assert!(attach.contains("User=admin"));
        assert!(attach.contains("Password=secret123"));
    }

    #[test]
    fn test_render_load_script_no_source_info() {
        // BIM-style model: query partition with data_source_name but no DataSourceInfo
        let model = TabularModel {
            name: "BimTest".into(),
            compatibility_level: 1200,
            tables: vec![TableInfo {
                name: "Sales".into(),
                ssas_name: "Sales".into(),
                description: String::new(),
                columns: vec![ColumnInfo {
                    name: "ID".into(),
                    data_type: "int64".into(),
                    source_column: "id".into(),
                    is_hidden: false,
                }],
                measures: vec![],
                partitions: vec![PartitionInfo {
                    name: "part".into(),
                    source_type: "query".into(),
                    is_m: false,
                    query: Some("SELECT * FROM [dbo].[vw_sales]".into()),
                    data_source_name: Some("MissingSource".into()),
                    mode: None,
                    schema: Some("dbo".into()),
                    database: Some("retaildb".into()),
                }],
                hierarchies: vec![],
            }],
            relationships: vec![],
            roles: vec![],
            data_sources: vec![],
        };

        let result = render_load_script(&model, &["Sales".into()], &[]);

        // Should emit a comment about missing data source
        assert!(result.contains("connection info not available"));
        assert!(result.contains("Load manually"));
        // Should NOT emit ATTACH
        assert!(!result.contains("ATTACH"));
    }

    #[test]
    fn test_retail_fixture_load_script() {
        let (model, _warnings) = parse_folder::parse_model("data/retailanalytics_tabular");

        let result = render_load_script(&model, &["Sales".into()], &["Dates".into()]);

        // Header
        assert!(result.contains("Data loading script"));

        // SQL Server header with extension instructions
        assert!(result.contains("sqlserver_scanner"));

        // ATTACH for the retail data source
        assert!(result.contains("ATTACH"));
        assert!(result.contains("DESKTOP-PONL6H6"));

        // INSERT for the sales fact table (query partition — commented template)
        assert!(result.contains("INSERT INTO sales"));
        assert!(result.contains("-- Raw SQL"));
        assert!(result.contains("[dbo].[vw_sales]"));

        // Other data tables should be present
        assert!(result.contains("INSERT INTO products"));
        assert!(result.contains("INSERT INTO customers"));
        assert!(result.contains("INSERT INTO stores"));
        assert!(result.contains("INSERT INTO promotions"));

        // Date table should be skipped
        assert!(result.contains("is populated by seed_date_dim.sql"));
        assert!(!result.contains("INSERT INTO dates"));

        // DAX (calculated) should be skipped
        assert!(result.contains("is computed by DAX, not loaded"));
        assert!(!result.contains("INSERT INTO dax"));
    }

    #[test]
    fn test_contoso_load_script() {
        let (model, _warnings) = crate::tools::parse_bim::parse_model("data/contoso/Contoso.bim");

        let result = render_load_script(&model, &["Sales".into()], &["Date".into()]);

        // Date table should be skipped (date role)
        assert!(result.contains("is populated by seed_date_dim.sql"));
        assert!(!result.contains("INSERT INTO date"));

        // Calculated tables should be skipped
        assert!(result.contains("is computed by DAX, not loaded"));

        // M-partition CSV tables should get CSV source comments
        assert!(result.contains("Customer.csv"));
        assert!(result.contains("Product.csv"));
        assert!(result.contains("Promotion.csv"));
        assert!(result.contains("Store.csv"));
        assert!(result.contains("parameterized URL"));

        // Sales has an empty M expression → manual load comment
        assert!(result.contains("Empty M expression for table 'Sales'"));
    }

    // ── Bug 2: M-partition SqlServer ATTACH dedup ─────────────────────────

    #[test]
    fn test_multi_m_partition_sqlserver_dedup() {
        let model = TabularModel {
            name: "DedupTest".into(),
            compatibility_level: 1200,
            tables: vec![
                TableInfo {
                    name: "Orders".into(),
                    ssas_name: "Orders".into(),
                    description: String::new(),
                    columns: vec![ColumnInfo {
                        name: "Order ID".into(),
                        data_type: "int64".into(),
                        source_column: "orderid".into(),
                        is_hidden: false,
                    }],
                    measures: vec![],
                    partitions: vec![PartitionInfo {
                        name: "m_orders".into(),
                        source_type: "m".into(),
                        is_m: true,
                        query: Some(r#"let
    Source = Sql.Database("pg01", "sales_db"),
    Table = Source{[Schema="public",Item="orders"]}[Data]
in
    Table"#.into()),
                        data_source_name: None,
                        mode: None,
                        schema: None,
                        database: None,
                    }],
                    hierarchies: vec![],
                },
                TableInfo {
                    name: "Customers".into(),
                    ssas_name: "Customers".into(),
                    description: String::new(),
                    columns: vec![ColumnInfo {
                        name: "Customer ID".into(),
                        data_type: "int64".into(),
                        source_column: "customerid".into(),
                        is_hidden: false,
                    }],
                    measures: vec![],
                    partitions: vec![PartitionInfo {
                        name: "m_customers".into(),
                        source_type: "m".into(),
                        is_m: true,
                        query: Some(r#"let
    Source = Sql.Database("pg01", "sales_db"),
    Table = Source{[Schema="public",Item="customers"]}[Data]
in
    Table"#.into()),
                        data_source_name: None,
                        mode: None,
                        schema: None,
                        database: None,
                    }],
                    hierarchies: vec![],
                },
            ],
            relationships: vec![],
            roles: vec![],
            data_sources: vec![],
        };

        let result = render_load_script(&model, &[], &[]);
        // Both tables share the same server:database → only one ATTACH
        let attach_count = result.matches("ATTACH").count();
        assert_eq!(attach_count, 1, "Two M tables sharing one SQL Server should emit one ATTACH");
        // Both should use src_1 alias
        assert!(result.contains("src_1.public.\"orders\""));
        assert!(result.contains("src_1.public.\"customers\""));
        // No hardcoded src alias
        assert!(!result.contains("AS src ("));
    }

    // ── Bug 4: Unicode truncation ─────────────────────────────────────────

    #[test]
    fn test_truncate_m_unicode_safe() {
        // Multi-byte UTF-8 should not panic
        let s = "héllo wörld 💯🔥";
        let result = truncate_m(s, 5);
        assert!(!result.contains('\n'));
        assert_eq!(result.chars().count(), 8); // 5 chars + "..."

        // Short string within bounds
        let short = "héllo";
        assert_eq!(truncate_m(short, 10), "héllo");

        // ASCII only
        let ascii = "hello world";
        let truncated = truncate_m(ascii, 3);
        assert_eq!(truncated, "hel...");

        // Empty string
        assert_eq!(truncate_m("", 5), "");
    }

    // ── Bug 5: FK to date table ───────────────────────────────────────────

    #[test]
    fn test_fk_to_date_table_generates_strftime() {
        let mut model = make_minimal_model();
        // Add datekey column to Sales so the FK relationship resolves
        model.tables[0].columns.push(ColumnInfo {
            name: "Date Key".into(),
            data_type: "int64".into(),
            source_column: "datekey".into(),
            is_hidden: false,
        });
        // Add a date table
        model.tables.push(TableInfo {
            name: "Dates".into(),
            ssas_name: "Dates".into(),
            description: String::new(),
            columns: vec![
                ColumnInfo {
                    name: "Date Key".into(),
                    data_type: "int64".into(),
                    source_column: "datekey".into(),
                    is_hidden: false,
                },
            ],
            measures: vec![],
            partitions: vec![],
            hierarchies: vec![],
        });
        // Add FK from Sales.Date Key to Dates.Date Key
        model.relationships.push(RelInfo {
            from_table: "Sales".into(),
            from_column: "Date Key".into(),
            to_table: "Dates".into(),
            to_column: "Date Key".into(),
        });

        let result = render_dummy_data_script(&model, &["Sales".into()], &["Dates".into()], 10000, 1000);
        // FK to date table should use strftime, not modulo
        assert!(!result.contains("(i % 1000) + 1 AS datekey"));
        assert!(result.contains("strftime(DATE '2020-01-01' + (i % 4018) * INTERVAL '1 day', '%Y%m%d')::INTEGER AS datekey"));

        // Non-date FK should still use modulo
        assert!(result.contains("(i % 1000) + 1 AS productid"));
    }

    #[test]
    fn test_fk_timestamp_column_generates_date_expr() {
        let mut model = make_minimal_model();
        // Add a TIMESTAMP FK column to Sales (data_type: "dateTime" → DuckDB TIMESTAMP)
        model.tables[0].columns.push(ColumnInfo {
            name: "Delivery Date".into(),
            data_type: "dateTime".into(),
            source_column: "deliverydate".into(),
            is_hidden: false,
        });
        // Add a Dates table (date role) with a TIMESTAMP column
        model.tables.push(TableInfo {
            name: "Dates".into(),
            ssas_name: "Dates".into(),
            description: String::new(),
            columns: vec![
                ColumnInfo {
                    name: "Date Key".into(),
                    data_type: "int64".into(),
                    source_column: "datekey".into(),
                    is_hidden: false,
                },
                ColumnInfo {
                    name: "Full Date".into(),
                    data_type: "dateTime".into(),
                    source_column: "fulldate".into(),
                    is_hidden: false,
                },
            ],
            measures: vec![],
            partitions: vec![],
            hierarchies: vec![],
        });
        // Add FK from Sales.Delivery Date to Dates.Full Date (TIMESTAMP → TIMESTAMP)
        model.relationships.push(RelInfo {
            from_table: "Sales".into(),
            from_column: "Delivery Date".into(),
            to_table: "Dates".into(),
            to_column: "Full Date".into(),
        });

        let result = render_dummy_data_script(
            &model,
            &["Sales".into()],
            &["Dates".into()],
            10000,
            1000,
        );

        // TIMESTAMP FK should generate a date expression, NOT a modulo
        assert!(
            !result.contains("(i % 1000) + 1 AS deliverydate"),
            "TIMESTAMP FK should not use integer modulo"
        );
        assert!(
            result.contains(
                "TIMESTAMP '2020-01-01 00:00:00' + (i % 365) * INTERVAL '1 day' AS deliverydate"
            ),
            "TIMESTAMP FK should generate TIMESTAMP expression"
        );

        // Non-TIMESTAMP FK (Product ID → Products) should still use modulo
        assert!(result.contains("(i % 1000) + 1 AS productid"));
    }
}
