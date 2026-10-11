// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/displacement.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::{color_to_565, create_checkerboard_image, int_to_scalar};
use skia_rust_core::color::ColorChannel;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_effects::image_filters::{displacement_map, image_sampled};
use skia_rust_tools::font_tool_utils::create_string_bitmap;

// Port of: gm/displacement.cpp#L27-L30 (chrome/m156), drawClippedBitmap
fn draw_clipped_bitmap(canvas: &Canvas, image: &Image, x: i32, y: i32, paint: &Paint) {
    canvas.save();
    canvas.translate((int_to_scalar(x), int_to_scalar(y)));
    canvas.clip_irect(image.bounds(), None);
    canvas.draw_image_with_sampling_options(
        image,
        (0.0, 0.0),
        SamplingOptions::default(),
        Some(paint),
    );
    canvas.restore();
}

// One `DisplacementMap` draw of the GM: the channel selectors, the scale, the offset at which the
// clipped bitmap is drawn, and the optional crop rect (`SkIRect(30, 30, 40, 40)` as a float rect).
struct Case {
    x_channel: ColorChannel,
    y_channel: ColorChannel,
    scale: f32,
    x: i32,
    y: i32,
    cropped: bool,
}

// Port of: gm/displacement.cpp#L34-L49 (chrome/m156), the first 25 draws of onDraw
const CASES: [Case; 21] = [
    Case {
        x_channel: ColorChannel::R,
        y_channel: ColorChannel::G,
        scale: 0.0,
        x: 0,
        y: 0,
        cropped: false,
    },
    Case {
        x_channel: ColorChannel::B,
        y_channel: ColorChannel::A,
        scale: 16.0,
        x: 100,
        y: 0,
        cropped: false,
    },
    Case {
        x_channel: ColorChannel::R,
        y_channel: ColorChannel::B,
        scale: 32.0,
        x: 200,
        y: 0,
        cropped: false,
    },
    Case {
        x_channel: ColorChannel::G,
        y_channel: ColorChannel::A,
        scale: 48.0,
        x: 300,
        y: 0,
        cropped: false,
    },
    Case {
        x_channel: ColorChannel::R,
        y_channel: ColorChannel::A,
        scale: 64.0,
        x: 400,
        y: 0,
        cropped: false,
    },
    Case {
        x_channel: ColorChannel::R,
        y_channel: ColorChannel::G,
        scale: 40.0,
        x: 0,
        y: 100,
        cropped: false,
    },
    Case {
        x_channel: ColorChannel::B,
        y_channel: ColorChannel::A,
        scale: 40.0,
        x: 100,
        y: 100,
        cropped: false,
    },
    Case {
        x_channel: ColorChannel::R,
        y_channel: ColorChannel::B,
        scale: 40.0,
        x: 200,
        y: 100,
        cropped: false,
    },
    Case {
        x_channel: ColorChannel::G,
        y_channel: ColorChannel::A,
        scale: 40.0,
        x: 300,
        y: 100,
        cropped: false,
    },
    Case {
        x_channel: ColorChannel::R,
        y_channel: ColorChannel::A,
        scale: 40.0,
        x: 400,
        y: 100,
        cropped: false,
    },
    Case {
        x_channel: ColorChannel::R,
        y_channel: ColorChannel::G,
        scale: 0.0,
        x: 0,
        y: 200,
        cropped: true,
    },
    Case {
        x_channel: ColorChannel::B,
        y_channel: ColorChannel::A,
        scale: 16.0,
        x: 100,
        y: 200,
        cropped: true,
    },
    Case {
        x_channel: ColorChannel::R,
        y_channel: ColorChannel::B,
        scale: 32.0,
        x: 200,
        y: 200,
        cropped: true,
    },
    Case {
        x_channel: ColorChannel::G,
        y_channel: ColorChannel::A,
        scale: 48.0,
        x: 300,
        y: 200,
        cropped: true,
    },
    Case {
        x_channel: ColorChannel::R,
        y_channel: ColorChannel::A,
        scale: 64.0,
        x: 400,
        y: 200,
        cropped: true,
    },
    Case {
        x_channel: ColorChannel::R,
        y_channel: ColorChannel::G,
        scale: 40.0,
        x: 0,
        y: 300,
        cropped: true,
    },
    Case {
        x_channel: ColorChannel::B,
        y_channel: ColorChannel::A,
        scale: 40.0,
        x: 100,
        y: 300,
        cropped: true,
    },
    Case {
        x_channel: ColorChannel::R,
        y_channel: ColorChannel::B,
        scale: 40.0,
        x: 200,
        y: 300,
        cropped: true,
    },
    Case {
        x_channel: ColorChannel::G,
        y_channel: ColorChannel::A,
        scale: 40.0,
        x: 300,
        y: 300,
        cropped: true,
    },
    Case {
        x_channel: ColorChannel::R,
        y_channel: ColorChannel::A,
        scale: 40.0,
        x: 400,
        y: 300,
        cropped: true,
    },
    // Test for negative scale.
    Case {
        x_channel: ColorChannel::G,
        y_channel: ColorChannel::A,
        scale: -40.0,
        x: 500,
        y: 0,
        cropped: false,
    },
];

