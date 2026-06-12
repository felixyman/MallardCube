/// Proxy project — loads a developer's Malloy files and proxy config
/// at startup, producing the runtime `SemanticModel` and the Malloy
/// source text that will be compiled.
///
/// This is the single entry-point that replaces `default_model()` when
/// a config is supplied.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use crate::engine::model::{SemanticModel, DimensionDef, MeasureDef, Dialect, FactTable, RelationshipDef};
use crate::engine::plan::QueryPlan;
use crate::proxy_config::ProxyConfig;

/// Module-level project singleton. `None` until `init_project()` is called.
static PROJECT: OnceLock<ProxyProject> = OnceLock::new();

pub fn project() -> &'static ProxyProject {
    PROJECT.get_or_init(|| ProxyProject::default_())
}

pub fn init_project(config_path: Option<&str>) -> Result<(), String> {
    let p = match config_path {
        Some(path) => ProxyProject::load(path)?,
        None => ProxyProject::default_(),
    };
    PROJECT.set(p).map_err(|_| "project already initialised".into())
}

pub struct ProxyProject {
    pub config: ProxyConfig,
    pub model: SemanticModel,
    pub malloy_model_text: String,
}

impl ProxyProject {
    pub fn load(config_path: &str) -> Result<Self, String> {
        let config: ProxyConfig = {
            let text = fs::read_to_string(config_path)
                .map_err(|e| format!("read config {config_path}: {e}"))?;
            serde_json::from_str(&text)
                .map_err(|e| format!("parse config {config_path}: {e}"))?
        };

        let malloy_path = Path::new(config_path).parent()
            .unwrap_or(Path::new("."))
            .join(&config.malloy_model_file);
        let malloy_model_text = fs::read_to_string(&malloy_path)
            .map_err(|e| format!(
                "read model {}: {e}",
                malloy_path.display(),
            ))?;

        let model = build_semantic_model(&config, Path::new(config_path).parent().unwrap_or(Path::new(".")));

        Ok(Self { config, model, malloy_model_text })
    }

    /// Return Malloy source to compile: either loaded model text +
    /// generated query fragment, or fully generated model+query.
    pub fn malloy_source(&self, plan: &QueryPlan) -> String {
        if self.malloy_model_text.is_empty() {
            crate::engine::malloy::malloy_source_for_query_plan(&self.model, plan)
        } else {
            crate::engine::malloy::malloy_source_with_model_text(
                &self.malloy_model_text, &self.model, plan,
            )
        }
    }

    /// Convenience: use the hardcoded defaults (equivalent to the old
    /// `default_model()`). Used by tests and when no config is supplied.
    pub fn default_() -> Self {
        Self {
            config: ProxyConfig {
                catalog: "KTH_KEX_MALLOY_CUBE".into(),
                cube: "Model".into(),
                source_name: "faktatabell".into(),
                table_name: "faktatabell".into(),
                dialect: "duckdb".into(),
                malloy_model_file: String::new(),
                db_path: None,
                fact_tables: vec![],
                relationships: vec![],
                dimensions: vec![
                    crate::proxy_config::DimensionConfig {
                        id: "Produktkategori".into(),
                        malloy_name: "produktkategori".into(),
                        physical_field: "produktkategori".into(),
                        caption: "Produktkategori".into(),
                        description: "Våra olika produkter".into(),
                        hierarchy_name: "Produktkategori".into(),
                        all_level_name: "(All)".into(),
                        leaf_level_name: "Produktkategori".into(),
                        ordinal: 1,
                        visible: true,
                        has_all: true,
                        cardinality_hint: 50,
                        fact_table: None,
                        shared: false,
                    },
                    crate::proxy_config::DimensionConfig {
                        id: "Region".into(),
                        malloy_name: "region".into(),
                        physical_field: "region".into(),
                        caption: "Region".into(),
                        description: "Geografisk region".into(),
                        hierarchy_name: "Region".into(),
                        all_level_name: "(All)".into(),
                        leaf_level_name: "Region".into(),
                        ordinal: 2,
                        visible: true,
                        has_all: true,
                        cardinality_hint: 10,
                        fact_table: None,
                        shared: false,
                    },
                ],
                measures: vec![
                    crate::proxy_config::MeasureConfig {
                        id: "TotalSales".into(),
                        malloy_name: "total_forsaljning".into(),
                        physical_expr: "sales.sum()".into(),
                        sql_expr: "SUM(sales)".into(),
                        caption: "Total Försäljning".into(),
                        display_name: "Total Försäljning (SEK)".into(),
                        description: "Vår totala försäljning".into(),
                        format_string: "#,##0.00 SEK".into(),
                        units: "SEK".into(),
                        ordinal: 1,
                        visible: true,
                        fact_table: None,
                        aggregator: 1,
                        measure_group_name: "Faktatabell".into(),
                        numeric_precision: 18,
                        numeric_scale: 2,
                        expression: "SUM('Faktatabell'[Sales])".into(),
                        sql_fallback_file: None,
                    },
                ],
            },
            model: crate::engine::model::default_model(),
            malloy_model_text: String::new(),
        }
    }
}

