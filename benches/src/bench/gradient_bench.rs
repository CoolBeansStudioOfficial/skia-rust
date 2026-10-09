// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/GradientBench.cpp

//! Gradient shaders drawn over a 400×400 rectangle or oval (`GradientBench`), and gradient
//! creation cost (`Gradient2Bench`). Rendering benches.

use skia_rust_core::color::{Color, Color4f, colors};
use skia_rust_core::floating_point::float_midpoint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{int_to_scalar, scalar, scalar_interp};
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient_shader::{self, GradientShaderColors};

use crate::def_bench;
use crate::prelude::*;

/// `static const SkColor4f gColors[]`: the five-colour pattern repeated ten times.
// Port of: bench/GradientBench.cpp#L34-L45 (chrome/m156)
fn g_colors() -> Vec<Color4f> {
    let pattern = [
        colors::RED,
        colors::GREEN,
        colors::BLUE,
        colors::WHITE,
        colors::BLACK,
    ];
    (0..10).flat_map(|_| pattern).collect()
}

/// `const SkColor4f gShallowColors[]`.
// Port of: bench/GradientBench.cpp#L47-L48 (chrome/m156)
fn g_shallow_colors() -> Vec<Color4f> {
    vec![
        Color4f::from_color(Color::new(0xFF55_5555)),
        Color4f::from_color(Color::new(0xFF44_4444)),
    ]
}

/// `static const float gPos[] = {0.25f, 0.75f};`
// Port of: bench/GradientBench.cpp#L49-L49 (chrome/m156)
const G_POS: [scalar; 2] = [0.25, 0.75];

/// `struct GradData`: a colour count, colours, optional positions and a name. The C++ holds
/// pointers into static arrays; the port owns the colours.
// Port of: bench/GradientBench.cpp#L19-L32 (chrome/m156)
#[derive(Clone, Debug)]
struct GradData {
    colors: Vec<Color4f>,
    pos: Option<Vec<scalar>>,
    name: &'static str,
}

impl GradData {
    /// `GradData::grad(tm)`: the colours of this data, without a colour space.
    // Port of: bench/GradientBench.cpp#L24-L31 (chrome/m156)
    fn shader_colors(&self) -> GradientShaderColors<'_> {
        GradientShaderColors::from(self.colors.as_slice())
    }

    fn pos(&self) -> Option<&[scalar]> {
        self.pos.as_deref()
    }
}

/// `gGradData[index]`.
// Port of: bench/GradientBench.cpp#L53-L59 (chrome/m156)
fn g_grad_data(index: usize) -> GradData {
    let all = g_colors();
    match index {
        // { 2, gColors, nullptr, "" }
        0 => GradData {
            colors: all[..2].to_vec(),
            pos: None,
            name: "",
        },
        // { 50, gColors, nullptr, "_hicolor" }: many color gradient
        1 => GradData {
            colors: all[..50].to_vec(),
            pos: None,
            name: "_hicolor",
        },
        // { 3, gColors, nullptr, "_3color" }
        2 => GradData {
            colors: all[..3].to_vec(),
            pos: None,
            name: "_3color",
        },
        // { 2, gShallowColors, nullptr, "_shallow" }
        3 => GradData {
            colors: g_shallow_colors(),
            pos: None,
            name: "_shallow",
        },
        // { 2, gColors, gPos, "_pos" }
        4 => GradData {
            colors: all[..2].to_vec(),
            pos: Some(G_POS.to_vec()),
            name: "_pos",
        },
        _ => unreachable!("gGradData has five entries"),
    }
}

/// The midpoint of the two points (`sk_float_midpoint` on each axis), used as the centre.
fn midpoint(pts: [Point; 2]) -> Point {
    Point::new(
        float_midpoint(pts[0].x, pts[1].x),
        float_midpoint(pts[0].y, pts[1].y),
    )
}

