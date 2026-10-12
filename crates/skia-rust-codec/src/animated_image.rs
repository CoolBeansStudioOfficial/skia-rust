// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/android/SkAnimatedImage.cpp (chrome/m156), and include/android/SkAnimatedImage.h
// Ported from: src/android/SkAnimatedImage.cpp, include/android/SkAnimatedImage.h

//! [`AnimatedImage`]: a drawable that plays an animated image, such as a GIF or WebP, from an
//! [`AndroidCodec`] (`SkAnimatedImage`).
//!
//! The image decodes one frame at a time into one of three frames: the frame on screen, the frame
//! being decoded, and a frame kept for a later `kRestorePrevious` disposal. A frame whose
//! pixels are still referenced (by an image or picture handed out earlier) is copied before it
//! is written, so those handles keep the pixels they were given.
//!
//! Skia's `SkAnimatedImage` is thread-unsafe. Here the decoding state lives behind a [`Mutex`]
//! so the drawable can be shared as a [`Drawable`]. Its `on*` methods lock the state; the
//! methods that reach the drawable from inside a locked method use the state directly, so no
//! method locks twice.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{AutoCanvasRestore, Canvas, SaveLayerRec};
use skia_rust_core::drawable::{Drawable, DrawableBase};
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_raster::CopyPixelsMode;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::picture::Picture;
use skia_rust_core::rect::{Contains, IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::size::ISize;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::android_codec::{AndroidCodec, AndroidOptions};
use crate::codec::{
    FrameInfo, NO_FRAME, Options, REPETITION_COUNT_INFINITE, Result as CodecResult,
};
use crate::codec_animation::DisposalMethod;
use crate::pixmap_utils::swap_width_height;

/// Port of `SkAnimatedImage::kFinished`: returned by [`AnimatedImage::decode_next_frame`] and
/// [`AnimatedImage::current_frame_duration`] when the animation is not running.
#[doc(alias = "kFinished")]
pub const FINISHED: i32 = -1;

// Port of: src/android/SkAnimatedImage.cpp#L162-L164 (chrome/m156)
fn is_restore_previous(dispose: DisposalMethod) -> bool {
    DisposalMethod::RestorePrevious == dispose
}

// How `Frame::init` treats the pixels of a frame that another handle still holds.
// Port of: include/android/SkAnimatedImage.h (Frame::OnInit)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnInit {
    // Restore the image from the old pixel ref to the new one.
    RestoreIfNecessary,
    // No need to restore.
    NoRestore,
}

// One decoded frame: its pixels, which frame they are, and how the frame is disposed.
// Port of: include/android/SkAnimatedImage.h (SkAnimatedImage::Frame)
#[derive(Debug)]
struct Frame {
    bitmap: Bitmap,
    index: i32,
    disposal_method: DisposalMethod,
}

impl Frame {
    // Port of: src/android/SkAnimatedImage.cpp#L112-L114 (Frame::Frame)
    fn new() -> Self {
        Self {
            bitmap: Bitmap::new(),
            index: NO_FRAME,
            disposal_method: DisposalMethod::Keep,
        }
    }

    // Port of: src/android/SkAnimatedImage.cpp#L116-L140 (Frame::init)
    fn init(&mut self, info: &ImageInfo, on_init: OnInit) -> bool {
        if self.bitmap.peek_pixels().is_some() {
            if self.bitmap.pixel_ref_is_unique() {
                assert!(self.bitmap.set_alpha_type(info.alpha_type()));
                return true;
            }

            // An SkCanvas provided to onDraw is still holding a reference. Copy before we decode
            // to ensure that we don't overwrite the expected contents of the image.
            if OnInit::RestoreIfNecessary == on_init {
                let mut tmp = Bitmap::new();
                if !tmp.try_alloc_pixels_info(info, None) {
                    return false;
                }
                copy_pixels(&self.bitmap, &mut tmp);
                std::mem::swap(&mut tmp, &mut self.bitmap);
                return true;
            }
        }

        self.bitmap.try_alloc_pixels_info(info, None)
    }