fn build_semantic_model(config: &ProxyConfig, config_dir: &Path) -> SemanticModel {
    let dialect = match config.dialect.as_str() {
        "duckdb" => Dialect::DuckDB,
        other => panic!("unsupported dialect: {other}"),
    };

    let fact_tables: Vec<FactTable> = if config.fact_tables.is_empty() {
        // Old single-table config: synthesize one fact table
        let mg = config.measures.first()
            .map(|m| m.measure_group_name.clone())
            .unwrap_or_else(|| config.cube.clone());
        vec![FactTable {
            id: "default".into(),
            source_name: config.source_name.clone(),
            table_name: config.table_name.clone(),
            measure_group_name: mg,
        }]
    } else {
        config.fact_tables.iter().map(|ft| FactTable {
            id: ft.id.clone(),
            source_name: ft.source_name.clone(),
            table_name: ft.table_name.clone(),
            measure_group_name: ft.measure_group_name.clone(),
        }).collect()
    };

    let dimensions: Vec<DimensionDef> = config.dimensions.iter().map(|dc| {
        let id = dc.id.clone();
        let table_name = dc.fact_table.as_ref().map(|ft_id| {
            fact_tables.iter().find(|ft| ft.id == *ft_id)
                .unwrap_or_else(|| panic!("config: dimension '{}' references unknown fact_table '{}'", id, ft_id))
                .table_name.clone()
        });
        DimensionDef {
            id,
            semantic_name: dc.malloy_name.clone(),
            physical_field: dc.physical_field.clone(),
            table_name,
            shared: dc.shared,
            caption: dc.caption.clone(),
            description: dc.description.clone(),
            visible: dc.visible,
            ordinal: dc.ordinal,
            hierarchy_name: dc.hierarchy_name.clone(),
            all_level_name: dc.all_level_name.clone(),
            leaf_level_name: dc.leaf_level_name.clone(),
            cardinality_hint: dc.cardinality_hint,
        }
    }).collect();

    let measures: Vec<MeasureDef> = config.measures.iter().map(|mc| {
        let id = mc.id.clone();
        let ft_idx = if config.fact_tables.is_empty() {
            0
        } else {
            let ft_id = mc.fact_table.as_deref().unwrap_or_else(|| {
                panic!("config: multi-fact-table config requires measure.fact_table for '{}'", id)
            });
            fact_tables.iter().position(|ft| ft.id == ft_id).unwrap_or_else(|| {
                panic!("config: measure '{}' references unknown fact_table '{}'", id, ft_id)
            })
        };

        let fallback_sql = mc.sql_fallback_file.as_ref().and_then(|f| {
            let path = config_dir.join(f);
            fs::read_to_string(&path).ok().or_else(|| {
                eprintln!("config: measure '{}' sql_fallback_file '{}' not found at {}", id, f, path.display());
                None
            })
        });

        MeasureDef {
            id,
            fact_table_idx: ft_idx,
            semantic_name: mc.malloy_name.clone(),
            physical_expr: mc.physical_expr.clone(),
            sql_expr: mc.sql_expr.clone(),
            caption: mc.caption.clone(),
            display_name: mc.display_name.clone(),
            description: mc.description.clone(),
            visible: mc.visible,
            aggregator: mc.aggregator,
            units: mc.units.clone(),
            format_string: mc.format_string.clone(),
            measure_group_name: mc.measure_group_name.clone(),
            numeric_precision: mc.numeric_precision,
            numeric_scale: mc.numeric_scale,
            expression: mc.expression.clone(),
            sql_fallback_sql: fallback_sql,
        }
    }).collect();

    let relationships: Vec<RelationshipDef> = config.relationships.iter().map(|rc| {
        crate::engine::model::RelationshipDef {
            fact_table_id: rc.fact_table.clone(),
            fact_column: rc.fact_column.clone(),
            dimension_id: rc.dimension_id.clone(),
            dim_table: rc.dim_table.clone(),
            dim_column: rc.dim_column.clone(),
        }
    }).collect();

    // Validate measure_group_name consistency
    for m in &measures {
        let ft_mg = &fact_tables[m.fact_table_idx].measure_group_name;
        if m.measure_group_name != *ft_mg {
            // Soft warn for now — emit to stderr but don't hard-fail.
            // This lets existing projects work while surfacing mismatches.
            eprintln!(
                "config: measure '{}' has measure_group_name '{}' but fact_table uses '{}'",
                m.id, m.measure_group_name, ft_mg,
            );
        }
    }

    SemanticModel {
        fact_tables,
        dialect,
        dimensions,
        measures,
        relationships,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::plan::QueryPlan;

    #[test]
    fn default_project_is_valid() {
        let p = ProxyProject::default_();
        assert_eq!(p.model.primary_source_name(), "faktatabell");
        assert_eq!(p.model.dimensions.len(), 2);
        assert_eq!(p.model.measures.len(), 1);
    }

    #[test]
    fn config_derived_model_matches_default() {
        let p = ProxyProject::default_();
        let built = build_semantic_model(&p.config, Path::new("."));
        assert_eq!(built.primary_source_name(), p.model.primary_source_name());
        assert_eq!(built.dimensions.len(), p.model.dimensions.len());
        assert_eq!(built.measures.len(), p.model.measures.len());
        assert_eq!(
            built.dim_def("Produktkategori").caption,
            p.model.dim_def("Produktkategori").caption,
        );
    }

    // ---- second project (different names, same shape) ----

    #[test]
    fn second_project_loads() {
        let p = ProxyProject::load("../project2/proxy-config.json")
            .expect("load project2");
        assert_eq!(p.config.catalog, "MY_CATALOG");
        assert_eq!(p.config.cube, "SalesCube");
        // Two differently-named dimensions
        assert_eq!(p.model.dimensions.len(), 2);
        let cat = p.model.dim_def("Category");
        assert_eq!(cat.caption, "Category");
        assert_eq!(cat.semantic_name, "produktkategori");
        assert_eq!(cat.physical_field, "produktkategori");
        let ter = p.model.dim_def("Territory");
        assert_eq!(ter.caption, "Territory");
        assert_eq!(ter.semantic_name, "region");
        // One differently-named measure
        assert_eq!(p.model.measures.len(), 1);
        let rev = p.model.meas_def("Revenue");
        assert_eq!(rev.caption, "Revenue");
        assert_eq!(rev.semantic_name, "revenue");
    }

    #[test]
    fn second_project_malloy_source() {
        let p = ProxyProject::load("../project2/proxy-config.json")
            .expect("load project2");
        let plan = QueryPlan::Total { measure: "Revenue".into(), filters: vec![] };
        let src = p.malloy_source(&plan);
        // Should use the loaded model text, not generated model
        assert!(src.contains("source: sales_data is duckdb.table('faktatabell')"));
        assert!(src.contains("measure: revenue is sales.sum()"));
        assert!(src.contains("aggregate: revenue"));
    }

    #[test]
    fn second_project_group_by() {
        let p = ProxyProject::load("../project2/proxy-config.json")
            .expect("load project2");
        let plan = QueryPlan::GroupBy {
            measure: "Revenue".into(),
            group_by: vec!["Category".into(), "Territory".into()],
            filters: vec![],
        };
        let src = p.malloy_source(&plan);
        assert!(src.contains("group_by: produktkategori, region"));
        assert!(src.contains("aggregate: revenue"));
    }

    #[test]
    fn second_project_defaults_differ() {
        let p = ProxyProject::load("../project2/proxy-config.json")
            .expect("load project2");
        assert_eq!(p.model.default_dimension_id().as_deref(), Some("Category"));
        assert_eq!(p.model.default_measure_id().as_deref(), Some("Revenue"));
    }

    // ---- project3: wider model (4 dims, 2 measures) ----

    #[test]
    fn third_project_loads() {
        let p = ProxyProject::load("../project3/proxy-config.json")
            .expect("load project3");
        assert_eq!(p.config.catalog, "SALES_ANALYTICS");
        assert_eq!(p.config.cube, "Sales");
        assert_eq!(p.model.dimensions.len(), 4);
        assert_eq!(p.model.measures.len(), 2);
        assert_eq!(p.model.dim_def("Category").caption, "Category");
        assert_eq!(p.model.dim_def("Territory").caption, "Territory");
        assert_eq!(p.model.dim_def("Channel").caption, "Channel");
        assert_eq!(p.model.dim_def("Segment").caption, "Segment");
        assert_eq!(p.model.meas_def("Revenue").caption, "Revenue");
        assert_eq!(p.model.meas_def("Units").caption, "Units");
    }

    #[test]
    fn third_project_malloy_source() {
        let p = ProxyProject::load("../project3/proxy-config.json")
            .expect("load project3");
        let plan = QueryPlan::Total { measure: "Revenue".into(), filters: vec![] };
        let src = p.malloy_source(&plan);
        assert!(src.contains("measure: total_revenue is revenue.sum()"));
        assert!(src.contains("measure: total_units is units.sum()"));
        assert!(src.contains("aggregate: total_revenue"));
    }

    #[test]
    fn third_project_group_by_2d() {
        let p = ProxyProject::load("../project3/proxy-config.json")
            .expect("load project3");
        let plan = QueryPlan::GroupBy {
            measure: "Revenue".into(),
            group_by: vec!["Category".into(), "Territory".into()],
            filters: vec![],
        };
        let src = p.malloy_source(&plan);
        assert!(src.contains("group_by: category, territory"));
    }

    // ---- multi-fact-table (Phase A) ----

    #[test]
    fn multi_fact_config_parses() {
        let json = r##"{
            "catalog": "TEST", "cube": "Ops",
            "source_name": "sales_data", "table_name": "sales_fact",
            "dialect": "duckdb", "malloy_model_file": "model.malloy",
            "db_path": null,
            "fact_tables": [
                { "id": "sales", "source_name": "sales_data",
                  "table_name": "sales_fact", "measure_group_name": "Sales" },
                { "id": "inventory", "source_name": "inv_data",
                  "table_name": "inv_fact", "measure_group_name": "Inventory" }
            ],
            "dimensions": [
                { "id": "Category", "malloy_name": "cat", "physical_field": "cat",
                  "caption": "Category", "hierarchy_name": "Category",
                  "all_level_name": "(All)", "leaf_level_name": "Category",
                  "ordinal": 1, "visible": true, "has_all": true, "cardinality_hint": 20 }
            ],
            "measures": [
                { "id": "Revenue", "fact_table": "sales",
                  "malloy_name": "rev", "physical_expr": "rev.sum()",
                  "sql_expr": "SUM(rev)", "caption": "Revenue",
                  "display_name": "Revenue", "format_string": "#,##0.00",
                  "units": "USD", "ordinal": 1, "visible": true,
                  "measure_group_name": "Sales" },
                { "id": "Stock", "fact_table": "inventory",
                  "malloy_name": "stock", "physical_expr": "stock.sum()",
                  "sql_expr": "SUM(stock)", "caption": "Stock",
                  "display_name": "Stock", "format_string": "#,##0",
                  "units": "", "ordinal": 2, "visible": true,
                  "measure_group_name": "Inventory" }
            ]
        }"##;
        let cfg: ProxyConfig = serde_json::from_str(json).expect("parse multi-fact config");
        assert_eq!(cfg.fact_tables.len(), 2);
        assert_eq!(cfg.fact_tables[0].id, "sales");
        assert_eq!(cfg.fact_tables[1].id, "inventory");
        assert_eq!(cfg.measures[0].fact_table.as_deref(), Some("sales"));
        assert_eq!(cfg.measures[1].fact_table.as_deref(), Some("inventory"));
    }

    #[test]
    fn multi_fact_model_builds() {
        let cfg: ProxyConfig = serde_json::from_str(
            r#"{
                "catalog": "TEST", "cube": "Ops",
                "source_name": "sales", "table_name": "sf",
                "dialect": "duckdb", "malloy_model_file": "m.malloy",
                "fact_tables": [
                    { "id": "sales", "source_name": "sales", "table_name": "sf", "measure_group_name": "Sales" },
                    { "id": "inv", "source_name": "inv", "table_name": "if", "measure_group_name": "Inventory" }
                ],
                "dimensions": [],
                "measures": [
                    { "id": "R", "fact_table": "sales", "malloy_name": "r", "physical_expr": "r.sum()", "sql_expr": "SUM(r)", "caption": "R", "display_name": "R", "format_string": "", "units": "", "ordinal": 1, "visible": true, "measure_group_name": "Sales" }
                ]
            }"#,
        ).unwrap();
        let m = build_semantic_model(&cfg, Path::new("."));
        assert_eq!(m.fact_tables.len(), 2);
        assert_eq!(m.fact_tables[0].table_name, "sf");
        assert_eq!(m.fact_tables[1].table_name, "if");
        assert_eq!(m.measures.len(), 1);
        assert_eq!(m.measures[0].fact_table_idx, 0);
        assert_eq!(m.fact_table_for_measure("R").id, "sales");
    }

    #[test]
    #[should_panic(expected = "unknown fact_table")]
    fn multi_fact_unknown_fact_table_panics() {
        let cfg: ProxyConfig = serde_json::from_str(
            r#"{
                "catalog": "T", "cube": "C",
                "source_name": "s", "table_name": "t",
                "dialect": "duckdb", "malloy_model_file": "m.malloy",
                "fact_tables": [
                    { "id": "sales", "source_name": "s", "table_name": "t", "measure_group_name": "G" }
                ],
                "dimensions": [],
                "measures": [
                    { "id": "R", "fact_table": "inventory", "malloy_name": "r", "physical_expr": "r.sum()", "sql_expr": "SUM(r)", "caption": "R", "display_name": "R", "format_string": "", "units": "", "ordinal": 1, "visible": true, "measure_group_name": "G" }
                ]
            }"#,
        ).unwrap();
        build_semantic_model(&cfg, Path::new("."));
    }

    #[test]
    #[should_panic(expected = "requires measure.fact_table")]
    fn multi_fact_missing_fact_table_panics() {
        let cfg: ProxyConfig = serde_json::from_str(
            r#"{
                "catalog": "T", "cube": "C",
                "source_name": "s", "table_name": "t",
                "dialect": "duckdb", "malloy_model_file": "m.malloy",
                "fact_tables": [
                    { "id": "sales", "source_name": "s", "table_name": "t", "measure_group_name": "G" }
                ],
                "dimensions": [],
                "measures": [
                    { "id": "R", "malloy_name": "r", "physical_expr": "r.sum()", "sql_expr": "SUM(r)", "caption": "R", "display_name": "R", "format_string": "", "units": "", "ordinal": 1, "visible": true, "measure_group_name": "G" }
                ]
            }"#,
        ).unwrap();
        build_semantic_model(&cfg, Path::new("."));
    }

    #[test]
    fn dimension_fact_table_resolves() {
        let cfg: ProxyConfig = serde_json::from_str(
            r##"{
                "catalog": "T", "cube": "C",
                "source_name": "s", "table_name": "t",
                "dialect": "duckdb", "malloy_model_file": "m.malloy",
                "fact_tables": [
                    { "id": "sales", "source_name": "s", "table_name": "sf", "measure_group_name": "SG" },
                    { "id": "inv", "source_name": "i", "table_name": "if", "measure_group_name": "IG" }
                ],
                "dimensions": [
                    { "id": "Cat", "malloy_name": "c", "physical_field": "c",
                      "caption": "Cat", "hierarchy_name": "C",
                      "all_level_name": "(All)", "leaf_level_name": "C",
                      "ordinal": 1, "visible": true, "has_all": true,
                      "cardinality_hint": 20, "fact_table": "inv" }
                ],
                "measures": [
                    { "id": "R", "fact_table": "sales", "malloy_name": "r", "physical_expr": "r.sum()", "sql_expr": "SUM(r)", "caption": "R", "display_name": "R", "format_string": "", "units": "", "ordinal": 1, "visible": true, "measure_group_name": "SG" }
                ]
            }"##,
        ).unwrap();
        let m = build_semantic_model(&cfg, Path::new("."));
        let dim = m.dim_def("Cat");
        assert_eq!(dim.table_name.as_deref(), Some("if"),
            "dimension Cat should resolve to inv fact table 'if'");
        assert_eq!(m.dim_table("Cat"), "if");
    }

    #[test]
    fn dimension_without_fact_table_uses_primary() {
        let cfg: ProxyConfig = serde_json::from_str(
            r##"{
                "catalog": "T", "cube": "C",
                "source_name": "s", "table_name": "t",
                "dialect": "duckdb", "malloy_model_file": "m.malloy",
                "dimensions": [
                    { "id": "Cat", "malloy_name": "c", "physical_field": "c",
                      "caption": "Cat", "hierarchy_name": "C",
                      "all_level_name": "(All)", "leaf_level_name": "C",
                      "ordinal": 1, "visible": true, "has_all": true,
                      "cardinality_hint": 20 }
                ],
                "measures": [
                    { "id": "R", "malloy_name": "r", "physical_expr": "r.sum()", "sql_expr": "SUM(r)", "caption": "R", "display_name": "R", "format_string": "", "units": "", "ordinal": 1, "visible": true, "measure_group_name": "G" }
                ]
            }"##,
        ).unwrap();
        let m = build_semantic_model(&cfg, Path::new("."));
        assert_eq!(m.dim_table("Cat"), "t",
            "dimension without fact_table should use primary table");
    }

    #[test]
    #[should_panic(expected = "unknown fact_table")]
    fn dimension_unknown_fact_table_panics() {
        let cfg: ProxyConfig = serde_json::from_str(
            r##"{
                "catalog": "T", "cube": "C",
                "source_name": "s", "table_name": "t",
                "dialect": "duckdb", "malloy_model_file": "m.malloy",
                "fact_tables": [
                    { "id": "sales", "source_name": "s", "table_name": "t", "measure_group_name": "G" }
                ],
                "dimensions": [
                    { "id": "Cat", "malloy_name": "c", "physical_field": "c",
                      "caption": "Cat", "hierarchy_name": "C",
                      "all_level_name": "(All)", "leaf_level_name": "C",
                      "ordinal": 1, "visible": true, "has_all": true,
                      "cardinality_hint": 20, "fact_table": "nonexistent" }
                ],
                "measures": [
                    { "id": "R", "malloy_name": "r", "physical_expr": "r.sum()", "sql_expr": "SUM(r)", "caption": "R", "display_name": "R", "format_string": "", "units": "", "ordinal": 1, "visible": true, "measure_group_name": "G" }
                ]
            }"##,
        ).unwrap();
        build_semantic_model(&cfg, Path::new("."));
    }

    // ---- project4: multi-fact with 2 fact tables, shared + scoped dimensions ----

    #[test]
    fn fourth_project_loads() {
        let p = ProxyProject::load("../project4/proxy-config.json")
            .expect("load project4");
        assert_eq!(p.config.catalog, "OPERATIONS_ANALYTICS");
        assert_eq!(p.config.cube, "Operations");
        assert_eq!(p.model.fact_tables.len(), 2);
        assert_eq!(p.model.dimensions.len(), 4);
        assert_eq!(p.model.measures.len(), 4);
    }

    #[test]
    fn fourth_project_fact_table_assignments() {
        let p = ProxyProject::load("../project4/proxy-config.json")
            .expect("load project4");
        let m = &p.model;
        // Measures scoped correctly
        assert_eq!(m.meas_def("Revenue").fact_table_idx, 0, "Revenue -> sales");
        assert_eq!(m.meas_def("Units").fact_table_idx, 0, "Units -> sales");
        assert_eq!(m.meas_def("Stock").fact_table_idx, 1, "Stock -> inventory");
        assert_eq!(m.meas_def("Cost").fact_table_idx, 1, "Cost -> inventory");
        // Fact table names
        assert_eq!(m.fact_tables[0].table_name, "sales_fact");
        assert_eq!(m.fact_tables[1].table_name, "inventory_fact");
    }

    #[test]
    fn fourth_project_dimension_tables() {
        let p = ProxyProject::load("../project4/proxy-config.json")
            .expect("load project4");
        let m = &p.model;
        // Shared dimensions fall back to primary
        assert_eq!(m.dim_table("Category"), "sales_fact",
            "undecorated shared dim uses primary");
        assert_eq!(m.dim_table("Territory"), "sales_fact");
        // Scoped dimensions use their fact table
        assert_eq!(m.dim_table("Channel"), "sales_fact",
            "Channel is scoped to sales");
        assert_eq!(m.dim_table("Warehouse"), "inventory_fact",
            "Warehouse is scoped to inventory");
    }

    #[test]
    fn fourth_project_measure_groups() {
        let p = ProxyProject::load("../project4/proxy-config.json")
            .expect("load project4");
        let m = &p.model;
        assert_eq!(m.fact_tables[0].measure_group_name, "Sales");
        assert_eq!(m.fact_tables[1].measure_group_name, "Inventory");
        // Each measure is in the right group
        assert_eq!(m.meas_def("Revenue").measure_group_name, "Sales");
        assert_eq!(m.meas_def("Stock").measure_group_name, "Inventory");
    }

    #[test]
    fn fourth_project_sql_uses_correct_table() {
        use crate::engine::sql::sql_for_query_plan;
        let p = ProxyProject::load("../project4/proxy-config.json")
            .expect("load project4");
        let m = &p.model;
        let sql = sql_for_query_plan(m, &QueryPlan::Total { measure: "Revenue".into(), filters: vec![] });
        assert!(sql.contains("FROM sales_fact"), "Revenue: {sql}");
        let sql = sql_for_query_plan(m, &QueryPlan::Total { measure: "Stock".into(), filters: vec![] });
        assert!(sql.contains("FROM inventory_fact"), "Stock: {sql}");
    }

    #[test]
    fn fourth_project_malloy_multi_source() {
        use crate::engine::malloy::malloy_model;
        let p = ProxyProject::load("../project4/proxy-config.json")
            .expect("load project4");
        let m = &p.model;
        let out = malloy_model(m);
        assert!(out.contains("source: sales_data"), "should have sales source");
        assert!(out.contains("source: inventory_data"), "should have inventory source");
        assert!(out.contains("total_revenue"), "should have Revenue measure");
        assert!(out.contains("total_stock"), "should have Stock measure");
    }

    #[test]
    fn fourth_project_fact_aware_default_measure() {
        use crate::engine::plan::plan_from_semantic_with_model;
        use crate::mdx_semantic::{SemanticQuery, SemanticQueryKind};
        let p = ProxyProject::load("../project4/proxy-config.json")
            .expect("load project4");
        let query = SemanticQuery {
            kind: SemanticQueryKind::DrilldownCategories,
            dim_props: vec![],
            cell_props: vec![],
            filters: vec![],
            cchildren_leaf_name: None,
            row_dimension: None,
            axis_dimensions: vec!["Warehouse".to_string()],
            slicers: vec![],
            excluded_members: vec![],
            drilldown_member_hierarchy: None,
            measure: None,
        };
        let plan = plan_from_semantic_with_model(&query, &p.model);
        match plan {
            QueryPlan::GroupBy { ref measure, ref group_by, .. } => {
                assert_eq!(group_by[0], "Warehouse", "should group by Warehouse");
                assert!(
                    measure == "Stock" || measure == "Cost",
                    "default measure for Warehouse should be from inventory_fact, got {measure}"
                );
            }
            other => panic!("expected GroupBy plan, got {other:?}"),
        }
    }

    #[test]
    fn fourth_project_fact_aware_default_measure_sales_dim() {
        use crate::engine::plan::plan_from_semantic_with_model;
        use crate::mdx_semantic::{SemanticQuery, SemanticQueryKind};
        let p = ProxyProject::load("../project4/proxy-config.json")
            .expect("load project4");
        let query = SemanticQuery {
            kind: SemanticQueryKind::DrilldownCategories,
            dim_props: vec![],
            cell_props: vec![],
            filters: vec![],
            cchildren_leaf_name: None,
            row_dimension: None,
            axis_dimensions: vec!["Channel".to_string()],
            slicers: vec![],
            excluded_members: vec![],
            drilldown_member_hierarchy: None,
            measure: None,
        };
        let plan = plan_from_semantic_with_model(&query, &p.model);
        match plan {
            QueryPlan::GroupBy { ref measure, .. } => {
                assert!(
                    measure == "Revenue" || measure == "Units",
                    "default measure for Channel should be from sales_fact, got {measure}"
                );
            }
            other => panic!("expected GroupBy plan, got {other:?}"),
        }
    }

    #[test]
    fn unrelated_filter_ignored() {
        use crate::engine::plan::plan_from_semantic_with_model;
        use crate::mdx_semantic::{SemanticQuery, SemanticQueryKind, DimensionFilter};
        let p = ProxyProject::load("../project4/proxy-config.json")
            .expect("load project4");
        // Cost (inventory) with Channel filter (sales-only dimension).
        // Channel should be ignored because it's unrelated.
        let query = SemanticQuery {
            kind: SemanticQueryKind::SlicerAllAndMeasure,
            dim_props: vec![],
            cell_props: vec![],
            filters: vec![DimensionFilter {
                dimension: "Channel".to_string(),
                members: vec!["Online".to_string()],
            }],
            cchildren_leaf_name: None,
            row_dimension: None,
            axis_dimensions: vec![],
            slicers: vec![],
            excluded_members: vec![],
            drilldown_member_hierarchy: None,
            measure: Some("Cost".to_string()),
        };
        let plan = plan_from_semantic_with_model(&query, &p.model);
        match plan {
            QueryPlan::Total { ref filters, .. } => {
                assert!(filters.is_empty(),
                    "Channel filter should be ignored for Cost measure: {filters:?}");
            }
            other => panic!("expected Total plan, got {other:?}"),
        }
    }

    #[test]
    fn shared_filter_passes_through() {
        use crate::engine::plan::plan_from_semantic_with_model;
        use crate::mdx_semantic::{SemanticQuery, SemanticQueryKind, DimensionFilter};
        let p = ProxyProject::load("../project4/proxy-config.json")
            .expect("load project4");
        // Category is shared — it should pass through for any measure.
        let query = SemanticQuery {
            kind: SemanticQueryKind::SlicerAllAndMeasure,
            dim_props: vec![],
            cell_props: vec![],
            filters: vec![DimensionFilter {
                dimension: "Category".to_string(),
                members: vec!["Electronics".to_string()],
            }],
            cchildren_leaf_name: None,
            row_dimension: None,
            axis_dimensions: vec![],
            slicers: vec![],
            excluded_members: vec![],
            drilldown_member_hierarchy: None,
            measure: Some("Cost".to_string()),
        };
        let plan = plan_from_semantic_with_model(&query, &p.model);
        match plan {
            QueryPlan::Total { ref filters, .. } => {
                assert_eq!(filters.len(), 1,
                    "Category filter should pass through (shared dimension)");
                assert_eq!(filters[0].dimension, "Category");
            }
            other => panic!("expected Total plan, got {other:?}"),
        }
    }
}
