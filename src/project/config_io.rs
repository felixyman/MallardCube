//! Config file I/O: JSON or YAML, section files, and the canonical writer
//! (plan 040).
//!
//! Large models get unwieldy in a single JSON file: no comments, escaped
//! non-ASCII, and hundreds of repeated boilerplate fields. The loader accepts
//! YAML as well as JSON (detected by extension, then by content), merges
//! optional section files (`dimensions_file`, `measures_file`,
//! `relationships_file`, `roles_file`), and applies derived defaults
//! ([`ProxyConfig::normalize`]). `mallard fmt` writes the canonical form back,
//! re-splitting sections that declare a file.

use crate::project::config::ProxyConfig;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Json,
    Yaml,
}

/// Extension first, then content sniffing, so a file without a known extension
/// (or piped input) still parses.
pub fn detect_format(path: &Path, text: &str) -> ConfigFormat {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("yaml") | Some("yml") => ConfigFormat::Yaml,
        Some("json") => ConfigFormat::Json,
        _ => {
            let trimmed = text.trim_start();
            if trimmed.starts_with('{') || trimmed.starts_with('[') {
                ConfigFormat::Json
            } else {
                ConfigFormat::Yaml
            }
        }
    }
}

fn parse<T: DeserializeOwned>(text: &str, format: ConfigFormat) -> Result<T, String> {
    match format {
        ConfigFormat::Json => serde_json::from_str(text).map_err(|e| e.to_string()),
        ConfigFormat::Yaml => yaml_serde::from_str(text).map_err(|e| e.to_string()),
    }
}

pub fn parse_str(text: &str, format: ConfigFormat) -> Result<ProxyConfig, String> {
    parse(text, format)
}

/// Canonical serialization: struct field order (stable), YAML block style,
/// trailing newline.
pub fn serialize(config: &ProxyConfig, format: ConfigFormat) -> Result<String, String> {
    match format {
        ConfigFormat::Json => {
            let mut json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
            json.push('\n');
            Ok(json)
        }
        ConfigFormat::Yaml => yaml_serde::to_string(config).map_err(|e| e.to_string()),
    }
}

/// Load a config: detect format, parse, merge section files, normalize
/// derived defaults. Section paths resolve relative to the config file.
pub fn load(path: &Path) -> Result<ProxyConfig, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let format = detect_format(path, &text);
    let mut config =
        parse_str(&text, format).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    merge_section(
        &dir,
        config.dimensions_file.as_deref(),
        &mut config.dimensions,
    )?;
    merge_section(&dir, config.measures_file.as_deref(), &mut config.measures)?;
    merge_section(
        &dir,
        config.relationships_file.as_deref(),
        &mut config.relationships,
    )?;
    merge_section(&dir, config.roles_file.as_deref(), &mut config.roles)?;
    config.normalize();
    Ok(config)
}

fn merge_section<T: DeserializeOwned>(
    dir: &Path,
    file: Option<&str>,
    target: &mut Vec<T>,
) -> Result<(), String> {
    let Some(file) = file.filter(|f| !f.is_empty()) else {
        return Ok(());
    };
    let path = dir.join(file);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("read section {}: {e}", path.display()))?;
    let format = detect_format(&path, &text);
    let mut items: Vec<T> =
        parse(&text, format).map_err(|e| format!("parse section {}: {e}", path.display()))?;
    // Inline entries come first, then the section file's.
    target.append(&mut items);
    Ok(())
}

