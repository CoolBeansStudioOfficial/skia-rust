// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/text/GlyphData.h, src/gpu/graphite/text/GlyphData.cpp
// (chrome/m156)

//! [`GlyphData`]: the Graphite backend data of a glyph vector: the text strike its glyphs are in,
//! their atlas entries, and the atlas generation they were last made current for.

use std::sync::Arc;

use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::strike_spec::BulkGlyphMetricsAndImages;

use crate::gpu::sk_log::skia_log_e;
use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::draw_atlas::{BulkUsePlotUpdater, ErrorCode, GenerationCounter};
use crate::graphite::draw_writer::{DrawWriter, Instances};
use crate::graphite::recorder::Recorder;
use crate::graphite::text::text_strike::{GlyphEntry, TextStrike};
use crate::text_gpu::glyph_vector::RendererData;
use crate::text_gpu::packed_gpu_glyph_id::PackedGpuGlyphId;
use crate::text_gpu::vertex_filler::VertexFiller;

/// A glyph of a sub run: its entry in the strike (`skgpu::graphite::Glyph`).
// Port of: src/gpu/graphite/text/GlyphData.h#L39-L58 (chrome/m156)
#[doc(alias = "skgpu::graphite::Glyph")]
#[derive(Clone, Debug)]
pub struct Glyph {
    /// `fEntry`.
    entry: Arc<GlyphEntry>,
}

impl Glyph {
    /// `Glyph(entry)`.
    #[must_use]
    pub fn new(entry: Arc<GlyphEntry>) -> Self {
        Self { entry }
    }

    /// `packedID()`.
    #[must_use]
    pub fn packed_id(&self) -> PackedGlyphId {
        self.entry.key().packed_glyph_id()
    }

    /// `entry()`.
    #[must_use]
    pub fn entry(&self) -> &GlyphEntry {
        &self.entry
    }
}

/// The backend data of a glyph vector (`skgpu::graphite::GlyphData`).
// Port of: src/gpu/graphite/text/GlyphData.h#L60-L96 (chrome/m156)
#[doc(alias = "skgpu::graphite::GlyphData")]
#[derive(Debug)]
pub struct GlyphData {
    /// `fTextStrike`.
    text_strike: Arc<TextStrike>,
    /// `fAtlasGeneration`.
    atlas_generation: u64,
    /// `fBulkUseUpdater`.
    bulk_use_updater: BulkUsePlotUpdater,
    /// `fRenderData`.
    render_data: RendererData,
    /// The glyphs converted from the packed ids (`accessBackendGlyphs<Glyph>()`).
    glyphs: Vec<Glyph>,
}

impl GlyphData {
    /// `GlyphData(strike, recorder, renderData)` together with the conversion of each packed id
    /// by `makeGlyphFromID`.
    // Port of: src/gpu/graphite/text/GlyphData.cpp#L28-L45 (chrome/m156)
    #[must_use]
    pub fn new(
        text_strike: Arc<TextStrike>,
        recorder: &Recorder,
        render_data: RendererData,
        packed_ids: &[PackedGlyphId],
    ) -> Self {
        let render_data = recorder
            .priv_()
            .atlas_provider()
            .borrow()
            .text_atlas_manager()
            .resolve_renderer_data(render_data);
        let glyphs = packed_ids
            .iter()
            .map(|id| Self::make_glyph_from_id(&text_strike, render_data, *id))
            .collect();
        Self {
            text_strike,
            atlas_generation: GenerationCounter::INVALID_GENERATION,
            bulk_use_updater: BulkUsePlotUpdater::default(),
            render_data,
            glyphs,
        }
    }

    /// `makeGlyphFromID(id)`.
    // Port of: src/gpu/graphite/text/GlyphData.cpp#L47-L53 (chrome/m156)
    fn make_glyph_from_id(
        text_strike: &TextStrike,
        render_data: RendererData,
        id: PackedGlyphId,
    ) -> Glyph {
        let gpu_id = PackedGpuGlyphId::new(
            id,
            render_data.mask_format,
            render_data.src_padding,
            render_data.is_sdf,
        );
        Glyph::new(text_strike.get_glyph(gpu_id))
    }

    /// `rendererData()`.
    // Port of: src/gpu/graphite/text/GlyphData.h#L72 (chrome/m156)
    #[must_use]
    pub fn renderer_data(&self) -> RendererData {
        self.render_data
    }

    /// `accessBackendGlyphs<Glyph>()`.
    #[must_use]
    pub fn glyphs(&self) -> &[Glyph] {
        &self.glyphs
    }

