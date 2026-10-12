// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The seed corpora (docs/design/fuzz.md §2): generated deterministically here, committed under
//! `fuzz/corpus/<Name>/`, and checked against this generator by a test.
//!
//! Skia's own corpora live in a cloud bucket, not in its repository, so they cannot be pinned.
//! For the API fuzzers (all but the encoders) every input is valid, so the seeds are built to
//! steer the fuzzer: each one picks one top-level branch with well-formed values ([`SeedWriter`]
//! is the inverse of [`crate::Fuzz`]), and a few `SkRandom` byte strings cover the rest. The
//! encoder seeds are real images in `_MakeEncoderCorpus`'s format (width, height, N32 premul
//! pixels), decoded from Skia's `resources/images`, plus a synthetic image that needs no
//! resources, each followed by the encoder's option bytes.
//!
//! Regenerate with `cargo run -p skia-rust-fuzz --bin fuzz-seeds` after changing this file.

use skia_rust_codec::images::deferred_from_encoded_data;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::random::Random;

/// One seed: its file name in `fuzz/corpus/<Name>/` and its bytes.
pub type Seed = (String, Vec<u8>);

/// Builds a seed value by value, in the order the fuzzer reads them (little-endian, the byte
/// order of every platform skia-rust targets).
#[derive(Debug, Default, Clone)]
pub struct SeedWriter(Vec<u8>);

impl SeedWriter {
    /// An empty seed.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The bytes written so far.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.0
    }

    /// A `uint8_t` (also an enum read as `uint8_t`).
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }

    /// A `bool` (one byte, low bit).
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.u8(u8::from(v))
    }

    /// A `uint16_t`.
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// An `int` (also an enum whose underlying type is `int`).
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// A `uint32_t` (also `SkColor`).
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// A `float`.
    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// Several `float`s.
    pub fn f32s(&mut self, vs: &[f32]) -> &mut Self {
        for &v in vs {
            self.f32(v);
        }
        self
    }

    /// A `double`.
    pub fn f64(&mut self, v: f64) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// Raw bytes.
    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.0.extend_from_slice(v);
        self
    }
}

/// `len` bytes from `SkRandom(seed)`.
#[must_use]
pub fn random_bytes(seed: u32, len: usize) -> Vec<u8> {
    let mut rand = Random::new(seed);
    let mut out = Vec::with_capacity(len + 3);
    while out.len() < len {
        out.extend_from_slice(&rand.next_u().to_le_bytes());
    }
    out.truncate(len);
    out
}

/// `count` random seeds named `random-<i>`, of lengths cycling through `lens`.
fn random_seeds(salt: u32, count: u32, lens: &[usize]) -> Vec<Seed> {
    (0..count)
        .map(|i| {
            let len = lens[i as usize % lens.len()];
            (format!("random-{i}"), random_bytes(salt * 1000 + i, len))
        })
        .collect()
}

fn seed(name: &str, w: &SeedWriter) -> Seed {
    (name.to_owned(), w.clone().finish())
}

/// The seeds of fuzzer `name`, or `None` if it has no generator. Encoder seeds from Skia's
/// resources are left out when the resources are not available (see [`resource_seed_names`]).
#[must_use]
pub fn generate(name: &str) -> Option<Vec<Seed>> {
    let mut seeds = match name {
        "CubicRoots" => cubic_roots(),
        "QuadRoots" => quad_roots(),
        "RegionOp" => region_op(),
        "ParsePath" => parse_path(),
        "PathMeasure" => path_measure(),
        "PolyUtils" => poly_utils(),
        "Pathop" => pathop(),
        "LegacyChromiumPathop" => legacy_chromium_pathop(),
        "Gradients" => gradients(),
        "DrawFunctions" => draw_functions(),
        "PNGEncoder" | "PNGRustEncoder" | "JPEGEncoder" | "WEBPEncoder" => encoder(name),
        "NullCanvas"
        | "RasterN32Canvas"
        | "RasterN32CanvasViaSerialization"
        | "PDFCanvas"
        | "SVGCanvas" => canvas_commands(),
        "ImageFilter" | "SerializedImageFilter" => image_filter_types(),
        _ => return None,
    };
    seeds.sort_by(|a, b| a.0.cmp(&b.0));
    Some(seeds)
}

