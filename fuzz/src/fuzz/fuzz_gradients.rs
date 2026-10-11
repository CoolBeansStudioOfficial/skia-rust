// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: fuzz/FuzzGradients.cpp (chrome/m156)
//
// skia-rust: the `--verbose` flag and `logLinearGradient` (a `SkDebugf` dump of the inputs) are
// not ported; with the flag at its default they do nothing.

use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::scalar;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, interpolation, shaders};
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

use crate::{Fuzz, def_fuzz};

// Port of: fuzz/FuzzGradients.cpp#L22 (chrome/m156)
const MAX_COUNT: i32 = 400;

// Port of: fuzz/FuzzGradients.cpp#L24-L28 (chrome/m156)
fn make_matrix(fuzz: &mut Fuzz<'_>, m: &mut Matrix) {
    let mat: Vec<scalar> = fuzz.next_n(9);
    let mat: [scalar; 9] = mat.try_into().expect("nextN(mat, 9) reads 9 values");
    m.set_9(&mat);
}

/// `(SkTileMode)m` for `m` in `[0, 2]`.
fn tile_mode(m: u8) -> TileMode {
    match m {
        0 => TileMode::Clamp,
        1 => TileMode::Repeat,
        _ => TileMode::Mirror,
    }
}

// Port of: fuzz/FuzzGradients.cpp#L30-L55 (chrome/m156)
fn init_gradient_params(
    fuzz: &mut Fuzz<'_>,
    colors: &mut Vec<Color4f>,
    pos: &mut Vec<scalar>,
    mode: &mut TileMode,
) {
    let count: i32 = fuzz.next_range(0, MAX_COUNT);

    // Use a uint8_t to conserve bytes.  This makes our "fuzzed bytes footprint"
    // smaller, which leads to more efficient fuzzing.
    let m: u8 = fuzz.next_range(0, 2);
    *mode = tile_mode(m);

    colors.clear();
    pos.clear();
    for _ in 0..count {
        let c: u32 = fuzz.next();
        let s: scalar = fuzz.next();
        colors.push(Color4f::from_color(Color::from(c)));
        pos.push(s);
    }
    if count != 0 {
        // skia-rust: `std::sort` with `operator<` has no specified result when a position is NaN
        // (not a strict weak ordering), and Rust's sort may panic on an inconsistent order.
        // `total_cmp` sorts every non-NaN value as `operator<` does (+0 and -0, equal to `<`, may
        // swap, as they may in an unstable sort).
        pos.sort_by(f32::total_cmp);
        // The order matters.  If count == 1, we want pos == 0.
        let last = pos.len() - 1;
        pos[last] = 1.0;
        pos[0] = 0.0;
    }
}

// Port of: fuzz/FuzzGradients.cpp#L106-L112 (chrome/m156)
fn fuzz_interp(fuzz: &mut Fuzz<'_>) -> Interpolation {
    let flags: u32 = fuzz.next();

    Interpolation {
        in_premul: if (flags & 1) != 0 {
            interpolation::InPremul::Yes
        } else {
            interpolation::InPremul::No
        },
        ..Interpolation::default()
    }
}

/// `SkSurfaces::Raster(SkImageInfo::MakeN32Premul(50, 50))`.
fn make_surface() -> Surface<'static> {
    surfaces::raster_n32_premul((50, 50)).expect("a 50x50 N32 raster surface always allocates")
}

/// The draw shared by the four gradients: `drawPaint` under a fuzzed matrix if asked.
fn draw(fuzz: &mut Fuzz<'_>, use_global_matrix: bool, p: &Paint) {
    let mut surface = make_surface();
    if use_global_matrix {
        let mut gm = Matrix::default();
        make_matrix(fuzz, &mut gm);
        let c = surface.canvas();
        c.set_matrix(&M44::from(&gm));
        c.draw_paint(p);
    } else {
        surface.canvas().draw_paint(p);
    }
}

// Port of: fuzz/FuzzGradients.cpp#L114-L146 (chrome/m156)
fn fuzz_linear_gradient(fuzz: &mut Fuzz<'_>) {
    let (x0, y0, x1, y1): (scalar, scalar, scalar, scalar) =
        (fuzz.next(), fuzz.next(), fuzz.next(), fuzz.next());
    let pts = [Point::new(x0, y0), Point::new(x1, y1)];
    let use_local_matrix = fuzz.next_bool();
    let use_global_matrix = fuzz.next_bool();

    let mut colors = Vec::new();
    let mut pos = Vec::new();
    let mut mode = TileMode::default();
    init_gradient_params(fuzz, &mut colors, &mut pos, &mut mode);

    let mut p = Paint::default();
    let interp = fuzz_interp(fuzz);

    let mut local_matrix = None;
    if use_local_matrix {
        let mut m = Matrix::default();
        make_matrix(fuzz, &mut m);
        local_matrix = Some(m);
    }
    p.set_shader(shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(Colors::new(&colors, Some(&pos), mode, None), interp),
        local_matrix.as_ref(),
    ));

    draw(fuzz, use_global_matrix, &p);
}

