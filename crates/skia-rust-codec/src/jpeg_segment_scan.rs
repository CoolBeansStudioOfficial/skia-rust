// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkJpegSegmentScan.cpp and src/codec/SkJpegSegmentScan.h (chrome/m156).
//
// A state machine over the bytes of a JPEG file that records the position, marker and parameter
// length of every marker segment, up to the stop marker (EndOfImage, or StartOfScan for the
// header). The `SkCodecPrintf` diagnostics are not ported: they are debug output only.

use skia_rust_core::data::Data;

use crate::jpeg_constants::{
    JPEG_MARKER_CODE_SIZE, JPEG_MARKER_START_OF_IMAGE, JPEG_SEGMENT_PARAMETER_LENGTH_SIZE,
};

/// A marker segment in a JPEG file (`SkJpegSegment`).
// Port of: src/codec/SkJpegSegmentScan.h#L19-L23 (chrome/m156)
#[doc(alias = "SkJpegSegment")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JpegSegment {
    /// The offset of the 0xFF that starts the marker, from the start of the file.
    pub offset: usize,
    /// The marker code after 0xFF.
    pub marker: u8,
    /// The length of the segment's parameters, including the two length bytes.
    pub parameter_length: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    StartOfImageByte0,
    StartOfImageByte1,
    SecondMarkerByte0,
    SecondMarkerByte1,
    SegmentParamLengthByte0,
    SegmentParamLengthByte1,
    SegmentParam,
    EntropyCodedData,
    EntropyCodedDataSentinel,
    PostEntropyCodedDataFill,
    Done,
    Error,
}

/// Scans JPEG bytes and records their marker segments (`SkJpegSegmentScanner`).
// Port of: src/codec/SkJpegSegmentScan.h#L25-L66 (chrome/m156)
#[doc(alias = "SkJpegSegmentScanner")]
#[derive(Debug)]
pub struct JpegSegmentScanner {
    state: State,
    stop_marker: u8,
    offset: usize,
    segment_param_length_byte0: u8,
    segment_param_bytes_remaining: usize,
    current_segment_offset: usize,
    current_segment_marker: u8,
    segments: Vec<JpegSegment>,
}

// Port of: src/codec/SkJpegSegmentScan.cpp#L24-L26 (chrome/m156), `MarkerStandsAlone`.
fn marker_stands_alone(marker: u8) -> bool {
    marker == 0x01 || (0xD0..=0xD9).contains(&marker)
}

impl JpegSegmentScanner {
    /// A scanner that stops at `stop_marker` (`SkJpegSegmentScanner(uint8_t stopMarker)`).
    // Port of: src/codec/SkJpegSegmentScan.cpp#L28 (chrome/m156)
    #[must_use]
    pub fn new(stop_marker: u8) -> Self {
        Self {
            state: State::StartOfImageByte0,
            stop_marker,
            offset: 0,
            segment_param_length_byte0: 0,
            segment_param_bytes_remaining: 0,
            current_segment_offset: 0,
            current_segment_marker: 0,
            segments: Vec::new(),
        }
    }