// ---------------------------------------------------------------------------------------------
// Roots

fn cubic_roots() -> Vec<Seed> {
    // (t - 0.25)(t - 0.5)(t - 0.75), a double root, one real root, degenerate degrees, extremes.
    let cases: [(&str, [f64; 4]); 10] = [
        ("three-roots", [1.0, -1.5, 0.6875, -0.09375]),
        ("double-root", [1.0, -1.0, 0.25, 0.0]),
        ("one-root", [1.0, 0.0, 1.0, -0.5]),
        ("quadratic", [0.0, 1.0, -1.0, 0.1875]),
        ("linear", [0.0, 0.0, 2.0, -1.0]),
        ("zero", [0.0, 0.0, 0.0, 0.0]),
        ("huge", [1e300, -3e300, 3e300, -1e300]),
        ("tiny", [1e-300, -1e-310, 5e-324, 0.0]),
        ("nan", [f64::NAN, 1.0, 1.0, 1.0]),
        ("inf", [1.0, f64::INFINITY, 0.0, -1.0]),
    ];
    let mut seeds: Vec<Seed> = cases
        .iter()
        .map(|(name, coeffs)| {
            let mut w = SeedWriter::new();
            for &c in coeffs {
                w.f64(c);
            }
            seed(name, &w)
        })
        .collect();
    seeds.extend(random_seeds(1, 6, &[32]));
    seeds
}

fn quad_roots() -> Vec<Seed> {
    let cases: [(&str, [f64; 3]); 8] = [
        ("two-roots", [1.0, -1.0, 0.1875]),
        ("double-root", [1.0, -1.0, 0.25]),
        ("no-roots", [1.0, 0.0, 1.0]),
        ("linear", [0.0, 2.0, -1.0]),
        ("zero", [0.0, 0.0, 0.0]),
        ("huge", [1e300, -3e300, 1e300]),
        ("nan", [f64::NAN, 1.0, 1.0]),
        ("inf", [1.0, f64::INFINITY, -1.0]),
    ];
    let mut seeds: Vec<Seed> = cases
        .iter()
        .map(|(name, coeffs)| {
            let mut w = SeedWriter::new();
            for &c in coeffs {
                w.f64(c);
            }
            seed(name, &w)
        })
        .collect();
    seeds.extend(random_seeds(2, 6, &[24]));
    seeds
}

// ---------------------------------------------------------------------------------------------
// Region

/// `FuzzNiceRegion` input: a count, then (l, t, r, b, op) per rect.
fn region_bytes(w: &mut SeedWriter, rects: &[([i32; 4], i32)]) {
    w.u8(u8::try_from(rects.len()).expect("at most 255 rects"));
    for (r, op) in rects {
        for &v in r {
            w.i32(v);
        }
        w.i32(*op);
    }
}

fn region_op() -> Vec<Seed> {
    let mut seeds = Vec::new();
    for op in 0..=5 {
        let mut w = SeedWriter::new();
        region_bytes(
            &mut w,
            &[
                ([0, 0, 100, 100], 2),
                ([50, 50, 150, 150], op),
                ([25, -10, 75, 200], op),
                ([-2_147_483_646, 0, 2_147_483_646, 1], op),
            ],
        );
        seeds.push(seed(&format!("op-{op}"), &w));
    }
    // A comb: many thin rects unioned, then xored with a big one.
    let mut rects: Vec<([i32; 4], i32)> = (0..40).map(|i| ([i * 4, 0, i * 4 + 2, 64], 2)).collect();
    rects.push(([10, 10, 150, 50], 3));
    rects.push(([0, 30, 200, 31], 0));
    let mut w = SeedWriter::new();
    region_bytes(&mut w, &rects);
    seeds.push(seed("comb", &w));
    seeds.extend(random_seeds(3, 6, &[64, 512, 2048]));
    seeds
}

