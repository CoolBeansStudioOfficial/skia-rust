// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/gradients_2pt_conical.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::color::colors;
use skia_rust_core::floating_point::float_midpoint;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar_interp;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/gradients_2pt_conical.cpp#L24-L35 (chrome/m156)
struct GradData {
    count: usize,
    colors: &'static [Color4f],
    pos: &'static [f32],
}

impl GradData {
    fn grad(&self, tm: TileMode) -> Gradient<'_> {
        Gradient::new(
            Colors::new(
                &self.colors[..self.count],
                Some(&self.pos[..self.count]),
                tm,
                None,
            ),
            Interpolation::default(),
        )
    }
}

// Port of: gm/gradients_2pt_conical.cpp#L37-L55 (chrome/m156)
const G_COLORS: [Color4f; 5] = [
    colors::RED,
    colors::GREEN,
    colors::BLUE,
    colors::WHITE,
    colors::BLACK,
];
const G_POS0: [f32; 2] = [0.0, 1.0];
const G_POS1: [f32; 2] = [1.0 / 4.0, 1.0 * 3.0 / 4.0];
const G_POS2: [f32; 5] = [0.0, 1.0 / 8.0, 1.0 / 2.0, 1.0 * 7.0 / 8.0, 1.0];

const G_POS_CLAMP: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const G_COLOR_CLAMP: [Color4f; 4] = [colors::RED, colors::GREEN, colors::GREEN, colors::BLUE];

// Port of: gm/gradients_2pt_conical.cpp#L57-L61 (chrome/m156)
static G_GRAD_DATA: [GradData; 4] = [
    GradData {
        count: 2,
        colors: &G_COLORS,
        pos: &G_POS0,
    },
    GradData {
        count: 2,
        colors: &G_COLORS,
        pos: &G_POS1,
    },
    GradData {
        count: 5,
        colors: &G_COLORS,
        pos: &G_POS2,
    },
    GradData {
        count: 4,
        colors: &G_COLOR_CLAMP,
        pos: &G_POS_CLAMP,
    },
];

fn two_point_conical(
    (center0, radius0): (Point, f32),
    (center1, radius1): (Point, f32),
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    shaders::two_point_conical_gradient(
        (center0, radius0),
        (center1, radius1),
        &data.grad(tm),
        Some(local_matrix),
    )
}

fn midpoint(pts: &[Point; 2]) -> Point {
    Point::new(
        float_midpoint(pts[0].x, pts[1].x),
        float_midpoint(pts[0].y, pts[1].y),
    )
}

/// `center1` of the "inside" cases.
fn interp3_5_1_4(pts: &[Point; 2]) -> Point {
    Point::new(
        scalar_interp(pts[0].x, pts[1].x, 3.0 / 5.0),
        scalar_interp(pts[0].y, pts[1].y, 1.0 / 4.0),
    )
}

type GradMaker = fn(&[Point; 2], &GradData, TileMode, &Matrix) -> Option<Shader>;

