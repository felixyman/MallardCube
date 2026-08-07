//! BIM-format parser for Tabular Editor 2.x projects.
//! Reads a single `.bim` JSON file and produces a `TabularModel`,
//! matching the interface of the folder-format parser (`parse_folder`).

use super::tabular_model::*;
use std::fs;

pub fn parse_model(path: &str) -> (TabularModel, Vec<String>) {
    let mut warnings: Vec<String> = Vec::new();
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            warnings.push(format!("failed to read .bim file: {}", e));
            return (
                TabularModel {
                    name: String::new(),
                    compatibility_level: 0,
                    tables: Vec::new(),
                    relationships: Vec::new(),
                    roles: Vec::new(),
                    data_sources: vec![],
                },
                warnings,
            );
        }
    };
    let root: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            warnings.push(format!("failed to parse .bim JSON: {}", e));
            return (
                TabularModel {
                    name: String::new(),
                    compatibility_level: 0,
                    tables: Vec::new(),
                    relationships: Vec::new(),
                    roles: Vec::new(),
                    data_sources: vec![],
                },
                warnings,
            );
        }
    };

    let name = root["name"].as_str().unwrap_or("SemanticModel").to_string();
    let compat_level = root["compatibilityLevel"].as_i64().unwrap_or(0);

    // Warn about unsupported compatibility levels
    if compat_level != 1700 && compat_level != 1567 {
        warnings.push(format!(
            "compatibilityLevel {} is not 1700 or 1567 (commonly supported)",
            compat_level
        ));
    }

    let model = &root["model"];

    let tables = parse_tables(model);
    let relationships = parse_relationships(model);
    let roles = parse_roles(model);

    // Warn about shared expressions (not supported)
    if let Some(exprs) = model["expressions"].as_array()
        && !exprs.is_empty()
    {
        let names: Vec<String> = exprs
            .iter()
            .filter_map(|e| e["name"].as_str().map(|s| s.to_string()))
            .collect();
        warnings.push(format!(
            "model.expressions contains {} shared expression(s): {} — NOT supported",
            names.len(),
            names.join(", ")
        ));
    }

    (
        TabularModel {
            name,
            compatibility_level: compat_level,
            tables,
            relationships,
            roles,
            data_sources: vec![],
        },
        warnings,
    )
}

fn parse_tables(model: &serde_json::Value) -> Vec<TableInfo> {
    let mut tables = Vec::new();
    let Some(arr) = model["tables"].as_array() else {
        return tables;
    };
    for table_val in arr {
        let tname = table_val["name"].as_str().unwrap_or("").to_string();
        let ssas_name = tname.clone();
        let description = table_val["description"].as_str().unwrap_or("").to_string();

        let columns = parse_columns(table_val);
        let measures = parse_measures(table_val);
        let partitions = parse_partitions(table_val);
        let hierarchies = parse_hierarchies(table_val);

        tables.push(TableInfo {
            name: tname,
            ssas_name,
            description,
            columns,
            measures,
            partitions,
            hierarchies,
        });
    }
    tables.sort_by(|a, b| a.name.cmp(&b.name));
    tables
}

fn parse_columns(table_val: &serde_json::Value) -> Vec<ColumnInfo> {
    let mut cols = Vec::new();
    let Some(arr) = table_val["columns"].as_array() else {
        return cols;
    };
    for col_val in arr {
        let name = col_val["name"].as_str().unwrap_or("").to_string();
        let dt = col_val["dataType"].as_str().unwrap_or("string").to_string();
        let sc = col_val["sourceColumn"]
            .as_str()
            .unwrap_or(&name)
            .to_string();
        let hidden = col_val["isHidden"].as_bool().unwrap_or(false);
        cols.push(ColumnInfo {
            name,
            data_type: dt,
            source_column: sc,
            is_hidden: hidden,
        });
    }
    cols.sort_by(|a, b| a.name.cmp(&b.name));
    cols
}

