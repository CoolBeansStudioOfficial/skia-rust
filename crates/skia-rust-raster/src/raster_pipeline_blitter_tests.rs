// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the raster pipeline blitter (task D3) against Skia's.
//!
//! `oracle/rp-builder/rp_builder.cpp`'s `run_d3` (run as `rp_builder.exe d3` by `build.ps1`, on
//! the SSE2 baseline tier) builds blitters through `SkCreateRasterPipelineBlitter` for a matrix
//! of cases and drives each through the same blit calls (`run_steps`). It writes
//!
//! - `rp_blitter_oracle/skia_d3_pixels.txt`: per case and step the FNV-1a hash of the whole
//!   destination (`<case> <step> <hash>`), `<case> null` when the blitter could not be made, and
//!   `direct <case> <value|none|null>` for `canDirectBlit()`;
//! - `rp_blitter_oracle/skia_d3_rp_dump.txt`: the oracle's record of every pipeline Skia built,
//!   tagged `<case>/<step>` (the lazily compiled blit pipelines).
//!
//! The cases here are the loops of `run_d3`, in the same order. They run the blitter on the
//! Sse2 tier and require every destination hash and every compiled pipeline (lowp decision on
//! Sse2, op list, `uniform_color`/matrix/transfer function contexts) to be Skia's.

use std::cell::Cell;
use std::fmt::Write as _;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::{Blender, BlenderBase, BlenderType};
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filter::{ColorFilter, ColorFilterBase, ColorFilterType};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::convert_pixels::convert_pixels;
use skia_rust_core::effect_priv::StageRec;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::{Mask, MaskFormat};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::raster_pipeline::{RasterPipeline, Stage};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::{self, MatrixRec, ShaderBase, ShaderType};
use skia_rust_simd::testing::force_tier;
use skia_rust_simd::{Selection, Tier};

use crate::blitter::Blitter;
use crate::raster_pipeline_blitter::{
    BlitKind, RasterPipelineBlitter, create_raster_pipeline_blitter,
    create_raster_pipeline_blitter_with_pipeline,
};

const SKIA_D3_PIXELS: &str = include_str!("rp_blitter_oracle/skia_d3_pixels.txt");
const SKIA_D3_PIXELS_ML3: &str = include_str!("rp_blitter_oracle/skia_d3_pixels_ml3.txt");
const SKIA_D3_PIXELS_ML4: &str = include_str!("rp_blitter_oracle/skia_d3_pixels_ml4.txt");
const SKIA_D3_RP_DUMP: &str = include_str!("rp_blitter_oracle/skia_d3_rp_dump.txt");

const W: i32 = 40;
const H: i32 = 6;

// ---- the test effects (the same classes as rp_builder.cpp's) ---------------------------------

/// `RampShader`: `seed_shader` and the inverse CTM (`r`, `g` = coordinates), made opaque,
/// optionally scaled to a translucent premul color.
#[derive(Debug)]
struct RampShader {
    translucent: bool,
}

impl ShaderBase for RampShader {
    fn is_opaque(&self) -> bool {
        !self.translucent
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::Runtime
    }

    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        if m_rec.apply(rec, Matrix::i()).is_none() {
            return false;
        }
        rec.pipeline.append(Stage::ForceOpaque);
        if self.translucent {
            let scale = rec.alloc.make(Cell::new(0.625f32));
            rec.pipeline.append(Stage::Scale1Float(scale));
        }
        true
    }
}

/// `TestColorFilter`: kind 0 `swap_rb` (alpha unchanged); 1 `scale_1_float` 0.5 (alpha changes);
/// 2 fails; 3 `premul` only when the shader is not opaque (alpha unchanged).
#[derive(Debug)]
struct TestColorFilter {
    kind: i32,
}

