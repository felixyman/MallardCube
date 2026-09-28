//! Contract → proxy-config projection (plan 057-A).
//!
//! The runtime never reads a contract: it serves the projection. Deployment
//! specifics (catalog, cube, database path) are parameters, not contract
//! fields; everything the contract declares maps mechanically, and shapes the
//! config cannot express (an upstream reference, an inactive relationship, a
//! composite date key) are refused rather than guessed.

use crate::project::config::{
    DateDimensionConfig, DateFlagColumns, DimensionConfig, FactTableConfig, HierarchyLevelConfig,
    MeasureConfig, MeasureTimeIntelligenceConfig, ModelPermission, ProxyConfig, RelationshipConfig,
    RoleConfig, TimeIntelligenceConfig, default_aggregator, default_dialect, default_precision,
    default_scale,
};
use crate::project::config_io::{self, ConfigFormat};
use crate::tools::contract::{self, Aggregation, Args, Contract, DEFAULT_CONTRACT_PATH, Measure};

/// Deployment specifics the contract deliberately does not carry.
#[derive(Debug, Clone)]
pub struct Deployment {
    pub catalog: String,
    pub cube: String,
    pub db_path: Option<String>,
}

/// Project a validated contract into the proxy config the runtime reads.
pub fn project(contract: &Contract, deployment: &Deployment) -> Result<ProxyConfig, Vec<String>> {
    let mut findings = Vec::new();

    let fact_id = |table: &str| -> String {
        contract
            .grain
            .iter()
            .find(|grain| grain.table == table)
            .and_then(|grain| grain.id.clone())
            .unwrap_or_else(|| table.to_string())
    };

    let fact_tables = contract
        .grain
        .iter()
        .map(|grain| FactTableConfig {
            id: grain.id.clone().unwrap_or_else(|| grain.table.clone()),
            source_name: String::new(),
            table_name: grain.table.clone(),
            measure_group_name: grain.measure_group.clone().unwrap_or_default(),
        })
        .collect();

    let dimensions = contract
        .dimensions
        .iter()
        .map(|dimension| DimensionConfig {
            id: dimension.id.clone(),
            physical_field: dimension.attribute.clone(),
            caption: if dimension.caption.is_empty() {
                dimension.id.clone()
            } else {
                dimension.caption.clone()
            },
            description: String::new(),
            hierarchy_name: dimension.hierarchy_name.clone().unwrap_or_default(),
            all_level_name: String::new(),
            leaf_level_name: String::new(),
            ordinal: dimension.ordinal.unwrap_or(0),
            visible: dimension.visible,
            has_all: true,
            cardinality_hint: dimension.cardinality_hint.unwrap_or(0),
            fact_table: None,
            shared: false,
            is_date_role: dimension.date_role,
            hierarchy_levels: dimension
                .levels
                .iter()
                .enumerate()
                .map(|(index, level)| HierarchyLevelConfig {
                    name: level.name.clone(),
                    column: level.column.clone(),
                    level_number: index as u32,
                    cardinality: level.cardinality_hint.unwrap_or(0),
                })
                .collect(),
            parent_child: None,
        })
        .collect();

    let relationships = contract
        .relationships
        .iter()
        .filter_map(|relationship| {
            if !relationship.active {
                findings.push(format!(
                    "relationship on '{}' is marked inactive; the proxy config cannot express an \
                     inactive relationship — remove it or make it active",
                    relationship.fact
                ));
                return None;
            }
            let dim_table = contract
                .dimensions
                .iter()
                .find(|dimension| dimension.id == relationship.dimension)
                .map(|dimension| dimension.table.clone())
                .unwrap_or_default();
            Some(RelationshipConfig {
                fact_table: fact_id(&relationship.fact),
                fact_column: relationship.columns[0].clone(),
                dimension_id: relationship.dimension.clone(),
                dim_table,
                dim_column: relationship.columns[1].clone(),
            })
        })
        .collect();

    let measures = contract
        .measures
        .iter()
        .filter_map(|measure| match measure_sql(measure) {
            Ok(sql_expr) => Some(MeasureConfig {
                id: measure.id.clone(),
                sql_expr,
                caption: if measure.caption.is_empty() {
                    measure.id.clone()
                } else {
                    measure.caption.clone()
                },
                display_name: String::new(),
                description: measure.description.clone(),
                format_string: measure.format.clone(),
                units: String::new(),
                ordinal: measure.ordinal.unwrap_or(0),
                visible: measure.visible,
                fact_table: Some(fact_id(&measure.source.table)),
                aggregator: default_aggregator(),
                measure_group_name: String::new(),
                numeric_precision: default_precision(),
                numeric_scale: default_scale(),
                expression: String::new(),
                sql_fallback_file: None,
                time_intelligence: measure.time_window.as_ref().map(|window| {
                    MeasureTimeIntelligenceConfig {
                        flag_column: window.flag.clone(),
                        dimension_id: Some(window.dimension.clone()),
                    }
                }),
                fallback_capability: None,
            }),
            Err(reason) => {
                findings.push(reason);
                None
            }
        })
        .collect();

    let time_intelligence = match project_time_intelligence(contract) {
        Ok(block) => block,
        Err(reasons) => {
            findings.extend(reasons);
            None
        }
    };

    // Security role references: the deployment's auth config binds members and
    // filters; the projection only carries the names so the roles exist.
    let roles = contract
        .security
        .as_ref()
        .map(|security| {
            security
                .roles
                .iter()
                .map(|name| RoleConfig {
                    name: name.clone(),
                    description: String::new(),
                    model_permission: ModelPermission::Read,
                    members: Vec::new(),
                    table_permissions: Vec::new(),
                })
                .collect()
        })
        .unwrap_or_default();

    if !findings.is_empty() {
        return Err(findings);
    }

    Ok(ProxyConfig {
        catalog: deployment.catalog.clone(),
        cube: deployment.cube.clone(),
        source_name: String::new(),
        table_name: String::new(),
        dialect: default_dialect(),
        db_path: deployment.db_path.clone(),
        fact_tables,
        relationships,
        roles,
        auth: None,
        time_intelligence,
        dimensions,
        measures,
        dimensions_file: None,
        measures_file: None,
        relationships_file: None,
        roles_file: None,
    })
}

