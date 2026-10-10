// Copyright 2010 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFTypes.{h,cpp}, src/pdf/SkPDFUnion.h (chrome/m156)

//! PDF object types: the non-virtual `SkPDFUnion` for scalar objects, and the `SkPDFObject`
//! family (arrays, dictionaries, indirect references) with their exact serialization.
//!
//! Strings are byte slices, as in Skia. A `const char*` argument of Skia is passed here as its
//! bytes, and an `SkString` as its bytes too: the two differ only for embedded NUL bytes, which
//! the C-string paths cut at the first NUL (`name_escaped` does that itself).

use skia_rust_core::stream::WStream;
use skia_rust_core::utf::next_utf8;

use crate::utils;

/// Uppercase hex digits (`SkHexadecimalDigits::gUpper`).
pub(crate) const HEX_DIGITS_UPPER: &[u8; 16] = b"0123456789ABCDEF";

/// `SkPDFIndirectReference`: a reference to an indirect object. A negative value is "none".
// Port of: src/pdf/SkPDFTypes.h#L30-L37 (chrome/m156)
#[doc(alias = "SkPDFIndirectReference")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PdfIndirectReference {
    /// The object number. Negative for "no reference".
    pub value: i32,
}

impl Default for PdfIndirectReference {
    fn default() -> Self {
        Self { value: -1 }
    }
}

impl PdfIndirectReference {
    /// `explicit operator bool`: whether this names an object.
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.value >= 0
    }
}

/// `SkPDFParentTreeKey`: a key of the structure parent tree. A negative value is "none".
// Port of: src/pdf/SkPDFTypes.h#L39-L42 (chrome/m156)
#[doc(alias = "SkPDFParentTreeKey")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PdfParentTreeKey {
    /// The key. Negative for "none".
    pub value: i32,
}

impl Default for PdfParentTreeKey {
    fn default() -> Self {
        Self { value: -1 }
    }
}

impl PdfParentTreeKey {
    /// `explicit operator bool`: whether this is a key.
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.value >= 0
    }
}

/// `SkPDFObject`: a PDF object that can print itself.
// Port of: src/pdf/SkPDFTypes.h#L50-L78 (chrome/m156)
#[doc(alias = "SkPDFObject")]
pub trait PdfObject: std::fmt::Debug {
    /// Prints the object to `stream`.
    fn emit_object(&self, stream: &mut dyn WStream);
}

/// `SkPDFUnion`: a non-virtualized implementation of the non-compound PDF object types (names,
/// strings, numbers, booleans), plus the references and boxed objects that arrays and
/// dictionaries hold.
// Port of: src/pdf/SkPDFUnion.h#L19-L108, src/pdf/SkPDFTypes.cpp#L1-L330 (chrome/m156)
#[doc(alias = "SkPDFUnion")]
#[derive(Debug)]
pub enum PdfUnion {
    /// `int32_t`.
    Int(i32),
    /// A colour component in `0..=255`, written in the shortest decimal form (see
    /// [`utils::color_to_decimal`]).
    ColorComponent(u8),
    /// A colour component in `0..=1`.
    ColorComponentF(f32),
    /// `bool`.
    Bool(bool),
    /// A scalar, written by [`utils::append_scalar`].
    Scalar(f32),
    /// A name written as it is. The caller promises it is a valid name (no whitespace,
    /// control characters or delimiters).
    Name(Vec<u8>),
    /// A name, escaped with `#xx` as `SkPDFUnion::Name(SkString)` does. Cut at the first NUL.
    NameEscaped(Vec<u8>),
    /// A byte string, `(...)` or `<...>`.
    ByteString(Vec<u8>),
    /// A text string, `PDFDocEncoding` or UTF-16BE.
    TextString(Vec<u8>),
    /// An object owned by this value.
    Object(Box<dyn PdfObject>),
    /// An indirect reference, `N 0 R`.
    Ref(PdfIndirectReference),
}

impl PdfUnion {
    /// `SkPDFUnion::Int`.
    #[must_use]
    pub fn int(value: i32) -> Self {
        Self::Int(value)
    }

    /// `SkPDFUnion::ColorComponent`.
    #[must_use]
    pub fn color_component(value: u8) -> Self {
        Self::ColorComponent(value)
    }

    /// `SkPDFUnion::ColorComponentF`.
    #[must_use]
    pub fn color_component_f(value: f32) -> Self {
        Self::ColorComponentF(value)
    }

    /// `SkPDFUnion::Bool`.
    #[must_use]
    pub fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    /// `SkPDFUnion::Scalar`.
    #[must_use]
    pub fn scalar(value: f32) -> Self {
        Self::Scalar(value)
    }

