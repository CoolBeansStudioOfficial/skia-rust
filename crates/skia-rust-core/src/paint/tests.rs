// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of [`Paint`](super::Paint) against the semantics of `SkPaint.cpp`.

use super::*;
use crate::color::colors;
use crate::color_filter::tests::TestFilter;
use crate::rect::IRect;
use crate::shaders;

/// A test mask filter outsetting by 3.
#[derive(Debug)]
struct Outset3;
impl crate::mask_filter::MaskFilterBase for Outset3 {
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        src.with_outset((3.0, 3.0))
    }
    fn filter_type(&self) -> crate::mask_filter::MaskFilterType {
        crate::mask_filter::MaskFilterType::Table
    }
}

/// A test image filter: doubles the bounds' right/bottom; `unbounded` affects transparent black.
#[derive(Debug)]
struct TestImageFilter {
    common: crate::image_filter::ImageFilterCommon,
    unbounded: bool,
}
impl TestImageFilter {
    fn new(unbounded: bool) -> Self {
        TestImageFilter {
            common: crate::image_filter::ImageFilterCommon::new(Vec::new(), None),
            unbounded,
        }
    }
}
impl crate::image_filter::ImageFilterBase for TestImageFilter {
    fn common(&self) -> &crate::image_filter::ImageFilterCommon {
        &self.common
    }
    fn on_affects_transparent_black(&self) -> bool {
        self.unbounded
    }
    fn on_filter_image(
        &self,
        _context: &crate::image_filter_types::Context<'_>,
    ) -> crate::image_filter_result::FilterResult {
        crate::image_filter_result::FilterResult::default()
    }
    fn on_get_input_layer_bounds(
        &self,
        _mapping: &crate::image_filter_types::Mapping,
        desired_output: IRect,
        _content_bounds: Option<IRect>,
    ) -> IRect {
        desired_output
    }
    fn on_get_output_layer_bounds(
        &self,
        _mapping: &crate::image_filter_types::Mapping,
        _content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        None
    }
    fn compute_fast_bounds(&self, b: &Rect) -> Rect {
        Rect::new(b.left, b.top, b.right * 2.0, b.bottom * 2.0)
    }
}

/// A test path effect: outsets by 1, or cannot compute bounds.
#[derive(Debug)]
struct TestPathEffect {
    bounded: bool,
}
impl crate::path_effect::PathEffectBase for TestPathEffect {
    fn on_filter_path(
        &self,
        _dst: &mut crate::path_builder::PathBuilder,
        _src: &crate::path::Path,
        _rec: &mut StrokeRec,
        _cull_rect: Option<&Rect>,
        _ctm: &crate::matrix::Matrix,
    ) -> bool {
        false
    }
    fn compute_fast_bounds(&self, bounds: Option<&mut Rect>) -> bool {
        if let Some(b) = bounds {
            b.outset((1.0, 1.0));
        }
        self.bounded
    }
}

#[test]
#[allow(clippy::float_cmp)] // exact values
fn defaults() {
    let p = Paint::default();
    assert!(!p.is_anti_alias());
    assert!(!p.is_dither());
    assert_eq!(p.style(), Style::Fill);
    assert_eq!(p.color(), Color::BLACK);
    assert_eq!(p.color4f(), colors::BLACK);
    assert_eq!(p.alpha(), 255);
    assert_eq!(p.stroke_width(), 0.0);
    assert_eq!(p.stroke_miter(), DEFAULT_MITER_LIMIT);
    assert_eq!(p.stroke_cap(), Cap::Butt);
    assert_eq!(p.stroke_join(), Join::Miter);
    assert!(p.shader().is_none());
    assert!(p.color_filter().is_none());
    assert!(p.blender().is_none());
    assert!(p.path_effect().is_none());
    assert!(p.mask_filter().is_none());
    assert!(p.image_filter().is_none());
    assert_eq!(p.as_blend_mode(), Some(BlendMode::SrcOver));
    assert!(p.is_src_over());
    assert_eq!(Style::COUNT, 3);
}

