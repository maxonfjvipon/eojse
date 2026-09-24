# Flat EOLANG runtime emulator

This is the JS emulator of [EOLANG](https://github.com/objectionary/eo) runtime
which tries to map the object-oriented paradigm to imperative operations.

## Why it exists

OOP is slow, pure OOP is even slower. Can it be faster? We're sure it can.
The only way to make a pure OOP language as fast as a procedural one is to impose
restrictions on the language that allow presenting complicated manipulations
with objects as a set of imperative instructions and effectively storing them in
linear memory.

We believe that EOLANG has such restrictions. On the one hand, the language is
quite simple and does not contain many of the features most popular OOP
languages have (classes, types, mutability, etc.), on the other hand, it's still
pure object-oriented.

## The main idea and key concepts

### Memory

Memory is a flat integer-indexed array (`memory[]`). Objects are laid out
sequentially, one after another, each at a numeric index. There is no
separation into stack and heap.

`push` places a new object at the next available index.
`del(idx)` is the single deletion interface: it nulls the slot, increments
`total_deleted`, decrements `live_count`, and removes `idx` from `ref_holders`.
All object deletions go through `del`, so no call site can forget any side-effect.
`live_count` is an O(1) integer counter of currently live objects; `push`
increments it and `del` decrements it, replacing the former O(n) `memory_size()`
scan that previously ran on every push.
`pop` calls `del(head())` and then decrements `memory.length` directly — O(1),
no scan needed, because `pop` always targets the last element, so exactly one
trailing null is created. `trim()` is only called inside `compact`, where the
move phase can leave multiple trailing nulls.

### Objects

EO objects stored in memory can be one of four types:

- Formation
- Application
- Dispatch
- Context — the bare `$` self-reference. Evaluates to whatever object is
  the runtime context at the moment of evaluation. Carries no fields
  past the type tag; its only behavior is "return context."

Each object is a JS object of the following structure:

```javascript
Object = {
  name:          String,       // human-readable label for debugging
  type:          String,       // "FRM" | "APP" | "DSP" | "CTX"
  target:        Object|Number,// formation target (attrs map) or index of target object;
                               // for CTX it is an empty object (no outgoing refs)
  attr:          String|Number|null, // attr name/index for Application or Dispatch; null for CTX
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
  1: {name: "foo",    type: "FRM", target: {"φ": {value: 2}, "x": {value: 5}}},
  2: {name: "$.x Q.y",type: "APP", target: 3, attr: 0, value: 4},
  3: {name: "$.x",    type: "DSP", target: -1, attr: "x"},
  4: {name: "Q.y",    type: "DSP", target: 0,  attr: "y"},
  5: {name: "x",      type: "FRM", target: {}}
}
```

As you see, each object has a rigid structure, so we may definitely say what size
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
Morphing a Context returns the runtime context — it is a leaf operation with
no sub-call (no target, no attribute, no children).

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
is added to its `written_attrs`, so the GC can update the cached index if the
referenced object moves.

### Tuning Flags

`helpers.js` exposes a small set of compile-time switches that control
runtime behavior. They are documented here so the Rust port preserves
their intent (likely as `cfg(feature = ...)` toggles or runtime
booleans, depending on the flag).

| Flag                  | Default | Effect                                                                                                                  |
|-----------------------|---------|-------------------------------------------------------------------------------------------------------------------------|
| `REMOVE_UNNECESSARY`  | `true`  | Inside `morph`, pop the just-consumed DSP/APP off `memory` before producing the result. Off → intermediate objects leak. |
| `USE_CACHE`           | `true`  | Attribute resolution caching (the `attr.cache` field). Off → every dispatch re-morphs the attribute value.              |
| `COPY_ON_APPLICATION` | `false` | When morphing an APPLICATION, always `COPY` the target before `SET`. Off → applications mutate the target in place.    |
| `GC_ENABLED_DEFAULT`  | `false` | Default for `dataize`'s `gc_enabled` arg. Main dataization passes `true` explicitly; atom-internal dataizations get this default. See [The `gc_enabled` flag](#the-gc_enabled-flag). |
| `HIDE_XI`             | `false` | Debug print: hide `xi` values in `print_memory` output to reduce noise.                                                 |

The "true" defaults (`REMOVE_UNNECESSARY`, `USE_CACHE`) are
correctness-essential in practice — leaving them off works but produces
substantially larger memory traces. The "false" defaults are off either
because the alternative semantics are wrong for the current pipeline
(`COPY_ON_APPLICATION`) or because they are scoped controls
(`GC_ENABLED_DEFAULT`, `HIDE_XI`).

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

### The `gc_enabled` flag

`gc_phi` is gated by an explicit `gc_enabled` argument; `gc_disp` is not.
The asymmetry is intentional.

`gc_enabled` is `true` only on the **main dataization thread** — the
top-level `dataize(0, head(), true)` call that drives the whole program.
Every other `dataize` call — specifically the ones atoms make to evaluate
their sub-expressions (`L_number_plus`, `L_number_gt`, `L_number_times`)
— uses the default `GC_ENABLED_DEFAULT = false` and therefore skips
`gc_phi`. Atoms hold intermediate object indices in plain JS locals that
the GC cannot see; if `gc_phi` ran during an atom's internal dataizations
those locals could be invalidated under the atom. Disabling phi-GC inside
atoms is the simplest way to keep their stack-local indices stable
without teaching the GC about JS-side roots.

`gc_disp` runs unconditionally because its trigger site (`morph` after a
`φ` dispatch) only fires on objects the morph state machine itself owns:
all live indices needed past the call are returned as the dispatch
result, and `gc_disp` remaps the pivot through its forwarding pointers
before returning. There are no off-machine roots to worry about.

**Why `gc_disp` is on the φ path but not the λ path.** Inside the DSP
case of `morph`, the φ-dispatch branch calls `gc_disp` twice (once
after resolving `φ`, once after dispatching the requested attribute on
the result). The λ-dispatch branch — which invokes an atom and then
dispatches the requested attribute on the atom's result — does not call
`gc_disp` at all. This is intentional: atoms execute as plain Rust/JS
functions and hold their intermediate indices in native local
variables that the GC cannot see, exactly the situation
`GC_ENABLED_DEFAULT = false` exists to handle. Compacting during the λ
path would invalidate those locals. The φ path, by contrast, has no
such hidden roots — every live index is on the explicit morph stack —
so `gc_disp` is safe and beneficial there.

### Mark Phase

Two marking functions share a single iterative `mark(seed, in_range)` helper
that uses an explicit worklist (a JS array used as a LIFO stack):

```javascript
const mark = (seed, in_range) => {
  const stack = [seed]
  while (stack.length > 0) {
    const index = stack.pop()
    const obj = memory[index]
    if (obj.stay) continue
    obj.stay = true
    Object.keys(obj.target).forEach((at) => {
      const ref = attr_ref(obj.target[at])              // cache ?? xi ?? null
      if (ref != null && in_range(ref) && memory[ref] != null && !memory[ref].stay) {
        stack.push(ref)
      }
    })
  }
}
```

`attr_ref` picks the "real" outgoing reference for an attribute: `cache` if
set, else `xi`, else nothing. Plain `value` references (pointing to
program-level objects that never move) are not followed during marking.

`mark_disp(start, from, to)` — iterative DFS from `start`, follows refs in
`[from, to]`. Called twice by `gc_disp` to seed from both endpoints of the
range.

`mark_phi(index, scope)` — iterative DFS from `index`, follows refs strictly
within `(scope, phi_watermark)`. `phi_watermark` is a global watermark
tracking the highest phi-point index ever seen, preventing nested GC from
re-collecting objects still live in an outer GC scope.

After marking: every live object in the range has `stay = true`. Dead
objects have `stay = null/false`.

The `if (obj.stay) continue` guard at the top of the worklist loop is
necessary because a reference may be pushed more than once (from different
holders) before being processed; the guard ensures we don't re-walk an
already-marked subgraph.

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

After each program run, the following counters are printed:

| Stat | Meaning |
|------|---------|
| `original program size` | number of objects in memory before evaluation starts |
| `total created` | total `push` calls during evaluation |
| `total created without program` | above minus program size |
| `total deleted` | objects nulled by GC (garbage) plus explicit `pop` calls |
| `max depth` | peak live object count at any single moment |
| `max depth without program` | above minus program size |
| `max ref holders` | peak size of the `ref_holders` set |
| `gc_phi`  | `gc_phi_count` calls, `gc_phi_reclaimed` objects nulled by phi-GC |
| `gc_disp` | `gc_disp_count` calls, `gc_disp_reclaimed` objects nulled by disp-GC |

---

## Future Rust Implementation

This JS emulator is a proof-of-concept. The final implementation will be
written in Rust and will operate directly on machine memory. The algorithmic
choices made here were designed with that target in mind. This section is
the working spec for the port — decisions made here are binding; items
listed under *Open Questions* are not yet decided.

### Memory

`memory` models linear, stack-grown program memory. Objects are pushed
one after another in order of creation; the runtime accesses them by
byte offset. The backing storage is a single contiguous byte buffer:

```rust
memory: Vec<u8>
```

The choice of `Vec<u8>` is an implementation detail — it gives us a
heap-backed contiguous region and growth via `reserve` / doubling. The
conceptual model is "linear OS memory": a buffer with a high-water mark
that only moves up (push) or down (pop), with lazily reclaimed holes
inside (`del` → later `compact`).

Every reference everywhere in the runtime — `attr.value`, `attr.xi`,
`attr.cache`, `ref_holders`, `fwd`, `phi_watermark`, the operand of
`del`, the result of `head()` — is a `u32` **byte offset** into this
buffer. An object's identity *is* its byte offset; pointer arithmetic
(`memory.as_ptr().add(offset)`) gives O(1) access to the block at any
known offset.

**Operations:**

- `push(block)` writes the block starting at a `next_free: u32` cursor
  and advances the cursor by the block's size in bytes.
- `head()` returns the byte offset where the last-pushed block starts.
  Because blocks are variable-sized, `head` is tracked alongside
  `next_free` (e.g. as `prev_head` updated on each push).
- `pop()` removes the last-pushed block: calls `del(head())`, then
  rewinds `next_free` to that offset and `head` to the previous block's
  start. O(1).
- `del(offset)` sets the `dead` flag bit of the block at `offset`,
  decrements `live_count`, and removes `offset` from `ref_holders`. The
  block's `n` (attribute count for FRM) and shape are preserved so the
  compact walker can still step past it. The storage stays in place
  until `compact` reclaims it.

**Cursor rewind on `compact`.** After the move sub-phase of `compact`,
both `next_free` and `prev_head` may need to be rewound. Let `last_surv`
be the offset of the highest-offset block still alive after compaction.
Then:

- `next_free = last_surv + block_size(memory[last_surv])` — the byte
  immediately after the last surviving block.
- `prev_head = last_surv` — the start of the last surviving block.

If every block in `[from, end]` survives, both cursors are unchanged. If
the trailing blocks were all garbage, `next_free` retreats by the sum of
their sizes. This is the byte-level analog of `trim()` in the JS
prototype, which shrinks `memory.length` to the last non-null slot.

**Growth strategy.** When the DSL loader finishes initializing the
program objects, it knows the exact byte size of the initial program.
The runtime then calls `memory.reserve(program_bytes * K + FLOOR)` with
some headroom factor (`K ≈ 4`) and minimum (`FLOOR ≈ 1 MB`). After
that, `Vec<u8>`'s default doubling handles any further growth. This
covers the load pass without micro-reallocations and gives most
programs enough headroom to never grow again.

**Backing-storage profiles.** The runtime is designed with three profiles
for the backing storage of `memory`, all behind the same internal API:

- **Stack profile (fixed-size buffer on the thread stack).** The default
  profile for the current Rust implementation. The host declares a
  fixed-size byte array as a local in `main` (e.g. `let mut buf = [0u8;
  4 * 1024 * 1024];`) and hands a `&mut [u8]` slice to the runtime.
  `Memory` borrows that slice and tracks a `len: u32` cursor; pushes
  write into the slice, pops rewind the cursor. No allocator on the hot
  path. The Δ arena stays on the heap (per the design principle: heap
  is for byte data, stack is for objects). Size is fixed at startup;
  `push` panics on overflow. Suitable for programs whose peak working
  set is bounded and known.
- **Skeleton profile (`Vec<u8>` + reserve + doubling).** Portable
  fallback. Growth events reallocate the buffer (`memcpy` of all live
  bytes to a new heap address). If the OS refuses to allocate a larger
  contiguous region, the runtime aborts. Use when peak working set is
  not known up-front, or when running on a thread with a small stack.
- **mmap profile (mmap-reserved virtual range, lazy page commit).**
  The runtime reserves a large virtual address range at startup — e.g.
  several GB — without committing physical RAM (Linux/macOS:
  `mmap(MAP_NORESERVE | MAP_ANONYMOUS | MAP_PRIVATE)`; Windows:
  `VirtualAlloc(MEM_RESERVE)` + per-page `MEM_COMMIT`). The OS commits
  pages only when first written. The base pointer never moves; there
  is no growth event and no `memcpy`. Use when programs have an
  unbounded peak working set and you want allocator-free push.

The three profiles share one byte-offset interface. Switching between
them is local to the `Memory` struct — **nothing else in the runtime
changes**, because refs are `u32` byte offsets and never hold raw
pointers across a push (see below).

**Stack vs. mmap tradeoff.** Both eliminate allocator calls on push.
Stack wins on simplicity (no platform code, no syscalls) and warmth
(buffer pages are CPU-cache-resident). mmap wins on capacity (TBs of
virtual address space available; the OS commits pages lazily). For
small bounded programs the stack profile is preferred; for production
deployment with unbounded inputs, mmap is the right fit.

**Stack profile sizing.** On macOS and Linux the main thread's stack
defaults to 8 MB; on Windows, 1 MB. Spawned threads usually default to
2 MB. Buffers up to ~4 MB on macOS/Linux's main thread are comfortable;
larger buffers may need a custom thread with an enlarged stack
(`std::thread::Builder::stack_size`) or one of the other two profiles.

**Pointer-stability rule.** In the skeleton profile, the underlying
buffer may be moved by a realloc, so raw pointers derived from
`memory.as_ptr()` are short-lived and must not be held across a
`push`. Re-derive `memory.as_ptr().add(offset)` per access. Refs
(`u32` byte offsets) remain valid across reallocs since they are
buffer-relative. In the stack and mmap profiles this rule becomes
vacuous — the buffer never moves — but writing the runtime code to
obey it unconditionally keeps all three profiles interchangeable.

### Block Layout

Every object is a contiguous block that starts with an 8-byte common
prefix and continues with a per-variant tail.

**Common prefix (8 bytes, every variant):**

```text
offset 0: flags  (u8)   ─ bit-packed: see below
offset 1: pad    (u8)
offset 2: n      (u16)  ─ FRM: attr_count; DSP/APP: attr_name_id
offset 4: fwd    (u32)  ─ u32::MAX = absent
```

The `flags` byte:

```text
bits 0-1: ty           0 = CTX, 1 = FRM, 2 = DSP, 3 = APP
bit  2:   dead         1 = block has been del'd (storage not yet reclaimed)
bit  3:   stay         GC mark
bit  4:   from_atom    debug; produced by atoms, consumed only by print_memory
bit  5:   unbound      APP only: 1 = `n` is a literal slot index,
                                  0 = `n` is a global attr_name_id
bits 6-7: reserved
```

The `n` slot has variant-specific meaning:

| Variant            | Meaning of `n`                                          |
|--------------------|---------------------------------------------------------|
| CTX                | unused (0)                                              |
| FRM                | `attr_count`                                            |
| DSP                | global `attr_name_id` of the attribute being dispatched |
| APP, `unbound = 0` | global `attr_name_id` (bound: scan `name_ids` for slot) |
| APP, `unbound = 1` | literal slot index (positional: direct write to attrs)  |

**FRM tail (variable size):**

```text
offset 8:             mask_words[w]    ─ w = ceil(n / 64), u64 each
offset 8 + 8w:        name_ids[n]      ─ u16 each, padded to 4-byte boundary
offset 8 + 8w + nm:   attrs[n]         ─ each attribute is 12 bytes

where nm = round_up_4(2n).

block_size(FRM) = round_up_8(8 + 8w + nm + 12n)
```

The `name_ids` band stores the formation's per-slot global attribute name
ids in declaration order: `name_ids[k]` is the global id of the attribute
whose value/xi/cache live at `attrs[k]`. See **Attribute Name Numbering**.

**DSP tail (fixed → block size 16):**

```text
offset  8: target    (u32)   ─ u32::MAX = use context
offset 12: pad       (u32)
```

**APP tail (fixed → block size 16):**

```text
offset  8: target    (u32)   ─ u32::MAX = use context
offset 12: value     (u32)   ─ u32::MAX = use context
```

DSP and APP are structurally identical (16 bytes); DSP simply leaves the
last 4 bytes unread.

**CTX tail (none → block size 8):**

A CTX block is exactly the 8-byte common prefix with `ty = 0` and `n = 0`.
There is no tail. `morph` resolves a CTX immediately to the runtime
context — no target, no attribute lookup, no children to walk. The
`mark` worklist never pushes children from a CTX block, and `compact`
treats it as a fixed-size leaf.

**Sentinels.**

| Field           | Sentinel    | Meaning (vs. JS)                                |
|-----------------|-------------|-------------------------------------------------|
| `fwd`           | `u32::MAX`  | no forwarding planned                           |
| `target`        | `u32::MAX`  | use context as target — JS `obj.target === -1`  |
| `value` (APP)   | `u32::MAX`  | use context as value — JS `obj.value === -1`    |
| `attr.value`    | `u32::MAX`  | absent                                          |
| `attr.xi`       | `u32::MAX`  | absent                                          |
| `attr.cache`    | `u32::MAX`  | absent                                          |

The `attr_name_id` field on DSP and APP is always a concrete u16 — the
bare-self case that previously needed a `u16::MAX` sentinel is now its
own variant (CTX). The loader rejects any DSP with `attr_name_id ==
u16::MAX` as a malformed input (`debug_assert!` panic), and the runtime
never produces one.

**Q-at-offset-0 invariant.** The block at byte offset 0 is the global
formation `Φ` (the runtime's "Q"). It is the only block guaranteed never
to move under compaction (it sits at the floor, below every compact
range). The DSL loader writes Q first and asserts post-load that
`memory[0]` has `ty = FRM`. Static references to Q are plain `target =
0` u32 offsets; no special encoding is needed.

**Name id width.** `attr_name_id: u16` gives 65k global attribute names per
program. The next step up to `u32` would force per-variant header sizes
(losing the `n` field overlap), which costs more than 65k names buys.

**Alignment.** Every block starts at an 8-byte-aligned offset. The common
prefix is 8 bytes; the mask band starts at offset 8 (u64-aligned by
construction); the attribute tail is u32-aligned. FRM block size is
padded up to a multiple of 8 via `round_up_8` so the next block also
starts aligned. DSP/APP are exactly 16 bytes by construction.

**Indicative sizes.**

| Variant         | Size                                          |
|-----------------|-----------------------------------------------|
| FRM, n=1        | `round_up_8(8 + 8 + 4 + 12)` = 32 B           |
| FRM, n=2        | `round_up_8(8 + 8 + 4 + 24)` = 48 B           |
| FRM, n=8        | `round_up_8(8 + 8 + 16 + 96)` = 128 B         |
| FRM, n=100      | `round_up_8(8 + 16 + 200 + 1200)` = 1424 B    |
| DSP             | 16 B                                          |
| APP             | 16 B                                          |
| CTX             | 8 B                                           |

### Attributes

Each attribute is exactly 12 bytes:

```rust
struct Attribute {
    value: u32,   // u32::MAX = absent
    xi:    u32,   // u32::MAX = absent
    cache: u32,   // u32::MAX = absent
}
```

Attribute names map to fixed slot offsets within a formation's attribute
tail. The slot order is determined by the DSL pipeline at compile time
and recorded in the formation's `name_ids` band (see *Attribute Name
Numbering*). Attribute lookup at runtime is an array index into the
tail, not a hash probe.

### Attribute Name Numbering

The DSL pipeline runs a **whole-program analysis pass** that enumerates
every distinct attribute name appearing anywhere in the program — in
formation declarations, dispatch references, and application bindings —
and assigns each a `u16` global id.

**Reserved low ids** for the four universal names:

| Name | id  |
|------|-----|
| `φ`  | 0   |
| `Δ`  | 1   |
| `ρ`  | 2   |
| `λ`  | 3   |

User-defined names are numbered starting from `4`. The runtime branches
on the reserved ids (e.g. when handling `φ` or `Δ`) with a single integer
comparison, bypassing the per-formation scan.

**Per-formation slot order** is preserved from the source: the
formation's `name_ids[n]` band stores the global ids in declaration
order. If a formation declares attributes `[x, y, φ]` in that order, its
`name_ids` is `[id_of_x, id_of_y, 0]` and `attrs[2]` is the slot holding
the value/xi/cache for `φ`.

### Per-formation Lookup

To dispatch — or do a bound application on — a target formation with a
given global `attr_name_id`, the runtime linear-scans the target's
`name_ids` band and uses the matching slot index into `attrs`:

```rust
fn slot_of(name_ids: &[u16], id: u16) -> Option<usize> {
    name_ids.iter().position(|&x| x == id)
}
```

For typical formations (n ≤ 16) this is a single cache-line read. For
wide formations (n in the hundreds, e.g. `Q`) the scan auto-vectorizes:
AVX2 compares 16 `u16`s per cycle, so even n=100 finishes in a handful
of cycles — well under a hashmap lookup.

**Unbound (positional) applications** skip the scan entirely. The DSL
resolves the slot index at compile time and the APP block carries it
directly, signaled by `unbound = 1` in the flags byte.

**Upgrade paths (Future Work).** If profiling reveals the scan as a hot
spot for very wide formations:

- **Sorted `name_ids` + binary search** — O(log n), no compile-time
  work, very cache-friendly.
- **DSL-emitted perfect minimal hash** — true O(1) by integer math,
  feasible because formation shapes are static.
- **Inline cache in DSP** — cache the resolved slot in the DSP block
  after first dispatch, analogous to the existing `cache` field.
- **Type-id-shared `name_ids` tables** — move `name_ids` out of the
  block into a per-formation-type static table indexed by a new
  `type_id: u32` header field. Reduces memory duplication when programs
  heavily COPY the same formations.

### Δ Storage

Δ bytes do not live in `memory`. They live in a separate arena outside of
it:

```rust
arena: Vec<u8>
```

The Δ attribute slot reinterprets its three `u32` fields:

| Field   | Meaning when the attribute is Δ |
|---------|---------------------------------|
| `value` | byte offset into `arena`        |
| `xi`    | length of the byte block        |
| `cache` | unused                          |

EOLANG byte data is **immutable**. When a Δ-bearing formation is cloned
(by `COPY`), the clone's Δ attribute carries the **same** arena offset and
length as the source — no copy of arena bytes happens, ever. Multiple
formations may point at the same arena range.

**Byte deduplication.** Because Δ bytes are immutable by language rule,
the arena interns them: `Arena::push(&bytes)` checks a side
`HashMap<Vec<u8>, u32>` and returns the existing offset if the same byte
sequence is already present. Two literal `1.0` numbers in source code
get the same arena offset; an atom that computes `2 + 3 = 5` and finds
the literal `5` was already used elsewhere reuses that offset too. The
sharing is invisible to the rest of the runtime — Δ slots store an
`(offset, length)` pair regardless, and COPY still verbatim-copies the
slot without touching arena bytes.

Concrete impact, measured (arena size at load + after full evaluation):

| Program | load arena | final arena | dedup hits | dedup misses |
|---------|-----------:|------------:|-----------:|-------------:|
| simple  |       18 B |        18 B |          0 |            4 |
| foo     |       18 B |        18 B |          0 |            4 |
| eleven  |       26 B |        34 B |          0 |            6 |
| dollar  |       18 B |        18 B |          0 |            4 |
| dup     |       18 B |        26 B |          1 |            5 |
| rec     |       26 B |        50 B |          6 |            8 |
| fibo    |       34 B |       106 B |        189 |           15 |

(`hits` = `Arena::push` returned an existing offset, no bytes appended.
`misses` = bytes were actually appended. Numbers cover both load-time
literals and runtime-pushed atom results.)

The big win is at runtime: atoms append `f64::to_be_bytes` results on
every arithmetic step. fibo alone does 204 `push` calls (15 unique
values, 189 repeats). Without dedup the arena would grow by 8 bytes
per push to ~1.6 KB; with dedup it stabilizes at 106 B — a **94%
reduction**. The pattern is general: any recursive numeric workload
revisits a small set of distinct values many times.

Load-time dedup also folds duplicate literals within a single
program. `dup.eo` (`1.plus 1`) and `rec.eo` (`x.plus 1` and `rec 1`)
each use the literal `1.0` twice in source; dedup makes them share
one 8-byte arena range.

For load-time literals, dedup also folds duplicates within a single
program. rec uses the literal `1.0` twice in source (`x.plus 1` and
`rec 1`), and dedup makes them share one 8-byte arena range.

**`dedup_hits` and `dedup_misses` counters** on `Arena` are observable
at runtime end — useful both for tuning and for verifying that dedup
is firing as expected. A hits/misses ratio significantly above zero
confirms the workload benefits from interning.

For payloads larger than ~32 bytes (e.g. hypothetical future strings),
the hash cost can dominate; the implementation may grow a length
threshold (`if bytes.len() ≤ THRESHOLD { dedup } else { just append }`)
when such payloads exist. For the f64-only workload today the threshold
is unnecessary.

**Dedup map memory cost.** The map itself uses heap: one `Vec<u8>`
key-allocation per unique payload, plus `HashMap` overhead. For fibo's
15 unique payloads × ~64 B per `HashMap` entry, that's ~1 KB of
overhead — much smaller than the savings even on a small program.
Scales linearly with the program's distinct-value set, which is
bounded by the source's expression complexity.

**Interaction with the GC.** The dedup map is independent of memory's
mark-compact GC. Memory blocks holding Δ slots can be deleted/moved
freely; their `(arena_offset, length)` pair just points into the
arena. The arena itself is never traversed by `mark`/`compact` — Δ
ranges are leaf data. When arena reclamation is eventually
implemented, the dedup map must be reclamation-aware (see *Reclamation*
below).

**JS prototype divergence.** The JS prototype stores Δ bytes inline as a
plain JS array in `attr.value` rather than in a separate arena. This is
a deliberate convenience choice: the runtime's behavioral contract
(immutability, sharing across `COPY`, byte equivalence) does not depend
on the storage split, so the prototype does not need to model it. The
arena layout described in this section is introduced in the Rust port,
where the split is forced by the byte-addressable block format. The
change is local to the Δ slot and the bytes-reading paths; no other
runtime code (mark, compact, ref_holders, dataize, morph) depends on
where Δ bytes live.

**Reclamation: deferred — grow-only for now.** In the initial skeleton,
`arena` is append-only: every fresh Δ payload appends bytes (after the
dedup check). When a Δ-bearing formation is `del`'d, its arena range
becomes unreachable in the formation sense but may still be referenced
by other formations (via dedup) or simply orphaned. Arena bytes are
never freed. This is acceptable for a research runtime and lets us land
a working end-to-end implementation without solving the sharing-aware
reclamation problem first. A real reclamation strategy must be
**dedup-aware**: once payloads are shared across formations, releasing
a payload requires refcounting (per arena range), not just "no
formation references it from its most recent Δ slot." The deferred
options — `Rc<[u8]>` per blob, refcounts inline in arena ranges, or
arena mark-compact riding on `memory`'s compact — all naturally
integrate with dedup.

**Endianness.** Numbers are encoded in `arena` as **big-endian** IEEE 754
`f64`, matching the JS emulator's `DataView.setFloat64(offset, value)`
default. The Rust port uses `f64::to_be_bytes` and `f64::from_be_bytes`.

### Variable-Width Formations

The runtime does not impose a `MAX_ATTRS` limit. Each formation is sized
exactly to its declared attribute count. The DSL emitter writes the count
into the formation's header; the runtime never resizes a formation.

Two consequences worth calling out:

1. Walking memory block-by-block in `compact` requires reading each
   header's `ty` and (for formations) `attr_count` to know how far to step
   forward. There is no constant stride.
2. A formation's `written_attrs` bitmask (see below) must be wide enough
   for that formation's attribute count, not for a global maximum.

### Clone Semantics on COPY

`COPY` of a Formation produces a new block of the same byte size as the
source, allocated by a regular `push` at `next_free` (no separate young
arena). Field-by-field:

| Field           | Source                 | Clone                                          |
|-----------------|------------------------|------------------------------------------------|
| `ty`            | FRM                    | FRM (verbatim)                                 |
| `dead`          | 0                      | 0                                              |
| `stay`          | any                    | 0 (reset)                                      |
| `from_atom`     | any                    | 0 (reset)                                      |
| `n`             | n                      | n (verbatim)                                   |
| `fwd`           | any                    | `u32::MAX` (reset)                             |
| `mask_words`    | source mask            | all `n` bits set (every attribute is "freshly written" in the clone, per JS behavior) |
| `attrs[k]`      | source attribute k     | verbatim 12-byte copy (`value`, `xi`, `cache`) |

For a Δ-bearing source the clone's Δ attribute carries the **same** arena
offset and length as the source — Δ bytes are immutable and shared, never
duplicated.

The clone is added to `ref_holders` because every attribute is in the
written set.

### written_attrs as a Bitmask

`written_attrs` is a bitmask where bit `k` corresponds to the attribute
at offset `k` in this formation's tail. Formations can have more than 64
attributes (the program-level `Q` formation typically does), so the mask
is always stored as an array of `u64` words sized for the formation:

```text
w = ceil(attr_count / 64)
mask_words: [u64; w]
```

The words are laid out inline in the block, between the `ObjHeader` and
the attribute tail. There is no separate "small" and "large" case in the
runtime — the multi-word logic always runs; for a formation with ≤64
attributes it simply has `w == 1` and the loops execute once.

Setting bit `k`:
```rust
let word = k / 64;
let bit  = k % 64;
mask_words[word] |= 1u64 << bit;
```

Iterating set bits (O(popcount), no branching per zero bit):
```rust
for (word_idx, word_ref) in mask_words.iter_mut().enumerate() {
    let mut word = *word_ref;
    while word != 0 {
        let bit_idx = word.trailing_zeros() as usize;
        word &= word - 1;
        let attr_offset = word_idx * 64 + bit_idx;
        update_attr(&mut attrs[attr_offset], r);
    }
}
```

### Forwarding Pointer

`fwd: u32` lives in the `ObjHeader`, with `u32::MAX` as the null sentinel.
During the *plan* sub-phase of `compact`, each surviving block's
destination byte offset is written into its own header's `fwd` field
before any block has moved. The remap function is one bounds check plus
one header read:

```rust
fn r(offset: u32, first_dest: u32, end: u32) -> u32 {
    if offset < first_dest || offset > end { return offset; }
    let fwd = header_at(offset).fwd;
    if fwd != u32::MAX { fwd } else { offset }
}
```

The bounds check `offset < first_dest || offset > end` short-circuits for
the majority of references that point outside the compact range, avoiding
even the header access for those cases.

### ref_holders

`ref_holders: Vec<u32>` — a flat array of byte offsets, rebuilt after each
compact (filter nulled entries, remap surviving offsets through `r`). No
hashing, no pointer chasing, cache-friendly sequential scan.

For programs with very large `ref_holders`, a generational approach (see
below) dramatically reduces the iteration cost.

### Iteration: mark, dataize, morph, needs_context

The JS emulator implements all four functions iteratively (no recursive
self-calls); the Rust port follows the same pattern with explicit
work-stacks instead of native call stacks.

**`mark`** — iterative DFS with a pre-allocated `Vec<u32>` worklist of byte
offsets, seeded from one offset (`mark_phi`) or two (`mark_disp` for both
range endpoints):

```rust
fn mark(seeds: &[u32], in_range: impl Fn(u32) -> bool) {
    let mut stack: Vec<u32> = seeds.to_vec();
    while let Some(offset) = stack.pop() {
        let obj = header_at_mut(offset);
        if obj.stay { continue; }   // may have been pushed twice
        obj.stay = true;
        for k in 0..obj.attr_count() {
            let attr = obj.attr(k);
            let r = effective_ref(attr);   // cache ?? xi ?? skip
            if r != u32::MAX && in_range(r) && !header_at(r).stay {
                stack.push(r);
            }
        }
    }
}
```

**`dataize`** — every recursive call in the JS shape is in tail position
(the recursive value flows straight to the return), so the Rust shape is a
plain `loop` that updates `index` in place:

```rust
fn dataize(mut index: u32, scope: u32, gc_enabled: bool) -> &[u8] {
    loop {
        let obj = header_at(index);
        if obj.ty != FORMATION {
            let op_i = morph(index, index, true);
            index = gc_phi(gc_enabled, op_i, scope);
            continue;
        }
        if let Some(delta) = obj.attr_by_name(DELTA) {
            return arena_slice(delta.value, delta.xi);
        }
        if obj.has_attr(PHI) { /* push DSP, morph, gc_phi, update index, continue */ }
        if obj.has_attr(LAMBDA) { /* run atom, morph, gc_phi, update index, continue */ }
        panic!("Can't dataize: no Δ/φ/λ at offset {index}");
    }
}
```

**`needs_context`** — small worklist that short-circuits on `-1` and on
the CTX type:

```rust
fn needs_context(seed: u32) -> bool {
    let mut stack = vec![seed];
    while let Some(i) = stack.pop() {
        let obj = header_at(i);
        match obj.ty {
            FORMATION => {}
            CONTEXT => return true,
            DISPATCH => {
                if obj.dsp_target == u32::MAX { return true; }
                stack.push(obj.dsp_target);
            }
            APPLICATION => {
                if obj.app_target == u32::MAX || obj.app_value == u32::MAX { return true; }
                stack.push(obj.app_target);
                stack.push(obj.app_value);
            }
        }
    }
    false
}
```

**`morph`** — the only function with **non-tail** recursive calls. The JS
prototype uses generator functions plus a trampoline (`yield morph_g(...)`
in place of each recursive call, with a driver loop that owns the explicit
stack). The Rust translation is an explicit state machine: an enum
representing each checkpoint where the JS generator yielded, carrying the
locals saved at that point.

Sketch:

```rust
enum MorphState {
    Start,
    AfterDspTarget,                    // tgt_i just produced
    AfterDspAttrMorph { tgt_i: u32 },  // at_i just produced (cache miss path)
    AfterDspPhi { tgt_i: u32 },        // phi_i just produced
    AfterDspPhiAttr { tgt_i: u32 },    // res just produced (tail of phi path)
    AfterDspAtomMorph { tgt_i: u32 },  // atom_res_i just produced
    AfterDspAtomAttr { tgt_i: u32 },   // res just produced (tail of lambda path)
    AfterAppTarget,                    // tgt_i just produced
}

struct MorphFrame {
    index: u32,
    context: u32,
    remove: bool,
    state: MorphState,
}

fn morph(index: u32, context: u32, remove: bool) -> u32 {
    let mut stack: Vec<MorphFrame> = vec![MorphFrame {
        index, context, remove, state: MorphState::Start,
    }];
    let mut result: u32 = 0;
    while let Some(frame) = stack.last_mut() {
        match step(frame, result) {
            StepResult::Return(v) => { result = v; stack.pop(); }
            StepResult::Call(sub) => { stack.push(sub); result = 0; }
        }
    }
    result
}
```

`step` runs the frame's code up to the next checkpoint, returning either
`Return(value)` (the frame is done) or `Call(new_frame)` (push a sub-call).
Each checkpoint resumes by reading `result` (the just-completed sub-call's
return). The full set of states corresponds 1:1 to the `yield morph_g(...)`
points in the JS prototype.

Same pattern works for any future function that has non-tail recursion.

**Atom re-entry.** Atoms (`L_number_plus`, `L_number_gt`, `L_number_times`,
…) are external Rust functions called from inside a `morph` step via the
`λ` path. Each atom evaluates its own sub-expressions by calling
`morph()` and `dataize()` directly. Because the morph state machine is
internal to the `morph` function, an atom call from within a morph step
performs a *native* call back into `morph`, instantiating a fresh
state-machine stack. The total native call depth is therefore
`O(atom-call depth)` — bounded by program nesting, not by morph's own
recursion (which is fully flattened onto the per-call explicit stack).
The same applies to `dataize`, which the atoms also call recursively.

This is intentional and matches the JS prototype, where every `dataize`
inside an atom is a regular JS call. The Rust port does not attempt to
flatten atom-driven re-entry — atoms are leaf operations from the
runtime's perspective, and the depth introduced by their internal calls
is bounded by the program's static atom-nesting depth, which is small.

### Debug & Observability

The Rust port preserves the JS emulator's debug-time inspection
facilities, but compiles them out of release builds.

**`name` strings.** Every JS object carries a human-readable label
(`(copy 17)`, `Q.bytes`, etc.) used by `print_memory`. In Rust, names
live in a side container (e.g. `HashMap<u32, Box<str>>` keyed by block
offset) gated behind a `debug_names` Cargo feature. Release builds ship
without the side container at all; the hot path never touches it.

**`from_atom` flag.** Bit 4 of the flags byte is reserved for this
debug-only marker, set by atoms (`L_number_plus`, etc.) on their result
formation and consumed only by `print_memory`-style diagnostics. In
release builds the bit stays 0 and is ignored.

**Stats counters.** Plain process-wide counters maintained by the
runtime, printed at the end of a successful program run:

| Counter              | Meaning                                  |
|----------------------|------------------------------------------|
| `program_size`       | objects in memory before evaluation      |
| `live_count`         | currently live objects                   |
| `peak_live`          | peak live object count                   |
| `total_created`      | total `push` calls                       |
| `total_deleted`      | total `del` calls                        |
| `max_ref_holders`    | peak size of `ref_holders`               |
| `gc_phi_count`       | number of `gc_phi` calls                 |
| `gc_phi_reclaimed`   | objects reclaimed by `gc_phi`            |
| `gc_disp_count`      | number of `gc_disp` calls                |
| `gc_disp_reclaimed`  | objects reclaimed by `gc_disp`           |

`u64` is sufficient for every counter.

### Concurrency

The runtime is **single-threaded**. `memory`, `arena`, `ref_holders`,
`phi_watermark`, and the stats counters are all global mutable state
that no synchronization protects. The Rust types reflect this: the
runtime is neither `Send` nor `Sync`, and the spec does not anticipate
a multi-threaded interpreter.

### Generational GC (Future Work)

The current design already behaves generationally in practice: `gc_phi`
and `gc_disp` compact small, recently-allocated windows of memory. Old
objects at low offsets are rarely inside a compact range.

A formal generational boundary would divide memory into a young region
(recent allocations) and an old region (stable objects). Most compacts
would touch only the young region. Objects that survive several
young-region compacts get promoted to the old region and are only
collected during infrequent full compacts.

This would reduce both the compact range size and the `ref_holders`
iteration cost. The `written_attrs` bitmask already gives the
per-attribute precision needed to efficiently maintain cross-generational
references — old objects holding refs into the young region — equivalent
to a card table but at single-attribute granularity rather than
64-object-card granularity.

### Deferred to a Later Pass

The runtime spec is complete. The remaining work is in the **DSL
pipeline**, which sits upstream of the runtime and is intentionally
deferred until the runtime skeleton is implemented and exercising real
programs. When the skeleton is ready we will design:

- **DSL grammar with numeric offsets.** The current `to-dsl.xsl` emits
  a textual format with string attribute names. The Rust loader needs
  an offset-aware grammar carrying the global `u16` ids assigned by
  the whole-program numbering pass described in *Attribute Name
  Numbering*.
- **Atom registry.** Atoms (`L_number_plus`, etc.) are referenced by
  name in the DSL; Rust needs a static dispatch table with name →
  index assignment as a small pipeline step.

Other items already deferred and called out in their respective
sections rather than here:

- *Arena reclamation* — grow-only in the initial skeleton; reclamation
  strategy designed after the skeleton runs (see *Δ Storage*).
- Various upgrade paths for wide-formation attribute lookup (sorted
  arrays, perfect hashes, inline caches, type-id-shared tables) — see
  *Per-formation Lookup*.

### Heap Minimization Plan

**Goal:** the runtime should use heap only for the Δ byte arena; every
other piece of state should live in stack-allocated, fixed-capacity
storage. This makes peak memory predictable, eliminates allocator
calls on the hot path, and matches the "objects on stack, data on heap"
design principle taken to its logical conclusion.

#### Current heap users (as of skeleton)

A complete audit; see the *Where do we use heap* discussion for context.

| # | Heap user | Bytes for fibo | Lifetime | On the way out? |
|---|-----------|---------------:|----------|----------------|
| 1 | `Arena::buf: Vec<u8>` (Δ payloads) | 106 B | permanent | **keep** — original goal |
| 2 | `Arena::dedup: HashMap<Vec<u8>, u32>` | ~1.2 KB | permanent | replace with stack hashmap |
| 3 | Attribute interner (`HashMap<String, u16>` + `Vec<String>`) | ~1.3 KB | permanent | eliminate via DSL numeric ids |
| 4 | Atom interner (same shape) | ~150 B | permanent | eliminate via DSL numeric ids |
| 5 | `Runtime::atom_fns: Vec<AtomFn>` | ~50 B | permanent | move to `static [AtomFn; N]` |
| 6 | `Image::id_to_off: Vec<u32>` | 320 B | permanent | drop after load (load-time only) |
| 7 | `Runtime::ref_holders: HashSet<u32>` | ~4.4 KB | permanent, mutates | replace with stack set |
| 8 | `Runtime::push_stack: Vec<u32>` | ~few hundred B | permanent, mutates | replace with stack array |
| 9 | Per-call scratch (morph stack, mark worklist, compact scratch) | transient | per call | reuse Runtime-owned buffers |
| 10 | Loader intermediates (`Vec<DslLine>`, parser `String`s) | ~few KB | load-time only | release after `load` returns |
| 11 | `fs::read_to_string` source | source-size | load-time only | unavoidable (std I/O) |
| 12 | `env::args(): Vec<String>` | ~tens of B | program start | unavoidable (std I/O) |

#### Plan, ordered by impact ÷ effort

**Phase 1 — eliminate per-call allocations** *(small refactor, removes
hundreds of transient allocations per run, no functional change)*

Move every scratch buffer from "local Vec/HashMap inside the function"
to a reusable field on `Runtime`:

```rust
pub struct Runtime<'a> {
    // ... existing fields ...
    scratch_morph: Vec<MorphFrame>,        // morph state stack
    scratch_mark: Vec<u32>,                // mark worklist
    scratch_block_offsets: Vec<u32>,       // compact phase 1
    scratch_planned: Vec<(u32, u32, u32)>, // (src, dst, size) — replaces HashMap
    scratch_holders_snapshot: Vec<u32>,    // compact phase 2b
    scratch_next_holders: HashSet<u32>,    // compact phase 2b
}
```

Each call does `field.clear()` instead of `Vec::new()`. The Vec's
capacity grows once to the program's peak need, then stabilizes. Same
for HashMap/HashSet.

This doesn't reduce *peak* heap by much but eliminates allocation
*churn* — the hot path stops calling `malloc`/`free`.

**Phase 2 — drop `id_to_off` after load** *(trivial)*

`id_to_off` is a load-time aid; the runtime never references it.
Currently it's stored on `Image` and survives into `Runtime` (320 B for
fibo). Move it into a load-only local and drop before constructing
`Runtime`. Free win.

