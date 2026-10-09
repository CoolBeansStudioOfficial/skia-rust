// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/PatchBench.cpp

//! `PatchBench`: `drawPatch` of a Coons patch (12 control points, 4 corner colors and 4 texture
//! coordinates) in four vertex modes, under four shapes: the default patch, `SquarePatchBench`,
//! `LODDiffPatchBench` and `LoopPatchBench` (rendering). `PatchUtilsBench` times
//! `SkPatchUtils::MakeVertices` (non-rendering).

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::scalar;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::patch_utils::{NUM_CORNERS, NUM_CTRL_PTS, make_vertices};
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};

use crate::def_bench;
use crate::prelude::*;

/// `PatchBench::VertexMode`.
// Port of: bench/PatchBench.cpp#L26-L31 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VertexMode {
    /// `kNone_VertexMode`.
    None,
    /// `kColors_VertexMode`.
    Colors,
    /// `kTexCoords_VertexMode`.
    TexCoords,
    /// `kBoth_VertexMode`.
    Both,
}

/// The patch classes: `PatchBench` itself and the three subclasses that override `setCubics`
/// and `appendName`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PatchKind {
    /// `PatchBench` (`appendName` "normal").
    Normal,
    /// `SquarePatchBench` ("square").
    Square,
    /// `LODDiffPatchBench` (`LOD_Diff`).
    LodDiff,
    /// `LoopPatchBench` ("loop").
    Loop,
}

impl PatchKind {
    /// `appendName`.
    // Port of: bench/PatchBench.cpp#L40-L44 (chrome/m156)
    fn append_name(self) -> &'static str {
        match self {
            PatchKind::Normal => "normal",
            PatchKind::Square => "square",
            PatchKind::LodDiff => "LOD_Diff",
            PatchKind::Loop => "loop",
        }
    }

    /// `setCubics()` of the class.
    fn cubics(self) -> [Point; NUM_CTRL_PTS] {
        // The control points are listed in the order of the C++ tables: top, right, bottom,
        // left.
        let pts: [(scalar, scalar); NUM_CTRL_PTS] = match self {
            // PatchBench::setCubics
            PatchKind::Normal => [
                (100.0, 100.0),
                (150.0, 50.0),
                (250.0, 150.0),
                (300.0, 100.0),
                (350.0, 150.0),
                (250.0, 200.0),
                (300.0, 300.0),
                (250.0, 250.0),
                (150.0, 350.0),
                (100.0, 300.0),
                (50.0, 250.0),
                (150.0, 50.0),
            ],
            // SquarePatchBench::setCubics
            PatchKind::Square => [
                (100.0, 100.0),
                (150.0, 100.0),
                (250.0, 100.0),
                (300.0, 100.0),
                (300.0, 150.0),
                (300.0, 250.0),
                (300.0, 300.0),
                (250.0, 300.0),
                (150.0, 300.0),
                (100.0, 300.0),
                (100.0, 250.0),
                (100.0, 150.0),
            ],
            // LODDiffPatchBench::setCubics
            PatchKind::LodDiff => [
                (100.0, 175.0),
                (150.0, 100.0),
                (250.0, 100.0),
                (300.0, 0.0),
                (300.0, 150.0),
                (300.0, 250.0),
                (300.0, 400.0),
                (250.0, 300.0),
                (150.0, 300.0),
                (100.0, 225.0),
                (100.0, 215.0),
                (100.0, 185.0),
            ],
            // LoopPatchBench::setCubics
            PatchKind::Loop => [
                (100.0, 100.0),
                (300.0, 200.0),
                (100.0, 200.0),
                (300.0, 100.0),
                (380.0, 400.0),
                (380.0, 0.0),
                (300.0, 300.0),
                (250.0, 250.0),
                (30.0, 200.0),
                (100.0, 300.0),
                (140.0, 325.0),
                (150.0, 150.0),
            ],
        };
        pts.map(|(x, y)| Point::new(x, y))
    }
}

