// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScalerContext.{h,cpp} (the non-virtual half; the raster helpers
// of SkScalerContext.cpp#L316-L697 are behind `GlyphPathRasterizer`)

//! [`ScalerContext`]: the scaler context. Its non-virtual half is here (`fRec`, the typeface,
//! the effects and the pre-blend). The virtuals of `SkScalerContext` are the
//! [`ScalerContextImpl`] trait, which every backend implements. Glyph images that come from a
//! path go through [`GlyphPathRasterizer`], the seam that keeps core below raster.

use std::fmt;
use std::sync::{Arc, LazyLock};

use crate::color::Color;
use crate::color_data::compute_luminance;
use crate::descriptor::{Descriptor, REC_TAG};
use crate::drawable::Drawable;
use crate::font::{Edging, Font};
use crate::font_metrics::FontMetrics;
use crate::glyph::Glyph;
use crate::mask::{MaskBuilder, MaskFormat};
use crate::mask_filter::MaskFilter;
use crate::mask_gamma::{MaskGamma, MaskPreBlend};
use crate::matrix::{Matrix, TypeMask};
use crate::packed_glyph_id::PackedGlyphId;
use crate::paint::{Cap, Join, Paint, Style};
use crate::paint_priv::compute_luminance_color;
use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::path_effect::PathEffect;
use crate::point::{Point, Vector};
use crate::rect::{IRect, Rect, RoundOut};
use crate::scalar::{scalar, scalar_round_to_scalar};
use crate::stroke_rec::StrokeRec;
use crate::surface_props::{PixelGeometry, SurfaceProps};
use crate::typeface::Typeface;
use crate::utils::matrix22::compute_givens_rotation;

use super::{ScalerContextFlags, ScalerContextRec};

/// `SK_ScalarNearlyZero`: `1 / (1 << 12)`.
// Port of: include/core/SkScalar.h (SK_ScalarNearlyZero, chrome/m156)
const SK_SCALAR_NEARLY_ZERO: scalar = 1.0 / 4096.0;

/// `SK_MAX_SIZE_FOR_LCDTEXT`: beyond this size LCD text is not used.
// Port of: src/core/SkScalerContext.cpp#L1158-L1160 (chrome/m156)
const SK_MAX_SIZE_FOR_LCDTEXT: scalar = 48.0;

/// `kStdFakeBoldInterpKeys`: the text sizes at which the fake bold extra interpolates.
// Port of: src/core/SkTextFormatParams.h#L19-L21 (chrome/m156)
const STD_FAKE_BOLD_INTERP_KEYS: [scalar; 2] = [9.0, 36.0];

/// `kStdFakeBoldInterpValues`: the fake bold extra, as a fraction of the text size.
// Port of: src/core/SkTextFormatParams.h#L23-L26 (chrome/m156)
const STD_FAKE_BOLD_INTERP_VALUES: [scalar; 2] = [1.0 / 24.0, 1.0 / 32.0];

/// `SkPaintDefaults_MiterLimit`: the default stroke miter limit.
// Port of: src/core/SkPaintDefaults.h (SkPaintDefaults_MiterLimit = 4, chrome/m156)
const PAINT_DEFAULT_MITER_LIMIT: scalar = 4.0;

/// `SK_GAMMA_CONTRAST` (the default text contrast).
// Port of: include/core/SkTypes.h#L93 (chrome/m156)
const SK_GAMMA_CONTRAST: scalar = 0.5;

/// `SK_GAMMA_EXPONENT` (0 selects sRGB).
// Port of: include/core/SkTypes.h#L85 (chrome/m156)
const SK_GAMMA_EXPONENT: scalar = 0.0;

/// `SkScalerContextFlags` (`SkScalerContext.h`): what a caller of `MakeRecAndEffects` asks for.
// Port of: src/core/SkScalerContext.h#L53-L58 (chrome/m156)
#[doc(alias = "SkScalerContextFlags")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ScalerContextBuildFlags(u32);

impl ScalerContextBuildFlags {
    /// `kNone`.
    // Port of: src/core/SkScalerContext.h#L54 (chrome/m156)
    pub const NONE: Self = Self(0);
    /// `kFakeGamma`.
    // Port of: src/core/SkScalerContext.h#L55 (chrome/m156)
    pub const FAKE_GAMMA: Self = Self(1 << 0);
    /// `kBoostContrast`.
    // Port of: src/core/SkScalerContext.h#L56 (chrome/m156)
    pub const BOOST_CONTRAST: Self = Self(1 << 1);
    /// `kFakeGammaAndBoostContrast`.
    // Port of: src/core/SkScalerContext.h#L57 (chrome/m156)
    pub const FAKE_GAMMA_AND_BOOST_CONTRAST: Self =
        Self(Self::FAKE_GAMMA.0 | Self::BOOST_CONTRAST.0);

    /// `SkToBool(flags & other)`.
    // Port of: src/core/SkScalerContext.h#L58 (SK_MAKE_BITFIELD_OPS, chrome/m156)
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

/// `SkAxisAlignment`: which axis of a glyph's position is rounded.
// Port of: src/core/SkGlyph.h#L218-L222 (chrome/m156)
#[doc(alias = "SkAxisAlignment")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AxisAlignment {
    /// Neither axis is rounded.
    // Port of: src/core/SkGlyph.h#L219 (chrome/m156)
    None,
    /// The x component is rounded.
    // Port of: src/core/SkGlyph.h#L220 (chrome/m156)
    X,
    /// The y component is rounded.
    // Port of: src/core/SkGlyph.h#L221 (chrome/m156)
    Y,
}

/// Which scale the pre matrix takes (`SkScalerContextRec::PreMatrixScale`).
// Port of: src/core/SkScalerContext.h#L176-L180 (chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreMatrixScale {
    /// The underlying port can apply both x and y scale.
    Full,
    /// The underlying port can only apply a y scale.
    Vertical,
    /// The underlying port can only apply an integer y scale.
    VerticalInteger,
}

/// The effects a scaler context needs from a paint (`SkScalerContextEffects`).
// Port of: src/core/SkScalerContext.h#L244-L251 (chrome/m156)
#[doc(alias = "SkScalerContextEffects")]
#[derive(Clone, Debug, Default)]
pub struct ScalerContextEffects {
    /// `fPathEffect`.
    pub path_effect: Option<PathEffect>,
    /// `fMaskFilter`.
    pub mask_filter: Option<MaskFilter>,
}

/// A path of a glyph that a scaler context generated, and whether it was modified
/// (`SkScalerContext::GeneratedPath`).
// Port of: src/core/SkScalerContext.h#L387-L390 (chrome/m156)
#[derive(Clone, Debug)]
pub struct GeneratedPath {
    /// The path, in glyph space.
    pub path: Path,
    /// Whether the path differs from the font's outline.
    pub modified: bool,
}

