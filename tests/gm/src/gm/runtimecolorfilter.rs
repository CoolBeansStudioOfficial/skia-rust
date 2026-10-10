// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/runtimecolorfilter.cpp (chrome/m156)

use crate::GM;
use crate::tool_utils::get_resource_as_image;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::size::ISize;
use skia_rust_core::vertices::{VertexMode, Vertices};
use skia_rust_raster::surfaces;

// Port of: gm/runtimecolorfilter.cpp#L10-L13 (chrome/m156), gNoop
const G_NOOP: &str = r"
    half4 main(half4 color) {
        return color;
    }
";

// Port of: gm/runtimecolorfilter.cpp#L14-L17 (chrome/m156), gLumaSrc
const G_LUMA_SRC: &str = r"
    half4 main(half4 color) {
        return dot(color.rgb, half3(0.3, 0.6, 0.1)).000r;
    }
";

// Port of: gm/runtimecolorfilter.cpp#L20-L27 (chrome/m156), gTernary
const G_TERNARY: &str = r"
    half4 main(half4 color) {
        half luma = dot(color.rgb, half3(0.3, 0.6, 0.1));
        half scale = luma < 0.33333 ? 0.5
                   : luma < 0.66666 ? (0.166666 + 2.0 * (luma - 0.33333)) / luma
                   :   /* else */     (0.833333 + 0.5 * (luma - 0.66666)) / luma;
        return half4(color.rgb * scale, color.a);
    }
";

// Port of: gm/runtimecolorfilter.cpp#L29-L43 (chrome/m156), gIfs
const G_IFS: &str = r"
    half4 main(half4 color) {
        half luma = dot(color.rgb, half3(0.3, 0.6, 0.1));
        half scale = 0;
        if (luma < 0.33333) {
            scale = 0.5;
        } else if (luma < 0.66666) {
            scale = (0.166666 + 2.0 * (luma - 0.33333)) / luma;
        } else {
            scale = (0.833333 + 0.5 * (luma - 0.66666)) / luma;
        }
        return half4(color.rgb * scale, color.a);
    }
";

// Port of: gm/runtimecolorfilter.cpp#L45-L60 (chrome/m156), gEarlyReturn
const G_EARLY_RETURN: &str = r"
    half4 main(half4 color) {
        half luma = dot(color.rgb, half3(0.3, 0.6, 0.1));
        half scale = 0;
        if (luma < 0.33333) {
            return half4(color.rgb * 0.5, color.a);
        } else if (luma < 0.66666) {
            scale = 0.166666 + 2.0 * (luma - 0.33333);
        } else {
            scale = 0.833333 + 0.5 * (luma - 0.66666);
        }
        return half4(color.rgb * (scale/luma), color.a);
    }
";

// Port of: gm/runtimecolorfilter.cpp#L62-L95 (chrome/m156), class RuntimeColorFilterGM
struct RuntimeColorFilterGm {
    img: Option<Image>,
}

impl GM for RuntimeColorFilterGm {
    fn name(&self) -> String {
        "runtimecolorfilter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(256 * 3, 256 * 2)
    }

    // Port of: gm/runtimecolorfilter.cpp#L71-L73 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.img = get_resource_as_image("images/mandrill_256.png");
    }

    // Port of: gm/runtimecolorfilter.cpp#L75-L92 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let img = self.img.clone().expect("images/mandrill_256.png");
        let draw_filter = |src: &str| {
            let effect = RuntimeEffect::make_for_color_filter(src, None)
                .expect("the colour filter compiles");
            let mut p = Paint::default();
            p.set_color_filter(effect.make_color_filter(Data::new_empty(), &[]));
            canvas.draw_image_with_sampling_options(
                &img,
                (0.0, 0.0),
                SamplingOptions::default(),
                Some(&p),
            );
            canvas.translate((256.0, 0.0));
        };
        for src in [G_NOOP, G_LUMA_SRC] {
            draw_filter(src);
        }
        canvas.translate((-256.0 * 2.0, 256.0));
        for src in [G_TERNARY, G_IFS, G_EARLY_RETURN] {
            draw_filter(src);
        }
    }
}

// Port of: gm/runtimecolorfilter.cpp#L95 (chrome/m156), DEF_GM(return new RuntimeColorFilterGM;)
crate::def_gm!(
    RuntimeColorFilterGM = "RuntimeColorFilterGM",
    RuntimeColorFilterGm { img: None }
);

