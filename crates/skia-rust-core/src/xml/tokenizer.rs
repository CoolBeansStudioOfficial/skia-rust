// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/xml/SkXMLParser.cpp (the expat callbacks), standing in for libexpat.

//! A safe XML tokenizer that produces the callbacks `SkXMLParser` receives from expat.
//!
//! `SkXMLParser::parse` hands the whole document to expat (`XML_Parse(..., final = true)`) and
//! forwards three kinds of events: element start (with the attributes of the tag, in expat's
//! order), element end, and the character data between two element events as one buffered
//! string. An entity declaration stops the parse (`XML_StopParser`), which `parse` reports as a
//! failure. This module reproduces those events and the documents expat rejects for the
//! constructs documents use in practice; `docs/API_MAPPING.md` lists what is intentionally left
//! out. The behavior was checked against expat 2.6 with a differential corpus
//! (`tests/xml_expat_corpus.rs`).
//!
//! The pieces, in the order a document meets them:
//!
//! - [`decode`]: the encoding (BOM sniffing, UTF-16, the XML declaration's `encoding`) and the
//!   check that every character is an XML `Char`. Like expat's incremental scanning, a document
//!   is parsed up to its first invalid character; the events before it are delivered.
//! - the XML declaration, strict like expat's `XmlParseXmlDecl`;
//! - the prolog and the DTD, scanned with expat's prolog token rules, with the declaration
//!   grammar of `xmlrole.c` for `<!ELEMENT>`, `<!ATTLIST>` (whose defaults and attribute types
//!   change the attributes of start tags), `<!NOTATION>` and `<!ENTITY>`;
//! - the root element and its content, and the epilog.

use std::collections::HashMap;

use super::name_tables::Names;
use super::parser::XmlParser;

/// A well-formedness error (the position is not needed: `SkXMLParser` only reports failure).
#[derive(Debug, Clone, Copy)]
struct Malformed;

type Res<T = ()> = Result<T, Malformed>;

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n')
}

/// The XML `Char` production (expat rejects every other character everywhere in a document).
fn is_xml_char(c: u32) -> bool {
    matches!(c, 0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x1_0000..=0x10_FFFF)
}

// ---------------------------------------------------------------------------------------------
// Encodings

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Initial {
    Utf8,
    Utf16Le,
    Utf16Be,
}

/// The encodings expat has built in (`xmltok.c`'s `encodingsNames`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Declared {
    Latin1,
    Ascii,
    Utf8,
    Utf16,
    Utf16Le,
    Utf16Be,
}

fn declared_encoding(name: &[u8]) -> Option<Declared> {
    let lower = name.to_ascii_lowercase();
    Some(match lower.as_slice() {
        b"iso-8859-1" => Declared::Latin1,
        b"us-ascii" => Declared::Ascii,
        b"utf-8" => Declared::Utf8,
        b"utf-16" => Declared::Utf16,
        b"utf-16le" => Declared::Utf16Le,
        b"utf-16be" => Declared::Utf16Be,
        _ => return None,
    })
}

/// What the XML declaration at the start of a document says.
#[derive(Debug, Default)]
struct XmlDecl {
    /// The byte length of the declaration, `<?xml ... ?>` included.
    len: usize,
    encoding: Option<Vec<u8>>,
    standalone: bool,
}

/// Parses an XML declaration at the start of `b` (a UTF-8 or ASCII view of the document), as
/// expat's `XmlParseXmlDecl` does. `Ok(None)` when `b` does not start with `<?xml` and a space.
fn parse_xml_decl(b: &[u8]) -> Res<Option<XmlDecl>> {
    if !(b.starts_with(b"<?xml") && b.get(5).copied().is_some_and(is_space)) {
        return Ok(None);
    }
    let mut pos = 5;
    let mut decl = XmlDecl::default();

    // One pseudo attribute: S name S? = S? ("value" | 'value'). Returns (name, value).
    let pseudo = |pos: &mut usize| -> Res<Option<(&[u8], &[u8])>> {
        let had_space = {
            let start = *pos;
            while b.get(*pos).copied().is_some_and(is_space) {
                *pos += 1;
            }
            *pos > start
        };
        if b[*pos..].starts_with(b"?>") {
            return Ok(None);
        }
        if !had_space {
            return Err(Malformed);
        }
        let name_start = *pos;
        while b
            .get(*pos)
            .copied()
            .is_some_and(|c| c.is_ascii_alphabetic())
        {
            *pos += 1;
        }
        let name = &b[name_start..*pos];
        if name.is_empty() {
            return Err(Malformed);
        }
        while b.get(*pos).copied().is_some_and(is_space) {
            *pos += 1;
        }
        if b.get(*pos) != Some(&b'=') {
            return Err(Malformed);
        }
        *pos += 1;
        while b.get(*pos).copied().is_some_and(is_space) {
            *pos += 1;
        }
        let Some(&quote @ (b'"' | b'\'')) = b.get(*pos) else {
            return Err(Malformed);
        };
        *pos += 1;
        let value_start = *pos;
        loop {
            match b.get(*pos) {
                Some(&c) if c == quote => break,
                // the value characters expat accepts in a pseudo attribute
                Some(&c) if c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_') => {
                    *pos += 1;
                }
                _ => return Err(Malformed),
            }
        }
        let value = &b[value_start..*pos];
        *pos += 1;
        Ok(Some((name, value)))
    };

    // version is required and comes first
    match pseudo(&mut pos)? {
        Some((b"version", _)) => {}
        _ => return Err(Malformed),
    }
    let mut next = pseudo(&mut pos)?;
    if let Some((b"encoding", value)) = next {
        // the encoding name is [A-Za-z][A-Za-z0-9._-]*
        let valid = value.first().is_some_and(u8::is_ascii_alphabetic)
            && value
                .iter()
                .all(|&c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'));
        if !valid {
            return Err(Malformed);
        }
        decl.encoding = Some(value.to_vec());
        next = pseudo(&mut pos)?;
    }
    if let Some((b"standalone", value)) = next {
        decl.standalone = match value {
            b"yes" => true,
            b"no" => false,
            _ => return Err(Malformed),
        };
        next = pseudo(&mut pos)?;
    }
    if next.is_some() {
        return Err(Malformed);
    }
    debug_assert!(b[pos..].starts_with(b"?>"));
    decl.len = pos + 2;
    Ok(Some(decl))
}

