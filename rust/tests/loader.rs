use std::fs;
use std::path::PathBuf;

use eo_runtime::block::*;
use eo_runtime::loader::{self, Image};

const TEST_CAP: usize = 1 << 20; // 1 MB heap-backed buffer per test

fn dsl_path(name: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("temp")
        .join(name)
        .join(format!("{name}.dsl"));
    if !p.exists() {
        panic!("missing fixture {p:?}; run `npm test` to regenerate temp/*.dsl");
    }
    p
}

fn fixture_buf() -> Vec<u8> {
    vec![0u8; TEST_CAP]
}

fn load_into<'a>(buf: &'a mut [u8], name: &str) -> Image<'a> {
    let src = fs::read_to_string(dsl_path(name)).expect("read dsl");
    loader::load(&src, buf)
}

#[test]
fn rec_loads() {
    let mut buf = fixture_buf();
    let img = load_into(&mut buf, "rec");
    assert_eq!(img.program_objects, 67);
    assert_eq!(img.memory.len(), 2032);
    // Arena dedup folds the duplicate 1.0 literal: 5.0 + 1.0 + (-1.0) + true + false
    // = 8 + 8 + 8 + 1 + 1 = 26 B (vs 34 B undedup'd).
    assert_eq!(img.arena.len(), 26);
    assert_eq!(img.memory.ty(0), TY_FRM);
    let q_attrs = img.memory.frm_attr_count(0);
    assert_eq!(q_attrs, 7, "Q should have 6 declared + 1 implicit ρ");
    assert!(img.attrs.id_of("rec").is_some(), "user attr 'rec' should be interned");
    assert_eq!(img.attrs.id_of(STR_PHI), Some(ID_PHI));
}

#[test]
fn fibo_loads() {
    let mut buf = fixture_buf();
    let img = load_into(&mut buf, "fibo");
    assert_eq!(img.program_objects, 80);
    assert_eq!(img.memory.len(), 2272);
    assert_eq!(img.memory.ty(0), TY_FRM);
    assert!(img.attrs.id_of("fibo").is_some());
}

#[test]
fn dollar_loads_with_user_ctx() {
    let mut buf = fixture_buf();
    let img = load_into(&mut buf, "dollar");
    let mut ctx_count = 0u32;
    let mut off = 0u32;
    while off < img.memory.len() {
        if img.memory.ty(off) == TY_CTX {
            ctx_count += 1;
        }
        off += img.memory.block_size_at(off);
    }
    assert!(ctx_count >= 2, "expected ≥2 CTX blocks (user + stdlib), got {ctx_count}");
    assert!(img.attrs.id_of("self").is_some(), "user attr 'self' should be interned");
}

#[test]
fn dsp_attr_is_concrete_after_ctx() {
    let mut buf = fixture_buf();
    let img = load_into(&mut buf, "rec");
    let mut off = 0u32;
    while off < img.memory.len() {
        if img.memory.ty(off) == TY_DSP {
            let attr = img.memory.n(off);
            assert_ne!(attr, u16::MAX, "DSP at offset {off} has sentinel attr");
        }
        off += img.memory.block_size_at(off);
    }
}

#[test]
fn ctx_block_size_is_8() {
    let mut buf = fixture_buf();
    let img = load_into(&mut buf, "dollar");
    let mut off = 0u32;
    while off < img.memory.len() {
        if img.memory.ty(off) == TY_CTX {
            assert_eq!(img.memory.block_size_at(off), 8);
        }
        off += img.memory.block_size_at(off);
    }
}

#[test]
fn id_to_off_translates_consistently() {
    let mut buf = fixture_buf();
    let img = load_into(&mut buf, "rec");
    let mut walk_off = 0u32;
    for (id, &off) in img.id_to_off.iter().enumerate() {
        assert_eq!(off, walk_off, "id {id}: walker at {walk_off}, map says {off}");
        walk_off += img.memory.block_size_at(off);
    }
    assert_eq!(walk_off, img.memory.len(), "walk consumes all of memory");
}

#[test]
fn arena_holds_delta_payload() {
    let mut buf = fixture_buf();
    let img = load_into(&mut buf, "dollar");
    assert!(img.arena.len() > 0, "dollar should have at least one Δ payload");
    assert_eq!(img.arena.slice(0, 1).len(), 1);
}

#[test]
fn buffer_capacity_exposed() {
    let mut buf = fixture_buf();
    let img = load_into(&mut buf, "simple");
    assert_eq!(img.memory.capacity(), TEST_CAP as u32);
}

#[test]
fn arena_dedups_duplicate_literal() {
    let mut buf = fixture_buf();
    let img = load_into(&mut buf, "dup");
    // dup.eo has the literal 1.0 twice; arena should fold it.
    // Payloads: 1.0 + (-1.0) + true + false = 8+8+1+1 = 18 B.
    assert_eq!(img.arena.len(), 18);
    assert!(img.arena.dedup_hits() >= 1, "expected at least 1 dedup hit at load");
}
