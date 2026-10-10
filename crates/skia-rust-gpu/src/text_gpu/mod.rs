//! Ports of `src/text/gpu/*` (`sktext::gpu`): how text is turned into GPU work. The glyphs of a
//! draw are split into sub runs (direct masks, transformed masks, distance field text, paths and
//! drawables) by the [`sub_run_container`]; the atlas sub runs are drawn through the device's
//! atlas delegate.

pub mod distance_field_adjust_table;
pub mod glyph_vector;
pub mod packed_gpu_glyph_id;
pub mod sdf_mask_filter;
pub mod slug_impl;
pub mod strike_cache;
pub mod sub_run_container;
pub mod sub_run_control;
pub mod text_blob;
pub mod text_blob_redraw_coordinator;
pub mod vertex_filler;

use skia_rust_core::mask::MaskFormat as GlyphMaskFormat;

use crate::gpu::mask_format::MaskFormat;

/// `FormatFromSkGlyph(format)`: converts a glyph's `SkMask::Format` to the `skgpu::MaskFormat` of
/// the atlas that holds it.
// Port of: src/text/gpu/GlyphUtils.h#L17-L36 (chrome/m156)
#[doc(alias = "FormatFromSkGlyph")]
#[must_use]
pub fn format_from_glyph(format: GlyphMaskFormat) -> MaskFormat {
    match format {
        // We store BW and SDF glyphs in our 8-bit cache, and ignore the mul and add planes of 3D
        // glyphs, just using the mask.
        GlyphMaskFormat::BW
        | GlyphMaskFormat::Sdf
        | GlyphMaskFormat::A8
        | GlyphMaskFormat::ThreeD => MaskFormat::A8,
        GlyphMaskFormat::Lcd16 => MaskFormat::A565,
        GlyphMaskFormat::Argb32 => MaskFormat::Argb,
    }
}