**Phase 3 — replace atom interner with a static table** *(small)*

Atoms are a closed set known at compile time
(`L_number_plus`, `L_number_times`, `L_number_gt`, ...). The XSL
pipeline can emit atom *indices* (u16) in the DSL instead of names.
Rust side becomes:

```rust
const ATOM_TABLE: [AtomFn; 3] = [number_plus, number_times, number_gt];
```

No interner, no HashMap, no Vec. The whole atom machinery becomes 24
bytes of static read-only memory.

**Phase 4 — replace attribute interner with DSL numeric ids** *(medium)*

The biggest single heap consumer after `ref_holders`. The README's
*Attribute Name Numbering* section already specifies a whole-program
numbering pass in XSL with reserved ids φ=0, Δ=1, ρ=2, λ=3 and user
names from 4. Implement that pass, change the DSL grammar to emit u16
ids, and the runtime's attribute interner disappears.

For `print_memory`-style debug output that needs names, gate behind a
`debug_names` Cargo feature that loads a separate `Vec<&'static str>`
table from the DSL. Release builds compile it out.

**Phase 5 — replace `ref_holders: HashSet<u32>` with a stack
collection** *(medium)*

Currently the heaviest non-byte heap user (~4 KB peak for fibo).
Options:

1. **`heapless::FnvIndexSet<u32, N>`** — drop-in, fixed capacity. N
   chosen at compile time (e.g., 256). Overflow → panic. Suitable for
   bounded workloads.
