// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/SubRunData.h

//! [`SubRunData`]: the geometry of a text draw: a range of the glyphs of an atlas sub run, and
//! what the text render steps need to draw them.

use std::rc::Weak;
use std::sync::Arc;

use skia_rust_core::color::Color;
use skia_rust_core::m44::M44;
use skia_rust_core::surface_props::PixelGeometry;

use crate::gpu::mask_format::MaskFormat;
use crate::graphite::geom::rect::Rect;
use crate::graphite::recorder::{Recorder, RecorderInner};
use crate::graphite::texture_proxy::TextureProxy;
use crate::text_gpu::glyph_vector::RendererData;
use crate::text_gpu::sub_run_container::AtlasSubRun;

/// A range of glyphs of an atlas sub run (`skgpu::graphite::SubRunData`).
///
/// skia-rust: the sub run is shared (`Arc`), which keeps the text blob or slug it is in alive as
/// long as the geometry is (`fSupportDataKeepAlive`). The recorder is a `Weak` handle: this sub
/// run can only be associated with that recorder's atlas.
// Port of: src/gpu/graphite/geom/SubRunData.h#L26-L96 (chrome/m156)
#[doc(alias = "skgpu::graphite::SubRunData")]
#[derive(Clone, Debug)]
pub struct SubRunData {
    /// `fSubRun`.
    sub_run: Arc<AtlasSubRun>,
    /// `fBounds`: bounds of the data stored in the sub run, in mask (texture) space.
    bounds: Rect,
    /// `fMaskToDevice`.
    mask_to_device: M44,
    /// `fStartGlyphIndex`.
    start_glyph_index: usize,
    /// `fGlyphCount`.
    glyph_count: usize,
    /// `fLuminanceColor`: only used by `SDFTextRenderStep`.
    luminance_color: Color,
    /// `fUseGammaCorrectDistanceTable`: only used by `SDFTextRenderStep`.
    use_gamma_correct_distance_table: bool,
    /// `fPixelGeometry`: only used by `SDFTextLCDRenderStep`.
    pixel_geometry: PixelGeometry,
    /// `fRecorder`.
    recorder: Weak<RecorderInner>,
}

impl SubRunData {
    /// `SubRunData(subRun, supportDataKeepAlive, maskBounds, maskToDevice, startGlyphIndex,
    /// glyphCount, luminanceColor, useGammaCorrectDistanceTable, pixelGeometry, recorder)`.
    // Port of: src/gpu/graphite/geom/SubRunData.h#L32-L52 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ constructor
    #[must_use]
    pub fn new(
        sub_run: Arc<AtlasSubRun>,
        mask_bounds: Rect,
        mask_to_device: M44,
        start_glyph_index: usize,
        glyph_count: usize,
        luminance_color: Color,
        use_gamma_correct_distance_table: bool,
        pixel_geometry: PixelGeometry,
        recorder: &Recorder,
    ) -> Self {
        Self {
            sub_run,
            bounds: mask_bounds,
            mask_to_device,
            start_glyph_index,
            glyph_count,
            luminance_color,
            use_gamma_correct_distance_table,
            pixel_geometry,
            recorder: recorder.downgrade(),
        }
    }

    /// `bounds()`: the bounding box of the originating sub run in mask (texture) space.
    // Port of: src/gpu/graphite/geom/SubRunData.h#L60 (chrome/m156)
    #[must_use]
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// `maskToDevice()`: the transform from the mask texture to device coordinates.
    // Port of: src/gpu/graphite/geom/SubRunData.h#L63 (chrome/m156)
    #[must_use]
    pub fn mask_to_device(&self) -> &M44 {
        &self.mask_to_device
    }

    /// `subRun()`.
    // Port of: src/gpu/graphite/geom/SubRunData.h#L66 (chrome/m156)
    #[must_use]
    pub fn sub_run(&self) -> &Arc<AtlasSubRun> {
        &self.sub_run
    }

    /// `startGlyphIndex()`.
    // Port of: src/gpu/graphite/geom/SubRunData.h#L67 (chrome/m156)
    #[must_use]
    pub fn start_glyph_index(&self) -> usize {
        self.start_glyph_index
    }

    /// `glyphCount()`.
    // Port of: src/gpu/graphite/geom/SubRunData.h#L68 (chrome/m156)
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.glyph_count
    }

    /// `luminanceColor()`.
    // Port of: src/gpu/graphite/geom/SubRunData.h#L69 (chrome/m156)
    #[must_use]
    pub fn luminance_color(&self) -> Color {
        self.luminance_color
    }

    /// `useGammaCorrectDistanceTable()`.
    // Port of: src/gpu/graphite/geom/SubRunData.h#L70 (chrome/m156)
    #[must_use]
    pub fn use_gamma_correct_distance_table(&self) -> bool {
        self.use_gamma_correct_distance_table
    }

    /// `pixelGeometry()`.
    // Port of: src/gpu/graphite/geom/SubRunData.h#L71 (chrome/m156)
    #[must_use]
    pub fn pixel_geometry(&self) -> PixelGeometry {
        self.pixel_geometry
    }

    /// `recorder()`: the recorder the sub run was prepared on, if it is still alive.
    // Port of: src/gpu/graphite/geom/SubRunData.h#L72 (chrome/m156)
    #[must_use]
    pub fn recorder(&self) -> Option<Recorder> {
        self.recorder.upgrade().map(Recorder::from_inner)
    }

    /// `rendererData()`: how the glyphs are drawn, resolved by the atlas manager when the
    /// backend data of the sub run was made.
    ///
    /// # Panics
    /// If the sub run has no backend data yet.
    // Port of: src/gpu/graphite/geom/SubRunData.h#L80-L84 (chrome/m156)
    #[must_use]
    pub fn renderer_data(&self) -> RendererData {
        self.sub_run
            .glyph_vector()
            .backend()
            .as_ref()
            .expect("the sub run has backend data")
            .renderer_data()
    }

    /// The textures of the active pages of the text atlas the glyphs are in, as the render steps
    /// get them (`recorder->priv().atlasProvider()->textAtlasManager()->getProxies(
    /// resolvedMaskFormat(), &numProxies)`). `None` if the recorder is gone or the atlas cannot be
    /// made.
    #[must_use]
    pub fn atlas_proxies(&self) -> Option<Vec<Arc<TextureProxy>>> {
        let recorder = self.recorder()?;
        let format = self.resolved_mask_format();
        let priv_ = recorder.priv_();
        let mut atlas_provider = priv_.atlas_provider().borrow_mut();
        atlas_provider
            .text_atlas_manager_mut()
            .get_proxies(format)
            .filter(|proxies| !proxies.is_empty())
    }

    /// `resolvedMaskFormat()`: after creating backend data, this should be used over
    /// `subRun()->maskFormat()` since that has not been resolved to what's supported on the
    /// device.
    // Port of: src/gpu/graphite/geom/SubRunData.h#L78 (chrome/m156)
    #[must_use]
    pub fn resolved_mask_format(&self) -> MaskFormat {
        self.renderer_data().mask_format
    }
}