    /// `SkPDFUnion::Name(const char*)`: `value` must already be a valid name.
    // Port of: src/pdf/SkPDFTypes.cpp#L330-L336 (chrome/m156)
    #[must_use]
    pub fn name(value: &str) -> Self {
        debug_assert!(is_valid_name(value.as_bytes()));
        Self::Name(c_string_bytes(value.as_bytes()).to_vec())
    }

    /// `SkPDFUnion::Name(SkString)`: escapes `value`, which need not be a valid name.
    // Port of: src/pdf/SkPDFTypes.cpp#L346-L349 (chrome/m156)
    #[must_use]
    pub fn name_escaped(value: impl AsRef<[u8]>) -> Self {
        Self::NameEscaped(c_string_bytes(value.as_ref()).to_vec())
    }

    /// `SkPDFUnion::ByteString`.
    // Port of: src/pdf/SkPDFTypes.cpp#L338-L343 (chrome/m156)
    #[must_use]
    pub fn byte_string(value: impl AsRef<[u8]>) -> Self {
        Self::ByteString(value.as_ref().to_vec())
    }

    /// `SkPDFUnion::TextString`.
    // Port of: src/pdf/SkPDFTypes.cpp#L338-L343 (chrome/m156)
    #[must_use]
    pub fn text_string(value: impl AsRef<[u8]>) -> Self {
        Self::TextString(value.as_ref().to_vec())
    }

    /// `SkPDFUnion::Object`.
    #[must_use]
    pub fn object(value: Box<dyn PdfObject>) -> Self {
        Self::Object(value)
    }

    /// `SkPDFUnion::Ref`. The reference must name an object (`value > 0`).
    // Port of: src/pdf/SkPDFTypes.cpp#L358-L361 (chrome/m156)
    #[must_use]
    pub fn reference(value: PdfIndirectReference) -> Self {
        debug_assert!(value.value > 0);
        Self::Ref(value)
    }

    /// `SkPDFUnion::isName`.
    #[must_use]
    pub fn is_name(&self) -> bool {
        matches!(self, Self::Name(_) | Self::NameEscaped(_))
    }

    /// `SkPDFUnion::emitObject`.
    // Port of: src/pdf/SkPDFTypes.cpp#L256-L318 (chrome/m156)
    pub fn emit_object(&self, stream: &mut dyn WStream) {
        match self {
            Self::Int(v) => {
                stream.write_dec_as_text(*v);
            }
            Self::ColorComponent(v) => utils::append_color_component(*v, stream),
            Self::ColorComponentF(v) => utils::append_color_component_f(*v, stream),
            Self::Bool(v) => {
                stream.write_text(if *v { "true" } else { "false" });
            }
            Self::Scalar(v) => utils::append_scalar(*v, stream),
            Self::Name(name) => {
                stream.write_text("/");
                stream.write(name);
            }
            Self::NameEscaped(name) => {
                stream.write_text("/");
                write_name_escaped(stream, name);
            }
            Self::ByteString(s) => write_byte_string(stream, s),
            Self::TextString(s) => write_text_string(stream, s),
            Self::Object(o) => o.emit_object(stream),
            Self::Ref(r) => {
                debug_assert!(r.value >= 0);
                stream.write_dec_as_text(r.value);
                stream.write_text(" 0 R"); // Generation number is always 0.
            }
        }
    }
}

/// `is_valid_name` (`SK_DEBUG` only in Skia): printable ASCII, and no delimiter.
// Port of: src/pdf/SkPDFTypes.cpp#L203-L213 (chrome/m156)
fn is_valid_name(n: &[u8]) -> bool {
    const CONTROL_CHARS: &[u8] = b"/%()<>[]{}";
    let c_name = c_string_bytes(n);
    c_name
        .iter()
        .all(|&c| (b'!'..=b'~').contains(&c) && !CONTROL_CHARS.contains(&c))
}

/// The bytes up to the first NUL, as a C string would see them.
fn c_string_bytes(s: &[u8]) -> &[u8] {
    match s.iter().position(|&c| c == 0) {
        Some(end) => &s[..end],
        None => s,
    }
}