/// The metrics a backend reports for one glyph (`SkScalerContext::GlyphMetrics`).
// Port of: src/core/SkScalerContext.h#L391-L409 (chrome/m156)
#[doc(alias = "SkScalerContext::GlyphMetrics")]
#[derive(Clone, Debug)]
pub struct GlyphMetrics {
    /// `advance`.
    pub advance: Vector,
    /// `bounds`.
    pub bounds: Rect,
    /// `maskFormat`.
    pub mask_format: MaskFormat,
    /// `extraBits`.
    pub extra_bits: u16,
    /// `neverRequestPath`.
    pub never_request_path: bool,
    /// `computeFromPath`.
    pub compute_from_path: bool,
    /// `generatedPath`.
    pub generated_path: Option<GeneratedPath>,
}

impl GlyphMetrics {
    /// `GlyphMetrics(SkMask::Format format)`: zero advance and bounds.
    // Port of: src/core/SkScalerContext.h#L400-L409 (chrome/m156)
    #[must_use]
    pub fn new(mask_format: MaskFormat) -> Self {
        Self {
            advance: Point::new(0.0, 0.0),
            bounds: Rect::from_ltrb(0.0, 0.0, 0.0, 0.0),
            mask_format,
            extra_bits: 0,
            never_request_path: false,
            compute_from_path: false,
            generated_path: None,
        }
    }
}

/// Draws a glyph path into an A8 mask and packs it into the glyph's format
/// (`SkScalerContext::GenerateImageFromPath`). Implemented by `skia_rust_raster`, the only
/// crate that can rasterize; core reaches it through this seam (design §3.2).
// Port of: src/core/SkScalerContext.cpp#L586-L697 (chrome/m156)
pub trait GlyphPathRasterizer: Send + Sync + fmt::Debug {
    /// `SkScalerContext::GenerateImageFromPath(dst, path, preBlend, doBGR, verticalLCD,
    /// a8FromLCD, hairline)`.
    // Port of: src/core/SkScalerContext.cpp#L586-L697 (chrome/m156)
    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)] // mirrors the C++ signature
    fn generate_image_from_path(
        &self,
        dst: &mut MaskBuilder,
        path: &Path,
        pre_blend: &MaskPreBlend,
        do_bgr: bool,
        vertical_lcd: bool,
        a8_from_lcd: bool,
        hairline: bool,
    );
}

/// The rasterizer of typefaces with no glyph paths (the empty typeface). Its glyphs have no
/// path, so `getImage` never reaches it; the method is unreachable by construction.
#[derive(Debug)]
pub struct NoPathRasterizer;

impl GlyphPathRasterizer for NoPathRasterizer {
    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)] // mirrors the trait
    fn generate_image_from_path(
        &self,
        _dst: &mut MaskBuilder,
        _path: &Path,
        _pre_blend: &MaskPreBlend,
        _do_bgr: bool,
        _vertical_lcd: bool,
        _a8_from_lcd: bool,
        _hairline: bool,
    ) {
        unreachable!("the empty typeface has no glyph paths, so no image is made from one");
    }
}

/// The rasterizer for contexts that never draw from paths.
pub static NO_PATH_RASTERIZER: NoPathRasterizer = NoPathRasterizer;

/// The non-virtual state of a scaler context, shared with the backend's
/// [`ScalerContextImpl`] methods (the protected members of `SkScalerContext`).
#[derive(Debug)]
pub struct ScalerContextBase {
    rec: ScalerContextRec,
    typeface: Typeface,
    path_effect: Option<PathEffect>,
    mask_filter: Option<MaskFilter>,
    generate_image_from_path: bool,
    pre_blend: MaskPreBlend,
    rasterizer: &'static dyn GlyphPathRasterizer,
}

impl ScalerContextBase {
    /// The record (`fRec`).
    #[must_use]
    pub fn rec(&self) -> &ScalerContextRec {
        &self.rec
    }

    /// The typeface (`fTypeface`).
    #[must_use]
    pub fn typeface(&self) -> &Typeface {
        &self.typeface
    }

    /// The pre-blend (`fPreBlend`).
    #[must_use]
    pub fn pre_blend(&self) -> &MaskPreBlend {
        &self.pre_blend
    }

    /// The post matrix of the record, `SkScalerContextRec::getMatrixFrom2x2`.
    // Port of: src/core/SkScalerContext.cpp#L950-L953 (chrome/m156)
    #[must_use]
    pub fn matrix_from_2x2(&self) -> Matrix {
        self.rec.get_matrix_from_2x2()
    }

    /// `SkScalerContext::GenerateImageFromPath` through the seam: the protected helper that a
    /// backend's `generateImage` calls, with the record's LCD flags.
    // Port of: src/core/SkScalerContext.cpp#L586-L697 (chrome/m156)
    pub fn generate_image_from_path(&self, dst: &mut MaskBuilder, path: &Path, hairline: bool) {
        let do_bgr = self.rec.flags.contains(ScalerContextFlags::LCD_BGR_ORDER);
        let do_vert = self.rec.flags.contains(ScalerContextFlags::LCD_VERTICAL);
        let a8_lcd = self.rec.flags.contains(ScalerContextFlags::GEN_A8_FROM_LCD);
        self.rasterizer.generate_image_from_path(
            dst,
            path,
            &self.pre_blend,
            do_bgr,
            do_vert,
            a8_lcd,
            hairline,
        );
    }
}

/// The virtuals of `SkScalerContext`, implemented by each backend (`generateMetrics`,
/// `generateImage`, `generatePath`, `generateDrawable`, `generateFontMetrics`).
// Port of: src/core/SkScalerContext.h#L410-L449 (chrome/m156)
pub trait ScalerContextImpl: fmt::Debug + Send {
    /// `generateMetrics`: advance, bounds, format and whether a path is needed.
    // Port of: src/core/SkScalerContext.h#L410 (chrome/m156)
    fn generate_metrics(&mut self, glyph: &Glyph, base: &ScalerContextBase) -> GlyphMetrics;

    /// `generateImage`: fills `image`, which is `glyph.imageSize()` bytes and already allocated.
    // Port of: src/core/SkScalerContext.h#L427 (chrome/m156)
    fn generate_image(&mut self, glyph: &Glyph, image: &mut [u8], base: &ScalerContextBase);

    /// `generatePath`: the glyph outline, or `None` if it has none.
    // Port of: src/core/SkScalerContext.h#L436 (chrome/m156)
    fn generate_path(&mut self, glyph: &Glyph, base: &ScalerContextBase) -> Option<GeneratedPath>;

    /// `generateDrawable`: the drawable of the glyph, or `None`.
    // Port of: src/core/SkScalerContext.h#L446 (chrome/m156)
    fn generate_drawable(&mut self, _glyph: &Glyph, _base: &ScalerContextBase) -> Option<Drawable> {
        None
    }

    /// `generateFontMetrics`.
    // Port of: src/core/SkScalerContext.h#L449 (chrome/m156)
    fn generate_font_metrics(&mut self, base: &ScalerContextBase) -> FontMetrics;
}

/// A scaler context (`SkScalerContext`): the non-virtual state plus its backend.
///
/// Owned by its strike and never shared. The strike locks it while it makes glyphs.
// Port of: src/core/SkScalerContext.h#L256-L480 (chrome/m156)
#[doc(alias = "SkScalerContext")]
#[derive(Debug)]
pub struct ScalerContext {
    base: ScalerContextBase,
    imp: Box<dyn ScalerContextImpl>,
}