impl ColorFilterBase for TestColorFilter {
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, shader_is_opaque: bool) -> bool {
        match self.kind {
            0 => rec.pipeline.append(Stage::SwapRb),
            1 => {
                let half = rec.alloc.make(Cell::new(0.5f32));
                rec.pipeline.append(Stage::Scale1Float(half));
            }
            3 => {
                if !shader_is_opaque {
                    rec.pipeline.append(Stage::Premul);
                }
            }
            _ => return false,
        }
        true
    }

    fn on_is_alpha_unchanged(&self) -> bool {
        self.kind == 0 || self.kind == 3
    }

    fn color_filter_type(&self) -> ColorFilterType {
        ColorFilterType::Noop
    }
}

/// `TestBlender`: not a blend mode; appends `multiply` (or fails).
#[derive(Debug)]
struct TestBlender {
    fail: bool,
}

impl BlenderBase for TestBlender {
    fn on_append_stages(&self, rec: &mut StageRec<'_, '_>) -> bool {
        if self.fail {
            return false;
        }
        rec.pipeline.append(Stage::Multiply);
        true
    }

    fn blender_type(&self) -> BlenderType {
        BlenderType::Runtime
    }
}

// ---- the destinations ------------------------------------------------------------------------

#[derive(Clone)]
struct DstCfg {
    name: &'static str,
    ct: ColorType,
    at: AlphaType,
    cs: Option<ColorSpace>,
}

fn cfg(name: &'static str, ct: ColorType, at: AlphaType, cs: Option<ColorSpace>) -> DstCfg {
    DstCfg { name, ct, at, cs }
}

/// `Dst`: `W x H` with two pixels of row padding, filled from a deterministic F32 ramp.
struct Dst {
    info: ImageInfo,
    bytes: Vec<u8>,
    row_bytes: usize,
}

impl Dst {
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // small loop indices
    fn new(cfg: &DstCfg) -> Dst {
        let info = ImageInfo::new((W, H), cfg.ct, cfg.at, cfg.cs.clone());
        let row_bytes = (W as usize + 2) * info.bytes_per_pixel();
        let mut bytes = vec![0u8; row_bytes * H as usize];
        let mut src = Vec::with_capacity((W * H * 4) as usize * 4);
        for y in 0..H {
            for x in 0..W {
                #[allow(clippy::cast_precision_loss)] // values below 256
                let float = |v: i32| v as f32;
                let red = float((x * 7 + y * 3) % 16) / 15.0;
                let green = float((x * 5 + y * 11) % 16) / 15.0;
                let blue = float((x * 3 + y * 13) % 16) / 15.0;
                let alpha = if cfg.at == AlphaType::Opaque {
                    1.0
                } else {
                    float((x * 13 + y * 5) % 255 + 1) / 255.0
                };
                for channel in [red, green, blue, alpha] {
                    src.extend_from_slice(&channel.to_ne_bytes());
                }
            }
        }
        let src_info = ImageInfo::new((W, H), ColorType::RGBAF32, AlphaType::Unpremul, None);
        assert!(convert_pixels(
            &info,
            &mut bytes,
            row_bytes,
            &src_info,
            &src,
            W as usize * 16
        ));
        Dst {
            info,
            bytes,
            row_bytes,
        }
    }

    fn pixmap(&mut self) -> Pixmap<'_> {
        Pixmap::new(&self.info, &mut self.bytes, self.row_bytes).expect("a valid pixmap")
    }
}

