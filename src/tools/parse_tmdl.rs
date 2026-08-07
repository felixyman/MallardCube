//! TMDL-format parser for Tabular Editor 2.x project exports.
//! Reads YAML-like .tmdl text files from a TMDL folder structure
//! and produces a `TabularModel`, matching the interface of the BIM/folder parsers.
//!
//! ## Two-phase design
//! Phase 1: TMDL text → `TmdlDocument` AST (line-by-line, indentation-aware)
//! Phase 2: `TmdlDocument` → `TabularModel` (build shared model types)
//!
//! ## Supported files
//! - `database.tmdl` — database name + compatibilityLevel
//! - `tables/*.tmdl` — table definitions with columns, measures, partitions, hierarchies
//! - `relationships.tmdl` — all relationships
//! - `roles/*.tmdl` — role definitions
//!
//! ## Unsupported (with warnings)
//! - `expressions.tmdl` — shared expressions
//! - `tablePermission` — RLS row-level security
//! - DirectQuery/dual partition modes
//! - Non-1700/1567 compatibility levels

use super::tabular_model::*;
use std::fs;
use std::path::Path;

// ── Phase 1: TMDL AST types ──────────────────────────────────────────────────

#[derive(Debug)]
struct TmdlObject {
    object_type: String,
    name: String,
    description: String,
    /// The `= expression` value (flattened from multi-line continuations).
    expression: String,
    /// `key: value` / `key` (boolean shortcut) properties.
    properties: Vec<(String, String)>,
    children: Vec<TmdlObject>,
}

#[derive(Debug)]
struct TmdlDocument {
    objects: Vec<TmdlObject>,
}

// ── Phase 1 helpers ──────────────────────────────────────────────────────────

/// Unquote a TMDL name: `'Name'` → `Name`, `''` → `'`.
fn tmdl_unquote(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('\'') && s.len() >= 2 {
        let inner = &s[1..s.len() - 1];
        inner.replace("''", "'")
    } else {
        s.to_string()
    }
}

/// Known TMDL boolean property keywords (no colon/value, just the key).
const TMDL_BOOLEAN_PROPERTIES: &[&str] = &[
    "isKey", "isHidden", "isNullable", "isUnique", "isDefaultLabel",
    "isDefaultImage", "isAvailableInMDX", "isDataTypeInferred", "isNameInferred",
    "isPrivate", "isSimpleMeasure", "isActive", "keepUniqueRows",
    "discourageImplicitMeasures", "discourageReportMeasures",
    "discourageCompositeModels", "excludeFromModelRefresh",
    "excludeFromAutomaticAggregations", "systemManaged",
    "forceUniqueNames", "relyOnReferentialIntegrity",
    "returnErrorValuesAsNull",
];

/// Known TMDL object type keywords (in order likely to be encountered).
fn is_known_object_type(word: &str) -> bool {
    matches!(
        word,
        "database"
            | "table"
            | "column"
            | "measure"
            | "partition"
            | "hierarchy"
            | "level"
            | "relationship"
            | "role"
            | "member"
            | "tablePermission"
            | "modelPermission"
    )
}

/// Does the line contain a `:` at the top level (not inside single quotes)?
fn has_top_level_colon(line: &str) -> bool {
    let mut in_quote = false;
    for c in line.chars() {
        match c {
            '\'' => in_quote = !in_quote,
            ':' if !in_quote => return true,
            _ => {}
        }
    }
    false
}

/// Split `objectType nameAndRest` into (type, rest). Returns None if first word
/// is not a known object type.
fn parse_object_declaration(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    let first_end = trimmed.find(char::is_whitespace)?;
    let word = &trimmed[..first_end];
    if is_known_object_type(word) {
        Some((word, trimmed[first_end..].trim()))
    } else {
        None
    }
}

