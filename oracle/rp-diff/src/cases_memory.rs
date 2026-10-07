// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Cases for the memory stages (Wave B1/B2): loads, stores and gathers of every pixel format,
//! `srcover_rgba_8888`, `swap_rb`, `alpha_to_*`, `load_src_rg`/`store_src_rg`.
//!
//! All of them run on real pixel memory: row strides larger than the rect, heights of 2 or more,
//! rects that start inside the buffer (`x0`, `y0` > 0), tails around every tier's stride,
//! negative origins ("fake base" pointers) and compiled pipelines run on several rects (stale
//! tail scratch). Gathers sample a small image through `seed_shader` and
//! `matrix_scale_translate`, so coordinates fall inside, between, on the edges of and far
//! outside the image (clamping), for both `round_down_at_integer` settings.

use skia_rust_simd::rp::Op;

use crate::case::{Buffer, Case, Ctx, Rect, StageSpec};
use crate::cases::{
    Cases, FILL, Inputs, Precision, REG_BYTES, REGISTER_WIDTHS, Rng, TAIL_WIDTHS, register_input,
};

/// One pixel format and its memory stages.
struct Fmt {
    name: &'static str,
    bpp: usize,
    /// lowp versions exist (`Precision::Auto` runs lowp).
    lowp: bool,
    load: Option<Op>,
    load_dst: Option<Op>,
    store: Option<Op>,
    gather: Option<Op>,
}

macro_rules! fmt {
    ($name:literal, $bpp:literal, $lowp:literal, $load:ident, $load_dst:ident, $store:ident, $gather:ident) => {
        Fmt {
            name: $name,
            bpp: $bpp,
            lowp: $lowp,
            load: Some(Op::$load),
            load_dst: Some(Op::$load_dst),
            store: Some(Op::$store),
            gather: Some(Op::$gather),
        }
    };
}

#[allow(clippy::too_many_lines)] // one table row per pixel format
fn formats() -> Vec<Fmt> {
    vec![
        // B1
        fmt!("a8", 1, true, LoadA8, LoadA8Dst, StoreA8, GatherA8),
        fmt!("565", 2, true, Load565, Load565Dst, Store565, Gather565),
        fmt!(
            "4444",
            2,
            true,
            Load4444,
            Load4444Dst,
            Store4444,
            Gather4444
        ),
        fmt!(
            "8888",
            4,
            true,
            Load8888,
            Load8888Dst,
            Store8888,
            Gather8888
        ),
        fmt!(
            "rg88",
            2,
            true,
            LoadRg88,
            LoadRg88Dst,
            StoreRg88,
            GatherRg88
        ),
        Fmt {
            name: "r8",
            bpp: 1,
            lowp: true,
            load: None,
            load_dst: None,
            store: Some(Op::StoreR8),
            gather: None,
        },
        // B2
        fmt!(
            "16161616",
            8,
            false,
            Load16161616,
            Load16161616Dst,
            Store16161616,
            Gather16161616
        ),
        fmt!("a16", 2, false, LoadA16, LoadA16Dst, StoreA16, GatherA16),
        fmt!("r16", 2, false, LoadR16, LoadR16Dst, StoreR16, GatherR16),
        fmt!(
            "rg1616",
            4,
            false,
            LoadRg1616,
            LoadRg1616Dst,
            StoreRg1616,
            GatherRg1616
        ),
        fmt!("f16", 8, false, LoadF16, LoadF16Dst, StoreF16, GatherF16),
        fmt!(
            "af16",
            2,
            false,
            LoadAf16,
            LoadAf16Dst,
            StoreAf16,
            GatherAf16
        ),
        fmt!(
            "rf16",
            2,
            false,
            LoadRf16,
            LoadRf16Dst,
            StoreRf16,
            GatherRf16
        ),
        fmt!(
            "rgf16",
            4,
            false,
            LoadRgf16,
            LoadRgf16Dst,
            StoreRgf16,
            GatherRgf16
        ),
        fmt!("f32", 16, false, LoadF32, LoadF32Dst, StoreF32, GatherF32),
        fmt!(
            "1010102",
            4,
            false,
            Load1010102,
            Load1010102Dst,
            Store1010102,
            Gather1010102
        ),
        fmt!(
            "1010102_xr",
            4,
            false,
            Load1010102Xr,
            Load1010102XrDst,
            Store1010102Xr,
            Gather1010102Xr
        ),
        fmt!(
            "10x6",
            8,
            false,
            Load10x6,
            Load10x6Dst,
            Store10x6,
            Gather10x6
        ),
        fmt!(
            "10101010_xr",
            8,
            false,
            Load10101010Xr,
            Load10101010XrDst,
            Store10101010Xr,
            Gather10101010Xr
        ),
    ]
}

