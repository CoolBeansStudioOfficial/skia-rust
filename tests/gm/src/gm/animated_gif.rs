// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/animated_gif.cpp#L34-L177 (AnimatedGifGM, chrome/m156), with its default
// --animatedGif resource (images/test640x479.gif). The GM is drawn at its first frame, as the
// golden is; the animation (onAnimate) is not run.
//
// `AnimCodecPlayerExifGM` (the other GM in that file) needs SkAnimCodecPlayer, which is not ported.

// The C++ converts between int and scalar, and between indices, as written: the sizes and frame
// indices here are small, so the conversions are exact.
#![allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]

use skia_rust_codec::codec::{FrameInfo, NO_FRAME, Options, Result as CodecResult};
use skia_rust_codec::{Codec, decoders};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::stream::MemoryStream;

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