    // Port of: src/android/SkAnimatedImage.cpp#L142-L151 (Frame::copyTo)
    fn copy_to(&self, dst: &mut Frame) -> bool {
        if !dst.init(self.bitmap.info(), OnInit::NoRestore) {
            return false;
        }

        copy_pixels(&self.bitmap, &mut dst.bitmap);
        dst.index = self.index;
        dst.disposal_method = self.disposal_method;
        true
    }
}

// The `memcpy(dst, src, computeByteSize())` of Frame::init and Frame::copyTo. `dst` has the same
// info as `src` (the callers init it with `src`'s info first).
// Port of: src/android/SkAnimatedImage.cpp#L116-L151 (the memcpy calls)
fn copy_pixels(src: &Bitmap, dst: &mut Bitmap) {
    let n = src.compute_byte_size();
    let (Some(src_pm), Some(mut dst_pm)) = (src.peek_pixels(), dst.peek_pixels_mut()) else {
        return;
    };
    let (Some(src_bytes), Some(dst_bytes)) = (src_pm.bytes(), dst_pm.bytes_mut()) else {
        return;
    };
    dst_bytes[..n].copy_from_slice(&src_bytes[..n]);
}

// The decoding state, behind the mutex. Port of the members of SkAnimatedImage.
#[derive(Debug)]
struct State {
    // Port of `fCodec`.
    codec: AndroidCodec<'static>,
    // Port of `fDecodeInfo`.
    decode_info: ImageInfo,
    // Port of `fCropRect`.
    crop_rect: IRect,
    // Port of `fPostProcess`.
    post_process: Option<Picture>,
    // Port of `fFrameCount`.
    frame_count: i32,
    // Port of `fMatrix`: crop, orientation and scale, applied in that order.
    matrix: Matrix,
    // Port of `fSampleSize`.
    sample_size: i32,
    // Port of `fFinished`.
    finished: bool,
    // Port of `fCurrentFrameDuration`.
    current_frame_duration: i32,
    // Port of `fDisplayFrame`.
    display_frame: Frame,
    // Port of `fDecodingFrame`.
    decoding_frame: Frame,
    // Port of `fRestoreFrame`.
    restore_frame: Frame,
    // Port of `fRepetitionCount`.
    repetition_count: i32,
    // Port of `fRepetitionsCompleted`.
    repetitions_completed: i32,
    // Port of `fFilterMode`.
    filter_mode: FilterMode,
}