/// `MakeLinear`: ignores scale.
// Port of: bench/GradientBench.cpp#L61-L65 (chrome/m156)
fn make_linear(pts: [Point; 2], data: &GradData, tm: TileMode, _scale: scalar) -> Option<Shader> {
    gradient_shader::linear(
        (pts[0], pts[1]),
        data.shader_colors(),
        data.pos(),
        tm,
        None,
        None,
    )
}

/// `MakeRadial`.
// Port of: bench/GradientBench.cpp#L67-L73 (chrome/m156)
fn make_radial(pts: [Point; 2], data: &GradData, tm: TileMode, scale: scalar) -> Option<Shader> {
    let center = midpoint(pts);
    gradient_shader::radial(
        center,
        center.x * scale,
        data.shader_colors(),
        data.pos(),
        tm,
        None,
        None,
    )
}

/// `MakeSweep`: ignores scale.
// Port of: bench/GradientBench.cpp#L76-L82 (chrome/m156)
fn make_sweep(pts: [Point; 2], data: &GradData, tm: TileMode, _scale: scalar) -> Option<Shader> {
    gradient_shader::sweep(
        midpoint(pts),
        data.shader_colors(),
        data.pos(),
        tm,
        None,
        None,
        None,
    )
}

/// The second centre of the conical benches: `SkScalarInterp` at 3/5 and 1/4.
// Port of: bench/GradientBench.cpp#L85-L95 (chrome/m156)
fn conical_center1(pts: [Point; 2]) -> Point {
    Point::new(
        scalar_interp(pts[0].x, pts[1].x, int_to_scalar(3) / 5.0),
        scalar_interp(pts[0].y, pts[1].y, int_to_scalar(1) / 4.0),
    )
}

/// `MakeConical`: ignores scale.
// Port of: bench/GradientBench.cpp#L85-L95 (chrome/m156)
fn make_conical(pts: [Point; 2], data: &GradData, tm: TileMode, _scale: scalar) -> Option<Shader> {
    gradient_shader::two_point_conical(
        conical_center1(pts),
        (pts[1].x - pts[0].x) / 7.0,
        midpoint(pts),
        (pts[1].x - pts[0].x) / 2.0,
        data.shader_colors(),
        data.pos(),
        tm,
        None,
        None,
    )
}

/// `MakeConicalZeroRad`: ignores scale.
// Port of: bench/GradientBench.cpp#L97-L108 (chrome/m156)
fn make_conical_zero_rad(
    pts: [Point; 2],
    data: &GradData,
    tm: TileMode,
    _scale: scalar,
) -> Option<Shader> {
    gradient_shader::two_point_conical(
        conical_center1(pts),
        0.0,
        midpoint(pts),
        (pts[1].x - pts[0].x) / 2.0,
        data.shader_colors(),
        data.pos(),
        tm,
        None,
        None,
    )
}

/// The outside-conical centres and radii: `radius0 = dx / 10`, `radius1 = dx / 3`.
// Port of: bench/GradientBench.cpp#L110-L119 (chrome/m156)
fn conical_outside_geometry(pts: [Point; 2]) -> (Point, scalar, Point, scalar) {
    let radius0 = (pts[1].x - pts[0].x) / 10.0;
    let radius1 = (pts[1].x - pts[0].x) / 3.0;
    let center0 = Point::new(pts[0].x + radius0, pts[0].y + radius0);
    let center1 = Point::new(pts[1].x - radius1, pts[1].y - radius1);
    (center0, radius0, center1, radius1)
}

/// `MakeConicalOutside`: ignores scale.
// Port of: bench/GradientBench.cpp#L110-L119 (chrome/m156)
fn make_conical_outside(
    pts: [Point; 2],
    data: &GradData,
    tm: TileMode,
    _scale: scalar,
) -> Option<Shader> {
    let (center0, radius0, center1, radius1) = conical_outside_geometry(pts);
    gradient_shader::two_point_conical(
        center0,
        radius0,
        center1,
        radius1,
        data.shader_colors(),
        data.pos(),
        tm,
        None,
        None,
    )
}