/// Given the portion after the object-type keyword, extract (name, optional_expression).
/// Expression is `= value` inline, or present but empty when continuation follows.
fn parse_name_and_expression(rest: &str) -> (String, Option<String>) {
    let rest = rest.trim();
    // Find the first unquoted `=` to split name from expression
    let mut in_quote = false;
    let mut eq_pos = None;
    for (i, c) in rest.char_indices() {
        match c {
            '\'' => in_quote = !in_quote,
            '=' if !in_quote => {
                eq_pos = Some(i);
                break;
            }
            _ => {}
        }
    }
    match eq_pos {
        Some(pos) => {
            let name = rest[..pos].trim().to_string();
            let expr_rest = rest[pos + 1..].trim();
            if expr_rest.is_empty() {
                (tmdl_unquote(&name), Some(String::new()))
            } else {
                (tmdl_unquote(&name), Some(expr_rest.to_string()))
            }
        }
        None => (tmdl_unquote(rest), None),
    }
}

/// Parse a `key: value` property from a line (colon not inside quotes).
/// Returns None if no top-level colon found.
fn parse_colon_property(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    let mut in_quote = false;
    let mut colon_pos = None;
    for (i, c) in trimmed.char_indices() {
        match c {
            '\'' => in_quote = !in_quote,
            ':' if !in_quote => {
                colon_pos = Some(i);
                break;
            }
            _ => {}
        }
    }
    let pos = colon_pos?;
    let key = trimmed[..pos].trim().to_string();
    let value = trimmed[pos + 1..].trim().to_string();
    Some((key, value))
}

/// Count leading tab characters.
fn count_leading_tabs(line: &str) -> usize {
    line.chars().take_while(|&c| c == '\t').count()
}

// ── Phase 1: Parse a single .tmdl file ───────────────────────────────────────

