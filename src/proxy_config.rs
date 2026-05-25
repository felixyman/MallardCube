/// Malloy-to-XMLA projection config.
///
/// A small JSON file that tells the proxy how to present a developer's
/// Malloy model to Excel.  Malloy owns the semantics; this owns the
/// Excel/XMLA-facing presentation details (captions, order, formatting,
/// whether a dimension has an All member, etc.).

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub catalog: String,
    pub cube: String,
    pub source_name: String,
    pub table_name: String,
    pub dialect: String,
    pub malloy_model_file: String,
    pub dimensions: Vec<DimensionConfig>,
    pub measures: Vec<MeasureConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DimensionConfig {
    pub id: String,
    pub malloy_name: String,
    pub physical_field: String,
    pub caption: String,
    #[serde(default)]
    pub description: String,
    pub hierarchy_name: String,
    pub all_level_name: String,
    pub leaf_level_name: String,
    pub ordinal: u32,
    pub visible: bool,
    pub has_all: bool,
    pub cardinality_hint: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MeasureConfig {
    pub id: String,
    pub malloy_name: String,
    pub physical_expr: String,
    pub sql_expr: String,
    pub caption: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    pub format_string: String,
    pub units: String,
    pub ordinal: u32,
    pub visible: bool,
    #[serde(default = "default_aggregator")]
    pub aggregator: u32,
    pub measure_group_name: String,
    #[serde(default = "default_precision")]
    pub numeric_precision: u16,
    #[serde(default = "default_scale")]
    pub numeric_scale: i16,
    #[serde(default)]
    pub expression: String,
}

fn default_aggregator() -> u32 { 1 }
fn default_precision() -> u16 { 18 }
fn default_scale() -> i16 { 2 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_config() {
        let json = r#"{
            "catalog": "TEST",
            "cube": "TestCube",
            "source_name": "test",
            "table_name": "test_table",
            "dialect": "duckdb",
            "malloy_model_file": "model.malloy",
            "dimensions": [{
                "id": "Produktkategori",
                "malloy_name": "produktkategori",
                "physical_field": "produktkategori",
                "caption": "Produktkategori",
                "hierarchy_name": "Produktkategori",
                "all_level_name": "(All)",
                "leaf_level_name": "Produktkategori",
                "ordinal": 1,
                "visible": true,
                "has_all": true,
                "cardinality_hint": 50
            }],
            "measures": [{
                "id": "TotalSales",
                "malloy_name": "total_forsaljning",
                "physical_expr": "sales.sum()",
                "sql_expr": "SUM(sales)",
                "caption": "Total",
                "display_name": "Total (SEK)",
                "format_string": "0.00",
                "units": "SEK",
                "ordinal": 1,
                "visible": true,
                "measure_group_name": "Faktatabell"
            }]
        }"#;
        let cfg: ProxyConfig = serde_json::from_str(json).expect("parse");
        assert_eq!(cfg.catalog, "TEST");
        assert_eq!(cfg.dimensions[0].id, "Produktkategori");
        assert_eq!(cfg.measures[0].caption, "Total");
    }
}
