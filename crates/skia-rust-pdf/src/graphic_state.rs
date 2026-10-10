// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFGraphicState.{h,cpp} (chrome/m156)

//! `SkPDFGraphicState`: the graphic state dictionaries (`ExtGState`) that carry the alpha, blend
//! mode and stroke settings of a paint, canonicalized so each is written once per document.

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::paint::{Cap, Join, Paint, Style};

use crate::document::DocHandle;
use crate::types::{PdfArray, PdfDict, PdfIndirectReference};
use crate::utils::blend_mode_name;

/// `SkPDFGraphicState::SkPDFSMaskMode`.
// Port of: src/pdf/SkPDFGraphicState.h#L22-L25 (chrome/m156)
#[doc(alias = "SkPDFSMaskMode")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SMaskMode {
    /// `kAlpha_SMaskMode`.
    Alpha,
    /// `kLuminosity_SMaskMode`.
    Luminosity,
}

/// `SkPDFStrokeGraphicState`: the key of a stroke graphic state. Skia compares the bytes of the
/// struct, so the floats are compared by their bits.
// Port of: src/pdf/SkPDFGraphicState.h#L52-L65 (chrome/m156)
#[doc(alias = "SkPDFStrokeGraphicState")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StrokeGraphicState {
    stroke_width: u32,
    stroke_miter: u32,
    alpha: u32,
    stroke_cap: Cap,
    stroke_join: Join,
    blend_mode: BlendMode,
}

/// `SkPDFFillGraphicState`: the key of a fill graphic state.
// Port of: src/pdf/SkPDFGraphicState.h#L67-L77 (chrome/m156)
#[doc(alias = "SkPDFFillGraphicState")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FillGraphicState {
    alpha: u32,
    blend_mode: BlendMode,
}

// Port of: src/pdf/SkPDFGraphicState.cpp#L22-L26 (chrome/m156)
fn as_pdf_blend_mode_name(mode: BlendMode) -> &'static str {
    let name = blend_mode_name(mode);
    debug_assert!(name.is_some());
    name.unwrap_or("Normal")
}

// Port of: src/pdf/SkPDFGraphicState.cpp#L28-L37 (chrome/m156)
fn to_stroke_cap(cap: Cap) -> i32 {
    // PDF32000.book section 8.4.3.3 "Line Cap Style"
    match cap {
        Cap::Butt => 0,
        Cap::Round => 1,
        Cap::Square => 2,
    }
}

// Port of: src/pdf/SkPDFGraphicState.cpp#L39-L48 (chrome/m156)
fn to_stroke_join(join: Join) -> i32 {
    // PDF32000.book section 8.4.3.4 "Line Join Style"
    match join {
        Join::Miter => 0,
        Join::Round => 1,
        Join::Bevel => 2,
    }
}

/// If a `BlendMode` is unsupported in PDF, this function returns `SrcOver`, otherwise, it
/// returns the blend mode.
// Port of: src/pdf/SkPDFGraphicState.cpp#L50-L59 (chrome/m156)
fn pdf_blend_mode(mode: BlendMode) -> BlendMode {
    if blend_mode_name(mode).is_none() || BlendMode::Xor == mode || BlendMode::Plus == mode {
        return BlendMode::SrcOver;
    }
    mode
}

