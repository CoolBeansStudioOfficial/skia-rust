// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/giantbitmap.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::raster_canvas::RasterCanvas;

const W: i32 = 257;
const H: i32 = 161;

// Port of: gm/giantbitmap.cpp#L27-L133 (chrome/m156)
struct GiantBitmapGm {
    bm: Option<Bitmap>,
    mode: TileMode,
    do_filter: bool,
    do_rotate: bool,
}

impl GiantBitmapGm {
    fn new(mode: TileMode, do_filter: bool, do_rotate: bool) -> GiantBitmapGm {
        GiantBitmapGm {
            bm: None,
            mode,
            do_filter,
            do_rotate,
        }
    }

    // Port of: gm/giantbitmap.cpp#L33-L68 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    fn get_bitmap(&mut self) -> &Bitmap {
        const COLORS: [Color; 4] = [Color::BLUE, Color::RED, Color::BLACK, Color::GREEN];
        self.bm.get_or_insert_with(|| {
            let mut bm = Bitmap::new();
            bm.alloc_n32_pixels((W, H), None);
            bm.erase_color(Color::WHITE);

            {
                let canvas = CoreCanvas::from_bitmap(&mut bm, None).expect("a canvas");
                let mut paint = Paint::default();
                paint.set_anti_alias(true);
                paint.set_stroke_width(20.0);

                // (The `#if 0` diagonal-line variant is not compiled.)
                let mut x = -W;
                while x < W {
                    #[allow(clippy::cast_sign_loss)] // `x/60 & 0x3` is in 0..=3
                    paint.set_color(COLORS[((x / 60) & 0x3) as usize]);

                    let xx = x as f32;
                    canvas.draw_line((xx, 0.0), (xx, H as f32), &paint);
                    x += 60;
                }
            }
            bm
        })
    }
}

impl GM for GiantBitmapGm {
    // Port of: gm/giantbitmap.cpp#L80-L103 (chrome/m156)
    fn name(&self) -> String {
        let mut str = String::from("giantbitmap_");
        match self.mode {
            TileMode::Clamp => str.push_str("clamp"),
            TileMode::Repeat => str.push_str("repeat"),
            TileMode::Mirror => str.push_str("mirror"),
            TileMode::Decal => str.push_str("decal"),
        }
        str.push_str(if self.do_filter { "_bilerp" } else { "_point" });
        str.push_str(if self.do_rotate { "_rotate" } else { "_scale" });
        str
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    // Port of: gm/giantbitmap.cpp#L107-L128 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();

        let mut m = Matrix::new_identity();
        if self.do_rotate {
            m.set_skew((1.0, 0.0), Point::new(0.0, 0.0));
        } else {
            let scale = 11.0 * 1.0 / 12.0;
            m.set_scale((scale, scale), None);
        }
        let (mode, do_filter) = (self.mode, self.do_filter);
        paint.set_shader(self.get_bitmap().to_shader(
            (mode, mode),
            SamplingOptions::from(if do_filter {
                FilterMode::Linear
            } else {
                FilterMode::Nearest
            }),
            &m,
        ));

        canvas.translate((50.0, 50.0));

        canvas.draw_paint(&paint);
    }
}

// Port of: gm/giantbitmap.cpp#L136-L148 (chrome/m156)
crate::def_gm!(
    GiantBitmapGM_kClamp_false_false = "GiantBitmapGM(SkTileMode::kClamp, false, false)",
    GiantBitmapGm::new(TileMode::Clamp, false, false)
);
crate::def_gm!(
    GiantBitmapGM_kRepeat_false_false = "GiantBitmapGM(SkTileMode::kRepeat, false, false)",
    GiantBitmapGm::new(TileMode::Repeat, false, false)
);
crate::def_gm!(
    GiantBitmapGM_kMirror_false_false = "GiantBitmapGM(SkTileMode::kMirror, false, false)",
    GiantBitmapGm::new(TileMode::Mirror, false, false)
);
crate::def_gm!(
    GiantBitmapGM_kClamp_true_false = "GiantBitmapGM(SkTileMode::kClamp, true, false)",
    GiantBitmapGm::new(TileMode::Clamp, true, false)
);
crate::def_gm!(
    GiantBitmapGM_kRepeat_true_false = "GiantBitmapGM(SkTileMode::kRepeat, true, false)",
    GiantBitmapGm::new(TileMode::Repeat, true, false)
);
crate::def_gm!(
    GiantBitmapGM_kMirror_true_false = "GiantBitmapGM(SkTileMode::kMirror, true, false)",
    GiantBitmapGm::new(TileMode::Mirror, true, false)
);

crate::def_gm!(
    GiantBitmapGM_kClamp_false_true = "GiantBitmapGM(SkTileMode::kClamp, false, true)",
    GiantBitmapGm::new(TileMode::Clamp, false, true)
);
crate::def_gm!(
    GiantBitmapGM_kRepeat_false_true = "GiantBitmapGM(SkTileMode::kRepeat, false, true)",
    GiantBitmapGm::new(TileMode::Repeat, false, true)
);
crate::def_gm!(
    GiantBitmapGM_kMirror_false_true = "GiantBitmapGM(SkTileMode::kMirror, false, true)",
    GiantBitmapGm::new(TileMode::Mirror, false, true)
);
crate::def_gm!(
    GiantBitmapGM_kClamp_true_true = "GiantBitmapGM(SkTileMode::kClamp, true, true)",
    GiantBitmapGm::new(TileMode::Clamp, true, true)
);
crate::def_gm!(
    GiantBitmapGM_kRepeat_true_true = "GiantBitmapGM(SkTileMode::kRepeat, true, true)",
    GiantBitmapGm::new(TileMode::Repeat, true, true)
);
crate::def_gm!(
    GiantBitmapGM_kMirror_true_true = "GiantBitmapGM(SkTileMode::kMirror, true, true)",
    GiantBitmapGm::new(TileMode::Mirror, true, true)
);
