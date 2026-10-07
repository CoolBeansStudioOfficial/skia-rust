// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

// The CPU draw layer against Skia itself. `oracle/draw/draw.cpp` runs the case script
// `draw_tests/cases.txt` (written by `oracle/draw/gen_cases.py`) through a real `SkBitmapDevice`
// (and, for two ops, a real `skcpu::Draw`): every shape x style x AA x stroke width x path effect
// x blend mode x clip x color type, the tiler's >8K devices, `readPixels`/`writePixels`. It
// prints a hash of the device after the draws, and `draw_tests/skia_dump*.txt` hold its output on
// each x86 code path (`oracle/draw/build.ps1`). This file interprets the same script with
// `BitmapDevice` and requires the same hashes on every tier it can run, natively or by model.
// The grammar is documented at the top of `oracle/draw/draw.cpp`.

use std::fmt::Write as _;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::PointMode;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::Color;
use skia_rust_core::color_priv::{get_packed_a32, get_packed_b32, get_packed_g32, get_packed_r32};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::device::Device;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::m44::M44;
use skia_rust_core::mask::{Mask, MaskFormat};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::PathEffect;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::{Op, Region};
use skia_rust_core::rrect::RRect;
use skia_rust_core::shaders;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_effects::{corner_path_effect, dash_path_effect};
use skia_rust_simd::Tier;
use skia_rust_simd::testing::{force_tier, oracle_selection};

use crate::bitmap_device::BitmapDevice;
use crate::draw::Draw;
use crate::raster_clip::RasterClip;

const CASES: &str = include_str!("draw_tests/cases.txt");
const DUMP_SSE2: &str = include_str!("draw_tests/skia_dump.txt");
const DUMP_SSE41: &str = include_str!("draw_tests/skia_dump_sse41.txt");
const DUMP_ML3: &str = include_str!("draw_tests/skia_dump_ml3.txt");
const DUMP_ML4: &str = include_str!("draw_tests/skia_dump_ml4.txt");
const DUMP_SCALAR: &str = include_str!("draw_tests/skia_dump_scalar.txt");

// ---- deterministic inputs (as in oracle/rp-builder/d4_blitters.cpp) -----------------------

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

// ---- parsing helpers ----------------------------------------------------------------------

fn f(tok: &str) -> f32 {
    tok.parse::<f32>()
        .unwrap_or_else(|e| panic!("bad float {tok:?}: {e}"))
}

fn i(tok: &str) -> i32 {
    tok.parse::<i32>()
        .unwrap_or_else(|e| panic!("bad int {tok:?}: {e}"))
}

fn u(tok: &str) -> u32 {
    tok.parse::<u32>()
        .unwrap_or_else(|e| panic!("bad uint {tok:?}: {e}"))
}

fn hex_color(tok: &str) -> Color {
    Color::from(u32::from_str_radix(tok, 16).unwrap_or_else(|e| panic!("bad color {tok:?}: {e}")))
}

fn rect_at(t: &[&str]) -> Rect {
    Rect::new(f(t[0]), f(t[1]), f(t[2]), f(t[3]))
}

fn clip_op(tok: &str) -> ClipOp {
    match tok {
        "intersect" => ClipOp::Intersect,
        "diff" => ClipOp::Difference,
        _ => panic!("bad clip op {tok:?}"),
    }
}

fn color_type(tok: &str) -> ColorType {
    match tok {
        "n32" => ColorType::N32,
        "a8" => ColorType::Alpha8,
        "gray8" => ColorType::Gray8,
        "rgb565" => ColorType::RGB565,
        "argb4444" => ColorType::ARGB4444,
        "rgbaf16" => ColorType::RGBAF16,
        "rgbaf32" => ColorType::RGBAF32,
        "rgba1010102" => ColorType::RGBA1010102,
        _ => panic!("bad color type {tok:?}"),
    }
}

fn alpha_type(tok: &str) -> AlphaType {
    match tok {
        "premul" => AlphaType::Premul,
        "opaque" => AlphaType::Opaque,
        "unpremul" => AlphaType::Unpremul,
        _ => panic!("bad alpha type {tok:?}"),
    }
}

