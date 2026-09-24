//! +-----------------------------------------------------------------+
//! | bench — wall-clock measurements for the hand-written native     |
//! | fibo across n=8..28. This is the perf target for `eo-compile`'s |
//! | future output. Rewrites bench/results-aot.md fresh on every run.|
//! +-----------------------------------------------------------------+

use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use eo_rt::Value;
use handwritten::fibo;

const OUTER_ITERS: usize = 10;
const TARGET_NS_PER_SAMPLE: u128 = 2_000_000;

fn probe_ns(n: i64) -> u128 {
    let t0 = Instant::now();
    let v = fibo(Value::small_int(n));
    std::hint::black_box(v);
    Instant::now().duration_since(t0).as_nanos().max(1)
}

fn inner_iters_for(probe: u128) -> usize {
    let estimated = (TARGET_NS_PER_SAMPLE / probe) as usize;
    estimated.clamp(1, 1_000_000)
}

fn sample_ns(n: i64, inner: usize) -> u128 {
    let arg = Value::small_int(n);
    let t0 = Instant::now();
    for _ in 0..inner {
        let v = fibo(std::hint::black_box(arg));
        std::hint::black_box(v);
    }
    Instant::now().duration_since(t0).as_nanos() / inner as u128
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
    Stats { min, mean, stdev: (var as f64).sqrt() as u128 }
}

fn fmt_ns(ns: u128) -> String {
    if ns < 1_000 {
        format!("{ns} ns")
    } else if ns < 1_000_000 {
        format!("{:.3} µs", ns as f64 / 1_000.0)
    } else {
        format!("{:.3} ms", ns as f64 / 1_000_000.0)
    }
}

struct Row {
    program: String,
    n: i64,
    result: i64,
    inner: usize,
    outer: usize,
    s: Stats,
    interp_ns: Option<u128>,
    jit_ns: Option<u128>,
}

fn bench_one(n: i64, ref_table: &std::collections::HashMap<String, (Option<u128>, Option<u128>)>) -> Row {
    let probe = probe_ns(n);
    let inner = inner_iters_for(probe);
    let result = fibo(Value::small_int(n)).as_small_int();
    let mut samples = Vec::with_capacity(OUTER_ITERS);
    for _ in 0..OUTER_ITERS {
        samples.push(sample_ns(n, inner));
    }
    let program = if n == 8 { "fibo".to_string() } else { format!("fibo{n}") };
    let (interp_ns, jit_ns) = ref_table.get(&program).copied().unwrap_or((None, None));
    Row { program, n, result, inner, outer: OUTER_ITERS, s: stats(&samples), interp_ns, jit_ns }
}

fn parse_ms(cell: &str) -> Option<u128> {
    let trimmed = cell.trim().trim_end_matches('×').trim();
    let value = trimmed.strip_suffix(" ms").or_else(|| trimmed.strip_suffix("ms"))?;
    let f: f64 = value.trim().parse().ok()?;
    Some((f * 1_000_000.0) as u128)
}

fn load_reference_table() -> std::collections::HashMap<String, (Option<u128>, Option<u128>)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("bench")
        .join("results.md");
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Default::default(),
    };
    let mut out = std::collections::HashMap::new();
    for line in text.lines() {
        if !line.starts_with("| fibo") {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 13 {
            continue;
        }
        let program = cells[1].to_string();
        let interp = parse_ms(cells[4]);
        let jit = parse_ms(cells[12]);
        out.insert(program, (interp, jit));
    }
    out
}

fn results_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("bench")
        .join("results-aot.md")
}

fn write_results(path: &PathBuf, rows: &[Row]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = String::new();
    out.push_str("# AOT Benchmarks (hand-written native form)\n\n");
    out.push_str(
        "Wall-clock measurements of the `examples/handwritten` crate — the shape \
        `eo-compile` should eventually emit. Each row is the per-call mean over \
        `inner × outer` Fibonacci invocations: `inner` is chosen per N so each \
        sample takes ~2 ms, then we average across `outer = 10` samples.\n\n",
    );
    out.push_str(
        "These numbers are the **perf target** for the compiler. The interpreter \
        results in [`results.md`](./results.md) and Java baselines should be read \
        side by side; the gap between this table and `results.md`'s Rust column is \
        what the AOT pipeline is meant to close.\n\n",
    );
    out.push_str(
        "Run with `cd rust-compiler && cargo run --release --bin bench`. The file is \
        rewritten fresh on every invocation.\n\n",
    );
    out.push_str("| Program | n | Result | Inner | Outer | AOT mean | Min | Stdev | Rust interp | Interp ÷ AOT | Java JIT | JIT ÷ AOT |\n");
    out.push_str("|---------|---:|------:|------:|------:|---------:|----:|------:|------------:|-------------:|---------:|----------:|\n");
    for r in rows {
        let interp_cell = r.interp_ns.map(fmt_ns).unwrap_or_else(|| "—".into());
        let interp_ratio = match r.interp_ns {
            Some(i) if r.s.mean > 0 => format!("{:.0}×", i as f64 / r.s.mean as f64),
            _ => "—".into(),
        };
        let jit_cell = r.jit_ns.map(fmt_ns).unwrap_or_else(|| "—".into());
        let jit_ratio = match r.jit_ns {
            Some(j) if r.s.mean > 0 => format!("{:.1}×", j as f64 / r.s.mean as f64),
            _ => "—".into(),
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.program,
            r.n,
            r.result,
            r.inner,
            r.outer,
            fmt_ns(r.s.mean),
            fmt_ns(r.s.min),
            fmt_ns(r.s.stdev),
            interp_cell,
            interp_ratio,
            jit_cell,
            jit_ratio,
        ));
    }
    fs::write(path, out)?;
    Ok(())
}

fn main() {
    let argv: Vec<String> = env::args().collect();
    let ns: Vec<i64> = if argv.len() > 1 {
        argv.iter()
            .skip(1)
            .map(|s| s.parse::<i64>().expect("arg must be an integer N"))
            .collect()
    } else {
        (8i64..=28).collect()
    };
    let ref_table = load_reference_table();
    let mut rows = Vec::with_capacity(ns.len());
    for n in ns {
        let row = bench_one(n, &ref_table);
        println!(
            "  {:>8}  n={:>2}  result={:>6}  mean={}",
            row.program, row.n, row.result, fmt_ns(row.s.mean),
        );
        rows.push(row);
    }
    let path = results_path();
    write_results(&path, &rows).expect("write results-aot.md");
    eprintln!("\nwrote {}", path.display());
}
