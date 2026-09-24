# Runtime Benchmarks

Wall-clock measurements of the Rust runtime. Each row is the mean over N timed iterations after one warm-up probe. N is chosen adaptively per program (10 for fast, down to 1 for >30 s). Times include load (DSL parse + image build + Runtime init) and dataize (the actual evaluation). Peak memory is the high-water mark of the object byte buffer; peak objects is the largest number of simultaneously-live objects.

Run with `cd rust && cargo run --release --bin bench` (or pass program names as args). The file is rewritten fresh on every invocation.

| Program | Result | Iters | Rust total (mean) | Rust min | Stdev | Dataize (mean) | Peak memory | Peak objects | Java OO (mean) | Rust ÷ Java |
|---------|-------:|------:|------------------:|---------:|------:|---------------:|------------:|-------------:|---------------:|------------:|
| fibo | 21 | 10 | 0.557 ms | 0.540 ms | 0.015 ms | 0.532 ms | 14.87 KB | 249 | 0.027 ms | 20.4× |
| fibo9 | 34 | 10 | 0.949 ms | 0.925 ms | 0.027 ms | 0.922 ms | 18.47 KB | 297 | 0.032 ms | 29.5× |
| fibo10 | 55 | 10 | 1.624 ms | 1.555 ms | 0.052 ms | 1.590 ms | 20.55 KB | 325 | 0.039 ms | 42.1× |
| fibo11 | 89 | 10 | 2.792 ms | 2.689 ms | 0.062 ms | 2.753 ms | 25.19 KB | 387 | 0.046 ms | 61.3× |
| fibo12 | 144 | 10 | 4.833 ms | 4.659 ms | 0.095 ms | 4.802 ms | 27.27 KB | 415 | 0.054 ms | 90.0× |
| fibo13 | 233 | 10 | 8.241 ms | 8.133 ms | 0.072 ms | 8.210 ms | 32.95 KB | 491 | 0.069 ms | 119.2× |
| fibo14 | 377 | 10 | 14.219 ms | 14.028 ms | 0.210 ms | 14.176 ms | 35.02 KB | 519 | 0.101 ms | 140.8× |
| fibo15 | 610 | 10 | 24.901 ms | 24.412 ms | 0.424 ms | 24.863 ms | 41.74 KB | 609 | 0.151 ms | 164.4× |
| fibo16 | 987 | 10 | 41.963 ms | 41.674 ms | 0.253 ms | 41.921 ms | 43.82 KB | 637 | 0.247 ms | 170.1× |
| fibo17 | 1597 | 10 | 73.258 ms | 72.000 ms | 1.505 ms | 73.213 ms | 51.58 KB | 741 | 0.411 ms | 178.4× |
| fibo18 | 2584 | 10 | 124.405 ms | 123.058 ms | 1.180 ms | 124.362 ms | 53.66 KB | 769 | 0.648 ms | 191.9× |
| fibo19 | 4181 | 10 | 216.758 ms | 210.249 ms | 8.377 ms | 216.715 ms | 62.45 KB | 887 | 1.054 ms | 205.7× |
| fibo20 | 6765 | 10 | 371.509 ms | 360.763 ms | 14.478 ms | 371.456 ms | 64.53 KB | 915 | 1.753 ms | 211.9× |
| fibo21 | 10946 | 5 | 636.377 ms | 621.491 ms | 15.495 ms | 636.326 ms | 74.37 KB | 1047 | 2.816 ms | 226.0× |
| fibo22 | 17711 | 5 | 1058.081 ms | 1054.073 ms | 3.142 ms | 1058.038 ms | 76.45 KB | 1075 | 6.002 ms | 176.3× |
| fibo23 | 28657 | 5 | 1814.329 ms | 1804.193 ms | 7.195 ms | 1814.287 ms | 87.32 KB | 1221 | 7.934 ms | 228.7× |
| fibo24 | 46368 | 5 | 3084.743 ms | 3074.236 ms | 11.349 ms | 3084.693 ms | 89.40 KB | 1249 | 10.866 ms | 283.9× |
| fibo25 | 75025 | 2 | 5229.436 ms | 5226.136 ms | 3.301 ms | 5229.388 ms | 101.31 KB | 1409 | 16.417 ms | 318.5× |
| fibo26 | 121393 | 2 | 9145.362 ms | 9026.555 ms | 118.807 ms | 9145.321 ms | 103.39 KB | 1437 | 28.101 ms | 325.4× |
| fibo27 | 196418 | 2 | 15272.274 ms | 15031.516 ms | 240.758 ms | 15272.229 ms | 116.34 KB | 1611 | 46.249 ms | 330.2× |
| fibo28 | 317811 | 2 | 26034.962 ms | 25931.410 ms | 103.552 ms | 26034.919 ms | 118.42 KB | 1639 | 79.663 ms | 326.8× |