/// `(uint8_t)v`.
fn low_byte(v: i32) -> u8 {
    v.to_le_bytes()[0]
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ---- records ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum Tok {
    Num(f32),
    Bar,
}

impl PartialEq for Tok {
    /// Bit equality (`-0` and `0` differ).
    fn eq(&self, other: &Tok) -> bool {
        match (self, other) {
            (Tok::Num(a), Tok::Num(b)) => a.to_bits() == b.to_bits(),
            (Tok::Bar, Tok::Bar) => true,
            _ => false,
        }
    }
}

/// One oracle record: the pipeline build's tag, lowp decision, op names and context lines.
#[derive(Debug, PartialEq)]
struct Record {
    id: String,
    lowp: bool,
    ops: Vec<String>,
    ctxs: Vec<(usize, String, Vec<Tok>)>,
}

/// The context values the oracle prints for `stage` (`oracle_dump_pipeline`'s switch), if any.
fn ctx_tokens(stage: &Stage<'_>) -> Option<Vec<Tok>> {
    let nums = |v: &[f32]| v.iter().map(|&f| Tok::Num(f)).collect::<Vec<_>>();
    Some(match *stage {
        Stage::UniformColor(u) | Stage::UniformColorDst(u) => {
            let mut t = nums(&[u.r, u.g, u.b, u.a]);
            t.push(Tok::Bar);
            t.extend(u.rgba.iter().map(|&c| Tok::Num(f32::from(c))));
            t
        }
        Stage::SetRgb(c) | Stage::UnboundedSetRgb(c) => nums(c),
        Stage::MatrixTranslate(c) => nums(&c),
        Stage::MatrixScaleTranslate(c) => nums(c),
        Stage::Matrix2x3(c) => nums(c),
        Stage::Matrix3x3(c) | Stage::MatrixPerspective(c) => nums(c),
        Stage::Parametric(tf) => nums(&[tf.g, tf.a, tf.b, tf.c, tf.d, tf.e, tf.f]),
        Stage::Gamma(g) => nums(&[g]),
        _ => return None,
    })
}

fn parse_records(text: &str) -> Vec<Record> {
    let mut records: Vec<Record> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# ") {
            let mut words = rest.split(' ');
            let index = words.next().unwrap().parse().unwrap();
            let op = words.next().unwrap().to_owned();
            let toks = words
                .map(|w| {
                    if w == "|" {
                        Tok::Bar
                    } else {
                        Tok::Num(w.parse().unwrap())
                    }
                })
                .collect();
            records.last_mut().unwrap().ctxs.push((index, op, toks));
        } else {
            let mut words = line.split(' ');
            let id = words.next().unwrap().to_owned();
            let lowp = match words.next().unwrap() {
                "lowp" => true,
                "highp" => false,
                w => panic!("unexpected precision {w}"),
            };
            records.push(Record {
                id,
                lowp,
                ops: words.map(str::to_owned).collect(),
                ctxs: Vec::new(),
            });
        }
    }
    records
}

fn record_of(id: &str, p: &RasterPipeline<'_>) -> Record {
    let stages = p.stages();
    Record {
        id: id.to_owned(),
        lowp: p.is_lowp(Tier::Sse2),
        ops: stages.iter().map(|s| s.op().name().to_owned()).collect(),
        ctxs: stages
            .iter()
            .enumerate()
            .filter_map(|(i, s)| Some((i, s.op().name().to_owned(), ctx_tokens(s)?)))
            .collect(),
    }
}

// ---- the cases (the loops of `run_d3`) ---------------------------------------------------------

/// What the cases produce.
#[derive(Default)]
struct Output {
    /// `skia_d3_pixels.txt`'s lines.
    lines: Vec<String>,
    /// The compiled blit pipelines, tagged `<case>/<step>`.
    records: Vec<Record>,
}

/// `make_paint`.
fn make_paint(pk: usize) -> Paint {
    let none = None::<&ColorSpace>;
    match pk {
        0 => Paint::new(Color4f::new(0.25, 0.5, 0.75, 1.0), none),
        1 => Paint::new(Color4f::new(0.1, 0.2, 0.3, 0.4), none),
        2 => {
            let mut p = Paint::new(Color4f::new(0.0, 0.0, 0.0, 1.0), none);
            p.set_shader(Shader::from_base(RampShader { translucent: false }));
            p
        }
        3 => {
            let mut p = Paint::new(Color4f::new(0.0, 0.0, 0.0, 0.75), none);
            p.set_shader(Shader::from_base(RampShader { translucent: true }));
            p
        }
        _ => unreachable!(),
    }
}

