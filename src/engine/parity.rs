/// Parity harness — verifies that Malloy emission and the direct SQL path
/// produce semantically equivalent SQL for the supported analytic subset.
///
/// Used for the `QueryPlan -> SQL` vs `QueryPlan -> Malloy -> (compiled) SQL`
/// comparison.

#[cfg(test)]
mod tests {
    use crate::backend::{Backend, BenchmarkDataConfig};
    use crate::engine::model::default_model;
    use crate::engine::plan::{
        QueryPlan, Dimension, Measure, TypedDimensionFilter,
        execute_plan_sql_with_backend,
        plan_from_semantic,
    };
    use crate::engine::sql::sql_for_query_plan;
    use crate::engine::malloy::{malloy_for_query_plan, malloy_query};
    use crate::engine::malloy_compiler::{CompileResult, NullCompiler, MalloyCompiler};
    use crate::engine::malloy_node_longlived::LongLivedNodeMalloyCompiler;
    use crate::mdx_semantic::semantic_query_from_mdx;
    use crate::test_fixtures::*;
    use std::sync::OnceLock;

    // ---- result-parity helpers ----

    fn parity_backend() -> &'static Backend {
        static B: OnceLock<Backend> = OnceLock::new();
        B.get_or_init(|| Backend::new_with_config(&BenchmarkDataConfig::small())
            .expect("parity backend"))
    }

    fn parity_compiler() -> &'static LongLivedNodeMalloyCompiler {
        static C: OnceLock<LongLivedNodeMalloyCompiler> = OnceLock::new();
        C.get_or_init(|| LongLivedNodeMalloyCompiler::new().expect("parity compiler"))
    }

    /// Returns (direct_result, malloy_result) using the same backend.
    fn parity_results(plan: &QueryPlan) -> (crate::engine::plan::QueryResult, crate::engine::plan::QueryResult) {
        let m = default_model();
        let backend = parity_backend();

        let direct = execute_plan_sql_with_backend(plan, &sql_for_query_plan(&m, plan), backend);

        let compiler = parity_compiler();
        let cr: CompileResult = compiler.compile_query(&m, plan)
            .expect("parity compile");
        let malloy = execute_plan_sql_with_backend(plan, &cr.sql, backend);

        (direct, malloy)
    }

    // ---- existing tests (updated for CompileResult) ----

    #[test]
    fn slicer_malloy_source_is_deterministic() {
        let model = default_model();
        let query = semantic_query_from_mdx(MDX_SLICER);
        let plan = plan_from_semantic(&query);
        let a = malloy_for_query_plan(&model, &plan);
        let b = malloy_for_query_plan(&model, &plan);
        assert_eq!(a, b);
    }

    #[test]
    fn total_compiler_accepts_supported_plan() {
        let model = default_model();
        let plan = QueryPlan::Total { measure: Measure::TotalSales, filters: vec![] };
        let compiler = NullCompiler;
        let result = compiler.compile_query(&model, &plan);
        assert!(result.is_ok());
    }

    #[test]
    fn group_by_compiler_accepts_1d() {
        let model = default_model();
        let plan = QueryPlan::GroupBy {
            measure: Measure::TotalSales,
            group_by: vec![Dimension::Produktkategori],
            filters: vec![],
        };
        let compiler = NullCompiler;
        assert!(compiler.compile_query(&model, &plan).is_ok());
    }

    #[test]
    fn group_by_compiler_accepts_2d() {
        let model = default_model();
        let plan = QueryPlan::GroupBy {
            measure: Measure::TotalSales,
            group_by: vec![Dimension::Produktkategori, Dimension::Region],
            filters: vec![],
        };
        let compiler = NullCompiler;
        assert!(compiler.compile_query(&model, &plan).is_ok());
    }

    #[test]
    fn compiler_rejects_count_plan() {
        let model = default_model();
        let plan = QueryPlan::Count { dimension: Dimension::Produktkategori };
        let compiler = NullCompiler;
        assert!(compiler.compile_query(&model, &plan).is_err());
    }

    #[test]
    fn compiler_rejects_empty_plan() {
        let model = default_model();
        let plan = QueryPlan::Empty;
        let compiler = NullCompiler;
        assert!(compiler.compile_query(&model, &plan).is_err());
    }

    #[test]
    fn sql_and_malloy_both_produce_text_for_same_plan() {
        let model = default_model();
        let plan = QueryPlan::Total {
            measure: Measure::TotalSales,
            filters: vec![TypedDimensionFilter {
                dimension: Dimension::Region,
                members: vec!["North".into()],
            }],
        };
        let sql = sql_for_query_plan(&model, &plan);
        let malloy = malloy_query(&model, &plan);
        assert!(!sql.is_empty());
        assert!(!malloy.is_empty());
        assert!(malloy.contains("North"));
        assert!(sql.contains("North"));
    }

    // ---- result-parity tests (direct SQL vs Malloy-compiled SQL) ----

    #[test]
    fn parity_total_matches() {
        let plan = QueryPlan::Total { measure: Measure::TotalSales, filters: vec![] };
        let (direct, malloy) = parity_results(&plan);
        let dv = match direct { crate::engine::plan::QueryResult::Scalar(v) => v, _ => panic!() };
        let mv = match malloy { crate::engine::plan::QueryResult::Scalar(v) => v, _ => panic!() };
        assert!((dv - mv).abs() < 0.001, "total mismatch: direct={dv}, malloy={mv}");
    }

    #[test]
    fn parity_group_by_1d_matches() {
        let plan = QueryPlan::GroupBy {
            measure: Measure::TotalSales,
            group_by: vec![Dimension::Produktkategori],
            filters: vec![],
        };
        let (direct, malloy) = parity_results(&plan);
        let mut dg = match direct { crate::engine::plan::QueryResult::Grouped(v) => v, _ => panic!() };
        let mut mg = match malloy { crate::engine::plan::QueryResult::Grouped(v) => v, _ => panic!() };
        dg.sort_by(|a, b| a.0.cmp(&b.0));
        mg.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(dg.len(), mg.len(), "group count mismatch");
        for ((dk, dv), (mk, mv)) in dg.iter().zip(mg.iter()) {
            assert_eq!(dk, mk, "key mismatch");
            assert!((dv - mv).abs() < 0.001, "value mismatch for {dk}: direct={dv}, malloy={mv}");
        }
    }

    #[test]
    fn parity_group_by_2d_matches() {
        let plan = QueryPlan::GroupBy {
            measure: Measure::TotalSales,
            group_by: vec![Dimension::Produktkategori, Dimension::Region],
            filters: vec![],
        };
        let (direct, malloy) = parity_results(&plan);
        let mut dp = match direct { crate::engine::plan::QueryResult::Pairs(v) => v, _ => panic!() };
        let mut mp = match malloy { crate::engine::plan::QueryResult::Pairs(v) => v, _ => panic!() };
        dp.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        mp.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        assert_eq!(dp.len(), mp.len(), "pair count mismatch");
        for ((d0, d1, dv), (m0, m1, mv)) in dp.iter().zip(mp.iter()) {
            assert_eq!(d0, m0, "key0 mismatch");
            assert_eq!(d1, m1, "key1 mismatch");
            assert!((dv - mv).abs() < 0.001, "value mismatch for ({d0},{d1}): direct={dv}, malloy={mv}");
        }
    }

    #[test]
    fn parity_filtered_matches() {
        let plan = QueryPlan::GroupBy {
            measure: Measure::TotalSales,
            group_by: vec![Dimension::Produktkategori],
            filters: vec![TypedDimensionFilter {
                dimension: Dimension::Region,
                members: vec!["Region 01".into()],
            }],
        };
        let (direct, malloy) = parity_results(&plan);
        let mut dg = match direct { crate::engine::plan::QueryResult::Grouped(v) => v, _ => panic!() };
        let mut mg = match malloy { crate::engine::plan::QueryResult::Grouped(v) => v, _ => panic!() };
        dg.sort_by(|a, b| a.0.cmp(&b.0));
        mg.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(dg.len(), mg.len(), "filtered group count mismatch");
        for ((dk, dv), (mk, mv)) in dg.iter().zip(mg.iter()) {
            assert_eq!(dk, mk, "key mismatch");
            assert!((dv - mv).abs() < 0.001, "value mismatch for {dk}: direct={dv}, malloy={mv}");
        }
    }
}
