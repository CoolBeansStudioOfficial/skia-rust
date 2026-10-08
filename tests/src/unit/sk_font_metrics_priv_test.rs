// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkFontMetricsPrivTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::font::Font;
use skia_rust_core::font_metrics::FontMetrics;
use skia_rust_core::font_metrics_priv;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::strike_spec::StrikeSpec;
use skia_rust_core::write_buffer::BinaryWriteBuffer;
use skia_rust_tools::font_tool_utils::create_test_typeface;

use crate::{def_test, reporter_assert};

// Port of: tests/SkFontMetricsPrivTest.cpp#L22-L51 (chrome/m156)
def_test!(
    #[ignore = "portable configuration only: the NativeFontations run needs T19b (docs/design/text.md §8)"]
    SkFontMetricsPriv_Basic,
    |reporter| {
        let typeface = create_test_typeface(Some("monospace"), FontStyle::default());
        let font = Font::from_size(typeface, 12.0);
        let spec = StrikeSpec::make_with_no_device(
            &font,
            None,
            skia_rust_core::scaler_context::ScalerContextBuildFlags::NONE,
        );
        let mut context = spec.create_scaler_context();

        // Check that font metrics round-trip.
        let src_metrics: FontMetrics = context.get_font_metrics();

        let mut write_buffer = BinaryWriteBuffer::new();
        font_metrics_priv::flatten(&mut write_buffer, &src_metrics);

        let data = write_buffer.snapshot_as_data();

        let mut read_buffer = ReadBuffer::new(data.as_bytes());

        let dst_metrics = font_metrics_priv::make_from_buffer(&mut read_buffer);

        reporter_assert!(reporter, dst_metrics.is_some());
        reporter_assert!(reporter, Some(src_metrics) == dst_metrics);

        // Check that a broken buffer is detected.
        // Must be multiple of 4 for a valid buffer.
        let broken_data: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut broken_buffer = ReadBuffer::new(&broken_data);

        let dst_metrics = font_metrics_priv::make_from_buffer(&mut broken_buffer);
        reporter_assert!(reporter, dst_metrics.is_none());
    }
);
