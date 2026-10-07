// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

// The legacy blitters against Skia itself: `oracle/rp-builder/d4_blitters.cpp` runs the real
// SkARGB32_*_Blitter, SkA8_*_Blitter, SkBlitter::Choose / ChooseSprite blitters and SkBlitRow
// procs on the deterministic inputs generated below and prints a hash of the resulting device per
// step; `skia_d4_dump.txt` holds that output (from `oracle/rp-builder/build-d4.ps1`, on the
// x64-sse2 oracle build at the baseline CPU tier). Every x86 tier and the scalar tier use the
// same kernels, so one dump checks all of them; `Neon` has different `blit_row_s32a_opaque` and
// `blit_mask_d32_a8` kernels and no oracle yet, so it is not compared here (its kernels are
// tested in skia-rust-simd).
//
// The input generators (`hash32`, `r`, `pm_pixel`, ...), the step list (`battery`) and the
// scenario names mirror the C++ one for one.

use std::collections::HashMap;
use std::fmt::Write;
use std::rc::Rc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::{Alpha, Color, PMColor};
use skia_rust_core::color_priv::{
    get_packed_a32, get_packed_b32, get_packed_g32, get_packed_r32, pack_argb32,
};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::draw_types::DrawCoverage;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::{Mask, MaskFormat};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_simd::testing::force_tier;
use skia_rust_simd::{Selection, Tier};

use crate::blit_row::{color32, factory32};
use crate::blitter::Blitter;
use crate::blitter_a8::choose_a8_blitter;
use crate::blitter_choose::choose_kind;
use crate::core_blitters::{Argb32BlackBlitter, Argb32Blitter, Argb32OpaqueBlitter};
use crate::sprite_blitter::choose_sprite;

// The oracle's output at the baseline CPU tier and under the ml3 and ml4 caps (they differ in the
// pipeline sprite lines).
const DUMP: &str = include_str!("skia_d4_dump.txt");
const DUMP_ML3: &str = include_str!("skia_d4_dump_ml3.txt");
const DUMP_ML4: &str = include_str!("skia_d4_dump_ml4.txt");

// ---- deterministic inputs ----------------------------------------------------------------

const N: usize = 23;
type Gen = fn(u32, u32) -> Px;

fn hash32(mut i: u32) -> u32 {
    i ^= i >> 16;
    i = i.wrapping_mul(0x7feb_352d);
    i ^= i >> 15;
    i = i.wrapping_mul(0x846c_a68b);
    i ^= i >> 16;
    i
}

fn r(seed: u32, k: u32) -> u32 {
    hash32(
        seed.wrapping_mul(0x9E37_79B1)
            .wrapping_add(k.wrapping_mul(0x85EB_CA77))
            .wrapping_add(0x1656_67B1),
    )
}

#[derive(Copy, Clone)]
struct Px {
    a: u32,
    r: u32,
    g: u32,
    b: u32,
}

// A valid premultiplied color (r, g, b <= a).
fn pm_pixel(seed: u32, k: u32) -> Px {
    let a = r(seed, 2 * k) & 0xFF;
    let c = r(seed, 2 * k + 1);
    Px {
        a,
        r: (c & 0xFF) * a / 255,
        g: ((c >> 8) & 0xFF) * a / 255,
        b: ((c >> 16) & 0xFF) * a / 255,
    }
}

fn opaque_pixel(seed: u32, k: u32) -> Px {
    let c = r(seed, 2 * k + 1);
    Px {
        a: 255,
        r: c & 0xFF,
        g: (c >> 8) & 0xFF,
        b: (c >> 16) & 0xFF,
    }
}

// Any bytes, premultiplied or not.
fn raw_pixel(seed: u32, k: u32) -> Px {
    let a = r(seed, 2 * k);
    let c = r(seed, 2 * k + 1);
    Px {
        a: a & 0xFF,
        r: c & 0xFF,
        g: (c >> 8) & 0xFF,
        b: (c >> 16) & 0xFF,
    }
}

const SENTINEL: Px = Px {
    a: 0xDE,
    r: 0xAD,
    g: 0xBE,
    b: 0xEF,
};

fn pack(p: Px) -> PMColor {
    pack_argb32(p.a, p.r, p.g, p.b)
}

