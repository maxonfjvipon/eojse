//! Microbenchmark for the EO runtime.
//!
//! Runs each program N times, measures wall-clock + peak resources, and
//! rewrites `bench/results.md` fresh on every invocation (no append; the
//! latest run is always the only one shown).
//!
//! Usage:
//!   cargo run --release --bin bench
//!   cargo run --release --bin bench -- fibo fibo15

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use eo_runtime::atoms;
use eo_runtime::loader;
use eo_runtime::runtime::Runtime;

const MEMORY_CAP: usize = 4 * 1024 * 1024;

/// Adaptive iteration count: a single warm-up probe estimates the runtime,
/// then we pick how many timed iterations to do based on cost. Very fast
/// programs get many samples for tight stdev; very slow ones get few so
/// the whole bench finishes in a reasonable time.
fn iters_for(probe_ms: u128) -> usize {
    if probe_ms > 30_000 { 1 }
    else if probe_ms > 5_000 { 2 }
    else if probe_ms > 500 { 5 }
    else { 10 }
}

/// Extract the Fibonacci N from a program name like "fibo" (n=8) or "fibo<N>".
fn fibo_n_of(program: &str) -> Option<u32> {
    if program == "fibo" {
        return Some(8);
    }
    program.strip_prefix("fibo").and_then(|s| s.parse::<u32>().ok())
}

/// Run `java Objects N CYCLES` from bench/java/ and parse the mean-ns line.
/// Returns None if java/javac unavailable, classpath missing, or output unparseable.
fn java_objects_ns(n: u32, cycles: u32) -> Option<u128> {
    let classpath = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("bench")
        .join("java");
    let class_file = classpath.join("Objects.class");
    if !class_file.exists() {
        return None;
    }
    let out = Command::new("java")
        .arg("-cp").arg(&classpath)
        .arg("Objects")
        .arg(n.to_string())
        .arg(cycles.to_string())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    // Looking for "Time is <ns> ns"
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Time is ") {
            if let Some(num) = rest.strip_suffix(" ns") {
                return num.trim().parse::<u128>().ok();
            }
        }
    }
    None
}

/// Cycles for the Java run. Java is ~1000× faster than the Rust EO interpreter,
/// so we can afford more cycles. But fibo(28) at 80 ms × cycles still adds up.
fn java_cycles_for(n: u32) -> u32 {
    if n >= 26 { 5 } else if n >= 20 { 10 } else { 20 }
}

fn dsl_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("temp")
        .join(name)
        .join(format!("{name}.dsl"))
}

fn results_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("bench")
        .join("results.md")
}

struct Sample {
    total_ns: u128,
    load_ns: u128,
    dataize_ns: u128,
    result: i64,
    peak_memory_bytes: u32,
    peak_objects: i64,
}

fn run_one(src: &str, buf: &mut [u8]) -> Sample {
    let t0 = Instant::now();
    let img = loader::load(src, buf);
    let atom_fns = atoms::atom_fns_for(&img.atoms.rev);
    let mut rt = Runtime::from_image(img, atom_fns);
    let t_loaded = Instant::now();
    let (off, len) = rt.dataize(0, rt.default_scope(), true);
    let t_done = Instant::now();
    let bytes = rt.arena.slice(off, len);
    let arr: [u8; 8] = bytes.try_into().expect("8-byte payload");
    let n = f64::from_be_bytes(arr) as i64;
    Sample {
        load_ns: t_loaded.duration_since(t0).as_nanos(),
        dataize_ns: t_done.duration_since(t_loaded).as_nanos(),
        total_ns: t_done.duration_since(t0).as_nanos(),
        result: n,
        peak_memory_bytes: rt.peak_memory_bytes,
        peak_objects: rt.peak_live,
    }
}

#[derive(Clone, Copy)]
struct Stats {
    min: u128,
    mean: u128,
    stdev: u128,
}

fn stats(samples: &[u128]) -> Stats {
    let n = samples.len() as u128;
    let min = *samples.iter().min().unwrap();
    let mean = samples.iter().sum::<u128>() / n;
    let var: u128 = samples
        .iter()
        .map(|&x| {
            let d = if x > mean { x - mean } else { mean - x };
            d * d
        })
        .sum::<u128>()
        / n;
    let stdev = (var as f64).sqrt() as u128;
    Stats { min, mean, stdev }
}

fn fmt_ms(ns: u128) -> String {
    format!("{:.3} ms", ns as f64 / 1_000_000.0)
}

fn fmt_kb(bytes: u32) -> String {
    format!("{:.2} KB", bytes as f64 / 1024.0)
}

struct Row {
    program: String,
    result: i64,
    iters: usize,
    total: Stats,
    #[allow(dead_code)]
    load: Stats,
    dataize: Stats,
    peak_memory_bytes: u32,
    peak_objects: i64,
    /// Java OO equivalent (Objects.java) mean time per iteration, ns.
    /// None if the program doesn't have a Java fibo equivalent (or javac/java unavailable).
    java_mean_ns: Option<u128>,
}