2. **Sorted `[u32; N]` + len** — O(log N) lookup via binary search,
   O(N) insertion via shift. Simple, no dependency. Fine for N ≤ 256.
3. **Bitset over offset ranges** — if dynamic memory occupies a known
   range, a bitset (`[u64; (max_off + 63) / 64]`) gives O(1)
   insert/lookup at zero per-entry overhead. Best fit if peak ref_holder
   density is known.

The right choice depends on the access pattern. (2) is the most
portable. (3) is the fastest if applicable.

**Phase 6 — replace `push_stack: Vec<u32>` with a fixed array** *(small)*

Peak is a few dozen entries. `[u32; 256]` + `len: u8` covers any
realistic morph nesting depth. Same shape as `ref_holders` change.

**Phase 7 — replace `Arena::dedup` HashMap with a stack hashmap** *(medium)*

`heapless::FnvIndexMap<Vec<u8>, u32, N>` — but `Vec<u8>` keys still
heap-allocate. Real win requires inline-key storage:

1. Treat the arena bytes themselves as the key storage and the dedup
   map stores `(arena_offset, length)` tuples with a hash of the bytes.
   Map size capped at compile time.
2. For typical f64 payloads (length 8), a perfect-hash specialized
   `HashMap<[u8; 8], u32>` works and never allocates per insert.