fn parse_measures(table_val: &serde_json::Value) -> Vec<MeasureInfo> {
    let mut measures = Vec::new();
    let Some(arr) = table_val["measures"].as_array() else {
        return measures;
    };
    for meas_val in arr {
        let name = meas_val["name"].as_str().unwrap_or("").to_string();
        let expr = flatten_json_array(&meas_val["expression"]);
        let folder = meas_val["displayFolder"].as_str().unwrap_or("").to_string();
        let classification = classify_dax(&expr);
        measures.push(MeasureInfo {
            name,
            expression: expr,
            display_folder: folder,
            classification,
        });
    }
    measures.sort_by(|a, b| a.name.cmp(&b.name));
    measures
}

fn parse_partitions(table_val: &serde_json::Value) -> Vec<PartitionInfo> {
    let mut parts = Vec::new();
    let Some(arr) = table_val["partitions"].as_array() else {
        return parts;
    };
    for part_val in arr {
        let name = part_val["name"].as_str().unwrap_or("").to_string();

        // Check source.type first, then fallback to sourceType at top level
        let st = part_val["source"]["type"]
            .as_str()
            .or_else(|| part_val["sourceType"].as_str())
            .unwrap_or("")
            .to_string();

        let is_m = st == "m";

        // Capture source query (string) or expression (array or string) — BIM can use either
        let query = part_val["source"]["query"]
            .as_str()
            .map(|s| s.to_string())
            .or_else(|| {
                let expr = &part_val["source"]["expression"];
                if expr.is_array() {
                    let flat = flatten_json_array(expr);
                    if flat.is_empty() { None } else { Some(flat) }
                } else {
                    expr.as_str().map(|s| s.to_string())
                }
            });

        let data_source_name = part_val["source"]["dataSource"]
            .as_str()
            .map(|s| s.to_string());

        let mode = part_val["mode"].as_str().map(|s| s.to_string());

        // Warn about DirectQuery or dual mode
        if let Some(ref mode_val) = mode
            && (mode_val == "directQuery" || mode_val == "dual")
        {
            eprintln!(
                "WARNING: partition '{}' has mode '{}' — cannot be loaded into DuckDB",
                name, mode_val
            );
        }

        parts.push(PartitionInfo {
            name,
            source_type: st,
            is_m,
            query,
            data_source_name,
            mode,
            schema: None,
            database: None,
        });
    }
    parts
}

fn parse_hierarchies(table_val: &serde_json::Value) -> Vec<String> {
    let mut hiers = Vec::new();
    let Some(arr) = table_val["hierarchies"].as_array() else {
        return hiers;
    };
    for h in arr {
        if let Some(name) = h["name"].as_str() {
            hiers.push(name.to_string());
        }
    }
    hiers
}

fn parse_relationships(model: &serde_json::Value) -> Vec<RelInfo> {
    let mut rels = Vec::new();
    let Some(arr) = model["relationships"].as_array() else {
        return rels;
    };
    for rel_val in arr {
        rels.push(RelInfo {
            from_table: rel_val["fromTable"].as_str().unwrap_or("").to_string(),
            from_column: rel_val["fromColumn"].as_str().unwrap_or("").to_string(),
            to_table: rel_val["toTable"].as_str().unwrap_or("").to_string(),
            to_column: rel_val["toColumn"].as_str().unwrap_or("").to_string(),
        });
    }
    rels.sort_by(|a, b| {
        a.from_table
            .cmp(&b.from_table)
            .then(a.to_table.cmp(&b.to_table))
    });
    rels
}

fn parse_roles(model: &serde_json::Value) -> Vec<RoleInfo> {
    let mut roles = Vec::new();
    let Some(arr) = model["roles"].as_array() else {
        return roles;
    };
    for role_val in arr {
        let name = role_val["name"].as_str().unwrap_or("").to_string();
        let description = role_val["description"].as_str().unwrap_or("").to_string();
        let model_permission = role_val["modelPermission"]
            .as_str()
            .unwrap_or("read")
            .to_string();

        let mut members = Vec::new();
        if let Some(members_arr) = role_val["members"].as_array() {
            for m in members_arr {
                members.push(RoleMemberInfo {
                    member_name: m["memberName"].as_str().unwrap_or("").to_string(),
                    member_type: m["memberType"].as_str().unwrap_or("").to_string(),
                });
            }
        }

        let mut table_permissions = Vec::new();
        if let Some(tps_arr) = role_val["tablePermissions"].as_array() {
            for tp in tps_arr {
                let dax_filter = tp["filterExpression"].as_str().map(|s| s.to_string());
                table_permissions.push(TablePermissionInfo {
                    table: tp["name"].as_str().unwrap_or("").to_string(),
                    filter_expression: String::new(),
                    dax_filter,
                    metadata_permission: tp["metadataPermission"]
                        .as_str()
                        .unwrap_or("read")
                        .to_string(),
                });
            }
        }

        roles.push(RoleInfo {
            name,
            description,
            model_permission,
            members,
            table_permissions,
        });
    }
    roles
}

