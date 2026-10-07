// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Builder tests: the pipelines of `oracle/rp-builder/rp_builder.cpp`, built with the same
//! appender calls, against Skia's output for them.
//!
//! - `skia_dump.txt`: `== <case>` plus `SkRasterPipeline::dump()` per case, compared verbatim
//!   with our [`Display`](core::fmt::Display) output.
//! - `skia_rp_dump.txt`: the oracle's compile-time records (`SKIA_ORACLE_RP_DUMP`): per
//!   non-empty pipeline, `<case> <lowp|highp> <ops…>` and `# <index> <op> <ctx values>` lines
//!   (floats printed with `%.9g`, which round-trips `f32`). Compared with our lowp decision on
//!   `Sse2` (the oracle ran its SSE2 baseline) and our context values, bit for bit.
//!
//! Regenerate both with `oracle/rp-builder/build.ps1`.

use super::{
    RasterPipeline, SRGB_INVERSE_TRANSFER_FUNCTION, SRGB_TRANSFER_FUNCTION, Stage,
    TransferFunction, transfer_function_ctx,
};
use core::fmt::Write as _;

use crate::alpha_type::AlphaType;
use crate::arena_alloc::ArenaAlloc;
use crate::color::Color4f;
use crate::color_type::ColorType;
use crate::image_info::ImageInfo;
use crate::matrix::Matrix;
use skia_rust_simd::Tier;
use skia_rust_simd::rp::{MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, MemoryCtxInfo, Op};

const SKIA_DUMP: &str = include_str!("skia_dump.txt");
const SKIA_RP_DUMP: &str = include_str!("skia_rp_dump.txt");

/// `kRGBA_8888_SkColorType`… in `SkColorType` order, from 1 (`kAlpha_8`) to 28 (`kR8_unorm`).
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

/// `gMem[i]`.
fn mem(i: u16) -> MemoryCtx {
    MemoryCtx::new(MemSlot(i))
}

/// The transfer functions of the `tf_*` cases (`rp_builder.cpp`'s `kTFs`).
#[allow(clippy::excessive_precision)] // the C++ literals, verbatim
fn transfer_functions() -> Vec<(&'static str, TransferFunction)> {
    let tf = |g, a, b, c, d, e, f| TransferFunction {
        g,
        a,
        b,
        c,
        d,
        e,
        f,
    };
    vec![
        ("tf_srgb", SRGB_TRANSFER_FUNCTION),
        ("tf_srgb_inv", SRGB_INVERSE_TRANSFER_FUNCTION),
        ("tf_gamma", tf(2.2, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0)),
        ("tf_linear", tf(1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0)),
        (
            "tf_pqish",
            tf(
                -2.0,
                -107.0 / 128.0,
                1.0,
                32.0 / 2523.0,
                2413.0 / 128.0,
                -2392.0 / 128.0,
                8192.0 / 1305.0,
            ),
        ),
        (
            "tf_hlgish",
            tf(
                -3.0,
                2.0,
                2.0,
                1.0 / 0.178_832_77,
                0.284_668_92,
                0.559_910_73,
                0.0,
            ),
        ),
        (
            "tf_hlginvish",
            tf(
                -4.0,
                2.0,
                2.0,
                1.0 / 0.178_832_77,
                0.284_668_92,
                0.559_910_73,
                0.0,
            ),
        ),
    ]
}

