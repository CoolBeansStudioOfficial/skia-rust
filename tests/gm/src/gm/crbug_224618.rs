// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_224618.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::color_priv::ColorConverter;
use skia_rust_core::image::Image;
use skia_rust_core::m44::{M44, V3, V4};
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::{degrees_to_radians, scalar_mod};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::interpolation::InPremul;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_raster::surfaces;

const K_MAX_VW: i32 = 800;
const K_MIN_VW: i32 = 300;

// `fTime` is only set by `onAnimate`, which the viewer calls; DM draws the GM at its initial 0.
const TIME: f32 = 0.0;

// Port of: gm/crbug_224618.cpp#L19-L114 (chrome/m156), CrBug224618GM
struct CrBug224618Gm {
    cube_image: Option<Image>,
}

impl GM for CrBug224618Gm {
    // Port of: gm/crbug_224618.cpp#L24 (chrome/m156), getName
    fn name(&self) -> String {
        "crbug_224618".to_string()
    }

    // Port of: gm/crbug_224618.cpp#L26 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(K_MAX_VW, K_MAX_VW)
    }

    // Port of: gm/crbug_224618.cpp#L35-L49 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let conv = ColorConverter::new(&[
            Color::TRANSPARENT,
            Color::from_argb(128, 255, 255, 255),
        ]);
        let gradient = Gradient::new(
            Colors::new(conv.colors4f(), None, TileMode::Mirror, None),
            Interpolation {
                in_premul: InPremul::Yes,
                ..Interpolation::default()
            },
        );
        let shader = gradient_shaders::radial_gradient(((200.0, 200.0), 25.0), &gradient, None);

        let mut surface = surfaces::raster_n32_premul((400, 400));
        if let Some(surface) = surface.as_mut() {
            let mut bg_paint = Paint::default();
            bg_paint.set_shader(shader);
            surface.shared_canvas().draw_paint(&bg_paint);
            self.cube_image = surface.image_snapshot();
        }
    }

    // Port of: gm/crbug_224618.cpp#L51-L107 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let viewport_width = scalar_mod(TIME, 10.0) / 10.0
            * int_to_scalar(K_MAX_VW - K_MIN_VW)
            + int_to_scalar(K_MIN_VW);
        let radius = viewport_width / 2.0; // round?

        // See https://developer.mozilla.org/en-US/docs/Web/CSS/transform-function/perspective
        let proj = M44::new(
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, -1.0 / radius, 1.0,
        );
        let zoom = M44::translate(0.0, 0.0, radius);
        let post_zoom = M44::translate(0.0, 0.0, -radius - 1.0);
        let rotate_horizontal = M44::rotate(V3::new(0.0, 1.0, 0.0), 2.356_194_490_192_345_f32);

        // w in degrees will need to be converted to radians
        let axis_angles = [
            V4::new(0.0, 1.0, 0.0, -90.0), // rotateY(-90deg)
            V4::new(1.0, 0.0, 0.0, 0.0),   // <none>
            V4::new(0.0, 1.0, 0.0, 90.0),  // rotateY(90deg)
            V4::new(0.0, 1.0, 0.0, 180.0), // rotateY(180deg)
            V4::new(1.0, 0.0, 0.0, -90.0), // rotateX(-90deg)
            V4::new(1.0, 0.0, 0.0, 90.0),  // rotateX(90deg)
        ];
        let face_colors = [
            Color::RED,
            Color::GREEN,
            Color::BLUE,
            Color::YELLOW,
            Color::from_argb(0xFF, 0xFF, 0xA5, 0x00), // orange css
            Color::from_argb(0xFF, 0x80, 0x00, 0x80), // purple css
        ];

        for (i, axis_angle) in axis_angles.iter().enumerate() {
            let model = M44::rotate(
                V3::new(axis_angle.x, axis_angle.y, axis_angle.z),
                degrees_to_radians(axis_angle.w),
            );
            // `SkM44::Translate(radius, radius) * proj * zoom * rotateHorizontal * model *
            // postZoom * SkM44::Translate(-radius, -radius)`, left to right.
            let model = M44::concat(
                &M44::concat(
                    &M44::concat(
                        &M44::concat(
                            &M44::concat(
                                &M44::concat(&M44::translate(radius, radius, 0.0), &proj),
                                &zoom,
                            ),
                            &rotate_horizontal,
                        ),
                        &model,
                    ),
                    &post_zoom,
                ),
                &M44::translate(-radius, -radius, 0.0),
            );

            canvas.save();
            canvas.concat_44(&model);

            let mut fill_paint = Paint::default();
            fill_paint.set_anti_alias(true);
            fill_paint.set_color(face_colors[i]);
            // Leverages FillRectOp on GPU backend
            canvas.draw_rect(
                Rect::from_wh(viewport_width, viewport_width),
                &fill_paint,
            );

            // Leverages TextureOp on GPU backend, to ensure sure both quad paths handle clipping
            if let Some(cube) = self.cube_image.as_ref() {
                let src = Rect::from_wh(int_to_scalar(cube.width()), int_to_scalar(cube.height()));
                canvas.draw_image_rect_with_sampling_options(
                    cube,
                    Some((&src, SrcRectConstraint::Fast)),
                    Rect::from_wh(viewport_width, viewport_width),
                    SamplingOptions::from(FilterMode::Linear),
                    &fill_paint,
                );
            }
            canvas.restore();
        }
    }
}

// Port of: gm/crbug_224618.cpp#L116 (chrome/m156), DEF_GM(return new CrBug224618GM();)
crate::def_gm!(
    CrBug224618GM_ = "CrBug224618GM()",
    CrBug224618Gm {
        cube_image: None
    }
);
