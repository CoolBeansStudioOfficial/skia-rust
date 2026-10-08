// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/smallpaths.cpp (chrome/m156)

#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::excessive_precision,
    clippy::float_cmp,
    clippy::inconsistent_digit_grouping,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{SCALAR_PI, scalar_cos, scalar_sin};

// Port of: gm/smallpaths.cpp#L19-L24 (chrome/m156)
struct PathDY {
    path: Path,
    dy: f32,
}

// Port of: gm/smallpaths.cpp#L28-L41 (chrome/m156)
fn make_triangle() -> PathDY {
    const G_COORD: [i32; 6] = [10, 20, 15, 5, 30, 30];
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    let coord = |i: usize| G_COORD[i] as f32;
    PathDY {
        path: PathBuilder::new()
            .move_to((coord(0), coord(1)))
            .line_to((coord(2), coord(3)))
            .line_to((coord(4), coord(5)))
            .close()
            .offset((10.0, 0.0))
            .detach(),
        dy: 30.0,
    }
}

// Port of: gm/smallpaths.cpp#L43-L50 (chrome/m156)
fn make_rect() -> PathDY {
    let r = Rect::from_ltrb(10.0, 10.0, 30.0, 30.0);
    PathDY {
        path: Path::rect(r.with_offset((10.0, 0.0)), None),
        dy: 30.0,
    }
}

// Port of: gm/smallpaths.cpp#L52-L59 (chrome/m156)
fn make_oval() -> PathDY {
    let r = Rect::from_ltrb(10.0, 10.0, 30.0, 30.0);
    PathDY {
        path: Path::oval(r.with_offset((10.0, 0.0)), None),
        dy: 30.0,
    }
}

// Port of: gm/smallpaths.cpp#L61-L76 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // int loop bounds, as in C++
fn make_star(n: i32) -> PathDY {
    let c: f32 = 45.0;
    let r: f32 = 20.0;

    let mut rad = -SCALAR_PI / 2.0;
    let drad = ((n >> 1) as f32) * SCALAR_PI * 2.0 / (n as f32);

    let mut b = PathBuilder::new();
    b.move_to((c, c - r));
    for _i in 1..n {
        rad += drad;
        b.line_to((c + scalar_cos(rad) * r, c + scalar_sin(rad) * r));
    }
    b.close();
    PathDY {
        path: b.detach(),
        dy: r * 2.0 * 6.0 / 5.0,
    }
}

// Port of: gm/smallpaths.cpp#L78 (chrome/m156)
fn make_star_5() -> PathDY {
    make_star(5)
}

// Port of: gm/smallpaths.cpp#L79 (chrome/m156)
fn make_star_13() -> PathDY {
    make_star(13)
}

// Port of: gm/smallpaths.cpp#L81-L98 (chrome/m156)
fn make_three_line() -> PathDY {
    const X_OFFSET: f32 = 34.0;
    const Y_OFFSET: f32 = 50.0;
    let mut b = PathBuilder::new();
    b.move_to((-32.5 + X_OFFSET, 0.0 + Y_OFFSET));
    b.line_to((32.5 + X_OFFSET, 0.0 + Y_OFFSET));

    b.move_to((-32.5 + X_OFFSET, 19.0 + Y_OFFSET));
    b.line_to((32.5 + X_OFFSET, 19.0 + Y_OFFSET));

    b.move_to((-32.5 + X_OFFSET, -19.0 + Y_OFFSET));
    b.line_to((32.5 + X_OFFSET, -19.0 + Y_OFFSET));
    b.line_to((-32.5 + X_OFFSET, -19.0 + Y_OFFSET));

    b.close();

    PathDY {
        path: b.detach(),
        dy: 70.0,
    }
}

// Port of: gm/smallpaths.cpp#L100-L117 (chrome/m156)
fn make_arrow() -> PathDY {
    const X_OFFSET: f32 = 34.0;
    const Y_OFFSET: f32 = 40.0;
    let mut b = PathBuilder::new();
    b.move_to((-26.0 + X_OFFSET, 0.0 + Y_OFFSET));
    b.line_to((26.0 + X_OFFSET, 0.0 + Y_OFFSET));

    b.move_to((-28.0 + X_OFFSET, -2.474_874_5 + Y_OFFSET));
    b.line_to((0.0 + X_OFFSET, 25.525_126 + Y_OFFSET));

    b.move_to((-28.0 + X_OFFSET, 2.474_874_5 + Y_OFFSET));
    b.line_to((0.0 + X_OFFSET, -25.525_126 + Y_OFFSET));
    b.line_to((-28.0 + X_OFFSET, 2.474_874_5 + Y_OFFSET));

    b.close();

    PathDY {
        path: b.detach(),
        dy: 70.0,
    }
}

