//! Memory and Arena: byte-buffer backing stores with typed accessors.
//!
//! Memory borrows a `&mut [u8]` slice from the host (typically a stack-
//! allocated array in `main`) and tracks a `len: u32` cursor. Push extends
//! the cursor; pop / truncate rewinds it. The buffer never reallocates,
//! its size is fixed at construction time, and overflow panics.
//!
//! Arena is heap-backed (`Vec<u8>`) because Δ payloads are immutable data,
//! shared across COPYs, and their size is harder to bound up-front. Arena
//! also interns byte sequences (`Arena::push` returns an existing offset
//! when the same bytes have been pushed before) — EO's immutability rule
//! makes the sharing always safe.

use std::collections::HashMap;

use crate::block::*;

pub struct Memory<'a> {
    buf: &'a mut [u8],
    len: u32,
}

impl<'a> Memory<'a> {
    pub fn from_buf(buf: &'a mut [u8]) -> Self {
        Self { buf, len: 0 }
    }

    pub fn capacity(&self) -> u32 {
        self.buf.len() as u32
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn resize(&mut self, n: u32) {
        if n > self.capacity() {
            panic!("memory overflow: resize to {n}, capacity {}", self.capacity());
        }
        if n > self.len {
            for i in self.len..n {
                self.buf[i as usize] = 0;
            }
        }
        self.len = n;
    }

    pub fn truncate(&mut self, n: u32) {
        debug_assert!(n <= self.len, "truncate({n}) on len {}", self.len);
        self.len = n;
    }

    pub fn extend_zero(&mut self, n: u32) {
        let new_len = self.len.checked_add(n).expect("u32 overflow");
        if new_len > self.capacity() {
            panic!(
                "memory overflow: extend by {n} (new len {new_len}, capacity {})",
                self.capacity()
            );
        }
        for i in self.len..new_len {
            self.buf[i as usize] = 0;
        }
        self.len = new_len;
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len as usize]
    }

    #[inline]
    pub fn read_u8(&self, off: u32) -> u8 {
        self.buf[off as usize]
    }

    #[inline]
    pub fn read_u16(&self, off: u32) -> u16 {
        let i = off as usize;
        u16::from_le_bytes([self.buf[i], self.buf[i + 1]])
    }

    #[inline]
    pub fn read_u32(&self, off: u32) -> u32 {
        let i = off as usize;
        u32::from_le_bytes([self.buf[i], self.buf[i + 1], self.buf[i + 2], self.buf[i + 3]])
    }

    #[inline]
    pub fn read_u64(&self, off: u32) -> u64 {
        let i = off as usize;
        u64::from_le_bytes([
            self.buf[i], self.buf[i + 1], self.buf[i + 2], self.buf[i + 3],
            self.buf[i + 4], self.buf[i + 5], self.buf[i + 6], self.buf[i + 7],
        ])
    }

    #[inline]
    pub fn write_u8(&mut self, off: u32, v: u8) {
        self.buf[off as usize] = v;
    }

    #[inline]
    pub fn write_u16(&mut self, off: u32, v: u16) {
        let i = off as usize;
        self.buf[i..i + 2].copy_from_slice(&v.to_le_bytes());
    }

    #[inline]
    pub fn write_u32(&mut self, off: u32, v: u32) {
        let i = off as usize;
        self.buf[i..i + 4].copy_from_slice(&v.to_le_bytes());
    }

    #[inline]
    pub fn write_u64(&mut self, off: u32, v: u64) {
        let i = off as usize;
        self.buf[i..i + 8].copy_from_slice(&v.to_le_bytes());
    }

    // --- header accessors ---

    #[inline]
    pub fn flags(&self, off: u32) -> u8 {
        self.read_u8(off)
    }

    #[inline]
    pub fn set_flags(&mut self, off: u32, v: u8) {
        self.write_u8(off, v);
    }

    #[inline]
    pub fn ty(&self, off: u32) -> u8 {
        self.flags(off) & TY_MASK
    }

    #[inline]
    pub fn n(&self, off: u32) -> u16 {
        self.read_u16(off + 2)
    }

    #[inline]
    pub fn set_n(&mut self, off: u32, v: u16) {
        self.write_u16(off + 2, v);
    }

    #[inline]
    pub fn fwd(&self, off: u32) -> u32 {
        self.read_u32(off + 4)
    }

    #[inline]
    pub fn set_fwd(&mut self, off: u32, v: u32) {
        self.write_u32(off + 4, v);
    }

    #[inline]
    pub fn dead(&self, off: u32) -> bool {
        self.flags(off) & FLAG_DEAD != 0
    }