// ---------------------------------------------------------------------------------------------
// ParsePath

/// `add_white` with no whitespace (`reps` 0) or one space (`reps` 1, `gWhiteSpace[8]`).
fn parse_white(w: &mut SeedWriter, space: bool) {
    if space {
        w.u8(1).u8(8);
    } else {
        w.u8(0);
    }
}

/// `add_comma`: no whitespace, a comma, then `add_some_white` with none.
fn parse_comma(w: &mut SeedWriter) {
    parse_white(w, false);
    w.bool(true);
    for _ in 0..10 {
        parse_white(w, false);
    }
}

/// `MakeRandomParsePathPiece` input for command `legal_index` (an index into `gLegal`), read
/// exactly as the fuzzer reads it, with a space before every coordinate.
fn parse_path_piece(w: &mut SeedWriter, legal_index: u8, lower: bool, reps: u8, start: f32) {
    // `fScalars` of `gLegal`.
    const SCALARS: [u8; 10] = [2, 1, 1, 2, 4, 2, 6, 4, 4, 0];
    let scalars = SCALARS[usize::from(legal_index)];
    w.u8(legal_index);
    parse_white(w, false);
    w.bool(lower).u8(reps);
    for rep in 0..reps {
        for index in 0..scalars {
            w.f32(start + f32::from(rep * 7 + index * 3));
            parse_white(w, true);
            if rep + 1 < reps && index + 1 < scalars {
                parse_comma(w);
            } else {
                for _ in 0..10 {
                    parse_white(w, false);
                }
            }
            if legal_index == 8 && index == 1 {
                // 'A': the rotation and the two flags.
                w.f32(30.0);
                parse_comma(w);
                w.bool(true);
                parse_comma(w);
                w.bool(false);
                parse_comma(w);
            }
        }
    }
}

fn parse_path() -> Vec<Seed> {
    let mut seeds = Vec::new();
    // Every command, upper and lower case, one to three reps.
    let mut w = SeedWriter::new();
    w.u8(40);
    for i in 0..40u8 {
        parse_path_piece(&mut w, i % 10, i % 3 == 1, 1 + i % 3, f32::from(i));
    }
    seeds.push(seed("all-commands", &w));
    // A short closed triangle: M, L, L, Z.
    let mut w = SeedWriter::new();
    w.u8(4);
    for (index, start) in [(0u8, 10.0f32), (3, 50.0), (3, 20.0), (9, 0.0)] {
        parse_path_piece(&mut w, index, false, 1, start);
    }
    seeds.push(seed("triangle", &w));
    seeds.extend(random_seeds(4, 8, &[128, 1024, 4096]));
    seeds
}

// ---------------------------------------------------------------------------------------------
// Paths

/// `FuzzEvilPath` / `BuildPath` input covering every verb, ending with `kDone_Verb`.
fn evil_path_bytes(w: &mut SeedWriter, scale: f32) {
    w.u8(0).f32s(&[10.0 * scale, 10.0 * scale]);
    w.u8(1).f32s(&[90.0 * scale, 15.0 * scale]);
    w.u8(2)
        .f32s(&[95.0 * scale, 60.0 * scale, 60.0 * scale, 90.0 * scale]);
    w.u8(3)
        .f32s(&[20.0 * scale, 95.0 * scale, 5.0 * scale, 50.0 * scale, 0.7]);
    w.u8(4).f32s(&[
        0.0,
        30.0 * scale,
        50.0 * scale,
        -20.0 * scale,
        70.0 * scale,
        40.0 * scale,
    ]);
    w.u8(5);
    w.u8(0).f32s(&[40.0 * scale, 40.0 * scale]);
    w.u8(1).f32s(&[60.0 * scale, 40.0 * scale]);
    w.u8(1).f32s(&[50.0 * scale, 60.0 * scale]);
    w.u8(5);
    w.u8(6);
}

