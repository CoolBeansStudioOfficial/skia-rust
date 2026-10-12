// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/gpu/SubRunContainer.h, src/text/gpu/SubRunContainer.cpp

//! GPU text: [`SubRunContainer`] splits the glyphs of a draw into sub runs, each drawn the way
//! that suits its glyphs best:
//!
//! - direct masks ([`AtlasSubRunKind::DirectMask`]): by far the most common sub run. The mask
//!   pixels are in 1:1 correspondence with the pixels on the device, and the destination
//!   rectangles are in device space. Handles color glyphs.
//! - transformed masks ([`AtlasSubRunKind::TransformedMask`]): glyphs whose image in the atlas
//!   needs to be transformed to the screen. It is usually used for large color glyphs which can't
//!   be drawn with paths or scaled distance fields, but will be used to draw bitmap glyphs to the
//!   screen if the matrix does not map 1:1 to the screen. The destination rectangles are in
//!   source space.
//! - distance field text ([`AtlasSubRunKind::Sdft`]): largish single color glyphs that still fit
//!   in the atlas; the sizes between direct and path sub runs. The destination rectangles are in
//!   source space.
//! - paths ([`PathSubRun`]) and drawables ([`DrawableSubRun`]).
//!
//! # Naming conventions
//!
//! * `drawMatrix`: the CTM from the canvas.
//! * `drawOrigin`: the x, y location of the drawTextBlob call.
//! * `positionMatrix`: the combination of the drawMatrix and the drawOrigin:
//!   `positionMatrix = drawMatrix * TranslationMatrix(drawOrigin.x, drawOrigin.y)`.
//!
//! skia-rust: Skia allocates the sub runs and their data in a `SubRunAllocator` arena and links
//! them in an intrusive list; here they are owned (a `Vec` of [`SubRun`]), and the atlas sub runs
//! are shared (`Arc`) with the draws that record them. The sub runs are made from the global
//! strike cache. The flattening of sub runs (`flatten`, `MakeFromBuffer`: Slug
//! serialization needs the remote glyph cache, T23), `EstimateAllocSize` and
//! `testingOnly_atlasSubRun`.

use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use skia_rust_core::device::{Device, DeviceTransformRestore};
use skia_rust_core::font::Font;
use skia_rust_core::font_priv::approximate_transformed_text_size;
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::glyph::{
    ActionType, GlyphAction, GlyphDigest, GlyphPositionRoundingSpec, GlyphRect, skglyph,
};
use skia_rust_core::glyph_run::GlyphRunList;
use skia_rust_core::m44::M44;
use skia_rust_core::mask::MaskFormat as GlyphMaskFormat;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::matrix_priv::differential_area_scale;
use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{Scalar, scalar, scalar_floor_to_scalar, scalar_sqrt};
use skia_rust_core::scaler_context::ScalerContextBuildFlags;
use skia_rust_core::strike::{
    StrikeForGpu, StrikeForGpuCacheInterface, StrikePromise, TypefaceIdTranslator,
};
use skia_rust_core::strike_cache::StrikeCache;
use skia_rust_core::strike_spec::StrikeSpec;
use skia_rust_core::stroke_rec::StrokeRec;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::write_buffer::BinaryWriteBuffer;
use skia_rust_effects::dash_path_effect;

use crate::gpu::mask_format::MaskFormat;
use crate::text_gpu::format_from_glyph;
use crate::text_gpu::glyph_vector::{GlyphVector, RendererData};
use crate::text_gpu::sdf_mask_filter;
use crate::text_gpu::sub_run_control::{SdftMatrixRange, SubRunControl};
use crate::text_gpu::vertex_filler::{FillerType, VertexFiller};

use skia_rust_core::distance_field_gen::DISTANCE_FIELD_INSET;

/// What a device tells the sub runs about how it draws text (`SkStrikeDeviceInfo`).
// Port of: src/core/SkDevice.h#L79-L84 (chrome/m156)
#[doc(alias = "SkStrikeDeviceInfo")]
#[derive(Clone, Copy, Debug)]
pub struct StrikeDeviceInfo {
    /// `fSurfaceProps`.
    pub surface_props: SurfaceProps,
    /// `fScalerContextFlags`.
    pub scaler_context_flags: ScalerContextBuildFlags,
    /// `fSubRunControl`.
    pub sub_run_control: SubRunControl,
}

/// Where the sub runs draw: the atlas sub runs go to the device's atlas delegate
/// (`AtlasDrawDelegate`), and the paths and drawables are drawn with the device
/// (`SkCanvas*`).
// Port of: src/text/gpu/SubRunContainer.h#L50-L53 (chrome/m156)
pub trait SubRunTarget {
    /// The `AtlasDrawDelegate`: draws the glyphs of an atlas sub run.
    fn draw_atlas_sub_run(
        &mut self,
        sub_run: &Arc<AtlasSubRun>,
        draw_origin: Point,
        paint: &Paint,
        renderer_data: RendererData,
    );

    /// The device the paths are drawn on (the canvas of the C++ `SubRun::draw`).
    fn device(&mut self) -> &mut dyn Device;
}

/// Parameters of the glyphs of an atlas sub run, for the renderer to choose from
/// (`AtlasSubRun::GlyphParams`).
// Port of: src/text/gpu/SubRunContainer.h#L97-L101 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphParams {
    /// `isSDF`.
    pub is_sdf: bool,
    /// `isLCD`.
    pub is_lcd: bool,
    /// `isAA`.
    pub is_aa: bool,
}

/// How an atlas sub run draws its glyphs.
// Port of: src/text/gpu/SubRunContainer.cpp#L420-L640 (chrome/m156), DirectMaskSubRun,
// TransformedMaskSubRun and SDFTSubRun
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AtlasSubRunKind {
    /// `DirectMaskSubRun`.
    DirectMask,
    /// `TransformedMaskSubRun`.
    TransformedMask {
        /// `fIsBigEnough`.
        is_big_enough: bool,
    },
    /// `SDFTSubRun`.
    Sdft {
        /// `fUseLCDText`.
        use_lcd_text: bool,
        /// `fAntiAliased`.
        anti_aliased: bool,
        /// `fMatrixRange`.
        matrix_range: SdftMatrixRange,
    },
}

/// `AtlasSubRun` is the API that the atlas text draw uses to generate vertex data for drawing.
/// It is specialized in the three ways of [`AtlasSubRunKind`].
// Port of: src/text/gpu/SubRunContainer.h#L82-L124 (chrome/m156)
#[doc(alias = "sktext::gpu::AtlasSubRun")]
#[derive(Debug)]
pub struct AtlasSubRun {
    kind: AtlasSubRunKind,
    /// `fVertexFiller`.
    vertex_filler: VertexFiller,
    /// `fGlyphVector`. Initially packed-ID span, converted to the backend specific per-glyph atlas
    /// location information.
    glyph_vector: GlyphVector,
}

impl AtlasSubRun {
    /// `kGlyphInsetting` of the direct mask and transformed mask sub runs.
    pub const MASK_GLYPH_INSETTING: i32 = 0;
    /// `SDFTSubRun::kGlyphInsetting`: `SK_DistanceFieldInset`.
    pub const SDFT_GLYPH_INSETTING: i32 = DISTANCE_FIELD_INSET;

    /// `IsBigEnough(matrix)`.
    // Port of: src/text/gpu/SubRunContainer.h#L89-L91 (chrome/m156)
    #[must_use]
    pub fn is_big_enough(matrix: &Matrix) -> bool {
        matrix.max_scale() >= 1.0
    }

    /// `DirectMaskSubRun::Make`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L426-L448 (chrome/m156)
    #[must_use]
    pub fn make_direct_mask(
        creation_bounds: Rect,
        accepted: &[(PackedGlyphId, Point)],
        creation_matrix: &Matrix,
        strike_promise: StrikePromise,
        mask_type: MaskFormat,
    ) -> Self {
        let positions: Vec<Point> = accepted.iter().map(|a| a.1).collect();
        let ids: Vec<PackedGlyphId> = accepted.iter().map(|a| a.0).collect();
        Self {
            kind: AtlasSubRunKind::DirectMask,
            vertex_filler: VertexFiller::make(
                mask_type,
                creation_matrix,
                creation_bounds,
                &positions,
                FillerType::IsDirect,
            ),
            glyph_vector: GlyphVector::make(strike_promise, &ids),
        }
    }

    /// `TransformedMaskSubRun::Make`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L535-L565 (chrome/m156)
    #[must_use]
    pub fn make_transformed_mask(
        accepted: &[(PackedGlyphId, Point)],
        initial_position_matrix: &Matrix,
        strike_promise: StrikePromise,
        creation_matrix: &Matrix,
        creation_bounds: Rect,
        mask_type: MaskFormat,
    ) -> Self {
        let positions: Vec<Point> = accepted.iter().map(|a| a.1).collect();
        let ids: Vec<PackedGlyphId> = accepted.iter().map(|a| a.0).collect();
        Self {
            kind: AtlasSubRunKind::TransformedMask {
                is_big_enough: Self::is_big_enough(initial_position_matrix),
            },
            vertex_filler: VertexFiller::make(
                mask_type,
                creation_matrix,
                creation_bounds,
                &positions,
                FillerType::IsTransformed,
            ),
            glyph_vector: GlyphVector::make(strike_promise, &ids),
        }
    }

