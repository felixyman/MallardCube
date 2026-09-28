//! Contract-sourced qualification (plan 057-A, section A step 3).
//!
//! The qualifier's default checks come from the served config. `--contract`
//! adds what the config cannot carry — the grain keys, the per-grain validity
//! of identity aggregates, the declared columns and expressions — checks them
//! against the data, and verifies that the served config is the projection of
//! the contract it claims to implement. Every check here is falsifiable: a
//! contract that declares something the data does not satisfy is a blocker,
//! not a note.

use crate::backend::QueryBackend;
use crate::proxy_project::ProxyProject;
use crate::tools::contract::Contract;
use crate::tools::contract_project::{self, Deployment};

/// Every contract check against one database. `(blocked, partial)`.
pub(crate) fn contract_findings<B: QueryBackend + ?Sized>(
    backend: &B,
    contract: &Contract,
    project: &ProxyProject,
) -> (Vec<String>, Vec<String>) {
    let mut blocked = Vec::new();
    let mut partial = Vec::new();

    correspondence(contract, project, &mut blocked, &mut partial);
    grain_uniqueness(backend, contract, &mut blocked, &mut partial);
    valid_grain_uniqueness(backend, contract, &mut blocked, &mut partial);
    measure_bindings(backend, contract, &mut blocked);
    column_existence(backend, contract, &mut blocked);

    (blocked, partial)
}

/// The served config must be the projection of this contract: a stale or
/// hand-edited config means the qualification would describe a different
/// model than the one that is served. Deployment-specific parts (`auth`,
/// catalog/cube/db path, extra role bindings) are deliberately not compared.
fn correspondence(
    contract: &Contract,
    project: &ProxyProject,
    blocked: &mut Vec<String>,
    partial: &mut Vec<String>,
) {
    let deployment = Deployment {
        catalog: project.config.catalog.clone(),
        cube: project.config.cube.clone(),
        db_path: project.config.db_path.clone(),
    };
    let mut projected = match contract_project::project(contract, &deployment) {
        Ok(projected) => projected,
        Err(findings) => {
            blocked.extend(findings);
            return;
        }
    };
    projected.normalize();
    let mut served = project.config.clone();
    served.normalize();

    compare(
        "fact table",
        &projected.fact_tables,
        &served.fact_tables,
        |fact| fact.id.clone(),
        blocked,
        partial,
    );
    compare(
        "dimension",
        &projected.dimensions,
        &served.dimensions,
        |dimension| dimension.id.clone(),
        blocked,
        partial,
    );
    compare(
        "measure",
        &projected.measures,
        &served.measures,
        |measure| measure.id.clone(),
        blocked,
        partial,
    );
    compare(
        "relationship",
        &projected.relationships,
        &served.relationships,
        |relationship| {
            format!(
                "{}.{} -> {}.{}",
                relationship.fact_table,
                relationship.fact_column,
                relationship.dimension_id,
                relationship.dim_column
            )
        },
        blocked,
        partial,
    );

    match (&projected.time_intelligence, &served.time_intelligence) {
        (Some(expected), Some(actual)) => {
            if serde_json::to_value(expected).ok() != serde_json::to_value(actual).ok() {
                blocked.push(
                    "the served time_intelligence block differs from the contract's flag \
                     catalogue; regenerate the projection (`mallard contract project`)"
                        .to_string(),
                );
            }
        }
        (Some(_), None) => blocked.push(
            "the served config has no time_intelligence block the contract declares".to_string(),
        ),
        (None, Some(_)) => partial.push(
            "the served config declares a time_intelligence block the contract does not"
                .to_string(),
        ),
        (None, None) => {}
    }

    let declared_roles: Vec<&str> = contract
        .security
        .as_ref()
        .map(|security| security.roles.iter().map(String::as_str).collect())
        .unwrap_or_default();
    for role in declared_roles {
        if !served
            .roles
            .iter()
            .any(|served_role| served_role.name == role)
        {
            blocked.push(format!(
                "security role '{role}' is declared but missing from the served config"
            ));
        }
    }
}