**Phase 8 — fixed capacity for `Arena::buf` itself (stretch)** *(only if
the design admits it)*

The arena is currently `Vec<u8>` — the canonical heap user. If the
deployment scenario can commit a maximum Δ-bytes budget at compile
time, the arena can be `&'a mut [u8]` from a second stack array,
exactly like `Memory.buf` already is. Then the runtime uses zero heap
(except std-lib-internal I/O buffers).

This is more aggressive than the original "data on heap" framing but
follows the same logic that put objects on stack: predictable
capacity, allocator-free, panics on overflow.

#### After full implementation

| Heap user | Status after plan |
|-----------|------------------|
| Δ payload bytes | heap (or stack with Phase 8) |
| Δ dedup map | stack (Phase 7) |
| Attribute interner | gone (Phase 4) |
| Atom interner | gone (Phase 3) |
| `atom_fns` | static (Phase 3) |
| `id_to_off` | gone after load (Phase 2) |
| `ref_holders` | stack (Phase 5) |
| `push_stack` | stack (Phase 6) |
| Per-call scratch | stack-reused (Phase 1) |
| Loader intermediates | unchanged — released by `drop` at end of load |
| `fs::read_to_string` | unchanged — `std`'s API |

Peak heap on fibo would drop from ~12 KB to ~106 B (just the arena
bytes). With Phase 8: 0 B (only std-lib I/O buffers).