const PAINT_NAMES: [&str; 4] = ["const_opaque", "const_translucent", "ramp", "ramp_alpha"];

/// `ramp_ctm`.
fn ramp_ctm() -> Matrix {
    Matrix::new_all(48.0, 6.0, -3.0, 0.0, 40.0, 2.0, 0.0, 0.0, 1.0)
}

/// The blit calls every case is driven through (`run_steps`), with the hash after each and the
/// pipelines compiled by each.
#[allow(clippy::too_many_lines)] // one block per blit call, as in rp_builder.cpp
fn run_steps(out: &mut Output, name: &str, b: &mut RasterPipelineBlitter<'_>) {
    let mut seen = [false; 5];
    let mut emit = |out: &mut Output, b: &RasterPipelineBlitter<'_>, step: &str| {
        let hash = fnv(b.dst().bytes().expect("pixels"));
        out.lines.push(format!("{name} {step} {hash:016x}"));
        for (i, kind) in BlitKind::ALL.into_iter().enumerate() {
            if b.is_compiled(kind) && !seen[i] {
                seen[i] = true;
                out.records
                    .push(record_of(&format!("{name}/{step}"), &b.pipeline_for(kind)));
            }
        }
    };
    emit(out, b, "init");

    b.blit_h(3, 1, 17);
    emit(out, b, "h");

    b.blit_rect(2, 2, 23, 3);
    emit(out, b, "rect");

    let anti = |spans: &[(i16, u8)], len: usize| {
        let mut aa = vec![0u8; len];
        let mut runs = vec![0i16; len];
        let mut i = 0usize;
        for &(n, a) in spans {
            runs[i] = n;
            aa[i] = a;
            i += usize::try_from(n).unwrap();
        }
        runs[i] = 0;
        (aa, runs)
    };
    let (mut aa, mut runs) = anti(
        &[
            (3, 0x40),
            (4, 0xff),
            (2, 0x00),
            (5, 0x80),
            (1, 0x01),
            (3, 0xfe),
            (9, 0x30),
        ],
        28,
    );
    b.blit_anti_h(1, 0, &mut aa, &mut runs);
    emit(out, b, "antih");

    let (mut aa, mut runs) = anti(&[(1, 0x7f), (16, 0x55), (9, 0xff)], 27);
    b.blit_anti_h(6, 5, &mut aa, &mut runs);
    emit(out, b, "antih2");

    b.blit_v(5, 0, 6, 0x90);
    emit(out, b, "v");
    b.blit_v(7, 1, 3, 0xff);
    emit(out, b, "v255");

    b.blit_anti_h2(10, 4, 0x20, 0xe0);
    emit(out, b, "aa2");
    b.blit_anti_v2(12, 3, 0x77, 0x05);
    emit(out, b, "av2");

    {
        let image: Vec<u8> = (0..24 * 4).map(|i: i32| low_byte(i * 37 + 11)).collect();
        let mask = Mask::new(&image, IRect::new(4, 1, 27, 5), 24, MaskFormat::A8);
        b.blit_mask(&mask, &IRect::new(6, 2, 25, 5));
        emit(out, b, "mask_a8");
    }
    {
        let mut image = vec![0u8; 40 * 3];
        for i in 0..20 * 3i32 {
            let [lo, hi, ..] = (i * 2579 + 101).to_le_bytes();
            let i = usize::try_from(i).unwrap();
            image[2 * i] = lo;
            image[2 * i + 1] = hi;
        }
        let mask = Mask::new(&image, IRect::new(2, 0, 20, 3), 40, MaskFormat::Lcd16);
        b.blit_mask(&mask, &IRect::new(3, 1, 19, 3));
        emit(out, b, "mask_lcd");
    }
    {
        let image: Vec<u8> = (0..16 * 3 * 3).map(|i: i32| low_byte(i * 53 + 7)).collect();
        let mask = Mask::new(&image, IRect::new(8, 2, 22, 5), 16, MaskFormat::ThreeD);
        b.blit_mask(&mask, &IRect::new(9, 2, 21, 5));
        emit(out, b, "mask_3d");
    }
    {
        let image: Vec<u8> = (0..12).map(|i: i32| low_byte(i * 91 + 0x5a)).collect();
        let mask = Mask::new(&image, IRect::new(0, 0, 19, 4), 3, MaskFormat::BW);
        b.blit_mask(&mask, &IRect::new(1, 1, 17, 3));
        emit(out, b, "mask_bw");
    }
}