impl ScalerContext {
    /// The constructor of `SkScalerContext`, with the backend built from the base.
    ///
    /// `make_imp` receives the base the backend's methods will be called with, so a backend can
    /// read the record while it is constructed.
    // Port of: src/core/SkScalerContext.cpp#L92-L109 (chrome/m156)
    pub fn new(
        typeface: Typeface,
        effects: &ScalerContextEffects,
        desc: &Descriptor,
        rasterizer: &'static dyn GlyphPathRasterizer,
        make_imp: impl FnOnce(&ScalerContextBase) -> Box<dyn ScalerContextImpl>,
    ) -> Self {
        let rec = preprocess_rec(&typeface, effects, desc);
        let generate_image_from_path = rec.frame_width >= 0.0 || effects.path_effect.is_some();
        let pre_blend = if effects.mask_filter.is_some() {
            MaskPreBlend::not_applicable()
        } else {
            get_mask_pre_blend(&rec)
        };
        let base = ScalerContextBase {
            rec,
            typeface,
            path_effect: effects.path_effect.clone(),
            mask_filter: effects.mask_filter.clone(),
            generate_image_from_path,
            pre_blend,
            rasterizer,
        };
        let imp = make_imp(&base);
        Self { base, imp }
    }

    /// `SkScalerContext::MakeEmpty`: a context whose glyphs have no metrics, images or paths.
    // Port of: src/core/SkScalerContext.cpp#L1391-L1419 (chrome/m156)
    #[must_use]
    pub fn make_empty(
        typeface: Typeface,
        effects: &ScalerContextEffects,
        desc: &Descriptor,
    ) -> Self {
        #[derive(Debug)]
        struct Empty;
        impl ScalerContextImpl for Empty {
            fn generate_metrics(
                &mut self,
                glyph: &Glyph,
                _base: &ScalerContextBase,
            ) -> GlyphMetrics {
                GlyphMetrics::new(glyph.mask_format())
            }
            fn generate_image(
                &mut self,
                _glyph: &Glyph,
                _image: &mut [u8],
                _base: &ScalerContextBase,
            ) {
            }
            fn generate_path(
                &mut self,
                _glyph: &Glyph,
                _base: &ScalerContextBase,
            ) -> Option<GeneratedPath> {
                None
            }
            fn generate_font_metrics(&mut self, _base: &ScalerContextBase) -> FontMetrics {
                FontMetrics::default()
            }
        }
        Self::new(typeface, effects, desc, &NO_PATH_RASTERIZER, |_| {
            Box::new(Empty)
        })
    }

    /// `SkScalerContext::MakeRecAndEffects`: the record and effects of a font, a paint and
    /// surface properties, as the descriptor is built from them.
    // Port of: src/core/SkScalerContext.cpp#L1177-L1312 (chrome/m156)
    #[must_use]
    #[allow(clippy::too_many_lines)] // mirrors SkScalerContext::MakeRecAndEffects
    pub fn make_rec_and_effects(
        font: &Font,
        paint: &Paint,
        surface_props: &SurfaceProps,
        scaler_context_flags: ScalerContextBuildFlags,
        device_matrix: &Matrix,
    ) -> (ScalerContextRec, ScalerContextEffects) {
        debug_assert!(!device_matrix.has_perspective());
        let mut rec = ScalerContextRec {
            typeface_id: font.typeface().unique_id(),
            text_size: font.size(),
            pre_scale_x: font.scale_x(),
            pre_skew_x: font.skew_x(),
            ..ScalerContextRec::default()
        };

        let mut check_post_2x2 = false;
        let mask = device_matrix.get_type();
        if mask.contains(TypeMask::SCALE) {
            rec.post_2x2[0][0] = sk_relax(device_matrix.scale_x());
            rec.post_2x2[1][1] = sk_relax(device_matrix.scale_y());
            check_post_2x2 = true;
        } else {
            rec.post_2x2[0][0] = 1.0;
            rec.post_2x2[1][1] = 1.0;
        }
        if mask.contains(TypeMask::AFFINE) {
            rec.post_2x2[0][1] = sk_relax(device_matrix.skew_x());
            rec.post_2x2[1][0] = sk_relax(device_matrix.skew_y());
            check_post_2x2 = true;
        } else {
            rec.post_2x2[0][1] = 0.0;
            rec.post_2x2[1][0] = 0.0;
        }

        let style = paint.style();
        let stroke_width = paint.stroke_width();
        let mut flags = ScalerContextFlags::empty();
        if font.is_embolden() {
            flags |= ScalerContextFlags::EMBOLDEN;
        }
        if style != Style::Fill && stroke_width >= 0.0 {
            rec.frame_width = stroke_width;
            rec.miter_limit = paint.stroke_miter();
            rec.set_stroke_join_cap(
                join_to_u8(paint.stroke_join()),
                cap_to_u8(paint.stroke_cap()),
            );
            if style == Style::StrokeAndFill {
                flags |= ScalerContextFlags::FRAME_AND_FILL;
            }
        } else {
            rec.frame_width = -1.0;
            rec.miter_limit = 0.0;
            rec.set_stroke_join_cap(0, 0);
        }

        rec.mask_format = compute_mask_format(font);

        // SDFLCD text never has LCD16 at this point; the subpixel geometry is piped in during
        // drawAtlasSubrun.
        if rec.mask_format == MaskFormat::Lcd16 {
            if too_big_for_lcd(&rec, check_post_2x2) {
                rec.mask_format = MaskFormat::A8;
                flags |= ScalerContextFlags::GEN_A8_FROM_LCD;
            } else {
                match surface_props.pixel_geometry() {
                    // Eeek, can't support LCD.
                    PixelGeometry::Unknown => {
                        rec.mask_format = MaskFormat::A8;
                        flags |= ScalerContextFlags::GEN_A8_FROM_LCD;
                    }
                    // Our default, do nothing.
                    PixelGeometry::RGBH => {}
                    PixelGeometry::BGRH => flags |= ScalerContextFlags::LCD_BGR_ORDER,
                    PixelGeometry::RGBV => flags |= ScalerContextFlags::LCD_VERTICAL,
                    PixelGeometry::BGRV => {
                        flags |= ScalerContextFlags::LCD_VERTICAL;
                        flags |= ScalerContextFlags::LCD_BGR_ORDER;
                    }
                }
            }
        }

        if font.is_embedded_bitmaps() {
            flags |= ScalerContextFlags::EMBEDDED_BITMAP_TEXT;
        }
        if font.is_subpixel() {
            flags |= ScalerContextFlags::SUBPIXEL_POSITIONING;
        }
        if font.is_force_auto_hinting() {
            flags |= ScalerContextFlags::FORCE_AUTOHINTING;
        }
        if font.is_linear_metrics() {
            flags |= ScalerContextFlags::LINEAR_METRICS;
        }
        if font.is_baseline_snap() {
            flags |= ScalerContextFlags::BASELINE_SNAP;
        }
        if font.typeface().glyph_mask_needs_current_color() {
            flags |= ScalerContextFlags::NEEDS_FOREGROUND_COLOR;
            rec.foreground_color = argb_bits(paint.color());
        }

        rec.flags = flags;

        // These modify flags, so they come after assigning flags.
        rec.set_hinting(font.hinting());
        rec.set_luminance_color(compute_luminance_color(paint));

        // The paint color is always converted to the device color space, so the paint gamma is
        // now always equal to the device gamma.
        rec.set_device_gamma(surface_props.text_gamma());
        rec.set_contrast(surface_props.text_contrast());

        if !scaler_context_flags.contains(ScalerContextBuildFlags::FAKE_GAMMA) {
            rec.ignore_gamma();
        }
        if !scaler_context_flags.contains(ScalerContextBuildFlags::BOOST_CONTRAST) {
            rec.set_contrast(0.0);
        }

        let effects = ScalerContextEffects {
            path_effect: paint.path_effect(),
            mask_filter: paint.mask_filter(),
        };
        (rec, effects)
    }