// Port of: src/android/SkAnimatedImage.cpp#L57-L104 (the SkAnimatedImage constructor)
impl State {
    // The `int` to `float` conversions of the constructor are exact for the sizes of images:
    // a dimension is an `int` well inside the float mantissa.
    #[allow(clippy::cast_precision_loss)] // mirrors the int-to-float conversions of the C++ code
    fn new(
        codec: AndroidCodec<'static>,
        requested_info: &ImageInfo,
        crop_rect: IRect,
        post_process: Option<Picture>,
    ) -> Self {
        let mut codec = codec;
        let frame_count = codec.codec_mut().get_frame_count();
        let repetition_count = codec.codec_mut().get_repetition_count();
        let mut state = Self {
            codec,
            decode_info: requested_info.clone(),
            crop_rect,
            post_process,
            frame_count,
            matrix: Matrix::new_identity(),
            sample_size: 1,
            finished: false,
            current_frame_duration: 0,
            display_frame: Frame::new(),
            decoding_frame: Frame::new(),
            restore_frame: Frame::new(),
            repetition_count,
            repetitions_completed: 0,
            filter_mode: FilterMode::Linear,
        };

        let mut scaled_size = requested_info.dimensions();

        // For simplicity in decoding and compositing frames, decode directly to a size and
        // orientation that fCodec can do directly, and then use fMatrix to handle crop (along with
        // a clip), orientation, and scaling outside of fCodec. The matrices are computed
        // individually and applied in the following order:
        //      [crop] X [origin] X [scale]
        let origin = state.codec.codec().origin();
        if origin != EncodedOrigin::DEFAULT {
            // The origin is applied after scaling, so use scaledSize, which is the final scaled
            // size.
            state.matrix = origin.to_matrix(scaled_size.width, scaled_size.height);

            if origin.swaps_width_height() {
                // The client asked for sizes post-rotation. Swap back to the pre-rotation sizes to
                // pass to fCodec and for the scale matrix computation.
                state.decode_info = swap_width_height(&state.decode_info);
                scaled_size = ISize::new(scaled_size.height, scaled_size.width);
            }
        }

        let mut decode_size = scaled_size;
        state.sample_size = state.codec.compute_sample_size(&mut decode_size);
        state.decode_info = state.decode_info.with_dimensions(decode_size);

        if !state
            .decoding_frame
            .bitmap
            .try_alloc_pixels_info(&state.decode_info, None)
        {
            return state;
        }

        if scaled_size != state.decode_info.dimensions() {
            let scale_x = scaled_size.width as f32 / state.decode_info.width() as f32;
            let scale_y = scaled_size.height as f32 / state.decode_info.height() as f32;
            state.matrix.pre_concat(&Matrix::scale((scale_x, scale_y)));
        }
        state.matrix.post_concat(&Matrix::translate((
            (-state.crop_rect.left) as f32,
            (-state.crop_rect.top) as f32,
        )));
        state.decode_next_frame();
        state
    }

    // Port of: src/android/SkAnimatedImage.cpp#L166-L182 (computeNextFrame)
    fn compute_next_frame(&mut self, current: i32, animation_ended: &mut bool) -> i32 {
        *animation_ended = false;

        let frame_to_decode = current + 1;
        if frame_to_decode == self.frame_count - 1 {
            // Final frame. Check to determine whether to stop.
            self.repetitions_completed += 1;
            if self.repetition_count != REPETITION_COUNT_INFINITE
                && self.repetitions_completed > self.repetition_count
            {
                *animation_ended = true;
            }
        } else if frame_to_decode == self.frame_count {
            return 0;
        }
        frame_to_decode
    }

    // Port of: src/android/SkAnimatedImage.cpp#L184-L188 (finish)
    fn finish(&mut self) -> i32 {
        self.finished = true;
        self.current_frame_duration = FINISHED;
        FINISHED
    }

