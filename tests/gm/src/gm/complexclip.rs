// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/complexclip.cpp (chrome/m156)

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::items_after_statements,
    clippy::fn_params_excessive_bools,
    clippy::struct_excessive_bools,
    clippy::unused_self
)]

use crate::prelude::*;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image::Image;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::RoundOut;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_tools::font_tool_utils::default_portable_typeface;

use crate::tool_utils::{get_resource_as_image, int_to_scalar};

// Port of: gm/complexclip.cpp#L14-L16 (chrome/m156)
const G_PATH_COLOR: Color = Color::BLACK;
// Port of: gm/complexclip.cpp#L14-L16 (chrome/m156)
const G_CLIP_A_COLOR: Color = Color::BLUE;
// Port of: gm/complexclip.cpp#L14-L16 (chrome/m156)
const G_CLIP_B_COLOR: Color = Color::RED;

// Port of: gm/complexclip.cpp#L18-L187 (chrome/m156)
struct ComplexClipGm {
    do_aa_clip: bool,
    do_save_layer: bool,
    invert_draw: bool,
}

impl ComplexClipGm {
    // Port of: gm/complexclip.cpp#L24-L30 (chrome/m156)
    fn new(aaclip: bool, save_layer: bool, invert_draw: bool) -> Self {
        Self {
            do_aa_clip: aaclip,
            do_save_layer: save_layer,
            invert_draw,
        }
    }

    // Port of: gm/complexclip.cpp#L168-L184 (chrome/m156)
    fn draw_hairlines(&self, canvas: &Canvas, path: &Path, clip_a: &Path, clip_b: &Path) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_style(Style::Stroke);
        let fade: u8 = 0x33;
        paint.set_color(G_PATH_COLOR);
        paint.set_alpha(fade);
        canvas.draw_path(path, &paint);
        paint.set_color(G_CLIP_A_COLOR);
        paint.set_alpha(fade);
        canvas.draw_path(clip_a, &paint);
        paint.set_color(G_CLIP_B_COLOR);
        paint.set_alpha(fade);
        canvas.draw_path(clip_b, &paint);
    }
}

impl GM for ComplexClipGm {
    fn name(&self) -> String {
        format!(
            "complexclip_{}{}{}",
            if self.do_aa_clip { "aa" } else { "bw" },
            if self.do_save_layer { "_layer" } else { "" },
            if self.invert_draw { "_invert" } else { "" },
        )
    }

    fn size(&mut self) -> ISize {
        ISize::new(388, 780)
    }

    fn bg_color(&self) -> Color {
        Color::new(0xFFDEDFDE)
    }

    // Port of: gm/complexclip.cpp#L36-L144 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut path = PathBuilder::new()
            .move_to((0.0, 50.0))
            .quad_to((0.0, 0.0), (50.0, 0.0))
            .line_to((175.0, 0.0))
            .quad_to((200.0, 0.0), (200.0, 25.0))
            .line_to((200.0, 150.0))
            .quad_to((200.0, 200.0), (150.0, 200.0))
            .line_to((0.0, 200.0))
            .close()
            .move_to((50.0, 50.0))
            .line_to((150.0, 50.0))
            .line_to((150.0, 125.0))
            .quad_to((150.0, 150.0), (125.0, 150.0))
            .line_to((50.0, 150.0))
            .close()
            .detach();
        if self.invert_draw {
            path.set_fill_type(PathFillType::InverseEvenOdd);
        } else {
            path.set_fill_type(PathFillType::EvenOdd);
        }

        let mut path_paint = Paint::default();
        path_paint.set_anti_alias(true);
        path_paint.set_color(G_PATH_COLOR);

        let mut clip_a = Path::polygon(
            &[
                (10.0, 20.0).into(),
                (165.0, 22.0).into(),
                (70.0, 105.0).into(),
                (165.0, 177.0).into(),
                (-5.0, 180.0).into(),
            ],
            true,
            None,
            None,
        );
        let mut clip_b = Path::polygon(
            &[
                (40.0, 10.0).into(),
                (190.0, 15.0).into(),
                (195.0, 190.0).into(),
                (40.0, 185.0).into(),
                (155.0, 100.0).into(),
            ],
            true,
            None,
            None,
        );

