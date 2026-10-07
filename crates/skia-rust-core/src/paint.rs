// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPaint.h, src/core/SkPaint.cpp, src/core/SkPaintDefaults.h

//! `SkPaint`: the options applied when drawing ([`Paint`]), its stroke enums ([`Cap`],
//! [`Join`], [`Style`]) and defaults.

use crate::alpha_type::AlphaType;
use crate::blend_mode::BlendMode;
use crate::blender::Blender;
use crate::color::{Color, Color4f};
use crate::color_filter::ColorFilter;
use crate::color_space::ColorSpace;
use crate::color_space_priv::srgb_singleton;
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::floating_point::float_round2int;
use crate::image_filter::ImageFilter;
use crate::mask_filter::MaskFilter;
use crate::path_effect::PathEffect;
use crate::rect::Rect;
use crate::scalar::scalar;
use crate::shader::Shader;
use crate::stroke_rec::StrokeRec;
use crate::t_pin::t_pin;

/// The default miter limit (`SkPaintDefaults_MiterLimit`).
// Port of: src/core/SkPaintDefaults.h#L27-L29 (chrome/m156)
#[doc(alias = "SkPaintDefaults_MiterLimit")]
pub const DEFAULT_MITER_LIMIT: scalar = 4.0;

/// How the ends of an open contour are drawn (`SkPaint::Cap`).
// Port of: include/core/SkPaint.h#L336-L342 (chrome/m156)
#[doc(alias = "SkPaint::Cap")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Cap {
    /// No stroke extension (`kButt_Cap`).
    #[default]
    Butt = 0,
    /// Adds a circle (`kRound_Cap`).
    Round = 1,
    /// Adds a square (`kSquare_Cap`).
    Square = 2,
}

impl Cap {
    /// The number of caps (`SkPaint::kCapCount`).
    #[doc(alias = "kCapCount")]
    pub const COUNT: usize = 3;
    /// The largest cap (`SkPaint::kLast_Cap`).
    #[doc(alias = "kLast_Cap")]
    pub const LAST: Cap = Cap::Square;
    /// `kDefault_Cap`.
    #[doc(alias = "kDefault_Cap")]
    pub const DEFAULT: Cap = Cap::Butt;
}

/// How corners between segments are drawn (`SkPaint::Join`).
// Port of: include/core/SkPaint.h#L361-L367 (chrome/m156)
#[doc(alias = "SkPaint::Join")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Join {
    /// Extends to the miter limit (`kMiter_Join`).
    #[default]
    Miter = 0,
    /// Adds a circle (`kRound_Join`).
    Round = 1,
    /// Connects the outside edges (`kBevel_Join`).
    Bevel = 2,
}

impl Join {
    /// The number of joins (`SkPaint::kJoinCount`).
    #[doc(alias = "kJoinCount")]
    pub const COUNT: usize = 3;
    /// The largest join (`SkPaint::kLast_Join`).
    #[doc(alias = "kLast_Join")]
    pub const LAST: Join = Join::Bevel;
    /// `kDefault_Join`.
    #[doc(alias = "kDefault_Join")]
    pub const DEFAULT: Join = Join::Miter;
}

/// Whether geometry is filled, stroked or both (`SkPaint::Style`).
// Port of: include/core/SkPaint.h#L168-L173 (chrome/m156)
#[doc(alias = "SkPaint::Style")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Style {
    /// Fills the geometry (`kFill_Style`).
    #[default]
    Fill = 0,
    /// Strokes the geometry (`kStroke_Style`).
    Stroke = 1,
    /// Fills and strokes the geometry (`kStrokeAndFill_Style`).
    StrokeAndFill = 2,
}

impl Style {
    /// The number of styles (`SkPaint::kStyleCount`).
    // Port of: include/core/SkPaint.h#L200 (chrome/m156)
    #[doc(alias = "kStyleCount")]
    pub const COUNT: usize = 3;
}

