//! Runtime: push/pop, exec, needs_context, mark/compact, morph, dataize.
//!
//! Mirrors the JS prototype's program.js but operates on a byte-addressable
//! Memory plus an external Arena for Δ payloads. All references are u32
//! byte offsets.

use crate::block::*;
use crate::loader::{Image, Interner};
use crate::memory::{Arena, Memory};

pub type AtomFn = for<'a> fn(&mut Runtime<'a>, u32) -> u32;

/// Set of block offsets stored as one bit per 8-byte slot in the object
/// buffer. Replaces a `HashSet<u32>` on the ref-holder hot path: every
/// `exec_set` / `exec_copy` writes one bit; every compact iterates set bits
/// via word-scan with `trailing_zeros`. The bitmap allocates once at
/// construction (sized to the buffer capacity) and never reallocates.
pub struct OffsetBitmap {
    bits: Vec<u64>,
    /// Cached popcount so `len()` is O(1) for the max_ref_holders watermark.
    count: u32,
    /// Upper bound on word indices that may hold set bits. Extended on
    /// insert, never contracted on remove — so clear/iterate can scope to
    /// `[0, high_water_word)` rather than the full bitmap. Critical for
    /// small-N workloads where the bitmap is sized to the buffer capacity
    /// (millions of bits) but only a handful are ever set per compact.
    high_water_word: usize,
}

impl OffsetBitmap {
    pub fn new(buffer_capacity: u32) -> Self {
        // 8-byte block alignment → one bit per 8 bytes of buffer.
        let n_bits = (buffer_capacity as usize + 7) / 8;
        let n_words = (n_bits + 63) / 64;
        Self { bits: vec![0u64; n_words], count: 0, high_water_word: 0 }
    }

    #[inline]
    fn locate(off: u32) -> (usize, u64) {
        let idx = (off >> 3) as usize;
        ((idx >> 6), 1u64 << (idx & 63))
    }

    /// Set the bit for `off`. Returns true if it was newly inserted.
    pub fn insert(&mut self, off: u32) -> bool {
        let (word, mask) = Self::locate(off);
        let prev = self.bits[word];
        if prev & mask != 0 {
            return false;
        }
        self.bits[word] = prev | mask;
        self.count += 1;
        if word >= self.high_water_word {
            self.high_water_word = word + 1;
        }
        true
    }

    /// Clear the bit for `off`. Returns true if it was previously set.
    pub fn remove(&mut self, off: u32) -> bool {
        let (word, mask) = Self::locate(off);
        let prev = self.bits[word];
        if prev & mask == 0 {
            return false;
        }
        self.bits[word] = prev & !mask;
        self.count -= 1;
        true
    }

    #[inline]
    pub fn len(&self) -> u32 {
        self.count
    }

    pub fn clear(&mut self) {
        // Only touch words that might be non-zero. The high_water_word is a
        // lazy upper bound — over-clearing would be safe but wasteful, and
        // under-clearing would be a bug, so we extend on insert and accept
        // never-shrinking.
        for w in 0..self.high_water_word {
            self.bits[w] = 0;
        }
        self.count = 0;
        self.high_water_word = 0;
    }

    /// Number of u64 words that could currently contain set bits. Iteration
    /// callers should use this as their upper bound, not `n_words()`.
    #[inline]
    pub fn live_words(&self) -> usize {
        self.high_water_word
    }

    #[inline]
    pub fn word(&self, i: usize) -> u64 {
        self.bits[i]
    }
}

pub struct Runtime<'a> {
    pub memory: Memory<'a>,
    pub arena: Arena,
    pub attrs: Interner,
    pub atoms: Interner,
    pub atom_fns: Vec<AtomFn>,

    pub program_size: u32,
    /// Cached `program_size.saturating_sub(1)`. Returned by `default_scope()`,
    /// used as the scope argument to top-level dataize. Computed once at
    /// construction since `program_size` is immutable after load.
    pub default_scope_cached: u32,
    /// Tracks every block offset whose FRM holds a dynamic ref (cache/xi/value
    /// written via exec_*). Iterated each compact to remap outside-range
    /// pointers. Bitmap form: one bit per 8 B of buffer, no allocations.
    pub ref_holders: OffsetBitmap,
    /// Scratch bitmap that compact populates with the post-move offsets; we
    /// swap it with `ref_holders` at the end of compact so no allocation
    /// happens per GC pass.
    pub ref_holders_next: OffsetBitmap,
    pub phi_watermark: u32, // SENTINEL until first phi pass
    pub push_stack: Vec<u32>,

    /// Reused worklist for `morph`. Lives across every morph call so the Vec
    /// allocation amortizes to zero. Re-entrant calls (atom → dataize → morph)
    /// stash a base offset in the local frame and only consume frames above it.
    morph_stack: Vec<MorphFrame>,
    /// Reused worklist for `mark`. mark is non-re-entrant so a single shared
    /// Vec with clear-on-entry is sufficient.
    mark_stack: Vec<u32>,
    /// Reused worklist for `needs_context`. Same rationale as `mark_stack`.
    needs_ctx_stack: Vec<u32>,
    /// Reused scratch for `compact` Phase 1: in-range block offsets.
    compact_block_offsets: Vec<u32>,
    /// Reused scratch for `compact` Phases 2/3: (src, dst, size) per surviving
    /// block. Storing `size` here makes the per-block `HashMap` the previous
    /// implementation used unnecessary.
    compact_planned_pairs: Vec<(u32, u32, u32)>,

    /// Direct offset of `Q.number` (the canonical `number` formation in the
    /// program-level region). Used by `make_number` to skip the
    /// `push_dsp + morph + finish_dsp_attr` round-trip and clone Q.number
    /// directly. SENTINEL if the program's Q doesn't expose `number` as a
    /// direct FRM ref — in which case `make_number` will panic.
    pub q_number_off: u32,
    /// Slot index of `ρ` inside the Q.number prototype, resolved once at
    /// load time. `make_number` uses `exec_set_by_slot` with this to skip
    /// the linear name-table scan `exec_set_by_id` would otherwise do.
    pub q_number_rho_slot: u16,
    /// Same shape as `q_number_off`, but for `Q.bytes`.
    pub q_bytes_off: u32,
    pub q_bytes_rho_slot: u16,
    /// Same shape, but for `Q.true`.
    pub q_true_off: u32,
    pub q_true_rho_slot: u16,
    /// Same shape, but for `Q.false`.
    pub q_false_off: u32,
    pub q_false_rho_slot: u16,

    pub live_count: i64,
    pub peak_live: i64,
    pub peak_memory_bytes: u32,
    pub total_created: u64,
    pub total_deleted: u64,
    pub max_ref_holders: u64,
    pub gc_phi_count: u64,
    pub gc_phi_reclaimed: u64,
    pub gc_disp_count: u64,
    pub gc_disp_reclaimed: u64,
    /// One counter per interned atom (indexed by atom_id). Incremented every
    /// time an atom is invoked (both from `dataize`'s λ branch and from
    /// `morph`'s λ branch).
    pub atom_calls: Vec<u64>,
}

