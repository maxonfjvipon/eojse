use std::fs;
use std::path::PathBuf;

use eo_runtime::atoms;
use eo_runtime::loader;
use eo_runtime::runtime::Runtime;

const TEST_CAP: usize = 1 << 20; // 1 MB heap-backed buffer

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

fn dataize_program(name: &str) -> i64 {
    let mut buf = vec![0u8; TEST_CAP];
    let src = fs::read_to_string(dsl_path(name)).expect("read dsl");
    let img = loader::load(&src, &mut buf);
    let atom_fns = atoms::atom_fns_for(&img.atoms.rev);
    let mut rt = Runtime::from_image(img, atom_fns);
    let (off, len) = rt.dataize(0, rt.default_scope(), true);
    assert_eq!(len, 8, "expected 8-byte f64 payload");
    let bytes = rt.arena.slice(off, len);
    let arr: [u8; 8] = bytes.try_into().unwrap();
    let n = f64::from_be_bytes(arr);
    assert!(n.is_finite() && n == (n as i64) as f64, "non-integer result: {n}");
    n as i64
}

#[test]
fn simple_returns_42() {
    assert_eq!(dataize_program("simple"), 42);
}

#[test]
fn foo_returns_35() {
    assert_eq!(dataize_program("foo"), 35);
}

#[test]
fn eleven_returns_11() {
    assert_eq!(dataize_program("eleven"), 11);
}

#[test]
fn dollar_returns_42() {
    assert_eq!(dataize_program("dollar"), 42);
}

#[test]
fn rec_returns_5() {
    assert_eq!(dataize_program("rec"), 5);
}

#[test]
fn fibo_returns_21() {
    assert_eq!(dataize_program("fibo"), 21);
}

#[test]
fn fibo9_returns_34() {
    assert_eq!(dataize_program("fibo9"), 34);
}

#[test]
fn fibo10_returns_55() {
    assert_eq!(dataize_program("fibo10"), 55);
}

#[test]
fn fibo11_returns_89() {
    assert_eq!(dataize_program("fibo11"), 89);
}

#[test]
fn fibo12_returns_144() {
    assert_eq!(dataize_program("fibo12"), 144);
}

#[test]
fn fibo13_returns_233() {
    assert_eq!(dataize_program("fibo13"), 233);
}

#[test]
fn fibo14_returns_377() {
    assert_eq!(dataize_program("fibo14"), 377);
}

#[test]
fn fibo15_returns_610() {
    assert_eq!(dataize_program("fibo15"), 610);
}

#[test]
fn fibo16_returns_987() {
    assert_eq!(dataize_program("fibo16"), 987);
}

#[test]
fn fibo17_returns_1597() {
    assert_eq!(dataize_program("fibo17"), 1597);
}

#[test]
fn fibo18_returns_2584() {
    assert_eq!(dataize_program("fibo18"), 2584);
}

#[test]
fn fibo19_returns_4181() {
    assert_eq!(dataize_program("fibo19"), 4181);
}

#[test]
fn fibo20_returns_6765() {
    assert_eq!(dataize_program("fibo20"), 6765);
}

#[test]
fn fibo21_returns_10946() {
    assert_eq!(dataize_program("fibo21"), 10946);
}

#[test]
fn fibo22_returns_17711() {
    assert_eq!(dataize_program("fibo22"), 17711);
}

#[test]
fn fibo23_returns_28657() {
    assert_eq!(dataize_program("fibo23"), 28657);
}

#[test]
fn fibo24_returns_46368() {
    assert_eq!(dataize_program("fibo24"), 46368);
}

// fibo25–fibo28 are checked by the benchmark binary's own assert_eq on every
// iteration; not added here to keep `cargo test` under a couple of minutes.
// Run via `cargo run --release --bin bench -- fibo25 fibo26 fibo27 fibo28`.

#[test]
fn dup_returns_2() {
    assert_eq!(dataize_program("dup"), 2);
}

#[test]
fn fibo_arena_stays_bounded_via_dedup() {
    // Without dedup, fibo's arena would grow by 8 bytes per atom call (~1.6 KB).
    // With dedup it should stabilize well under that.
    let mut buf = vec![0u8; TEST_CAP];
    let src = fs::read_to_string(dsl_path("fibo")).expect("read dsl");
    let img = loader::load(&src, &mut buf);
    let atom_fns = atoms::atom_fns_for(&img.atoms.rev);
    let mut rt = Runtime::from_image(img, atom_fns);
    let _ = rt.dataize(0, rt.default_scope(), true);
    assert!(
        rt.arena.dedup_hits() > rt.arena.dedup_misses(),
        "fibo should have more dedup hits than misses (got hits={}, misses={})",
        rt.arena.dedup_hits(),
        rt.arena.dedup_misses()
    );
    assert!(
        rt.arena.len() < 256,
        "fibo's arena should stay under 256 B (got {} B)",
        rt.arena.len()
    );
}

#[test]
fn gc_actually_reclaims() {
    let mut buf = vec![0u8; TEST_CAP];
    let src = fs::read_to_string(dsl_path("fibo")).expect("read dsl");
    let img = loader::load(&src, &mut buf);
    let atom_fns = atoms::atom_fns_for(&img.atoms.rev);
    let mut rt = Runtime::from_image(img, atom_fns);
    let _ = rt.dataize(0, rt.default_scope(), true);
    assert!(rt.gc_phi_count > 0, "fibo with gc_enabled should trigger phi-GC");
    assert!(rt.gc_phi_reclaimed > 0, "fibo should reclaim objects via phi-GC");
    assert!(rt.gc_disp_count > 0, "fibo should trigger disp-GC");
    assert!(rt.gc_disp_reclaimed > 0, "fibo should reclaim objects via disp-GC");
}