#[allow(clippy::cast_possible_truncation)] // channels are bytes
fn unpack(c: PMColor) -> [u8; 4] {
    [
        get_packed_a32(c) as u8,
        get_packed_r32(c) as u8,
        get_packed_g32(c) as u8,
        get_packed_b32(c) as u8,
    ]
}

#[derive(Copy, Clone)]
enum Bg {
    Premul,
    Opaque,
}

fn bg_pixel(bg: Bg, k: u32) -> Px {
    match bg {
        Bg::Premul => pm_pixel(1, k),
        Bg::Opaque => opaque_pixel(2, k),
    }
}

const W: i32 = 19; // device width in pixels
const H: i32 = 4; // device height
const RB_PX: i32 = 21; // device row stride in pixels (2 pixels of padding)

// ---- hashing -----------------------------------------------------------------------------

fn fnv(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn expected(tier: Tier) -> HashMap<&'static str, &'static str> {
    let dump = match tier {
        Tier::Ml3 => DUMP_ML3,
        Tier::Ml4 => DUMP_ML4,
        _ => DUMP,
    };
    dump.lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| l.split_once(' ').expect("`name hash` lines"))
        .collect()
}

// ---- devices -----------------------------------------------------------------------------

struct N32Dev {
    w: i32,
    h: i32,
    rb_px: i32,
    bytes: Vec<u8>,
    cs: Option<ColorSpace>,
}

impl N32Dev {
    fn new(w: i32, h: i32, rb_px: i32, bg: Bg) -> N32Dev {
        let mut bytes = Vec::new();
        for y in 0..h {
            for x in 0..rb_px {
                let px = if x >= w {
                    SENTINEL
                } else {
                    bg_pixel(bg, u32::try_from(y * rb_px + x).unwrap())
                };
                bytes.extend_from_slice(&pack(px).to_ne_bytes());
            }
        }
        N32Dev {
            w,
            h,
            rb_px,
            bytes,
            cs: None,
        }
    }

    fn with_color_space(mut self, cs: ColorSpace) -> N32Dev {
        self.cs = Some(cs);
        self
    }

    fn info(&self) -> ImageInfo {
        ImageInfo::new_n32_premul((self.w, self.h), self.cs.clone())
    }

    fn rb(&self) -> usize {
        usize::try_from(self.rb_px).unwrap() * 4
    }

    fn logical(&self) -> Vec<u8> {
        self.bytes
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|c| unpack(u32::from_ne_bytes(*c)))
            .collect()
    }
}

struct A8Dev {
    w: i32,
    h: i32,
    rb: i32,
    bytes: Vec<u8>,
}

impl A8Dev {
    fn new(w: i32, h: i32, rb: i32) -> A8Dev {
        let mut bytes = Vec::new();
        for y in 0..h {
            for x in 0..rb {
                bytes.push(if x >= w {
                    0xA5
                } else {
                    (r(4, u32::try_from(y * rb + x).unwrap()) & 0xFF) as u8
                });
            }
        }
        A8Dev { w, h, rb, bytes }
    }
}

// ---- masks -------------------------------------------------------------------------------

struct Masks {
    a8: Vec<u8>,
    bw: Vec<u8>,
    lcd: Vec<u8>,
    argb: Vec<u8>,
    b: IRect,
    w: usize,
}

impl Masks {
    #[allow(clippy::many_single_char_names)] // mirrors the C++ generators
    fn new(b: IRect) -> Masks {
        let w = usize::try_from(b.width()).unwrap();
        let h = usize::try_from(b.height()).unwrap();
        let k = |i: usize| u32::try_from(i).unwrap();
        let a8 = (0..w * h)
            .map(|i| {
                let mut v = (r(6, k(i)) & 0xFF) as u8;
                if i % 7 == 0 {
                    v = 0;
                }
                if i % 11 == 0 {
                    v = 255;
                }
                v
            })
            .collect();
        let bw_rb = (w + 7) >> 3;
        let mut bw = vec![0u8; bw_rb * h];
        for y in 0..h {
            for x in 0..w {
                if r(7, k(y * w + x)) & 1 != 0 {
                    bw[y * bw_rb + (x >> 3)] |= 0x80 >> (x & 7);
                }
            }
        }
        let mut lcd = Vec::new();
        for i in 0..w * h {
            let mut v = (r(8, k(i)) & 0xFFFF) as u16;
            if i % 5 == 0 {
                v = 0;
            }
            if i % 13 == 0 {
                v = 0xFFFF;
            }
            lcd.extend_from_slice(&v.to_ne_bytes());
        }
        let mut argb = Vec::new();
        for i in 0..w * h {
            argb.extend_from_slice(&pack(pm_pixel(9, k(i))).to_ne_bytes());
        }
        Masks {
            a8,
            bw,
            lcd,
            argb,
            b,
            w,
        }
    }

