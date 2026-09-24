//! +-----------------------------------------------------------------+
//! | eval — compile-time partial evaluator over the IR graph.        |
//! |                                                                  |
//! | Used by `lower` to fold programs whose entire result is a        |
//! | constant Δ into a single literal. Mirrors the runtime's morph/   |
//! | dataize semantics but only handles the cases that show up in a  |
//! | constant fold: Formation, Dispatch, Application, Context, and Δ.|
//! | Any reachable atom (`λ:...`) or cycle terminates with an error  |
//! | so the caller falls back to the slow lowering path.             |
//! +-----------------------------------------------------------------+

use std::collections::HashMap;

use crate::ir::{AttrRef, AttrSpec, Graph, Node, NodeId, Target};

#[derive(Clone)]
pub enum Val<'g> {
    Form { id: NodeId, bindings: HashMap<String, NodeId> },
    Bytes(&'g [u8]),
}

const MAX_STEPS: usize = 1024;

pub fn eval_constant_program(graph: &Graph) -> Result<&[u8], String> {
    if graph.nodes.is_empty() {
        return Err("graph is empty".into());
    }
    let root = NodeId(0);
    match &graph.nodes[0] {
        Node::Formation { .. } => {}
        other => return Err(format!("root node must be a Formation, got {other:?}")),
    }
    let initial = Val::Form { id: root, bindings: HashMap::new() };
    let phi = lookup_attr_target(graph, root, &HashMap::new(), "φ")?;
    let mut steps = 0usize;
    let mut val = morph(graph, phi, &initial, &mut steps)?;
    loop {
        match val {
            Val::Bytes(b) => return Ok(b),
            Val::Form { id, ref bindings } => {
                if has_delta(graph, id) {
                    return Ok(delta_bytes(graph, id).expect("has_delta lied"));
                }
                let phi_target = lookup_attr_target(graph, id, bindings, "φ")?;
                let ctx = val.clone();
                val = morph(graph, phi_target, &ctx, &mut steps)?;
            }
        }
    }
}

fn morph<'g>(
    graph: &'g Graph,
    node: NodeId,
    ctx: &Val<'g>,
    steps: &mut usize,
) -> Result<Val<'g>, String> {
    *steps += 1;
    if *steps > MAX_STEPS {
        return Err(format!("eval exceeded {MAX_STEPS} steps; not a constant program"));
    }
    match &graph.nodes[node.0 as usize] {
        Node::Formation { attrs, .. } => {
            for (name, spec) in attrs {
                if name == "Δ" {
                    if let AttrSpec::Delta(bytes) = spec {
                        return Ok(Val::Bytes(bytes));
                    }
                }
                if name == "λ" {
                    return Err(format!(
                        "node {node:?} carries an atom binding; constant fold cannot reach atoms"
                    ));
                }
            }
            Ok(Val::Form { id: node, bindings: HashMap::new() })
        }
        Node::Context => Ok(ctx.clone()),
        Node::Dispatch { target, attr } => {
            let tgt = match target {
                Target::Object(id) => morph(graph, *id, ctx, steps)?,
                Target::Sentinel => ctx.clone(),
            };
            let (form_id, bindings) = match &tgt {
                Val::Form { id, bindings } => (*id, bindings.clone()),
                Val::Bytes(_) => {
                    return Err(format!(
                        "dispatch {node:?} target produced Δ bytes; cannot take an attr from bytes"
                    ));
                }
            };
            let attr_name = attr_ref_to_name(graph, form_id, attr)?;
            let next = lookup_attr_target(graph, form_id, &bindings, &attr_name)?;
            morph(graph, next, &tgt, steps)
        }
        Node::Application { target, attr, value } => {
            let tgt = match target {
                Target::Object(id) => morph(graph, *id, ctx, steps)?,
                Target::Sentinel => ctx.clone(),
            };
            let (form_id, mut bindings) = match tgt {
                Val::Form { id, bindings } => (id, bindings),
                Val::Bytes(_) => {
                    return Err(format!(
                        "application {node:?} target produced Δ bytes; cannot bind a slot of bytes"
                    ));
                }
            };
            let attr_name = attr_ref_to_name(graph, form_id, attr)?;
            bindings.insert(attr_name, *value);
            Ok(Val::Form { id: form_id, bindings })
        }
    }
}

