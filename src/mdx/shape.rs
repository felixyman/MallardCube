//! The shape of a query: which dimensions and measures sit on which axis.
//!
//! Built once from the parsed statement and consumed by both sides of the
//! engine:
//!
//! - the **plan** mirrors [`QueryShape::flat_dims`] positionally for its
//!   group-by columns, so key column *n* always belongs to the *n*-th
//!   dimension of this shape;
//! - the **renderers** ask [`QueryShape::key_index`] for a dimension's column
//!   instead of deriving positions from the axis specs.
//!
//! That single source is what plan 051's cross-wired-axes bug needed: the plan
//! followed the statement's clause order while the renderer assumed axis
//! order, and nothing tied the two together. `axis_dimensions` is now a view of
//! this shape rather than a second list, and [`QueryShape::validate`] states
//! the invariants the two sides rely on.

use crate::mdx::frontend::{AxisSlot, AxisSpec};

/// One axis: its dimensions, its measures, and the order the statement wrote
/// them in (a cross-joined measure keeps the side it was written on).
#[derive(Debug, Clone, PartialEq)]
pub struct AxisShape {
    /// `ON COLUMNS` = 0, `ON ROWS` = 1.
    pub ordinal: u32,
    /// Dimensions on this axis, in tuple order.
    pub dims: Vec<String>,
    /// Measures on this axis, in tuple order.
    pub measures: Vec<String>,
    /// The interleaving of the two, for tuple construction.
    pub slots: Vec<AxisSlot>,
}

impl AxisShape {
    /// Is this axis the one carrying the measures (a cross-join with Values)?
    pub fn carries_measures(&self) -> bool {
        !self.measures.is_empty()
    }
}

/// Every axis of a statement, in axis order (COLUMNS first).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QueryShape {
    pub axes: Vec<AxisShape>,
}

impl QueryShape {
    /// Build from the parsed axes. `axis_specs` is already in axis order.
    pub fn from_axis_specs(specs: &[AxisSpec]) -> Self {
        Self {
            axes: specs
                .iter()
                .map(|spec| AxisShape {
                    ordinal: spec.ordinal,
                    dims: spec.dims.clone(),
                    measures: spec.measures.clone(),
                    slots: spec.slots.clone(),
                })
                .collect(),
        }
    }

    /// The axes that carry dimensions, in axis order.
    pub fn dim_axes(&self) -> Vec<&AxisShape> {
        self.axes.iter().filter(|a| !a.dims.is_empty()).collect()
    }

    /// Dimensions in key-column order: every dimension of every axis, axes in
    /// order and each axis's dimensions in tuple order. The plan's group-by
    /// columns follow this list positionally.
    pub fn flat_dims(&self) -> Vec<String> {
        self.axes
            .iter()
            .flat_map(|axis| axis.dims.iter().cloned())
            .collect()
    }

    /// The key column of `dim`, or `None` when the shape does not reference it.
    ///
    /// A miss here means the plan and the renderer disagree about the query —
    /// it used to produce silently missing cells (plan 051), so it is loud.
    pub fn key_index(&self, dim: &str) -> Option<usize> {
        let found = self.flat_dims().iter().position(|d| d == dim);
        if found.is_none() {
            debug_assert!(false, "dimension {dim} missing from the query shape");
            eprintln!("!!! query shape: dimension {dim} has no key column in {self:?}");
        }
        found
    }

    /// The invariants the plan and the renderers rely on. Cheap; call it where
    /// the plan is built so a disagreement fails in tests instead of silently
    /// returning a wrong cellset.
    pub fn validate(&self) -> Result<(), String> {
        if self.axes.is_empty() {
            return Ok(()); // slicer-only and probe shapes have no axes
        }
        for axis in &self.axes {
            if axis.dims.is_empty() && axis.measures.is_empty() {
                return Err(format!(
                    "Axis{} has neither dimensions nor measures",
                    axis.ordinal
                ));
            }
            if axis.dims.len() > 2 {
                return Err(format!(
                    "Axis{} carries {} dimensions; at most two are supported",
                    axis.ordinal,
                    axis.dims.len()
                ));
            }
            for dim in &axis.dims {
                if dim.trim().is_empty() {
                    return Err(format!("Axis{} has an empty dimension name", axis.ordinal));
                }
            }
        }
        let flat = self.flat_dims();
        let mut seen = std::collections::HashSet::new();
        for dim in &flat {
            if !seen.insert(dim.clone()) {
                return Err(format!("dimension {dim} appears on more than one axis"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mdx::frontend::parse_select;

    fn shape_of(mdx: &str) -> QueryShape {
        let sel = parse_select(mdx).expect("parse");
        QueryShape::from_axis_specs(&crate::mdx::frontend::axis_specs(&sel))
    }

    /// The flat list follows axis order regardless of how the statement wrote
    /// its clauses — the property whose absence cross-wired the axes.
    #[test]
    fn flat_dims_follow_axis_order_not_clause_order() {
        let rows_first = shape_of(
            "SELECT NON EMPTY [Category].[Category].Members ON ROWS, NON EMPTY [Date].[Calendar].Members ON COLUMNS FROM [Sales]",
        );
        let columns_first = shape_of(
            "SELECT NON EMPTY [Date].[Calendar].Members ON COLUMNS, NON EMPTY [Category].[Category].Members ON ROWS FROM [Sales]",
        );
        assert_eq!(rows_first.flat_dims(), vec!["Date", "Category"]);
        assert_eq!(columns_first.flat_dims(), rows_first.flat_dims());
        assert_eq!(rows_first.key_index("Date"), Some(0));
        assert_eq!(rows_first.key_index("Category"), Some(1));
    }

    /// A cross-joined pair keeps its order, and the measures keep their side.
    #[test]
    fn dims_and_measures_keep_their_edges() {
        let shape = shape_of(
            "SELECT NON EMPTY CrossJoin([Category].[Category].Members, [Channel].[Channel].Members) ON COLUMNS, \
             NON EMPTY CrossJoin([Date].[Calendar].Members, {[Measures].[Revenue]}) ON ROWS FROM [Sales]",
        );
        let dim_axes = shape.dim_axes();
        assert_eq!(dim_axes.len(), 2);
        assert_eq!(dim_axes[0].dims, vec!["Category", "Channel"]);
        assert!(dim_axes[0].measures.is_empty());
        assert_eq!(dim_axes[1].dims, vec!["Date"]);
        assert_eq!(dim_axes[1].measures, vec!["Revenue"]);
        assert_eq!(shape.flat_dims(), vec!["Category", "Channel", "Date"]);
        assert_eq!(shape.key_index("Channel"), Some(1));
        assert!(shape.validate().is_ok());
    }

    #[test]
    fn validate_rejects_duplicated_and_overloaded_axes() {
        let duplicated = QueryShape {
            axes: vec![
                AxisShape {
                    ordinal: 0,
                    dims: vec!["Date".into()],
                    measures: vec![],
                    slots: vec![],
                },
                AxisShape {
                    ordinal: 1,
                    dims: vec!["Date".into()],
                    measures: vec![],
                    slots: vec![],
                },
            ],
        };
        assert!(duplicated.validate().is_err(), "a dimension on two axes");

        let three = QueryShape {
            axes: vec![AxisShape {
                ordinal: 0,
                dims: vec!["A".into(), "B".into(), "C".into()],
                measures: vec![],
                slots: vec![],
            }],
        };
        assert!(three.validate().is_err(), "three dimensions on one axis");
    }
}