// Port of: gm/smallpaths.cpp#L119-L129 (chrome/m156)
#[allow(clippy::excessive_precision)] // literals as written in the C++ source
fn make_curve() -> PathDY {
    const X_OFFSET: f32 = -382.0;
    const Y_OFFSET: f32 = -50.0;
    let mut b = PathBuilder::new();
    b.move_to((491.0 + X_OFFSET, 56.0 + Y_OFFSET));
    b.conic_to(
        (435.932_92 + X_OFFSET, 56.000_031 + Y_OFFSET),
        (382.610_78 + X_OFFSET, 69.752_716 + Y_OFFSET),
        0.992_046_3,
    );

    PathDY {
        path: b.detach(),
        dy: 40.0,
    }
}

// Port of: gm/smallpaths.cpp#L131-L156 (chrome/m156)
#[allow(clippy::excessive_precision)] // literals as written in the C++ source
fn make_battery() -> PathDY {
    const X_OFFSET: f32 = 5.0;

    let mut b = PathBuilder::new();
    b.move_to((24.67 + X_OFFSET, 0.330_000_04));
    b.line_to((8.329_999_9 + X_OFFSET, 0.330_000_04));
    b.line_to((8.329_999_9 + X_OFFSET, 5.329_999_9));
    b.line_to((0.330_000_04 + X_OFFSET, 5.329_999_9));
    b.line_to((0.330_000_04 + X_OFFSET, 50.669_998));
    b.line_to((32.669_998 + X_OFFSET, 50.669_998));
    b.line_to((32.669_998 + X_OFFSET, 5.329_999_9));
    b.line_to((24.67 + X_OFFSET, 5.329_999_9));
    b.line_to((24.67 + X_OFFSET, 0.330_000_04));
    b.close();

    b.move_to((25.727_224 + X_OFFSET, 12.886_665));
    b.line_to((10.907_918 + X_OFFSET, 12.886_665));
    b.line_to((7.516_665_9 + X_OFFSET, 28.683_645));
    b.line_to((14.810_181 + X_OFFSET, 28.683_645));
    b.line_to((7.702_487_9 + X_OFFSET, 46.135_998));
    b.line_to((28.049_999 + X_OFFSET, 25.136_419));
    b.line_to((16.854_223 + X_OFFSET, 25.136_419));
    b.line_to((25.727_224 + X_OFFSET, 12.886_665));
    b.close();
    PathDY {
        path: b.detach(),
        dy: 50.0,
    }
}

// Port of: gm/smallpaths.cpp#L158-L180 (chrome/m156)
#[allow(clippy::excessive_precision)] // literals as written in the C++ source
fn make_battery2() -> PathDY {
    const X_OFFSET: f32 = 225.625;

    let mut b = PathBuilder::new();
    b.move_to((32.669_998 + X_OFFSET, 9.864_000_3));
    b.line_to((0.330_000_04 + X_OFFSET, 9.864_000_3));
    b.line_to((0.330_000_04 + X_OFFSET, 50.669_998));
    b.line_to((32.669_998 + X_OFFSET, 50.669_998));
    b.line_to((32.669_998 + X_OFFSET, 9.864_000_3));
    b.close();

    b.move_to((10.907_918 + X_OFFSET, 12.886_665));
    b.line_to((25.727_224 + X_OFFSET, 12.886_665));
    b.line_to((16.854_223 + X_OFFSET, 25.136_419));
    b.line_to((28.049_999 + X_OFFSET, 25.136_419));
    b.line_to((7.702_487_9 + X_OFFSET, 46.135_998));
    b.line_to((14.810_181 + X_OFFSET, 28.683_645));
    b.line_to((7.516_665_9 + X_OFFSET, 28.683_645));
    b.line_to((10.907_918 + X_OFFSET, 12.886_665));
    b.close();

    PathDY {
        path: b.detach(),
        dy: 60.0,
    }
}