    /// Regenerates the atlas entries for the glyphs in the range `[begin, end)`. Returns
    /// `(success, glyphs_placed_in_atlas)`.
    // Port of: src/gpu/graphite/text/GlyphData.cpp#L55-L132 (chrome/m156)
    pub fn regenerate_atlas(
        &mut self,
        begin: usize,
        end: usize,
        recorder: &Recorder,
    ) -> (bool, usize) {
        let priv_ = recorder.priv_();
        let mut atlas_provider = priv_.atlas_provider().borrow_mut();
        let atlas_manager = atlas_provider.text_atlas_manager_mut();

        // TODO: this is not a great place for this -- need a better way to init atlases when
        // needed.
        if atlas_manager
            .get_proxies(self.render_data.mask_format)
            .is_none()
        {
            skia_log_e!("Could not allocate backing texture for atlas");
            return (false, 0);
        }

        let current_atlas_gen = atlas_manager.atlas_generation(self.render_data.mask_format);

        #[allow(clippy::if_not_else)] // keeps the C++ branch order
        if self.atlas_generation != current_atlas_gen {
            // Calculate the texture coordinates for the vertexes during first use
            // (fAtlasGeneration is set to kInvalidAtlasGeneration) or the atlas has changed in
            // subsequent calls. Always reset the update, even when begin > 0. When begin > 0,
            // this subrun was split across flushes and previously used glyphs in [0, begin) have
            // an older token. If those glyphs are not used in [begin, end], they shouldn't be
            // part of the next bulk update (and because we won't have updated the whole subrun
            // in one go, we won't set `fAtlasGeneration` to take the fast path on reuse). If we
            // do reuse the glyphs, we need the bulk update to have been reset so that the call
            // to addGlyphToBulkAndSetUseToken() sees the first use with the new token and
            // updates the atlas locator.
            self.bulk_use_updater.reset();

            let metrics_and_images = BulkGlyphMetricsAndImages::new(self.text_strike.strike_spec());

            // Update the atlas information in the GrStrike.
            let mut glyphs_placed_in_atlas = 0;
            let mut success = true;
            for glyph in &self.glyphs[begin..end] {
                debug_assert_eq!(
                    glyph.entry().key().mask_format(),
                    self.render_data.mask_format
                );
                debug_assert_eq!(glyph.entry().key().padding(), self.render_data.src_padding);
                debug_assert_eq!(glyph.entry().key().is_sdf(), self.render_data.is_sdf);
                if !atlas_manager.has_glyph(glyph.entry()) {
                    let sk_glyph = metrics_and_images.glyph(glyph.packed_id());
                    let code = atlas_manager.add_glyph_to_atlas(recorder, &sk_glyph, glyph.entry());
                    if code != ErrorCode::Succeeded {
                        success = code != ErrorCode::Error;
                        break;
                    }
                }
                let token = priv_.token_tracker().borrow().next_flush_token();
                atlas_manager.add_glyph_to_bulk_and_set_use_token(
                    &mut self.bulk_use_updater,
                    glyph.entry(),
                    token,
                );
                glyphs_placed_in_atlas += 1;
            }

            // Update atlas generation if there are no more glyphs to put in the atlas. We can
            // only do this if we successfully checked/added the entire glyph vector in one pass.
            // Otherwise, on a partial update, some of the previous glyphs of the subrun that were
            // already drawn could have been evicted to make room for these remaining glyphs.
            if success && begin == 0 && glyphs_placed_in_atlas == self.glyphs.len() {
                // Need to get the freshest value of the atlas' generation because
                // updateTextureCoordinates may have changed it.
                self.atlas_generation =
                    atlas_manager.atlas_generation(self.render_data.mask_format);
            }

            (success, glyphs_placed_in_atlas)
        } else {
            // The atlas hasn't changed, so our texture coordinates are still valid.
            if end == self.glyphs.len() {
                // The atlas hasn't changed and the texture coordinates are all still valid.
                // Update all the plots used to the new use token.
                let token = priv_.token_tracker().borrow().next_flush_token();
                atlas_manager.set_use_token_bulk(
                    &self.bulk_use_updater,
                    token,
                    self.render_data.mask_format,
                );
            }
            (true, end - begin)
        }
    }

    /// Writes the instances of the glyphs `[offset, offset + count)`: the size, the atlas
    /// position, the glyph's top left, the atlas page and the mask format flags, the scale from
    /// the strike to the source (always 1), the depth and the SSBO index.
    // Port of: src/gpu/graphite/text/GlyphData.cpp#L139-L172 (chrome/m156)
    // The instance data narrows the atlas coordinates to 16 bits, as the C++ `uint16_t(...)`.
    #[allow(clippy::cast_possible_truncation, clippy::too_many_arguments)]
    pub fn fill_instance_data(
        &self,
        vf: &VertexFiller,
        dw: &mut DrawWriter<'_>,
        offset: usize,
        count: usize,
        flags: u16,
        ssbo_index: u32,
        depth: f32,
    ) {
        let mut instances =
            Instances::new(dw, BindBufferInfo::default(), BindBufferInfo::default(), 4);
        instances.reserve(count as u32);
        // Need to send width, height, uvPos, xyPos, and strikeToSourceScale
        // pre-transform coords = (s*w*b_x + t_x, s*h*b_y + t_y)
        // where (b_x, b_y) are the vertexID coords
        for (glyph, left_top) in self.glyphs[offset..offset + count]
            .iter()
            .zip(&vf.top_lefts()[offset..offset + count])
        {
            let [al, at, ar, ab] = glyph.entry().atlas_locator().uvs();
            let mut writer = instances.append(1);
            writer
                .put(&[ar.wrapping_sub(al), ab.wrapping_sub(at)])
                .put(&[al & 0x1fff, at])
                .put(&[left_top.x, left_top.y])
                .put(&[al >> 13, flags])
                .put(&1.0f32)
                .put(&depth)
                .put(&ssbo_index);
        }
    }
}