/// `write_name_escaped`.
// Port of: src/pdf/SkPDFTypes.cpp#L215-L230 (chrome/m156)
fn write_name_escaped(stream: &mut dyn WStream, name: &[u8]) {
    const TO_ESCAPE: &[u8] = b"#/%()<>[]{}";
    for &v in c_string_bytes(name) {
        if !(b'!'..=b'~').contains(&v) || TO_ESCAPE.contains(&v) {
            let buffer = [
                b'#',
                HEX_DIGITS_UPPER[usize::from(v >> 4)],
                HEX_DIGITS_UPPER[usize::from(v & 0xF)],
            ];
            stream.write(&buffer);
        } else {
            stream.write(&[v]);
        }
    }
}

/// `write_literal_byte_string`.
// Port of: src/pdf/SkPDFTypes.cpp#L232-L252 (chrome/m156)
fn write_literal_byte_string(stream: &mut dyn WStream, cin: &[u8]) {
    stream.write_text("(");
    for &c in cin {
        if (b' '..=b'~').contains(&c) {
            if c == b'\\' || c == b'(' || c == b')' {
                stream.write_text("\\");
            }
            stream.write(&[c]);
        } else {
            let octal = [
                b'\\',
                b'0' | (c >> 6),
                b'0' | ((c >> 3) & 0x07),
                b'0' | (c & 0x07),
            ];
            stream.write(&octal);
        }
    }
    stream.write_text(")");
}

/// `write_hex_byte_string`.
// Port of: src/pdf/SkPDFTypes.cpp#L254-L266 (chrome/m156)
fn write_hex_byte_string(stream: &mut dyn WStream, cin: &[u8]) {
    stream.write_text("<");
    for &c in cin {
        let hex = [
            HEX_DIGITS_UPPER[usize::from(c >> 4)],
            HEX_DIGITS_UPPER[usize::from(c & 0xF)],
        ];
        stream.write(&hex);
    }
    stream.write_text(">");
}

/// `write_optimized_byte_string`: the literal form when it is no longer than the hex form.
// Port of: src/pdf/SkPDFTypes.cpp#L268-L278 (chrome/m156)
fn write_optimized_byte_string(stream: &mut dyn WStream, cin: &[u8], literal_extras: usize) {
    let hex_length = 2 + 2 * cin.len();
    let literal_length = 2 + cin.len() + literal_extras;
    if literal_length <= hex_length {
        write_literal_byte_string(stream, cin);
    } else {
        write_hex_byte_string(stream, cin);
    }
}

/// `write_byte_string`.
// Port of: src/pdf/SkPDFTypes.cpp#L280-L295 (chrome/m156)
fn write_byte_string(stream: &mut dyn WStream, cin: &[u8]) {
    debug_assert!(cin.len() <= 65535);
    let mut literal_extras = 0;
    for &c in cin {
        if !(b' '..=b'~').contains(&c) {
            literal_extras += 3;
        } else if c == b'\\' || c == b'(' || c == b')' {
            literal_extras += 1;
        }
    }
    write_optimized_byte_string(stream, cin, literal_extras);
}

/// `write_text_string`: `PDFDocEncoding` when every character allows it, UTF-16BE otherwise.
// Port of: src/pdf/SkPDFTypes.cpp#L297-L340 (chrome/m156)
fn write_text_string(stream: &mut dyn WStream, cin: &[u8]) {
    debug_assert!(cin.len() <= 65535);
    let mut input_is_valid_utf8 = true;
    let mut input_is_pdf_doc_encoding = true;
    let mut literal_extras = 0;
    {
        let mut text = cin;
        while !text.is_empty() {
            let unichar = next_utf8(&mut text);
            if unichar < 0 {
                input_is_valid_utf8 = false;
                break;
            }
            if (0x15 < unichar && unichar < 0x20) || 0x7E < unichar {
                input_is_pdf_doc_encoding = false;
                break;
            }
            if unichar < i32::from(b' ') || i32::from(b'~') < unichar {
                literal_extras += 3;
            } else if unichar == i32::from(b'\\')
                || unichar == i32::from(b'(')
                || unichar == i32::from(b')')
            {
                literal_extras += 1;
            }
        }
    }
    if !input_is_valid_utf8 {
        stream.write_text("<>");
        return;
    }
    if input_is_pdf_doc_encoding {
        write_optimized_byte_string(stream, cin, literal_extras);
        return;
    }
    stream.write_text("<FEFF");
    let mut text = cin;
    while !text.is_empty() {
        let unichar = next_utf8(&mut text);
        utils::write_utf16be_hex(stream, unichar);
    }
    stream.write_text(">");
}

/// `SkPDFWriteTextString`, exposed for unit testing.
// Port of: src/pdf/SkPDFTypes.cpp#L342-L345 (chrome/m156)
pub fn write_text_string_to(stream: &mut dyn WStream, cin: &[u8]) {
    write_text_string(stream, cin);
}