fn path_measure() -> Vec<Seed> {
    let mut seeds = Vec::new();
    for (name, bits, scale) in [
        ("plain", 0u8, 1.0f32),
        ("forced-closed", 7, 1.0),
        ("large", 2, 900.0),
        ("too-large", 1, 1e6),
    ] {
        let mut w = SeedWriter::new();
        w.u8(bits).f32s(&[5.0, 10.0, 120.0, 30.0, -5.0, 1e9]);
        evil_path_bytes(&mut w, scale);
        seeds.push(seed(name, &w));
    }
    seeds.extend(random_seeds(5, 6, &[64, 256, 1024]));
    seeds
}

fn poly_utils() -> Vec<Seed> {
    let polygons: [(&str, &[[f32; 2]]); 4] = [
        (
            "convex",
            &[[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]],
        ),
        (
            "concave",
            &[
                [0.0, 0.0],
                [100.0, 0.0],
                [100.0, 100.0],
                [50.0, 40.0],
                [0.0, 100.0],
            ],
        ),
        (
            "bowtie",
            &[[0.0, 0.0], [100.0, 100.0], [100.0, 0.0], [0.0, 100.0]],
        ),
        (
            "star",
            &[
                [50.0, 0.0],
                [61.0, 35.0],
                [98.0, 35.0],
                [68.0, 57.0],
                [79.0, 91.0],
                [50.0, 70.0],
                [21.0, 91.0],
                [32.0, 57.0],
                [2.0, 35.0],
                [39.0, 35.0],
            ],
        ),
    ];
    let mut seeds = Vec::new();
    for (name, pts) in polygons {
        for (suffix, inset, offset) in [("", 4.0f32, 3.0f32), ("-outset", -6.0, -8.0)] {
            let mut w = SeedWriter::new();
            w.i32(i32::try_from(pts.len()).expect("small polygon"));
            for p in pts {
                w.f32s(p);
            }
            // Convex: the inset. Simple (convex polygons are too): the offset, then the
            // index map.
            if name == "convex" {
                w.f32(inset);
            }
            w.f32(offset);
            for i in 0..pts.len() {
                w.u16(u16::try_from(i).expect("small polygon"));
            }
            seeds.push(seed(&format!("{name}{suffix}"), &w));
        }
    }
    seeds.extend(random_seeds(6, 6, &[64, 512, 4200]));
    seeds
}

fn pathop() -> Vec<Seed> {
    let mut seeds = Vec::new();
    // choice 0: OpBuilder with three paths and every fill type and op.
    let mut w = SeedWriter::new();
    w.u8(0).u8(3);
    for (i, scale) in [1.0f32, 0.5, 2.0].into_iter().enumerate() {
        evil_path_bytes(&mut w, scale);
        w.u8(u8::try_from(i).expect("small"))
            .i32(i32::try_from(i).expect("small") + 1);
    }
    seeds.push(seed("builder", &w));
    // choice 1: Simplify.
    let mut w = SeedWriter::new();
    w.u8(1);
    evil_path_bytes(&mut w, 1.0);
    w.u8(1);
    seeds.push(seed("simplify", &w));
    // choice 2: Op, one seed per op.
    for op in 0..=4 {
        let mut w = SeedWriter::new();
        w.u8(2);
        evil_path_bytes(&mut w, 1.0);
        w.u8(0);
        evil_path_bytes(&mut w, 0.75);
        w.u8(1).i32(op).u8(1);
        seeds.push(seed(&format!("op-{op}"), &w));
    }
    // choice 3: AsWinding.
    let mut w = SeedWriter::new();
    w.u8(3);
    evil_path_bytes(&mut w, 1.0);
    w.u8(1).bool(true);
    seeds.push(seed("as-winding", &w));
    // choice 4: ComputeTightBounds.
    let mut w = SeedWriter::new();
    w.u8(4);
    evil_path_bytes(&mut w, 3.0);
    w.u8(2);
    seeds.push(seed("tight-bounds", &w));
    seeds.extend(random_seeds(7, 6, &[96, 512, 2048]));
    seeds
}