        let font = Font::from_size(default_portable_typeface(), 20.0);
        // extra spaces in names for measureText
        let ops: [(ClipOp, &str); 2] =
            [(ClipOp::Intersect, "Isect "), (ClipOp::Difference, "Diff ")];

        canvas.translate((20.0, 20.0));
        canvas.scale((3.0 / 4.0, 3.0 / 4.0));
        if self.do_save_layer {
            let mut bounds = Rect::from_ltrb(
                4.0_f32 / 3.0 * -20.0,
                4.0_f32 / 3.0 * -20.0,
                4.0_f32 / 3.0 * (388 - 20) as f32,
                4.0_f32 / 3.0 * (780 - 20) as f32,
            );
            bounds.inset((100.0, 100.0));
            let mut bound_paint = Paint::default();
            bound_paint.set_color(Color::RED);
            bound_paint.set_style(Style::Stroke);
            canvas.draw_rect(bounds, &bound_paint);
            canvas.clip_rect(bounds, None, None);
            canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));
        }
        for inv_bits in 0..4 {
            canvas.save();
            for (op, op_name) in ops {
                self.draw_hairlines(canvas, &path, &clip_a, &clip_b);
                let do_inv_a = (inv_bits & 1) != 0;
                let do_inv_b = (inv_bits & 2) != 0;
                canvas.save();
                clip_a.set_fill_type(if do_inv_a {
                    PathFillType::InverseEvenOdd
                } else {
                    PathFillType::EvenOdd
                });
                clip_b.set_fill_type(if do_inv_b {
                    PathFillType::InverseEvenOdd
                } else {
                    PathFillType::EvenOdd
                });
                canvas.clip_path(&clip_a, None, self.do_aa_clip);
                canvas.clip_path(&clip_b, op, self.do_aa_clip);
                if self.invert_draw {
                    let mut rect_clip = *clip_a.bounds();
                    rect_clip.join(path.bounds());
                    rect_clip.join(path.bounds());
                    rect_clip.outset((5.0, 5.0));
                    canvas.clip_rect(rect_clip, None, None);
                }
                canvas.draw_path(&path, &path_paint);
                canvas.restore();

                let mut paint = Paint::default();
                let mut txt_x: f32 = 45.0;
                paint.set_color(G_CLIP_A_COLOR);
                let a_txt = if do_inv_a { "InvA " } else { "A " };
                canvas.draw_simple_text(a_txt, TextEncoding::UTF8, (txt_x, 220.0), &font, &paint);
                txt_x += font
                    .measure_text(a_txt.as_bytes(), TextEncoding::UTF8, None)
                    .0;
                paint.set_color(Color::BLACK);
                canvas.draw_simple_text(op_name, TextEncoding::UTF8, (txt_x, 220.0), &font, &paint);
                txt_x += font
                    .measure_text(op_name.as_bytes(), TextEncoding::UTF8, None)
                    .0;
                paint.set_color(G_CLIP_B_COLOR);
                let b_txt = if do_inv_b { "InvB " } else { "B " };
                canvas.draw_simple_text(b_txt, TextEncoding::UTF8, (txt_x, 220.0), &font, &paint);
                canvas.translate((250.0, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, 250.0));
        }
        if self.do_save_layer {
            canvas.restore();
        }
    }
}

// Port of: gm/complexclip.cpp#L207-L214 (chrome/m156)
crate::def_gm!(
    ComplexClipGM_false_false_false = "ComplexClipGM(false, false, false)",
    ComplexClipGm::new(false, false, false)
);
crate::def_gm!(
    ComplexClipGM_false_false_true = "ComplexClipGM(false, false, true)",
    ComplexClipGm::new(false, false, true)
);
crate::def_gm!(
    ComplexClipGM_false_true_false = "ComplexClipGM(false, true, false)",
    ComplexClipGm::new(false, true, false)
);
crate::def_gm!(
    ComplexClipGM_false_true_true = "ComplexClipGM(false, true, true)",
    ComplexClipGm::new(false, true, true)
);
crate::def_gm!(
    ComplexClipGM_true_false_false = "ComplexClipGM(true, false, false)",
    ComplexClipGm::new(true, false, false)
);
crate::def_gm!(
    ComplexClipGM_true_false_true = "ComplexClipGM(true, false, true)",
    ComplexClipGm::new(true, false, true)
);
crate::def_gm!(
    ComplexClipGM_true_true_false = "ComplexClipGM(true, true, false)",
    ComplexClipGm::new(true, true, false)
);
crate::def_gm!(
    ComplexClipGM_true_true_true = "ComplexClipGM(true, true, true)",
    ComplexClipGm::new(true, true, true)
);