fn color_space(tok: &str) -> Option<ColorSpace> {
    match tok {
        "srgb" => Some(ColorSpace::new_srgb()),
        "linear" => Some(ColorSpace::new_srgb_linear()),
        "none" => None,
        _ => panic!("bad color space {tok:?}"),
    }
}

fn floats(list: &str) -> Vec<f32> {
    list.split(',').map(f).collect()
}

/// `on,off,...@phase` as a dash effect.
fn dash(spec: &str) -> PathEffect {
    let (intervals, phase) = spec.split_once('@').expect("dash spec `on,off@phase`");
    dash_path_effect::new(&floats(intervals), f(phase)).expect("a valid dash")
}

fn corner(spec: &str) -> PathEffect {
    corner_path_effect::new(f(spec)).expect("a valid corner radius")
}

fn parse_paint(keys: &[&str]) -> Paint {
    let mut paint = Paint::default();
    for key in keys {
        let (k, v) = key.split_once('=').expect("key=value");
        match k {
            "color" => {
                paint.set_color(hex_color(v));
            }
            "aa" => {
                paint.set_anti_alias(v == "1");
            }
            "dither" => {
                paint.set_dither(v == "1");
            }
            "style" => {
                paint.set_style(match v {
                    "fill" => Style::Fill,
                    "stroke" => Style::Stroke,
                    "strokefill" => Style::StrokeAndFill,
                    _ => panic!("bad style {v:?}"),
                });
            }
            "width" => {
                paint.set_stroke_width(f(v));
            }
            "cap" => {
                paint.set_stroke_cap(match v {
                    "butt" => Cap::Butt,
                    "round" => Cap::Round,
                    "square" => Cap::Square,
                    _ => panic!("bad cap {v:?}"),
                });
            }
            "join" => {
                paint.set_stroke_join(match v {
                    "miter" => Join::Miter,
                    "round" => Join::Round,
                    "bevel" => Join::Bevel,
                    _ => panic!("bad join {v:?}"),
                });
            }
            "miter" => {
                paint.set_stroke_miter(f(v));
            }
            "blend" => {
                paint.set_blend_mode(BlendMode::VALUES[usize::try_from(u(v)).unwrap()]);
            }
            "shader" => {
                let c = v.strip_prefix("solid:").expect("shader=solid:AARRGGBB");
                paint.set_shader(shaders::color(hex_color(c)));
            }
            "pe" => {
                let (kind, rest) = v.split_once(':').expect("pe=kind:spec");
                let effect = match kind {
                    "dash" => dash(rest),
                    "corner" => corner(rest),
                    "sum_dash_corner" => {
                        let (d, c) = rest.rsplit_once(':').expect("dash:radius");
                        PathEffect::sum(dash(d), corner(c))
                    }
                    "compose_corner_dash" => {
                        let (d, c) = rest.rsplit_once(':').expect("dash:radius");
                        PathEffect::compose(corner(c), dash(d))
                    }
                    _ => panic!("bad path effect {kind:?}"),
                };
                paint.set_path_effect(effect);
            }
            _ => panic!("bad paint key {k:?}"),
        }
    }
    paint
}

/// `FILL VERBS...` as a path.
fn parse_path(t: &[&str]) -> Path {
    let mut b = PathBuilder::new();
    b.set_fill_type(match t[0] {
        "w" => PathFillType::Winding,
        "e" => PathFillType::EvenOdd,
        "iw" => PathFillType::InverseWinding,
        "ie" => PathFillType::InverseEvenOdd,
        other => panic!("bad fill type {other:?}"),
    });
    let mut k = 1;
    while k < t.len() {
        match t[k] {
            "M" => {
                b.move_to((f(t[k + 1]), f(t[k + 2])));
                k += 3;
            }
            "L" => {
                b.line_to((f(t[k + 1]), f(t[k + 2])));
                k += 3;
            }
            "Q" => {
                b.quad_to((f(t[k + 1]), f(t[k + 2])), (f(t[k + 3]), f(t[k + 4])));
                k += 5;
            }
            "K" => {
                b.conic_to(
                    (f(t[k + 1]), f(t[k + 2])),
                    (f(t[k + 3]), f(t[k + 4])),
                    f(t[k + 5]),
                );
                k += 6;
            }
            "C" => {
                b.cubic_to(
                    (f(t[k + 1]), f(t[k + 2])),
                    (f(t[k + 3]), f(t[k + 4])),
                    (f(t[k + 5]), f(t[k + 6])),
                );
                k += 7;
            }
            "Z" => {
                b.close();
                k += 1;
            }
            other => panic!("bad path verb {other:?}"),
        }
    }
    b.detach()
}