    fn rb(&self, bytes_per_px: usize) -> u32 {
        u32::try_from(self.w * bytes_per_px).unwrap()
    }

    fn a8_mask(&self) -> Mask<'_> {
        Mask::new(&self.a8, self.b, self.rb(1), MaskFormat::A8)
    }

    fn bw_mask(&self) -> Mask<'_> {
        Mask::new(
            &self.bw,
            self.b,
            u32::try_from((self.w + 7) >> 3).unwrap(),
            MaskFormat::BW,
        )
    }

    fn lcd_mask(&self) -> Mask<'_> {
        Mask::new(&self.lcd, self.b, self.rb(2), MaskFormat::Lcd16)
    }

    fn argb_mask(&self) -> Mask<'_> {
        Mask::new(&self.argb, self.b, self.rb(4), MaskFormat::Argb32)
    }
}

// ---- the battery of blitter calls --------------------------------------------------------

type StepFn = Box<dyn Fn(&mut dyn Blitter)>;

struct Step {
    name: String,
    bg: Bg,
    run: StepFn,
}

#[allow(clippy::many_single_char_names)] // mirrors the C++ helper
fn antih(b: &mut dyn Blitter, y: i32, runs: &[(i16, Alpha)]) {
    let n: usize = runs.iter().map(|(c, _)| usize::try_from(*c).unwrap()).sum();
    let mut r = vec![0i16; n + 1];
    let mut a = vec![0 as Alpha; n + 1];
    let mut i = 0usize;
    for (count, alpha) in runs {
        r[i] = *count;
        a[i] = *alpha;
        i += usize::try_from(*count).unwrap();
    }
    b.blit_anti_h(0, y, &mut a, &mut r);
}

// Steps every blitter supports; `all_masks` adds the mask formats beyond A8.
fn battery(n32: bool, all_masks: bool) -> Vec<Step> {
    let mut s: Vec<Step> = Vec::new();
    let mut add = |name: &str, bg: Bg, run: StepFn| {
        s.push(Step {
            name: name.to_string(),
            bg,
            run,
        });
    };
    let pm = Bg::Premul;
    add("h_mid", pm, Box::new(|b| b.blit_h(2, 1, 13)));
    add(
        "h_full",
        pm,
        Box::new(|b| {
            b.blit_h(0, 0, 19);
            b.blit_h(0, 3, 1);
        }),
    );
    add("rect", pm, Box::new(|b| b.blit_rect(1, 0, 17, 3)));
    add("rect1", pm, Box::new(|b| b.blit_rect(18, 3, 1, 1)));
    for a in [0u8, 1, 77, 128, 254, 255] {
        add(
            &format!("v_{a}"),
            pm,
            Box::new(move |b| {
                b.blit_v(5, 0, 4, a);
                b.blit_v(18, 1, 2, a);
            }),
        );
    }
    add(
        "antih",
        pm,
        Box::new(|b| {
            antih(b, 0, &[(3, 0), (5, 255), (1, 17), (4, 128), (6, 254)]);
            antih(b, 1, &[(19, 200)]);
            antih(b, 2, &[(1, 255), (17, 3), (1, 255)]);
            antih(b, 3, &[(2, 1), (2, 254), (15, 90)]);
        }),
    );
    if n32 {
        let pairs = [
            (0, 0),
            (1, 254),
            (255, 128),
            (77, 200),
            (255, 255),
            (0, 255),
        ];
        for (i, (a0, a1)) in pairs.into_iter().enumerate() {
            add(
                &format!("anti2_{i}"),
                pm,
                Box::new(move |b| {
                    b.blit_anti_h2(3, 1, a0, a1);
                    b.blit_anti_v2(12, 0, a0, a1);
                }),
            );
        }
    }
    // (name, mask bounds, clip)
    let cases = [
        ("full", IRect::new(0, 0, 19, 4), IRect::new(0, 0, 19, 4)),
        ("sub", IRect::new(0, 0, 19, 4), IRect::new(3, 1, 16, 3)),
        ("offset", IRect::new(1, 0, 18, 4), IRect::new(1, 0, 18, 4)),
        ("offsub", IRect::new(1, 0, 18, 4), IRect::new(4, 1, 13, 3)),
        ("byte", IRect::new(0, 0, 19, 4), IRect::new(8, 0, 16, 2)),
        ("onebyte", IRect::new(0, 0, 19, 4), IRect::new(5, 2, 6, 3)),
        ("narrow", IRect::new(0, 0, 19, 4), IRect::new(9, 1, 12, 4)),
    ];
    for (name, bounds, clip) in cases {
        let m = Rc::new(Masks::new(bounds));
        let m1 = Rc::clone(&m);
        add(
            &format!("mask_a8_{name}"),
            pm,
            Box::new(move |b| b.blit_mask(&m1.a8_mask(), &clip)),
        );
        if !all_masks {
            continue;
        }
        let m2 = Rc::clone(&m);
        add(
            &format!("mask_bw_{name}"),
            pm,
            Box::new(move |b| b.blit_mask(&m2.bw_mask(), &clip)),
        );
        let m3 = Rc::clone(&m);
        add(
            &format!("mask_argb_{name}"),
            pm,
            Box::new(move |b| b.blit_mask(&m3.argb_mask(), &clip)),
        );
        let m4 = Rc::clone(&m);
        add(
            &format!("mask_lcd_{name}"),
            Bg::Opaque,
            Box::new(move |b| b.blit_mask(&m4.lcd_mask(), &clip)),
        );
    }
    s
}