/// `PatchBench::VertexMode` as the name of the mode, for `onGetName`.
// Port of: bench/PatchBench.cpp#L81-L96 (chrome/m156)
fn vertex_mode_name(mode: VertexMode) -> &'static str {
    match mode {
        VertexMode::None => "meshlines",
        VertexMode::Colors => "colors",
        VertexMode::TexCoords => "texs",
        VertexMode::Both => "colors_texs",
    }
}

/// `class PatchBench` and its subclasses.
// Port of: bench/PatchBench.cpp#L24-L157 (chrome/m156)
struct PatchBench {
    kind: PatchKind,
    /// `fScale`.
    scale: Point,
    /// `fVertexMode`.
    vertex_mode: VertexMode,
    /// `fName`.
    name: String,
    /// `fPaint`, set up in `onDelayedSetup`.
    paint: Paint,
    /// `fCubics`.
    cubics: [Point; NUM_CTRL_PTS],
    /// `fTexCoords`.
    tex_coords: [Point; NUM_CORNERS],
    /// `fColors`.
    colors: [Color; NUM_CORNERS],
}

impl PatchBench {
    fn new(kind: PatchKind, vertex_mode: VertexMode, scale: Point) -> Self {
        // fName.printf("patch_%s_%s_%fx%f", type, vertexMode, fScale.x(), fScale.y());
        let name = format!(
            "patch_{}_{}_{:.6}x{:.6}",
            kind.append_name(),
            vertex_mode_name(vertex_mode),
            scale.x,
            scale.y
        );
        Self {
            kind,
            scale,
            vertex_mode,
            name,
            paint: Paint::default(),
            cubics: [Point::new(0.0, 0.0); NUM_CTRL_PTS],
            tex_coords: [Point::new(0.0, 0.0); NUM_CORNERS],
            colors: [Color::new(0); NUM_CORNERS],
        }
    }

    /// `createShader()`: a linear gradient of seven colors, mirrored.
    // Port of: bench/PatchBench.cpp#L62-L71 (chrome/m156)
    fn create_shader() -> Option<skia_rust_core::shader::Shader> {
        let colors = [
            Color4f::new(1.0, 0.0, 0.0, 1.0),
            Color4f::new(0.0, 1.0, 1.0, 1.0),
            Color4f::new(0.0, 1.0, 0.0, 1.0),
            Color4f::new(1.0, 1.0, 1.0, 1.0),
            Color4f::new(1.0, 0.0, 1.0, 1.0),
            Color4f::new(0.0, 0.0, 1.0, 1.0),
            Color4f::new(1.0, 1.0, 0.0, 1.0),
        ];
        // const SkPoint pts[] = { { 200.f / 4.f, 0.f }, { 3.f * 200.f / 4, 200.f } };
        let pts = (
            Point::new(200.0 / 4.0, 0.0),
            Point::new(3.0 * 200.0 / 4.0, 200.0),
        );
        gradient_shaders::linear_gradient(
            pts,
            &Gradient::new(
                Colors::new(&colors, None, TileMode::Mirror, None),
                Interpolation::default(),
            ),
            None,
        )
    }
}