#### Trade-offs to accept

Each phase replaces a growable heap collection with a fixed-capacity
stack one. The price is:

- **Capacity must be committed at compile time** (or via build-time
  constants). Overflow becomes a panic, not silent growth.
- **The runtime gains a `'a` lifetime** in more places (already true
  for `Memory`; would extend to `Arena` if Phase 8 lands).
- **Debug-time inspection** (e.g. `print_memory`) needs `debug_names`
  to retain a side table of attr/atom names — release builds compile
  out, debug builds re-introduce a small heap allocation.

These prices are reasonable for a runtime targeting predictability and
small footprint. They're wrong for a "host arbitrary user programs"
deployment, where unbounded inputs need unbounded collections.

#### Non-goals

- **Going `no_std`.** None of the phases above require dropping `std`.
  They use `std::collections` only when nothing better fits, and use
  stack alternatives elsewhere. Going `no_std` would be a separate
  initiative.
- **Custom allocator.** A bump allocator on a pre-allocated slab would
  give similar properties but introduces ownership / lifetime
  complexity that the heapless-style approach avoids.

---

### Known Suboptimalities in the Current Rust Implementation

Concrete improvements identified during the skeleton work, kept here so
they don't get lost. Ordered by hot-path frequency.

**Hot path (every morph / atom call):**