/// `run_case`.
fn run_case(
    out: &mut Output,
    name: &str,
    dst_cfg: &DstCfg,
    paint: &Paint,
    clip_shader: Option<&Shader>,
) {
    let mut dst = Dst::new(dst_cfg);
    let alloc = ArenaAlloc::new();
    let Some(mut b) = create_raster_pipeline_blitter(
        dst.pixmap(),
        paint,
        &ramp_ctm(),
        &alloc,
        clip_shader,
        &skia_rust_core::surface_props::SurfaceProps::default(),
        &Rect::new_empty(),
    ) else {
        out.lines.push(format!("{name} null"));
        return;
    };
    run_steps(out, name, &mut b);
}

/// `print_direct`.
fn print_direct(out: &mut Output, name: &str, dst_cfg: &DstCfg, paint: &Paint) {
    let mut dst = Dst::new(dst_cfg);
    let alloc = ArenaAlloc::new();
    let Some(mut b) = create_raster_pipeline_blitter(
        dst.pixmap(),
        paint,
        &ramp_ctm(),
        &alloc,
        None,
        &skia_rust_core::surface_props::SurfaceProps::default(),
        &Rect::new_empty(),
    ) else {
        out.lines.push(format!("direct {name} null"));
        return;
    };
    match b.can_direct_blit() {
        Some(d) => out.lines.push(format!("direct {name} {:x}", d.value)),
        None => out.lines.push(format!("direct {name} none")),
    }
}

/// Every color type but `kUnknown`, in `SkColorType` order (1 to 28).
const COLOR_TYPES: [ColorType; 28] = [
    ColorType::Alpha8,
    ColorType::RGB565,
    ColorType::ARGB4444,
    ColorType::RGBA8888,
    ColorType::RGB888x,
    ColorType::BGRA8888,
    ColorType::RGBA1010102,
    ColorType::BGRA1010102,
    ColorType::RGB101010x,
    ColorType::BGR101010x,
    ColorType::BGR101010xXR,
    ColorType::BGRA10101010XR,
    ColorType::RGBA10x6,
    ColorType::Gray8,
    ColorType::RGBAF16Norm,
    ColorType::RGBAF16,
    ColorType::RGBF16F16F16x,
    ColorType::RGBAF32,
    ColorType::R8G8UNorm,
    ColorType::A16Float,
    ColorType::R16Float,
    ColorType::R16G16Float,
    ColorType::A16UNorm,
    ColorType::R16UNorm,
    ColorType::R16G16UNorm,
    ColorType::R16G16B16A16UNorm,
    ColorType::SRGBA8888,
    ColorType::R8UNorm,
];

/// `tier`, natively where the host can, else its model (as `tier_selections()`).
fn selection_of(tier: Tier) -> Selection {
    skia_rust_simd::testing::oracle_selection(tier)
}

