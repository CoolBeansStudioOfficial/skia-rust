// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/asyncrescaleandread.cpp (chrome/m156), the AyncYUVNoScaleGM class

use crate::prelude::*;
use crate::tool_utils::get_resource_as_image;
use skia_rust_core::color::Color;
use skia_rust_core::image::Image;

// The YUV readback of a raster surface. The C++ `SkSurface_Base::onAsyncRescaleAndReadPixelsYUV420`
// has no raster implementation: it calls the client's callback with no result, and
// `readAndScaleYUVA` then returns null.
// Port of: src/image/SkSurface_Base.cpp#L89-L96 (chrome/m156), `onAsyncRescaleAndReadPixelsYUV420`
#[allow(clippy::unnecessary_wraps)] // the `None` is the callback's result, which is always absent
fn async_rescale_and_read_yuv420_raster() -> Option<Image> {
    None
}

// Port of: gm/asyncrescaleandread.cpp#L598-L632 (chrome/m156), AyncYUVNoScaleGM
struct AyncYuvNoScaleGm;

impl GM for AyncYuvNoScaleGm {
    fn name(&self) -> String {
        "async_yuv_no_scale".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(400, 300)
    }

    // Port of: gm/asyncrescaleandread.cpp#L601-L626 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        let Some(image) = get_resource_as_image("images/yellow_rose.webp") else {
            return DrawResult::Fail;
        };
        canvas.draw_image(&image, (15.0, 12.0), None);
        let yuv_image = async_rescale_and_read_yuv420_raster();
        canvas.clear(Color::WHITE);
        if let Some(yuv_image) = yuv_image {
            canvas.draw_image(&yuv_image, (0.0, 0.0), None);
        }
        DrawResult::Ok
    }
}

// Port of: gm/asyncrescaleandread.cpp#L598 (chrome/m156), DEF_GM(return new AyncYUVNoScaleGM();)
crate::def_gm!(AyncYUVNoScaleGM_ = "AyncYUVNoScaleGM()", AyncYuvNoScaleGm);
