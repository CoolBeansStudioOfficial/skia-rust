// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPaintPriv.{h,cpp}

//! `SkPaintPriv`: private paint helpers (overwrite analysis, dithering, luminance color).
//!
//! skia-rust: `Flatten` / `Unflatten` need `SkWriteBuffer` / `SkReadBuffer` (not ported); they
//! come with those.

use crate::blend_mode::{BlendMode, BlendModeCoeff};
use crate::color::{Color, Color4f};
use crate::color_filter::ColorFilter;
use crate::color_space::ColorSpace;
use crate::color_space_priv::srgb_singleton;
use crate::color_type::ColorType;
use crate::paint::Paint;
use crate::shader::Shader;
use crate::shaders::ColorFilterShader;

/// What overrides the paint's shader when drawing (an image or bitmap shader), and whether it
/// is opaque (`SkPaintPriv::ShaderOverrideOpacity`).
// Port of: src/core/SkPaintPriv.h#L21-L25 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ShaderOverrideOpacity {
    /// There is no overriding shader (bitmap or image) (`kNone_ShaderOverrideOpacity`).
    None,
    /// The overriding shader is opaque (`kOpaque_ShaderOverrideOpacity`).
    Opaque,
    /// The overriding shader may not be opaque (`kNotOpaque_ShaderOverrideOpacity`).
    NotOpaque,
}

// Port of: src/core/SkPaintPriv.cpp#L36-L39 (chrome/m156)
fn changes_alpha(paint: &Paint) -> bool {
    paint
        .color_filter()
        .is_some_and(|cf| !cf.is_alpha_unchanged())
}

// Port of: src/core/SkPaintPriv.cpp#L41-L50 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SrcColorOpacity {
    /// The src color is known to be opaque (alpha == 255).
    Opaque,
    /// The src color is known to be fully transparent (color == 0).
    TransparentBlack,
    /// The src alpha is known to be fully transparent (alpha == 0).
    TransparentAlpha,
    /// The src color opacity is unknown.
    Unknown,
}

// Port of: src/core/SkPaintPriv.cpp#L52-L81 (chrome/m156)
fn blend_mode_is_opaque(mode: BlendMode, opacity_type: SrcColorOpacity) -> bool {
    let Some((src, dst)) = mode.as_coeff() else {
        return false;
    };

    match src {
        BlendModeCoeff::DA | BlendModeCoeff::DC | BlendModeCoeff::IDA | BlendModeCoeff::IDC => {
            return false;
        }
        _ => {}
    }

    match dst {
        BlendModeCoeff::Zero => true,
        BlendModeCoeff::ISA => SrcColorOpacity::Opaque == opacity_type,
        BlendModeCoeff::SA => {
            SrcColorOpacity::TransparentBlack == opacity_type
                || SrcColorOpacity::TransparentAlpha == opacity_type
        }
        BlendModeCoeff::SC => SrcColorOpacity::TransparentBlack == opacity_type,
        _ => false,
    }
}

/// True if drawing with this paint (or `None`) will overwrite all affected pixels
/// (`SkPaintPriv::Overwrites`). Conservative: may return false even though the paint might in
/// fact overwrite its pixels.
// Port of: src/core/SkPaintPriv.cpp#L83-L111 (chrome/m156)
#[doc(alias = "Overwrites")]
#[must_use]
pub fn overwrites(paint: Option<&Paint>, override_opacity: ShaderOverrideOpacity) -> bool {
    let Some(paint) = paint else {
        // No paint means we default to SRC_OVER, so we overwrite iff our shader-override
        // is opaque, or we don't have one.
        return override_opacity != ShaderOverrideOpacity::NotOpaque;
    };

    let mut opacity_type = SrcColorOpacity::Unknown;

    if !changes_alpha(paint) {
        let paint_alpha = paint.alpha();
        let shader = paint.shader();
        if 0xff == paint_alpha
            && override_opacity != ShaderOverrideOpacity::NotOpaque
            && shader.as_ref().is_none_or(Shader::is_opaque)
        {
            opacity_type = SrcColorOpacity::Opaque;
        } else if 0 == paint_alpha {
            if override_opacity == ShaderOverrideOpacity::None && shader.is_none() {
                opacity_type = SrcColorOpacity::TransparentBlack;
            } else {
                opacity_type = SrcColorOpacity::TransparentAlpha;
            }
        }
    }

    let Some(bm) = paint.as_blend_mode() else {
        return false; // don't know for sure, so we play it safe and return false.
    };
    blend_mode_is_opaque(bm, opacity_type)
}

