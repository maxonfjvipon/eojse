# Flat EOLANG runtime emulator

This is the JS emulator of [EOLANG](https://github.com/objectionary/eo) runtime
which tries to map the object-oriented paradigm to imperative operations.

## Why it exists

OOP is slow, pure OOP is even slower. Can it be faster? We're sure it can.
The only way to make pure OOP language as fast as procedural one - impose
restrictions on the language, which allow to present complicated manipulations
with objects as set of imperative instructions and effectively store them in
linear memory.

We believe that EOLANG has such restrictions. On one hand, the language is
quite simple and does not contain huge amount features which most famous OOP
languages have (classes, types, mutability, etc.), on other hand, it's still
pure object-oriented.

## The main idea and key concepts

### Memory

Memory is a flat integer-indexed array (`memory[]`). Objects are laid out
sequentially, one after another, each at a numeric index. There is no
separation into stack and heap.

`push` places a new object at the next available index.
`del(idx)` is the single deletion interface: it nulls the slot, increments
`total_deleted`, decrements `live_count`, and removes `idx` from `ref_holders`.
All object deletions go through `del` so no call site can forget any side-effect.
`live_count` is an O(1) integer counter of currently live objects; `push`
increments it and `del` decrements it, replacing the former O(n) `memory_size()`
scan that previously ran on every push.
`pop` calls `del(head())` and then decrements `memory.length` directly — O(1),
no scan needed, because `pop` always targets the last element so exactly one
trailing null is created. `trim()` is only called inside `compact`, where the
move phase can leave multiple trailing nulls.

### Objects

EO objects stored in memory can be one of three types:

- Formation
- Application
- Dispatch

Each object is a JS object of the following structure:

```javascript
Object = {
  name:          String,       // human-readable label for debugging
  type:          String,       // "FRM" | "APP" | "DSP"
  target:        Object|Number,// formation target (attrs map) or index of target object
  attr:          String|Number|null, // attr name/index for Application or Dispatch
  value:         Number|null,  // index of the value object for Application
  written_attrs: Set<String>,  // attrs that were explicitly written or moved by GC
  stay:          Boolean|null, // GC mark flag: true = live, null = dead
  fwd:           Number|null,  // GC forwarding pointer: destination index during compact
}

// Formation target: a plain JS object where keys are EO attribute names
Formation = {
  "attr1": Attribute,
  "attr2": Attribute,
  ...
}

Attribute = {
  value: Number,      // index of the referenced object in memory
  xi:    Number|null, // context index (when context differs from the object owning this attr)
  cache: Number|null, // cached result of dispatching this attr (set after first resolution)
}
```

Consider this EO program:

```
[] > foo
  $.x Q.y > @
  [] > x
```

The `memory` for that program looks like this (fields with `null` are ignored):

```javascript
{
  0: {name: "Q",      type: "FRM", target: {"foo": {value: 1}}},
  1: {name: "foo",    type: "FRM", target: {"@": {value: 2}, "x": {value: 5}}},
  2: {name: "$.x Q.y",type: "APP", target: 3, attr: 0, value: 4},
  3: {name: "$.x",    type: "DSP", target: -1, attr: "x"},
  4: {name: "Q.y",    type: "DSP", target: 0,  attr: "y"},
  5: {name: "x",      type: "FRM", target: {}}
}
```

As you see, each object has a rigid structure so we may definitely say what size
in bytes it has. This is important for the future Rust implementation.

### Dataization

Dataization is the core process in EOLANG which extracts raw byte data from
the program. Data in EOLANG is a sequence of bytes attached to the special `Δ`
attribute of a Formation:

```javascript
42: {name: "?", type: "FRM", target: {"Δ": {value: [0x00, 0x01, ..., 0x10]}}}
```

Dataization of a Formation:

1. If `Δ` is present — return its byte array directly.
2. If `φ` is present — dispatch `φ`, get a new Formation, dataize it recursively.
3. If `λ` is present — execute the named atom, get a result Formation, dataize it.
4. Otherwise — throw an error.

Dataization of a Dispatch or Application first morphs it to a Formation, then
applies the rules above.

### Morphing

Morphing converts any object to a Formation. Morphing a Formation returns it
unchanged. Morphing a Dispatch resolves the target and attribute, possibly
copying the resolved Formation and attaching `ρ` (parent context). Morphing an
Application resolves the target, then sets the specified attribute on it.

### Copying

When morphing a Dispatch resolves an attribute to a Formation that does not
already have `ρ` set, a shallow copy of that Formation is made and `ρ` is
attached to the copy. Attributes inside the Formation are stored as index
references, so copying an object does not recursively copy nested objects — only
the top-level attribute map is cloned, with all attribute values remaining as
index references to existing memory slots.

### Caching

After a Dispatch resolves an attribute for the first time, the result index is
stored in `attr.cache`. Subsequent dispatches of the same attribute on the same
object return the cached index directly, skipping re-morphing. The object that
holds the cache entry is added to `ref_holders` and the specific attribute name
is added to its `written_attrs` so the GC can update the cached index if the
referenced object moves.

---

## Garbage Collection

During dataization, new objects are pushed to memory continuously. Objects
become unreachable once the dataization step that created them completes.
The emulator uses an inline **mark-compact** GC triggered at specific points
during evaluation. There is no separate GC thread or stop-the-world pause
beyond the inline compact call itself.

### GC Trigger Points

There are two trigger sites, both identified by the structure of the EO
evaluation model:

**`gc_phi(gc_enabled, value, scope)`** — called inside `dataize` each time a
`φ` attribute is resolved and the result Formation is obtained. If there is a
gap between `scope` (the last known live boundary) and `value` (the index of
the new result), objects in `[scope+1, value]` may be garbage. It runs:

```
mark_phi(value, scope)                 // mark live objects reachable from 'value'
value = compact(scope+1, value, value) // compact, remap caller's pivot
```

**`gc_disp(from, phi)`** — called inside `morph` after a Dispatch
resolves through `φ`. It marks and compacts the range `[from, phi]`:

```
mark_disp(from, from, phi)  // mark live objects reachable from 'from'
mark_disp(phi,  from, phi)  // mark live objects reachable from 'phi'
return compact(from, phi, phi)
```

### Mark Phase

Two marking functions share a single `mark(index, in_range, recurse)` helper:

```javascript
const mark = (index, in_range, recurse) => {
  memory[index].stay = true
  Object.keys(memory[index].target).forEach((at) => {
    const ref = attr_ref(memory[index].target[at])   // cache ?? xi ?? null
    if (ref != null && in_range(ref) && memory[ref] != null && !memory[ref].stay)
      recurse(ref)
  })
}
```

`attr_ref` picks the "real" outgoing reference for an attribute: `cache` if
set, else `xi`, else nothing. Plain `value` references (pointing to
program-level objects that never move) are not followed during marking.

`mark_disp(start, from, to)` — DFS from `start`, follows refs in `[from, to]`.
Called twice by `gc_disp` to seed from both endpoints of the range.

`mark_phi(index, scope)` — DFS from `index`, follows refs strictly within
`(scope, phi_watermark)`. `phi_watermark` is a global watermark tracking the highest
phi-point index ever seen, preventing nested GC from re-collecting objects
still live in an outer GC scope.

After marking: every live object in the range has `stay = true`. Dead objects
have `stay = null/false`.

> **Future work:** `mark_phi` and `mark_disp` are currently recursive DFS. For
> programs with deep object graphs this risks a JS stack overflow. Replacing the
> recursion with an explicit iterative worklist (an array used as a stack) would
> eliminate that risk and give more predictable performance. The Rust port already
> plans this — see *Mark Phase — Iterative DFS* in the Rust section below.

### Compact Phase

`compact(from, to, pivot)` runs in three sub-phases and returns `pivot` remapped
to its new position.

#### Sub-phase 1 — Plan (forwarding pointers)

```
cursor = from
advance cursor past leading in-place live objects, clearing their 'stay' flag
if cursor > end: nothing to do, return pivot unchanged

first_dest = cursor
dest = cursor
for i = cursor .. end:
  if memory[i] is null: skip
  if memory[i].stay:
    memory[i].stay = null
    memory[i].fwd = dest   // record destination on the object itself
    dest++
  else:
    del(i)                 // null slot, increment total_deleted, remove from ref_holders
```

After this sub-phase: live objects know their destination via `obj.fwd`. Garbage
slots are null. **No object has moved yet.**

The key design choice is storing the forwarding pointer **on the object itself**
(`obj.fwd`) rather than in a separate `Map`. This means the remap function is:

```javascript
const r = (idx) => {
  if (idx < first_dest || idx > end) return idx
  const obj = memory[idx]
  return (obj != null && obj.fwd != null) ? obj.fwd : idx
}
```

The bounds check short-circuits before touching memory for any ref outside
`[first_dest, end]` — the moving zone. Most refs point to program-level objects
or objects above `end`, so the early return fires for the majority of calls.
Since objects have not moved yet, `memory[idx]` still holds the object at its
old position during sub-phase 2.

#### Sub-phase 2 — Update References

All live objects inside `[from, end]` are still at their old positions. Scan
them and rewrite every attribute reference through `r`:

```
for i = from .. end:
  if memory[i] != null: remap_refs(memory[i], r)
```

`remap_refs` rewrites `cache`, `xi`, and `value` fields of each attribute.
It only performs the rewrite when the value actually changes (`r(old) != old`),
and when it does, it also adds that attribute name to `obj.written_attrs` — so
future GC passes know to re-check that attribute when the object is outside the
compact range.

Then handle **objects outside `[from, end]`** that hold refs into the range.
These are tracked in `ref_holders`. For each:

```
for idx in ref_holders:
  cur = r(idx)                   // new position of this ref_holder object
  obj = memory[idx]              // object is still at old position
  if obj is null: skip (was garbage-collected)
  next_holders.add(cur)
  if idx is outside [from, end]:
    for at in obj.written_attrs: // only check attrs we know were written
      update attr obj.target[at] through r
ref_holders = next_holders
```

Using `written_attrs` here is the key optimization: instead of checking every
attribute of every ref_holder object (which could be large), only the specific
attributes that were ever explicitly written (via SET, cache, or copy) are
checked.

Finally, remap `phi_watermark` and compute the return value:

```
phi_watermark = r(phi_watermark)
result = r(pivot)
```

#### Sub-phase 3 — Move

Now physically move objects to their planned destinations:

```
for i = first_dest .. end:
  obj = memory[i]
  if obj is null or obj.fwd is null: skip
  dst = obj.fwd
  memory[dst] = obj
  if dst != i: memory[i] = null
trim()
```

Scanning left-to-right is safe because destinations are always ≤ sources
(compacting leftward). A source is never overwritten before it is read.

`trim()` shrinks `memory.length` to remove trailing nulls, keeping `head()` O(1).

`obj.fwd` is **not** cleared after the move. After the move, `obj` sits at
`memory[dst]` and `obj.fwd == dst`. In any future compact, `r(dst)` reads
`memory[dst].fwd == dst` and returns `dst` — identical to the result if `fwd`
were null. If the object needs to move again in a future compact, sub-phase 1
overwrites `fwd` with the new destination anyway.

### ref_holders

`ref_holders` is a `Set<index>` tracking every object anywhere in memory that
holds dynamic references — references that might point into a future compact
range. An object is added to `ref_holders` when:

- A COPY is performed (the clone may hold refs inherited from the source)
- A SET is executed on it
- A cache entry is written into one of its attributes

During each compact, `ref_holders` is rebuilt as `next_holders` with all
indices remapped through `r`. Objects whose slots are null (garbage-collected
since the last compact) are dropped automatically.

`ref_holders` answers **who** to update during sub-phase 2. Without it, every
compact would require a full scan of all memory to find outside objects with
relevant refs — O(total\_memory) instead of O(|ref\_holders|).

### written_attrs

`written_attrs: Set<String>` is a field on every object. It records which
specific attributes of that object were written or moved by the GC:

| Event | Effect |
|-------|--------|
| `exec(SET)` on attr `a` | `obj.written_attrs.add(a)` |
| Cache written for attr `a` | `obj.written_attrs.add(a)` |
| `exec(COPY)` | clone inherits source's `written_attrs`; all cloned attr names added |
| Range scan moves a ref in attr `a` | `obj.written_attrs.add(a)` |

`written_attrs` answers **what** to update during the ref_holders loop in
sub-phase 2. For an object with many attributes (only a few of which hold
dynamic refs), iterating `written_attrs` is much cheaper than iterating all
attributes.

`ref_holders` and `written_attrs` are complementary, not alternatives:
`ref_holders` finds the right objects, `written_attrs` updates only the right
attributes inside each of those objects.

### Statistics

After each program run the following counters are printed:

| Stat | Meaning |
|------|---------|
| `original program size` | number of objects in memory before evaluation starts |
| `total created` | total `push` calls during evaluation |
| `total created without program` | above minus program size |
| `total deleted` | objects nulled by GC (garbage) plus explicit `pop` calls |
| `max depth` | peak live object count at any single moment |
| `max depth without program` | above minus program size |
| `max ref holders` | peak size of the `ref_holders` set |

---

## Future Rust Implementation

This JS emulator is a proof-of-concept. The final implementation will be
written in Rust and will operate directly on machine memory. The algorithmic
choices made here were designed with that target in mind.

### Memory Layout

Memory is a contiguous `Vec<Object>` (or raw `*mut Object` pointer array with
a length counter). Each `Object` is a fixed-size struct — EO's restrictions
eliminate dynamic dispatch and variable-size layouts. Null slots are represented
by a sentinel value (e.g. a dedicated tag bit or a `type` field value of 0)
rather than `Option<Object>`, to avoid the extra indirection.

`trim()` becomes a single store to the length counter — O(1) with no iteration.

### Attributes

In Rust, attribute names are compile-time integer offsets into a fixed-size
attribute array embedded directly in the `Object` struct. There are no string
keys and no hash lookups.

```rust
struct Attribute {
    value: u32,        // index into memory array
    xi:    u32,        // context index; u32::MAX = absent
    cache: u32,        // cached result index; u32::MAX = absent
}

struct Object {
    ty:            u8,         // Formation / Dispatch / Application
    stay:          bool,       // GC mark flag
    fwd:           u32,        // forwarding pointer; u32::MAX = absent
    written_attrs: u64,        // bitmask: bit k = attr at offset k was written
    attrs:         [Attribute; MAX_ATTRS],
}
```

### written_attrs as a Bitmask

In JS, `written_attrs` is a `Set<String>`. In Rust it becomes a `u64` bitmask
where bit `k` corresponds to the attribute at offset `k`. This eliminates all
heap allocation for this field.

Setting a bit:
```rust
obj.written_attrs |= 1u64 << attr_offset;
```

Iterating set bits (O(popcount), no branching per zero bit):
```rust
let mut mask = obj.written_attrs;
while mask != 0 {
    let k = mask.trailing_zeros() as usize;
    mask &= mask - 1;   // clear lowest set bit
    update_attr(&mut obj.attrs[k], r);
}
```

For objects with more than 64 attributes, extend to `u128` or `[u64; N]`.

### Forwarding Pointer

`fwd: u32` with `u32::MAX` as the null sentinel. No `Option<>` wrapper needed.
The remap function is a single bounds check plus one array access:

```rust
fn r(idx: u32, first_dest: u32, end: u32, memory: &[Object]) -> u32 {
    if idx < first_dest || idx > end { return idx; }
    let fwd = memory[idx as usize].fwd;
    if fwd != u32::MAX { fwd } else { idx }
}
```

The bounds check `idx < first_dest || idx > end` short-circuits for the
majority of references that point outside the compact range, avoiding even the
array access for those cases.

### ref_holders

In Rust, `ref_holders` is a `Vec<u32>` — a flat array of memory indices,
rebuilt after each compact (filter nulled entries, remap surviving indices).
No hashing, no pointer chasing, cache-friendly sequential scan.

For programs with very large `ref_holders`, a generational approach (see below)
dramatically reduces the size of the set that needs iterating.

### Mark Phase — Iterative DFS

The recursive `mark_disp` / `mark_phi` DFS in JS becomes an iterative DFS
using a pre-allocated `Vec<u32>` worklist in Rust, eliminating call stack
overhead and stack overflow risk for deep object graphs:

```rust
fn mark(seeds: &[u32], in_range: impl Fn(u32) -> bool, memory: &mut [Object]) {
    let mut stack: Vec<u32> = seeds.to_vec();
    while let Some(idx) = stack.pop() {
        let obj = &mut memory[idx as usize];
        obj.stay = true;
        for attr in obj.attrs.iter() {
            let ref_ = effective_ref(attr);   // cache ?? xi ?? skip
            if ref_ != u32::MAX && in_range(ref_) && !memory[ref_ as usize].stay {
                stack.push(ref_);
            }
        }
    }
}
```

### Generational GC (Future Work)

The current design already behaves generationally in practice: `gc_phi`
and `gc_disp` compact small, recently-allocated windows of memory.
Old objects at low indices are rarely inside a compact range.

A formal generational boundary would divide memory into a young region (recent
allocations) and an old region (stable objects). Most compacts would touch only
the young region. Objects that survive several young-region compacts get
promoted to the old region and are only collected during infrequent full
compacts.

This would reduce both the compact range size and the `ref_holders` iteration
cost. The `written_attrs` bitmask already gives the per-attribute precision
needed to efficiently maintain cross-generational references (old objects
holding refs into the young region) — equivalent to a card table but at
single-attribute granularity rather than 64-object-card granularity.

---

## How to play

Test programs are placed inside `test-resources` directory.
To disable or enable a specific program, you can comment/uncomment it
in `test/test_programs.js` file.

Then run `npm test`. It compiles EO program into JS, creates `temp` directory
with your programs inside and run them (takes around 11 seconds per each
program).

To see the full log and `memory` status for a specific program, e.g. `fibo`
you can do:

```bash
node temp/fibo/.eoc/program.js
```

The main runtime code is in `resources/program.js`.
Shared constants and utilities are in `resources/helpers.js`.

To add new `eo-runtime` objects you should extend `resources/runtime.xsl`.

## How to contribute

You need `node` installed on your computer.

Submit a PR with your changes. To avoid frustration please run `npm test`
before sending. Make sure all the tests pass.