    // Port of: src/android/SkAnimatedImage.cpp#L190-L334 (decodeNextFrame)
    // The C++ function is long too (L190-L334); it is kept whole so each step can be checked against it.
    #[allow(clippy::too_many_lines)]
    fn decode_next_frame(&mut self) -> i32 {
        if self.finished {
            return FINISHED;
        }

        let mut animation_ended = false;
        let frame_to_decode =
            self.compute_next_frame(self.display_frame.index, &mut animation_ended);

        let mut frame_info = FrameInfo::default();
        if let Some(info) = self.codec.codec().get_frame_info(frame_to_decode) {
            frame_info = info;
            if !frame_info.fully_received {
                // SkCodecPrintf("Frame %i not fully received\n", frameToDecode)
                return self.finish();
            }

            self.current_frame_duration = frame_info.duration;
        } else {
            animation_ended = true;
            if 0 == frame_to_decode {
                // Static image. This is okay.
                frame_info.required_frame = NO_FRAME;
                frame_info.alpha_type = self.codec.info().alpha_type();
                frame_info.disposal_method = DisposalMethod::Keep;
                // These fields won't be read.
                frame_info.duration = i32::MAX;
                frame_info.fully_received = true;
                self.current_frame_duration = FINISHED;
            } else {
                // SkCodecPrintf("Error getting frameInfo for frame %i\n", frameToDecode)
                return self.finish();
            }
        }

        if frame_to_decode == self.display_frame.index {
            if animation_ended {
                return self.finish();
            }
            return self.current_frame_duration;
        }

        // for (Frame* frame : { &fRestoreFrame, &fDecodingFrame }) { ... }
        if frame_to_decode == self.restore_frame.index {
            std::mem::swap(&mut self.display_frame, &mut self.restore_frame);
            if animation_ended {
                return self.finish();
            }
            return self.current_frame_duration;
        }
        if frame_to_decode == self.decoding_frame.index {
            std::mem::swap(&mut self.display_frame, &mut self.decoding_frame);
            if animation_ended {
                return self.finish();
            }
            return self.current_frame_duration;
        }

        // The following code makes an effort to avoid overwriting a frame that will be used
        // again. If frame |i| is_restore_previous, frame |i+1| will not depend on frame |i|, so do
        // not overwrite frame |i-1|, which may be needed for frame |i+1|.
        // We could be even smarter about which frames to save by looking at the entire dependency
        // chain.
        let mut options = AndroidOptions {
            base: Options {
                frame_index: frame_to_decode,
                ..Options::default()
            },
            sample_size: self.sample_size,
        };
        if frame_info.required_frame == NO_FRAME {
            if is_restore_previous(frame_info.disposal_method) {
                // frameToDecode will be discarded immediately after drawing, so do not overwrite a
                // frame which could possibly be used in the future.
                if self.decoding_frame.index != NO_FRAME
                    && !is_restore_previous(self.decoding_frame.disposal_method)
                {
                    std::mem::swap(&mut self.decoding_frame, &mut self.restore_frame);
                }
            }
        } else {
            let required_frame = frame_info.required_frame;
            let valid_prior_frame = |frame: &Frame| {
                if NO_FRAME == frame.index || is_restore_previous(frame.disposal_method) {
                    return false;
                }

                frame.index >= required_frame && frame.index < frame_to_decode
            };
            if valid_prior_frame(&self.decoding_frame) {
                if is_restore_previous(frame_info.disposal_method) {
                    // fDecodingFrame is a good frame to use for this one, but we don't want to
                    // overwrite it.
                    self.decoding_frame.copy_to(&mut self.restore_frame);
                }
                options.base.prior_frame = self.decoding_frame.index;
            } else if valid_prior_frame(&self.display_frame) {
                if !self.display_frame.copy_to(&mut self.decoding_frame) {
                    // SkCodecPrintf("Failed to allocate pixels for frame\n")
                    return self.finish();
                }
                options.base.prior_frame = self.decoding_frame.index;
            } else if valid_prior_frame(&self.restore_frame) {
                if !is_restore_previous(frame_info.disposal_method) {
                    std::mem::swap(&mut self.decoding_frame, &mut self.restore_frame);
                } else if !self.restore_frame.copy_to(&mut self.decoding_frame) {
                    // SkCodecPrintf("Failed to restore frame\n")
                    return self.finish();
                }
                options.base.prior_frame = self.decoding_frame.index;
            }
        }

        let alpha_type = if AlphaType::Opaque == frame_info.alpha_type {
            AlphaType::Opaque
        } else {
            AlphaType::Premul
        };
        let info = self.decode_info.with_alpha_type(alpha_type);
        if !self.decoding_frame.init(&info, OnInit::RestoreIfNecessary) {
            return self.finish();
        }

        let dst_info = self.decoding_frame.bitmap.info().clone();
        let row_bytes = self.decoding_frame.bitmap.row_bytes();
        let result = {
            let Some(mut dst_pm) = self.decoding_frame.bitmap.peek_pixels_mut() else {
                return self.finish();
            };
            let Some(dst_pixels) = dst_pm.bytes_mut() else {
                return self.finish();
            };
            self.codec
                .get_android_pixels(&dst_info, dst_pixels, row_bytes, Some(&options))
        };
        if result != CodecResult::Success {
            // SkCodecPrintf("%s, frame %i of %i\n", ...)
            return self.finish();
        }

        self.decoding_frame.index = frame_to_decode;
        self.decoding_frame.disposal_method = frame_info.disposal_method;

        std::mem::swap(&mut self.decoding_frame, &mut self.display_frame);
        self.display_frame.bitmap.notify_pixels_changed();

        if animation_ended {
            return self.finish();
        } else if self.codec.encoded_format() == EncodedImageFormat::HEIF {
            // HEIF doesn't know the frame duration until after decoding. Update to the correct
            // value. Note that earlier returns in this method either return kFinished, or
            // fCurrentFrameDuration. If they return the latter, it is a frame that was previously
            // decoded, so it has the updated value.
            // SkCodecPrintf("Failed to getFrameInfo on second attempt (HEIF)") if this fails.
            if let Some(info) = self.codec.codec().get_frame_info(frame_to_decode) {
                self.current_frame_duration = info.duration;
            }
        }
        self.current_frame_duration
    }