/// The options applied when drawing (`SkPaint`): everything outside the canvas clip and matrix.
///
/// Various options apply to strokes and fills, and images. A paint collects the effects and
/// filters that describe single-pass and multiple-pass algorithms that alter the drawing
/// geometry, color, and transparency; it does not implement dashing or blur itself, but holds
/// the objects that do.
///
/// Cloning is shallow: the effects ([`PathEffect`], [`Shader`], [`MaskFilter`],
/// [`ColorFilter`], [`ImageFilter`], [`Blender`]) are shared. Equality is Skia's: the effects
/// are compared by identity, the color and stroke values with `==`.
// Port of: include/core/SkPaint.h#L44-L696 (chrome/m156)
#[doc(alias = "SkPaint")]
#[derive(Clone, Debug)]
pub struct Paint {
    path_effect: Option<PathEffect>,
    shader: Option<Shader>,
    mask_filter: Option<MaskFilter>,
    color_filter: Option<ColorFilter>,
    image_filter: Option<ImageFilter>,
    blender: Option<Blender>,
    color4f: Color4f,
    width: scalar,
    miter_limit: scalar,
    // `fBitfields`
    anti_alias: bool,
    dither: bool,
    cap: Cap,
    join: Join,
    style: Style,
}

impl Default for Paint {
    /// A paint with default values: opaque black, fill, hairline width, the default miter
    /// limit, cap and join, no antialiasing, dithering or effects (`SkPaint()`).
    // Port of: src/core/SkPaint.cpp#L38-L50 (chrome/m156)
    fn default() -> Paint {
        Paint {
            path_effect: None,
            shader: None,
            mask_filter: None,
            color_filter: None,
            image_filter: None,
            blender: None,
            color4f: Color4f::new(0.0, 0.0, 0.0, 1.0), // opaque black
            width: 0.0,
            miter_limit: DEFAULT_MITER_LIMIT,
            anti_alias: false,
            dither: false,
            cap: Cap::DEFAULT,
            join: Join::DEFAULT,
            style: Style::Fill,
        }
    }
}

impl PartialEq for Paint {
    /// Skia's `operator==`: the effects by identity (equal effects at different addresses
    /// differ), the color, stroke width, miter limit and flags by value.
    // Port of: src/core/SkPaint.cpp#L66-L80 (chrome/m156)
    #[allow(clippy::float_cmp)] // Skia compares the floats with ==
    fn eq(&self, b: &Paint) -> bool {
        let a = self;
        a.path_effect == b.path_effect
            && a.shader == b.shader
            && a.mask_filter == b.mask_filter
            && a.color_filter == b.color_filter
            && a.blender == b.blender
            && a.image_filter == b.image_filter
            && a.color4f == b.color4f
            && a.width == b.width
            && a.miter_limit == b.miter_limit
            // fBitfieldsUInt
            && a.anti_alias == b.anti_alias
            && a.dither == b.dither
            && a.cap == b.cap
            && a.join == b.join
            && a.style == b.style
    }
}