/// The document as text: the longest prefix of valid characters, and whether anything invalid
/// followed it.
struct Decoded {
    text: String,
    /// The document continues with an invalid character (or a malformed encoding): after the
    /// events of `text`, the parse fails.
    poisoned: bool,
    /// The document is ISO-8859-1 (names are classified with expat's Latin-1 table).
    latin1: bool,
}

/// Appends `c` to `out` if it is an XML `Char`.
fn push_char(out: &mut String, c: char) -> bool {
    if is_xml_char(u32::from(c)) {
        out.push(c);
        true
    } else {
        false
    }
}

fn decode_utf16(units: &[u8], big_endian: bool) -> Decoded {
    let mut text = String::with_capacity(units.len() / 2);
    let mut iter = units.as_chunks::<2>().0.iter().map(|&p| {
        if big_endian {
            u16::from_be_bytes(p)
        } else {
            u16::from_le_bytes(p)
        }
    });
    let mut poisoned = !units.len().is_multiple_of(2);
    while let Some(unit) = iter.next() {
        // expat takes any unit after a high surrogate as the second half of the pair, using its
        // low ten bits; a low surrogate that starts a character is invalid.
        let code = match unit {
            0xD800..=0xDBFF => {
                let Some(next) = iter.next() else {
                    poisoned = true;
                    break;
                };
                0x1_0000 + ((u32::from(unit) & 0x3FF) << 10) + (u32::from(next) & 0x3FF)
            }
            0xDC00..=0xDFFF => {
                poisoned = true;
                break;
            }
            _ => u32::from(unit),
        };
        match char::from_u32(code) {
            Some(c) if push_char(&mut text, c) => {}
            _ => {
                poisoned = true;
                break;
            }
        }
    }
    Decoded {
        text,
        poisoned,
        latin1: false,
    }
}

fn decode_utf8(bytes: &[u8]) -> Decoded {
    let (valid, mut poisoned) = match std::str::from_utf8(bytes) {
        Ok(s) => (s, false),
        Err(e) => (
            std::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or(""),
            true,
        ),
    };
    let mut text = String::with_capacity(valid.len());
    for c in valid.chars() {
        if !push_char(&mut text, c) {
            poisoned = true;
            break;
        }
    }
    Decoded {
        text,
        poisoned,
        latin1: false,
    }
}

fn decode_single_byte(bytes: &[u8], ascii_only: bool) -> Decoded {
    let mut text = String::with_capacity(bytes.len());
    let mut poisoned = false;
    for &b in bytes {
        if (ascii_only && b >= 0x80) || !push_char(&mut text, char::from(b)) {
            poisoned = true;
            break;
        }
    }
    Decoded {
        text,
        poisoned,
        latin1: false,
    }
}

/// Decodes `doc` the way expat does: the initial encoding comes from a BOM or the first two
/// bytes, the XML declaration may switch it, and a declared encoding the initial one cannot be
/// is an error. Returns the decoded document and whether it is standalone.
fn decode(doc: &[u8]) -> Res<(Decoded, bool)> {
    // initScan
    let (initial, body) = if let Some(rest) = doc.strip_prefix(b"\xEF\xBB\xBF") {
        (Initial::Utf8, rest)
    } else if let Some(rest) = doc.strip_prefix(b"\xFF\xFE") {
        (Initial::Utf16Le, rest)
    } else if let Some(rest) = doc.strip_prefix(b"\xFE\xFF") {
        (Initial::Utf16Be, rest)
    } else if doc.len() >= 2 && doc[0] == 0 && doc[1] != 0 {
        (Initial::Utf16Be, doc)
    } else if doc.len() >= 2 && doc[1] == 0 && doc[0] != 0 {
        (Initial::Utf16Le, doc)
    } else {
        (Initial::Utf8, doc)
    };

    if initial == Initial::Utf8 {
        let decl = parse_xml_decl(body)?;
        let standalone = decl.as_ref().is_some_and(|d| d.standalone);
        let declared = match decl.and_then(|d| d.encoding) {
            Some(name) => declared_encoding(&name).ok_or(Malformed)?,
            None => Declared::Utf8,
        };
        let decoded = match declared {
            Declared::Utf8 => decode_utf8(body),
            Declared::Latin1 => {
                let mut d = decode_single_byte(body, false);
                d.latin1 = true;
                d
            }
            Declared::Ascii => decode_single_byte(body, true),
            // a UTF-16 declaration in a document that is not UTF-16
            Declared::Utf16 | Declared::Utf16Le | Declared::Utf16Be => return Err(Malformed),
        };
        return Ok((decoded, standalone));
    }

    let big_endian = initial == Initial::Utf16Be;
    let decoded = decode_utf16(body, big_endian);
    let decl = parse_xml_decl(decoded.text.as_bytes())?;
    let standalone = decl.as_ref().is_some_and(|d| d.standalone);
    if let Some(name) = decl.and_then(|d| d.encoding) {
        match declared_encoding(&name).ok_or(Malformed)? {
            Declared::Utf16 => {}
            Declared::Utf16Le if !big_endian => {}
            Declared::Utf16Be if big_endian => {}
            _ => return Err(Malformed),
        }
    }
    Ok((decoded, standalone))
}

