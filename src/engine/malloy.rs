/// Malloy emitter — converts a `QueryPlan` into Malloy source text.
///
/// Emits both the static semantic model and the dynamic query fragment.
/// The model is generated from a `SemanticModel`, not from hardcoded strings.
///
/// Designed to be the semantic target for MDX -> Malloy translation.

use crate::engine::plan::{QueryPlan, TypedDimensionFilter};
use crate::engine::model::SemanticModel;

// ---- public API ----

/// Return the full Malloy source: static model + dynamic query.
pub fn malloy_for_query_plan(model: &SemanticModel, plan: &QueryPlan) -> String {
    format!("{}\n\n{}", malloy_model(model), malloy_query(model, plan))
}

/// Alias for use by the compiler abstraction.
pub fn malloy_source_for_query_plan(model: &SemanticModel, plan: &QueryPlan) -> String {
    malloy_for_query_plan(model, plan)
}

/// Return Malloy source using a pre-loaded model text instead of
/// generating the model definition from `SemanticModel`.
/// The loaded model text is concatenated with the generated query fragment.
pub fn malloy_source_with_model_text(
    model_text: &str,
    model: &SemanticModel,
    plan: &QueryPlan,
) -> String {
    format!("{}\n\n{}", model_text, malloy_query(model, plan))
}

/// Emit only the static model definition.
pub fn malloy_model(model: &SemanticModel) -> String {
    let mut lines = vec![
        format!(
            "source: {} is {}('{}') extend {{",
            model.source_name,
            model.dialect.as_malloy_source_prefix(),
            model.table_name,
        ),
    ];

    for dim in &model.dimensions {
        // Only emit dimension definitions when they rename the column.
        // If semantic_name == physical_field, the column is already a
        // dimension and redefining it causes a Malloy compile error.
        if dim.semantic_name != dim.physical_field {
            lines.push(format!(
                "  dimension: {} is {}",
                dim.semantic_name,
                dim.physical_field,
            ));
        }
    }

    for meas in &model.measures {
        lines.push(format!(
            "  measure: {} is {}",
            meas.semantic_name,
            meas.physical_expr,
        ));
    }

    lines.push("}".to_string());
    lines.join("\n")
}

/// Emit only the dynamic query fragment.
pub fn malloy_query(model: &SemanticModel, plan: &QueryPlan) -> String {
    match plan {
        QueryPlan::Total { measure, filters } => {
            let meas = model.meas_def(measure);
            query_block(model, "aggregate", &[&meas.semantic_name], None, filters)
        }

        QueryPlan::GroupBy { measure, group_by, filters } => {
            let dim_names: Vec<&str> = group_by.iter()
                .map(|d| model.dim_def(d).semantic_name.as_str())
                .collect();
            let meas = model.meas_def(measure);

            let body = if dim_names.is_empty() {
                format!("aggregate: {}", meas.semantic_name)
            } else {
                format!("group_by: {}\n  aggregate: {}", dim_names.join(", "), meas.semantic_name)
            };
            let where_line = where_clause(model, filters);
            let inner = if let Some(w) = where_line {
                format!("{}\n  {}", w, body)
            } else {
                body
            };
            format!("run: {} -> {{\n  {}\n}}", model.source_name, inner)
        }

        QueryPlan::Count { .. } => {
            String::from("-- Count probe: no Malloy equivalent\n")
        }

        QueryPlan::Empty => {
            String::from("-- Empty probe: no Malloy equivalent\n")
        }
    }
}

// ---- helpers ----

fn query_block(
    model: &SemanticModel,
    aggregate: &str,
    aggs: &[&str],
    group_by: Option<&[&str]>,
    filters: &[TypedDimensionFilter],
) -> String {
    let body = match group_by {
        Some(dims) if !dims.is_empty() => {
            format!("group_by: {}\n  {}: {}", dims.join(", "), aggregate, aggs.join(", "))
        }
        _ => {
            format!("{}: {}", aggregate, aggs.join(", "))
        }
    };
    let where_line = where_clause(model, filters);
    let inner = if let Some(w) = where_line {
        format!("{}\n  {}", w, body)
    } else {
        body
    };
    format!("run: {} -> {{\n  {}\n}}", model.source_name, inner)
}

