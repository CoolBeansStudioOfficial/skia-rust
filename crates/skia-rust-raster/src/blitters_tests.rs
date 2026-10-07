// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

// Tests of the legacy blitters that do not need Skia's output: the selection logic of
// `SkBlitter::Choose` / `ChooseSprite`, the shader blitter (which Skia never instantiates in the
// pinned builds) against independent per-channel formulas derived from the C++, the blit row
// procs against models of the SSE2/NEON lane math, and the sprite pipeline's use of the raster
// pipeline blitter. `legacy_blitters_tests.rs` compares the blitters' pixels with Skia's.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::{Color, PMColor};
use skia_rust_core::color_filter::{ColorFilter, ColorFilterBase, ColorFilterType};
use skia_rust_core::color_priv::{
    get_packed_a32, get_packed_b32, get_packed_g32, get_packed_r32, pack_argb32,
};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::draw_types::DrawCoverage;
use skia_rust_core::effect_priv::StageRec;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::{Mask, MaskFormat};
use skia_rust_core::mask_filter::{MaskFilter, MaskFilterBase};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::paint_priv::remove_color_filter;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::raster_pipeline::Stage;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::{self, OPAQUE_ALPHA_FLAG, ShaderContext};
use skia_rust_simd::testing::force_tier;
use skia_rust_simd::{Selection, Tier};

use crate::blit_row::{blend_argb32_lanes, color32, factory32};
use crate::blitter::{Blitter, NullBlitter};
use crate::blitter_choose::{Chosen, choose_kind, use_legacy_blitter};
use crate::core_blitters::{
    Argb32BlackBlitter, Argb32Blitter, Argb32OpaqueBlitter, Argb32ShaderBlitter,
};
use crate::raster_pipeline_blitter::{BlitKind, create_raster_pipeline_blitter};
use crate::sprite_blitter::{SpriteKind, choose_sprite_kind};

fn px(a: u32, r: u32, g: u32, b: u32) -> PMColor {
    pack_argb32(a, r, g, b)
}

fn chans(c: PMColor) -> [u32; 4] {
    [
        get_packed_a32(c),
        get_packed_r32(c),
        get_packed_g32(c),
        get_packed_b32(c),
    ]
}

fn from_chans(c: [u32; 4]) -> PMColor {
    px(c[0], c[1], c[2], c[3])
}

// ---- SkBlitRow ---------------------------------------------------------------------------

// `SkPMLerp_SSE2`, `SkPMLerp_neon` and the like on one channel: `dst + (((src - dst) * scale) >>
// 8)` with the product wrapping to 16 bits and a byte add.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)] // 16-bit lane math
fn lerp_lane(src: u32, dst: u32, scale: u32) -> u32 {
    let prod = ((src as i32 - dst as i32) * scale as i32) as u16;
    u32::from((dst as u8).wrapping_add((prod >> 8) as u8))
}

#[test]
fn pm_lerp_is_the_sse2_lerp_for_every_byte_and_scale() {
    use skia_rust_core::color_data::pm_lerp;
    let step = if cfg!(miri) { 37 } else { 1 };
    for scale in (0..=256u32).step_by(step) {
        for src in (0..=255u32).step_by(step) {
            for dst in (0..=255u32).step_by(step) {
                // All four channels carry the same pair, so a lane mixup shows too.
                let s = px(src, src, src, src);
                let d = px(dst, dst, dst, dst);
                let want = lerp_lane(src, dst, scale);
                assert_eq!(
                    chans(pm_lerp(s, d, scale)),
                    [want; 4],
                    "src {src} dst {dst} scale {scale}"
                );
            }
        }
    }
}

#[test]
fn blend_argb32_lanes_is_blend_argb32_for_premultiplied_sources() {
    use skia_rust_core::color_data::blend_argb32;
    let step = if cfg!(miri) { 61 } else { 5 };
    let mut sas: Vec<u32> = (0..=255u32).step_by(step).collect();
    sas.push(255);
    for sa in sas {
        for aa in 0..=255u32 {
            for s in [0, sa / 2, sa] {
                for d in 0..=255u32 {
                    let src = px(sa, s, sa - s, s / 3);
                    let dst = px(d, d ^ 0x5A, d.wrapping_add(77) & 0xFF, 255 - d);
                    assert_eq!(
                        blend_argb32_lanes(src, dst, aa),
                        blend_argb32(src, dst, aa),
                        "src {src:08x} dst {dst:08x} aa {aa}"
                    );
                }
            }
        }
    }
}

#[test]
fn neon_dst_scale_is_the_x86_dst_scale() {
    // blit_row_s32a_blend on NEON computes SkAlphaMulInv256(srcA, alpha256) as
    // `1 + ((v + (v >> 8)) >> 8)` for `v = 0xFF00 - srcA * alpha256`, valid for alpha256 <= 255;
    // x86 as `(p + (p >> 8)) >> 8` for `p = 0xFFFF - srcA * alpha256`. One lane model serves both.
    for src_a in 0..=255u32 {
        for alpha256 in 1..=255u32 {
            let p = 0xFFFF - src_a * alpha256;
            let x86 = (p + (p >> 8)) >> 8;
            let v = 0xFF00 - src_a * alpha256;
            let v = v + (v >> 8);
            let neon = 1 + (v >> 8);
            assert_eq!(x86, neon, "srcA {src_a} alpha256 {alpha256}");
        }
    }
}

