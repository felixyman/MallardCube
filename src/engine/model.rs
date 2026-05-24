/// Semantic model — the canonical source of truth for dimensions, measures,
/// and their physical mappings.
///
/// This is what both the Malloy emitter and the execution engine should
/// agree on. Currently static, but designed to be derived from config,
/// source introspection, or external metadata in the future.

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
}

pub struct MeasureDef {
    pub id: Measure,
    pub semantic_name: &'static str,
    pub physical_expr: &'static str,
    pub sql_expr: &'static str,
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
            },
            DimensionDef {
                id: Dimension::Region,
                semantic_name: "region",
                physical_field: "region",
            },
        ],
        measures: &[
            MeasureDef {
                id: Measure::TotalSales,
                semantic_name: "total_forsaljning",
                physical_expr: "sales.sum()",
                sql_expr: "SUM(sales)",
            },
        ],
    }
}