1. **Atoms re-intern attribute names every call.** `arithmetic_atom`
   does `lookup_attr(rt, "x")`, `lookup_attr(rt, "number")`,
   `lookup_attr(rt, "bytes")` per invocation — three
   `HashMap<String, u16>` probes. Should be precomputed into
   `Runtime.cached_ids` at startup. ~400 hash lookups saved on fibo.
2. **`morph()` allocates a fresh `Vec<MorphFrame>` per call.** Atoms
   call morph 7+ times each. ~900 transient Vec allocations on fibo.
   Fix: carry a scratch `Vec<MorphFrame>` on `Runtime` and `.clear()`
   between calls. Same applies to `mark`'s worklist.
3. **`exec_copy` uses byte-by-byte copy.** Bounds-checked indexing
   per byte. Should be a `memcpy` via `Memory::copy_within(src, dst,
   len)`. ~10× win on cloning a 144-byte number FRM.

**Per GC compact (called dozens of times):**

4. **`compact` allocates scratch every call** (`block_offsets`,
   `planned_pairs`, `size_for_block: HashMap`, `next_holders`,
   `holders_snapshot`). ~700 transient allocations on fibo. Fix:
   scratch fields on `Runtime`.
5. **`size_for_block: HashMap<u32, u32>` is over-engineered.** Replace
   with `Vec<(src, dst, size)>` triples, or read size from `dst` after
   the move.
