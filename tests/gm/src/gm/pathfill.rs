// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/pathfill.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_utils::fill_path_with_paint_to_path;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::scalar::{scalar_cos, scalar_sin, SCALAR_PI};

fn p(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

// Port of: gm/pathfill.cpp#L27-L30 (chrome/m156)
struct PathDY {
    path: Path,
    dy: f32,
}

// Port of: gm/pathfill.cpp#L36-L44 (chrome/m156)
fn make_frame() -> PathDY {
    let r = Rect::new(10.0, 10.0, 630.0, 470.0);
    let path = Path::rrect(RRect::new_rect_xy(r, 15.0, 15.0), None);
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(5.0);
    PathDY {
        path: fill_path_with_paint_to_path(&path, &paint).0,
        dy: 15.0,
    }
}

// Port of: gm/pathfill.cpp#L46-L60 (chrome/m156)
fn make_triangle() -> PathDY {
    const G_COORD: [i32; 6] = [10, 20, 15, 5, 30, 30];
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    let c = |i: usize| G_COORD[i] as f32;
    PathDY {
        path: PathBuilder::new()
            .move_to((c(0), c(1)))
            .line_to((c(2), c(3)))
            .line_to((c(4), c(5)))
            .close()
            .offset((10.0, 0.0))
            .detach(),
        dy: 30.0,
    }
}

// Port of: gm/pathfill.cpp#L62-L70 (chrome/m156)
fn make_rect() -> PathDY {
    let r = Rect::new(10.0, 10.0, 30.0, 30.0);
    PathDY {
        path: PathBuilder::new()
            .add_rect(r, None, None)
            .offset((10.0, 0.0))
            .detach(),
        dy: 30.0,
    }
}

// Port of: gm/pathfill.cpp#L72-L80 (chrome/m156)
fn make_oval() -> PathDY {
    let r = Rect::new(10.0, 10.0, 30.0, 30.0);
    PathDY {
        path: PathBuilder::new()
            .add_oval(r, None, None)
            .offset((10.0, 0.0))
            .detach(),
        dy: 30.0,
    }
}

// Port of: gm/pathfill.cpp#L82-L102 (chrome/m156)
fn make_sawtooth(teeth: i32) -> PathDY {
    let mut x: f32 = 20.0;
    let y: f32 = 20.0;
    let x0 = x;
    let dx: f32 = 5.0;
    let dy: f32 = 10.0;

    let mut builder = PathBuilder::new();
    builder.move_to((x, y));
    for _ in 0..teeth {
        x += dx;
        builder.line_to((x, y - dy));
        x += dx;
        builder.line_to((x, y + dy));
    }
    builder.line_to((x, y + (2.0 * dy)));
    builder.line_to((x0, y + (2.0 * dy)));
    builder.close();

    PathDY {
        path: builder.detach(),
        dy: 30.0,
    }
}

// Port of: gm/pathfill.cpp#L104-L105 (chrome/m156)
fn make_sawtooth_3() -> PathDY {
    make_sawtooth(3)
}
fn make_sawtooth_32() -> PathDY {
    make_sawtooth(32)
}

// Port of: gm/pathfill.cpp#L107-L138 (chrome/m156)
fn make_house() -> PathDY {
    let mut builder = PathBuilder::new();
    builder
        .add_polygon(
            &[
                p(21.0, 23.0),
                p(21.0, 11.534),
                p(22.327, 12.741),
                p(23.673, 11.261),
                p(12.0, 0.648),
                p(8.0, 4.285),
                p(8.0, 2.0),
                p(4.0, 2.0),
                p(4.0, 7.921),
                p(0.327, 11.26),
                p(1.673, 12.74),
                p(3.0, 11.534),
                p(3.0, 23.0),
                p(11.0, 23.0),
                p(11.0, 18.0),
                p(13.0, 18.0),
                p(13.0, 23.0),
                p(21.0, 23.0),
            ],
            true,
        )
        .polyline_to(&[
            p(9.0, 16.0),
            p(9.0, 21.0),
            p(5.0, 21.0),
            p(5.0, 9.715),
            p(12.0, 3.351),
            p(19.0, 9.715),
            p(19.0, 21.0),
            p(15.0, 21.0),
            p(15.0, 16.0),
            p(9.0, 16.0),
        ])
        .close()
        .offset((20.0, 0.0));
    PathDY {
        path: builder.detach(),
        dy: 30.0,
    }
}

// Port of: gm/pathfill.cpp#L140-L158 (chrome/m156)
fn make_star(n: i32) -> PathDY {
    let c: f32 = 45.0;
    let r: f32 = 20.0;

    let mut rad: f32 = -SCALAR_PI / 2.0;
    #[allow(clippy::cast_precision_loss)] // int * SkScalar in C++
    let drad: f32 = (n >> 1) as f32 * SCALAR_PI * 2.0 / n as f32;

    let mut builder = PathBuilder::new();
    builder.move_to((c, c - r));
    for _ in 1..n {
        rad += drad;
        builder.line_to((c + scalar_cos(rad) * r, c + scalar_sin(rad) * r));
    }
    builder.close();

    PathDY {
        path: builder.detach(),
        dy: r * 2.0 * 6.0 / 5.0,
    }
}

// Port of: gm/pathfill.cpp#L160-L161 (chrome/m156)
fn make_star_5() -> PathDY {
    make_star(5)
}
fn make_star_13() -> PathDY {
    make_star(13)
}

// We don't expect any output from this path.
// Port of: gm/pathfill.cpp#L164-L175 (chrome/m156)
fn make_line() -> PathDY {
    PathDY {
        path: PathBuilder::new()
            .move_to((30.0, 30.0))
            .line_to((120.0, 40.0))
            .close()
            .move_to((150.0, 30.0))
            .line_to((150.0, 30.0))
            .line_to((300.0, 40.0))
            .close()
            .detach(),
        dy: 40.0,
    }
}

// Port of: gm/pathfill.cpp#L177-L208 (chrome/m156)
#[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
fn make_info() -> Path {
    let mut path = PathBuilder::new();
    path.move_to((24.0, 4.0));
    path.cubic_to((12.94999980926514, 4.0), (4.0, 12.94999980926514), (4.0, 24.0));
    path.cubic_to((4.0, 35.04999923706055), (12.94999980926514, 44.0), (24.0, 44.0));
    path.cubic_to((35.04999923706055, 44.0), (44.0, 35.04999923706055), (44.0, 24.0));
    path.cubic_to((44.0, 12.95000076293945), (35.04999923706055, 4.0), (24.0, 4.0));
    path.close();
    path.move_to((26.0, 34.0));
    path.line_to((22.0, 34.0));
    path.line_to((22.0, 22.0));
    path.line_to((26.0, 22.0));
    path.line_to((26.0, 34.0));
    path.close();
    path.move_to((26.0, 18.0));
    path.line_to((22.0, 18.0));
    path.line_to((22.0, 14.0));
    path.line_to((26.0, 14.0));
    path.line_to((26.0, 18.0));
    path.close();
    path.detach()
}

// Port of: gm/pathfill.cpp#L210-L248 (chrome/m156)
#[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
fn make_accessibility() -> Path {
    let mut path = PathBuilder::new();
    path.move_to((12.0, 2.0));
    path.cubic_to((13.10000038146973, 2.0), (14.0, 2.900000095367432), (14.0, 4.0));
    path.cubic_to((14.0, 5.099999904632568), (13.10000038146973, 6.0), (12.0, 6.0));
    path.cubic_to((10.89999961853027, 6.0), (10.0, 5.099999904632568), (10.0, 4.0));
    path.cubic_to((10.0, 2.900000095367432), (10.89999961853027, 2.0), (12.0, 2.0));
    path.close();
    path.move_to((21.0, 9.0));
    path.line_to((15.0, 9.0));
    path.line_to((15.0, 22.0));
    path.line_to((13.0, 22.0));
    path.line_to((13.0, 16.0));
    path.line_to((11.0, 16.0));
    path.line_to((11.0, 22.0));
    path.line_to((9.0, 22.0));
    path.line_to((9.0, 9.0));
    path.line_to((3.0, 9.0));
    path.line_to((3.0, 7.0));
    path.line_to((21.0, 7.0));
    path.line_to((21.0, 9.0));
    path.close();
    path.detach()
}

// test case for http://crbug.com/695196
// Port of: gm/pathfill.cpp#L250-L296 (chrome/m156)
fn make_visualizer() -> Path {
    let mut path = PathBuilder::new();
    path.move_to((1.9520, 2.0000));
    path.conic_to((1.5573, 1.9992), (1.2782, 2.2782), 0.9235);
    path.conic_to((0.9992, 2.5573), (1.0000, 2.9520), 0.9235);
    path.line_to((1.0000, 5.4300));
    path.line_to((17.0000, 5.4300));
    path.line_to((17.0000, 2.9520));
    path.conic_to((17.0008, 2.5573), (16.7218, 2.2782), 0.9235);
    path.conic_to((16.4427, 1.9992), (16.0480, 2.0000), 0.9235);
    path.line_to((1.9520, 2.0000));
    path.close();
    path.move_to((2.7140, 3.1430));
    path.conic_to((3.0547, 3.1287), (3.2292, 3.4216), 0.8590);
    path.conic_to((3.4038, 3.7145), (3.2292, 4.0074), 0.8590);
    path.conic_to((3.0547, 4.3003), (2.7140, 4.2860), 0.8590);
    path.conic_to((2.1659, 4.2631), (2.1659, 3.7145), 0.7217);
    path.conic_to((2.1659, 3.1659), (2.7140, 3.1430), 0.7217);
    path.line_to((2.7140, 3.1430));
    path.close();
    path.move_to((5.0000, 3.1430));
    path.conic_to((5.3407, 3.1287), (5.5152, 3.4216), 0.8590);
    path.conic_to((5.6898, 3.7145), (5.5152, 4.0074), 0.8590);
    path.conic_to((5.3407, 4.3003), (5.0000, 4.2860), 0.8590);
    path.conic_to((4.4519, 4.2631), (4.4519, 3.7145), 0.7217);
    path.conic_to((4.4519, 3.1659), (5.0000, 3.1430), 0.7217);
    path.line_to((5.0000, 3.1430));
    path.close();
    path.move_to((7.2860, 3.1430));
    path.conic_to((7.6267, 3.1287), (7.8012, 3.4216), 0.8590);
    path.conic_to((7.9758, 3.7145), (7.8012, 4.0074), 0.8590);
    path.conic_to((7.6267, 4.3003), (7.2860, 4.2860), 0.8590);
    path.conic_to((6.7379, 4.2631), (6.7379, 3.7145), 0.7217);
    path.conic_to((6.7379, 3.1659), (7.2860, 3.1430), 0.7217);
    path.close();
    path.move_to((1.0000, 6.1900));
    path.line_to((1.0000, 14.3810));
    path.conic_to((0.9992, 14.7757), (1.2782, 15.0548), 0.9235);
    path.conic_to((1.5573, 15.3338), (1.9520, 15.3330), 0.9235);
    path.line_to((16.0480, 15.3330));
    path.conic_to((16.4427, 15.3338), (16.7218, 15.0548), 0.9235);
    path.conic_to((17.0008, 14.7757), (17.0000, 14.3810), 0.9235);
    path.line_to((17.0000, 6.1910));
    path.line_to((1.0000, 6.1910));
    path.line_to((1.0000, 6.1900));
    path.close();
    path.detach()
}

const G_PROCS: [fn() -> PathDY; 10] = [
    make_frame,
    make_triangle,
    make_rect,
    make_oval,
    make_sawtooth_32,
    make_star_5,
    make_star_13,
    make_line,
    make_house,
    make_sawtooth_3,
];

const N: usize = G_PROCS.len();

// Port of: gm/pathfill.cpp#L312-L358 (chrome/m156)
struct PathFillGM {
    path: Vec<Path>,
    dy: [f32; N],
    info_path: Path,
    accessibility_path: Path,
    visualizer_path: Path,
}

impl PathFillGM {
    fn new() -> Self {
        Self {
            path: Vec::new(),
            dy: [0.0; N],
            info_path: Path::default(),
            accessibility_path: Path::default(),
            visualizer_path: Path::default(),
        }
    }
}

impl GM for PathFillGM {
    fn name(&self) -> String {
        "pathfill".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    fn on_once_before_draw(&mut self) {
        for (i, proc) in G_PROCS.iter().enumerate() {
            let PathDY { path, dy } = proc();
            self.path.push(path);
            self.dy[i] = dy;
        }

        self.info_path = make_info();
        self.accessibility_path = make_accessibility();
        self.visualizer_path = make_visualizer();
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        for i in 0..N {
            canvas.draw_path(&self.path[i], &paint);
            canvas.translate((0.0, self.dy[i]));
        }

        canvas.save();
        canvas.scale((0.300000011920929, 0.300000011920929));
        canvas.translate((50.0, 50.0));
        canvas.draw_path(&self.info_path, &paint);
        canvas.restore();

        canvas.scale((2.0, 2.0));
        canvas.translate((5.0, 15.0));
        canvas.draw_path(&self.accessibility_path, &paint);

        canvas.scale((0.5, 0.5));
        canvas.translate((5.0, 50.0));
        canvas.draw_path(&self.visualizer_path, &paint);
    }
}

// test inverse-fill w/ a clip that completely excludes the geometry
// Port of: gm/pathfill.cpp#L361-L420 (chrome/m156)
struct PathInverseFillGM {
    path: Vec<Path>,
}

impl PathInverseFillGM {
    fn new() -> Self {
        Self { path: Vec::new() }
    }

    // Port of: gm/pathfill.cpp#L379-L388 (chrome/m156)
    fn show(canvas: &Canvas, path: &Path, paint: &Paint, clip: Option<&Rect>, top: f32, bottom: f32) {
        canvas.save();
        if let Some(clip) = clip {
            let mut r = *clip;
            r.top = top;
            r.bottom = bottom;
            canvas.clip_rect(r, None, None);
        }
        canvas.draw_path(path, paint);
        canvas.restore();
    }
}

impl GM for PathInverseFillGM {
    fn name(&self) -> String {
        "pathinvfill".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(450, 220)
    }

    fn on_once_before_draw(&mut self) {
        for proc in &G_PROCS {
            let PathDY { path, .. } = proc();
            self.path.push(path);
        }
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let path = PathBuilder::new()
            .add_circle((50.0, 50.0), 40.0, None)
            .toggle_inverse_fill_type()
            .detach();

        let clip_r = Rect::new(0.0, 0.0, 100.0, 200.0);

        canvas.translate((10.0, 10.0));

        for doclip in 0..=1 {
            for aa in 0..=1 {
                let mut paint = Paint::default();
                paint.set_anti_alias(aa != 0);

                canvas.save();
                canvas.clip_rect(clip_r, None, None);

                let clip_ptr = if doclip != 0 { Some(&clip_r) } else { None };

                Self::show(canvas, &path, &paint, clip_ptr, clip_r.top, clip_r.center_y());
                Self::show(
                    canvas,
                    &path,
                    &paint,
                    clip_ptr,
                    clip_r.center_y(),
                    clip_r.bottom,
                );

                canvas.restore();
                canvas.translate((110.0, 0.0));
            }
        }
    }
}

// Port of: gm/pathfill.cpp#L422-L444 (chrome/m156)
crate::def_simple_gm!(rotatedcubicpath, canvas, 200, 200, {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_style(Style::Fill);

    canvas.translate((50.0, 50.0));
    let path = PathBuilder::new()
        .move_to((48.0, -23.0))
        .cubic_to((48.0, -29.5), (6.0, -30.0), (6.0, -30.0))
        .cubic_to((6.0, -30.0), (2.0, 0.0), (2.0, 0.0))
        .cubic_to((2.0, 0.0), (44.0, -21.5), (48.0, -23.0))
        .close()
        .detach();

    p.set_color(Color::BLUE);
    canvas.draw_path(&path, &p);

    // Rotated path, which is not antialiased on GPU
    p.set_color(Color::RED);
    canvas.rotate(90.0, None);
    canvas.draw_path(&path, &p);
});

// Port of: gm/pathfill.cpp#L448-L449 (chrome/m156)
crate::def_gm!(PathFillGM_ = "PathFillGM", PathFillGM::new());
crate::def_gm!(PathInverseFillGM_ = "PathInverseFillGM", PathInverseFillGM::new());

// Port of: gm/pathfill.cpp#L451-L596 (chrome/m156)
crate::def_simple_gm!(bug7792, canvas, 800, 800, {
    // from skbug.com/40039046 bug description
    let p = Paint::default();
    let mut path = PathBuilder::new()
        .move_to((10.0, 10.0))
        .move_to((75.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((75.0, 150.0))
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c3
    canvas.translate((200.0, 0.0));
    path = PathBuilder::new()
        .move_to((75.0, 50.0))
        .move_to((100.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((75.0, 150.0))
        .line_to((75.0, 50.0))
        .close()
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c9
    canvas.translate((200.0, 0.0));
    path = PathBuilder::new()
        .move_to((10.0, 10.0))
        .move_to((75.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((75.0, 150.0))
        .close()
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c11
    canvas.translate((-200.0 * 2.0, 200.0));
    path = PathBuilder::new()
        .move_to((75.0, 150.0))
        .line_to((75.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((75.0, 150.0))
        .move_to((75.0, 150.0))
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c14
    canvas.translate((200.0, 0.0));
    path = PathBuilder::new()
        .move_to((250.0, 75.0))
        .move_to((250.0, 75.0))
        .move_to((250.0, 75.0))
        .move_to((100.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((75.0, 150.0))
        .line_to((75.0, 75.0))
        .close()
        .line_to((0.0, 0.0))
        .close()
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c15
    canvas.translate((200.0, 0.0));
    path = PathBuilder::new()
        .move_to((75.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((75.0, 150.0))
        .move_to((250.0, 75.0))
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c17
    canvas.translate((-200.0 * 2.0, 200.0));
    path = PathBuilder::new()
        .move_to((75.0, 10.0))
        .move_to((75.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((75.0, 150.0))
        .line_to((75.0, 10.0))
        .close()
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c19
    canvas.translate((200.0, 0.0));
    path = PathBuilder::new()
        .move_to((75.0, 75.0))
        .line_to((75.0, 75.0))
        .line_to((75.0, 75.0))
        .line_to((75.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((75.0, 150.0))
        .close()
        .move_to((10.0, 10.0))
        .line_to((30.0, 10.0))
        .line_to((10.0, 30.0))
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c23
    canvas.translate((200.0, 0.0));
    path = PathBuilder::new()
        .move_to((75.0, 75.0))
        .line_to((75.0, 75.0))
        .move_to((75.0, 75.0))
        .line_to((75.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((75.0, 150.0))
        .close()
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c29
    canvas.translate((-200.0 * 2.0, 200.0));
    path = PathBuilder::new()
        .move_to((75.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((75.0, 150.0))
        .line_to((75.0, 250.0))
        .move_to((75.0, 75.0))
        .close()
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c31
    canvas.translate((200.0, 0.0));
    path = PathBuilder::new()
        .move_to((75.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((75.0, 150.0))
        .line_to((75.0, 10.0))
        .move_to((75.0, 75.0))
        .close()
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c36
    canvas.translate((200.0, 0.0));
    path = PathBuilder::new()
        .move_to((75.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((10.0, 150.0))
        .move_to((75.0, 75.0))
        .line_to((75.0, 75.0))
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c39
    canvas.translate((200.0, -200.0 * 3.0));
    path = PathBuilder::new()
        .move_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((75.0, 150.0))
        .line_to((75.0, 100.0))
        .detach();
    canvas.draw_path(&path, &p);
    // from zero_length_paths_aa
    canvas.translate((0.0, 200.0));
    path = PathBuilder::new()
        .move_to((150.0, 100.0))
        .line_to((150.0, 100.0))
        .line_to((150.0, 150.0))
        .line_to((75.0, 150.0))
        .line_to((75.0, 100.0))
        .line_to((75.0, 75.0))
        .line_to((150.0, 75.0))
        .close()
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c41
    canvas.translate((0.0, 200.0));
    path = PathBuilder::new()
        .move_to((75.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((140.0, 150.0))
        .line_to((140.0, 75.0))
        .move_to((75.0, 75.0))
        .close()
        .detach();
    canvas.draw_path(&path, &p);
    // from skbug.com/40039046#c53
    canvas.translate((0.0, 200.0));
    path = PathBuilder::new()
        .move_to((75.0, 75.0))
        .line_to((150.0, 75.0))
        .line_to((150.0, 150.0))
        .line_to((140.0, 150.0))
        .line_to((140.0, 75.0))
        .move_to((75.0, 75.0))
        .close()
        .detach();
    canvas.draw_path(&path, &p);
});

// Port of: gm/pathfill.cpp#L637-L653 (chrome/m156)
crate::def_simple_gm!(path_arcto_skbug_9077, canvas, 200, 200, {
    let mut p = Paint::default();
    p.set_color(Color::RED);
    p.set_anti_alias(true);
    p.set_style(Style::Stroke);
    p.set_stroke_width(2.0);

    let mut path = PathBuilder::new();
    let pts = [
        Point::new(20.0, 20.0),
        Point::new(100.0, 20.0),
        Point::new(100.0, 60.0),
        Point::new(130.0, 150.0),
        Point::new(180.0, 160.0),
    ];
    let radius = 60.0;
    path.move_to(pts[0]);
    path.line_to(pts[1]);
    path.line_to(pts[2]);
    path.close();
    path.arc_to_tangent(pts[3], pts[4], radius);
    canvas.draw_path(&path.detach(), &p);
});

// Port of: gm/pathfill.cpp#L655-L675 (chrome/m156)
crate::def_simple_gm!(path_skbug_11859, canvas, 512, 512, {
    let mut paint = Paint::default();
    paint.set_color(Color::RED);
    paint.set_anti_alias(true);

    let path = PathBuilder::new()
        .move_to((258.0, -2.0))
        .line_to((258.0, 258.0))
        .line_to((237.0, 258.0))
        .line_to((240.0, -2.0))
        .line_to((258.0, -2.0))
        .move_to((-2.0, -2.0))
        .line_to((240.0, -2.0))
        .line_to((238.0, 131.0))
        .line_to((-2.0, 131.0))
        .line_to((-2.0, -2.0))
        .detach();

    canvas.scale((2.0, 2.0));
    canvas.draw_path(&path, &paint);
});

// Port of: gm/pathfill.cpp#L677-L691 (chrome/m156)
crate::def_simple_gm!(path_skbug_11886, canvas, 256, 256, {
    let m = Point::new(0.0, 770.0);
    let path = PathBuilder::new()
        .move_to(m)
        .cubic_to(
            Point::new(m.x + 0.0, m.y + 1.0),
            Point::new(m.x + 20.0, m.y + -750.0),
            Point::new(m.x + 83.0, m.y + -746.0),
        )
        .detach();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    canvas.draw_path(&path, &paint);
});
