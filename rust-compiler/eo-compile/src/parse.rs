//! +-----------------------------------------------------------------+
//! | parse — DSL text -> typed line stream.                           |
//! |                                                                  |
//! | The DSL (emitted by resources/to-dsl.xsl) is line-based and      |
//! | strictly token-positional, so a hand-rolled parser is fine.      |
//! | Each line becomes one `RawLine` variant carrying everything      |
//! | downstream stages need to build the IR graph.                    |
//! |                                                                  |
//! | Grammar (one statement per line, '#' starts a comment):          |
//! |                                                                  |
//! |   FORM <id> <name> [<attr>:<spec>]*                              |
//! |       spec := '?'             — void slot                        |
//! |             | <int>['!']      — ref to object <int>; '!' = cached|
//! |             | <hex>('-'<hex>)*— Δ literal (when attr is 'Δ')     |
//! |             | <ident>         — atom name (when attr is 'λ')     |
//! |                                                                  |
//! |   DISP <id> <from> <attr>                                        |
//! |       from := <int> | '-1'                                       |
//! |                                                                  |
//! |   APP  <id> <from> <attr> <value>                                |
//! |       attr := <int> (slot) | <ident> (name)                      |
//! |                                                                  |
//! |   CTX  <id>                                                      |
//! |                                                                  |
//! | Whitespace separates tokens. Blank lines are skipped.            |
//! +-----------------------------------------------------------------+

#[derive(Debug, Eq, PartialEq)]
pub enum AttrSpec {
    Void,
    Ref { id: u32, cached: bool },
    Delta(Vec<u8>),
    Atom(String),
}

