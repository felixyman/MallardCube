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
///
/// Test-only convenience: the qualifier calls the two groups separately, so
/// the projection check can run without a database.
#[cfg(test)]
pub(crate) fn contract_findings<B: QueryBackend + ?Sized>(
    backend: &B,
    contract: &Contract,
    project: &ProxyProject,
) -> (Vec<String>, Vec<String>) {
    let (mut blocked, mut partial) = correspondence_findings(contract, project);
    let (data_blocked, data_partial) = contract_data_findings(backend, contract, project);
    blocked.extend(data_blocked);
    partial.extend(data_partial);
    (blocked, partial)
}

/// The served config must be the projection of this contract. This needs no
/// database, so the qualifier runs it even when the data is unreachable: a
/// stale config is a gate failure, not a skipped check.
pub(crate) fn correspondence_findings(
    contract: &Contract,
    project: &ProxyProject,
) -> (Vec<String>, Vec<String>) {
    let mut blocked = Vec::new();
    let mut partial = Vec::new();
    correspondence(contract, project, &mut blocked, &mut partial);
    (blocked, partial)
}

/// The data-side contract checks: grain keys, identity aggregates, declared
/// SQL and named columns.
pub(crate) fn contract_data_findings<B: QueryBackend + ?Sized>(
    backend: &B,
    contract: &Contract,
    _project: &ProxyProject,
) -> (Vec<String>, Vec<String>) {
    let mut blocked = Vec::new();
    let mut partial = Vec::new();
    grain_uniqueness(backend, contract, &mut blocked, &mut partial);
    valid_grain_uniqueness(backend, contract, &mut blocked);
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
        INEXPRESSIBLE_FACT,
        blocked,
        partial,
    );
    compare(
        "dimension",
        &projected.dimensions,
        &served.dimensions,
        |dimension| dimension.id.clone(),
        INEXPRESSIBLE_DIMENSION,
        blocked,
        partial,
    );
    compare(
        "measure",
        &projected.measures,
        &served.measures,
        |measure| measure.id.clone(),
        INEXPRESSIBLE_MEASURE,
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
        &[],
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

/// Config fields the contract cannot express: presentation and serving hints
/// the deployment owns. They are dropped from the comparison, so setting them
/// is a legitimate serving edit rather than a stale projection; everything the
/// contract *does* declare is compared exactly.
const INEXPRESSIBLE_FACT: &[&str] = &["source_name"];
const INEXPRESSIBLE_DIMENSION: &[&str] = &[
    "description",
    "all_level_name",
    "leaf_level_name",
    "has_all",
    "parent_child",
    "fact_table",
    "shared",
];
const INEXPRESSIBLE_MEASURE: &[&str] = &[
    "display_name",
    "units",
    "numeric_precision",
    "numeric_scale",
    // Pinned to Sum until the reference oracle measures the aggregator codes
    // (plan 057 leftover); a deployment may set it without diverging.
    "aggregator",
];

/// Compare one config section entry by entry: a difference or a missing entry
/// blocks, an extra entry in the served config is a partial finding (the
/// contract does not describe everything that is served).
fn compare<T: serde::Serialize>(
    label: &str,
    expected: &[T],
    served: &[T],
    id_of: impl Fn(&T) -> String,
    ignored: &[&str],
    blocked: &mut Vec<String>,
    partial: &mut Vec<String>,
) {
    for expected_entry in expected {
        let id = id_of(expected_entry);
        match served.iter().find(|entry| id_of(entry) == id) {
            Some(served_entry) => {
                if comparable(expected_entry, ignored) != comparable(served_entry, ignored) {
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

/// One entry as a comparable value: serialized, with the fields the contract
/// cannot express removed. Serialization cannot fail for these types, so a
/// failure would be a bug, not a finding.
fn comparable<T: serde::Serialize>(entry: &T, ignored: &[&str]) -> serde_json::Value {
    let mut value = serde_json::to_value(entry).expect("config sections serialize");
    if let Some(object) = value.as_object_mut() {
        for field in ignored {
            object.remove(*field);
        }
    }
    value
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
        let rows = match row_count(backend, &grain.table) {
            Ok(rows) => rows,
            Err(failure) => {
                blocked.push(format!(
                    "grain '{}' row count cannot run: {failure}",
                    grain.table
                ));
                continue;
            }
        };
        if rows == 0.0 {
            partial.push(format!(
                "declared grain table '{}' is empty; nothing to qualify",
                grain.table
            ));
        }
        // NULLs group together, so a *lone* NULL key row is the case this
        // partial catches; two or more already trip the duplicate block above.
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
///
/// NULL keys need no separate check here: the validator requires every
/// `valid_grain` column to be part of the table's declared grain, and
/// `grain_uniqueness` already reports NULLs on those columns.
fn valid_grain_uniqueness<B: QueryBackend + ?Sized>(
    backend: &B,
    contract: &Contract,
    blocked: &mut Vec<String>,
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
        if let Some(finding) = quoted_identifier(&measure.source.table) {
            blocked.push(format!("measure '{}': {finding}", measure.id));
            continue;
        }
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
    if let Some(finding) = quoted_identifier(table).or_else(|| quoted_identifier(column)) {
        blocked.push(format!("{owner}: {finding}"));
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

/// Contract identifiers are spliced into SQL; a quote would break the
/// statement and blame the data. Refuse it with the real reason, like
/// `data_findings` does for config identifiers.
fn quoted_identifier(value: &str) -> Option<String> {
    if value.contains('"') || value.contains('\'') {
        Some(format!(
            "identifier '{value}' contains a quote; fix the contract"
        ))
    } else {
        None
    }
}

/// The number of key combinations that occur more than once.
fn duplicate_groups<B: QueryBackend + ?Sized>(
    backend: &B,
    table: &str,
    columns: &[&str],
) -> Result<f64, String> {
    if let Some(finding) = quoted_identifier(table) {
        return Err(finding);
    }
    for column in columns {
        if let Some(finding) = quoted_identifier(column) {
            return Err(finding);
        }
    }
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

/// The number of rows in a table.
fn row_count<B: QueryBackend + ?Sized>(backend: &B, table: &str) -> Result<f64, String> {
    if let Some(finding) = quoted_identifier(table) {
        return Err(finding);
    }
    let _ = backend.take_failure();
    let rows = backend.query_scalar(&format!("SELECT COUNT(*) FROM \"{table}\""));
    match backend.take_failure() {
        Some(failure) => Err(failure),
        None => Ok(rows),
    }
}

/// The number of rows with a NULL in any key column.
fn null_keys<B: QueryBackend + ?Sized>(
    backend: &B,
    table: &str,
    columns: &[&str],
) -> Result<f64, String> {
    if let Some(finding) = quoted_identifier(table) {
        return Err(finding);
    }
    for column in columns {
        if let Some(finding) = quoted_identifier(column) {
            return Err(finding);
        }
    }
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
    use crate::project::config::ProxyConfig;
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
        let path = write_contract(text, name);
        let contract = crate::tools::contract::validate_file(path.to_str().expect("utf-8"))
            .expect("contract must validate");
        let _ = std::fs::remove_file(&path);
        contract
    }

    /// Write the contract and keep the file (for `qualify_with_contract`).
    fn write_contract(text: &str, name: &str) -> PathBuf {
        let path = temp_path(name, "yaml");
        std::fs::write(&path, text).expect("write contract");
        path
    }

    /// The projected config for a contract, normalized and not yet written.
    fn projected_config(contract: &Contract, db: &Path) -> ProxyConfig {
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
        config
    }

    fn write_projected(config: ProxyConfig, name: &str) -> PathBuf {
        let path = temp_path(name, "yaml");
        std::fs::write(
            &path,
            config_io::serialize(&config, ConfigFormat::Yaml).expect("serialize"),
        )
        .expect("write config");
        path
    }

    /// Project the contract into a config file beside the database, so
    /// `ProxyProject::load` and the qualifier can read it back.
    fn write_config(contract: &Contract, db: &Path, name: &str) -> PathBuf {
        write_projected(projected_config(contract, db), name)
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

    /// The projection check needs no database: a stale config blocks even when
    /// the data is unreachable. The gate must not fail open on a fresh
    /// checkout where the database has not been built yet.
    #[test]
    fn a_missing_database_does_not_skip_the_projection_check() {
        let contract = parse_contract(SYNTHETIC_CONTRACT, "no-db");
        let missing_db = temp_path("no-db-missing", "duckdb"); // never created
        let mut served = projected_config(&contract, &missing_db);
        served.measures[0].format_string = "0.00".into();
        let config_path = write_projected(served, "no-db");
        let contract_path = write_contract(SYNTHETIC_CONTRACT, "no-db-contract");

        let verdict = crate::tools::qualify::qualify_with_contract(
            config_path.to_str().expect("utf-8"),
            None,
            Some(contract_path.to_str().expect("utf-8")),
        );
        assert_eq!(verdict.label(), "BLOCKED", "{:?}", verdict.reasons());
        assert!(
            verdict
                .reasons()
                .iter()
                .any(|reason| reason.contains("differs from the served config")),
            "{:?}",
            verdict.reasons()
        );
        let _ = std::fs::remove_file(&config_path);
        let _ = std::fs::remove_file(&contract_path);
    }

    /// An invalid contract (not just an unreadable one) blocks.
    #[test]
    fn an_invalid_contract_is_blocked() {
        let db = synthetic_db("invalid");
        let contract = parse_contract(SYNTHETIC_CONTRACT, "invalid");
        let config_path = write_config(&contract, &db, "invalid");
        let invalid_path = write_contract(
            &SYNTHETIC_CONTRACT.replace(
                "model: { name: demo }",
                "model: { name: demo, sql: \"SELECT 1\" }",
            ),
            "invalid-file",
        );
        let verdict = crate::tools::qualify::qualify_with_contract(
            config_path.to_str().expect("utf-8"),
            None,
            Some(invalid_path.to_str().expect("utf-8")),
        );
        assert_eq!(verdict.label(), "BLOCKED");
        assert!(
            verdict
                .reasons()
                .iter()
                .any(|reason| reason.contains("unknown field")),
            "{:?}",
            verdict.reasons()
        );
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
        let _ = std::fs::remove_file(&invalid_path);
    }

    /// Presentation fields the contract cannot express (display names, units,
    /// precision, dimension descriptions) are the deployment's; setting them is
    /// a legitimate serving edit, not a stale projection.
    #[test]
    fn inexpressible_presentation_fields_are_tolerated() {
        let db = synthetic_db("presentation");
        let contract = parse_contract(SYNTHETIC_CONTRACT, "presentation");
        let mut served = projected_config(&contract, &db);
        served.measures[0].display_name = "Net revenue".into();
        served.measures[0].units = "USD".into();
        served.measures[0].numeric_scale = 4;
        served.dimensions[0].description = "the calendar".into();
        served.dimensions[0].all_level_name = "All dates".into();
        let config_path = write_projected(served, "presentation");

        let (blocked, partial) = findings_for(&contract, &db, &config_path);
        assert!(blocked.is_empty(), "{blocked:?}");
        assert!(partial.is_empty(), "{partial:?}");
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }

    /// An empty declared grain table qualifies nothing: it is a partial
    /// finding, not a silent pass.
    #[test]
    fn empty_grain_tables_are_partial() {
        let db = synthetic_db("empty");
        {
            let connection = duckdb::Connection::open(&db).expect("open temp db");
            connection
                .execute_batch("CREATE TABLE empty_mart(a INTEGER, b INTEGER);")
                .expect("create empty table");
        }
        let mut contract = parse_contract(SYNTHETIC_CONTRACT, "empty");
        contract.grain.push(crate::tools::contract::Grain {
            table: "empty_mart".into(),
            key: crate::tools::contract::Key::Composite(vec!["a".into(), "b".into()]),
            id: None,
            measure_group: None,
        });
        let config_path = write_config(&contract, &db, "empty");
        let (blocked, partial) = findings_for(&contract, &db, &config_path);
        assert!(blocked.is_empty(), "{blocked:?}");
        assert!(
            partial.iter().any(|finding| finding.contains("is empty")),
            "{partial:?}"
        );
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }

    /// A lone NULL in a composite grain key is a partial finding (several
    /// NULLs would register as duplicates instead).
    #[test]
    fn lone_null_grain_keys_are_partial() {
        let db = synthetic_db("nulls");
        {
            let connection = duckdb::Connection::open(&db).expect("open temp db");
            connection
                .execute_batch(
                    "CREATE TABLE nullable(a INTEGER, b INTEGER);
                     INSERT INTO nullable VALUES (1, 1), (2, NULL);",
                )
                .expect("seed nullable table");
        }
        let mut contract = parse_contract(SYNTHETIC_CONTRACT, "nulls");
        contract.grain.push(crate::tools::contract::Grain {
            table: "nullable".into(),
            key: crate::tools::contract::Key::Composite(vec!["a".into(), "b".into()]),
            id: None,
            measure_group: None,
        });
        let config_path = write_config(&contract, &db, "nulls");
        let (blocked, partial) = findings_for(&contract, &db, &config_path);
        assert!(blocked.is_empty(), "{blocked:?}");
        assert!(
            partial
                .iter()
                .any(|finding| finding.contains("NULL key column")),
            "{partial:?}"
        );
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }

    /// A served entry the contract does not declare is a partial finding: the
    /// contract is incomplete, not wrong.
    #[test]
    fn extra_served_entries_are_partial() {
        let db = synthetic_db("extra");
        let contract = parse_contract(SYNTHETIC_CONTRACT, "extra");
        let mut served = projected_config(&contract, &db);
        let mut extra = served.measures[0].clone();
        extra.id = "Extra".into();
        extra.caption = "Extra".into();
        served.measures.push(extra);
        let config_path = write_projected(served, "extra");

        let (blocked, partial) = findings_for(&contract, &db, &config_path);
        assert!(blocked.is_empty(), "{blocked:?}");
        assert!(
            partial.iter().any(|finding| finding.contains("'Extra'")),
            "{partial:?}"
        );
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }

    /// A role the contract declares must exist in the served config.
    #[test]
    fn declared_roles_must_be_served() {
        let db = synthetic_db("roles");
        let mut contract = parse_contract(SYNTHETIC_CONTRACT, "roles");
        contract.security = Some(crate::tools::contract::Security {
            roles: vec!["Sales".into()],
        });
        let mut served = projected_config(&contract, &db);
        served.roles.clear();
        let config_path = write_projected(served, "roles");

        let (blocked, _) = findings_for(&contract, &db, &config_path);
        assert!(
            blocked
                .iter()
                .any(|finding| finding.contains("security role 'Sales'")),
            "{blocked:?}"
        );
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }

    /// Quoted identifiers are refused with the real reason instead of a
    /// broken statement that blames the data.
    #[test]
    fn quoted_identifiers_are_refused() {
        let db = synthetic_db("quotes");
        let mut quoted_table = parse_contract(SYNTHETIC_CONTRACT, "quotes-table");
        quoted_table.dimensions[1].table = "o'brien".into();
        let config_path = write_config(&quoted_table, &db, "quotes-table");
        let (blocked, _) = findings_for(&quoted_table, &db, &config_path);
        assert!(
            blocked
                .iter()
                .any(|finding| finding.contains("contains a quote")),
            "{blocked:?}"
        );
        let _ = std::fs::remove_file(&config_path);

        let mut quoted_column = parse_contract(SYNTHETIC_CONTRACT, "quotes-column");
        quoted_column.dimensions[0].levels[0].column = "ye\"ar".into();
        let config_path = write_config(&quoted_column, &db, "quotes-column");
        let (blocked, _) = findings_for(&quoted_column, &db, &config_path);
        assert!(
            blocked
                .iter()
                .any(|finding| finding.contains("contains a quote")),
            "{blocked:?}"
        );
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }

    /// Level columns and flag columns are checked, not just the attribute.
    #[test]
    fn level_and_flag_columns_are_checked() {
        let db = synthetic_db("level-column");
        let mut contract = parse_contract(SYNTHETIC_CONTRACT, "level-column");
        contract.dimensions[0].levels[2].column = "nope".into();
        let config_path = write_config(&contract, &db, "level-column");
        let (blocked, _) = findings_for(&contract, &db, &config_path);
        assert!(
            blocked
                .iter()
                .any(|finding| finding.contains("level 'Month'")),
            "{blocked:?}"
        );
        let _ = std::fs::remove_file(&config_path);

        let mut flag = parse_contract(SYNTHETIC_CONTRACT, "flag-column");
        flag.time_intelligence
            .as_mut()
            .expect("catalogue")
            .flags
            .ytd = Some("nope".into());
        let config_path = write_config(&flag, &db, "flag-column");
        let (blocked, _) = findings_for(&flag, &db, &config_path);
        assert!(
            blocked.iter().any(|finding| finding.contains("flag 'ytd'")),
            "{blocked:?}"
        );
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }

    /// The served flag catalogue must match the contract's.
    #[test]
    fn the_time_intelligence_catalogue_must_match() {
        let db = synthetic_db("ti-mismatch");
        let contract = parse_contract(SYNTHETIC_CONTRACT, "ti-mismatch");
        let mut served = projected_config(&contract, &db);
        served
            .time_intelligence
            .as_mut()
            .expect("catalogue")
            .date_dimension
            .flag_columns
            .ytd_flag_column = "other_flag".into();
        let config_path = write_projected(served, "ti-mismatch");

        let (blocked, _) = findings_for(&contract, &db, &config_path);
        assert!(
            blocked
                .iter()
                .any(|finding| finding.contains("time_intelligence block differs")),
            "{blocked:?}"
        );
        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_file(&config_path);
    }
}