#[test]
fn factory32_picks_the_proc_for_its_flags() {
    // The x86 formulas (`blit_row_s32a_opaque` differs on Neon, and has its own tests).
    let _guard = force_tier(Selection::native(Tier::Scalar)).unwrap();
    let src = [px(255, 1, 2, 3), px(128, 64, 32, 16), px(0, 0, 0, 0)];
    let dst0 = [
        px(255, 9, 9, 9),
        px(255, 100, 100, 100),
        px(200, 50, 60, 70),
    ];

    // No flags: a copy.
    let mut dst = dst0;
    factory32(0)(&mut dst, &src, 255);
    assert_eq!(dst, src);

    // Source pixel alpha only: source-over.
    let mut dst = dst0;
    factory32(2)(&mut dst, &src, 255);
    assert_eq!(dst[0], src[0]);
    assert_eq!(
        chans(dst[1]),
        [
            128 + ((255 * 128) >> 8),
            64 + ((100 * 128) >> 8),
            32 + ((100 * 128) >> 8),
            16 + ((100 * 128) >> 8)
        ]
    );
    assert_eq!(dst[2], dst0[2]); // a transparent source leaves the destination

    // Global alpha only: a lerp by alpha + 1.
    let mut dst = dst0;
    factory32(1)(&mut dst, &src, 127);
    let lerp = |s: u32, d: u32| (s * 128 + 128 * d) >> 8;
    assert_eq!(
        chans(dst[0]),
        [lerp(255, 255), lerp(1, 9), lerp(2, 9), lerp(3, 9)]
    );

    // Both: the source alpha scales the destination too.
    let mut dst = dst0;
    factory32(3)(&mut dst, &src, 127);
    // pixel 1: srcA 128, src_scale 128; dst_scale = SkAlphaMulInv256(128, 128).
    let p = 0xFFFFu32 - 128 * 128;
    let ds = (p + (p >> 8)) >> 8;
    assert_eq!(
        chans(dst[1]),
        [
            (128 * 128 + 255 * ds) >> 8,
            (64 * 128 + 100 * ds) >> 8,
            (32 * 128 + 100 * ds) >> 8,
            (16 * 128 + 100 * ds) >> 8
        ]
    );
}

#[test]
fn color32_skips_transparent_fills_opaque_and_blends_the_rest() {
    let dst0: Vec<u32> = (0..7).map(|i| px(255 - i, 10 * i, 20, 30)).collect();

    let mut dst = dst0.clone();
    color32(&mut dst, px(0, 0, 0, 0));
    assert_eq!(dst, dst0);

    color32(&mut dst, px(255, 1, 2, 3));
    assert!(dst.iter().all(|c| *c == px(255, 1, 2, 3)));

    // src-over of a premultiplied color: s + d * (255 - sa) / 255, rounded as Skia does.
    let mut dst = dst0.clone();
    color32(&mut dst, px(128, 100, 50, 25));
    for (d, d0) in dst.iter().zip(&dst0) {
        let want = [
            128 + skia_mul_div255(chans(*d0)[0], 255 - 128),
            100 + skia_mul_div255(chans(*d0)[1], 255 - 128),
            50 + skia_mul_div255(chans(*d0)[2], 255 - 128),
            25 + skia_mul_div255(chans(*d0)[3], 255 - 128),
        ];
        assert_eq!(chans(*d), want);
    }
}

// round(a * b / 255), as `SkMulDiv255Round`.
fn skia_mul_div255(a: u32, b: u32) -> u32 {
    let prod = a * b + 128;
    (prod + (prod >> 8)) >> 8
}

// ---- Choose ------------------------------------------------------------------------------

// A mask filter that makes 3D masks.
#[derive(Debug)]
struct ThreeD;

impl MaskFilterBase for ThreeD {
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        *src
    }

    fn format(&self) -> MaskFormat {
        MaskFormat::ThreeD
    }
}

// A mask filter with the default (A8) format.
#[derive(Debug)]
struct PlainA8;

impl MaskFilterBase for PlainA8 {
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        *src
    }
}

// A color filter that makes everything transparent black.
#[derive(Debug)]
struct ClearFilter;

impl ColorFilterBase for ClearFilter {
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, _shader_is_opaque: bool) -> bool {
        rec.pipeline.append(Stage::Clear);
        true
    }

    fn color_filter_type(&self) -> ColorFilterType {
        ColorFilterType::Noop
    }
}