/// Compare one config section entry by entry: a difference or a missing entry
/// blocks, an extra entry in the served config is a partial finding (the
/// contract does not describe everything that is served).
fn compare<T: serde::Serialize>(
    label: &str,
    expected: &[T],
    served: &[T],
    id_of: impl Fn(&T) -> String,
    blocked: &mut Vec<String>,
    partial: &mut Vec<String>,
) {
    for expected_entry in expected {
        let id = id_of(expected_entry);
        match served.iter().find(|entry| id_of(entry) == id) {
            Some(served_entry) => {
                if serde_json::to_value(expected_entry).ok()
                    != serde_json::to_value(served_entry).ok()
                {
                    blocked.push(format!(
                        "{label} '{id}' differs from the served config; regenerate the projection \
                         (`mallard contract project`)"
                    ));
                }
            }
            None => blocked.push(format!(
                "{label} '{id}' is declared but missing from the served config"
            )),
        }
    }
    for served_entry in served {
        let id = id_of(served_entry);
        if !expected.iter().any(|entry| id_of(entry) == id) {
            partial.push(format!(
                "the served config has {label} '{id}' that the contract does not declare"
            ));
        }
    }
}

/// Every declared grain key must be unique in its table, or a pivot at that
/// grain multiplies rows. NULL key columns are unaddressable rows.
fn grain_uniqueness<B: QueryBackend + ?Sized>(
    backend: &B,
    contract: &Contract,
    blocked: &mut Vec<String>,
    partial: &mut Vec<String>,
) {
    for grain in &contract.grain {
        let columns = grain.key.columns();
        if columns.is_empty() {
            continue; // the validator refuses this
        }
        let duplicates = match duplicate_groups(backend, &grain.table, &columns) {
            Ok(duplicates) => duplicates,
            Err(failure) => {
                blocked.push(format!(
                    "grain '{}' on ({}) cannot be checked: {failure}",
                    grain.table,
                    columns.join(", ")
                ));
                continue;
            }
        };
        if duplicates > 0.0 {
            blocked.push(format!(
                "grain '{}' on ({}) is not unique: {duplicates} duplicate key(s); a pivot at that \
                 grain would multiply rows",
                grain.table,
                columns.join(", ")
            ));
        }
        let nulls = match null_keys(backend, &grain.table, &columns) {
            Ok(nulls) => nulls,
            Err(failure) => {
                blocked.push(format!(
                    "grain '{}' NULL check cannot run: {failure}",
                    grain.table
                ));
                continue;
            }
        };
        if nulls > 0.0 {
            partial.push(format!(
                "grain '{}' has {nulls} row(s) with a NULL key column; those rows are not \
                 addressable at the declared grain",
                grain.table
            ));
        }
    }
}

/// A per-grain mart value is an identity aggregate: the declared `valid_grain`
/// must be unique in the source table, or the value would be re-aggregated
/// across grains (the exact non-additive hazard the contract declares).
fn valid_grain_uniqueness<B: QueryBackend + ?Sized>(
    backend: &B,
    contract: &Contract,
    blocked: &mut Vec<String>,
    partial: &mut Vec<String>,
) {
    let mut checked: Vec<(String, Vec<String>)> = Vec::new();
    for measure in &contract.measures {
        let valid_grain = measure.valid_grain();
        if valid_grain.is_empty() {
            continue;
        }
        let columns: Vec<String> = valid_grain.to_vec();
        if checked
            .iter()
            .any(|(table, columns)| table == &measure.source.table && columns == valid_grain)
        {
            continue;
        }
        checked.push((measure.source.table.clone(), columns.clone()));
        let column_refs: Vec<&str> = columns.iter().map(String::as_str).collect();
        let duplicates = match duplicate_groups(backend, &measure.source.table, &column_refs) {
            Ok(duplicates) => duplicates,
            Err(failure) => {
                blocked.push(format!(
                    "measure '{}' valid_grain ({}) cannot be checked: {failure}",
                    measure.id,
                    columns.join(", ")
                ));
                continue;
            }
        };
        if duplicates > 0.0 {
            blocked.push(format!(
                "measure '{}' declares valid_grain ({}) but '{}' has {duplicates} key(s) with more \
                 than one row; the identity aggregate would re-aggregate across grains",
                measure.id,
                columns.join(", "),
                measure.source.table
            ));
        }
        let _ = partial;
    }
}

/// The declared SQL must bind against the declared table: this is where a
/// typo'd expression (`SUM(revnue)`) or a missing column stops.
fn measure_bindings<B: QueryBackend + ?Sized>(
    backend: &B,
    contract: &Contract,
    blocked: &mut Vec<String>,
) {
    for measure in &contract.measures {
        let sql = match contract_project::measure_sql(measure) {
            Ok(sql) => sql,
            Err(reason) => {
                blocked.push(reason);
                continue;
            }
        };
        let _ = backend.take_failure();
        let _ = backend.query_rows(&format!(
            "EXPLAIN SELECT {sql} FROM \"{}\"",
            measure.source.table
        ));
        if let Some(failure) = backend.take_failure() {
            blocked.push(format!(
                "measure '{}' does not bind against '{}' (`{sql}`): {failure}",
                measure.id, measure.source.table
            ));
        }
    }
}