impl<'a> Runtime<'a> {
    pub fn from_image(img: Image<'a>, atom_fns: Vec<AtomFn>) -> Self {
        let program_size = img.memory.len();
        // count program-level objects by walking the image
        let mut live = 0i64;
        let mut off = 0u32;
        while off < program_size {
            live += 1;
            off += img.memory.block_size_at(off);
        }
        let live = live;
        let n_atoms = atom_fns.len();
        let buffer_capacity = img.memory.capacity();
        // Resolve Q's stdlib refs once. Q lives at offset 0 (asserted by
        // loader.rs). Each slot value points directly at the corresponding
        // formation when the DSL emits a Ref/Cached attr — which is what
        // every observed program does. If the slot is absent or non-FRM,
        // store SENTINEL and let make_number / make_bool panic with a clear
        // message on first use.
        // Resolve a (prototype_offset, rho_slot) pair for each Q stdlib name.
        // (SENTINEL, 0) means "not present"; the corresponding make_* method
        // panics in that case on first use.
        let lookup_q = |id: u16| -> (u32, u16) {
            if img.memory.is_empty() {
                return (SENTINEL, 0);
            }
            let slot = match img.memory.frm_slot_of(0, id) {
                Some(s) => s,
                None => return (SENTINEL, 0),
            };
            let v = img.memory.frm_attr_value(0, slot);
            if v == SENTINEL || img.memory.ty(v) != TY_FRM {
                return (SENTINEL, 0);
            }
            let rho_slot = img.memory.frm_slot_of(v, ID_RHO).unwrap_or(0);
            (v, rho_slot)
        };
        let (q_number_off, q_number_rho_slot) = lookup_q(ID_NUMBER);
        let (q_bytes_off, q_bytes_rho_slot) = lookup_q(ID_BYTES);
        let (q_true_off, q_true_rho_slot) = lookup_q(ID_TRUE);
        let (q_false_off, q_false_rho_slot) = lookup_q(ID_FALSE);
        Self {
            memory: img.memory,
            arena: img.arena,
            attrs: img.attrs,
            atoms: img.atoms,
            atom_fns,
            program_size,
            default_scope_cached: program_size.saturating_sub(1),
            ref_holders: OffsetBitmap::new(buffer_capacity),
            ref_holders_next: OffsetBitmap::new(buffer_capacity),
            phi_watermark: SENTINEL,
            push_stack: Vec::new(),
            morph_stack: Vec::new(),
            mark_stack: Vec::new(),
            needs_ctx_stack: Vec::new(),
            compact_block_offsets: Vec::new(),
            compact_planned_pairs: Vec::new(),
            q_number_off,
            q_number_rho_slot,
            q_bytes_off,
            q_bytes_rho_slot,
            q_true_off,
            q_true_rho_slot,
            q_false_off,
            q_false_rho_slot,
            live_count: live,
            peak_live: live,
            peak_memory_bytes: program_size,
            total_created: live as u64,
            total_deleted: 0,
            max_ref_holders: 0,
            atom_calls: vec![0; n_atoms],
            gc_phi_count: 0,
            gc_phi_reclaimed: 0,
            gc_disp_count: 0,
            gc_disp_reclaimed: 0,
        }
    }

    // --- push / pop / del ---

    fn note_push(&mut self, off: u32) {
        self.push_stack.push(off);
        self.live_count += 1;
        if self.live_count > self.peak_live {
            self.peak_live = self.live_count;
        }
        let len = self.memory.len();
        if len > self.peak_memory_bytes {
            self.peak_memory_bytes = len;
        }
        self.total_created += 1;
    }

    pub fn push_dsp(&mut self, target: u32, attr_id: u16) -> u32 {
        let off = self.memory.len();
        self.memory.extend_zero(DSP_APP_SIZE);
        self.memory.set_flags(off, TY_DSP);
        self.memory.set_n(off, attr_id);
        self.memory.set_fwd(off, SENTINEL);
        self.memory.set_dsp_target(off, target);
        self.memory.write_u32(off + 12, 0);
        self.note_push(off);
        off
    }