    /// `SDFTSubRun::Make`.
    ///
    /// NOTE: The mask format is hard coded to A8 here. This means that when `isLCD()` is called
    /// from `VertexFiller`, it always returns false. However, `VertexFiller` has no other
    /// behavior that depends on this function, and `drawAtlasSubrun` instead determines LCD
    /// based on the `use_lcd_text` member. If this ever changes, `VertexFiller` will need to take
    /// an additional `isLCD` argument on the constructor.
    ///
    /// NOTE: When creating the sub run, only the run font's edging determines LCD usage. This is
    /// because the upstream preparation in `SubRunContainer::make` is identical between LCD and
    /// non-LCD. Instead LCD is set here, so that when the sub run is consumed in
    /// `drawAtlasSubRun`, the SDF LCD renderer is chosen.
    // Port of: src/text/gpu/SubRunContainer.cpp#L586-L620 (chrome/m156)
    #[must_use]
    pub fn make_sdft(
        accepted: &[(PackedGlyphId, Point)],
        run_font: &Font,
        strike_promise: StrikePromise,
        creation_matrix: &Matrix,
        creation_bounds: Rect,
        matrix_range: SdftMatrixRange,
    ) -> Self {
        let positions: Vec<Point> = accepted.iter().map(|a| a.1).collect();
        let ids: Vec<PackedGlyphId> = accepted.iter().map(|a| a.0).collect();
        Self {
            kind: AtlasSubRunKind::Sdft {
                use_lcd_text: run_font.edging() == skia_rust_core::font::Edging::SubpixelAntiAlias,
                anti_aliased: run_font.has_some_anti_aliasing(),
                matrix_range,
            },
            vertex_filler: VertexFiller::make(
                MaskFormat::A8,
                creation_matrix,
                creation_bounds,
                &positions,
                FillerType::IsTransformed,
            ),
            glyph_vector: GlyphVector::make(strike_promise, &ids),
        }
    }

    /// `DirectMaskSubRun::MakeFromBuffer`. Returns `None` (and an invalid buffer) if the data is
    /// bad.
    // Port of: src/text/gpu/SubRunContainer.cpp#L450-L465 (chrome/m156)
    #[must_use]
    pub fn direct_mask_from_buffer(
        buffer: &mut ReadBuffer<'_>,
        client: Option<&dyn TypefaceIdTranslator>,
    ) -> Option<Self> {
        let glyph_vector = GlyphVector::make_from_buffer(buffer, client);
        if !buffer.validate(glyph_vector.is_some()) {
            return None;
        }
        let glyph_vector = glyph_vector?;

        let vertex_filler = make_vertex_filler_from_buffer(
            buffer,
            &glyph_vector,
            ActionType::DirectMask,
            Self::MASK_GLYPH_INSETTING,
        );
        if !buffer.validate(vertex_filler.is_some()) {
            return None;
        }

        Some(Self {
            kind: AtlasSubRunKind::DirectMask,
            vertex_filler: vertex_filler?,
            glyph_vector,
        })
    }

    /// `TransformedMaskSubRun::MakeFromBuffer`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L567-L581 (chrome/m156)
    #[must_use]
    pub fn transformed_mask_from_buffer(
        buffer: &mut ReadBuffer<'_>,
        client: Option<&dyn TypefaceIdTranslator>,
    ) -> Option<Self> {
        let glyph_vector = GlyphVector::make_from_buffer(buffer, client);
        if !buffer.validate(glyph_vector.is_some()) {
            return None;
        }
        let glyph_vector = glyph_vector?;

        let vertex_filler = make_vertex_filler_from_buffer(
            buffer,
            &glyph_vector,
            ActionType::Mask,
            Self::MASK_GLYPH_INSETTING,
        );
        if !buffer.validate(vertex_filler.is_some()) {
            return None;
        }
        let vertex_filler = vertex_filler?;

        Some(Self {
            kind: AtlasSubRunKind::TransformedMask {
                is_big_enough: Self::is_big_enough(vertex_filler.creation_matrix()),
            },
            vertex_filler,
            glyph_vector,
        })
    }

    /// `SDFTSubRun::MakeFromBuffer`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L939-L961 (chrome/m156)
    #[must_use]
    pub fn sdft_from_buffer(
        buffer: &mut ReadBuffer<'_>,
        client: Option<&dyn TypefaceIdTranslator>,
    ) -> Option<Self> {
        let use_lcd = buffer.read_int();
        let is_anti_aliased = buffer.read_int();
        let matrix_range = SdftMatrixRange::make_from_buffer(buffer);

        let glyph_vector = GlyphVector::make_from_buffer(buffer, client);
        if !buffer.validate(glyph_vector.is_some()) {
            return None;
        }
        let glyph_vector = glyph_vector?;

        let vertex_filler = make_vertex_filler_from_buffer(
            buffer,
            &glyph_vector,
            ActionType::Sdft,
            Self::SDFT_GLYPH_INSETTING,
        );
        if !buffer.validate(vertex_filler.is_some()) {
            return None;
        }
        let vertex_filler = vertex_filler?;

        if !buffer.validate(vertex_filler.mask_format() == MaskFormat::A8) {
            return None;
        }

        Some(Self {
            kind: AtlasSubRunKind::Sdft {
                use_lcd_text: use_lcd != 0,
                anti_aliased: is_anti_aliased != 0,
                matrix_range,
            },
            vertex_filler,
            glyph_vector,
        })
    }

    /// `subRunStreamTag()`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L765-L767, #L866-L868, #L999 (chrome/m156)
    fn sub_run_stream_tag(&self) -> i32 {
        match self.kind {
            AtlasSubRunKind::DirectMask => stream_tag::DIRECT_MASK,
            AtlasSubRunKind::TransformedMask { .. } => stream_tag::TRANSFORM_MASK,
            AtlasSubRunKind::Sdft { .. } => stream_tag::SDFT,
        }
    }

    /// `doFlatten(buffer)`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L769-L772, #L870-L873, #L1000-L1006 (chrome/m156)
    fn do_flatten(&self, buffer: &mut BinaryWriteBuffer) {
        if let AtlasSubRunKind::Sdft {
            use_lcd_text,
            anti_aliased,
            matrix_range,
        } = self.kind
        {
            buffer.write_int(i32::from(use_lcd_text));
            buffer.write_int(i32::from(anti_aliased));
            matrix_range.flatten(buffer);
        }
        self.glyph_vector.flatten(buffer);
        flatten_vertex_filler(buffer, &self.vertex_filler);
    }

    /// The kind of the sub run.
    #[must_use]
    pub fn kind(&self) -> AtlasSubRunKind {
        self.kind
    }

