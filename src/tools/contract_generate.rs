//! Contract generation from upstream metadata (plan 057-A, section D).
//!
//! The first source is SQLMesh. Generation parses the checked-in project files
//! — no Python, no SQLMesh at generation time — maps what the metadata
//! mechanically carries (grain, relationships from the custom `relationships`
//! audit, tables, columns, metrics) and merges a hand-written overlay for what
//! it cannot: dimension ids, attributes, levels, date roles, display and time
//! intelligence. Anything ambiguous is refused, never guessed; the output is
//! validated before it is written.

use crate::tools::contract::{self, Aggregation, Contract};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// `mallard contract generate --from sqlmesh <project> [--overlay <file>]
/// [--out <file>] [--check]`.
/// The generator's machine-readable verdict (house shape).
fn verdict_json(
    verdict: &str,
    ok: bool,
    project: &str,
    overlay: &str,
    out: Option<&str>,
    reasons: &[String],
    exit_code: i32,
) -> String {
    serde_json::json!({
        "contract": "mallardcube.contract/1",
        "verdict": verdict,
        "ok": ok,
        "project": project,
        "overlay": overlay,
        "out": out,
        "reasons": reasons,
        "exit_code": exit_code,
    })
    .to_string()
}

pub(crate) fn run(args: &contract::Args) -> i32 {
    let from = args.from.as_deref().unwrap_or("sqlmesh");
    if from != "sqlmesh" {
        eprintln!("contract: unknown generator '{from}' (expected: sqlmesh)");
        return 2;
    }
    let project = match args.file.as_deref() {
        Some(project) => project,
        None => {
            eprintln!("contract: generate needs a project directory");
            return 2;
        }
    };
    if args.check && args.out.is_none() {
        eprintln!("contract: generate --check needs --out <path> (the checked-in contract)");
        return 2;
    }
    let overlay = args.overlay.clone().unwrap_or_else(|| {
        Path::new(project)
            .join("contract.overlay.yaml")
            .display()
            .to_string()
    });
    let generated = match generate(Path::new(project), Path::new(&overlay)) {
        Ok(generated) => generated,
        Err(findings) => {
            if args.json {
                println!(
                    "{}",
                    verdict_json(
                        "refused",
                        false,
                        project,
                        &overlay,
                        args.out.as_deref(),
                        &findings,
                        1
                    )
                );
            } else {
                for finding in &findings {
                    println!("  [FAIL] {finding}");
                }
            }
            return 1;
        }
    };

    match args.out.as_deref() {
        Some(out) if args.check => match std::fs::read_to_string(out) {
            Ok(checked_in) if checked_in == generated => {
                if args.json {
                    println!(
                        "{}",
                        verdict_json("matches", true, project, &overlay, Some(out), &[], 0)
                    );
                } else {
                    println!("Generated contract matches {out}");
                }
                0
            }
            Ok(_) => {
                if args.json {
                    println!(
                        "{}",
                        verdict_json("differs", false, project, &overlay, Some(out), &[], 1)
                    );
                } else {
                    println!(
                        "Generated contract differs from {out}; regenerate and review the diff"
                    );
                }
                1
            }
            Err(error) => {
                let reason = format!("cannot read {out}: {error}");
                if args.json {
                    println!(
                        "{}",
                        verdict_json("error", false, project, &overlay, Some(out), &[reason], 2)
                    );
                } else {
                    eprintln!("contract: {reason}");
                }
                2
            }
        },
        Some(out) => {
            if let Err(error) = std::fs::write(out, &generated) {
                let reason = format!("cannot write {out}: {error}");
                if args.json {
                    println!(
                        "{}",
                        verdict_json("error", false, project, &overlay, Some(out), &[reason], 1)
                    );
                } else {
                    eprintln!("contract: {reason}");
                }
                return 1;
            }
            if args.json {
                println!(
                    "{}",
                    verdict_json("generated", true, project, &overlay, Some(out), &[], 0)
                );
            } else {
                println!("Generated {project} -> {out}");
            }
            0
        }
        None => {
            if args.json {
                // Validate-only: the verdict, not the contract text.
                println!(
                    "{}",
                    verdict_json("generated", true, project, &overlay, None, &[], 0)
                );
            } else {
                print!("{generated}");
            }
            0
        }
    }
}

/// Generate, validate, and return the contract text (deterministic).
pub(crate) fn generate(project_dir: &Path, overlay_path: &Path) -> Result<String, Vec<String>> {
    let overlay_text = std::fs::read_to_string(overlay_path)
        .map_err(|error| vec![format!("cannot read {}: {error}", overlay_path.display())])?;
    let overlay: Overlay = yaml_serde::from_str(&overlay_text).map_err(|error| {
        vec![format!(
            "{} is not a valid overlay: {error}",
            overlay_path.display()
        )]
    })?;

    let models = load_models(project_dir)?;
    let metrics = load_metrics(project_dir)?;
    let inputs = input_hash(project_dir, &overlay_text)?;
    let contract = build(&models, &metrics, &overlay, inputs)?;

    let text = yaml_serde::to_string(&contract)
        .map_err(|error| vec![format!("cannot serialize the contract: {error}")])?;
    // The same validation a hand-written contract goes through.
    contract::validate_text(&overlay_path.display().to_string(), &text)?;
    Ok(text)
}