    /// The record (`fRec`).
    #[must_use]
    pub fn rec(&self) -> &ScalerContextRec {
        &self.base.rec
    }

    /// The shared base the backend's methods receive.
    #[must_use]
    pub fn base(&self) -> &ScalerContextBase {
        &self.base
    }

    /// `SkScalerContext::getMaskFormat`.
    // Port of: src/core/SkScalerContext.h#L296-L298 (chrome/m156)
    #[must_use]
    pub fn mask_format(&self) -> MaskFormat {
        self.base.rec.mask_format
    }

    /// `SkScalerContext::isSubpixel`.
    // Port of: src/core/SkScalerContext.h#L299-L301 (chrome/m156)
    #[must_use]
    pub fn is_subpixel(&self) -> bool {
        self.base
            .rec
            .flags
            .contains(ScalerContextFlags::SUBPIXEL_POSITIONING)
    }

    /// `SkScalerContext::computeAxisAlignmentForHText`.
    // Port of: src/core/SkScalerContext.cpp#L1077-L1079 (chrome/m156)
    #[must_use]
    pub fn compute_axis_alignment_for_h_text(&self) -> AxisAlignment {
        self.base.rec.compute_axis_alignment_for_h_text()
    }

    /// `SkScalerContext::makeGlyph`: the metrics of a glyph, without its image.
    // Port of: src/core/SkScalerContext.cpp#L200-L202 (chrome/m156)
    pub fn make_glyph(&mut self, packed_id: PackedGlyphId) -> Glyph {
        self.internal_make_glyph(packed_id, self.base.rec.mask_format)
    }

    /// `SkScalerContext::internalMakeGlyph`.
    // Port of: src/core/SkScalerContext.cpp#L256-L314 (chrome/m156)
    fn internal_make_glyph(&mut self, packed_id: PackedGlyphId, format: MaskFormat) -> Glyph {
        let mut glyph = Glyph::new(packed_id);
        glyph.set_mask_format(format); // subclass may return a different value
        let mut mx = self.imp.generate_metrics(&glyph, &self.base);
        debug_assert!(!mx.never_request_path || !mx.compute_from_path);
        glyph.set_advances(mx.advance.x, mx.advance.y);
        glyph.set_mask_format(mx.mask_format);
        glyph.set_scaler_context_bits(mx.extra_bits);

        if mx.compute_from_path || (self.base.generate_image_from_path && !mx.never_request_path) {
            let generated = mx.generated_path.take();
            self.internal_get_path(&mut glyph, generated);
            if let Some(dev_path) = glyph.path().cloned() {
                let do_vert = self
                    .base
                    .rec
                    .flags
                    .contains(ScalerContextFlags::LCD_VERTICAL);
                let a8_lcd = self
                    .base
                    .rec
                    .flags
                    .contains(ScalerContextFlags::GEN_A8_FROM_LCD);
                let hairline = glyph.path_is_hairline();
                generate_metrics_from_path(&mut glyph, &dev_path, do_vert, a8_lcd, hairline);
            }
        } else {
            saturate_glyph_bounds_rect(&mut glyph, mx.bounds);
            if mx.never_request_path {
                glyph.set_path(None, false, false);
            }
        }

        // If either dimension is empty, zap the image bounds of the glyph.
        if glyph.width == 0 || glyph.height == 0 {
            zero_bounds(&mut glyph);
            return glyph;
        }

        if let Some(mask_filter) = self.base.mask_filter.clone() {
            // Only want the bounds from the filter.
            let src = crate::mask::Mask::new(
                &[],
                glyph.i_rect(),
                row_bytes_u32(&glyph),
                glyph.mask_format(),
            );
            let mut dst = MaskBuilder::default();
            if mask_filter
                .as_base()
                .filter_mask(&mut dst, &src, &self.base.matrix_from_2x2(), None)
            {
                if dst.bounds.is_empty() {
                    zero_bounds(&mut glyph);
                    return glyph;
                }
                saturate_glyph_bounds_irect(&mut glyph, dst.bounds);
                glyph.set_mask_format(dst.format);
            }
        }
        glyph
    }

    /// `SkScalerContext::getImage`: fills `glyph`'s image, which is allocated to
    /// `image_size()` bytes. It is made from the path when the context draws from paths, else
    /// by the backend, and then the mask filter is applied to it.
    ///
    /// C++ filters inside the glyph's arena storage. Here the unfiltered mask is built in an
    /// owned buffer and the intersection is copied into the glyph's image. The bytes are the
    /// same.
    // Port of: src/core/SkScalerContext.cpp#L699-L846 (chrome/m156)
    pub fn get_image(&mut self, glyph: &mut Glyph) {
        let image = glyph
            .take_image()
            .map_or_else(|| vec![0u8; glyph.image_size()], Vec::from);

        let Some(mask_filter) = self.base.mask_filter.clone() else {
            let image = self.generate_unfiltered(glyph, glyph, image);
            glyph.set_image(image.into_boxed_slice());
            return;
        };

        // Need the original bounds, sans our mask filter.
        let saved = self.base.mask_filter.take();
        let tmp = self.make_glyph(glyph.packed_id());
        self.base.mask_filter = saved;

        let tmp_image = self.generate_unfiltered(glyph, &tmp, vec![0u8; tmp.image_size()]);
        let unfiltered = MaskBuilder::new(
            tmp_image,
            tmp.i_rect(),
            row_bytes_u32(&tmp),
            tmp.mask_format(),
        );
        let mut filtered = MaskBuilder::default();
        let src = if mask_filter.as_base().filter_mask(
            &mut filtered,
            &unfiltered.as_mask(),
            &self.base.matrix_from_2x2(),
            None,
        ) {
            filtered
        } else {
            unfiltered
        };
        debug_assert_eq!(src.format, glyph.mask_format());

        let mut dst = MaskBuilder::new(
            image,
            glyph.i_rect(),
            row_bytes_u32(glyph),
            glyph.mask_format(),
        );
        copy_mask_intersection(&src, &mut dst);
        glyph.set_image(dst.image.into_boxed_slice());
    }