    // Port of: src/android/SkAnimatedImage.cpp#L153-L160 (reset)
    fn reset(&mut self) {
        self.finished = false;
        self.repetitions_completed = 0;
        if self.display_frame.index != 0 {
            self.display_frame.index = NO_FRAME;
            self.decode_next_frame();
        }
    }

    // Port of: include/android/SkAnimatedImage.h (simple): true if there is no crop, orientation,
    // or post decoding scaling.
    fn simple(&self) -> bool {
        self.matrix.is_identity()
            && self.post_process.is_none()
            && self.crop_rect == self.decode_info.bounds()
    }

    // Port of: src/android/SkAnimatedImage.cpp#L364-L369 (getCurrentFrameSimple)
    fn get_current_frame_simple(&self) -> Option<Image> {
        // This SkBitmap may be reused later to decode the following frame. But Frame::init lazily
        // copies the pixel ref if it has any other references. So it is safe to not do a deep copy
        // here.
        skia_rust_core::images::make_image_from_raster_bitmap(
            &self.display_frame.bitmap,
            CopyPixelsMode::Never,
        )
    }

    // Port of: src/android/SkAnimatedImage.cpp#L371-L389 (getCurrentFrame)
    fn get_current_frame(&self) -> Option<Image> {
        if self.simple() {
            return self.get_current_frame_simple();
        }

        let mut image_info = self
            .display_frame
            .bitmap
            .info()
            .with_dimensions(self.crop_rect.size());
        if self.post_process.is_some() {
            // Defensively use premul in case the post process adds alpha.
            image_info = image_info.with_alpha_type(AlphaType::Premul);
        }

        let mut dst = Bitmap::new();
        if !dst.try_alloc_pixels_info(&image_info, None) {
            return None;
        }

        {
            let canvas = Canvas::from_bitmap(&mut dst, None)?;
            // SkDrawable::draw(canvas): save, onDraw, restore.
            let count = canvas.save();
            self.on_draw(&canvas);
            canvas.restore_to_count(count);
        }
        dst.set_immutable();
        skia_rust_core::images::make_image_from_raster_bitmap(&dst, CopyPixelsMode::Never)
    }

    // Port of: src/android/SkAnimatedImage.cpp#L336-L358 (onDraw)
    fn on_draw(&self, canvas: &Canvas) {
        let image = self.get_current_frame_simple();

        if self.simple() {
            if let Some(image) = image {
                canvas.draw_image_with_sampling_options(
                    &image,
                    (0.0, 0.0),
                    SamplingOptions::from(self.filter_mode),
                    None,
                );
            }
            return;
        }

        let bounds = Rect::from_iwh(self.crop_rect.width(), self.crop_rect.height());
        let post_process = self.post_process.as_ref();
        if post_process.is_some() {
            let rec = SaveLayerRec::default().bounds(&bounds);
            canvas.save_layer(&rec);
        }
        canvas.clip_rect(bounds, None, None);
        {
            let guard = AutoCanvasRestore::guard(canvas, post_process.is_some());
            guard.concat(&self.matrix);
            if let Some(image) = image {
                guard.draw_image_with_sampling_options(
                    &image,
                    (0.0, 0.0),
                    SamplingOptions::from(self.filter_mode),
                    None,
                );
            }
        }
        if let Some(post_process) = post_process {
            canvas.draw_picture(post_process, None, None);
            canvas.restore();
        }
    }
}