/// Where a case's rects sit in its buffers.
struct Geo {
    suffix: String,
    rects: Vec<Rect>,
    compiled: bool,
    /// Single rect, exactly-sized buffers addressed through a negative origin.
    origin: bool,
}

const X0: usize = 2;
const Y0: usize = 1;
const H: usize = 2;

/// Plain (strided, rect inside the buffer), negative-origin and compiled geometries.
fn geos(widths: &[usize], extras: bool) -> Vec<Geo> {
    let mut v: Vec<Geo> = widths
        .iter()
        .map(|&w| Geo {
            suffix: format!("w{w}"),
            rects: vec![Rect::new(X0, Y0, w, H)],
            compiled: false,
            origin: false,
        })
        .collect();
    if extras {
        for &w in &[3, 17] {
            v.push(Geo {
                suffix: format!("w{w}_origin"),
                rects: vec![Rect::new(X0, Y0, w, H)],
                compiled: false,
                origin: true,
            });
        }
        v.push(Geo {
            suffix: "compiled".to_owned(),
            rects: vec![
                Rect::new(0, 0, 5, 1),
                Rect::new(1, 1, 17, 2),
                Rect::new(2, 0, 3, 3),
                Rect::new(0, 3, 16, 1),
                Rect::new(4, 2, 1, 2),
            ],
            compiled: true,
            origin: false,
        });
    }
    v
}

impl Geo {
    /// `(stride in pixels, rows, origin in bytes)` of a buffer of `bpp` bytes per pixel that
    /// holds every rect, with `slack` extra pixels per row.
    fn layout(&self, bpp: usize, slack: usize) -> (usize, usize, isize) {
        if self.origin {
            let r = self.rects[0];
            let stride = r.w + slack;
            let origin = -isize::try_from((r.y * stride + r.x) * bpp).expect("small");
            (stride, r.h, origin)
        } else {
            let xmax = self.rects.iter().map(|r| r.x + r.w).max().expect("rects");
            let ymax = self.rects.iter().map(|r| r.y + r.h).max().expect("rects");
            (xmax + slack, ymax, 0)
        }
    }

    /// A buffer for this geometry; `fill` gives its bytes from the length.
    fn buffer(&self, bpp: usize, slack: usize, fill: impl FnOnce(usize) -> Vec<u8>) -> Buffer {
        let (stride, rows, origin) = self.layout(bpp, slack);
        Buffer::new(fill(stride * rows * bpp))
            .with_layout(isize::try_from(stride).expect("small"), origin)
    }
}

fn rng_for(seed: u32) -> Rng {
    Rng::new(seed.wrapping_mul(0x9e37_79b1) | 1)
}

/// Random pixel bytes, with all-ones and all-zeros pixels sprinkled in.
fn pixel_bytes(seed: u32, len: usize, bpp: usize) -> Vec<u8> {
    let mut rng = rng_for(seed);
    let mut v: Vec<u8> = (0..len)
        .map(|_| u8::try_from(rng.below(256)).expect("below 256"))
        .collect();
    for (i, px) in v.chunks_mut(bpp).enumerate() {
        match i % 11 {
            3 => px.fill(0xff),
            7 => px.fill(0),
            _ => {}
        }
    }
    v
}

fn out_buffer(len: usize) -> Vec<u8> {
    vec![FILL; len]
}

fn mem(slot: u16) -> Ctx {
    Ctx::Mem { slot }
}