    #[inline]
    pub fn set_dead(&mut self, off: u32, on: bool) {
        let f = self.flags(off);
        self.set_flags(off, if on { f | FLAG_DEAD } else { f & !FLAG_DEAD });
    }

    #[inline]
    pub fn stay(&self, off: u32) -> bool {
        self.flags(off) & FLAG_STAY != 0
    }

    #[inline]
    pub fn set_stay(&mut self, off: u32, on: bool) {
        let f = self.flags(off);
        self.set_flags(off, if on { f | FLAG_STAY } else { f & !FLAG_STAY });
    }

    #[inline]
    pub fn unbound(&self, off: u32) -> bool {
        self.flags(off) & FLAG_UNBOUND != 0
    }

    // --- DSP / APP target / value ---

    #[inline]
    pub fn dsp_target(&self, off: u32) -> u32 {
        self.read_u32(off + 8)
    }

    #[inline]
    pub fn set_dsp_target(&mut self, off: u32, v: u32) {
        self.write_u32(off + 8, v);
    }

    #[inline]
    pub fn app_target(&self, off: u32) -> u32 {
        self.read_u32(off + 8)
    }

    #[inline]
    pub fn set_app_target(&mut self, off: u32, v: u32) {
        self.write_u32(off + 8, v);
    }

    #[inline]
    pub fn app_value(&self, off: u32) -> u32 {
        self.read_u32(off + 12)
    }

    #[inline]
    pub fn set_app_value(&mut self, off: u32, v: u32) {
        self.write_u32(off + 12, v);
    }

    // --- FRM accessors ---

    #[inline]
    pub fn frm_attr_count(&self, off: u32) -> u16 {
        self.n(off)
    }

    pub fn frm_name_id(&self, off: u32, slot: u16) -> u16 {
        let n = self.n(off);
        let base = frm_names_off(off, n);
        self.read_u16(base + 2 * u32::from(slot))
    }

    pub fn set_frm_name_id(&mut self, off: u32, slot: u16, v: u16) {
        let n = self.n(off);
        let base = frm_names_off(off, n);
        self.write_u16(base + 2 * u32::from(slot), v);
    }

    pub fn frm_attr_value(&self, off: u32, slot: u16) -> u32 {
        let n = self.n(off);
        self.read_u32(frm_attr_off(off, n, slot))
    }

    pub fn set_frm_attr_value(&mut self, off: u32, slot: u16, v: u32) {
        let n = self.n(off);
        self.write_u32(frm_attr_off(off, n, slot), v);
    }

    pub fn frm_attr_xi(&self, off: u32, slot: u16) -> u32 {
        let n = self.n(off);
        self.read_u32(frm_attr_off(off, n, slot) + 4)
    }

    pub fn set_frm_attr_xi(&mut self, off: u32, slot: u16, v: u32) {
        let n = self.n(off);
        self.write_u32(frm_attr_off(off, n, slot) + 4, v);
    }

    pub fn frm_attr_cache(&self, off: u32, slot: u16) -> u32 {
        let n = self.n(off);
        self.read_u32(frm_attr_off(off, n, slot) + 8)
    }

    pub fn set_frm_attr_cache(&mut self, off: u32, slot: u16, v: u32) {
        let n = self.n(off);
        self.write_u32(frm_attr_off(off, n, slot) + 8, v);
    }

    pub fn frm_slot_of(&self, off: u32, name_id: u16) -> Option<u16> {
        let n = self.n(off);
        let base = frm_names_off(off, n);
        for k in 0..n {
            let id = self.read_u16(base + 2 * u32::from(k));
            if id == name_id {
                return Some(k);
            }
        }
        None
    }

    pub fn frm_mask_get(&self, off: u32, slot: u16) -> bool {
        let word = u32::from(slot) / 64;
        let bit = u64::from(slot) % 64;
        let w = self.read_u64(frm_mask_off(off) + 8 * word);
        (w >> bit) & 1 != 0
    }

    pub fn frm_mask_set(&mut self, off: u32, slot: u16) {
        let word = u32::from(slot) / 64;
        let bit = u64::from(slot) % 64;
        let addr = frm_mask_off(off) + 8 * word;
        let w = self.read_u64(addr) | (1u64 << bit);
        self.write_u64(addr, w);
    }

    pub fn block_size_at(&self, off: u32) -> u32 {
        match self.ty(off) {
            TY_CTX => CTX_SIZE,
            TY_FRM => frm_size(self.n(off)),
            TY_DSP | TY_APP => DSP_APP_SIZE,
            _ => panic!("bad block at offset {off}"),
        }
    }