6. **`ref_holders: HashSet<u32>` rebuilt from scratch.** Could swap-and-
   reuse two sets alternating.
7. **Gap-zeroing in compact does 4 byte writes per phantom.** Could
   write one `u64` per 8-byte phantom (single store).

**Algorithmic (acknowledged in README sections):**

8. **`frm_slot_of` is a linear scan over `name_ids`.** Fine for n ≤ 16,
   gets noticeable for wider formations. Upgrade paths in
   *Per-formation Lookup*: sorted ids + binary search, perfect hash,
   inline-cache slot in DSP, type-id-shared name_ids tables.
9. **`gc_phi` always runs `mark_phi`.** Even when nothing was
   allocated since the last compact. Could skip when `live_count`
   hasn't grown.

**Architecture:**

10. **`MorphFrame` carries unused fields per state.** Should be a
    tagged-union enum keyed on `MorphState`, carrying only the locals
    relevant to each variant. Saves ~8 B per frame and removes the
    "what do I save in which state" footgun.
11. **`runtime.rs` is ~900 lines.** Split into `runtime/{mod, morph,
    compact, dataize}.rs` for navigability.

**Maintenance:**

12. **Some `assert!`s should be `debug_assert!`** on the hot path
    (`finish_dsp_attr`, `morph` sub-frame bounds checks). Loader-time
    structural invariants and `assert_q_at_zero` should stay `assert!`.
13. **Loader does 3+ passes** (parse, plan offsets, emit). One-off
    startup cost, low impact, but trivially fuseable.

**Quick-win priority for the next pass:** #1 + #2 are the easiest big
wins (5-line + small refactor, removes hundreds of hash lookups and
allocations per program). #4 + #5 cluster well — one pass to move
compact scratch onto `Runtime` cleans up several.

---

## How to play

Test programs are placed inside `test-resources` directory.
To disable or enable a specific program, you can comment/uncomment it
in `test/test_programs.js` file.

Then run `npm test`. It compiles each EO program into JS and a DSL file, creates
`temp` directory with your programs inside and runs them (takes around 11 seconds
per program).

To see the full log and `memory` status for a specific program, e.g. `fibo`
you can do:

```bash
node temp/fibo/.eoc/program.js
```

The main runtime code is in `resources/program.js`.
Shared constants and utilities are in `resources/helpers.js`.

To add new `eo-runtime` objects, you should extend `resources/runtime.xsl`.

## How to contribute

You need `node` installed on your computer.

Submit a PR with your changes. To avoid frustration, please run `npm test`
before sending. Make sure all the tests pass.