// Port of: gm/complexclip.cpp#L217-L252 (chrome/m156), clip_shader
crate::def_simple_gm!(clip_shader, canvas, 840, 650, {
    let img = crate::tool_utils::get_resource_as_image("images/yellow_rose.png")
        .expect("images/yellow_rose.png (set SKIA_RESOURCES)");
    let sh = img
        .to_shader(None, SamplingOptions::default(), None)
        .expect("shader");

    let r = Rect::from_isize(img.dimensions());
    let mut p = Paint::default();

    canvas.translate((10.0, 10.0));
    canvas.draw_image(&img, (0.0, 0.0), None);

    canvas.save();
    canvas.translate((int_to_scalar(img.width()) + 10.0, 0.0));
    canvas.clip_shader(sh.clone(), ClipOp::Intersect);
    p.set_color(Color::RED);
    canvas.draw_rect(r, &p);
    canvas.restore();

    canvas.save();
    canvas.translate((0.0, int_to_scalar(img.height()) + 10.0));
    canvas.clip_shader(sh.clone(), ClipOp::Difference);
    p.set_color(Color::GREEN);
    canvas.draw_rect(r, &p);
    canvas.restore();

    canvas.save();
    canvas.translate((
        int_to_scalar(img.width()) + 10.0,
        int_to_scalar(img.height()) + 10.0,
    ));
    canvas.clip_shader(sh, ClipOp::Intersect);
    canvas.save();
    let lm = Matrix::scale((1.0 / 5.0, 1.0 / 5.0));
    canvas.clip_shader(
        img.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            SamplingOptions::default(),
            &lm,
        )
        .expect("shader"),
        None,
    );
    canvas.draw_image(&img, (0.0, 0.0), None);

    canvas.restore();
    canvas.restore();
});

// Port of: gm/complexclip.cpp#L254-L268 (chrome/m156)
crate::def_simple_gm!(clip_shader_layer, canvas, 430, 320, {
    let img = crate::tool_utils::get_resource_as_image("images/yellow_rose.png")
        .expect("images/yellow_rose.png (set SKIA_RESOURCES)");
    let sh = img
        .to_shader(
            None,
            skia_rust_core::sampling_options::SamplingOptions::default(),
            None,
        )
        .expect("shader");

    let r = Rect::from_wh(
        crate::tool_utils::int_to_scalar(img.width()),
        crate::tool_utils::int_to_scalar(img.height()),
    );

    canvas.translate((10.0, 10.0));
    // now add the cool clip
    canvas.clip_rect(r, None, None);
    canvas.clip_shader(sh, None);
    // now draw a layer with the same image, and watch it get restored w/ the clip
    canvas.save_layer(&SaveLayerRec::default().bounds(&r));
    canvas.draw_color(Color::new(0xFFFF_0000), None);
    canvas.restore();
});