/// Runs `run_d3`'s cases on `tier`.
#[allow(clippy::too_many_lines)] // one block per case group, as in rp_builder.cpp
fn run_all(tier: Tier) -> Output {
    let _guard = force_tier(selection_of(tier)).expect("the tier runs here, natively or modeled");
    let mut out = Output::default();
    let srgb = ColorSpace::new_srgb();
    let linear = ColorSpace::new_srgb_linear();

    // Matrix 1: every blend mode x three destinations x four paints.
    let dsts = [
        cfg("rgba8888", ColorType::RGBA8888, AlphaType::Premul, None),
        cfg(
            "bgra_srgb",
            ColorType::BGRA8888,
            AlphaType::Premul,
            Some(srgb.clone()),
        ),
        cfg(
            "f16_linear",
            ColorType::RGBAF16,
            AlphaType::Premul,
            Some(linear.clone()),
        ),
    ];
    for mode in BlendMode::VALUES {
        for dst in &dsts {
            for (pk, paint_name) in PAINT_NAMES.iter().enumerate() {
                let mut paint = make_paint(pk);
                paint.set_blend_mode(mode);
                run_case(
                    &mut out,
                    &format!("m1_{}_{}_{}", mode.name(), dst.name, paint_name),
                    dst,
                    &paint,
                    None,
                );
            }
        }
    }

    // Matrix 2: every color type x alpha types x five paint/mode/dither configurations.
    let p2 = [
        ("opq_over", 0, BlendMode::SrcOver, false),
        ("tr_over", 1, BlendMode::SrcOver, false),
        ("tr_src", 1, BlendMode::Src, false),
        ("ramp_dither_over", 2, BlendMode::SrcOver, true),
        ("rampad_dither_plus", 3, BlendMode::Plus, true),
    ];
    for (i, &ct) in COLOR_TYPES.iter().enumerate() {
        let mut ats = vec![("p", AlphaType::Premul)];
        match ct {
            ColorType::RGBA8888
            | ColorType::BGRA8888
            | ColorType::RGBAF16
            | ColorType::RGBAF32
            | ColorType::RGBA1010102
            | ColorType::BGRA1010102
            | ColorType::SRGBA8888
            | ColorType::ARGB4444
            | ColorType::R16G16B16A16UNorm => ats.push(("u", AlphaType::Unpremul)),
            ColorType::RGB565 | ColorType::RGB888x | ColorType::Gray8 | ColorType::RGB101010x => {
                ats.push(("o", AlphaType::Opaque));
            }
            _ => {}
        }
        for (at_name, at) in ats {
            let dst = cfg("", ct, at, None);
            for &(name, pk, mode, dither) in &p2 {
                let mut paint = make_paint(pk);
                paint.set_blend_mode(mode);
                paint.set_dither(dither);
                run_case(
                    &mut out,
                    &format!("m2_ct{}_{at_name}_{name}", i + 1),
                    &dst,
                    &paint,
                    None,
                );
            }
        }
    }

    // Matrix 3: clip shaders, color filters, blenders, failures, pre-baked pipelines.
    let special_dsts = [
        cfg("rgba8888", ColorType::RGBA8888, AlphaType::Premul, None),
        cfg("bgra8888", ColorType::BGRA8888, AlphaType::Premul, None),
        cfg(
            "f16_linear",
            ColorType::RGBAF16,
            AlphaType::Premul,
            Some(linear.clone()),
        ),
    ];
    let clip_const = shaders::color_in_space(Color4f::new(0.0, 0.0, 0.0, 0.5), None).unwrap();
    let clip_ramp = Shader::from_base(RampShader { translucent: true });
    for dst in &special_dsts {
        let sp = |n: &str| format!("sp_{n}_{}", dst.name);
        let with_mode = |pk: usize, mode: BlendMode| {
            let mut p = make_paint(pk);
            p.set_blend_mode(mode);
            p
        };
        run_case(
            &mut out,
            &sp("clip_const_over"),
            dst,
            &with_mode(1, BlendMode::SrcOver),
            Some(&clip_const),
        );
        run_case(
            &mut out,
            &sp("clip_ramp_over"),
            dst,
            &with_mode(2, BlendMode::SrcOver),
            Some(&clip_ramp),
        );
        run_case(
            &mut out,
            &sp("clip_ramp_opq_src"),
            dst,
            &with_mode(0, BlendMode::Src),
            Some(&clip_ramp),
        );
        run_case(
            &mut out,
            &sp("clip_ramp_rampad_plus"),
            dst,
            &with_mode(3, BlendMode::Plus),
            Some(&clip_ramp),
        );
        run_case(
            &mut out,
            &sp("clip_const_multiply"),
            dst,
            &with_mode(1, BlendMode::Multiply),
            Some(&clip_const),
        );
        run_case(
            &mut out,
            &sp("clip_ramp_clear"),
            dst,
            &with_mode(0, BlendMode::Clear),
            Some(&clip_ramp),
        );
        run_case(
            &mut out,
            &sp("clip_ramp_dst"),
            dst,
            &with_mode(2, BlendMode::Dst),
            Some(&clip_ramp),
        );
        run_case(
            &mut out,
            &sp("clip_empty"),
            dst,
            &with_mode(1, BlendMode::SrcOver),
            Some(&shaders::empty()),
        );

        for kind in 0..4 {
            for pk in [0, 2, 3] {
                let mut p = make_paint(pk);
                p.set_color_filter(ColorFilter::from_base(TestColorFilter { kind }));
                run_case(
                    &mut out,
                    &sp(&format!("cf{kind}_{}", PAINT_NAMES[pk])),
                    dst,
                    &p,
                    None,
                );
            }
        }

        for pk in [1, 2] {
            let mut p = make_paint(pk);
            p.set_blender(Blender::from_base(TestBlender { fail: false }));
            run_case(
                &mut out,
                &sp(&format!("blender_{}", PAINT_NAMES[pk])),
                dst,
                &p,
                None,
            );
            p.set_blender(Blender::from_base(TestBlender { fail: true }));
            run_case(
                &mut out,
                &sp(&format!("blender_fail_{}", PAINT_NAMES[pk])),
                dst,
                &p,
                None,
            );
        }

        {
            let mut p = make_paint(0);
            p.set_shader(shaders::empty());
            run_case(&mut out, &sp("empty_shader"), dst, &p, None);
        }

        // Pre-baked shader pipelines (sprites, vertices, atlases).
        for variant in 0..4 {
            for mode in [BlendMode::SrcOver, BlendMode::Src, BlendMode::Modulate] {
                let mut d = Dst::new(dst);
                let alloc = ArenaAlloc::new();
                let mut shader_pipeline = RasterPipeline::new();
                shader_pipeline.append(Stage::SeedShader);
                shader_pipeline.append(Stage::ForceOpaque);
                let mut opaque = true;
                if variant & 1 != 0 {
                    shader_pipeline.append(Stage::Scale1Float(alloc.make(Cell::new(0.5f32))));
                    opaque = false;
                }
                let mut p = make_paint(1);
                p.set_blend_mode(mode);
                p.set_dither(variant & 2 != 0);
                let name = sp(&format!("pipeline{variant}_{}", mode.name()));
                let b = create_raster_pipeline_blitter_with_pipeline(
                    d.pixmap(),
                    &p,
                    &shader_pipeline,
                    opaque,
                    &alloc,
                    if variant & 2 != 0 {
                        Some(&clip_const)
                    } else {
                        None
                    },
                );
                if let Some(mut b) = b {
                    run_steps(&mut out, &name, &mut b);
                } else {
                    out.lines.push(format!("{name} null"));
                }
            }
        }
    }

    // canDirectBlit.
    let direct_dsts = [
        cfg("rgba8888", ColorType::RGBA8888, AlphaType::Premul, None),
        cfg(
            "bgra_unpremul",
            ColorType::BGRA8888,
            AlphaType::Unpremul,
            None,
        ),
        cfg("rgb565", ColorType::RGB565, AlphaType::Opaque, None),
        cfg("f16", ColorType::RGBAF16, AlphaType::Premul, None),
        cfg("gray8", ColorType::Gray8, AlphaType::Opaque, None),
        cfg("f32", ColorType::RGBAF32, AlphaType::Premul, None),
        cfg("alpha8", ColorType::Alpha8, AlphaType::Premul, None),
        cfg(
            "rgba1010102",
            ColorType::RGBA1010102,
            AlphaType::Premul,
            None,
        ),
        cfg(
            "rgba8888_srgb",
            ColorType::RGBA8888,
            AlphaType::Premul,
            Some(srgb),
        ),
    ];
    for dst in &direct_dsts {
        let mut direct = |n: &str, p: &Paint| {
            print_direct(&mut out, &format!("{}_{n}", dst.name), dst, p);
        };
        direct("opq_over", &make_paint(0));
        direct("tr_over", &make_paint(1));
        let mut p = make_paint(1);
        p.set_blend_mode(BlendMode::Src);
        direct("tr_src", &p);
        let mut p = make_paint(0);
        p.set_blend_mode(BlendMode::Clear);
        direct("opq_clear", &p);
        let mut p = make_paint(0);
        p.set_blend_mode(BlendMode::Plus);
        direct("opq_plus", &p);
        direct("ramp", &make_paint(2));
        let mut p = make_paint(0);
        p.set_dither(true);
        direct("opq_dither", &p);
        let mut p = make_paint(0);
        p.set_color_filter(ColorFilter::from_base(TestColorFilter { kind: 0 }));
        direct("opq_cf", &p);
        let mut p = make_paint(0);
        p.set_blender(Blender::from_base(TestBlender { fail: false }));
        direct("opq_blender", &p);
    }
    out
}

