//! DSL parser + image builder.
//!
//! Reads a `.dsl` text file (see resources/to-dsl.xsl) and produces a Memory
//! byte image plus a separate Arena for Δ payloads. All id refs in the DSL
//! are translated to byte offsets.

use std::collections::HashMap;

use crate::block::*;
use crate::memory::{Arena, Memory};

pub struct Interner {
    pub map: HashMap<String, u16>,
    pub rev: Vec<String>,
}

impl Interner {
    pub fn new_for_attrs() -> Self {
        let mut i = Self { map: HashMap::new(), rev: Vec::new() };
        // The four runtime-magic names must take the fixed ids declared in
        // block.rs (φ/Δ/ρ/λ); the runtime branches on those ids directly.
        let _ = i.intern(STR_PHI);
        let _ = i.intern(STR_DELTA);
        let _ = i.intern(STR_RHO);
        let _ = i.intern(STR_LAMBDA);
        // Stdlib-shape attr names get fixed ids too so atom code can use them
        // as constants without a HashMap lookup per invocation. These are not
        // semantically magic — they're just frequent enough that paying for
        // string→id resolution per atom call would dominate the hot path.
        let _ = i.intern(STR_X);
        let _ = i.intern(STR_NUMBER);
        let _ = i.intern(STR_BYTES);
        let _ = i.intern(STR_TRUE);
        let _ = i.intern(STR_FALSE);
        debug_assert_eq!(i.id_of(STR_PHI), Some(ID_PHI));
        debug_assert_eq!(i.id_of(STR_DELTA), Some(ID_DELTA));
        debug_assert_eq!(i.id_of(STR_RHO), Some(ID_RHO));
        debug_assert_eq!(i.id_of(STR_LAMBDA), Some(ID_LAMBDA));
        debug_assert_eq!(i.id_of(STR_X), Some(ID_X));
        debug_assert_eq!(i.id_of(STR_NUMBER), Some(ID_NUMBER));
        debug_assert_eq!(i.id_of(STR_BYTES), Some(ID_BYTES));
        debug_assert_eq!(i.id_of(STR_TRUE), Some(ID_TRUE));
        debug_assert_eq!(i.id_of(STR_FALSE), Some(ID_FALSE));
        i
    }

    pub fn new_empty() -> Self {
        Self { map: HashMap::new(), rev: Vec::new() }
    }

    pub fn intern(&mut self, s: &str) -> u16 {
        if let Some(&id) = self.map.get(s) {
            return id;
        }
        let id = u16::try_from(self.rev.len()).expect("interner overflow");
        self.map.insert(s.to_string(), id);
        self.rev.push(s.to_string());
        id
    }

    pub fn id_of(&self, s: &str) -> Option<u16> {
        self.map.get(s).copied()
    }
}

pub enum AttrSpec {
    Void,
    Bytes(Vec<u8>),
    Atom(String),
    Ref(u32),
    Cached(u32),
}

pub struct FormLine {
    pub id: u32,
    pub name: String,
    pub attrs: Vec<(String, AttrSpec)>,
}

pub enum AttrField {
    Sentinel,
    Slot(u16),
    Name(String),
}

pub struct DispLine {
    pub id: u32,
    pub from: Option<u32>,
    pub attr: AttrField,
}

pub struct AppLine {
    pub id: u32,
    pub from: Option<u32>,
    pub attr: AttrField,
    pub arg: u32,
    pub cache_flag: bool,
}

pub struct CtxLine {
    pub id: u32,
}

pub enum DslLine {
    Form(FormLine),
    Disp(DispLine),
    App(AppLine),
    Ctx(CtxLine),
}

impl DslLine {
    pub fn id(&self) -> u32 {
        match self {
            DslLine::Form(f) => f.id,
            DslLine::Disp(d) => d.id,
            DslLine::App(a) => a.id,
            DslLine::Ctx(c) => c.id,
        }
    }

    pub fn block_size(&self) -> u32 {
        match self {
            DslLine::Form(f) => frm_size(form_total_slots(f)),
            DslLine::Disp(_) | DslLine::App(_) => DSP_APP_SIZE,
            DslLine::Ctx(_) => CTX_SIZE,
        }
    }
}

/// Number of slots a FORM block actually gets in the loaded image. Every
/// FORM that does not declare ρ in source receives one implicit ρ slot
/// appended at the end. The append (not prepend) preserves positional APP
/// slot indices into the original declared attrs.
pub fn form_total_slots(f: &FormLine) -> u16 {
    let declared = u16::try_from(f.attrs.len()).expect("attr_count > u16");
    if f.attrs.iter().any(|(n, _)| n == STR_RHO) {
        declared
    } else {
        declared + 1
    }
}