    /// `glyphCount()`.
    // Port of: src/text/gpu/SubRunContainer.h#L108 (chrome/m156)
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.glyph_vector.glyph_count()
    }

    /// `maskFormat()`.
    // Port of: src/text/gpu/SubRunContainer.h#L109 (chrome/m156)
    #[must_use]
    pub fn mask_format(&self) -> MaskFormat {
        self.vertex_filler.mask_format()
    }

    /// `glyphSrcPadding()`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L491, #L575, #L627 (chrome/m156)
    #[must_use]
    pub fn glyph_src_padding(&self) -> i32 {
        match self.kind {
            AtlasSubRunKind::DirectMask => Self::MASK_GLYPH_INSETTING,
            // Padding NOT equal to insetting.
            AtlasSubRunKind::TransformedMask { .. } => 1,
            AtlasSubRunKind::Sdft { .. } => Self::SDFT_GLYPH_INSETTING,
        }
    }

    /// `instanceFlags()`: the mask format as the flags of the instance data.
    // Port of: src/text/gpu/SubRunContainer.h#L111 (chrome/m156)
    #[must_use]
    pub fn instance_flags(&self) -> u16 {
        self.mask_format() as u16
    }

    /// `deviceRectAndNeedsTransform(positionMatrix)`: whether the glyphs need a transform to be
    /// drawn with `position_matrix`, and the device bounds of all the glyphs.
    // Port of: src/text/gpu/SubRunContainer.cpp#L493-L498, #L593, #L636 (chrome/m156)
    #[must_use]
    pub fn device_rect_and_needs_transform(&self, position_matrix: &Matrix) -> (bool, Rect) {
        let (integer_translate, device_rect) = self
            .vertex_filler
            .device_rect_and_check_transform(position_matrix);
        match self.kind {
            AtlasSubRunKind::DirectMask => (!integer_translate, device_rect),
            AtlasSubRunKind::TransformedMask { .. } | AtlasSubRunKind::Sdft { .. } => {
                (true, device_rect)
            }
        }
    }

    /// `glyphParams()`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L500-L503, #L599-L602, #L638-L640 (chrome/m156)
    #[must_use]
    pub fn glyph_params(&self) -> GlyphParams {
        match self.kind {
            // Since this is non-SDF, isAA will be ignored so we just pass true.
            AtlasSubRunKind::DirectMask | AtlasSubRunKind::TransformedMask { .. } => GlyphParams {
                is_sdf: false,
                is_lcd: self.vertex_filler.is_lcd(),
                is_aa: true,
            },
            AtlasSubRunKind::Sdft {
                use_lcd_text,
                anti_aliased,
                ..
            } => GlyphParams {
                is_sdf: true,
                is_lcd: use_lcd_text,
                is_aa: anti_aliased,
            },
        }
    }

    /// `vertexFiller()`.
    // Port of: src/text/gpu/SubRunContainer.h#L120 (chrome/m156)
    #[must_use]
    pub fn vertex_filler(&self) -> &VertexFiller {
        &self.vertex_filler
    }

    /// `glyphVector()`.
    // Port of: src/text/gpu/SubRunContainer.h#L122 (chrome/m156)
    #[must_use]
    pub fn glyph_vector(&self) -> &GlyphVector {
        &self.glyph_vector
    }

    /// The `RendererData` the sub run draws with (what each `draw` passes the atlas delegate).
    // Port of: src/text/gpu/SubRunContainer.cpp#L476-L489, #L567-L579, #L622-L631 (chrome/m156)
    #[must_use]
    pub fn renderer_data(&self) -> RendererData {
        match self.kind {
            AtlasSubRunKind::DirectMask | AtlasSubRunKind::TransformedMask { .. } => RendererData {
                src_padding: self.glyph_src_padding(),
                is_sdf: false,
                is_lcd: self.vertex_filler.is_lcd(),
                mask_format: self.vertex_filler.mask_format(),
            },
            AtlasSubRunKind::Sdft { use_lcd_text, .. } => RendererData {
                src_padding: self.glyph_src_padding(),
                is_sdf: true,
                is_lcd: use_lcd_text,
                mask_format: MaskFormat::A8,
            },
        }
    }

    /// `canReuse(paint, positionMatrix)`: given an already cached sub run, can this sub run
    /// handle this combination of paint, matrix, and position?
    // Port of: src/text/gpu/SubRunContainer.cpp#L505-L508, #L553-L557, #L609-L612 (chrome/m156)
    #[must_use]
    pub fn can_reuse(&self, _paint: &Paint, position_matrix: &Matrix) -> bool {
        match self.kind {
            AtlasSubRunKind::DirectMask => {
                let (reuse, _) = self
                    .vertex_filler
                    .device_rect_and_check_transform(position_matrix);
                reuse
            }
            // If we are not scaling the cache entry to be larger, than a cache with smaller
            // glyphs may be better.
            AtlasSubRunKind::TransformedMask { is_big_enough } => is_big_enough,
            AtlasSubRunKind::Sdft { matrix_range, .. } => {
                matrix_range.matrix_in_range(position_matrix)
            }
        }
    }

    /// `unflattenSize()`.
    #[must_use]
    pub fn unflatten_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.glyph_vector.unflatten_size()
            + self.vertex_filler.unflatten_size()
    }
}

/// `PathOpSubmitter`: holds glyph ids until ready to draw. During drawing, the glyph ids are
/// converted to paths.
// Port of: src/text/gpu/SubRunContainer.cpp#L168-L343 (chrome/m156)
#[derive(Debug)]
pub struct PathOpSubmitter {
    /// `fIDsOrPaths` as ids: the glyphs.
    ids: Vec<GlyphId>,
    /// `fPositions`.
    positions: Vec<Point>,
    /// `fStrikeToSourceScale`.
    strike_to_source_scale: scalar,
    /// `fIsAntiAliased`.
    is_anti_aliased: bool,
    /// `fStrikePromise`.
    strike_promise: Mutex<StrikePromise>,
    /// The paths the ids were converted to (`fConvertIDsToPaths`).
    paths: OnceLock<Vec<Option<Path>>>,
}

impl PathOpSubmitter {
    /// `PathOpSubmitter::Make`.
    ///
    /// # Panics
    /// If `accepted` is empty.
    // Port of: src/text/gpu/SubRunContainer.cpp#L267-L282 (chrome/m156)
    #[must_use]
    pub fn make(
        accepted: &[(GlyphId, Point)],
        is_anti_aliased: bool,
        strike_to_source_scale: scalar,
        strike_promise: StrikePromise,
    ) -> Self {
        assert_ne!(accepted.len(), 0);
        Self {
            ids: accepted.iter().map(|a| a.0).collect(),
            positions: accepted.iter().map(|a| a.1).collect(),
            strike_to_source_scale,
            is_anti_aliased,
            strike_promise: Mutex::new(strike_promise),
            paths: OnceLock::new(),
        }
    }

    /// `unflattenSize()`.
    #[must_use]
    pub fn unflatten_size(&self) -> usize {
        self.positions.len() * std::mem::size_of::<Point>()
            + self.ids.len() * std::mem::size_of::<usize>()
    }

    /// `flatten(buffer)`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L258-L266 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        self.strike_promise
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .flatten(buffer);

        buffer.write_int(i32::from(self.is_anti_aliased));
        buffer.write_scalar(self.strike_to_source_scale);
        buffer.write_point_array(&self.positions);
        for &id in &self.ids {
            buffer.write_int(i32::from(id));
        }
    }

    /// `MakeFromBuffer(buffer, alloc, client)`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L269-L305 (chrome/m156)
    fn make_from_buffer(
        buffer: &mut ReadBuffer<'_>,
        client: Option<&dyn TypefaceIdTranslator>,
    ) -> Option<Self> {
        let strike_promise = StrikePromise::make_from_buffer(buffer, client, StrikeCache::global());
        if !buffer.validate(strike_promise.is_some()) {
            return None;
        }
        let strike_promise = strike_promise?;

        let is_anti_alias = buffer.read_int() != 0;

        let strike_to_source_scale = buffer.read_scalar();
        if !buffer.validate(0.0 < strike_to_source_scale) {
            return None;
        }

        let positions = make_points_from_buffer(buffer);
        if positions.is_empty() {
            return None;
        }

        // Remember, we stored an int for glyph id.
        let ids = read_glyph_ids(buffer, positions.len())?;

        if !buffer.is_valid() {
            return None;
        }

        Some(Self {
            ids,
            positions,
            strike_to_source_scale,
            is_anti_aliased: is_anti_alias,
            strike_promise: Mutex::new(strike_promise),
            paths: OnceLock::new(),
        })
    }

    /// Converts the glyph ids to paths if that hasn't been done yet. This is thread safe.
    // Port of: src/text/gpu/SubRunContainer.cpp#L286-L296 (chrome/m156)
    fn paths(&self) -> &[Option<Path>] {
        self.paths.get_or_init(|| {
            let mut promise = self
                .strike_promise
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let paths = promise
                .strike()
                .prepare_paths(&self.ids)
                .into_iter()
                .map(|glyph| glyph.path().cloned())
                .collect();
            // Drop ref to strike so that it can be purged from the cache if needed.
            promise.reset_strike();
            paths
        })
    }

    /// `submitDraws(canvas, drawOrigin, paint)`: not thread safe. It only occurs in the single
    /// thread drawing portion of the GPU rendering.
    // Port of: src/text/gpu/SubRunContainer.cpp#L285-L346 (chrome/m156)
    pub fn submit_draws(&self, device: &mut dyn Device, draw_origin: Point, paint: &Paint) {
        let paths = self.paths();

        let mut run_paint = paint.clone();
        run_paint.set_anti_alias(self.is_anti_aliased);

        let mask_filter = run_paint.mask_filter();

        // Calculate the matrix that maps the path glyphs from their size in the strike to the
        // graphics source space.
        let mut strike_to_source =
            Matrix::scale((self.strike_to_source_scale, self.strike_to_source_scale));
        strike_to_source.post_translate(draw_origin);

        // If there are shaders, non-blur mask filters or styles, the path must be scaled into
        // source space independently of the CTM. This allows the CTM to be correct for the
        // different effects.
        let style = StrokeRec::from_paint(&run_paint, None, None);
        let needs_exact_ctm = run_paint.shader().is_some()
            || run_paint.path_effect().is_some()
            || (!style.is_fill_style() && !style.is_hairline_style())
            || mask_filter
                .as_ref()
                .is_some_and(|mf| mf.as_base().as_a_blur().is_none());
        if needs_exact_ctm {
            // Transform the path to device because the deviceMatrix must be unchanged to draw
            // effect, filter or shader paths.
            for (path, pos) in paths.iter().zip(&self.positions) {
                let Some(path) = path else { continue };
                // Transform the glyph to source space.
                let mut path_matrix = strike_to_source.clone();
                path_matrix.post_translate(*pos);

                let mut device_outline = path.make_transform(&path_matrix);
                device_outline.set_is_volatile(true);
                device.draw_path(&device_outline, &run_paint);
            }
        } else {
            // If there is a blur mask filter, then sigma needs to be adjusted to account for the
            // scaling of fStrikeToSourceScale.
            if let Some(blur_rec) = mask_filter.and_then(|mf| mf.as_base().as_a_blur()) {
                run_paint.set_mask_filter(MaskFilter::blur(
                    blur_rec.style,
                    blur_rec.sigma / self.strike_to_source_scale,
                    None,
                ));
            }
            for (path, pos) in paths.iter().zip(&self.positions) {
                let Some(path) = path else { continue };
                // Transform the glyph to source space.
                let mut path_matrix = strike_to_source.clone();
                path_matrix.post_translate(*pos);

                // SkAutoCanvasRestore acr(canvas, true); canvas->concat(pathMatrix);
                let local_to_device =
                    M44::concat(device.state().local_to_device44(), &M44::from(path_matrix));
                let mut restore = DeviceTransformRestore::new(device, &local_to_device);
                restore.device().draw_path(path, &run_paint);
            }
        }
    }
}