// Port of: fuzz/FuzzGradients.cpp#L148-L179 (chrome/m156)
fn fuzz_radial_gradient(fuzz: &mut Fuzz<'_>) {
    let (cx, cy): (scalar, scalar) = (fuzz.next(), fuzz.next());
    let center = Point::new(cx, cy);
    let radius: scalar = fuzz.next();
    let use_local_matrix = fuzz.next_bool();
    let use_global_matrix = fuzz.next_bool();

    let mut colors = Vec::new();
    let mut pos = Vec::new();
    let mut mode = TileMode::default();
    init_gradient_params(fuzz, &mut colors, &mut pos, &mut mode);

    let mut p = Paint::default();
    let interp = fuzz_interp(fuzz);

    let mut local_matrix = None;
    if use_local_matrix {
        let mut m = Matrix::default();
        make_matrix(fuzz, &mut m);
        local_matrix = Some(m);
    }
    p.set_shader(shaders::radial_gradient(
        (center, radius),
        &Gradient::new(Colors::new(&colors, Some(&pos), mode, None), interp),
        local_matrix.as_ref(),
    ));

    draw(fuzz, use_global_matrix, &p);
}

// Port of: fuzz/FuzzGradients.cpp#L181-L215 (chrome/m156)
fn fuzz_two_point_conical_gradient(fuzz: &mut Fuzz<'_>) {
    let (sx, sy): (scalar, scalar) = (fuzz.next(), fuzz.next());
    let start = Point::new(sx, sy);
    let (ex, ey): (scalar, scalar) = (fuzz.next(), fuzz.next());
    let end = Point::new(ex, ey);
    let start_radius: scalar = fuzz.next();
    let end_radius: scalar = fuzz.next();
    let use_local_matrix = fuzz.next_bool();
    let use_global_matrix = fuzz.next_bool();

    let mut colors = Vec::new();
    let mut pos = Vec::new();
    let mut mode = TileMode::default();
    init_gradient_params(fuzz, &mut colors, &mut pos, &mut mode);

    let mut p = Paint::default();
    let interp = fuzz_interp(fuzz);

    let mut local_matrix = None;
    if use_local_matrix {
        let mut m = Matrix::default();
        make_matrix(fuzz, &mut m);
        local_matrix = Some(m);
    }
    p.set_shader(shaders::two_point_conical_gradient(
        (start, start_radius),
        (end, end_radius),
        &Gradient::new(Colors::new(&colors, Some(&pos), mode, None), interp),
        local_matrix.as_ref(),
    ));

    draw(fuzz, use_global_matrix, &p);
}

// Port of: fuzz/FuzzGradients.cpp#L217-L252 (chrome/m156)
fn fuzz_sweep_gradient(fuzz: &mut Fuzz<'_>) {
    let (cx, cy): (scalar, scalar) = (fuzz.next(), fuzz.next());
    let use_local_matrix = fuzz.next_bool();
    let use_global_matrix = fuzz.next_bool();

    let mut colors = Vec::new();
    let mut pos = Vec::new();
    let mut mode = TileMode::default();
    init_gradient_params(fuzz, &mut colors, &mut pos, &mut mode);

    let mut p = Paint::default();
    // `SweepGradient(center, grad, lm)` is `SweepGradient(center, 0, 360, grad, lm)`.
    if use_local_matrix {
        let mut m = Matrix::default();
        make_matrix(fuzz, &mut m);
        let interp = fuzz_interp(fuzz);

        p.set_shader(shaders::sweep_gradient(
            (cx, cy),
            (0.0, 360.0),
            &Gradient::new(
                Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
                interp,
            ),
            &m,
        ));
    } else {
        p.set_shader(shaders::sweep_gradient(
            (cx, cy),
            (0.0, 360.0),
            &Gradient::new(
                Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
    }

    draw(fuzz, use_global_matrix, &p);
}

// Port of: fuzz/FuzzGradients.cpp#L254-L275 (chrome/m156)
def_fuzz!(Gradients, fuzz_Gradients, |fuzz| {
    let i: u8 = fuzz.next();

    match i {
        0 => {
            fuzz_linear_gradient(fuzz);
            return;
        }
        1 => {
            fuzz_radial_gradient(fuzz);
            return;
        }
        2 => {
            fuzz_two_point_conical_gradient(fuzz);
            return;
        }
        _ => {}
    }
    fuzz_sweep_gradient(fuzz);
});