fn paint_for(color: u32, mode: BlendMode) -> Paint {
    let mut p = Paint::default();
    p.set_color(Color::new(color));
    p.set_blend_mode(mode);
    p
}

// What a scenario's steps produced: (step name, device hash, device pixels).
type Results = Vec<(String, String, Vec<u8>)>;

// Runs every step of `battery` against the blitter that `make` creates on a fresh N32 device.
fn run_n32(
    scenario: &str,
    all_masks: bool,
    make: impl for<'a> Fn(Pixmap<'a>, &'a ArenaAlloc) -> Option<Box<dyn Blitter + 'a>>,
    out: &mut Results,
) {
    for st in battery(true, all_masks) {
        let mut dev = N32Dev::new(W, H, RB_PX, st.bg);
        let alloc = ArenaAlloc::new();
        {
            let info = dev.info();
            let rb = dev.rb();
            let pm = Pixmap::new(&info, &mut dev.bytes, rb).unwrap();
            if let Some(mut b) = make(pm, &alloc) {
                (st.run)(&mut *b);
            }
        }
        let logical = dev.logical();
        out.push((format!("{scenario}/{}", st.name), fnv(&logical), logical));
    }
}

fn run_a8(
    scenario: &str,
    make: impl for<'a> Fn(Pixmap<'a>, &'a ArenaAlloc) -> Option<Box<dyn Blitter + 'a>>,
    out: &mut Results,
) {
    let mut steps = battery(false, false);
    // Non-A8 masks use the blitter's default.
    let m = Rc::new(Masks::new(IRect::new(0, 0, 19, 4)));
    steps.push(Step {
        name: "mask_bw_sub".to_string(),
        bg: Bg::Premul,
        run: Box::new(move |b| b.blit_mask(&m.bw_mask(), &IRect::new(3, 1, 16, 3))),
    });
    for st in steps {
        let mut dev = A8Dev::new(W, H, W + 4);
        let alloc = ArenaAlloc::new();
        {
            let info = ImageInfo::new_a8((dev.w, dev.h));
            let rb = usize::try_from(dev.rb).unwrap();
            let pm = Pixmap::new(&info, &mut dev.bytes, rb).unwrap();
            if let Some(mut b) = make(pm, &alloc) {
                (st.run)(&mut *b);
            }
        }
        out.push((
            format!("{scenario}/{}", st.name),
            fnv(&dev.bytes),
            dev.bytes.clone(),
        ));
    }
}

