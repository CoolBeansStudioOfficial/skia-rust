// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/animated_image_orientation.cpp (chrome/m156)

// The `int` to `float` and `float` to `int` conversions of the GM are written as in the C++: the
// sizes here are small, so the int-to-float conversions are exact, and the truncations are the
// C++ implicit conversions of positive values.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use skia_rust_codec::Codec;
use skia_rust_codec::android_codec::AndroidCodec;
use skia_rust_codec::animated_image::{AnimatedImage, FINISHED};
use skia_rust_codec::decoders;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::rrect::RRect;
use skia_rust_core::scalar::scalar_floor_to_int;
use skia_rust_core::stream::MemoryStream;

use crate::prelude::*;
use crate::tool_utils::get_resource_as_data;

// Port of: gm/animated_image_orientation.cpp#L26-L43 (post_processor)
fn post_processor(bounds: Rect) -> Option<Picture> {
    let radius = ((bounds.width() + bounds.height()) / 6.0) as i32;
    let mut path_builder = PathBuilder::new();
    path_builder
        .set_fill_type(PathFillType::InverseEvenOdd)
        .add_rrect(
            RRect::new_rect_xy(bounds, radius as f32, radius as f32),
            None,
            None,
        );

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::TRANSPARENT);
    paint.set_blend_mode(BlendMode::Src);

    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(bounds, false);
    canvas.draw_path(&path_builder.detach(), &paint);
    recorder.finish_recording_as_picture(None)
}

// Port of `SkCodec::MakeFromData` on the GM's resource bytes.
fn codec_from_data(data: &[u8]) -> Option<Codec<'static>> {
    Codec::make_from_stream(MemoryStream::make_copy(data), decoders()).ok()
}

// Port of: gm/animated_image_orientation.cpp#L45-L140 (AnimatedImageGM)
struct AnimatedImageGm {
    path: &'static str,
    name: &'static str,
    step: i32,
    crop_rect: IRect,
    size: ISize,
    translate: i32,
    // Port of `fData`: the resource, read by `init`.
    data: Option<Vec<u8>>,
}

// Port of `AnimatedImageGM::kMaxFrames`.
const MAX_FRAMES: i32 = 2;

impl AnimatedImageGm {
    // Port of: gm/animated_image_orientation.cpp#L52-L66 (the constructor)
    fn new(path: &'static str, name: &'static str, step: i32, crop_rect: IRect) -> Self {
        Self {
            path,
            name,
            step,
            crop_rect,
            size: ISize::new(0, 0),
            translate: 0,
            data: None,
        }
    }

    // Port of: gm/animated_image_orientation.cpp#L68-L84 (init)
    fn init(&mut self) {
        if self.data.is_some() {
            return;
        }
        let Some(data) = get_resource_as_data(self.path) else {
            return;
        };
        let Some(codec) = codec_from_data(&data) else {
            return;
        };
        let dimensions = codec.dimensions();

        // may be rotated
        self.translate = ((dimensions.width.max(dimensions.height) as f32) * 1.25 // will be scaled up
            + 2.0) as i32; // padding

        self.size = ISize::new(
            self.translate * MAX_FRAMES * 2 // crop and no-crop
                * 2, // post-process and no post-process
            self.translate * 4 // 4 scales
                * 2, // makePictureSnapshot and getCurrentFrame
        );
        self.data = Some(data);
    }
}

// Port of: gm/animated_image_orientation.cpp#L86-L94 (the draw procedure of onDraw)
fn draw_proc(canvas: &Canvas, use_pic: bool, animated_image: &AnimatedImage) {
    if use_pic {
        if let Some(pic) = animated_image.make_picture_snapshot() {
            canvas.draw_picture(&pic, None, None);
        }
    } else if let Some(image) = animated_image.get_current_frame() {
        canvas.draw_image(&image, (0.0, 0.0), None);
    }
}

impl GM for AnimatedImageGm {
    // Port of: gm/animated_image_orientation.cpp#L96-L98 (getName)
    fn name(&self) -> String {
        format!("{}_animated_image", self.name)
    }