fn int_rects_region(t: &[&str]) -> Region {
    let n = usize::try_from(u(t[0])).unwrap();
    let mut rgn = Region::new();
    for k in 0..n {
        let rect = IRect::new(
            i(t[1 + 4 * k]),
            i(t[2 + 4 * k]),
            i(t[3 + 4 * k]),
            i(t[4 + 4 * k]),
        );
        rgn.op_rect(rect, Op::Union);
    }
    rgn
}

// ---- hashing ------------------------------------------------------------------------------

struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    fn byte(&mut self, b: u8) {
        self.0 ^= u64::from(b);
        self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// FNV-1a 64 over the `width x height` pixels, row by row without padding. `n32` pixels hash as
/// their logical A, R, G, B bytes so the result does not depend on the host's byte order.
#[allow(clippy::cast_possible_truncation)] // channels are bytes
fn hash_pixmap(pm: &Pixmap<'_>) -> String {
    let w = usize::try_from(pm.width()).unwrap();
    let h = usize::try_from(pm.height()).unwrap();
    let bpp = pm.info().bytes_per_pixel();
    let rb = pm.row_bytes();
    let bytes = pm.bytes().expect("pixels");
    let mut fnv = Fnv::new();
    // Debugging aid, as in oracle/draw/draw.cpp: DRAW_PIXELS=1 prints the hashed bytes.
    let dump = std::env::var_os("DRAW_PIXELS").is_some();
    for y in 0..h {
        let row = &bytes[y * rb..y * rb + w * bpp];
        if pm.color_type() == ColorType::N32 {
            for px in row.as_chunks::<4>().0 {
                let c = u32::from_ne_bytes(*px);
                for b in [
                    get_packed_a32(c) as u8,
                    get_packed_r32(c) as u8,
                    get_packed_g32(c) as u8,
                    get_packed_b32(c) as u8,
                ] {
                    fnv.byte(b);
                    if dump {
                        eprint!("{b:02x}");
                    }
                }
            }
        } else {
            for b in row {
                fnv.byte(*b);
                if dump {
                    eprint!("{b:02x}");
                }
            }
        }
    }
    format!("{:016x}", fnv.0)
}

fn hash_device(dev: &BitmapDevice) -> String {
    hash_pixmap(&dev.bitmap().pixmap())
}

/// Fills `bitmap` with `noise SEED` pixels.
#[allow(clippy::many_single_char_names)] // a, r, g, b, x, y, w, h, k as in the C++
fn noise(bitmap: &mut Bitmap, seed: u32) {
    let w = bitmap.width();
    let h = bitmap.height();
    for y in 0..h {
        for x in 0..w {
            let k = u32::try_from(y * w + x).unwrap();
            let a = r(seed, 3 * k) & 0xFF;
            let rr = r(seed, 3 * k + 1) & 0xFF;
            let g = r(seed, 3 * k + 2) & 0xFF;
            let b = (r(seed, 3 * k) >> 8) & 0xFF;
            #[allow(clippy::cast_possible_truncation)] // masked to bytes
            let c = Color::from_argb(a as u8, rr as u8, g as u8, b as u8);
            bitmap.erase(c, IRect::from_xywh(x, y, 1, 1));
        }
    }
}

// ---- the interpreter ----------------------------------------------------------------------

struct Interp<'o> {
    name: String,
    out: &'o mut Vec<String>,
    dev: Option<BitmapDevice>,
    paint: Paint,
    saves: u32,
    skipped: bool,
}

