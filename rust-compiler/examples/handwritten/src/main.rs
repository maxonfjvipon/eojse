use std::env;
use std::process::ExitCode;

use eo_rt::Value;
use handwritten::{
    dollar, dup, eleven, fibo, fibo_default, fibo_minus, foo, rec, self_ref, simple,
};

fn run_named(name: &str, arg: Option<i64>) -> Result<i64, String> {
    let v: Value = match name {
        "simple" => simple(),
        "foo" => foo(),
        "eleven" => eleven(),
        "rec" => rec(),
        "dollar" => dollar(),
        "dup" => dup(),
        "self-ref" => self_ref(),
        "fibo" => match arg {
            Some(n) => fibo(Value::small_int(n)),
            None => fibo_default(),
        },
        "fibo_minus" => fibo_minus(),
        other => return Err(format!("unknown program {other}")),
    };
    Ok(v.as_small_int())
}

fn print_all() {
    let cases = [
        ("simple", 42),
        ("foo", 35),
        ("eleven", 11),
        ("rec", 5),
        ("dollar", 42),
        ("dup", 2),
        ("self-ref", 5),
        ("fibo", 21),
        ("fibo_minus", 21),
    ];
    let mut failed = 0;
    for (name, expected) in cases {
        let got = run_named(name, None).unwrap();
        let mark = if got == expected { "ok" } else { "FAIL" };
        if got != expected {
            failed += 1;
        }
        println!("  {mark:>4}  {name:<12} = {got}  (expected {expected})");
    }
    if failed != 0 {
        eprintln!("{failed} program(s) FAILED");
        std::process::exit(1);
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    match args.len() {
        1 => print_all(),
        2 => match run_named(&args[1], None) {
            Ok(v) => println!("{v}"),
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        },
        3 => {
            let n: i64 = args[2].parse().expect("second arg must be an integer");
            match run_named(&args[1], Some(n)) {
                Ok(v) => println!("{v}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(2);
                }
            }
        }
        _ => {
            eprintln!("usage: {} [program [arg]]", args[0]);
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