    pub fn push_app(&mut self, target: u32, attr_id: u16, value: u32, unbound: bool) -> u32 {
        let off = self.memory.len();
        self.memory.extend_zero(DSP_APP_SIZE);
        let flags = TY_APP | if unbound { FLAG_UNBOUND } else { 0 };
        self.memory.set_flags(off, flags);
        self.memory.set_n(off, attr_id);
        self.memory.set_fwd(off, SENTINEL);
        self.memory.set_app_target(off, target);
        self.memory.set_app_value(off, value);
        self.note_push(off);
        off
    }

    /// Push a fresh FORMATION block with the given attribute name ids.
    /// Caller fills in the attribute slots afterward via `set_frm_attr_*`.
    /// All attrs start as (SENTINEL, SENTINEL, SENTINEL) — fully void.
    pub fn push_frm(&mut self, name_ids: &[u16]) -> u32 {
        let off = self.memory.len();
        let n = u16::try_from(name_ids.len()).expect("attr count > u16");
        self.memory.extend_zero(frm_size(n));
        self.memory.set_flags(off, TY_FRM);
        self.memory.set_n(off, n);
        self.memory.set_fwd(off, SENTINEL);
        for (k, &nid) in name_ids.iter().enumerate() {
            self.memory.set_frm_name_id(off, k as u16, nid);
        }
        for k in 0..n {
            self.memory.set_frm_attr_value(off, k, SENTINEL);
            self.memory.set_frm_attr_xi(off, k, SENTINEL);
            self.memory.set_frm_attr_cache(off, k, SENTINEL);
        }
        self.note_push(off);
        off
    }

    pub fn head(&self) -> u32 {
        *self.push_stack.last().expect("head on empty push stack")
    }

    pub fn pop(&mut self) {
        let off = self.push_stack.pop().expect("pop on empty stack");
        self.del(off);
        self.memory.truncate(off);
    }

    pub fn del(&mut self, off: u32) {
        if !self.memory.dead(off) {
            self.memory.set_dead(off, true);
            self.live_count -= 1;
            self.total_deleted += 1;
        }
        self.ref_holders.remove(off);
    }

    fn touch_ref_holders(&mut self, off: u32) {
        self.ref_holders.insert(off);
        if self.ref_holders.len() as u64 > self.max_ref_holders {
            self.max_ref_holders = self.ref_holders.len() as u64;
        }
    }

    // --- needs_context ---

    pub fn needs_context(&mut self, seed: u32) -> bool {
        // Reuse the owned worklist instead of allocating a fresh Vec per call.
        // needs_context is not re-entrant within a single call (no callbacks),
        // and the only caller chain is morph→needs_context, also non-nested,
        // so clear-on-entry is sufficient.
        self.needs_ctx_stack.clear();
        self.needs_ctx_stack.push(seed);
        while let Some(off) = self.needs_ctx_stack.pop() {
            match self.memory.ty(off) {
                TY_FRM => {}
                TY_CTX => return true,
                TY_DSP => {
                    let t = self.memory.dsp_target(off);
                    if t == SENTINEL {
                        return true;
                    }
                    self.needs_ctx_stack.push(t);
                }
                TY_APP => {
                    let t = self.memory.app_target(off);
                    let v = self.memory.app_value(off);
                    if t == SENTINEL || v == SENTINEL {
                        return true;
                    }
                    self.needs_ctx_stack.push(t);
                    self.needs_ctx_stack.push(v);
                }
                _ => panic!("bad type at offset {off}"),
            }
        }
        false
    }

    // --- exec ---

    /// Shallow-copy a FORMATION block. The clone's `written_attrs` mask is
    /// set for every attribute (every attr is "freshly written" in the clone,
    /// per JS behavior). The clone is added to ref_holders.
    pub fn exec_copy(&mut self, src: u32) -> u32 {
        assert_eq!(self.memory.ty(src), TY_FRM, "COPY of non-FRM at {src}");
        let n = self.memory.n(src);
        let size = frm_size(n);
        let off = self.memory.len();
        self.memory.extend_zero(size);
        self.memory.copy_within(src, off, size);
        // Reset transient header bits: stay/dead/from_atom; keep ty + unbound.
        let flags = self.memory.flags(off) & TY_MASK;
        self.memory.set_flags(off, flags);
        self.memory.set_fwd(off, SENTINEL);
        // Mark every attribute as written.
        for k in 0..n {
            self.memory.frm_mask_set(off, k);
        }
        self.note_push(off);
        self.touch_ref_holders(off);
        off
    }

    /// Bound SET: target's named-attr slot is replaced with (value, xi, cache).
    pub fn exec_set_by_id(
        &mut self,
        target: u32,
        attr_id: u16,
        value: u32,
        xi: u32,
        cache: u32,
    ) -> u32 {
        assert_eq!(self.memory.ty(target), TY_FRM, "SET on non-FRM at {target}");
        let slot = self
            .memory
            .frm_slot_of(target, attr_id)
            .unwrap_or_else(|| panic!("SET: attr id {attr_id} not in target {target}"));
        self.memory.set_frm_attr_value(target, slot, value);
        self.memory.set_frm_attr_xi(target, slot, xi);
        self.memory.set_frm_attr_cache(target, slot, cache);
        self.memory.frm_mask_set(target, slot);
        self.touch_ref_holders(target);
        target
    }

    /// Positional SET: target's slot-indexed attr is replaced.
    pub fn exec_set_by_slot(
        &mut self,
        target: u32,
        slot: u16,
        value: u32,
        xi: u32,
        cache: u32,
    ) -> u32 {
        assert_eq!(self.memory.ty(target), TY_FRM, "SET on non-FRM at {target}");
        self.memory.set_frm_attr_value(target, slot, value);
        self.memory.set_frm_attr_xi(target, slot, xi);
        self.memory.set_frm_attr_cache(target, slot, cache);
        self.memory.frm_mask_set(target, slot);
        self.touch_ref_holders(target);
        target
    }