/// The columns the contract names must exist: dimension attributes and level
/// columns, the date key and the declared flag columns.
fn column_existence<B: QueryBackend + ?Sized>(
    backend: &B,
    contract: &Contract,
    blocked: &mut Vec<String>,
) {
    let mut checked: Vec<(String, String)> = Vec::new();
    for dimension in &contract.dimensions {
        require_column(
            backend,
            &dimension.table,
            &dimension.attribute,
            &format!("dimension '{}'", dimension.id),
            blocked,
            &mut checked,
        );
        for level in &dimension.levels {
            require_column(
                backend,
                &dimension.table,
                &level.column,
                &format!("dimension '{}' level '{}'", dimension.id, level.name),
                blocked,
                &mut checked,
            );
        }
    }
    if let Some(time_intelligence) = &contract.time_intelligence {
        let date = contract
            .dimensions
            .iter()
            .find(|dimension| dimension.id == time_intelligence.date_dimension);
        if let Some(date) = date {
            for key in date.key.columns() {
                require_column(
                    backend,
                    &date.table,
                    key,
                    &format!("time_intelligence date key of '{}'", date.id),
                    blocked,
                    &mut checked,
                );
            }
            for (flag, column) in time_intelligence.flags.present() {
                require_column(
                    backend,
                    &date.table,
                    column,
                    &format!("time_intelligence flag '{flag}'"),
                    blocked,
                    &mut checked,
                );
            }
        }
    }
}

fn require_column<B: QueryBackend + ?Sized>(
    backend: &B,
    table: &str,
    column: &str,
    owner: &str,
    blocked: &mut Vec<String>,
    checked: &mut Vec<(String, String)>,
) {
    if table.is_empty() || column.is_empty() {
        return;
    }
    let key = (table.to_string(), column.to_string());
    if checked.contains(&key) {
        return;
    }
    checked.push(key);
    let _ = backend.take_failure();
    let _ = backend.query_rows(&format!("SELECT \"{column}\" FROM \"{table}\" LIMIT 0"));
    if let Some(failure) = backend.take_failure() {
        blocked.push(format!(
            "{owner} column '{column}' is not readable on '{table}': {failure}"
        ));
    }
}

