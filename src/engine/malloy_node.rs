/// Malloy compiler that delegates to a Node.js subprocess.
///
/// Reads Malloy source from the semantic model + query plan,
/// writes it to the Node process, and reads compiled SQL back.
///
/// For the feasibility spike. A long-lived process or rquickjs
/// would be more efficient, but this proves the compilation path.

use std::io::Write;
use std::process::{Command, Stdio};
use crate::engine::malloy_compiler::{CompileResult, MalloyCompiler, MalloyCompileError};

pub struct NodeMalloyCompiler;

impl NodeMalloyCompiler {
    /// Full path to the malloy-cli.js script, relative to the project root
    /// or provided by an env var.
    fn script_path() -> String {
        std::env::var("MALLOY_CLI_PATH")
            .unwrap_or_else(|_| "js/malloy-cli.js".into())
    }
}

impl MalloyCompiler for NodeMalloyCompiler {
    fn compile_malloy(&self, source: &str) -> Result<CompileResult, MalloyCompileError> {
        let mut child = Command::new("node")
            .arg(Self::script_path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| MalloyCompileError::RuntimeError(format!("spawn node: {e}")))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(source.as_bytes())
                .map_err(|e| MalloyCompileError::RuntimeError(format!("write stdin: {e}")))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| MalloyCompileError::RuntimeError(format!("wait node: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MalloyCompileError::RuntimeError(stderr.into_owned()));
        }

        let sql = String::from_utf8_lossy(&output.stdout).into_owned();
        Ok(CompileResult { sql: sql.trim().to_string(), compile_ms: 0.0 })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::default_model;
    use crate::engine::plan::{Dimension, Measure, QueryPlan};

    #[test]
    fn compile_total_query() {
        let model = default_model();
        let plan = QueryPlan::Total {
            measure: Measure::TotalSales,
            filters: vec![],
        };
        let compiler = NodeMalloyCompiler;
        let r = compiler.compile_query(&model, &plan).expect("compile total");
        assert!(!r.sql.is_empty());
        assert!(r.sql.to_uppercase().contains("SUM"));
    }

    #[test]
    fn compile_group_by_one_dim() {
        let model = default_model();
        let plan = QueryPlan::GroupBy {
            measure: Measure::TotalSales,
            group_by: vec![Dimension::Produktkategori],
            filters: vec![],
        };
        let compiler = NodeMalloyCompiler;
        let r = compiler
            .compile_query(&model, &plan)
            .expect("compile groupby 1d");
        assert!(!r.sql.is_empty());
        assert!(r.sql.contains("GROUP BY"));
    }

    #[test]
    fn compile_group_by_two_dims() {
        let model = default_model();
        let plan = QueryPlan::GroupBy {
            measure: Measure::TotalSales,
            group_by: vec![Dimension::Produktkategori, Dimension::Region],
            filters: vec![],
        };
        let compiler = NodeMalloyCompiler;
        let r = compiler
            .compile_query(&model, &plan)
            .expect("compile groupby 2d");
        assert!(!r.sql.is_empty());
        assert!(r.sql.contains("GROUP BY"));
    }

    #[test]
    fn compile_filtered_query() {
        let model = default_model();
        let plan = QueryPlan::GroupBy {
            measure: Measure::TotalSales,
            group_by: vec![Dimension::Produktkategori],
            filters: vec![
                crate::engine::plan::TypedDimensionFilter {
                    dimension: Dimension::Region,
                    members: vec!["North".into()],
                },
            ],
        };
        let compiler = NodeMalloyCompiler;
        let r = compiler
            .compile_query(&model, &plan)
            .expect("compile filtered");
        assert!(!r.sql.is_empty());
        assert!(r.sql.to_uppercase().contains("WHERE"));
    }

    #[test]
    fn compile_rejects_count() {
        let model = default_model();
        let plan = QueryPlan::Count {
            dimension: Dimension::Produktkategori,
        };
        let compiler = NodeMalloyCompiler;
        assert!(compiler.compile_query(&model, &plan).is_err());
    }
}
