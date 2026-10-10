// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/colorfilters.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::{get_resource_as_image, int_to_scalar};
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters;
use skia_rust_core::color_priv::ColorConverter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};

// Port of: gm/colorfilters.cpp#L8-L20 (chrome/m156), make_shader
fn make_shader(bounds: Rect) -> Option<Shader> {
    let pts = [
        Point::new(bounds.left(), bounds.top()),
        Point::new(bounds.right(), bounds.bottom()),
    ];
    let colors = [
        Color4f::from(Color::RED),
        Color4f::from(Color::GREEN),
        Color4f::from(Color::BLUE),
        Color4f::from(Color::BLACK),
        Color4f::from(Color::CYAN),
        Color4f::from(Color::MAGENTA),
        Color4f::from(Color::YELLOW),
    ];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/colorfilters.cpp#L22-L27 (chrome/m156), the install procs
type InstallPaint = fn(&mut Paint, u32, u32);

fn install_nothing(paint: &mut Paint, _mul: u32, _add: u32) {
    paint.set_color_filter(None);
}

// Port of: gm/colorfilters.cpp#L26-L27 (chrome/m156), install_lighting
fn install_lighting(paint: &mut Paint, mul: u32, add: u32) {
    paint.set_color_filter(color_filters::lighting(Color::from(mul), Color::from(add)));
}

// Port of: gm/colorfilters.cpp#L29-L55 (chrome/m156), ColorFiltersGM
struct ColorFiltersGm;

impl GM for ColorFiltersGm {
    fn name(&self) -> String {
        "lightingcolorfilter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(620, 430)
    }

    // Port of: gm/colorfilters.cpp#L35-L54 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let r = Rect::new(0.0, 0.0, 600.0, 50.0);
        let mut paint = Paint::default();
        paint.set_shader(make_shader(r));
        let rec: [(InstallPaint, u32, u32); 7] = [
            (install_nothing, 0, 0),
            (install_lighting, 0xFF_0000, 0),
            (install_lighting, 0x00_FF00, 0),
            (install_lighting, 0x00_00FF, 0),
            (install_lighting, 0x00_0000, 0xFF_0000),
            (install_lighting, 0x00_0000, 0x00_FF00),
            (install_lighting, 0x00_0000, 0x00_00FF),
        ];
        canvas.translate((10.0, 10.0));
        for (proc_fn, data0, data1) in rec {
            proc_fn(&mut paint, data0, data1);
            canvas.draw_rect(r, &paint);
            canvas.translate((0.0, r.height() + 10.0));
        }
    }
}

// Port of: gm/colorfilters.cpp#L57 (chrome/m156)
crate::def_gm!(ColorFiltersGM, ColorFiltersGm);

// Port of: gm/colorfilters.cpp#L59-L60 (chrome/m156), kWheelSize and kSteps
const K_WHEEL_SIZE: f32 = 100.0;
const K_STEPS: i32 = 7;

// The (start, end) range of one channel of a test (`std::tuple<float, float>`).
type Range = (f32, f32);

// Port of: gm/colorfilters.cpp#L107-L129 (chrome/m156), HSLColorFilterGM::make_filter
fn make_hsl_filter(h: f32, s: f32, l: f32) -> Option<ColorFilter> {
    // These are roughly AE semantics.
    let h_bias = h;
    let h_scale: f32 = 1.0;
    let s_bias = s.max(0.0);
    let s_scale = 1.0 - s.abs();
    let l_bias = l.max(0.0);
    let l_scale = 1.0 - l.abs();
    #[rustfmt::skip]
    let cm: [f32; 20] = [
        h_scale, 0.0, 0.0, 0.0, h_bias,
        0.0, s_scale, 0.0, 0.0, s_bias,
        0.0, 0.0, l_scale, 0.0, l_bias,
        0.0, 0.0, 0.0, 1.0, 0.0,
    ];
    color_filters::hsla_matrix(&cm)
}