    /// Makes the unfiltered mask of `metrics` into `image`: from `orig`'s path when the
    /// context draws from paths and the glyph has one, else by the backend.
    // Port of: src/core/SkScalerContext.cpp#L712-L735 (chrome/m156)
    fn generate_unfiltered(&mut self, orig: &Glyph, metrics: &Glyph, image: Vec<u8>) -> Vec<u8> {
        if let Some(dev_path) = orig.path().filter(|_| self.base.generate_image_from_path) {
            let mut mask = MaskBuilder::new(
                image,
                metrics.i_rect(),
                row_bytes_u32(metrics),
                metrics.mask_format(),
            );
            self.base
                .generate_image_from_path(&mut mask, dev_path, orig.path_is_hairline());
            return mask.image;
        }
        let mut image = image;
        self.imp.generate_image(metrics, &mut image, &self.base);
        image
    }

    /// `SkScalerContext::getPath`.
    // Port of: src/core/SkScalerContext.cpp#L848-L850 (chrome/m156)
    pub fn get_path(&mut self, glyph: &mut Glyph) {
        self.internal_get_path(glyph, None);
    }

    /// `SkScalerContext::getDrawable`.
    // Port of: src/core/SkScalerContext.cpp#L852-L854 (chrome/m156)
    pub fn get_drawable(&mut self, glyph: &Glyph) -> Option<Drawable> {
        self.imp.generate_drawable(glyph, &self.base)
    }

    /// `SkScalerContext::getFontMetrics`.
    // Port of: src/core/SkScalerContext.cpp#L860-L864 (chrome/m156)
    pub fn get_font_metrics(&mut self) -> FontMetrics {
        self.imp.generate_font_metrics(&self.base)
    }

    /// `SkScalerContext::internalGetPath`: the glyph's path, with the fake bold stroke, the path
    /// effect and the stroke rec applied.
    // Port of: src/core/SkScalerContext.cpp#L867-L947 (chrome/m156)
    fn internal_get_path(&mut self, glyph: &mut Glyph, generated: Option<GeneratedPath>) {
        if glyph.set_path_has_been_called() {
            return;
        }
        let generated = generated.or_else(|| self.imp.generate_path(glyph, &self.base));
        let Some(generated) = generated else {
            glyph.set_path(None, false, false);
            return;
        };
        let mut path = generated.path;
        let mut path_modified = generated.modified;

        if self
            .base
            .rec
            .flags
            .contains(ScalerContextFlags::SUBPIXEL_POSITIONING)
        {
            let dx = glyph.packed_id().sub_x_fixed();
            let dy = glyph.packed_id().sub_y_fixed();
            if (dx | dy) != 0 {
                path_modified = true;
                path = path.make_offset(Point::new(
                    crate::fixed::fixed_to_scalar(dx),
                    crate::fixed::fixed_to_scalar(dy),
                ));
            }
        }

        if self.base.rec.frame_width < 0.0 && self.base.path_effect.is_none() {
            glyph.set_path(Some(path), false, path_modified);
            return;
        }

        // It could still end up the same, but it's probably going to change.
        path_modified = true;

        // Need the path in user space, with only the point size applied, so that stroking and
        // effects operate as they would if the user had extracted the path and called drawPath.
        let matrix = self.base.rec.get_matrix_from_2x2();
        // We apply the inverse, so that localPath is only affected by the paint settings and not
        // the canvas matrix.
        let Some(inverse) = matrix.invert() else {
            glyph.set_path(Some(Path::new()), false, path_modified);
            return;
        };
        let mut local_path = path.make_transform(&inverse);

        let mut rec = StrokeRec::new_fill();
        if self.base.rec.frame_width >= 0.0 {
            rec.set_stroke_style(
                self.base.rec.frame_width,
                self.base
                    .rec
                    .flags
                    .contains(ScalerContextFlags::FRAME_AND_FILL),
            );
            // Glyphs are always closed contours, so the cap is ignored; pass something.
            rec.set_stroke_params(
                cap_from_u8(self.base.rec.stroke_cap()),
                join_from_u8(self.base.rec.stroke_join()),
                self.base.rec.miter_limit,
            );
        }

        // `SkPathEffect::filterPath` updates `rec` in place even when it returns false, so the
        // in-place variant is used rather than `filter_path`'s `Option`.
        if let Some(path_effect) = self.base.path_effect.as_ref() {
            let mut builder = PathBuilder::new();
            if path_effect.filter_path_inplace_with_matrix(
                &mut builder,
                &local_path,
                &mut rec,
                None,
                &matrix,
            ) {
                local_path = builder.detach();
            }
        }

        if rec.need_to_apply() {
            let mut builder = PathBuilder::new();
            if rec.apply_to_path(&mut builder, &local_path) {
                local_path = builder.detach();
            }
        }

        let dev_path = local_path.make_transform(&matrix);
        glyph.set_path(Some(dev_path), rec.is_hairline_style(), path_modified);
    }
}

/// `SkScalerContext::PreprocessRec`: the record of the descriptor, adjusted by the typeface and
/// by the mask filter.
// Port of: src/core/SkScalerContext.cpp#L60-L90 (chrome/m156)
fn preprocess_rec(
    typeface: &Typeface,
    effects: &ScalerContextEffects,
    desc: &Descriptor,
) -> ScalerContextRec {
    let entry = desc
        .find_entry(REC_TAG)
        .expect("a descriptor has a record entry");
    let bytes: &[u8; crate::scaler_context::SCALER_CONTEXT_REC_SIZE] = entry
        .try_into()
        .expect("the record entry has the record size");
    let mut rec = ScalerContextRec::from_bytes(bytes);

    // Allow the typeface to adjust the rec.
    typeface.filter_rec(&mut rec);

    if effects.mask_filter.is_some() {
        // Pre-blend is not currently applied to filtered text. The primary filter is blur, for
        // which contrast makes no sense, and for which the destination guess error is more
        // visible. Also, all existing users of blur have calibrated for linear.
        rec.ignore_pre_blend();
    }

    let mut lum_color = rec.luminance_color();
    if rec.mask_format == MaskFormat::A8 {
        // SkComputeLuminance returns at most 255.
        #[allow(clippy::cast_possible_truncation)]
        let lum = compute_luminance(
            u32::from(lum_color.r()),
            u32::from(lum_color.g()),
            u32::from(lum_color.b()),
        ) as u8;
        lum_color = Color::from_rgb(lum, lum, lum);
    }
    // TODO in C++: remove CanonicalColor when we fix up Chrome layout tests.
    rec.set_luminance_color(lum_color);
    rec
}

/// `SkScalerContext::GetMaskPreBlend`: the pre-blend of a record's gamma and luminance.
// Port of: src/core/SkScalerContext.cpp#L167-L174 (chrome/m156)
#[must_use]
pub fn get_mask_pre_blend(rec: &ScalerContextRec) -> MaskPreBlend {
    let mask_gamma = rec.cached_mask_gamma();
    mask_gamma.pre_blend(rec.luminance_color())
}

/// `SkScalerContext::GetGammaLUTSize`: the table width and height, and the size in bytes.
// Port of: src/core/SkScalerContext.cpp#L176-L184 (chrome/m156)
#[must_use]
pub fn get_gamma_lut_size() -> (usize, usize, usize) {
    let (width, height) = MaskGamma::gamma_table_dimensions();
    (width, height, MaskGamma::gamma_table_size_in_bytes())
}