/// The drawable half of an [`AnimatedImage`]: the `on*` virtuals of `SkAnimatedImage`, sharing
/// its state.
#[derive(Debug)]
struct AnimatedImageDrawable {
    state: Arc<Mutex<State>>,
}

impl DrawableBase for AnimatedImageDrawable {
    // Port of: src/android/SkAnimatedImage.cpp#L108-L110 (onGetBounds)
    fn on_get_bounds(&self) -> Rect {
        let state = lock(&self.state);
        Rect::from_iwh(state.crop_rect.width(), state.crop_rect.height())
    }

    // Port of: src/android/SkAnimatedImage.cpp#L336-L358 (onDraw)
    fn on_draw(&self, canvas: &Canvas) {
        lock(&self.state).on_draw(canvas);
    }
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// An animated image drawable, such as a GIF or WebP that plays frame by frame (`SkAnimatedImage`).
///
/// Clones share the animation, like `sk_sp<SkAnimatedImage>`.
#[derive(Clone, Debug)]
#[doc(alias = "SkAnimatedImage")]
pub struct AnimatedImage {
    state: Arc<Mutex<State>>,
}

impl AnimatedImage {
    /// Creates an animated image from `codec` with the default size, no cropping and no
    /// post-processing (`SkAnimatedImage::Make(std::unique_ptr<SkAndroidCodec>)`). `None` if
    /// there is no codec, or the first frame cannot be decoded (no pixels could be allocated).
    // Port of: src/android/SkAnimatedImage.cpp#L43-L55 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(codec: Option<AndroidCodec<'static>>) -> Option<Self> {
        let codec = codec?;
        let mut decode_info = codec.info();
        let origin = codec.codec().origin();
        if origin.swaps_width_height() {
            decode_info = decode_info.with_wh(decode_info.height(), decode_info.width());
        }
        let crop_rect = IRect::from_size(decode_info.dimensions());
        Self::make_with_info(Some(codec), &decode_info, crop_rect, None)
    }