// ---- SQLMesh file parsing ----

/// One `MODEL (…)` block plus its SQL body.
#[derive(Debug, Clone)]
struct ModelFile {
    /// `schema.table` as written in the `name` key.
    name: String,
    #[allow(dead_code)]
    kind: String,
    grain: Vec<String>,
    columns: Vec<String>,
    /// `(name, args)` for every audit call, args kept as raw text.
    audits: Vec<(String, BTreeMap<String, String>)>,
}

impl ModelFile {
    /// The physical table name (the model name's table part).
    fn table(&self) -> &str {
        self.name.rsplit('.').next().unwrap_or(&self.name)
    }

    fn audit(&self, name: &str) -> Vec<&BTreeMap<String, String>> {
        self.audits
            .iter()
            .filter(|(audit, _)| audit == name)
            .map(|(_, args)| args)
            .collect()
    }

    /// The declared grain, or the `unique_combination_of_columns` audit.
    fn grain_or_audit(&self) -> Result<Vec<String>, String> {
        if !self.grain.is_empty() {
            return Ok(self.grain.clone());
        }
        for args in self.audit("unique_combination_of_columns") {
            if let Some(columns) = args.get("columns") {
                let parsed = list(columns);
                if !parsed.is_empty() {
                    return Ok(parsed);
                }
            }
        }
        Err(format!(
            "model '{}' declares no grain and no unique_combination_of_columns audit; \
             a contract cannot be generated without the grain",
            self.name
        ))
    }
}

/// Read every `models/**/*.sql` in a deterministic order.
fn load_models(project_dir: &Path) -> Result<Vec<ModelFile>, Vec<String>> {
    let files = sql_files(&project_dir.join("models"))?;
    if files.is_empty() {
        return Err(vec![format!(
            "no models found under {}/models",
            project_dir.display()
        )]);
    }
    let mut models = Vec::new();
    let mut findings = Vec::new();
    for path in files {
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                findings.push(format!("cannot read {}: {error}", path.display()));
                continue;
            }
        };
        match parse_model(&text) {
            Ok(model) => models.push(model),
            Err(reason) => findings.push(format!("{}: {reason}", path.display())),
        }
    }
    if findings.is_empty() {
        Ok(models)
    } else {
        Err(findings)
    }
}

/// One `METRIC (…)` block.
#[derive(Debug, Clone)]
struct MetricFile {
    name: String,
    description: String,
    expression: String,
}

fn load_metrics(project_dir: &Path) -> Result<Vec<MetricFile>, Vec<String>> {
    let files = sql_files(&project_dir.join("metrics"))?;
    let mut metrics = Vec::new();
    let mut findings = Vec::new();
    for path in files {
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                findings.push(format!("cannot read {}: {error}", path.display()));
                continue;
            }
        };
        match parse_metric(&text) {
            Ok(metric) => metrics.push(metric),
            Err(reason) => findings.push(format!("{}: {reason}", path.display())),
        }
    }
    if findings.is_empty() {
        Ok(metrics)
    } else {
        Err(findings)
    }
}

/// `*.sql` under a directory, sorted by path (deterministic).
fn sql_files(dir: &Path) -> Result<Vec<PathBuf>, Vec<String>> {
    let mut files = Vec::new();
    collect_sql(dir, &mut files)
        .map_err(|error| vec![format!("cannot walk {}: {error}", dir.display())])?;
    files.sort();
    Ok(files)
}

fn collect_sql(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_sql(&path, out)?;
        } else if path.extension().is_some_and(|extension| extension == "sql") {
            out.push(path);
        }
    }
    Ok(())
}

/// Parse a `MODEL (…)` block (the subset the generator needs).
fn parse_model(text: &str) -> Result<ModelFile, String> {
    let block = block_body(text, "MODEL")?.ok_or_else(|| "no MODEL block".to_string())?;
    let mut name = String::new();
    let mut kind = String::new();
    let mut grain = Vec::new();
    let mut columns = Vec::new();
    let mut audits = Vec::new();
    for (key, value) in entries(&block) {
        match key.as_str() {
            "name" => name = value.trim().to_string(),
            "kind" => kind = value.trim().to_string(),
            "grain" => grain = list(&value),
            "columns" => {
                columns = list(&value)
                    .iter()
                    .filter_map(|entry| entry.split_whitespace().next())
                    .map(str::to_string)
                    .collect()
            }
            "audits" => {
                for entry in list(&value) {
                    let (audit, args) = call(&entry)?;
                    audits.push((audit, args));
                }
            }
            _ => {}
        }
    }
    if name.is_empty() {
        return Err("MODEL block has no name".into());
    }
    Ok(ModelFile {
        name,
        kind,
        grain,
        columns,
        audits,
    })
}