/// `PathSubRun`: glyphs drawn as paths.
// Port of: src/text/gpu/SubRunContainer.cpp#L348-L396 (chrome/m156)
#[derive(Debug)]
pub struct PathSubRun {
    /// `fPathDrawing`.
    path_drawing: PathOpSubmitter,
}

/// `DrawableOpSubmitter`: the shared code for submitting draws of glyphs as drawables.
// Port of: src/text/gpu/SubRunContainer.cpp#L398-L542 (chrome/m156)
#[derive(Debug)]
pub struct DrawableOpSubmitter {
    /// `fIDsOrDrawables` as ids.
    ids: Vec<GlyphId>,
    /// `fPositions`.
    positions: Vec<Point>,
    /// `fStrikeToSourceScale`.
    strike_to_source_scale: scalar,
    /// `fStrikePromise`: when the promise is converted to a strike it acts as the ref on the
    /// strike to keep the drawable data alive.
    strike_promise: Mutex<StrikePromise>,
}

impl DrawableOpSubmitter {
    /// `DrawableOpSubmitter::Make`.
    ///
    /// # Panics
    /// If `accepted` is empty.
    // Port of: src/text/gpu/SubRunContainer.cpp#L416-L424 (chrome/m156)
    #[must_use]
    pub fn make(
        accepted: &[(GlyphId, Point)],
        strike_to_source_scale: scalar,
        strike_promise: StrikePromise,
    ) -> Self {
        assert_ne!(accepted.len(), 0);
        Self {
            ids: accepted.iter().map(|a| a.0).collect(),
            positions: accepted.iter().map(|a| a.1).collect(),
            strike_to_source_scale,
            strike_promise: Mutex::new(strike_promise),
        }
    }

    /// `submitDraws(canvas, drawOrigin, paint)`.
    ///
    /// Drawing a glyph's `Drawable` needs a canvas (`saveLayer` and `SkDrawable::draw`), which a
    /// device cannot make, so glyph drawables are not drawn yet: a typeface only has them when
    /// it is built with drawable glyphs (SVG, custom). TODO(text-T17).
    // Port of: src/text/gpu/SubRunContainer.cpp#L478-L526 (chrome/m156)
    pub fn submit_draws(&self, _device: &mut dyn Device, _draw_origin: Point, _paint: &Paint) {
        // Convert glyph IDs to Drawables. Do not drop the strike because it must remain owned to
        // ensure the Drawable data is not freed.
        let drawables = self
            .strike_promise
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .strike()
            .prepare_drawables(&self.ids);
        let _ = (drawables, &self.positions, self.strike_to_source_scale);
        crate::gpu::sk_log::skia_log_w!(
            "Glyph drawables need a canvas to draw (T17); the drawable glyphs are not drawn."
        );
    }

    /// `unflattenSize()`.
    #[must_use]
    pub fn unflatten_size(&self) -> usize {
        self.positions.len() * std::mem::size_of::<Point>()
            + self.ids.len() * std::mem::size_of::<usize>()
    }

    /// `flatten(buffer)`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L519-L527 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        self.strike_promise
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .flatten(buffer);

        buffer.write_scalar(self.strike_to_source_scale);
        buffer.write_point_array(&self.positions);
        for &id in &self.ids {
            buffer.write_int(i32::from(id));
        }
    }

    /// `MakeFromBuffer(buffer, alloc, client)`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L529-L563 (chrome/m156)
    fn make_from_buffer(
        buffer: &mut ReadBuffer<'_>,
        client: Option<&dyn TypefaceIdTranslator>,
    ) -> Option<Self> {
        let strike_promise = StrikePromise::make_from_buffer(buffer, client, StrikeCache::global());
        if !buffer.validate(strike_promise.is_some()) {
            return None;
        }
        let strike_promise = strike_promise?;

        let strike_to_source_scale = buffer.read_scalar();
        if !buffer.validate(0.0 < strike_to_source_scale) {
            return None;
        }

        let positions = make_points_from_buffer(buffer);
        if positions.is_empty() {
            return None;
        }

        let ids = read_glyph_ids(buffer, positions.len())?;

        if !buffer.is_valid() {
            return None;
        }

        Some(Self {
            ids,
            positions,
            strike_to_source_scale,
            strike_promise: Mutex::new(strike_promise),
        })
    }
}

/// `DrawableSubRun`: glyphs drawn as drawables.
// Port of: src/text/gpu/SubRunContainer.cpp#L528-L586 (chrome/m156)
#[derive(Debug)]
pub struct DrawableSubRun {
    /// `fDrawingDrawing`.
    drawing: DrawableOpSubmitter,
}

/// One of the sub runs of a draw (`sktext::gpu::SubRun`).
// Port of: src/text/gpu/SubRunContainer.h#L58-L80 (chrome/m156)
#[doc(alias = "sktext::gpu::SubRun")]
#[derive(Debug)]
pub enum SubRun {
    /// A `DirectMaskSubRun`, `TransformedMaskSubRun` or `SDFTSubRun`.
    Atlas(Arc<AtlasSubRun>),
    /// A `PathSubRun`.
    Path(PathSubRun),
    /// A `DrawableSubRun`.
    Drawable(DrawableSubRun),
}

impl SubRun {
    /// `draw(canvas, drawOrigin, paint, subRunStorage, drawAtlas)`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L476-L489 and the sub runs' `draw` (chrome/m156)
    pub fn draw(&self, target: &mut dyn SubRunTarget, draw_origin: Point, paint: &Paint) {
        match self {
            SubRun::Atlas(sub_run) => {
                target.draw_atlas_sub_run(sub_run, draw_origin, paint, sub_run.renderer_data());
            }
            SubRun::Path(sub_run) => {
                sub_run
                    .path_drawing
                    .submit_draws(target.device(), draw_origin, paint);
            }
            SubRun::Drawable(sub_run) => {
                sub_run
                    .drawing
                    .submit_draws(target.device(), draw_origin, paint);
            }
        }
    }

    /// `flatten(buffer)`: the stream tag, then the sub run's data.
    // Port of: src/text/gpu/SubRunContainer.cpp#L1048-L1051 (chrome/m156)
    pub fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        match self {
            SubRun::Atlas(sub_run) => {
                buffer.write_int(sub_run.sub_run_stream_tag());
                sub_run.do_flatten(buffer);
            }
            SubRun::Path(sub_run) => {
                buffer.write_int(stream_tag::PATH);
                sub_run.path_drawing.flatten(buffer);
            }
            SubRun::Drawable(sub_run) => {
                buffer.write_int(stream_tag::DRAWABLE);
                sub_run.drawing.flatten(buffer);
            }
        }
    }

    /// `MakeFromBuffer(buffer, alloc, client)`: reads the stream tag and then the sub run it
    /// names. Returns `None` if the data is bad.
    // Port of: src/text/gpu/SubRunContainer.cpp#L1053-L1081 (chrome/m156)
    #[must_use]
    pub fn make_from_buffer(
        buffer: &mut ReadBuffer<'_>,
        client: Option<&dyn TypefaceIdTranslator>,
    ) -> Option<SubRun> {
        let sub_run_type = buffer.read_int();
        if !buffer.validate(stream_tag::BAD < sub_run_type && sub_run_type < stream_tag::COUNT) {
            return None;
        }
        match sub_run_type {
            stream_tag::DIRECT_MASK => AtlasSubRun::direct_mask_from_buffer(buffer, client)
                .map(|sub_run| SubRun::Atlas(Arc::new(sub_run))),
            stream_tag::SDFT => AtlasSubRun::sdft_from_buffer(buffer, client)
                .map(|sub_run| SubRun::Atlas(Arc::new(sub_run))),
            stream_tag::TRANSFORM_MASK => AtlasSubRun::transformed_mask_from_buffer(buffer, client)
                .map(|sub_run| SubRun::Atlas(Arc::new(sub_run))),
            stream_tag::PATH => {
                let path_drawing = PathOpSubmitter::make_from_buffer(buffer, client);
                if !buffer.validate(path_drawing.is_some()) {
                    return None;
                }
                path_drawing.map(|path_drawing| SubRun::Path(PathSubRun { path_drawing }))
            }
            stream_tag::DRAWABLE => {
                let drawing = DrawableOpSubmitter::make_from_buffer(buffer, client);
                if !buffer.validate(drawing.is_some()) {
                    return None;
                }
                drawing.map(|drawing| SubRun::Drawable(DrawableSubRun { drawing }))
            }
            _ => None,
        }
    }

    /// `canReuse(paint, positionMatrix)`.
    #[must_use]
    pub fn can_reuse(&self, paint: &Paint, position_matrix: &Matrix) -> bool {
        match self {
            SubRun::Atlas(sub_run) => sub_run.can_reuse(paint, position_matrix),
            SubRun::Path(_) | SubRun::Drawable(_) => true,
        }
    }

    /// `unflattenSize()`: size hint for unflattening this run.
    #[must_use]
    pub fn unflatten_size(&self) -> usize {
        match self {
            SubRun::Atlas(sub_run) => sub_run.unflatten_size(),
            SubRun::Path(sub_run) => {
                std::mem::size_of::<PathSubRun>() + sub_run.path_drawing.unflatten_size()
            }
            SubRun::Drawable(sub_run) => {
                std::mem::size_of::<DrawableSubRun>() + sub_run.drawing.unflatten_size()
            }
        }
    }

    /// The underlying atlas sub run if it exists (`testingOnly_atlasSubRun`).
    #[must_use]
    pub fn atlas_sub_run(&self) -> Option<&Arc<AtlasSubRun>> {
        match self {
            SubRun::Atlas(sub_run) => Some(sub_run),
            SubRun::Path(_) | SubRun::Drawable(_) => None,
        }
    }
}