/// Builds every case of `rp_builder.cpp`, in order, and calls `f(name, pipeline)`.
#[allow(clippy::too_many_lines)] // one block per case group, as in rp_builder.cpp
fn for_each_case(mut f: impl FnMut(&str, &RasterPipeline<'_>)) {
    let alloc = ArenaAlloc::new();

    let colors: [(&str, [f32; 4]); 9] = [
        ("constant_black", [0.0, 0.0, 0.0, 1.0]),
        ("constant_white", [1.0, 1.0, 1.0, 1.0]),
        ("constant_neg_zero", [-0.0, -0.0, -0.0, 1.0]),
        ("constant_opaque", [0.25, 0.5, 0.75, 1.0]),
        ("constant_rounding", [0.1, 0.2, 0.3, 0.4]),
        (
            "constant_half",
            [0.5 / 255.0, 1.5 / 255.0, 254.5 / 255.0, 1.0],
        ),
        ("constant_transparent", [0.0, 0.0, 0.0, 0.0]),
        ("constant_unpremul", [0.6, 0.0, 0.0, 0.5]),
        ("constant_out_of_range", [1.5, -0.25, 0.0, 1.0]),
    ];
    for (name, rgba) in &colors {
        let mut p = RasterPipeline::new();
        p.append_constant_color(&alloc, rgba);
        f(name, &p);
    }
    let mut p = RasterPipeline::new();
    p.append_constant_color4f(&alloc, &Color4f::new(0.125, 0.25, 0.375, 0.5));
    f("constant_color4f", &p);

    let rgbs: [(&str, [f32; 3]); 4] = [
        ("set_rgb_in_range", [0.5, 0.25, 1.0]),
        ("set_rgb_zero", [0.0, -0.0, 0.0]),
        ("set_rgb_negative", [-0.1, 0.0, 0.0]),
        ("set_rgb_over", [0.0, 0.0, 1.0001]),
    ];
    for (name, rgb) in &rgbs {
        let mut p = RasterPipeline::new();
        p.append_set_rgb(&alloc, rgb);
        f(name, &p);
    }
    let mut p = RasterPipeline::new();
    p.append_set_rgb_color4f(&alloc, &Color4f::new(0.75, 0.5, 0.25, 7.0));
    f("set_rgb_color4f", &p);

    let matrices = [
        ("matrix_identity", Matrix::new_identity()),
        ("matrix_translate", Matrix::translate((3.0, -4.5))),
        ("matrix_scale", Matrix::scale((2.0, 3.0))),
        (
            "matrix_scale_translate",
            Matrix::new_all(2.0, 0.0, 5.0, 0.0, -3.0, 7.0, 0.0, 0.0, 1.0),
        ),
        ("matrix_skew", Matrix::skew((0.5, 0.0))),
        (
            "matrix_affine",
            Matrix::new_all(0.75, -0.5, 10.0, 0.5, 0.75, -20.0, 0.0, 0.0, 1.0),
        ),
        (
            "matrix_perspective",
            Matrix::new_all(1.0, 0.25, 3.0, 0.5, 2.0, 4.0, 0.001, -0.002, 1.0),
        ),
        (
            "matrix_persp_scale",
            Matrix::new_all(2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0),
        ),
    ];
    for (name, m) in &matrices {
        let mut p = RasterPipeline::new();
        p.append_matrix(&alloc, m);
        p.append(Stage::SeedShader);
        f(name, &p);
    }

    // Every color type except kUnknown, through appendLoad, appendLoadDst, appendStore.
    for (i, &ct) in COLOR_TYPES.iter().enumerate() {
        let mut p = RasterPipeline::new();
        p.append_load(ct, mem(0));
        p.append_load_dst(ct, mem(1));
        p.append(Stage::Srcover);
        p.append_store(ct, mem(1));
        f(&format!("load_store_ct{}", i + 1), &p);
    }

    let tfs = transfer_functions();
    for (name, tf) in &tfs {
        let mut p = RasterPipeline::new();
        p.append(Stage::Load8888(mem(0)));
        p.append_transfer_function(tf);
        p.append(Stage::Store8888(mem(0)));
        f(name, &p);
    }

    for (i, &ct) in COLOR_TYPES.iter().enumerate() {
        let mut p = RasterPipeline::new();
        p.append(Stage::SeedShader);
        p.append_clamp_if_normalized(&ImageInfo::new((1, 1), ct, AlphaType::Premul, None));
        f(&format!("clamp_if_normalized_ct{}", i + 1), &p);
    }

    let mut p = RasterPipeline::new();
    p.append(Stage::Load8888(mem(0)));
    p.append_stack_rewind();
    p.append(Stage::SwapRb);
    p.append_stack_rewind();
    p.append(Stage::Store8888(mem(0)));
    f("stack_rewind", &p);

    let mut q = RasterPipeline::new();
    q.append(Stage::Load565(mem(1)));
    q.append(Stage::Srcover);
    q.append(Stage::Store565(mem(1)));
    let empty = RasterPipeline::new();
    let mut p = RasterPipeline::new();
    p.append(Stage::Load8888(mem(0)));
    p.extend(&empty);
    p.extend(&q);
    p.extend(&q);
    f("extend", &p);

    let mut q = RasterPipeline::new();
    q.append(Stage::SeedShader);
    q.append_stack_rewind();
    q.append(Stage::Store8888(mem(0)));
    let mut p = RasterPipeline::new();
    p.append(Stage::BlackColor);
    p.extend(&q);
    f("extend_rewind", &p);

    let mut p = RasterPipeline::new();
    p.append(Stage::Load8888(mem(0)));
    p.append(Stage::Unpremul);
    p.append(Stage::Store8888(mem(0)));
    f("highp_only_op", &p);

    f("empty", &RasterPipeline::new());
}

#[test]
fn color_types_are_in_skcolortype_order() {
    for (i, &ct) in COLOR_TYPES.iter().enumerate() {
        assert_eq!(ct as usize, i + 1);
    }
}

#[test]
fn dump_matches_skia() {
    let mut ours = String::new();
    for_each_case(|name, p| {
        writeln!(ours, "== {name}").unwrap();
        write!(ours, "{p}").unwrap();
    });
    let skia: Vec<&str> = SKIA_DUMP.lines().collect();
    let ours: Vec<&str> = ours.lines().collect();
    for (i, (s, o)) in skia.iter().zip(&ours).enumerate() {
        assert_eq!(s, o, "skia_dump.txt line {}", i + 1);
    }
    assert_eq!(skia.len(), ours.len());
}

/// A token of an oracle context line: a number or the `|` separator.
#[derive(Debug)]
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

/// One oracle record: the lowp decision, the op names and the context lines.
#[derive(Debug, PartialEq)]
struct Record {
    name: String,
    lowp: bool,
    ops: Vec<String>,
    ctxs: Vec<(usize, String, Vec<Tok>)>,
}

fn parse_records(text: &str) -> Vec<Record> {
    let mut records: Vec<Record> = Vec::new();
    for line in text.lines() {
        let mut words = line.split(' ');
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
            let name = words.next().unwrap().to_owned();
            let lowp = match words.next().unwrap() {
                "lowp" => true,
                "highp" => false,
                w => panic!("unexpected precision {w}"),
            };
            records.push(Record {
                name,
                lowp,
                ops: words.map(str::to_owned).collect(),
                ctxs: Vec::new(),
            });
        }
    }
    records
}

