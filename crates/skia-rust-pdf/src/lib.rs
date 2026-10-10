// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/ and include/docs/SkPDFDocument.h (chrome/m156), the first two waves of
// docs/design/modules.md section 7 (tasks M24 to M26).

//! Skia's PDF backend: the PDF object model, the content-stream utilities, the document, the
//! device that draws a page, the shaders, the images, the structure tree and the annotations.
//!
//! What is ported: `SkPDFUnion`, `SkPDFArray`, `SkPDFDict` and the string encoders (`types`),
//! `SkPDFUtils` with `SkFloatToDecimal`, `EmitPath` and `GetDateTime` (`utils`,
//! `float_to_decimal`), `SkDeflateWStream` (`deflate`), the document with its offset map,
//! cross-reference table, trailer, page tree, pages, links and named destinations (`document`),
//! the document metadata with the XMP packet and the sRGB output intent of PDF/A (`metadata`,
//! `srgb_icc`), `SkPDFDevice` with its graphic stack, graphic states, resource dictionaries and
//! form `XObjects` (`device`, `graphic_stack_state`, `graphic_state`, `resource_dict`,
//! `form_xobject`), the gradient and image shaders (`gradient_shader`, `shader`), the images
//! with their JPEG pass-through (`bitmap`, `keyed_image`, `jpeg`), `SkClusterator`
//! (`clusterator`), the structure tree (`tag`), and the fonts (M26): `SkPDFFont` with its Type3
//! and Type0/CID fonts, the `ToUnicode` maps, the CID glyph widths, Type1 fonts, and the text
//! of the device (`font`, `glyph_use`, `to_unicode_cmap`, `cid_glyph_widths`, `type1_font`).
//!
//! Fonts are embedded whole: the subsetter is not ported (`docs/design/modules.md` Q4), and
//! `subset_font` is the seam where it will go. It is the branch Skia takes when it is built
//! without `SK_PDF_USE_HARFBUZZ_SUBSET`.

pub mod bitmap;
pub mod cid_glyph_widths;
pub mod clip_stack_device;
pub mod clusterator;
pub mod date_time;
pub mod deflate;
pub mod device;
pub mod document;
pub mod float_to_decimal;
pub mod font;
pub mod form_xobject;
pub mod glyph_use;
pub mod gradient_shader;
pub mod graphic_stack_state;
pub mod graphic_state;
pub mod jpeg;
pub mod keyed_image;
pub mod metadata;
pub mod resource_dict;
pub mod shader;
mod srgb_icc;
pub mod subset_font;
pub mod tag;
pub mod to_unicode_cmap;
pub mod type1_font;
pub mod types;
pub mod utils;

pub use date_time::DateTime;
pub use document::{Document, new_document, set_node_id};
pub use float_to_decimal::{MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH, float_to_decimal};
pub use metadata::{CompressionLevel, Metadata, Outline};
pub use types::{
    PdfArray, PdfDict, PdfIndirectReference, PdfObject, PdfOptionalArray, PdfParentTreeKey,
    PdfUnion,
};
