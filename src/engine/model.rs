/// Semantic model — the canonical source of truth for cube metadata.
///
/// This drives:
/// - Malloy emitter
/// - SQL emitter
/// - XMLA metadata rowsets (dimensions, hierarchies, levels, measures)
///
/// Currently static, but designed to be derived from config, source
/// introspection, or external metadata in the future.

use crate::engine::plan::{Dimension, Measure};

// ---------------------------------------------------------------------------
// Model types
// ---------------------------------------------------------------------------

pub enum Dialect {
    DuckDB,
}

impl Dialect {
    pub fn as_malloy_source_prefix(&self) -> &str {
        match self {
            Dialect::DuckDB => "duckdb.table",
        }
    }
}

pub struct DimensionDef {
    pub id: Dimension,
    pub semantic_name: &'static str,
    pub physical_field: &'static str,
    /// XMLA DIMENSION_NAME / HIERARCHY_NAME
    pub caption: &'static str,
    /// XMLA DESCRIPTION
    pub description: &'static str,
    /// XMLA DIMENSION_IS_VISIBLE / HIERARCHY_IS_VISIBLE
    pub visible: bool,
    /// XMLA DIMENSION_ORDINAL
    pub ordinal: u32,
    /// XMLA HIERARCHY_UNIQUE_NAME relative part
    pub hierarchy_name: &'static str,
    /// LEVEL_NAME for (All) level
    pub all_level_name: &'static str,
    /// LEVEL_NAME for leaf level
    pub leaf_level_name: &'static str,
    /// Cardinality hint for XMLA DIMENSION_CARDINALITY
    pub cardinality_hint: u32,
}

impl DimensionDef {
    pub fn dimension_unique_name(&self) -> String {
        format!("[{}]", self.caption)
    }

    pub fn hierarchy_unique_name(&self) -> String {
        format!("[{}].[{}]", self.caption, self.hierarchy_name)
    }

    pub fn all_member_unique_name(&self) -> String {
        format!("[{}].[{}].[All]", self.caption, self.hierarchy_name)
    }

    pub fn all_level_unique_name(&self) -> String {
        format!("[{}].[{}].[{}]", self.caption, self.hierarchy_name, self.all_level_name)
    }

    pub fn leaf_level_unique_name(&self) -> String {
        format!("[{}].[{}].[{}]", self.caption, self.hierarchy_name, self.leaf_level_name)
    }
}

pub struct MeasureDef {
    pub id: Measure,
    pub semantic_name: &'static str,
    pub physical_expr: &'static str,
    pub sql_expr: &'static str,
    /// XMLA MEASURE_NAME
    pub caption: &'static str,
    /// XMLA MEASURE_CAPTION / MEASURE_UNQUALIFIED_CAPTION
    pub display_name: &'static str,
    /// XMLA DESCRIPTION
    pub description: &'static str,
    /// XMLA MEASURE_IS_VISIBLE
    pub visible: bool,
    /// XMLA MEASURE_AGGREGATOR (1=sum)
    pub aggregator: u32,
    /// XMLA MEASURE_UNITS
    pub units: &'static str,
    /// XMLA DEFAULT_FORMAT_STRING
    pub format_string: &'static str,
    /// XMLA MEASUREGROUP_NAME
    pub measure_group_name: &'static str,
    /// XMLA NUMERIC_PRECISION
    pub numeric_precision: u16,
    /// XMLA NUMERIC_SCALE
    pub numeric_scale: i16,
    /// XMLA EXPRESSION
    pub expression: &'static str,
}

impl MeasureDef {
    pub fn measure_unique_name(&self) -> String {
        format!("[Measures].[{}]", self.caption)
    }
}

pub struct SemanticModel {
    pub source_name: &'static str,
    pub table_name: &'static str,
    pub dialect: Dialect,
    pub dimensions: &'static [DimensionDef],
    pub measures: &'static [MeasureDef],
}

impl SemanticModel {
    pub fn dim_def(&self, dim: &Dimension) -> &DimensionDef {
        self.dimensions.iter().find(|d| &d.id == dim).unwrap()
    }

    pub fn meas_def(&self, m: &Measure) -> &MeasureDef {
        self.measures.iter().find(|d| &d.id == m).unwrap()
    }
}

// ---------------------------------------------------------------------------
// Default model for the current demo dataset
// ---------------------------------------------------------------------------

pub fn default_model() -> SemanticModel {
    SemanticModel {
        source_name: "faktatabell",
        table_name: "faktatabell",
        dialect: Dialect::DuckDB,
        dimensions: &[
            DimensionDef {
                id: Dimension::Produktkategori,
                semantic_name: "produktkategori",
                physical_field: "produktkategori",
                caption: "Produktkategori",
                description: "Våra olika produkter",
                visible: true,
                ordinal: 1,
                hierarchy_name: "Produktkategori",
                all_level_name: "(All)",
                leaf_level_name: "Produktkategori",
                cardinality_hint: 50,
            },
            DimensionDef {
                id: Dimension::Region,
                semantic_name: "region",
                physical_field: "region",
                caption: "Region",
                description: "Geografisk region",
                visible: true,
                ordinal: 2,
                hierarchy_name: "Region",
                all_level_name: "(All)",
                leaf_level_name: "Region",
                cardinality_hint: 10,
            },
        ],
        measures: &[
            MeasureDef {
                id: Measure::TotalSales,
                semantic_name: "total_forsaljning",
                physical_expr: "sales.sum()",
                sql_expr: "SUM(sales)",
                caption: "Total Försäljning",
                display_name: "Total Försäljning (SEK)",
                description: "Vår totala försäljning",
                visible: true,
                aggregator: 1,
                units: "SEK",
                format_string: "#,##0.00 SEK",
                measure_group_name: "Faktatabell",
                numeric_precision: 18,
                numeric_scale: 2,
                expression: "SUM('Faktatabell'[Sales])",
            },
        ],
    }
}