/// `SkScalerContext::GetGammaLUTData`: the gamma tables for a contrast and gamma, or `None` for
/// the linear case, which has no tables.
// Port of: src/core/SkScalerContext.cpp#L186-L198 (chrome/m156)
#[must_use]
pub fn get_gamma_lut_data(contrast: scalar, device_gamma: scalar) -> Option<Vec<u8>> {
    let mask_gamma = cached_mask_gamma_for(
        ScalerContextRec::internal_contrast_from_external(contrast),
        ScalerContextRec::internal_gamma_from_external(device_gamma),
    );
    mask_gamma.gamma_tables().map(<[u8]>::to_vec)
}

/// `SkScalerContextRec::CachedMaskGamma(contrast, gamma)`: the default and linear gammas are
/// built once; any other value is built for the caller. The tables are a pure function of
/// their inputs, so this matches C++'s single-entry cache in its results.
// Port of: src/core/SkScalerContext.cpp#L137-L162 (chrome/m156)
#[must_use]
pub fn cached_mask_gamma_for(contrast: u8, gamma: u8) -> Arc<MaskGamma> {
    static LINEAR: LazyLock<Arc<MaskGamma>> = LazyLock::new(|| Arc::new(MaskGamma::linear()));
    static DEFAULT: LazyLock<Arc<MaskGamma>> =
        LazyLock::new(|| Arc::new(MaskGamma::new(SK_GAMMA_CONTRAST, SK_GAMMA_EXPONENT)));
    let linear_contrast = ScalerContextRec::internal_contrast_from_external(0.0);
    let linear_gamma = ScalerContextRec::internal_gamma_from_external(1.0);
    if contrast == linear_contrast && gamma == linear_gamma {
        return Arc::clone(&LINEAR);
    }
    let default_contrast = ScalerContextRec::internal_contrast_from_external(SK_GAMMA_CONTRAST);
    let default_gamma = ScalerContextRec::internal_gamma_from_external(SK_GAMMA_EXPONENT);
    if contrast == default_contrast && gamma == default_gamma {
        return Arc::clone(&DEFAULT);
    }
    Arc::new(MaskGamma::new(
        ScalerContextRec::external_contrast_from_internal(contrast),
        ScalerContextRec::external_gamma_from_internal(gamma),
    ))
}

/// `SkScalerContext::makeGlyph`'s saturating bounds from a float rect (`SaturateGlyphBounds`).
// Port of: src/core/SkScalerContext.cpp#L211-L216 (chrome/m156)
fn saturate_glyph_bounds_rect(glyph: &mut Glyph, rect: Rect) {
    let r: IRect = rect.round_out();
    glyph.left = saturate_i16(r.left);
    glyph.top = saturate_i16(r.top);
    glyph.width = saturate_u16_from_i64(i64::from(r.width()));
    glyph.height = saturate_u16_from_i64(i64::from(r.height()));
}

/// `SkScalerContext::SaturateGlyphBounds(SkGlyph*, const SkIRect&)`.
// Port of: src/core/SkScalerContext.cpp#L218-L223 (chrome/m156)
fn saturate_glyph_bounds_irect(glyph: &mut Glyph, r: IRect) {
    glyph.left = saturate_i16(r.left);
    glyph.top = saturate_i16(r.top);
    glyph.width = saturate_u16_from_i64(r.width_64());
    glyph.height = saturate_u16_from_i64(r.height_64());
}

/// `sk_saturate_cast<int16_t>(float)`: clamps (NaN goes to the maximum) and truncates.
// Port of: src/core/SkScalerContext.cpp#L205-L209 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // clamped to the range first
fn saturate_i16<T: SaturateFrom>(value: T) -> i16 {
    value.saturate_i16()
}

/// `sk_saturate_cast<uint16_t>(int)`.
// Port of: src/core/SkScalerContext.cpp#L205-L209 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to the range first
fn saturate_u16_from_i64(value: i64) -> u16 {
    value.clamp(i64::from(u16::MIN), i64::from(u16::MAX)) as u16
}

/// The source types of [`saturate_i16`].
trait SaturateFrom {
    fn saturate_i16(self) -> i16;
}

impl SaturateFrom for scalar {
    #[allow(clippy::cast_possible_truncation)] // clamped to the range first
    fn saturate_i16(self) -> i16 {
        let s = if self < f32::from(i16::MAX) {
            self
        } else {
            f32::from(i16::MAX)
        };
        let s = if s > f32::from(i16::MIN) {
            s
        } else {
            f32::from(i16::MIN)
        };
        s as i16
    }
}

impl SaturateFrom for i32 {
    #[allow(clippy::cast_possible_truncation)] // clamped to the range first
    fn saturate_i16(self) -> i16 {
        i64::from(self).clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16
    }
}

/// `SkScalerContext::GenerateMetricsFromPath`: the bounds of a glyph drawn from a path, grown by
/// one pixel in each LCD direction, and the format a path can produce.
// Port of: src/core/SkScalerContext.cpp#L225-L254 (chrome/m156)
fn generate_metrics_from_path(
    glyph: &mut Glyph,
    dev_path: &Path,
    vertical_lcd: bool,
    a8_from_lcd: bool,
    hairline: bool,
) {
    // Only BW, A8, and LCD16 can be produced from paths.
    if glyph.mask_format() != MaskFormat::BW
        && glyph.mask_format() != MaskFormat::A8
        && glyph.mask_format() != MaskFormat::Lcd16
    {
        glyph.set_mask_format(MaskFormat::A8);
    }
    let mut bounds = *dev_path.bounds();
    if !bounds.is_empty() {
        let from_lcd = glyph.mask_format() == MaskFormat::Lcd16
            || (glyph.mask_format() == MaskFormat::A8 && a8_from_lcd);
        let need_extra_width = (from_lcd && !vertical_lcd) || hairline;
        let need_extra_height = (from_lcd && vertical_lcd) || hairline;
        if need_extra_width {
            bounds = bounds.round_out();
            bounds.outset(Vector::new(1.0, 0.0));
        }
        if need_extra_height {
            bounds = bounds.round_out();
            bounds.outset(Vector::new(0.0, 1.0));
        }
    }
    saturate_glyph_bounds_rect(glyph, bounds);
}

/// `zeroBounds`: zeroes the image bounds of a glyph.
// Port of: src/core/SkScalerContext.cpp#L278-L283 (chrome/m156)
fn zero_bounds(glyph: &mut Glyph) {
    glyph.left = 0;
    glyph.top = 0;
    glyph.width = 0;
    glyph.height = 0;
}