/// `SkPDFGraphicState::GetGraphicStateForPaint`: the graphic state for the passed paint.
// Port of: src/pdf/SkPDFGraphicState.cpp#L61-L103 (chrome/m156)
#[doc(alias = "SkPDFGraphicState::GetGraphicStateForPaint")]
#[must_use]
pub fn get_graphic_state_for_paint(doc: &DocHandle, p: &Paint) -> PdfIndirectReference {
    let mode = p.blend_mode_or(BlendMode::SrcOver);

    if Style::Fill == p.style() {
        let fill_key = FillGraphicState {
            alpha: p.color4f().a.to_bits(),
            blend_mode: pdf_blend_mode(mode),
        };
        if let Some(state_ref) = doc.with(|d| d.fill_gs_map.get(&fill_key).copied()) {
            return state_ref;
        }
        let mut state = PdfDict::new(None);
        state.reserve(2);
        state.insert_color_component_f("ca", f32::from_bits(fill_key.alpha));
        state.insert_name("BM", as_pdf_blend_mode_name(fill_key.blend_mode));
        let reference = doc.emit_new(&state);
        doc.with(|d| d.fill_gs_map.insert(fill_key, reference));
        reference
    } else {
        let stroke_key = StrokeGraphicState {
            stroke_width: p.stroke_width().to_bits(),
            stroke_miter: p.stroke_miter().to_bits(),
            alpha: p.color4f().a.to_bits(),
            stroke_cap: p.stroke_cap(),
            stroke_join: p.stroke_join(),
            blend_mode: pdf_blend_mode(mode),
        };
        if let Some(state_ref) = doc.with(|d| d.stroke_gs_map.get(&stroke_key).copied()) {
            return state_ref;
        }
        let mut state = PdfDict::new(None);
        state.reserve(8);
        state.insert_color_component_f("CA", f32::from_bits(stroke_key.alpha));
        state.insert_color_component_f("ca", f32::from_bits(stroke_key.alpha));
        state.insert_int("LC", to_stroke_cap(stroke_key.stroke_cap));
        state.insert_int("LJ", to_stroke_join(stroke_key.stroke_join));
        state.insert_scalar("LW", f32::from_bits(stroke_key.stroke_width));
        state.insert_scalar("ML", f32::from_bits(stroke_key.stroke_miter));
        state.insert_bool("SA", true); // SA = Auto stroke adjustment.
        state.insert_name("BM", as_pdf_blend_mode_name(stroke_key.blend_mode));
        let reference = doc.emit_new(&state);
        doc.with(|d| d.stroke_gs_map.insert(stroke_key, reference));
        reference
    }
}

////////////////////////////////////////////////////////////////////////////////

// Port of: src/pdf/SkPDFGraphicState.cpp#L107-L119 (chrome/m156)
fn make_invert_function(doc: &DocHandle) -> PdfIndirectReference {
    // Acrobat crashes if we use a type 0 function, kpdf crashes if we use
    // a type 2 function, so we use a type 4 function.
    const PS_INVERT: &[u8] = b"{1 exch sub}";

    let mut dict = PdfDict::new(None);
    dict.insert_int("FunctionType", 4);
    let mut domain = PdfArray::new();
    domain.append_int(0);
    domain.append_int(1);
    dict.insert_object("Domain", Box::new(domain));
    let mut range = PdfArray::new();
    range.append_int(0);
    range.append_int(1);
    dict.insert_object("Range", Box::new(range));
    doc.stream_out(Some(dict), PS_INVERT, true)
}

/// `SkPDFGraphicState::GetSMaskGraphicState`: a graphic state that only sets the passed soft
/// mask. `s_mask` is the form xobject to use as a soft mask, `invert` whether the alpha of it
/// should be inverted. These are not de-duped.
// Port of: src/pdf/SkPDFGraphicState.cpp#L121-L148 (chrome/m156)
#[doc(alias = "SkPDFGraphicState::GetSMaskGraphicState")]
#[must_use]
pub fn get_smask_graphic_state(
    s_mask: PdfIndirectReference,
    invert: bool,
    s_mask_mode: SMaskMode,
    doc: &DocHandle,
) -> PdfIndirectReference {
    // The practical chances of using the same mask more than once are unlikely
    // enough that it's not worth canonicalizing.
    let mut s_mask_dict = PdfDict::new(Some("Mask"));
    match s_mask_mode {
        SMaskMode::Alpha => s_mask_dict.insert_name("S", "Alpha"),
        SMaskMode::Luminosity => s_mask_dict.insert_name("S", "Luminosity"),
    }
    s_mask_dict.insert_ref("G", s_mask);
    if invert {
        // let the doc deduplicate this object.
        let existing = doc.with(|d| d.invert_function);
        let invert_function = if existing == PdfIndirectReference::default() {
            let made = make_invert_function(doc);
            doc.with(|d| d.invert_function = made);
            made
        } else {
            existing
        };
        s_mask_dict.insert_ref("TR", invert_function);
    }
    let mut result = PdfDict::new(Some("ExtGState"));
    result.insert_object("SMask", Box::new(s_mask_dict));
    doc.emit_new(&result)
}