impl Interp<'_> {
    fn emit(&mut self, label: &str, rest: &str) {
        self.out.push(format!("{} {label} {rest}", self.name));
    }

    fn device_op(&mut self, t: &[&str]) {
        let (w, h) = (i(t[0]), i(t[1]));
        let ct = color_type(t[2]);
        let info = ImageInfo::new((w, h), ct, alpha_type(t[3]), color_space(t[4]));
        let pad = t.get(5).map_or(0, |p| usize::try_from(i(p)).unwrap());
        let mut bitmap = Bitmap::new();
        let ok = w > 0 && h > 0 && {
            let rb = (usize::try_from(w).unwrap() + pad) * ct.bytes_per_pixel();
            bitmap.try_alloc_pixels_info(&info, Some(rb))
        };
        if !ok {
            self.out.push(format!("{} skip", self.name));
            self.skipped = true;
            return;
        }
        self.dev = Some(BitmapDevice::with_props(bitmap, SurfaceProps::default()));
    }

    #[allow(clippy::too_many_lines)] // one arm per script op
    fn op(&mut self, line: &str) {
        let t: Vec<&str> = line.split(' ').collect();
        let (cmd, a) = (t[0], &t[1..]);
        if cmd == "device" {
            self.device_op(a);
            return;
        }
        if cmd == "paint" {
            self.paint = parse_paint(a);
            return;
        }
        let Some(dev) = self.dev.as_mut() else {
            panic!("`{cmd}` before `device`");
        };
        match cmd {
            "erase" => dev.bitmap_mut().erase_color(hex_color(a[0])),
            "noise" => noise(dev.bitmap_mut(), u(a[0])),
            "save" => {
                dev.push_clip_stack();
                self.saves += 1;
            }
            "restore" => {
                if self.saves > 0 {
                    dev.pop_clip_stack();
                    self.saves -= 1;
                }
            }
            "ctm" => {
                let m = if a[0] == "id" {
                    Matrix::new_identity()
                } else {
                    Matrix::new_all(
                        f(a[0]),
                        f(a[1]),
                        f(a[2]),
                        f(a[3]),
                        f(a[4]),
                        f(a[5]),
                        f(a[6]),
                        f(a[7]),
                        f(a[8]),
                    )
                };
                dev.state_mut().set_local_to_device(&M44::from(&m));
            }
            "cliprect" => dev.clip_rect(&rect_at(a), clip_op(a[4]), a[5] == "1"),
            "cliprrect" => {
                let rr = RRect::new_rect_xy(rect_at(a), f(a[4]), f(a[5]));
                dev.clip_rrect(&rr, clip_op(a[6]), a[7] == "1");
            }
            "cliprrect4" => {
                let rr = RRect::new_rect_radii(rect_at(a), &radii(&a[4..12]));
                dev.clip_rrect(&rr, clip_op(a[12]), a[13] == "1");
            }
            "clippath" => {
                let path = parse_path(&a[2..]);
                dev.clip_path(&path, clip_op(a[0]), a[1] == "1");
            }
            "clipregion" => {
                let rgn = int_rects_region(&a[1..]);
                dev.clip_region(&rgn, clip_op(a[0]));
            }
            "replaceclip" => {
                dev.replace_clip(&IRect::new(i(a[0]), i(a[1]), i(a[2]), i(a[3])));
            }
            "drawpaint" => dev.draw_paint(&self.paint),
            "rect" => dev.draw_rect(&rect_at(a), &self.paint),
            "oval" => dev.draw_oval(&rect_at(a), &self.paint),
            "rrect" => {
                let rr = RRect::new_rect_xy(rect_at(a), f(a[4]), f(a[5]));
                dev.draw_rrect(&rr, &self.paint);
            }
            "rrect4" => {
                let rr = RRect::new_rect_radii(rect_at(a), &radii(&a[4..12]));
                dev.draw_rrect(&rr, &self.paint);
            }
            "drrect" => {
                let outer = RRect::new_rect_xy(rect_at(&a[0..4]), f(a[4]), f(a[5]));
                let inner = RRect::new_rect_xy(rect_at(&a[6..10]), f(a[10]), f(a[11]));
                dev.draw_drrect(&outer, &inner, &self.paint);
            }
            "region" => {
                let rgn = int_rects_region(a);
                dev.draw_region(&rgn, &self.paint);
            }
            "path" => {
                let path = parse_path(a);
                dev.draw_path(&path, &self.paint);
            }
            "points" => {
                let mode = match a[0] {
                    "points" => PointMode::Points,
                    "lines" => PointMode::Lines,
                    "polygon" => PointMode::Polygon,
                    _ => panic!("bad point mode"),
                };
                let n = usize::try_from(u(a[1])).unwrap();
                let pts: Vec<Point> = (0..n)
                    .map(|k| Point::new(f(a[2 + 2 * k]), f(a[3 + 2 * k])))
                    .collect();
                dev.draw_points(mode, &pts, &self.paint);
            }
            "devmask" => self.devmask(a),
            "pathcov" => self.pathcov(a),
            "hash" => {
                let h = hash_device(dev);
                self.emit(a[0], &h);
            }
            "query" => {
                let q = format!(
                    "q empty={} rect={} aa={} wide={} bounds={}",
                    u8::from(dev.is_clip_empty()),
                    u8::from(dev.is_clip_rect()),
                    u8::from(dev.is_clip_anti_aliased()),
                    u8::from(dev.is_clip_wide_open()),
                    {
                        let b = dev.dev_clip_bounds();
                        format!("{},{},{},{}", b.left, b.top, b.right, b.bottom)
                    }
                );
                self.emit(a[0], &q);
            }
            "readpixels" => {
                let info = ImageInfo::new(
                    (i(a[3]), i(a[4])),
                    color_type(a[5]),
                    alpha_type(a[6]),
                    color_space(a[7]),
                );
                let rb = info.min_row_bytes();
                let mut buf = vec![0xA5u8; rb * usize::try_from(i(a[4])).unwrap()];
                let mut pm = Pixmap::new(&info, &mut buf, rb).expect("a valid destination");
                let ok = dev.read_pixels(&mut pm, i(a[1]), i(a[2]));
                let h = hash_pixmap(&pm);
                self.emit(a[0], &format!("rp ok={} {h}", u8::from(ok)));
            }
            "writepixels" => {
                let info = ImageInfo::new(
                    (i(a[3]), i(a[4])),
                    color_type(a[5]),
                    alpha_type(a[6]),
                    color_space(a[7]),
                );
                let mut src = Bitmap::new();
                assert!(src.try_alloc_pixels_info(&info, None));
                noise(&mut src, u(a[8]));
                let pm = src.peek_pixels().expect("pixels");
                let ok = dev.write_pixels(&pm, i(a[1]), i(a[2]));
                self.emit(a[0], &format!("wp ok={}", u8::from(ok)));
            }
            "end" => {
                let h = hash_device(dev);
                self.emit("end", &h);
            }
            other => panic!("unknown op {other:?}"),
        }
    }

    /// The raster clip of a direct `Draw` op (`rect:L,T,R,B` or `oval:L,T,R,B`).
    fn direct_clip(&self, spec: &str) -> RasterClip {
        let (kind, rest) = spec.split_once(':').expect("clip spec");
        match kind {
            "rect" => {
                let v: Vec<i32> = rest.split(',').map(i).collect();
                RasterClip::from_rect(&IRect::new(v[0], v[1], v[2], v[3]))
            }
            "oval" => {
                let dev = self.dev.as_ref().unwrap();
                let v = floats(rest);
                RasterClip::from_path(
                    &Path::oval(Rect::new(v[0], v[1], v[2], v[3]), None),
                    &IRect::from_wh(dev.bitmap().width(), dev.bitmap().height()),
                    true,
                )
            }
            _ => panic!("bad clip spec {spec:?}"),
        }
    }

    // devmask CLIP FMT ML MT MR MB SEED
    fn devmask(&mut self, a: &[&str]) {
        let rc = self.direct_clip(a[0]);
        let format = match a[1] {
            "a8" => MaskFormat::A8,
            "bw" => MaskFormat::BW,
            "lcd16" => MaskFormat::Lcd16,
            _ => panic!("bad mask format"),
        };
        let bounds = IRect::new(i(a[2]), i(a[3]), i(a[4]), i(a[5]));
        let seed = u(a[6]);
        let w = u32::try_from(bounds.width()).unwrap();
        let h = usize::try_from(bounds.height()).unwrap();
        let row_bytes = match format {
            MaskFormat::A8 => (w + 3) & !3,
            MaskFormat::BW => (((w + 7) >> 3) + 3) & !3,
            MaskFormat::Lcd16 => (w * 2 + 3) & !3,
            _ => unreachable!(),
        };
        let size = row_bytes as usize * h;
        #[allow(clippy::cast_possible_truncation)] // low byte, as in the C++
        let image: Vec<u8> = (0..size)
            .map(|k| (r(seed, u32::try_from(k).unwrap()) & 0xFF) as u8)
            .collect();
        let mask = Mask::new(&image, bounds, row_bytes, format);

        let dev = self.dev.as_mut().unwrap();
        let ctm = dev.state().local_to_device().clone();
        let dst = dev.access_pixels().expect("pixels");
        let mut draw = Draw::new(dst, &ctm, &rc);
        draw.draw_dev_mask(&mask, &self.paint, None);
    }

    // pathcov CLIP FILL VERBS...
    fn pathcov(&mut self, a: &[&str]) {
        let rc = self.direct_clip(a[0]);
        let path = parse_path(&a[1..]);
        let dev = self.dev.as_mut().unwrap();
        let ctm = dev.state().local_to_device().clone();
        let dst = dev.access_pixels().expect("pixels");
        let mut draw = Draw::new(dst, &ctm, &rc);
        draw.draw_path_coverage(&path, &self.paint, None);
    }
}

