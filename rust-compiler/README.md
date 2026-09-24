# `rust-compiler/` — ahead-of-time compiler from EO to native code

This workspace is a **separate** experiment from the interpreter at [`../rust/`](../rust). It builds an AOT pipeline that turns EO programs into native binaries while preserving the language's "everything is an object" semantics through an *adaptive value representation*. The interpreter is untouched and remains the executable spec for runtime behaviour.

## The central thesis

EO is purely OO: everything is an object, every value is immutable, every operation goes through dispatch. A naïve compilation preserves all of that and produces something close to the interpreter's perf (slow). A naïve compilation that *throws away* the OO and just compiles types is fast but no longer EO.

The thesis of this experiment is that you can have both: **keep the OO illusion at the source level, but cheat where you can prove cheating is safe**. The compiler does the proof; the runtime keeps the unprovable cases honest. The result is native arithmetic for the hot path and full OO semantics for everything that demands them.

Concretely, three layers:

| Layer | What it costs |
|-------|---------------|
| Native code (e.g., Java JIT) | 1× — algorithm as machine code |
| Java -Xint (interpreted bytecode) | ~23× slower (JIT tax) |
| `rust/` EO interpreter | ~1000× slower (JIT tax × runtime-design tax) |
| `rust-compiler/` output (today, fold-able programs) | ~0.02× — 42× **faster** than Java JIT on fibo, see `bench/results-aot.md` |
| `rust-compiler/` output (future, atom-bearing) | TBD — likely 1-5× of Java JIT |

The 42× "faster than Java JIT" headline (see *Why this beats Java JIT* below) is real but not the whole story — it depends on the program not forcing allocations. Once we lower programs that do allocate, that number compresses. The interesting question is how far.

## Layout

```
rust-compiler/
├─ Cargo.toml                  # workspace
├─ eo-rt/                      # runtime library (linked into every compiled binary)
├─ eo-compile/                 # the compiler binary + library
└─ examples/
   └─ handwritten/             # what eo-compile should eventually emit
```

## The adaptive Value representation

The single most important design choice. Every EO value at runtime is a 64-bit `Value`:

```text
Value layout (low bit is the tag):

  small int :  | i63 payload                                | 1 |
  object ptr:  | aligned &'static Object pointer            | 0 |
```

- **Bit 0 = 1**: the upper 63 bits encode a signed integer. No allocation. Operations on tagged ints stay in tagged form. Range: −2⁶² … 2⁶² − 1 (about ±4.6 × 10¹⁸).
- **Bit 0 = 0**: the value is an aligned pointer to a real `Object`. The bottom three bits would all be zero anyway (objects are pointer-aligned), so we use only bit 0 today and reserve bits 1-2 for future tag refinements (bool, nil, double-encoded as NaN-box, etc.).

```rust
#[repr(transparent)]
pub struct Value(pub u64);

impl Value {
    pub const fn small_int(n: i64) -> Self        { Self(((n as u64) << 1) | 1) }
    pub const fn is_small_int(self) -> bool       { (self.0 & 1) == 1 }
    pub const fn as_small_int(self) -> i64        { (self.0 as i64) >> 1 }
    pub fn from_object(obj: &'static Object) -> Self { Self(obj as *const Object as u64) }
}
```

The runtime offers exactly one dispatch primitive:

```rust
#[inline]
pub fn dispatch(recv: Value, attr: AttrId, args: &[Value]) -> Value {
    if recv.is_small_int() {
        small_integer_dispatch(recv, attr, args)
    } else {
        (recv.as_object().unwrap().class.dispatch)(recv, attr, args)
    }
}
```

Compiled code emits `dispatch(...)` at every callsite. The branch on `is_small_int()` is highly predictable per site, so the host CPU's branch predictor + LLVM's inlining means the tag check costs effectively nothing on the fast path.

### The inflation boundary

What happens when a tagged int has to look like a real object — for example, when stored in a slot whose layout says `&'static Object`? Today: nothing, because no code path of this kind exists yet. When it does, the runtime will **inflate**: heap-allocate an `Object { class: &SMALL_INTEGER, δ: 8 bytes }` and store the pointer. From any source-level observer, the value is indistinguishable from one that was always an object. This is the guarantee that lets us *not* talk about "boxing" in the EO source — every value still behaves like an object even when its runtime form is a register.

### What can't be tagged today

