// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/animatedimageblurs.cpp (chrome/m156)
//
// DM draws these GMs without animating them: `onAnimate` (the `TimeUtils::PingPong` motion) is
// not part of the raster harness (`crate::GM`), so every GM here is drawn at its
// `onOnceBeforeDraw` state, as the first frame is.

use crate::prelude::*;
use crate::tool_utils::get_resource_as_image;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{blur, crop};
use skia_rust_tools::font_tool_utils::default_portable_typeface;

const BLUR_MAX: f32 = 7.0;
const NUM_NODES: usize = 30;
const WIDTH: i32 = 512;
const HEIGHT: i32 = 512;

// Port of: gm/animatedimageblurs.cpp#L23-L110 (chrome/m156), AnimatedImageBlurs
struct AnimatedImageBlursGm {
    nodes: [Node; NUM_NODES],
    rand: Random,
}

// Port of: gm/animatedimageblurs.cpp (chrome/m156), AnimatedImageBlurs::Node
#[derive(Clone, Copy)]
struct Node {
    size: f32,
    pos: Point,
    dir: Point,
    blur_offset: f32,
    blur: f32,
    speed: f32,
}

impl Node {
    // Port of: gm/animatedimageblurs.cpp#L69-L86 (chrome/m156), Node::Node
    fn new() -> Self {
        Self {
            size: 0.0,
            pos: Point::new(0.0, 0.0),
            dir: Point::new(1.0, 0.0),
            blur_offset: 0.0,
            blur: 0.0,
            speed: 0.0,
        }
    }

    // Port of: gm/animatedimageblurs.cpp#L88-L102 (chrome/m156), Node::init
    fn init(&mut self, rand: &mut Random) {
        self.size = rand.next_range_f(10.0, 60.0);
        self.pos.x = rand.next_range_f(self.size, int_to_scalar(WIDTH) - self.size);
        self.pos.y = rand.next_range_f(self.size, int_to_scalar(HEIGHT) - self.size);
        self.dir.x = rand.next_range_f(-1.0, 1.0);
        self.dir.y = (1.0 - self.dir.x * self.dir.x).sqrt();
        if rand.next_bool() {
            self.dir.y = -self.dir.y;
        }
        self.blur_offset = rand.next_range_f(0.0, BLUR_MAX);
        self.blur = self.blur_offset;
        self.speed = rand.next_range_f(20.0, 60.0);
    }
}

impl GM for AnimatedImageBlursGm {
    fn name(&self) -> String {
        "animated-image-blurs".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFCC_CCCC)
    }

    // Port of: gm/animatedimageblurs.cpp#L42-L46 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        for node in &mut self.nodes {
            node.init(&mut self.rand);
        }
    }

    // Port of: gm/animatedimageblurs.cpp#L48-L67 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        for node in &self.nodes {
            let mut layer_paint = Paint::default();
            layer_paint.set_image_filter(blur(node.blur, node.blur, TileMode::Decal, None, None));

            canvas.save_layer(&SaveLayerRec::default().paint(&layer_paint));
            // The rect is outset to block the circle case
            let rect = Rect::new(
                node.pos.x - node.size - 0.5,
                node.pos.y - node.size - 0.5,
                node.pos.x + node.size + 0.5,
                node.pos.y + node.size + 0.5,
            );
            let rr = RRect::new_rect_xy(rect, node.size, node.size);
            canvas.draw_rrect(rr, &paint);
            canvas.restore();
        }
    }
}

// Port of: gm/animatedimageblurs.cpp#L110-L186 (chrome/m156), AnimatedTiledImageBlur
struct AnimatedTiledImageBlurGm {
    image: Option<Image>,
    blur_sigma: f32,
}

impl AnimatedTiledImageBlurGm {
    // Port of: gm/animatedimageblurs.cpp#L113-L116 (chrome/m156), the constructor
    fn new() -> Self {
        const K_MAX_BLUR_SIGMA: f32 = 250.0;
        Self {
            image: None,
            blur_sigma: 0.3 * K_MAX_BLUR_SIGMA,
        }
    }
}