// Port of: gm/gradients_2pt_conical.cpp#L63-L72 (chrome/m156)
fn make_2_conical_outside(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let radius0 = (pts[1].x - pts[0].x) / 10.0;
    let radius1 = (pts[1].x - pts[0].x) / 3.0;
    let center0 = Point::new(pts[0].x + radius0, pts[0].y + radius0);
    let center1 = Point::new(pts[1].x - radius1, pts[1].y - radius1);
    two_point_conical(
        (center0, radius0),
        (center1, radius1),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L74-L82 (chrome/m156)
fn make_2_conical_outside_strip(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let radius = (pts[1].x - pts[0].x) / 3.0;
    let center0 = Point::new(pts[0].x, pts[0].y);
    let center1 = Point::new(pts[1].x, pts[1].y);
    two_point_conical((center0, radius), (center1, radius), data, tm, local_matrix)
}

// Port of: gm/gradients_2pt_conical.cpp#L84-L93 (chrome/m156)
fn make_2_conical_outside_flip(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let radius0 = (pts[1].x - pts[0].x) / 10.0;
    let radius1 = (pts[1].x - pts[0].x) / 3.0;
    let center0 = Point::new(pts[0].x + radius0, pts[0].y + radius0);
    let center1 = Point::new(pts[1].x - radius1, pts[1].y - radius1);
    two_point_conical(
        (center1, radius1),
        (center0, radius0),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L95-L105 (chrome/m156)
fn make_2_conical_inside(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let center0 = midpoint(pts);
    let center1 = interp3_5_1_4(pts);
    two_point_conical(
        (center1, (pts[1].x - pts[0].x) / 7.0),
        (center0, (pts[1].x - pts[0].x) / 2.0),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L107-L117 (chrome/m156)
fn make_2_conical_inside_flip(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let center0 = midpoint(pts);
    let center1 = interp3_5_1_4(pts);
    two_point_conical(
        (center0, (pts[1].x - pts[0].x) / 2.0),
        (center1, (pts[1].x - pts[0].x) / 7.0),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L119-L126 (chrome/m156)
fn make_2_conical_inside_center(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let center0 = midpoint(pts);
    two_point_conical(
        (center0, (pts[1].x - pts[0].x) / 7.0),
        (center0, (pts[1].x - pts[0].x) / 2.0),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L128-L135 (chrome/m156)
fn make_2_conical_inside_center_reversed(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let center0 = midpoint(pts);
    two_point_conical(
        (center0, (pts[1].x - pts[0].x) / 2.0),
        (center0, (pts[1].x - pts[0].x) / 7.0),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L137-L147 (chrome/m156)
fn make_2_conical_zero_rad(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let center0 = midpoint(pts);
    let center1 = interp3_5_1_4(pts);
    two_point_conical(
        (center1, 0.0),
        (center0, (pts[1].x - pts[0].x) / 2.0),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L149-L158 (chrome/m156)
fn make_2_conical_zero_rad_flip(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let center0 = midpoint(pts);
    let center1 = interp3_5_1_4(pts);
    two_point_conical(
        (center1, (pts[1].x - pts[0].x) / 2.0),
        (center0, 0.0),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L160-L169 (chrome/m156)
fn make_2_conical_zero_rad_center(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let center0 = midpoint(pts);
    two_point_conical(
        (center0, 0.0),
        (center0, (pts[1].x - pts[0].x) / 2.0),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L171-L181 (chrome/m156)
fn make_2_conical_zero_rad_outside(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let radius0 = 0.0f32;
    let radius1 = (pts[1].x - pts[0].x) / 3.0;
    let center0 = Point::new(pts[0].x + radius0, pts[0].y + radius0);
    let center1 = Point::new(pts[1].x - radius1, pts[1].y - radius1);
    two_point_conical(
        (center0, radius0),
        (center1, radius1),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L183-L192 (chrome/m156)
fn make_2_conical_zero_rad_flip_outside(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let radius0 = 0.0f32;
    let radius1 = (pts[1].x - pts[0].x) / 3.0;
    let center0 = Point::new(pts[0].x + radius0, pts[0].y + radius0);
    let center1 = Point::new(pts[1].x - radius1, pts[1].y - radius1);
    two_point_conical(
        (center1, radius1),
        (center0, radius0),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L194-L203 (chrome/m156)
fn make_2_conical_edge_x(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let radius0 = (pts[1].x - pts[0].x) / 7.0;
    let radius1 = (pts[1].x - pts[0].x) / 3.0;
    let center1 = midpoint(pts);
    let center0 = Point::new(center1.x + radius1, center1.y);
    two_point_conical(
        (center0, radius0),
        (center1, radius1),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L205-L214 (chrome/m156)
fn make_2_conical_edge_y(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let radius0 = (pts[1].x - pts[0].x) / 7.0;
    let radius1 = (pts[1].x - pts[0].x) / 3.0;
    let center1 = midpoint(pts);
    let center0 = Point::new(center1.x, center1.y + radius1);
    two_point_conical(
        (center0, radius0),
        (center1, radius1),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L216-L225 (chrome/m156)
fn make_2_conical_zero_rad_edge_x(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let radius0 = 0.0f32;
    let radius1 = (pts[1].x - pts[0].x) / 3.0;
    let center1 = midpoint(pts);
    let center0 = Point::new(center1.x + radius1, center1.y);
    two_point_conical(
        (center0, radius0),
        (center1, radius1),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L227-L236 (chrome/m156)
fn make_2_conical_zero_rad_edge_y(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let radius0 = 0.0f32;
    let radius1 = (pts[1].x - pts[0].x) / 3.0;
    let center1 = midpoint(pts);
    let center0 = Point::new(center1.x, center1.y + radius1);
    two_point_conical(
        (center0, radius0),
        (center1, radius1),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L238-L247 (chrome/m156)
fn make_2_conical_touch_x(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let radius0 = (pts[1].x - pts[0].x) / 7.0;
    let radius1 = (pts[1].x - pts[0].x) / 3.0;
    let center1 = midpoint(pts);
    let center0 = Point::new(center1.x - radius1 + radius0, center1.y);
    two_point_conical(
        (center0, radius0),
        (center1, radius1),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L249-L258 (chrome/m156)
fn make_2_conical_touch_y(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let radius0 = (pts[1].x - pts[0].x) / 7.0;
    let radius1 = (pts[1].x - pts[0].x) / 3.0;
    let center1 = midpoint(pts);
    let center0 = Point::new(center1.x, center1.y + radius1 - radius0);
    two_point_conical(
        (center0, radius0),
        (center1, radius1),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L260-L269 (chrome/m156)
#[allow(clippy::excessive_precision)] // Skia's literal kept verbatim
fn make_2_conical_inside_small_rad(
    pts: &[Point; 2],
    data: &GradData,
    tm: TileMode,
    local_matrix: &Matrix,
) -> Option<Shader> {
    let center0 = midpoint(pts);
    two_point_conical(
        (center0, 0.000_000_000_000_000_000_1),
        (center0, (pts[1].x - pts[0].x) / 2.0),
        data,
        tm,
        local_matrix,
    )
}

// Port of: gm/gradients_2pt_conical.cpp#L271-L275 (chrome/m156)
const G_GRAD_MAKERS_OUTSIDE: [GradMaker; 5] = [
    make_2_conical_outside,
    make_2_conical_outside_flip,
    make_2_conical_zero_rad_outside,
    make_2_conical_zero_rad_flip_outside,
    make_2_conical_outside_strip,
];

// Port of: gm/gradients_2pt_conical.cpp#L277-L282 (chrome/m156)
const G_GRAD_MAKERS_INSIDE: [GradMaker; 7] = [
    make_2_conical_inside,
    make_2_conical_inside_flip,
    make_2_conical_inside_center,
    make_2_conical_zero_rad,
    make_2_conical_zero_rad_flip,
    make_2_conical_zero_rad_center,
    make_2_conical_inside_center_reversed,
];

// Port of: gm/gradients_2pt_conical.cpp#L284-L289 (chrome/m156)
const G_GRAD_MAKERS_EDGE_CASES: [GradMaker; 7] = [
    make_2_conical_edge_x,
    make_2_conical_edge_y,
    make_2_conical_zero_rad_edge_x,
    make_2_conical_zero_rad_edge_y,
    make_2_conical_touch_x,
    make_2_conical_touch_y,
    make_2_conical_inside_small_rad,
];

// Port of: gm/gradients_2pt_conical.cpp#L291-L302 (chrome/m156)
struct GradCase {
    maker: &'static [GradMaker],
    name: &'static str,
}

static G_GRAD_CASES: [GradCase; 3] = [
    GradCase {
        maker: &G_GRAD_MAKERS_OUTSIDE,
        name: "outside",
    },
    GradCase {
        maker: &G_GRAD_MAKERS_INSIDE,
        name: "inside",
    },
    GradCase {
        maker: &G_GRAD_MAKERS_EDGE_CASES,
        name: "edge",
    },
];

// these must match the order in gGradCases
// Port of: gm/gradients_2pt_conical.cpp#L304-L308 (chrome/m156)
#[derive(Copy, Clone)]
enum GradCaseType {
    Outside = 0,
    Inside = 1,
    Edge = 2,
}

///////////////////////////////////////////////////////////////////////////////

// Port of: gm/gradients_2pt_conical.cpp#L312-L384 (chrome/m156)
struct ConicalGradientsGm {
    grad_case_type: GradCaseType,
    name: String,
    dither: bool,
    mode: TileMode,
    bg: Color,
}

impl ConicalGradientsGm {
    fn new(grad_case_type: GradCaseType, dither: bool, mode: TileMode) -> Self {
        let mut name = format!(
            "gradients_2pt_conical_{}{}",
            G_GRAD_CASES[grad_case_type as usize].name,
            if dither { "" } else { "_nodither" }
        );
        match mode {
            TileMode::Repeat => name.push_str("_repeat"),
            TileMode::Mirror => name.push_str("_mirror"),
            _ => {}
        }
        Self {
            grad_case_type,
            name,
            dither,
            mode,
            bg: Color::WHITE,
        }
    }
}

impl GM for ConicalGradientsGm {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn size(&mut self) -> ISize {
        ISize::new(840, 815)
    }

    fn bg_color(&self) -> Color {
        self.bg
    }

    fn on_once_before_draw(&mut self) {
        self.bg = Color::new(0xFFDD_DDDD);
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let pts = [Point::new(0.0, 0.0), Point::new(100.0, 100.0)];
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_dither(self.dither);

        canvas.translate((20.0, 20.0));

        let grad_maker = G_GRAD_CASES[self.grad_case_type as usize].maker;
        let count = grad_maker.len();

        for (i, data) in G_GRAD_DATA.iter().enumerate() {
            canvas.save();
            for maker in grad_maker.iter().take(count) {
                let mut scale = Matrix::new_identity();

                if i == 3 {
                    // if the clamp case
                    scale.set_scale((0.5, 0.5), None);
                    scale.post_translate((25.0, 25.0));
                }

                paint.set_shader(maker(&pts, data, self.mode, &scale));
                canvas.draw_rect(r, &paint);
                canvas.translate((0.0, 120.0));
            }
            canvas.restore();
            canvas.translate((120.0, 0.0));
        }
    }
}

///////////////////////////////////////////////////////////////////////////////

// Port of: gm/gradients_2pt_conical.cpp#L388-L402 (chrome/m156)
crate::def_gm!(
    ConicalGradientsGM_kInside_true = "ConicalGradientsGM(kInside_GradCaseType, true)",
    ConicalGradientsGm::new(GradCaseType::Inside, true, TileMode::Clamp)
);
crate::def_gm!(
    ConicalGradientsGM_kOutside_true = "ConicalGradientsGM(kOutside_GradCaseType, true)",
    ConicalGradientsGm::new(GradCaseType::Outside, true, TileMode::Clamp)
);
crate::def_gm!(
    ConicalGradientsGM_kEdge_true = "ConicalGradientsGM(kEdge_GradCaseType, true)",
    ConicalGradientsGm::new(GradCaseType::Edge, true, TileMode::Clamp)
);

crate::def_gm!(
    ConicalGradientsGM_kInside_true_repeat =
        "ConicalGradientsGM(kInside_GradCaseType, true, SkTileMode::kRepeat)",
    ConicalGradientsGm::new(GradCaseType::Inside, true, TileMode::Repeat)
);
crate::def_gm!(
    ConicalGradientsGM_kOutside_true_repeat =
        "ConicalGradientsGM(kOutside_GradCaseType, true, SkTileMode::kRepeat)",
    ConicalGradientsGm::new(GradCaseType::Outside, true, TileMode::Repeat)
);
crate::def_gm!(
    ConicalGradientsGM_kEdge_true_repeat =
        "ConicalGradientsGM(kEdge_GradCaseType, true, SkTileMode::kRepeat)",
    ConicalGradientsGm::new(GradCaseType::Edge, true, TileMode::Repeat)
);

crate::def_gm!(
    ConicalGradientsGM_kInside_true_mirror =
        "ConicalGradientsGM(kInside_GradCaseType, true, SkTileMode::kMirror)",
    ConicalGradientsGm::new(GradCaseType::Inside, true, TileMode::Mirror)
);
crate::def_gm!(
    ConicalGradientsGM_kOutside_true_mirror =
        "ConicalGradientsGM(kOutside_GradCaseType, true, SkTileMode::kMirror)",
    ConicalGradientsGm::new(GradCaseType::Outside, true, TileMode::Mirror)
);
crate::def_gm!(
    ConicalGradientsGM_kEdge_true_mirror =
        "ConicalGradientsGM(kEdge_GradCaseType, true, SkTileMode::kMirror)",
    ConicalGradientsGm::new(GradCaseType::Edge, true, TileMode::Mirror)
);

crate::def_gm!(
    ConicalGradientsGM_kInside_false = "ConicalGradientsGM(kInside_GradCaseType, false)",
    ConicalGradientsGm::new(GradCaseType::Inside, false, TileMode::Clamp)
);
crate::def_gm!(
    ConicalGradientsGM_kOutside_false = "ConicalGradientsGM(kOutside_GradCaseType, false)",
    ConicalGradientsGm::new(GradCaseType::Outside, false, TileMode::Clamp)
);
crate::def_gm!(
    ConicalGradientsGM_kEdge_false = "ConicalGradientsGM(kEdge_GradCaseType, false)",
    ConicalGradientsGm::new(GradCaseType::Edge, false, TileMode::Clamp)
);