    // --- mark / compact ---

    /// Return the effective outgoing reference for a FRM slot (cache, else xi, else nothing).
    fn attr_ref(&self, frm_off: u32, slot: u16) -> u32 {
        let c = self.memory.frm_attr_cache(frm_off, slot);
        if c != SENTINEL {
            return c;
        }
        let x = self.memory.frm_attr_xi(frm_off, slot);
        x // SENTINEL if absent
    }

    pub fn mark<R>(&mut self, seeds: &[u32], in_range: R)
    where
        R: Fn(u32) -> bool,
    {
        // Reuse the owned worklist. mark is non-re-entrant: the closure
        // captures by value (Copy `u32`s), the memory accessors don't call
        // back into mark, and mark_phi/mark_disp invocations are sequential
        // — never overlapping in time — so clear-on-entry is sufficient.
        self.mark_stack.clear();
        self.mark_stack.extend_from_slice(seeds);
        while let Some(off) = self.mark_stack.pop() {
            if self.memory.stay(off) {
                continue;
            }
            self.memory.set_stay(off, true);
            if self.memory.ty(off) != TY_FRM {
                continue;
            }
            let n = self.memory.n(off);
            for k in 0..n {
                let r = self.attr_ref(off, k);
                if r != SENTINEL && in_range(r) && !self.memory.stay(r) {
                    self.mark_stack.push(r);
                }
            }
        }
    }

    pub fn mark_disp(&mut self, start: u32, from: u32, to: u32) {
        self.mark(&[start], move |r| r >= from && r <= to);
    }

    pub fn mark_phi(&mut self, seed: u32, scope: u32) {
        let wm = self.phi_watermark;
        self.mark(&[seed], move |r| r > scope && r < wm);
    }

    pub fn compact(&mut self, from: u32, to: u32, pivot: u32) -> u32 {
        let mem_end = if self.memory.is_empty() {
            0
        } else {
            self.memory.len()
        };
        let end_target = to;
        // Walk to find offsets in [from, end_target] in block order. Reuse the
        // owned scratch vec so no allocation happens per compact.
        self.compact_block_offsets.clear();
        let mut walk = from;
        while walk < mem_end && walk <= end_target {
            let sz = self.memory.block_size_at(walk);
            self.compact_block_offsets.push(walk);
            walk += sz;
        }
        if self.compact_block_offsets.is_empty() {
            return pivot;
        }

        // Phase 1: plan. Advance past in-place survivors; assign fwd for others.
        let mut cursor = from;
        let mut first_idx = 0;
        while first_idx < self.compact_block_offsets.len() {
            let off = self.compact_block_offsets[first_idx];
            if self.memory.dead(off) || !self.memory.stay(off) {
                break;
            }
            self.memory.set_stay(off, false);
            cursor = off + self.memory.block_size_at(off);
            first_idx += 1;
        }
        if first_idx == self.compact_block_offsets.len() {
            return pivot;
        }

        let first_dest = cursor;
        let mut dest = cursor;
        // Reuse the planned-pairs scratch vec. Tuple includes block size so
        // Phase 3 doesn't need a parallel HashMap to recover it.
        self.compact_planned_pairs.clear();
        let n_blocks = self.compact_block_offsets.len();
        for k in first_idx..n_blocks {
            let off = self.compact_block_offsets[k];
            if self.memory.dead(off) {
                continue;
            }
            let sz = self.memory.block_size_at(off);
            if self.memory.stay(off) {
                self.memory.set_stay(off, false);
                self.memory.set_fwd(off, dest);
                self.compact_planned_pairs.push((off, dest, sz));
                dest += sz;
            } else {
                self.del(off);
            }
        }

        let scan_end = walk; // exclusive upper bound of the scanned range
        let r = |this: &Memory<'_>, idx: u32| -> u32 {
            if idx == SENTINEL || idx < first_dest || idx >= scan_end {
                return idx;
            }
            // The slot at `idx` is still at its old position; if it's planned,
            // its header has `fwd = dst`.
            if this.dead(idx) {
                return idx;
            }
            let f = this.fwd(idx);
            if f != SENTINEL { f } else { idx }
        };

        // Phase 2a: rewrite refs in [from, scan_end) for surviving in-range objects.
        let n_pairs = self.compact_planned_pairs.len();
        for k in 0..n_pairs {
            let src = self.compact_planned_pairs[k].0;
            self.remap_block_refs(src, &r);
        }
        // The in-place survivors before first_dest also hold refs that may
        // point into the planning zone — remap them too.
        let mut walk2 = from;
        while walk2 < first_dest {
            self.remap_block_refs(walk2, &r);
            walk2 += self.memory.block_size_at(walk2);
        }

