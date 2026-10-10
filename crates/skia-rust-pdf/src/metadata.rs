// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/docs/SkPDFDocument.h#L93-L200 (the `Metadata` struct), src/pdf/SkPDFMetadata.{h,cpp}
// (chrome/m156)

//! The document metadata (`SkPDF::Metadata`) and `SkPDFMetadata`: the information dictionary,
//! the document and instance UUIDs, the `/ID` array and the XMP packet of PDF/A documents.

use std::fmt::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

use skia_rust_codec::codec::Codec;
use skia_rust_core::data::Data;
use skia_rust_core::md5::Md5;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::stream::WStream;

use crate::date_time::DateTime;
use crate::tag::StructureElementNode;
use crate::types::{PdfArray, PdfDict};

/// `SK_MILESTONE`, the milestone of the Skia this crate ports.
const SK_MILESTONE: u32 = 156;

/// `SkPDF::Metadata::CompressionLevel`: the compression of stream objects.
// Port of: include/docs/SkPDFDocument.h#L194-L200 (chrome/m156)
#[doc(alias = "SkPDF::Metadata::CompressionLevel")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum CompressionLevel {
    /// zlib's default.
    #[default]
    Default = -1,
    /// Stored, not compressed.
    None = 0,
    /// Fastest.
    LowButFast = 1,
    /// zlib's level 6.
    Average = 6,
    /// Smallest.
    HighButSlow = 9,
}

/// `SkPDF::Metadata::Outline`: the outline (bookmarks) of a tagged document.
// Port of: include/docs/SkPDFDocument.h#L163-L168 (chrome/m156)
#[doc(alias = "SkPDF::Metadata::Outline")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Outline {
    /// No outline.
    #[default]
    None = 0,
    /// Headers of the structure tree.
    StructureElementHeaders = 1,
    /// Every structure element.
    StructureElements = 2,
}

/// `SkPDF::DecodeJpegCallback`: decodes a JPEG, so that Skia can embed its bytes as they are.
// Port of: include/docs/SkPDFDocument.h#L90 (chrome/m156)
#[doc(alias = "SkPDF::DecodeJpegCallback")]
pub type DecodeJpegCallback = fn(Data) -> Option<Codec<'static>>;