/// Parse the content of a single .tmdl file into a list of root-level TmdlObjects.
fn parse_tmdl_file_text(text: &str, warnings: &mut Vec<String>) -> Vec<TmdlObject> {
    let mut root_objects: Vec<TmdlObject> = Vec::new();
    let mut stack: Vec<(usize, TmdlObject)> = Vec::new();
    let mut pending_description = String::new();

    // Expression continuation state
    let mut expr_active = false;
    let mut expr_base_indent: usize = 0;
    let mut expr_parts: Vec<String> = Vec::new();

    // Backtick verbatim mode
    let mut in_backtick = false;
    let mut backtick_content = String::new();

    // Tab validation: on first non-blank line, reject space indentation
    let mut first_nonblank = true;

    let lines: Vec<&str> = text.lines().collect();
    let mut line_idx = 0;
    while line_idx < lines.len() {
        let raw_line = lines[line_idx];
        let tab_count = count_leading_tabs(raw_line);
        let content = raw_line.trim_start_matches('\t');
        let trimmed = content.trim();

        // ── Tab validation on first non-blank line ──
        if first_nonblank && !trimmed.is_empty() {
            first_nonblank = false;
            if !raw_line.starts_with('\t') && !raw_line.starts_with("///") {
                // If the line starts with spaces (not tab) and is not a top-level /// description
                let leading_spaces = raw_line.len() - raw_line.trim_start().len();
                if leading_spaces > 0 && !raw_line.starts_with('\t') {
                    warnings.push(
                        "TMDL uses tab indentation; found space indentation".to_string(),
                    );
                    // Continue parsing — be lenient
                }
            }
        }

        // ── Skip blank lines ──
        if trimmed.is_empty() {
            line_idx += 1;
            continue;
        }

        // ── Backtick toggle ──
        if trimmed == "```" {
            if in_backtick {
                // Closing backtick — set accumulated content as current expression
                in_backtick = false;
                if let Some((_, obj)) = stack.last_mut() {
                    let bt = backtick_content.trim().to_string();
                    if obj.expression.is_empty() {
                        obj.expression = bt;
                    } else {
                        obj.expression.push('\n');
                        obj.expression.push_str(&bt);
                    }
                }
                backtick_content.clear();
            } else {
                in_backtick = true;
                backtick_content.clear();
            }
            line_idx += 1;
            continue;
        }

        // ── Backtick accumulation ──
        if in_backtick {
            backtick_content.push_str(raw_line);
            backtick_content.push('\n');
            line_idx += 1;
            continue;
        }

        // ── /// description ──
        if let Some(desc) = trimmed.strip_prefix("///") {
            let desc = desc.trim();
            if !pending_description.is_empty() {
                pending_description.push('\n');
            }
            pending_description.push_str(desc);
            line_idx += 1;
            continue;
        }

        // ── Expression continuation handling ──
        if expr_active {
            if tab_count > expr_base_indent {
                // Deeper indent — could be continuation or a property/object
                // Check if it's a known property or object
                let is_obj = parse_object_declaration(&raw_line).is_some();
                let is_colon = has_top_level_colon(trimmed);
                // Boolean shortcut must be a known keyword
                let is_boolean = !is_obj && TMDL_BOOLEAN_PROPERTIES.contains(&trimmed);

                if is_obj || is_colon || is_boolean {
                    // End expression and fall through to process this line
                    if let Some((_, obj)) = stack.last_mut() {
                        if !expr_parts.is_empty() {
                            obj.expression = expr_parts.join(" ");
                        }
                    }
                    expr_active = false;
                    expr_parts.clear();
                    // Fall through — don't increment line_idx
                } else {
                    // Continuation — append
                    expr_parts.push(trimmed.to_string());
                    line_idx += 1;
                    continue;
                }
            } else {
                // Same or lesser indent — end expression
                if let Some((_, obj)) = stack.last_mut() {
                    if !expr_parts.is_empty() {
                        obj.expression = expr_parts.join(" ");
                    }
                }
                expr_active = false;
                expr_parts.clear();
                // Fall through
            }
        }

        // ── Indentation: pop stack to match current level ──
        // Pop while the top of stack is at >= current indent level
        while let Some(&(top_level, _)) = stack.last() {
            if top_level >= tab_count {
                let (_, obj) = stack.pop().unwrap();
                if let Some((_, parent)) = stack.last_mut() {
                    parent.children.push(obj);
                } else {
                    root_objects.push(obj);
                }
            } else {
                break;
            }
        }

        // ── Classify the line ──

        // Object type declaration?
        if let Some((obj_type, rest)) = parse_object_declaration(&raw_line) {
            let (name, expr_opt) = parse_name_and_expression(rest);
            let expr = expr_opt.clone().unwrap_or_default();

            let obj = TmdlObject {
                object_type: obj_type.to_string(),
                name,
                description: pending_description.clone(),
                expression: expr,
                properties: Vec::new(),
                children: Vec::new(),
            };
            pending_description.clear();

            // If expression was present but empty, and the line had = at the end,
            // enter continuation mode.
            if expr_opt.as_ref().map_or(false, String::is_empty) {
                expr_active = true;
                expr_base_indent = tab_count;
                expr_parts.clear();
            }

            stack.push((tab_count, obj));
            line_idx += 1;
            continue;
        }

        // Colon-delimited property?
        if let Some((key, value)) = parse_colon_property(trimmed) {
            if let Some((_, obj)) = stack.last_mut() {
                obj.properties.push((key, value));
            }
            line_idx += 1;
            continue;
        }

        // Boolean shortcut (no colon, no equals)?
        if !trimmed.contains(':') && !trimmed.contains('=') {
            if let Some((_, obj)) = stack.last_mut() {
                obj.properties.push((trimmed.to_string(), "true".to_string()));
            }
            line_idx += 1;
            continue;
        }

        // If we get here, the line is unrecognized
        warnings.push(format!("Unrecognized TMDL line: {}", trimmed));
        line_idx += 1;
    }

    // ── Flush remaining expression continuation ──
    if expr_active {
        if let Some((_, obj)) = stack.last_mut() {
            if !expr_parts.is_empty() {
                obj.expression = expr_parts.join(" ");
            }
        }
    }

    // ── Flush remaining backtick mode ──
    if in_backtick {
        warnings.push("Unclosed backtick expression — content may be truncated".to_string());
    }

    // ── Pop remaining stack ──
    while let Some((_, obj)) = stack.pop() {
        if let Some((_, parent)) = stack.last_mut() {
            parent.children.push(obj);
        } else {
            root_objects.push(obj);
        }
    }

    root_objects
}

// ── Phase 2: TmdlDocument → TabularModel ────────────────────────────────────