fn legacy_chromium_pathop() -> Vec<Seed> {
    let mut seeds = Vec::new();
    for (name, ops) in [("union", [2u8, 2, 2]), ("mixed", [0, 1, 4])] {
        let mut w = SeedWriter::new();
        for (op, scale) in ops.into_iter().zip([1.0f32, 0.6, 1.4]) {
            w.u8(op);
            evil_path_bytes(&mut w, scale);
        }
        seeds.push(seed(name, &w));
    }
    // A path cut short: `BuildPath` depletes the input when a verb's points do not fit.
    let mut w = SeedWriter::new();
    w.u8(2).u8(0).f32s(&[1.0, 2.0]).u8(4).f32s(&[3.0, 4.0]);
    seeds.push(seed("truncated", &w));
    seeds.extend(random_seeds(8, 6, &[96, 512, 2048]));
    seeds
}

// ---------------------------------------------------------------------------------------------
// Gradients and drawing

/// `initGradientParams` input: count, tile mode, then (color, pos) pairs.
fn gradient_params(w: &mut SeedWriter, mode: u8) {
    let stops: [(u32, f32); 4] = [
        (0xFFFF_0000, 0.0),
        (0x8000_FF00, 0.3),
        (0xFF00_00FF, 0.6),
        (0x00FF_FFFF, 1.0),
    ];
    w.i32(4).u8(mode);
    for (c, p) in stops {
        w.u32(c).f32(p);
    }
}

const ROTATE_SCALE: [f32; 9] = [0.8, -0.6, 25.0, 0.6, 0.8, -5.0, 0.0, 0.0, 1.0];
const PERSPECTIVE: [f32; 9] = [1.0, 0.2, 0.0, 0.1, 1.0, 0.0, 0.001, 0.002, 1.0];

fn gradients() -> Vec<Seed> {
    let mut seeds = Vec::new();
    for (kind, name) in [(0u8, "linear"), (1, "radial"), (2, "conical"), (3, "sweep")] {
        for (variant, local, global, mode) in
            [("", false, false, 0u8), ("-matrices", true, true, 2)]
        {
            let mut w = SeedWriter::new();
            w.u8(kind);
            match kind {
                0 => {
                    w.f32s(&[5.0, 5.0, 45.0, 40.0]).bool(local).bool(global);
                }
                1 => {
                    w.f32s(&[25.0, 25.0, 20.0]).bool(local).bool(global);
                }
                2 => {
                    w.f32s(&[15.0, 20.0, 35.0, 25.0, 5.0, 20.0])
                        .bool(local)
                        .bool(global);
                }
                _ => {
                    w.f32s(&[25.0, 25.0]).bool(local).bool(global);
                }
            }
            gradient_params(&mut w, mode);
            if kind == 3 {
                // Sweep reads the local matrix before the interpolation flags.
                if local {
                    w.f32s(&ROTATE_SCALE).u32(1);
                }
            } else {
                w.u32(u32::from(local));
                if local {
                    w.f32s(&ROTATE_SCALE);
                }
            }
            if global {
                w.f32s(&PERSPECTIVE);
            }
            seeds.push(seed(&format!("{name}{variant}"), &w));
        }
    }
    // A single stop and many stops.
    let mut w = SeedWriter::new();
    w.u8(0).f32s(&[0.0, 0.0, 50.0, 0.0]).bool(false).bool(false);
    w.i32(1).u8(1).u32(0xFF12_3456).f32(0.5).u32(0);
    seeds.push(seed("one-stop", &w));
    let mut w = SeedWriter::new();
    w.u8(1).f32s(&[25.0, 25.0, 30.0]).bool(false).bool(false);
    w.i32(400).u8(1);
    let mut rand = Random::new(9);
    for _ in 0..400 {
        w.u32(rand.next_u()).f32(rand.next_f());
    }
    w.u32(0);
    seeds.push(seed("many-stops", &w));
    seeds.extend(random_seeds(9, 6, &[64, 512, 4096]));
    seeds
}

/// `init_paint` input. `style`: 0 fill, 1 stroke, 2 stroke-and-fill.
fn paint_bytes(w: &mut SeedWriter, aa: bool, style: u8) {
    w.bool(aa)
        .u8(3) // kSrcOver
        .u32(0xFF33_6699)
        .bool(false)
        .u8(1) // round cap
        .u8(2) // bevel join
        .f32(4.0)
        .f32(3.0)
        .u8(style);
}