/// `SkPDF::EncodeJpegCallback`: encodes `src` as a JPEG of the given quality into `dst`.
// Port of: include/docs/SkPDFDocument.h#L91 (chrome/m156)
#[doc(alias = "SkPDF::EncodeJpegCallback")]
pub type EncodeJpegCallback = fn(dst: &mut dyn WStream, src: &Pixmap<'_>, quality: i32) -> bool;

/// `SkPDF::Metadata`: optional metadata for the PDF document.
///
/// Not ported: `fSubsetter` (`HarfBuzz` is the only one).
// Port of: include/docs/SkPDFDocument.h#L93-L240 (chrome/m156)
#[doc(alias = "SkPDF::Metadata")]
#[derive(Clone)]
pub struct Metadata {
    /// The document's title.
    pub title: String,
    /// The name of the person who created the document.
    pub author: String,
    /// The subject of the document.
    pub subject: String,
    /// Keywords associated with the document, separated by commas.
    pub keywords: String,
    /// The name of the product that created the original document.
    pub creator: String,
    /// The name of the product that converted the original document to PDF.
    pub producer: String,
    /// The creation date. The zero value is "unknown".
    pub creation: DateTime,
    /// The modification date. The zero value is "unknown".
    pub modified: DateTime,
    /// The natural language of the text in the PDF.
    pub lang: String,
    /// The DPI at which features are rasterized (`SK_ScalarDefaultRasterDPI` by default).
    pub raster_dpi: f32,
    /// Produces a PDF/A document (with XMP metadata and an sRGB output intent).
    pub pdfa: bool,
    /// The JPEG quality for opaque images, 0..=100, or 101 for lossless.
    pub encoding_quality: i32,
    /// Rasterizes alpha gradients when printing.
    pub rasterize_alpha_gradients_for_printing: bool,
    /// The compression of stream objects.
    pub compression_level: CompressionLevel,
    /// The outline (bookmarks) of a tagged document.
    pub outline: Outline,
    /// An optional tree of structured document tags that provide a semantic representation of
    /// the content (`fStructureElementTreeRoot`). The document keeps a copy.
    pub structure_element_tree_root: Option<StructureElementNode>,
    /// A way to decode JPEGs (`jpegDecoder`); `jpeg::decode` is Skia's.
    pub jpeg_decoder: Option<DecodeJpegCallback>,
    /// A way to encode JPEGs (`jpegEncoder`); `jpeg::encode` is Skia's.
    pub jpeg_encoder: Option<EncodeJpegCallback>,
    /// Allows a document that can't embed JPEGs, to avoid an assert (`allowNoJpegs`).
    pub allow_no_jpegs: bool,
    /// Executor to handle threaded work within PDF Backend (`fExecutor`). Experimental.
    ///
    /// skia-rust: the document is not `Send` (its devices are shared through `Rc`), so the work
    /// that Skia hands to the executor (serializing images, deflating streams) is done on the
    /// calling thread, in order, which is what a document without an executor does. The output is
    /// the reproducible one, and `abort` has no job to wait for.
    pub executor: Option<std::sync::Arc<dyn skia_rust_core::executor::Executor>>,
}

impl std::fmt::Debug for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Metadata")
            .field("title", &self.title)
            .field("author", &self.author)
            .field("subject", &self.subject)
            .field("keywords", &self.keywords)
            .field("creator", &self.creator)
            .field("producer", &self.producer)
            .field("creation", &self.creation)
            .field("modified", &self.modified)
            .field("lang", &self.lang)
            .field("raster_dpi", &self.raster_dpi)
            .field("pdfa", &self.pdfa)
            .field("encoding_quality", &self.encoding_quality)
            .field(
                "rasterize_alpha_gradients_for_printing",
                &self.rasterize_alpha_gradients_for_printing,
            )
            .field("compression_level", &self.compression_level)
            .field("outline", &self.outline)
            .field(
                "structure_element_tree_root",
                &self.structure_element_tree_root,
            )
            .field("has_jpeg_decoder", &self.jpeg_decoder.is_some())
            .field("has_jpeg_encoder", &self.jpeg_encoder.is_some())
            .field("allow_no_jpegs", &self.allow_no_jpegs)
            .field("has_executor", &self.executor.is_some())
            .finish()
    }
}

impl Default for Metadata {
    // Port of: include/docs/SkPDFDocument.h#L97-L200 (default member values, chrome/m156)
    fn default() -> Self {
        Self {
            title: String::new(),
            author: String::new(),
            subject: String::new(),
            keywords: String::new(),
            creator: String::new(),
            producer: format!("Skia/PDF m{SK_MILESTONE}"),
            creation: DateTime::default(),
            modified: DateTime::default(),
            lang: String::new(),
            raster_dpi: 72.0,
            pdfa: false,
            encoding_quality: 101,
            rasterize_alpha_gradients_for_printing: false,
            compression_level: CompressionLevel::Default,
            outline: Outline::None,
            structure_element_tree_root: None,
            jpeg_decoder: None,
            jpeg_encoder: None,
            allow_no_jpegs: false,
            executor: None,
        }
    }
}

/// `SkPDF::DateTime` equal to `kZeroTime`: the unknown time.
// Port of: src/pdf/SkPDFMetadata.cpp#L17 (chrome/m156)
const K_ZERO_TIME: DateTime = DateTime {
    time_zone_minutes: 0,
    year: 0,
    month: 0,
    day_of_week: 0,
    day: 0,
    hour: 0,
    minute: 0,
    second: 0,
};

/// `SkUUID`: a 128-bit identifier.
// Port of: src/pdf/SkUUID.h (chrome/m156)
#[doc(alias = "SkUUID")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Uuid {
    /// The 16 bytes.
    pub data: [u8; 16],
}