/// `MakeConicalOutsideZeroRad`: ignores scale. The start radius is 0; the centres are the same.
// Port of: bench/GradientBench.cpp#L121-L130 (chrome/m156)
fn make_conical_outside_zero_rad(
    pts: [Point; 2],
    data: &GradData,
    tm: TileMode,
    _scale: scalar,
) -> Option<Shader> {
    let (center0, _radius0, center1, radius1) = conical_outside_geometry(pts);
    gradient_shader::two_point_conical(
        center0,
        0.0,
        center1,
        radius1,
        data.shader_colors(),
        data.pos(),
        tm,
        None,
        None,
    )
}

/// `enum GradType`, in the order of `gGrads`.
// Port of: bench/GradientBench.cpp#L135-L157 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GradType {
    Linear,
    Radial,
    Sweep,
    Conical,
    ConicalZero,
    ConicalOut,
    ConicalOutZero,
}

impl GradType {
    /// `gGrads[gradType].fName`.
    // Port of: bench/GradientBench.cpp#L139-L147 (chrome/m156)
    fn name(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Radial => "radial1",
            Self::Sweep => "sweep",
            Self::Conical => "conical",
            Self::ConicalZero => "conicalZero",
            Self::ConicalOut => "conicalOut",
            Self::ConicalOutZero => "conicalOutZero",
        }
    }

    /// `gGrads[gradType].fMaker(pts, data, tm, scale)`.
    fn make(self, pts: [Point; 2], data: &GradData, tm: TileMode, scale: scalar) -> Option<Shader> {
        match self {
            Self::Linear => make_linear(pts, data, tm, scale),
            Self::Radial => make_radial(pts, data, tm, scale),
            Self::Sweep => make_sweep(pts, data, tm, scale),
            Self::Conical => make_conical(pts, data, tm, scale),
            Self::ConicalZero => make_conical_zero_rad(pts, data, tm, scale),
            Self::ConicalOut => make_conical_outside(pts, data, tm, scale),
            Self::ConicalOutZero => make_conical_outside_zero_rad(pts, data, tm, scale),
        }
    }
}

/// `enum GeomType`.
// Port of: bench/GradientBench.cpp#L159-L162 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeomType {
    Rect,
    Oval,
}

impl GeomType {
    /// `geomtypename(gt)`.
    // Port of: bench/GradientBench.cpp#L164-L174 (chrome/m156)
    fn name(self) -> &'static str {
        match self {
            Self::Rect => "rectangle",
            Self::Oval => "oval",
        }
    }
}

/// `ToolUtils::tilemode_name(mode)`.
// Port of: tools/ToolUtils.cpp#L132-L140 (chrome/m156)
pub(crate) fn tilemode_name(tm: TileMode) -> &'static str {
    match tm {
        TileMode::Clamp => "clamp",
        TileMode::Repeat => "repeat",
        TileMode::Mirror => "mirror",
        TileMode::Decal => "decal",
    }
}

/// `MakeShader(gradType, data, tm, scale)`: from (0, 0) to (kSize, kSize).
// Port of: bench/GradientBench.cpp#L246-L255 (chrome/m156)
fn make_shader(
    grad_type: GradType,
    data: &GradData,
    tm: TileMode,
    scale: scalar,
) -> Option<Shader> {
    let size = int_to_scalar(GRADIENT_SIZE);
    let pts = [Point::new(0.0, 0.0), Point::new(size, size)];
    grad_type.make(pts, data, tm, scale)
}

/// `static const int kSize = 400;`
// Port of: bench/GradientBench.cpp#L258-L258 (chrome/m156)
const GRADIENT_SIZE: i32 = 400;

/// `class GradientBench`: a gradient-filled rectangle or oval, drawn `loops` times.
// Port of: bench/GradientBench.cpp#L178-L261 (chrome/m156)
struct GradientBench {
    name: String,
    paint: Paint,
    geom_type: GeomType,
}