// Every scenario of d4_blitters.cpp, in its order.
#[allow(clippy::too_many_lines)] // one function, as the C++ main
fn run_all() -> Results {
    let mut out = Results::new();

    // ---- the ARGB32 blitters, constructed directly ----
    for c in [
        0x8033_6699u32,
        0x01FF_FFFF,
        0xFE01_02FD,
        0xC080_4020,
        0x0011_2233,
        0x7F7F_7F7F,
    ] {
        let paint = paint_for(c, BlendMode::SrcOver);
        run_n32(
            &format!("argb32/{c:08x}"),
            true,
            |pm, _alloc| Some(Box::new(Argb32Blitter::new(pm, &paint))),
            &mut out,
        );
    }
    for c in [0xFF33_6699u32, 0xFFFF_FFFF, 0xFF01_0203] {
        let paint = paint_for(c, BlendMode::SrcOver);
        run_n32(
            &format!("opaque/{c:08x}"),
            true,
            |pm, _alloc| Some(Box::new(Argb32OpaqueBlitter::new(pm, &paint))),
            &mut out,
        );
    }
    let black = paint_for(0xFF00_0000, BlendMode::SrcOver);
    run_n32(
        "black",
        true,
        |pm, _alloc| Some(Box::new(Argb32BlackBlitter::new(pm, &black))),
        &mut out,
    );

    // ---- SkBlitter::Choose on N32 devices (the paints the legacy blitters draw) ----
    let choose_cases = [
        ("srcover_translucent", 0x8033_6699u32, BlendMode::SrcOver),
        ("srcover_opaque", 0xFF33_6699, BlendMode::SrcOver),
        ("srcover_black", 0xFF00_0000, BlendMode::SrcOver),
        ("srcover_alpha0", 0x0033_6699, BlendMode::SrcOver),
        ("src_opaque", 0xFF33_6699, BlendMode::Src),
        ("src_black", 0xFF00_0000, BlendMode::Src),
        ("dst", 0x8033_6699, BlendMode::Dst),
    ];
    for (name, color, mode) in choose_cases {
        let paint = paint_for(color, mode);
        run_n32(
            &format!("choose/{name}"),
            false,
            |pm, alloc| {
                Some(
                    choose_kind(
                        pm,
                        &Matrix::new_identity(),
                        &paint,
                        alloc,
                        DrawCoverage::No,
                        None,
                        &Rect::from_wh(19.0, 4.0),
                        false,
                    )
                    .into_blitter(),
                )
            },
            &mut out,
        );
    }

    // ---- A8 ----
    for alpha in [0xFFu32, 0x80, 0x33, 0x00] {
        for mode in [BlendMode::SrcOver, BlendMode::Src] {
            let color = (alpha << 24) | 0x0012_3456;
            let name = format!(
                "a8/{}{alpha}",
                if mode == BlendMode::Src {
                    "src_"
                } else {
                    "srcover_"
                }
            );
            let paint = paint_for(color, mode);
            run_a8(
                &name,
                |pm, _alloc| {
                    choose_a8_blitter(pm, &Matrix::new_identity(), &paint, DrawCoverage::No, None)
                },
                &mut out,
            );
        }
    }
    let black = paint_for(0xFF00_0000, BlendMode::SrcOver);
    run_a8(
        "a8/coverage",
        |pm, _alloc| {
            choose_a8_blitter(pm, &Matrix::new_identity(), &black, DrawCoverage::Yes, None)
        },
        &mut out,
    );
    run_a8(
        "a8/choose_coverage",
        |pm, alloc| {
            Some(
                choose_kind(
                    pm,
                    &Matrix::new_identity(),
                    &black,
                    alloc,
                    DrawCoverage::Yes,
                    None,
                    &Rect::from_wh(19.0, 4.0),
                    false,
                )
                .into_blitter(),
            )
        },
        &mut out,
    );

    // ---- sprites ----
    let sprites = [
        ("memcpy_src", 0xFFu32, BlendMode::Src, false),
        ("memcpy_srcover_opaque", 0xFF, BlendMode::SrcOver, true),
        ("d32_srcover", 0xFF, BlendMode::SrcOver, false),
        ("d32_alpha_translucent", 0x80, BlendMode::SrcOver, false),
        ("d32_alpha_opaque", 0x80, BlendMode::SrcOver, true),
        ("d32_alpha_1", 0x01, BlendMode::SrcOver, false),
    ];
    let calls = [
        ("full", 3, 2, 8, 5),
        ("sub", 5, 3, 4, 2),
        ("one", 10, 6, 1, 1),
        ("row", 3, 4, 8, 1),
    ];
    for (name, alpha, mode, opaque_source) in sprites {
        for (call, x, y, w, h) in calls {
            let mut dst = N32Dev::new(W, 8, RB_PX, Bg::Premul);
            let alloc = ArenaAlloc::new();
            let (sw, sh, srb_px) = (8i32, 5i32, 11i32);
            let mut src = Vec::new();
            for sy in 0..sh {
                for sx in 0..srb_px {
                    let k = u32::try_from(sy * srb_px + sx).unwrap();
                    let px = if sx >= sw {
                        Px {
                            a: 0xCA,
                            r: 0xFE,
                            g: 0xF0,
                            b: 0x0D,
                        }
                    } else if opaque_source {
                        opaque_pixel(5, k)
                    } else {
                        pm_pixel(5, k)
                    };
                    src.extend_from_slice(&pack(px).to_ne_bytes());
                }
            }
            let at = if opaque_source {
                AlphaType::Opaque
            } else {
                AlphaType::Premul
            };
            let sinfo = ImageInfo::new_n32((sw, sh), at, None);
            let spm =
                Pixmap::new_readonly(&sinfo, &src, usize::try_from(srb_px).unwrap() * 4).unwrap();
            {
                let dinfo = dst.info();
                let rb = dst.rb();
                let dpm = Pixmap::new(&dinfo, &mut dst.bytes, rb).unwrap();
                let p = paint_for((alpha << 24) | 0x0033_6699, mode);
                let mut b = choose_sprite(dpm, &p, spm, 3, 2, &alloc, None, false)
                    .unwrap_or_else(|| panic!("no sprite blitter for {name}"));
                b.blit_rect(x, y, w, h);
            }
            let logical = dst.logical();
            out.push((format!("sprite/{name}/{call}"), fnv(&logical), logical));
        }
    }

    // ---- sprites drawn by SkRasterPipelineSpriteBlitter ----
    let rp_sprites = [
        (
            "plus_opaque",
            ColorType::N32,
            true,
            0xFFu32,
            BlendMode::Plus,
            false,
        ),
        (
            "plus_translucent",
            ColorType::N32,
            false,
            0x80,
            BlendMode::Plus,
            false,
        ),
        (
            "a8_srcover",
            ColorType::Alpha8,
            false,
            0xC0,
            BlendMode::SrcOver,
            false,
        ),
        (
            "srgb_linear",
            ColorType::N32,
            false,
            0xFF,
            BlendMode::SrcOver,
            true,
        ),
        (
            "multiply",
            ColorType::N32,
            false,
            0xFF,
            BlendMode::Multiply,
            false,
        ),
    ];
    let rp_calls = [("full", 3, 2, 8, 5), ("sub", 5, 3, 4, 2)];
    for (name, src_ct, opaque_source, alpha, mode, linear_dst) in rp_sprites {
        for (call, x, y, w, h) in rp_calls {
            let mut dst = N32Dev::new(W, 8, RB_PX, Bg::Premul);
            if linear_dst {
                dst = dst.with_color_space(ColorSpace::new_srgb_linear());
            }
            let alloc = ArenaAlloc::new();
            let (sw, sh) = (8i32, 5i32);
            let mut src = Vec::new();
            let (sinfo, srb) = if src_ct == ColorType::Alpha8 {
                for sy in 0..sh {
                    for sx in 0..11 {
                        let k = u32::try_from(sy * 11 + sx).unwrap();
                        src.push(if sx >= sw {
                            0xCA
                        } else {
                            (r(5, k) & 0xFF) as u8
                        });
                    }
                }
                (ImageInfo::new_a8((sw, sh)), 11usize)
            } else {
                let srb_px = 11i32;
                for sy in 0..sh {
                    for sx in 0..srb_px {
                        let k = u32::try_from(sy * srb_px + sx).unwrap();
                        let px = if sx >= sw {
                            Px {
                                a: 0xCA,
                                r: 0xFE,
                                g: 0xF0,
                                b: 0x0D,
                            }
                        } else if opaque_source {
                            opaque_pixel(5, k)
                        } else {
                            pm_pixel(5, k)
                        };
                        src.extend_from_slice(&pack(px).to_ne_bytes());
                    }
                }
                let at = if opaque_source {
                    AlphaType::Opaque
                } else {
                    AlphaType::Premul
                };
                (ImageInfo::new_n32((sw, sh), at, None), 44usize)
            };
            let spm = Pixmap::new_readonly(&sinfo, &src, srb).unwrap();
            {
                let dinfo = dst.info();
                let rb = dst.rb();
                let dpm = Pixmap::new(&dinfo, &mut dst.bytes, rb).unwrap();
                let p = paint_for((alpha << 24) | 0x0033_6699, mode);
                let mut b = choose_sprite(dpm, &p, spm, 3, 2, &alloc, None, false)
                    .unwrap_or_else(|| panic!("no sprite blitter for {name}"));
                b.blit_rect(x, y, w, h);
            }
            let logical = dst.logical();
            out.push((format!("sprite_rp/{name}/{call}"), fnv(&logical), logical));
        }
    }

    // ---- SkBlitRow ----
    let srcs: [(&str, Gen); 2] = [("pm", pm_pixel), ("raw", raw_pixel)];
    for (rc_name, gen_px) in srcs {
        for flags in 0..4u32 {
            let alphas: Vec<u32> = if flags == 0 || flags == 2 {
                vec![255]
            } else {
                vec![0, 1, 77, 128, 254, 255]
            };
            for alpha in alphas {
                for n in [1usize, 3, 4, 5, 8, 23] {
                    let mut dst: Vec<u32> = (0..N)
                        .map(|i| pack(raw_pixel(10, u32::try_from(i).unwrap())))
                        .collect();
                    let src: Vec<u32> = (0..N)
                        .map(|i| pack(gen_px(11, u32::try_from(i).unwrap())))
                        .collect();
                    factory32(flags)(&mut dst[..n], &src, alpha);
                    let logical: Vec<u8> = dst.iter().flat_map(|c| unpack(*c)).collect();
                    out.push((
                        format!("blitrow/{rc_name}/f{flags}/a{alpha}/n{n}"),
                        fnv(&logical),
                        logical,
                    ));
                }
            }
        }
    }
    for c in [
        0x0000_0000u32,
        0xFF33_6699,
        0x8033_6699,
        0x0101_0101,
        0xFEFD_FCFB,
    ] {
        // c is given as logical a,r,g,b; pack it.
        let pmc = pack_argb32(c >> 24, (c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF);
        for n in [1usize, 3, 4, 5, 8, 23] {
            let mut dst: Vec<u32> = (0..N)
                .map(|i| pack(pm_pixel(12, u32::try_from(i).unwrap())))
                .collect();
            color32(&mut dst[..n], pmc);
            let logical: Vec<u8> = dst.iter().flat_map(|c| unpack(*c)).collect();
            out.push((format!("color32/{c:08x}/n{n}"), fnv(&logical), logical));
        }
    }
    out
}

// The tiers whose kernels the dump covers: every one but `Neon`, natively where this host has it
// and by its model otherwise.
fn x86_and_scalar_tiers() -> Vec<Selection> {
    Tier::ALL
        .into_iter()
        .filter(|t| *t != Tier::Neon)
        .map(skia_rust_simd::testing::oracle_selection)
        .collect()
}

#[test]
fn legacy_blitters_match_skia_on_every_tier() {
    let mut checked = 0usize;
    for sel in x86_and_scalar_tiers() {
        let _guard = force_tier(sel).expect("a checked selection");
        let expected = expected(sel.tier);
        let results = run_all();
        assert_eq!(
            expected.len(),
            results.len(),
            "{sel}: the dump and the test disagree on the number of steps"
        );
        let mut failures = Vec::new();
        for (name, hash, pixels) in &results {
            // The pipelines' lowp/highp results are recorded for the Sse2 (baseline), Ml3 and Ml4
            // paths only (as for the raster pipeline blitter tests).
            if name.starts_with("sprite_rp/")
                && !matches!(sel.tier, Tier::Sse2 | Tier::Ml3 | Tier::Ml4)
            {
                continue;
            }
            let Some(want) = expected.get(name.as_str()) else {
                failures.push(format!("{name}: not in the dump"));
                continue;
            };
            if want != hash {
                let hex: String = pixels.iter().fold(String::new(), |mut s, b| {
                    let _ = write!(s, "{b:02x}");
                    s
                });
                failures.push(format!("{name}: skia {want}, ours {hash}\n  ours {hex}"));
            }
            checked += 1;
        }
        assert!(failures.is_empty(), "{sel}:\n{}", failures.join("\n"));
    }
    assert!(checked > 0);
}