/// Applies the paint's color filter to its color or shader and clears it
/// (`SkPaintPriv::RemoveColorFilter`): with a shader the filter moves into a
/// [`ColorFilterShader`] (which modulates the shader's colors by the paint alpha first, so the
/// paint becomes opaque); with just a color, the filtered color replaces the paint's.
// Port of: src/core/SkPaintPriv.cpp#L161-L175 (chrome/m156)
#[doc(alias = "RemoveColorFilter")]
pub fn remove_color_filter(p: &mut Paint, dst_cs: Option<&ColorSpace>) {
    if let Some(filter) = p.color_filter() {
        if let Some(shader) = p.shader() {
            // SkColorFilterShader will modulate the shader color by paint alpha
            // before applying the filter, so we'll reset it to opaque.
            let alpha = p.alpha_f();
            p.set_shader(ColorFilterShader::make(shader, alpha, Some(filter)));
            p.set_alpha_f(1.0);
        } else {
            let filtered =
                ColorFilter::filter_color4f(&filter, p.color4f(), Some(srgb_singleton()), dst_cs);
            p.set_color4f(filtered, dst_cs);
        }
        p.set_color_filter(None);
    }
}

/// True if drawing with `p` into `dst_ct` should dither (`SkPaintPriv::ShouldDither`).
// Port of: src/core/SkPaintPriv.cpp#L113-L131 (chrome/m156)
#[doc(alias = "ShouldDither")]
#[must_use]
pub fn should_dither(p: &Paint, dst_ct: ColorType) -> bool {
    // The paint dither flag can veto.
    if !p.is_dither() {
        return false;
    }

    if dst_ct == ColorType::Unknown {
        return false;
    }

    // We always dither 565 or 4444 when requested.
    if dst_ct == ColorType::RGB565 || dst_ct == ColorType::ARGB4444 {
        return true;
    }

    // Otherwise, dither is only needed for non-const paints.
    p.image_filter().is_some()
        || p.mask_filter().is_some()
        || p.shader()
            .is_some_and(|s| s.as_base().is_constant().is_none())
}

/// The paint's color if it is just a single color (not a shader without a luminance color),
/// filtered by its color filter. If it is a shader, we can't compute a const luminance for it.
// Port of: src/core/SkPaintPriv.cpp#L133-L151 (chrome/m156)
fn just_a_color(paint: &Paint) -> Option<Color4f> {
    let mut c = paint.color4f();

    if let Some(shader) = paint.shader() {
        c = shader.as_base().as_luminance_color()?;
    }
    if let Some(color_filter) = paint.color_filter() {
        // TODO: This colorspace is meaningless, replace it with something else
        c = color_filter.filter_color4f(c, None, None);
    }
    Some(c)
}