/// Canonical text for the main file and every section file, without writing.
/// Sections that declare a `*_file` move entirely into that file; the rest
/// stay inline.
pub fn canonical_texts(
    config: &ProxyConfig,
    path: &Path,
) -> Result<Vec<(PathBuf, String)>, String> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let format = detect_format(path, &existing);
    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut main = config.clone();
    // Compact canonical form: fields equal to their derived default are left
    // out, so a minimal config stays minimal.
    main.deminimize();
    let mut out: Vec<(PathBuf, String)> = Vec::new();

    if let Some(file) = main.dimensions_file.clone().filter(|f| !f.is_empty()) {
        let target = dir.join(&file);
        out.push((target, section_text(&main.dimensions, &dir, &file)?));
        main.dimensions.clear();
    }
    if let Some(file) = main.measures_file.clone().filter(|f| !f.is_empty()) {
        let target = dir.join(&file);
        out.push((target, section_text(&main.measures, &dir, &file)?));
        main.measures.clear();
    }
    if let Some(file) = main.relationships_file.clone().filter(|f| !f.is_empty()) {
        let target = dir.join(&file);
        out.push((target, section_text(&main.relationships, &dir, &file)?));
        main.relationships.clear();
    }
    if let Some(file) = main.roles_file.clone().filter(|f| !f.is_empty()) {
        let target = dir.join(&file);
        out.push((target, section_text(&main.roles, &dir, &file)?));
        main.roles.clear();
    }
    out.push((path.to_path_buf(), serialize(&main, format)?));
    Ok(out)
}

fn section_text<T: Serialize>(items: &[T], dir: &Path, file: &str) -> Result<String, String> {
    let path = dir.join(file);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    match detect_format(&path, &existing) {
        ConfigFormat::Json => {
            let mut text = serde_json::to_string_pretty(items).map_err(|e| e.to_string())?;
            text.push('\n');
            Ok(text)
        }
        ConfigFormat::Yaml => yaml_serde::to_string(items).map_err(|e| e.to_string()),
    }
}

/// Write the canonical form. Returns the files written.
pub fn write_canonical(config: &ProxyConfig, path: &Path) -> Result<Vec<PathBuf>, String> {
    let texts = canonical_texts(config, path)?;
    for (target, text) in &texts {
        std::fs::write(target, text).map_err(|e| format!("write {}: {e}", target.display()))?;
    }
    Ok(texts.into_iter().map(|(target, _)| target).collect())
}

/// Files that are not in canonical form (empty = clean).
pub fn non_canonical(config: &ProxyConfig, path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut dirty = Vec::new();
    for (target, text) in canonical_texts(config, path)? {
        let current = std::fs::read_to_string(&target).unwrap_or_default();
        if current != text {
            dirty.push(target);
        }
    }
    Ok(dirty)
}