/// `SubRunStreamTag`: how a sub run is tagged in a stream.
// Port of: src/text/gpu/SubRunContainer.cpp#L76-L87 (chrome/m156)
mod stream_tag {
    /// `kBad`: 0, to line up with errors from `readInt`.
    pub const BAD: i32 = 0;
    /// `kDirectMaskStreamTag`.
    pub const DIRECT_MASK: i32 = 1;
    /// `kSDFTStreamTag`.
    pub const SDFT: i32 = 2;
    /// `kTransformMaskStreamTag`.
    pub const TRANSFORM_MASK: i32 = 3;
    /// `kPathStreamTag`.
    pub const PATH: i32 = 4;
    /// `kDrawableStreamTag`.
    pub const DRAWABLE: i32 = 5;
    /// `kSubRunStreamTagCount`.
    pub const COUNT: i32 = 6;
}

/// `BagOfBytes::WillCountFit<SkPoint>(n)`.
// Port of: src/text/gpu/SubRunAllocator.h#L119-L123 (chrome/m156)
fn will_count_fit_points(n: u32) -> bool {
    /// `BagOfBytes::kMaxByteSize`: `INT_MAX - k4K`.
    const MAX_BYTE_SIZE: i32 = i32::MAX - (1 << 12);
    const MAX_N: i32 = MAX_BYTE_SIZE / 8;
    i32::try_from(n).is_ok_and(|n| (0..MAX_N).contains(&n))
}

/// `make_points_from_buffer`: the points of an array in the buffer. Returns the empty vector if
/// there is a problem reading the positions.
// Port of: src/text/gpu/SubRunContainer.cpp#L115-L130 (chrome/m156)
fn make_points_from_buffer(buffer: &mut ReadBuffer<'_>) -> Vec<Point> {
    let glyph_count = buffer.get_array_count();

    // Zero indicates a problem with serialization.
    if !buffer.validate(glyph_count != 0) {
        return Vec::new();
    }

    // Check that the count will not overflow the arena.
    if !buffer.validate(will_count_fit_points(glyph_count)) {
        return Vec::new();
    }

    // skia-rust: the points are not read into storage that the count sized before it is checked
    // against the data: a count that the rest of the buffer cannot hold fails the read below as
    // well, without a large allocation.
    let count = glyph_count as usize;
    if !buffer.validate_can_read_n(count, size_of::<Point>()) {
        return Vec::new();
    }

    let mut positions = vec![Point::new(0.0, 0.0); count];
    if !buffer.read_point_array(&mut positions) {
        return Vec::new();
    }
    positions
}

/// Reads `count` glyph ids, which were written as `int`s (`SkTo<SkGlyphID>(buffer.readInt())`).
/// C++ aborts on an id that does not fit; here the buffer becomes invalid.
// Port of: src/text/gpu/SubRunContainer.cpp#L288-L293, #L541-L546 (chrome/m156)
fn read_glyph_ids(buffer: &mut ReadBuffer<'_>, count: usize) -> Option<Vec<GlyphId>> {
    if !buffer.validate_can_read_n(count, size_of::<i32>()) {
        return None;
    }
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        let id = GlyphId::try_from(buffer.read_int());
        if !buffer.validate(id.is_ok()) {
            return None;
        }
        ids.push(id.ok()?);
    }
    Some(ids)
}

/// `glyph_bounds_from_left_top(digest, inset, leftTop)`.
// Port of: src/text/gpu/SubRunContainer.cpp#L92-L97 (chrome/m156)
fn glyph_bounds_from_left_top(digest: GlyphDigest, inset: i32, left_top: Point) -> GlyphRect {
    let bounds = digest.bounds();
    #[allow(clippy::cast_precision_loss)] // the insets are 0, 1 and 2
    let inset_f = inset as scalar;
    let offset = left_top - bounds.left_top() - Point::new(inset_f, inset_f);
    glyph_bounds(digest, inset, offset)
}

/// `reconstruct_safe_bounds`: the bounds of the glyphs at the positions, from the strike's own
/// metrics rather than from the bounds in the stream.
// Port of: src/text/gpu/SubRunContainer.cpp#L132-L153 (chrome/m156)
fn reconstruct_safe_bounds(
    left_top: &[Point],
    glyph_vector: &GlyphVector,
    action_type: ActionType,
    inset: i32,
) -> Rect {
    let strike = Arc::clone(glyph_vector.strike_promise().strike());
    let mut guard = strike.lock();

    let mut bounding_rect = skglyph::empty_rect();
    for (i, &position) in left_top.iter().enumerate() {
        let packed_id = glyph_vector.get_packed_glyph_id(i);
        let digest = guard.digest_for(action_type, packed_id);
        bounding_rect = skglyph::rect_union(
            bounding_rect,
            glyph_bounds_from_left_top(digest, inset, position),
        );
    }
    if bounding_rect.is_empty() {
        Rect::new_empty()
    } else {
        bounding_rect.rect()
    }
}

/// `flatten_vertex_filler`.
// Port of: src/text/gpu/SubRunContainer.cpp#L155-L160 (chrome/m156)
fn flatten_vertex_filler(buffer: &mut BinaryWriteBuffer, vf: &VertexFiller) {
    buffer.write_int(vf.mask_format() as i32);
    buffer.write_bool(vf.can_draw_direct());
    buffer.write_matrix(vf.creation_matrix());
    buffer.write_point_array(vf.top_lefts());
}

/// `make_vertex_filler_from_buffer`.
// Port of: src/text/gpu/SubRunContainer.cpp#L162-L193 (chrome/m156)
fn make_vertex_filler_from_buffer(
    buffer: &mut ReadBuffer<'_>,
    glyph_vector: &GlyphVector,
    action_type: ActionType,
    inset: i32,
) -> Option<VertexFiller> {
    /// `MaskFormat::kLast`.
    const MASK_FORMAT_LAST: u32 = 2;
    let mask_format = match buffer.read32_le(MASK_FORMAT_LAST) {
        1 => MaskFormat::A565,
        2 => MaskFormat::Argb,
        _ => MaskFormat::A8,
    };

    let can_draw_direct = buffer.read_bool();

    let creation_matrix = buffer.read_matrix();
    // The only valid creationMatrices do not have perspective and many rendering parts assume
    // they are non-affine, so reject any malformed matrices here.
    if !buffer.validate(!creation_matrix.has_perspective()) {
        return None;
    }

    let left_top = make_points_from_buffer(buffer);
    if left_top.is_empty() {
        return None;
    }

    if !buffer.validate(glyph_vector.glyph_count() == left_top.len()) {
        return None;
    }

    let safe_bounds = reconstruct_safe_bounds(&left_top, glyph_vector, action_type, inset);

    Some(VertexFiller::new(
        mask_format,
        &creation_matrix,
        safe_bounds,
        &left_top,
        can_draw_direct,
    ))
}

/// `glyph_bounds(digest, inset, origin)`.
// Port of: src/text/gpu/SubRunContainer.cpp#L88-L90 (chrome/m156)
fn glyph_bounds(digest: GlyphDigest, inset: i32, origin: Point) -> GlyphRect {
    #[allow(clippy::cast_precision_loss)] // the insets are 0, 1 and 2
    let inset = inset as scalar;
    digest.bounds().inset(inset, inset).offset_point(origin)
}

/// `SkIsFinite(x, y)`.
fn is_finite_position(pos: Point) -> bool {
    pos.x.is_finite() && pos.y.is_finite()
}

/// The glyphs accepted for atlas drawing: the packed id, the top left of the glyph and, for the
/// mask drawing, its mask format.
type AcceptedMasks = Vec<(PackedGlyphId, Point, GlyphMaskFormat)>;
/// The glyphs a drawing action did not accept.
type Rejected = Vec<(GlyphId, Point)>;

