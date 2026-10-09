// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkFontMetricsPriv.{h,cpp}

//! `SkFontMetricsPriv`: the flattening of [`FontMetrics`] into a buffer.

use crate::font_metrics::{Flags, FontMetrics};
use crate::read_buffer::ReadBuffer;
use crate::write_buffer::BinaryWriteBuffer;

/// Writes the metrics (`SkFontMetricsPriv::Flatten`): the flags, then the 15 scalars in
/// declaration order.
// Port of: src/core/SkFontMetricsPriv.cpp#L15-L31 (chrome/m156)
#[doc(alias = "Flatten")]
pub fn flatten(buffer: &mut BinaryWriteBuffer, metrics: &FontMetrics) {
    buffer.write_uint(metrics.flags.bits());
    buffer.write_scalar(metrics.top);
    buffer.write_scalar(metrics.ascent);
    buffer.write_scalar(metrics.descent);
    buffer.write_scalar(metrics.bottom);
    buffer.write_scalar(metrics.leading);
    buffer.write_scalar(metrics.avg_char_width);
    buffer.write_scalar(metrics.max_char_width);
    buffer.write_scalar(metrics.x_min);
    buffer.write_scalar(metrics.x_max);
    buffer.write_scalar(metrics.x_height);
    buffer.write_scalar(metrics.cap_height);
    buffer.write_scalar(metrics.underline_thickness);
    buffer.write_scalar(metrics.underline_position);
    buffer.write_scalar(metrics.strikeout_thickness);
    buffer.write_scalar(metrics.strikeout_position);
}

/// Reads metrics written by [`flatten`] (`SkFontMetricsPriv::MakeFromBuffer`). `None` if the
/// buffer became invalid.
// Port of: src/core/SkFontMetricsPriv.cpp#L34-L61 (chrome/m156)
#[doc(alias = "MakeFromBuffer")]
#[must_use]
pub fn make_from_buffer(buffer: &mut ReadBuffer<'_>) -> Option<FontMetrics> {
    let metrics = FontMetrics {
        // The flags are kept as written, unknown bits included (`fFlags = buffer.readUInt()`).
        flags: Flags::from_bits_retain(buffer.read_uint()),
        top: buffer.read_scalar(),
        ascent: buffer.read_scalar(),
        descent: buffer.read_scalar(),
        bottom: buffer.read_scalar(),
        leading: buffer.read_scalar(),
        avg_char_width: buffer.read_scalar(),
        max_char_width: buffer.read_scalar(),
        x_min: buffer.read_scalar(),
        x_max: buffer.read_scalar(),
        x_height: buffer.read_scalar(),
        cap_height: buffer.read_scalar(),
        underline_thickness: buffer.read_scalar(),
        underline_position: buffer.read_scalar(),
        strikeout_thickness: buffer.read_scalar(),
        strikeout_position: buffer.read_scalar(),
    };
    // All the reads above were valid, so return the metrics.
    buffer.is_valid().then_some(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_round_trip_through_a_buffer() {
        let metrics = FontMetrics {
            flags: Flags::UNDERLINE_THICKNESS_IS_VALID | Flags::BOUNDS_INVALID,
            top: -1.5,
            ascent: -10.0,
            descent: 3.0,
            bottom: 4.5,
            leading: 0.25,
            avg_char_width: 6.0,
            max_char_width: 9.0,
            x_min: -2.0,
            x_max: 8.0,
            x_height: 5.0,
            cap_height: 7.0,
            underline_thickness: 0.5,
            underline_position: -1.0,
            strikeout_thickness: 0.75,
            strikeout_position: 2.5,
        };
        let mut buffer = BinaryWriteBuffer::new();
        flatten(&mut buffer, &metrics);
        let mut data = vec![0; buffer.bytes_written()];
        buffer.write_to_memory(&mut data);

        let mut reader = ReadBuffer::new(&data);
        assert_eq!(make_from_buffer(&mut reader), Some(metrics));
        // A truncated buffer is invalid.
        let mut reader = ReadBuffer::new(&data[..data.len() - 4]);
        assert_eq!(make_from_buffer(&mut reader), None);
    }
}