// Port of: gm/smallpaths.cpp#L182-L256 (chrome/m156)
#[allow(clippy::excessive_precision)] // literals as written in the C++ source
#[allow(clippy::too_many_lines)]
fn make_ring() -> PathDY {
    const X_OFFSET: f32 = 120.0;
    const Y_OFFSET: f32 = -270.0;

    let mut b = PathBuilder::new();
    b.set_fill_type(PathFillType::Winding);
    b.move_to((X_OFFSET + 144.859, Y_OFFSET + 285.172));
    b.line_to((X_OFFSET + 144.859, Y_OFFSET + 285.172));
    b.line_to((X_OFFSET + 144.859, Y_OFFSET + 285.172));
    b.line_to((X_OFFSET + 143.132, Y_OFFSET + 284.617));
    b.line_to((X_OFFSET + 144.859, Y_OFFSET + 285.172));
    b.close();
    b.move_to((X_OFFSET + 135.922, Y_OFFSET + 286.844));
    b.line_to((X_OFFSET + 135.922, Y_OFFSET + 286.844));
    b.line_to((X_OFFSET + 135.922, Y_OFFSET + 286.844));
    b.line_to((X_OFFSET + 135.367, Y_OFFSET + 288.571));
    b.line_to((X_OFFSET + 135.922, Y_OFFSET + 286.844));
    b.close();
    b.move_to((X_OFFSET + 135.922, Y_OFFSET + 286.844));
    b.cubic_to(
        (X_OFFSET + 137.07, Y_OFFSET + 287.219),
        (X_OFFSET + 138.242, Y_OFFSET + 287.086),
        (X_OFFSET + 139.242, Y_OFFSET + 286.578),
    );
    b.cubic_to(
        (X_OFFSET + 140.234, Y_OFFSET + 286.078),
        (X_OFFSET + 141.031, Y_OFFSET + 285.203),
        (X_OFFSET + 141.406, Y_OFFSET + 284.055),
    );
    b.line_to((X_OFFSET + 144.859, Y_OFFSET + 285.172));
    b.cubic_to(
        (X_OFFSET + 143.492, Y_OFFSET + 289.375),
        (X_OFFSET + 138.992, Y_OFFSET + 291.656),
        (X_OFFSET + 134.797, Y_OFFSET + 290.297),
    );
    b.line_to((X_OFFSET + 135.922, Y_OFFSET + 286.844));
    b.close();
    b.move_to((X_OFFSET + 129.68, Y_OFFSET + 280.242));
    b.line_to((X_OFFSET + 129.68, Y_OFFSET + 280.242));
    b.line_to((X_OFFSET + 129.68, Y_OFFSET + 280.242));
    b.line_to((X_OFFSET + 131.407, Y_OFFSET + 280.804));
    b.line_to((X_OFFSET + 129.68, Y_OFFSET + 280.242));
    b.close();
    b.move_to((X_OFFSET + 133.133, Y_OFFSET + 281.367));
    b.cubic_to(
        (X_OFFSET + 132.758, Y_OFFSET + 282.508),
        (X_OFFSET + 132.883, Y_OFFSET + 283.687),
        (X_OFFSET + 133.391, Y_OFFSET + 284.679),
    );
    b.cubic_to(
        (X_OFFSET + 133.907, Y_OFFSET + 285.679),
        (X_OFFSET + 134.774, Y_OFFSET + 286.468),
        (X_OFFSET + 135.922, Y_OFFSET + 286.843),
    );
    b.line_to((X_OFFSET + 134.797, Y_OFFSET + 290.296));
    b.cubic_to(
        (X_OFFSET + 130.602, Y_OFFSET + 288.929),
        (X_OFFSET + 128.313, Y_OFFSET + 284.437),
        (X_OFFSET + 129.68, Y_OFFSET + 280.241),
    );
    b.line_to((X_OFFSET + 133.133, Y_OFFSET + 281.367));
    b.close();
    b.move_to((X_OFFSET + 139.742, Y_OFFSET + 275.117));
    b.line_to((X_OFFSET + 139.742, Y_OFFSET + 275.117));
    b.line_to((X_OFFSET + 139.18, Y_OFFSET + 276.844));
    b.line_to((X_OFFSET + 139.742, Y_OFFSET + 275.117));
    b.close();
    b.move_to((X_OFFSET + 138.609, Y_OFFSET + 278.57));
    b.cubic_to(
        (X_OFFSET + 137.461, Y_OFFSET + 278.203),
        (X_OFFSET + 136.297, Y_OFFSET + 278.328),
        (X_OFFSET + 135.297, Y_OFFSET + 278.836),
    );
    b.cubic_to(
        (X_OFFSET + 134.297, Y_OFFSET + 279.344),
        (X_OFFSET + 133.508, Y_OFFSET + 280.219),
        (X_OFFSET + 133.133, Y_OFFSET + 281.367),
    );
    b.line_to((X_OFFSET + 129.68, Y_OFFSET + 280.242));
    b.cubic_to(
        (X_OFFSET + 131.047, Y_OFFSET + 276.039),
        (X_OFFSET + 135.539, Y_OFFSET + 273.758),
        (X_OFFSET + 139.742, Y_OFFSET + 275.117),
    );
    b.line_to((X_OFFSET + 138.609, Y_OFFSET + 278.57));
    b.close();
    b.move_to((X_OFFSET + 141.406, Y_OFFSET + 284.055));
    b.cubic_to(
        (X_OFFSET + 141.773, Y_OFFSET + 282.907),
        (X_OFFSET + 141.648, Y_OFFSET + 281.735),
        (X_OFFSET + 141.148, Y_OFFSET + 280.735),
    );
    b.cubic_to(
        (X_OFFSET + 140.625, Y_OFFSET + 279.735),
        (X_OFFSET + 139.757, Y_OFFSET + 278.946),
        (X_OFFSET + 138.609, Y_OFFSET + 278.571),
    );
    b.line_to((X_OFFSET + 139.742, Y_OFFSET + 275.118));
    b.cubic_to(
        (X_OFFSET + 143.937, Y_OFFSET + 276.493),
        (X_OFFSET + 146.219, Y_OFFSET + 280.977),
        (X_OFFSET + 144.859, Y_OFFSET + 285.173),
    );
    b.line_to((X_OFFSET + 141.406, Y_OFFSET + 284.055));
    b.close();

    // uncomment to reveal PathOps bug, see https://bugs.chromium.org/p/skia/issues/detail?id=9732
    // (void) Simplify(*path, path);

    PathDY {
        path: b.detach(),
        dy: 15.0,
    }
}

