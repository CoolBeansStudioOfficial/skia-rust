// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/postercircle.cpp (chrome/m156)

// Mimics https://output.jsbin.com/falefice/1/quiet?CC_POSTER_CIRCLE, which can't be captured as
// an SKP due to many 3D layers being composited post-SKP capture.
// See skbug.com/40040313
//
// DM never calls `onAnimate`, so the animation time stays at 0 for the raster goldens.

// The int/float mixing mirrors the C++ arithmetic of the GM.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

use crate::prelude::*;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::font::Edging;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image::Image;
use skia_rust_core::m44::{M44, V3};
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::{degrees_to_radians, scalar_mod};
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_font;

const ANGLE_STEP: i32 = 30;
const NUM_ANGLES: usize = 12; // 0 through 330 degrees

const STAGE_WIDTH: i32 = 600;
const STAGE_HEIGHT: i32 = 400;
const RING_RADIUS: i32 = 200;
const POSTER_SIZE: i32 = 100;

// These rotation limits were chosen manually to line up with current projection.
const BACK_MIN_ANGLE: f32 = 70.0;
const BACK_MAX_ANGLE: f32 = 290.0;

// Port of: gm/postercircle.cpp (chrome/m156), class PosterCircleGM
#[derive(Debug)]
pub struct PosterCircleGm {
    time: f32,
    poster_images: Vec<Option<Image>>,
}

impl PosterCircleGm {
    // Port of: gm/postercircle.cpp (chrome/m156), PosterCircleGM()
    #[must_use]
    pub fn new() -> Self {
        Self {
            time: 0.0,
            poster_images: vec![None; NUM_ANGLES],
        }
    }
}

impl Default for PosterCircleGm {
    fn default() -> Self {
        Self::new()
    }
}

impl GM for PosterCircleGm {
    fn name(&self) -> String {
        "poster_circle".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(STAGE_WIDTH, STAGE_HEIGHT + 50)
    }

    // Port of: gm/postercircle.cpp (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let mut font = default_portable_font();
        font.set_edging(Edging::AntiAlias);
        font.set_embolden(true);
        font.set_size(24.0);

        let mut surface =
            surfaces::raster_n32_premul((POSTER_SIZE, POSTER_SIZE)).expect("a raster surface");
        for i in 0..NUM_ANGLES {
            let canvas = surface.canvas();

            let mut fill_paint = Paint::default();
            fill_paint.set_anti_alias(true);
            fill_paint.set_color(if i % 2 == 0 {
                Color::from_argb(0xFF, 0x99, 0x5C, 0x7F)
            } else {
                Color::from_argb(0xFF, 0x83, 0x5A, 0x99)
            });
            canvas.draw_rrect(
                RRect::new_rect_xy(
                    Rect::from_wh(POSTER_SIZE as f32, POSTER_SIZE as f32),
                    10.0,
                    10.0,
                ),
                &fill_paint,
            );

            let label = i.to_string();
            let (_, label_bounds) = font.measure_text(label.as_bytes(), TextEncoding::UTF8, None);
            let label_x = 0.5 * POSTER_SIZE as f32 - 0.5 * label_bounds.width();
            let label_y = 0.5 * POSTER_SIZE as f32 + 0.5 * label_bounds.height();

            let mut label_paint = Paint::default();
            label_paint.set_anti_alias(true);
            canvas.draw_str(&label, (label_x, label_y), &font, &label_paint);

            self.poster_images[i] = surface.image_snapshot();
        }
    }

    // Port of: gm/postercircle.cpp (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        // See https://developer.mozilla.org/en-US/docs/Web/CSS/transform-function/perspective
        // for projection matrix when --webkit-perspective: 800px is used.
        let mut proj = M44::default();
        proj.set_rc(3, 2, -1.0 / 800.0);

        for pass in 0..2 {
            // Want to draw 90 to 270 first (the back), then 270 to 90 (the front), but do all 3
            // rings backsides, then their frontsides since the front projections overlap across
            // rings. Note: we skip the poster circle's x axis rotation because that complicates the
            // back-to-front drawing order and it isn't necessary to trigger draws aligned with Z.
            let draw_front = pass > 0;

            for y in 0..3 {
                let ring_y = (y - 1) as f32 * (POSTER_SIZE as f32 + 10.0);
                for i in 0..NUM_ANGLES {
                    // Add an extra 45 degree rotation, which triggers the bug by aligning some of
                    // the posters with the z axis.
                    let y_duration = 5.0 - y as f32;
                    let y_rotation = scalar_mod(
                        (ANGLE_STEP * i as i32) as f32
                            + 360.0 * scalar_mod(self.time / y_duration, y_duration),
                        360.0,
                    );
                    // These rotation limits were chosen manually to line up with current projection
                    if draw_front {
                        if (BACK_MIN_ANGLE..=BACK_MAX_ANGLE).contains(&y_rotation) {
                            // Back portion during a front draw
                            continue;
                        }
                    } else if !(BACK_MIN_ANGLE..=BACK_MAX_ANGLE).contains(&y_rotation) {
                        // Front portion during a back draw
                        continue;
                    }

                    canvas.save();

                    // Matrix matches transform: rotateY(<angle>deg) translateZ(200px); nested in an
                    // element with the perspective projection matrix above.
                    let model = M44::concat(
                        &M44::concat(
                            &M44::concat(
                                &M44::concat(
                                    &M44::translate(
                                        (STAGE_WIDTH / 2) as f32,
                                        (STAGE_HEIGHT / 2 + 25) as f32,
                                        0.0,
                                    ),
                                    &proj,
                                ),
                                &M44::translate(0.0, ring_y, 0.0),
                            ),
                            &M44::rotate(
                                V3 {
                                    x: 0.0,
                                    y: 1.0,
                                    z: 0.0,
                                },
                                degrees_to_radians(y_rotation),
                            ),
                        ),
                        &M44::translate(0.0, 0.0, RING_RADIUS as f32),
                    );
                    canvas.concat_44(&model);

                    let poster = Rect::from_ltrb(
                        -0.5 * POSTER_SIZE as f32,
                        -0.5 * POSTER_SIZE as f32,
                        0.5 * POSTER_SIZE as f32,
                        0.5 * POSTER_SIZE as f32,
                    );
                    let mut fill_paint = Paint::default();
                    fill_paint.set_anti_alias(true);
                    fill_paint.set_alpha_f(0.7);
                    if let Some(image) = &self.poster_images[i] {
                        canvas.draw_image_rect_with_sampling_options(
                            image,
                            None::<(&Rect, SrcRectConstraint)>,
                            poster,
                            SamplingOptions::from(FilterMode::Linear),
                            &fill_paint,
                        );
                    }

                    canvas.restore();
                }
            }
        }
    }
}

// Port of: gm/postercircle.cpp#L152-L152 (chrome/m156), DEF_GM(return new PosterCircleGM();)
crate::def_gm!(PosterCircleGM_ = "PosterCircleGM()", PosterCircleGm::new());
