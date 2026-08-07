/// Extract unique ExecuteStatement MDX from xmla-trace.jsonl and emit
/// Rust const declarations ready for pasting into src/test_support/fixtures.rs.
///
/// Usage: cargo run --bin extract_trace_mdx [-- xmla-trace.jsonl]
///
/// The output goes to stdout in a format suitable for appending to
/// `EXCEL_TRACE_PROJECT3_EXECUTES` and/or adding new named constants.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn run(args: Vec<String>) -> i32 {
    let path = args.get(1).cloned().unwrap_or_else(|| "xmla-trace.jsonl".into());
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot open {path}: {e}");
            return 1;
        }
    };

    let mut unique: BTreeSet<String> = BTreeSet::new();
    for line in BufReader::new(file).lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("request_kind").and_then(|k| k.as_str()) == Some("ExecuteStatement") {
            if let Some(mdx) = v.get("mdx").and_then(|m| m.as_str()) {
                unique.insert(mdx.to_string());
            }
        }
    }

    let mut idx = 0usize;
    for mdx in &unique {
        let annotation = detect_shape(mdx);
        let name = if annotation.is_empty() {
            format!("EXCEL_TRACE_{idx:03}")
        } else {
            format!("EXCEL_TRACE_{idx:03}__{annotation}")
        };
        println!(
            "// {annotation}\npub const {name}: &str = r#####\"{mdx}\"#####;\n",
        );
        idx += 1;
    }

    if unique.is_empty() {
        eprintln!("No ExecuteStatement entries found in {path}");
        return 1;
    }

    eprintln!("{} unique ExecuteStatement MDX strings extracted from {path}", unique.len());
    0
}

fn detect_shape(mdx: &str) -> String {
    let mut tags: Vec<String> = Vec::new();

    // Shape
    if mdx.contains("CrossJoin") {
        tags.push("crossjoin".into());
    }
    if mdx.contains("DrilldownMember") {
        tags.push("collapse".into());
    }
    if mdx.contains("DrilldownLevel") && !mdx.contains("CrossJoin") {
        tags.push("drilldown".into());
    }
    if mdx.contains("cChildren") {
        tags.push("cchildren".into());
    }
    if mdx.contains(".Members") && !mdx.contains("AllMembers") {
        tags.push("members_probe".into());
    }
    if mdx.contains(".Children") {
        tags.push("children_probe".into());
    }

    // Row-axis dimensions — only scan the SELECT clause (before FROM) so that
    // WHERE / slicer filter dimensions are not mis-tagged as row dimensions.
    let select_part = mdx.split(" FROM ").next().unwrap_or("");
    for dim in &["Territory", "Category", "Channel", "Segment"] {
        let dl = dim.to_lowercase();
        let pattern = format!("[{}].[{}].", dim, dim);
        if select_part.contains(&pattern) && !tags.iter().any(|t: &String| t == &dl) {
            tags.push(dl);
        }
    }

    // Measures
    if let Some(rest) = mdx.split("[Measures].").nth(1) {
        let meas = rest.split(']').next().unwrap_or("").trim_start_matches('[');
        if !meas.is_empty() && !meas.starts_with("cChildren") {
            tags.push(meas.to_lowercase());
        }
    }

    // Leaf filters in WHERE (skip [All] since those are no-ops)
    if let Some(where_part) = mdx.split("WHERE (").nth(1) {
        let where_content = where_part.split(')').next().unwrap_or("");
        for dim in &["Territory", "Category", "Channel", "Segment"] {
            let marker = format!("[{}].[{}].&[", dim, dim);
            if where_content.contains(&marker) {
                tags.push(format!("filt_{}", dim.to_lowercase()));
            }
        }
    }

    // Subquery filters
    if mdx.contains("(SELECT ({") {
        tags.push("subq_filter".into());
    }

    tags.join("_")
}