#[test]
fn compiled_records_match_skia() {
    let mut ours = Vec::new();
    for_each_case(|name, p| {
        // `compile()` of an empty pipeline returns before building (no record).
        if p.empty() {
            return;
        }
        let stages = p.stages();
        ours.push(Record {
            name: name.to_owned(),
            lowp: p.is_lowp(Tier::Sse2),
            ops: stages.iter().map(|s| s.op().name().to_owned()).collect(),
            ctxs: stages
                .iter()
                .enumerate()
                .filter_map(|(i, s)| Some((i, s.op().name().to_owned(), ctx_tokens(s)?)))
                .collect(),
        });
    });
    let skia = parse_records(SKIA_RP_DUMP);
    for (s, o) in skia.iter().zip(&ours) {
        assert_eq!(s, o);
    }
    assert_eq!(skia.len(), ours.len());
}

#[test]
fn memory_contexts_are_registered_like_skia() {
    let info = |slot, bytes_per_pixel, load, store| MemoryCtxInfo {
        context: mem(slot),
        bytes_per_pixel,
        load,
        store,
    };
    for_each_case(|name, p| {
        if let Some(i) = name.strip_prefix("load_store_ct") {
            let ct = COLOR_TYPES[i.parse::<usize>().unwrap() - 1];
            let bpp = ct.bytes_per_pixel();
            assert_eq!(
                p.memory_ctx_infos(),
                [info(0, bpp, true, false), info(1, bpp, true, true)],
                "{name}"
            );
        }
        match name {
            "extend" => assert_eq!(
                p.memory_ctx_infos(),
                [info(0, 4, true, false), info(1, 2, true, true)]
            ),
            "stack_rewind" => assert_eq!(p.memory_ctx_infos(), [info(0, 4, true, true)]),
            _ => {}
        }
    });
}

#[test]
fn rewind_context_and_stages_needed() {
    for_each_case(|name, p| {
        let rewind = matches!(name, "stack_rewind" | "extend_rewind");
        assert_eq!(
            p.stages_needed(),
            p.num_stages() + 1 + usize::from(rewind),
            "{name}"
        );
        if rewind {
            assert!(!p.is_lowp(Tier::Sse2));
        }
    });

    // reset() drops the rewind context with the stages.
    let mut p = RasterPipeline::new();
    p.append_stack_rewind();
    p.reset();
    p.append(Stage::Srcover);
    assert!(p.is_lowp(Tier::Sse2));
    assert_eq!(p.stages_needed(), 2);

    // Scalar has no lowp pipeline; gForceHighPrecisionRasterPipeline forces highp.
    assert!(!p.is_lowp(Tier::Scalar));
    p.set_force_high_precision(true);
    assert!(!p.is_lowp(Tier::Sse2));
}