/// The SQL expression a measure projects to: the declared expression verbatim,
/// or the declared aggregation applied to the declared column. The window
/// filter itself is the engine's; `time_window` aggregates additively.
fn measure_sql(measure: &Measure) -> Result<String, String> {
    if let Some(expression) = measure
        .expression
        .as_deref()
        .filter(|expression| !expression.trim().is_empty())
    {
        return Ok(expression.to_string());
    }
    if let Some(reference) = measure
        .source
        .reference
        .as_deref()
        .filter(|reference| !reference.trim().is_empty())
    {
        return Err(format!(
            "measure '{}' is sourced by upstream reference '{reference}'; expand it to a column \
             or expression before projecting",
            measure.id
        ));
    }
    let column = measure
        .source
        .column
        .as_deref()
        .filter(|column| !column.trim().is_empty());
    match measure.aggregation {
        Aggregation::Sum | Aggregation::TimeWindow => column.map(|column| format!("SUM({column})")),
        Aggregation::Min => column.map(|column| format!("MIN({column})")),
        Aggregation::Max => column.map(|column| format!("MAX({column})")),
        Aggregation::Count => Some(
            column
                .map(|column| format!("COUNT({column})"))
                .unwrap_or_else(|| "COUNT(*)".to_string()),
        ),
        Aggregation::DistinctCount => column.map(|column| format!("COUNT(DISTINCT {column})")),
        Aggregation::Ratio => None,
    }
    .ok_or_else(|| {
        format!(
            "measure '{}' is a {} but declares no column or expression to project",
            measure.id,
            measure.aggregation.label()
        )
    })
}

/// The model-level flag catalogue, from the declared date role's key, leaf and
/// levels. The calendar slots come from levels named Year/Quarter/Month; a
/// missing one stays empty (the engine treats it as unavailable, never guesses).
fn project_time_intelligence(
    contract: &Contract,
) -> Result<Option<TimeIntelligenceConfig>, Vec<String>> {
    let Some(time_intelligence) = &contract.time_intelligence else {
        return Ok(None);
    };
    let Some(date) = contract
        .dimensions
        .iter()
        .find(|dimension| dimension.id == time_intelligence.date_dimension)
    else {
        return Err(vec![format!(
            "time_intelligence names unknown dimension '{}'",
            time_intelligence.date_dimension
        )]);
    };
    let key_columns = date.key.columns();
    if key_columns.len() != 1 {
        return Err(vec![format!(
            "time_intelligence needs a single-column date key; dimension '{}' declares {key_columns:?}",
            date.id
        )]);
    }
    let level_column = |name: &str| {
        date.levels
            .iter()
            .find(|level| level.name.eq_ignore_ascii_case(name))
            .map(|level| level.column.clone())
            .unwrap_or_default()
    };
    Ok(Some(TimeIntelligenceConfig {
        date_dimension: DateDimensionConfig {
            dimension_id: date.id.clone(),
            date_key_column: key_columns[0].to_string(),
            full_date_column: date.attribute.clone(),
            table_name: date.table.clone(),
            flag_columns: DateFlagColumns {
                year_column: level_column("year"),
                quarter_column: level_column("quarter"),
                month_column: level_column("month"),
                ytd_flag_column: time_intelligence.flags.ytd.clone().unwrap_or_default(),
                prior_year_ytd_flag_column: time_intelligence
                    .flags
                    .prior_year_ytd
                    .clone()
                    .unwrap_or_default(),
                current_year_flag_column: time_intelligence
                    .flags
                    .current_year
                    .clone()
                    .unwrap_or_default(),
                qtd_flag_column: time_intelligence.flags.qtd.clone().unwrap_or_default(),
                mtd_flag_column: time_intelligence.flags.mtd.clone().unwrap_or_default(),
            },
        },
    }))
}

