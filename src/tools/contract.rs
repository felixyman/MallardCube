//! The source-neutral serving contract (plan 057-A).
//!
//! A contract describes the model's *meaning* — grain, keys, relationships and
//! measure declarations — with no SQL and no engine specifics. The runtime
//! never reads it: generators write it, `contract validate` checks it, and it
//! projects to a proxy config. `0.x` is unstable until 1.0, and a field earns
//! core status only with a qualifier check behind it.

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fmt;

/// The contract line this build understands.
pub const SUPPORTED_MAJOR: u32 = 0;
pub const SUPPORTED_MINOR: u32 = 1;
pub const SUPPORTED_VERSION: &str = "0.1";

/// The one place the CLI's default path lives.
pub const DEFAULT_CONTRACT_PATH: &str = "contracts/upstream_marts/contract.yaml";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub contract_version: String,
    pub model: ModelInfo,
    pub grain: Vec<Grain>,
    pub dimensions: Vec<Dimension>,
    pub relationships: Vec<Relationship>,
    pub measures: Vec<Measure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_intelligence: Option<TimeIntelligence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<Security>,
    pub provenance: Provenance,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Grain {
    pub table: String,
    pub key: Key,
    /// Serving id relationships and measures bind to; defaults to the table name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Measure-group caption in Excel; defaults to the table name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measure_group: Option<String>,
}

/// A single key column or a composite key.
#[derive(Debug, Deserialize, PartialEq, Serialize, Clone)]
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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Dimension {
    pub id: String,
    pub table: String,
    pub key: Key,
    /// The member value column Excel names members by; for a date role this is
    /// the leaf (key attribute).
    pub attribute: String,
    /// The user hierarchy's name; defaults to the caption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hierarchy_name: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub date_role: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub caption: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordinal: Option<u32>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cardinality_hint: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<Level>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Level {
    pub name: String,
    pub column: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cardinality_hint: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    pub fact: String,
    pub dimension: String,
    /// `[fact column, dimension column]`; the dimension column must be part of
    /// the dimension's key.
    pub columns: Vec<String>,
    #[serde(default, skip_serializing_if = "is_many_to_one")]
    pub cardinality: Cardinality,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub active: bool,
}

/// Only `many_to_one` exists: any other cardinality multiplies fact rows.
#[derive(Debug, Deserialize, Default, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cardinality {
    #[default]
    ManyToOne,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Measure {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub caption: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordinal: Option<u32>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub visible: bool,
    pub source: Source,
    pub aggregation: Aggregation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_window: Option<TimeWindow>,
    /// The grain a per-grain mart value is valid at; `Some([])` is a declared
    /// empty list and is refused, `None` means absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_grain: Option<Vec<String>>,
}

impl Measure {
    /// The declared valid grain, when present and non-empty.
    pub fn valid_grain(&self) -> &[String] {
        self.valid_grain.as_deref().unwrap_or_default()
    }
}

/// Declared, not inferred: a ratio is never emitted as if it were additive.
#[derive(Debug, Deserialize, PartialEq, Serialize, Clone)]
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

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub table: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    /// An upstream artifact reference (a model or mart id) instead of a column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TimeWindow {
    pub dimension: String,
    /// The upstream column the proxy only filters on (e.g. `ytd_flag`).
    pub flag: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TimeIntelligence {
    pub date_dimension: String,
    pub flags: TimeIntelligenceFlags,
}

/// The windows the engine serves, bound to upstream flag columns. Typed, so an
/// invented window is an unknown field rather than a silently ignored one.
#[derive(Debug, Deserialize, Default, Serialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TimeIntelligenceFlags {
    #[serde(default)]
    pub ytd: Option<String>,
    #[serde(default)]
    pub qtd: Option<String>,
    #[serde(default)]
    pub mtd: Option<String>,
    #[serde(default)]
    pub prior_year_ytd: Option<String>,
    #[serde(default)]
    pub current_year: Option<String>,
}