// ---------------------------------------------------------------------------------------------
// The scanner

/// One attribute declared by an `<!ATTLIST>` (expat's `DEFAULT_ATTRIBUTE`).
#[derive(Debug, Clone)]
struct AttDef {
    name: String,
    is_cdata: bool,
    /// The (normalized) default value, if the declaration has one.
    value: Option<String>,
}

/// The tokens of the DTD (the prolog tokens of expat's `xmltok_impl.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tok<'a> {
    /// `<!KEYWORD` followed by white space or `%`.
    DeclOpen(&'a str),
    Name(&'a str),
    Nmtoken,
    /// `#NAME`.
    PoundName(&'a str),
    Literal(&'a str),
    /// `%name;`.
    ParamEntityRef,
    Percent,
    OpenBracket,
    CloseBracket,
    OpenParen,
    /// `)` with its optional suffix.
    CloseParen(Option<u8>),
    /// A name directly followed by `?`, `*` or `+`.
    NameSuffixed,
    Or,
    Comma,
    DeclClose,
    Comment,
    Pi,
    Eof,
}

struct Scanner<'a, 'h> {
    s: &'a str,
    b: &'a [u8],
    pos: usize,
    handler: &'h mut dyn XmlParser,
    standalone: bool,
    names: Names,
    /// expat's `dtd->hasParamEntityRefs`: set by a parameter entity reference and by an
    /// external DTD id. While it is set (and the document is not standalone), undefined general
    /// entities are skipped instead of being errors.
    has_param_entity_refs: bool,
    /// expat's `dtd->keepProcessing`.
    keep_processing: bool,
    /// The attributes declared by `<!ATTLIST>`, by element name (`ELEMENT_TYPE::defaultAtts`).
    att_defs: HashMap<String, Vec<AttDef>>,
}

impl<'a> Scanner<'a, '_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.pos).copied()
    }

    fn peek_at(&self, n: usize) -> Option<u8> {
        self.b.get(self.pos + n).copied()
    }

    fn starts_with(&self, lit: &str) -> bool {
        self.b[self.pos..].starts_with(lit.as_bytes())
    }

    fn skip_space(&mut self) -> bool {
        let start = self.pos;
        while self.peek().is_some_and(is_space) {
            self.pos += 1;
        }
        self.pos > start
    }

    fn peek_char(&self) -> Option<char> {
        self.s[self.pos..].chars().next()
    }

    /// Scans a name at the current position.
    fn name(&mut self) -> Res<&'a str> {
        let start = self.pos;
        let mut chars = self.s[start..].chars();
        match chars.next() {
            Some(c) if self.names.start(c) => {}
            _ => return Err(Malformed),
        }
        let mut end = start + self.s[start..].chars().next().map_or(0, char::len_utf8);
        for c in chars {
            if !self.names.name(c) {
                break;
            }
            end += c.len_utf8();
        }
        self.pos = end;
        Ok(&self.s[start..end])
    }

    /// Skips up to and including `end`.
    fn skip_past(&mut self, end: &str) -> Res {
        let hay = &self.b[self.pos..];
        let e = end.as_bytes();
        match hay.windows(e.len()).position(|w| w == e) {
            Some(i) => {
                self.pos += i + e.len();
                Ok(())
            }
            None => Err(Malformed),
        }
    }

    /// `<!--` ... `-->`; `--` is not allowed inside. At the `<`.
    fn comment(&mut self) -> Res {
        debug_assert!(self.starts_with("<!--"));
        self.pos += 4;
        let hay = &self.b[self.pos..];
        let i = hay.windows(2).position(|w| w == b"--").ok_or(Malformed)?;
        if hay.get(i + 2) != Some(&b'>') {
            return Err(Malformed);
        }
        self.pos += i + 3;
        Ok(())
    }

    /// `<?target ...?>`. At the `<`. The target `xml` (in any case) is reserved.
    fn pi(&mut self) -> Res {
        debug_assert!(self.starts_with("<?"));
        self.pos += 2;
        let target = self.name()?;
        if target.eq_ignore_ascii_case("xml") {
            return Err(Malformed);
        }
        match self.peek() {
            Some(c) if is_space(c) => self.skip_past("?>"),
            Some(b'?') if self.peek_at(1) == Some(b'>') => {
                self.pos += 2;
                Ok(())
            }
            _ => Err(Malformed),
        }
    }

    // -----------------------------------------------------------------------------------------
    // References and attribute values

    /// Parses a reference at the `&`: returns the replacement for a predefined entity or a
    /// character reference, `None` for any other well-formed entity reference (an undefined
    /// entity). Errors on malformed references.
    fn reference(&mut self) -> Res<Option<char>> {
        debug_assert_eq!(self.peek(), Some(b'&'));
        self.pos += 1;
        if self.peek() == Some(b'#') {
            self.pos += 1;
            let (radix, digits_from) = if self.peek() == Some(b'x') {
                (16, self.pos + 1)
            } else {
                (10, self.pos)
            };
            let mut end = digits_from;
            while self
                .b
                .get(end)
                .is_some_and(|&c| char::from(c).is_digit(radix))
            {
                end += 1;
            }
            if end == digits_from || self.b.get(end) != Some(&b';') {
                return Err(Malformed);
            }
            let digits = &self.s[digits_from..end];
            let code = u32::from_str_radix(digits, radix).map_err(|_| Malformed)?;
            if !is_xml_char(code) {
                return Err(Malformed);
            }
            self.pos = end + 1;
            return char::from_u32(code).map(Some).ok_or(Malformed);
        }
        let name = self.name()?;
        if self.peek() != Some(b';') {
            return Err(Malformed);
        }
        self.pos += 1;
        Ok(match name {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => {
                // [WFC: Entity Declared]: only with a DTD that might declare it is an
                // undeclared entity skipped. No entity is ever declared (an entity
                // declaration stops the parse).
                if !self.has_param_entity_refs || self.standalone {
                    return Err(Malformed);
                }
                None
            }
        })
    }

    /// An attribute value literal (expat's `appendAttributeValue`), at the opening quote.
    /// `is_cdata` is false for the declared types that normalize white space.
    fn attribute_value(&mut self, is_cdata: bool) -> Res<String> {
        let Some(quote @ (b'"' | b'\'')) = self.peek() else {
            return Err(Malformed);
        };
        self.pos += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(Malformed);
            };
            match c {
                c if c == quote => {
                    self.pos += 1;
                    break;
                }
                b'<' => return Err(Malformed),
                b'&' => {
                    if let Some(ch) = self.reference()? {
                        // a character reference to a space collapses like white space
                        if !is_cdata && ch == ' ' && (out.is_empty() || out.ends_with(' ')) {
                            continue;
                        }
                        out.push(ch);
                    }
                }
                b' ' | b'\t' | b'\n' | b'\r' => {
                    self.pos += 1;
                    if c == b'\r' && self.peek() == Some(b'\n') {
                        self.pos += 1;
                    }
                    if !is_cdata && (out.is_empty() || out.ends_with(' ')) {
                        continue;
                    }
                    out.push(' ');
                }
                _ => {
                    let ch = self.peek_char().ok_or(Malformed)?;
                    self.pos += ch.len_utf8();
                    out.push(ch);
                }
            }
        }
        if !is_cdata && out.ends_with(' ') {
            out.pop();
        }
        Ok(out)
    }

    // -----------------------------------------------------------------------------------------
    // The DTD

    /// Reads the next token of the DTD (expat's `prologTok`), skipping white space.
    #[allow(clippy::too_many_lines)] // one arm per token kind, like `prologTok`
    fn dtd_token(&mut self) -> Res<Tok<'a>> {
        self.skip_space();
        let Some(c) = self.peek() else {
            return Ok(Tok::Eof);
        };
        // After a name, a number token or a literal only these may follow.
        let name_end_ok = |next: Option<u8>| {
            next.is_some_and(|n| {
                is_space(n) || matches!(n, b'>' | b')' | b',' | b'|' | b'[' | b'%')
            })
        };
        match c {
            b'"' | b'\'' => {
                self.pos += 1;
                let start = self.pos;
                let end = self.b[self.pos..]
                    .iter()
                    .position(|&x| x == c)
                    .ok_or(Malformed)?;
                self.pos += end + 1;
                if !self
                    .peek()
                    .is_some_and(|n| is_space(n) || matches!(n, b'>' | b'%' | b'['))
                {
                    return Err(Malformed);
                }
                Ok(Tok::Literal(&self.s[start..start + end]))
            }
            b'<' => {
                if self.starts_with("<!--") {
                    self.comment()?;
                    return Ok(Tok::Comment);
                }
                if self.starts_with("<?") {
                    self.pi()?;
                    return Ok(Tok::Pi);
                }
                if self.starts_with("<![") {
                    // conditional sections are only allowed in external entities
                    return Err(Malformed);
                }
                if !self.starts_with("<!") {
                    return Err(Malformed);
                }
                let start = self.pos + 2;
                let mut end = start;
                while self.b.get(end).is_some_and(u8::is_ascii_alphabetic) {
                    end += 1;
                }
                if end == start {
                    return Err(Malformed);
                }
                match self.b.get(end) {
                    Some(&n) if is_space(n) => {}
                    Some(b'%') => {
                        // don't allow <!ENTITY% foo "whatever">
                        if !self
                            .b
                            .get(end + 1)
                            .is_some_and(|&n| is_space(n) || n == b'%')
                        {
                            return Err(Malformed);
                        }
                    }
                    _ => return Err(Malformed),
                }
                self.pos = end;
                Ok(Tok::DeclOpen(&self.s[start..end]))
            }
            b'>' => {
                self.pos += 1;
                Ok(Tok::DeclClose)
            }
            b'[' => {
                self.pos += 1;
                Ok(Tok::OpenBracket)
            }
            b']' => {
                // `]]>` closes a conditional section, which is not allowed here either
                if self.starts_with("]]>") {
                    return Err(Malformed);
                }
                self.pos += 1;
                Ok(Tok::CloseBracket)
            }
            b'(' => {
                self.pos += 1;
                Ok(Tok::OpenParen)
            }
            b')' => {
                self.pos += 1;
                match self.peek() {
                    Some(s @ (b'*' | b'?' | b'+')) => {
                        self.pos += 1;
                        Ok(Tok::CloseParen(Some(s)))
                    }
                    Some(n) if is_space(n) || matches!(n, b'>' | b',' | b'|' | b')') => {
                        Ok(Tok::CloseParen(None))
                    }
                    _ => Err(Malformed),
                }
            }
            b'|' => {
                self.pos += 1;
                Ok(Tok::Or)
            }
            b',' => {
                self.pos += 1;
                Ok(Tok::Comma)
            }
            b'%' => {
                self.pos += 1;
                if self.peek_char().is_some_and(|c| self.names.start(c)) {
                    self.name()?;
                    if self.peek() != Some(b';') {
                        return Err(Malformed);
                    }
                    self.pos += 1;
                    Ok(Tok::ParamEntityRef)
                } else if self.peek().is_some_and(|n| is_space(n) || n == b'%') {
                    Ok(Tok::Percent)
                } else {
                    Err(Malformed)
                }
            }
            b'#' => {
                self.pos += 1;
                if !self.peek_char().is_some_and(|c| self.names.start(c)) {
                    return Err(Malformed);
                }
                let name = self.name()?;
                if !name_end_ok(self.peek()) {
                    return Err(Malformed);
                }
                Ok(Tok::PoundName(name))
            }
            _ => {
                let ch = self.peek_char().ok_or(Malformed)?;
                if self.names.start(ch) {
                    let name = self.name()?;
                    match self.peek() {
                        Some(b'?' | b'*' | b'+') => {
                            self.pos += 1;
                            Ok(Tok::NameSuffixed)
                        }
                        n if name_end_ok(n) => Ok(Tok::Name(name)),
                        _ => Err(Malformed),
                    }
                } else if self.names.name(ch) {
                    // an Nmtoken that is not a name
                    while self.peek_char().is_some_and(|c| self.names.name(c)) {
                        self.pos += self.peek_char().map_or(0, char::len_utf8);
                    }
                    if !name_end_ok(self.peek()) {
                        return Err(Malformed);
                    }
                    Ok(Tok::Nmtoken)
                } else {
                    Err(Malformed)
                }
            }
        }
    }

    /// `<!DOCTYPE` ... `>` (the grammar of `xmlrole.c`'s `doctype0` ... `doctype5`). At the `<`.
    fn doctype(&mut self) -> Res {
        debug_assert!(self.starts_with("<!DOCTYPE"));
        // `<!DOCTYPE` must be followed by white space
        match self.dtd_token()? {
            Tok::DeclOpen("DOCTYPE") => {}
            _ => return Err(Malformed),
        }
        match self.dtd_token()? {
            Tok::Name(_) => {}
            _ => return Err(Malformed),
        }
        let mut external = false;
        let mut tok = self.dtd_token()?;
        match tok {
            Tok::Name("SYSTEM") => {
                self.expect_system_literal()?;
                external = true;
                tok = self.dtd_token()?;
            }
            Tok::Name("PUBLIC") => {
                self.expect_public_literal()?;
                self.expect_system_literal()?;
                external = true;
                tok = self.dtd_token()?;
            }
            _ => {}
        }
        if tok == Tok::OpenBracket {
            self.internal_subset()?;
            tok = self.dtd_token()?;
        }
        if tok != Tok::DeclClose {
            return Err(Malformed);
        }
        if external {
            // the external subset is not read, so its declarations might declare entities
            self.has_param_entity_refs = true;
            self.keep_processing = self.standalone;
        }
        Ok(())
    }

    fn expect_system_literal(&mut self) -> Res {
        match self.dtd_token()? {
            Tok::Literal(_) => Ok(()),
            _ => Err(Malformed),
        }
    }

    fn expect_public_literal(&mut self) -> Res {
        match self.dtd_token()? {
            Tok::Literal(id) if id.bytes().all(is_public_id_char) => Ok(()),
            _ => Err(Malformed),
        }
    }

    /// The internal subset, after the `[`, up to and including the `]`.
    fn internal_subset(&mut self) -> Res {
        loop {
            match self.dtd_token()? {
                Tok::Comment | Tok::Pi => {}
                Tok::ParamEntityRef => {
                    self.has_param_entity_refs = true;
                    if !self.standalone {
                        self.keep_processing = false;
                    }
                }
                Tok::CloseBracket => return Ok(()),
                Tok::DeclOpen("ELEMENT") => self.element_decl()?,
                Tok::DeclOpen("ATTLIST") => self.attlist_decl()?,
                Tok::DeclOpen("ENTITY") => self.entity_decl()?,
                Tok::DeclOpen("NOTATION") => self.notation_decl()?,
                _ => return Err(Malformed),
            }
        }
    }

    fn expect_decl_close(&mut self) -> Res {
        match self.dtd_token()? {
            Tok::DeclClose => Ok(()),
            _ => Err(Malformed),
        }
    }

    /// `<!ELEMENT name contentspec>`: the content model is only checked, not used.
    fn element_decl(&mut self) -> Res {
        match self.dtd_token()? {
            Tok::Name(_) => {}
            _ => return Err(Malformed),
        }
        match self.dtd_token()? {
            Tok::Name("EMPTY" | "ANY") => {}
            Tok::OpenParen => {
                let mut tok = self.dtd_token()?;
                if tok == Tok::PoundName("PCDATA") {
                    // Mixed: (#PCDATA) | (#PCDATA) * | (#PCDATA | name)*
                    tok = self.dtd_token()?;
                    match tok {
                        Tok::CloseParen(None | Some(b'*')) => {}
                        Tok::Or => loop {
                            if !matches!(self.dtd_token()?, Tok::Name(_)) {
                                return Err(Malformed);
                            }
                            match self.dtd_token()? {
                                Tok::Or => {}
                                Tok::CloseParen(Some(b'*')) => break,
                                _ => return Err(Malformed),
                            }
                        },
                        _ => return Err(Malformed),
                    }
                } else {
                    self.content_group(tok)?;
                }
            }
            _ => return Err(Malformed),
        }
        self.expect_decl_close()
    }

    /// A content model group after its `(`; `first` is the token after the `(`.
    fn content_group(&mut self, first: Tok<'a>) -> Res {
        self.content_particle(&first)?;
        let mut tok = self.dtd_token()?;
        let separator = match tok {
            Tok::Comma | Tok::Or => Some(tok),
            _ => None,
        };
        if let Some(sep) = separator {
            while tok == sep {
                let next = self.dtd_token()?;
                self.content_particle(&next)?;
                tok = self.dtd_token()?;
            }
        }
        match tok {
            Tok::CloseParen(_) => Ok(()),
            _ => Err(Malformed),
        }
    }

    fn content_particle(&mut self, tok: &Tok<'a>) -> Res {
        match tok {
            Tok::Name(_) | Tok::NameSuffixed => Ok(()),
            Tok::OpenParen => {
                let first = self.dtd_token()?;
                self.content_group(first)
            }
            _ => Err(Malformed),
        }
    }

    /// `<!ATTLIST element (name type default)*>`. Records the declared attributes.
    fn attlist_decl(&mut self) -> Res {
        let element = match self.dtd_token()? {
            Tok::Name(n) => n.to_owned(),
            _ => return Err(Malformed),
        };
        loop {
            let name = match self.dtd_token()? {
                Tok::DeclClose => return Ok(()),
                Tok::Name(n) => n.to_owned(),
                _ => return Err(Malformed),
            };
            // the type
            let mut is_cdata = false;
            let mut is_id = false;
            match self.dtd_token()? {
                Tok::Name("CDATA") => is_cdata = true,
                Tok::Name("ID") => is_id = true,
                Tok::Name("IDREF" | "IDREFS" | "ENTITY" | "ENTITIES" | "NMTOKEN" | "NMTOKENS") => {}
                Tok::Name("NOTATION") => {
                    if self.dtd_token()? != Tok::OpenParen {
                        return Err(Malformed);
                    }
                    loop {
                        if !matches!(self.dtd_token()?, Tok::Name(_)) {
                            return Err(Malformed);
                        }
                        match self.dtd_token()? {
                            Tok::Or => {}
                            Tok::CloseParen(None) => break,
                            _ => return Err(Malformed),
                        }
                    }
                }
                Tok::OpenParen => loop {
                    if !matches!(self.dtd_token()?, Tok::Name(_) | Tok::Nmtoken) {
                        return Err(Malformed);
                    }
                    match self.dtd_token()? {
                        Tok::Or => {}
                        Tok::CloseParen(None) => break,
                        _ => return Err(Malformed),
                    }
                },
                _ => return Err(Malformed),
            }
            // the default
            self.skip_space();
            let literal_at = self.pos;
            let value = match self.dtd_token()? {
                Tok::PoundName("FIXED") if self.keep_processing => {
                    Some(self.attlist_default(is_cdata)?)
                }
                Tok::Literal(_) if self.keep_processing => {
                    // the literal is parsed again as an attribute value
                    self.pos = literal_at;
                    Some(self.attlist_default(is_cdata)?)
                }
                Tok::PoundName("FIXED") => {
                    // skipped declarations are only scanned
                    self.expect_system_literal()?;
                    None
                }
                Tok::PoundName("REQUIRED" | "IMPLIED") | Tok::Literal(_) => None,
                _ => return Err(Malformed),
            };
            if !self.keep_processing {
                continue;
            }
            self.define_attribute(&element, name, is_cdata, is_id, value);
        }
    }

    /// The literal of a `#FIXED` or plain default, as an attribute value.
    fn attlist_default(&mut self, is_cdata: bool) -> Res<String> {
        self.skip_space();
        let value = self.attribute_value(is_cdata)?;
        if !self
            .peek()
            .is_some_and(|n| is_space(n) || matches!(n, b'>' | b'%' | b'['))
        {
            return Err(Malformed);
        }
        Ok(value)
    }

    /// expat's `defineAttribute`.
    fn define_attribute(
        &mut self,
        element: &str,
        name: String,
        is_cdata: bool,
        is_id: bool,
        value: Option<String>,
    ) {
        let defs = self.att_defs.entry(element.to_owned()).or_default();
        if value.is_some() || is_id {
            // The handling of default attributes gets messed up if we have a default which
            // duplicates a non-default.
            if defs.iter().any(|d| d.name == name) {
                return;
            }
        }
        defs.push(AttDef {
            name,
            is_cdata,
            value,
        });
    }

    /// `<!ENTITY ...>`. The declaration is checked against the grammar; unless it is skipped
    /// (a predefined general entity, or `keepProcessing` is off) it stops the parse, which is
    /// what `SkXMLParser`'s entity declaration handler does.
    fn entity_decl(&mut self) -> Res {
        let mut tok = self.dtd_token()?;
        let is_param = tok == Tok::Percent;
        if is_param {
            tok = self.dtd_token()?;
        }
        let Tok::Name(name) = tok else {
            return Err(Malformed);
        };
        let predefined = !is_param && matches!(name, "lt" | "gt" | "amp" | "quot" | "apos");
        match self.dtd_token()? {
            Tok::Literal(value) => {
                if self.keep_processing {
                    check_entity_value(value, self.names)?;
                }
            }
            Tok::Name("SYSTEM") => {
                self.expect_system_literal()?;
                if !is_param {
                    // optional NDATA name
                    match self.dtd_token()? {
                        Tok::DeclClose => return self.finish_entity(predefined),
                        Tok::Name("NDATA") => {
                            if !matches!(self.dtd_token()?, Tok::Name(_)) {
                                return Err(Malformed);
                            }
                        }
                        _ => return Err(Malformed),
                    }
                }
            }
            Tok::Name("PUBLIC") => {
                self.expect_public_literal()?;
                self.expect_system_literal()?;
                if !is_param {
                    match self.dtd_token()? {
                        Tok::DeclClose => return self.finish_entity(predefined),
                        Tok::Name("NDATA") => {
                            if !matches!(self.dtd_token()?, Tok::Name(_)) {
                                return Err(Malformed);
                            }
                        }
                        _ => return Err(Malformed),
                    }
                }
            }
            _ => return Err(Malformed),
        }
        self.expect_decl_close()?;
        self.finish_entity(predefined)
    }

    fn finish_entity(&mut self, predefined: bool) -> Res {
        if predefined || !self.keep_processing {
            Ok(())
        } else {
            // entity_decl_handler: XML_StopParser
            Err(Malformed)
        }
    }

    /// `<!NOTATION name (SYSTEM literal | PUBLIC pubid [literal])>`.
    fn notation_decl(&mut self) -> Res {
        if !matches!(self.dtd_token()?, Tok::Name(_)) {
            return Err(Malformed);
        }
        match self.dtd_token()? {
            Tok::Name("SYSTEM") => {
                self.expect_system_literal()?;
                self.expect_decl_close()
            }
            Tok::Name("PUBLIC") => {
                self.expect_public_literal()?;
                match self.dtd_token()? {
                    Tok::DeclClose => Ok(()),
                    Tok::Literal(_) => self.expect_decl_close(),
                    _ => Err(Malformed),
                }
            }
            _ => Err(Malformed),
        }
    }

    // -----------------------------------------------------------------------------------------
    // The document

    /// The whole document: prolog, root element, epilog.
    fn document(&mut self, decl_len: usize) -> Res {
        self.pos = decl_len;

        // The prolog.
        let mut seen_doctype = false;
        loop {
            self.skip_space();
            if self.peek().is_none() {
                return Err(Malformed); // no element found
            }
            if self.starts_with("<!--") {
                self.comment()?;
            } else if self.starts_with("<?") {
                self.pi()?;
            } else if self.starts_with("<!DOCTYPE") && !seen_doctype {
                seen_doctype = true;
                self.doctype()?;
            } else {
                break;
            }
        }

        self.element()?;

        // The epilog.
        loop {
            self.skip_space();
            if self.peek().is_none() {
                return Ok(());
            }
            if self.starts_with("<!--") {
                self.comment()?;
            } else if self.starts_with("<?") {
                self.pi()?;
            } else {
                return Err(Malformed);
            }
        }
    }

    /// The root element and its content. At the `<` of its start tag.
    fn element(&mut self) -> Res {
        let mut stack: Vec<&'a str> = Vec::new();
        let mut text = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(Malformed); // unclosed token
            };
            if c != b'<' && stack.is_empty() {
                return Err(Malformed);
            }
            match c {
                b'<' => {
                    if self.starts_with("<!--") {
                        if stack.is_empty() {
                            return Err(Malformed);
                        }
                        self.comment()?;
                    } else if self.starts_with("<?") {
                        self.pi()?;
                    } else if self.starts_with("<![CDATA[") {
                        if stack.is_empty() {
                            return Err(Malformed);
                        }
                        self.pos += 9;
                        let start = self.pos;
                        self.skip_past("]]>")?;
                        push_normalized(&self.s[start..self.pos - 3], &mut text);
                    } else if self.starts_with("</") {
                        self.pos += 2;
                        let name = self.name()?;
                        self.skip_space();
                        if self.peek() != Some(b'>') {
                            return Err(Malformed);
                        }
                        self.pos += 1;
                        if stack.pop() != Some(name) {
                            return Err(Malformed);
                        }
                        flush_text(&mut text, self.handler);
                        self.handler.end_element(name);
                        if stack.is_empty() {
                            return Ok(());
                        }
                    } else if self.starts_with("<!") {
                        return Err(Malformed);
                    } else {
                        let (name, empty) = self.start_tag(&mut text)?;
                        if empty {
                            self.handler.end_element(name);
                            if stack.is_empty() {
                                return Ok(());
                            }
                        } else {
                            stack.push(name);
                        }
                    }
                }
                b'&' => {
                    if let Some(ch) = self.reference()? {
                        text.push(ch);
                    }
                }
                b'\r' => {
                    self.pos += 1;
                    if self.peek() == Some(b'\n') {
                        self.pos += 1;
                    }
                    text.push('\n');
                }
                b']' if self.starts_with("]]>") => return Err(Malformed),
                _ => {
                    let ch = self.peek_char().ok_or(Malformed)?;
                    self.pos += ch.len_utf8();
                    text.push(ch);
                }
            }
        }
    }

    /// A start tag, at the `<`: delivers the start element and its attributes. Returns the
    /// element name and whether the tag was empty (`<a/>`; the caller delivers its end).
    fn start_tag(&mut self, text: &mut String) -> Res<(&'a str, bool)> {
        self.pos += 1;
        let name = self.name()?;
        let mut attrs: Vec<(&'a str, String)> = Vec::new();
        let empty;
        loop {
            let had_space = self.skip_space();
            match self.peek() {
                Some(b'>') => {
                    self.pos += 1;
                    empty = false;
                    break;
                }
                Some(b'/') => {
                    self.pos += 1;
                    if self.peek() != Some(b'>') {
                        return Err(Malformed);
                    }
                    self.pos += 1;
                    empty = true;
                    break;
                }
                Some(_) if had_space => {
                    let attr_name = self.name()?;
                    self.skip_space();
                    if self.peek() != Some(b'=') {
                        return Err(Malformed);
                    }
                    self.pos += 1;
                    self.skip_space();
                    // a declared type other than CDATA normalizes the value
                    let is_cdata = self
                        .att_defs
                        .get(name)
                        .and_then(|defs| defs.iter().find(|d| d.name == attr_name))
                        .is_none_or(|d| d.is_cdata);
                    let value = self.attribute_value(is_cdata)?;
                    if attrs.iter().any(|(n, _)| *n == attr_name) {
                        return Err(Malformed);
                    }
                    attrs.push((attr_name, value));
                }
                _ => return Err(Malformed),
            }
        }
        // the declared defaults of the attributes the tag does not have
        let mut defaults: Vec<(String, String)> = Vec::new();
        if let Some(defs) = self.att_defs.get(name) {
            for d in defs {
                if let Some(v) = &d.value
                    && !attrs.iter().any(|(n, _)| *n == d.name)
                    && !defaults.iter().any(|(n, _)| *n == d.name)
                {
                    defaults.push((d.name.clone(), v.clone()));
                }
            }
        }
        flush_text(text, self.handler);
        self.handler.start_element(name);
        for (n, v) in &attrs {
            self.handler.add_attribute(n, v);
        }
        for (n, v) in &defaults {
            self.handler.add_attribute(n, v);
        }
        Ok((name, empty))
    }
}