/// Walk the AST and build the shared `TabularModel`.
fn build_model(
    doc: TmdlDocument,
    warnings: &mut Vec<String>,
    model_dir: &str,
) -> TabularModel {
    let mut name = String::from("SemanticModel");
    let mut compatibility_level: i64 = 0;
    let mut tables: Vec<TableInfo> = Vec::new();
    let mut relationships: Vec<RelInfo> = Vec::new();
    let mut roles: Vec<RoleInfo> = Vec::new();

    // ── Check for expressions.tmdl ──
    let expressions_path = Path::new(model_dir).join("expressions.tmdl");
    if expressions_path.exists() {
        warnings.push(
            "expressions.tmdl found — shared expressions are not supported".to_string(),
        );
    }

    for obj in &doc.objects {
        match obj.object_type.as_str() {
            "database" => {
                name = obj.name.clone();
                for (key, val) in &obj.properties {
                    if key == "compatibilityLevel" {
                        compatibility_level = val.parse::<i64>().unwrap_or(0);
                    }
                }
                // Warn about unsupported compatibility levels (like BIM parser)
                if compatibility_level != 0
                    && compatibility_level != 1700
                    && compatibility_level != 1567
                {
                    warnings.push(format!(
                        "compatibilityLevel {} is not 1700 or 1567 (commonly supported)",
                        compatibility_level
                    ));
                }
            }
            "table" => {
                if let Some(ti) = build_table(obj, warnings) {
                    tables.push(ti);
                }
            }
            "relationship" => {
                if let Some(rel) = build_relationship(obj, warnings) {
                    relationships.push(rel);
                }
            }
            "role" => {
                if let Some(role) = build_role(obj, warnings) {
                    roles.push(role);
                }
            }
            _ => {}
        }
    }

    // Sort consistently with other parsers
    tables.sort_by(|a, b| a.name.cmp(&b.name));
    relationships.sort_by(|a, b| {
        a.from_table
            .cmp(&b.from_table)
            .then(a.to_table.cmp(&b.to_table))
    });

    TabularModel {
        name,
        compatibility_level,
        tables,
        relationships,
        roles,
        data_sources: vec![],
    }
}

fn build_table(obj: &TmdlObject, warnings: &mut Vec<String>) -> Option<TableInfo> {
    let name = obj.name.clone();
    let ssas_name = name.clone();
    let description = obj.description.clone();

    let mut columns: Vec<ColumnInfo> = Vec::new();
    let mut measures: Vec<MeasureInfo> = Vec::new();
    let mut partitions: Vec<PartitionInfo> = Vec::new();
    let mut hierarchies: Vec<String> = Vec::new();

    for child in &obj.children {
        match child.object_type.as_str() {
            "column" => {
                if let Some(col) = build_column(child) {
                    columns.push(col);
                }
            }
            "measure" => {
                if let Some(meas) = build_measure(child) {
                    measures.push(meas);
                }
            }
            "partition" => {
                if let Some(part) = build_partition(child, warnings) {
                    partitions.push(part);
                }
            }
            "hierarchy" => {
                hierarchies.push(child.name.clone());
            }
            _ => {}
        }
    }

    // Sort columns and measures by name (consistent with other parsers)
    columns.sort_by(|a, b| a.name.cmp(&b.name));
    measures.sort_by(|a, b| a.name.cmp(&b.name));

    Some(TableInfo {
        name,
        ssas_name,
        description,
        columns,
        measures,
        partitions,
        hierarchies,
    })
}

fn build_column(child: &TmdlObject) -> Option<ColumnInfo> {
    let name = child.name.clone();
    let mut data_type = "string".to_string();
    let mut source_column = name.clone();
    let mut is_hidden = false;

    for (key, val) in &child.properties {
        match key.as_str() {
            "dataType" => data_type = val.clone(),
            "sourceColumn" => source_column = val.clone(),
            "isHidden" => {
                is_hidden = val == "true";
            }
            _ => {}
        }
    }

    Some(ColumnInfo {
        name,
        data_type,
        source_column,
        is_hidden,
    })
}