// Port of: gm/runtimecolorfilter.cpp#L97-L192 (chrome/m156), DEF_SIMPLE_GM(runtimecolorfilter_vertices_atlas_and_patch)
crate::def_simple_gm!(
    runtimecolorfilter_vertices_atlas_and_patch,
    canvas,
    404,
    404,
    {
        vertices_atlas_and_patch(canvas);
    }
);

// The body of the GM above, as one function: expanded inside the GM macro it is over the
// too_many_lines limit.
#[allow(clippy::too_many_lines)] // mirrors gm/runtimecolorfilter.cpp, one long DEF_SIMPLE_GM body
fn vertices_atlas_and_patch(canvas: &Canvas) {
    let r = Rect::from_wh(128.0, 128.0);
    // Make a vertices that draws the same as SkRect 'r'.
    let pos = r.to_quad(None);
    let k_colors = [Color::BLUE, Color::GREEN, Color::CYAN, Color::YELLOW];
    let verts = Vertices::new_copy(
        VertexMode::TriangleFan,
        &pos,
        Some(&pos),
        Some(&k_colors),
        None,
    )
    .expect("a triangle fan");

    // Make an image from the vertices to do equivalent drawAtlas, drawPatch using an image shader.
    let info = ImageInfo::new(
        (128, 128),
        ColorType::RGBA8888,
        AlphaType::Premul,
        canvas.image_info().color_space(),
    );
    let mut surf = surfaces::raster(&info, None, None).expect("a surface");
    surf.canvas()
        .draw_vertices(&verts, BlendMode::Dst, &Paint::default());
    let atlas = surf.image_snapshot().expect("a snapshot");
    let xform = RSXform::new(1.0, 0.0, (0.0, 0.0));

    // Make a patch that draws the same as the SkRect 'r'
    let mut vx = pos[1] - pos[0];
    let mut vy = pos[3] - pos[0];
    vx.set_length(vx.length() / 3.0);
    vy.set_length(vy.length() / 3.0);
    let cubics = [
        pos[0],
        pos[0] + vx,
        pos[1] - vx,
        pos[1],
        pos[1] + vy,
        pos[2] - vy,
        pos[2],
        pos[2] - vx,
        pos[3] + vx,
        pos[3],
        pos[3] - vy,
        pos[0] + vy,
    ];
    let colorfilter = RuntimeEffect::make_for_color_filter(G_LUMA_SRC, None)
        .expect("the colour filter compiles")
        .make_color_filter(Data::new_empty(), &[]);

    let make_paint = |use_cf: bool, use_shader: bool| {
        let mut paint = Paint::default();
        paint.set_color_filter(if use_cf { colorfilter.clone() } else { None });
        paint.set_shader(if use_shader {
            Some(
                atlas
                    .to_shader(None, SamplingOptions::from(FilterMode::Nearest), None)
                    .expect("a shader"),
            )
        } else {
            None
        });
        paint
    };
    let draw_vertices = |x: f32, use_cf: bool, use_shader: bool| {
        canvas.save();
        canvas.translate((x, 0.0));
        // Use just the shader or just the vertex colors.
        let mode = if use_shader {
            BlendMode::Src
        } else {
            BlendMode::Dst
        };
        canvas.draw_vertices(&verts, mode, &make_paint(use_cf, use_shader));
        canvas.restore();
    };
    let draw_atlas = |x: f32, use_cf: bool| {
        canvas.save();
        canvas.translate((x, 0.0));
        let paint = make_paint(use_cf, false);
        let k_color = Color::WHITE;
        canvas.draw_atlas(
            &atlas,
            &[xform],
            &[r],
            Some(&[k_color][..]),
            BlendMode::Modulate,
            SamplingOptions::from(FilterMode::Nearest),
            None,
            &paint,
        );
        canvas.restore();
    };
    let draw_patch = |x: f32, use_cf: bool| {
        canvas.save();
        canvas.translate((x, 0.0));
        let paint = make_paint(use_cf, true);
        canvas.draw_patch(
            &cubics,
            None::<&[Color; 4]>,
            Some(&pos),
            BlendMode::Modulate,
            &paint,
        );
        canvas.restore();
    };
    draw_vertices(0.0, false, false);
    draw_vertices(r.width() + 10.0, true, false);
    draw_vertices(2.0 * (r.width() + 10.0), true, true);
    canvas.translate((0.0, r.height() + 10.0));
    draw_atlas(0.0, false);
    draw_atlas(r.width() + 10.0, true);
    canvas.translate((0.0, r.height() + 10.0));
    draw_patch(0.0, false);
    draw_patch(r.width() + 10.0, true);
}
