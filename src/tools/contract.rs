//! The source-neutral serving contract (plan 057-A).
//!
//! A contract describes the model's *meaning* — grain, keys, relationships and
//! measure declarations — with no SQL and no engine specifics. The runtime
//! never reads it: generators write it, `contract validate` checks it, and it
//! projects to a proxy config. `0.x` is unstable until 1.0, and a field earns
//! core status only with a qualifier check behind it.

use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

/// The contract line this build understands.
pub const SUPPORTED_MAJOR: u32 = 0;
pub const SUPPORTED_MINOR: u32 = 1;
pub const SUPPORTED_VERSION: &str = "0.1";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub contract_version: String,
    pub model: ModelInfo,
    pub grain: Vec<Grain>,
    pub dimensions: Vec<Dimension>,
    pub relationships: Vec<Relationship>,
    pub measures: Vec<Measure>,
    #[serde(default)]
    pub security: Option<Security>,
    pub provenance: Provenance,
    #[serde(default)]
    pub annotations: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelInfo {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grain {
    pub table: String,
    pub key: Key,
}

/// A single key column or a composite key.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Key {
    Single(String),
    Composite(Vec<String>),
}

impl Key {
    pub fn columns(&self) -> Vec<&str> {
        match self {
            Key::Single(column) => vec![column.as_str()],
            Key::Composite(columns) => columns.iter().map(String::as_str).collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dimension {
    pub id: String,
    pub table: String,
    pub key: Key,
    #[serde(default)]
    pub date_role: bool,
    #[serde(default)]
    pub caption: String,
    #[serde(default)]
    pub ordinal: Option<u32>,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub cardinality_hint: Option<u32>,
    #[serde(default)]
    pub levels: Vec<Level>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    pub name: String,
    pub column: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    pub fact: String,
    pub dimension: String,
    /// `[fact column, dimension column]`.
    pub columns: Vec<String>,
    #[serde(default)]
    pub cardinality: Cardinality,
    #[serde(default = "default_true")]
    pub active: bool,
}

/// Only `many_to_one` exists: any other cardinality multiplies fact rows.
#[derive(Debug, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Cardinality {
    #[default]
    ManyToOne,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measure {
    pub id: String,
    #[serde(default)]
    pub caption: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub ordinal: Option<u32>,
    #[serde(default = "default_true")]
    pub visible: bool,
    pub source: Source,
    pub aggregation: Aggregation,
    #[serde(default)]
    pub expression: Option<String>,
    #[serde(default)]
    pub time_window: Option<TimeWindow>,
    #[serde(default)]
    pub valid_grain: Vec<String>,
}

/// Declared, not inferred: a ratio is never emitted as if it were additive.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Aggregation {
    Sum,
    Min,
    Max,
    Count,
    DistinctCount,
    Ratio,
    TimeWindow,
}

impl Aggregation {
    pub fn label(&self) -> &'static str {
        match self {
            Aggregation::Sum => "sum",
            Aggregation::Min => "min",
            Aggregation::Max => "max",
            Aggregation::Count => "count",
            Aggregation::DistinctCount => "distinct_count",
            Aggregation::Ratio => "ratio",
            Aggregation::TimeWindow => "time_window",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub table: String,
    #[serde(default)]
    pub column: Option<String>,
    /// An upstream artifact reference (a model or mart id) instead of a column.
    #[serde(default)]
    pub reference: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeWindow {
    pub dimension: String,
    /// The upstream column the proxy only filters on (e.g. `ytd_flag`).
    pub flag: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Security {
    /// Role references; the deployment's auth config binds them.
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub source_system: SourceSystem,
    #[serde(default)]
    pub source_hash: Option<String>,
    pub generator: String,
    #[serde(default)]
    pub generated_at: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceSystem {
    Sqlmesh,
    Dbt,
    Manual,
    Cube,
}

fn default_true() -> bool {
    true
}

/// Parse `major.minor` from a semver-ish string.
fn version_parts(version: &str) -> Result<(u32, u32), String> {
    let mut parts = version.split('.');
    let major = parts
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .ok_or_else(|| {
            format!("contract_version '{version}' is not semver (expected e.g. 0.1.0)")
        })?;
    let minor = parts
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(0);
    Ok((major, minor))
}

/// The version rules (plan 057-A): an unknown/newer version refuses with an
/// upgrade hint, an older one routes through an explicit migration — never
/// silent tolerance.
pub fn check_version(version: &str) -> Result<(), String> {
    let (major, minor) = version_parts(version)?;
    if major != SUPPORTED_MAJOR {
        return Err(format!(
            "contract_version {version} is on the {major}.x line; this build serves the 0.x \
             contract line (the 1.0 commitment is plan 060's, not yet made)"
        ));
    }
    if minor > SUPPORTED_MINOR {
        return Err(format!(
            "contract_version {version} is newer than this build supports ({SUPPORTED_VERSION}.x); \
             upgrade mallardcube, or migrate the contract with `mallard contract migrate`"
        ));
    }
    if minor < SUPPORTED_MINOR {
        return Err(format!(
            "contract_version {version} predates this build's {SUPPORTED_VERSION}.x; run \
             `mallard contract migrate <file>` (no migration is defined before 0.1)"
        ));
    }
    Ok(())
}

/// Shape checks that never touch the database: unique ids, known references,
/// and declarations complete enough to be checked later.
fn consistency(contract: &Contract) -> Vec<String> {
    let mut findings = Vec::new();
    let tables: HashSet<&str> = contract
        .grain
        .iter()
        .map(|grain| grain.table.as_str())
        .collect();

    let mut dimension_ids: HashSet<&str> = HashSet::new();
    for dimension in &contract.dimensions {
        if !dimension_ids.insert(dimension.id.as_str()) {
            findings.push(format!("duplicate dimension id '{}'", dimension.id));
        }
        if dimension.date_role && dimension.levels.is_empty() {
            findings.push(format!(
                "date dimension '{}' declares no levels; a date role serves its calendar from them",
                dimension.id
            ));
        }
    }

    for relationship in &contract.relationships {
        if !dimension_ids.contains(relationship.dimension.as_str()) {
            findings.push(format!(
                "relationship on '{}' names unknown dimension '{}'",
                relationship.fact, relationship.dimension
            ));
        }
        if !tables.contains(relationship.fact.as_str()) {
            findings.push(format!(
                "relationship on '{}' is not a declared grain table",
                relationship.fact
            ));
        }
    }

    let mut measure_ids: HashSet<&str> = HashSet::new();
    for measure in &contract.measures {
        if !measure_ids.insert(measure.id.as_str()) {
            findings.push(format!("duplicate measure id '{}'", measure.id));
        }
        if !tables.contains(measure.source.table.as_str()) {
            findings.push(format!(
                "measure '{}' sources from '{}', which is not a declared grain table",
                measure.id, measure.source.table
            ));
        }
        if matches!(
            measure.aggregation,
            Aggregation::Ratio | Aggregation::TimeWindow
        ) && measure.expression.is_none()
            && measure.source.column.is_none()
        {
            findings.push(format!(
                "measure '{}' is a {} but declares neither an expression nor a column",
                measure.id,
                measure.aggregation.label()
            ));
        }
    }
    findings
}

/// Read, parse and check one contract file. `Err` carries every finding, so CI
/// can print them all rather than the first.
pub fn validate_file(path: &str) -> Result<Contract, Vec<String>> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| vec![format!("cannot read {path}: {error}")])?;
    let contract: Contract = yaml_serde::from_str(&text)
        .map_err(|error| vec![format!("{path} is not a valid contract: {error}")])?;
    let mut findings = Vec::new();
    if let Err(reason) = check_version(&contract.contract_version) {
        findings.push(reason);
    }
    findings.extend(consistency(&contract));
    if findings.is_empty() {
        Ok(contract)
    } else {
        Err(findings)
    }
}

/// `mallard contract validate <file> [--json]`.
pub fn run(args: Vec<String>) -> i32 {
    let json = args.iter().any(|arg| arg == "--json");
    let positional: Vec<&str> = args
        .iter()
        .skip(1)
        .filter(|arg| !arg.starts_with("--"))
        .map(String::as_str)
        .collect();
    let action = positional.first().copied().unwrap_or("validate");
    let file = positional
        .get(1)
        .copied()
        .unwrap_or("contracts/upstream_marts/contract.yaml");

    match action {
        "validate" => match validate_file(file) {
            Ok(contract) => {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "contract": "mallardcube.contract/1",
                            "valid": true,
                            "file": file,
                            "version": contract.contract_version,
                            "model": contract.model.name,
                            "reasons": [],
                        })
                    );
                } else {
                    println!("=== Contract {file} ===");
                    println!("  version:  {}", contract.contract_version);
                    println!("  model:    {}", contract.model.name);
                    println!(
                        "  shape:    {} grain table(s), {} dimension(s), {} relationship(s), {} measure(s)",
                        contract.grain.len(),
                        contract.dimensions.len(),
                        contract.relationships.len(),
                        contract.measures.len()
                    );
                    println!();
                    println!("Contract: VALID");
                }
                0
            }
            Err(reasons) => {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "contract": "mallardcube.contract/1",
                            "valid": false,
                            "file": file,
                            "reasons": reasons,
                        })
                    );
                } else {
                    println!("=== Contract {file} ===");
                    for reason in &reasons {
                        println!("  [FAIL] {reason}");
                    }
                    println!();
                    println!("Contract: INVALID ({} finding(s))", reasons.len());
                }
                1
            }
        },
        other => {
            eprintln!("contract: unknown action '{other}' (expected: validate)");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "contracts/upstream_marts/contract.yaml";

    /// A minimal valid contract; tests mutate one line at a time.
    fn minimal() -> String {
        r#"
contract_version: "0.1.0"
model: { name: demo }
grain:
  - { table: fact_orders, key: order_id }
dimensions:
  - { id: Date, table: dim_date, key: date_key, date_role: true,
      levels: [ { name: Year, column: year } ] }
relationships:
  - { fact: fact_orders, dimension: Date, columns: [order_date_key, date_key] }
measures:
  - { id: Revenue, source: { table: fact_orders, column: revenue }, aggregation: sum }
provenance: { source_system: manual, generator: test/0.1.0 }
"#
        .to_string()
    }

    fn check(text: &str) -> Vec<String> {
        let contract: Contract = match yaml_serde::from_str(text) {
            Ok(contract) => contract,
            Err(error) => return vec![error.to_string()],
        };
        let mut findings = Vec::new();
        if let Err(reason) = check_version(&contract.contract_version) {
            findings.push(reason);
        }
        findings.extend(consistency(&contract));
        findings
    }

    /// The checked-in fixture is the contract the CI gate runs against.
    #[test]
    fn the_fixture_validates() {
        let contract = validate_file(FIXTURE).expect("the fixture must validate");
        assert_eq!(contract.contract_version, "0.1.0");
        assert_eq!(contract.model.name, "upstream_marts");
        assert_eq!(contract.grain.len(), 3);
        assert_eq!(contract.dimensions.len(), 6);
        assert_eq!(contract.measures.len(), 11);
        // The median declares its mart grain, so the qualifier can refuse a
        // re-aggregation instead of assuming additivity.
        let median = contract
            .measures
            .iter()
            .find(|measure| measure.id == "Median lead time")
            .expect("median measure");
        assert_eq!(median.aggregation, Aggregation::Max);
        assert_eq!(median.valid_grain, vec!["year", "month", "product_key"]);
    }

    /// An unknown core field is a hard error: a rule you think is enforced but
    /// is not is worse than no rule.
    #[test]
    fn an_unknown_core_field_is_refused() {
        let text = minimal().replace(
            "measures:",
            "sql: SELECT SUM(revenue) FROM fact_orders\nmeasures:",
        );
        let findings = check(&text);
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("unknown field") && finding.contains("sql")),
            "{findings:?}"
        );
    }

    /// A newer contract version refuses with an upgrade hint; an older one
    /// routes through an explicit migration.
    #[test]
    fn version_rules_refuse_unknown_versions() {
        assert!(check_version("0.1.0").is_ok());
        assert!(check_version("0.1.7").is_ok());
        let newer = check_version("0.2.0").expect_err("0.2 is newer");
        assert!(newer.contains("newer than this build supports"), "{newer}");
        let older = check_version("0.0.9").expect_err("0.0 predates");
        assert!(older.contains("migrate"), "{older}");
        let foreign = check_version("1.0.0").expect_err("1.x is not this line");
        assert!(foreign.contains("0.x contract line"), "{foreign}");
        assert!(check_version("nonsense").is_err());
    }

    /// Only many_to_one exists: anything else multiplies rows.
    #[test]
    fn a_many_to_many_relationship_is_refused() {
        let text = minimal().replace(
            "columns: [order_date_key, date_key] }",
            "columns: [order_date_key, date_key], cardinality: many_to_many }",
        );
        let findings = check(&text);
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("many_to_many")),
            "{findings:?}"
        );
    }

    /// Cross-field shape: a relationship to an unknown dimension, and a
    /// measure sourced from a table the contract does not declare.
    #[test]
    fn dangling_references_are_refused() {
        let unknown_dimension = minimal().replace("dimension: Date", "dimension: Nope");
        assert!(
            check(&unknown_dimension)
                .iter()
                .any(|finding| finding.contains("unknown dimension 'Nope'")),
            "{:?}",
            check(&unknown_dimension)
        );
        let unknown_table = minimal().replace(
            "table: fact_orders, column: revenue",
            "table: nope, column: revenue",
        );
        assert!(
            check(&unknown_table)
                .iter()
                .any(|finding| finding.contains("not a declared grain table")),
            "{:?}",
            check(&unknown_table)
        );
    }

    /// A ratio without an expression or column cannot be checked later.
    #[test]
    fn an_empty_ratio_is_refused() {
        let text = minimal().replace(
            "source: { table: fact_orders, column: revenue }, aggregation: sum",
            "source: { table: fact_orders }, aggregation: ratio",
        );
        assert!(
            check(&text)
                .iter()
                .any(|finding| finding.contains("neither an expression nor a column")),
            "{:?}",
            check(&text)
        );
    }
}