/// The machine-readable verdict (house shape): a stable key set on every path.
fn verdict_json(
    ok: bool,
    file: &str,
    out: Option<&str>,
    reasons: &[String],
    exit_code: i32,
) -> String {
    serde_json::json!({
        "contract": "mallardcube.contract/1",
        "verdict": if ok { "projected" } else { "invalid" },
        "ok": ok,
        "file": file,
        "out": out,
        "reasons": reasons,
        "exit_code": exit_code,
    })
    .to_string()
}

/// `mallard contract project <file> [--catalog X] [--cube Y] [--db-path P]
/// [--out PATH] [--json]`.
pub fn run(args: &Args) -> i32 {
    if args.json && args.out.is_none() {
        eprintln!(
            "contract: project --json needs --out <path> so the config and the verdict do not \
             share stdout"
        );
        return 2;
    }
    let file = args.file.as_deref().unwrap_or(DEFAULT_CONTRACT_PATH);
    let contract = match contract::validate_file(file) {
        Ok(contract) => contract,
        Err(reasons) => {
            if args.json {
                println!(
                    "{}",
                    verdict_json(false, file, args.out.as_deref(), &reasons, 1)
                );
            } else {
                for reason in &reasons {
                    println!("  [FAIL] {reason}");
                }
            }
            return 1;
        }
    };
    let deployment = Deployment {
        catalog: args
            .catalog
            .clone()
            .unwrap_or_else(|| contract.model.name.to_uppercase()),
        cube: args
            .cube
            .clone()
            .unwrap_or_else(|| contract.model.name.clone()),
        db_path: args.db_path.clone(),
    };
    let mut config = match project(&contract, &deployment) {
        Ok(config) => config,
        Err(reasons) => {
            if args.json {
                println!(
                    "{}",
                    verdict_json(false, file, args.out.as_deref(), &reasons, 1)
                );
            } else {
                for reason in &reasons {
                    println!("  [FAIL] {reason}");
                }
            }
            return 1;
        }
    };
    // Emit only what the contract declared: derived values stay omitted.
    config.deminimize();
    let yaml = match config_io::serialize(&config, ConfigFormat::Yaml) {
        Ok(yaml) => yaml,
        Err(error) => {
            eprintln!("contract: cannot serialize the projection: {error}");
            return 1;
        }
    };
    match args.out.as_deref() {
        Some(out) => {
            if let Err(error) = std::fs::write(out, &yaml) {
                eprintln!("contract: cannot write {out}: {error}");
                return 1;
            }
            if args.json {
                println!("{}", verdict_json(true, file, Some(out), &[], 0));
            } else {
                println!("Projected {file} -> {out}");
            }
            0
        }
        None => {
            print!("{yaml}");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::config::ProxyConfig;

    const FIXTURE: &str = "contracts/upstream_marts/contract.yaml";
    const CHECKED_IN: &str = "projects/upstream_marts/proxy-config.yaml";

    fn deployment() -> Deployment {
        Deployment {
            catalog: "UPSTREAM_DEMO".into(),
            cube: "Orders".into(),
            db_path: Some("data/upstream_marts.duckdb".into()),
        }
    }

    fn fixture_contract() -> Contract {
        contract::validate_file(FIXTURE).expect("the fixture must validate")
    }

    fn project_fixture() -> ProxyConfig {
        project(&fixture_contract(), &deployment()).expect("the fixture must project")
    }

    /// The projection round-trips: projecting the fixture reproduces the
    /// checked-in config semantically (comments and key order aside).
    #[test]
    fn the_fixture_projects_to_the_checked_in_config() {
        let mut projected = project_fixture();
        projected.deminimize();
        let yaml = config_io::serialize(&projected, ConfigFormat::Yaml).expect("serialize");
        let mut projected: ProxyConfig = yaml_serde::from_str(&yaml).expect("reparse");
        projected.normalize();
        let checked_in =
            config_io::load(std::path::Path::new(CHECKED_IN)).expect("checked-in config");

        assert_eq!(projected.catalog, checked_in.catalog);
        assert_eq!(projected.cube, checked_in.cube);
        assert_eq!(projected.db_path, checked_in.db_path);
        for (label, projected, checked_in) in [
            (
                "fact_tables",
                serde_json::to_value(&projected.fact_tables).unwrap(),
                serde_json::to_value(&checked_in.fact_tables).unwrap(),
            ),
            (
                "dimensions",
                serde_json::to_value(&projected.dimensions).unwrap(),
                serde_json::to_value(&checked_in.dimensions).unwrap(),
            ),
            (
                "relationships",
                serde_json::to_value(&projected.relationships).unwrap(),
                serde_json::to_value(&checked_in.relationships).unwrap(),
            ),
            (
                "measures",
                serde_json::to_value(&projected.measures).unwrap(),
                serde_json::to_value(&checked_in.measures).unwrap(),
            ),
            (
                "time_intelligence",
                serde_json::to_value(&projected.time_intelligence).unwrap(),
                serde_json::to_value(&checked_in.time_intelligence).unwrap(),
            ),
        ] {
            assert_eq!(projected, checked_in, "section '{label}' differs");
        }
    }

    /// Same contract, same bytes — the projection is a pure function.
    #[test]
    fn projection_is_deterministic() {
        let contract = fixture_contract();
        let render = || {
            config_io::serialize(
                &project(&contract, &deployment()).unwrap(),
                ConfigFormat::Yaml,
            )
            .unwrap()
        };
        assert_eq!(render(), render());
    }

    /// Shapes the config cannot express are refused, never guessed.
    #[test]
    fn unexpressible_shapes_are_refused() {
        let mut reference_only = fixture_contract();
        let measure = reference_only
            .measures
            .iter_mut()
            .find(|measure| measure.id == "Revenue")
            .expect("Revenue");
        measure.source.column = None;
        measure.source.reference = Some("mart_revenue".into());
        let findings = project(&reference_only, &deployment()).expect_err("reference-only source");
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("upstream reference")),
            "{findings:?}"
        );

        let mut inactive = fixture_contract();
        inactive.relationships[0].active = false;
        let findings = project(&inactive, &deployment()).expect_err("inactive relationship");
        assert!(
            findings.iter().any(|finding| finding.contains("inactive")),
            "{findings:?}"
        );

        let mut composite_date = fixture_contract();
        let date = composite_date
            .dimensions
            .iter_mut()
            .find(|dimension| dimension.id == "Date")
            .expect("Date");
        date.key = contract::Key::Composite(vec!["date_key".into(), "year".into()]);
        let findings = project(&composite_date, &deployment()).expect_err("composite date key");
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("single-column date key")),
            "{findings:?}"
        );
    }

    /// Security role references are carried so the deployment can bind them.
    #[test]
    fn security_role_references_are_carried() {
        let mut contract = fixture_contract();
        contract.security = Some(contract::Security {
            roles: vec!["SalesEU".into()],
        });
        let config = project(&contract, &deployment()).expect("projection");
        assert_eq!(config.roles.len(), 1);
        assert_eq!(config.roles[0].name, "SalesEU");
    }

    /// The CLI: stdout rendering, the --out write path, and the refusals.
    #[test]
    fn run_exit_codes_and_output() {
        assert_eq!(
            run(&Args {
                action: "project".into(),
                file: Some(FIXTURE.into()),
                catalog: Some("UPSTREAM_DEMO".into()),
                cube: Some("Orders".into()),
                ..Args::default()
            }),
            0
        );
        assert_eq!(
            run(&Args {
                action: "project".into(),
                file: Some(FIXTURE.into()),
                json: true,
                ..Args::default()
            }),
            2,
            "--json without --out must not mix the config and the verdict"
        );
        assert_eq!(
            run(&Args {
                action: "project".into(),
                file: Some("does-not-exist.yaml".into()),
                ..Args::default()
            }),
            1
        );

        let path = std::env::temp_dir().join(format!(
            "mallard-projection-{}-{}.yaml",
            std::process::id(),
            "run_exit_codes_and_output"
        ));
        let args = Args {
            action: "project".into(),
            file: Some(FIXTURE.into()),
            json: true,
            catalog: Some("UPSTREAM_DEMO".into()),
            cube: Some("Orders".into()),
            db_path: Some("data/upstream_marts.duckdb".into()),
            out: Some(path.to_string_lossy().into_owned()),
        };
        assert_eq!(run(&args), 0);
        let config = config_io::load(&path).expect("the projected config loads");
        assert_eq!(config.cube, "Orders");
        assert_eq!(config.dimensions.len(), 6);
        let _ = std::fs::remove_file(&path);
    }
}