fn parse_hex_bytes(s: &str) -> Vec<u8> {
    s.split('-')
        .filter(|p| !p.is_empty())
        .map(|p| u8::from_str_radix(p, 16).expect("bad hex byte"))
        .collect()
}

fn parse_attr_spec(name: &str, spec: &str) -> AttrSpec {
    if spec == "?" {
        return AttrSpec::Void;
    }
    if name == STR_DELTA {
        return AttrSpec::Bytes(parse_hex_bytes(spec));
    }
    if name == STR_LAMBDA {
        return AttrSpec::Atom(spec.to_string());
    }
    if let Some(num) = spec.strip_suffix('!') {
        return AttrSpec::Cached(num.parse().expect("bad cached ref"));
    }
    AttrSpec::Ref(spec.parse().expect("bad ref"))
}

fn parse_attr_field(tok: &str) -> AttrField {
    if tok == "-1" {
        AttrField::Sentinel
    } else if let Ok(n) = tok.parse::<u16>() {
        AttrField::Slot(n)
    } else {
        AttrField::Name(tok.to_string())
    }
}

fn parse_signed_id(tok: &str) -> Option<u32> {
    if tok == "-1" {
        None
    } else {
        Some(tok.parse().expect("bad id"))
    }
}

fn parse_form(rest: &[&str]) -> FormLine {
    let id: u32 = rest[0].parse().expect("FORM id");
    let name = rest[1].to_string();
    let mut attrs = Vec::new();
    for pair in &rest[2..] {
        let (n, s) = pair.split_once(':').expect("FORM attr needs ':'");
        attrs.push((n.to_string(), parse_attr_spec(n, s)));
    }
    FormLine { id, name, attrs }
}

fn parse_disp(rest: &[&str]) -> DispLine {
    DispLine {
        id: rest[0].parse().expect("DISP id"),
        from: parse_signed_id(rest[1]),
        attr: parse_attr_field(rest[2]),
    }
}

fn parse_app(rest: &[&str]) -> AppLine {
    let id: u32 = rest[0].parse().expect("APP id");
    let from = parse_signed_id(rest[1]);
    let attr = parse_attr_field(rest[2]);
    let arg: u32 = rest[3].parse().expect("APP arg");
    let cache_flag = rest.get(4).copied() == Some("CACHE");
    AppLine { id, from, attr, arg, cache_flag }
}

fn parse_ctx(rest: &[&str]) -> CtxLine {
    CtxLine { id: rest[0].parse().expect("CTX id") }
}

pub fn parse_dsl(src: &str) -> Vec<DslLine> {
    let mut out = Vec::new();
    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let toks: Vec<&str> = line.split_whitespace().collect();
        match toks[0] {
            "EOR" | "N" => continue,
            "FORM" => out.push(DslLine::Form(parse_form(&toks[1..]))),
            "DISP" => out.push(DslLine::Disp(parse_disp(&toks[1..]))),
            "APP" => out.push(DslLine::App(parse_app(&toks[1..]))),
            "CTX" => out.push(DslLine::Ctx(parse_ctx(&toks[1..]))),
            other => panic!("unknown DSL tag: {other}"),
        }
    }
    out
}

pub struct Image<'a> {
    pub memory: Memory<'a>,
    pub arena: Arena,
    pub attrs: Interner,
    pub atoms: Interner,
    pub id_to_off: Vec<u32>,
    /// Number of program-level objects loaded (one per DSL line).
    pub program_objects: u32,
}

pub fn load<'a>(src: &str, buf: &'a mut [u8]) -> Image<'a> {
    let lines = parse_dsl(src);

    let max_id = lines.iter().map(|l| l.id()).max().unwrap_or(0);
    let n_objects = (max_id + 1) as usize;
    assert_eq!(n_objects, lines.len(), "non-contiguous ids in DSL");

    let total: u32 = lines.iter().map(|l| l.block_size()).sum();
    assert!(
        total as usize <= buf.len(),
        "program needs {total} bytes but buffer is {} bytes",
        buf.len()
    );

    let mut img = Image {
        memory: Memory::from_buf(buf),
        arena: Arena::new(),
        attrs: Interner::new_for_attrs(),
        atoms: Interner::new_empty(),
        id_to_off: vec![SENTINEL; n_objects],
        program_objects: n_objects as u32,
    };
    img.memory.resize(total);

    let mut off: u32 = 0;
    for line in &lines {
        img.id_to_off[line.id() as usize] = off;
        off += line.block_size();
    }

    for line in &lines {
        emit_one(&mut img, line);
    }

    img.assert_q_at_zero();
    img
}

fn resolve(img: &Image<'_>, id: u32) -> u32 {
    let off = img.id_to_off[id as usize];
    assert!(off != SENTINEL, "ref to unresolved id {id}");
    off
}