/// The checks expat's `storeEntityValue` makes on an entity value: character references must be
/// well-formed and name an XML character, other references must be well-formed, and a parameter
/// entity reference is an error in the internal subset.
fn check_entity_value(value: &str, names: Names) -> Res {
    let b = value.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' => return Err(Malformed),
            b'&' => {
                let rest = &value[i + 1..];
                if let Some(num) = rest.strip_prefix('#') {
                    let (digits, radix, prefix) = match num.strip_prefix('x') {
                        Some(h) => (h, 16, 1),
                        None => (num, 10, 0),
                    };
                    let end = digits
                        .bytes()
                        .position(|c| !char::from(c).is_digit(radix))
                        .unwrap_or(digits.len());
                    if end == 0 || digits.as_bytes().get(end) != Some(&b';') {
                        return Err(Malformed);
                    }
                    let code = u32::from_str_radix(&digits[..end], radix).map_err(|_| Malformed)?;
                    if !is_xml_char(code) {
                        return Err(Malformed);
                    }
                    // '&', '#', the 'x', the digits and the ';'
                    i += 2 + prefix + end + 1;
                } else {
                    let mut chars = rest.chars();
                    let Some(first) = chars.next().filter(|&c| names.start(c)) else {
                        return Err(Malformed);
                    };
                    let mut len = first.len_utf8();
                    for c in chars {
                        if !names.name(c) {
                            break;
                        }
                        len += c.len_utf8();
                    }
                    if rest.as_bytes().get(len) != Some(&b';') {
                        return Err(Malformed);
                    }
                    i += 1 + len + 1;
                }
            }
            _ => i += 1,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

/// Whether `c` may be in a public identifier literal (expat's `isPublicId`).
fn is_public_id_char(c: u8) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            b' ' | b'\r'
                | b'\n'
                | b'-'
                | b'\''
                | b'('
                | b')'
                | b'+'
                | b','
                | b'.'
                | b'/'
                | b':'
                | b'='
                | b'?'
                | b';'
                | b'!'
                | b'*'
                | b'#'
                | b'@'
                | b'$'
                | b'_'
                | b'%'
        )
}

