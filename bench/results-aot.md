# AOT Benchmarks (hand-written native form)

Wall-clock measurements of the `examples/handwritten` crate — the shape `eo-compile` should eventually emit. Each row is the per-call mean over `inner × outer` Fibonacci invocations: `inner` is chosen per N so each sample takes ~2 ms, then we average across `outer = 10` samples.

These numbers are the **perf target** for the compiler. The interpreter results in [`results.md`](./results.md) and Java baselines should be read side by side; the gap between this table and `results.md`'s Rust column is what the AOT pipeline is meant to close.

Run with `cd rust-compiler && cargo run --release --bin bench`. The file is rewritten fresh on every invocation.

| Program | n | Result | Inner | Outer | AOT mean | Min | Stdev | Rust interp | Interp ÷ AOT | Java JIT | JIT ÷ AOT |
|---------|---:|------:|------:|------:|---------:|----:|------:|------------:|-------------:|---------:|----------:|
| fibo | 8 | 21 | 48 | 10 | 126 ns | 126 ns | 0 ns | 1.613 ms | 12802× | 29.000 µs | 230.2× |
| fibo9 | 9 | 34 | 8000 | 10 | 201 ns | 195 ns | 5 ns | 2.839 ms | 14124× | 28.000 µs | 139.3× |
| fibo10 | 10 | 55 | 5333 | 10 | 326 ns | 316 ns | 8 ns | 4.893 ms | 15009× | 46.000 µs | 141.1× |
| fibo11 | 11 | 89 | 3430 | 10 | 538 ns | 518 ns | 9 ns | 8.547 ms | 15887× | 46.000 µs | 85.5× |
| fibo12 | 12 | 144 | 2398 | 10 | 864 ns | 839 ns | 18 ns | 14.634 ms | 16938× | 54.000 µs | 62.5× |
| fibo13 | 13 | 233 | 1454 | 10 | 1.392 µs | 1.361 µs | 36 ns | 25.428 ms | 18267× | 71.000 µs | 51.0× |
| fibo14 | 14 | 377 | 888 | 10 | 2.224 µs | 2.177 µs | 52 ns | 44.244 ms | 19894× | 97.000 µs | 43.6× |
| fibo15 | 15 | 610 | 466 | 10 | 3.666 µs | 3.542 µs | 112 ns | 75.607 ms | 20624× | 155.000 µs | 42.3× |
| fibo16 | 16 | 987 | 326 | 10 | 5.973 µs | 5.706 µs | 153 ns | 129.152 ms | 21623× | 252.000 µs | 42.2× |
| fibo17 | 17 | 1597 | 203 | 10 | 9.486 µs | 9.238 µs | 228 ns | 225.546 ms | 23777× | 388.000 µs | 40.9× |
| fibo18 | 18 | 2584 | 114 | 10 | 15.350 µs | 14.968 µs | 356 ns | 388.358 ms | 25300× | 664.000 µs | 43.3× |
| fibo19 | 19 | 4181 | 58 | 10 | 24.965 µs | 24.326 µs | 494 ns | 669.321 ms | 26810× | 1.109 ms | 44.4× |
| fibo20 | 20 | 6765 | 35 | 10 | 40.041 µs | 39.290 µs | 1.192 µs | 1118.989 ms | 27946× | 2.107 ms | 52.6× |
| fibo21 | 21 | 10946 | 30 | 10 | 64.967 µs | 63.238 µs | 1.277 µs | 1938.160 ms | 29833× | 3.406 ms | 52.4× |
| fibo22 | 22 | 17711 | 19 | 10 | 106.259 µs | 102.956 µs | 3.017 µs | 3275.083 ms | 30822× | 4.650 ms | 43.8× |
| fibo23 | 23 | 28657 | 11 | 10 | 168.856 µs | 165.556 µs | 3.679 µs | 5546.720 ms | 32849× | 6.452 ms | 38.2× |
| fibo24 | 24 | 46368 | 7 | 10 | 277.519 µs | 267.916 µs | 4.934 µs | 9308.055 ms | 33540× | 9.870 ms | 35.6× |
| fibo25 | 25 | 75025 | 4 | 10 | 441.315 µs | 433.437 µs | 5.359 µs | 15987.134 ms | 36226× | 17.004 ms | 38.5× |
| fibo26 | 26 | 121393 | 2 | 10 | 710.464 µs | 701.354 µs | 10.305 µs | 26986.139 ms | 37984× | 28.547 ms | 40.2× |
| fibo27 | 27 | 196418 | 1 | 10 | 1.146 ms | 1.135 ms | 17.598 µs | 45555.056 ms | 39739× | 46.483 ms | 40.5× |
| fibo28 | 28 | 317811 | 1 | 10 | 1.880 ms | 1.836 ms | 50.739 µs | 76998.069 ms | 40947× | 78.685 ms | 41.8× |