fn where_clause(model: &SemanticModel, filters: &[TypedDimensionFilter]) -> Option<String> {
    let dim_parts: Vec<String> = filters.iter()
        .filter(|f| !f.members.is_empty())
        .filter_map(|f| {
            model.dim_def_opt(&f.dimension).map(|d| (d.semantic_name.as_str(), &f.members))
        })
        .map(|(dim_name, members)| {
            let member_conds: Vec<String> = members.iter()
                .map(|m| format!("{} = '{}'", dim_name, m))
                .collect();
            if member_conds.len() == 1 {
                member_conds.into_iter().next().unwrap()
            } else {
                // OR within the same dimension
                format!("({})", member_conds.join(" or "))
            }
        })
        .collect();

    if dim_parts.is_empty() {
        None
    } else {
        // AND across dimensions: comma-separated in Malloy
        Some(format!("where: {}", dim_parts.join(", ")))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::plan::TypedDimensionFilter;
    use crate::engine::model::default_model;

    #[test]
    fn model_emits_measures_and_source() {
        let model = default_model();
        let out = malloy_model(&model);
        assert!(out.contains("measure: total_forsaljning is sales.sum()"));
        assert!(out.contains("source: faktatabell is duckdb.table('faktatabell')"));
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["Produktkategori".into()],
            filters: vec![],
        };
        let query = malloy_query(&model, &plan);
        assert!(query.contains("group_by: produktkategori"));
    }

    #[test]
    fn query_total_no_filters() {
        let plan = QueryPlan::Total { measure: "TotalSales".into(), filters: vec![] };
        let out = malloy_query(&default_model(), &plan);
        assert!(out.contains("aggregate: total_forsaljning"));
        assert!(!out.contains("where:"));
    }

    #[test]
    fn query_total_with_filter() {
        let plan = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![TypedDimensionFilter {
                dimension: "Region".into(),
                members: vec!["North".into()],
            }],
        };
        let out = malloy_query(&default_model(), &plan);
        assert!(out.contains("where: region = 'North'"));
        assert!(out.contains("aggregate: total_forsaljning"));
    }

    #[test]
    fn query_group_by_one_dim() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["Produktkategori".into()],
            filters: vec![],
        };
        let out = malloy_query(&default_model(), &plan);
        assert!(out.contains("group_by: produktkategori"));
        assert!(out.contains("aggregate: total_forsaljning"));
    }

    #[test]
    fn query_group_by_two_dims() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["Produktkategori".into(), "Region".into()],
            filters: vec![],
        };
        let out = malloy_query(&default_model(), &plan);
        assert!(out.contains("group_by: produktkategori, region"));
    }

    #[test]
    fn query_group_by_with_filter() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["Produktkategori".into()],
            filters: vec![TypedDimensionFilter {
                dimension: "Region".into(),
                members: vec!["North".into()],
            }],
        };
        let out = malloy_query(&default_model(), &plan);
        assert!(out.contains("where: region = 'North'"));
        assert!(out.contains("group_by: produktkategori"));
    }

    #[test]
    fn full_emission_includes_model_and_query() {
        let plan = QueryPlan::GroupBy {
            measure: "TotalSales".into(),
            group_by: vec!["Produktkategori".into(), "Region".into()],
            filters: vec![],
        };
        let out = malloy_for_query_plan(&default_model(), &plan);
        assert!(out.contains("source: faktatabell is duckdb.table('faktatabell')"));
        assert!(out.contains("group_by: produktkategori, region"));
    }

    #[test]
    fn query_two_dim_filters_use_and() {
        let plan = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![
                TypedDimensionFilter {
                    dimension: "Region".into(),
                    members: vec!["North".into()],
                },
                TypedDimensionFilter {
                    dimension: "Produktkategori".into(),
                    members: vec!["Kategori A".into()],
                },
            ],
        };
        let out = malloy_query(&default_model(), &plan);
        // Different dimensions = AND: comma-separated, no `|` between dims
        assert!(out.contains("region = 'North'"));
        assert!(out.contains("produktkategori = 'Kategori A'"));
        assert!(!out.contains(" | "), "different dims must not use OR: {out}");
    }

    #[test]
    fn query_same_dim_multi_member_uses_or() {
        let plan = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![TypedDimensionFilter {
                dimension: "Produktkategori".into(),
                members: vec!["Kategori A".into(), "Kategori B".into()],
            }],
        };
        let out = malloy_query(&default_model(), &plan);
        // Same dimensions = OR grouped
        assert!(out.contains("(produktkategori = 'Kategori A' or produktkategori = 'Kategori B')"));
    }

    #[test]
    fn query_mixed_filters_handles_both() {
        let plan = QueryPlan::Total {
            measure: "TotalSales".into(),
            filters: vec![
                TypedDimensionFilter {
                    dimension: "Region".into(),
                    members: vec!["North".into()],
                },
                TypedDimensionFilter {
                    dimension: "Produktkategori".into(),
                    members: vec!["Kategori A".into(), "Kategori B".into()],
                },
            ],
        };
        let out = malloy_query(&default_model(), &plan);
        // Region = single member, Produktkategori = multi-member
        assert!(out.contains("region = 'North'"));
        assert!(out.contains("(produktkategori = 'Kategori A' or produktkategori = 'Kategori B')"));
        // AND across dims = comma-separated, no bare `|`
        assert!(!out.contains(" | "), "different dims must not use OR: {out}");
    }
}
