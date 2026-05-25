/// Semantic model — the canonical source of truth for cube metadata.
///
/// This drives:
/// - Malloy emitter
/// - SQL emitter
/// - XMLA metadata rowsets (dimensions, hierarchies, levels, measures)
///
/// Configured from `ProxyConfig` at startup or built from hardcoded
/// defaults via `default_model()`.

use crate::engine::plan::{DimId, MeasId};

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
    pub id: DimId,
    pub semantic_name: String,
    pub physical_field: String,
    /// XMLA DIMENSION_NAME / HIERARCHY_NAME
    pub caption: String,
    /// XMLA DESCRIPTION
    pub description: String,
    /// XMLA DIMENSION_IS_VISIBLE / HIERARCHY_IS_VISIBLE
    pub visible: bool,
    /// XMLA DIMENSION_ORDINAL
    pub ordinal: u32,
    /// XMLA HIERARCHY_UNIQUE_NAME relative part
    pub hierarchy_name: String,
    /// LEVEL_NAME for (All) level
    pub all_level_name: String,
    /// LEVEL_NAME for leaf level
    pub leaf_level_name: String,
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
    pub id: MeasId,
    pub semantic_name: String,
    pub physical_expr: String,
    pub sql_expr: String,
    /// XMLA MEASURE_NAME
    pub caption: String,
    /// XMLA MEASURE_CAPTION / MEASURE_UNQUALIFIED_CAPTION
    pub display_name: String,
    /// XMLA DESCRIPTION
    pub description: String,
    /// XMLA MEASURE_IS_VISIBLE
    pub visible: bool,
    /// XMLA MEASURE_AGGREGATOR (1=sum)
    pub aggregator: u32,
    /// XMLA MEASURE_UNITS
    pub units: String,
    /// XMLA DEFAULT_FORMAT_STRING
    pub format_string: String,
    /// XMLA MEASUREGROUP_NAME
    pub measure_group_name: String,
    /// XMLA NUMERIC_PRECISION
    pub numeric_precision: u16,
    /// XMLA NUMERIC_SCALE
    pub numeric_scale: i16,
    /// XMLA EXPRESSION
    pub expression: String,
}

impl MeasureDef {
    pub fn measure_unique_name(&self) -> String {
        format!("[Measures].[{}]", self.caption)
    }
}

pub struct SemanticModel {
    pub source_name: String,
    pub table_name: String,
    pub dialect: Dialect,
    pub dimensions: Vec<DimensionDef>,
    pub measures: Vec<MeasureDef>,
}

impl SemanticModel {
    pub fn dim_def(&self, id: &str) -> &DimensionDef {
        self.dimensions.iter().find(|d| d.id == id).unwrap()
    }

    pub fn dim_def_opt(&self, id: &str) -> Option<&DimensionDef> {
        self.dimensions.iter().find(|d| d.id == id)
    }

    pub fn meas_def(&self, id: &str) -> &MeasureDef {
        self.measures.iter().find(|m| m.id == id).unwrap()
    }

    pub fn meas_def_opt(&self, id: &str) -> Option<&MeasureDef> {
        self.measures.iter().find(|m| m.id == id)
    }

    /// Find a dimension definition by its XMLA caption.
    pub fn dim_by_caption(&self, caption: &str) -> Option<&DimensionDef> {
        self.dimensions.iter().find(|d| d.caption == caption)
    }

    /// Return the ID of the first visible measure, or None.
    pub fn default_measure_id(&self) -> Option<MeasId> {
        self.measures.iter()
            .find(|m| m.visible)
            .map(|m| m.id.clone())
    }

    /// Return the ID of the first visible dimension, or None.
    pub fn default_dimension_id(&self) -> Option<DimId> {
        self.dimensions.iter()
            .filter(|d| d.visible)
            .min_by_key(|d| d.ordinal)
            .map(|d| d.id.clone())
    }

    /// Try to find a dimension by caption, id, or an XMLA-style bracketed
    /// reference like "[Produktkategori]" or "[Region]".
    pub fn lookup_dimension(&self, text: &str) -> Option<&DimensionDef> {
        let clean = text.trim_matches(|c: char| c == '[' || c == ']');
        self.dimensions.iter().find(|d| {
            d.caption == clean
                || d.id == clean
                || d.id == text
                || d.caption == text
                || d.dimension_unique_name().contains(clean)
        })
    }
}

// ---------------------------------------------------------------------------
// Default model for the current demo dataset
// ---------------------------------------------------------------------------

pub fn default_model() -> SemanticModel {
    SemanticModel {
        source_name: "faktatabell".into(),
        table_name: "faktatabell".into(),
        dialect: Dialect::DuckDB,
        dimensions: vec![
            DimensionDef {
                id: "Produktkategori".into(),
                semantic_name: "produktkategori".into(),
                physical_field: "produktkategori".into(),
                caption: "Produktkategori".into(),
                description: "Våra olika produkter".into(),
                visible: true,
                ordinal: 1,
                hierarchy_name: "Produktkategori".into(),
                all_level_name: "(All)".into(),
                leaf_level_name: "Produktkategori".into(),
                cardinality_hint: 50,
            },
            DimensionDef {
                id: "Region".into(),
                semantic_name: "region".into(),
                physical_field: "region".into(),
                caption: "Region".into(),
                description: "Geografisk region".into(),
                visible: true,
                ordinal: 2,
                hierarchy_name: "Region".into(),
                all_level_name: "(All)".into(),
                leaf_level_name: "Region".into(),
                cardinality_hint: 10,
            },
        ],
        measures: vec![
            MeasureDef {
                id: "TotalSales".into(),
                semantic_name: "total_forsaljning".into(),
                physical_expr: "sales.sum()".into(),
                sql_expr: "SUM(sales)".into(),
                caption: "Total Försäljning".into(),
                display_name: "Total Försäljning (SEK)".into(),
                description: "Vår totala försäljning".into(),
                visible: true,
                aggregator: 1,
                units: "SEK".into(),
                format_string: "#,##0.00 SEK".into(),
                measure_group_name: "Faktatabell".into(),
                numeric_precision: 18,
                numeric_scale: 2,
                expression: "SUM('Faktatabell'[Sales])".into(),
            },
        ],
    }
}