/// Parse a `METRIC (…)` block.
fn parse_metric(text: &str) -> Result<MetricFile, String> {
    let block = block_body(text, "METRIC")?.ok_or_else(|| "no METRIC block".to_string())?;
    let mut name = String::new();
    let mut description = String::new();
    let mut expression = String::new();
    for (key, value) in entries(&block) {
        match key.as_str() {
            "name" => name = value.trim().to_string(),
            "description" => {
                description = value
                    .trim()
                    .trim_matches('\'')
                    .trim_matches('"')
                    .to_string()
            }
            "expression" => expression = value.trim().to_string(),
            _ => {}
        }
    }
    if name.is_empty() || expression.is_empty() {
        return Err("METRIC block needs a name and an expression".into());
    }
    Ok(MetricFile {
        name,
        description,
        expression,
    })
}

/// The body of `KEY ( … )`, with balanced parentheses, `--` comments and
/// single-quoted strings respected.
fn block_body(text: &str, key: &str) -> Result<Option<String>, String> {
    let stripped = strip_comments(text);
    let Some(start) = stripped.find(&format!("{key} (")) else {
        return Ok(None);
    };
    let open = start + key.len() + 1;
    let mut depth = 0usize;
    let mut quote = false;
    for (index, character) in stripped[open..].char_indices() {
        match character {
            '\'' => quote = !quote,
            '(' if !quote => depth += 1,
            ')' if !quote => {
                depth -= 1;
                if depth == 0 {
                    return Ok(Some(stripped[open + 1..open + index].to_string()));
                }
            }
            _ => {}
        }
    }
    Err(format!("unbalanced parentheses in the {key} block"))
}

/// Drop `--` line comments (keeps the text length irrelevant afterwards).
fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("--") {
            Some(index) => &line[..index],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Top-level `key value` entries of a block body.
fn entries(body: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for part in split_top_level(body) {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (key, value) = match trimmed.find(char::is_whitespace) {
            Some(index) => (trimmed[..index].to_string(), trimmed[index..].to_string()),
            None => (trimmed.to_string(), String::new()),
        };
        out.push((key.to_lowercase(), value.trim().to_string()));
    }
    out
}

/// Split on commas that are not inside parentheses or strings.
fn split_top_level(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut quote = false;
    let mut current = String::new();
    for character in text.chars() {
        match character {
            '\'' => {
                quote = !quote;
                current.push(character);
            }
            '(' if !quote => {
                depth += 1;
                current.push(character);
            }
            ')' if !quote => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            ',' if !quote && depth == 0 => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(character),
        }
    }
    parts.push(current);
    parts
}

/// The items of a `(a, b, c)` list (a bare value is a one-item list).
fn list(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    let inner = trimmed
        .strip_prefix('(')
        .and_then(|rest| rest.strip_suffix(')'))
        .unwrap_or(trimmed);
    if inner.trim().is_empty() {
        return Vec::new();
    }
    split_top_level(inner)
        .iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

/// `name(arg := value, …)`, returning the call name and its named arguments.
fn call(text: &str) -> Result<(String, BTreeMap<String, String>), String> {
    let trimmed = text.trim();
    let Some(open) = trimmed.find('(') else {
        // A bare audit name (no arguments).
        return Ok((trimmed.to_string(), BTreeMap::new()));
    };
    let name = trimmed[..open].trim().to_string();
    if name.is_empty() {
        return Err(format!("malformed audit call '{trimmed}'"));
    }
    let inner = trimmed[open + 1..]
        .strip_suffix(')')
        .ok_or_else(|| format!("unbalanced audit call '{trimmed}'"))?;
    let mut args = BTreeMap::new();
    for part in split_top_level(inner) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, value) = match part.split_once(":=") {
            Some((key, value)) => (key.trim().to_lowercase(), value.trim().to_string()),
            None => (part.to_string(), String::new()),
        };
        args.insert(key, value);
    }
    Ok((name, args))
}

/// A short, deterministic hash of every input — the project's model, metric
/// and audit files plus the overlay (staleness detection, not cryptography).
fn input_hash(project_dir: &Path, overlay_text: &str) -> Result<String, Vec<String>> {
    let mut bytes: Vec<u8> = Vec::new();
    for sub in ["models", "metrics", "audits"] {
        for path in sql_files(&project_dir.join(sub))? {
            bytes.extend_from_slice(path.display().to_string().as_bytes());
            bytes.extend_from_slice(
                &std::fs::read(&path)
                    .map_err(|error| vec![format!("cannot read {}: {error}", path.display())])?,
            );
        }
    }
    bytes.extend_from_slice(overlay_text.as_bytes());
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    Ok(format!("fnv1a64:{hash:016x}"))
}

