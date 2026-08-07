use criterion::{Criterion, black_box, criterion_group, criterion_main};
use xmla_proxy::backend::{Backend, BenchmarkDataConfig};
use xmla_proxy::engine::cache::PlanCache;
use xmla_proxy::engine::malloy::{
    malloy_for_query_plan, malloy_query, malloy_source_for_query_plan,
};
use xmla_proxy::engine::malloy_compiler::{CompileResult, MalloyCompiler, NullCompiler};
use xmla_proxy::engine::malloy_node_longlived::LongLivedNodeMalloyCompiler;
use xmla_proxy::engine::model::default_model;
use xmla_proxy::engine::normalize::plan_key;
use xmla_proxy::engine::plan::{
    execute_plan_sql_with_backend, execute_plan_with_backend, plan_from_semantic,
};
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
    BenchCase {
        name: "total",
        mdx: MDX_SLICER,
    },
    BenchCase {
        name: "group1d",
        mdx: MDX_DRILLDOWN,
    },
    BenchCase {
        name: "group1d_filtered",
        mdx: MDX_KAT_ROWS_REGION_FILTER,
    },
    BenchCase {
        name: "group2d",
        mdx: MDX_CROSSJOIN_PROBE,
    },
    BenchCase {
        name: "group2d_nested_filters",
        mdx: MDX_NESTED_BOTH_FILTERS,
    },
    BenchCase {
        name: "collapse",
        mdx: MDX_DRILLDOWN_MEMBER_COLLAPSE,
    },
];

const DATA_CONFIGS: &[(&str, fn() -> BenchmarkDataConfig)] = &[
    ("small", BenchmarkDataConfig::small),
    ("medium", BenchmarkDataConfig::medium),
    ("large", BenchmarkDataConfig::large),
];