fn radii(t: &[&str]) -> [Vector; 4] {
    [
        Vector::new(f(t[0]), f(t[1])),
        Vector::new(f(t[2]), f(t[3])),
        Vector::new(f(t[4]), f(t[5])),
        Vector::new(f(t[6]), f(t[7])),
    ]
}

fn run_case(name: &str, lines: &[&str], out: &mut Vec<String>) {
    let mut it = Interp {
        name: name.to_string(),
        out,
        dev: None,
        paint: Paint::default(),
        saves: 0,
        skipped: false,
    };
    for line in lines {
        if it.skipped {
            break;
        }
        it.op(line);
    }
}

/// Runs the whole script, returning the output lines.
fn run_script(script: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut lines_of_case: Vec<&str> = Vec::new();
    let mut name = String::new();
    for line in script.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(n) = line.strip_prefix("case ") {
            name = n.to_string();
            lines_of_case.clear();
            continue;
        }
        lines_of_case.push(line);
        // Debugging aid: DRAW_ONLY=<case name> runs a single case.
        let only = std::env::var("DRAW_ONLY").ok();
        if line == "end" && only.as_deref().is_none_or(|o| o == name) {
            run_case(&name, &lines_of_case, &mut out);
            lines_of_case.clear();
        }
    }
    out
}

