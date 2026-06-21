//! Folder-format parser for Tabular Editor 2.x project exports.
//! Reads a directory tree with database.json, tables/, relationships/, roles/
//! and produces a `TabularModel`.

use super::tabular_model::*;
use std::fs;
use std::path::Path;

pub fn parse_model(src_dir: &str) -> (TabularModel, Vec<String>) {
    let warnings: Vec<String> = Vec::new();
    let (name, compat_level) = parse_database_meta(&format!("{src_dir}/database.json"));
    let tables = parse_all_tables(&format!("{src_dir}/tables"));
    let relationships = parse_relationships(&format!("{src_dir}/relationships"));
    let roles = parse_roles(&format!("{src_dir}/roles"));
    let data_sources = parse_data_sources(&format!("{src_dir}/dataSources"));

    (
        TabularModel {
            name,
            compatibility_level: compat_level,
            tables,
            relationships,
            roles,
            data_sources,
        },
        warnings,
    )
}

fn parse_database_meta(path: &str) -> (String, i64) {
    if let Ok(text) = fs::read_to_string(path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            let name = v["name"].as_str().unwrap_or("SemanticModel").to_string();
            let compat = v["compatibilityLevel"].as_i64().unwrap_or(0);
            return (name, compat);
        }
    }
    ("SemanticModel".into(), 0)
}

fn parse_all_tables(dir: &str) -> Vec<TableInfo> {
    let mut tables = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return tables; };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        let meta_path = path.join(format!("{name}.json"));
        let (ssas_name, desc) = parse_table_meta(&meta_path);

        let columns = parse_columns(&path.join("columns"));
        let measures = parse_measures(&path.join("measures"));
        let partitions = parse_partitions(&path.join("partitions"));
        let hierarchies = parse_hierarchies(&path.join("hierarchies"));

        tables.push(TableInfo {
            name,
            ssas_name,
            description: desc,
            columns,
            measures,
            partitions,
            hierarchies,
        });
    }
    tables.sort_by(|a, b| a.name.cmp(&b.name));
    tables
}

fn parse_table_meta(path: &Path) -> (String, String) {
    if let Ok(text) = fs::read_to_string(path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            let ssas = v["name"].as_str().unwrap_or("").to_string();
            let desc = v["description"].as_str().unwrap_or("").to_string();
            return (ssas, desc);
        }
    }
    (String::new(), String::new())
}

fn parse_columns(dir: &Path) -> Vec<ColumnInfo> {
    let mut cols = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return cols; };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                let dt = v["dataType"].as_str().unwrap_or("string").to_string();
                let sc = v["sourceColumn"].as_str().unwrap_or(&name).to_string();
                let hidden = v["isHidden"].as_bool().unwrap_or(false);
                cols.push(ColumnInfo { name, data_type: dt, source_column: sc, is_hidden: hidden });
            }
        }
    }
    cols.sort_by(|a, b| a.name.cmp(&b.name));
    cols
}

fn parse_measures(dir: &Path) -> Vec<MeasureInfo> {
    let mut measures = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return measures; };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                let expr = flatten_json_array(&v["expression"]);
                let folder = v["displayFolder"].as_str().unwrap_or("").to_string();
                let classification = classify_dax(&expr);
                measures.push(MeasureInfo {
                    name,
                    expression: expr,
                    display_folder: folder,
                    classification,
                });
            }
        }
    }
    measures.sort_by(|a, b| a.name.cmp(&b.name));
    measures
}

fn parse_partitions(dir: &Path) -> Vec<PartitionInfo> {
    let mut parts = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return parts; };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                let st = v["source"]["type"].as_str().unwrap_or("").to_string();
                let is_m = st == "m";

                // Capture source query (array or string) or expression (calculated)
                let query = if !v["source"]["query"].is_null() {
                    let flat = flatten_json_array(&v["source"]["query"]);
                    if flat.is_empty() { None } else { Some(flat) }
                } else if !v["source"]["expression"].is_null() {
                    let flat = flatten_json_array(&v["source"]["expression"]);
                    if flat.is_empty() { None } else { Some(flat) }
                } else {
                    None
                };

                let data_source_name = v["source"]["dataSource"]
                    .as_str()
                    .map(|s| s.to_string());

                let mode = v["mode"].as_str().map(|s| s.to_string());

                // Parse TabularEditor_TableSchema annotation
                let (schema, database) = parse_table_schema_annotation(&v["annotations"]);

                parts.push(PartitionInfo {
                    name,
                    source_type: st,
                    is_m,
                    query,
                    data_source_name,
                    mode,
                    schema,
                    database,
                });
            }
        }
    }
    parts
}

/// Parse the `TabularEditor_TableSchema` annotation from a partition's annotations array.
/// Returns (schema, database) if found and parsable, otherwise (None, None).
fn parse_table_schema_annotation(annotations: &serde_json::Value) -> (Option<String>, Option<String>) {
    if let Some(arr) = annotations.as_array() {
        for ann in arr {
            if ann["name"].as_str() == Some("TabularEditor_TableSchema") {
                if let Some(val_str) = ann["value"].as_str() {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(val_str) {
                        let schema = val["Schema"].as_str().map(|s| s.to_string());
                        let database = val["Database"].as_str().map(|s| s.to_string());
                        return (schema, database);
                    }
                }
            }
        }
    }
    (None, None)
}