    /// Move `size` bytes from `src` to `dst` within the buffer. Wraps
    /// `slice::copy_within` so callers don't have to range-juggle u32→usize.
    /// LLVM lowers this to memmove with vector chunks; the per-byte loop the
    /// previous `exec_copy` had defeats that.
    #[inline]
    pub fn copy_within(&mut self, src: u32, dst: u32, size: u32) {
        let s = src as usize;
        let d = dst as usize;
        let n = size as usize;
        self.buf.copy_within(s..s + n, d);
    }
}

pub struct Arena {
    buf: Vec<u8>,
    /// Fast-path dedup for 8-byte payloads (every numeric f64). Keying on a
    /// u64 avoids the `bytes.to_vec()` clone the byte-slice map needs.
    dedup8: HashMap<u64, u32>,
    /// Fallback dedup for non-8-byte payloads (strings, raw bytes, etc.).
    /// Still pays a clone on miss; hits clone via `to_vec` until raw_entry
    /// stabilizes — acceptable because numeric programs never touch this.
    dedup_other: HashMap<Vec<u8>, u32>,
    /// Number of times `push` returned an existing offset (no append).
    dedup_hits: u64,
    /// Number of times `push` actually appended new bytes.
    dedup_misses: u64,
}

impl Arena {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            dedup8: HashMap::new(),
            dedup_other: HashMap::new(),
            dedup_hits: 0,
            dedup_misses: 0,
        }
    }

    pub fn len(&self) -> u32 {
        self.buf.len() as u32
    }

    pub fn push(&mut self, bytes: &[u8]) -> u32 {
        if bytes.len() == 8 {
            let key = u64::from_le_bytes(bytes.try_into().unwrap());
            if let Some(&off) = self.dedup8.get(&key) {
                self.dedup_hits += 1;
                return off;
            }
            let off = self.buf.len() as u32;
            self.buf.extend_from_slice(bytes);
            self.dedup8.insert(key, off);
            self.dedup_misses += 1;
            off
        } else {
            if let Some(&off) = self.dedup_other.get(bytes) {
                self.dedup_hits += 1;
                return off;
            }
            let off = self.buf.len() as u32;
            self.buf.extend_from_slice(bytes);
            self.dedup_other.insert(bytes.to_vec(), off);
            self.dedup_misses += 1;
            off
        }
    }

    pub fn slice(&self, off: u32, len: u32) -> &[u8] {
        let s = off as usize;
        &self.buf[s..s + len as usize]
    }

    pub fn dedup_hits(&self) -> u64 {
        self.dedup_hits
    }

    pub fn dedup_misses(&self) -> u64 {
        self.dedup_misses
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_dedup_returns_same_offset_for_same_bytes() {
        let mut arena = Arena::new();
        let off_a = arena.push(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let off_b = arena.push(&[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(off_a, off_b, "identical bytes must dedup to same offset");
        assert_eq!(arena.len(), 8, "arena should hold only one copy");
        assert_eq!(arena.dedup_hits(), 1);
        assert_eq!(arena.dedup_misses(), 1);
    }

    #[test]
    fn arena_dedup_distinguishes_distinct_bytes() {
        let mut arena = Arena::new();
        let a = arena.push(&[0; 8]);
        let b = arena.push(&[1; 8]);
        assert_ne!(a, b);
        assert_eq!(arena.len(), 16);
        assert_eq!(arena.dedup_hits(), 0);
        assert_eq!(arena.dedup_misses(), 2);
    }

    #[test]
    fn arena_dedup_handles_different_lengths() {
        let mut arena = Arena::new();
        let a = arena.push(&[42]);
        let b = arena.push(&[42, 42]);
        let c = arena.push(&[42]);
        assert_ne!(a, b, "different lengths must not collide");
        assert_eq!(a, c, "same bytes must dedup");
        assert_eq!(arena.dedup_hits(), 1);
    }

    #[test]
    fn arena_slice_reads_back_pushed_bytes() {
        let mut arena = Arena::new();
        let off = arena.push(&[10, 20, 30]);
        assert_eq!(arena.slice(off, 3), &[10, 20, 30]);
    }

    #[test]
    fn arena_slice_after_dedup_reads_back_original() {
        let mut arena = Arena::new();
        let off1 = arena.push(&[99, 98, 97]);
        let off2 = arena.push(&[99, 98, 97]);
        assert_eq!(off1, off2);
        assert_eq!(arena.slice(off2, 3), &[99, 98, 97]);
    }
}
