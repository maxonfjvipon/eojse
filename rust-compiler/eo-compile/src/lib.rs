//! +-----------------------------------------------------------------+
//! | eo-compile — ahead-of-time compiler from EO DSL to Rust source.  |
//! |                                                                  |
//! | Pipeline (planned):                                               |
//! |                                                                  |
//! |   DSL text                                                       |
//! |     -> parse (reuses eo-runtime::loader)                          |
//! |     -> IR  (ir.rs)        — typed graph of formations/dispatches |
//! |     -> infer (infer.rs)   — assigns shapes and value types        |
//! |     -> shapes (shapes.rs) — interns hidden classes                |
//! |     -> lower (lower.rs)   — emits Rust source that calls eo-rt    |
//! |     -> rustc              — produces a native binary              |
//! |                                                                  |
//! | The output binary links only against `eo-rt`; the interpreter    |
//! | crate is a build-time dependency, never a runtime one.           |
//! +-----------------------------------------------------------------+

pub mod build;
pub mod eval;
pub mod infer;
pub mod ir;
pub mod lower;
pub mod parse;
pub mod shapes;
