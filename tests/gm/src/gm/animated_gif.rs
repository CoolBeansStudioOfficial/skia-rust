// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/animated_gif.cpp#L34-L177 (AnimatedGifGM, chrome/m156), with its default
// --animatedGif resource (images/test640x479.gif). The GM is drawn at its first frame, as the
// golden is; the animation (onAnimate) is not run.
//
// `AnimCodecPlayerExifGM` (the other GMs in that file) is ported below, with SkAnimCodecPlayer.

// The C++ converts between int and scalar, and between indices, as written: the sizes and frame
// indices here are small, so the conversions are exact.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]

use skia_rust_codec::codec::{FrameInfo, NO_FRAME, Options, Result as CodecResult};
use skia_rust_codec::{Codec, decoders};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::point::Point;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::MemoryStream;
use skia_rust_resources::AnimCodecPlayer;

use crate::prelude::*;
use crate::tool_utils::get_resource_as_data;

const RESOURCE: &str = "images/test640x479.gif";

// Port of: gm/animated_gif.cpp#L34-L176 (AnimatedGifGM)
struct AnimatedGifGm {
    codec: Option<Codec<'static>>,
    frame: usize,
    total_frames: usize,
    frame_infos: Vec<FrameInfo>,
    // The pixels of each frame drawn so far, with the info they were decoded to.
    frames: Vec<Option<(ImageInfo, Vec<u8>)>>,
}

impl AnimatedGifGm {
    fn new() -> Self {
        Self {
            codec: None,
            frame: 0,
            total_frames: 0,
            frame_infos: Vec::new(),
            frames: Vec::new(),
        }
    }

    // Port of: gm/animated_gif.cpp#L110-L131 (initCodec)
    fn init_codec(&mut self) -> bool {
        if self.codec.is_some() {
            return true;
        }
        let Some(data) = get_resource_as_data(RESOURCE) else {
            return false;
        };
        let Ok(codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
            return false;
        };
        self.codec = Some(codec);
        self.frame = 0;
        let Some(codec) = self.codec.as_mut() else {
            return false;
        };
        self.frame_infos = codec.frame_infos();
        self.total_frames = self.frame_infos.len();
        true
    }

    // Port of: gm/animated_gif.cpp#L36-L69 (drawFrame)
    fn draw_frame(&mut self, canvas: &Canvas, frame_index: usize) {
        if frame_index >= self.frames.len() {
            self.frames.resize_with(frame_index + 1, || None);
        }
        if self.frames[frame_index].is_none() {
            let Some(codec) = self.codec.as_mut() else {
                return;
            };
            let info = codec.info().with_color_type(ColorType::N32);
            let row_bytes = info.min_row_bytes();
            let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
            let mut opts = Options {
                frame_index: i32::try_from(frame_index).unwrap_or(i32::MAX),
                ..Options::default()
            };
            // For simplicity, do not try to cache old frames: a required frame that was drawn is
            // copied into this frame's pixels, and decoding blends over it.
            let required_frame = self.frame_infos[frame_index].required_frame;
            if required_frame != NO_FRAME
                && let Some(Some((required_info, required_pixels))) =
                    self.frames.get(required_frame as usize)
                && *required_info == info
            {
                pixels.clone_from(required_pixels);
                opts.prior_frame = required_frame;
            }
            let result = codec.get_pixels(&info, &mut pixels, row_bytes, Some(&opts));
            if result != CodecResult::Success {
                eprintln!("Could not getPixels for frame {frame_index}: {RESOURCE}");
                self.frames[frame_index] = Some((info, pixels));
                return;
            }
            self.frames[frame_index] = Some((info, pixels));
        }
        let Some((info, pixels)) = self.frames[frame_index].as_ref() else {
            return;
        };
        let row_bytes = info.min_row_bytes();
        let mut bm = Bitmap::new();
        let installed = bm.install_pixels(info, pixels.clone(), row_bytes);
        debug_assert!(installed);
        if let Some(image) = bm.as_image() {
            canvas.draw_image(&image, (0.0, 0.0), None);
        }
    }
}

impl GM for AnimatedGifGm {
    fn name(&self) -> String {
        "animatedGif".to_string()
    }

    // Port of: gm/animated_gif.cpp#L104-L115 (getISize)
    fn size(&mut self) -> ISize {
        if self.init_codec() {
            let Some(codec) = self.codec.as_ref() else {
                return ISize::new(640, 480);
            };
            let dims = codec.info().dimensions();
            // Wide enough to display all the frames, and tall enough to show the row of frames
            // plus an animating version.
            return ISize::new(
                dims.width * i32::try_from(self.total_frames).unwrap_or(i32::MAX),
                dims.height * 2,
            );
        }
        ISize::new(640, 480)
    }

