// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skresources/src/SkAnimCodecPlayer.h, modules/skresources/src/
// SkAnimCodecPlayer.cpp (chrome/m156)
//
// Plays the frames of an animated codec (GIF, WebP, ...) by time code. A still image is one
// deferred image.

use skia_rust_codec::codec::{Codec, FrameInfo, NO_FRAME, Options, Result as CodecResult};
use skia_rust_codec::codecs;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::data::Data;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::size::ISize;
use skia_rust_raster::raster_canvas::RasterCanvas;

/// Plays an encoded image: an animation by frame, or a still image.
// Port of: modules/skresources/src/SkAnimCodecPlayer.h#L14-L60 (chrome/m156) (`class SkAnimCodecPlayer`)
#[doc(alias = "SkAnimCodecPlayer")]
pub struct AnimCodecPlayer {
    /// The codec; `None` for a still image, whose codec moved into its deferred image.
    codec: Option<Codec<'static>>,
    image_info: ImageInfo,
    /// The frames. After construction, `duration` is the end time of each frame, in milliseconds.
    frame_infos: Vec<FrameInfo>,
    /// The decoded frames, one per entry of `frame_infos`, or the still image.
    images: Vec<Option<Image>>,
    curr_index: usize,
    total_duration: u32,
}

impl AnimCodecPlayer {
    /// Takes the codec and reads its frames (`SkAnimCodecPlayer(std::unique_ptr<SkCodec>)`).
    // Port of: modules/skresources/src/SkAnimCodecPlayer.cpp#L14-L36 (chrome/m156) (`SkAnimCodecPlayer::SkAnimCodecPlayer`)
    #[must_use]
    pub fn new(mut codec: Codec<'static>) -> Self {
        let image_info = codec.info();
        let mut frame_infos = codec.frame_infos();
        let mut images = vec![None; frame_infos.len()];

        // Change the interpretation of the duration to an end time for that frame.
        let mut dur: u64 = 0;
        for frame in &mut frame_infos {
            dur += u64::try_from(frame.duration).unwrap_or(0);
            // The end times of a codec's frames fit in `int32_t`, as in Skia.
            #[allow(clippy::cast_possible_truncation)] // the cumulative duration fits in i32
            {
                frame.duration = dur as i32;
            }
        }
        // The total is kept as a 32-bit value, as in Skia.
        #[allow(clippy::cast_possible_truncation)] // the total is stored in uint32_t
        let total_duration = dur as u32;

        let codec = if total_duration == 0 {
            // Static image: it may or may not have returned a single frame info.
            frame_infos.clear();
            images.clear();
            images.push(codecs::deferred_image(Some(codec), None));
            None
        } else {
            Some(codec)
        };
        Self {
            codec,
            image_info,
            frame_infos,
            images,
            curr_index: 0,
            total_duration,
        }
    }

