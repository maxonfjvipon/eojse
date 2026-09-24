//! +-----------------------------------------------------------------+
//! | lower — emits Rust source that calls `eo-rt` primitives.         |
//! |                                                                  |
//! | Today: handles the trivial fold-to-constant case via             |
//! | `eval::eval_constant_program`. Output is a single-file Rust      |
//! | program with no external dependencies — `rustc` can compile it  |
//! | directly. Anything that touches an atom or recurses bails out   |
//! | with a clear error so the caller knows to wait for the full     |
//! | dispatch-emitting lowering.                                      |
//! +-----------------------------------------------------------------+

use crate::eval::{delta_as_i64, eval_constant_program};
use crate::ir::Graph;

#[derive(Debug)]
pub struct LowerError {
    pub message: String,
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lower error: {}", self.message)
    }
}

impl std::error::Error for LowerError {}

pub fn lower(graph: &Graph) -> Result<String, LowerError> {
    let bytes = eval_constant_program(graph)
        .map_err(|e| LowerError { message: format!("constant-fold failed: {e}") })?;
    Ok(emit_constant_program(bytes))
}

fn emit_constant_program(bytes: &[u8]) -> String {
    let payload = match delta_as_i64(bytes) {
        Some(n) => format!("println!(\"data: {{}}\", {n}i64);"),
        None => {
            let pretty = bytes
                .iter()
                .map(|b| format!("0x{b:02x}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("println!(\"data: ({} bytes) [{}]\");", bytes.len(), pretty)
        }
    };
    format!(
        "fn main() {{\n    {payload}\n}}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build, parse};

    fn lower_dsl(path: &str) -> Result<String, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let lines = parse::parse(&text).map_err(|e| e.to_string())?;
        let graph = build::build(&lines).map_err(|e| e.to_string())?;
        lower(&graph).map_err(|e| e.to_string())
    }

    #[test]
    fn lowers_simple_to_a_main_printing_42() {
        let src = lower_dsl(
            "/Users/maxonfjvipon/code/javascript/eo-runtime/temp/simple/simple.dsl",
        )
        .expect("simple.dsl must lower to a constant program");
        assert!(
            src.contains("42i64"),
            "lowered simple.dsl must contain the literal 42i64, got:\n{src}"
        );
        assert!(
            src.contains("fn main"),
            "lowered program must declare a main fn, got:\n{src}"
        );
    }

    #[test]
    fn cannot_lower_program_with_atom_call() {
        let err = lower_dsl(
            "/Users/maxonfjvipon/code/javascript/eo-runtime/temp/eleven/eleven.dsl",
        )
        .expect_err("eleven.dsl must not constant-fold today");
        assert!(
            err.contains("constant-fold"),
            "atom-bearing program must fail with a constant-fold error, got {err}"
        );
    }
}