    // Port of: gm/animated_gif.cpp#L149-L168 (onDraw)
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        if !self.init_codec() {
            *error_msg = format!("Could not create codec from {RESOURCE}");
            return DrawResult::Fail;
        }
        let Some(codec) = self.codec.as_ref() else {
            return DrawResult::Fail;
        };
        let width = codec.info().width();
        let height = codec.info().height();

        let saved = canvas.save();
        for frame_index in 0..self.total_frames {
            self.draw_frame(canvas, frame_index);
            canvas.translate((width as f32, 0.0));
        }
        canvas.restore_to_count(saved);

        let saved = canvas.save();
        canvas.translate((0.0, height as f32));
        let frame = self.frame;
        self.draw_frame(canvas, frame);
        canvas.restore_to_count(saved);
        DrawResult::Ok
    }
}

crate::def_gm!(AnimatedGifGM, AnimatedGifGm::new());

// Port of: gm/animated_gif.cpp#L179-L245 (AnimCodecPlayerExifGM, chrome/m156), under
// SK_ENABLE_SKOTTIE. Draws one frame of an animation per grid cell, using the player.
struct AnimCodecPlayerExifGm {
    path: &'static str,
    size: ISize,
    player: Option<AnimCodecPlayer>,
    frame_infos: Vec<FrameInfo>,
}

impl AnimCodecPlayerExifGm {
    fn new(path: &'static str) -> Self {
        Self {
            path,
            size: ISize::new(0, 0),
            player: None,
            frame_infos: Vec::new(),
        }
    }

    // Port of: gm/animated_gif.cpp#L181-L200 (init)
    fn init(&mut self) {
        if self.player.is_some() {
            return;
        }
        let Some(data) = get_resource_as_data(self.path) else {
            return;
        };
        let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders())
        else {
            return;
        };
        self.frame_infos = codec.frame_infos();
        let player = AnimCodecPlayer::new(codec);

        // We'll draw one of each frame, so make it big enough to hold them all in a grid. The
        // grid will be roughly square, with "factor" frames per row and up to "factor" rows.
        let count = self.frame_infos.len();
        let root = (count as f32).sqrt();
        let factor = root.ceil() as i32;
        let image_size = player.dimensions();
        self.size.width = image_size.width * factor;
        self.size.height = image_size.height * ((count as f32 / factor as f32).ceil() as i32);
        self.player = Some(player);
    }
}

impl GM for AnimCodecPlayerExifGm {
    // Port of: gm/animated_gif.cpp#L202-L205 (getName)
    fn name(&self) -> String {
        let basename = self.path.rsplit('/').next().unwrap_or(self.path);
        format!("AnimCodecPlayerExif_{basename}")
    }

    // Port of: gm/animated_gif.cpp#L207-L210 (getISize)
    fn size(&mut self) -> ISize {
        self.init();
        self.size
    }

    // Port of: gm/animated_gif.cpp#L212-L243 (onDraw)
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        self.init();
        let Some(player) = self.player.as_mut() else {
            return DrawResult::Ok;
        };

        let root = (self.frame_infos.len() as f32).sqrt();
        let factor = root.ceil() as i32;
        let dimensions = player.dimensions();

        let mut duration: u32 = 0;
        let mut frame: usize = 0;
        while duration < player.duration() {
            let saved = canvas.save();
            let frame_i32 = i32::try_from(frame).unwrap_or(i32::MAX);
            let x_translate = (frame_i32 % factor) * dimensions.width;
            let y_translate = (frame_i32 / factor) * dimensions.height;
            canvas.translate((x_translate as f32, y_translate as f32));

            if let Some(image) = player.get_frame() {
                canvas.draw_image(&image, Point { x: 0.0, y: 0.0 }, None);
            }
            // The frame durations are the codec's, not the player's end times.
            let frame_duration = self.frame_infos[frame].duration;
            duration += u32::try_from(frame_duration).unwrap_or(0);
            player.seek(duration);
            canvas.restore_to_count(saved);
            frame += 1;
        }
        DrawResult::Ok
    }
}

crate::def_gm!(
    AnimCodecPlayerExifGM_required_webp = "AnimCodecPlayerExifGM(\"images/required.webp\")",
    AnimCodecPlayerExifGm::new("images/required.webp")
);
crate::def_gm!(
    AnimCodecPlayerExifGM_required_gif = "AnimCodecPlayerExifGM(\"images/required.gif\")",
    AnimCodecPlayerExifGm::new("images/required.gif")
);
crate::def_gm!(
    AnimCodecPlayerExifGM_stoplight_h_webp = "AnimCodecPlayerExifGM(\"images/stoplight_h.webp\")",
    AnimCodecPlayerExifGm::new("images/stoplight_h.webp")
);