/// A pixel sink after a load: `(name, store op, bytes per pixel, lowp-capable)`.
type Sink = (&'static str, Op, usize, bool);

const SINK_8888: Sink = ("s8888", Op::Store8888, 4, true);
const SINK_F32: Sink = ("sf32", Op::StoreF32, 16, false);

fn precisions(lowp: bool) -> &'static [Precision] {
    if lowp {
        Precision::BOTH
    } else {
        Precision::HIGHP
    }
}

/// Entry: every memory stage case.
pub fn memory(c: &mut Cases) {
    let fmts = formats();
    for f in &fmts {
        loads(c, f);
        stores(c, f);
        gathers(c, f);
    }
    load_8888_store_r8(c);
    srcover_rgba_8888(c);
    for op in [
        Op::SwapRb,
        Op::SwapRbDst,
        Op::AlphaToGray,
        Op::AlphaToGrayDst,
        Op::AlphaToRed,
        Op::AlphaToRedDst,
    ] {
        c.registers(
            op.name(),
            &[StageSpec::new(op)],
            Precision::BOTH,
            REGISTER_WIDTHS,
        );
    }
    src_rg(c);
}

/// `load_X`/`load_X_dst` into registers, stored with 8888 / f32 / the format's own store.
fn loads(c: &mut Cases, f: &Fmt) {
    for (dst, op) in [(false, f.load), (true, f.load_dst)] {
        let Some(op) = op else { continue };
        let mut sinks: Vec<Sink> = Vec::new();
        if f.lowp {
            sinks.push(SINK_8888);
        }
        sinks.push(SINK_F32);
        if let Some(store) = f.store
            && f.name != "8888"
        {
            sinks.push(("same", store, f.bpp, f.lowp));
        }
        let widths: &[usize] = if dst { &[1, 5, 17] } else { TAIL_WIDTHS };
        for (sink, store, sink_bpp, sink_lowp) in sinks {
            let lowp = f.lowp && sink_lowp;
            for &p in precisions(lowp) {
                for geo in geos(widths, true) {
                    let seed = c.next_seed();
                    let input = geo.buffer(f.bpp, 3, |n| pixel_bytes(seed, n, f.bpp));
                    let output = geo.buffer(sink_bpp, 5, out_buffer);
                    let mut stages = vec![StageSpec::with(op, mem(0))];
                    if dst {
                        stages.push(StageSpec::new(Op::MoveDstSrc));
                    }
                    stages.push(StageSpec::with(store, mem(1)));
                    c.push(Case {
                        name: format!("mem/{}/{sink}/{}/{}", op.name(), p.name(), geo.suffix),
                        force_highp: p == Precision::Highp,
                        compiled: geo.compiled,
                        buffers: vec![input, output],
                        stages,
                        runs: geo.rects.clone(),
                    });
                }
            }
        }
    }
}

/// `load_src` registers (special, random, NaN inputs) into `store_X`.
fn stores(c: &mut Cases, f: &Fmt) {
    let Some(store) = f.store else { return };
    for &p in precisions(f.lowp) {
        let kinds: Vec<Inputs> = Inputs::for_precision(p)
            .iter()
            .copied()
            .filter(|&i| i != Inputs::NanDst)
            .collect();
        for geo in geos(TAIL_WIDTHS, true) {
            let plain = !geo.origin && !geo.compiled;
            for &inputs in kinds.iter().take(if plain { kinds.len() } else { 1 }) {
                let seed = c.next_seed();
                let regs = register_input(inputs, seed);
                let output = geo.buffer(f.bpp, 3, out_buffer);
                c.push(Case {
                    name: format!(
                        "mem/{}/{}/{}/{}",
                        store.name(),
                        p.name(),
                        inputs.name(),
                        geo.suffix
                    ),
                    force_highp: p == Precision::Highp,
                    compiled: geo.compiled,
                    buffers: vec![Buffer::new(regs), output],
                    stages: vec![
                        StageSpec::with(Op::LoadSrc, Ctx::Ptr { slot: 0, offset: 0 }),
                        StageSpec::with(store, mem(1)),
                    ],
                    runs: geo.rects.clone(),
                });
            }
        }
    }
}

