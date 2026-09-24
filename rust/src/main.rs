use std::env;
use std::fs;
use std::process::ExitCode;

use eo_runtime::atoms;
use eo_runtime::loader::{self, DslLine, Image};
use eo_runtime::runtime::Runtime;

/// Size of the on-stack object buffer. macOS / Linux main thread default
/// stack is 8 MB; 4 MB leaves room for the rest of the program's frames.
const MEMORY_CAP: usize = 4 * 1024 * 1024;

fn print_loaded(img: &Image<'_>, lines: &[DslLine]) {
    eprintln!(
        "loaded: {} objects, memory={} B / {} B, arena={} B, attrs={}, atoms={}",
        lines.len(),
        img.memory.len(),
        img.memory.capacity(),
        img.arena.len(),
        img.attrs.rev.len(),
        img.atoms.rev.len(),
    );
}

fn print_stats(rt: &Runtime<'_>) {
    println!("original program size: {} B", rt.program_size);
    println!("peak memory: {} B (of {} B buffer)", rt.peak_memory_bytes, rt.memory.capacity());
    println!("final memory: {} B", rt.memory.len());
    println!("total created: {}", rt.total_created);
    println!("total deleted: {}", rt.total_deleted);
    println!("max depth (objects): {}", rt.peak_live);
    println!("max ref holders: {}", rt.max_ref_holders);
    println!("gc_phi:  {} calls, {} reclaimed", rt.gc_phi_count, rt.gc_phi_reclaimed);
    println!("gc_disp: {} calls, {} reclaimed", rt.gc_disp_count, rt.gc_disp_reclaimed);
    println!(
        "arena: final {} B, dedup hits {}, misses {}",
        rt.arena.len(),
        rt.arena.dedup_hits(),
        rt.arena.dedup_misses(),
    );
    println!("atom calls:");
    for (i, &count) in rt.atom_calls.iter().enumerate() {
        println!("  {}: {}", rt.atoms.rev[i], count);
    }
}

fn run(path: &str, buf: &mut [u8]) -> Result<(), String> {
    let src = fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    let lines = loader::parse_dsl(&src);
    let img = loader::load(&src, buf);
    print_loaded(&img, &lines);

    let atom_fns = atoms::atom_fns_for(&img.atoms.rev);
    let mut rt = Runtime::from_image(img, atom_fns);

    // Top-level dataize: index 0 (Q), gc enabled. Atoms internally run
    // with gc_enabled=false via the default (matches the JS prototype).
    let gc_enabled = std::env::var("EO_NO_GC").is_err();
    let (off, len) = rt.dataize(0, rt.default_scope(), gc_enabled);
    let bytes = rt.arena.slice(off, len);
    if bytes.len() == 8 {
        let arr: [u8; 8] = bytes.try_into().unwrap();
        let n = f64::from_be_bytes(arr);
        if n.is_finite() && n == (n as i64) as f64 {
            println!("data: {}", n as i64);
        } else {
            println!("data: {}", n);
        }
    } else {
        println!("data: ({} bytes)", bytes.len());
    }

    print_stats(&rt);
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: {} <path.dsl>", args[0]);
        return ExitCode::from(2);
    }

    // The object buffer lives on the main thread's stack. Δ payloads stay
    // on the heap (the runtime's Arena). See README §"Backing-storage profiles".
    let mut buf = [0u8; MEMORY_CAP];

    match run(&args[1], &mut buf) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
