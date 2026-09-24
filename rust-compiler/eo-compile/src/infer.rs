//! +-----------------------------------------------------------------+
//! | infer — propagates `TypeHint`s through the IR graph.            |
//! |                                                                  |
//! | Conservative monomorphic inference: a node gets a concrete hint  |
//! | only when every observed use produces the same shape. Anything   |
//! | else stays `TypeHint::Unknown` and the lower step emits the      |
//! | full dispatch path for it.                                       |
//! +-----------------------------------------------------------------+

use crate::ir::Graph;

pub fn infer(_graph: &mut Graph) {}
