//! Contract → proxy-config projection (plan 057-A).
//!
//! The runtime never reads a contract: it serves the projection. Deployment
//! specifics (catalog, cube, database path) are parameters, not contract
//! fields; everything the contract declares maps mechanically, and shapes the
//! config cannot express (an upstream reference, an inactive relationship, a
//! composite date key, an unbound dimension, a second date table) are refused
//! rather than guessed. The config cannot carry the fact grain keys, the model
//! description or the contract's provenance — those exist for qualification
//! and for the generators. The emitted file is the canonical `mallard fmt`
//! form, so reloading it changes nothing.

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
            if relationship.columns.len() != 2 {
                findings.push(format!(
                    "relationship on '{}' must name exactly [fact column, dimension column]",
                    relationship.fact
                ));
                return None;
            }
            let Some(dimension) = contract
                .dimensions
                .iter()
                .find(|dimension| dimension.id == relationship.dimension)
            else {
                findings.push(format!(
                    "relationship on '{}' names unknown dimension '{}'",
                    relationship.fact, relationship.dimension
                ));
                return None;
            };
            Some(RelationshipConfig {
                fact_table: fact_id(&relationship.fact),
                fact_column: relationship.columns[0].clone(),
                dimension_id: relationship.dimension.clone(),
                dim_table: dimension.table.clone(),
                dim_column: relationship.columns[1].clone(),
            })
        })
        .collect();

    // The config binds a dimension to its table and join column only through
    // a relationship (or, for a date role, the global date table), and the
    // engine joins a dimension through its *first* relationship — so every
    // dimension needs one, its relationships must agree on the join columns,
    // and every date role must share the catalogue's table and full-date
    // column. Anything else is a plausible wrong answer, not a fault.
    let date_role = contract.time_intelligence.as_ref().and_then(|catalogue| {
        contract
            .dimensions
            .iter()
            .find(|dimension| dimension.id == catalogue.date_dimension)
    });
    for dimension in &contract.dimensions {
        let active: Vec<&contract::Relationship> = contract
            .relationships
            .iter()
            .filter(|relationship| relationship.dimension == dimension.id && relationship.active)
            .collect();
        if active.is_empty() {
            findings.push(format!(
                "dimension '{}' has no active relationship; the engine binds a dimension to its \
                 table and join column through a relationship, and without one it guesses the \
                 primary fact table",
                dimension.id
            ));
        }
        if let Some(first) = active.first() {
            for other in active.iter().skip(1) {
                if other.columns != first.columns {
                    findings.push(format!(
                        "dimension '{}' is joined differently on '{}' and '{}' ({:?} vs {:?}); the \
                         engine joins a dimension through its first relationship",
                        dimension.id, first.fact, other.fact, first.columns, other.columns
                    ));
                }
            }
        }
        if let Some(date) = date_role.filter(|_| dimension.date_role) {
            if dimension.table != date.table {
                findings.push(format!(
                    "date role '{}' is on '{}' but the config serves one date table globally \
                     ('{}'); a second date table would enumerate members from it",
                    dimension.id, dimension.table, date.table
                ));
            } else if dimension.attribute != date.attribute {
                findings.push(format!(
                    "date role '{}' names full-date column '{}' but the config serves one \
                     full-date column globally ('{}'); a window on this role would filter the \
                     wrong column",
                    dimension.id, dimension.attribute, date.attribute
                ));
            }
        }
    }

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
pub(crate) fn measure_sql(measure: &Measure) -> Result<String, String> {
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
    verdict: &str,
    ok: bool,
    file: &str,
    out: Option<&str>,
    reasons: &[String],
    notes: &[String],
    exit_code: i32,
) -> String {
    serde_json::json!({
        "contract": "mallardcube.contract/1",
        "verdict": verdict,
        "ok": ok,
        "file": file,
        "out": out,
        "reasons": reasons,
        "notes": notes,
        "exit_code": exit_code,
    })
    .to_string()
}

/// A failure path that keeps the JSON verdict shape (house rule: the key set
/// does not change with the outcome).
fn fail(
    args: &Args,
    verdict: &str,
    file: &str,
    reasons: &[String],
    notes: &[String],
    exit_code: i32,
) -> i32 {
    if args.json {
        println!(
            "{}",
            verdict_json(
                verdict,
                false,
                file,
                args.out.as_deref(),
                reasons,
                notes,
                exit_code
            )
        );
    } else {
        for reason in reasons {
            println!("  [FAIL] {reason}");
        }
    }
    exit_code
}