fn kind(c: &Chosen<'_>) -> &'static str {
    match c {
        Chosen::Null(_) => "null",
        Chosen::A8Coverage(_) => "a8-coverage",
        Chosen::Shader(_) => "shader",
        Chosen::Black(_) => "black",
        Chosen::Opaque(_) => "opaque",
        Chosen::Argb32(_) => "argb32",
        Chosen::RasterPipeline(_) => "raster-pipeline",
    }
}

// Which blitter `choose` makes for `paint` on a device with `info`, and for the raster pipeline
// blitter the dump of its rect pipeline.
fn chosen(
    info: &ImageInfo,
    paint: &Paint,
    coverage: DrawCoverage,
    clip_shader: Option<&Shader>,
    force_rp: bool,
) -> (&'static str, Option<String>) {
    let alloc = ArenaAlloc::new();
    let mut bytes = vec![0u8; 19 * 4 * 4 * 2];
    let pm = if info.color_type() == ColorType::Unknown {
        Pixmap::default()
    } else {
        Pixmap::new(info, &mut bytes, info.min_row_bytes()).unwrap()
    };
    let c = choose_kind(
        pm,
        &Matrix::translate((1.0, 2.0)),
        paint,
        &alloc,
        coverage,
        clip_shader,
        &skia_rust_core::surface_props::SurfaceProps::default(),
        &Rect::from_wh(19.0, 4.0),
        force_rp,
    );
    let dump = match &c {
        Chosen::RasterPipeline(b) => Some(b.pipeline_for(BlitKind::Rect).to_string()),
        _ => None,
    };
    (kind(&c), dump)
}

// The rect pipeline of the raster pipeline blitter made directly for `paint`: what `choose`
// should have made for the paint it tweaked.
fn direct(info: &ImageInfo, paint: &Paint, clip_shader: Option<&Shader>) -> String {
    let alloc = ArenaAlloc::new();
    let mut bytes = vec![0u8; 19 * 4 * 4 * 2];
    let pm = Pixmap::new(info, &mut bytes, info.min_row_bytes()).unwrap();
    create_raster_pipeline_blitter(
        pm,
        paint,
        &Matrix::translate((1.0, 2.0)),
        &alloc,
        clip_shader,
        &skia_rust_core::surface_props::SurfaceProps::default(),
        &Rect::from_wh(19.0, 4.0),
    )
    .expect("the pipeline can draw the paint")
    .pipeline_for(BlitKind::Rect)
    .to_string()
}

fn n32_info() -> ImageInfo {
    ImageInfo::new_n32_premul((19, 4), None)
}

fn paint(color: u32, mode: BlendMode) -> Paint {
    let mut p = Paint::default();
    p.set_color(Color::new(color));
    p.set_blend_mode(mode);
    p
}

#[test]
fn choose_picks_the_legacy_blitter_by_color() {
    let info = n32_info();
    let no = DrawCoverage::No;
    let src_over = BlendMode::SrcOver;
    let k = |color, mode| chosen(&info, &paint(color, mode), no, None, false);

    assert_eq!(k(0x8033_6699, src_over).0, "argb32");
    assert_eq!(k(0x0033_6699, src_over).0, "argb32");
    assert_eq!(k(0xFF33_6699, src_over).0, "opaque");
    assert_eq!(k(0xFF00_0000, src_over).0, "black");
    // Src with an opaque color is source-over (CheckFastPath).
    assert_eq!(k(0xFF33_6699, BlendMode::Src).0, "opaque");
    assert_eq!(k(0xFF00_0000, BlendMode::Src).0, "black");
    // Dst draws nothing.
    assert_eq!(k(0x8033_6699, BlendMode::Dst).0, "null");
}

#[test]
fn choose_falls_through_to_the_raster_pipeline_blitter() {
    let info = n32_info();
    let no = DrawCoverage::No;
    let src_over = BlendMode::SrcOver;

    // A non-source-over blend mode that is not a fast path.
    let p = paint(0x8033_6699, BlendMode::Plus);
    let (k, dump) = chosen(&info, &p, no, None, false);
    assert_eq!(k, "raster-pipeline");
    assert_eq!(dump.unwrap(), direct(&info, &p, None));

    // A shader: Skia has no legacy shader context in the pinned builds.
    let mut p = paint(0xFFFF_FFFF, src_over);
    p.set_shader(shaders::color(Color::new(0xFF11_2233)));
    let (k, dump) = chosen(&info, &p, no, None, false);
    assert_eq!(k, "raster-pipeline");
    assert_eq!(dump.unwrap(), direct(&info, &p, None));

    // A clip shader.
    let clip = shaders::color(Color::new(0x8000_0000));
    let opaque = paint(0xFF33_6699, src_over);
    let (k, dump) = chosen(&info, &opaque, no, Some(&clip), false);
    assert_eq!(k, "raster-pipeline");
    let dump = dump.unwrap();
    assert_eq!(dump, direct(&info, &opaque, Some(&clip)));
    assert_ne!(dump, direct(&info, &opaque, None));

    // gSkForceRasterPipelineBlitter.
    let (k, dump) = chosen(&info, &opaque, no, None, true);
    assert_eq!(k, "raster-pipeline");
    assert_eq!(dump.unwrap(), direct(&info, &opaque, None));

    // A shader the pipeline cannot draw (the empty shader fails to append its stages) makes
    // SkCreateRasterPipelineBlitter fail, which is a null blitter.
    let mut p = paint(0xFF33_6699, src_over);
    p.set_shader(shaders::empty());
    assert_eq!(chosen(&info, &p, no, None, false).0, "null");
}

#[test]
fn choose_by_device() {
    let no = DrawCoverage::No;
    let yes = DrawCoverage::Yes;
    let p = paint(0xFF33_6699, BlendMode::SrcOver);

    // Unknown color type: nothing to draw into.
    let unknown = ImageInfo::new_unknown(None);
    assert_eq!(chosen(&unknown, &p, no, None, false).0, "null");

    // Other color types.
    let a8 = ImageInfo::new_a8((19, 4));
    assert_eq!(chosen(&a8, &p, no, None, false).0, "raster-pipeline");
    let rgb565 = ImageInfo::new((19, 4), ColorType::RGB565, AlphaType::Opaque, None);
    assert_eq!(chosen(&rgb565, &p, no, None, false).0, "raster-pipeline");

    // Coverage: an A8 device gets the coverage blitter, any other nothing.
    let black = paint(0xFF00_0000, BlendMode::SrcOver);
    assert_eq!(chosen(&a8, &black, yes, None, false).0, "a8-coverage");
    assert_eq!(chosen(&n32_info(), &black, yes, None, false).0, "null");

    // Unpremultiplied N32.
    let unpremul = ImageInfo::new_n32((19, 4), AlphaType::Unpremul, None);
    assert_eq!(chosen(&unpremul, &p, no, None, false).0, "raster-pipeline");

    // Color spaces: only sRGB devices, and only for colors that fit in bytes.
    let srgb = ImageInfo::new_n32_premul((19, 4), Some(ColorSpace::new_srgb()));
    assert_eq!(chosen(&srgb, &p, no, None, false).0, "opaque");
    let linear = ImageInfo::new_n32_premul((19, 4), Some(ColorSpace::new_srgb_linear()));
    assert_eq!(chosen(&linear, &p, no, None, false).0, "raster-pipeline");
    let mut wide = Paint::default();
    wide.set_color4f(
        skia_rust_core::color::Color4f::new(1.5, 0.0, 0.0, 1.0),
        Some(&ColorSpace::new_srgb_linear()),
    );
    assert_eq!(chosen(&srgb, &wide, no, None, false).0, "raster-pipeline");
}

#[test]
fn choose_handles_dither_mask_filters_and_color_filters() {
    let info = n32_info();
    let no = DrawCoverage::No;

    // Dither does nothing for a plain color on an 8888 device, so it is dropped.
    let mut p = paint(0xFF33_6699, BlendMode::SrcOver);
    p.set_dither(true);
    assert_eq!(chosen(&info, &p, no, None, false).0, "opaque");
    // use_legacy_blitter itself refuses dithered paints.
    let mut bytes = vec![0u8; 19 * 4 * 4];
    let pm = Pixmap::new(&info, &mut bytes, 19 * 4).unwrap();
    assert!(!use_legacy_blitter(&pm, &p, &Matrix::new_identity(), false));
    p.set_dither(false);
    assert!(use_legacy_blitter(&pm, &p, &Matrix::new_identity(), false));
    assert!(!use_legacy_blitter(&pm, &p, &Matrix::new_identity(), true));

    // A 3D mask filter.
    p.set_mask_filter(MaskFilter::from_base(ThreeD));
    assert_eq!(chosen(&info, &p, no, None, false).0, "raster-pipeline");
    p.set_mask_filter(MaskFilter::from_base(PlainA8));
    assert_eq!(chosen(&info, &p, no, None, false).0, "opaque");

    // A color filter on a plain color is applied to the color.
    let mut p = paint(0xFF33_6699, BlendMode::SrcOver);
    p.set_color_filter(ColorFilter::from_base(ClearFilter));
    // The color became transparent, so it is not the opaque blitter any more.
    assert_eq!(chosen(&info, &p, no, None, false).0, "argb32");

    // On a shader, it moves into a color filter shader, which only the pipeline can draw.
    let mut p = paint(0x8033_6699, BlendMode::SrcOver);
    p.set_shader(shaders::color(Color::new(0xFF11_2233)));
    p.set_color_filter(ColorFilter::from_base(ClearFilter));
    let (k, dump) = chosen(&info, &p, no, None, false);
    assert_eq!(k, "raster-pipeline");
    let mut expected = p.clone();
    remove_color_filter(&mut expected, None);
    // The paint's alpha moved into the shader: the paint is opaque now.
    assert_eq!(expected.alpha(), 0xFF);
    assert!(expected.color_filter().is_none());
    assert_eq!(dump.unwrap(), direct(&info, &expected, None));

    // Clear ignores the whole color pipeline: Src of transparent black.
    let mut p = paint(0xFF33_6699, BlendMode::Clear);
    p.set_shader(shaders::color(Color::new(0xFF11_2233)));
    let (k, dump) = chosen(&info, &p, no, None, false);
    assert_eq!(k, "raster-pipeline");
    let expected = paint(0x0000_0000, BlendMode::Src);
    assert_eq!(dump.unwrap(), direct(&info, &expected, None));
}

// ---- Direct blits ------------------------------------------------------------------------

#[test]
fn opaque_blitters_can_direct_blit() {
    let info = n32_info();
    let mut bytes = vec![0u8; 19 * 4 * 4];

    let mut b = Argb32OpaqueBlitter::new(
        Pixmap::new(&info, &mut bytes, 19 * 4).unwrap(),
        &paint(0xFF33_6699, BlendMode::SrcOver),
    );
    let direct = b.can_direct_blit().expect("opaque blitters allow it");
    assert_eq!(direct.value, u64::from(px(255, 0x33, 0x66, 0x99)));
    assert_eq!(direct.pm.dimensions(), info.dimensions());
    drop(direct);
    drop(b);

    let mut b = Argb32BlackBlitter::new(
        Pixmap::new(&info, &mut bytes, 19 * 4).unwrap(),
        &paint(0xFF00_0000, BlendMode::SrcOver),
    );
    assert_eq!(
        b.can_direct_blit().unwrap().value,
        u64::from(px(255, 0, 0, 0))
    );
    drop(b);

    let mut b = Argb32Blitter::new(
        Pixmap::new(&info, &mut bytes, 19 * 4).unwrap(),
        &paint(0x8033_6699, BlendMode::SrcOver),
    );
    assert!(b.can_direct_blit().is_none());
    assert!(NullBlitter::new().can_direct_blit().is_none());
}

// ---- The shader blitter ------------------------------------------------------------------

// The colors the test shader context produces at device pixel (x, y): valid premultiplied
// pixels, opaque if `opaque`.
fn ctx_pixel(opaque: bool, x: i32, y: i32) -> PMColor {
    let x = u32::try_from(x).unwrap();
    let y = u32::try_from(y).unwrap();
    let a = if opaque {
        255
    } else {
        (x * 29 + y * 61 + 40) & 0xFF
    };
    let c = |k: u32| ((x * 37 + y * 11 + k * 83) & 0xFF) * a / 255;
    px(a, c(1), c(2), c(3))
}

#[derive(Debug)]
struct TestContext {
    opaque: bool,
    calls: usize,
}

impl ShaderContext for TestContext {
    fn flags(&self) -> u32 {
        if self.opaque { OPAQUE_ALPHA_FLAG } else { 0 }
    }

    fn shade_span(&mut self, x: i32, y: i32, span: &mut [PMColor]) {
        self.calls += 1;
        for (i, c) in span.iter_mut().enumerate() {
            *c = ctx_pixel(self.opaque, x + i32::try_from(i).unwrap(), y);
        }
    }
}

const SW: i32 = 12;
const SH: i32 = 5;

fn shaded_device(opaque: bool) -> (Vec<u8>, ImageInfo) {
    // 12x5 pixels of a deterministic background (any bytes).
    let mut bytes = Vec::new();
    for i in 0..SW * SH {
        let i = u32::try_from(i).unwrap();
        bytes.extend_from_slice(
            &px(200 + i % 50, i * 7 % 256, i * 13 % 256, i * 29 % 256).to_ne_bytes(),
        );
    }
    let _ = opaque;
    (bytes, ImageInfo::new_n32_premul((SW, SH), None))
}

fn device_pixel(bytes: &[u8], x: i32, y: i32) -> PMColor {
    let o = usize::try_from(y * SW + x).unwrap() * 4;
    u32::from_ne_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]])
}