/// The copy at the end of `getImage` after a mask filter: the intersection of the filtered mask
/// and the glyph's mask is copied, and the glyph's image is cleared first when the intersection
/// is smaller than the glyph.
// Port of: src/core/SkScalerContext.cpp#L821-L845 (chrome/m156)
fn copy_mask_intersection(src: &MaskBuilder, dst: &mut MaskBuilder) {
    let orig = dst.bounds;
    let inter = IRect::from_ltrb(
        src.bounds.left.max(dst.bounds.left),
        src.bounds.top.max(dst.bounds.top),
        src.bounds.right.min(dst.bounds.right),
        src.bounds.bottom.min(dst.bounds.bottom),
    );
    // If not filling the full original glyph, clear it out first.
    if inter != orig {
        dst.image.iter_mut().for_each(|b| *b = 0);
    }
    let width = usize::try_from(inter.width().max(0)).unwrap_or(0);
    let height = usize::try_from(inter.height().max(0)).unwrap_or(0);
    let src_rb = src.row_bytes as usize;
    let dst_rb = dst.row_bytes as usize;
    let src_off = usize::try_from(inter.top - src.bounds.top).unwrap_or(0) * src_rb
        + usize::try_from(inter.left - src.bounds.left).unwrap_or(0);
    let dst_off = usize::try_from(inter.top - dst.bounds.top).unwrap_or(0) * dst_rb
        + usize::try_from(inter.left - dst.bounds.left).unwrap_or(0);
    for row in 0..height {
        let s = src_off + row * src_rb;
        let d = dst_off + row * dst_rb;
        dst.image[d..d + width].copy_from_slice(&src.image[s..s + width]);
    }
}

/// `SkScalerContext::computeMaskFormat` (`compute_mask_format`).
// Port of: src/core/SkScalerContext.cpp#L1143-L1155 (chrome/m156)
fn compute_mask_format(font: &Font) -> MaskFormat {
    match font.edging() {
        Edging::Alias => MaskFormat::BW,
        Edging::AntiAlias => MaskFormat::A8,
        Edging::SubpixelAntiAlias => MaskFormat::Lcd16,
    }
}

/// `too_big_for_lcd`: LCD does not improve quality beyond a size, and costs more.
// Port of: src/core/SkScalerContext.cpp#L1164-L1175 (chrome/m156)
fn too_big_for_lcd(rec: &ScalerContextRec, check_post_2x2: bool) -> bool {
    if check_post_2x2 {
        let mut area =
            rec.post_2x2[0][0] * rec.post_2x2[1][1] - rec.post_2x2[1][0] * rec.post_2x2[0][1];
        area *= rec.text_size * rec.text_size;
        area > SK_MAX_SIZE_FOR_LCDTEXT * SK_MAX_SIZE_FOR_LCDTEXT
    } else {
        rec.text_size > SK_MAX_SIZE_FOR_LCDTEXT
    }
}

/// `sk_relax`: the scalar at 1/1024 precision, so that tiny matrix changes share a cache key.
// Port of: src/core/SkScalerContext.cpp#L1138-L1141 (chrome/m156)
fn sk_relax(x: scalar) -> scalar {
    let n = scalar_round_to_scalar(x * 1024.0);
    n / 1024.0
}

/// `SkFontPriv::MakeTextMatrix`: scale by size (and scale x), then skew x.
// Port of: src/core/SkFontPriv.h#L41-L47 (chrome/m156)
#[must_use]
pub fn make_text_matrix(size: scalar, scale_x: scalar, skew_x: scalar) -> Matrix {
    let mut m = Matrix::scale((size * scale_x, size));
    if skew_x != 0.0 {
        m.post_skew((skew_x, 0.0), None);
    }
    m
}

/// `SkPaint::Cap` from a record's stroke cap byte.
// Port of: include/core/SkPaint.h (Cap values, chrome/m156)
fn cap_from_u8(value: u8) -> Cap {
    match value {
        1 => Cap::Round,
        2 => Cap::Square,
        _ => Cap::Butt,
    }
}

/// `SkPaint::Join` from a record's stroke join byte.
// Port of: include/core/SkPaint.h (Join values, chrome/m156)
fn join_from_u8(value: u8) -> Join {
    match value {
        1 => Join::Round,
        2 => Join::Bevel,
        _ => Join::Miter,
    }
}

/// A `SkPaint::Cap` as the byte the record stores.
// Port of: include/core/SkPaint.h (Cap values, chrome/m156)
fn cap_to_u8(cap: Cap) -> u8 {
    match cap {
        Cap::Butt => 0,
        Cap::Round => 1,
        Cap::Square => 2,
    }
}

/// A `SkPaint::Join` as the byte the record stores.
// Port of: include/core/SkPaint.h (Join values, chrome/m156)
fn join_to_u8(join: Join) -> u8 {
    match join {
        Join::Miter => 0,
        Join::Round => 1,
        Join::Bevel => 2,
    }
}

impl ScalerContextRec {
    /// `SkScalerContextRec::setLuminanceColor`: stores the canonical form of the color's RGB.
    // Port of: src/core/SkScalerContext.cpp#L1104-L1107 (chrome/m156)
    pub fn set_luminance_color(&mut self, color: Color) {
        let canonical =
            MaskGamma::canonical_color(Color::from_rgb(color.r(), color.g(), color.b()));
        self.lum_bits = argb_bits(canonical);
    }

    /// `SkScalerContextRec::ignoreGamma`: no luminance color, and a device gamma of 1.
    // Port of: src/core/SkScalerContext.h#L126-L129 (chrome/m156)
    pub fn ignore_gamma(&mut self) {
        self.set_luminance_color(Color::from_argb(0, 0, 0, 0));
        self.set_device_gamma(1.0);
    }

    /// `SkScalerContextRec::ignorePreBlend`: `ignoreGamma` and no contrast.
    // Port of: src/core/SkScalerContext.h#L135-L138 (chrome/m156)
    pub fn ignore_pre_blend(&mut self) {
        self.ignore_gamma();
        self.set_contrast(0.0);
    }

    /// `SkScalerContextRec::cachedMaskGamma`: the gamma of this record's contrast and gamma.
    // Port of: src/core/SkScalerContext.h#L118-L120 (chrome/m156)
    #[must_use]
    pub fn cached_mask_gamma(&self) -> Arc<MaskGamma> {
        cached_mask_gamma_for(self.contrast, self.device_gamma)
    }

    /// `SkScalerContextRec::useStrokeForFakeBold`: moves the embolden into the stroke.
    // Port of: src/core/SkScalerContext.cpp#L1109-L1135 (chrome/m156)
    pub fn use_stroke_for_fake_bold(&mut self) {
        if !self.flags.contains(ScalerContextFlags::EMBOLDEN) {
            return;
        }
        self.flags.remove(ScalerContextFlags::EMBOLDEN);
        let fake_bold_scale = crate::scalar::float_interp_func(
            self.text_size,
            &STD_FAKE_BOLD_INTERP_KEYS,
            &STD_FAKE_BOLD_INTERP_VALUES,
        );
        let extra = self.text_size * fake_bold_scale;
        if self.frame_width >= 0.0 {
            self.frame_width += extra;
        } else {
            self.flags |= ScalerContextFlags::FRAME_AND_FILL;
            self.frame_width = extra;
            // The default paint: miter limit 4, miter join, butt cap.
            self.miter_limit = PAINT_DEFAULT_MITER_LIMIT;
            self.set_stroke_join_cap(join_to_u8(Join::Miter), cap_to_u8(Cap::Butt));
        }
    }

    /// `SkScalerContextRec::getMatrixFrom2x2`.
    // Port of: src/core/SkScalerContext.cpp#L950-L953 (chrome/m156)
    #[must_use]
    pub fn get_matrix_from_2x2(&self) -> Matrix {
        Matrix::new_all(
            self.post_2x2[0][0],
            self.post_2x2[0][1],
            0.0,
            self.post_2x2[1][0],
            self.post_2x2[1][1],
            0.0,
            0.0,
            0.0,
            1.0,
        )
    }