// Port of: gm/displacement.cpp#L27-L30 (chrome/m156), the sized-displacement and no-displacement
// draws at the end of onDraw
const SIZED_CASES: [(ColorChannel, ColorChannel, i32, i32); 4] = [
    (ColorChannel::R, ColorChannel::G, 0, 400),
    (ColorChannel::B, ColorChannel::A, 100, 400),
    (ColorChannel::R, ColorChannel::B, 200, 400),
    (ColorChannel::G, ColorChannel::A, 300, 400),
];

// Port of: gm/displacement.cpp#L13-L80 (chrome/m156), DisplacementMapGM
struct DisplacementMapGm {
    image: Option<Image>,
    checkerboard: Option<Image>,
    small: Option<Image>,
    large: Option<Image>,
    large_w: Option<Image>,
    large_h: Option<Image>,
}

impl DisplacementMapGm {
    fn new() -> Self {
        Self {
            image: None,
            checkerboard: None,
            small: None,
            large: None,
            large_w: None,
            large_h: None,
        }
    }
}

impl GM for DisplacementMapGm {
    fn name(&self) -> String {
        "displacement".to_string()
    }

    // Port of: gm/displacement.cpp#L16-L18 (chrome/m156), setBGColor(0xFF000000)
    fn bg_color(&self) -> Color {
        Color::BLACK
    }

    fn size(&mut self) -> ISize {
        ISize::new(600, 500)
    }

    // Port of: gm/displacement.cpp#L20-L31 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        // ToolUtils::CreateStringImage is CreateStringBitmap(...).asImage().
        self.image =
            create_string_bitmap(80, 80, Color::from(0xFF88_4422), 15, 55, 96, "g").as_image();
        let c1 = color_to_565(Color::from(0xFF24_4484));
        let c2 = color_to_565(Color::from(0xFF80_4020));
        self.checkerboard = Some(create_checkerboard_image(80, 80, c1, c2, 8));
        self.small = Some(create_checkerboard_image(64, 64, c1, c2, 8));
        self.large = Some(create_checkerboard_image(96, 96, c1, c2, 8));
        self.large_w = Some(create_checkerboard_image(96, 64, c1, c2, 8));
        self.large_h = Some(create_checkerboard_image(64, 96, c1, c2, 8));
    }

    // Port of: gm/displacement.cpp#L40-L114 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let image = self
            .image
            .clone()
            .expect("the image is made in onOnceBeforeDraw");
        let checkerboard = self.checkerboard.clone().expect("the checkerboard is made");
        canvas.clear(Color::BLACK);

        let mut paint = Paint::default();
        let displ: Option<ImageFilter> = image_sampled(
            Some(checkerboard),
            SamplingOptions::from(FilterMode::Linear),
        );
        // SkIRect::MakeXYWH(30, 30, 40, 40)
        let crop_rect = Rect::from_ltrb(30.0, 30.0, 70.0, 70.0);

        for case in &CASES {
            let crop = if case.cropped { Some(crop_rect) } else { None };
            paint.set_image_filter(displacement_map(
                (case.x_channel, case.y_channel),
                case.scale,
                displ.clone(),
                None,
                crop,
            ));
            draw_clipped_bitmap(canvas, &image, case.x, case.y, &paint);
        }

        // Tests for images of different sizes
        let sized = [
            self.small.clone(),
            self.large.clone(),
            self.large_w.clone(),
            self.large_h.clone(),
        ];
        for ((x_channel, y_channel, x, y), small_image) in SIZED_CASES.into_iter().zip(sized) {
            let displ = image_sampled(small_image, SamplingOptions::from(FilterMode::Linear));
            paint.set_image_filter(displacement_map(
                (x_channel, y_channel),
                40.0,
                displ,
                None,
                None,
            ));
            draw_clipped_bitmap(canvas, &image, x, y, &paint);
        }

        // Test for no given displacement input. In this case, both displacement
        // and color should use the same bitmap, given to SkCanvas::drawBitmap()
        // as an input argument.
        paint.set_image_filter(displacement_map(
            (ColorChannel::G, ColorChannel::A),
            40.0,
            None,
            None,
            None,
        ));
        draw_clipped_bitmap(canvas, &image, 400, 400, &paint);
    }
}

// Port of: gm/displacement.cpp#L116 (chrome/m156)
crate::def_gm!(DisplacementMapGM, DisplacementMapGm::new());
