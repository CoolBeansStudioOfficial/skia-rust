// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).
//
//! Port of `src/dec/idec_dec.c`: the incremental decoder behind `WebPIDecode`, `WebPIUpdate` and
//! `WebPIDecGetRGB`, which `SkWebpCodec` uses to decode a frame as its bytes arrive.
//!
//! The caller passes the whole data available so far on every call (map mode, `WebPIUpdate`).
//! The decoder keeps its position: the header state, the VP8 macroblock row and column (with the
//! partitions extended to the new data), and the rows emitted so far. A call returns
//! `Status::Suspended` when the data runs out and `Status::Ok` when the image is complete.
//!
//! Not ported yet: the VP8L (lossless) incremental path (`DecodeVP8LHeader`/`DecodeVP8LData`), and
//! the scaled output (`EmitRescaledRGB` works on whole frames here). Both report
//! `UnsupportedFeature` until they land, and the replay test covers the lossy, unscaled cases.

use crate::CspMode;
use crate::alpha::{self, AlphaDecoder};
use crate::io::{Io, Status};
use crate::output::{Emitter, batch_rows, bytes_per_pixel};
use crate::vp8_dec::{self, Crop, Planes, RowSink, Vp8Stream};
use crate::vp8_tables_small::K_FILTER_EXTRA_ROWS;
use crate::webp_dec::{self, DecodeOptions, IoParams};

/// `VP8_FRAME_HEADER_SIZE`: the bytes a VP8 frame header needs before `DecodeVP8FrameHeader`.
const VP8_FRAME_HEADER_SIZE: usize = vp8_dec::VP8_FRAME_HEADER_SIZE;

/// A non-negative size as a `usize`. The sizes here are validated before they are converted.
fn to_usize(v: i32) -> usize {
    usize::try_from(v).unwrap_or(0)
}

/// A row or size count as the `i32` the rest of the decoder uses (rows are far below `i32::MAX`).
fn to_i32(v: usize) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

/// Port of `DecState`. The order matters: `GetOutputBuffer` compares states with `<=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DecState {
    WebpHeader,
    Vp8Header,
    Vp8Parts0,
    Vp8Data,
    Vp8lHeader,
    Vp8lData,
    Done,
    Error,
}

/// The output of `WebPIDecGetRGB`: the pixels of the RGB buffer, the rows written so far and the
/// buffer's size.
#[derive(Debug, Clone, Copy)]
pub struct DecodedRgb<'a> {
    /// The output pixels, `stride` bytes per row. Rows at and past `last_y` are not written yet.
    pub pixels: &'a [u8],
    /// Bytes per row of `pixels`.
    pub stride: usize,
    /// The output width, which is the crop or scaled width.
    pub width: i32,
    /// The output height, which is the crop or scaled height.
    pub height: i32,
    /// `WebPDecParams::last_y`: the rows of output written so far.
    pub last_y: i32,
}

/// Port of `WebPIDecoder`. Positions are absolute offsets into the data passed to `update`.
#[derive(Debug)]
#[doc(alias = "WebPIDecoder")]
pub struct IDecoder {
    state: DecState,
    mode: CspMode,
    options: DecodeOptions,
    /// `idec->mem_.start_`: the first byte of the bitstream still needed.
    start: usize,
    /// `idec->chunk_size_`: the VP8/VP8L chunk size from the headers.
    chunk_size: usize,
    is_lossless: bool,
    /// `dec->alpha_data_`: the `ALPH` payload of a lossy image.
    alpha_data: Option<Vec<u8>>,
    /// The `ALPH` decoder, created when the first rows need alpha.
    alpha: Option<AlphaDecoder>,
    /// `dec->part0_size_`: the frame header and partition 0, which must be complete.
    part0_size: usize,
    /// The window and scaling of `WebPIoInitFromOptions`, from the image size.
    io_params: Option<IoParams>,
    stream: Option<Vp8Stream>,
    emitter: Emitter,
    /// The output buffer (`WebPAllocateDecBuffer`), allocated once partition 0 is read.
    output: Vec<u8>,
    output_allocated: bool,
    out_stride: usize,
    out_width: i32,
    out_height: i32,
    /// `WebPDecParams::last_y`.
    last_y: i32,
}

impl IDecoder {
    /// Port of `WebPIDecode` with a config (`WebPINewDecoder` with `output.colorspace = mode` and
    /// the decoder options): the output is owned by the decoder. Returns `None` for a colour
    /// space the RGB output does not support.
    #[doc(alias = "WebPIDecode")]
    #[must_use]
    pub fn new(mode: CspMode, options: DecodeOptions) -> Option<Self> {
        bytes_per_pixel(mode)?;
        Some(Self {
            state: DecState::WebpHeader,
            mode,
            options,
            start: 0,
            chunk_size: 0,
            is_lossless: false,
            alpha_data: None,
            alpha: None,
            part0_size: 0,
            io_params: None,
            stream: None,
            emitter: Emitter::default(),
            output: Vec::new(),
            output_allocated: false,
            out_stride: 0,
            out_width: 0,
            out_height: 0,
            last_y: 0,
        })
    }