// ---- the overlay (the semantic layer SQLMesh cannot carry) ----

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Overlay {
    model: OverlayModel,
    #[serde(default)]
    grain: Vec<OverlayGrain>,
    #[serde(default)]
    dimensions: Vec<OverlayDimension>,
    #[serde(default)]
    measures: Vec<OverlayMeasure>,
    #[serde(default)]
    time_intelligence: Option<contract::TimeIntelligence>,
    #[serde(default)]
    security: Option<contract::Security>,
    #[serde(default)]
    annotations: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayModel {
    name: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayGrain {
    model: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    measure_group: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayDimension {
    model: String,
    id: String,
    attribute: String,
    #[serde(default)]
    date_role: bool,
    #[serde(default)]
    hierarchy_name: Option<String>,
    #[serde(default)]
    caption: String,
    #[serde(default)]
    ordinal: Option<u32>,
    #[serde(default = "default_true")]
    visible: bool,
    #[serde(default)]
    cardinality_hint: Option<u32>,
    #[serde(default)]
    levels: Vec<contract::Level>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayMeasure {
    metric: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    caption: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    ordinal: Option<u32>,
    #[serde(default = "default_true")]
    visible: bool,
    #[serde(default)]
    time_window: Option<contract::TimeWindow>,
    /// Overrides the classification when the SQL expression is not a shape the
    /// generator recognises.
    #[serde(default)]
    aggregation: Option<Aggregation>,
    #[serde(default)]
    source: Option<OverlaySource>,
    #[serde(default)]
    expression: Option<String>,
    #[serde(default)]
    valid_grain: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlaySource {
    table: String,
    #[serde(default)]
    column: Option<String>,
    #[serde(default)]
    reference: Option<String>,
}

fn default_true() -> bool {
    true
}

// ---- building the contract ----

fn build(
    models: &[ModelFile],
    metrics: &[MetricFile],
    overlay: &Overlay,
    source_hash: String,
) -> Result<Contract, Vec<String>> {
    let mut findings = Vec::new();
    let by_name: BTreeMap<&str, &ModelFile> = models
        .iter()
        .map(|model| (model.name.as_str(), model))
        .collect();

    for entry in &overlay.grain {
        if !by_name.contains_key(entry.model.as_str()) {
            findings.push(format!("grain entry names unknown model '{}'", entry.model));
        }
    }
    for dimension in &overlay.dimensions {
        if !by_name.contains_key(dimension.model.as_str()) {
            findings.push(format!(
                "dimension '{}' names unknown model '{}'",
                dimension.id, dimension.model
            ));
        }
    }
    let metrics_by_name: BTreeMap<&str, &MetricFile> = metrics
        .iter()
        .map(|metric| (metric.name.as_str(), metric))
        .collect();
    for measure in &overlay.measures {
        if !metrics_by_name.contains_key(measure.metric.as_str()) {
            findings.push(format!(
                "measure '{}' names unknown metric '{}'",
                measure.id.clone().unwrap_or_else(|| measure.metric.clone()),
                measure.metric
            ));
        }
    }

    // Relationship calls: audit name -> (column, reference model, reference column).
    struct Reference {
        fact: String,
        column: String,
        model: String,
        reference_column: String,
    }
    let mut references = Vec::new();
    for model in models {
        for args in model.audit("relationships") {
            let (Some(column), Some(reference), Some(reference_column)) = (
                args.get("column"),
                args.get("reference"),
                args.get("reference_column"),
            ) else {
                findings.push(format!(
                    "model '{}': relationships audit needs column, reference and reference_column",
                    model.name
                ));
                continue;
            };
            if !by_name.contains_key(reference.as_str()) {
                findings.push(format!(
                    "model '{}' references unknown model '{reference}'",
                    model.name
                ));
                continue;
            }
            references.push(Reference {
                fact: model.name.clone(),
                column: column.trim().to_string(),
                model: reference.trim().to_string(),
                reference_column: reference_column.trim().to_string(),
            });
        }
    }
    if references.is_empty() {
        findings.push(
            "no relationships audits found; a contract needs relationships (declare the custom \
             `relationships` audit on each fact model)"
                .to_string(),
        );
    }

    // Every referenced model is a dimension and must be declared in the overlay.
    let dimension_models: Vec<&str> = {
        let mut seen = Vec::new();
        for reference in &references {
            if !seen.contains(&reference.model.as_str()) {
                seen.push(reference.model.as_str());
            }
        }
        seen
    };
    for model in &dimension_models {
        if !overlay
            .dimensions
            .iter()
            .any(|dimension| dimension.model == *model)
        {
            findings.push(format!(
                "model '{model}' is referenced by a relationship but has no dimension in the \
                 overlay; dimension ids are never guessed"
            ));
        }
    }
    if !findings.is_empty() {
        return Err(findings);
    }

    // Dimensions: the join key comes from the relationship that references the
    // model; every relationship to it must agree.
    let mut dimensions = Vec::new();
    for overlay_dimension in &overlay.dimensions {
        let model = by_name[overlay_dimension.model.as_str()];
        let referenced: Vec<&Reference> = references
            .iter()
            .filter(|reference| reference.model == overlay_dimension.model)
            .collect();
        let key_columns: Vec<&str> = referenced
            .iter()
            .map(|reference| reference.reference_column.as_str())
            .collect();
        let Some(first_key) = key_columns.first() else {
            findings.push(format!(
                "dimension '{}' is on model '{}', which no relationship references",
                overlay_dimension.id, overlay_dimension.model
            ));
            continue;
        };
        if key_columns.iter().any(|column| column != first_key) {
            findings.push(format!(
                "model '{}' is referenced on different columns ({key_columns:?}); one dimension \
                 table has one key",
                overlay_dimension.model
            ));
            continue;
        }
        for column in std::iter::once(overlay_dimension.attribute.as_str())
            .chain(
                overlay_dimension
                    .levels
                    .iter()
                    .map(|level| level.column.as_str()),
            )
            .chain(std::iter::once(*first_key))
        {
            if !model.columns.iter().any(|declared| declared == column) {
                findings.push(format!(
                    "dimension '{}' names column '{column}', which is not declared in the columns \
                     of '{}'",
                    overlay_dimension.id, overlay_dimension.model
                ));
            }
        }
        dimensions.push(contract::Dimension {
            id: overlay_dimension.id.clone(),
            table: model.table().to_string(),
            key: contract::Key::Single((*first_key).to_string()),
            attribute: overlay_dimension.attribute.clone(),
            hierarchy_name: overlay_dimension.hierarchy_name.clone(),
            date_role: overlay_dimension.date_role,
            caption: if overlay_dimension.caption.is_empty() {
                overlay_dimension.id.clone()
            } else {
                overlay_dimension.caption.clone()
            },
            ordinal: overlay_dimension.ordinal,
            visible: overlay_dimension.visible,
            cardinality_hint: overlay_dimension.cardinality_hint,
            levels: overlay_dimension.levels.clone(),
        });
    }

    // Facts: models that are not dimension models.
    let overlay_grain: BTreeMap<&str, &OverlayGrain> = overlay
        .grain
        .iter()
        .map(|entry| (entry.model.as_str(), entry))
        .collect();
    let mut grain = Vec::new();
    let mut facts = Vec::new();
    for model in models {
        if dimension_models.contains(&model.name.as_str()) {
            continue;
        }
        let key = match model.grain_or_audit() {
            Ok(columns) => columns,
            Err(reason) => {
                findings.push(reason);
                continue;
            }
        };
        for column in &key {
            if !model.columns.is_empty() && !model.columns.iter().any(|declared| declared == column)
            {
                findings.push(format!(
                    "grain column '{column}' is not declared in the columns of '{}'",
                    model.name
                ));
            }
        }
        let entry = overlay_grain.get(model.name.as_str());
        grain.push(contract::Grain {
            table: model.table().to_string(),
            key: if key.len() == 1 {
                contract::Key::Single(key[0].clone())
            } else {
                contract::Key::Composite(key.clone())
            },
            id: entry.and_then(|entry| entry.id.clone()),
            measure_group: entry.and_then(|entry| entry.measure_group.clone()),
        });
        facts.push((model.name.clone(), key));
    }

    // Relationships: fact-major, dimensions in overlay order.
    let mut relationships = Vec::new();
    for (fact_name, _) in &facts {
        let fact = by_name[fact_name.as_str()];
        for overlay_dimension in &overlay.dimensions {
            let Some(reference) = references.iter().find(|reference| {
                reference.fact == *fact_name && reference.model == overlay_dimension.model
            }) else {
                continue;
            };
            relationships.push(contract::Relationship {
                fact: fact.table().to_string(),
                dimension: overlay_dimension.id.clone(),
                columns: vec![reference.column.clone(), reference.reference_column.clone()],
                cardinality: contract::Cardinality::ManyToOne,
                active: true,
            });
        }
    }

    // Measures: the overlay order first, then the remaining metrics in file
    // order; each expression is classified mechanically.
    let mut ordered: Vec<Option<&OverlayMeasure>> = vec![None; metrics.len()];
    for overlay_measure in &overlay.measures {
        if let Some(index) = metrics
            .iter()
            .position(|metric| metric.name == overlay_measure.metric)
        {
            ordered[index] = Some(overlay_measure);
        }
    }
    let mut measures = Vec::new();
    for (index, metric) in metrics.iter().enumerate() {
        let entry = ordered[index];
        match measure(metric, entry, models, &by_name, &grain) {
            Ok(measure) => measures.push(measure),
            Err(reason) => findings.push(reason),
        }
    }

    if !findings.is_empty() {
        return Err(findings);
    }

    Ok(Contract {
        contract_version: contract::SUPPORTED_VERSION.to_string() + ".0",
        model: contract::ModelInfo {
            name: overlay.model.name.clone(),
            description: overlay.model.description.clone(),
        },
        grain,
        dimensions,
        relationships,
        measures,
        time_intelligence: overlay.time_intelligence.clone(),
        security: overlay.security.clone(),
        provenance: contract::Provenance {
            source_system: contract::SourceSystem::Sqlmesh,
            source_hash: Some(source_hash),
            generator: format!("mallard-contract/{}", env!("CARGO_PKG_VERSION")),
            generated_at: None,
        },
        annotations: overlay.annotations.clone(),
    })
}

/// Classify one metric into a measure: the overlay wins, then the expression's
/// shape (`SUM`, `COUNT`, `COUNT(DISTINCT …)`, `MIN`/`MAX`, a ratio).
fn measure(
    metric: &MetricFile,
    overlay: Option<&OverlayMeasure>,
    models: &[ModelFile],
    by_name: &BTreeMap<&str, &ModelFile>,
    grain: &[contract::Grain],
) -> Result<contract::Measure, String> {
    let id = overlay
        .and_then(|entry| entry.id.clone())
        .unwrap_or_else(|| metric.name.clone());
    let caption = overlay
        .and_then(|entry| entry.caption.clone())
        .unwrap_or_else(|| id.clone());

    // A fully declared overlay measure needs no classification.
    if let Some(entry) = overlay
        && let (Some(aggregation), Some(source)) =
            (entry.aggregation.clone(), entry.source.as_ref())
    {
        return Ok(contract::Measure {
            id,
            caption,
            description: entry
                .description
                .clone()
                .unwrap_or_else(|| metric.description.clone()),
            format: entry.format.clone().unwrap_or_default(),
            ordinal: entry.ordinal,
            visible: entry.visible,
            source: contract::Source {
                table: source.table.clone(),
                column: source.column.clone(),
                reference: source.reference.clone(),
            },
            aggregation,
            expression: entry.expression.clone(),
            time_window: entry.time_window.clone(),
            valid_grain: entry.valid_grain.clone(),
        });
    }

    let expression = metric.expression.trim();
    let known_models: Vec<&str> = models.iter().map(|model| model.name.as_str()).collect();
    let parsed = classify(expression, &known_models).ok_or_else(|| {
        format!(
            "metric '{}' has an expression this generator cannot classify (`{expression}`); \
             declare aggregation and source in the overlay",
            metric.name
        )
    })?;
    let (aggregation, table, column) = parsed;
    if table.is_empty() {
        return Err(format!(
            "metric '{}' names no model (`{expression}`); declare aggregation and source in the \
             overlay",
            metric.name
        ));
    }
    let model = by_name.get(table.as_str()).ok_or_else(|| {
        format!(
            "metric '{}' references unknown model '{table}'",
            metric.name
        )
    })?;
    let valid_grain = match aggregation {
        Aggregation::Min | Aggregation::Max => grain
            .iter()
            .find(|entry| entry.table == model.table())
            .map(|entry| entry.key.columns().iter().map(|c| c.to_string()).collect()),
        _ => None,
    };
    let is_ratio = aggregation == Aggregation::Ratio;
    Ok(contract::Measure {
        id,
        caption,
        description: overlay
            .and_then(|entry| entry.description.clone())
            .unwrap_or_else(|| metric.description.clone()),
        format: overlay
            .and_then(|entry| entry.format.clone())
            .unwrap_or_default(),
        ordinal: overlay.and_then(|entry| entry.ordinal),
        visible: overlay.map(|entry| entry.visible).unwrap_or(true),
        source: contract::Source {
            table: model.table().to_string(),
            column,
            reference: None,
        },
        aggregation,
        expression: if is_ratio {
            Some(normalize_expression(expression, &table))
        } else {
            None
        },
        time_window: overlay.and_then(|entry| entry.time_window.clone()),
        valid_grain: overlay
            .and_then(|entry| entry.valid_grain.clone())
            .or(valid_grain),
    })
}

/// `SUM(model.column)` and friends, or a ratio of sums over one model.
///
/// The aggregate forms only match a *plain* column reference: a `SUM(` whose
/// matching `)` is not the expression's last character (a ratio) falls through
/// to the ratio branch.
fn classify(
    expression: &str,
    known_models: &[&str],
) -> Option<(Aggregation, String, Option<String>)> {
    let trimmed = expression.trim();
    for (prefix, aggregation) in [
        ("SUM(", Aggregation::Sum),
        ("MIN(", Aggregation::Min),
        ("MAX(", Aggregation::Max),
        ("COUNT(DISTINCT ", Aggregation::DistinctCount),
        ("COUNT(", Aggregation::Count),
    ] {
        if let Some(inner) = trimmed
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(')'))
            .map(str::trim)
        {
            if inner == "*" {
                return Some((aggregation, String::new(), None));
            }
            if inner.contains('(') || inner.contains(')') {
                // Not a simple aggregate (a ratio of sums, a nested call).
                break;
            }
            let (table, column) = split_reference(inner)?;
            return Some((aggregation, table, Some(column.to_string())));
        }
    }
    if trimmed.contains('/') && trimmed.to_uppercase().contains("SUM(") {
        // A ratio of sums: the contract's source is singular, so the
        // expression must name exactly one *known* model (constants and
        // function names are not models).
        let mut tables: Vec<String> = Vec::new();
        for token in trimmed.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.')) {
            if let Some((table, _)) = split_reference(token)
                && known_models.contains(&table.as_str())
                && !tables.contains(&table)
            {
                tables.push(table);
            }
        }
        if tables.len() == 1 {
            return Some((Aggregation::Ratio, tables.remove(0), None));
        }
    }
    None
}

/// The contract's expression is source-neutral: the source table is already
/// named by `source.table`, and SQLMesh expressions are model-qualified and
/// multi-line.
fn normalize_expression(expression: &str, table: &str) -> String {
    expression
        .replace(&format!("{table}."), "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `schema.model.column` or `model.column` → (model, column).
fn split_reference(text: &str) -> Option<(String, &str)> {
    let (model, column) = text.trim().rsplit_once('.')?;
    if model.is_empty() || column.is_empty() {
        return None;
    }
    Some((model.to_string(), column))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT: &str = "projects/upstream_marts_sqlmesh";
    const HAND_WRITTEN: &str = "contracts/upstream_marts/contract.yaml";

    fn generate_fixture() -> String {
        generate(
            Path::new(PROJECT),
            &Path::new(PROJECT).join("contract.overlay.yaml"),
        )
        .expect("the fixture must generate")
    }

    fn parse(text: &str) -> Contract {
        let path = std::env::temp_dir().join(format!(
            "mallardcube-generate-{}-{}.yaml",
            std::process::id(),
            text.len()
        ));
        std::fs::write(&path, text).expect("write");
        let contract = contract::validate_file(path.to_str().expect("utf-8")).expect("valid");
        let _ = std::fs::remove_file(&path);
        contract
    }

    /// The checked-in generated contract is exactly what the generator emits.
    #[test]
    fn the_fixture_generates_its_checked_in_contract() {
        let checked_in = std::fs::read_to_string(Path::new(PROJECT).join("contract.yaml"))
            .expect("checked-in generated contract");
        assert_eq!(generate_fixture(), checked_in);
    }

    /// And it is semantically the hand-written contract: same grain,
    /// dimensions, relationships, measures and flag catalogue (provenance and
    /// the source's annotations differ by design).
    #[test]
    fn the_generated_contract_matches_the_hand_written_one() {
        let generated = parse(&generate_fixture());
        let hand_written = contract::validate_file(HAND_WRITTEN).expect("hand-written fixture");

        let sorted = |values: Vec<String>| {
            let mut values = values;
            values.sort();
            values
        };

        assert_eq!(generated.model.name, hand_written.model.name);
        assert_eq!(
            sorted(
                generated
                    .grain
                    .iter()
                    .map(|entry| format!("{}:{:?}", entry.table, entry.key.columns()))
                    .collect()
            ),
            sorted(
                hand_written
                    .grain
                    .iter()
                    .map(|entry| format!("{}:{:?}", entry.table, entry.key.columns()))
                    .collect()
            )
        );
        assert_eq!(
            sorted(
                generated
                    .dimensions
                    .iter()
                    .map(|dimension| dimension.id.clone())
                    .collect()
            ),
            sorted(
                hand_written
                    .dimensions
                    .iter()
                    .map(|dimension| dimension.id.clone())
                    .collect()
            )
        );
        for (generated_dimension, hand_written_dimension) in generated
            .dimensions
            .iter()
            .zip(hand_written.dimensions.iter())
        {
            assert_eq!(generated_dimension, hand_written_dimension);
        }
        assert_eq!(
            sorted(
                generated
                    .relationships
                    .iter()
                    .map(|relationship| format!(
                        "{}.{} -> {}",
                        relationship.fact, relationship.columns[0], relationship.dimension
                    ))
                    .collect()
            ),
            sorted(
                hand_written
                    .relationships
                    .iter()
                    .map(|relationship| format!(
                        "{}.{} -> {}",
                        relationship.fact, relationship.columns[0], relationship.dimension
                    ))
                    .collect()
            )
        );
        assert_eq!(
            sorted(generated.measures.iter().map(|m| m.id.clone()).collect()),
            sorted(hand_written.measures.iter().map(|m| m.id.clone()).collect())
        );
        for measure in &generated.measures {
            let expected = hand_written
                .measures
                .iter()
                .find(|candidate| candidate.id == measure.id)
                .unwrap_or_else(|| panic!("hand-written measure '{}'", measure.id));
            // Descriptions come from the upstream metric metadata and surface
            // as Excel measure descriptions; the hand-written fixture keeps
            // that prose out, so compare everything else.
            assert_eq!(measure.id, expected.id);
            assert_eq!(measure.caption, expected.caption);
            assert_eq!(measure.format, expected.format);
            assert_eq!(measure.aggregation, expected.aggregation);
            assert_eq!(measure.source, expected.source);
            assert_eq!(measure.expression, expected.expression);
            assert_eq!(measure.time_window, expected.time_window);
            assert_eq!(measure.valid_grain, expected.valid_grain);
        }
        assert_eq!(generated.time_intelligence, hand_written.time_intelligence);
    }

    /// Generation is deterministic: same inputs, same bytes.
    #[test]
    fn generation_is_deterministic() {
        assert_eq!(generate_fixture(), generate_fixture());
    }

    /// The source hash covers the overlay: editing it changes the hash, so a
    /// stale checked-in contract is detectable.
    #[test]
    fn the_source_hash_covers_the_overlay() {
        let project = Path::new(PROJECT);
        let overlay = std::fs::read_to_string(project.join("contract.overlay.yaml")).expect("read");
        let edited_text = overlay.replace("  name: upstream_marts\n", "  name: upstream_marts_x\n");
        assert_ne!(edited_text, overlay, "the overlay fixture changed shape");
        let path = std::env::temp_dir().join(format!(
            "mallardcube-generate-hash-{}.yaml",
            std::process::id()
        ));
        std::fs::write(&path, edited_text).expect("write");
        let edited = generate(project, &path).expect("edited overlay generates");

        let hash_line = |text: &str| {
            text.lines()
                .find(|line| line.contains("source_hash:"))
                .unwrap_or_default()
                .to_string()
        };
        assert_ne!(hash_line(&generate_fixture()), hash_line(&edited));
        let _ = std::fs::remove_file(&path);
    }

    /// `generate --check` without `--out` is a usage error, not a silent
    /// fall-through to stdout.
    #[test]
    fn check_without_out_is_refused() {
        let args = contract::Args {
            action: "generate".into(),
            file: Some(PROJECT.into()),
            check: true,
            ..contract::Args::default()
        };
        assert_eq!(run(&args), 2);
    }

    /// An unknown model in the overlay refuses.
    #[test]
    fn unknown_overlay_models_are_refused() {
        let project = Path::new(PROJECT);
        let overlay = std::fs::read_to_string(project.join("contract.overlay.yaml")).expect("read");
        let broken = overlay.replace(
            "model: upstream_marts.dim_product, id: Category",
            "model: upstream_marts.dim_nope, id: Category",
        );
        let path = std::env::temp_dir().join(format!(
            "mallardcube-generate-overlay-{}.yaml",
            std::process::id()
        ));
        std::fs::write(&path, broken).expect("write");
        let findings = generate(project, &path).expect_err("unknown model");
        assert!(
            findings.iter().any(|finding| finding.contains("dim_nope")),
            "{findings:?}"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// A metric expression the generator cannot classify refuses with the
    /// overlay escape hatch named.
    #[test]
    fn unclassifiable_metrics_are_refused() {
        let models = &[
            "upstream_marts.fact_orders",
            "upstream_marts.mart_cumulative_month",
        ];
        assert!(classify("SUM(upstream_marts.fact_orders.revenue)", models).is_some());
        assert!(
            classify(
                "MAX(upstream_marts.mart_cumulative_month.cumulative_revenue_cy)",
                models
            )
            .is_some()
        );
        assert!(classify("COUNT(*)", models).is_some());
        assert!(classify("COUNT(DISTINCT upstream_marts.fact_orders.status)", models).is_some());
        // A ratio of sums over one known model, with constants and NULLIF.
        let ratio = "SUM(upstream_marts.fact_orders.is_within_sla) * 100.0\n \
                     / NULLIF(SUM(upstream_marts.fact_orders.is_order), 0)";
        assert_eq!(
            classify(ratio, models).map(|(aggregation, table, _)| (aggregation, table)),
            Some((Aggregation::Ratio, "upstream_marts.fact_orders".to_string()))
        );
        assert!(
            classify(
                "custom_function(upstream_marts.fact_orders.revenue)",
                models
            )
            .is_none()
        );
        assert!(
            classify("SUM(a.x) / SUM(b.y)", models).is_none(),
            "two source models cannot be one measure source"
        );
    }

    /// The parser reads the fixture's model and metric files.
    #[test]
    fn the_parser_reads_the_fixture() {
        let models = load_models(Path::new(PROJECT)).expect("models");
        assert_eq!(models.len(), 6);
        let fact = models
            .iter()
            .find(|model| model.name == "upstream_marts.fact_orders")
            .expect("fact_orders");
        assert_eq!(fact.grain, vec!["order_id"]);
        assert!(fact.columns.contains(&"revenue".to_string()));
        assert_eq!(fact.audit("relationships").len(), 3);
        assert!(fact.grain_or_audit().is_ok());

        let metrics = load_metrics(Path::new(PROJECT)).expect("metrics");
        assert_eq!(metrics.len(), 11);
        assert!(metrics.iter().any(|metric| metric.name == "revenue"));
    }
}