fn lookup_attr_target(
    graph: &Graph,
    form_id: NodeId,
    bindings: &HashMap<String, NodeId>,
    attr_name: &str,
) -> Result<NodeId, String> {
    if let Some(&id) = bindings.get(attr_name) {
        return Ok(id);
    }
    let attrs = formation_attrs(graph, form_id)?;
    for (name, spec) in attrs {
        if name == attr_name {
            return match spec {
                AttrSpec::Ref { id, .. } => Ok(*id),
                AttrSpec::Void => Err(format!(
                    "attr {attr_name:?} on node {form_id:?} is void and not bound"
                )),
                AttrSpec::Delta(_) => Err(format!(
                    "attr {attr_name:?} on node {form_id:?} is a Δ literal, not a ref"
                )),
                AttrSpec::Atom(_) => Err(format!(
                    "attr {attr_name:?} on node {form_id:?} is an atom; cannot constant fold"
                )),
            };
        }
    }
    Err(format!(
        "attr {attr_name:?} not found on formation {form_id:?}"
    ))
}

fn attr_ref_to_name(graph: &Graph, form_id: NodeId, attr: &AttrRef) -> Result<String, String> {
    match attr {
        AttrRef::Name(s) => Ok(s.clone()),
        AttrRef::Slot(n) => {
            let attrs = formation_attrs(graph, form_id)?;
            attrs.get(*n as usize).map(|(name, _)| name.clone()).ok_or_else(|| {
                format!("slot {n} out of range on formation {form_id:?} ({} attrs)", attrs.len())
            })
        }
    }
}

fn formation_attrs(graph: &Graph, form_id: NodeId) -> Result<&Vec<(String, AttrSpec)>, String> {
    match &graph.nodes[form_id.0 as usize] {
        Node::Formation { attrs, .. } => Ok(attrs),
        other => Err(format!("node {form_id:?} is not a Formation ({other:?})")),
    }
}

fn has_delta(graph: &Graph, form_id: NodeId) -> bool {
    let attrs = match formation_attrs(graph, form_id) {
        Ok(a) => a,
        Err(_) => return false,
    };
    attrs.iter().any(|(n, s)| n == "Δ" && matches!(s, AttrSpec::Delta(_)))
}

fn delta_bytes<'g>(graph: &'g Graph, form_id: NodeId) -> Option<&'g [u8]> {
    let attrs = formation_attrs(graph, form_id).ok()?;
    for (name, spec) in attrs {
        if name == "Δ" {
            if let AttrSpec::Delta(b) = spec {
                return Some(b);
            }
        }
    }
    None
}

/// Interpret 8-byte big-endian f64 Δ bytes as an integer when exact.
pub fn delta_as_i64(bytes: &[u8]) -> Option<i64> {
    if bytes.len() != 8 {
        return None;
    }
    let arr: [u8; 8] = bytes.try_into().ok()?;
    let f = f64::from_be_bytes(arr);
    if !f.is_finite() {
        return None;
    }
    let n = f as i64;
    if (n as f64) == f {
        Some(n)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build, parse};

    fn eval_dsl(text: &str) -> Result<Vec<u8>, String> {
        let lines = parse::parse(text).map_err(|e| e.to_string())?;
        let graph = build::build(&lines).map_err(|e| e.to_string())?;
        eval_constant_program(&graph).map(|b| b.to_vec())
    }

    #[test]
    fn folds_simple_program_to_42() {
        let dsl = std::fs::read_to_string(
            "/Users/maxonfjvipon/code/javascript/eo-runtime/temp/simple/simple.dsl",
        )
        .expect("simple.dsl must exist (run `npm test` to regenerate)");
        let bytes = eval_dsl(&dsl).expect("simple.dsl must constant-fold");
        let n = delta_as_i64(&bytes).expect("simple's Δ must decode as integer");
        assert_eq!(n, 42, "simple.dsl must fold to 42, got {n}");
    }

    #[test]
    fn folds_dollar_program_to_42() {
        let dsl = std::fs::read_to_string(
            "/Users/maxonfjvipon/code/javascript/eo-runtime/temp/dollar/dollar.dsl",
        )
        .expect("dollar.dsl must exist (run `npm test` to regenerate)");
        let bytes = eval_dsl(&dsl).expect("dollar.dsl must constant-fold");
        let n = delta_as_i64(&bytes).expect("dollar's Δ must decode as integer");
        assert_eq!(n, 42, "dollar.dsl must fold to 42, got {n}");
    }

    #[test]
    fn cannot_fold_program_that_reaches_an_atom() {
        let dsl = std::fs::read_to_string(
            "/Users/maxonfjvipon/code/javascript/eo-runtime/temp/eleven/eleven.dsl",
        )
        .expect("eleven.dsl must exist (run `npm test` to regenerate)");
        let err = eval_dsl(&dsl).expect_err("eleven calls L_number_plus and cannot fold");
        assert!(
            err.contains("atom") || err.contains("steps"),
            "eleven must fail with an atom-related error, got {err}"
        );
    }
}