/// The canonical minimal text: normalize (fill derived defaults) then
/// deminimize (omit them again) — exactly what `mallard fmt` writes, so a
/// projected file is `fmt`-stable and reloading it changes nothing. The format
/// follows the target path, like `fmt` does.
fn canonical_text(config: &ProxyConfig, format: ConfigFormat) -> Result<String, String> {
    let mut config = config.clone();
    config.normalize();
    config.deminimize();
    config_io::serialize(&config, format)
}

/// What the operator should know about the emitted file.
fn projection_notes(contract: &Contract, config: &ProxyConfig, args: &Args) -> Vec<String> {
    let mut notes = Vec::new();
    if !config.roles.is_empty() {
        notes.push(format!(
            "{} security role reference(s) projected without members, filters or an `auth` block: \
             the proxy runs admin-default and these roles enforce nothing until the deployment \
             adds auth and table permissions",
            config.roles.len()
        ));
    }
    if args.db_path.is_none() {
        notes.push("no --db-path: the config will use the runtime's demo database".to_string());
    }
    let unmeasured = contract
        .measures
        .iter()
        .filter(|measure| {
            matches!(
                measure.aggregation,
                Aggregation::Count
                    | Aggregation::Min
                    | Aggregation::Max
                    | Aggregation::DistinctCount
            )
        })
        .count();
    if unmeasured > 0 {
        notes.push(format!(
            "{unmeasured} measure(s) declare count/min/max/distinct_count; DISCOVER \
             MEASURE_AGGREGATOR is pinned to Sum until the reference codes are measured \
             (plan 057 leftover)"
        ));
    }
    notes
}