impl TimeIntelligenceFlags {
    /// Every declared flag as `(name, column)`.
    pub fn present(&self) -> Vec<(&'static str, &str)> {
        let mut flags = Vec::new();
        for (name, column) in [
            ("ytd", &self.ytd),
            ("qtd", &self.qtd),
            ("mtd", &self.mtd),
            ("prior_year_ytd", &self.prior_year_ytd),
            ("current_year", &self.current_year),
        ] {
            if let Some(column) = column {
                flags.push((name, column.as_str()));
            }
        }
        flags
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Security {
    /// Role references; the deployment's auth config binds them.
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub source_system: SourceSystem,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
    pub generator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
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

fn is_true(value: &bool) -> bool {
    *value
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_many_to_one(value: &Cardinality) -> bool {
    *value == Cardinality::ManyToOne
}

/// Reject duplicate mapping keys.
///
/// YAML's last-wins silently turns a template merge into a different contract,
/// and serde cannot see it: a second `aggregation:` is a *known* field that
/// overwrites the first. So the document is walked once as an event tree before
/// the typed parse, and a repeated key in any mapping is a hard error.
struct NoDuplicateKeys;

impl<'de> Deserialize<'de> for NoDuplicateKeys {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(NoDuplicateKeysVisitor)
    }
}

struct NoDuplicateKeysVisitor;

impl<'de> Visitor<'de> for NoDuplicateKeysVisitor {
    type Value = NoDuplicateKeys;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("any YAML value")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen: HashSet<yaml_serde::Value> = HashSet::new();
        while let Some(key) = map.next_key::<yaml_serde::Value>()? {
            if matches!(&key, yaml_serde::Value::String(name) if name == "<<") {
                return Err(de::Error::custom(
                    "YAML merge keys (`<<`) are not supported: the schema checker expands them and \
                     the validator does not, so the two gates would read different documents; write \
                     the fields explicitly",
                ));
            }
            if !seen.insert(key.clone()) {
                return Err(de::Error::custom(format!("duplicate mapping key {key:?}")));
            }
            map.next_value::<NoDuplicateKeys>()?;
        }
        Ok(NoDuplicateKeys)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while seq.next_element::<NoDuplicateKeys>()?.is_some() {}
        Ok(NoDuplicateKeys)
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }
}

/// Parse `major.minor.patch`; three numeric components, no leading zeros and no
/// pre-release, exactly what the schema's pattern admits.
fn version_parts(version: &str) -> Result<(u32, u32, u32), String> {
    let parts: Vec<&str> = version.split('.').collect();
    let is_number = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|byte| byte.is_ascii_digit())
            && (part.len() == 1 || !part.starts_with('0'))
    };
    if parts.len() != 3 || !parts.iter().all(|part| is_number(part)) {
        return Err(format!(
            "contract_version '{version}' is not semver (expected three numeric components, e.g. 0.1.0)"
        ));
    }
    let parse = |part: &str| {
        part.parse::<u32>().map_err(|_| {
            format!(
                "contract_version '{version}' has a component that does not fit a version number"
            )
        })
    };
    Ok((parse(parts[0])?, parse(parts[1])?, parse(parts[2])?))
}

