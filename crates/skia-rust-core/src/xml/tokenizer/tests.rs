// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

use super::*;

#[derive(Default)]
struct Log(Vec<String>);

impl XmlParser for Log {
    fn on_start_element(&mut self, e: &str) -> bool {
        self.0.push(format!("<{e}"));
        false
    }
    fn on_add_attribute(&mut self, n: &str, v: &str) -> bool {
        self.0.push(format!("{n}={v}"));
        false
    }
    fn on_end_element(&mut self, e: &str) -> bool {
        self.0.push(format!("</{e}"));
        false
    }
    fn on_text(&mut self, t: &str) -> bool {
        self.0.push(format!("'{t}'"));
        false
    }
}

fn events(doc: &[u8]) -> (bool, Vec<String>) {
    let mut l = Log::default();
    let ok = parse(doc, &mut l);
    (ok, l.0)
}

#[test]
fn events_and_text_buffering() {
    let (ok, e) = events(
        b"<?xml version='1.0'?><!-- c --><a x='1\n2' y=\"&lt;&#65;\">t&amp;<b/>u<![CDATA[<>]]>\r\nv</a>\n",
    );
    assert!(ok);
    assert_eq!(
        e,
        [
            "<a", "x=1 2", "y=<A", "'t&'", "<b", "</b", "'u<>\nv'", "</a"
        ]
    );
}

#[test]
fn events_before_an_error_are_delivered() {
    // the text before the error is buffered and never delivered, as in SkXMLParser
    let (ok, e) = events(b"<a><b>text</c></a>");
    assert!(!ok);
    assert_eq!(e, ["<a", "<b"]);
    let (ok, e) = events(b"<a>x\xff</a>");
    assert!(!ok);
    assert_eq!(e, ["<a"]);
}

#[test]
fn entity_declarations_stop_the_parse() {
    assert!(!events(b"<!DOCTYPE a [<!ENTITY e 'x'>]><a>&e;</a>").0);
    // predefined entities may be redeclared, which expat ignores
    assert!(events(b"<!DOCTYPE a [<!ENTITY lt 'x'>]><a/>").0);
    // undefined entities are errors unless a DTD might declare them
    assert!(!events(b"<a>&nbsp;</a>").0);
    assert!(events(b"<!DOCTYPE a SYSTEM 'a.dtd'><a>&nbsp;</a>").0);
}

#[test]
fn attlist_defaults_and_types() {
    let (ok, e) =
        events(b"<!DOCTYPE a [<!ATTLIST a x CDATA 'd' n NMTOKENS #IMPLIED>]><a n='  p   q '/>");
    assert!(ok);
    assert_eq!(e, ["<a", "n=p q", "x=d", "</a"]);
}

#[test]
fn encodings() {
    // UTF-16 with a BOM, ISO-8859-1 by declaration
    let utf16: Vec<u8> = [0xFEFF_u16]
        .into_iter()
        .chain("<a>\u{e9}</a>".encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    assert_eq!(
        events(&utf16),
        (true, vec!["<a".into(), "'\u{e9}'".into(), "</a".into()])
    );
    let latin1 = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><a>\xe9</a>";
    assert_eq!(events(latin1).1[1], "'\u{e9}'");
    // encodings expat does not have built in, and a UTF-16 declaration in UTF-8 text
    assert!(!events(b"<?xml version=\"1.0\" encoding=\"windows-1252\"?><a/>").0);
    assert!(!events(b"<?xml version=\"1.0\" encoding=\"UTF-16\"?><a/>").0);
}

#[test]
fn malformed() {
    for d in [
        &b""[..],
        b"<a>",
        b"<a></b>",
        b"<a/><b/>",
        b"<a x=1/>",
        b"<a x='1' x='2'/>",
        b"<a>&foo;</a>",
        b"<a",
        b"x<a/>",
        b"<a><</a>",
        b"<a><!-- a -- b --></a>",
        b"<?xml?><a/>",
        b"<a>\x01</a>",
        b"<a>\xef\xbf\xbe</a>",
        b"<\xc3\x97/>",
    ] {
        assert!(!events(d).0, "{}", String::from_utf8_lossy(d));
    }
}
