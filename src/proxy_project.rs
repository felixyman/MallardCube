/// Proxy project — loads a developer's Malloy files and proxy config
/// at startup, producing the runtime `SemanticModel` and the Malloy
/// source text that will be compiled.
///
/// This is the single entry-point that replaces `default_model()` when
/// a config is supplied.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use crate::engine::model::{SemanticModel, DimensionDef, MeasureDef, Dialect};
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

        let model = build_semantic_model(&config);

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
                        aggregator: 1,
                        measure_group_name: "Faktatabell".into(),
                        numeric_precision: 18,
                        numeric_scale: 2,
                        expression: "SUM('Faktatabell'[Sales])".into(),
                    },
                ],
            },
            model: crate::engine::model::default_model(),
            malloy_model_text: String::new(),
        }
    }
}

fn build_semantic_model(config: &ProxyConfig) -> SemanticModel {
    let dialect = match config.dialect.as_str() {
        "duckdb" => Dialect::DuckDB,
        other => panic!("unsupported dialect: {other}"),
    };

    let dimensions: Vec<DimensionDef> = config.dimensions.iter().map(|dc| {
        let id = dc.id.clone();
        DimensionDef {
            id,
            semantic_name: dc.malloy_name.clone(),
            physical_field: dc.physical_field.clone(),
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
        MeasureDef {
            id,
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
        }
    }).collect();

    SemanticModel {
        source_name: config.source_name.clone(),
        table_name: config.table_name.clone(),
        dialect,
        dimensions,
        measures,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::plan::QueryPlan;

    #[test]
    fn default_project_is_valid() {
        let p = ProxyProject::default_();
        assert_eq!(p.model.source_name, "faktatabell");
        assert_eq!(p.model.dimensions.len(), 2);
        assert_eq!(p.model.measures.len(), 1);
    }

    #[test]
    fn config_derived_model_matches_default() {
        let p = ProxyProject::default_();
        let built = build_semantic_model(&p.config);
        assert_eq!(built.source_name, p.model.source_name);
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
        let p = ProxyProject::load("project2/proxy-config.json")
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
        let p = ProxyProject::load("project2/proxy-config.json")
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
        let p = ProxyProject::load("project2/proxy-config.json")
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
        let p = ProxyProject::load("project2/proxy-config.json")
            .expect("load project2");
        assert_eq!(p.model.default_dimension_id().as_deref(), Some("Category"));
        assert_eq!(p.model.default_measure_id().as_deref(), Some("Revenue"));
    }
}