/// The version rules (plan 057-A): an unknown/newer version refuses with an
/// upgrade hint, an older one routes through an explicit migration — never
/// silent tolerance.
pub fn check_version(version: &str) -> Result<(), String> {
    let (major, minor, _patch) = version_parts(version)?;
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
/// and declarations complete enough for the qualifier to check later. The JSON
/// Schema is the published contract; these checks are the enforcement, so they
/// must not be weaker than it.
fn consistency(contract: &Contract) -> Vec<String> {
    let mut findings = Vec::new();

    if contract.model.name.trim().is_empty() {
        findings.push("model.name is empty".to_string());
    }
    if contract.grain.is_empty() {
        findings.push("grain declares no fact tables".to_string());
    }
    if contract.dimensions.is_empty() {
        findings.push("dimensions declares no dimensions".to_string());
    }
    if contract.measures.is_empty() {
        findings.push("measures declares no measures".to_string());
    }

    let mut grain_keys: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for grain in &contract.grain {
        let columns = grain.key.columns();
        if columns.is_empty() || columns.iter().any(|column| column.trim().is_empty()) {
            findings.push(format!(
                "grain table '{}' declares an empty key",
                grain.table
            ));
        }
        if grain_keys.insert(grain.table.as_str(), columns).is_some() {
            findings.push(format!("duplicate grain table '{}'", grain.table));
        }
        if grain.id.as_deref().is_some_and(|id| id.trim().is_empty()) {
            findings.push(format!(
                "grain table '{}' declares an empty serving id",
                grain.table
            ));
        }
        if grain
            .measure_group
            .as_deref()
            .is_some_and(|group| group.trim().is_empty())
        {
            findings.push(format!(
                "grain table '{}' declares an empty measure group",
                grain.table
            ));
        }
    }
    let tables: HashSet<&str> = grain_keys.keys().copied().collect();

    let mut dimension_ids: HashSet<&str> = HashSet::new();
    let mut table_keys: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for dimension in &contract.dimensions {
        if dimension.id.trim().is_empty()
            || dimension.table.trim().is_empty()
            || dimension.attribute.trim().is_empty()
        {
            findings.push(format!(
                "dimension '{}' has an empty id, table or attribute",
                dimension.id
            ));
        }
        if !dimension_ids.insert(dimension.id.as_str()) {
            findings.push(format!("duplicate dimension id '{}'", dimension.id));
        }
        let key_columns = dimension.key.columns();
        if key_columns.is_empty() || key_columns.iter().any(|column| column.trim().is_empty()) {
            findings.push(format!(
                "dimension '{}' declares an empty key",
                dimension.id
            ));
        }
        if let Some(existing) = table_keys.get(dimension.table.as_str()) {
            if existing != &key_columns {
                findings.push(format!(
                    "dimension '{}' and another dimension on '{}' declare different keys \
                     ({key_columns:?} vs {existing:?}); one table has one key",
                    dimension.id, dimension.table
                ));
            }
        } else {
            table_keys.insert(dimension.table.as_str(), key_columns);
        }
        let mut level_columns: HashSet<&str> = HashSet::new();
        for level in &dimension.levels {
            if level.name.trim().is_empty() || level.column.trim().is_empty() {
                findings.push(format!(
                    "dimension '{}' has a level with an empty name or column",
                    dimension.id
                ));
            }
            if !level_columns.insert(level.column.as_str()) {
                findings.push(format!(
                    "dimension '{}' repeats level column '{}'",
                    dimension.id, level.column
                ));
            }
        }
        if dimension.date_role {
            if dimension.levels.is_empty() {
                findings.push(format!(
                    "date dimension '{}' declares no levels; a date role serves its calendar from them",
                    dimension.id
                ));
            } else if dimension.levels.last().map(|level| level.column.as_str())
                != Some(dimension.attribute.as_str())
            {
                findings.push(format!(
                    "date dimension '{}' must end its levels at its attribute '{}' (the key attribute \
                     Excel names members by)",
                    dimension.id, dimension.attribute
                ));
            }
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
        if relationship.columns.len() != 2
            || relationship
                .columns
                .iter()
                .any(|column| column.trim().is_empty())
        {
            findings.push(format!(
                "relationship on '{}' must name exactly [fact column, dimension column]",
                relationship.fact
            ));
        } else if let Some(dimension) = contract
            .dimensions
            .iter()
            .find(|dimension| dimension.id == relationship.dimension)
            .filter(|dimension| {
                !dimension
                    .key
                    .columns()
                    .contains(&relationship.columns[1].as_str())
            })
        {
            findings.push(format!(
                "relationship on '{}' joins to '{}', which is not part of dimension '{}' key {:?}",
                relationship.fact,
                relationship.columns[1],
                dimension.id,
                dimension.key.columns()
            ));
        }
    }

    let flag_columns: HashSet<&str> = contract
        .time_intelligence
        .as_ref()
        .map(|time_intelligence| {
            time_intelligence
                .flags
                .present()
                .into_iter()
                .map(|(_, column)| column)
                .collect()
        })
        .unwrap_or_default();

    let mut measure_ids: HashSet<&str> = HashSet::new();
    for measure in &contract.measures {
        if measure.id.trim().is_empty() {
            findings.push("a measure has an empty id".to_string());
        }
        if !measure_ids.insert(measure.id.as_str()) {
            findings.push(format!("duplicate measure id '{}'", measure.id));
        }
        if measure.source.table.trim().is_empty() || !tables.contains(measure.source.table.as_str())
        {
            findings.push(format!(
                "measure '{}' sources from '{}', which is not a declared grain table",
                measure.id, measure.source.table
            ));
        }
        if measure
            .expression
            .as_deref()
            .is_some_and(|expression| expression.trim().is_empty())
        {
            findings.push(format!(
                "measure '{}' declares an empty expression",
                measure.id
            ));
        }
        if measure
            .source
            .column
            .as_deref()
            .is_some_and(|column| column.trim().is_empty())
        {
            findings.push(format!("measure '{}' source column is empty", measure.id));
        }
        if measure
            .source
            .reference
            .as_deref()
            .is_some_and(|reference| reference.trim().is_empty())
        {
            findings.push(format!(
                "measure '{}' source reference is empty",
                measure.id
            ));
        }
        let declared_expression = measure
            .expression
            .as_deref()
            .is_some_and(|expression| !expression.trim().is_empty());
        let declared_source = declared_expression
            || measure
                .source
                .column
                .as_deref()
                .is_some_and(|column| !column.trim().is_empty())
            || measure
                .source
                .reference
                .as_deref()
                .is_some_and(|reference| !reference.trim().is_empty());
        match measure.aggregation {
            Aggregation::Ratio => {
                if !declared_expression {
                    findings.push(format!(
                        "measure '{}' is a ratio but declares no expression",
                        measure.id
                    ));
                }
            }
            Aggregation::Count => {}
            _ => {
                if !declared_source {
                    findings.push(format!(
                        "measure '{}' is a {} but declares neither an expression, a column nor a reference",
                        measure.id,
                        measure.aggregation.label()
                    ));
                }
            }
        }
        if measure.aggregation == Aggregation::TimeWindow && measure.time_window.is_none() {
            findings.push(format!(
                "measure '{}' declares the time_window aggregation without a time_window",
                measure.id
            ));
        }
        if let Some(valid_grain) = &measure.valid_grain {
            if valid_grain.is_empty() {
                findings.push(format!(
                    "measure '{}' declares an empty valid_grain",
                    measure.id
                ));
            } else {
                if !matches!(measure.aggregation, Aggregation::Min | Aggregation::Max) {
                    findings.push(format!(
                        "measure '{}' declares valid_grain but its aggregation is '{}'; a per-grain value \
                         is an identity aggregate (min or max)",
                        measure.id,
                        measure.aggregation.label()
                    ));
                }
                if let Some(grain_key) = grain_keys.get(measure.source.table.as_str()) {
                    for column in valid_grain {
                        if !grain_key.contains(&column.as_str()) {
                            findings.push(format!(
                                "measure '{}' valid_grain names '{}', which is not part of the grain of '{}' {:?}",
                                measure.id, column, measure.source.table, grain_key
                            ));
                        }
                    }
                }
            }
        }
        if let Some(window) = &measure.time_window {
            match &contract.time_intelligence {
                None => findings.push(format!(
                    "measure '{}' declares a time_window but the contract has no time_intelligence \
                     flag catalogue",
                    measure.id
                )),
                Some(time_intelligence) => {
                    if window.dimension != time_intelligence.date_dimension {
                        findings.push(format!(
                            "measure '{}' time_window names '{}' but the flag catalogue serves '{}'",
                            measure.id, window.dimension, time_intelligence.date_dimension
                        ));
                    }
                }
            }
            match contract
                .dimensions
                .iter()
                .find(|dimension| dimension.id == window.dimension)
            {
                Some(dimension) if !dimension.date_role => findings.push(format!(
                    "measure '{}' time_window names '{}', which is not a date role",
                    measure.id, window.dimension
                )),
                None => findings.push(format!(
                    "measure '{}' time_window names unknown dimension '{}'",
                    measure.id, window.dimension
                )),
                Some(_) => {}
            }
            if window.flag.trim().is_empty() {
                findings.push(format!(
                    "measure '{}' time_window declares an empty flag",
                    measure.id
                ));
            } else if !flag_columns.is_empty() && !flag_columns.contains(window.flag.as_str()) {
                findings.push(format!(
                    "measure '{}' time_window flag '{}' is not in time_intelligence flags",
                    measure.id, window.flag
                ));
            }
        }
    }

    if let Some(time_intelligence) = &contract.time_intelligence {
        match contract
            .dimensions
            .iter()
            .find(|dimension| dimension.id == time_intelligence.date_dimension)
        {
            Some(dimension) if !dimension.date_role => findings.push(format!(
                "time_intelligence date_dimension '{}' is not a date role",
                time_intelligence.date_dimension
            )),
            None => findings.push(format!(
                "time_intelligence names unknown dimension '{}'",
                time_intelligence.date_dimension
            )),
            Some(_) => {}
        }
        let flags = time_intelligence.flags.present();
        if flags.is_empty() {
            findings.push("time_intelligence declares no flags".to_string());
        }
        for (name, column) in flags {
            if column.trim().is_empty() {
                findings.push(format!(
                    "time_intelligence flag '{name}' has an empty column"
                ));
            }
        }
    }

    if let Some(security) = &contract.security {
        for role in &security.roles {
            if role.trim().is_empty() {
                findings.push("security.roles contains an empty role".to_string());
            }
        }
    }
    if contract.provenance.generator.trim().is_empty() {
        findings.push("provenance.generator is empty".to_string());
    }

    findings
}

/// Read, parse and check one contract file. `Err` carries every finding, so CI
/// can print them all rather than the first.
pub fn validate_file(path: &str) -> Result<Contract, Vec<String>> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| vec![format!("cannot read {path}: {error}")])?;
    validate_text(path, &text)
}