    /// Port of `WebPIUpdate`: decodes with `data`, the whole input available so far (it begins
    /// with the bytes of every earlier call). Returns the status of `IDecode`, or of the decoder's
    /// final state once it is done or has failed.
    #[doc(alias = "WebPIUpdate")]
    pub fn update(&mut self, data: &[u8]) -> Status {
        match self.state {
            DecState::Error => return Status::BitstreamError,
            DecState::Done => return Status::Ok,
            _ => {}
        }
        self.i_decode(data)
    }

    /// Port of `WebPIDecGetRGB`: the output buffer and the rows written. `None` until the output
    /// is allocated.
    #[doc(alias = "WebPIDecGetRGB")]
    #[must_use]
    pub fn rgb(&self) -> Option<DecodedRgb<'_>> {
        if self.state <= DecState::Vp8Parts0 || !self.output_allocated {
            return None;
        }
        Some(DecodedRgb {
            pixels: &self.output,
            stride: self.out_stride,
            width: self.out_width,
            height: self.out_height,
            last_y: self.last_y,
        })
    }

    /// Port of `IDecError`: the decoder enters the error state and reports `status`.
    fn error(&mut self, status: Status) -> Status {
        self.state = DecState::Error;
        status
    }

    /// Port of `IDecode`: runs each state's step in turn, keeping the status of the last step.
    fn i_decode(&mut self, data: &[u8]) -> Status {
        let mut status = Status::Suspended;
        if self.state == DecState::WebpHeader {
            status = self.decode_webp_headers(data);
        }
        if self.state == DecState::Vp8Header {
            status = self.decode_vp8_frame_header(data);
        }
        if self.state == DecState::Vp8Parts0 {
            status = self.decode_partition0(data);
        }
        if self.state == DecState::Vp8Data {
            status = self.decode_remaining(data);
        }
        if self.state == DecState::Vp8lHeader {
            status = self.decode_vp8l_header(data);
        }
        if self.state == DecState::Vp8lData {
            status = self.decode_vp8l_data(data);
        }
        status
    }

    /// Port of `DecodeWebPHeaders`.
    fn decode_webp_headers(&mut self, data: &[u8]) -> Status {
        let headers = match webp_dec::parse_headers_partial(data) {
            Ok(h) => h,
            // We haven't found a VP8 chunk yet.
            Err(Status::NotEnoughData) => return Status::Suspended,
            Err(s) => return self.error(s),
        };
        self.chunk_size = headers.compressed_size;
        self.is_lossless = headers.is_lossless;
        // ChangeState(idec, STATE_..._HEADER, headers.offset): the memory starts at the bitstream.
        self.start = headers.offset;
        if self.is_lossless {
            self.state = DecState::Vp8lHeader;
        } else {
            self.alpha_data = headers.alpha_data.map(<[u8]>::to_vec);
            self.state = DecState::Vp8Header;
        }
        Status::Ok
    }

    /// Port of `DecodeVP8FrameHeader`.
    fn decode_vp8_frame_header(&mut self, data: &[u8]) -> Status {
        let frame = &data[self.start..];
        if frame.len() < VP8_FRAME_HEADER_SIZE {
            // Not enough data bytes to extract the VP8 frame header.
            return Status::Suspended;
        }
        if vp8_dec::get_info(frame, self.chunk_size).is_none() {
            return self.error(Status::BitstreamError);
        }
        let bits = u32::from(frame[0]) | (u32::from(frame[1]) << 8) | (u32::from(frame[2]) << 16);
        self.part0_size = (bits >> 5) as usize + VP8_FRAME_HEADER_SIZE;
        self.state = DecState::Vp8Parts0;
        Status::Ok
    }

    /// Port of `DecodePartition0`: parses the frame header and sets up the output buffer.
    fn decode_partition0(&mut self, data: &[u8]) -> Status {
        let frame = &data[self.start..];
        // Wait till we have enough data for the whole partition #0.
        if frame.len() < self.part0_size {
            return Status::Suspended;
        }
        let options = self.options;
        let mode = self.mode;
        let mut params = None;
        let stream = Vp8Stream::new(frame, |width, height| {
            let p = webp_dec::io_params(width, height, &options, true)?;
            let (out_w, out_h) = p.output_size();
            if out_w <= 0 || out_h <= 0 || bytes_per_pixel(mode).is_none() {
                return Err(Status::InvalidParam);
            }
            let crop = Crop {
                left: p.x,
                top: p.y,
                right: p.x + p.w,
                bottom: p.y + p.h,
            };
            let bypass = p.bypass_filtering;
            params = Some(p);
            Ok((crop, bypass))
        });
        let stream = match stream {
            Ok(s) => s,
            Err(Status::NotEnoughData | Status::Suspended) => return Status::Suspended,
            Err(s) => return self.error(s),
        };
        let Some(p) = params else {
            return self.error(Status::BitstreamError);
        };
        let (out_w, out_h) = p.output_size();
        let bpp = bytes_per_pixel(mode).unwrap_or(0);
        self.out_stride = to_usize(out_w) * bpp;
        self.out_width = out_w;
        self.out_height = out_h;
        self.output = vec![0; self.out_stride * to_usize(out_h)];
        self.output_allocated = true;
        self.io_params = Some(p);
        self.stream = Some(stream);
        self.state = DecState::Vp8Data;
        Status::Ok
    }

    /// Port of `DecodeRemaining`: the macroblock rows from where the last call stopped, with the
    /// batches emitted by `FinishRow`.
    fn decode_remaining(&mut self, data: &[u8]) -> Status {
        let frame = &data[self.start..];
        let Some(p) = self.io_params else {
            return self.error(Status::BitstreamError);
        };
        if p.use_scaling {
            // The rescaler works on whole frames (see the module note).
            return self.error(Status::UnsupportedFeature);
        }
        let Some(stream) = self.stream.as_mut() else {
            return self.error(Status::BitstreamError);
        };
        let mut io = Io::new(
            &mut self.output,
            self.out_stride,
            self.mode,
            stream.size().0,
            stream.size().1,
        );
        io.use_cropping = self.options.crop.is_some();
        io.crop_left = p.x;
        io.crop_top = p.y;
        io.crop_right = p.x + p.w;
        io.crop_bottom = p.y + p.h;
        io.mb_w = p.w;
        io.mb_h = p.h;
        io.use_scaling = false;
        io.fancy_upsampling = p.fancy_upsampling;
        io.bypass_filtering = p.bypass_filtering;
        io.last_y = self.last_y;
        let mut sink = LossySink {
            io,
            emitter: &mut self.emitter,
            alpha: &mut self.alpha,
            alpha_data: self.alpha_data.as_deref(),
            window: (p.x, p.x + p.w, p.y, p.y + p.h),
        };
        let result = stream.decode_rows(frame, &mut sink);
        self.last_y = sink.io.last_y;
        match result {
            Ok(true) => {
                // FinishDecoding
                self.state = DecState::Done;
                Status::Ok
            }
            Ok(false) => Status::Suspended,
            Err(s) => self.error(s),
        }
    }

    /// Lossless headers. Not ported yet (see the module note).
    fn decode_vp8l_header(&mut self, _data: &[u8]) -> Status {
        self.error(Status::UnsupportedFeature)
    }

    /// Lossless data. Not ported yet (see the module note).
    fn decode_vp8l_data(&mut self, _data: &[u8]) -> Status {
        self.error(Status::UnsupportedFeature)
    }
}