#[cfg(test)]
mod tests {
    use super::super::parse_folder;
    use super::*;

    #[test]
    fn test_parse_retailanalytics_bim() {
        let path = "data/retailanalytics.bim";
        let (model, _warnings) = parse_model(path);

        // Check model identity
        assert_eq!(model.name, "SemanticModel");
        assert_eq!(model.compatibility_level, 1700);

        // 7 tables: Sales, Products, Customers, Stores, Promotions, Dates, DAX
        assert_eq!(model.tables.len(), 7, "expected 7 tables");

        // Verify table names (sorted alphabetically by folder parser)
        let table_names: Vec<&str> = model.tables.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            table_names,
            vec![
                "Customers",
                "DAX",
                "Dates",
                "Products",
                "Promotions",
                "Sales",
                "Stores"
            ]
        );

        // Verify Sales table is a fact table with 22 columns, 0 measures, 1 partition
        let sales = model.tables.iter().find(|t| t.name == "Sales").unwrap();
        assert_eq!(sales.columns.len(), 22, "Sales should have 22 columns");
        assert_eq!(sales.measures.len(), 0, "Sales should have 0 measures");
        assert_eq!(sales.partitions.len(), 1, "Sales should have 1 partition");
        assert_eq!(
            sales.hierarchies.len(),
            0,
            "Sales should have 0 hierarchies"
        );
        assert_eq!(sales.ssas_name, "Sales");
        assert_eq!(sales.partitions[0].source_type, "query");
        assert!(!sales.partitions[0].is_m);