impl GM for AnimatedTiledImageBlurGm {
    fn name(&self) -> String {
        "animated-tiled-image-blur".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(530, 530)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFCC_CCCC)
    }

    // Port of: gm/animatedimageblurs.cpp#L129-L131 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.image = get_resource_as_image("images/mandrill_512.png");
    }

    // Port of: gm/animatedimageblurs.cpp#L133-L152 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(image) = self.image.clone() else {
            return;
        };
        let sigma = self.blur_sigma;
        let draw_blurred_image = |tx: f32, ty: f32, tile_mode: TileMode| {
            let mut paint = Paint::default();
            let rect = Rect::from_wh(250.0, 250.0);
            canvas.save();
            canvas.translate((tx, ty));
            paint.set_image_filter(blur(sigma, sigma, tile_mode, None, Some(rect)));
            canvas.draw_image_rect_with_sampling_options(
                &image,
                None,
                rect,
                FilterMode::Linear,
                &paint,
            );
            canvas.restore();
        };

        draw_blurred_image(10.0, 10.0, TileMode::Decal);
        draw_blurred_image(270.0, 10.0, TileMode::Clamp);
        draw_blurred_image(10.0, 270.0, TileMode::Repeat);
        draw_blurred_image(270.0, 270.0, TileMode::Mirror);
    }
}

// Port of: gm/animatedimageblurs.cpp#L188-L264 (chrome/m156), AnimatedBackdropBlur
struct AnimatedBackdropBlurGm {
    filter: Option<ImageFilter>,
    image: Option<Image>,
    font: Option<Font>,
}

impl AnimatedBackdropBlurGm {
    fn new() -> Self {
        Self {
            filter: None,
            image: None,
            font: None,
        }
    }
}

impl GM for AnimatedBackdropBlurGm {
    fn name(&self) -> String {
        "animated-backdrop-blur".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(512, 1024)
    }

    // Port of: gm/animatedimageblurs.cpp#L196-L205 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.font = Some(Font::from_size(default_portable_typeface(), 20.0));
        self.image = get_resource_as_image("images/color_wheel.png");
        let crop_rect = Rect::new(0.0, 100.0, 512.0, 400.0);
        self.filter = crop(
            &crop_rect,
            TileMode::Decal,
            blur(
                30.0,
                30.0,
                TileMode::Decal,
                crop(&crop_rect, TileMode::Mirror, None),
                None,
            ),
        );
    }

    // Port of: gm/animatedimageblurs.cpp#L207-L234 (chrome/m156), onDraw. `fVOffset` is 0: it is
    // only set by `onAnimate`.
    fn on_draw(&mut self, canvas: &Canvas) {
        const TXTS: [&str; 14] = [
            "Lorem ipsum dolor sit amet,",
            "consectetur adipiscing elit,",
            "sed do eiusmod tempor incididunt",
            "ut labore et dolore magna aliqua.",
            "",
            "",
            "Ut enim ad minim veniam,",
            "quis nostrud exercitation ullamco laboris",
            "nisi ut aliquip ex ea commodo consequat.",
            "",
            "",
            "Duis aute irure dolor in reprehenderit",
            "in voluptate velit esse cillum dolore",
            "eu fugiat nulla pariatur.",
        ];
        let Some(font) = self.font.clone() else {
            return;
        };

        let paint = Paint::default();
        let mut voffset: f32 = 0.0;
        for txt in TXTS {
            canvas.draw_simple_text(txt, TextEncoding::UTF8, (0.0, voffset), &font, &paint);
            voffset += font.size();
        }

        if let Some(image) = &self.image {
            let dst_height = int_to_scalar(image.height()) * 128.0 / int_to_scalar(image.width());
            canvas.draw_image_rect_with_sampling_options(
                image,
                None,
                Rect::from_xywh(16.0, 0.0, 128.0, dst_height),
                FilterMode::Linear,
                &Paint::default(),
            );
        }

        let mut layer_rec = SaveLayerRec::default();
        if let Some(filter) = &self.filter {
            layer_rec = layer_rec.backdrop(filter);
        }
        canvas.save_layer(&layer_rec);
        canvas.restore();
    }
}

// Port of: gm/animatedimageblurs.cpp#L266-L268 (chrome/m156)
crate::def_gm!(
    AnimatedImageBlurs,
    AnimatedImageBlursGm {
        nodes: [Node::new(); NUM_NODES],
        rand: Random::default()
    }
);
crate::def_gm!(AnimatedTiledImageBlur, AnimatedTiledImageBlurGm::new());
crate::def_gm!(AnimatedBackdropBlur, AnimatedBackdropBlurGm::new());