        // Phase 2b: rewrite refs in out-of-range ref_holders. Walk the bitmap
        // word-by-word using `trailing_zeros` so iteration cost is one tick per
        // set bit, not per slot. Populate `ref_holders_next` in parallel and
        // swap at the end — zero allocations per compact.
        self.ref_holders_next.clear();
        let n_words = self.ref_holders.live_words();
        for word_i in 0..n_words {
            let mut word = self.ref_holders.word(word_i);
            while word != 0 {
                let bit = word.trailing_zeros() as usize;
                word &= word - 1;
                let idx = ((word_i * 64 + bit) as u32) << 3;
                if self.memory.dead(idx) {
                    continue;
                }
                let cur = r(&self.memory, idx);
                self.ref_holders_next.insert(cur);
                // For outside-range holders the block hasn't moved yet; remap
                // refs through its written-attrs mask so we touch only the
                // attrs that were actually written.
                if idx < from || idx >= scan_end {
                    if self.memory.ty(idx) == TY_FRM {
                        self.remap_frm_written(idx, &r);
                    }
                }
            }
        }
        std::mem::swap(&mut self.ref_holders, &mut self.ref_holders_next);
        if self.ref_holders.len() as u64 > self.max_ref_holders {
            self.max_ref_holders = self.ref_holders.len() as u64;
        }

        if self.phi_watermark != SENTINEL {
            self.phi_watermark = r(&self.memory, self.phi_watermark);
        }
        let new_pivot = r(&self.memory, pivot);

        // Phase 3: move planned blocks left-to-right (destinations <= sources).
        // We do NOT zero source bytes — destinations are non-overlapping and
        // tile [first_dest, dest) exactly, so every byte in that range is
        // overwritten by a move. Bytes in (dest, scan_end] are stale OLD
        // content; they get removed by the truncate below when the range
        // extends to the tail. Size lives in the planned-pairs tuple so no
        // parallel HashMap is needed.
        for k in 0..n_pairs {
            let (src, dst, sz) = self.compact_planned_pairs[k];
            if src == dst {
                continue;
            }
            self.memory.copy_within(src, dst, sz);
        }

        let new_end = dest;
        if scan_end == self.memory.len() {
            // No blocks past the range — just drop the gap.
            self.memory.truncate(new_end);
        } else if new_end < scan_end {
            // Blocks past scan_end exist (typically atom-internal pushes
            // referenced from cache chains in the result). Leave them alone
            // and tile the gap with 8-byte dead-CTX phantoms so it's both
            // walkable and skip-able. The gap is a multiple of 8 (block
            // alignment).
            let mut i = new_end;
            while i < scan_end {
                self.memory.write_u8(i, TY_CTX | FLAG_DEAD);
                self.memory.write_u8(i + 1, 0);
                self.memory.write_u16(i + 2, 0);
                self.memory.write_u32(i + 4, SENTINEL);
                i += 8;
            }
        }