    // Port of: gm/animated_image_orientation.cpp#L100-L103 (getISize)
    fn size(&mut self) -> ISize {
        self.init();
        self.size
    }

    // Port of: gm/animated_image_orientation.cpp#L105-L168 (onDraw)
    fn on_draw(&mut self, canvas: &Canvas) {
        self.init();
        let Some(data) = self.data.clone() else {
            return;
        };
        for use_pic in [true, false] {
            for scale in [1.25f32, 1.0, 0.75, 0.5] {
                canvas.save();
                for do_crop in [false, true] {
                    for do_post_process in [false, true] {
                        let Some(codec) = codec_from_data(&data) else {
                            return;
                        };
                        let origin = codec.origin();
                        let Some(android_codec) = AndroidCodec::make_from_codec(codec) else {
                            return;
                        };
                        let info = android_codec.info();
                        let unscaled_size = if origin.swaps_width_height() {
                            ISize::new(info.height(), info.width())
                        } else {
                            info.dimensions()
                        };

                        let scaled_size = ISize::new(
                            scalar_floor_to_int(unscaled_size.width as f32 * scale),
                            scalar_floor_to_int(unscaled_size.height as f32 * scale),
                        );
                        let info: ImageInfo = info.with_dimensions(scaled_size);

                        let mut crop_rect = IRect::from_size(scaled_size);
                        if do_crop {
                            let mut matrix = Matrix::rect_to_rect_or_identity(
                                Rect::from_iwh(unscaled_size.width, unscaled_size.height),
                                Rect::from_iwh(scaled_size.width, scaled_size.height),
                                None,
                            );
                            matrix.pre_concat(
                                &origin.to_matrix(unscaled_size.width, unscaled_size.height),
                            );
                            let crop_rect_float = Rect::from_irect(self.crop_rect);
                            let (crop_rect_float, _) = matrix.map_rect(crop_rect_float);
                            crop_rect = crop_rect_float.round_out();
                        }

                        let post_processor = if do_post_process {
                            post_processor(Rect::from_iwh(crop_rect.width(), crop_rect.height()))
                        } else {
                            None
                        };
                        let Some(animated_image) = AnimatedImage::make_with_info(
                            Some(android_codec),
                            &info,
                            crop_rect,
                            post_processor,
                        ) else {
                            return;
                        };
                        animated_image.set_repetition_count(0);

                        for _frame in 0..MAX_FRAMES {
                            {
                                let guard = AutoCanvasRestore::guard(canvas, do_crop);
                                if do_crop {
                                    guard.translate((crop_rect.left as f32, crop_rect.top as f32));
                                }
                                draw_proc(&guard, use_pic, &animated_image);
                            }

                            canvas.translate((self.translate as f32, 0.0));
                            let duration = animated_image.current_frame_duration();
                            if duration == FINISHED {
                                break;
                            }
                            for _ in 0..self.step {
                                animated_image.decode_next_frame();
                            }
                        }
                    }
                }
                canvas.restore();
                canvas.translate((0.0, self.translate as f32));
            }
        }
    }
}

// Port of the DEF_GM registrations at the end of gm/animated_image_orientation.cpp.
crate::def_gm!(
    AnimatedImageGM_stoplight_h_webp = "AnimatedImageGM(\"images/stoplight_h.webp\", \"stoplight\", 2, // Deliberately not centered in X or Y, and shows all three // lights, but otherwise arbitrary. SkIRect::MakeLTRB(5, 6, 11, 29))",
    AnimatedImageGm::new(
        "images/stoplight_h.webp",
        "stoplight",
        2,
        IRect::new(5, 6, 11, 29),
    )
);
crate::def_gm!(
    #[ignore = "see notes/gm_animated_image_orientation_cpp_AnimatedImageGM_flight.md"]
    AnimatedImageGM_flightAnim_gif = "AnimatedImageGM(\"images/flightAnim.gif\", \"flight\", 20, // Deliberately starts in the upper left corner to exercise // a special case, but otherwise arbitrary. SkIRect::MakeLTRB(0, 0, 300, 200))",
    AnimatedImageGm::new(
        "images/flightAnim.gif",
        "flight",
        20,
        IRect::new(0, 0, 300, 200),
    )
);