/// `init_surface` input: a 64x48 surface.
fn surface_bytes(w: &mut SeedWriter) {
    w.u8(64).u8(48);
}

fn draw_functions() -> Vec<Seed> {
    let mut seeds = Vec::new();

    // 0: text.
    let mut w = SeedWriter::new();
    w.u8(0);
    paint_bytes(&mut w, true, 0);
    surface_bytes(&mut w);
    w.bytes(b"Skia");
    w.f32s(&[4.0, 30.0]);
    for b in [false, true, false, true, false, true] {
        w.bool(b);
    }
    w.f32s(&[1.0, -0.25, 18.0, 2.0, 3.0]);
    seeds.push(seed("text", &w));

    // 1: rect, then a clip.
    let mut w = SeedWriter::new();
    w.u8(1);
    paint_bytes(&mut w, true, 1);
    surface_bytes(&mut w);
    w.f32s(&[5.5, 6.25, 40.0, 30.0])
        .bool(true)
        .f32s(&[0.0, 0.0, 20.0, 20.0]);
    seeds.push(seed("rect", &w));

    // 2: circle.
    let mut w = SeedWriter::new();
    w.u8(2);
    paint_bytes(&mut w, true, 2);
    surface_bytes(&mut w);
    w.f32s(&[32.0, 24.0, 20.0]);
    seeds.push(seed("circle", &w));

    // 3: line.
    let mut w = SeedWriter::new();
    w.u8(3);
    paint_bytes(&mut w, false, 1);
    surface_bytes(&mut w);
    w.f32s(&[2.0, 3.0, 60.0, 40.0]);
    seeds.push(seed("line", &w));

    // 4: path with every operation, then a clip.
    let mut w = SeedWriter::new();
    w.u8(4);
    paint_bytes(&mut w, true, 0);
    surface_bytes(&mut w);
    w.u8(6);
    w.u8(0).f32s(&[5.0, 5.0]);
    w.u8(1).f32s(&[50.0, 8.0]);
    w.u8(2).f32s(&[60.0, 30.0, 40.0, 40.0]);
    w.u8(3).f32s(&[20.0, 45.0, 10.0, 25.0, 0.5]);
    w.u8(4).f32s(&[0.0, 10.0, 30.0, 0.0, 30.0, 20.0]);
    w.u8(5).f32s(&[50.0, 20.0, 50.0, 40.0, 8.0]);
    w.bool(true);
    seeds.push(seed("path", &w));

    // 5: image, both draws, over several color types.
    for (name, draw_image, color_type) in [
        ("image", true, 4u8),
        ("image-rect-565", false, 2),
        ("image-rect-101010x", false, 10),
    ] {
        let mut w = SeedWriter::new();
        w.u8(5);
        paint_bytes(&mut w, true, 0);
        surface_bytes(&mut w);
        // init_bitmap: color type, opaque, erase color, paint flag + paint.
        w.u8(color_type).bool(false).u32(0x80FF_8000).bool(true);
        paint_bytes(&mut w, false, 0);
        w.bool(draw_image).f32s(&[40.0, 30.0]);
        if !draw_image {
            w.f32s(&[12.0, 20.0]).u8(0);
        }
        seeds.push(seed(name, &w));
    }

    // 6: paint.
    let mut w = SeedWriter::new();
    w.u8(6);
    paint_bytes(&mut w, false, 0);
    surface_bytes(&mut w);
    seeds.push(seed("paint", &w));

    // Random tails behind each branch.
    for i in 0..7u8 {
        let mut w = SeedWriter::new();
        w.u8(i).bytes(&random_bytes(10 * 1000 + u32::from(i), 160));
        seeds.push(seed(&format!("random-branch-{i}"), &w));
    }
    seeds
}

// ---------------------------------------------------------------------------------------------
// Encoders