        new_pivot
    }

    fn remap_block_refs<F>(&mut self, off: u32, r: &F)
    where
        F: Fn(&Memory<'_>, u32) -> u32,
    {
        match self.memory.ty(off) {
            TY_FRM => {
                let n = self.memory.n(off);
                for k in 0..n {
                    let c = self.memory.frm_attr_cache(off, k);
                    if c != SENTINEL {
                        let nc = r(&self.memory, c);
                        if nc != c {
                            self.memory.set_frm_attr_cache(off, k, nc);
                            self.memory.frm_mask_set(off, k);
                        }
                    } else {
                        let xi = self.memory.frm_attr_xi(off, k);
                        if xi != SENTINEL {
                            let v = self.memory.frm_attr_value(off, k);
                            let nv = r(&self.memory, v);
                            let nx = r(&self.memory, xi);
                            if nv != v || nx != xi {
                                self.memory.set_frm_attr_value(off, k, nv);
                                self.memory.set_frm_attr_xi(off, k, nx);
                                self.memory.frm_mask_set(off, k);
                            }
                        } else {
                            let v = self.memory.frm_attr_value(off, k);
                            if v != SENTINEL {
                                let nv = r(&self.memory, v);
                                if nv != v {
                                    self.memory.set_frm_attr_value(off, k, nv);
                                    self.memory.frm_mask_set(off, k);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn remap_frm_written<F>(&mut self, off: u32, r: &F)
    where
        F: Fn(&Memory<'_>, u32) -> u32,
    {
        let n = self.memory.n(off);
        for k in 0..n {
            if !self.memory.frm_mask_get(off, k) {
                continue;
            }
            let c = self.memory.frm_attr_cache(off, k);
            if c != SENTINEL {
                let nc = r(&self.memory, c);
                if nc != c {
                    self.memory.set_frm_attr_cache(off, k, nc);
                }
                continue;
            }
            let xi = self.memory.frm_attr_xi(off, k);
            if xi != SENTINEL {
                let v = self.memory.frm_attr_value(off, k);
                let nv = r(&self.memory, v);
                let nx = r(&self.memory, xi);
                if nv != v {
                    self.memory.set_frm_attr_value(off, k, nv);
                }
                if nx != xi {
                    self.memory.set_frm_attr_xi(off, k, nx);
                }
                continue;
            }
            let v = self.memory.frm_attr_value(off, k);
            if v != SENTINEL {
                let nv = r(&self.memory, v);
                if nv != v {
                    self.memory.set_frm_attr_value(off, k, nv);
                }
            }
        }
    }

    pub fn gc_phi(&mut self, enabled: bool, value: u32, scope: u32) -> u32 {
        let enabled = enabled && (self.phi_watermark == SENTINEL || value > self.phi_watermark);
        if !enabled {
            return value;
        }
        self.phi_watermark = value;
        let start = scope.wrapping_add(1);
        if value > start {
            self.gc_phi_count += 1;
            let before = self.live_count;
            self.mark_phi(value, scope);
            // Compact only [start, value]; blocks past `value` (atom-internal
            // intermediates referenced from cache chains, etc.) stay alive
            // and are reclaimed by later passes once `value` advances past
            // them. Matches the JS prototype's behavior.
            let result = self.compact(start, value, value);
            self.gc_phi_reclaimed += (before - self.live_count) as u64;
            return result;
        }
        value
    }

    pub fn gc_disp(&mut self, from: u32, phi: u32) -> u32 {
        self.mark_disp(from, from, phi);
        self.mark_disp(phi, from, phi);
        self.gc_disp_count += 1;
        let before = self.live_count;
        let res = self.compact(from, phi, phi);
        self.gc_disp_reclaimed += (before - self.live_count) as u64;
        res
    }

    // --- morph state machine ---

    pub fn morph(&mut self, index: u32, context: u32, remove: bool) -> u32 {
        assert!(
            index < self.memory.len(),
            "morph: index={index} oob (memory.len={}, context={context}, remove={remove})",
            self.memory.len()
        );
        // Reuse the owned morph stack across calls. Re-entrant invocations
        // (atom → dataize → morph) stash a `base` offset and consume only
        // frames above it, so each invocation behaves like a private stack
        // while the underlying Vec allocation amortizes to zero.
        let base = self.morph_stack.len();
        self.morph_stack.push(MorphFrame::new(index, context, remove));
        let mut result: u32 = 0;
        while self.morph_stack.len() > base {
            // Pop the top frame so `step_one` can take `&mut self` without
            // aliasing the stack. On Return we drop it; on Call we re-push
            // it under the new sub-frame so it resumes after the sub-frame
            // finishes.
            let mut frame = self.morph_stack.pop().unwrap();
            match self.step_one(&mut frame, result) {
                StepResult::Return(v) => {
                    result = v;
                }
                StepResult::Call(sub) => {
                    assert!(
                        sub.index < self.memory.len(),
                        "sub-frame index={} oob (memory.len={})",
                        sub.index,
                        self.memory.len()
                    );
                    self.morph_stack.push(frame);
                    self.morph_stack.push(sub);
                    result = 0;
                }
            }
        }
        result
    }

    fn step_one(&mut self, frame: &mut MorphFrame, mut result: u32) -> StepResult {
        loop {
            match frame.state {
                MorphState::Start => {
                    let obj_off = frame.index;
                    // Read the 8-byte header in one shot. Layout: byte 0 = flags
                    // (ty in low 2 bits, UNBOUND/etc in upper), byte 1 = pad,
                    // bytes 2..4 = `n` (u16), bytes 4..8 = `fwd` (u32).
                    let hdr = self.memory.read_u64(obj_off);
                    let flags = hdr as u8;
                    let ty = flags & TY_MASK;
                    match ty {
                        TY_FRM => return StepResult::Return(obj_off),
                        TY_CTX => {
                            if frame.remove {
                                self.pop();
                            }
                            return StepResult::Return(frame.context);
                        }
                        TY_DSP => {
                            let attr_id = (hdr >> 16) as u16;
                            // DSP body (bytes 8..16): low 32 bits = target.
                            let target = self.memory.read_u64(obj_off + 8) as u32;
                            // Stash attr_id into the frame so post-pop states can read it
                            // without re-touching the (possibly truncated) header.
                            frame.cached_attr = attr_id;
                            if frame.remove {
                                self.pop();
                            }
                            if target == SENTINEL {
                                result = frame.context;
                                frame.state = MorphState::AfterDspTarget;
                                continue;
                            } else {
                                frame.state = MorphState::AfterDspTarget;
                                return StepResult::Call(MorphFrame::new(target, frame.context, false));
                            }
                        }
                        TY_APP => {
                            let attr_or_slot = (hdr >> 16) as u16;
                            // APP body (bytes 8..16): low 32 bits = target, high 32 = value.
                            let body = self.memory.read_u64(obj_off + 8);
                            let target = body as u32;
                            let value = (body >> 32) as u32;
                            let unbound = flags & FLAG_UNBOUND != 0;
                            frame.cached_attr = attr_or_slot;
                            frame.cached_value = value;
                            frame.cached_unbound = unbound;
                            if frame.remove {
                                self.pop();
                            }
                            if target == SENTINEL {
                                result = frame.context;
                                frame.state = MorphState::AfterAppTarget;
                                continue;
                            } else {
                                frame.state = MorphState::AfterAppTarget;
                                return StepResult::Call(MorphFrame::new(target, frame.context, false));
                            }
                        }
                        _ => panic!("bad block type at offset {obj_off}"),
                    }
                }
                MorphState::AfterDspTarget => {
                    let tgt_i = result;
                    let attr_id = frame.cached_attr;

                    // tgt must be a FRM (morph always reduces to one).
                    if self.memory.ty(tgt_i) != TY_FRM {
                        panic!("DSP target morphed to non-FRM at {tgt_i}");
                    }

                    if let Some(slot) = self.memory.frm_slot_of(tgt_i, attr_id) {
                        let cache = self.memory.frm_attr_cache(tgt_i, slot);
                        if cache != SENTINEL {
                            // cache hit — straight to RHO-attach logic
                            return StepResult::Return(self.finish_dsp_attr(cache, attr_id, tgt_i));
                        }
                        // cache miss path — morph the attr value
                        let value = self.memory.frm_attr_value(tgt_i, slot);
                        let xi = self.memory.frm_attr_xi(tgt_i, slot);
                        let ctx = if xi != SENTINEL { xi } else { tgt_i };
                        frame.saved = tgt_i;
                        frame.state = MorphState::AfterDspAttrMorph;
                        return StepResult::Call(MorphFrame::new(value, ctx, false));
                    } else if let Some(_) = self.memory.frm_slot_of(tgt_i, ID_PHI) {
                        // PHI dispatch path
                        let dsp = self.push_dsp(tgt_i, ID_PHI);
                        frame.saved = tgt_i;
                        frame.state = MorphState::AfterDspPhi;
                        return StepResult::Call(MorphFrame::new(dsp, tgt_i, true));
                    } else if let Some(lambda_slot) = self.memory.frm_slot_of(tgt_i, ID_LAMBDA) {
                        // LAMBDA: invoke atom
                        let atom_id = self.memory.frm_attr_value(tgt_i, lambda_slot) as usize;
                        self.atom_calls[atom_id] += 1;
                        let atom_fn = self.atom_fns[atom_id];
                        let atom_res = atom_fn(self, tgt_i);
                        frame.saved = tgt_i;
                        frame.state = MorphState::AfterDspAtomMorph;
                        return StepResult::Call(MorphFrame::new(atom_res, tgt_i, false));
                    } else {
                        panic!("bad dispatch: tgt={tgt_i} has no attr {attr_id}, no φ, no λ");
                    }
                }
                MorphState::AfterDspAttrMorph => {
                    let at_i = result;
                    let tgt_i = frame.saved;
                    let attr_id = frame.cached_attr;
                    assert!(
                        at_i < self.memory.len(),
                        "AfterDspAttrMorph: at_i={at_i} oob (memory.len={}, tgt_i={tgt_i}, attr_id={attr_id})",
                        self.memory.len()
                    );
                    // Write the cache if absent.
                    if let Some(slot) = self.memory.frm_slot_of(tgt_i, attr_id) {
                        if self.memory.frm_attr_cache(tgt_i, slot) == SENTINEL {
                            self.memory.set_frm_attr_cache(tgt_i, slot, at_i);
                            self.memory.frm_mask_set(tgt_i, slot);
                            self.touch_ref_holders(tgt_i);
                        }
                    }
                    return StepResult::Return(self.finish_dsp_attr(at_i, attr_id, tgt_i));
                }
                MorphState::AfterDspPhi => {
                    let phi_i = result;
                    let tgt_i = frame.saved;
                    let phi_i = self.gc_disp(tgt_i, phi_i);
                    let attr_id = frame.cached_attr;
                    let dsp = self.push_dsp(phi_i, attr_id);
                    frame.saved = tgt_i;
                    frame.state = MorphState::AfterDspPhiAttr;
                    return StepResult::Call(MorphFrame::new(dsp, phi_i, true));
                }
                MorphState::AfterDspPhiAttr => {
                    let res = result;
                    let tgt_i = frame.saved;
                    let res = self.gc_disp(tgt_i, res);
                    return StepResult::Return(res);
                }
                MorphState::AfterDspAtomMorph => {
                    let atom_res_i = result;
                    let attr_id = frame.cached_attr;
                    let dsp = self.push_dsp(atom_res_i, attr_id);
                    frame.state = MorphState::AfterDspAtomAttr;
                    return StepResult::Call(MorphFrame::new(dsp, atom_res_i, true));
                }
                MorphState::AfterDspAtomAttr => {
                    return StepResult::Return(result);
                }
                MorphState::AfterAppTarget => {
                    let tgt_i = result;
                    let attr_field = frame.cached_attr;
                    let unbound = frame.cached_unbound;
                    let value = frame.cached_value;

                    let (at_value, at_xi) = if value == SENTINEL {
                        (frame.context, SENTINEL)
                    } else if self.needs_context(value) {
                        (value, frame.context)
                    } else {
                        (value, SENTINEL)
                    };

                    let at_cache = if at_value != SENTINEL && self.memory.ty(at_value) == TY_FRM {
                        at_value
                    } else {
                        SENTINEL
                    };

                    let res = if unbound {
                        self.exec_set_by_slot(tgt_i, attr_field, at_value, at_xi, at_cache)
                    } else {
                        self.exec_set_by_id(tgt_i, attr_field, at_value, at_xi, at_cache)
                    };
                    return StepResult::Return(res);
                }
            }
        }
    }

    /// Common tail for DSP attr-resolution: if the resolved attr value is not
    /// Q-rooted (`at_i != 0`), the requested attr is not ρ, and the resolved
    /// formation does not already have ρ *set* — copy + attach ρ. Otherwise
    /// return at_i. The implicit ρ slot pre-allocated by the loader is unset
    /// (value=SENTINEL) until exec_set fills it in.
    fn finish_dsp_attr(&mut self, at_i: u32, attr_id: u16, tgt_i: u32) -> u32 {
        if at_i == 0 || attr_id == ID_RHO {
            return at_i;
        }
        assert!(
            at_i < self.memory.len(),
            "finish_dsp_attr: at_i={at_i} oob (memory.len={}, attr_id={attr_id}, tgt_i={tgt_i})",
            self.memory.len()
        );
        if self.memory.ty(at_i) == TY_FRM {
            if let Some(slot) = self.memory.frm_slot_of(at_i, ID_RHO) {
                if self.memory.frm_attr_value(at_i, slot) != SENTINEL {
                    return at_i;
                }
            } else {
                // No ρ slot at all — formation can't carry ρ.
                return at_i;
            }
        }
        let cloned = self.exec_copy(at_i);
        self.exec_set_by_id(cloned, ID_RHO, tgt_i, SENTINEL, tgt_i);
        cloned
    }
}

#[derive(Copy, Clone)]
enum MorphState {
    Start,
    AfterDspTarget,
    AfterDspAttrMorph,
    AfterDspPhi,
    AfterDspPhiAttr,
    AfterDspAtomMorph,
    AfterDspAtomAttr,
    AfterAppTarget,
}

struct MorphFrame {
    index: u32,
    context: u32,
    remove: bool,
    state: MorphState,
    saved: u32,
    cached_attr: u16,
    cached_value: u32,
    cached_unbound: bool,
}

impl MorphFrame {
    fn new(index: u32, context: u32, remove: bool) -> Self {
        Self {
            index,
            context,
            remove,
            state: MorphState::Start,
            saved: SENTINEL,
            cached_attr: 0,
            cached_value: SENTINEL,
            cached_unbound: false,
        }
    }
}

enum StepResult {
    Return(u32),
    Call(MorphFrame),
}

impl<'a> Runtime<'a> {
    pub fn default_scope(&self) -> u32 {
        self.default_scope_cached
    }

    /// Returns (arena_offset, length) of the Δ payload reached by dataizing.
    pub fn dataize(&mut self, mut index: u32, scope: u32, gc_enabled: bool) -> (u32, u32) {
        loop {
            if index >= self.memory.len() {
                panic!(
                    "dataize: index {index} out of bounds (memory.len={}, scope={scope})",
                    self.memory.len()
                );
            }
            let ty = self.memory.ty(index);
            if ty != TY_FRM {
                let op_i = self.morph(index, index, true);
                index = self.gc_phi(gc_enabled, op_i, scope);
                continue;
            }
            if let Some(slot) = self.memory.frm_slot_of(index, ID_DELTA) {
                let off = self.memory.frm_attr_value(index, slot);
                let len = self.memory.frm_attr_xi(index, slot);
                return (off, len);
            }
            if self.memory.frm_slot_of(index, ID_PHI).is_some() {
                let dsp = self.push_dsp(index, ID_PHI);
                let phi_i = self.morph(dsp, index, true);
                index = self.gc_phi(gc_enabled, phi_i, scope);
                continue;
            }
            if let Some(lambda_slot) = self.memory.frm_slot_of(index, ID_LAMBDA) {
                let atom_id = self.memory.frm_attr_value(index, lambda_slot) as usize;
                self.atom_calls[atom_id] += 1;
                let atom_fn = self.atom_fns[atom_id];
                let atom_res = atom_fn(self, index);
                let morphed = self.morph(atom_res, index, false);
                index = self.gc_phi(gc_enabled, morphed, scope);
                continue;
            }
            panic!("can't dataize {index}: no Δ, no φ, no λ");
        }
    }

    pub fn dataize_to_f64(&mut self, index: u32, scope: u32, gc_enabled: bool) -> f64 {
        let (off, len) = self.dataize(index, scope, gc_enabled);
        assert_eq!(len, 8, "expected 8-byte f64 payload at {off}, got len={len}");
        let s = self.arena.slice(off, len);
        let arr: [u8; 8] = s.try_into().expect("8 bytes");
        f64::from_be_bytes(arr)
    }

    pub fn intern_attr(&mut self, name: &str) -> u16 {
        self.attrs.intern(name)
    }

    /// Build a `number ← bytes ← Δ` formation directly, bypassing the
    /// `push_dsp + morph + finish_dsp_attr + push_app + morph` round-trip
    /// the arithmetic atoms used to spin through ten times per primitive op.
    /// The shape is fixed (stdlib invariant), so we clone Q.number and
    /// Q.bytes prototypes, set ρ=Q, bind the Δ-frm into the bytes-clone's
    /// slot 0, and bind that into the number-clone's slot 0. Net effect is
    /// byte-identical to the morph path (verified by the runtime test
    /// suite), but with no trampoline traffic for the wrap layers.
    pub fn make_number(&mut self, value: f64) -> u32 {
        debug_assert!(
            self.q_number_off != SENTINEL,
            "make_number: Q.number not resolved at load time"
        );
        debug_assert!(
            self.q_bytes_off != SENTINEL,
            "make_number: Q.bytes not resolved at load time"
        );
        let arena_off = self.arena.push(&value.to_be_bytes());
        let data = self.push_frm(&[ID_DELTA]);
        self.memory.set_frm_attr_value(data, 0, arena_off);
        self.memory.set_frm_attr_xi(data, 0, 8);
        let bts = self.exec_copy(self.q_bytes_off);
        let bts_rho = self.q_bytes_rho_slot;
        self.exec_set_by_slot(bts, bts_rho, 0, SENTINEL, 0);
        self.exec_set_by_slot(bts, 0, data, SENTINEL, data);
        let num = self.exec_copy(self.q_number_off);
        let num_rho = self.q_number_rho_slot;
        self.exec_set_by_slot(num, num_rho, 0, SENTINEL, 0);
        self.exec_set_by_slot(num, 0, bts, SENTINEL, bts);
        let f = self.memory.flags(num);
        self.memory.set_flags(num, f | FLAG_FROM_ATOM);
        num
    }

    /// Build a `true` or `false` formation directly by cloning Q.true or
    /// Q.false. Mirrors the dispatch path `morph(dsp(0, ID_TRUE|FALSE), ...)`
    /// the comparison atom used to take: clone, set ρ=Q, mark FROM_ATOM.
    pub fn make_bool(&mut self, b: bool) -> u32 {
        let (proto, rho_slot) = if b {
            (self.q_true_off, self.q_true_rho_slot)
        } else {
            (self.q_false_off, self.q_false_rho_slot)
        };
        debug_assert!(
            proto != SENTINEL,
            "make_bool: Q.{} not resolved at load time",
            if b { "true" } else { "false" }
        );
        let cloned = self.exec_copy(proto);
        self.exec_set_by_slot(cloned, rho_slot, 0, SENTINEL, 0);
        let f = self.memory.flags(cloned);
        self.memory.set_flags(cloned, f | FLAG_FROM_ATOM);
        cloned
    }
}
