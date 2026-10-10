// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/ and include/docs/SkPDFDocument.h (chrome/m156), the first wave of
// docs/design/modules.md section 7 (task M24).

//! Skia's PDF backend: the PDF object model, the content-stream utilities, and the document
//! skeleton.
//!
//! What is ported so far: `SkPDFUnion`, `SkPDFArray`, `SkPDFDict` and the string encoders
//! (`types`), `SkPDFUtils` with `SkFloatToDecimal`, `EmitPath` and `GetDateTime` (`utils`,
//! `float_to_decimal`), and `SkPDF::DateTime` (`date_time`). The device, the fonts, the JPEG
//! helpers and the page drawing follow in later waves (`docs/design/modules.md` M25 and M26).

pub mod date_time;
pub mod deflate;
pub mod float_to_decimal;
pub mod types;
pub mod utils;

pub use date_time::DateTime;
pub use float_to_decimal::{MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH, float_to_decimal};
pub use types::{
    PdfArray, PdfDict, PdfIndirectReference, PdfObject, PdfOptionalArray, PdfParentTreeKey,
    PdfUnion,
};