fn write_results(path: &PathBuf, rows: &[Row]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = String::new();
    out.push_str("# Runtime Benchmarks\n\n");
    out.push_str(
        "Wall-clock measurements of the Rust runtime. Each row is the mean over N \
        timed iterations after one warm-up probe. N is chosen adaptively per program \
        (10 for fast, down to 1 for >30 s). Times include load (DSL parse + image \
        build + Runtime init) and dataize (the actual evaluation). Peak memory is \
        the high-water mark of the object byte buffer; peak objects is the largest \
        number of simultaneously-live objects.\n\n",
    );
    out.push_str("Run with `cd rust && cargo run --release --bin bench` (or pass program names as args). The file is rewritten fresh on every invocation.\n\n");
    out.push_str("| Program | Result | Iters | Rust total (mean) | Rust min | Stdev | Dataize (mean) | Peak memory | Peak objects | Java OO (mean) | Rust ÷ Java |\n");
    out.push_str("|---------|-------:|------:|------------------:|---------:|------:|---------------:|------------:|-------------:|---------------:|------------:|\n");
    for r in rows {
        let java_cell = r.java_mean_ns.map(fmt_ms).unwrap_or_else(|| "—".to_string());
        let ratio_cell = match r.java_mean_ns {
            Some(j) if j > 0 => format!("{:.1}×", r.total.mean as f64 / j as f64),
            _ => "—".to_string(),
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.program,
            r.result,
            r.iters,
            fmt_ms(r.total.mean),
            fmt_ms(r.total.min),
            fmt_ms(r.total.stdev),
            fmt_ms(r.dataize.mean),
            fmt_kb(r.peak_memory_bytes),
            r.peak_objects,
            java_cell,
            ratio_cell,
        ));
    }
    fs::write(path, out)?;
    Ok(())
}

fn benchmark(program: &str) -> std::io::Result<Row> {
    let path = dsl_path(program);
    let src = fs::read_to_string(&path).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("can't read {}: {e}", path.display()),
        )
    })?;
    let mut buf = vec![0u8; MEMORY_CAP];

    // One probe doubles as a warm-up and a runtime estimate for picking N.
    let probe = run_one(&src, &mut buf);
    let expected = probe.result;
    let peak_memory_bytes = probe.peak_memory_bytes;
    let peak_objects = probe.peak_objects;

    let iters = iters_for(probe.total_ns / 1_000_000);

    let mut total = Vec::with_capacity(iters);
    let mut load = Vec::with_capacity(iters);
    let mut dataize = Vec::with_capacity(iters);
    for _ in 0..iters {
        let s = run_one(&src, &mut buf);
        assert_eq!(s.result, expected, "non-deterministic result");
        load.push(s.load_ns);
        dataize.push(s.dataize_ns);
        total.push(s.total_ns);
    }

    let total_s = stats(&total);
    let load_s = stats(&load);
    let dataize_s = stats(&dataize);

    // Optional Java OO measurement for fibo programs.
    let java_mean_ns = fibo_n_of(program)
        .and_then(|n| java_objects_ns(n, java_cycles_for(n)));

    let java_str = java_mean_ns
        .map(|ns| fmt_ms(ns))
        .unwrap_or_else(|| "—".to_string());
    let ratio_str = match java_mean_ns {
        Some(j) if j > 0 => format!("{:.1}×", total_s.mean as f64 / j as f64),
        _ => "—".to_string(),
    };
    println!(
        "{program:8} (expected {expected})  iters {iters}  Rust {} (min {}, stdev {})  peak {} / {} objs  Java OO {} (Rust ÷ Java = {})",
        fmt_ms(total_s.mean),
        fmt_ms(total_s.min),
        fmt_ms(total_s.stdev),
        fmt_kb(peak_memory_bytes),
        peak_objects,
        java_str,
        ratio_str,
    );

    Ok(Row {
        program: program.to_string(),
        result: expected,
        iters,
        total: total_s,
        load: load_s,
        dataize: dataize_s,
        peak_memory_bytes,
        peak_objects,
        java_mean_ns,
    })
}

fn default_programs() -> Vec<&'static str> {
    vec![
        "fibo", "fibo9", "fibo10", "fibo11", "fibo12", "fibo13",
        "fibo14", "fibo15", "fibo16", "fibo17",
        "fibo18", "fibo19", "fibo20", "fibo21",
        "fibo22", "fibo23", "fibo24", "fibo25",
        "fibo26", "fibo27", "fibo28",
    ]
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let programs: Vec<String> = if args.is_empty() {
        default_programs().into_iter().map(String::from).collect()
    } else {
        args
    };

    println!("bench: adaptive iterations (10/5/2/1 by probe time), ms + KB units");
    println!("programs: {}", programs.join(", "));

    let mut rows = Vec::with_capacity(programs.len());
    for prog in &programs {
        match benchmark(prog) {
            Ok(row) => rows.push(row),
            Err(e) => {
                eprintln!("bench({prog}) failed: {e}");
                std::process::exit(1);
            }
        }
    }

    let path = results_path();
    if let Err(e) = write_results(&path, &rows) {
        eprintln!("can't write {}: {e}", path.display());
        std::process::exit(1);
    }
    println!("results: {}", path.display());
}