fn shader_paint() -> Paint {
    let mut p = Paint::default();
    p.set_shader(shaders::color(Color::new(0xFF11_2233)));
    p
}

fn shader_blitter(pm: Pixmap<'_>, opaque: bool) -> Argb32ShaderBlitter<'_> {
    Argb32ShaderBlitter::new(
        pm,
        &shader_paint(),
        Box::new(TestContext { opaque, calls: 0 }),
    )
}

// Source-over of premultiplied `s` onto `d` per channel, saturating, as the CPU tier's
// `blit_row_s32a_opaque` does it: `SkPMSrcOver` on x86, `SkMulDiv255Round` on Neon.
fn model_src_over(tier: Tier, s: PMColor, d: PMColor) -> PMColor {
    let (s, d) = (chans(s), chans(d));
    if tier == Tier::Neon {
        from_chans([0, 1, 2, 3].map(|i| (s[i] + skia_mul_div255(255 - s[0], d[i])).min(255)))
    } else {
        let scale = 256 - s[0];
        from_chans([0, 1, 2, 3].map(|i| (s[i] + ((d[i] * scale) >> 8)).min(255)))
    }
}

// Every CPU tier this host can run: natively where it can, else by the tier's model.
fn all_tiers() -> Vec<Selection> {
    Tier::ALL
        .into_iter()
        .map(skia_rust_simd::testing::oracle_selection)
        .collect()
}

// `SkAlphaBlend`-style lerp by `alpha + 1`: dst + ((src - dst) * (alpha + 1) >> 8).
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)] // channels are bytes
fn model_lerp(s: PMColor, d: PMColor, alpha: u32) -> PMColor {
    let (s, d) = (chans(s), chans(d));
    let scale = (alpha + 1) as i32;
    from_chans(
        [0, 1, 2, 3].map(|i| (d[i] as i32 + (((s[i] as i32 - d[i] as i32) * scale) >> 8)) as u32),
    )
}

