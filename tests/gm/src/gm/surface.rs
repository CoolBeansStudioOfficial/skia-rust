// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/surface.cpp (chrome/m156)
//
// The `DEF_SURFACE_TESTS` GMs are not in the inventory. Only `SurfacePropsGM(0)` is ported from the
// `SurfacePropsGM` family (the `kUseDeviceIndependentFonts` variant is not in the manifest).

// The int-to-scalar casts of small sizes and counts mirror the C++ arithmetic of the GM.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

use crate::prelude::*;
use crate::tool_utils;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions as SurfaceSamplingOptions;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::surface_props::{PixelGeometry, SurfaceProps, SurfacePropsFlags};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::text_utils::{self, Align};
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/surface.cpp#L178-L211 (chrome/m156)
struct NewSurfaceGm;

impl NewSurfaceGm {
    fn draw_into(canvas: &Canvas) {
        canvas.draw_color(Color::RED, None);
    }
}

impl GM for NewSurfaceGm {
    fn name(&self) -> String {
        "surfacenew".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(300, 140)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let info = ImageInfo::new_n32_premul((100, 100), None);

        let mut surf = tool_utils::make_surface(canvas, &info, None).expect("a surface");
        Self::draw_into(surf.canvas());

        let image = surf.image_snapshot().expect("a snapshot");
        canvas.draw_image(&image, (10.0, 10.0), None);

        let mut surf2 = surf.new_surface(&info).expect("a surface");
        Self::draw_into(surf2.canvas());

        // Assert that the props were communicated transitively through the first image
        debug_assert_eq!(surf.props(), surf2.props());

        let image2 = surf2.image_snapshot().expect("a snapshot");
        #[allow(clippy::cast_precision_loss)] // SkIntToScalar
        canvas.draw_image(&image2, (10.0 + image.width() as f32 + 10.0, 10.0), None);
    }
}

// Port of: gm/surface.cpp#L213 (chrome/m156)
crate::def_gm!(NewSurfaceGM, NewSurfaceGm);

// Port of: gm/surface.cpp#L379-L418 (chrome/m156)
crate::def_simple_gm!(snap_with_mips, canvas, 80, 75, {
    const PAD: i32 = 8;

    let canvas_info = canvas.image_info();
    let ct = if canvas_info.color_type() == ColorType::Unknown {
        ColorType::RGBA8888
    } else {
        canvas_info.color_type()
    };
    let ii = ImageInfo::new((32, 32), ct, AlphaType::Premul, canvas_info.color_space());
    let mut surface = surfaces::raster(&ii, None, None).expect("a surface");

    #[allow(clippy::cast_precision_loss)] // surface->width() *2/5.f
    let mut next_image = |color: Color| {
        let (w, h) = (surface.width() as f32, surface.height() as f32);
        surface.canvas().clear(color);
        let mut paint = Paint::default();
        paint.set_color(Color::new(!u32::from(color) | 0xFF00_0000));
        surface.canvas().draw_rect(
            Rect::new(w * 2.0 / 5.0, h * 2.0 / 5.0, w * 3.0 / 5.0, h * 3.0 / 5.0),
            &paint,
        );
        surface
            .image_snapshot()
            .expect("a snapshot")
            .with_default_mipmaps()
            .expect("an image")
    };

    let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear);

    #[allow(clippy::cast_precision_loss)] // ii.width() + kPad
    let step = (ii.width() + PAD) as f32;
    canvas.save();
    for _ in 0..3 {
        canvas.save();
        let colors = [Color::new(0xFFF0_F0F0), Color::BLUE];
        for color in colors {
            let image = next_image(color);
            canvas.draw_image_with_sampling_options(&image, (0.0, 0.0), sampling, None);
            canvas.translate((step, 0.0));
        }
        canvas.restore();
        canvas.translate((0.0, step));
        canvas.scale((0.4, 0.4));
    }
    canvas.restore();
});

// Port of: gm/surface.cpp#L14-L16 (chrome/m156), #define W / H
const SP_W: i32 = 800;
const SP_H: i32 = 100;

// SK_GAMMA_CONTRAST and SK_GAMMA_EXPONENT (include/core/SkTypes.h#L85, #L93).
const SK_GAMMA_CONTRAST: f32 = 0.5;
const SK_GAMMA_EXPONENT: f32 = 0.0;

// Port of: gm/surface.cpp#L27-L35 (chrome/m156), make_shader
fn sp_make_shader() -> Option<Shader> {
    let a = 0x99 as f32 / 255.0;
    let b = 0xBB as f32 / 255.0;
    let pts = [
        skia_rust_core::point::Point::new(0.0, 0.0),
        skia_rust_core::point::Point::new(SP_W as f32, SP_H as f32),
    ];
    let grad_colors = [Color4f::new(a, a, a, 1.0), Color4f::new(b, b, b, 1.0)];
    let grad = Gradient::new(
        Colors::new(&grad_colors, None, TileMode::Clamp, None),
        Interpolation::default(),
    );
    shaders::linear_gradient((pts[0], pts[1]), &grad, None)
}

// Port of: gm/surface.cpp#L49-L58 (chrome/m156), test_draw
fn sp_test_draw(canvas: &Canvas, label: &str) {
    let mut paint = Paint::default();

    paint.set_anti_alias(true);
    paint.set_dither(true);

    paint.set_shader(sp_make_shader());
    canvas.draw_rect(Rect::from_wh(SP_W as f32, SP_H as f32), &paint);
    paint.set_shader(None);

    paint.set_color(Color::WHITE);
    let mut font = Font::from_size(default_portable_typeface(), 32.0);
    font.set_edging(Edging::SubpixelAntiAlias);
    text_utils::draw_string(
        canvas,
        label,
        (SP_W / 2) as f32,
        (SP_H * 3 / 4) as f32,
        &font,
        &paint,
        Align::Center,
    );
}

