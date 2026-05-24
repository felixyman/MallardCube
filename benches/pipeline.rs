use criterion::{black_box, criterion_group, criterion_main, Criterion};
use xmla_proxy::backend::{Backend, BenchmarkDataConfig, QueryBackend};
use xmla_proxy::engine::model::default_model;
use xmla_proxy::engine::plan::{plan_from_semantic, execute_plan_with_backend};
use xmla_proxy::engine::malloy::malloy_query;
use xmla_proxy::engine::sql::sql_for_query_plan;
use xmla_proxy::execute_builders::get_execute_cellset_response_with_backend;
use xmla_proxy::mdx_parser::parse_mdx;
use xmla_proxy::mdx_semantic::semantic_query_from_mdx;
use xmla_proxy::test_fixtures::*;

// ---------------------------------------------------------------------------
// Pipeline overhead (unchanged — tiny demo dataset via singleton)
// ---------------------------------------------------------------------------

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");
    group.bench_function("parse_slicer", |b| {
        b.iter(|| parse_mdx(black_box(MDX_SLICER)))
    });
    group.bench_function("parse_drilldown", |b| {
        b.iter(|| parse_mdx(black_box(MDX_DRILLDOWN)))
    });
    group.bench_function("parse_crossjoin", |b| {
        b.iter(|| parse_mdx(black_box(MDX_CROSSJOIN_PROBE)))
    });
    group.bench_function("parse_collapse", |b| {
        b.iter(|| parse_mdx(black_box(MDX_DRILLDOWN_MEMBER_COLLAPSE)))
    });
    group.finish();
}

fn bench_semantic(c: &mut Criterion) {
    let mut group = c.benchmark_group("semantic");
    group.bench_function("semantic_slicer", |b| {
        b.iter(|| semantic_query_from_mdx(black_box(MDX_SLICER)))
    });
    group.bench_function("semantic_drilldown", |b| {
        b.iter(|| semantic_query_from_mdx(black_box(MDX_DRILLDOWN)))
    });
    group.bench_function("semantic_crossjoin", |b| {
        b.iter(|| semantic_query_from_mdx(black_box(MDX_CROSSJOIN_PROBE)))
    });
    group.bench_function("semantic_collapse", |b| {
        b.iter(|| semantic_query_from_mdx(black_box(MDX_DRILLDOWN_MEMBER_COLLAPSE)))
    });
    group.finish();
}

fn bench_plan(c: &mut Criterion) {
    let query_slicer = semantic_query_from_mdx(MDX_SLICER);
    let query_drilldown = semantic_query_from_mdx(MDX_DRILLDOWN);
    let query_crossjoin = semantic_query_from_mdx(MDX_CROSSJOIN_PROBE);
    let query_collapse = semantic_query_from_mdx(MDX_DRILLDOWN_MEMBER_COLLAPSE);

    let mut group = c.benchmark_group("plan");
    group.bench_function("plan_from_slicer", |b| {
        b.iter(|| plan_from_semantic(black_box(&query_slicer)))
    });
    group.bench_function("plan_from_drilldown", |b| {
        b.iter(|| plan_from_semantic(black_box(&query_drilldown)))
    });
    group.bench_function("plan_from_crossjoin", |b| {
        b.iter(|| plan_from_semantic(black_box(&query_crossjoin)))
    });
    group.bench_function("plan_from_collapse", |b| {
        b.iter(|| plan_from_semantic(black_box(&query_collapse)))
    });
    group.finish();
}

fn bench_emit(c: &mut Criterion) {
    let model = default_model();
    let plan_slicer = plan_from_semantic(&semantic_query_from_mdx(MDX_SLICER));
    let plan_drilldown = plan_from_semantic(&semantic_query_from_mdx(MDX_DRILLDOWN));
    let plan_crossjoin = plan_from_semantic(&semantic_query_from_mdx(MDX_CROSSJOIN_PROBE));

    let mut group = c.benchmark_group("emit_malloy");
    group.bench_function("malloy_total", |b| {
        b.iter(|| malloy_query(black_box(&model), black_box(&plan_slicer)))
    });
    group.bench_function("malloy_drilldown", |b| {
        b.iter(|| malloy_query(black_box(&model), black_box(&plan_drilldown)))
    });
    group.bench_function("malloy_crossjoin", |b| {
        b.iter(|| malloy_query(black_box(&model), black_box(&plan_crossjoin)))
    });
    group.finish();

    let mut group2 = c.benchmark_group("emit_sql");
    group2.bench_function("sql_total", |b| {
        b.iter(|| sql_for_query_plan(black_box(&model), black_box(&plan_slicer)))
    });
    group2.bench_function("sql_drilldown", |b| {
        b.iter(|| sql_for_query_plan(black_box(&model), black_box(&plan_drilldown)))
    });
    group2.bench_function("sql_crossjoin", |b| {
        b.iter(|| sql_for_query_plan(black_box(&model), black_box(&plan_crossjoin)))
    });
    group2.finish();
}

// ---------------------------------------------------------------------------
// Scale benchmarks — DuckDB backend, dataset-size-dependent
// ---------------------------------------------------------------------------

struct BenchCase {
    name: &'static str,
    mdx: &'static str,
}

const BENCH_CASES: &[BenchCase] = &[
    BenchCase { name: "total", mdx: MDX_SLICER },
    BenchCase { name: "group1d", mdx: MDX_DRILLDOWN },
    BenchCase { name: "group1d_filtered", mdx: MDX_KAT_ROWS_REGION_FILTER },
    BenchCase { name: "group2d", mdx: MDX_CROSSJOIN_PROBE },
    BenchCase { name: "group2d_nested_filters", mdx: MDX_NESTED_BOTH_FILTERS },
    BenchCase { name: "collapse", mdx: MDX_DRILLDOWN_MEMBER_COLLAPSE },
];

const DATA_CONFIGS: &[(&str, fn() -> BenchmarkDataConfig)] = &[
    ("small",  BenchmarkDataConfig::small),
    ("medium", BenchmarkDataConfig::medium),
    ("large",  BenchmarkDataConfig::large),
];

fn bench_execute_scale(c: &mut Criterion) {
    let model = default_model();

    for (size_label, config_fn) in DATA_CONFIGS {
        let backend = Backend::new_with_config(&config_fn())
            .expect("failed to create benchmark backend");

        let mut group = c.benchmark_group(format!("execute_{}", size_label));

        for case in BENCH_CASES {
            let query = semantic_query_from_mdx(case.mdx);
            let plan = plan_from_semantic(&query);
            group.bench_function(case.name, |b| {
                b.iter(|| {
                    execute_plan_with_backend(
                        black_box(&plan),
                        black_box(&model),
                        black_box(&backend),
                    )
                })
            });
        }

        group.finish();
    }
}

fn bench_e2e_scale(c: &mut Criterion) {
    let model = default_model();

    for (size_label, config_fn) in DATA_CONFIGS {
        let backend = Backend::new_with_config(&config_fn())
            .expect("failed to create benchmark backend");

        let mut group = c.benchmark_group(format!("e2e_{}", size_label));

        for case in BENCH_CASES {
            group.bench_function(case.name, |b| {
                b.iter(|| {
                    get_execute_cellset_response_with_backend(
                        black_box(case.mdx),
                        black_box(&backend),
                        black_box(&model),
                    )
                })
            });
        }

        group.finish();
    }
}

criterion_group!(
    pipeline,
    bench_parse,
    bench_semantic,
    bench_plan,
    bench_emit,
);

criterion_group!(
    scale,
    bench_execute_scale,
    bench_e2e_scale,
);

criterion_main!(pipeline, scale);