#[test]
fn opaque_shader_context_shades_into_the_device() {
    let (mut bytes, info) = shaded_device(true);
    let before = bytes.clone();
    {
        let mut b = shader_blitter(Pixmap::new(&info, &mut bytes, 48).unwrap(), true);
        b.blit_h(2, 1, 5);
        b.blit_rect(1, 3, 4, 2);
        b.blit_v(9, 0, 3, 255);
        b.blit_v(10, 0, 3, 100);
        let mut aa = [0, 255, 77, 0, 0, 0];
        let mut runs = [1i16, 1, 1, 0, 0, 0];
        // run 1 (x = 6) is 255: direct; run 2 (x = 7) is 77: a lerp; run 0 is skipped.
        b.blit_anti_h(6, 4, &mut aa, &mut runs);
    }
    let want = |x: i32, y: i32| ctx_pixel(true, x, y);
    let was = |x: i32, y: i32| device_pixel(&before, x, y);
    let got = |x: i32, y: i32| device_pixel(&bytes, x, y);
    for x in 2..7 {
        assert_eq!(got(x, 1), want(x, 1));
    }
    for y in 3..5 {
        for x in 1..5 {
            assert_eq!(got(x, y), want(x, y));
        }
    }
    for y in 0..3 {
        assert_eq!(got(9, y), want(9, y));
        // four_byte_interp(c, d, 100): scale 101.
        assert_eq!(got(10, y), model_lerp(want(10, y), was(10, y), 100));
    }
    assert_eq!(got(6, 4), was(6, 4)); // alpha 0 run
    assert_eq!(got(7, 4), want(7, 4));
    assert_eq!(got(8, 4), model_lerp(want(8, 4), was(8, 4), 77));
    assert_eq!(got(0, 0), was(0, 0)); // untouched
}