/// `mallard contract project <file> [--catalog X] [--cube Y] [--db-path P]
/// [--out PATH] [--json]`.
pub fn run(args: &Args) -> i32 {
    let file = args.file.as_deref().unwrap_or(DEFAULT_CONTRACT_PATH);
    if args.json && args.out.is_none() {
        return fail(
            args,
            "error",
            file,
            &["project --json needs --out <path> so the config and the verdict do not share stdout"
                .to_string()],
            &[],
            2,
        );
    }
    if args.out.as_deref() == Some("") {
        return fail(
            args,
            "error",
            file,
            &["--out needs a path".to_string()],
            &[],
            2,
        );
    }
    let contract = match contract::validate_file(file) {
        Ok(contract) => contract,
        Err(reasons) => return fail(args, "invalid", file, &reasons, &[], 1),
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
    let config = match project(&contract, &deployment) {
        Ok(config) => config,
        Err(reasons) => return fail(args, "refused", file, &reasons, &[], 1),
    };
    let format = args
        .out
        .as_deref()
        .map(|out| config_io::detect_format(std::path::Path::new(out), ""))
        .unwrap_or(ConfigFormat::Yaml);
    let text = match canonical_text(&config, format) {
        Ok(text) => text,
        Err(error) => {
            let reason = format!("cannot serialize the projection: {error}");
            return fail(args, "error", file, &[reason], &[], 1);
        }
    };
    let mut notes = projection_notes(&contract, &config, args);
    if let Some(out) = args
        .out
        .as_deref()
        .filter(|out| std::path::Path::new(out).exists())
    {
        notes.push(format!(
            "overwriting {out}; a rewrite does not preserve comments"
        ));
    }
    for note in &notes {
        eprintln!("  [NOTE] {note}");
    }
    match args.out.as_deref() {
        Some(out) => {
            if let Err(error) = std::fs::write(out, &text) {
                let reason = format!("cannot write {out}: {error}");
                return fail(args, "error", file, &[reason], &notes, 1);
            }
            if args.json {
                println!(
                    "{}",
                    verdict_json("projected", true, file, Some(out), &[], &notes, 0)
                );
            } else {
                println!("Projected {file} -> {out}");
            }
            0
        }
        None => {
            print!("{text}");
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
        let text = canonical_text(&project_fixture(), ConfigFormat::Yaml).expect("serialize");
        let mut projected: ProxyConfig = yaml_serde::from_str(&text).expect("reparse");
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
        // And nothing outside the named sections drifts (roles, auth, dialect,
        // section files, source/table names).
        assert_eq!(
            serde_json::to_value(&projected).unwrap(),
            serde_json::to_value(&checked_in).unwrap(),
            "the whole config differs"
        );
    }

    /// The emitted file is the canonical minimal form: reloading and
    /// re-emitting it changes nothing. A contract that omits a calendar slot,
    /// a flag or a format must carry the explicit emptiness, because the
    /// loader would otherwise refill conventional names.
    #[test]
    fn the_canonical_form_is_idempotent() {
        let mut contract = fixture_contract();
        let time_intelligence = contract.time_intelligence.as_mut().expect("catalogue");
        time_intelligence.flags.qtd = None;
        contract
            .dimensions
            .iter_mut()
            .find(|dimension| dimension.id == "Date")
            .expect("Date")
            .levels
            .retain(|level| level.name != "Quarter");
        contract.measures[0].format.clear();

        let first = canonical_text(
            &project(&contract, &deployment()).expect("projection"),
            ConfigFormat::Yaml,
        )
        .expect("serialize");
        assert!(
            !first.contains("quarter_column"),
            "an omitted slot must not be written as a guessed name:\n{first}"
        );
        let mut reloaded: ProxyConfig = yaml_serde::from_str(&first).expect("reparse");
        reloaded.normalize();
        let reloaded_flags = &reloaded
            .time_intelligence
            .as_ref()
            .expect("catalogue")
            .date_dimension
            .flag_columns;
        assert!(
            reloaded_flags.quarter_column.is_empty(),
            "reloading must keep the omitted slot unavailable"
        );
        let second = canonical_text(&reloaded, ConfigFormat::Yaml).expect("serialize");
        assert_eq!(first, second);
    }

    /// Display fields survive the projection into the emitted form.
    #[test]
    fn display_fields_are_carried() {
        let mut contract = fixture_contract();
        contract.measures[0].visible = false;
        contract.measures[0].ordinal = Some(9);
        contract.dimensions[0].visible = false;
        let text = canonical_text(
            &project(&contract, &deployment()).expect("projection"),
            ConfigFormat::Yaml,
        )
        .expect("serialize");
        let mut config: ProxyConfig = yaml_serde::from_str(&text).expect("reparse");
        config.normalize();
        assert!(!config.measures[0].visible);
        assert_eq!(config.measures[0].ordinal, 9);
        assert!(!config.dimensions[0].visible);
    }

    /// The config binds a dimension to its table only through a relationship,
    /// joins it through the first relationship, and serves one date table and
    /// full-date column globally: every gap is refused.
    #[test]
    fn unbound_dimensions_and_second_date_tables_are_refused() {
        let mut unbound = fixture_contract();
        unbound
            .relationships
            .retain(|relationship| relationship.dimension != "Category");
        let findings = project(&unbound, &deployment()).expect_err("unbound dimension");
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("has no active relationship")),
            "{findings:?}"
        );

        let ship_date = |table: &str, attribute: &str| contract::Dimension {
            id: "Ship Date".into(),
            table: table.into(),
            key: contract::Key::Single("ship_date_key".into()),
            attribute: attribute.into(),
            hierarchy_name: None,
            date_role: true,
            caption: "Ship Date".into(),
            ordinal: None,
            visible: true,
            cardinality_hint: None,
            levels: vec![contract::Level {
                name: "Full Date".into(),
                column: attribute.into(),
                cardinality_hint: None,
            }],
        };

        let mut second_date = fixture_contract();
        second_date
            .dimensions
            .push(ship_date("dim_ship_date", "ship_date"));
        second_date.relationships.push(contract::Relationship {
            fact: "fact_orders".into(),
            dimension: "Ship Date".into(),
            columns: vec!["ship_date_key".into(), "ship_date_key".into()],
            cardinality: contract::Cardinality::ManyToOne,
            active: true,
        });
        let findings = project(&second_date, &deployment()).expect_err("second date table");
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("one date table globally")),
            "{findings:?}"
        );

        // The same table but a different full-date column: a window on this
        // role would filter the global column, so it is refused too.
        let mut second_attribute = fixture_contract();
        second_attribute
            .dimensions
            .push(ship_date("dim_date", "ship_date"));
        second_attribute.relationships.push(contract::Relationship {
            fact: "fact_orders".into(),
            dimension: "Ship Date".into(),
            columns: vec!["order_date_key".into(), "date_key".into()],
            cardinality: contract::Cardinality::ManyToOne,
            active: true,
        });
        let findings = project(&second_attribute, &deployment()).expect_err("second date column");
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("one full-date column globally")),
            "{findings:?}"
        );

        // Two relationships for one dimension with different join columns:
        // the engine joins through the first, so the disagreement is refused.
        let mut disagreement = fixture_contract();
        disagreement.relationships.push(contract::Relationship {
            fact: "mart_cumulative_month".into(),
            dimension: "Date".into(),
            columns: vec!["month_key".into(), "date_key".into()],
            cardinality: contract::Cardinality::ManyToOne,
            active: true,
        });
        let findings = project(&disagreement, &deployment()).expect_err("disagreeing joins");
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("joined differently")),
            "{findings:?}"
        );
    }

    /// Same contract, same bytes — the projection is a pure function.
    #[test]
    fn projection_is_deterministic() {
        let contract = fixture_contract();
        let render = || {
            canonical_text(
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

        // Library callers can hand over shapes the validator would refuse;
        // these must be findings, not panics or empty joins.
        let mut short_join = fixture_contract();
        short_join.relationships[0].columns.truncate(1);
        let findings = project(&short_join, &deployment()).expect_err("short join pair");
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("exactly [fact column, dimension column]")),
            "{findings:?}"
        );

        let mut unknown_dimension = fixture_contract();
        unknown_dimension.relationships[0].dimension = "Nope".into();
        let findings = project(&unknown_dimension, &deployment()).expect_err("unknown dimension");
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("unknown dimension 'Nope'")),
            "{findings:?}"
        );
    }

    /// The verdict shape is the house convention: the same key set whatever
    /// the outcome, exit code included.
    #[test]
    fn verdict_json_keeps_its_key_set() {
        let verdict: serde_json::Value = serde_json::from_str(&verdict_json(
            "refused",
            false,
            "contract.yaml",
            Some("out.yaml"),
            &["why".into()],
            &["note".into()],
            1,
        ))
        .expect("json");
        let mut keys: Vec<&str> = verdict
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "contract",
                "exit_code",
                "file",
                "notes",
                "ok",
                "out",
                "reasons",
                "verdict"
            ]
        );
        assert_eq!(verdict["verdict"], "refused");
        assert_eq!(verdict["exit_code"], 1);
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

        assert_eq!(
            run(&Args {
                action: "project".into(),
                file: Some(FIXTURE.into()),
                out: Some(String::new()),
                ..Args::default()
            }),
            2,
            "an empty --out must be refused"
        );

        // Without --catalog/--cube the deployment names default to the model.
        let default_path = std::env::temp_dir().join(format!(
            "mallard-projection-{}-defaults.yaml",
            std::process::id()
        ));
        assert_eq!(
            run(&Args {
                action: "project".into(),
                file: Some(FIXTURE.into()),
                out: Some(default_path.to_string_lossy().into_owned()),
                ..Args::default()
            }),
            0
        );
        let config = config_io::load(&default_path).expect("the defaulted config loads");
        assert_eq!(config.catalog, "UPSTREAM_MARTS");
        assert_eq!(config.cube, "upstream_marts");
        let _ = std::fs::remove_file(&default_path);

        // The output format follows the target path, like `fmt`: a .json
        // target is written as JSON and stays loadable.
        let json_path = std::env::temp_dir().join(format!(
            "mallard-projection-{}-format.json",
            std::process::id()
        ));
        assert_eq!(
            run(&Args {
                action: "project".into(),
                file: Some(FIXTURE.into()),
                out: Some(json_path.to_string_lossy().into_owned()),
                ..Args::default()
            }),
            0
        );
        let config = config_io::load(&json_path).expect("the .json projection loads");
        assert_eq!(config.dimensions.len(), 6);
        let _ = std::fs::remove_file(&json_path);
    }
}
