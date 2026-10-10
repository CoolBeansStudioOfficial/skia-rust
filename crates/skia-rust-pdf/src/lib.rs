// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/ and include/docs/SkPDFDocument.h (chrome/m156), the first wave of
// docs/design/modules.md section 7 (task M24).

//! Skia's PDF backend: the PDF object model, the content-stream utilities, the document skeleton
//! and the metadata.
//!
//! What is ported so far: `SkPDFUnion`, `SkPDFArray`, `SkPDFDict` and the string encoders
//! (`types`), `SkPDFUtils` with `SkFloatToDecimal`, `EmitPath` and `GetDateTime` (`utils`,
//! `float_to_decimal`), `SkDeflateWStream` (`deflate`), the document skeleton with its offset
//! map, cross-reference table, trailer and page tree (`document`), and the document metadata with
//! the XMP packet and the sRGB output intent of PDF/A (`metadata`, `srgb_icc`).
//!
//! The page drawing (`SkPDFDevice`, `beginPage`/`endPage`), the fonts, the JPEG helpers, the
//! structure tree and the annotations follow in later waves (`docs/design/modules.md`, M25 and
//! M26).

pub mod date_time;
pub mod deflate;
pub mod document;
pub mod float_to_decimal;
pub mod metadata;
mod srgb_icc;
pub mod types;
pub mod utils;

pub use date_time::DateTime;
pub use document::{Document, new_document};
pub use float_to_decimal::{MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH, float_to_decimal};
pub use metadata::{CompressionLevel, Metadata, Outline};
pub use types::{
    PdfArray, PdfDict, PdfIndirectReference, PdfObject, PdfOptionalArray, PdfParentTreeKey,
    PdfUnion,
};