fn build_measure(child: &TmdlObject) -> Option<MeasureInfo> {
    let name = child.name.clone();
    let expression = child.expression.clone();
    let mut display_folder = String::new();

    for (key, val) in &child.properties {
        if key == "displayFolder" {
            display_folder = val.clone();
        }
    }

    let classification = if expression.is_empty() {
        "manual".to_string()
    } else {
        classify_dax(&expression)
    };

    Some(MeasureInfo {
        name,
        expression,
        display_folder,
        classification,
    })
}

fn build_partition(child: &TmdlObject, warnings: &mut Vec<String>) -> Option<PartitionInfo> {
    let name = child.name.clone();
    let source_type = child.expression.trim().to_string();
    let is_m = source_type == "m";

    // Capture mode and source expression from properties
    let mut mode: Option<String> = None;
    let mut source_expr: Option<String> = None;

    for (key, val) in &child.properties {
        if key == "mode" {
            let mode_lower = val.to_lowercase();
            if mode_lower == "directquery" || mode_lower == "dual" {
                warnings.push(format!(
                    "partition '{}' has mode '{}' — cannot be loaded into DuckDB",
                    name, val
                ));
            }
            mode = Some(val.clone());
        }
        if key == "source" {
            source_expr = Some(val.clone());
        }
    }

    // Also check children for source expression (TMDL `source = <expression>` syntax)
    for child_obj in &child.children {
        if child_obj.name == "source" && !child_obj.expression.is_empty() {
            source_expr = Some(child_obj.expression.clone());
        }
    }

    // If this is an "m" type partition and we have no source yet, check children
    // whose name is empty (inline source expression)
    if source_expr.is_none() {
        for child_obj in &child.children {
            if !child_obj.expression.is_empty() {
                source_expr = Some(child_obj.expression.clone());
                break;
            }
        }
    }

    Some(PartitionInfo {
        name,
        source_type,
        is_m,
        query: source_expr,
        data_source_name: None,
        mode,
        schema: None,
        database: None,
    })
}

/// Parse `Table.Column` or `'Table'.Column` dot notation.
fn parse_dot_notation(value: &str) -> Option<(String, String)> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with('\'') {
        // Quoted table name: 'Date'.DateKey
        let close_rel = value[1..].find('\'')?;
        let close_abs = 1 + close_rel; // absolute index of closing quote
        let table = value[1..close_abs].to_string();
        // After closing quote and dot
        let rest = value[close_abs + 2..].trim();
        let column = rest.to_string();
        Some((table, column))
    } else {
        // Unquoted: Sales.ProductKey — split on last dot
        let dot_pos = value.rfind('.')?;
        let table = value[..dot_pos].to_string();
        let column = value[dot_pos + 1..].to_string();
        Some((table, column))
    }
}

fn build_relationship(obj: &TmdlObject, warnings: &mut Vec<String>) -> Option<RelInfo> {
    let mut from_table = String::new();
    let mut from_column = String::new();
    let mut to_table = String::new();
    let mut to_column = String::new();

    for (key, val) in &obj.properties {
        match key.as_str() {
            "fromColumn" => {
                if let Some((t, c)) = parse_dot_notation(val) {
                    from_table = t;
                    from_column = c;
                } else {
                    warnings.push(format!(
                        "Could not parse fromColumn: '{}'",
                        val
                    ));
                }
            }
            "toColumn" => {
                if let Some((t, c)) = parse_dot_notation(val) {
                    to_table = t;
                    to_column = c;
                } else {
                    warnings.push(format!(
                        "Could not parse toColumn: '{}'",
                        val
                    ));
                }
            }
            _ => {}
        }
    }

    if from_table.is_empty() || to_table.is_empty() {
        warnings.push(format!(
            "Incomplete relationship '{}': fromColumn or toColumn missing",
            obj.name
        ));
        return None;
    }

    Some(RelInfo {
        from_table,
        from_column,
        to_table,
        to_column,
    })
}