    /// The size of the images that [`AnimCodecPlayer::get_frame`] returns, with the EXIF
    /// orientation applied (`dimensions`).
    // Port of: modules/skresources/src/SkAnimCodecPlayer.cpp#L38-L47 (chrome/m156) (`SkAnimCodecPlayer::dimensions`)
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        match &self.codec {
            None => self
                .images
                .first()
                .and_then(Option::as_ref)
                .map_or(ISize::new(0, 0), Image::dimensions),
            Some(codec) => {
                if codec.origin().swaps_width_height() {
                    ISize::new(self.image_info.height(), self.image_info.width())
                } else {
                    ISize::new(self.image_info.width(), self.image_info.height())
                }
            }
        }
    }

    /// The total duration of the animation, in milliseconds; 0 for a still image
    /// (`duration`).
    // Port of: modules/skresources/src/SkAnimCodecPlayer.h#L32-L32 (chrome/m156) (`SkAnimCodecPlayer::duration`)
    #[must_use]
    pub fn duration(&self) -> u32 {
        self.total_duration
    }

    /// The image of the current frame. Calling this again without [`AnimCodecPlayer::seek`]
    /// returns the same image, or `None` if decoding failed (`getFrame`).
    // Port of: modules/skresources/src/SkAnimCodecPlayer.cpp#L162-L167 (chrome/m156) (`SkAnimCodecPlayer::getFrame`)
    pub fn get_frame(&mut self) -> Option<Image> {
        if self.total_duration > 0 {
            self.get_frame_at(self.curr_index)
        } else {
            self.images.first().cloned().flatten()
        }
    }

    /// Makes the frame at `msec` (taken modulo the duration) the current frame. Returns true if
    /// that changed the current frame (`seek`).
    // Port of: modules/skresources/src/SkAnimCodecPlayer.cpp#L169-L182 (chrome/m156) (`SkAnimCodecPlayer::seek`)
    pub fn seek(&mut self, msec: u32) -> bool {
        if self.total_duration == 0 {
            return false;
        }
        let msec = msec % self.total_duration;
        // std::lower_bound: the first frame whose end time is after `msec`.
        let lower = self
            .frame_infos
            .partition_point(|info| info.duration.cast_unsigned() <= msec);
        let prev_index = self.curr_index;
        self.curr_index = lower;
        self.curr_index != prev_index
    }

    /// The image of frame `index`, decoded on first use and cached (`getFrameAt`).
    // Port of: modules/skresources/src/SkAnimCodecPlayer.cpp#L49-L118 (chrome/m156) (`SkAnimCodecPlayer::getFrameAt`)
    fn get_frame_at(&mut self, index: usize) -> Option<Image> {
        if let Some(image) = &self.images[index] {
            return Some(image.clone());
        }
        let origin = self.codec.as_ref()?.origin();
        let oriented = self.dimensions();
        let origin_matrix = origin.to_matrix(oriented.width, oriented.height);
        let mut paint = Paint::default();
        paint.set_blend_mode(BlendMode::Src);

        let mut image_info = self.image_info.clone();
        let alpha_type = self.frame_infos[index].alpha_type;
        if alpha_type != AlphaType::Opaque && image_info.is_opaque() {
            image_info = image_info.with_alpha_type(AlphaType::Premul);
        }
        let mut rb = image_info.min_row_bytes();
        let mut pixels = vec![0_u8; image_info.compute_byte_size(rb)];

        let required_frame = self.frame_infos[index].required_frame;
        let mut opts = Options {
            frame_index: i32::try_from(index).unwrap_or(i32::MAX),
            prior_frame: NO_FRAME,
            ..Options::default()
        };
        let required_image = usize::try_from(required_frame)
            .ok()
            .and_then(|required| self.images.get(required))
            .and_then(Clone::clone);
        if let Some(required_image) = required_image {
            // Draw the required frame into the buffer, then decode the delta over it.
            if let Some(canvas) = Canvas::from_raster_direct(&image_info, &mut pixels, rb, None) {
                if origin != EncodedOrigin::TopLeft {
                    // The required frame is stored after applying the origin. Undo that,
                    // because the codec decodes prior to applying the origin.
                    if let Some(inverse) = origin_matrix.invert() {
                        canvas.concat(&inverse);
                    }
                }
                canvas.draw_image(&required_image, Point { x: 0.0, y: 0.0 }, Some(&paint));
            }
            opts.prior_frame = required_frame;
        }

        let codec = self.codec.as_mut()?;
        if codec.get_pixels(&image_info, &mut pixels, rb, Some(&opts)) != CodecResult::Success {
            return None;
        }
        let mut image = images::raster_from_data(&image_info, Data::new_copy(&pixels), rb);

        if origin != EncodedOrigin::TopLeft {
            image_info = image_info.with_dimensions(oriented);
            rb = image_info.min_row_bytes();
            let mut oriented_pixels = vec![0_u8; image_info.compute_byte_size(rb)];
            if let Some(canvas) =
                Canvas::from_raster_direct(&image_info, &mut oriented_pixels, rb, None)
            {
                canvas.concat(&origin_matrix);
                if let Some(decoded) = &image {
                    canvas.draw_image(decoded, Point { x: 0.0, y: 0.0 }, Some(&paint));
                }
            }
            image = images::raster_from_data(&image_info, Data::new_copy(&oriented_pixels), rb);
        }
        self.images[index].clone_from(&image);
        image
    }
}

impl std::fmt::Debug for AnimCodecPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnimCodecPlayer")
            .field("duration", &self.total_duration)
            .field("frames", &self.frame_infos.len())
            .field("current_frame", &self.curr_index)
            .finish_non_exhaustive()
    }
}
