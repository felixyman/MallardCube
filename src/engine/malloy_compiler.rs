use crate::engine::malloy::malloy_source_for_query_plan;
use crate::engine::model::SemanticModel;
use crate::engine::plan::QueryPlan;
/// Malloy compiler abstraction.
///
/// Defines a trait for compiling Malloy source text into executable SQL.
/// Decouples the semantic pipeline from the compilation runtime.
///
/// Implementations:
/// - `NullCompiler` — returns the Malloy source as-is (for testing).
/// - `DenoCoreCompiler` — embedded V8-based compiler (future).
use std::fmt;

#[derive(Debug)]
pub enum MalloyCompileError {
    UnsupportedPlan,
    EmitError(String),
    RuntimeError(String),
}

impl fmt::Display for MalloyCompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MalloyCompileError::UnsupportedPlan => write!(f, "unsupported plan"),
            MalloyCompileError::EmitError(e) => write!(f, "emit error: {e}"),
            MalloyCompileError::RuntimeError(e) => write!(f, "runtime error: {e}"),
        }
    }
}

pub struct CompileResult {
    pub sql: String,
    /// JS-side compile duration in milliseconds (from worker or 0.0 if N/A).
    pub compile_ms: f64,
}

pub trait MalloyCompiler {
    /// Compile a Malloy source string into SQL with JS-side timing.
    fn compile_malloy(&self, source: &str) -> Result<CompileResult, MalloyCompileError>;

    /// Convenience: emit Malloy from plan + model, then compile.
    fn compile_query(
        &self,
        model: &SemanticModel,
        plan: &QueryPlan,
    ) -> Result<CompileResult, MalloyCompileError> {
        if matches!(plan, QueryPlan::Count { .. } | QueryPlan::Empty) {
            return Err(MalloyCompileError::UnsupportedPlan);
        }
        let source = malloy_source_for_query_plan(model, plan);
        self.compile_malloy(&source)
    }
}

// ---------------------------------------------------------------------------
// Null compiler — returns source as-is (for testing the compile pipeline)
// ---------------------------------------------------------------------------

pub struct NullCompiler;

impl MalloyCompiler for NullCompiler {
    fn compile_malloy(&self, source: &str) -> Result<CompileResult, MalloyCompileError> {
        Ok(CompileResult {
            sql: format!("-- null compiler, source follows:\n{source}"),
            compile_ms: 0.0,
        })
    }
}