/// The Skia resource images the encoder seeds are made from.
/// Small ones, to keep the corpus small: random pixels and a photo.
pub const ENCODER_RESOURCE_IMAGES: [&str; 2] = ["images/randPixels.png", "images/mandrill_32.png"];

/// The option bytes each encoder reads after the bitmap, by seed suffix.
fn encoder_options(name: &str) -> Vec<(&'static str, Vec<u8>)> {
    let mut opts = Vec::new();
    match name {
        "PNGEncoder" => {
            for (suffix, level) in [("z0", 0), ("z6", 6), ("z9", 9)] {
                let mut w = SeedWriter::new();
                w.i32(level);
                opts.push((suffix, w.finish()));
            }
        }
        "PNGRustEncoder" => {
            // `int compression`: the three levels, and one past them (the default).
            for (suffix, level) in [("low", 0), ("medium", 1), ("high", 2), ("default", 3)] {
                let mut w = SeedWriter::new();
                w.i32(level);
                opts.push((suffix, w.finish()));
            }
        }
        "JPEGEncoder" => {
            for (suffix, q) in [("q0", 0), ("q50", 50), ("q100", 100)] {
                let mut w = SeedWriter::new();
                w.i32(q);
                opts.push((suffix, w.finish()));
            }
        }
        _ => {
            for (suffix, q, lossy) in [("lossy-q75", 75.0f32, true), ("lossless", 100.0, false)] {
                let mut w = SeedWriter::new();
                w.f32(q).bool(lossy);
                opts.push((suffix, w.finish()));
            }
        }
    }
    opts
}

/// `_MakeEncoderCorpus`'s format: width and height as `uint32_t`, then the pixels.
fn encoder_bitmap_bytes(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let mut w = SeedWriter::new();
    w.u32(width).u32(height).bytes(pixels);
    w.finish()
}

/// A 24x16 premultiplied image with a gradient, transparent and opaque areas, as BGRA bytes.
fn synthetic_image() -> (u32, u32, Vec<u8>) {
    let (width, height) = (24u32, 16u32);
    let mut pixels = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let a = if x < 8 { 255 } else { 255 - 15 * y.min(17) };
            let premul = |c: u32| c * a / 255;
            // B, G, R, A.
            for c in [premul(y * 16), premul(128), premul(x * 10), a] {
                pixels.push(u8::try_from(c).expect("a channel"));
            }
        }
    }
    (width, height, pixels)
}

/// The resource image at `path` decoded as `_MakeEncoderCorpus` does, to premultiplied BGRA (the
/// oracle's N32, fixed so the seeds do not depend on the host). `None` without the resources.
#[must_use]
pub fn decode_resource_image(path: &str) -> Option<(u32, u32, Vec<u8>)> {
    let mut full = skia_rust_tools::resources::resource_dir()?;
    for component in path.split('/') {
        full.push(component);
    }
    let bytes = std::fs::read(full).ok()?;
    let img = deferred_from_encoded_data(Some(Data::new_copy(&bytes)), None)?;
    let (width, height) = (img.width(), img.height());
    let info = ImageInfo::new(
        (width, height),
        ColorType::BGRA8888,
        AlphaType::Premul,
        None,
    );
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; row_bytes * usize::try_from(height).ok()?];
    if !img.read_pixels(&info, &mut pixels, row_bytes, (0, 0)) {
        return None;
    }
    Some((
        u32::try_from(width).ok()?,
        u32::try_from(height).ok()?,
        pixels,
    ))
}

/// The names of the seeds of `name` that need Skia's resources.
#[must_use]
pub fn resource_seed_names(name: &str) -> Vec<String> {
    if !matches!(
        name,
        "PNGEncoder" | "PNGRustEncoder" | "JPEGEncoder" | "WEBPEncoder"
    ) {
        return Vec::new();
    }
    let mut names = Vec::new();
    for path in ENCODER_RESOURCE_IMAGES {
        let stem = resource_stem(path);
        for (suffix, _) in encoder_options(name) {
            names.push(format!("{stem}-{suffix}"));
        }
    }
    names
}

fn resource_stem(path: &str) -> &str {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.split('.').next().unwrap_or(file)
}