/// Validate contract text (the generator's output goes through the same gate
/// as a hand-written file). `origin` names the source in error messages.
pub(crate) fn validate_text(origin: &str, text: &str) -> Result<Contract, Vec<String>> {
    // Duplicate keys first: they change meaning before serde sees the fields.
    if let Err(error) = yaml_serde::from_str::<NoDuplicateKeys>(text) {
        return Err(vec![format!("{origin} is not a valid contract: {error}")]);
    }
    let contract: Contract = yaml_serde::from_str(text)
        .map_err(|error| vec![format!("{origin} is not a valid contract: {error}")])?;
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

/// The machine-readable verdict (house shape, like `qualify`): a stable key set
/// on every path, including the exit code.
fn verdict_json(
    verdict: &str,
    ok: bool,
    file: &str,
    version: Option<&str>,
    model: Option<&str>,
    reasons: &[String],
    exit_code: i32,
) -> String {
    serde_json::json!({
        "contract": "mallardcube.contract/1",
        "verdict": verdict,
        "ok": ok,
        "file": file,
        "version": version,
        "model": model,
        "reasons": reasons,
        "exit_code": exit_code,
    })
    .to_string()
}

/// Parsed `mallard contract` arguments. The house entry point is
/// `run(Vec<String>)`; `parse_args` keeps value flags and positionals apart so
/// `project` can take deployment parameters.
#[derive(Debug, Default)]
pub struct Args {
    pub action: String,
    pub file: Option<String>,
    pub json: bool,
    pub catalog: Option<String>,
    pub cube: Option<String>,
    pub db_path: Option<String>,
    pub out: Option<String>,
    pub from: Option<String>,
    pub overlay: Option<String>,
    pub check: bool,
}

/// Parse `["contract", <action>, <file>, --flag <value> ...]`.
pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args {
        action: "validate".to_string(),
        ..Args::default()
    };
    let mut positionals: Vec<&str> = Vec::new();
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--json" => parsed.json = true,
            "--check" => parsed.check = true,
            "--catalog" => parsed.catalog = Some(flag_value(&mut iter, "--catalog")?),
            "--cube" => parsed.cube = Some(flag_value(&mut iter, "--cube")?),
            "--db-path" => parsed.db_path = Some(flag_value(&mut iter, "--db-path")?),
            "--out" => parsed.out = Some(flag_value(&mut iter, "--out")?),
            "--from" => parsed.from = Some(flag_value(&mut iter, "--from")?),
            "--overlay" => parsed.overlay = Some(flag_value(&mut iter, "--overlay")?),
            other if other.starts_with("--") => {
                return Err(format!("unknown flag '{other}'"));
            }
            other => positionals.push(other),
        }
    }
    if let Some(action) = positionals.first() {
        parsed.action = (*action).to_string();
    }
    if let Some(file) = positionals.get(1) {
        parsed.file = Some((*file).to_string());
    }
    if let Some(extra) = positionals.get(2) {
        return Err(format!("unexpected argument '{extra}'"));
    }
    Ok(parsed)
}

