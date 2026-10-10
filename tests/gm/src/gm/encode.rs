// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/encode.cpp (chrome/m156). The `jpeg_orientation` GM is compiled out (`#if 0`) in
// the C++ and is not ported.

use skia_rust_codec::encode::{jpeg_encoder, png_encoder};
use skia_rust_codec::images::deferred_from_encoded_data;
use skia_rust_core::font::Edging;
use skia_rust_core::paint::Paint;
use skia_rust_tools::font_tool_utils::default_portable_font;

use crate::prelude::*;
use crate::tool_utils::get_resource_as_image;

/// Port of `class EncodeGM` (encode.cpp#L28-L58): the mandrill encoded as PNG and as JPEG, side by
/// side, which should look identical.
struct EncodeGm;

impl GM for EncodeGm {
    fn name(&self) -> String {
        "encode".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1024, 600)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(orig) = get_resource_as_image("images/mandrill_512_q075.jpg") else {
            return;
        };
        let Some(bitmap) = orig.as_legacy_bitmap() else {
            return;
        };
        let Some(src) = bitmap.peek_pixels() else {
            return;
        };
        // SkASSERT_RELEASE: the encoders must succeed.
        let png_data = png_encoder::encode_pixmap(&src, &png_encoder::Options::default())
            .expect("SkPngEncoder::Encode");
        let jpg_data = jpeg_encoder::encode_pixmap(&src, &jpeg_encoder::Options::default())
            .expect("SkJpegEncoder::Encode");
        let png_image = deferred_from_encoded_data(Some(png_data), None);
        let jpg_image = deferred_from_encoded_data(Some(jpg_data), None);
        if let Some(image) = &png_image {
            canvas.draw_image(image, (0.0, 0.0), None);
        }
        if let Some(image) = &jpg_image {
            canvas.draw_image(image, (512.0, 0.0), None);
        }

        let mut font = default_portable_font();
        font.set_edging(Edging::Alias);
        canvas.draw_str(
            "Images should look identical.",
            (450.0, 550.0),
            &font,
            &Paint::default(),
        );
    }
}

// Port of: gm/encode.cpp#L60 (chrome/m156)
crate::def_gm!(EncodeGM, EncodeGm);