/// `prepare_for_SDFT_drawing`.
// Port of: src/text/gpu/SubRunContainer.cpp#L1131-L1166 (chrome/m156)
fn prepare_for_sdft_drawing(
    strike: &dyn StrikeForGpu,
    creation_matrix: &Matrix,
    source: &[(GlyphId, Point)],
) -> (Vec<(PackedGlyphId, Point)>, Rejected, Rect) {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut bounding_rect = skglyph::empty_rect();
    let mut guard = strike.lock_for_gpu();
    for &(glyph_id, pos) in source {
        if !is_finite_position(pos) {
            continue;
        }

        let packed_id = PackedGlyphId::from_glyph_id(glyph_id);
        let digest = guard.digest_for(ActionType::Sdft, packed_id);
        match digest.action_for(ActionType::Sdft) {
            GlyphAction::Accept => {
                let mapped_pos = creation_matrix.map_point(pos);
                let bounds = glyph_bounds(digest, AtlasSubRun::SDFT_GLYPH_INSETTING, mapped_pos);
                bounding_rect = skglyph::rect_union(bounding_rect, bounds);
                accepted.push((packed_id, bounds.left_top()));
            }
            GlyphAction::Reject => rejected.push((glyph_id, pos)),
            _ => {}
        }
    }
    (accepted, rejected, bounding_rect.rect())
}

/// `prepare_for_direct_mask_drawing`.
// Port of: src/text/gpu/SubRunContainer.cpp#L1168-L1214 (chrome/m156)
fn prepare_for_direct_mask_drawing(
    strike: &dyn StrikeForGpu,
    position_matrix: &Matrix,
    source: &[(GlyphId, Point)],
    action_type: ActionType,
) -> (AcceptedMasks, Rejected, Rect) {
    let rounding_spec: &GlyphPositionRoundingSpec = strike.rounding_spec_for_gpu();
    let mask = rounding_spec.ignore_position_field_mask;
    let half_sample_freq = rounding_spec.half_axis_sample_freq;

    // Build up the mapping from source space to device space. Add the rounding constant
    // halfSampleFreq, so we just need to floor to get the device result.
    let mut position_matrix_with_rounding = position_matrix.clone();
    position_matrix_with_rounding.post_translate(half_sample_freq);

    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut bounding_rect = skglyph::empty_rect();
    let mut guard = strike.lock_for_gpu();
    for &(glyph_id, pos) in source {
        if !is_finite_position(pos) {
            continue;
        }

        let mapped_pos = position_matrix_with_rounding.map_point(pos);
        let packed_id = PackedGlyphId::from_point(glyph_id, mapped_pos, mask);
        let digest = guard.digest_for(action_type, packed_id);
        match digest.action_for(action_type) {
            GlyphAction::Accept => {
                let rounded_pos = Point::new(
                    scalar_floor_to_scalar(mapped_pos.x),
                    scalar_floor_to_scalar(mapped_pos.y),
                );
                let bounds = glyph_bounds(digest, AtlasSubRun::MASK_GLYPH_INSETTING, rounded_pos);
                bounding_rect = skglyph::rect_union(bounding_rect, bounds);
                accepted.push((packed_id, bounds.left_top(), digest.mask_format()));
            }
            GlyphAction::Reject => rejected.push((glyph_id, pos)),
            _ => {}
        }
    }
    (accepted, rejected, bounding_rect.rect())
}

/// `prepare_for_mask_drawing`.
// Port of: src/text/gpu/SubRunContainer.cpp#L1216-L1253 (chrome/m156)
fn prepare_for_mask_drawing(
    strike: &dyn StrikeForGpu,
    creation_matrix: &Matrix,
    source: &[(GlyphId, Point)],
) -> (AcceptedMasks, Rejected, Rect) {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut bounding_rect = skglyph::empty_rect();
    let mut guard = strike.lock_for_gpu();
    for &(glyph_id, pos) in source {
        if !is_finite_position(pos) {
            continue;
        }

        let packed_id = PackedGlyphId::from_glyph_id(glyph_id);
        let digest = guard.digest_for(ActionType::Mask, packed_id);
        match digest.action_for(ActionType::Mask) {
            GlyphAction::Accept => {
                let mapped_pos = creation_matrix.map_point(pos);
                let bounds = glyph_bounds(digest, AtlasSubRun::MASK_GLYPH_INSETTING, mapped_pos);
                bounding_rect = skglyph::rect_union(bounding_rect, bounds);
                accepted.push((packed_id, bounds.left_top(), digest.mask_format()));
            }
            GlyphAction::Reject => rejected.push((glyph_id, pos)),
            _ => {}
        }
    }
    (accepted, rejected, bounding_rect.rect())
}

/// `prepare_for_path_drawing` and `prepare_for_drawable_drawing`: the same code with a different
/// action.
// Port of: src/text/gpu/SubRunContainer.cpp#L1255-L1309 (chrome/m156)
fn prepare_for_path_or_drawable_drawing(
    strike: &dyn StrikeForGpu,
    action_type: ActionType,
    source: &[(GlyphId, Point)],
) -> (Vec<(GlyphId, Point)>, Rejected) {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut guard = strike.lock_for_gpu();
    for &(glyph_id, pos) in source {
        if !is_finite_position(pos) {
            continue;
        }

        match guard
            .digest_for(action_type, PackedGlyphId::from_glyph_id(glyph_id))
            .action_for(action_type)
        {
            GlyphAction::Accept => accepted.push((glyph_id, pos)),
            GlyphAction::Reject => rejected.push((glyph_id, pos)),
            _ => {}
        }
    }
    (accepted, rejected)
}

/// `find_maximum_glyph_dimension`: the largest dimension of the mask of any of the glyphs.
// Port of: src/text/gpu/SubRunContainer.cpp#L1112-L1122 (chrome/m156)
fn find_maximum_glyph_dimension(strike: &dyn StrikeForGpu, glyphs: &[GlyphId]) -> scalar {
    let mut guard = strike.lock_for_gpu();
    let mut max_dimension: scalar = 0.0;
    for &glyph_id in glyphs {
        let digest = guard.digest_for(ActionType::Mask, PackedGlyphId::from_glyph_id(glyph_id));
        max_dimension = scalar::from(digest.max_dimension()).max(max_dimension);
    }
    max_dimension
}

/// `add_multi_mask_format`: calls `add_single_mask_format` for each run of glyphs with the same
/// mask format.
// Port of: src/text/gpu/SubRunContainer.cpp#L702-L732 (chrome/m156)
fn add_multi_mask_format(
    mut add_single_mask_format: impl FnMut(&[(PackedGlyphId, Point)], MaskFormat),
    accepted: &[(PackedGlyphId, Point, GlyphMaskFormat)],
) {
    if accepted.is_empty() {
        return;
    }

    let mut format = format_from_glyph(accepted[0].2);
    let mut start_index = 0;
    for i in 1..accepted.len() {
        let next_format = format_from_glyph(accepted[i].2);
        if format != next_format {
            // Only pass the packed glyph ids and positions.
            let glyphs_with_same_format: Vec<_> = accepted[start_index..i]
                .iter()
                .map(|a| (a.0, a.1))
                .collect();
            // Take a ref on the strike. This should rarely happen.
            add_single_mask_format(&glyphs_with_same_format, format);
            format = next_format;
            start_index = i;
        }
    }
    let glyphs_with_same_format: Vec<_> =
        accepted[start_index..].iter().map(|a| (a.0, a.1)).collect();
    add_single_mask_format(&glyphs_with_same_format, format);
}

/// `make_sdft_strike_spec`: the strike spec for distance field text, the scale from the strike
/// to the source, and the matrix range the strike can be reused in.
// Port of: src/text/gpu/SubRunContainer.cpp#L1311-L1355 (chrome/m156)
fn make_sdft_strike_spec(
    font: &Font,
    paint: &Paint,
    surface_props: &SurfaceProps,
    device_matrix: &Matrix,
    text_location: Point,
    control: &SubRunControl,
) -> (StrikeSpec, scalar, SdftMatrixRange) {
    // Add filter to the paint which creates the SDFT data for A8 masks.
    let mut df_paint = paint.clone();
    df_paint.set_mask_filter(sdf_mask_filter::make());

    let (df_font, strike_to_source_scale, matrix_range) =
        control.get_sdf_font(font, device_matrix, text_location);

    // Adjust the stroke width by the scale factor for drawing the SDFT.
    df_paint.set_stroke_width(paint.stroke_width() / strike_to_source_scale);

    // Check for dashing and adjust the intervals.
    if let Some(path_effect) = paint.path_effect() {
        #[allow(clippy::collapsible_if)] // keeps the C++ nesting
        if let Some(info) = path_effect.as_a_dash() {
            debug_assert!(info.intervals.len() > 1);
            // Allocate the intervals.
            let scaled_intervals: Vec<scalar> = info
                .intervals
                .iter()
                .map(|interval| interval / strike_to_source_scale)
                .collect();
            let scaled_dashes =
                dash_path_effect::new(&scaled_intervals, info.phase / strike_to_source_scale);
            df_paint.set_path_effect(scaled_dashes);
        }
    }

    // Fake-gamma and subpixel antialiasing are applied in the shader, so we ignore the passed-in
    // scaler context flags. (It's only used when we fall-back to bitmap text).
    let flags = ScalerContextBuildFlags::NONE;
    let strike_spec = StrikeSpec::make_mask(&df_font, &df_paint, surface_props, flags, Matrix::i());

    (strike_spec, strike_to_source_scale, matrix_range)
}