#[test]
fn unbounded_uniform_color_keeps_floats() {
    // The oracle does not print unbounded_uniform_color's context.
    let alloc = ArenaAlloc::new();
    let mut p = RasterPipeline::new();
    p.append_constant_color(&alloc, &[1.5, -0.25, 0.0, 1.0]);
    let Stage::UnboundedUniformColor(c) = p.stages()[0] else {
        panic!("expected unbounded_uniform_color");
    };
    assert_eq!(
        [c.r, c.g, c.b, c.a].map(f32::to_bits),
        [1.5f32, -0.25, 0.0, 1.0].map(f32::to_bits)
    );
    assert_eq!(c.rgba, [0; 4]);
}

#[test]
fn transfer_function_contexts_are_borrowed() {
    for (name, tf) in transfer_functions() {
        let mut p = RasterPipeline::new();
        p.append_transfer_function(&tf);
        let ctx = match p.stages()[0] {
            Stage::Parametric(c) | Stage::PQish(c) | Stage::HLGish(c) | Stage::HLGinvish(c) => c,
            Stage::Gamma(g) => {
                assert_eq!(g.to_bits(), tf.g.to_bits(), "{name}");
                continue;
            }
            s => panic!("{name}: unexpected {:?}", s.op()),
        };
        assert!(core::ptr::eq(ctx, core::ptr::from_ref(&tf)), "{name}");
    }
}

#[test]
fn srgb_transfer_functions_match_skcms() {
    assert_eq!(
        SRGB_TRANSFER_FUNCTION,
        transfer_function_ctx(skia_rust_skcms::srgb_transfer_function())
    );
    assert_eq!(
        SRGB_INVERSE_TRANSFER_FUNCTION,
        transfer_function_ctx(skia_rust_skcms::srgb_inverse_transfer_function())
    );
}

/// Runs `p` over 5 pixels at (3, 2) with `run()` and with `compile()`, returning both outputs
/// (slots 0..`slots`, 4 registers of up to 16 floats each).
fn run_both(p: &RasterPipeline<'_>, slots: u16) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut outs = [
        vec![vec![0u8; 4 * 4 * 16]; usize::from(slots)],
        vec![vec![0u8; 4 * 4 * 16]; usize::from(slots)],
    ];
    for (k, out) in outs.iter_mut().enumerate() {
        let mut mem = MemoryBindings::new();
        for (i, buf) in out.iter_mut().enumerate() {
            mem.bind(MemSlot(u16::try_from(i).unwrap()), MemView::write(buf));
        }
        if k == 0 {
            p.run(3, 2, 5, 1, &mut mem);
        } else {
            p.compile().run(3, 2, 5, 1, &mut mem);
        }
    }
    let [a, b] = outs;
    (a, b)
}

#[test]
fn run_matches_compile_beyond_the_stack_buffers() {
    // 40 stages (more than run()'s 32 stack slots) and 3 load_src/store_src buffers.
    let mut p = RasterPipeline::new();
    p.append(Stage::SeedShader);
    for i in 0..39u16 {
        p.append(Stage::StoreSrc(MemPtr::new(MemSlot(i % 3), 0)));
    }
    assert_eq!(p.num_stages(), 40);
    let (run, compiled) = run_both(&p, 3);
    assert_eq!(run, compiled);
    // store_src wrote the last chunk's x = r (pixel centers, non-zero).
    assert!(run.iter().all(|buf| buf.iter().any(|&b| b != 0)));

    // The same with a rewind context (highp, with a leading stack_checkpoint).
    let mut q = RasterPipeline::new();
    q.append(Stage::SeedShader);
    q.append_stack_rewind();
    q.append(Stage::StoreSrc(MemPtr::new(MemSlot(0), 0)));
    let (run, compiled) = run_both(&q, 1);
    assert_eq!(run, compiled);
}

#[test]
fn op_names() {
    assert_eq!(RasterPipeline::get_op_name(Op::Load8888), "load_8888");
    assert_eq!(RasterPipeline::get_op_name(Op::Gamma), "gamma_");
}