impl GradientBench {
    /// `GradientBench(GradType, GradData, SkTileMode, GeomType, float scale)`.
    // Port of: bench/GradientBench.cpp#L181-L202 (chrome/m156)
    fn new(
        grad_type: GradType,
        data: &GradData,
        tm: TileMode,
        geom_type: GeomType,
        scale: scalar,
    ) -> Self {
        // fName.printf("gradient_%s_%s", gGrads[gradType].fName, tilemode_name(tm));
        let mut name = format!("gradient_{}_{}", grad_type.name(), tilemode_name(tm));
        if geom_type != GeomType::Rect {
            // fName.appendf("_%s", geomtypename(geomType));
            name = format!("{name}_{}", geom_type.name());
        }
        // The exact comparison mirrors `scale != 1.f` in C++.
        #[allow(clippy::float_cmp)]
        if scale != 1.0 {
            // fName.appendf("_scale_%g", scale); (%g of these scales is the shortest form)
            name = format!("{name}_scale_{scale}");
        }
        name.push_str(data.name);
        // this->setupPaint(&fPaint): the default turns anti-aliasing on.
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_shader(make_shader(grad_type, data, tm, scale));
        Self {
            name,
            paint,
            geom_type,
        }
    }

    /// `GradientBench(GradType, GradData, bool dither)`: clamp tiling, optional dithering.
    // Port of: bench/GradientBench.cpp#L204-L220 (chrome/m156)
    fn new_dither(grad_type: GradType, data: &GradData, dither: bool) -> Self {
        // fName.printf("gradient_%s_%s", gGrads[gradType].fName, tmname) with tmname "clamp"
        let mut name = format!("gradient_{}_clamp", grad_type.name());
        name.push_str(data.name);
        if dither {
            name.push_str("_dither");
        }
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_shader(make_shader(grad_type, data, TileMode::Clamp, 1.0));
        paint.set_dither(dither);
        Self {
            name,
            paint,
            geom_type: GeomType::Rect,
        }
    }
}

impl Benchmark for GradientBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/GradientBench.cpp#L225-L227 (chrome/m156)
    fn size(&mut self) -> ISize {
        ISize::new(GRADIENT_SIZE, GRADIENT_SIZE)
    }

    // Port of: bench/GradientBench.cpp#L229-L244 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("GradientBench is a rendering bench");
        // const SkRect r = SkRect::MakeIWH(kSize, kSize);
        let r = Rect::from_iwh(GRADIENT_SIZE, GRADIENT_SIZE);
        for _ in 0..loops {
            match self.geom_type {
                GeomType::Rect => {
                    canvas.draw_rect(r, &self.paint);
                }
                GeomType::Oval => {
                    canvas.draw_oval(r, &self.paint);
                }
            }
        }
    }
}

/// `class Gradient2Bench`: a three-colour linear gradient created for every draw.
// Port of: bench/GradientBench.cpp#L315-L354 (chrome/m156)
struct Gradient2Bench {
    name: String,
    has_alpha: bool,
}

impl Gradient2Bench {
    /// `Gradient2Bench(bool hasAlpha)`.
    // Port of: bench/GradientBench.cpp#L318-L322 (chrome/m156)
    fn new(has_alpha: bool) -> Self {
        Self {
            // fName.printf("gradient_create_%s", hasAlpha ? "alpha" : "opaque");
            name: format!(
                "gradient_create_{}",
                if has_alpha { "alpha" } else { "opaque" }
            ),
            has_alpha,
        }
    }
}