    /// Creates an animated image from `codec` that decodes to `info` (whose width and height may
    /// require scaling), crops to `crop_rect` after scaling, and then applies `post_process`
    /// (`SkAnimatedImage::Make(std::unique_ptr<SkAndroidCodec>, info, cropRect, postProcess)`).
    ///
    /// Returns `None` if there is no codec, `crop_rect` is not inside `info`'s bounds, or the first
    /// frame cannot be decoded.
    // Port of: src/android/SkAnimatedImage.cpp#L23-L41 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make_with_info(
        codec: Option<AndroidCodec<'static>>,
        info: &ImageInfo,
        crop_rect: IRect,
        post_process: Option<Picture>,
    ) -> Option<Self> {
        let codec = codec?;
        if !info.bounds().contains(crop_rect) {
            return None;
        }

        let state = State::new(codec, info, crop_rect, post_process);
        // No pixels: tryAllocPixels failed.
        state.display_frame.bitmap.peek_pixels()?;

        Some(Self {
            state: Arc::new(Mutex::new(state)),
        })
    }

    /// The drawable that this animated image draws (`SkAnimatedImage` as an `SkDrawable`).
    #[must_use]
    pub fn drawable(&self) -> Drawable {
        Drawable::new(Arc::new(AnimatedImageDrawable {
            state: Arc::clone(&self.state),
        }))
    }

    /// `SkDrawable::draw(canvas)`: draws the current frame, saving and restoring the canvas.
    pub fn draw(&self, canvas: &Canvas) {
        self.drawable().draw(canvas, None);
    }

    /// `SkDrawable::getBounds`: the crop rectangle at the origin.
    #[doc(alias = "getBounds")]
    #[must_use]
    pub fn bounds(&self) -> Rect {
        self.drawable().bounds()
    }

    /// `SkDrawable::makePictureSnapshot`: the drawable recorded into a picture whose cull rect is
    /// its bounds. `None` if the recording fails.
    #[doc(alias = "makePictureSnapshot")]
    #[must_use]
    pub fn make_picture_snapshot(&self) -> Option<Picture> {
        self.drawable().make_picture_snapshot()
    }

    /// Resets the animation to the beginning (`reset`).
    // Port of: src/android/SkAnimatedImage.cpp#L153-L160 (chrome/m156)
    pub fn reset(&self) {
        lock(&self.state).reset();
    }

    /// Whether the animation completed. Returns true after all repetitions are complete, or an
    /// error stops the animation. Gets reset to false if the animation is restarted (`isFinished`).
    #[doc(alias = "isFinished")]
    #[must_use]
    pub fn is_finished(&self) -> bool {
        lock(&self.state).finished
    }

    /// Decodes the next frame. If the animation is on the last frame or has hit an error, returns
    /// [`FINISHED`] (`decodeNextFrame`).
    // Port of: src/android/SkAnimatedImage.cpp#L190-L334 (chrome/m156)
    #[doc(alias = "decodeNextFrame")]
    // The result is often ignored (the animation is driven by the caller), as in C++.
    #[allow(clippy::must_use_candidate)]
    pub fn decode_next_frame(&self) -> i32 {
        lock(&self.state).decode_next_frame()
    }

    /// The current frame as an image, which does not change after it has been returned. `None` if
    /// there is no current frame (`getCurrentFrame`).
    #[doc(alias = "getCurrentFrame")]
    #[must_use]
    pub fn get_current_frame(&self) -> Option<Image> {
        lock(&self.state).get_current_frame()
    }

    /// How long to display the current frame, in milliseconds. Useful for the first frame, for
    /// which [`Self::decode_next_frame`] is called internally (`currentFrameDuration`).
    #[doc(alias = "currentFrameDuration")]
    #[must_use]
    pub fn current_frame_duration(&self) -> i32 {
        lock(&self.state).current_frame_duration
    }

    /// Changes the repetition count. By default, the image repeats the number of times indicated
    /// in the encoded data. Use [`REPETITION_COUNT_INFINITE`] for infinite, and 0 to show all
    /// frames once and then stop (`setRepetitionCount`).
    #[doc(alias = "setRepetitionCount")]
    pub fn set_repetition_count(&self, count: i32) {
        lock(&self.state).repetition_count = count;
    }

    /// The currently set repetition count (`getRepetitionCount`).
    #[doc(alias = "getRepetitionCount")]
    #[must_use]
    pub fn repetition_count(&self) -> i32 {
        lock(&self.state).repetition_count
    }

    /// The total number of frames in the animation (`getFrameCount`).
    #[doc(alias = "getFrameCount")]
    #[must_use]
    pub fn frame_count(&self) -> i32 {
        lock(&self.state).frame_count
    }

    /// Changes the filter mode used when drawing. By default, bilinear (`setFilterMode`).
    #[doc(alias = "setFilterMode")]
    pub fn set_filter_mode(&self, filter_mode: FilterMode) {
        lock(&self.state).filter_mode = filter_mode;
    }

    /// The filter mode used for sampling when drawing (`getFilterMode`).
    #[doc(alias = "getFilterMode")]
    #[must_use]
    pub fn filter_mode(&self) -> FilterMode {
        lock(&self.state).filter_mode
    }
}