    /// `SkScalerContextRec::getLocalMatrix`: `SkFontPriv::MakeTextMatrix` of the size, scale and
    /// skew.
    // Port of: src/core/SkScalerContext.cpp#L955-L958 (chrome/m156)
    #[must_use]
    pub fn get_local_matrix(&self) -> Matrix {
        make_text_matrix(self.text_size, self.pre_scale_x, self.pre_skew_x)
    }

    /// `SkScalerContextRec::getSingleMatrix`: the local matrix, then the 2x2.
    // Port of: src/core/SkScalerContext.cpp#L960-L962 (chrome/m156)
    #[must_use]
    pub fn get_single_matrix(&self) -> Matrix {
        let mut m = self.get_local_matrix();
        m.post_concat(&self.get_matrix_from_2x2());
        m
    }

    /// `SkScalerContextRec::computeAxisAlignmentForHText`.
    // Port of: src/core/SkScalerContext.cpp#L1081-L1102 (chrome/m156)
    #[must_use]
    pub fn compute_axis_alignment_for_h_text(&self) -> AxisAlignment {
        if !self.flags.contains(ScalerContextFlags::BASELINE_SNAP) {
            return AxisAlignment::None;
        }
        if self.post_2x2[1][0] == 0.0 {
            // The x axis is mapped onto the x axis.
            return AxisAlignment::X;
        }
        if self.post_2x2[0][0] == 0.0 {
            // The x axis is mapped onto the y axis.
            return AxisAlignment::Y;
        }
        AxisAlignment::None
    }

    /// `SkScalerContextRec::computeMatrices`: splits the total matrix into a pre scale and the
    /// remaining matrices. `remaining_rotation` is G inverse. Returns `false` for a singular or
    /// non-finite total matrix; the matrices are then zero (and `scale` is 1).
    // Port of: src/core/SkScalerContext.cpp#L964-L1075 (chrome/m156)
    #[allow(clippy::float_cmp)] // exact comparisons, as the C++ tests them
    pub fn compute_matrices(
        &self,
        pre_matrix_scale: PreMatrixScale,
        scale: &mut Vector,
        remaining: &mut Matrix,
        remaining_without_rotation: Option<&mut Matrix>,
        remaining_rotation: Option<&mut Matrix>,
        total: Option<&mut Matrix>,
    ) -> bool {
        // A is the 'total' matrix.
        let a = self.get_single_matrix();
        // The caller may find the 'total' matrix useful when dealing directly with EM sizes.
        if let Some(total) = total {
            *total = a.clone();
        }

        // GA is the matrix A with rotation removed.
        let skewed_or_flipped =
            a.skew_x() != 0.0 || a.skew_y() != 0.0 || a.scale_x() < 0.0 || a.scale_y() < 0.0;
        let (ga, g_inv) = if skewed_or_flipped {
            // QR by Givens rotations. G is Q^T and GA is R. G is rotational (no reflections).
            // h is where A maps the horizontal baseline.
            let h = a.map_point(Point::new(1.0, 0.0));
            // G is the Givens Matrix for A (rotational matrix where GA[0][1] == 0).
            let g = compute_givens_rotation(h);
            let mut ga = g.clone();
            ga.pre_concat(&a);
            // The 'remainingRotation' is G inverse, which is fairly simple since G is 2x2 rotational.
            let g_inv = Matrix::new_all(
                g.get(0usize),
                -g.get(1usize),
                g.get(2usize),
                -g.get(3usize),
                g.get(4usize),
                g.get(5usize),
                g.get(6usize),
                g.get(7usize),
                g.get(8usize),
            );
            (ga, g_inv)
        } else {
            (a.clone(), Matrix::new_identity())
        };

        // If the 'total' matrix is singular, set the 'scale' to something finite and zero the
        // matrices. All underlying ports have issues with zero text size, so use the matrices to
        // zero. If one of the scale factors is less than 1/256, an EM filling square never affects
        // any pixels. If there are any nonfinite numbers, bail out too.
        if ga.scale_x().abs() <= SK_SCALAR_NEARLY_ZERO
            || ga.scale_y().abs() <= SK_SCALAR_NEARLY_ZERO
            || !ga.is_finite()
        {
            *scale = Vector::new(1.0, 1.0);
            *remaining = Matrix::scale((0.0, 0.0));
            if let Some(without_rotation) = remaining_without_rotation {
                *without_rotation = Matrix::scale((0.0, 0.0));
            }
            if let Some(rotation) = remaining_rotation {
                *rotation = Matrix::new_identity();
            }
            return false;
        }

        // At this point, given GA, create s.
        match pre_matrix_scale {
            PreMatrixScale::Full => {
                scale.x = ga.scale_x().abs();
                scale.y = ga.scale_y().abs();
            }
            PreMatrixScale::Vertical => {
                let y_scale = ga.scale_y().abs();
                scale.x = y_scale;
                scale.y = y_scale;
            }
            PreMatrixScale::VerticalInteger => {
                let real_y_scale = ga.scale_y().abs();
                let mut int_y_scale = scalar_round_to_scalar(real_y_scale);
                if int_y_scale == 0.0 {
                    int_y_scale = 1.0;
                }
                scale.x = int_y_scale;
                scale.y = int_y_scale;
            }
        }

        // The 'remaining' matrix sA is the total matrix A without the scale.
        if !skewed_or_flipped
            && (pre_matrix_scale == PreMatrixScale::Full
                || (pre_matrix_scale == PreMatrixScale::Vertical && a.scale_x() == a.scale_y()))
        {
            // If GA == A and kFull, sA is identity. If GA == A and kVertical and A.scaleX ==
            // A.scaleY, sA is identity.
            *remaining = Matrix::new_identity();
        } else if !skewed_or_flipped && pre_matrix_scale == PreMatrixScale::Vertical {
            // If GA == A and kVertical, sA.scaleY is 1.
            *remaining = Matrix::new_identity();
            remaining.set_scale_x(a.scale_x() / scale.y);
        } else {
            // TODO in C++: like kVertical, kVerticalInteger with int scales.
            *remaining = a.clone();
            remaining.pre_scale((1.0 / scale.x, 1.0 / scale.y), None);
        }

        // The 'remainingWithoutRotation' matrix GsA is the non-rotational part of A without the
        // scale. G is rotational so reorders with the scale.
        if let Some(without_rotation) = remaining_without_rotation {
            *without_rotation = ga;
            // G is rotational so reorders with the scale.
            without_rotation.pre_scale((1.0 / scale.x, 1.0 / scale.y), None);
        }
        if let Some(rotation) = remaining_rotation {
            *rotation = g_inv;
        }
        true
    }
}

/// Packs a color as `0xAARRGGBB` (`SkColor`).
fn argb_bits(c: Color) -> u32 {
    u32::from_be_bytes([c.a(), c.r(), c.g(), c.b()])
}

/// `SkGlyph::rowBytes` as the `u32` a mask carries.
fn row_bytes_u32(glyph: &Glyph) -> u32 {
    u32::try_from(glyph.row_bytes()).expect("a glyph's row bytes fit in a mask's u32")
}