#[test]
#[allow(clippy::float_cmp)] // exact values
fn setters() {
    let mut p = Paint::default();
    p.set_anti_alias(true).set_dither(true).set_stroke(true);
    assert!(p.is_anti_alias() && p.is_dither());
    assert_eq!(p.style(), Style::Stroke);
    p.set_stroke(false);
    assert_eq!(p.style(), Style::Fill);

    // Negative and NaN widths and miters are ignored.
    p.set_stroke_width(3.0).set_stroke_width(-1.0);
    p.set_stroke_width(f32::NAN);
    assert_eq!(p.stroke_width(), 3.0);
    p.set_stroke_miter(0.0).set_stroke_miter(-2.0);
    assert_eq!(p.stroke_miter(), 0.0);

    // Alpha is pinned, and rounds to a byte with sk_float_round2int.
    p.set_alpha_f(1.5);
    assert_eq!(p.alpha_f(), 1.0);
    p.set_alpha_f(-1.0);
    assert_eq!(p.alpha_f(), 0.0);
    p.set_alpha_f(0.5);
    assert_eq!(p.alpha(), 128); // 127.5 rounds up
    p.set_alpha(0x7F);
    assert_eq!(p.alpha_f(), 127.0 * (1.0 / 255.0));
    assert_eq!(p.alpha(), 0x7F);

    p.set_argb(0x80, 0x10, 0x20, 0x30);
    assert_eq!(p.color(), Color::from_argb(0x80, 0x10, 0x20, 0x30));

    // set_color4f pins alpha and converts to sRGB.
    p.set_color4f(Color4f::new(0.25, 0.5, 2.0, 7.0), None);
    assert_eq!(p.color4f(), Color4f::new(0.25, 0.5, 2.0, 1.0));
    let linear = ColorSpace::new_srgb_linear();
    p.set_color4f(Color4f::new(0.5, 0.5, 0.5, 0.5), &linear);
    let mut expected = [0.5, 0.5, 0.5, 0.5];
    ColorSpaceXformSteps::new(
        Some(&linear),
        AlphaType::Unpremul,
        Some(srgb_singleton()),
        AlphaType::Unpremul,
    )
    .apply(&mut expected);
    assert_eq!(p.color4f().as_array(), expected);
    assert_ne!(expected[0], 0.5);
    assert_eq!(
        Paint::new(Color4f::new(0.5, 0.5, 0.5, 0.5), &linear).color4f(),
        p.color4f()
    );
    assert_eq!(
        Paint::new(Color4f::new(0.5, 0.5, 0.5, 0.5), None).color4f(),
        Color4f::new(0.5, 0.5, 0.5, 0.5)
    );
}

#[test]
#[allow(clippy::redundant_clone, clippy::many_single_char_names)] // the clones are what is tested; a, b, c… are paints
fn equality_is_skias() {
    let mut a = Paint::default();
    let b = a.clone();
    assert_eq!(a, b);

    // Floats compare with ==: -0 == 0, NaN != NaN.
    let mut c = a.clone();
    c.width = -0.0;
    assert_eq!(a, c);
    c.color4f.r = f32::NAN;
    assert_ne!(c, c.clone());

    // Effects compare by identity.
    let s = shaders::color(Color::RED);
    a.set_shader(s.clone());
    let mut d = a.clone();
    assert_eq!(a, d);
    d.set_shader(shaders::color(Color::RED));
    assert_ne!(a, d);

    // Blend modes share singletons; src-over is no blender.
    let mut e = Paint::default();
    let mut f = Paint::default();
    e.set_blend_mode(BlendMode::Modulate);
    f.set_blend_mode(BlendMode::Modulate);
    assert_eq!(e, f);
    assert_eq!(e.as_blend_mode(), Some(BlendMode::Modulate));
    assert!(!e.is_src_over());
    e.set_blend_mode(BlendMode::SrcOver);
    assert!(e.blender().is_none());
    assert_eq!(e, Paint::default());
    e.set_blender(Blender::mode(BlendMode::SrcOver));
    assert_ne!(e, Paint::default());
    assert!(e.is_src_over());
    assert_eq!(e.blend_mode_or(BlendMode::Clear), BlendMode::SrcOver);

    // Each flag matters.
    let base = Paint::default();
    let mut g = base.clone();
    g.set_stroke_cap(Cap::Round);
    assert_ne!(g, base);
    let mut g = base.clone();
    g.set_stroke_join(Join::Bevel);
    assert_ne!(g, base);
    let mut g = base.clone();
    g.set_anti_alias(true);
    assert_ne!(g, base);
    let mut g = base.clone();
    g.set_style(Style::StrokeAndFill);
    assert_ne!(g, base);

    // reset() restores the defaults.
    a.reset();
    assert_eq!(a, Paint::default());
}