/// `pdf_date`: a PDF date string, `D:YYYYMMDDhhmmssZZ'mm'`.
// Port of: src/pdf/SkPDFMetadata.cpp#L28-L40 (chrome/m156)
fn pdf_date(dt: &DateTime) -> String {
    let time_zone_minutes = i32::from(dt.time_zone_minutes);
    let timezone_sign = if time_zone_minutes >= 0 { '+' } else { '-' };
    let time_zone_hours = time_zone_minutes.abs() / 60;
    let time_zone_minutes = time_zone_minutes.abs() % 60;
    let mut out = String::new();
    let _ = write!(
        out,
        "D:{:04}{:02}{:02}{:02}{:02}{:02}{}{:02}'{:02}'",
        dt.year,
        dt.month,
        dt.day,
        dt.hour,
        dt.minute,
        dt.second,
        timezone_sign,
        time_zone_hours,
        time_zone_minutes
    );
    out
}

/// The text keys of the information dictionary and of the UUID, with their metadata field.
// Port of: src/pdf/SkPDFMetadata.cpp#L42-L55 (chrome/m156)
fn metadata_keys(metadata: &Metadata) -> [(&'static str, &str); 6] {
    [
        ("Title", &metadata.title),
        ("Author", &metadata.author),
        ("Subject", &metadata.subject),
        ("Keywords", &metadata.keywords),
        ("Creator", &metadata.creator),
        ("Producer", &metadata.producer),
    ]
}

/// `SkPDFMetadata::MakeDocumentInformationDict`.
// Port of: src/pdf/SkPDFMetadata.cpp#L57-L72 (chrome/m156)
#[doc(alias = "SkPDFMetadata::MakeDocumentInformationDict")]
#[must_use]
pub fn make_document_information_dict(metadata: &Metadata) -> PdfDict {
    let mut dict = PdfDict::new(None);
    for (key, value) in metadata_keys(metadata) {
        if !value.is_empty() {
            dict.insert_text_string(key, value);
        }
    }
    if metadata.creation != K_ZERO_TIME {
        dict.insert_text_string("CreationDate", pdf_date(&metadata.creation));
    }
    if metadata.modified != K_ZERO_TIME {
        dict.insert_text_string("ModDate", pdf_date(&metadata.modified));
    }
    dict
}

/// The native-endian bytes of a `SkPDF::DateTime`, laid out as the C++ struct (10 bytes).
fn date_time_bytes(dt: &DateTime) -> [u8; 10] {
    let mut out = [0u8; 10];
    out[0..2].copy_from_slice(&dt.time_zone_minutes.to_ne_bytes());
    out[2..4].copy_from_slice(&dt.year.to_ne_bytes());
    out[4] = dt.month;
    out[5] = dt.day_of_week;
    out[6] = dt.day;
    out[7] = dt.hour;
    out[8] = dt.minute;
    out[9] = dt.second;
    out
}

/// The current time in milliseconds, as `SkTime::GetMSecs()` gives it (any monotone-ish clock
/// serves here: the UUID only needs to be unique).
fn msecs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

/// `SkPDFMetadata::CreateUUID`: a UUID from the time, the date and the metadata.
// Port of: src/pdf/SkPDFMetadata.cpp#L74-L101 (chrome/m156)
#[doc(alias = "SkPDFMetadata::CreateUUID")]
#[must_use]
pub fn create_uuid(metadata: &Metadata) -> Uuid {
    // The main requirement is for the UUID to be unique; the exact format of the data that will
    // be hashed is not important.
    let mut md5 = Md5::new();
    md5.write_bytes(b"org.skia.pdf\n");
    md5.write_bytes(&msecs().to_ne_bytes());
    let date_time = crate::utils::get_date_time();
    md5.write_bytes(&date_time_bytes(&date_time));
    md5.write_bytes(&date_time_bytes(&metadata.creation));
    md5.write_bytes(&date_time_bytes(&metadata.modified));
    for (key, value) in metadata_keys(metadata) {
        md5.write_bytes(key.as_bytes());
        md5.write_bytes(b"\x1f");
        md5.write_bytes(value.as_bytes());
        md5.write_bytes(b"\x1e");
    }
    let mut digest = md5.finish().data;
    // See RFC 4122, pages 6-7.
    digest[6] = (digest[6] & 0x0F) | 0x30;
    // The C++ reads `digest.data[6]` here, the byte modified just above, not `data[8]`. That is
    // Skia's behaviour, and the port keeps it.
    digest[8] = (digest[6] & 0x3F) | 0x80;
    Uuid { data: digest }
}

/// `SkPDFMetadata::MakePdfId`: the `/ID` array, `[<doc> <instance>]` as byte strings.
// Port of: src/pdf/SkPDFMetadata.cpp#L103-L111 (chrome/m156)
#[doc(alias = "SkPDFMetadata::MakePdfId")]
#[must_use]
pub fn make_pdf_id(doc: Uuid, instance: Uuid) -> PdfArray {
    let mut array = PdfArray::new();
    array.append_byte_string(doc.data);
    array.append_byte_string(instance.data);
    array
}

/// `hexify`: `count` bytes of `input` as lowercase hex.
// Port of: src/pdf/SkPDFMetadata.cpp#L124-L133 (chrome/m156)
fn hexify(out: &mut String, input: &[u8]) {
    const LOWER: &[u8; 16] = b"0123456789abcdef";
    for &value in input {
        out.push(char::from(LOWER[usize::from(value >> 4)]));
        out.push(char::from(LOWER[usize::from(value & 0xF)]));
    }
}

/// `uuid_to_string`: `8-4-4-4-12` lowercase hex.
// Port of: src/pdf/SkPDFMetadata.cpp#L135-L152 (chrome/m156)
#[must_use]
pub fn uuid_to_string(uuid: &Uuid) -> String {
    let mut out = String::with_capacity(36);
    let data = &uuid.data;
    hexify(&mut out, &data[0..4]);
    out.push('-');
    hexify(&mut out, &data[4..6]);
    out.push('-');
    hexify(&mut out, &data[6..8]);
    out.push('-');
    hexify(&mut out, &data[8..10]);
    out.push('-');
    hexify(&mut out, &data[10..16]);
    out
}

/// `count_xml_escape_size`.
// Port of: src/pdf/SkPDFMetadata.cpp#L175-L186 (chrome/m156)
fn count_xml_escape_size(input: &str) -> usize {
    let mut extra = 0;
    for c in input.bytes() {
        if c == b'&' {
            extra += 4; // strlen("&amp;") - strlen("&")
        } else if c == b'<' {
            extra += 3; // strlen("&lt;") - strlen("<")
        }
    }
    extra
}

/// `escape_xml`: `&` becomes `&amp;` and `<` becomes `&lt;`, with optional text before and after.
/// The text is UTF-8 and is xml content, not an attribute value.
// Port of: src/pdf/SkPDFMetadata.cpp#L188-L226 (chrome/m156)
fn escape_xml(input: &str, before: &str, after: &str) -> String {
    if input.is_empty() {
        return String::new();
    }
    let mut output = String::with_capacity(input.len() + before.len() + after.len());
    output.push_str(before);
    for c in input.chars() {
        match c {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            _ => output.push(c),
        }
    }
    output.push_str(after);
    output
}

/// The XMP packet of a PDF/A document, from `MakeXMPObject`'s template.
// Port of: src/pdf/SkPDFMetadata.cpp#L228-L329 (chrome/m156)
#[must_use]
pub fn xmp_packet(metadata: &Metadata, doc: Uuid, instance: Uuid) -> String {
    let mut creation_date = String::new();
    let mut modification_date = String::new();
    if metadata.creation != K_ZERO_TIME {
        // YYYY-mm-ddTHH:MM:SS[+|-]ZZ:ZZ; no need to escape
        let tmp = metadata.creation.to_iso8601();
        debug_assert_eq!(count_xml_escape_size(&tmp), 0);
        creation_date = format!("<xmp:CreateDate>{tmp}</xmp:CreateDate>\n");
    }
    if metadata.modified != K_ZERO_TIME {
        let tmp = metadata.modified.to_iso8601();
        debug_assert_eq!(count_xml_escape_size(&tmp), 0);
        modification_date = format!("<xmp:ModifyDate>{tmp}</xmp:ModifyDate>\n");
    }
    let title = escape_xml(
        &metadata.title,
        "<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">",
        "</rdf:li></rdf:Alt></dc:title>\n",
    );
    let author = escape_xml(
        &metadata.author,
        "<dc:creator><rdf:Seq><rdf:li>",
        "</rdf:li></rdf:Seq></dc:creator>\n",
    );
    // TODO in Skia: in theory, XMP can support multiple authors. Split on a delimiter?
    let subject = escape_xml(
        &metadata.subject,
        "<dc:description><rdf:Alt><rdf:li xml:lang=\"x-default\">",
        "</rdf:li></rdf:Alt></dc:description>\n",
    );
    let keywords1 = escape_xml(
        &metadata.keywords,
        "<dc:subject><rdf:Bag><rdf:li>",
        "</rdf:li></rdf:Bag></dc:subject>\n",
    );
    let keywords2 = escape_xml(&metadata.keywords, "<pdf:Keywords>", "</pdf:Keywords>\n");
    // TODO in Skia: in theory, keywords can be a list too.
    let producer = escape_xml(&metadata.producer, "<pdf:Producer>", "</pdf:Producer>\n");
    let creator = escape_xml(
        &metadata.creator,
        "<xmp:CreatorTool>",
        "</xmp:CreatorTool>\n",
    );
    let document_id = uuid_to_string(&doc); // no need to escape
    let instance_id = uuid_to_string(&instance);

    // The template keeps every space of the C++ string, so the packet is built piece by piece.
    let mut out = String::new();
    out.push_str("<?xpacket begin=\"\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n");
    out.push_str("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"\n");
    out.push_str(" x:xmptk=\"Adobe XMP Core 5.4-c005 78.147326, 2012/08/23-13:03:03\">\n");
    out.push_str("<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n");
    out.push_str("<rdf:Description rdf:about=\"\"\n");
    out.push_str(" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n");
    out.push_str(" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n");
    out.push_str(" xmlns:xmpMM=\"http://ns.adobe.com/xap/1.0/mm/\"\n");
    out.push_str(" xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\"\n");
    out.push_str(" xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\">\n");
    out.push_str("<pdfaid:part>2</pdfaid:part>\n");
    out.push_str("<pdfaid:conformance>B</pdfaid:conformance>\n");
    out.push_str(&modification_date);
    out.push_str(&creation_date);
    out.push_str(&creator);
    out.push_str("<dc:format>application/pdf</dc:format>\n");
    out.push_str(&title);
    out.push_str(&subject);
    out.push_str(&author);
    out.push_str(&keywords1);
    let _ = writeln!(
        out,
        "<xmpMM:DocumentID>uuid:{document_id}</xmpMM:DocumentID>"
    );
    let _ = writeln!(
        out,
        "<xmpMM:InstanceID>uuid:{instance_id}</xmpMM:InstanceID>"
    );
    out.push_str(&producer);
    out.push_str(&keywords2);
    out.push_str("</rdf:Description>\n");
    out.push_str("</rdf:RDF>\n");
    out.push_str("</x:xmpmeta>\n"); // Note: the standard suggests 4k of padding.
    out.push_str("<?xpacket end=\"w\"?>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_format() {
        let uuid = Uuid {
            data: [
                0x81, 0xb1, 0x4a, 0xaf, 0xa3, 0x13, 0xdb, 0x63, 0xdb, 0xd6, 0xf9, 0x81, 0xe4, 0x9f,
                0x94, 0xf4,
            ],
        };
        assert_eq!(
            uuid_to_string(&uuid),
            "81b14aaf-a313-db63-dbd6-f981e49f94f4"
        );
    }

    #[test]
    fn pdf_date_format() {
        let dt = DateTime {
            time_zone_minutes: 0,
            year: 1999,
            month: 12,
            day_of_week: 5,
            day: 31,
            hour: 23,
            minute: 59,
            second: 59,
        };
        assert_eq!(pdf_date(&dt), "D:19991231235959+00'00'");
    }
}