// Port of: gm/complexclip.cpp#L270-L332 (chrome/m156), clip_shader_nested
crate::def_simple_gm!(clip_shader_nested, canvas, 256, 256, {
    let w: f32 = 64.0;
    let h: f32 = 64.0;

    // SkColorConverter conv({ SK_ColorBLACK, SkColorSetARGB(128, 128, 128, 128) })
    let conv = [
        Color4f::from_color(Color::BLACK),
        Color4f::from_color(Color::new(0x8080_8080)),
    ];
    let s = shaders::radial_gradient(
        (Point::new(0.5 * w, 0.5 * h), 0.1 * w),
        &Gradient::new(
            Colors::new(&conv, None, TileMode::Repeat, None),
            Interpolation::default(),
        ),
        None,
    )
    .expect("radial gradient");

    let mut p = Paint::default();

    // A large black rect affected by two gradient clips
    canvas.save();
    canvas.clip_shader(s.clone(), None);
    canvas.scale((2.0, 2.0));
    canvas.clip_shader(s.clone(), None);
    canvas.draw_rect(Rect::from_wh(w, h), &p);
    canvas.restore();

    canvas.translate((0.0, 2.0 * h));

    // A small red rect, with no clipping
    canvas.save();
    p.set_color(Color::RED);
    canvas.draw_rect(Rect::from_wh(w, h), &p);
    canvas.restore();

    canvas.translate((2.0 * w, -2.0 * h));

    // A small green rect, with clip shader and rrect clipping
    canvas.save();
    canvas.clip_shader(s.clone(), None);
    canvas.clip_rrect(
        RRect::new_rect_xy(Rect::from_wh(w, h), 10.0, 10.0),
        None,
        true,
    );
    p.set_color(Color::GREEN);
    canvas.draw_rect(Rect::from_wh(w, h), &p);
    canvas.restore();

    canvas.translate((0.0, 2.0 * h));

    // A small blue rect, with clip shader and path clipping
    let star_path = PathBuilder::new()
        .move_to((0.0, -33.3333))
        .line_to((9.62, -16.6667))
        .line_to((28.867, -16.6667))
        .line_to((19.24, 0.0))
        .line_to((28.867, 16.6667))
        .line_to((9.62, 16.6667))
        .line_to((0.0, 33.3333))
        .line_to((-9.62, 16.6667))
        .line_to((-28.867, 16.6667))
        .line_to((-19.24, 0.0))
        .line_to((-28.867, -16.6667))
        .line_to((-9.62, -16.6667))
        .close()
        .detach();

    canvas.save();
    canvas.clip_shader(s, None);
    canvas.translate((w / 2.0, h / 2.0));
    canvas.clip_path(&star_path, None, None);
    p.set_color(Color::BLUE);
    canvas.translate((-w / 2.0, -h / 2.0));
    canvas.draw_rect(Rect::from_wh(w, h), &p);
    canvas.restore();
});

// Where is canvas->concat(persp) called relative to the clipShader calls.
// Port of: gm/complexclip.cpp#L334-L340 (chrome/m156), ConcatPerspective
// The variant names mirror the C++ enumerators (kConcatBeforeClips, ...), which all end in "Clips".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
enum ConcatPerspective {
    BeforeClips,
    AfterClips,
    BetweenClips,
}

// Order in which clipShader(image) and clipShader(gradient) are specified; only meaningful
// when CanvasPerspective is kConcatBetweenClips.
// Port of: gm/complexclip.cpp#L341-L348 (chrome/m156), ClipOrder
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClipOrder {
    ImageFirst,
    GradientFirst,
}

// Which shaders have perspective applied as a local matrix.
// Port of: gm/complexclip.cpp#L349-L355 (chrome/m156), LocalMatrix
// The variant names mirror the C++ enumerators (kNoLocalMat, ...), which all end in "LocalMat".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
enum LocalMatrix {
    NoLocalMat,
    ImageWithLocalMat,
    GradientWithLocalMat,
    BothWithLocalMat,
}

// Port of: gm/complexclip.cpp#L356-L360 (chrome/m156), Config
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PerspConfig {
    concat: ConcatPerspective,
    order: ClipOrder,
    lm: LocalMatrix,
}

// Port of: gm/complexclip.cpp#L363-L384 (chrome/m156), draw_banner
fn draw_banner(canvas: &Canvas, config: PerspConfig) {
    let mut banner = String::from("Persp: ");

    if config.concat == ConcatPerspective::BeforeClips || config.lm == LocalMatrix::BothWithLocalMat
    {
        banner.push_str("Both Clips");
    } else if (config.concat == ConcatPerspective::BetweenClips
        && config.order == ClipOrder::ImageFirst)
        || config.lm == LocalMatrix::GradientWithLocalMat
    {
        banner.push_str("Gradient");
    } else {
        banner.push_str("Image");
    }
    if config.lm != LocalMatrix::NoLocalMat {
        banner.push_str(" (w/ LM, should equal top row)");
    }

    let font = Font::from_size(default_portable_typeface(), 12.0);
    canvas.draw_str(&banner, (20.0, -30.0), &font, &Paint::default());
}