/// Section-file list used by the CLI to report what it wrote.
pub fn section_files(config: &ProxyConfig) -> Vec<String> {
    let mut files = Vec::new();
    for file in [
        &config.dimensions_file,
        &config.measures_file,
        &config.relationships_file,
        &config.roles_file,
    ]
    .into_iter()
    .flatten()
    {
        if !file.is_empty() {
            files.push(file.clone());
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mallardcube-config-io-{}-{}-{name}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const MINIMAL_YAML: &str = r#"
catalog: SHOP
cube: Sales
table_name: sales_fact
dimensions:
  - id: Category
    caption: Category
measures:
  - id: Revenue
    caption: Revenue
    sql_expr: SUM(revenue)
"#;

    const MINIMAL_JSON: &str = r#"{
  "catalog": "SHOP",
  "cube": "Sales",
  "table_name": "sales_fact",
  "dimensions": [{"id": "Category", "caption": "Category"}],
  "measures": [{"id": "Revenue", "caption": "Revenue", "sql_expr": "SUM(revenue)"}]
}"#;

    #[test]
    fn format_detection_prefers_extension_then_content() {
        assert_eq!(detect_format(Path::new("a.yaml"), "{}"), ConfigFormat::Yaml);
        assert_eq!(detect_format(Path::new("a.yml"), ""), ConfigFormat::Yaml);
        assert_eq!(
            detect_format(Path::new("a.json"), "catalog: x"),
            ConfigFormat::Json
        );
        assert_eq!(
            detect_format(Path::new("noext"), "  {\"a\": 1}"),
            ConfigFormat::Json
        );
        assert_eq!(
            detect_format(Path::new("noext"), "catalog: x"),
            ConfigFormat::Yaml
        );
    }

    #[test]
    fn yaml_and_json_parse_to_the_same_config() {
        let from_yaml = parse_str(MINIMAL_YAML, ConfigFormat::Yaml).expect("yaml");
        let from_json = parse_str(MINIMAL_JSON, ConfigFormat::Json).expect("json");
        assert_eq!(from_yaml.catalog, from_json.catalog);
        assert_eq!(from_yaml.dimensions[0].id, from_json.dimensions[0].id);
        assert_eq!(
            from_yaml.dimensions[0].hierarchy_name,
            from_json.dimensions[0].hierarchy_name
        );
        assert_eq!(
            from_yaml.measures[0].format_string,
            from_json.measures[0].format_string
        );
    }

    #[test]
    fn minimal_config_gets_derived_defaults() {
        let mut config = parse_str(MINIMAL_YAML, ConfigFormat::Yaml).expect("yaml");
        config.normalize();
        let dim = &config.dimensions[0];
        assert_eq!(dim.hierarchy_name, "Category", "caption cascades");
        assert_eq!(dim.leaf_level_name, "Category");
        assert_eq!(dim.all_level_name, "(All)");
        assert_eq!(dim.ordinal, 1, "ordinal follows array order");
        assert!(dim.visible, "visible defaults to true");
        assert_eq!(dim.physical_field, "Category", "physical_field = id");
        let measure = &config.measures[0];
        assert_eq!(measure.display_name, "Revenue");
        assert_eq!(measure.format_string, "#,##0.00");
        assert_eq!(measure.ordinal, 1);
        assert_eq!(measure.fact_table.as_deref(), Some("default"));
        assert_eq!(
            measure.measure_group_name, "Sales",
            "falls back to the cube"
        );
        assert_eq!(config.dialect, "duckdb");
    }

    #[test]
    fn explicit_values_are_never_overwritten() {
        let mut config = parse_str(MINIMAL_YAML, ConfigFormat::Yaml).expect("yaml");
        config.dimensions[0].hierarchy_name = "Category Hierarchy".into();
        config.dimensions[0].ordinal = 7;
        config.measures[0].format_string = "0.0%".into();
        config.normalize();
        assert_eq!(config.dimensions[0].hierarchy_name, "Category Hierarchy");
        assert_eq!(config.dimensions[0].ordinal, 7);
        assert_eq!(config.measures[0].format_string, "0.0%");
    }

    #[test]
    fn section_files_merge_inline_first() {
        let dir = temp_dir("sections");
        std::fs::write(
            dir.join("dimensions.yaml"),
            "- id: Region\n  caption: Region\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("proxy-config.yaml"),
            r#"
catalog: SHOP
cube: Sales
table_name: sales_fact
dimensions:
  - id: Category
    caption: Category
dimensions_file: dimensions.yaml
measures:
  - id: Revenue
    caption: Revenue
    sql_expr: SUM(revenue)
"#,
        )
        .unwrap();
        let config = load(&dir.join("proxy-config.yaml")).expect("load");
        let ids: Vec<&str> = config.dimensions.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["Category", "Region"], "inline first, then the file");
        assert_eq!(config.dimensions[1].hierarchy_name, "Region");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn canonical_writer_round_trips_and_reports_dirty_files() {
        let dir = temp_dir("canonical");
        let path = dir.join("proxy-config.yaml");
        std::fs::write(&path, MINIMAL_YAML).unwrap();

        let config = load(&path).expect("load");
        // The minimal file is not canonical (defaults are now explicit).
        let dirty = non_canonical(&config, &path).expect("check");
        assert_eq!(dirty, vec![path.clone()], "the file needs formatting");

        write_canonical(&config, &path).expect("write");
        assert!(
            non_canonical(&config, &path).expect("check").is_empty(),
            "after fmt the file is canonical"
        );

        let reloaded = load(&path).expect("reload");
        assert_eq!(reloaded.dimensions[0].hierarchy_name, "Category");
        assert_eq!(reloaded.measures[0].format_string, "#,##0.00");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn canonical_writer_splits_and_reloads() {
        let dir = temp_dir("split");
        let path = dir.join("proxy-config.yaml");
        std::fs::write(
            &path,
            r#"
catalog: SHOP
cube: Sales
table_name: sales_fact
dimensions_file: dimensions.yaml
measures:
  - id: Revenue
    caption: Revenue
    sql_expr: SUM(revenue)
"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("dimensions.yaml"),
            "- id: Category\n  caption: Category\n",
        )
        .unwrap();

        let config = load(&path).expect("load");
        let written = write_canonical(&config, &path).expect("write");
        assert!(
            written.iter().any(|p| p.ends_with("dimensions.yaml")),
            "the section file is rewritten: {written:?}"
        );
        // The main file no longer inlines the section.
        let main_text = std::fs::read_to_string(&path).unwrap();
        assert!(main_text.contains("dimensions_file: dimensions.yaml"));
        assert!(
            !main_text.contains("id: Category"),
            "moved to the section file"
        );

        let reloaded = load(&path).expect("reload");
        assert_eq!(reloaded.dimensions.len(), 1);
        assert_eq!(reloaded.dimensions[0].id, "Category");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn json_configs_stay_json() {
        let dir = temp_dir("json");
        let path = dir.join("proxy-config.json");
        std::fs::write(&path, MINIMAL_JSON).unwrap();
        let config = load(&path).expect("load");
        write_canonical(&config, &path).expect("write");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.trim_start().starts_with('{'), "JSON in, JSON out");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The shipped YAML twin (with defaults and a section file) must describe
    /// the same model as the JSON fixture it mirrors.
    #[test]
    fn project2_yaml_twin_matches_the_json_fixture() {
        let json = load(Path::new("projects/project2/proxy-config.json")).expect("json twin");
        let yaml = load(Path::new("projects/project2/proxy-config.yaml")).expect("yaml twin");
        assert_eq!(json.catalog, yaml.catalog);
        assert_eq!(json.cube, yaml.cube);
        assert_eq!(json.source_name, yaml.source_name);
        assert_eq!(json.table_name, yaml.table_name);
        assert_eq!(json.dialect, yaml.dialect);
        assert_eq!(json.dimensions.len(), yaml.dimensions.len());
        for (a, b) in json.dimensions.iter().zip(&yaml.dimensions) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.physical_field, b.physical_field);
            assert_eq!(a.caption, b.caption);
            assert_eq!(a.description, b.description);
            assert_eq!(a.hierarchy_name, b.hierarchy_name);
            assert_eq!(a.all_level_name, b.all_level_name);
            assert_eq!(a.leaf_level_name, b.leaf_level_name);
            assert_eq!(a.ordinal, b.ordinal);
            assert_eq!(a.visible, b.visible);
            assert_eq!(a.has_all, b.has_all);
            assert_eq!(a.cardinality_hint, b.cardinality_hint);
        }
        assert_eq!(json.measures.len(), yaml.measures.len());
        for (a, b) in json.measures.iter().zip(&yaml.measures) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.sql_expr, b.sql_expr);
            assert_eq!(a.caption, b.caption);
            assert_eq!(a.display_name, b.display_name);
            assert_eq!(a.description, b.description);
            assert_eq!(a.format_string, b.format_string);
            assert_eq!(a.units, b.units);
            assert_eq!(a.ordinal, b.ordinal);
            assert_eq!(a.visible, b.visible);
            assert_eq!(a.measure_group_name, b.measure_group_name);
        }
    }
}