fn dump_for(tier: Tier) -> &'static str {
    match tier {
        Tier::Sse41 => DUMP_SSE41,
        Tier::Ml3 => DUMP_ML3,
        Tier::Ml4 => DUMP_ML4,
        Tier::Scalar => DUMP_SCALAR,
        _ => DUMP_SSE2,
    }
}

fn check_tier(tier: Tier) {
    // Native only where the host's rcp/rsqrt estimates are the oracle host's, else the model.
    let sel = oracle_selection(tier);
    let _guard = force_tier(sel).expect("a checked selection");
    let want: Vec<&str> = dump_for(tier).lines().filter(|l| !l.is_empty()).collect();
    let got = run_script(CASES);
    let mut report = String::new();
    if got.len() != want.len() {
        let _ = writeln!(
            report,
            "{sel}: {} output lines, the dump has {}",
            got.len(),
            want.len()
        );
    }
    let bad: Vec<String> = got
        .iter()
        .zip(&want)
        .enumerate()
        .filter(|(_, (g, w))| g != w)
        .map(|(k, (g, w))| format!("  line {k}: skia `{w}`, ours `{g}`"))
        .collect();
    if !bad.is_empty() {
        let _ = writeln!(report, "{sel}: {} mismatches (first shown):", bad.len());
        for b in bad.iter().take(30) {
            let _ = writeln!(report, "{b}");
        }
    }
    assert!(report.is_empty(), "{report}");
    assert_ne!(got.len(), 0);
}

// One test per tier, so the (long) replays run in parallel.
#[test]
fn draw_layer_matches_skia_scalar() {
    check_tier(Tier::Scalar);
}

#[test]
fn draw_layer_matches_skia_sse2() {
    check_tier(Tier::Sse2);
}

#[test]
fn draw_layer_matches_skia_sse41() {
    check_tier(Tier::Sse41);
}

#[test]
fn draw_layer_matches_skia_ml3() {
    check_tier(Tier::Ml3);
}

#[test]
fn draw_layer_matches_skia_ml4() {
    check_tier(Tier::Ml4);
}