// Port of: gm/complexclip.cpp#L386-L390 (chrome/m156), the drawConfig lambda of clip_shader_persp
fn draw_persp_config(
    canvas: &Canvas,
    img: &Image,
    persp: &Matrix,
    scale: &Matrix,
    config: PerspConfig,
) {
    canvas.save();

    draw_banner(canvas, config);

    // Make clipShaders (possibly with local matrices)
    let grad_lm = config.lm == LocalMatrix::GradientWithLocalMat
        || config.lm == LocalMatrix::BothWithLocalMat;
    let conv = [
        Color4f::from_color(Color::BLACK),
        Color4f::from_color(Color::new(0x8080_8080)),
    ];
    let img_w = int_to_scalar(img.width());
    let img_h = int_to_scalar(img.height());
    let grad_shader = shaders::radial_gradient(
        (Point::new(0.5 * img_w, 0.5 * img_h), 0.1 * img_w),
        &Gradient::new(
            Colors::new(&conv, None, TileMode::Repeat, None),
            Interpolation::default(),
        ),
        if grad_lm { Some(persp) } else { None },
    )
    .expect("radial gradient");
    let image_lm =
        config.lm == LocalMatrix::ImageWithLocalMat || config.lm == LocalMatrix::BothWithLocalMat;
    let perspective_scale = Matrix::concat(persp, scale);
    let img_shader = img
        .to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            SamplingOptions::default(),
            if image_lm { &perspective_scale } else { scale },
        )
        .expect("image shader");

    // Perspective before any clipShader
    if config.concat == ConcatPerspective::BeforeClips {
        canvas.concat(persp);
    }

    // First clipshader
    let (first, second) = if config.order == ClipOrder::ImageFirst {
        (img_shader.clone(), grad_shader.clone())
    } else {
        (grad_shader.clone(), img_shader.clone())
    };
    canvas.clip_shader(first, None);

    // Perspective between clipShader
    if config.concat == ConcatPerspective::BetweenClips {
        canvas.concat(persp);
    }

    // Second clipShader
    canvas.clip_shader(second, None);

    // Perspective after clipShader
    if config.concat == ConcatPerspective::AfterClips {
        canvas.concat(persp);
    }

    // Actual draw and clip boundary are the same for all configs
    canvas.clip_irect(img.bounds(), None);
    canvas.clear(Color::BLACK);
    canvas.draw_image(img, (0.0, 0.0), None);

    canvas.restore();
}

// Port of: gm/complexclip.cpp#L391-L484 (chrome/m156), clip_shader_persp
crate::def_simple_gm!(clip_shader_persp, canvas, 1370, 1030, {
    use ClipOrder::{GradientFirst, ImageFirst};
    use ConcatPerspective::{AfterClips, BeforeClips, BetweenClips};
    use LocalMatrix::{BothWithLocalMat, GradientWithLocalMat, ImageWithLocalMat, NoLocalMat};

    // Pairs of configs that should match in appearance where first config doesn't use a local
    // matrix (top row of GM) and the second does (bottom row of GM).
    let matches = [
        // Everything has perspective
        [
            PerspConfig {
                concat: BeforeClips,
                order: ImageFirst,
                lm: NoLocalMat,
            },
            PerspConfig {
                concat: AfterClips,
                order: ImageFirst,
                lm: BothWithLocalMat,
            },
        ],
        // Image shader has perspective
        [
            PerspConfig {
                concat: BetweenClips,
                order: GradientFirst,
                lm: NoLocalMat,
            },
            PerspConfig {
                concat: AfterClips,
                order: ImageFirst,
                lm: ImageWithLocalMat,
            },
        ],
        // Gradient shader has perspective
        [
            PerspConfig {
                concat: BetweenClips,
                order: ImageFirst,
                lm: NoLocalMat,
            },
            PerspConfig {
                concat: AfterClips,
                order: ImageFirst,
                lm: GradientWithLocalMat,
            },
        ],
    ];

    // The image that is drawn
    let img = crate::tool_utils::get_resource_as_image("images/yellow_rose.png")
        .expect("images/yellow_rose.png (set SKIA_RESOURCES)");
    // Scale factor always applied to the image shader so that it tiles
    let scale = Matrix::scale((1.0 / 4.0, 1.0 / 4.0));
    // The perspective matrix applied wherever needed
    let src = Rect::from_isize(img.dimensions()).to_quad(None);
    let iw = int_to_scalar(img.width());
    let ih = int_to_scalar(img.height());
    let dst = [
        Point::new(0.0, 80.0),
        Point::new(iw + 28.0, -100.0),
        Point::new(iw - 28.0, ih + 100.0),
        Point::new(0.0, ih - 80.0),
    ];
    let mut persp = Matrix::default();
    assert!(
        persp.set_poly_to_poly(&src, &dst),
        "SkAssertResult(setPolyToPoly)"
    );

    let mut grid: IRect = persp
        .map_rect(Rect::from_isize(img.dimensions()))
        .0
        .round_out();
    grid.left -= 20; // manual adjust to look nicer

    canvas.translate((10.0, 10.0));

    for pair in &matches {
        canvas.save();
        canvas.translate((int_to_scalar(-grid.left), int_to_scalar(-grid.top)));
        draw_persp_config(canvas, &img, &persp, &scale, pair[0]);
        canvas.translate((0.0, int_to_scalar(grid.height())));
        draw_persp_config(canvas, &img, &persp, &scale, pair[1]);
        canvas.restore();

        canvas.translate((int_to_scalar(grid.width()), 0.0));
    }
});