#[test]
fn translucent_shader_context_blends_with_the_pixel_alpha() {
    for sel in all_tiers() {
        let _guard = force_tier(sel).unwrap();
        translucent_shader_context_check(sel.tier);
    }
}

fn translucent_shader_context_check(tier: Tier) {
    let (mut bytes, info) = shaded_device(false);
    let before = bytes.clone();
    {
        let mut b = shader_blitter(Pixmap::new(&info, &mut bytes, 48).unwrap(), false);
        b.blit_h(0, 0, 12);
        b.blit_rect(2, 2, 5, 2);
        b.blit_v(11, 1, 3, 255);
        b.blit_v(10, 1, 3, 90);
        let mut aa = [255, 0, 128, 0, 0, 0, 0];
        let mut runs = [1i16, 1, 2, 0, 0, 0, 0];
        b.blit_anti_h(3, 4, &mut aa, &mut runs);
    }
    let was = |x: i32, y: i32| device_pixel(&before, x, y);
    let got = |x: i32, y: i32| device_pixel(&bytes, x, y);
    let src = |x: i32, y: i32| ctx_pixel(false, x, y);
    for x in 0..12 {
        assert_eq!(
            got(x, 0),
            model_src_over(tier, src(x, 0), was(x, 0)),
            "x {x}"
        );
    }
    for y in 2..4 {
        for x in 2..7 {
            assert_eq!(got(x, y), model_src_over(tier, src(x, y), was(x, y)));
        }
    }
    for y in 1..4 {
        assert_eq!(got(11, y), model_src_over(tier, src(11, y), was(11, y)));
        // blit_row_s32a_blend with alpha 90: the source alpha scales the destination too.
        let (s, d) = (chans(src(10, y)), chans(was(10, y)));
        let ss = 91;
        let p = 0xFFFFu32 - s[0] * ss;
        let ds = (p + (p >> 8)) >> 8;
        assert_eq!(
            chans(got(10, y)),
            [0, 1, 2, 3].map(|i| (s[i] * ss + d[i] * ds) >> 8)
        );
    }
    // Row 4: run 0 (x = 3) is 255 -> source-over; run 1 (x = 4) skipped; run 2 (x = 5, 6) is 128.
    assert_eq!(got(3, 4), model_src_over(tier, src(3, 4), was(3, 4)));
    assert_eq!(got(4, 4), was(4, 4));
    for x in 5..7 {
        let (s, d) = (chans(src(x, 4)), chans(was(x, 4)));
        let ss = 129;
        let p = 0xFFFFu32 - s[0] * ss;
        let ds = (p + (p >> 8)) >> 8;
        assert_eq!(
            chans(got(x, 4)),
            [0, 1, 2, 3].map(|i| (s[i] * ss + d[i] * ds) >> 8)
        );
    }
}

fn div255_round(x: u32) -> u32 {
    (x + 127) / 255
}

#[test]
fn shader_blitter_masks() {
    // An A8 mask over 12x5 pixels.
    let cov: Vec<u8> = (0..SW * SH)
        .map(|i| u8::try_from(i * 53 % 256).unwrap())
        .collect();
    let bounds = IRect::new(0, 0, SW, SH);
    let mask = Mask::new(&cov, bounds, 12, MaskFormat::A8);
    let clip = IRect::new(2, 1, 10, 4);

    // Opaque context: lerp by coverage, div255(s*c + d*(255-c)).
    let (mut bytes, info) = shaded_device(true);
    let before = bytes.clone();
    {
        let mut b = shader_blitter(Pixmap::new(&info, &mut bytes, 48).unwrap(), true);
        b.blit_mask(&mask, &clip);
    }
    for y in clip.top..clip.bottom {
        for x in clip.left..clip.right {
            let c = u32::from(cov[usize::try_from(y * SW + x).unwrap()]);
            let s = chans(ctx_pixel(true, x, y));
            let d = chans(device_pixel(&before, x, y));
            let want = [0, 1, 2, 3].map(|i| div255_round(s[i] * c + d[i] * (255 - c)));
            assert_eq!(chans(device_pixel(&bytes, x, y)), want, "({x}, {y})");
        }
    }
    assert_eq!(device_pixel(&bytes, 0, 0), device_pixel(&before, 0, 0));

    // Translucent context: s' = approx_scale(s, c); out = s' + approx_scale(d, 255 - s'.a).
    let (mut bytes, info) = shaded_device(false);
    let before = bytes.clone();
    {
        let mut b = shader_blitter(Pixmap::new(&info, &mut bytes, 48).unwrap(), false);
        b.blit_mask(&mask, &clip);
    }
    let approx = |x: u32, y: u32| (x * y + x) >> 8;
    for y in clip.top..clip.bottom {
        for x in clip.left..clip.right {
            let c = u32::from(cov[usize::try_from(y * SW + x).unwrap()]);
            let s = chans(ctx_pixel(false, x, y));
            let d = chans(device_pixel(&before, x, y));
            let s_aa = s.map(|s| approx(s, c));
            let want = [0, 1, 2, 3].map(|i| (s_aa[i] + approx(d[i], 255 - s_aa[0])) & 0xFF);
            assert_eq!(chans(device_pixel(&bytes, x, y)), want, "({x}, {y})");
        }
    }

    // BW masks go to the default blitMask, which uses blit_anti_h with the shader.
    let bits = vec![0b1010_1010u8, 0b1100_0000, 0b0011_0000, 0, 0, 0, 0, 0, 0, 0];
    let bw = Mask::new(&bits, IRect::new(0, 0, 10, 5), 2, MaskFormat::BW);
    let (mut bytes, info) = shaded_device(true);
    let before = bytes.clone();
    {
        let mut b = shader_blitter(Pixmap::new(&info, &mut bytes, 48).unwrap(), true);
        b.blit_mask(&bw, &IRect::new(0, 0, 10, 2));
    }
    for y in 0..2 {
        for x in 0..10 {
            let byte = bits[usize::try_from(y * 2 + (x >> 3)).unwrap()];
            let on = byte & (0x80 >> (x & 7)) != 0;
            let want = if on {
                ctx_pixel(true, x, y)
            } else {
                device_pixel(&before, x, y)
            };
            assert_eq!(device_pixel(&bytes, x, y), want, "({x}, {y})");
        }
    }
}