fn build_role(obj: &TmdlObject, warnings: &mut Vec<String>) -> Option<RoleInfo> {
    let name = obj.name.clone();
    let description = obj.description.clone();

    // Extract modelPermission from properties (default "read")
    let mut model_permission = "read".to_string();
    for (key, val) in &obj.properties {
        if key == "modelPermission" {
            model_permission = val.clone();
        }
    }

    // Parse member children
    let mut members = Vec::new();
    for child in &obj.children {
        if child.object_type == "member" {
            let member_name = child.name.clone();
            let mut member_type = String::new();
            for (key, val) in &child.properties {
                if key == "memberType" {
                    member_type = val.clone();
                }
            }
            members.push(RoleMemberInfo {
                member_name,
                member_type,
            });
        }
    }

    // Parse tablePermission children
    let mut table_permissions = Vec::new();
    for child in &obj.children {
        if child.object_type == "tablePermission" {
            let table = child.name.clone();
            let dax_filter = if child.expression.is_empty() {
                None
            } else {
                Some(child.expression.clone())
            };
            let mut metadata_permission = "read".to_string();
            for (key, val) in &child.properties {
                if key == "metadataPermission" {
                    metadata_permission = val.clone();
                }
            }
            table_permissions.push(TablePermissionInfo {
                table,
                filter_expression: String::new(),
                dax_filter,
                metadata_permission,
            });
        }
    }

    Some(RoleInfo {
        name,
        description,
        model_permission,
        members,
        table_permissions,
    })
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Read all .tmdl files from a directory, parsing each and returning the objects.
fn read_tmdl_dir(dir: &Path, warnings: &mut Vec<String>) -> Vec<TmdlObject> {
    let mut objects = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "tmdl") {
                if let Ok(text) = fs::read_to_string(&path) {
                    objects.extend(parse_tmdl_file_text(&text, warnings));
                }
            }
        }
    }
    objects
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Parse a TMDL folder into a `TabularModel`.
///
/// Returns the model plus any warnings encountered during parsing.
///
/// Expects a directory containing `database.tmdl`, `tables/`, and optionally
/// `relationships.tmdl` and `roles/`.
pub fn parse_model(src_dir: &str) -> (TabularModel, Vec<String>) {
    let mut warnings: Vec<String> = Vec::new();
    let mut doc = TmdlDocument {
        objects: Vec::new(),
    };

    // ── database.tmdl ──
    let db_path = Path::new(src_dir).join("database.tmdl");
    if db_path.exists() {
        if let Ok(text) = fs::read_to_string(&db_path) {
            let objs = parse_tmdl_file_text(&text, &mut warnings);
            doc.objects.extend(objs);
        } else {
            warnings.push("failed to read database.tmdl".to_string());
        }
    } else {
        warnings.push("database.tmdl not found — using defaults".to_string());
    }

    // ── tables/*.tmdl ──
    let tables_dir = Path::new(src_dir).join("tables");
    if tables_dir.is_dir() {
        doc.objects.extend(read_tmdl_dir(&tables_dir, &mut warnings));
    } else {
        warnings.push("tables/ directory not found".to_string());
    }

    // ── relationships.tmdl ──
    let rel_path = Path::new(src_dir).join("relationships.tmdl");
    if rel_path.exists() {
        if let Ok(text) = fs::read_to_string(&rel_path) {
            let objs = parse_tmdl_file_text(&text, &mut warnings);
            doc.objects.extend(objs);
        } else {
            warnings.push("failed to read relationships.tmdl".to_string());
        }
    }

    // ── roles/*.tmdl ──
    let roles_dir = Path::new(src_dir).join("roles");
    if roles_dir.is_dir() {
        doc.objects.extend(read_tmdl_dir(&roles_dir, &mut warnings));
    }

    let result = build_model(doc, &mut warnings, src_dir);
    (result, warnings)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tmdl_fixture() {
        let path = "data/retailanalytics_tmdl";
        let (model, _warnings) = parse_model(path);

        // Check model identity
        assert_eq!(model.name, "Database");
        assert_eq!(model.compatibility_level, 1700);

        // 3 tables: Date, Product, Sales (sorted alphabetically)
        assert_eq!(model.tables.len(), 3, "expected 3 tables");

        let table_names: Vec<&str> = model.tables.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(table_names, vec!["Date", "Product", "Sales"]);

        // Verify Sales table
        let sales = model.tables.iter().find(|t| t.name == "Sales").unwrap();
        assert_eq!(sales.ssas_name, "Sales");
        assert_eq!(sales.description, "Sales transactions fact table");
        assert_eq!(sales.columns.len(), 3, "Sales should have 3 columns");
        let col_names: Vec<&str> = sales.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(col_names, vec!["OrderDate", "OrderKey", "ProductKey"]);
        assert_eq!(sales.measures.len(), 2, "Sales should have 2 measures");
        let meas_names: Vec<&str> = sales.measures.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(meas_names, vec!["Revenue", "Total Revenue"]);
        assert_eq!(sales.partitions.len(), 1, "Sales should have 1 partition");
        assert_eq!(sales.partitions[0].source_type, "m");
        assert!(sales.partitions[0].is_m);
        assert_eq!(sales.hierarchies.len(), 0, "Sales should have 0 hierarchies");

        // Verify Product table
        let product = model.tables.iter().find(|t| t.name == "Product").unwrap();
        assert_eq!(product.columns.len(), 2, "Product should have 2 columns");
        // Column with isKey
        let pk = product.columns.iter().find(|c| c.name == "ProductKey").unwrap();
        assert_eq!(pk.data_type, "int64");
        // Column with isHidden
        let pn = product.columns.iter().find(|c| c.name == "ProductName").unwrap();
        assert_eq!(pn.data_type, "string");
        assert!(pn.is_hidden, "ProductName should be hidden");

        // Verify Date table
        let date = model.tables.iter().find(|t| t.name == "Date").unwrap();
        assert_eq!(date.columns.len(), 2, "Date should have 2 columns");
        assert_eq!(date.hierarchies.len(), 1, "Date should have 1 hierarchy");
        assert_eq!(date.hierarchies[0], "Calendar Hierarchy");
        assert_eq!(date.partitions.len(), 1);

        // Verify relationships (sorted: Sales→Date, Sales→Product)
        assert_eq!(model.relationships.len(), 2, "expected 2 relationships");
        assert_eq!(model.relationships[0].from_table, "Sales");
        assert_eq!(model.relationships[0].to_table, "Date");
        assert_eq!(model.relationships[0].to_column, "DateKey");
        assert_eq!(model.relationships[1].from_table, "Sales");
        assert_eq!(model.relationships[1].to_table, "Product");
        assert_eq!(model.relationships[1].from_column, "ProductKey");

        // Verify roles
        assert_eq!(model.roles.len(), 1, "expected 1 role");
        assert_eq!(model.roles[0].name, "Reader");
        assert_eq!(model.roles[0].description, "Read-only access for analysts");

        // Verify measure expressions are flattened correctly
        let rev = sales.measures.iter().find(|m| m.name == "Revenue").unwrap();
        assert!(rev.expression.contains("CALCULATE"));
        assert_eq!(rev.classification, "simple");
        assert_eq!(rev.display_folder, "Financial Metrics");

        let total_rev = sales.measures.iter().find(|m| m.name == "Total Revenue").unwrap();
        assert!(total_rev.expression.contains("SUM"));
        assert!(total_rev.display_folder.is_empty(), "Total Revenue has no displayFolder");
        assert_eq!(total_rev.classification, "simple");
    }

    #[test]
    fn test_tmdl_quoted_name() {
        // Verify that 'Total Revenue' parses to name "Total Revenue"
        let path = "data/retailanalytics_tmdl";
        let (model, _warnings) = parse_model(path);
        let sales = model.tables.iter().find(|t| t.name == "Sales").unwrap();
        let has_quoted = sales.measures.iter().any(|m| m.name == "Total Revenue");
        assert!(has_quoted, "Should find 'Total Revenue' measure by unquoted name");
    }

    #[test]
    fn test_tmdl_relationship_dot_notation() {
        // Verify dot notation parsing
        let (t, c) = parse_dot_notation("Sales.ProductKey").unwrap();
        assert_eq!(t, "Sales");
        assert_eq!(c, "ProductKey");

        // Verify quoted table name
        let (t2, c2) = parse_dot_notation("'Date'.DateKey").unwrap();
        assert_eq!(t2, "Date");
        assert_eq!(c2, "DateKey");
    }

    #[test]
    fn test_tmdl_boolean_shortcut() {
        // Boolean shortcut 'isKey' (no value) should be parsed as property ("isKey", "true")
        let path = "data/retailanalytics_tmdl";
        let (model, _warnings) = parse_model(path);
        let product = model.tables.iter().find(|t| t.name == "Product").unwrap();
        // ProductKey should parse correctly with isKey shortcut (no crash, column exists)
        let pk = product.columns.iter().find(|c| c.name == "ProductKey").unwrap();
        assert_eq!(pk.name, "ProductKey");
        assert_eq!(pk.data_type, "int64");
        // isHidden should be false for ProductKey (isKey is not isHidden)
        assert!(!pk.is_hidden);
    }

    #[test]
    fn test_tmdl_multiline_expression() {
        // Verify multi-line measure expression is flattened correctly
        let text = "measure 'Test Meas' =\n\tSUM(Sales[Amount])\n\tformatString: #,##0\n";
        let mut warnings = Vec::new();
        let objs = parse_tmdl_file_text(text, &mut warnings);
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0].object_type, "measure");
        assert_eq!(objs[0].name, "Test Meas");
        // Expression should be flattened (continuations joined with space)
        assert_eq!(objs[0].expression, "SUM(Sales[Amount])");
        // formatString should be a property, not part of expression
        assert_eq!(objs[0].properties.len(), 1);
        assert_eq!(objs[0].properties[0].0, "formatString");
        assert_eq!(objs[0].properties[0].1, "#,##0");
    }

    #[test]
    fn test_tmdl_partition_inline_expression() {
        // Partition with inline = expression: partition Sales = m
        let text = "table T\n\tpartition Sales = m\n\t\tmode: import\n";
        let mut warnings = Vec::new();
        let objs = parse_tmdl_file_text(text, &mut warnings);
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0].object_type, "table");
        assert_eq!(objs[0].children.len(), 1);
        let part = &objs[0].children[0];
        assert_eq!(part.object_type, "partition");
        assert_eq!(part.name, "Sales");
        assert_eq!(part.expression, "m");
        assert_eq!(part.properties.len(), 1);
        assert_eq!(part.properties[0].0, "mode");
    }

    #[test]
    fn test_tmdl_tab_validation() {
        // Test that space indentation on the first non-blank line produces a warning
        let text = "  column X\n\tdataType: string\n";
        let mut warnings = Vec::new();
        let objs = parse_tmdl_file_text(text, &mut warnings);
        assert!(
            warnings.iter().any(|w| w.contains("space indentation")),
            "Should warn about space indentation, got: {:?}",
            warnings
        );
        // Should still parse despite the warning (be lenient)
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0].object_type, "column");
    }

    #[test]
    fn test_tmdl_backtick_expression() {
        // Verify that a backtick-delimited measure expression is captured correctly.
        // The declaration line has an empty expression (trailing =), then a backtick
        // block opens on the next line and closes before the property.
        let text = "measure 'Profit Margin' =\n\t```\n\tDIVIDE(\n\t\t[Total Revenue] - [Total Cost],\n\t\t[Total Revenue]\n\t)\n\t```\n\tformatString: 0.00%\n";
        let mut warnings = Vec::new();
        let objs = parse_tmdl_file_text(text, &mut warnings);
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0].object_type, "measure");
        assert_eq!(objs[0].name, "Profit Margin");
        // Expression should be the backtick content (trimmed, no outer backticks)
        let expected = "DIVIDE(\n\t\t[Total Revenue] - [Total Cost],\n\t\t[Total Revenue]\n\t)";
        assert_eq!(objs[0].expression, expected);
        // formatString should be a property, not part of expression
        assert_eq!(objs[0].properties.len(), 1);
        assert_eq!(objs[0].properties[0].0, "formatString");
        assert_eq!(objs[0].properties[0].1, "0.00%");
        assert!(warnings.is_empty(), "Expected no warnings, got: {:?}", warnings);
    }
}