        // Verify DAX table (calculated) has 4 measures
        let dax = model.tables.iter().find(|t| t.name == "DAX").unwrap();
        assert_eq!(dax.measures.len(), 4, "DAX should have 4 measures");
        assert!(dax.is_calculated(), "DAX should be a calculated table");
        // Verify measure names sorted alphabetically
        let measure_names: Vec<&str> = dax.measures.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            measure_names,
            vec![
                "Gross Margin %",
                "Gross Profit",
                "Total COGS",
                "Total Revenue"
            ]
        );

        // Verify measure expressions
        let rev = dax
            .measures
            .iter()
            .find(|m| m.name == "Total Revenue")
            .unwrap();
        assert!(
            rev.expression.contains("CALCULATE"),
            "Total Revenue should contain CALCULATE"
        );
        // CALCULATE(SUM(...), filter_without_FILTER) classifies as "simple"
        // (the converter will downgrade to sql_fallback when SQL hints are unavailable)
        assert_eq!(
            rev.classification, "simple",
            "CALCULATE with simple filter classifies as simple"
        );

        let cogs = dax
            .measures
            .iter()
            .find(|m| m.name == "Total COGS")
            .unwrap();
        assert_eq!(cogs.classification, "sql_fallback");

        let gp = dax
            .measures
            .iter()
            .find(|m| m.name == "Gross Profit")
            .unwrap();
        assert_eq!(gp.classification, "sql_fallback");

        let gm = dax
            .measures
            .iter()
            .find(|m| m.name == "Gross Margin %")
            .unwrap();
        assert_eq!(gm.classification, "simple");

        // Verify Dates table
        let dates = model.tables.iter().find(|t| t.name == "Dates").unwrap();
        assert_eq!(dates.columns.len(), 17, "Dates should have 17 columns");
        assert_eq!(dates.partitions.len(), 1, "Dates should have 1 partition");
        assert_eq!(dates.hierarchies.len(), 1, "Dates should have 1 hierarchy");
        assert_eq!(dates.hierarchies[0], "Calendar Hierarchy");

        // Verify Products table
        let products = model.tables.iter().find(|t| t.name == "Products").unwrap();
        assert_eq!(
            products.columns.len(),
            24,
            "Products should have 24 columns"
        );
        assert_eq!(
            products.partitions.len(),
            1,
            "Products should have 1 partition"
        );

        // Verify Customers table
        let customers = model.tables.iter().find(|t| t.name == "Customers").unwrap();
        assert_eq!(
            customers.columns.len(),
            23,
            "Customers should have 23 columns"
        );

        // Verify Stores table
        let stores = model.tables.iter().find(|t| t.name == "Stores").unwrap();
        assert_eq!(stores.columns.len(), 17, "Stores should have 17 columns");

        // Verify Promotions table
        let promotions = model
            .tables
            .iter()
            .find(|t| t.name == "Promotions")
            .unwrap();
        assert_eq!(
            promotions.columns.len(),
            17,
            "Promotions should have 17 columns"
        );

        // Check relationships
        assert_eq!(model.relationships.len(), 5, "expected 5 relationships");
        // All relationships are from Sales → dimension tables
        let rel_from = model
            .relationships
            .iter()
            .map(|r| r.from_table.as_str())
            .collect::<Vec<_>>();
        assert!(
            rel_from.iter().all(|&r| r == "Sales"),
            "all relationships should originate from Sales"
        );

        let rel_to: Vec<&str> = model
            .relationships
            .iter()
            .map(|r| r.to_table.as_str())
            .collect();
        assert!(rel_to.contains(&"Customers"));
        assert!(rel_to.contains(&"Products"));
        assert!(rel_to.contains(&"Stores"));
        assert!(rel_to.contains(&"Promotions"));
        assert!(rel_to.contains(&"Dates"));

        // Check roles (none in this model)
        assert_eq!(model.roles.len(), 0, "expected 0 roles");
    }

    #[test]
    fn test_flatten_json_array_bim_context() {
        // String format (BIM style)
        let string_val = serde_json::Value::String("= CALCULATE(SUM('Sales'[Amount]))".to_string());
        let result = flatten_json_array(&string_val);
        assert_eq!(result, "= CALCULATE(SUM('Sales'[Amount]))");

        // Array format (folder style)
        let arr_val = serde_json::json!(["", "= CALCULATE(", "SUM(", "'Sales'[Amount])", ")"]);
        let result = flatten_json_array(&arr_val);
        // The first empty element trims to "" and joins with a space, producing a leading space
        assert!(result.contains("= CALCULATE("));
        assert!(result.contains("SUM("));
        assert!(result.contains("'Sales'[Amount])"));

        // Empty/null value
        let null_val = serde_json::Value::Null;
        let result = flatten_json_array(&null_val);
        assert_eq!(result, "");
    }

    #[test]
    fn test_bim_partition_capture() {
        let path = "data/retailanalytics.bim";
        let (model, _warnings) = parse_model(path);

        // Find Sales partition
        let sales = model.tables.iter().find(|t| t.name == "Sales").unwrap();
        assert_eq!(sales.partitions.len(), 1);
        let part = &sales.partitions[0];
        assert_eq!(part.source_type, "query");
        // BIM stores query as a flat string
        assert_eq!(
            part.query.as_deref(),
            Some("SELECT * FROM [dbo].[vw_sales]")
        );
        assert_eq!(
            part.data_source_name.as_deref(),
            Some("DESKTOP-PONL6H6\\MSSQLSERVER01 retailanalytics")
        );
        // BIM partitions don't have TabularEditor_TableSchema annotations
        assert_eq!(part.schema, None);
        assert_eq!(part.database, None);
        // BIM Sales partition has no mode
        assert_eq!(part.mode, None);

        // Verify data_sources is empty (BIM has no dataSources array)
        assert!(model.data_sources.is_empty());
    }

    #[test]
    fn test_parse_bim_with_invalid_path() {
        let (model, warnings) = parse_model("data/nonexistent.bim");
        assert!(
            !warnings.is_empty(),
            "should have warnings for missing file"
        );
        assert_eq!(model.name, ""); // empty default name on failure
    }

    #[test]
    fn test_folder_bim_structural_equivalence() {
        let (folder_model, _) = parse_folder::parse_model("data/retailanalytics_tabular");
        let (bim_model, _) = parse_model("data/retailanalytics.bim");

        // Same model name and compatibility level
        assert_eq!(folder_model.name, bim_model.name);
        assert_eq!(
            folder_model.compatibility_level,
            bim_model.compatibility_level
        );

        // Same number of tables
        assert_eq!(folder_model.tables.len(), bim_model.tables.len());

        // Same table names (both sorted alphabetically)
        let folder_names: Vec<&str> = folder_model
            .tables
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        let bim_names: Vec<&str> = bim_model.tables.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(folder_names, bim_names);

        // Same number of relationships
        assert_eq!(
            folder_model.relationships.len(),
            bim_model.relationships.len()
        );

        // Same number of roles
        assert_eq!(folder_model.roles.len(), bim_model.roles.len());

        // Per-table structural checks
        for (ft, bt) in folder_model.tables.iter().zip(bim_model.tables.iter()) {
            assert_eq!(ft.name, bt.name, "table name mismatch");
            assert_eq!(
                ft.columns.len(),
                bt.columns.len(),
                "column count mismatch for {}",
                ft.name
            );
            assert_eq!(
                ft.measures.len(),
                bt.measures.len(),
                "measure count mismatch for {}",
                ft.name
            );
            assert_eq!(
                ft.hierarchies.len(),
                bt.hierarchies.len(),
                "hierarchy count mismatch for {}",
                ft.name
            );

            // Check measure names match
            let fm_names: Vec<&str> = ft.measures.iter().map(|m| m.name.as_str()).collect();
            let bm_names: Vec<&str> = bt.measures.iter().map(|m| m.name.as_str()).collect();
            assert_eq!(fm_names, bm_names, "measure names mismatch for {}", ft.name);

            // Check measure classifications match
            for (fm, bm) in ft.measures.iter().zip(bt.measures.iter()) {
                assert_eq!(
                    fm.classification, bm.classification,
                    "classification mismatch for measure {} in table {}",
                    fm.name, ft.name
                );
            }

            // Check column names match
            let fc_names: Vec<&str> = ft.columns.iter().map(|c| c.name.as_str()).collect();
            let bc_names: Vec<&str> = bt.columns.iter().map(|c| c.name.as_str()).collect();
            assert_eq!(fc_names, bc_names, "column names mismatch for {}", ft.name);
        }

        // Check relationship endpoints match (sorted)
        let folder_rels: Vec<_> = folder_model
            .relationships
            .iter()
            .map(|r| {
                (
                    r.from_table.as_str(),
                    r.from_column.as_str(),
                    r.to_table.as_str(),
                    r.to_column.as_str(),
                )
            })
            .collect();
        let bim_rels: Vec<_> = bim_model
            .relationships
            .iter()
            .map(|r| {
                (
                    r.from_table.as_str(),
                    r.from_column.as_str(),
                    r.to_table.as_str(),
                    r.to_column.as_str(),
                )
            })
            .collect();
        assert_eq!(folder_rels, bim_rels, "relationships mismatch");
    }

    #[test]
    fn test_format_detection() {
        use super::super::tabular_model::{TabularFormat, detect_format};
        use std::path::Path;

        // BIM file detection
        assert_eq!(
            detect_format(Path::new("data/retailanalytics.bim")),
            Some(TabularFormat::Bim)
        );

        // TMDL directory detection
        assert_eq!(
            detect_format(Path::new("data/retailanalytics_tmdl")),
            Some(TabularFormat::Tmdl)
        );

        // Folder format detection
        assert_eq!(
            detect_format(Path::new("data/retailanalytics_tabular")),
            Some(TabularFormat::Folder)
        );

        // Non-existent path
        assert_eq!(detect_format(Path::new("data/nonexistent_path")), None);

        // Non-.bim file
        assert_eq!(detect_format(Path::new("data/seed_date_dim.sql")), None);
    }
}