#[test]
fn nothing_to_draw() {
    let mut p = Paint::default();
    assert!(!p.nothing_to_draw());
    p.set_alpha(0);
    assert!(p.nothing_to_draw());

    p.set_alpha(0xFF);
    p.set_blend_mode(BlendMode::Dst);
    assert!(p.nothing_to_draw());

    p.set_alpha(0);
    for mode in BlendMode::VALUES {
        p.set_blend_mode(mode);
        let expected = matches!(
            mode,
            BlendMode::SrcOver
                | BlendMode::SrcATop
                | BlendMode::DstOut
                | BlendMode::DstOver
                | BlendMode::Plus
                | BlendMode::Dst
        );
        assert_eq!(p.nothing_to_draw(), expected, "{mode:?}");
    }
    p.set_blend_mode(BlendMode::SrcOver);

    // A color filter that keeps alpha, then one that may change it.
    p.set_color_filter(ColorFilter::from_base(TestFilter {
        clear: false,
        alpha_unchanged: true,
    }));
    assert!(p.nothing_to_draw());
    p.set_color_filter(ColorFilter::from_base(TestFilter {
        clear: false,
        alpha_unchanged: false,
    }));
    assert!(!p.nothing_to_draw());

    // Any image filter may change alpha.
    p.set_color_filter(None);
    p.set_image_filter(ImageFilter::from_base(TestImageFilter::new(false)));
    assert!(!p.nothing_to_draw());
}

#[test]
fn fast_bounds() {
    let r = Rect::new(10.0, 20.0, 30.0, 40.0);
    let mut p = Paint::default();
    assert!(p.can_compute_fast_bounds());
    assert_eq!(p.compute_fast_bounds(&r), r);

    // Stroking outsets by the inflation radius (miter join: width/2 * miter limit).
    p.set_style(Style::Stroke).set_stroke_width(2.0);
    assert_eq!(p.compute_fast_bounds(&r), r.with_outset((4.0, 4.0)));
    p.set_stroke_join(Join::Round);
    assert_eq!(p.compute_fast_bounds(&r), r.with_outset((1.0, 1.0)));
    // Hairlines outset by 1.
    p.set_stroke_width(0.0);
    assert_eq!(p.compute_fast_bounds(&r), r.with_outset((1.0, 1.0)));
    p.set_style(Style::Fill);
    p.set_stroke_width(2.0);
    assert_eq!(p.compute_fast_stroke_bounds(&r), r.with_outset((1.0, 1.0)));
    assert_eq!(p.do_compute_fast_bounds(&r, Style::Fill), r);

    // Path effect, then stroke, then mask filter, then image filter.
    p.set_stroke_join(Join::Miter);
    p.set_path_effect(PathEffect::from_base(TestPathEffect { bounded: true }));
    p.set_mask_filter(MaskFilter::from_base(Outset3));
    p.set_image_filter(ImageFilter::from_base(TestImageFilter::new(false)));
    assert!(p.can_compute_fast_bounds());
    // Fill: no stroke outset (1 + 0 + 3).
    let e = r.with_outset((4.0, 4.0));
    assert_eq!(
        p.compute_fast_bounds(&r),
        Rect::new(e.left, e.top, e.right * 2.0, e.bottom * 2.0)
    );

    p.set_image_filter(ImageFilter::from_base(TestImageFilter::new(true)));
    assert!(!p.can_compute_fast_bounds());
    p.set_image_filter(None);
    p.set_path_effect(PathEffect::from_base(TestPathEffect { bounded: false }));
    assert!(!p.can_compute_fast_bounds());
}