/// The output half of `FinishRow` for lossy frames: the `io->put` of `io_dec.c`, with the alpha
/// rows decoded for each batch (`VP8DecompressAlphaRows`).
struct LossySink<'a> {
    io: Io<'a>,
    emitter: &'a mut Emitter,
    alpha: &'a mut Option<AlphaDecoder>,
    alpha_data: Option<&'a [u8]>,
    /// `(left, right, top, bottom)` of the crop window, as `ALPHInit` takes it.
    window: (i32, i32, i32, i32),
}

impl RowSink for LossySink<'_> {
    fn finish_row(
        &mut self,
        planes: &Planes,
        mb_y: usize,
        is_last_row: bool,
    ) -> Result<(), Status> {
        let extra = to_usize(K_FILTER_EXTRA_ROWS[usize::from(planes.filter_type)]);
        let (y_start, y_end) = batch_rows(mb_y, is_last_row, extra, to_usize(self.io.crop_bottom));
        let mut alpha_rows = None;
        if let Some(alph) = self.alpha_data {
            if y_start < y_end {
                if self.alpha.is_none() {
                    *self.alpha = Some(
                        alpha::alpha_init(alph, planes.width, planes.height, self.window)
                            .ok_or(Status::BitstreamError)?,
                    );
                }
                let dec = self.alpha.as_mut().ok_or(Status::BitstreamError)?;
                if !alpha::alpha_decode(dec, to_i32(y_start), to_i32(y_end - y_start)) {
                    return Err(Status::BitstreamError); // "Could not decode alpha data."
                }
            }
            alpha_rows = self.alpha.as_ref().map(AlphaDecoder::plane);
        }
        self.emitter
            .emit_batch(planes, alpha_rows, &mut self.io, mb_y, is_last_row);
        Ok(())
    }
}