/// `\r\n` and `\r` become `\n`.
fn push_normalized(raw: &str, out: &mut String) {
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
}

/// Delivers the buffered character data (`ParsingContext::flushText`).
// Port of: src/xml/SkXMLParser.cpp#L82-L87 (chrome/m156)
fn flush_text(text: &mut String, handler: &mut dyn XmlParser) {
    if !text.is_empty() {
        handler.text(text);
        text.clear();
    }
}

/// Runs `handler` over the events of `doc`, like `SkXMLParser::parse` driving expat. Returns
/// whether the document parsed (events before an error have been delivered).
// Port of: src/xml/SkXMLParser.cpp#L124-L207 (chrome/m156) (expat's role)
pub(crate) fn parse(doc: &[u8], handler: &mut dyn XmlParser) -> bool {
    let Ok((decoded, standalone)) = decode(doc) else {
        return false;
    };
    let text = decoded.text.as_str();
    let decl_len = match parse_xml_decl(text.as_bytes()) {
        Ok(Some(d)) => d.len,
        Ok(None) => 0,
        Err(Malformed) => return false,
    };
    let mut scanner = Scanner {
        s: text,
        b: text.as_bytes(),
        pos: 0,
        handler,
        standalone,
        names: Names {
            latin1: decoded.latin1,
        },
        has_param_entity_refs: false,
        keep_processing: true,
        att_defs: HashMap::new(),
    };
    scanner.document(decl_len).is_ok() && !decoded.poisoned
}