fn check_pixels(tier: Tier, expected: &str) {
    let ours = run_all(tier);
    let skia: Vec<&str> = expected.lines().collect();
    for (i, (s, o)) in skia.iter().zip(&ours.lines).enumerate() {
        assert_eq!(s, o, "{tier:?}, line {i}");
    }
    assert_eq!(skia.len(), ours.lines.len());
}

// The oracle's hashes: `skia_d3_pixels.txt` from the SSE2 baseline, `_ml3`/`_ml4` from the same
// build with `SKIA_ORACLE_CPU_CAP=ml3`/`ml4` (`build.ps1 -Cap ml3 -Tier ml3`). The Sse41 tier has
// no oracle output yet (that build predates the oracle record patch).
#[test]
fn destination_pixels_match_skia_sse2() {
    check_pixels(Tier::Sse2, SKIA_D3_PIXELS);
}

#[test]
fn destination_pixels_match_skia_ml3() {
    check_pixels(Tier::Ml3, SKIA_D3_PIXELS_ML3);
}

#[test]
fn destination_pixels_match_skia_ml4() {
    check_pixels(Tier::Ml4, SKIA_D3_PIXELS_ML4);
}

#[test]
fn compiled_blit_pipelines_match_skia() {
    let ours = run_all(Tier::Sse2);
    // Skia also records the pipelines it runs while creating a blitter (`<case>/create`) and
    // the ones that convert the initial destination pixels (`convert`).
    let skia: Vec<Record> = parse_records(SKIA_D3_RP_DUMP)
        .into_iter()
        .filter(|r| r.id != "convert" && !r.id.ends_with("/create"))
        .collect();
    let mut diff = String::new();
    for (i, (s, o)) in skia.iter().zip(&ours.records).enumerate() {
        if s != o {
            writeln!(diff, "record {i}: skia {s:?}\n   ours {o:?}").unwrap();
            if diff.len() > 4000 {
                break;
            }
        }
    }
    assert!(diff.is_empty(), "{diff}");
    assert_eq!(skia.len(), ours.records.len());
}