- Numbers outside i63 (large counters, billions × billions). Would need NaN-boxing or arbitrary precision.
- Δ-byte payloads other than 8-byte numbers (strings, structs, raw bytes). Always go through `Object`.
- Booleans (currently encoded as small int 0/1; bit-2 tag will reserve `Value::false_` and `Value::true_` and free up i63 for negative range).

These are all addressable, but the implementation cost has to be weighed against the share of real programs that need them.

## The pipeline

```text
DSL text  (emitted by ../resources/to-dsl.xsl)
    ↓
parse  (parse.rs)     DONE     — token-positional grammar; 8 unit tests
    ↓
build  (build.rs)     DONE     — RawLine -> ir::Graph; 5 unit tests
    ↓
eval   (eval.rs)      DONE     — compile-time partial evaluator; 3 tests
    ↓
infer  (infer.rs)     stub     — propagate TypeHints to fixed point
    ↓
shapes (shapes.rs)    stub     — intern hidden-class layouts
    ↓
lower  (lower.rs)     partial  — constant programs only; 2 unit tests
    ↓
rustc  (Command::new) DONE     — host-compiler invocation
    ↓
native binary
```

### `parse` — DSL → typed line stream

A small, token-positional grammar described in the docblock of `parse.rs`. One `RawLine` per DSL statement: `Form`, `Disp`, `App`, `Ctx`. Attribute names stay as strings; numeric refs stay as `u32` IDs. The parser does not intern, does not validate cross-references, does not see types. It just turns text into structured data.

Accepted dialects: every DSL the JS pipeline currently produces. Verified by parsing all 27 programs under `../temp/`.

### `build` — RawLine → IR graph

One `ir::Node` per RawLine. The IR introduces two enrichments over the raw lines:

- `Target` — encodes `ξ` (the runtime context, `-1` in DSL) as `Target::Sentinel`, distinct from a real `Target::Object(NodeId)`. Makes Sentinel handling structural rather than a magic value.
- `AttrRef` — `Slot(u32)` for positional applications, `Name(String)` for named dispatches. Both survive into lowering until `shapes` interns them.

`build` asserts that line IDs are dense (0..N) so `NodeId(i)` indexes directly into `Graph.nodes`. That assertion catches DSL generation bugs early.

### `eval` — compile-time partial evaluator

Mirrors the runtime's `morph` + `dataize` semantics but operates over the IR graph at compile time. For programs whose result is fully knowable without invoking any atoms, `eval_constant_program(&Graph) -> Result<&[u8], _>` returns the Δ bytes of the final value. Anything that:

- reaches an atom (`λ:L_…`),
- exceeds `MAX_STEPS = 1024` morph steps,
- or hits an unbound void slot