    /// Whether the stop marker has been reached.
    // Port of: src/codec/SkJpegSegmentScan.h (isDone)
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.state == State::Done
    }

    /// Whether the bytes were not JPEG markers.
    // Port of: src/codec/SkJpegSegmentScan.h (hadError)
    #[must_use]
    pub fn had_error(&self) -> bool {
        self.state == State::Error
    }

    /// The segments found so far.
    // Port of: src/codec/SkJpegSegmentScan.cpp#L35-L37 (chrome/m156), `getSegments`.
    #[must_use]
    pub fn segments(&self) -> &[JpegSegment] {
        &self.segments
    }

    /// The parameters of `segment` in `scanned_data`, which holds the bytes that were scanned
    /// (`SkJpegSegmentScanner::GetParameters`).
    // Port of: src/codec/SkJpegSegmentScan.cpp#L39-L45 (chrome/m156)
    #[must_use]
    pub fn get_parameters(scanned_data: &Data, segment: &JpegSegment) -> Option<Data> {
        let offset = segment.offset + JPEG_MARKER_CODE_SIZE + JPEG_SEGMENT_PARAMETER_LENGTH_SIZE;
        let length = usize::from(segment.parameter_length)
            .checked_sub(JPEG_SEGMENT_PARAMETER_LENGTH_SIZE)?;
        scanned_data.share_subset(offset, length)
    }

    /// Scans `data`, which continues the bytes given to earlier calls.
    // Port of: src/codec/SkJpegSegmentScan.cpp#L47-L84 (chrome/m156), `onBytes`.
    pub fn on_bytes(&mut self, data: &[u8]) {
        let mut pos = 0usize;
        let mut bytes_remaining = data.len();
        while bytes_remaining > 0 {
            let bytes = &data[pos..];
            let bytes_to_move_forward = match self.state {
                State::SegmentParam => {
                    let to_move = self.segment_param_bytes_remaining.min(bytes_remaining);
                    self.segment_param_bytes_remaining -= to_move;
                    if self.segment_param_bytes_remaining == 0 {
                        self.state = State::EntropyCodedData;
                    }
                    to_move
                }
                State::EntropyCodedData => {
                    if let Some(sentinel) = bytes[..bytes_remaining].iter().position(|&b| b == 0xFF)
                    {
                        self.state = State::EntropyCodedDataSentinel;
                        sentinel + 1
                    } else {
                        bytes_remaining
                    }
                }
                State::Done => bytes_remaining,
                _ => {
                    self.on_byte(bytes[0]);
                    1
                }
            };
            debug_assert!(bytes_to_move_forward > 0);
            self.offset += bytes_to_move_forward;
            pos += bytes_to_move_forward;
            bytes_remaining -= bytes_to_move_forward;
        }
    }

    // Port of: src/codec/SkJpegSegmentScan.cpp#L86-L92 (chrome/m156), `saveCurrentSegment`.
    fn save_current_segment(&mut self, length: u16) {
        self.segments.push(JpegSegment {
            offset: self.current_segment_offset,
            marker: self.current_segment_marker,
            parameter_length: length,
        });
        self.current_segment_marker = 0;
        self.current_segment_offset = 0;
    }

    // Port of: src/codec/SkJpegSegmentScan.cpp#L94-L119 (chrome/m156), `onMarkerSecondByte`.
    fn on_marker_second_byte(&mut self, byte: u8) {
        self.current_segment_marker = byte;
        self.current_segment_offset = self.offset - 1;
        if byte == self.stop_marker {
            self.save_current_segment(0);
            self.state = State::Done;
        } else if byte == JPEG_MARKER_START_OF_IMAGE {
            self.save_current_segment(0);
            self.state = State::SecondMarkerByte0;
        } else if marker_stands_alone(byte) {
            self.save_current_segment(0);
            self.state = State::EntropyCodedData;
        } else {
            self.current_segment_marker = byte;
            self.state = State::SegmentParamLengthByte0;
        }
    }

    // Port of: src/codec/SkJpegSegmentScan.cpp#L121-L199 (chrome/m156), `onByte`.
    fn on_byte(&mut self, byte: u8) {
        match self.state {
            State::StartOfImageByte0 => {
                if byte != 0xFF {
                    // "First byte was %02x, not 0xFF"
                    self.state = State::Error;
                    return;
                }
                self.state = State::StartOfImageByte1;
            }
            State::StartOfImageByte1 => {
                if byte != JPEG_MARKER_START_OF_IMAGE {
                    // "Second byte was %02x, not %02x"
                    self.state = State::Error;
                    return;
                }
                self.on_marker_second_byte(byte);
            }
            State::SecondMarkerByte0 => {
                if byte != 0xFF {
                    // "Third byte was %02x, not 0xFF"
                    self.state = State::Error;
                    return;
                }
                self.state = State::SecondMarkerByte1;
            }
            State::SecondMarkerByte1 => {
                if byte == 0xFF || byte == 0x00 {
                    // "SkJpegSegment marker was 0xFF,0xFF or 0xFF,0x00"
                    self.state = State::Error;
                    return;
                }
                self.on_marker_second_byte(byte);
            }
            State::SegmentParamLengthByte0 => {
                self.segment_param_length_byte0 = byte;
                self.state = State::SegmentParamLengthByte1;
            }
            State::SegmentParamLengthByte1 => {
                let param_length =
                    256u16 * u16::from(self.segment_param_length_byte0) + u16::from(byte);
                self.segment_param_length_byte0 = 0;
                if usize::from(param_length) < JPEG_SEGMENT_PARAMETER_LENGTH_SIZE {
                    // "SkJpegSegment payload length was %u < 2 bytes"
                    self.state = State::Error;
                    return;
                }
                self.save_current_segment(param_length);
                self.segment_param_bytes_remaining =
                    usize::from(param_length) - JPEG_SEGMENT_PARAMETER_LENGTH_SIZE;
                self.state = if self.segment_param_bytes_remaining > 0 {
                    State::SegmentParam
                } else {
                    State::EntropyCodedData
                };
            }
            State::SegmentParam => {
                self.segment_param_bytes_remaining -= 1;
                if self.segment_param_bytes_remaining == 0 {
                    self.state = State::EntropyCodedData;
                }
            }
            State::EntropyCodedData => {
                if byte == 0xFF {
                    self.state = State::EntropyCodedDataSentinel;
                }
            }
            State::EntropyCodedDataSentinel => {
                if byte == 0x00 {
                    self.state = State::EntropyCodedData;
                } else if byte == 0xFF {
                    self.state = State::PostEntropyCodedDataFill;
                } else {
                    self.on_marker_second_byte(byte);
                }
            }
            State::PostEntropyCodedDataFill => {
                if byte == 0xFF {
                    self.state = State::PostEntropyCodedDataFill;
                } else if byte == 0x00 {
                    // "Post entropy coded data had 0xFF,0x00"
                    self.state = State::Error;
                } else {
                    self.on_marker_second_byte(byte);
                }
            }
            State::Done | State::Error => {}
        }
    }
}