fn flag_value<'a>(
    iter: &mut impl Iterator<Item = &'a String>,
    flag: &str,
) -> Result<String, String> {
    iter.next()
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value"))
}

/// `mallard contract validate <file> [--json]`.
fn run_validate(args: &Args) -> i32 {
    let json = args.json;
    let file = args.file.as_deref().unwrap_or(DEFAULT_CONTRACT_PATH);
    match validate_file(file) {
        Ok(contract) => {
            if json {
                println!(
                    "{}",
                    verdict_json(
                        "valid",
                        true,
                        file,
                        Some(&contract.contract_version),
                        Some(&contract.model.name),
                        &[],
                        0,
                    )
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
                    verdict_json("invalid", false, file, None, None, &reasons, 1)
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
    }
}

/// `mallard contract <action> ...`: `validate` (default) or `project`.
pub fn run(args: Vec<String>) -> i32 {
    let parsed = match parse_args(&args) {
        Ok(parsed) => parsed,
        Err(reason) => {
            eprintln!("contract: {reason}");
            return 2;
        }
    };
    match parsed.action.as_str() {
        "validate" => {
            if parsed.catalog.is_some()
                || parsed.cube.is_some()
                || parsed.db_path.is_some()
                || parsed.out.is_some()
            {
                let file = parsed.file.as_deref().unwrap_or(DEFAULT_CONTRACT_PATH);
                let reason = "--catalog/--cube/--db-path/--out need the project action".to_string();
                if parsed.json {
                    println!(
                        "{}",
                        verdict_json("error", false, file, None, None, &[reason], 2)
                    );
                } else {
                    eprintln!("contract: {reason}");
                }
                return 2;
            }
            if parsed.from.is_some() || parsed.overlay.is_some() || parsed.check {
                eprintln!("contract: --from/--overlay/--check need the generate action");
                return 2;
            }
            run_validate(&parsed)
        }
        "project" => {
            if parsed.from.is_some() || parsed.overlay.is_some() || parsed.check {
                eprintln!("contract: --from/--overlay/--check need the generate action");
                return 2;
            }
            crate::tools::contract_project::run(&parsed)
        }
        "generate" => crate::tools::contract_generate::run(&parsed),
        other => {
            let file = parsed.file.as_deref().unwrap_or(DEFAULT_CONTRACT_PATH);
            let reason =
                format!("unknown action '{other}' (expected: validate, project, generate)");
            if parsed.json {
                println!(
                    "{}",
                    verdict_json("error", false, file, None, None, &[reason], 2)
                );
            } else {
                eprintln!("contract: {reason}");
            }
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const FIXTURE: &str = "contracts/upstream_marts/contract.yaml";

    /// A minimal valid contract; tests mutate one line at a time.
    fn minimal() -> String {
        r#"
contract_version: "0.1.0"
model: { name: demo }
grain:
  - { table: fact_orders, key: order_id }
dimensions:
  - { id: Date, table: dim_date, key: date_key, attribute: full_date, date_role: true,
      levels: [ { name: Full Date, column: full_date } ] }
relationships:
  - { fact: fact_orders, dimension: Date, columns: [order_date_key, date_key] }
measures:
  - { id: Revenue, source: { table: fact_orders, column: revenue }, aggregation: sum }
provenance: { source_system: manual, generator: test/0.1.0 }
"#
        .to_string()
    }

    /// Write the text to a temp file and run the real pipeline, so a regression
    /// in `validate_file` fails these tests too.
    fn validate_text(text: &str) -> Result<Contract, Vec<String>> {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "mallard-contract-test-{}-{}.yaml",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::write(&path, text).expect("write temp contract");
        let result = validate_file(path.to_str().expect("utf-8 temp path"));
        let _ = std::fs::remove_file(&path);
        result
    }

    fn findings(text: &str) -> Vec<String> {
        validate_text(text).expect_err("the contract must be refused")
    }

    fn refused_for(text: &str, needle: &str) {
        let findings = findings(text);
        assert!(
            findings.iter().any(|finding| finding.contains(needle)),
            "expected a finding containing {needle:?}, got {findings:?}"
        );
    }

    /// The checked-in fixture is the contract the CI gate runs against.
    #[test]
    fn the_fixture_validates() {
        let contract = validate_file(FIXTURE).expect("the fixture must validate");
        assert_eq!(contract.contract_version, "0.1.0");
        assert_eq!(contract.model.name, "upstream_marts");
        assert_eq!(contract.grain.len(), 3);
        assert_eq!(contract.dimensions.len(), 6);
        assert_eq!(contract.relationships.len(), 10);
        assert_eq!(contract.measures.len(), 11);
        // The median declares its mart grain, so the qualifier can refuse a
        // re-aggregation instead of assuming additivity.
        let median = contract
            .measures
            .iter()
            .find(|measure| measure.id == "Median lead time")
            .expect("median measure");
        assert_eq!(median.aggregation, Aggregation::Max);
        let valid_grain = median.valid_grain();
        assert_eq!(valid_grain.len(), 3);
        assert_eq!(valid_grain[0], "year");
        assert_eq!(valid_grain[1], "month");
        assert_eq!(valid_grain[2], "product_key");
        // The median mart joins Category too; a projection without it would
        // lose the dimension from every median pivot.
        assert!(
            contract.relationships.iter().any(|relationship| {
                relationship.fact == "mart_lead_time_median_month"
                    && relationship.dimension == "Category"
            }),
            "the median mart must join Category"
        );
        // The date role's key attribute is the leaf, not the join column.
        let date = contract
            .dimensions
            .iter()
            .find(|dimension| dimension.id == "Date")
            .expect("date dimension");
        assert_eq!(date.key, Key::Single("date_key".to_string()));
        assert_eq!(date.attribute, "full_date");
        assert_eq!(date.hierarchy_name.as_deref(), Some("Calendar"));
        let time_intelligence = contract.time_intelligence.as_ref().expect("flag catalogue");
        assert_eq!(time_intelligence.date_dimension, "Date");
        assert_eq!(time_intelligence.flags.present().len(), 5);
    }

    /// An unknown core field is a hard error: a rule you think is enforced but
    /// is not is worse than no rule.
    #[test]
    fn an_unknown_core_field_is_refused() {
        let text = minimal().replace(
            "model: { name: demo }",
            "model: { name: demo, sql: \"SELECT 1\" }",
        );
        refused_for(&text, "unknown field");
    }

    /// YAML's last-wins would silently rewrite the contract: a template merge
    /// that appends a second `aggregation:` must not pick the last one.
    #[test]
    fn duplicate_mapping_keys_are_refused() {
        let text = r#"
contract_version: "0.1.0"
model: { name: demo }
grain:
  - { table: fact_orders, key: order_id }
dimensions:
  - { id: Date, table: dim_date, key: date_key, attribute: full_date, date_role: true,
      levels: [ { name: Full Date, column: full_date } ] }
relationships:
  - { fact: fact_orders, dimension: Date, columns: [order_date_key, date_key] }
measures:
  - id: Median lead time
    source: { table: fact_orders, column: median_lead_time }
    aggregation: max
    valid_grain: [order_id]
    aggregation: sum
provenance: { source_system: manual, generator: test/0.1.0 }
"#;
        refused_for(text, "duplicate mapping key");
    }

    /// A newer contract version refuses with an upgrade hint; an older one
    /// routes through an explicit migration; malformed versions never parse
    /// into a silently different meaning.
    #[test]
    fn version_rules_refuse_unknown_versions() {
        assert!(check_version("0.1.0").is_ok());
        assert!(check_version("0.1.7").is_ok());
        for malformed in [
            "0.1",
            "0.1.",
            "00.1.0",
            "0.01.0",
            "0.1.0-beta",
            "1",
            "x.y.z",
            "",
        ] {
            let reason = check_version(malformed).expect_err("must be refused as not semver");
            assert!(reason.contains("not semver"), "{malformed}: {reason}");
        }
        let newer = check_version("0.2.0").expect_err("0.2 is newer");
        assert!(newer.contains("newer than this build supports"), "{newer}");
        let older = check_version("0.0.9").expect_err("0.0 predates");
        assert!(older.contains("migrate"), "{older}");
        let foreign = check_version("1.0.0").expect_err("1.x is not this line");
        assert!(foreign.contains("0.x contract line"), "{foreign}");
        let huge = check_version("0.99999999999.0").expect_err("must not fit");
        assert!(huge.contains("does not fit"), "{huge}");
    }

    /// The schema's structural minima are enforced by the validator too, so the
    /// Rust gate is never the weaker one.
    #[test]
    fn schema_constraints_are_enforced() {
        refused_for(
            &minimal().replace(
                "grain:\n  - { table: fact_orders, key: order_id }",
                "grain: []",
            ),
            "grain declares no fact tables",
        );
        refused_for(&minimal().replace("key: order_id", "key: []"), "empty key");
        refused_for(
            &minimal().replace(
                "columns: [order_date_key, date_key]",
                "columns: [order_date_key]",
            ),
            "exactly [fact column, dimension column]",
        );
        refused_for(
            &minimal().replace("name: demo", "name: \"\""),
            "model.name is empty",
        );
        refused_for(
            &minimal().replace("key: order_id", "key: \"\""),
            "empty key",
        );
    }

    /// Only many_to_one exists: anything else multiplies rows.
    #[test]
    fn a_many_to_many_relationship_is_refused() {
        let text = minimal().replace(
            "columns: [order_date_key, date_key]",
            "columns: [order_date_key, date_key], cardinality: many_to_many",
        );
        refused_for(&text, "many_to_many");
    }

    /// Cross-field shape: dangling references must not reach the qualifier.
    #[test]
    fn dangling_references_are_refused() {
        refused_for(
            &minimal().replace("dimension: Date", "dimension: Nope"),
            "unknown dimension 'Nope'",
        );
        refused_for(
            &minimal().replace("fact: fact_orders", "fact: nope"),
            "not a declared grain table",
        );
        refused_for(
            &minimal().replace(
                "columns: [order_date_key, date_key]",
                "columns: [order_date_key, full_date]",
            ),
            "not part of dimension 'Date' key",
        );
        refused_for(
            &minimal().replace("aggregation: sum", "aggregation: max, valid_grain: [nope]"),
            "not part of the grain of 'fact_orders'",
        );
        refused_for(
            &minimal().replace(
                "aggregation: sum",
                "aggregation: sum, time_window: { dimension: Nope, flag: ytd_flag }",
            ),
            "time_window names unknown dimension 'Nope'",
        );
        refused_for(
            &minimal().replace(
                "aggregation: sum",
                "aggregation: sum, time_window: { dimension: Date, flag: \"\" }",
            ),
            "time_window declares an empty flag",
        );
    }

    /// Additivity invariants: a per-grain value is an identity aggregate, a
    /// ratio is an expression, and nothing aggregate-shaped is left empty.
    #[test]
    fn additivity_invariants_are_enforced() {
        refused_for(
            &minimal().replace(
                "aggregation: sum",
                "aggregation: sum, valid_grain: [order_id]",
            ),
            "identity aggregate (min or max)",
        );
        refused_for(
            &minimal().replace(
                "- { id: Revenue, source: { table: fact_orders, column: revenue }, aggregation: sum }",
                "- { id: Rate, source: { table: fact_orders }, aggregation: ratio }",
            ),
            "is a ratio but declares no expression",
        );
        refused_for(
            &minimal().replace(
                "- { id: Revenue, source: { table: fact_orders, column: revenue }, aggregation: sum }",
                "- { id: Empty, source: { table: fact_orders }, aggregation: sum }",
            ),
            "neither an expression, a column nor a reference",
        );
        refused_for(
            &minimal().replace(
                "- { id: Revenue, source: { table: fact_orders, column: revenue }, aggregation: sum }",
                "- { id: Windowed, source: { table: fact_orders, column: revenue }, aggregation: time_window }",
            ),
            "without a time_window",
        );
        // A plain row count needs no source column.
        let count = minimal().replace(
            "- { id: Revenue, source: { table: fact_orders, column: revenue }, aggregation: sum }",
            "- { id: Rows, source: { table: fact_orders }, aggregation: count }",
        );
        assert!(validate_text(&count).is_ok(), "count needs no column");
    }

    /// The date role serves its calendar: levels must exist and end at the key
    /// attribute Excel names members by.
    #[test]
    fn date_role_shape_is_enforced() {
        refused_for(
            &minimal().replace(
                "levels: [ { name: Full Date, column: full_date } ]",
                "levels: []",
            ),
            "declares no levels",
        );
        refused_for(
            &minimal().replace("attribute: full_date", "attribute: date_key"),
            "must end its levels at its attribute",
        );
    }

    /// One table has one key, and ids are unique.
    #[test]
    fn duplicate_ids_and_split_keys_are_refused() {
        refused_for(
            &minimal().replace(
                "relationships:",
                "  - { id: Other, table: dim_date, key: full_date, attribute: full_date }\nrelationships:",
            ),
            "one table has one key",
        );
        refused_for(
            &minimal().replace(
                "relationships:",
                "  - { id: Date, table: dim_date, key: date_key, attribute: full_date }\nrelationships:",
            ),
            "duplicate dimension id 'Date'",
        );
        refused_for(
            &minimal().replace(
                "provenance:",
                "  - { id: Revenue, source: { table: fact_orders, column: revenue }, aggregation: sum }\nprovenance:",
            ),
            "duplicate measure id 'Revenue'",
        );
    }

    /// The flag catalogue binds windows to real columns; a measure must not
    /// reference a flag the catalogue does not serve.
    #[test]
    fn time_intelligence_is_checked() {
        refused_for(
            &minimal().replace(
                "provenance:",
                "time_intelligence: { date_dimension: Nope, flags: { ytd: ytd_flag } }\nprovenance:",
            ),
            "time_intelligence names unknown dimension 'Nope'",
        );
        refused_for(
            &minimal()
                .replace("date_role: true", "date_role: false")
                .replace(
                    "provenance:",
                    "time_intelligence: { date_dimension: Date, flags: { ytd: ytd_flag } }\nprovenance:",
                ),
            "is not a date role",
        );
        refused_for(
            &minimal().replace(
                "provenance:",
                "time_intelligence: { date_dimension: Date, flags: {} }\nprovenance:",
            ),
            "declares no flags",
        );
        refused_for(
            &minimal()
                .replace(
                    "aggregation: sum",
                    "aggregation: sum, time_window: { dimension: Date, flag: qtd_flag }",
                )
                .replace(
                    "provenance:",
                    "time_intelligence: { date_dimension: Date, flags: { ytd: ytd_flag } }\nprovenance:",
                ),
            "is not in time_intelligence flags",
        );
        // The matching flag validates.
        let bound = minimal()
            .replace(
                "aggregation: sum",
                "aggregation: sum, time_window: { dimension: Date, flag: ytd_flag }",
            )
            .replace(
                "provenance:",
                "time_intelligence: { date_dimension: Date, flags: { ytd: ytd_flag } }\nprovenance:",
            );
        assert!(
            validate_text(&bound).is_ok(),
            "the bound flag must validate"
        );
    }

    /// Every declared-but-empty field the schema refuses is refused here too:
    /// the validator is never the weaker gate.
    #[test]
    fn empty_declarations_are_refused() {
        refused_for(
            &minimal().replace("aggregation: sum", "aggregation: sum, expression: \"\""),
            "empty expression",
        );
        refused_for(
            &minimal().replace("column: revenue", "column: \"\""),
            "source column is empty",
        );
        refused_for(
            &minimal().replace("column: revenue", "reference: \"\""),
            "source reference is empty",
        );
        refused_for(
            &minimal().replace("aggregation: sum", "aggregation: sum, valid_grain: []"),
            "empty valid_grain",
        );
        refused_for(
            &minimal().replace("key: order_id", "key: order_id, id: \"\""),
            "empty serving id",
        );
        refused_for(
            &minimal().replace("key: order_id", "key: order_id, measure_group: \"\""),
            "empty measure group",
        );
    }

    /// A window measure must bind to the declared flag catalogue, and to the
    /// catalogue's date role — not to an arbitrary flag the model never serves.
    #[test]
    fn window_measures_require_the_flag_catalogue() {
        let windowed = minimal().replace(
            "aggregation: sum",
            "aggregation: sum, time_window: { dimension: Date, flag: ytd_flag }",
        );
        refused_for(&windowed, "no time_intelligence flag catalogue");
        let mismatch = minimal()
            .replace(
                "relationships:",
                "  - { id: Other, table: dim_date, key: date_key, attribute: full_date, \
                 date_role: true, levels: [ { name: Full Date, column: full_date } ] }\nrelationships:",
            )
            .replace(
                "aggregation: sum",
                "aggregation: sum, time_window: { dimension: Other, flag: ytd_flag }",
            )
            .replace(
                "provenance:",
                "time_intelligence: { date_dimension: Date, flags: { ytd: ytd_flag } }\nprovenance:",
            );
        refused_for(&mismatch, "flag catalogue serves 'Date'");
    }

    /// YAML merge keys would make the two gates read different documents (the
    /// schema checker expands them, the validator does not), so they refuse.
    #[test]
    fn yaml_merge_keys_are_refused() {
        let text = r#"
contract_version: "0.1.0"
model: { name: demo }
grain:
  - { table: fact_orders, key: order_id }
dimensions:
  - { id: Date, table: dim_date, key: date_key, attribute: full_date, date_role: true,
      levels: [ { name: Full Date, column: full_date } ] }
relationships:
  - { fact: fact_orders, dimension: Date, columns: [order_date_key, date_key] }
measures:
  - &base { id: Revenue, source: { table: fact_orders, column: revenue }, aggregation: sum }
  - <<: *base
    id: Revenue copy
provenance: { source_system: manual, generator: test/0.1.0 }
"#;
        refused_for(text, "merge keys");
    }

    /// The CLI's exit codes and default path (one source of truth).
    #[test]
    fn run_exit_codes() {
        assert_eq!(
            run(vec!["contract".into()]),
            0,
            "the default fixture must validate"
        );
        assert_eq!(
            run(vec![
                "contract".into(),
                "validate".into(),
                "does-not-exist.yaml".into()
            ]),
            1
        );
        assert_eq!(run(vec!["contract".into(), "bogus".into()]), 2);
        assert_eq!(run(vec!["contract".into(), "--bogus".into()]), 2);
        assert_eq!(
            run(vec!["contract".into(), "validate".into(), FIXTURE.into()]),
            0
        );
        assert_eq!(
            run(vec![
                "contract".into(),
                "validate".into(),
                FIXTURE.into(),
                "--out".into(),
                "x.yaml".into()
            ]),
            2,
            "project-only flags must not be silently ignored by validate"
        );
        assert_eq!(
            run(vec![
                "contract".into(),
                "validate".into(),
                "does-not-exist.yaml".into(),
                "--json".into()
            ]),
            1
        );
        assert_eq!(
            run(vec!["contract".into(), "bogus".into(), "--json".into()]),
            2
        );
    }
}