impl Benchmark for PatchBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/PatchBench.cpp#L98-L115 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // this->setCubics(); this->setColors(); this->setTexCoords();
        self.cubics = self.kind.cubics();
        // SK_ColorRED, SK_ColorGREEN, SK_ColorBLUE, SK_ColorCYAN
        self.colors = [
            Color::new(0xFFFF_0000),
            Color::new(0xFF00_FF00),
            Color::new(0xFF00_00FF),
            Color::new(0xFF00_FFFF),
        ];
        self.tex_coords = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        // this->setupPaint(&fPaint);
        let mut paint = Paint::default();
        self.setup_paint(&mut paint);
        match self.vertex_mode {
            // fPaint.setShader(this->createShader());
            VertexMode::TexCoords | VertexMode::Both => {
                paint.set_shader(Self::create_shader());
            }
            // fPaint.setShader(nullptr);
            VertexMode::None | VertexMode::Colors => {
                paint.set_shader(None);
            }
        }
        self.paint = paint;
    }

    // Port of: bench/PatchBench.cpp#L117-L139 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("PatchBench is a rendering bench");
        // canvas->scale(fScale.x(), fScale.y());
        canvas.scale((self.scale.x, self.scale.y));
        let colors: Option<&[Color; NUM_CORNERS]> = Some(&self.colors);
        let tex_coords: Option<&[Point; NUM_CORNERS]> = Some(&self.tex_coords);
        for _ in 0..loops {
            // canvas->drawPatch(fCubics, colors?, texs?, SkBlendMode::kModulate, fPaint);
            match self.vertex_mode {
                VertexMode::None => {
                    canvas.draw_patch(&self.cubics, None, None, BlendMode::Modulate, &self.paint);
                }
                VertexMode::Colors => {
                    canvas.draw_patch(&self.cubics, colors, None, BlendMode::Modulate, &self.paint);
                }
                VertexMode::TexCoords => {
                    canvas.draw_patch(
                        &self.cubics,
                        None,
                        tex_coords,
                        BlendMode::Modulate,
                        &self.paint,
                    );
                }
                VertexMode::Both => {
                    canvas.draw_patch(
                        &self.cubics,
                        colors,
                        tex_coords,
                        BlendMode::Modulate,
                        &self.paint,
                    );
                }
            }
        }
    }
}

/// `class PatchUtilsBench`.
// Port of: bench/PatchBench.cpp#L330-L359 (chrome/m156)
struct PatchUtilsBench {
    /// `fName`.
    name: String,
    /// `fLinearInterp`.
    linear_interp: bool,
}

impl PatchUtilsBench {
    // Port of: bench/PatchBench.cpp#L334-L337 (chrome/m156)
    fn new(linear_interp: bool) -> Self {
        // fName.printf("patchutils_%s", linearInterp ? "linear" : "legacy");
        Self {
            name: format!(
                "patchutils_{}",
                if linear_interp { "linear" } else { "legacy" }
            ),
            linear_interp,
        }
    }
}

impl Benchmark for PatchUtilsBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/PatchBench.cpp#L340-L342 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PatchBench.cpp#L343-L359 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // const SkColor colors[] = { 0xFF000000, 0xFF00FF00, 0xFF0000FF, 0xFFFF0000 };
        let colors = [
            Color::new(0xFF00_0000),
            Color::new(0xFF00_FF00),
            Color::new(0xFF00_00FF),
            Color::new(0xFFFF_0000),
        ];
        // const SkPoint pts[] = { ... 12 points ... };
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(30.0, 0.0),
            Point::new(30.0, 10.0),
            Point::new(30.0, 20.0),
            Point::new(30.0, 30.0),
            Point::new(20.0, 30.0),
            Point::new(10.0, 30.0),
            Point::new(0.0, 30.0),
            Point::new(0.0, 20.0),
            Point::new(0.0, 10.0),
        ];
        // const SkPoint tex[] = { { 0, 0 }, { 10, 0 }, { 10, 10 }, { 0, 10 } };
        let tex = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        // auto cs = fLinearInterp ? SkColorSpace::MakeSRGBLinear() : nullptr;
        let cs = self.linear_interp.then(ColorSpace::new_srgb_linear);
        for _ in 0..100 * loops {
            // SkPatchUtils::MakeVertices(pts, colors, tex, 20, 20, cs.get());
            let _ = make_vertices(&pts, Some(&colors), Some(&tex), 20, 20, cs.as_ref());
        }
    }
}