/// `(name, scale x, scale y, translate x, translate y)` of the coordinate transforms of the
/// gather cases: `seed_shader` gives `(i + 0.5, j + 0.5)`.
type Transform = (&'static str, [f32; 4]);

fn transforms() -> Vec<Transform> {
    // `matrix_scale_translate` takes [sx, sy, tx, ty].
    vec![
        // Integer-valued positions (x = i + 0.5 - 0.5): round_down_at_integer matters.
        ("int", [1.0, 1.0, -0.5, -0.5]),
        // Starts left/above the image, runs past its right/bottom edge.
        ("shift", [1.0, 1.0, -2.25, -1.75]),
        ("frac", [0.37, 0.61, -0.25, 0.1]),
        ("neg", [-1.3, -0.7, 3.0, 2.0]),
        ("far", [1000.0, -1000.0, -3.0, 5.0]),
        ("huge", [3.0e9, -4.0e9, 1.0e10, -1.0e10]),
        ("inf", [f32::INFINITY, 1.0, 0.0, f32::INFINITY]),
        ("nan", [1.0, 1.0, f32::NAN, 1.5]),
    ]
}

const IMG_W: usize = 5;
const IMG_H: usize = 3;
const IMG_STRIDE: usize = 7;

/// `seed_shader matrix_scale_translate gather_X store_*`, sampling a 5x3 image (stride 7) with
/// clamping at the edges.
fn gathers(c: &mut Cases, f: &Fmt) {
    let Some(gather) = f.gather else { return };
    let mut sinks: Vec<Sink> = Vec::new();
    if f.lowp {
        sinks.push(SINK_8888);
    }
    sinks.push(SINK_F32);
    for (sink, store, sink_bpp, sink_lowp) in sinks {
        let lowp = f.lowp && sink_lowp;
        for &p in precisions(lowp) {
            for (tname, m) in transforms() {
                for round_down in [false, true] {
                    let widths: &[usize] = &[5, 17];
                    for geo in geos(widths, false) {
                        let seed = c.next_seed();
                        let pixels = pixel_bytes(seed, IMG_STRIDE * IMG_H * f.bpp, f.bpp);
                        let output = geo.buffer(sink_bpp, 3, out_buffer);
                        c.push(Case {
                            name: format!(
                                "mem/{}/{sink}/{}/{tname}/{}/{}",
                                gather.name(),
                                p.name(),
                                if round_down { "down" } else { "up" },
                                geo.suffix
                            ),
                            force_highp: p == Precision::Highp,
                            compiled: false,
                            buffers: vec![output],
                            stages: gather_stages(gather, store, m, pixels, round_down),
                            runs: geo.rects.clone(),
                        });
                    }
                }
            }
            // A compiled pipeline on several rects.
            let seed = c.next_seed();
            let pixels = pixel_bytes(seed, IMG_STRIDE * IMG_H * f.bpp, f.bpp);
            let geo = &geos(&[], true)[2];
            let output = geo.buffer(sink_bpp, 3, out_buffer);
            c.push(Case {
                name: format!("mem/{}/{sink}/{}/compiled", gather.name(), p.name()),
                force_highp: p == Precision::Highp,
                compiled: true,
                buffers: vec![output],
                stages: gather_stages(gather, store, [0.9, 1.1, -1.5, -0.5], pixels, false),
                runs: geo.rects.clone(),
            });
        }
    }
}

fn gather_stages(
    gather: Op,
    store: Op,
    m: [f32; 4],
    pixels: Vec<u8>,
    round_down_at_integer: bool,
) -> Vec<StageSpec> {
    vec![
        StageSpec::new(Op::SeedShader),
        StageSpec::with(Op::MatrixScaleTranslate, Ctx::F32(m.to_vec())),
        StageSpec::with(
            gather,
            Ctx::Gather {
                pixels,
                stride: i32::try_from(IMG_STRIDE).expect("small"),
                #[allow(clippy::cast_precision_loss)] // 5 and 3
                width: IMG_W as f32,
                #[allow(clippy::cast_precision_loss)]
                height: IMG_H as f32,
                round_down_at_integer,
            },
        ),
        StageSpec::with(store, mem(0)),
    ]
}

/// `load_8888 store_r8` (`store_r8` takes the red channel of any source).
fn load_8888_store_r8(c: &mut Cases) {
    for &p in Precision::BOTH {
        for geo in geos(TAIL_WIDTHS, true) {
            let seed = c.next_seed();
            let input = geo.buffer(4, 3, |n| pixel_bytes(seed, n, 4));
            let output = geo.buffer(1, 5, out_buffer);
            c.push(Case {
                name: format!("mem/load_8888/store_r8/{}/{}", p.name(), geo.suffix),
                force_highp: p == Precision::Highp,
                compiled: geo.compiled,
                buffers: vec![input, output],
                stages: vec![
                    StageSpec::with(Op::Load8888, mem(0)),
                    StageSpec::with(Op::StoreR8, mem(1)),
                ],
                runs: geo.rects.clone(),
            });
        }
    }
}

/// `load_src srcover_rgba_8888`: source-over of the registers onto 8888 memory.
fn srcover_rgba_8888(c: &mut Cases) {
    for &p in Precision::BOTH {
        let kinds: Vec<Inputs> = Inputs::for_precision(p)
            .iter()
            .copied()
            .filter(|&i| i != Inputs::NanDst)
            .collect();
        for geo in geos(TAIL_WIDTHS, true) {
            let plain = !geo.origin && !geo.compiled;
            for &inputs in kinds.iter().take(if plain { kinds.len() } else { 1 }) {
                let seed = c.next_seed();
                let regs = register_input(inputs, seed);
                let dst = geo.buffer(4, 3, |n| pixel_bytes(seed ^ 0x55, n, 4));
                c.push(Case {
                    name: format!(
                        "mem/srcover_rgba_8888/{}/{}/{}",
                        p.name(),
                        inputs.name(),
                        geo.suffix
                    ),
                    force_highp: p == Precision::Highp,
                    compiled: geo.compiled,
                    buffers: vec![Buffer::new(regs), dst],
                    stages: vec![
                        StageSpec::with(Op::LoadSrc, Ctx::Ptr { slot: 0, offset: 0 }),
                        StageSpec::with(Op::SrcoverRgba8888, mem(1)),
                    ],
                    runs: geo.rects.clone(),
                });
            }
        }
    }
}

/// `load_src_rg` / `store_src_rg` (highp only; a `MemPtr` to `N` floats of `r`, then `N` of `g`).
fn src_rg(c: &mut Cases) {
    for inputs in [Inputs::Special, Inputs::NanSrc, Inputs::Bits] {
        for offset in [0u32, 32] {
            for &w in REGISTER_WIDTHS {
                let (s1, s2) = (c.next_seed(), c.next_seed());
                // load_src gives b and a; load_src_rg overwrites r and g from slot 1.
                c.push(Case {
                    name: format!("mem/load_src_rg/{}/o{offset}/w{w}", inputs.name()),
                    force_highp: true,
                    compiled: false,
                    buffers: vec![
                        Buffer::new(register_input(inputs, s1)),
                        Buffer::new(register_input(inputs, s2)),
                        crate::cases::output(REG_BYTES),
                    ],
                    stages: vec![
                        StageSpec::with(Op::LoadSrc, Ctx::Ptr { slot: 0, offset: 0 }),
                        StageSpec::with(Op::LoadSrcRg, Ctx::Ptr { slot: 1, offset }),
                        StageSpec::with(Op::StoreSrc, Ctx::Ptr { slot: 2, offset: 0 }),
                    ],
                    runs: vec![Rect::new(0, 0, w, 1)],
                });
                c.push(Case {
                    name: format!("mem/store_src_rg/{}/o{offset}/w{w}", inputs.name()),
                    force_highp: true,
                    compiled: false,
                    buffers: vec![
                        Buffer::new(register_input(inputs, s2)),
                        crate::cases::output(REG_BYTES),
                    ],
                    stages: vec![
                        StageSpec::with(Op::LoadSrc, Ctx::Ptr { slot: 0, offset: 0 }),
                        StageSpec::with(Op::StoreSrcRg, Ctx::Ptr { slot: 1, offset }),
                    ],
                    runs: vec![Rect::new(0, 0, w, 1)],
                });
            }
        }
    }
}