#[test]
fn shader_blitter_does_one_body_pass_for_degenerate_heights() {
    // blitRect and blitV are do/while loops in Skia: a height of 0 still shades one row.
    let (mut bytes, info) = shaded_device(true);
    {
        let mut b = shader_blitter(Pixmap::new(&info, &mut bytes, 48).unwrap(), true);
        b.blit_rect(0, 0, 3, 0);
        b.blit_v(5, 1, 0, 255);
    }
    for x in 0..3 {
        assert_eq!(device_pixel(&bytes, x, 0), ctx_pixel(true, x, 0));
    }
    assert_eq!(device_pixel(&bytes, 5, 1), ctx_pixel(true, 5, 1));
}

// ---- LCD16 masks -------------------------------------------------------------------------

#[test]
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)] // channels are bytes
fn lcd16_masks_on_an_opaque_device() {
    // 4 pixels of an opaque background and one LCD16 row: 0 (no change), 0xFFFF (the color), and
    // half coverages. Hand-derived from blend_lcd16_opaque / blend_lcd16.
    let info = ImageInfo::new_n32_premul((4, 1), None);
    let bg = px(255, 200, 100, 0);
    let mask16: [u16; 4] = [0, 0xFFFF, 0x8410, 0xF800];
    let mut lcd = Vec::new();
    for m in mask16 {
        lcd.extend_from_slice(&m.to_ne_bytes());
    }
    let mask = Mask::new(&lcd, IRect::new(0, 0, 4, 1), 8, MaskFormat::Lcd16);
    let clip = IRect::new(0, 0, 4, 1);

    let run = |color: u32| {
        let mut bytes: Vec<u8> = (0..4).flat_map(|_| bg.to_ne_bytes()).collect();
        let paint = paint(color, BlendMode::SrcOver);
        if color >> 24 == 0xFF {
            let mut b =
                Argb32OpaqueBlitter::new(Pixmap::new(&info, &mut bytes, 16).unwrap(), &paint);
            b.blit_mask(&mask, &clip);
        } else {
            let mut b = Argb32Blitter::new(Pixmap::new(&info, &mut bytes, 16).unwrap(), &paint);
            b.blit_mask(&mask, &clip);
        }
        (0..4)
            .map(|x| device_pixel_n(&bytes, x))
            .collect::<Vec<_>>()
    };

    // Opaque white text. 0x8410 = r 16, g 32, b 16 (5/6/5 bits): g >> 1 = 16: all (16 + 1) / 32
    // after upscaling: 16 + (16 >> 4) = 17 -> blend_32(src, dst, 17).
    let opaque = run(0xFFFF_FFFF);
    assert_eq!(opaque[0], bg);
    assert_eq!(opaque[1], px(255, 255, 255, 255)); // 0xFFFF -> opaque_dst
    let blend = |s: u32, d: u32, m: i32| (d as i32 + (((s as i32 - d as i32) * m) >> 5)) as u32;
    assert_eq!(
        chans(opaque[2]),
        [
            255,
            blend(255, 200, 17),
            blend(255, 100, 17),
            blend(255, 0, 17)
        ]
    );
    // 0xF800: r = 31 -> 32, g = b = 0.
    assert_eq!(chans(opaque[3]), [255, blend(255, 200, 32), 100, 0]);

    // Translucent text: coverage scaled by srcA = 0x80 + 1 = 129 (>> 8).
    let translucent = run(0x80FF_FFFF);
    assert_eq!(translucent[0], bg);
    let m = |c: i32| (c * 129) >> 8;
    // 0xFFFF: 31 -> 32 for each channel.
    assert_eq!(
        chans(translucent[1]),
        [
            255,
            blend(255, 200, m(32)),
            blend(255, 100, m(32)),
            blend(255, 0, m(32))
        ]
    );
}

fn device_pixel_n(bytes: &[u8], x: usize) -> PMColor {
    u32::from_ne_bytes([
        bytes[x * 4],
        bytes[x * 4 + 1],
        bytes[x * 4 + 2],
        bytes[x * 4 + 3],
    ])
}

// ---- Sprites -----------------------------------------------------------------------------

// Which sprite blitter `choose_sprite` makes for a source with `src_info`.
fn sprite_kind(
    src_info: &ImageInfo,
    paint: &Paint,
    clip_shader: Option<&Shader>,
    force_rp: bool,
) -> Option<SpriteKind> {
    let alloc = ArenaAlloc::new();
    let src_bytes = vec![7u8; src_info.compute_byte_size(src_info.min_row_bytes())];
    let src = Pixmap::new_readonly(src_info, &src_bytes, src_info.min_row_bytes()).unwrap();
    let dst_info = ImageInfo::new_n32_premul((19, 8), None);
    let mut dst_bytes = vec![0u8; 19 * 8 * 4];
    let dst = Pixmap::new(&dst_info, &mut dst_bytes, 19 * 4).unwrap();
    choose_sprite_kind(dst, paint, src, 0, 0, &alloc, clip_shader, force_rp).map(|(k, _)| k)
}

