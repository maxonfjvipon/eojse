//! Block layout constants, sentinels, and pure offset helpers.
//!
//! See README's "Block Layout" section. Every reference everywhere is a `u32`
//! byte offset into a Memory buffer. Sentinels live at the top of the value
//! range so a single `== MAX` check distinguishes "absent."

pub const TY_CTX: u8 = 0;
pub const TY_FRM: u8 = 1;
pub const TY_DSP: u8 = 2;
pub const TY_APP: u8 = 3;
pub const TY_MASK: u8 = 0b11;

pub const FLAG_DEAD: u8 = 1 << 2;
pub const FLAG_STAY: u8 = 1 << 3;
pub const FLAG_FROM_ATOM: u8 = 1 << 4;
pub const FLAG_UNBOUND: u8 = 1 << 5;

pub const SENTINEL: u32 = u32::MAX;

pub const ID_PHI: u16 = 0;
pub const ID_DELTA: u16 = 1;
pub const ID_RHO: u16 = 2;
pub const ID_LAMBDA: u16 = 3;
pub const ID_X: u16 = 4;
pub const ID_NUMBER: u16 = 5;
pub const ID_BYTES: u16 = 6;
pub const ID_TRUE: u16 = 7;
pub const ID_FALSE: u16 = 8;

pub const STR_PHI: &str = "φ";
pub const STR_DELTA: &str = "Δ";
pub const STR_RHO: &str = "ρ";
pub const STR_LAMBDA: &str = "λ";
pub const STR_X: &str = "x";
pub const STR_NUMBER: &str = "number";
pub const STR_BYTES: &str = "bytes";
pub const STR_TRUE: &str = "true";
pub const STR_FALSE: &str = "false";

pub const ATTR_SIZE: u32 = 12;
pub const HEADER_SIZE: u32 = 8;
pub const DSP_APP_SIZE: u32 = 16;
pub const CTX_SIZE: u32 = 8;

#[inline]
pub fn round_up(x: u32, align: u32) -> u32 {
    (x + align - 1) & !(align - 1)
}

#[inline]
pub fn mask_words(n: u16) -> u32 {
    (u32::from(n) + 63) / 64
}

#[inline]
pub fn frm_size(n: u16) -> u32 {
    let w = mask_words(n);
    let nm = round_up(2 * u32::from(n), 4);
    round_up(HEADER_SIZE + 8 * w + nm + ATTR_SIZE * u32::from(n), 8)
}

#[inline]
pub fn frm_mask_off(off: u32) -> u32 {
    off + HEADER_SIZE
}

#[inline]
pub fn frm_names_off(off: u32, n: u16) -> u32 {
    frm_mask_off(off) + 8 * mask_words(n)
}

#[inline]
pub fn frm_attrs_off(off: u32, n: u16) -> u32 {
    frm_names_off(off, n) + round_up(2 * u32::from(n), 4)
}

#[inline]
pub fn frm_attr_off(off: u32, n: u16, slot: u16) -> u32 {
    frm_attrs_off(off, n) + u32::from(slot) * ATTR_SIZE
}

#[inline]
pub fn ty_str(ty: u8) -> &'static str {
    match ty & TY_MASK {
        TY_CTX => "CTX",
        TY_FRM => "FRM",
        TY_DSP => "DSP",
        TY_APP => "APP",
        _ => "???",
    }
}