fn bench_execute_scale(c: &mut Criterion) {
    let model = default_model();

    for (size_label, config_fn) in DATA_CONFIGS {
        let backend =
            Backend::new_with_config(&config_fn()).expect("failed to create benchmark backend");

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
        let backend =
            Backend::new_with_config(&config_fn()).expect("failed to create benchmark backend");

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

criterion_group!(scale, bench_execute_scale, bench_e2e_scale,);

fn bench_comparison(c: &mut Criterion) {
    let model = default_model();
    let compiler = NullCompiler;
    let cache = PlanCache::new();

    let cases: &[(&str, &str)] = &[
        ("total", MDX_SLICER),
        ("group1d", MDX_DRILLDOWN),
        ("group2d", MDX_CROSSJOIN_PROBE),
        ("filtered", MDX_KAT_ROWS_REGION_FILTER),
    ];

    // Benchmark: direct SQL emission (uncached)
    let mut group = c.benchmark_group("emit_direct");
    for (name, mdx) in cases {
        let query = semantic_query_from_mdx(mdx);
        let plan = plan_from_semantic(&query);
        group.bench_function(*name, |b| {
            b.iter(|| sql_for_query_plan(black_box(&model), black_box(&plan)))
        });
    }
    group.finish();

    // Benchmark: Malloy emission (uncached)
    let mut group = c.benchmark_group("emit_malloy_full");
    for (name, mdx) in cases {
        let query = semantic_query_from_mdx(mdx);
        let plan = plan_from_semantic(&query);
        group.bench_function(*name, |b| {
            b.iter(|| malloy_for_query_plan(black_box(&model), black_box(&plan)))
        });
    }
    group.finish();

    // Benchmark: Malloy compile (via NullCompiler — measures emit+compile overhead)
    let mut group = c.benchmark_group("compile_null");
    for (name, mdx) in cases {
        let query = semantic_query_from_mdx(mdx);
        let plan = plan_from_semantic(&query);
        group.bench_function(*name, |b| {
            b.iter(|| {
                let _ = compiler.compile_query(black_box(&model), black_box(&plan));
            })
        });
    }
    group.finish();

    // Benchmark: PlanKey normalization
    let mut group = c.benchmark_group("normalize_key");
    for (name, mdx) in cases {
        let query = semantic_query_from_mdx(mdx);
        let plan = plan_from_semantic(&query);
        group.bench_function(*name, |b| b.iter(|| plan_key(black_box(&plan))));
    }
    group.finish();

    // Benchmark: Cached SQL lookup
    let mut group = c.benchmark_group("cache_sql_hit");
    for (name, mdx) in cases {
        let query = semantic_query_from_mdx(mdx);
        let plan = plan_from_semantic(&query);
        // Warm the cache
        cache.get_or_generate_sql(&plan, &model);
        group.bench_function(*name, |b| {
            b.iter(|| cache.get_or_generate_sql(black_box(&plan), black_box(&model)))
        });
    }
    group.finish();

    // Benchmark: Cached Malloy lookup
    let mut group = c.benchmark_group("cache_malloy_hit");
    for (name, mdx) in cases {
        let query = semantic_query_from_mdx(mdx);
        let plan = plan_from_semantic(&query);
        cache.get_or_generate_malloy(&plan, &model);
        group.bench_function(*name, |b| {
            b.iter(|| cache.get_or_generate_malloy(black_box(&plan), black_box(&model)))
        });
    }
    group.finish();
}

criterion_group!(comparison, bench_comparison,);

fn bench_malloy_runtime(c: &mut Criterion) {
    let model = default_model();
    let compiler =
        LongLivedNodeMalloyCompiler::new().expect("failed to start long-lived Malloy compiler");
    let cache = PlanCache::new();

    let cases: &[(&str, &str)] = &[
        ("total", MDX_SLICER),
        ("group1d", MDX_DRILLDOWN),
        ("group2d", MDX_CROSSJOIN_PROBE),
        ("filtered", MDX_KAT_ROWS_REGION_FILTER),
    ];

    // Benchmark: warm compile (same source, Malloy internal cache hot)
    let mut group = c.benchmark_group("malloy_compile_warm");
    for (name, mdx) in cases {
        let query = semantic_query_from_mdx(mdx);
        let plan = plan_from_semantic(&query);
        // One warm-up outside measurement
        let _ = compiler.compile_query(&model, &plan);
        group.bench_function(*name, |b| {
            b.iter(|| {
                let _ = compiler.compile_query(black_box(&model), black_box(&plan));
            })
        });
    }
    group.finish();

    // Benchmark: cold compile (unique source each iteration defeats Malloy cache)
    let mut group = c.benchmark_group("malloy_compile_cold");
    let mut counter: u64 = 0;
    for (name, mdx) in cases {
        let query = semantic_query_from_mdx(mdx);
        let plan = plan_from_semantic(&query);
        let base = malloy_source_for_query_plan(&model, &plan);
        // One warm-up to ensure worker/connection is ready
        let _ = compiler.compile_malloy(&base);
        group.bench_function(*name, |b| {
            b.iter(|| {
                counter = counter.wrapping_add(1);
                let unique = format!("{base}\n-- c{counter}");
                let _ = compiler.compile_malloy(black_box(&unique));
            })
        });
    }
    group.finish();

    // Benchmark: cached compiled SQL hit (Rust-level PlanCache)
    let mut group = c.benchmark_group("malloy_compile_cached");
    for (name, mdx) in cases {
        let query = semantic_query_from_mdx(mdx);
        let plan = plan_from_semantic(&query);
        // Warm the cache
        let _ = cache.get_or_compile(&plan, &model, &compiler).unwrap();
        group.bench_function(*name, |b| {
            b.iter(|| {
                let _ =
                    cache.get_or_compile(black_box(&plan), black_box(&model), black_box(&compiler));
            })
        });
    }
    group.finish();

    // Benchmark: direct SQL vs Malloy compile + execute
    let backend =
        Backend::new_with_config(&BenchmarkDataConfig::small()).expect("benchmark backend");
    for (name, mdx) in cases {
        let query = semantic_query_from_mdx(mdx);
        let plan = plan_from_semantic(&query);

        // Direct SQL path
        let mut group = c.benchmark_group(format!("execute_direct_{}", name));
        group.bench_function("direct", |b| {
            b.iter(|| {
                execute_plan_with_backend(black_box(&plan), black_box(&model), black_box(&backend))
            })
        });
        group.finish();

        // Malloy compile + execute path (execute the compiled SQL)
        let mut group = c.benchmark_group(format!("execute_malloy_{}", name));
        let cr: CompileResult = compiler
            .compile_query(&model, &plan)
            .expect("compile for benchmark");
        let compiled_sql = cr.sql;
        group.bench_function("malloy", |b| {
            b.iter(|| {
                execute_plan_sql_with_backend(
                    black_box(&plan),
                    black_box(&compiled_sql),
                    black_box(&backend),
                )
            })
        });
        group.finish();
    }
}

criterion_group!(malloy_runtime, bench_malloy_runtime,);

criterion_main!(pipeline, scale, comparison, malloy_runtime);
