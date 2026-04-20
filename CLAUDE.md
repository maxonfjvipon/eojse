    # CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Keeping docs in sync

After any change that affects architecture or naming, update both `README.md` and `CLAUDE.md` before considering the task done:

- **Renamed a function or variable** — update every mention of the old name in both files.
- **Changed how a concept works** (GC phases, trigger sites, data structures, object fields) — update the relevant section in `README.md` (under *Garbage Collection* or the affected concept) and the matching description in `CLAUDE.md` (under *Runtime*).
- **Added or removed a top-level function** — add or remove it from the *Runtime* section in `CLAUDE.md` and from `README.md` if it is described there.

## Commands

```bash
npm test          # compile and run all active test programs (~11s each)
bash run.sh fibo  # run a previously compiled program directly (no recompile)
```

To run a single program, comment/uncomment its name in `test/test_programs.js` under the `programs` array. Currently active: `rec`, `fibo`.

Each test compiles an EO source (from `test-resources/<name>.yaml`) through the full pipeline and asserts the printed `data:` value matches `expected`.

## Architecture

### The pipeline (what `npm test` does)

1. **EO → XMIR**: `eoc parse` compiles `.eo` source to XML IR (XMIR)
2. **XMIR → XMIR**: a chain of XSLT transforms in `resources/*.xsl` progressively flatten and lower the IR
3. **XMIR → JS**: `to-js.xsl` emits a JS snippet of `memory.push(...)` calls
4. **Inject + run**: the snippet is spliced into `resources/program.js` at the `// OBJECTS` marker, then executed with `node`

The final `program.js` is self-contained — it inlines the program objects and the full runtime.

### Runtime (`resources/program.js` + `resources/helpers.js`)

`helpers.js` exports constants (`FORMATION`, `DISPATCH`, `APPLICATION`, `PHI`, `RHO`, `DELTA`, `LAMBDA`, flags) and the shared `memory` array.

`program.js` is the runtime engine. Key concepts:

**Memory** — flat integer-indexed array. Objects sit at sequential indices. `push` appends, `pop` removes the tail (O(1)), `del(idx)` is the single deletion interface (nulls slot, decrements `live_count`, removes from `ref_holders`), `trim()` shrinks trailing nulls after bulk deletes.

**Object types** — every object is one of `FORMATION` (has a `target` attrs-map), `DISPATCH` (target index + attr name), or `APPLICATION` (target index + attr name + value index).

**`morph(index, context, remove)`** — converts any object to a Formation by resolving dispatches and applications recursively. This is the core evaluation step.

**`dataize(index, scope, gc_enabled)`** — extracts raw bytes from an object. Calls `morph` internally, recurses through `φ` and `λ` attributes.

**`exec(op)`** — executes `COPY` or `SET` operations, updating `ref_holders` and `written_attrs`.

### Garbage Collector

The GC is mark-compact, triggered inline at two sites:

- **`gc_phi(gc_enabled, value, scope)`** — called in `dataize` after resolving `φ`. Runs `mark_phi` then `compact(scope+1, value, value)`.
- **`gc_disp(from, phi)`** — called in `morph` after dispatching through `φ`. Runs `mark_disp` from both endpoints then `compact(from, phi, phi)`.

**Mark** — `mark_phi` and `mark_disp` both delegate to `mark(index, in_range, recurse)`, a recursive DFS that sets `obj.stay = true` on live objects. `attr_ref` selects the effective outgoing ref per attribute (`cache` > `xi` > nothing).

**Compact** (`compact(from, to, pivot)`) — three phases:
1. **Plan**: advance `cursor` past in-place objects; set `obj.fwd = dest` on live objects; `del` garbage.
2. **Update refs**: scan `[from, end]` via `remap_refs(obj, r)`; update outside-range objects via their `written_attrs`. The remap function `r` has a bounds check (`idx < first_dest || idx > end`) to skip memory access for out-of-range refs.
3. **Move**: copy objects to `obj.fwd` destinations left-to-right (safe: destinations ≤ sources); `trim()`.

Returns the remapped `pivot` index.

**`ref_holders`** — `Set<index>` of objects anywhere in memory that hold dynamic refs (populated by `exec COPY/SET` and cache writes). Iterated on every compact to update outside-range objects. Rebuilt as `next_holders` after each compact.

**`written_attrs`** — `Set<attr_name>` on every object. Tracks which specific attrs were written (SET, cache, copy, or moved by a previous compact). The compact ref_holders loop iterates only `written_attrs` instead of all attrs — critical for objects with many attributes.

**`phi_watermark`** — global high-water mark of the highest phi-point index seen. Prevents nested GC from re-collecting objects still live in an outer scope.

### Adding a new atom

Add an entry to the `atoms` map in `program.js` with key matching the `λ` attribute value (e.g. `'L_number_plus'`). The function receives `self` (the index of the formation being dataized) and must return the index of a result formation.

### Adding a new test program

1. Create `test-resources/<name>.yaml` with `program:` (EO source) and `expected:` (the expected numeric result as a string).
2. Add `'<name>'` to the `programs` array in `test/test_programs.js`.