// The registrations, generated from the manifest ids (their names are copied verbatim).
def_bench!(
    patch_loddiff_0_1_0_1_both =
        "LODDiffPatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kBoth_VertexMode)",
    PatchBench::new(PatchKind::LodDiff, VertexMode::Both, Point::new(0.1, 0.1))
);
def_bench!(
    patch_loddiff_0_1_0_1_colors =
        "LODDiffPatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kColors_VertexMode)",
    PatchBench::new(PatchKind::LodDiff, VertexMode::Colors, Point::new(0.1, 0.1))
);
def_bench!(
    patch_loddiff_0_1_0_1_none =
        "LODDiffPatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kNone_VertexMode)",
    PatchBench::new(PatchKind::LodDiff, VertexMode::None, Point::new(0.1, 0.1))
);
def_bench!(
    patch_loddiff_0_1_0_1_texcoords =
        "LODDiffPatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kTexCoords_VertexMode)",
    PatchBench::new(
        PatchKind::LodDiff,
        VertexMode::TexCoords,
        Point::new(0.1, 0.1)
    )
);
def_bench!(
    patch_loddiff_1_0_1_0_both =
        "LODDiffPatchBench(SkVector::Make(1.0f, 1.0f), PatchBench::kBoth_VertexMode)",
    PatchBench::new(PatchKind::LodDiff, VertexMode::Both, Point::new(1.0, 1.0))
);
def_bench!(
    patch_loddiff_1_0_1_0_colors =
        "LODDiffPatchBench(SkVector::Make(1.0f, 1.0f), PatchBench::kColors_VertexMode)",
    PatchBench::new(PatchKind::LodDiff, VertexMode::Colors, Point::new(1.0, 1.0))
);
def_bench!(
    patch_loddiff_1_0_1_0_texcoords =
        "LODDiffPatchBench(SkVector::Make(1.0f, 1.0f), PatchBench::kTexCoords_VertexMode)",
    PatchBench::new(
        PatchKind::LodDiff,
        VertexMode::TexCoords,
        Point::new(1.0, 1.0)
    )
);
def_bench!(
    patch_loddiff_1_0_1_0_none =
        "LODDiffPatchBench(SkVector::Make(1.f, 1.0f), PatchBench::kNone_VertexMode)",
    PatchBench::new(PatchKind::LodDiff, VertexMode::None, Point::new(1.0, 1.0))
);
def_bench!(
    patch_loddiff_3_0_3_0_both =
        "LODDiffPatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kBoth_VertexMode)",
    PatchBench::new(PatchKind::LodDiff, VertexMode::Both, Point::new(3.0, 3.0))
);
def_bench!(
    patch_loddiff_3_0_3_0_colors =
        "LODDiffPatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kColors_VertexMode)",
    PatchBench::new(PatchKind::LodDiff, VertexMode::Colors, Point::new(3.0, 3.0))
);
def_bench!(
    patch_loddiff_3_0_3_0_none =
        "LODDiffPatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kNone_VertexMode)",
    PatchBench::new(PatchKind::LodDiff, VertexMode::None, Point::new(3.0, 3.0))
);
def_bench!(
    patch_loddiff_3_0_3_0_texcoords =
        "LODDiffPatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kTexCoords_VertexMode)",
    PatchBench::new(
        PatchKind::LodDiff,
        VertexMode::TexCoords,
        Point::new(3.0, 3.0)
    )
);
def_bench!(
    patch_loop_0_1_0_1_both =
        "LoopPatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kBoth_VertexMode)",
    PatchBench::new(PatchKind::Loop, VertexMode::Both, Point::new(0.1, 0.1))
);
def_bench!(
    patch_loop_0_1_0_1_colors =
        "LoopPatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kColors_VertexMode)",
    PatchBench::new(PatchKind::Loop, VertexMode::Colors, Point::new(0.1, 0.1))
);
def_bench!(
    patch_loop_0_1_0_1_none =
        "LoopPatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kNone_VertexMode)",
    PatchBench::new(PatchKind::Loop, VertexMode::None, Point::new(0.1, 0.1))
);
def_bench!(
    patch_loop_0_1_0_1_texcoords =
        "LoopPatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kTexCoords_VertexMode)",
    PatchBench::new(PatchKind::Loop, VertexMode::TexCoords, Point::new(0.1, 0.1))
);
def_bench!(
    patch_loop_1_0_1_0_both =
        "LoopPatchBench(SkVector::Make(1.0f, 1.0f), PatchBench::kBoth_VertexMode)",
    PatchBench::new(PatchKind::Loop, VertexMode::Both, Point::new(1.0, 1.0))
);
def_bench!(
    patch_loop_1_0_1_0_colors =
        "LoopPatchBench(SkVector::Make(1.0f, 1.0f), PatchBench::kColors_VertexMode)",
    PatchBench::new(PatchKind::Loop, VertexMode::Colors, Point::new(1.0, 1.0))
);
def_bench!(
    patch_loop_1_0_1_0_texcoords =
        "LoopPatchBench(SkVector::Make(1.0f, 1.0f), PatchBench::kTexCoords_VertexMode)",
    PatchBench::new(PatchKind::Loop, VertexMode::TexCoords, Point::new(1.0, 1.0))
);
def_bench!(
    patch_loop_1_0_1_0_none =
        "LoopPatchBench(SkVector::Make(1.f, 1.0f), PatchBench::kNone_VertexMode)",
    PatchBench::new(PatchKind::Loop, VertexMode::None, Point::new(1.0, 1.0))
);
def_bench!(
    patch_loop_3_0_3_0_both =
        "LoopPatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kBoth_VertexMode)",
    PatchBench::new(PatchKind::Loop, VertexMode::Both, Point::new(3.0, 3.0))
);
def_bench!(
    patch_loop_3_0_3_0_colors =
        "LoopPatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kColors_VertexMode)",
    PatchBench::new(PatchKind::Loop, VertexMode::Colors, Point::new(3.0, 3.0))
);
def_bench!(
    patch_loop_3_0_3_0_none =
        "LoopPatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kNone_VertexMode)",
    PatchBench::new(PatchKind::Loop, VertexMode::None, Point::new(3.0, 3.0))
);
def_bench!(
    patch_loop_3_0_3_0_texcoords =
        "LoopPatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kTexCoords_VertexMode)",
    PatchBench::new(PatchKind::Loop, VertexMode::TexCoords, Point::new(3.0, 3.0))
);
def_bench!(
    patch_normal_0_1_0_1_both =
        "PatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kBoth_VertexMode)",
    PatchBench::new(PatchKind::Normal, VertexMode::Both, Point::new(0.1, 0.1))
);
def_bench!(
    patch_normal_0_1_0_1_colors =
        "PatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kColors_VertexMode)",
    PatchBench::new(PatchKind::Normal, VertexMode::Colors, Point::new(0.1, 0.1))
);
def_bench!(
    patch_normal_0_1_0_1_none =
        "PatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kNone_VertexMode)",
    PatchBench::new(PatchKind::Normal, VertexMode::None, Point::new(0.1, 0.1))
);
def_bench!(
    patch_normal_0_1_0_1_texcoords =
        "PatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kTexCoords_VertexMode)",
    PatchBench::new(
        PatchKind::Normal,
        VertexMode::TexCoords,
        Point::new(0.1, 0.1)
    )
);
def_bench!(
    patch_normal_1_0_1_0_both =
        "PatchBench(SkVector::Make(1.0f, 1.0f), PatchBench::kBoth_VertexMode)",
    PatchBench::new(PatchKind::Normal, VertexMode::Both, Point::new(1.0, 1.0))
);
def_bench!(
    patch_normal_1_0_1_0_colors =
        "PatchBench(SkVector::Make(1.0f, 1.0f), PatchBench::kColors_VertexMode)",
    PatchBench::new(PatchKind::Normal, VertexMode::Colors, Point::new(1.0, 1.0))
);
def_bench!(
    patch_normal_1_0_1_0_texcoords =
        "PatchBench(SkVector::Make(1.0f, 1.0f), PatchBench::kTexCoords_VertexMode)",
    PatchBench::new(
        PatchKind::Normal,
        VertexMode::TexCoords,
        Point::new(1.0, 1.0)
    )
);
def_bench!(
    patch_normal_1_0_1_0_none =
        "PatchBench(SkVector::Make(1.f, 1.0f), PatchBench::kNone_VertexMode)",
    PatchBench::new(PatchKind::Normal, VertexMode::None, Point::new(1.0, 1.0))
);
def_bench!(
    patch_normal_3_0_3_0_both =
        "PatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kBoth_VertexMode)",
    PatchBench::new(PatchKind::Normal, VertexMode::Both, Point::new(3.0, 3.0))
);
def_bench!(
    patch_normal_3_0_3_0_colors =
        "PatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kColors_VertexMode)",
    PatchBench::new(PatchKind::Normal, VertexMode::Colors, Point::new(3.0, 3.0))
);
def_bench!(
    patch_normal_3_0_3_0_none =
        "PatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kNone_VertexMode)",
    PatchBench::new(PatchKind::Normal, VertexMode::None, Point::new(3.0, 3.0))
);
def_bench!(
    patch_normal_3_0_3_0_texcoords =
        "PatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kTexCoords_VertexMode)",
    PatchBench::new(
        PatchKind::Normal,
        VertexMode::TexCoords,
        Point::new(3.0, 3.0)
    )
);
def_bench!(
    patch_utils_legacy = "PatchUtilsBench(false)",
    PatchUtilsBench::new(false)
);
def_bench!(
    patch_utils_linear = "PatchUtilsBench(true)",
    PatchUtilsBench::new(true)
);
def_bench!(
    patch_square_0_1_0_1_both =
        "SquarePatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kBoth_VertexMode)",
    PatchBench::new(PatchKind::Square, VertexMode::Both, Point::new(0.1, 0.1))
);
def_bench!(
    patch_square_0_1_0_1_colors =
        "SquarePatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kColors_VertexMode)",
    PatchBench::new(PatchKind::Square, VertexMode::Colors, Point::new(0.1, 0.1))
);
def_bench!(
    patch_square_0_1_0_1_none =
        "SquarePatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kNone_VertexMode)",
    PatchBench::new(PatchKind::Square, VertexMode::None, Point::new(0.1, 0.1))
);
def_bench!(
    patch_square_0_1_0_1_texcoords =
        "SquarePatchBench(SkVector::Make(0.1f, 0.1f), PatchBench::kTexCoords_VertexMode)",
    PatchBench::new(
        PatchKind::Square,
        VertexMode::TexCoords,
        Point::new(0.1, 0.1)
    )
);
def_bench!(
    patch_square_1_0_1_0_both =
        "SquarePatchBench(SkVector::Make(1.0f, 1.0f), PatchBench::kBoth_VertexMode)",
    PatchBench::new(PatchKind::Square, VertexMode::Both, Point::new(1.0, 1.0))
);
def_bench!(
    patch_square_1_0_1_0_colors =
        "SquarePatchBench(SkVector::Make(1.0f, 1.0f), PatchBench::kColors_VertexMode)",
    PatchBench::new(PatchKind::Square, VertexMode::Colors, Point::new(1.0, 1.0))
);
def_bench!(
    patch_square_1_0_1_0_texcoords =
        "SquarePatchBench(SkVector::Make(1.0f, 1.0f), PatchBench::kTexCoords_VertexMode)",
    PatchBench::new(
        PatchKind::Square,
        VertexMode::TexCoords,
        Point::new(1.0, 1.0)
    )
);
def_bench!(
    patch_square_1_0_1_0_none =
        "SquarePatchBench(SkVector::Make(1.f, 1.0f), PatchBench::kNone_VertexMode)",
    PatchBench::new(PatchKind::Square, VertexMode::None, Point::new(1.0, 1.0))
);
def_bench!(
    patch_square_3_0_3_0_both =
        "SquarePatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kBoth_VertexMode)",
    PatchBench::new(PatchKind::Square, VertexMode::Both, Point::new(3.0, 3.0))
);
def_bench!(
    patch_square_3_0_3_0_colors =
        "SquarePatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kColors_VertexMode)",
    PatchBench::new(PatchKind::Square, VertexMode::Colors, Point::new(3.0, 3.0))
);
def_bench!(
    patch_square_3_0_3_0_none =
        "SquarePatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kNone_VertexMode)",
    PatchBench::new(PatchKind::Square, VertexMode::None, Point::new(3.0, 3.0))
);
def_bench!(
    patch_square_3_0_3_0_texcoords =
        "SquarePatchBench(SkVector::Make(3.0f, 3.0f), PatchBench::kTexCoords_VertexMode)",
    PatchBench::new(
        PatchKind::Square,
        VertexMode::TexCoords,
        Point::new(3.0, 3.0)
    )
);