/// The color that determines which gamma-canonical color glyph masks map to
/// (`SkPaintPriv::ComputeLuminanceColor`): the paint's color (through its color filter), or
/// mid-grey if a shader has no luminance color.
// Port of: src/core/SkPaintPriv.cpp#L153-L159 (chrome/m156)
#[doc(alias = "ComputeLuminanceColor")]
#[must_use]
pub fn compute_luminance_color(paint: &Paint) -> Color {
    let c = just_a_color(paint).unwrap_or(Color4f::new(0.5, 0.5, 0.5, 1.0));
    c.to_color()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_filter::ColorFilter;
    use crate::color_filter::tests::TestFilter;
    use crate::shaders;

    #[test]
    fn overwrites_cases() {
        use ShaderOverrideOpacity as O;
        // No paint: src-over.
        assert!(overwrites(None, O::None));
        assert!(overwrites(None, O::Opaque));
        assert!(!overwrites(None, O::NotOpaque));

        let mut p = Paint::default();
        assert!(overwrites(Some(&p), O::None));
        assert!(!overwrites(Some(&p), O::NotOpaque));
        p.set_alpha(0x80);
        assert!(!overwrites(Some(&p), O::None));

        // Src and Clear overwrite regardless of opacity (dst coeff zero).
        p.set_blend_mode(BlendMode::Src);
        assert!(overwrites(Some(&p), O::NotOpaque));
        p.set_blend_mode(BlendMode::Clear);
        assert!(overwrites(Some(&p), O::NotOpaque));
        // Src-in reads dst alpha.
        p.set_blend_mode(BlendMode::SrcIn);
        assert!(!overwrites(Some(&p), O::None));
        // Dst-in with transparent black (dst coeff SA) overwrites (with zero).
        p.set_blend_mode(BlendMode::DstIn);
        p.set_alpha(0);
        assert!(overwrites(Some(&p), O::None));
        assert!(overwrites(Some(&p), O::Opaque)); // transparent alpha
        // Modulate (dst coeff SC) needs transparent black, not just transparent alpha.
        p.set_blend_mode(BlendMode::Modulate);
        assert!(overwrites(Some(&p), O::None));
        assert!(!overwrites(Some(&p), O::Opaque));
        // Advanced modes never.
        p.set_blend_mode(BlendMode::Overlay);
        assert!(!overwrites(Some(&p), O::None));

        // Shader opacity.
        let mut p = Paint::default();
        p.set_shader(shaders::color(Color::from_argb(0x80, 0, 0, 0)));
        assert!(!overwrites(Some(&p), O::None));
        p.set_shader(shaders::color(Color::BLACK));
        assert!(overwrites(Some(&p), O::None));
        // A color filter that may change alpha makes the opacity unknown.
        p.set_color_filter(ColorFilter::from_base(TestFilter {
            clear: false,
            alpha_unchanged: false,
        }));
        assert!(!overwrites(Some(&p), O::None));
    }

    #[test]
    fn should_dither_cases() {
        let mut p = Paint::default();
        assert!(!should_dither(&p, ColorType::RGB565));
        p.set_dither(true);
        assert!(!should_dither(&p, ColorType::Unknown));
        assert!(should_dither(&p, ColorType::RGB565));
        assert!(should_dither(&p, ColorType::ARGB4444));
        assert!(!should_dither(&p, ColorType::RGBA8888));
        // A constant shader is still a constant paint; the empty shader is not constant.
        p.set_shader(shaders::color(Color::RED));
        assert!(!should_dither(&p, ColorType::RGBA8888));
        p.set_shader(shaders::empty());
        assert!(should_dither(&p, ColorType::RGBA8888));
    }

    #[test]
    fn luminance_color() {
        let mut p = Paint::default();
        p.set_color(Color::from_argb(0x80, 0x10, 0x20, 0x30));
        assert_eq!(
            compute_luminance_color(&p),
            Color::from_argb(0x80, 0x10, 0x20, 0x30)
        );
        // A color shader's color, made opaque.
        p.set_shader(shaders::color(Color::from_argb(0x40, 0xFF, 0x00, 0x80)));
        assert_eq!(
            compute_luminance_color(&p),
            Color::from_argb(0xFF, 0xFF, 0x00, 0x80)
        );
        // No luminance color: mid-grey.
        p.set_shader(shaders::empty());
        assert_eq!(
            compute_luminance_color(&p),
            Color4f::new(0.5, 0.5, 0.5, 1.0).to_color()
        );
        // The color filter applies.
        p.set_shader(None);
        p.set_color_filter(ColorFilter::from_base(TestFilter {
            clear: true,
            alpha_unchanged: false,
        }));
        assert_eq!(compute_luminance_color(&p), Color::from_argb(0, 0, 0, 0));
    }

    #[test]
    fn remove_color_filter_cases() {
        let clear = ColorFilter::from_base(TestFilter {
            clear: true,
            alpha_unchanged: false,
        });

        // No color filter: nothing changes.
        let mut p = Paint::default();
        p.set_color(Color::from_argb(0x80, 0x10, 0x20, 0x30));
        let before = p.clone();
        remove_color_filter(&mut p, None);
        assert_eq!(p, before);

        // A plain color is run through the filter.
        p.set_color_filter(clear.clone());
        remove_color_filter(&mut p, None);
        assert!(p.color_filter().is_none());
        assert_eq!(p.color(), Color::from_argb(0, 0, 0, 0));
        assert!(p.shader().is_none());

        // With a shader, the filter moves into a color filter shader, which takes over the paint's
        // alpha (the paint becomes opaque).
        let shader = shaders::color(Color::from_argb(0xFF, 1, 2, 3));
        let mut p = Paint::default();
        p.set_color(Color::from_argb(0x40, 0x10, 0x20, 0x30));
        p.set_shader(shader.clone());
        p.set_color_filter(clear.clone());
        remove_color_filter(&mut p, None);
        assert!(p.color_filter().is_none());
        #[allow(clippy::float_cmp)] // alpha was reset to exactly 1
        {
            assert_eq!(p.alpha_f(), 1.0);
        }
        let new_shader = p.shader().unwrap();
        let base: &dyn core::any::Any = new_shader.as_base();
        let cfs = base.downcast_ref::<ColorFilterShader>().unwrap();
        assert_eq!(cfs.shader(), &shader);
        assert_eq!(cfs.filter(), &clear);
        assert_eq!(cfs.alpha(), f32::from(0x40u8) / 255.0);
    }
}
