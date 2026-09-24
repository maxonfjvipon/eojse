# `rust/` — EO interpreter in Rust

A from-scratch, dependency-free port of the JavaScript runtime at [`../resources/program.js`](../resources/program.js). The intent is to be the **executable specification** of EO runtime semantics in a strongly typed language with explicit memory layout, while staying readable as a reference for future ports. The separate AOT compiler at [`../rust-compiler/`](../rust-compiler) is a different experiment and does not share code with this crate.

## Design goals

1. **Be the spec.** Every EO test program in `../test-resources/` must produce the same printed value as the JS prototype. If the JS prototype changes, this crate's tests must change with it.
2. **Stay readable.** Prefer obvious data layouts over clever ones; one module per concept (`block`, `memory`, `loader`, `runtime`, `atoms`); no implicit conversions; no third-party crates.
3. **Make memory observable.** Every allocation, every reference, every GC step should be inspectable from outside. That's why every ref is a `u32` byte offset (printable, comparable, copyable) and not a Rust reference.
4. **Be allocator-honest.** Don't hide where bytes live. The object buffer is a borrowed `&mut [u8]`; the Δ heap is an explicit `Arena`. Both have public `len()`, dedup counters, and high-water marks.

What this crate explicitly does **not** try to be:

- **A fast EO runtime.** It is ~40× slower than Java -Xint on the same OO Fibonacci. See *Trade-offs* below.
- **JIT-capable.** No code generation at runtime. The compiler experiment lives separately.
- **Thread-safe.** `Runtime<'a>` is `!Sync` by construction. EO programs are single-threaded today.

## What works today

| Feature | Status |
|--------|--------|
| Parser (`.dsl` → in-memory image) | Done |
| Block layout (FRM / DSP / APP / CTX) | Done — byte-packed, u32-offset refs |
| Mark-compact GC at `φ` and dispatch sites | Done |
| Arena with Δ-byte dedup | Done |
| Stack-allocated 4 MB object buffer (`Memory<'a>`) | Done |
| Atoms (L_number_plus / minus / times / gt / eq / etc.) | Done |
| All EO test programs (`simple`..`fibo28`) | 36 tests passing |

## Running

```bash
# Tests (release; under 1 s for the whole suite):
cargo test --release

# Run one DSL program directly (must already be compiled by `npm test`
# in the repo root, which leaves DSLs under ../temp/<name>/<name>.dsl):
cargo run --release --bin eo-load -- ../temp/fibo/fibo.dsl

# Adaptive-iteration benchmark across fibo8..fibo28:
cargo run --release --bin bench
```

Each program prints `data: <integer>` plus runtime stats (peak memory, peak live object count, GC call counts, arena dedup hits, per-atom call counts).

## Architecture

```
src/
├─ lib.rs        # re-exports
├─ block.rs      # block layout constants + offset helpers
├─ memory.rs     # Memory<'a> (object buffer) + Arena (Δ heap)
├─ loader.rs     # .dsl text -> Image (Memory + Arena + interners)
├─ runtime.rs    # Runtime<'a> — morph / dataize / GC / exec
├─ atoms.rs      # built-in atoms (L_number_plus, ...)
├─ main.rs       # eo-load CLI (single-program driver)
└─ bin/bench.rs  # multi-program benchmark with adaptive iteration counts
```

### Block layout — and why byte offsets

Every object — Formation, Dispatch, Application, Context — lives at a `u32` byte offset inside `Memory`. The byte at the offset is the type tag (TY_FRM / TY_DSP / TY_APP / TY_CTX) plus four flags (DEAD, STAY, FROM_ATOM, UNBOUND). All inter-object refs are byte offsets, so there are no Rust references between blocks and no aliasing concerns inside the buffer.

| Block | Size | Contents |
|-------|------|----------|
| FRM   | dynamic | header (8 B) + presence-bitmask (8 B per 64 slots) + name table (2 B × slots, padded to 4) + slots (12 B each: spec + ref) |
| DSP   | 16 B | header + target offset + attr name id |
| APP   | 16 B | header + target offset + attr (slot index or name id) + value offset |
| CTX   | 8 B  | header only — bare `$` |