fn parse_data_sources(dir: &str) -> Vec<DataSourceInfo> {
    let mut sources = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return sources; };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                let name = v["name"].as_str().unwrap_or("").to_string();
                let conn_str = v["connectionString"].as_str().unwrap_or("").to_string();
                let provider = v["provider"].as_str().unwrap_or("").to_string();
                let ado_map = parse_ado_connection_string(&conn_str);
                let server = ado_server(&ado_map);
                let database = ado_database(&ado_map);
                sources.push(DataSourceInfo { name, provider, server, database, connection_string: conn_str });
            }
        }
    }
    sources.sort_by(|a, b| a.name.cmp(&b.name));
    sources
}

fn parse_relationships(dir: &str) -> Vec<RelInfo> {
    let mut rels = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return rels; };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                rels.push(RelInfo {
                    from_table: v["fromTable"].as_str().unwrap_or("").to_string(),
                    from_column: v["fromColumn"].as_str().unwrap_or("").to_string(),
                    to_table: v["toTable"].as_str().unwrap_or("").to_string(),
                    to_column: v["toColumn"].as_str().unwrap_or("").to_string(),
                });
            }
        }
    }
    rels.sort_by(|a, b| a.from_table.cmp(&b.from_table).then(a.to_table.cmp(&b.to_table)));
    rels
}

fn parse_roles(dir: &str) -> Vec<RoleInfo> {
    let mut roles = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return roles; };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                roles.push(RoleInfo {
                    name: v["name"].as_str().unwrap_or("").to_string(),
                    description: v["description"].as_str().unwrap_or("").to_string(),
                });
            }
        }
    }
    roles
}

fn parse_hierarchies(dir: &Path) -> Vec<String> {
    let mut hiers = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return hiers; };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        hiers.push(name.trim_end_matches(".json").to_string());
    }
    hiers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_capture() {
        let (model, _warnings) = parse_model("data/retailanalytics_tabular");

        // Find Sales partition
        let sales = model.tables.iter().find(|t| t.name == "Sales").unwrap();
        assert_eq!(sales.partitions.len(), 1);
        let part = &sales.partitions[0];
        assert_eq!(part.source_type, "query");
        // Query should be flattened from array (flatten_json_array trims each fragment)
        assert_eq!(
            part.query.as_deref(),
            Some("SELECT * FROM [dbo].[vw_sales]")
        );
        assert_eq!(
            part.data_source_name.as_deref(),
            Some("DESKTOP-PONL6H6\\MSSQLSERVER01 retailanalytics")
        );
        // Schema and database from TabularEditor_TableSchema annotation
        assert_eq!(part.schema.as_deref(), Some("dbo"));
        assert_eq!(part.database.as_deref(), Some("retailanalytics"));
        // Partition has no mode field
        assert_eq!(part.mode, None);
    }

    #[test]
    fn test_data_source_capture() {
        let (model, _warnings) = parse_model("data/retailanalytics_tabular");

        assert_eq!(model.data_sources.len(), 1);
        let ds = &model.data_sources[0];
        assert_eq!(ds.name, "DESKTOP-PONL6H6\\MSSQLSERVER01 retailanalytics");
        assert_eq!(ds.server, "DESKTOP-PONL6H6\\MSSQLSERVER01");
        assert_eq!(ds.database, "retailanalytics");
        assert_eq!(ds.provider, "System.Data.SqlClient");
    }

    #[test]
    fn test_ado_connection_string_parser() {
        let conn_str = "data source=DESKTOP-PONL6H6\\MSSQLSERVER01;initial catalog=retailanalytics;persist security info=True;user id=sa";
        let map = parse_ado_connection_string(conn_str);
        assert_eq!(
            ado_server(&map),
            "DESKTOP-PONL6H6\\MSSQLSERVER01"
        );
        assert_eq!(ado_database(&map), "retailanalytics");
        assert_eq!(map.get("user id").map(|s| s.as_str()), Some("sa"));
        assert_eq!(map.get("persist security info").map(|s| s.as_str()), Some("True"));
    }

    #[test]
    fn test_partition_capture_calculated() {
        // DAX table (calculated table) should have query=Some(expression) and no dataSource
        let (model, _warnings) = parse_model("data/retailanalytics_tabular");
        let dax = model.tables.iter().find(|t| t.name == "DAX").unwrap();
        assert_eq!(dax.partitions.len(), 1);
        let part = &dax.partitions[0];
        assert_eq!(part.source_type, "calculated");
        assert!(part.is_m == false);
        // Calculated table has an expression
        assert!(part.query.is_some());
        assert!(part.query.as_deref().unwrap_or("").contains('{'));
        // No data source for calculated tables
        assert_eq!(part.data_source_name, None);
        // Mode should be "import"
        assert_eq!(part.mode.as_deref(), Some("import"));
    }

    #[test]
    fn test_partition_schema_database_on_all_tables() {
        let (model, _warnings) = parse_model("data/retailanalytics_tabular");
        // All query-type partitions should have schema + database from annotation
        for table in &model.tables {
            for part in &table.partitions {
                if part.source_type == "query" {
                    assert!(
                        part.schema.is_some(),
                        "query partition '{}' in table '{}' should have schema from annotation",
                        part.name,
                        table.name
                    );
                    assert!(
                        part.database.is_some(),
                        "query partition '{}' in table '{}' should have database from annotation",
                        part.name,
                        table.name
                    );
                }
            }
        }
    }
}