// Port of: gm/colorfilters.cpp#L62-L64 (chrome/m156), gGrads
const GRADS: [[u32; 4]; 2] = [
    [0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0xffff_0000],
    [0xdfc0_8040, 0xdf80_40c0, 0xdf40_c080, 0xdfc0_8040],
];

// Port of: gm/colorfilters.cpp#L61-L131 (chrome/m156), HSLColorFilterGM
struct HslColorFilterGm {
    shaders: Vec<Option<Shader>>,
}

impl HslColorFilterGm {
    fn new() -> Self {
        Self {
            shaders: Vec::new(),
        }
    }
}

impl GM for HslColorFilterGm {
    fn name(&self) -> String {
        "hslcolorfilter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(840, 1100)
    }

    // Port of: gm/colorfilters.cpp#L66-L81 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let Some(mandrill) = get_resource_as_image("images/mandrill_256.png") else {
            return;
        };
        let lm = Matrix::rect_to_rect_or_identity(
            Rect::from_wh(
                int_to_scalar(mandrill.width()),
                int_to_scalar(mandrill.height()),
            ),
            Rect::from_wh(K_WHEEL_SIZE, K_WHEEL_SIZE),
            None,
        );
        self.shaders
            .push(mandrill.to_shader(None, SamplingOptions::default(), &lm));

        for cols in GRADS {
            let colors: Vec<Color> = cols.iter().map(|&c| Color::from(c)).collect();
            let conv = ColorConverter::new(&colors);
            self.shaders.push(gradient_shaders::sweep_gradient(
                (K_WHEEL_SIZE / 2.0, K_WHEEL_SIZE / 2.0),
                (-90.0, 270.0),
                &Gradient::new(
                    Colors::new(conv.colors4f(), None, TileMode::Repeat, None),
                    Interpolation::default(),
                ),
                None,
            ));
        }
    }

    // Port of: gm/colorfilters.cpp#L83-L106 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        // (h, s, l) ranges: (start, end) for each channel
        let g_tests: [(Range, Range, Range); 3] = [
            ((-0.5, 0.5), (0.0, 0.0), (0.0, 0.0)),
            ((0.0, 0.0), (-1.0, 1.0), (0.0, 0.0)),
            ((0.0, 0.0), (0.0, 0.0), (-1.0, 1.0)),
        ];

        let rect = Rect::from_wh(K_WHEEL_SIZE, K_WHEEL_SIZE);
        canvas.draw_color(Color::from(0xffcc_cccc), None);
        let mut paint = Paint::default();
        for shader in &self.shaders {
            paint.set_shader(shader.clone());
            for tst in g_tests {
                canvas.translate((0.0, K_WHEEL_SIZE * 0.1));
                let steps_m1 = int_to_scalar(K_STEPS - 1);
                let dh = (tst.0.1 - tst.0.0) / steps_m1;
                let ds = (tst.1.1 - tst.1.0) / steps_m1;
                let dl = (tst.2.1 - tst.2.0) / steps_m1;
                let mut h = tst.0.0;
                let mut s = tst.1.0;
                let mut l = tst.2.0;
                {
                    canvas.save();
                    for _ in 0..K_STEPS {
                        paint.set_color_filter(make_hsl_filter(h, s, l));
                        canvas.translate((K_WHEEL_SIZE * 0.1, 0.0));
                        canvas.draw_rect(rect, &paint);
                        canvas.translate((K_WHEEL_SIZE * 1.1, 0.0));
                        h += dh;
                        s += ds;
                        l += dl;
                    }
                    canvas.restore();
                }
                canvas.translate((0.0, K_WHEEL_SIZE * 1.1));
            }
            canvas.translate((0.0, K_WHEEL_SIZE * 0.1));
        }
    }
}

// Port of: gm/colorfilters.cpp#L133 (chrome/m156)
crate::def_gm!(HSLColorFilterGM, HslColorFilterGm::new());