impl Paint {
    /// A paint with default values and the given color: unpremultiplied RGBA in `color_space`
    /// (sRGB if `None`) (`SkPaint(const SkColor4f&, SkColorSpace*)`).
    // Port of: src/core/SkPaint.cpp#L52-L54 (chrome/m156)
    #[must_use]
    pub fn new<'a>(
        color: impl AsRef<Color4f>,
        color_space: impl Into<Option<&'a ColorSpace>>,
    ) -> Paint {
        let mut paint = Paint::default();
        paint.set_color4f(color, color_space);
        paint
    }

    /// Sets all contents to their initial values, as `Paint::default()` (`reset`).
    // Port of: src/core/SkPaint.cpp#L103 (chrome/m156)
    pub fn reset(&mut self) -> &mut Self {
        *self = Paint::default();
        self
    }

    /// True if pixels on the active edges of a path may be drawn with partial transparency
    /// (`isAntiAlias`).
    #[doc(alias = "isAntiAlias")]
    #[must_use]
    pub fn is_anti_alias(&self) -> bool {
        self.anti_alias
    }

    /// Requests, but does not require, that edge pixels draw opaque or with partial
    /// transparency (`setAntiAlias`).
    #[doc(alias = "setAntiAlias")]
    pub fn set_anti_alias(&mut self, anti_alias: bool) -> &mut Self {
        self.anti_alias = anti_alias;
        self
    }

    /// True if color error may be distributed to smooth color transition (`isDither`).
    #[doc(alias = "isDither")]
    #[must_use]
    pub fn is_dither(&self) -> bool {
        self.dither
    }

    /// Requests, but does not require, to distribute color error (`setDither`).
    #[doc(alias = "setDither")]
    pub fn set_dither(&mut self, dither: bool) -> &mut Self {
        self.dither = dither;
        self
    }

    /// Whether the geometry is filled, stroked, or filled and stroked (`getStyle`).
    #[doc(alias = "getStyle")]
    #[must_use]
    pub fn style(&self) -> Style {
        self.style
    }

    /// Sets whether the geometry is filled, stroked, or filled and stroked (`setStyle`). (Skia
    /// ignores out-of-range values, which a Rust enum cannot hold.)
    // Port of: src/core/SkPaint.cpp#L105-L113 (chrome/m156)
    #[doc(alias = "setStyle")]
    pub fn set_style(&mut self, style: Style) -> &mut Self {
        self.style = style;
        self
    }

    /// Sets the style to [`Style::Stroke`] if `stroke`, else [`Style::Fill`] (`setStroke`).
    // Port of: src/core/SkPaint.cpp#L115-L117 (chrome/m156)
    #[doc(alias = "setStroke")]
    pub fn set_stroke(&mut self, stroke: bool) -> &mut Self {
        self.style = if stroke { Style::Stroke } else { Style::Fill };
        self
    }

    /// The color, unpremultiplied, packed into 32 bits (`getColor`).
    // Port of: include/core/SkPaint.h#L225 (chrome/m156)
    #[doc(alias = "getColor")]
    #[must_use]
    pub fn color(&self) -> Color {
        self.color4f.to_color()
    }

    /// The color, unpremultiplied extended sRGB (`getColor4f`).
    #[doc(alias = "getColor4f")]
    #[must_use]
    pub fn color4f(&self) -> Color4f {
        self.color4f
    }

    /// Sets the color (alpha and RGB, unpremultiplied) (`setColor(SkColor)`).
    // Port of: src/core/SkPaint.cpp#L119-L121 (chrome/m156)
    #[doc(alias = "setColor")]
    pub fn set_color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.color4f = Color4f::from_color(color.into());
        self
    }

    /// Sets the color: unpremultiplied RGBA in `color_space` (sRGB if `None`), converted to
    /// extended sRGB, with the alpha pinned to `[0, 1]` (`setColor(const SkColor4f&,
    /// SkColorSpace*)`, `setColor4f`).
    // Port of: src/core/SkPaint.cpp#L123-L128 (chrome/m156)
    #[doc(alias = "setColor4f")]
    #[doc(alias = "setColor")]
    pub fn set_color4f<'a>(
        &mut self,
        color: impl AsRef<Color4f>,
        color_space: impl Into<Option<&'a ColorSpace>>,
    ) -> &mut Self {
        let steps = ColorSpaceXformSteps::new(
            color_space.into(),
            AlphaType::Unpremul,
            Some(srgb_singleton()),
            AlphaType::Unpremul,
        );
        let mut c = color.as_ref().pin_alpha().as_array();
        steps.apply(&mut c);
        self.color4f = Color4f::new(c[0], c[1], c[2], c[3]);
        self
    }

    /// The alpha, from 0 (transparent) to 1 (opaque) (`getAlphaf`).
    #[doc(alias = "getAlphaf")]
    #[must_use]
    pub fn alpha_f(&self) -> f32 {
        self.color4f.a
    }

    /// The alpha as a byte (`getAlpha`).
    // Port of: include/core/SkPaint.h#L264-L266 (chrome/m156)
    #[doc(alias = "getAlpha")]
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // static_cast<uint8_t>
    pub fn alpha(&self) -> u8 {
        float_round2int(self.alpha_f() * 255.0) as u8
    }

    /// Replaces the alpha, leaving RGB unchanged; pinned to `[0, 1]` (`setAlphaf`).
    // Port of: src/core/SkPaint.cpp#L130-L132 (chrome/m156)
    #[doc(alias = "setAlphaf")]
    pub fn set_alpha_f(&mut self, alpha: f32) -> &mut Self {
        self.color4f.a = t_pin(alpha, 0.0f32, 1.0f32);
        self
    }

    /// Replaces the alpha with `alpha / 255`, leaving RGB unchanged (`setAlpha`).
    // Port of: include/core/SkPaint.h#L279-L281 (chrome/m156)
    #[doc(alias = "setAlpha")]
    pub fn set_alpha(&mut self, alpha: u8) -> &mut Self {
        self.set_alpha_f(f32::from(alpha) * (1.0f32 / 255.0))
    }

    /// Sets the color from 8-bit unpremultiplied components (`setARGB`).
    // Port of: src/core/SkPaint.cpp#L134-L136 (chrome/m156)
    #[doc(alias = "setARGB")]
    pub fn set_argb(&mut self, a: u8, r: u8, g: u8, b: u8) -> &mut Self {
        self.set_color(Color::from_argb(a, r, g, b))
    }

    /// The thickness of the pen used to outline the shape; 0 for hairline
    /// (`getStrokeWidth`).
    #[doc(alias = "getStrokeWidth")]
    #[must_use]
    pub fn stroke_width(&self) -> scalar {
        self.width
    }

    /// Sets the thickness of the pen; 0 is a hairline, negative (or NaN) widths are ignored
    /// (`setStrokeWidth`).
    // Port of: src/core/SkPaint.cpp#L159-L167 (chrome/m156)
    #[doc(alias = "setStrokeWidth")]
    pub fn set_stroke_width(&mut self, width: scalar) -> &mut Self {
        if width >= 0.0 {
            self.width = width;
        }
        self
    }

    /// The limit at which a sharp corner is drawn beveled (`getStrokeMiter`).
    #[doc(alias = "getStrokeMiter")]
    #[must_use]
    pub fn stroke_miter(&self) -> scalar {
        self.miter_limit
    }

    /// Sets the miter limit; negative (or NaN) limits are ignored (`setStrokeMiter`).
    // Port of: src/core/SkPaint.cpp#L169-L177 (chrome/m156)
    #[doc(alias = "setStrokeMiter")]
    pub fn set_stroke_miter(&mut self, miter_limit: scalar) -> &mut Self {
        if miter_limit >= 0.0 {
            self.miter_limit = miter_limit;
        }
        self
    }

    /// The geometry drawn at the beginning and end of strokes (`getStrokeCap`).
    #[doc(alias = "getStrokeCap")]
    #[must_use]
    pub fn stroke_cap(&self) -> Cap {
        self.cap
    }

    /// Sets the geometry drawn at the beginning and end of strokes (`setStrokeCap`).
    // Port of: src/core/SkPaint.cpp#L179-L187 (chrome/m156)
    #[doc(alias = "setStrokeCap")]
    pub fn set_stroke_cap(&mut self, cap: Cap) -> &mut Self {
        self.cap = cap;
        self
    }

    /// The geometry drawn at the corners of strokes (`getStrokeJoin`).
    #[doc(alias = "getStrokeJoin")]
    #[must_use]
    pub fn stroke_join(&self) -> Join {
        self.join
    }

    /// Sets the geometry drawn at the corners of strokes (`setStrokeJoin`).
    // Port of: src/core/SkPaint.cpp#L189-L197 (chrome/m156)
    #[doc(alias = "setStrokeJoin")]
    pub fn set_stroke_join(&mut self, join: Join) -> &mut Self {
        self.join = join;
        self
    }

    /// The optional shader, which fills geometry instead of the color (`refShader`).
    #[doc(alias = "refShader")]
    #[doc(alias = "getShader")]
    #[must_use]
    pub fn shader(&self) -> Option<Shader> {
        self.shader.clone()
    }

    /// Sets the optional shader; `None` uses the color instead (`setShader`).
    #[doc(alias = "setShader")]
    pub fn set_shader(&mut self, shader: impl Into<Option<Shader>>) -> &mut Self {
        self.shader = shader.into();
        self
    }

    /// The optional color filter (`refColorFilter`).
    #[doc(alias = "refColorFilter")]
    #[doc(alias = "getColorFilter")]
    #[must_use]
    pub fn color_filter(&self) -> Option<ColorFilter> {
        self.color_filter.clone()
    }

    /// Sets the optional color filter (`setColorFilter`).
    #[doc(alias = "setColorFilter")]
    pub fn set_color_filter(&mut self, color_filter: impl Into<Option<ColorFilter>>) -> &mut Self {
        self.color_filter = color_filter.into();
        self
    }

    /// The blend mode of the blender: src-over without one, `None` for a blender that is not
    /// a blend mode (`asBlendMode`).
    // Port of: src/core/SkPaint.cpp#L138-L141 (chrome/m156)
    #[doc(alias = "asBlendMode")]
    #[must_use]
    pub fn as_blend_mode(&self) -> Option<BlendMode> {
        match &self.blender {
            Some(blender) => blender.as_base().as_blend_mode(),
            None => Some(BlendMode::SrcOver),
        }
    }

    /// [`as_blend_mode`](Self::as_blend_mode), or `default_mode` for a blender that is not a
    /// blend mode (`getBlendMode_or`).
    // Port of: src/core/SkPaint.cpp#L143-L145 (chrome/m156)
    #[doc(alias = "getBlendMode_or")]
    #[must_use]
    pub fn blend_mode_or(&self, default_mode: BlendMode) -> BlendMode {
        self.as_blend_mode().unwrap_or(default_mode)
    }

    /// True if there is no blender or it is src-over (`isSrcOver`).
    // Port of: src/core/SkPaint.cpp#L147-L149 (chrome/m156)
    #[doc(alias = "isSrcOver")]
    #[must_use]
    pub fn is_src_over(&self) -> bool {
        match &self.blender {
            None => true,
            Some(blender) => blender.as_base().as_blend_mode() == Some(BlendMode::SrcOver),
        }
    }

    /// Sets the blender to the blend mode's (no blender for src-over) (`setBlendMode`).
    // Port of: src/core/SkPaint.cpp#L151-L153 (chrome/m156)
    #[doc(alias = "setBlendMode")]
    pub fn set_blend_mode(&mut self, mode: BlendMode) -> &mut Self {
        self.set_blender(if mode == BlendMode::SrcOver {
            None
        } else {
            Some(Blender::mode(mode))
        })
    }

    /// The optional blender; `None` is src-over (`refBlender`).
    #[doc(alias = "refBlender")]
    #[doc(alias = "getBlender")]
    #[must_use]
    pub fn blender(&self) -> Option<Blender> {
        self.blender.clone()
    }

    /// Sets the optional blender, which combines the source and destination colors
    /// (`setBlender`).
    // Port of: src/core/SkPaint.cpp#L155-L157 (chrome/m156)
    #[doc(alias = "setBlender")]
    pub fn set_blender(&mut self, blender: impl Into<Option<Blender>>) -> &mut Self {
        self.blender = blender.into();
        self
    }

    /// The optional path effect, which modifies the geometry before drawing
    /// (`refPathEffect`).
    #[doc(alias = "refPathEffect")]
    #[doc(alias = "getPathEffect")]
    #[must_use]
    pub fn path_effect(&self) -> Option<PathEffect> {
        self.path_effect.clone()
    }

    /// Sets the optional path effect (`setPathEffect`).
    #[doc(alias = "setPathEffect")]
    pub fn set_path_effect(&mut self, path_effect: impl Into<Option<PathEffect>>) -> &mut Self {
        self.path_effect = path_effect.into();
        self
    }

    /// The optional mask filter, which modifies the coverage mask (`refMaskFilter`).
    #[doc(alias = "refMaskFilter")]
    #[doc(alias = "getMaskFilter")]
    #[must_use]
    pub fn mask_filter(&self) -> Option<MaskFilter> {
        self.mask_filter.clone()
    }

    /// Sets the optional mask filter (`setMaskFilter`).
    #[doc(alias = "setMaskFilter")]
    pub fn set_mask_filter(&mut self, mask_filter: impl Into<Option<MaskFilter>>) -> &mut Self {
        self.mask_filter = mask_filter.into();
        self
    }

    /// The optional image filter (`refImageFilter`).
    #[doc(alias = "refImageFilter")]
    #[doc(alias = "getImageFilter")]
    #[must_use]
    pub fn image_filter(&self) -> Option<ImageFilter> {
        self.image_filter.clone()
    }

    /// Sets the optional image filter (`setImageFilter`).
    #[doc(alias = "setImageFilter")]
    pub fn set_image_filter(&mut self, image_filter: impl Into<Option<ImageFilter>>) -> &mut Self {
        self.image_filter = image_filter.into();
        self
    }

    /// True if the paint prevents all drawing; false means the paint may or may not allow
    /// drawing (`nothingToDraw`). For example, true if the blend mode combined with the alpha
    /// computes a new alpha of zero.
    // Port of: src/core/SkPaint.cpp#L273-L294 (chrome/m156)
    #[doc(alias = "nothingToDraw")]
    #[must_use]
    pub fn nothing_to_draw(&self) -> bool {
        let Some(bm) = self.as_blend_mode() else {
            return false;
        };
        match bm {
            BlendMode::SrcOver
            | BlendMode::SrcATop
            | BlendMode::DstOut
            | BlendMode::DstOver
            | BlendMode::Plus => {
                if 0 == self.alpha() {
                    return !color_filter_affects_alpha(self.color_filter.as_ref())
                        && !image_filter_affects_alpha(self.image_filter.as_ref());
                }
            }
            BlendMode::Dst => return true,
            _ => {}
        }
        false
    }

    /// True if the paint does not include elements requiring extensive computation to compute
    /// the device bounds of drawn geometry (`canComputeFastBounds`). For instance, a paint with
    /// a path effect that cannot compute its bounds returns false.
    // Port of: src/core/SkPaint.cpp#L201-L211 (chrome/m156)
    #[doc(alias = "canComputeFastBounds")]
    #[must_use]
    pub fn can_compute_fast_bounds(&self) -> bool {
        if let Some(image_filter) = &self.image_filter
            && !image_filter.can_compute_fast_bounds()
        {
            return false;
        }
        // Pass None for the bounds to determine if they can be computed
        if let Some(path_effect) = &self.path_effect
            && !path_effect.as_base().compute_fast_bounds(None)
        {
            return false;
        }
        true
    }

    /// The bounds of geometry with (sorted) bounds `orig`, adjusted for the paint's stylistic
    /// effects (stroking, path effect, mask and image filters), for quick-reject tests
    /// (`computeFastBounds`). Only valid if
    /// [`can_compute_fast_bounds`](Self::can_compute_fast_bounds) is true. (Skia's `storage`
    /// out-parameter is the return value.)
    // Port of: src/core/SkPaint.cpp#L213-L229 (chrome/m156)
    #[doc(alias = "computeFastBounds")]
    #[must_use]
    pub fn compute_fast_bounds(&self, orig: &Rect) -> Rect {
        // Things like stroking, etc... will do math on the bounds rect, assuming that it's sorted.
        debug_assert!(orig.is_sorted());
        let style = self.style();
        // ultra fast-case: filling with no effects that affect geometry
        if Style::Fill == style
            && self.mask_filter.is_none()
            && self.path_effect.is_none()
            && self.image_filter.is_none()
        {
            return *orig;
        }

        self.do_compute_fast_bounds(orig, style)
    }

    /// [`compute_fast_bounds`](Self::compute_fast_bounds) as if the style were
    /// [`Style::Stroke`] (`computeFastStrokeBounds`).
    // Port of: include/core/SkPaint.h#L643-L646 (chrome/m156)
    #[doc(alias = "computeFastStrokeBounds")]
    #[must_use]
    pub fn compute_fast_stroke_bounds(&self, orig: &Rect) -> Rect {
        self.do_compute_fast_bounds(orig, Style::Stroke)
    }

    /// The fast bounds with `style` overriding the paint's style (`doComputeFastBounds`).
    // Port of: src/core/SkPaint.cpp#L231-L257 (chrome/m156)
    #[doc(alias = "doComputeFastBounds")]
    #[must_use]
    pub fn do_compute_fast_bounds(&self, orig_src: &Rect, style: Style) -> Rect {
        let mut src = *orig_src;
        if let Some(path_effect) = &self.path_effect {
            let computed = path_effect.as_base().compute_fast_bounds(Some(&mut src));
            debug_assert!(computed);
        }

        let radius = StrokeRec::inflation_radius_from_paint_and_style(self, style);
        let mut storage = src.with_outset((radius, radius));

        if let Some(mask_filter) = &self.mask_filter {
            storage = mask_filter.as_base().compute_fast_bounds(&storage);
        }

        if let Some(image_filter) = &self.image_filter {
            storage = image_filter.compute_fast_bounds(storage);
        }

        storage
    }
}

/// True if the filter exists, and may affect alpha.
// Port of: src/core/SkPaint.cpp#L261-L264 (chrome/m156)
fn color_filter_affects_alpha(cf: Option<&ColorFilter>) -> bool {
    cf.is_some_and(|cf| !cf.is_alpha_unchanged())
}

/// True if the filter exists, and may affect alpha.
// Port of: src/core/SkPaint.cpp#L266-L271 (chrome/m156)
fn image_filter_affects_alpha(imf: Option<&ImageFilter>) -> bool {
    // TODO: check if we should allow imagefilters to broadcast that they don't affect alpha
    // ala colorfilters
    imf.is_some()
}

#[cfg(test)]
mod tests;
