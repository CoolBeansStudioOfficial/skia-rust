// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/xml/SkXMLParser.h, src/xml/SkDOM.h, src/xml/SkXMLWriter.h

//! `src/xml`: the XML parser callbacks, the DOM and the XML writer.
//!
//! Skia parses with expat. Here a small safe tokenizer (`parser::tokenize`) produces the same
//! element, attribute and text events (docs/design/codecs.md Q4).

pub mod dom;
pub mod parser;
pub mod writer;

pub use dom::{Attr, AttrIter, Dom, DomParser, Node, NodeType};
pub use parser::{XmlParser, XmlParserError, XmlParserErrorCode};
pub use writer::{XmlParserWriter, XmlStreamWriter, XmlWriter, XmlWriterBase, K_NO_PRETTY_FLAG};