/// The number of key combinations that occur more than once.
fn duplicate_groups<B: QueryBackend + ?Sized>(
    backend: &B,
    table: &str,
    columns: &[&str],
) -> Result<f64, String> {
    let projection = columns
        .iter()
        .map(|column| format!("\"{column}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = backend.take_failure();
    let duplicates = backend.query_scalar(&format!(
        "SELECT COUNT(*) FROM (SELECT {projection} FROM \"{table}\" GROUP BY ALL HAVING COUNT(*) > 1)"
    ));
    match backend.take_failure() {
        Some(failure) => Err(failure),
        None => Ok(duplicates),
    }
}

/// The number of rows with a NULL in any key column.
fn null_keys<B: QueryBackend + ?Sized>(
    backend: &B,
    table: &str,
    columns: &[&str],
) -> Result<f64, String> {
    let predicate = columns
        .iter()
        .map(|column| format!("\"{column}\" IS NULL"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let _ = backend.take_failure();
    let nulls = backend.query_scalar(&format!(
        "SELECT COUNT(*) FROM \"{table}\" WHERE {predicate}"
    ));
    match backend.take_failure() {
        Some(failure) => Err(failure),
        None => Ok(nulls),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::BackendSource;
    use crate::project::config_io::{self, ConfigFormat};
    use std::path::{Path, PathBuf};

    const FIXTURE: &str = "contracts/upstream_marts/contract.yaml";

    const SYNTHETIC_CONTRACT: &str = r#"
contract_version: "0.1.0"
model: { name: demo }
grain:
  - { table: fact_orders, key: order_id, id: orders, measure_group: Orders }
  - { table: mart_median, key: [year, month, product_key], id: median, measure_group: Median }
dimensions:
  - { id: Date, table: dim_date, key: date_key, attribute: full_date, date_role: true,
      levels: [ { name: Year, column: year }, { name: Quarter, column: quarter },
                { name: Month, column: month }, { name: Full Date, column: full_date } ] }
  - { id: Product, table: dim_product, key: product_key, attribute: product_name }
relationships:
  - { fact: fact_orders, dimension: Date, columns: [order_date_key, date_key] }
  - { fact: fact_orders, dimension: Product, columns: [product_key, product_key] }
  - { fact: mart_median, dimension: Product, columns: [product_key, product_key] }
measures:
  - { id: Revenue, source: { table: fact_orders, column: revenue }, aggregation: sum }
  - { id: Median lead time, source: { table: mart_median, column: median_lead_time },
      aggregation: max, valid_grain: [year, month, product_key] }
time_intelligence:
  date_dimension: Date
  flags: { ytd: ytd_flag }
provenance: { source_system: manual, generator: test/0.1.0 }
"#;

    const SYNTHETIC_SQL: &str = "
        CREATE TABLE fact_orders(order_id INTEGER, order_date_key INTEGER, product_key INTEGER,
                                 revenue DOUBLE);
        INSERT INTO fact_orders VALUES (1, 1, 1, 10.0), (2, 1, 2, 20.0), (3, 2, 1, 30.0);
        CREATE TABLE dim_date(date_key INTEGER, full_date DATE, year INTEGER, quarter INTEGER,
                              month INTEGER, ytd_flag INTEGER);
        INSERT INTO dim_date VALUES (1, DATE '2024-01-01', 2024, 1, 1, 1),
                                    (2, DATE '2024-02-01', 2024, 1, 2, 1);
        CREATE TABLE dim_product(product_key INTEGER, product_name VARCHAR);
        INSERT INTO dim_product VALUES (1, 'a'), (2, 'b');
        CREATE TABLE mart_median(year INTEGER, month INTEGER, product_key INTEGER,
                                 median_lead_time DOUBLE);
        INSERT INTO mart_median VALUES (2024, 1, 1, 2.0), (2024, 1, 2, 3.0), (2024, 2, 1, 4.0);
    ";

    fn temp_path(name: &str, extension: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mallardcube-cq-{}-{name}.{extension}",
            std::process::id()
        ))
    }

    fn synthetic_db(name: &str) -> PathBuf {
        let path = temp_path(name, "duckdb");
        let _ = std::fs::remove_file(&path);
        let connection = duckdb::Connection::open(&path).expect("open temp db");
        connection
            .execute_batch(SYNTHETIC_SQL)
            .expect("seed temp db");
        drop(connection);
        path
    }

    fn upstream_db() -> PathBuf {
        let path = temp_path("upstream", "duckdb");
        let _ = std::fs::remove_file(&path);
        let connection = duckdb::Connection::open(&path).expect("open temp db");
        for file in ["schema.sql", "seed.sql", "marts.sql"] {
            let sql = std::fs::read_to_string(format!("projects/upstream_marts/upstream/{file}"))
                .unwrap_or_else(|error| panic!("read {file}: {error}"));
            connection
                .execute_batch(&sql)
                .unwrap_or_else(|error| panic!("seed {file}: {error}"));
        }
        drop(connection);
        path
    }

    fn parse_contract(text: &str, name: &str) -> Contract {
        let path = temp_path(name, "yaml");
        std::fs::write(&path, text).expect("write contract");
        let contract = crate::tools::contract::validate_file(path.to_str().expect("utf-8"))
            .expect("contract must validate");
        let _ = std::fs::remove_file(&path);
        contract
    }

    /// Project the contract into a config file beside the database, so
    /// `ProxyProject::load` and the qualifier can read it back.
    fn write_config(contract: &Contract, db: &Path, name: &str) -> PathBuf {
        let mut config = contract_project::project(
            contract,
            &Deployment {
                catalog: "DEMO".into(),
                cube: "Demo".into(),
                db_path: Some(db.to_string_lossy().into_owned()),
            },
        )
        .expect("the contract must project");
        config.normalize();
        let path = temp_path(name, "yaml");
        std::fs::write(
            &path,
            config_io::serialize(&config, ConfigFormat::Yaml).expect("serialize"),
        )
        .expect("write config");
        path
    }

    fn findings_for(
        contract: &Contract,
        db: &Path,
        config_path: &Path,
    ) -> (Vec<String>, Vec<String>) {
        let project =
            ProxyProject::load(config_path.to_str().expect("utf-8")).expect("load config");
        let source = BackendSource::file(db).expect("open db");
        contract_findings(source.checkout().as_ref(), contract, &project)
    }

    /// The checked-in fixture against the real upstream data: every declared
    /// grain is unique, every identity aggregate is valid at its grain, every
    /// expression binds and every named column exists.
    #[test]
    fn the_fixture_contract_qualifies_against_the_upstream_data() {
        let db = upstream_db();
        let contract = crate::tools::contract::validate_file(FIXTURE).expect("fixture");
        let config_path = write_config(&contract, &db, "upstream");
        let (blocked, partial) = findings_for(&contract, &db, &config_path);
        assert!(blocked.is_empty(), "{blocked:?}");
        assert!(partial.is_empty(), "{partial:?}");
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }

    /// A config that is not the projection of the contract is a blocker: the
    /// qualification would otherwise describe a different model than the one
    /// that is served.
    #[test]
    fn a_stale_config_is_blocked() {
        let db = synthetic_db("stale");
        let contract = parse_contract(SYNTHETIC_CONTRACT, "stale");
        let config_path = write_config(&contract, &db, "stale");

        let mut changed = parse_contract(SYNTHETIC_CONTRACT, "stale-changed");
        changed.measures[0].format = "0.00".into();
        let (blocked, _) = findings_for(&changed, &db, &config_path);
        assert!(
            blocked
                .iter()
                .any(|finding| finding.contains("differs from the served config")),
            "{blocked:?}"
        );

        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }

    /// A declared grain that is not unique in the data blocks: a pivot at that
    /// grain would multiply rows.
    #[test]
    fn a_non_unique_grain_is_blocked() {
        let db = synthetic_db("grain");
        let mut contract = parse_contract(SYNTHETIC_CONTRACT, "grain");
        contract.grain[0].key = crate::tools::contract::Key::Single("order_date_key".into());
        let config_path = write_config(&contract, &db, "grain");
        let (blocked, _) = findings_for(&contract, &db, &config_path);
        assert!(
            blocked.iter().any(|finding| finding.contains("not unique")),
            "{blocked:?}"
        );
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }

    /// A `valid_grain` the data does not satisfy blocks: the identity
    /// aggregate would be re-aggregated across grains.
    #[test]
    fn a_wrong_valid_grain_is_blocked() {
        let db = synthetic_db("valid-grain");
        let mut contract = parse_contract(SYNTHETIC_CONTRACT, "valid-grain");
        contract.measures[1].valid_grain = Some(vec!["year".into(), "month".into()]);
        let config_path = write_config(&contract, &db, "valid-grain");
        let (blocked, _) = findings_for(&contract, &db, &config_path);
        assert!(
            blocked
                .iter()
                .any(|finding| finding.contains("would re-aggregate")),
            "{blocked:?}"
        );
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }

    /// A typo'd expression or a missing column blocks at the binder, not at a
    /// plausible number.
    #[test]
    fn unbound_sql_is_blocked() {
        let db = synthetic_db("unbound");

        let mut typo = parse_contract(SYNTHETIC_CONTRACT, "typo");
        typo.measures[0].expression = Some("SUM(revnue)".into());
        let config_path = write_config(&typo, &db, "typo");
        let (blocked, _) = findings_for(&typo, &db, &config_path);
        assert!(
            blocked
                .iter()
                .any(|finding| finding.contains("does not bind")),
            "{blocked:?}"
        );
        let _ = std::fs::remove_file(&config_path);

        let mut missing = parse_contract(SYNTHETIC_CONTRACT, "missing");
        missing.dimensions[1].attribute = "nope".into();
        let config_path = write_config(&missing, &db, "missing");
        let (blocked, _) = findings_for(&missing, &db, &config_path);
        assert!(
            blocked
                .iter()
                .any(|finding| finding.contains("not readable")),
            "{blocked:?}"
        );

        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }

    /// An unreadable contract file blocks instead of skipping the checks.
    #[test]
    fn an_unreadable_contract_is_blocked() {
        let db = synthetic_db("unreadable");
        let contract = parse_contract(SYNTHETIC_CONTRACT, "unreadable");
        let config_path = write_config(&contract, &db, "unreadable");
        let verdict = crate::tools::qualify::qualify_with_contract(
            config_path.to_str().expect("utf-8"),
            None,
            Some("does-not-exist-contract.yaml"),
        );
        assert_eq!(verdict.label(), "BLOCKED");
        assert!(
            verdict
                .reasons()
                .iter()
                .any(|reason| reason.contains("cannot read")),
            "{:?}",
            verdict.reasons()
        );
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }
}