#[derive(Debug, Eq, PartialEq)]
pub struct FormLine {
    pub id: u32,
    pub name: String,
    pub attrs: Vec<(String, AttrSpec)>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum FromRef {
    Object(u32),
    Sentinel,
}

#[derive(Debug, Eq, PartialEq)]
pub enum AttrRef {
    Slot(u32),
    Name(String),
}

#[derive(Debug, Eq, PartialEq)]
pub struct DispLine {
    pub id: u32,
    pub from: FromRef,
    pub attr: AttrRef,
}

#[derive(Debug, Eq, PartialEq)]
pub struct AppLine {
    pub id: u32,
    pub from: FromRef,
    pub attr: AttrRef,
    pub value: u32,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CtxLine {
    pub id: u32,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RawLine {
    Form(FormLine),
    Disp(DispLine),
    App(AppLine),
    Ctx(CtxLine),
}

impl RawLine {
    pub fn id(&self) -> u32 {
        match self {
            RawLine::Form(f) => f.id,
            RawLine::Disp(d) => d.id,
            RawLine::App(a) => a.id,
            RawLine::Ctx(c) => c.id,
        }
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error at line {}: {}", self.line + 1, self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(text: &str) -> Result<Vec<RawLine>, ParseError> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        out.push(parse_line(i, line)?);
    }
    Ok(out)
}

fn parse_line(line_no: usize, line: &str) -> Result<RawLine, ParseError> {
    let mut tokens = line.split_ascii_whitespace();
    let kind = tokens
        .next()
        .ok_or_else(|| err(line_no, "empty statement"))?;
    match kind {
        "FORM" => parse_form(line_no, tokens),
        "DISP" => parse_disp(line_no, tokens),
        "APP" => parse_app(line_no, tokens),
        "CTX" => parse_ctx(line_no, tokens),
        other => Err(err(line_no, format!("unknown statement {other}"))),
    }
}

fn parse_form<'a, I: Iterator<Item = &'a str>>(
    line_no: usize,
    mut tokens: I,
) -> Result<RawLine, ParseError> {
    let id = take_u32(line_no, &mut tokens, "FORM id")?;
    let name = tokens
        .next()
        .ok_or_else(|| err(line_no, "FORM missing name"))?
        .to_string();
    let mut attrs = Vec::new();
    for tok in tokens {
        let (attr_name, rest) = tok
            .split_once(':')
            .ok_or_else(|| err(line_no, format!("FORM attr without colon: {tok}")))?;
        let spec = parse_attr_spec(line_no, attr_name, rest)?;
        attrs.push((attr_name.to_string(), spec));
    }
    Ok(RawLine::Form(FormLine { id, name, attrs }))
}

fn parse_attr_spec(line_no: usize, name: &str, rest: &str) -> Result<AttrSpec, ParseError> {
    if rest == "?" {
        return Ok(AttrSpec::Void);
    }
    if name == "Δ" {
        return Ok(AttrSpec::Delta(parse_hex_bytes(line_no, rest)?));
    }
    if name == "λ" {
        return Ok(AttrSpec::Atom(rest.to_string()));
    }
    let (digits, cached) = if let Some(rest) = rest.strip_suffix('!') {
        (rest, true)
    } else {
        (rest, false)
    };
    let id = digits
        .parse::<u32>()
        .map_err(|e| err(line_no, format!("FORM attr ref {rest:?}: {e}")))?;
    Ok(AttrSpec::Ref { id, cached })
}

fn parse_hex_bytes(line_no: usize, rest: &str) -> Result<Vec<u8>, ParseError> {
    let mut bytes = Vec::new();
    for chunk in rest.split('-') {
        if chunk.is_empty() {
            continue;
        }
        let b = u8::from_str_radix(chunk, 16)
            .map_err(|e| err(line_no, format!("bad hex byte {chunk:?}: {e}")))?;
        bytes.push(b);
    }
    Ok(bytes)
}

fn parse_disp<'a, I: Iterator<Item = &'a str>>(
    line_no: usize,
    mut tokens: I,
) -> Result<RawLine, ParseError> {
    let id = take_u32(line_no, &mut tokens, "DISP id")?;
    let from = take_from(line_no, &mut tokens, "DISP from")?;
    let attr_tok = tokens
        .next()
        .ok_or_else(|| err(line_no, "DISP missing attr"))?;
    let attr = parse_attr_ref(attr_tok);
    if tokens.next().is_some() {
        return Err(err(line_no, "DISP trailing tokens"));
    }
    Ok(RawLine::Disp(DispLine { id, from, attr }))
}

fn parse_app<'a, I: Iterator<Item = &'a str>>(
    line_no: usize,
    mut tokens: I,
) -> Result<RawLine, ParseError> {
    let id = take_u32(line_no, &mut tokens, "APP id")?;
    let from = take_from(line_no, &mut tokens, "APP from")?;
    let attr_tok = tokens
        .next()
        .ok_or_else(|| err(line_no, "APP missing attr"))?;
    let attr = parse_attr_ref(attr_tok);
    let value = take_u32(line_no, &mut tokens, "APP value")?;
    if tokens.next().is_some() {
        return Err(err(line_no, "APP trailing tokens"));
    }
    Ok(RawLine::App(AppLine { id, from, attr, value }))
}

fn parse_ctx<'a, I: Iterator<Item = &'a str>>(
    line_no: usize,
    mut tokens: I,
) -> Result<RawLine, ParseError> {
    let id = take_u32(line_no, &mut tokens, "CTX id")?;
    if tokens.next().is_some() {
        return Err(err(line_no, "CTX trailing tokens"));
    }
    Ok(RawLine::Ctx(CtxLine { id }))
}

fn take_u32<'a, I: Iterator<Item = &'a str>>(
    line_no: usize,
    tokens: &mut I,
    what: &str,
) -> Result<u32, ParseError> {
    let tok = tokens
        .next()
        .ok_or_else(|| err(line_no, format!("missing {what}")))?;
    tok.parse::<u32>()
        .map_err(|e| err(line_no, format!("bad {what} {tok:?}: {e}")))
}

fn take_from<'a, I: Iterator<Item = &'a str>>(
    line_no: usize,
    tokens: &mut I,
    what: &str,
) -> Result<FromRef, ParseError> {
    let tok = tokens
        .next()
        .ok_or_else(|| err(line_no, format!("missing {what}")))?;
    if tok == "-1" {
        return Ok(FromRef::Sentinel);
    }
    let id = tok
        .parse::<u32>()
        .map_err(|e| err(line_no, format!("bad {what} {tok:?}: {e}")))?;
    Ok(FromRef::Object(id))
}

fn parse_attr_ref(tok: &str) -> AttrRef {
    if let Ok(n) = tok.parse::<u32>() {
        AttrRef::Slot(n)
    } else {
        AttrRef::Name(tok.to_string())
    }
}

fn err(line: usize, message: impl Into<String>) -> ParseError {
    ParseError { line, message: message.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_form_with_void_attr() {
        let lines = parse("FORM 19 plus x:? λ:L_number_plus").expect("parse must succeed");
        assert_eq!(
            lines,
            vec![RawLine::Form(FormLine {
                id: 19,
                name: "plus".into(),
                attrs: vec![
                    ("x".into(), AttrSpec::Void),
                    ("λ".into(), AttrSpec::Atom("L_number_plus".into())),
                ],
            })],
            "FORM with void slot and atom must parse"
        );
    }

    #[test]
    fn parses_form_with_cached_and_plain_refs() {
        let lines = parse("FORM 0 Φ program:1! bytes:14").expect("parse must succeed");
        assert_eq!(
            lines,
            vec![RawLine::Form(FormLine {
                id: 0,
                name: "Φ".into(),
                attrs: vec![
                    ("program".into(), AttrSpec::Ref { id: 1, cached: true }),
                    ("bytes".into(), AttrSpec::Ref { id: 14, cached: false }),
                ],
            })],
            "FORM with cached and plain refs must distinguish them"
        );
    }

    #[test]
    fn parses_delta_bytes() {
        let lines = parse("FORM 8 anon Δ:40-14-00-00-00-00-00-00")
            .expect("Δ literal must parse");
        let attrs = match &lines[0] {
            RawLine::Form(f) => &f.attrs,
            _ => panic!("expected FORM line"),
        };
        assert_eq!(
            attrs,
            &vec![(
                "Δ".into(),
                AttrSpec::Delta(vec![0x40, 0x14, 0, 0, 0, 0, 0, 0])
            )],
            "Δ bytes must decode hex-with-dashes"
        );
    }

    #[test]
    fn parses_disp_with_sentinel_from() {
        let lines = parse("DISP 13 -1 x").expect("DISP -1 must parse");
        assert_eq!(
            lines,
            vec![RawLine::Disp(DispLine {
                id: 13,
                from: FromRef::Sentinel,
                attr: AttrRef::Name("x".into()),
            })],
            "DISP with -1 from must use Sentinel"
        );
    }

    #[test]
    fn parses_app_with_slot_attr() {
        let lines = parse("APP 4 5 0 6").expect("APP must parse");
        assert_eq!(
            lines,
            vec![RawLine::App(AppLine {
                id: 4,
                from: FromRef::Object(5),
                attr: AttrRef::Slot(0),
                value: 6,
            })],
            "APP with positional slot attr must keep it as Slot"
        );
    }

    #[test]
    fn parses_ctx_line() {
        let lines = parse("CTX 17").expect("CTX must parse");
        assert_eq!(
            lines,
            vec![RawLine::Ctx(CtxLine { id: 17 })],
            "bare CTX must produce a Ctx line"
        );
    }

    #[test]
    fn skips_blank_lines_and_comments() {
        let lines = parse("\n# header\nCTX 0\n  # mid\nCTX 1\n").expect("comments must skip");
        assert_eq!(
            lines.len(),
            2,
            "blank lines and full-line comments cannot become statements"
        );
    }

    #[test]
    fn cannot_parse_unknown_statement_kind() {
        let err = parse("FOO 1 2").expect_err("unknown kind must fail");
        assert!(
            err.message.contains("unknown statement"),
            "unknown statement message must surface the bad kind"
        );
    }
}
