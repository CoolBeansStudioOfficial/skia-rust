// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/xml/SkXMLParser.h, src/xml/SkDOM.h, src/xml/SkXMLWriter.h

//! `src/xml`: the XML parser callbacks, the DOM and the XML writer.
//!
//! Skia parses with expat. Here a safe tokenizer (`tokenizer`) produces the same element,
//! attribute and text events and rejects the documents expat rejects (docs/design/codecs.md Q4,
//! and docs/API_MAPPING.md for what is left out).

pub mod dom;
mod name_tables;
pub mod parser;
mod tokenizer;
pub mod writer;

pub use dom::{Attr, AttrIter, Dom, DomParser, Node, NodeType};
pub use parser::{XmlParser, XmlParserError, XmlParserErrorCode};
pub use writer::{K_NO_PRETTY_FLAG, XmlParserWriter, XmlStreamWriter, XmlWriter, XmlWriterBase};
