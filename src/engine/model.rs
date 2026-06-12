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

/// A fact table that hosts measures and dimensions.
pub struct FactTable {
    pub id: String,
    pub source_name: String,
    pub table_name: String,
    pub measure_group_name: String,
}

/// A relationship between a fact table and a dimension table.
pub struct RelationshipDef {
    pub fact_table_id: String,
    pub fact_column: String,
    pub dimension_id: String,
    pub dim_table: String,
    pub dim_column: String,
}

pub struct DimensionDef {
    pub id: DimId,
    pub semantic_name: String,
    pub physical_field: String,
    /// Physical table for distinct queries (None = use primary fact table)
    pub table_name: Option<String>,
    /// True if this dimension is shared across all fact tables.
    pub shared: bool,
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
    pub fact_table_idx: usize,
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
    /// Pre-loaded fallback SQL text (from sql_fallback/*.sql).
    /// When set, execution uses this SQL directly instead of generating.
    pub sql_fallback_sql: Option<String>,
}

impl MeasureDef {
    pub fn measure_unique_name(&self) -> String {
        format!("[Measures].[{}]", self.caption)
    }
}

pub struct SemanticModel {
    pub fact_tables: Vec<FactTable>,
    pub dialect: Dialect,
    pub dimensions: Vec<DimensionDef>,
    pub measures: Vec<MeasureDef>,
    pub relationships: Vec<RelationshipDef>,
}

impl SemanticModel {
    pub fn fact_table(&self, idx: usize) -> &FactTable {
        &self.fact_tables[idx]
    }

    pub fn fact_table_for_measure(&self, measure_id: &str) -> &FactTable {
        let m = self.meas_def(measure_id);
        &self.fact_tables[m.fact_table_idx]
    }

    /// Backward-compatible: the primary (first) fact table's source name.
    pub fn primary_source_name(&self) -> &str {
        &self.fact_tables[0].source_name
    }

    /// Backward-compatible: the primary (first) fact table's table name.
    pub fn primary_table_name(&self) -> &str {
        &self.fact_tables[0].table_name
    }

    /// Check whether a dimension and a measure belong to compatible
    /// fact tables.  Shared dimensions are always compatible.
    /// Fact-scoped dimensions are only compatible with measures from
    /// the same fact table.
    pub fn dim_is_compatible_with_measure(&self, dim_id: &str, meas_id: &str) -> bool {
        let dim = match self.dim_def_opt(dim_id) {
            Some(d) => d,
            None => return true, // unknown dims are treated as compatible
        };
        if dim.shared {
            return true;
        }
        let meas = self.meas_def(meas_id);
        if let Some(ref dim_table) = dim.table_name {
            return self.fact_tables[meas.fact_table_idx].table_name == *dim_table;
        }
        true
    }

    /// The effective physical table for a dimension.
    /// Falls back to the primary fact table if no explicit binding.
    pub fn dim_table(&self, dim_id: &str) -> &str {
        let dim = self.dim_def(dim_id);
        dim.table_name.as_deref().unwrap_or(self.primary_table_name())
    }

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

    /// Find a relationship that connects this dimension to the fact table.
    pub fn rel_for_dimension(&self, dim_id: &str) -> Option<&RelationshipDef> {
        self.relationships.iter().find(|r| r.dimension_id == dim_id)
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

    /// Return the ID of the first visible measure belonging to a
    /// specific fact table (by physical table name). Falls back to
    /// the global default if no matching measure exists.
    pub fn default_measure_for_table(&self, table_name: &str) -> Option<MeasId> {
        self.measures.iter()
            .find(|m| m.visible && self.fact_tables[m.fact_table_idx].table_name == table_name)
            .or_else(|| self.measures.iter().find(|m| m.visible))
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
        fact_tables: vec![
            FactTable {
                id: "default".into(),
                source_name: "faktatabell".into(),
                table_name: "faktatabell".into(),
                measure_group_name: "Faktatabell".into(),
            },
        ],
        dialect: Dialect::DuckDB,
        dimensions: vec![
            DimensionDef {
                id: "Produktkategori".into(),
                semantic_name: "produktkategori".into(),
                physical_field: "produktkategori".into(),
                table_name: None,
                shared: false,
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
                table_name: None,
                shared: false,
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
                fact_table_idx: 0,
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
                sql_fallback_sql: None,
            },
        ],
        relationships: vec![],
    }
}
