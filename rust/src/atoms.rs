//! Built-in atoms. Names match the JS prototype's `L_number_*` keys.
//!
//! Atoms call back into the runtime via `morph` / `dataize`. They hold their
//! intermediate offsets in plain Rust locals, which is why phi-GC is gated to
//! the main dataization thread (`GC_ENABLED_DEFAULT = false`).

use crate::block::*;
use crate::runtime::{AtomFn, Runtime};

/// (left + right). Mirrors L_number_plus from program.js.
pub fn number_plus(rt: &mut Runtime, self_off: u32) -> u32 {
    arithmetic_atom(rt, self_off, |a, b| a + b)
}

/// (left * right). Mirrors L_number_times.
pub fn number_times(rt: &mut Runtime, self_off: u32) -> u32 {
    arithmetic_atom(rt, self_off, |a, b| a * b)
}

/// (left > right) → true/false formation. Mirrors L_number_gt.
pub fn number_gt(rt: &mut Runtime, self_off: u32) -> u32 {
    let scope = rt.default_scope();

    let rho_dsp = rt.push_dsp(self_off, ID_RHO);
    let m_left = rt.morph(rho_dsp, self_off, true);
    let left = rt.dataize_to_f64(m_left, scope, false);

    let x_dsp = rt.push_dsp(self_off, ID_X);
    let m_right = rt.morph(x_dsp, self_off, true);
    let right = rt.dataize_to_f64(m_right, scope, false);

    rt.make_bool(left > right)
}

fn arithmetic_atom(rt: &mut Runtime, self_off: u32, op: impl Fn(f64, f64) -> f64) -> u32 {
    let scope = rt.default_scope();

    let rho_dsp = rt.push_dsp(self_off, ID_RHO);
    let m_left = rt.morph(rho_dsp, self_off, true);
    let left = rt.dataize_to_f64(m_left, scope, false);

    let x_dsp = rt.push_dsp(self_off, ID_X);
    let m_right = rt.morph(x_dsp, self_off, true);
    let right = rt.dataize_to_f64(m_right, scope, false);

    rt.make_number(op(left, right))
}

/// Build the atom dispatch table, mapping the loader's interned atom ids to
/// runtime function pointers. Panics if the program references an unknown atom.
pub fn atom_fns_for(atom_names: &[String]) -> Vec<AtomFn> {
    atom_names
        .iter()
        .map(|name| -> AtomFn {
            match name.as_str() {
                "L_number_plus" => number_plus,
                "L_number_times" => number_times,
                "L_number_gt" => number_gt,
                other => panic!("unknown atom: {other}"),
            }
        })
        .collect()
}
