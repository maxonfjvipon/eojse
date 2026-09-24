//! +-----------------------------------------------------------------+
//! | build — converts the parser's RawLine stream into an IR Graph.  |
//! |                                                                  |
//! | The parser guarantees lines are emitted in id order and that    |
//! | ids are dense starting at 0; this stage asserts that invariant  |
//! | once, then walks the lines and constructs one `Node` per line.   |
//! | All ref translations (parse's `u32` ids -> ir's `NodeId`,       |
//! | parse's `FromRef` -> ir's `Target`) happen here.                 |
//! +-----------------------------------------------------------------+

use crate::ir::{AttrRef, AttrSpec, Graph, Node, NodeId, Target};
use crate::parse::{self, RawLine};

#[derive(Debug)]
pub struct BuildError {
    pub message: String,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "build error: {}", self.message)
    }
}

impl std::error::Error for BuildError {}

pub fn build(lines: &[RawLine]) -> Result<Graph, BuildError> {
    for (i, line) in lines.iter().enumerate() {
        if line.id() as usize != i {
            return Err(BuildError {
                message: format!(
                    "line #{i} declares id {} (parser must emit dense, ordered ids)",
                    line.id()
                ),
            });
        }
    }
    let mut graph = Graph::new();
    for line in lines {
        graph.push(node_from(line));
    }
    Ok(graph)
}

fn node_from(line: &RawLine) -> Node {
    match line {
        RawLine::Form(f) => Node::Formation {
            name: f.name.clone(),
            attrs: f
                .attrs
                .iter()
                .map(|(name, spec)| (name.clone(), spec_from(spec)))
                .collect(),
        },
        RawLine::Disp(d) => Node::Dispatch {
            target: target_from(&d.from),
            attr: attr_from(&d.attr),
        },
        RawLine::App(a) => Node::Application {
            target: target_from(&a.from),
            attr: attr_from(&a.attr),
            value: NodeId(a.value),
        },
        RawLine::Ctx(_) => Node::Context,
    }
}

fn target_from(from: &parse::FromRef) -> Target {
    match from {
        parse::FromRef::Object(id) => Target::Object(NodeId(*id)),
        parse::FromRef::Sentinel => Target::Sentinel,
    }
}

fn attr_from(attr: &parse::AttrRef) -> AttrRef {
    match attr {
        parse::AttrRef::Slot(n) => AttrRef::Slot(*n),
        parse::AttrRef::Name(s) => AttrRef::Name(s.clone()),
    }
}

fn spec_from(spec: &parse::AttrSpec) -> AttrSpec {
    match spec {
        parse::AttrSpec::Void => AttrSpec::Void,
        parse::AttrSpec::Ref { id, cached } => AttrSpec::Ref { id: NodeId(*id), cached: *cached },
        parse::AttrSpec::Delta(bytes) => AttrSpec::Delta(bytes.clone()),
        parse::AttrSpec::Atom(name) => AttrSpec::Atom(name.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{NodeKind, Target as IrTarget};
    use crate::parse::parse;

    #[test]
    fn builds_graph_for_small_program() {
        let text = "FORM 0 Φ φ:2!\nFORM 1 anon Δ:01-02\nCTX 2";
        let lines = parse(text).expect("parse must succeed");
        let g = build(&lines).expect("build must succeed");
        assert_eq!(
            g.nodes.len(),
            3,
            "graph must have one node per DSL statement"
        );
    }

    #[test]
    fn ctx_line_becomes_context_node() {
        let lines = parse("CTX 0").expect("parse must succeed");
        let g = build(&lines).expect("build must succeed");
        assert_eq!(
            g.nodes[0].kind(),
            NodeKind::Context,
            "CTX line cannot map to anything other than Context"
        );
    }

    #[test]
    fn dispatch_with_minus_one_uses_sentinel_target() {
        let lines = parse("DISP 0 -1 x").expect("parse must succeed");
        let g = build(&lines).expect("build must succeed");
        match &g.nodes[0] {
            Node::Dispatch { target, .. } => {
                assert_eq!(*target, IrTarget::Sentinel, "DISP -1 must yield Sentinel target");
            }
            other => panic!("expected Dispatch, got {other:?}"),
        }
    }

    #[test]
    fn application_translates_value_node_id() {
        let text = "FORM 0 anon\nFORM 1 anon\nAPP 2 0 0 1";
        let lines = parse(text).expect("parse must succeed");
        let g = build(&lines).expect("build must succeed");
        match &g.nodes[2] {
            Node::Application { value, .. } => {
                assert_eq!(*value, NodeId(1), "APP value field must become a NodeId");
            }
            other => panic!("expected Application, got {other:?}"),
        }
    }

    #[test]
    fn cannot_build_when_ids_are_not_dense() {
        let text = "FORM 0 Φ\nFORM 5 anon";
        let lines = parse(text).expect("parse must succeed");
        let err = build(&lines).expect_err("non-dense ids must fail");
        assert!(
            err.message.contains("dense"),
            "non-dense id error must say so"
        );
    }
}