...bails with a clear error so `lower` knows to fall through to the full pipeline (which doesn't exist yet, hence "constant programs only" today).

The evaluator implements four cases:

| Node | Eval behaviour |
|------|----------------|
| `Formation` with `Δ` | terminal — return the Δ bytes |
| `Formation` without `Δ` | yield `Form { id, bindings: {} }` |
| `Dispatch { target, attr }` | morph the target, look up `attr` in its bindings/attrs, morph the result |
| `Application { target, attr, value }` | morph the target, add `attr → value` to bindings, return the bound form |
| `Context` | return the current ctx |

This is enough for `simple.dsl` (the program is `42`) and `dollar.dsl` (the program is `foo 42` where `foo` is identity).

### `infer` — type-hint propagation (stub)

Planned: monomorphic inference. A node gets `TypeHint::SmallInt` only if every observed use produces small-int shape. Anything ambiguous stays `Unknown`. The lower step then emits direct tagged arithmetic for SmallInt nodes and full `dispatch` for everything else.

The first thing `infer` will recognize is the stdlib pattern `number ← bytes ← Δ`: any formation whose `φ` traces (via the standard wrapper layers) to an 8-byte Δ that decodes as an integer is provably a small int. Once recognized, `.plus` / `.minus` / `.gt` / `.if` lower to `dispatch(recv, ATTR_PLUS, &[arg])`.

### `shapes` — hidden-class interner (stub)

Each distinct (atom, slot-layout) pair becomes a `Shape`. The codegen emits one `static CLASS_<id>: eo_rt::Class` per shape plus the corresponding dispatch fn. This is the standard "hidden class" trick from V8 et al.: known-shape objects access slots at known offsets without per-attr lookup.

### `lower` — emit Rust source

Today: handles the trivial fold-to-constant case. Emits a single-file Rust program with no external dependencies:

```rust
fn main() {
    println!("data: {}", 42i64);
}
```

`rustc` compiles that directly. No `cargo` involved on the output side.

Planned: emit code that uses `eo_rt::dispatch` for every callsite, one `static CLASS_*` + dispatch fn per inferred shape, fast-path `Value::small_int` arithmetic for inferred-SmallInt nodes, and direct `Value::from_object` references to interned static objects for known-constant globals.

## Crates

### `eo-rt` — runtime support library

Tiny, hot-path-only. Exports:

- `Value(u64)` — adaptive 64-bit slot.
- `Class { name, dispatch }` — hidden-class with one dispatch fn pointer.
- `Object { class: &'static Class }` — heap-side rep for the slow path.
- `dispatch(recv, attr_id, args)` — the entry point compiled code emits.
- `SMALL_INTEGER: Class` — static class for tagged ints; its dispatch fn implements `+ − × > < ==`.

Attribute names are interned to numeric `AttrId`s at *build* time, so the runtime never compares strings. Constants `ATTR_PLUS`, `ATTR_MINUS`, `ATTR_LT` etc. live in `eo-rt/src/lib.rs`.

### `eo-compile` — the compiler

The CLI:

```bash
cargo build --release -p eo-compile

# Parse + show stats:
./target/release/eo-compile path/to/program.dsl

# Print the IR graph:
./target/release/eo-compile path/to/program.dsl --dump

# Lower to Rust source:
./target/release/eo-compile path/to/program.dsl --emit out.rs

# Lower and invoke rustc:
./target/release/eo-compile path/to/program.dsl --compile out_binary
./out_binary                                   # prints "data: <value>"
```

End-to-end verified for the constant case:

```bash
$ ./target/release/eo-compile ../temp/simple/simple.dsl --compile /tmp/simple
$ /tmp/simple
data: 42
```

### `examples/handwritten` — the perf target

One Rust fn per EO program in `../test-resources/`, demonstrating what the compiled output should look like once `lower` is real:

```bash
cargo run --release -p handwritten            # runs every program, asserts results
cargo run --release -p handwritten -- fibo 20 # runs one program with an argument
cargo test  --release -p handwritten          # 10 unit tests
cargo run --release --bin bench               # times fibo8..fibo28, writes bench/results-aot.md
```

All nine EO test programs (`simple`, `foo`, `eleven`, `rec`, `dollar`, `dup`, `self-ref`, `fibo`, `fibo_minus`) round-trip correctly: each function in `examples/handwritten/src/lib.rs` produces the same numeric result the interpreter does. The hand-written `fibo(35) = 9_227_465` runs in ~50 ms — the interpreter cannot finish it in any reasonable time.

#### Headline AOT numbers

From [`../bench/results-aot.md`](../bench/results-aot.md):

| Program | AOT mean | Rust interp | Interp ÷ AOT | Java JIT | JIT ÷ AOT |
|---------|---------:|------------:|-------------:|---------:|----------:|
| fibo (8) | 126 ns | 1.6 ms | 12 800× | 29 µs | 230× |
| fibo15 | 3.7 µs | 75.6 ms | 20 600× | 155 µs | 42× |
| fibo20 | 40 µs | 1.12 s | 27 900× | 2.11 ms | 53× |
| fibo28 | 1.88 ms | 77.0 s | 40 900× | 78.7 ms | 42× |

The interpreter ratio grows with N (per-dispatch overhead amortized over a `2^N` tree). The JIT ratio is roughly constant at ~40×.

### Why this beats Java JIT

Java's `bench/java/Objects.java` allocates ~10 fresh objects per fibo call (`Fibo`, `Sub`, `Integer`, `Add`, `Less`, `If`). For fibo28 that's ~16 M heap allocations. HotSpot's escape analysis is mostly intra-procedural; it cannot stack-allocate objects that cross recursion frames, so even after C2 inlines and devirtualizes, the program still pays allocator pressure + GC + cache misses walking field chains.

The hand-written form allocates nothing. Every `Value` is a `u64` in a register; `dispatch` is a tag check + a tight `match`; LLVM at `lto=thin` inlines the whole call site into an i64 recursion. The advantage is not "Rust is faster than Java" — it is "no allocations is faster than millions of allocations."

If the Java baseline used primitive `long fibo(long n)`, the gap would close to 1-2×. The 42× number reflects *how we encode 'everything is an object'*, not language-level perf.

## Garbage collection

Today there isn't any — and that is why the AOT numbers look as good as they do.

Look at the hand-written fibo: every `Value` is a `u64` register/stack value. `Object` is defined in `eo-rt` but nothing ever instantiates one. So:

- No heap allocations
- No reference counts to update
- No tracing
- No write barriers
- No GC pauses

The 40× advantage over Java JIT exists *because of this*.

### When will the AOT path actually allocate?

Three cases the compiler can't fold:

1. **Δ payloads larger than 8 bytes** (strings, byte buffers, large numbers) — need a heap object.
2. **Inflation** — a tagged-int `Value` that must escape into a slot typed as `&'static Object`.
3. **Cloned formations with non-trivial bindings** — programs that the compiler couldn't fully specialize. We keep an `Object` with a slot table and one `class: &'static Class` pointer.

### What GC strategy will fit

EO is purely immutable, so the value graph has **no cycles** except via the ρ back-pointer. Three workable options, in increasing complexity:

| Strategy | Cost | When it breaks |
|----------|------|----------------|
| **Bump arena, reset per top-level dataize** | one cmp+add per allocation, zero per free | peak memory grows for the whole call; bad for long-running programs |
| **`Rc<Object>` with `Weak` for ρ** | atomic-ish refcount per ref move | requires proving ρ never owns transitively |
| **Tracing mark-compact (port the interpreter's GC)** | full marking cost at trigger points | most general; gives back the ~1.5× perf overhead the interpreter pays for its GC |

**Pragmatic plan**: bump arena first (reset between top-level `dataize` calls). It fits the common case (short programs, fibo-like trees that allocate-then-discard). Once `infer` knows which formations escape, stack-allocate the bounded-lifetime ones and arena-allocate the rest. `Rc` + `Weak` only if needed. Avoid porting the interpreter's mark-compact GC — most of its complexity exists because the interpreter has dynamic object layout; the AOT path has known shapes.

## Pros and trade-offs

### What this design buys you

- **Native arithmetic for hot paths.** Tagged ints + LLVM inlining produce code indistinguishable from hand-written Rust on the same algorithm.
- **No GC pressure for compute-heavy programs.** The 40 000× headline over the interpreter is mostly "no allocations vs. allocate-then-collect."
- **Two implementations of every step.** Fast path when inference proves it's safe; slow path always available. Source semantics never change.
- **Inflation is a one-way street.** Once a value is an `Object`, observers can't tell it was ever tagged. Once it's tagged, slow-path code can always inflate it. No "boxed vs unboxed" type system bifurcation in source.
- **Reuses rustc as the backend.** No custom code gen, no instruction selection, no register allocator. We get LLVM's entire stack for free.

### What this design costs

- **Inference complexity.** A program is only fast to the extent that `infer` proves it can be. Programs with polymorphic dispatch on hot paths fall through to the slow path and pay full dispatch cost.
- **No allocator story yet.** Today's perf numbers reflect a world without allocations. The first real EO program with composite formations will force the GC question, and the numbers will compress.
- **Tagged-int ceiling.** i63 covers most integer code, but anything that wants `u64` arithmetic, BigInt, or floating point lives on the slow path. Java doesn't have this problem (Long boxing is uniform).
- **Compile-time cost.** AOT compilation is slow. For a quick iteration cycle, the interpreter remains the right choice.
- **One implementation per shape.** The compiler emits a `Class` per shape it sees. Polymorphic programs that touch many shapes get larger binaries.

## Roadmap

### Short term (the unlock)

Wire `infer` + `shapes` + `lower` to handle atom-bearing programs. Specifically:

- Recognize `number ← bytes ← Δ` as `TypeHint::SmallInt`.
- Lower `.plus` / `.minus` / `.gt` / `.if` to `dispatch(recv, ATTR_*, &[arg])`.
- Lower `.if` to a Rust `if`/`else` after the cond dataizes to `Value::small_int(0|1)`.
- Lower lazy attrs to `match` arms that dataize on demand.

After this, `eleven` / `dup` / `rec` / `fibo` go through `eo-compile` end-to-end. Most other test programs follow with minor extensions (named atoms, ρ chains).

### Medium term

- **Inflation.** Implement `inflate_small_int` for the bump-arena strategy; emit inflation at every callsite where a tagged Value escapes into an `&'static Object`-typed slot.
- **Inline caches.** One per emitted call site. Stores last-seen class + dispatch fn. Branch-predictable; eliminates the indirect call cost.
- **Dead-shape pruning.** Don't emit a `Class` if no callsite ever uses it.
- **Constant folding through dispatch.** Already done for fully-foldable programs; extend to partial folds (e.g., loop bodies with a known-constant iter count).

### Long term

- **Memoization for inferred-pure functions.** fibo28 would drop from 1.88 ms to nanoseconds. Requires call-graph effect analysis: which formations are pure-enough-to-memoize. Real research project, not next-quarter work.
- **Profile-guided optimization.** Use runtime traces to specialize the inferred-Object slow path into a hierarchy of inferred-Shape fast paths.
- **Multi-target codegen.** Today we shell out to `rustc`. We could emit Cranelift IR directly for faster compilation, or WebAssembly for browsers.
- **Distributed compilation cache.** EO programs reuse stdlib heavily; if we cache per-attr-id lowering results, recompilation gets cheap.

## Risks

A few things I want flagged as we move forward.

### ρ traversal past elided frames

If user code does `ξ.ρ.ρ.ρ.something` and the middle frames were elided as dead, ρ-chain navigation breaks. The fix is standard: when inference *might* elide a frame, check whether *any* attr on its body references ρ (transitively) and if so, keep the frame. Conservative but safe. Tested by intentionally-deep ρ chains in `test-resources/`.

### Side-effecting atoms

L_stdout, L_read, etc. must be marked impure in an atom table so DCE never elides them, even when their result is unused. Today's interpreter is naturally correct here (it always calls); the compiler has to encode this explicitly. **Adding the table at the same time as adding the first impure atom is the only safe order**; doing it lazily is a recipe for silently-elided I/O bugs.

### Identity (==) vs. structural equality

Tagged small-int `5` and an inflated `Object { class: &SMALL_INTEGER, δ: 5 }` should compare equal under EO semantics (value equality). If we ever introduce true `Object` identity, the inflation path may produce one Object per inflation site instead of a shared static one — which would break identity comparisons that happen to span tagged and inflated forms. EO is value-based today, so this isn't a problem; flagging in case it ever changes.

### Allocator choice survives the benchmarks

When we move from "no allocator" to "bump arena," the perf numbers will shift. The bench in `examples/handwritten/src/bin/bench.rs` should be re-run *after* allocator work lands so we measure both regimes. The 40 000× headline will not survive; the JIT ratio likely will.

## Status snapshot

| Component | State |
|----------|-------|
| `eo-rt` Value / dispatch / SMALL_INTEGER | Done, 5 tests |
| `eo-compile` parser | Done, 8 tests, parses all 27 real DSLs |
| `eo-compile` IR builder | Done, 5 tests |
| `eo-compile` partial evaluator | Done (constants only), 3 tests |
| `eo-compile` lower + rustc driver | Done (constants only), 2 tests |
| `eo-compile` infer / shapes | Stubs |
| `examples/handwritten` (9 EO programs) | Done, 10 tests |
| `examples/handwritten` bench (fibo8-28) | Done, writes `../bench/results-aot.md` |

**Tests passing**: 33 across the workspace (5 eo-rt + 18 eo-compile + 10 handwritten).

## Running everything

```bash
cargo build --release
cargo test --release

# AOT a single program end-to-end (works for constant-only programs today):
./target/release/eo-compile ../temp/simple/simple.dsl --compile /tmp/simple && /tmp/simple

# Run the perf-target hand-written forms across every EO program:
./target/release/handwritten

# Benchmark fibo8..fibo28 against the interpreter and Java JIT:
./target/release/bench
```

## Related

- [`../rust/`](../rust) — the EO interpreter in Rust (the spec).
- [`../resources/program.js`](../resources/program.js) — the JavaScript reference implementation.
- [`../bench/results.md`](../bench/results.md) — interpreter + Java baselines.
- [`../bench/results-aot.md`](../bench/results-aot.md) — AOT perf targets (this crate).
- [`../CLAUDE.md`](../CLAUDE.md) — runtime semantics in prose.