impl Benchmark for Gradient2Bench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/GradientBench.cpp#L328-L351 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("Gradient2Bench is a rendering bench");
        let mut paint = Paint::default();
        self.setup_paint(&mut paint);
        // const SkRect r = { 0, 0, SkIntToScalar(4), SkIntToScalar(4) };
        let r = Rect::from_iwh(4, 4);
        // const SkPoint pts[] = { { 0, 0 }, { SkIntToScalar(100), SkIntToScalar(100) } };
        let pts = [Point::new(0.0, 0.0), Point::new(100.0, 100.0)];
        for i in 0..loops {
            let gray = u8::try_from(i % 256).expect("i % 256 fits in a byte");
            let alpha = if self.has_alpha { gray } else { 0xFF };
            // SkColorSetARGB(alpha, gray, gray, gray)
            let colors = [
                colors::BLACK,
                Color4f::from_color(Color::from_argb(alpha, gray, gray, gray)),
                colors::WHITE,
            ];
            // SkShaders::LinearGradient(pts, {{colors, {}, kClamp}, {}})
            let shader = gradient_shader::linear(
                (pts[0], pts[1]),
                colors.as_slice(),
                None,
                TileMode::Clamp,
                None,
                None,
            );
            paint.set_shader(shader);
            canvas.draw_rect(r, &paint);
        }
    }
}