fn resolve_opt(img: &Image<'_>, id: Option<u32>) -> u32 {
    id.map(|i| resolve(img, i)).unwrap_or(SENTINEL)
}

fn emit_one(img: &mut Image<'_>, line: &DslLine) {
    match line {
        DslLine::Form(f) => emit_frm(img, f),
        DslLine::Disp(d) => emit_dsp(img, d),
        DslLine::App(a) => emit_app(img, a),
        DslLine::Ctx(c) => emit_ctx(img, c),
    }
}

fn emit_frm(img: &mut Image<'_>, f: &FormLine) {
    let off = img.id_to_off[f.id as usize];
    let total = form_total_slots(f);
    let declared = u16::try_from(f.attrs.len()).expect("attr_count > u16");

    img.memory.set_flags(off, TY_FRM);
    img.memory.set_n(off, total);
    img.memory.set_fwd(off, SENTINEL);

    for (k, (name, spec)) in f.attrs.iter().enumerate() {
        let slot = k as u16;
        let aid = img.attrs.intern(name);
        img.memory.set_frm_name_id(off, slot, aid);

        let (value, xi, cache) = match spec {
            AttrSpec::Void => (SENTINEL, SENTINEL, SENTINEL),
            AttrSpec::Ref(id) => (resolve(img, *id), SENTINEL, SENTINEL),
            AttrSpec::Cached(id) => {
                let o = resolve(img, *id);
                (o, SENTINEL, o)
            }
            AttrSpec::Bytes(bytes) => {
                let arena_off = img.arena.push(bytes);
                let len = u32::try_from(bytes.len()).expect("Δ length overflow");
                (arena_off, len, SENTINEL)
            }
            AttrSpec::Atom(name) => {
                let aid = img.atoms.intern(name);
                (u32::from(aid), SENTINEL, SENTINEL)
            }
        };

        img.memory.set_frm_attr_value(off, slot, value);
        img.memory.set_frm_attr_xi(off, slot, xi);
        img.memory.set_frm_attr_cache(off, slot, cache);
    }

    // Implicit ρ slot for runtime to attach context on copy.
    if total > declared {
        let rho_slot = declared;
        img.memory.set_frm_name_id(off, rho_slot, ID_RHO);
        img.memory.set_frm_attr_value(off, rho_slot, SENTINEL);
        img.memory.set_frm_attr_xi(off, rho_slot, SENTINEL);
        img.memory.set_frm_attr_cache(off, rho_slot, SENTINEL);
    }
}

fn emit_dsp(img: &mut Image<'_>, d: &DispLine) {
    let off = img.id_to_off[d.id as usize];
    let target = resolve_opt(img, d.from);
    let attr_id = match &d.attr {
        AttrField::Sentinel => panic!(
            "DSP {} has attr=-1: bare $ must be encoded as CTX, not DSP",
            d.id
        ),
        AttrField::Slot(_) => panic!("DSP can only carry an attr name, not a positional slot"),
        AttrField::Name(s) => img.attrs.intern(s),
    };
    img.memory.set_flags(off, TY_DSP);
    img.memory.set_n(off, attr_id);
    img.memory.set_fwd(off, SENTINEL);
    img.memory.set_dsp_target(off, target);
    img.memory.write_u32(off + 12, 0);
}

fn emit_app(img: &mut Image<'_>, a: &AppLine) {
    let off = img.id_to_off[a.id as usize];
    let target = resolve_opt(img, a.from);
    let value = resolve(img, a.arg);
    let (flags, n_field) = match &a.attr {
        AttrField::Sentinel => panic!("APP with sentinel attr not supported"),
        AttrField::Slot(s) => (TY_APP | FLAG_UNBOUND, *s),
        AttrField::Name(s) => (TY_APP, img.attrs.intern(s)),
    };
    img.memory.set_flags(off, flags);
    img.memory.set_n(off, n_field);
    img.memory.set_fwd(off, SENTINEL);
    img.memory.set_app_target(off, target);
    img.memory.set_app_value(off, value);
    // CACHE flag on APP not yet defined in the runtime spec; recorded but no-op.
    let _ = a.cache_flag;
}

fn emit_ctx(img: &mut Image<'_>, c: &CtxLine) {
    let off = img.id_to_off[c.id as usize];
    img.memory.set_flags(off, TY_CTX);
    img.memory.set_n(off, 0);
    img.memory.set_fwd(off, SENTINEL);
}

impl<'a> Image<'a> {
    pub fn assert_q_at_zero(&self) {
        debug_assert!(!self.memory.is_empty(), "empty memory: no Q present");
        let ty = self.memory.ty(0);
        debug_assert!(
            ty == TY_FRM,
            "first block (Q) must be a FORMATION, got type {}",
            ty_str(ty)
        );
    }
}
