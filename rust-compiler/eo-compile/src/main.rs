use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

use eo_compile::{build, lower, parse};

struct Args {
    input: String,
    dump: bool,
    emit: Option<String>,
    compile: Option<String>,
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut input: Option<String> = None;
    let mut dump = false;
    let mut emit: Option<String> = None;
    let mut compile: Option<String> = None;
    let mut iter = argv.iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--dump" => dump = true,
            "--emit" => {
                emit = Some(
                    iter.next()
                        .ok_or_else(|| "--emit needs a path".to_string())?
                        .clone(),
                );
            }
            "--compile" => {
                compile = Some(
                    iter.next()
                        .ok_or_else(|| "--compile needs a path".to_string())?
                        .clone(),
                );
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown flag {other}"));
            }
            other => {
                if input.is_some() {
                    return Err(format!("unexpected positional arg {other}"));
                }
                input = Some(other.to_string());
            }
        }
    }
    Ok(Args {
        input: input.ok_or_else(|| "missing <input.dsl>".to_string())?,
        dump,
        emit,
        compile,
    })
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = env::args().collect();
    let args = parse_args(&argv).map_err(|e| {
        format!(
            "{e}\nusage: {} <input.dsl> [--dump] [--emit out.rs] [--compile out_binary]",
            argv[0]
        )
    })?;
    let text = fs::read_to_string(&args.input)
        .map_err(|e| format!("read {}: {e}", args.input))?;
    let lines = parse::parse(&text).map_err(|e| e.to_string())?;
    let graph = build::build(&lines).map_err(|e| e.to_string())?;
    let [forms, disps, apps, ctxs] = graph.count_by_kind();
    eprintln!(
        "parsed {} statement(s) from {}: {} FORM, {} DISP, {} APP, {} CTX",
        lines.len(),
        args.input,
        forms,
        disps,
        apps,
        ctxs,
    );
    if args.dump {
        for (i, node) in graph.nodes.iter().enumerate() {
            println!("{i}: {node:?}");
        }
    }
    if args.emit.is_some() || args.compile.is_some() {
        let src = lower::lower(&graph).map_err(|e| e.to_string())?;
        if let Some(path) = args.emit.as_deref() {
            fs::write(path, &src).map_err(|e| format!("write {path}: {e}"))?;
            eprintln!("emitted Rust source to {path} ({} bytes)", src.len());
        }
        if let Some(bin) = args.compile.as_deref() {
            invoke_rustc(&src, bin)?;
            eprintln!("compiled to {bin}");
        }
    } else if !args.dump {
        eprintln!("(pass --dump to print the IR, --emit out.rs to lower, --compile bin to build)");
    }
    Ok(())
}

fn invoke_rustc(src: &str, binary: &str) -> Result<(), String> {
    let tmp = std::env::temp_dir().join(format!("eo-compile-{}.rs", std::process::id()));
    fs::write(&tmp, src).map_err(|e| format!("write temp source: {e}"))?;
    let status = Command::new("rustc")
        .arg("-O")
        .arg("--edition=2021")
        .arg("-o")
        .arg(binary)
        .arg(&tmp)
        .status()
        .map_err(|e| format!("spawn rustc: {e}"))?;
    let _ = fs::remove_file(&tmp);
    if !status.success() {
        return Err(format!("rustc failed with status {status}"));
    }
    if !Path::new(binary).exists() {
        return Err(format!("rustc reported success but {binary} is missing"));
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