// Port of: gm/smallpaths.cpp#L258-L285 (chrome/m156)
const G_WIDTHS: [f32; 11] = [2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 7.0, 14.0, 0.0, 0.0, 0.0];

// Port of: gm/smallpaths.cpp#L287-L300 (chrome/m156)
const G_MITERS: [f32; 11] = [2.0, 3.0, 3.0, 3.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0];

// Port of: gm/smallpaths.cpp#L302-L315 (chrome/m156)
const G_X_TRANSLATE: [f32; 11] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -220.625, 0.0, 0.0];

// Port of: gm/smallpaths.cpp#L317 (chrome/m156)
const N: usize = 11;

// Port of: gm/smallpaths.cpp#L319-L392 (chrome/m156)
// This GM tests out drawing small paths (i.e., for Ganesh, using the Distance
// Field path renderer) which are filled, stroked and filledAndStroked. In
// particular this ensures that any cache keys in use include the stroking
// parameters.
struct SmallPathsGm {
    paths: Vec<Path>,
    dy: [f32; N],
}

impl SmallPathsGm {
    // Port of: gm/smallpaths.cpp#L327-L333 (chrome/m156)
    fn new() -> Self {
        let procs: [fn() -> PathDY; N] = [
            make_triangle,
            make_rect,
            make_oval,
            make_star_5,
            make_star_13,
            make_three_line,
            make_arrow,
            make_curve,
            make_battery,
            make_battery2,
            make_ring,
        ];
        let mut paths = Vec::with_capacity(N);
        let mut dy = [0.0_f32; N];
        for (i, proc_) in procs.iter().enumerate() {
            let pdy = proc_();
            paths.push(pdy.path);
            dy[i] = pdy.dy;
        }
        Self { paths, dy }
    }
}

impl GM for SmallPathsGm {
    fn name(&self) -> String {
        "smallpaths".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 512)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        // first column: filled paths
        canvas.save();
        for i in 0..N {
            canvas.draw_path(&self.paths[i], &paint);
            canvas.translate((G_X_TRANSLATE[i], self.dy[i]));
        }
        canvas.restore();
        canvas.translate((120.0, 0.0));

        // second column: stroked paths
        canvas.save();
        paint.set_style(Style::Stroke);
        paint.set_stroke_cap(Cap::Butt);
        for i in 0..N {
            paint.set_stroke_width(G_WIDTHS[i]);
            paint.set_stroke_miter(G_MITERS[i]);
            canvas.draw_path(&self.paths[i], &paint);
            canvas.translate((G_X_TRANSLATE[i], self.dy[i]));
        }
        canvas.restore();
        canvas.translate((120.0, 0.0));

        // third column: stroked paths with different widths
        canvas.save();
        paint.set_style(Style::Stroke);
        paint.set_stroke_cap(Cap::Butt);
        for i in 0..N {
            paint.set_stroke_width(G_WIDTHS[i] + 2.0);
            paint.set_stroke_miter(G_MITERS[i]);
            canvas.draw_path(&self.paths[i], &paint);
            canvas.translate((G_X_TRANSLATE[i], self.dy[i]));
        }
        canvas.restore();
        canvas.translate((120.0, 0.0));

        // fourth column: stroked and filled paths
        paint.set_style(Style::StrokeAndFill);
        paint.set_stroke_cap(Cap::Butt);
        for i in 0..N {
            paint.set_stroke_width(G_WIDTHS[i]);
            paint.set_stroke_miter(G_MITERS[i]);
            canvas.draw_path(&self.paths[i], &paint);
            canvas.translate((G_X_TRANSLATE[i], self.dy[i]));
        }
    }
}

crate::def_gm!(SmallPathsGM, SmallPathsGm::new());