/// `SkPDFWriteByteString`, exposed for unit testing.
// Port of: src/pdf/SkPDFTypes.cpp#L347-L350 (chrome/m156)
pub fn write_byte_string_to(stream: &mut dyn WStream, cin: &[u8]) {
    write_byte_string(stream, cin);
}

/// `SkPDFArray`: an array object in a PDF.
// Port of: src/pdf/SkPDFTypes.h#L80-L125, src/pdf/SkPDFTypes.cpp#L354-L400 (chrome/m156)
#[doc(alias = "SkPDFArray")]
#[derive(Debug, Default)]
pub struct PdfArray {
    values: Vec<PdfUnion>,
}

impl PdfArray {
    /// Creates an empty array.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of entries.
    #[must_use]
    pub fn size(&self) -> usize {
        self.values.len()
    }

    /// Preallocates space for `length` entries.
    pub fn reserve(&mut self, length: usize) {
        self.values.reserve(length);
    }

    /// The entries (`SkPDFArray::values`, protected in Skia).
    #[must_use]
    pub fn values(&self) -> &[PdfUnion] {
        &self.values
    }

    fn append(&mut self, value: PdfUnion) {
        self.values.push(value);
    }

    /// `appendInt`.
    pub fn append_int(&mut self, value: i32) {
        self.append(PdfUnion::int(value));
    }

    /// `appendColorComponent`.
    pub fn append_color_component(&mut self, value: u8) {
        self.append(PdfUnion::color_component(value));
    }

    /// `appendColorComponentF`.
    pub fn append_color_component_f(&mut self, value: f32) {
        self.append(PdfUnion::color_component_f(value));
    }

    /// `appendBool`.
    pub fn append_bool(&mut self, value: bool) {
        self.append(PdfUnion::bool(value));
    }

    /// `appendScalar`.
    pub fn append_scalar(&mut self, value: f32) {
        self.append(PdfUnion::scalar(value));
    }

    /// `appendName(const char[])`.
    pub fn append_name(&mut self, name: &str) {
        self.append(PdfUnion::name(name));
    }

    /// `appendName(SkString)`: escaped.
    pub fn append_name_escaped(&mut self, name: impl AsRef<[u8]>) {
        self.append(PdfUnion::name_escaped(name));
    }

    /// `appendByteString`.
    pub fn append_byte_string(&mut self, value: impl AsRef<[u8]>) {
        self.append(PdfUnion::byte_string(value));
    }

    /// `appendTextString`.
    pub fn append_text_string(&mut self, value: impl AsRef<[u8]>) {
        self.append(PdfUnion::text_string(value));
    }

    /// `appendObject`.
    pub fn append_object(&mut self, value: Box<dyn PdfObject>) {
        self.append(PdfUnion::object(value));
    }

    /// `appendRef`.
    pub fn append_ref(&mut self, value: PdfIndirectReference) {
        self.append(PdfUnion::reference(value));
    }
}

impl PdfObject for PdfArray {
    // Port of: src/pdf/SkPDFTypes.cpp#L402-L414 (chrome/m156)
    fn emit_object(&self, stream: &mut dyn WStream) {
        stream.write_text("[");
        for (i, v) in self.values.iter().enumerate() {
            v.emit_object(stream);
            if i + 1 < self.values.len() {
                stream.write_text(" ");
            }
        }
        stream.write_text("]");
    }
}

/// `SkPDFOptionalArray`: an array that is emitted as its single entry when it has one.
// Port of: src/pdf/SkPDFTypes.h#L127-L134, src/pdf/SkPDFTypes.cpp#L416-L422 (chrome/m156)
#[doc(alias = "SkPDFOptionalArray")]
#[derive(Debug, Default)]
pub struct PdfOptionalArray {
    array: PdfArray,
}

impl PdfOptionalArray {
    /// The wrapped array, to append to.
    pub fn array_mut(&mut self) -> &mut PdfArray {
        &mut self.array
    }
}

impl PdfObject for PdfOptionalArray {
    fn emit_object(&self, stream: &mut dyn WStream) {
        if self.array.size() == 1 {
            self.array.values()[0].emit_object(stream);
        } else {
            self.array.emit_object(stream);
        }
    }
}

/// `SkPDFDict`: a dictionary object in a PDF.
// Port of: src/pdf/SkPDFTypes.h#L136-L186, src/pdf/SkPDFTypes.cpp#L424-L520 (chrome/m156)
#[doc(alias = "SkPDFDict")]
#[derive(Debug, Default)]
pub struct PdfDict {
    records: Vec<(PdfUnion, PdfUnion)>,
}