/// Whether `make` adds the sub runs, or only does the strike calculations
/// (`SubRunContainer::SubRunCreationBehavior`).
// Port of: src/text/gpu/SubRunContainer.h#L197 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubRunCreationBehavior {
    /// `kAddSubRuns`.
    AddSubRuns,
    /// `kStrikeCalculationsOnly`.
    StrikeCalculationsOnly,
}

/// The sub runs of one draw, and the position matrix they were made with
/// (`sktext::gpu::SubRunContainer`).
// Port of: src/text/gpu/SubRunContainer.h#L170-L225 (chrome/m156)
#[doc(alias = "sktext::gpu::SubRunContainer")]
#[derive(Debug)]
pub struct SubRunContainer {
    /// `fInitialPositionMatrix`.
    initial_position_matrix: Matrix,
    /// `fSubRuns`.
    sub_runs: Vec<SubRun>,
}

impl SubRunContainer {
    /// `SubRunContainer(initialPositionMatrix)`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L735-L736 (chrome/m156)
    #[must_use]
    pub fn new(initial_position_matrix: &Matrix) -> Self {
        Self {
            initial_position_matrix: initial_position_matrix.clone(),
            sub_runs: Vec::new(),
        }
    }

    /// `initialPosition()`.
    #[must_use]
    pub fn initial_position(&self) -> &Matrix {
        &self.initial_position_matrix
    }