The full constant table is in [`src/block.rs`](src/block.rs); the formulas in `frm_size` / `frm_*_off` lay out a FORM's variable-size body. The loader appends an implicit ρ slot to every FORM that doesn't declare one in source, so every formation can receive ρ at dispatch time without a re-layout.

**Why u32 byte offsets instead of `&Object` references?**

- *Compactness.* A u32 is half the size of a Rust pointer; objects pack tighter, more fit in cache.
- *Movability.* The GC compacts the buffer by moving blocks left. With offsets, we update one integer per ref and we're done — no pointer-rewriting, no provenance tracking, no `unsafe` writes through opaque references.
- *Sentinel availability.* `u32::MAX` is the "absent" sentinel; a single `== MAX` check distinguishes "this slot has no value" from "this slot points to block 0." With Rust references, the equivalent would be `Option<&Object>`, which doubles the field size on stable Rust.
- *Serializability.* The full image is just bytes. We could write `Memory` + `Arena` to disk and load them back without any pointer fix-up. (We don't yet; we could.)

The trade-off: every ref dereference is a bounds-checked array index instead of a register pointer. That's measurable — see *Trade-offs* below — but the GC simplicity it buys is worth it for a reference implementation.

### Memory + Arena — and the stack/heap split

- `Memory<'a>` borrows a caller-owned `[u8]` (the binary preallocates a 4 MB stack buffer in `main.rs`). Pushes append, `pop` removes the tail in O(1), `del(off)` nulls a block and decrements `live_count`. The object buffer is the only mutable byte slice the runtime touches at this layer.
- `Arena` holds Δ payloads on the heap with hash-dedup (`HashMap<Vec<u8>, u32>`). Identical literals (e.g., the two `1`s in `1.plus 1`) share one offset.

`Image<'a>` is the loader's output: `Memory<'a>`, `Arena`, two interners (`attrs`, `atoms`), and the program size in bytes. `Runtime::from_image` consumes it.

**Why the split?**

Objects (FRM/DSP/APP/CTX) have *predictable*, bounded total size and are frequently created and destroyed. A stack buffer with bump-on-push + GC-on-trigger fits that pattern: no syscalls, no fragmentation, no per-object allocator metadata. A 4 MB ceiling is plenty for every program we've measured (peak ~120 KB at fibo28).

Δ payloads are *unbounded* in principle (a user could put a 1 GB byte string in source) and rarely change. The heap-side `Arena` handles those without bloating the stack buffer, and dedup means literal-heavy programs (think a million `0` bytes) collapse to one stored copy.

### Runtime — `morph`, `dataize`, `exec`

- **`morph(off, context)`** converts any object to a Formation by resolving dispatches, applications, and contexts. The core evaluation step. Implemented as a trampoline (`morph_g` is a generator-shaped state machine driven by an explicit work-stack inside `morph`) so the native call stack stays small even when morphing 30-level-deep fibonacci frames. Without the trampoline, fibo20 would blow Rust's default thread stack.
- **`dataize(off, scope, gc_enabled)`** extracts raw bytes. Calls `morph`, then walks `φ` / `λ` attributes in a `while(true)` loop, updating the current offset in place (every recursive path in the JS shape was tail-recursive). For `λ`-bearing formations it delegates to `atoms.rs`.
- **`exec(op)`** runs the `COPY` / `SET` operations the JS runtime used; in Rust they're inlined call sites inside `morph` and `dataize` but the same write-tracking happens here.

**Why a trampoline instead of native recursion?**

EO programs nest. Each formation has attrs that may dispatch through other formations whose `φ` may dispatch through still more. For fibo28 the dispatch chain on the way down is ~30 deep, multiplied by the ~832 040 tree leaves. With native recursion, each frame holds a `MorphFrame` (~80 bytes) — 30 × 832 040 frames at the deepest moment is ~2 GB of stack, far beyond any sensible limit. The trampoline puts those frames on a heap-allocated `Vec<MorphFrame>` instead, which Rust can grow as needed.

### Garbage collector — mark-compact with watermark

Mark-compact, triggered inline at two sites:

- `gc_phi` — runs after `dataize` resolves `φ` from a phi-point. Marks live, compacts the range above the phi-point's scope.
- `gc_disp` — runs after `morph` dispatches through `φ`. Marks from both endpoints (`from` and the newly-produced phi), compacts the range above `from`.

Mark uses an explicit DFS worklist (a `Vec<u32>` used as a LIFO stack) with double-push guards (`if obj.stay { continue }`). Compact has three phases:

1. **Plan** — advance `cursor` past in-place objects; set `obj.fwd = dest` on live ones; `del` garbage.
2. **Update refs** — scan the compaction range and rewrite refs through `remap_refs(obj, |r| ...)`. Outside-range objects with dynamic refs (the `ref_holders` set populated by COPY / SET / cache writes) get updated through their `written_attrs` index, which scopes the rewrite to attrs that were actually written.
3. **Move** — copy objects to `obj.fwd` destinations left-to-right (safe because destinations ≤ sources), then `trim()` trailing nulls.

A global `phi_watermark` tracks the highest phi-point index seen, preventing nested GC from re-collecting objects still live in an outer scope.

**Why mark-compact specifically?**

Three alternatives were considered and rejected:

- *Reference counting.* EO's φ-cache can introduce ρ-cycles (cache result holds parent via ρ, parent holds cache). Naive Rc would leak. Cycle-collecting Rc costs as much as tracing GC.
- *Generational copying.* Most EO programs have very short-lived objects (every dispatch creates temporaries), so a young/old split is natural — but it doubles the buffer requirement (semi-spaces) and adds a write barrier on every cross-generational ref. Not worth it for sub-millisecond peak memory.
- *Free list / pool.* Defragmentation by hand. Adds per-block free/used tracking, doesn't reclaim contiguous space, doesn't help cache locality. Mark-compact gives all that for free at the cost of one pass.

The downside of mark-compact: every collection is O(live objects in range). For fibo28 that's ~1640 objects scanned per `gc_phi`, called many times — visible in the per-collection time. But peak memory stays bounded (~120 KB) and the GC never pauses for milliseconds.

### Atoms

`atoms.rs` exports a map from `λ`-attribute string (e.g., `"L_number_plus"`) to `fn(&mut Runtime, self_off) -> u32`. Each atom dataizes its operand(s) through `Runtime::dataize`, performs the primitive operation on the Δ bytes, and returns the offset of a new number formation. `Runtime::from_image` builds the `atom_fns: Vec<fn>` parallel-indexed with `atoms.rev` (the interner's id → name table) so dispatch is a single bounds-checked array lookup.

## Performance

From [`../bench/results.md`](../bench/results.md), measured on this crate:

| Program | Result | Iters | Mean | Peak mem | Peak objs |
|---------|-------:|------:|-----:|---------:|----------:|
| fibo (8) | 21 | 10 | 1.6 ms | 14.9 KB | 250 |
| fibo15 | 610 | 10 | 75.6 ms | 41.8 KB | 610 |
| fibo20 | 6765 | 5 | 1.12 s | 64.6 KB | 916 |
| fibo28 | 317811 | 1 | 77 s | 118.4 KB | 1640 |

Peak memory grows sub-linearly in fibo argument because the GC reclaims aggressively at every dispatch. The relative cost vs. fully-compiled OO is ~1000× (Java JIT) and vs. interpreted OO ~40× (Java -Xint).

## Pros and trade-offs

### What this design buys you

- **Predictable memory.** Peak object-buffer size grows sub-linearly with input. No mystery heap growth.
- **No allocator-induced pauses.** GC is inline-deterministic; the longest pause is one compact phase scaled to the current live set (sub-millisecond at fibo28's working set).
- **No third-party dependencies.** Cargo lockfile is trivial; auditing the runtime is a finite-time exercise.
- **Movability for free.** GC compaction is a `memcpy` per object plus an integer rewrite per ref. No `unsafe` pointer arithmetic across object boundaries.
- **Faithful to the spec.** Every quirk of the JS prototype (implicit ρ slots, cache flags, the `xi` short-circuit) shows up identically here.

### What this design costs

The interpreter is ~40× slower than Java -Xint on the same OO Fibonacci tree. The headline costs:

1. **Linear attr name lookups.** `frm_slot_of(off, name_id)` is a sequential scan over the FRM's name table — fine for the typical 2-6 attrs per formation, but unavoidable per dispatch. Java's interpreter has direct field offsets.
2. **Multi-step morph per primitive.** A single `5.plus 6` in source triggers ~10 morph steps (cache check → copy → set ρ → dataize → atom invoke → arena push → wrap in number → wrap in bytes). Java's bytecode interpreter does a few `invokevirtual` + `iadd`.
3. **GC bookkeeping per object touch.** Every `exec_set` updates `ref_holders` (HashSet insert) and `written_attrs` (bit set). Java's interpreter does none of this.
4. **HashMap-based interners.** Attribute name and atom name lookups go through Rust's `HashMap`. Java internalizes strings at parse time into `intern` tables that are faster.
5. **Trampoline overhead.** Each morph step is a `Vec::push` + match — a function call's worth of work without being a function call. Removing it would blow the stack; tightening it is hard without rewriting the morph state machine.

The full decomposition is in [`../bench/results.md`](../bench/results.md). The combined effect on small-N fibos is dominated by costs 1 and 2; on large-N by costs 3 and 4 (cache pressure).

## Heap-minimization roadmap

Eight phases described inline in `../README.md`'s "Heap Minimization Plan" section. Summary of where the leverage is:

| Phase | What | Expected win |
|-------|------|--------------|
| 1 | Numeric attr ids in DSP/APP (eliminate name lookups) | 2-3× |
| 2 | Pre-allocated morph state-machine arena (no per-call `Vec::push`) | 1.2× |
| 3 | Bitmap `written_attrs` (replace HashSet) | 1.2× |
| 4 | COW-style binding chains (no HashMap clone per Application) | 1.5× |
| 5 | Skip `morph` when target is already a number formation | 1.5× |
| 6 | Inline number-atom fast paths (avoid arena round-trip) | 2× |
| 7 | Specialized FRM layouts for stdlib shapes | 1.5× |
| 8 | Per-thread arena (currently global with Mutex) | 1.1× — only if threading lands |

Stacked, these would put the interpreter near Java -Xint speed (within 2-3×). Whether that's a *good use of time* depends on whether `../rust-compiler/` can take the same programs to native AOT first — if yes, the interpreter just stays as the spec and these optimizations are nice-to-have.

## Known suboptimalities (13 specific items)

Inline in [`../README.md`](../README.md) under "Known Suboptimalities." Each item is small and well-scoped. None requires rewriting the architecture.

## Source map

| File | What it owns |
|------|--------------|
| `src/block.rs` | type tags, flags, sentinels, offset helpers |
| `src/memory.rs` | `Memory<'a>` (borrowed object buffer) + `Arena` (Δ heap with dedup) |
| `src/loader.rs` | DSL parser + Image builder + interner |
| `src/runtime.rs` | `Runtime<'a>` — morph, dataize, GC, exec, Stats |
| `src/atoms.rs` | atom function table |
| `src/main.rs` | `eo-load` CLI |
| `src/bin/bench.rs` | benchmark binary |
| `tests/loader.rs` | DSL parsing + image construction |
| `tests/runtime.rs` | end-to-end program execution |

## Related

- [`../resources/program.js`](../resources/program.js) — the JavaScript reference implementation.
- [`../CLAUDE.md`](../CLAUDE.md) — runtime semantics in prose (the source of truth for what `morph` / `dataize` / GC should do).
- [`../rust-compiler/`](../rust-compiler) — the AOT compiler experiment built on top of the same DSL. ~40 000× faster than this interpreter for programs it can fully fold; ~42× faster than Java JIT on the same fibo. See [`../bench/results-aot.md`](../bench/results-aot.md).