// Port of: bench/GradientBench.cpp#L263-L263 (chrome/m156)
def_bench!(
    gradient_bench_linear_0 = "GradientBench(kLinear_GradType, gGradData[0])",
    GradientBench::new(
        GradType::Linear,
        &g_grad_data(0),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L264-L264 (chrome/m156)
def_bench!(
    gradient_bench_linear_1 = "GradientBench(kLinear_GradType, gGradData[1])",
    GradientBench::new(
        GradType::Linear,
        &g_grad_data(1),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L265-L265 (chrome/m156)
def_bench!(
    gradient_bench_linear_2 = "GradientBench(kLinear_GradType, gGradData[2])",
    GradientBench::new(
        GradType::Linear,
        &g_grad_data(2),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L266-L266 (chrome/m156)
def_bench!(
    gradient_bench_linear_4 = "GradientBench(kLinear_GradType, gGradData[4])",
    GradientBench::new(
        GradType::Linear,
        &g_grad_data(4),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L267-L267 (chrome/m156)
def_bench!(
    gradient_bench_linear_0_repeat =
        "GradientBench(kLinear_GradType, gGradData[0], SkTileMode::kRepeat)",
    GradientBench::new(
        GradType::Linear,
        &g_grad_data(0),
        TileMode::Repeat,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L268-L268 (chrome/m156)
def_bench!(
    gradient_bench_linear_1_repeat =
        "GradientBench(kLinear_GradType, gGradData[1], SkTileMode::kRepeat)",
    GradientBench::new(
        GradType::Linear,
        &g_grad_data(1),
        TileMode::Repeat,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L269-L269 (chrome/m156)
def_bench!(
    gradient_bench_linear_2_repeat =
        "GradientBench(kLinear_GradType, gGradData[2], SkTileMode::kRepeat)",
    GradientBench::new(
        GradType::Linear,
        &g_grad_data(2),
        TileMode::Repeat,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L270-L270 (chrome/m156)
def_bench!(
    gradient_bench_linear_0_mirror =
        "GradientBench(kLinear_GradType, gGradData[0], SkTileMode::kMirror)",
    GradientBench::new(
        GradType::Linear,
        &g_grad_data(0),
        TileMode::Mirror,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L271-L271 (chrome/m156)
def_bench!(
    gradient_bench_linear_1_mirror =
        "GradientBench(kLinear_GradType, gGradData[1], SkTileMode::kMirror)",
    GradientBench::new(
        GradType::Linear,
        &g_grad_data(1),
        TileMode::Mirror,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L272-L272 (chrome/m156)
def_bench!(
    gradient_bench_linear_2_mirror =
        "GradientBench(kLinear_GradType, gGradData[2], SkTileMode::kMirror)",
    GradientBench::new(
        GradType::Linear,
        &g_grad_data(2),
        TileMode::Mirror,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L274-L274 (chrome/m156)
def_bench!(
    gradient_bench_radial_0 = "GradientBench(kRadial_GradType, gGradData[0])",
    GradientBench::new(
        GradType::Radial,
        &g_grad_data(0),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L275-L275 (chrome/m156)
def_bench!(
    gradient_bench_radial_1 = "GradientBench(kRadial_GradType, gGradData[1])",
    GradientBench::new(
        GradType::Radial,
        &g_grad_data(1),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L276-L276 (chrome/m156)
def_bench!(
    gradient_bench_radial_2 = "GradientBench(kRadial_GradType, gGradData[2])",
    GradientBench::new(
        GradType::Radial,
        &g_grad_data(2),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L279-L279 (chrome/m156)
def_bench!(
    gradient_bench_radial_0_half =
        "GradientBench(kRadial_GradType, gGradData[0], SkTileMode::kClamp, kRect_GeomType, 0.5f)",
    GradientBench::new(
        GradType::Radial,
        &g_grad_data(0),
        TileMode::Clamp,
        GeomType::Rect,
        0.5
    )
);
// Port of: bench/GradientBench.cpp#L283-L283 (chrome/m156)
def_bench!(
    gradient_bench_radial_0_oval =
        "GradientBench(kRadial_GradType, gGradData[0], SkTileMode::kClamp, kOval_GeomType)",
    GradientBench::new(
        GradType::Radial,
        &g_grad_data(0),
        TileMode::Clamp,
        GeomType::Oval,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L285-L285 (chrome/m156)
def_bench!(
    gradient_bench_radial_0_mirror =
        "GradientBench(kRadial_GradType, gGradData[0], SkTileMode::kMirror)",
    GradientBench::new(
        GradType::Radial,
        &g_grad_data(0),
        TileMode::Mirror,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L286-L286 (chrome/m156)
def_bench!(
    gradient_bench_radial_0_repeat =
        "GradientBench(kRadial_GradType, gGradData[0], SkTileMode::kRepeat)",
    GradientBench::new(
        GradType::Radial,
        &g_grad_data(0),
        TileMode::Repeat,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L287-L287 (chrome/m156)
def_bench!(
    gradient_bench_sweep = "GradientBench(kSweep_GradType)",
    GradientBench::new(
        GradType::Sweep,
        &g_grad_data(0),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L288-L288 (chrome/m156)
def_bench!(
    gradient_bench_sweep_1 = "GradientBench(kSweep_GradType, gGradData[1])",
    GradientBench::new(
        GradType::Sweep,
        &g_grad_data(1),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L289-L289 (chrome/m156)
def_bench!(
    gradient_bench_sweep_2 = "GradientBench(kSweep_GradType, gGradData[2])",
    GradientBench::new(
        GradType::Sweep,
        &g_grad_data(2),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L290-L290 (chrome/m156)
def_bench!(
    gradient_bench_conical = "GradientBench(kConical_GradType)",
    GradientBench::new(
        GradType::Conical,
        &g_grad_data(0),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L291-L291 (chrome/m156)
def_bench!(
    gradient_bench_conical_1 = "GradientBench(kConical_GradType, gGradData[1])",
    GradientBench::new(
        GradType::Conical,
        &g_grad_data(1),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L292-L292 (chrome/m156)
def_bench!(
    gradient_bench_conical_2 = "GradientBench(kConical_GradType, gGradData[2])",
    GradientBench::new(
        GradType::Conical,
        &g_grad_data(2),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L293-L293 (chrome/m156)
def_bench!(
    gradient_bench_conical_zero = "GradientBench(kConicalZero_GradType)",
    GradientBench::new(
        GradType::ConicalZero,
        &g_grad_data(0),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L294-L294 (chrome/m156)
def_bench!(
    gradient_bench_conical_zero_1 = "GradientBench(kConicalZero_GradType, gGradData[1])",
    GradientBench::new(
        GradType::ConicalZero,
        &g_grad_data(1),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L295-L295 (chrome/m156)
def_bench!(
    gradient_bench_conical_zero_2 = "GradientBench(kConicalZero_GradType, gGradData[2])",
    GradientBench::new(
        GradType::ConicalZero,
        &g_grad_data(2),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L296-L296 (chrome/m156)
def_bench!(
    gradient_bench_conical_out = "GradientBench(kConicalOut_GradType)",
    GradientBench::new(
        GradType::ConicalOut,
        &g_grad_data(0),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L297-L297 (chrome/m156)
def_bench!(
    gradient_bench_conical_out_1 = "GradientBench(kConicalOut_GradType, gGradData[1])",
    GradientBench::new(
        GradType::ConicalOut,
        &g_grad_data(1),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L298-L298 (chrome/m156)
def_bench!(
    gradient_bench_conical_out_2 = "GradientBench(kConicalOut_GradType, gGradData[2])",
    GradientBench::new(
        GradType::ConicalOut,
        &g_grad_data(2),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L299-L299 (chrome/m156)
def_bench!(
    gradient_bench_conical_out_zero = "GradientBench(kConicalOutZero_GradType)",
    GradientBench::new(
        GradType::ConicalOutZero,
        &g_grad_data(0),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L300-L300 (chrome/m156)
def_bench!(
    gradient_bench_conical_out_zero_1 = "GradientBench(kConicalOutZero_GradType, gGradData[1])",
    GradientBench::new(
        GradType::ConicalOutZero,
        &g_grad_data(1),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Port of: bench/GradientBench.cpp#L301-L301 (chrome/m156)
def_bench!(
    gradient_bench_conical_out_zero_2 = "GradientBench(kConicalOutZero_GradType, gGradData[2])",
    GradientBench::new(
        GradType::ConicalOutZero,
        &g_grad_data(2),
        TileMode::Clamp,
        GeomType::Rect,
        1.0
    )
);
// Dithering: Port of: bench/GradientBench.cpp#L304-L311 (chrome/m156)
def_bench!(
    gradient_bench_linear_3_dither = "GradientBench(kLinear_GradType, gGradData[3], true)",
    GradientBench::new_dither(GradType::Linear, &g_grad_data(3), true)
);
def_bench!(
    gradient_bench_linear_3_no_dither = "GradientBench(kLinear_GradType, gGradData[3], false)",
    GradientBench::new_dither(GradType::Linear, &g_grad_data(3), false)
);
def_bench!(
    gradient_bench_radial_3_dither = "GradientBench(kRadial_GradType, gGradData[3], true)",
    GradientBench::new_dither(GradType::Radial, &g_grad_data(3), true)
);
def_bench!(
    gradient_bench_radial_3_no_dither = "GradientBench(kRadial_GradType, gGradData[3], false)",
    GradientBench::new_dither(GradType::Radial, &g_grad_data(3), false)
);
def_bench!(
    gradient_bench_sweep_3_dither = "GradientBench(kSweep_GradType, gGradData[3], true)",
    GradientBench::new_dither(GradType::Sweep, &g_grad_data(3), true)
);
def_bench!(
    gradient_bench_sweep_3_no_dither = "GradientBench(kSweep_GradType, gGradData[3], false)",
    GradientBench::new_dither(GradType::Sweep, &g_grad_data(3), false)
);
def_bench!(
    gradient_bench_conical_3_dither = "GradientBench(kConical_GradType, gGradData[3], true)",
    GradientBench::new_dither(GradType::Conical, &g_grad_data(3), true)
);
def_bench!(
    gradient_bench_conical_3_no_dither = "GradientBench(kConical_GradType, gGradData[3], false)",
    GradientBench::new_dither(GradType::Conical, &g_grad_data(3), false)
);
// Port of: bench/GradientBench.cpp#L356-L356 (chrome/m156)
def_bench!(
    gradient_bench_gradient2_false = "Gradient2Bench(false)",
    Gradient2Bench::new(false)
);
// Port of: bench/GradientBench.cpp#L357-L357 (chrome/m156)
def_bench!(
    gradient_bench_gradient2_true = "Gradient2Bench(true)",
    Gradient2Bench::new(true)
);