// Port of: gm/surface.cpp#L95-L154 (chrome/m156), class SurfacePropsGM (fFlags = 0)
struct SurfacePropsRec {
    geo: PixelGeometry,
    label: &'static str,
    contrast: f32,
    gamma: f32,
}

// Port of: gm/surface.cpp#L95-L154 (chrome/m156), the recs table
const SP_RECS: [SurfacePropsRec; 9] = [
    SurfacePropsRec {
        geo: PixelGeometry::Unknown,
        label: "Unknown geometry, default contrast/gamma",
        contrast: SK_GAMMA_CONTRAST,
        gamma: SK_GAMMA_EXPONENT,
    },
    SurfacePropsRec {
        geo: PixelGeometry::RGBH,
        label: "RGB_H, default contrast/gamma",
        contrast: SK_GAMMA_CONTRAST,
        gamma: SK_GAMMA_EXPONENT,
    },
    SurfacePropsRec {
        geo: PixelGeometry::BGRH,
        label: "BGR_H, default contrast/gamma",
        contrast: SK_GAMMA_CONTRAST,
        gamma: SK_GAMMA_EXPONENT,
    },
    SurfacePropsRec {
        geo: PixelGeometry::RGBV,
        label: "RGB_V, default contrast/gamma",
        contrast: SK_GAMMA_CONTRAST,
        gamma: SK_GAMMA_EXPONENT,
    },
    SurfacePropsRec {
        geo: PixelGeometry::BGRV,
        label: "BGR_V, default contrast/gamma",
        contrast: SK_GAMMA_CONTRAST,
        gamma: SK_GAMMA_EXPONENT,
    },
    SurfacePropsRec {
        geo: PixelGeometry::RGBH,
        label: "RGB_H contrast : 0 gamma: 0",
        contrast: 0.0,
        gamma: 0.0,
    },
    SurfacePropsRec {
        geo: PixelGeometry::RGBH,
        label: "RGB_H contrast : 1 gamma: 0",
        contrast: 1.0,
        gamma: 0.0,
    },
    SurfacePropsRec {
        geo: PixelGeometry::RGBH,
        label: "RGB_H contrast : 0 gamma: 3.9",
        contrast: 0.0,
        gamma: 3.9,
    },
    SurfacePropsRec {
        geo: PixelGeometry::RGBH,
        label: "RGB_H contrast : 1 gamma: 3.9",
        contrast: 1.0,
        gamma: 3.9,
    },
];

// Port of: gm/surface.cpp#L95-L154 (chrome/m156), class SurfacePropsGM
#[derive(Debug)]
pub struct SurfacePropsGm {
    /// `fFlags`: the surface props flags of every surface (0 or `kUseDeviceIndependentFonts`).
    flags: SurfacePropsFlags,
}

impl SurfacePropsGm {
    // Port of: gm/surface.cpp#L97 (chrome/m156), SurfacePropsGM(uint32_t flags)
    #[must_use]
    pub fn new(flags: SurfacePropsFlags) -> Self {
        Self { flags }
    }
}

impl GM for SurfacePropsGm {
    // Port of: gm/surface.cpp#L127-L130 (chrome/m156), getName
    fn name(&self) -> String {
        if self.flags.is_empty() {
            "surfaceprops".to_owned()
        } else {
            "surfaceprops_df".to_owned()
        }
    }

    fn size(&mut self) -> ISize {
        ISize::new(SP_W, SP_H * SP_RECS.len() as i32)
    }

    // Port of: gm/surface.cpp#L134-L154 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        // must be opaque to have a hope of testing LCD text
        let info = ImageInfo::new((SP_W, SP_H), ColorType::N32, AlphaType::Opaque, None);

        let x: f32 = 0.0;
        let mut y: f32 = 0.0;
        for rec in &SP_RECS {
            let props = SurfaceProps::new_with_text_properties(
                self.flags,
                rec.geo,
                rec.contrast,
                rec.gamma,
            );
            // make_surface on a raster sink: SkSurfaces::Raster(info, &props)
            let Some(mut surface) = surfaces::raster(&info, None, Some(&props)) else {
                continue;
            };
            sp_test_draw(surface.canvas(), rec.label);
            surface.draw(canvas, (x, y), SurfaceSamplingOptions::default(), None);
            y += SP_H as f32;
        }
    }
}

// Port of: gm/surface.cpp#L213-L213 (chrome/m156), DEF_GM( return new SurfacePropsGM(0); )
crate::def_gm!(
    SurfacePropsGM_0 = "SurfacePropsGM(0)",
    SurfacePropsGm::new(SurfacePropsFlags::empty())
);
// Port of: gm/surface.cpp#L214 (chrome/m156),
// DEF_GM( return new SurfacePropsGM(SkSurfaceProps::kUseDeviceIndependentFonts_Flag); )
crate::def_gm!(
    SurfacePropsGM_df = "SurfacePropsGM(SkSurfaceProps::kUseDeviceIndependentFonts_Flag)",
    SurfacePropsGm::new(SurfacePropsFlags::USE_DEVICE_INDEPENDENT_FONTS)
);