    /// `isEmpty()`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sub_runs.is_empty()
    }

    /// The sub runs, in drawing order.
    #[must_use]
    pub fn sub_runs(&self) -> &[SubRun] {
        &self.sub_runs
    }

    /// `flattenAllocSizeHint` without the buffer: the sum of the sizes of the sub runs.
    #[must_use]
    pub fn unflatten_size_hint(&self) -> usize {
        self.sub_runs.iter().map(SubRun::unflatten_size).sum()
    }

    /// `flattenAllocSizeHint(buffer)`: writes the sum of the sizes of the sub runs.
    // Port of: src/text/gpu/SubRunContainer.cpp#L1083-L1089 (chrome/m156)
    pub fn flatten_alloc_size_hint(&self, buffer: &mut BinaryWriteBuffer) {
        let unflatten_size_hint: usize = self.sub_runs.iter().map(SubRun::unflatten_size).sum();
        buffer.write_int(i32::try_from(unflatten_size_hint).unwrap_or(i32::MAX));
    }

    /// `AllocSizeHintFromBuffer(buffer)`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L1091-L1100 (chrome/m156)
    #[must_use]
    pub fn alloc_size_hint_from_buffer(buffer: &mut ReadBuffer<'_>) -> i32 {
        let mut sub_runs_size_hint = buffer.read_int();

        // Since the hint doesn't affect correctness, if it looks fishy just pick a reasonable
        // value.
        if !(0..=(1 << 16)).contains(&sub_runs_size_hint) {
            sub_runs_size_hint = 128;
        }
        sub_runs_size_hint
    }

    /// `flattenRuns(buffer)`: the initial position matrix, the count of sub runs and the sub
    /// runs.
    // Port of: src/text/gpu/SubRunContainer.cpp#L1102-L1112 (chrome/m156)
    pub fn flatten_runs(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_matrix(&self.initial_position_matrix);
        buffer.write_int(i32::try_from(self.sub_runs.len()).unwrap_or(i32::MAX));
        for sub_run in &self.sub_runs {
            sub_run.flatten(buffer);
        }
    }

    /// `MakeFromBufferInAlloc(buffer, client, alloc)`. Returns `None` if the data is bad.
    // Port of: src/text/gpu/SubRunContainer.cpp#L1114-L1132 (chrome/m156)
    #[must_use]
    pub fn make_from_buffer(
        buffer: &mut ReadBuffer<'_>,
        client: Option<&dyn TypefaceIdTranslator>,
    ) -> Option<SubRunContainer> {
        let position_matrix = buffer.read_matrix();
        if !buffer.is_valid() {
            return None;
        }
        let mut container = SubRunContainer::new(&position_matrix);

        let sub_run_count = buffer.read_int();
        if !buffer.validate(sub_run_count > 0) {
            return None;
        }
        for _ in 0..sub_run_count {
            let sub_run = SubRun::make_from_buffer(buffer, client);
            if !buffer.validate(sub_run.is_some()) {
                return None;
            }
            if let Some(sub_run) = sub_run {
                container.sub_runs.push(sub_run);
            }
        }
        Some(container)
    }

    /// `MakeInAlloc(glyphRunList, positionMatrix, runPaint, strikeDeviceInfo, strikeCache, alloc,
    /// creationBehavior, tag)`: the container of the sub runs for the glyphs of `glyph_run_list`.
    /// If `creation_behavior` is `StrikeCalculationsOnly`, then the returned container is empty.
    // Port of: src/text/gpu/SubRunContainer.cpp#L1357-L1659 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    #[must_use]
    pub fn make(
        glyph_run_list: &GlyphRunList<'_>,
        position_matrix: &Matrix,
        run_paint: &Paint,
        strike_device_info: &StrikeDeviceInfo,
        strike_cache: &dyn StrikeForGpuCacheInterface,
        creation_behavior: SubRunCreationBehavior,
    ) -> SubRunContainer {
        let mut container = SubRunContainer::new(position_matrix);

        let device_props = &strike_device_info.surface_props;
        let scaler_context_flags = strike_device_info.scaler_context_flags;
        let sub_run_control = &strike_device_info.sub_run_control;
        let max_mask_size = sub_run_control.max_size();
        let add_sub_runs = creation_behavior == SubRunCreationBehavior::AddSubRuns;

        let glyph_run_list_location = glyph_run_list.source_bounds().center();

        // Handle all the runs in the glyphRunList
        for glyph_run in glyph_run_list.runs() {
            let mut source: Vec<(GlyphId, Point)> = glyph_run.source().collect();
            let run_font = glyph_run.font();

            let approximate_device_text_size =
                // Since the positionMatrix has the origin prepended, use the plain sourceBounds
                // from above.
                approximate_transformed_text_size(run_font, position_matrix, glyph_run_list_location);

            // Atlas mask cases - SDFT and direct mask
            // Only consider using direct or SDFT drawing if not drawing hairlines and not too
            // big.
            if (run_paint.style() != Style::Stroke || run_paint.stroke_width() != 0.0)
                && approximate_device_text_size < max_mask_size
            {
                // SDFT case
                if sub_run_control.is_sdft(approximate_device_text_size, run_paint, position_matrix)
                {
                    // Process SDFT - This should be the .009% case.
                    let (strike_spec, strike_to_source_scale, matrix_range) = make_sdft_strike_spec(
                        run_font,
                        run_paint,
                        device_props,
                        position_matrix,
                        glyph_run_list_location,
                        sub_run_control,
                    );

                    if !strike_to_source_scale.nearly_zero(None) {
                        let strike = strike_cache.find_or_create_scoped_strike(&strike_spec);

                        // The creationMatrix needs to scale the strike data when inverted and
                        // multiplied by the positionMatrix. The final CTM should be:
                        //   [positionMatrix][scale by strikeToSourceScale],
                        // which should equal the following because of the transform during the
                        // vertex calculation,
                        //   [positionMatrix][creationMatrix]^-1.
                        // So, the creation matrix needs to be
                        //   [scale by 1/strikeToSourceScale].
                        let creation_matrix = Matrix::scale((
                            1.0 / strike_to_source_scale,
                            1.0 / strike_to_source_scale,
                        ));

                        let (accepted, rejected, creation_bounds) =
                            prepare_for_sdft_drawing(&*strike, &creation_matrix, &source);
                        source = rejected;

                        if add_sub_runs && !accepted.is_empty() {
                            container.sub_runs.push(SubRun::Atlas(Arc::new(
                                AtlasSubRun::make_sdft(
                                    &accepted,
                                    run_font,
                                    strike.strike_promise(),
                                    &creation_matrix,
                                    creation_bounds,
                                    matrix_range,
                                ),
                            )));
                        }
                    }
                }

                // Mask filters with 3D format (e.g. EmbossMaskFilter) need to be drawn as a path
                // in order to apply the filter through the Canvas AutoLayer system.
                let needs_auto_layer = run_paint
                    .mask_filter()
                    .is_some_and(|mf| mf.as_base().format() == GlyphMaskFormat::ThreeD);
                // Direct Mask case
                // Handle all the directly mapped mask subruns.
                if !source.is_empty() && !position_matrix.has_perspective() && !needs_auto_layer {
                    // Process masks including ARGB - this should be the 99.99% case.
                    // This will handle medium size emoji that are sharing the run with SDFT drawn
                    // text. If things are too big they will be passed along to the drawing of
                    // last resort below.
                    let strike_spec = StrikeSpec::make_mask(
                        run_font,
                        run_paint,
                        device_props,
                        scaler_context_flags,
                        position_matrix,
                    );

                    let strike = strike_cache.find_or_create_scoped_strike(&strike_spec);

                    // Bilerp sampling requires 1px atlas padding so interpolation doesn't bleed
                    // into adjacent glyphs. kMask calls fitsInAtlasInterpolated()
                    // (SkGlyph.cpp:662), which enforces max size 254px to provide this padding.
                    //
                    // Passing kMask is safe here because prepare_for_direct_mask_drawing uses
                    // subpixel-positioned SkPackedGlyphIDs, whereas prepare_for_mask_drawing uses
                    // unpositioned SkPackedGlyphIDs, ensuring independent strike cache digest
                    // entries.
                    let action_type = if sub_run_control.use_bilerp() {
                        ActionType::Mask
                    } else {
                        ActionType::DirectMask
                    };
                    let (accepted, rejected, creation_bounds) = prepare_for_direct_mask_drawing(
                        &*strike,
                        position_matrix,
                        &source,
                        action_type,
                    );
                    source = rejected;

                    if add_sub_runs && !accepted.is_empty() {
                        let initial_position = container.initial_position().clone();
                        let mut new_sub_runs = Vec::new();
                        add_multi_mask_format(
                            |subrun, format| {
                                new_sub_runs.push(SubRun::Atlas(Arc::new(
                                    AtlasSubRun::make_direct_mask(
                                        creation_bounds,
                                        subrun,
                                        &initial_position,
                                        Arc::clone(&strike).strike_promise(),
                                        format,
                                    ),
                                )));
                            },
                            &accepted,
                        );
                        container.sub_runs.extend(new_sub_runs);
                    }
                }
            }

            // Drawable case
            // Handle all the drawable glyphs - usually large or perspective color glyphs.
            if !source.is_empty() {
                let (strike_spec, strike_to_source_scale) =
                    StrikeSpec::make_path(run_font, run_paint, device_props, scaler_context_flags);

                if !strike_to_source_scale.nearly_zero(None) {
                    let strike = strike_cache.find_or_create_scoped_strike(&strike_spec);

                    let (accepted, rejected) = prepare_for_path_or_drawable_drawing(
                        &*strike,
                        ActionType::Drawable,
                        &source,
                    );
                    source = rejected;

                    if add_sub_runs && !accepted.is_empty() {
                        container.sub_runs.push(SubRun::Drawable(DrawableSubRun {
                            drawing: DrawableOpSubmitter::make(
                                &accepted,
                                strike_to_source_scale,
                                strike.strike_promise(),
                            ),
                        }));
                    }
                }
            }

            // Path case
            // Handle path subruns. Mainly, large or large perspective glyphs with no color.
            if !source.is_empty() {
                let (strike_spec, strike_to_source_scale) =
                    StrikeSpec::make_path(run_font, run_paint, device_props, scaler_context_flags);

                if !strike_to_source_scale.nearly_zero(None) {
                    let strike = strike_cache.find_or_create_scoped_strike(&strike_spec);

                    let (accepted, rejected) =
                        prepare_for_path_or_drawable_drawing(&*strike, ActionType::Path, &source);
                    source = rejected;

                    if add_sub_runs && !accepted.is_empty() {
                        let is_anti_aliased =
                            sub_run_control.force_path_aa() || run_font.has_some_anti_aliasing();
                        container.sub_runs.push(SubRun::Path(PathSubRun {
                            path_drawing: PathOpSubmitter::make(
                                &accepted,
                                is_anti_aliased,
                                strike_to_source_scale,
                                strike.strike_promise(),
                            ),
                        }));
                    }
                }
            }

            // Drawing of last resort case
            // Draw all the rest of the rejected glyphs from above. This scales out of the atlas
            // to the screen, so quality will suffer. This mainly handles large color or
            // perspective color not handled by Drawables.
            if !source.is_empty() && !approximate_device_text_size.nearly_zero(None) {
                // Creation matrix will be changed below to meet the following criteria:
                // * No perspective - the font scaler and the strikes can't handle perspective
                //   masks.
                // * Fits atlas - creationMatrix will be conditioned so that the maximum glyph
                //   dimension for this run will be < kMaxBilerpAtlasDimension.
                let mut creation_matrix = position_matrix.clone();

                // Condition creationMatrix for perspective.
                if creation_matrix.has_perspective() {
                    // Find a scale factor that reduces pixelation caused by keystoning.
                    let center = glyph_run_list.source_bounds().center();
                    let max_area_scale = differential_area_scale(&creation_matrix, center);
                    let mut perspective_factor = 1.0;
                    if max_area_scale.is_finite() && !max_area_scale.nearly_zero(None) {
                        perspective_factor = scalar_sqrt(max_area_scale);
                    }

                    // Masks can not be created in perspective. Create a non-perspective font
                    // with a scale that will support the perspective keystoning.
                    creation_matrix = Matrix::scale((perspective_factor, perspective_factor));
                }

                // Reduce to make a one pixel border for the bilerp padding.
                let max_bilerp_atlas_dimension: scalar =
                    scalar::from(GlyphDigest::SIDE_TOO_BIG_FOR_ATLAS) - 2.0;

                // Get the raw glyph IDs to simulate device drawing to figure the maximum device
                // dimension.
                let glyphs: Vec<GlyphId> = source.iter().map(|s| s.0).collect();

                // maxGlyphDimension always returns an integer even though the return type is
                // SkScalar.
                let max_glyph_dimension = |m: &Matrix| {
                    let strike_spec = StrikeSpec::make_transform_mask(
                        run_font,
                        run_paint,
                        device_props,
                        scaler_context_flags,
                        m,
                    );
                    let gauging_strike = strike_cache.find_or_create_scoped_strike(&strike_spec);
                    // TODO: There is a problem where a small character (say .) and a large
                    //  character (say M) are in the same run. If the run is scaled to be very
                    //  large, then the M may return 0 because its dimensions are > 65535, but
                    //  the small character produces regular result because its largest dimension
                    //  is < 65535. This will create an improper scale factor causing the M to
                    //  be too large to fit in the atlas. Tracked by skbug.com/40044801.
                    find_maximum_glyph_dimension(&*gauging_strike, &glyphs)
                };

                // Condition the creationMatrix so that glyphs fit in the atlas.
                let mut max_dimension = max_glyph_dimension(&creation_matrix);
                while max_bilerp_atlas_dimension < max_dimension {
                    // The SkScalerContext has a limit of 65536 maximum dimension.
                    // reductionFactor will always be < 1 because maxDimension >
                    // kMaxBilerpAtlasDimension, and because maxDimension will always be an
                    // integer the reduction factor will always be at most 254 / 255.
                    let reduction_factor = max_bilerp_atlas_dimension / max_dimension;
                    creation_matrix.post_scale((reduction_factor, reduction_factor), None);
                    max_dimension = max_glyph_dimension(&creation_matrix);
                }

                // Draw using the creationMatrix.
                let strike_spec = StrikeSpec::make_transform_mask(
                    run_font,
                    run_paint,
                    device_props,
                    scaler_context_flags,
                    &creation_matrix,
                );

                let strike = strike_cache.find_or_create_scoped_strike(&strike_spec);

                let (accepted, _rejected, creation_bounds) =
                    prepare_for_mask_drawing(&*strike, &creation_matrix, &source);

                if add_sub_runs && !accepted.is_empty() {
                    let initial_position = container.initial_position().clone();
                    let mut new_sub_runs = Vec::new();
                    add_multi_mask_format(
                        |subrun, format| {
                            new_sub_runs.push(SubRun::Atlas(Arc::new(
                                AtlasSubRun::make_transformed_mask(
                                    subrun,
                                    &initial_position,
                                    Arc::clone(&strike).strike_promise(),
                                    &creation_matrix,
                                    creation_bounds,
                                    format,
                                ),
                            )));
                        },
                        &accepted,
                    );
                    container.sub_runs.extend(new_sub_runs);
                }
            }
        }

        container
    }

    /// `draw(canvas, drawOrigin, paint, subRunStorage, atlasDelegate)`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L1661-L1669 (chrome/m156)
    pub fn draw(&self, target: &mut dyn SubRunTarget, draw_origin: Point, paint: &Paint) {
        for sub_run in &self.sub_runs {
            sub_run.draw(target, draw_origin, paint);
        }
    }

    /// `canReuse(paint, positionMatrix)`.
    // Port of: src/text/gpu/SubRunContainer.cpp#L1671-L1678 (chrome/m156)
    #[must_use]
    pub fn can_reuse(&self, paint: &Paint, position_matrix: &Matrix) -> bool {
        self.sub_runs
            .iter()
            .all(|sub_run| sub_run.can_reuse(paint, position_matrix))
    }
}