// ---------------------------------------------------------------------------------------------
// FuzzCanvas

/// The `drawCommand` values of `FuzzCanvas` that draw something (`0`, `28`, `46` to `50`, and the
/// values Skia has no case for read nothing, so they are left out).
const DRAW_COMMANDS: [u32; 48] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 29, 30, 31, 32, 33, 35, 37, 44, 45, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62,
];

/// Seeds for the `FuzzCanvas` fuzzers (`NullCanvas`, `RasterN32Canvas`, `PDFCanvas`): one per draw
/// command. Each seed is 40 commands (`N`), the first of them the seed's command, and the bytes
/// after it are random, so the arguments of every later command are random too.
fn canvas_commands() -> Vec<Seed> {
    let mut seeds: Vec<Seed> = DRAW_COMMANDS
        .iter()
        .map(|&cmd| {
            let mut w = SeedWriter::new();
            w.u32(40).u32(cmd).bytes(&random_bytes(1000 + cmd, 700));
            seed(&format!("command-{cmd:02}"), &w)
        })
        .collect();
    seeds.extend(random_seeds(7, 6, &[64, 512, 4096]));
    seeds
}

/// Seeds for `ImageFilter`: one per `MakeFuzzImageFilter` type (`1` to `22`), so the first filter
/// is that type, with random bytes for its arguments and its inputs.
fn image_filter_types() -> Vec<Seed> {
    let mut seeds: Vec<Seed> = (1u8..=22)
        .map(|t| {
            let mut w = SeedWriter::new();
            w.u8(t).bytes(&random_bytes(2000 + u32::from(t), 700));
            seed(&format!("type-{t:02}"), &w)
        })
        .collect();
    seeds.extend(random_seeds(8, 6, &[64, 512, 4096]));
    seeds
}

fn encoder(name: &str) -> Vec<Seed> {
    let mut images = vec![("synthetic".to_owned(), synthetic_image())];
    for path in ENCODER_RESOURCE_IMAGES {
        if let Some(image) = decode_resource_image(path) {
            images.push((resource_stem(path).to_owned(), image));
        }
    }
    let mut seeds = Vec::new();
    for (stem, (width, height, pixels)) in &images {
        for (suffix, opts) in encoder_options(name) {
            let mut bytes = encoder_bitmap_bytes(*width, *height, pixels);
            bytes.extend_from_slice(&opts);
            seeds.push((format!("{stem}-{suffix}"), bytes));
        }
    }
    seeds
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fuzz::FUZZERS;
    use crate::replay::{corpus_dir, read_dir_sorted};

    /// The committed corpus is exactly what [`generate`] produces (without Skia's resources, the
    /// resource-image seeds are only checked to be present).
    #[test]
    fn committed_corpus_matches_the_generator() {
        let have_resources = skia_rust_tools::resources::resource_dir().is_some();
        for fuzzable in FUZZERS {
            let name = fuzzable.name;
            let generated = generate(name).expect("every registered fuzzer has seeds");
            let committed = read_dir_sorted(&corpus_dir(name));
            if have_resources {
                assert!(
                    committed == generated,
                    "fuzz/corpus/{name} is out of date; run `cargo run -p skia-rust-fuzz --bin \
                     fuzz-seeds`"
                );
            } else {
                let resource_only = resource_seed_names(name);
                let rest: Vec<Seed> = committed
                    .into_iter()
                    .filter(|(file, _)| !resource_only.contains(file))
                    .collect();
                assert!(
                    rest == generated,
                    "fuzz/corpus/{name} is out of date; run `cargo run -p skia-rust-fuzz --bin \
                     fuzz-seeds`"
                );
            }
        }
    }

    #[test]
    fn writer_is_little_endian() {
        let mut w = SeedWriter::new();
        w.u8(1).bool(true).u16(0x0203).i32(-2).f32(1.0);
        assert_eq!(
            w.finish(),
            [1, 1, 3, 2, 0xFE, 0xFF, 0xFF, 0xFF, 0, 0, 0x80, 0x3F]
        );
    }
}