#[test]
fn sprite_choice_follows_skia() {
    let premul = ImageInfo::new_n32((8, 5), AlphaType::Premul, None);
    let opaque = ImageInfo::new_n32((8, 5), AlphaType::Opaque, None);
    let mut alpha80 = paint(0xFF00_0000, BlendMode::SrcOver);
    alpha80.set_alpha(0x80);
    let src_over = paint(0xFF33_6699, BlendMode::SrcOver);

    // Source-over N32 onto N32 with a paint alpha: the legacy blitter.
    assert_eq!(
        sprite_kind(&premul, &alpha80, None, false),
        Some(SpriteKind::D32S32)
    );
    assert_eq!(
        sprite_kind(&premul, &src_over, None, false),
        Some(SpriteKind::D32S32)
    );
    // A copy: Src, or source-over of an opaque source, with a paint alpha of 1.
    assert_eq!(
        sprite_kind(&premul, &paint(0xFF33_6699, BlendMode::Src), None, false),
        Some(SpriteKind::Memcpy)
    );
    assert_eq!(
        sprite_kind(&opaque, &src_over, None, false),
        Some(SpriteKind::Memcpy)
    );
    // A forced pipeline, a clip shader: the pipeline.
    assert_eq!(
        sprite_kind(&premul, &alpha80, None, true),
        Some(SpriteKind::RasterPipeline)
    );
    let clip = shaders::color(Color::new(0x8000_0000));
    assert_eq!(
        sprite_kind(&premul, &alpha80, Some(&clip), false),
        Some(SpriteKind::RasterPipeline)
    );
    // Unpremultiplied sources: none (the caller wraps them in a shader).
    let unpremul = ImageInfo::new_n32((8, 5), AlphaType::Unpremul, None);
    assert_eq!(sprite_kind(&unpremul, &alpha80, None, false), None);
    // A different source color type, another blend mode, a color filter: the pipeline.
    let a8 = ImageInfo::new_a8((8, 5));
    assert_eq!(
        sprite_kind(&a8, &alpha80, None, false),
        Some(SpriteKind::RasterPipeline)
    );
    assert_eq!(
        sprite_kind(&premul, &paint(0xFF00_0000, BlendMode::Plus), None, false),
        Some(SpriteKind::RasterPipeline)
    );
    let mut with_filter = alpha80.clone();
    with_filter.set_color_filter(ColorFilter::from_base(ClearFilter));
    assert_eq!(
        sprite_kind(&premul, &with_filter, None, false),
        Some(SpriteKind::RasterPipeline)
    );
    // A mask filter rules out every sprite blitter.
    let mut with_mask_filter = alpha80.clone();
    with_mask_filter.set_mask_filter(MaskFilter::from_base(PlainA8));
    assert_eq!(sprite_kind(&premul, &with_mask_filter, None, false), None);
}

#[test]
fn raster_pipeline_sprites_read_the_source_at_the_right_offset() {
    // Plus onto transparent pixels copies an opaque source: device pixel (x, y) shows the
    // source's pixel (x - left, y - top).
    let (sw, sh, srb_px) = (8i32, 5i32, 11i32);
    let src_info = ImageInfo::new_n32((sw, sh), AlphaType::Opaque, None);
    let src_pixel = |x: i32, y: i32| {
        px(
            255,
            u32::try_from(10 * x + 1).unwrap(),
            u32::try_from(20 * y + 2).unwrap(),
            u32::try_from(x * y + 3).unwrap(),
        )
    };
    let mut src_bytes = Vec::new();
    for y in 0..sh {
        for x in 0..srb_px {
            // (The padding differs from every pixel.)
            let c = if x < sw { src_pixel(x, y) } else { 0xCAFE_F00D };
            src_bytes.extend_from_slice(&c.to_ne_bytes());
        }
    }
    let src = Pixmap::new_readonly(&src_info, &src_bytes, 44).unwrap();

    let dst_info = ImageInfo::new_n32_premul((19, 8), None);
    let mut dst_bytes = vec![0u8; 19 * 8 * 4];
    let alloc = ArenaAlloc::new();
    {
        let dst = Pixmap::new(&dst_info, &mut dst_bytes, 19 * 4).unwrap();
        let p = paint(0xFF00_0000, BlendMode::Plus);
        let (kind, mut b) = choose_sprite_kind(dst, &p, src, 4, 2, &alloc, None, false).unwrap();
        assert_eq!(kind, SpriteKind::RasterPipeline);
        b.blit_rect(5, 3, 3, 2);
        b.blit_rect(4, 2, 8, 1);
    }

    let at = |x: i32, y: i32| device_pixel_w(&dst_bytes, 19, x, y);
    for y in 0..8 {
        for x in 0..19 {
            let in_first = (5..8).contains(&x) && (3..5).contains(&y);
            let in_second = (4..12).contains(&x) && y == 2;
            let want = if in_first || in_second {
                src_pixel(x - 4, y - 2)
            } else {
                0
            };
            assert_eq!(at(x, y), want, "({x}, {y})");
        }
    }
}

fn device_pixel_w(bytes: &[u8], width: i32, x: i32, y: i32) -> PMColor {
    let o = usize::try_from(y * width + x).unwrap() * 4;
    u32::from_ne_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]])
}