impl PdfDict {
    /// Creates a dictionary. With `Some(type)`, the first entry is `/Type /type`.
    // Port of: src/pdf/SkPDFTypes.cpp#L424-L430 (chrome/m156)
    #[must_use]
    pub fn new(type_name: Option<&str>) -> Self {
        let mut dict = Self::default();
        if let Some(t) = type_name {
            dict.insert_name("Type", t);
        }
        dict
    }

    /// The number of entries.
    #[must_use]
    pub fn size(&self) -> usize {
        self.records.len()
    }

    /// Preallocates space for `n` key-value pairs.
    pub fn reserve(&mut self, n: usize) {
        self.records.reserve(n);
    }

    fn insert(&mut self, key: &str, value: PdfUnion) {
        self.records.push((PdfUnion::name(key), value));
    }

    /// `insertRef`.
    pub fn insert_ref(&mut self, key: &str, value: PdfIndirectReference) {
        self.insert(key, PdfUnion::reference(value));
    }

    /// `insertRef(SkString, ...)`: the key is escaped.
    pub fn insert_ref_escaped_key(
        &mut self,
        key: impl AsRef<[u8]>,
        value: PdfIndirectReference,
    ) {
        self.records
            .push((PdfUnion::name_escaped(key), PdfUnion::reference(value)));
    }

    /// `insertObject(SkString, ...)`: the key is escaped.
    pub fn insert_object_escaped_key(&mut self, key: impl AsRef<[u8]>, value: Box<dyn PdfObject>) {
        self.records
            .push((PdfUnion::name_escaped(key), PdfUnion::object(value)));
    }

    /// `insertObject`.
    pub fn insert_object(&mut self, key: &str, value: Box<dyn PdfObject>) {
        self.insert(key, PdfUnion::object(value));
    }

    /// `insertBool`.
    pub fn insert_bool(&mut self, key: &str, value: bool) {
        self.insert(key, PdfUnion::bool(value));
    }

    /// `insertInt`.
    pub fn insert_int(&mut self, key: &str, value: i32) {
        self.insert(key, PdfUnion::int(value));
    }

    /// `insertInt(const char[], size_t)`: the value is narrowed to `i32` (`SkToS32`).
    pub fn insert_int_usize(&mut self, key: &str, value: usize) {
        self.insert_int(key, i32::try_from(value).unwrap_or(i32::MAX));
    }

    /// `insertScalar`.
    pub fn insert_scalar(&mut self, key: &str, value: f32) {
        self.insert(key, PdfUnion::scalar(value));
    }

    /// `insertColorComponentF`.
    pub fn insert_color_component_f(&mut self, key: &str, value: f32) {
        self.insert(key, PdfUnion::color_component_f(value));
    }

    /// `insertName(const char[], const char[])`.
    pub fn insert_name(&mut self, key: &str, name_value: &str) {
        self.insert(key, PdfUnion::name(name_value));
    }

    /// `insertName(const char[], SkString)`: the value is escaped.
    pub fn insert_name_escaped(&mut self, key: &str, name_value: impl AsRef<[u8]>) {
        self.insert(key, PdfUnion::name_escaped(name_value));
    }

    /// `insertByteString`.
    pub fn insert_byte_string(&mut self, key: &str, value: impl AsRef<[u8]>) {
        self.insert(key, PdfUnion::byte_string(value));
    }

    /// `insertTextString`.
    pub fn insert_text_string(&mut self, key: &str, value: impl AsRef<[u8]>) {
        self.insert(key, PdfUnion::text_string(value));
    }

    /// `insertUnion`.
    pub fn insert_union(&mut self, key: &str, value: PdfUnion) {
        self.insert(key, value);
    }
}

impl PdfObject for PdfDict {
    // Port of: src/pdf/SkPDFTypes.cpp#L446-L461 (chrome/m156)
    fn emit_object(&self, stream: &mut dyn WStream) {
        stream.write_text("<<");
        for (i, (key, value)) in self.records.iter().enumerate() {
            key.emit_object(stream);
            stream.write_text(" ");
            value.emit_object(stream);
            if i + 1 < self.records.len() {
                stream.write_text("\n");
            }
        }
        stream.write_text(">>");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_escape_only_when_asked() {
        let mut out = skia_rust_core::stream::DynamicMemoryWStream::new();
        PdfUnion::name("A").emit_object(&mut out);
        assert_eq!(out.detach_as_vector(), b"/A");
    }
}