// Port of: gm/complexclip.cpp#L486-L547 (chrome/m156), clip_shader_difference
crate::def_simple_gm!(clip_shader_difference, canvas, 512, 512, {
    let image = get_resource_as_image("images/yellow_rose.png")
        .expect("images/yellow_rose.png (set SKIA_RESOURCES)");
    canvas.clear(Color::GRAY);
    let rect = Rect::from_wh(256.0, 256.0);
    let local = Matrix::rect_to_rect_or_identity(
        Rect::from_isize(image.dimensions()),
        Rect::from_wh(64.0, 64.0),
        None,
    );
    let shader = image
        .to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            SamplingOptions::default(),
            &local,
        )
        .expect("an image shader");

    let mut paint = Paint::default();
    paint.set_color(Color::RED);
    paint.set_anti_alias(true);

    // TL: A rectangle
    canvas.save();
    canvas.translate((0.0, 0.0));
    canvas.clip_shader(shader.clone(), ClipOp::Difference);
    canvas.draw_rect(rect, &paint);
    canvas.restore();

    // TR: A round rectangle
    canvas.save();
    canvas.translate((256.0, 0.0));
    canvas.clip_shader(shader.clone(), ClipOp::Difference);
    canvas.draw_rrect(RRect::new_rect_xy(rect, 64.0, 64.0), &paint);
    canvas.restore();

    // BL: A path
    canvas.save();
    canvas.translate((0.0, 256.0));
    canvas.clip_shader(shader.clone(), ClipOp::Difference);
    let mut path = PathBuilder::new();
    path.move_to((0.0, 128.0));
    path.line_to((128.0, 256.0));
    path.line_to((256.0, 128.0));
    path.line_to((128.0, 0.0));
    // SK_ScalarSqrt2 (the C literal `1.41421356f`, same f32).
    #[allow(clippy::approx_constant)]
    let d = 64.0_f32 * 1.414_213_5_f32;
    path.move_to((128.0 - d, 128.0 - d));
    path.line_to((128.0 - d, 128.0 + d));
    path.line_to((128.0 + d, 128.0 + d));
    path.line_to((128.0 + d, 128.0 - d));
    canvas.draw_path(&path.detach(), &paint);
    canvas.restore();

    // BR: Text
    canvas.save();
    canvas.translate((256.0, 256.0));
    canvas.clip_shader(shader.clone(), ClipOp::Difference);
    let font = Font::from_size(default_portable_typeface(), 64.0);
    for y in 0..4 {
        canvas.draw_simple_text(
            b"Hello",
            TextEncoding::UTF8,
            (32.0, y as f32 * 64.0),
            &font,
            &paint,
        );
    }
    canvas.restore();
});
