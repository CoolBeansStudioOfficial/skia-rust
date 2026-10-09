// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/mesh.cpp (chrome/m156), the GMs that need no image decoding

//! Custom mesh GMs. On the CPU, `SkBitmapDevice::drawMesh` draws nothing, so these GMs show what
//! they draw around their meshes: the clear color, or the picture they record.
//!
//! `MeshWithShadersGM` (four GMs) draws with image shaders, a colour filter and a blender; the CPU
//! `drawMesh` draws nothing, so they show their clear background.
//!
//! Not ported: `custommesh_cs_uniforms`, which is GPU-only (its body runs only with a recording
//! context, so the raster sink skips it).

use std::sync::Arc;

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::mesh::meshes::{self, make_index_buffer, make_vertex_buffer};
use skia_rust_core::mesh::{
    Attribute, AttributeType, IndexBuffer, Mesh, MeshSpecification, Mode, Varying, VaryingType,
    VertexBuffer,
};
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::ChildPtr;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::interpolation::ColorSpace as GradientColorSpace;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_raster::surfaces;

/// `SkMeshSpecification::Make(attrs, stride, varyings, vs, fs, cs, at)`, logging the error as the
/// GM does (`SkDebugf`); the result is `None` when the specification did not compile.
fn make_spec(
    attributes: &[Attribute],
    stride: usize,
    varyings: &[Varying],
    vs: &str,
    fs: &str,
    cs: Option<ColorSpace>,
    at: AlphaType,
) -> Option<Arc<MeshSpecification>> {
    let result =
        MeshSpecification::make_with_alpha_type(attributes, stride, varyings, vs, fs, cs, at);
    if result.specification.is_none() {
        eprintln!("{}", result.error);
    }
    result.specification
}

/// A small loop index as a float (exact for the counts the GMs use).
fn index_f32(i: usize) -> f32 {
    f32::from(u16::try_from(i).expect("the GMs use small counts"))
}

/// The bytes of `values` in native order, as the GM's `memcpy` of its structs copies them.
fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

fn u32_bytes(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

fn u16_bytes(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

/// Writes `bytes` into `dst` at `offset` (`memcpy(SkTAddOffset(data, offset), src, size)`).
fn copy_at(dst: &mut [u8], offset: usize, bytes: &[u8]) {
    dst[offset..offset + bytes.len()].copy_from_slice(bytes);
}

/// A gradient shader with the colors and tile mode the GMs use (`SkShaders::RadialGradient`).
fn radial_gradient(
    center: Point,
    radius: f32,
    colors: &[Color4f],
    tile: TileMode,
) -> Option<Shader> {
    gradient_shaders::radial_gradient(
        (center, radius),
        &Gradient::new(
            Colors::new(colors, None, tile, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/mesh.cpp#L33-L338 (chrome/m156)
struct MeshGm {
    shader: Option<Shader>,
    spec_with_color: Option<Arc<MeshSpecification>>,
    spec_with_no_color: Option<Arc<MeshSpecification>>,
    // On GPU the first IB is a CPU buffer and the second is a GPU buffer; on the CPU both are the
    // same CPU buffer.
    ib: [Option<Arc<IndexBuffer>>; 2],
    color_vb: Option<Arc<VertexBuffer>>,
    no_color_vb: Option<Arc<VertexBuffer>>,
    color_indexed_vb: Option<Arc<VertexBuffer>>,
    no_color_indexed_vb: Option<Arc<VertexBuffer>>,
}

impl MeshGm {
    const COLOR_STRIDE: usize = 24; // sizeof(ColorVertex)
    const NO_COLOR_STRIDE: usize = 16; // sizeof(NoColorVertex)
    const RECT: Rect = Rect::from_ltrb(20.0, 20.0, 120.0, 120.0);
    const UV: Rect = Rect::from_ltrb(0.0, 0.0, 20.0, 20.0);
    // For some buffers we add an offset to ensure we're exercising drawing from mid-buffer.
    const NO_COLOR_OFFSET: usize = 16; // sizeof(NoColorVertex)
    const COLOR_INDEXED_OFFSET: usize = 2 * 24; // 2*sizeof(ColorVertex)
    const INDEX_OFFSET: usize = 6;

    fn new() -> Self {
        MeshGm {
            shader: None,
            spec_with_color: None,
            spec_with_no_color: None,
            ib: [None, None],
            color_vb: None,
            no_color_vb: None,
            color_indexed_vb: None,
            no_color_indexed_vb: None,
        }
    }

    /// `ColorVertex` as bytes: `{pad, brag, xuyv[4]}`.
    fn color_vertex(brag: u32, xuyv: [f32; 4]) -> Vec<u8> {
        let mut bytes = u32_bytes(&[0, brag]);
        bytes.extend(f32_bytes(&xuyv));
        bytes
    }

    /// The data of `ensureBuffers`.
    // Port of: gm/mesh.cpp#L238-L268 (chrome/m156)
    fn ensure_buffers(&mut self) {
        let (l, t, r, b) = (
            Self::RECT.left(),
            Self::RECT.top(),
            Self::RECT.right(),
            Self::RECT.bottom(),
        );
        let (ul, ut, ur, ub) = (
            Self::UV.left(),
            Self::UV.top(),
            Self::UV.right(),
            Self::UV.bottom(),
        );
        let xuyv = |x: f32, u: f32, y: f32, v: f32| [x, u, y, v];

        if self.color_vb.is_none() {
            let mut quad = Vec::new();
            quad.extend(Self::color_vertex(0x00FF_FF00, xuyv(l, ul, t, ut)));
            quad.extend(Self::color_vertex(0x00FF_FFFF, xuyv(r, ur, t, ut)));
            quad.extend(Self::color_vertex(0xFFFF_00FF, xuyv(l, ul, b, ub)));
            quad.extend(Self::color_vertex(0xFFFF_FF00, xuyv(r, ur, b, ub)));
            self.color_vb = Some(make_vertex_buffer(Some(&quad), quad.len()));
        }

        if self.no_color_vb.is_none() {
            // Make this one such that the data is offset into the buffer.
            let mut data = vec![0u8; Self::NO_COLOR_OFFSET];
            for quad in [
                xuyv(l, ul, t, ut),
                xuyv(r, ur, t, ut),
                xuyv(l, ul, b, ub),
                xuyv(r, ur, b, ub),
            ] {
                data.extend(f32_bytes(&quad));
            }
            self.no_color_vb = Some(make_vertex_buffer(Some(&data), data.len()));
        }

        if self.color_indexed_vb.is_none() {
            // This buffer also has an offset.
            let mut data = vec![0u8; Self::COLOR_INDEXED_OFFSET];
            // The indexed quads draw the same as the non-indexed. They just have unused vertices
            // that the index buffer skips over draw with triangles instead of a triangle strip.
            data.extend(Self::color_vertex(0x00FF_FF00, xuyv(l, ul, t, ut)));
            data.extend(Self::color_vertex(
                0x0000_0000,
                xuyv(100.0, 0.0, 100.0, 5.0),
            )); // unused
            data.extend(Self::color_vertex(0x00FF_FFFF, xuyv(r, ur, t, ut)));
            data.extend(Self::color_vertex(
                0x0000_0000,
                xuyv(200.0, 10.0, 200.0, 10.0),
            )); // unused
            data.extend(Self::color_vertex(0xFFFF_00FF, xuyv(l, ul, b, ub)));
            data.extend(Self::color_vertex(0xFFFF_FF00, xuyv(r, ur, b, ub)));
            self.color_indexed_vb = Some(make_vertex_buffer(Some(&data), data.len()));
        }

        if self.no_color_indexed_vb.is_none() {
            let mut quad = Vec::new();
            for v in [
                xuyv(l, ul, t, ut),
                xuyv(100.0, 0.0, 100.0, 5.0), // unused
                xuyv(r, ur, t, ut),
                xuyv(200.0, 10.0, 200.0, 10.0), // unused
                xuyv(l, ul, b, ub),
                xuyv(r, ur, b, ub),
            ] {
                quad.extend(f32_bytes(&v));
            }
            self.no_color_indexed_vb = Some(make_vertex_buffer(Some(&quad), quad.len()));
        }

        if self.ib[0].is_none() {
            // The index buffer has an offset.
            let mut data = vec![0u8; Self::INDEX_OFFSET];
            data.extend(u16_bytes(&[0, 2, 4, 2, 5, 4]));
            self.ib[0] = Some(make_index_buffer(Some(&data), data.len()));
        }

        if self.ib[1].is_none() {
            // On CPU we always use the same CPU-backed index buffer.
            let shared = self.ib[0].clone();
            self.ib[1] = shared;
        }
    }
}

impl GM for MeshGm {
    fn name(&self) -> String {
        "custommesh".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(435, 1180)
    }

    // Port of: gm/mesh.cpp#L43-L138 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        {
            let attributes = [
                Attribute {
                    ty: AttributeType::Float4,
                    offset: 8,
                    name: "xuyv".to_owned(),
                },
                Attribute {
                    ty: AttributeType::UByte4Unorm,
                    offset: 4,
                    name: "brag".to_owned(),
                },
            ];
            let varyings = [
                Varying {
                    ty: VaryingType::Half4,
                    name: "color".to_owned(),
                },
                Varying {
                    ty: VaryingType::Float2,
                    name: "uv".to_owned(),
                },
            ];
            let vs = r"
                    half4 unswizzle_color(half4 color) { return color.garb; }

                    Varyings main(const in Attributes attributes) {
                        Varyings varyings;
                        varyings.color    = unswizzle_color(attributes.brag);
                        varyings.uv       = attributes.xuyv.yw;
                        varyings.position = attributes.xuyv.xz;
                        return varyings;
                    }
            ";
            let fs = r"
                    uniform colorFilter filter;

                    float2 main(const in Varyings varyings, out float4 color) {
                        color = filter.eval(varyings.color);
                        return varyings.uv;
                    }
            ";
            self.spec_with_color = make_spec(
                &attributes,
                Self::COLOR_STRIDE,
                &varyings,
                vs,
                fs,
                Some(ColorSpace::new_srgb()),
                AlphaType::Premul,
            );
        }
        {
            let attributes = [Attribute {
                ty: AttributeType::Float4,
                offset: 0,
                name: "xuyv".to_owned(),
            }];
            let varyings = [Varying {
                ty: VaryingType::Float2,
                name: "vux2".to_owned(),
            }];
            let vs = r"
                    Varyings main(const in Attributes a) {
                        Varyings v;
                        v.vux2     = 2*a.xuyv.wy;
                        v.position = a.xuyv.xz;
                        return v;
                    }
            ";
            let fs = r"
                    float2 helper(in float2 vux2) { return vux2.yx/2; }
                    float2 main(const in Varyings varyings) {
                        return helper(varyings.vux2);
                    }
            ";
            self.spec_with_no_color = make_spec(
                &attributes,
                Self::NO_COLOR_STRIDE,
                &varyings,
                vs,
                fs,
                Some(ColorSpace::new_srgb()),
                AlphaType::Premul,
            );
        }

        let colors = [
            Color4f::new(0.0, 0.0, 0.0, 0.0),
            Color4f::from_color(Color::WHITE),
        ];
        self.shader = radial_gradient(Point::new(10.0, 10.0), 3.0, &colors, TileMode::Mirror);
    }

    // Port of: gm/mesh.cpp#L121-L131 (chrome/m156): the GPU buffers are copies on a GPU; on the
    // CPU the buffers are the ones `ensureBuffers` makes.
    fn on_gpu_setup(&mut self, _canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        self.ensure_buffers();
        DrawResult::Ok
    }

    // Port of: gm/mesh.cpp#L144-L213 (chrome/m156)
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        let Some(spec_with_color) = self.spec_with_color.clone() else {
            return DrawResult::Fail;
        };
        let Some(spec_with_no_color) = self.spec_with_no_color.clone() else {
            return DrawResult::Fail;
        };
        let mut i = 0;
        for blender in [
            Blender::mode(BlendMode::Dst),
            Blender::mode(BlendMode::Src),
            Blender::mode(BlendMode::Saturation),
        ] {
            canvas.save();
            for alpha in [0xFFu8, 0x40] {
                for colors in [false, true] {
                    for shader in [false, true] {
                        // Rather than pile onto the combinatorics we draw every other test case
                        // indexed.
                        let result = if i & 1 == 0 {
                            if colors {
                                Mesh::make(
                                    Some(spec_with_color.clone()),
                                    Mode::TriangleStrip,
                                    self.color_vb.clone(),
                                    4,
                                    0,
                                    None,
                                    &[ChildPtr::Empty],
                                    Self::RECT,
                                )
                            } else {
                                Mesh::make(
                                    Some(spec_with_no_color.clone()),
                                    Mode::TriangleStrip,
                                    self.no_color_vb.clone(),
                                    4,
                                    Self::NO_COLOR_OFFSET,
                                    None,
                                    &[],
                                    Self::RECT,
                                )
                            }
                        } else {
                            // Alternate between CPU and GPU-backend index buffers.
                            let ib = if i % 4 == 0 {
                                self.ib[0].clone()
                            } else {
                                self.ib[1].clone()
                            };
                            if colors {
                                Mesh::make_indexed(
                                    Some(spec_with_color.clone()),
                                    Mode::Triangles,
                                    self.color_indexed_vb.clone(),
                                    6,
                                    Self::COLOR_INDEXED_OFFSET,
                                    ib,
                                    6,
                                    Self::INDEX_OFFSET,
                                    None,
                                    &[ChildPtr::Empty],
                                    Self::RECT,
                                )
                            } else {
                                Mesh::make_indexed(
                                    Some(spec_with_no_color.clone()),
                                    Mode::Triangles,
                                    self.no_color_indexed_vb.clone(),
                                    6,
                                    0,
                                    ib,
                                    6,
                                    Self::INDEX_OFFSET,
                                    None,
                                    &[],
                                    Self::RECT,
                                )
                            }
                        };
                        if !result.mesh.is_valid() {
                            eprintln!("Mesh creation failed: {}", result.error);
                            return DrawResult::Fail;
                        }

                        let mut paint = Paint::default();
                        paint.set_color(Color::new(0xFF00_FF00));
                        paint.set_shader(if shader { self.shader.clone() } else { None });
                        paint.set_alpha(alpha);

                        canvas.draw_mesh(&result.mesh, blender.clone(), &paint);

                        canvas.translate((0.0, 150.0));
                        i += 1;
                    }
                }
            }
            canvas.restore();
            canvas.translate((150.0, 0.0));
        }
        DrawResult::Ok
    }
}
crate::def_gm!(MeshGM, MeshGm::new());

// Port of: gm/mesh.cpp#L340-L506 (chrome/m156)
struct MeshColorSpaceGm {
    vb: Option<Arc<VertexBuffer>>,
    specs: [Option<Arc<MeshSpecification>>; 4],
    shader: Option<Shader>,
}

impl MeshColorSpaceGm {
    const RECT: Rect = Rect::from_ltrb(20.0, 20.0, 120.0, 120.0);

    fn new() -> Self {
        MeshColorSpaceGm {
            vb: None,
            specs: [None, None, None, None],
            shader: None,
        }
    }

    /// `SpecIndex`: the arguments are passed as `(unpremul, spin)` at every call.
    fn spec_index(unpremul: bool, spin: bool) -> usize {
        usize::from(unpremul) + 2 * usize::from(spin)
    }
}

impl GM for MeshColorSpaceGm {
    fn name(&self) -> String {
        "custommesh_cs".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(468, 258)
    }

    // Port of: gm/mesh.cpp#L348-L405 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        let attributes = [
            Attribute {
                ty: AttributeType::Float2,
                offset: 0,
                name: "pos".to_owned(),
            },
            Attribute {
                ty: AttributeType::Float4,
                offset: 8,
                name: "color".to_owned(),
            },
        ];
        let varyings = [Varying {
            ty: VaryingType::Half4,
            name: "color".to_owned(),
        }];
        let premul_vs = r"
                Varyings main(const in Attributes attributes) {
                    Varyings varyings;
                    varyings.color = half4(attributes.color.a*attributes.color.rgb,
                                           attributes.color.a);
                    varyings.position = attributes.pos;
                    return varyings;
                }
        ";
        let unpremul_vs = r"
                Varyings main(const in Attributes attributes) {
                    Varyings varyings;
                    varyings.color    = attributes.color;
                    varyings.position = attributes.pos;
                    return varyings;
                }
        ";
        let fs = r"
                float2 main(in const Varyings varyings, out half4 color) {
                    color = varyings.color;
                    return varyings.position;
                }
        ";
        for unpremul in [false, true] {
            let at = if unpremul {
                AlphaType::Unpremul
            } else {
                AlphaType::Premul
            };
            let vs = if unpremul { unpremul_vs } else { premul_vs };
            for spin in [false, true] {
                let mut cs = ColorSpace::new_srgb();
                if spin {
                    cs = cs.with_color_spin();
                }
                self.specs[Self::spec_index(unpremul, spin)] =
                    make_spec(&attributes, 24, &varyings, vs, fs, Some(cs), at);
            }
        }
        let pts = [
            Point::new(Self::RECT.left(), 0.0),
            Point::new(Self::RECT.center_x(), 0.0),
        ];
        let colors = [
            Color4f::from_color(Color::WHITE),
            Color4f::new(0.0, 0.0, 0.0, 0.0),
        ];
        self.shader = gradient_shaders::linear_gradient(
            (pts[0], pts[1]),
            &Gradient::new(
                Colors::new(&colors, None, TileMode::Mirror, None),
                Interpolation::default(),
            ),
            None,
        );

        // Vertex {pos, color}, 24 bytes.
        let (l, t, r, b) = (
            Self::RECT.left(),
            Self::RECT.top(),
            Self::RECT.right(),
            Self::RECT.bottom(),
        );
        let quad = [
            ([l, t], [1.0, 0.0, 0.0, 1.0]),
            ([r, t], [0.0, 1.0, 0.0, 0.0]),
            ([l, b], [1.0, 1.0, 0.0, 0.0]),
            ([r, b], [0.0, 0.0, 1.0, 1.0]),
        ];
        let mut data = Vec::new();
        for (pos, color) in quad {
            data.extend(f32_bytes(&pos));
            data.extend(f32_bytes(&color));
        }
        self.vb = Some(make_vertex_buffer(Some(&data), data.len()));
    }

    // Port of: gm/mesh.cpp#L413-L470 (chrome/m156)
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        // Force an intermediate surface if the canvas is in "legacy" mode.
        let surface_info = canvas.image_info();
        let mut surface = None;
        if surface_info.color_space().is_none() {
            let info = surface_info.with_color_space(Some(ColorSpace::new_srgb()));
            let Some(mut created) = surfaces::raster(&info, None, None) else {
                // This GM won't work on configs that use a recording canvas.
                return DrawResult::Skip;
            };
            created.canvas().clear(Color::WHITE);
            surface = Some(created);
        }
        let c: &Canvas = match surface.as_mut() {
            Some(surface) => surface.canvas(),
            None => canvas,
        };
        for use_shader in [false, true] {
            for unpremul in [false, true] {
                c.save();
                for spin in [false, true] {
                    let Some(spec) = self.specs[Self::spec_index(unpremul, spin)].clone() else {
                        return DrawResult::Fail;
                    };
                    let result = Mesh::make(
                        Some(spec),
                        Mode::TriangleStrip,
                        self.vb.clone(),
                        4,
                        0,
                        None,
                        &[],
                        Self::RECT,
                    );
                    if !result.mesh.is_valid() {
                        eprintln!("Mesh creation failed: {}", result.error);
                        return DrawResult::Fail;
                    }

                    let mut paint = Paint::default();
                    paint.set_shader(if use_shader {
                        self.shader.clone()
                    } else {
                        None
                    });
                    let mode = if use_shader {
                        BlendMode::Modulate
                    } else {
                        BlendMode::Dst
                    };
                    c.draw_mesh(&result.mesh, Blender::mode(mode), &paint);

                    c.translate((0.0, Self::RECT.height() + 10.0));
                }
                c.restore();
                c.translate((Self::RECT.width() + 10.0, 0.0));
                c.save();
            }
        }
        if let Some(mut surface) = surface {
            surface.draw(canvas, (0.0, 0.0), SamplingOptions::default(), None);
        }
        DrawResult::Ok
    }
}
crate::def_gm!(MeshColorSpaceGM, MeshColorSpaceGm::new());

// Port of: gm/mesh.cpp#L508-L667 (chrome/m156)
struct MeshUniformsGm {
    degrees: f32,
    color: Color4f,
    vb: Option<Arc<VertexBuffer>>,
    spec: Option<Arc<MeshSpecification>>,
    shader: Option<Shader>,
}

impl MeshUniformsGm {
    const RECT: Rect = Rect::from_ltrb(20.0, 20.0, 120.0, 120.0);
    // Our logical tex coords are [0..1] but we insert an arbitrary translation that gets undone
    // with a uniform.
    const COORD_TRANS: (f32, f32) = (75.0, -37.0);
    const GRAD_CENTER: (f32, f32) = (0.3, 0.2);

    fn new() -> Self {
        let mut gm = MeshUniformsGm {
            degrees: 0.0,
            color: Color4f::new(0.0, 0.0, 0.0, 1.0),
            vb: None,
            spec: None,
            shader: None,
        };
        // The GM animates from the constructor (`onAnimate(0)`). The GM trait has no animation
        // hook, so the time-zero values are the ones drawn.
        gm.animate_at_zero();
        gm
    }

    /// `onAnimate(0)`: `fDegrees` and `fColor` at time zero.
    // Port of: gm/mesh.cpp#L609-L619 (chrome/m156), at `nanos == 0`
    // The float sine of `TimeUtils::SineWave` is taken in single precision, as in C++.
    #[allow(clippy::cast_possible_truncation)]
    fn animate_at_zero(&mut self) {
        // `TimeUtils::SineWave(0, period, phase, 0, 1)`: `0.5*sin(phase*2π/period) + 0.5`.
        let sine_wave = |period: f32, phase: f32| {
            let t = f64::from(phase) * 2.0 * f64::from(std::f32::consts::PI) / f64::from(period);
            0.5 * (t as f32).sin() + 0.5
        };
        self.degrees = 0.0 * 360.0 / 10.0 + 45.0;
        // prime number periods, like locusts.
        self.color = Color4f::new(
            sine_wave(13.0, 0.0),
            sine_wave(23.0, 5.0),
            sine_wave(11.0, 0.0),
            1.0,
        );
    }
}

impl GM for MeshUniformsGm {
    fn name(&self) -> String {
        "custommesh_uniforms".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(140, 250)
    }

    // Port of: gm/mesh.cpp#L525-L585 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        let attributes = [
            Attribute {
                ty: AttributeType::Float2,
                offset: 0,
                name: "pos".to_owned(),
            },
            Attribute {
                ty: AttributeType::Float2,
                offset: 8,
                name: "coords".to_owned(),
            },
        ];
        let varyings = [Varying {
            ty: VaryingType::Float2,
            name: "coords".to_owned(),
        }];
        // To exercise shared VS/FS uniforms we have a matrix that is applied twice, once in each
        // stage.
        let vs = r"
                uniform float t[2];
                uniform half3x3 m;
                Varyings main(in const Attributes attributes) {
                    Varyings varyings;
                    varyings.coords   = (m*float3(attributes.coords + float2(t[0], t[1]), 1)).xy;
                    varyings.position = attributes.pos;
                    return varyings;
                }
        ";
        let fs = r"
                uniform half3x3 m;
                layout(color) uniform half4 color;
                float2 main(const Varyings varyings, out half4 c) {
                    c = color;
                    return (m*float3(varyings.coords, 1)).xy;
                }
        ";
        self.spec = make_spec(
            &attributes,
            16,
            &varyings,
            vs,
            fs,
            Some(ColorSpace::new_srgb()),
            AlphaType::Premul,
        );

        let colors = [
            Color4f::from_color(Color::WHITE),
            Color4f::from_color(Color::BLACK),
        ];
        self.shader = radial_gradient(
            Point::new(Self::GRAD_CENTER.0, Self::GRAD_CENTER.1),
            0.4,
            &colors,
            TileMode::Mirror,
        );

        // Vertex {pos, tex}, 16 bytes.
        let coord_rect = Rect::from_xywh(Self::COORD_TRANS.0, Self::COORD_TRANS.1, 1.0, 1.0);
        let (l, t, r, b) = (
            Self::RECT.left(),
            Self::RECT.top(),
            Self::RECT.right(),
            Self::RECT.bottom(),
        );
        let (cl, ct, cr, cb) = (
            coord_rect.left(),
            coord_rect.top(),
            coord_rect.right(),
            coord_rect.bottom(),
        );
        let quad = [
            ([l, t], [cl, ct]),
            ([r, t], [cr, ct]),
            ([l, b], [cl, cb]),
            ([r, b], [cr, cb]),
        ];
        let mut data = Vec::new();
        for (pos, tex) in quad {
            data.extend(f32_bytes(&pos));
            data.extend(f32_bytes(&tex));
        }
        self.vb = Some(make_vertex_buffer(Some(&data), data.len()));
    }

    // Port of: gm/mesh.cpp#L587-L624 (chrome/m156)
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        let matrices = [
            // self inverse so no effect.
            Matrix::new_all(-1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0),
            Matrix::rotate_deg_pivot(self.degrees / 2.0, (0.5, 0.5)),
        ];
        let Some(spec) = self.spec.clone() else {
            return DrawResult::Fail;
        };
        for m in matrices {
            let mut unis = vec![0u8; spec.uniform_size()];
            let trans = (-Self::COORD_TRANS.0, -Self::COORD_TRANS.1);
            let Some(u) = spec.find_uniform("t") else {
                return DrawResult::Fail;
            };
            copy_at(&mut unis, u.offset(), &f32_bytes(&[trans.0, trans.1]));
            let Some(u) = spec.find_uniform("m") else {
                return DrawResult::Fail;
            };
            let mut offset = u.offset();
            for col in 0..3 {
                for row in 0..3 {
                    copy_at(&mut unis, offset, &f32_bytes(&[m.rc(row, col)]));
                    offset += 4;
                }
            }
            let Some(u) = spec.find_uniform("color") else {
                return DrawResult::Fail;
            };
            copy_at(
                &mut unis,
                u.offset(),
                &f32_bytes(&[self.color.r, self.color.g, self.color.b, self.color.a]),
            );

            let result = Mesh::make(
                Some(spec.clone()),
                Mode::TriangleStrip,
                self.vb.clone(),
                4,
                0,
                Some(Data::new_copy(&unis)),
                &[],
                Self::RECT,
            );
            if !result.mesh.is_valid() {
                eprintln!("Mesh creation failed: {}", result.error);
                return DrawResult::Fail;
            }
            let mut paint = Paint::default();
            paint.set_shader(self.shader.clone());
            canvas.draw_mesh(&result.mesh, Blender::mode(BlendMode::Modulate), &paint);
            canvas.translate((0.0, Self::RECT.height() + 10.0));
        }
        DrawResult::Ok
    }
}
crate::def_gm!(MeshUniformsGM_ = "MeshUniformsGM()", MeshUniformsGm::new());

// Port of: gm/mesh.cpp#L669-L881 (chrome/m156)
struct MeshUpdateGm {
    spec: Option<Arc<MeshSpecification>>,
    shader: Option<Shader>,
}

impl MeshUpdateGm {
    const WIDTH: i32 = 270;
    const HEIGHT: i32 = 490;
    const VERTICAL_PADDING: f32 = 10.0;

    fn new() -> Self {
        MeshUpdateGm {
            spec: None,
            shader: None,
        }
    }
}

impl GM for MeshUpdateGm {
    fn name(&self) -> String {
        "mesh_updates".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(Self::WIDTH, Self::HEIGHT)
    }

    // Port of: gm/mesh.cpp#L676-L713 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        let attributes = [
            Attribute {
                ty: AttributeType::Float2,
                offset: 0,
                name: "pos".to_owned(),
            },
            Attribute {
                ty: AttributeType::Float2,
                offset: 8,
                name: "coords".to_owned(),
            },
        ];
        let varyings = [Varying {
            ty: VaryingType::Float2,
            name: "coords".to_owned(),
        }];
        let vs = r"
                Varyings main(const in Attributes attributes) {
                    Varyings varyings;
                    varyings.coords   = attributes.coords;
                    varyings.position = attributes.pos;
                    return varyings;
                }
        ";
        let fs = r"
                float2 main(const Varyings varyings) { return varyings.coords; }
        ";
        self.spec = make_spec(
            &attributes,
            16,
            &varyings,
            vs,
            fs,
            Some(ColorSpace::new_srgb()),
            AlphaType::Premul,
        );

        // The image is 2x2 BGRA premultiplied, the colors below as SK_Color values.
        let colors: [u32; 4] = [0xFFFF_FF00, 0xFFFF_00FF, 0xFF00_FFFF, 0xFFFF_FFFF];
        let info = ImageInfo::new(
            (2, 2),
            skia_rust_core::color_type::ColorType::BGRA8888,
            AlphaType::Premul,
            None,
        );
        let pixels = u32_bytes(&colors);
        let mut pixels = pixels;
        let pixmap = Pixmap::new(&info, &mut pixels, 8).expect("the 2x2 pixmap is valid");
        let image = skia_rust_core::images::raster_from_pixmap_copy(&pixmap)
            .expect("the 2x2 image is valid");
        self.shader = image.to_shader(
            (TileMode::Clamp, TileMode::Clamp),
            SamplingOptions::default(),
            None,
        );
    }

    // Port of: gm/mesh.cpp#L727-L880 (chrome/m156), on the CPU: both passes use CPU buffers.
    #[allow(clippy::too_many_lines)] // one function in Skia, ported as written
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        // How many rects worth of storage is in the index buffer, and how many times it is
        // updated (wrapping to the start of the buffer when past `IB_RECTS`).
        const IB_RECTS: usize = 2;
        const NUM_IB_UPDATES: usize = 3;
        canvas.clear(Color::BLACK);
        let Some(spec) = self.spec.clone() else {
            return DrawResult::Fail;
        };

        let mut paint = Paint::default();
        paint.set_shader(self.shader.clone());
        let r = Rect::from_xywh(10.0, 10.0, 50.0, 50.0);

        // We test updating CPU and GPU buffers. Without a GPU context both are CPU buffers.
        for _pass in 0..2 {
            // How many rects worth of storage is in the vertex buffer?
            const VB_RECTS: usize = 2;
            // How many times do we update the vertex buffer? Wraps to start of buffer if >
            // VB_RECTS.
            const UPDATES_RECTS: usize = 3;
            let vb = make_vertex_buffer(None, VB_RECTS * 6 * 16);
            let mut bounds = Rect::new_empty();
            for i in 0..UPDATES_RECTS {
                let p = r.with_offset((100.0 * index_f32(i), 0.0));
                if i > 0 {
                    bounds.join(p);
                } else {
                    bounds = p;
                }
                let mut t = Rect::from_wh(2.0, 2.0).to_quad(None);
                Matrix::rotate_deg_pivot(90.0 * index_f32(i), (1.0, 1.0))
                    .map_points_inplace(&mut t);
                let vertices = [
                    ([p.left(), p.top()], t[0]),
                    ([p.left(), p.bottom()], t[3]),
                    ([p.right(), p.top()], t[1]),
                    ([p.right(), p.top()], t[1]),
                    ([p.left(), p.bottom()], t[3]),
                    ([p.right(), p.bottom()], t[2]),
                ];
                let mut bytes = Vec::new();
                for (pos, tex) in vertices {
                    bytes.extend(f32_bytes(&pos));
                    bytes.extend(f32_bytes(&[tex.x, tex.y]));
                }
                let offset = 6 * (i % VB_RECTS) * 16;
                assert!(vb.update(&bytes, offset));
                let rect_count = (i + 1).min(VB_RECTS);
                let result = Mesh::make(
                    Some(spec.clone()),
                    Mode::Triangles,
                    Some(vb.clone()),
                    6 * rect_count,
                    0,
                    None,
                    &[],
                    bounds,
                );
                if !result.mesh.is_valid() {
                    eprintln!("Mesh creation failed: {}", result.error);
                    return DrawResult::Fail;
                }
                canvas.draw_mesh(&result.mesh, Blender::mode(BlendMode::Dst), &paint);
                canvas.translate((0.0, r.height() + Self::VERTICAL_PADDING));
            }

            // Now test updating an IB.
            // Make the vertex buffer large enough to hold all the rects and populate.
            let vb = make_vertex_buffer(None, NUM_IB_UPDATES * 4 * 16);
            for i in 0..NUM_IB_UPDATES {
                let rect = r.with_offset((100.0 * index_f32(i), 0.0));
                let p = rect.to_quad(None);
                if i > 0 {
                    bounds.join(rect);
                } else {
                    bounds = rect;
                }
                let mut t = Rect::from_wh(2.0, 2.0).to_quad(None);
                Matrix::rotate_deg_pivot(90.0 * index_f32(i), (1.0, 1.0))
                    .map_points_inplace(&mut t);
                let mut bytes = Vec::new();
                for (pos, tex) in p.iter().zip(&t) {
                    bytes.extend(f32_bytes(&[pos.x, pos.y]));
                    bytes.extend(f32_bytes(&[tex.x, tex.y]));
                }
                assert!(vb.update(&bytes, i * 4 * 16));
            }
            let ib = make_index_buffer(None, IB_RECTS * 6 * 2);
            for i in 0..NUM_IB_UPDATES {
                let base = u16::try_from(4 * i).expect("a small index");
                let indices: [u16; 6] = [base, base + 3, base + 1, base + 1, base + 3, base + 2];
                let offset = 6 * (i % IB_RECTS) * 2;
                assert!(ib.update(&u16_bytes(&indices), offset));
                let result = Mesh::make_indexed(
                    Some(spec.clone()),
                    Mode::Triangles,
                    Some(vb.clone()),
                    4 * NUM_IB_UPDATES,
                    0,
                    Some(ib.clone()),
                    6,
                    offset,
                    None,
                    &[],
                    bounds,
                );
                if !result.mesh.is_valid() {
                    eprintln!("Mesh creation failed: {}", result.error);
                    return DrawResult::Fail;
                }
                canvas.draw_mesh(&result.mesh, Blender::mode(BlendMode::Dst), &paint);
            }
            canvas.translate((0.0, r.height() + Self::VERTICAL_PADDING));
        }
        DrawResult::Ok
    }
}
crate::def_gm!(MeshUpdateGM_ = "MeshUpdateGM()", MeshUpdateGm::new());

// Port of: gm/mesh.cpp#L883-L1026 (chrome/m156)
struct MeshZeroInitGm {
    spec: [Option<Arc<MeshSpecification>>; 2],
}

impl MeshZeroInitGm {
    fn new() -> Self {
        MeshZeroInitGm { spec: [None, None] }
    }
}

impl GM for MeshZeroInitGm {
    fn name(&self) -> String {
        "mesh_zero_init".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(90, 30)
    }

    // Port of: gm/mesh.cpp#L889-L925 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        let attributes1 = [
            Attribute {
                ty: AttributeType::UByte4Unorm,
                offset: 0,
                name: "color".to_owned(),
            },
            Attribute {
                ty: AttributeType::Float2,
                offset: 4,
                name: "pos".to_owned(),
            },
        ];
        let varyings = [Varying {
            ty: VaryingType::Half4,
            name: "color".to_owned(),
        }];
        let vs = r"
                Varyings main(const in Attributes attributes) {
                    Varyings varyings;
                    varyings.color    = attributes.color;
                    varyings.position = attributes.pos;
                    return varyings;
                }
        ";
        let fs = r"
                float2 main(const Varyings varyings, out half4 color) {
                    color = varyings.color;
                    return varyings.position;
                }
        ";
        self.spec[0] = make_spec(
            &attributes1,
            12,
            &varyings,
            vs,
            fs,
            Some(ColorSpace::new_srgb()),
            AlphaType::Premul,
        );
        self.spec[1] = make_spec(
            &attributes1,
            12,
            &varyings,
            vs,
            fs,
            Some(ColorSpace::new_srgb()),
            AlphaType::Premul,
        );
    }

    // Port of: gm/mesh.cpp#L942-L1025 (chrome/m156), on the CPU: both passes use CPU buffers.
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        const TRI: [[f32; 2]; 3] = [[10.0, 10.0], [20.0, 10.0], [10.0, 20.0]];
        // The zero will come from the uninit part of the buffer.
        const TI_INDICES: [u16; 2] = [1, 2];
        for _pass in 0..2 {
            for i in 0..2 {
                let Some(spec) = self.spec[i].clone() else {
                    return DrawResult::Fail;
                };
                let Some(pos) = spec.find_attribute("pos") else {
                    return DrawResult::Fail;
                };
                let pos_offset = pos.offset;
                let stride = spec.stride();
                let vb = make_vertex_buffer(None, stride * TRI.len());
                for (j, point) in TRI.iter().enumerate() {
                    assert!(vb.update(&f32_bytes(point), stride * j + pos_offset));
                }
                // The first time we make the indices be 0,1,2 using the zero'ed buffer for the
                // first. However, because uploads must be 4 byte aligned it's actually 0,0,1,2.
                // The second time we upload 1,2 to beginning of the buffer to form 1,2,0.
                let index_upload_offset = if i == 0 { 4 } else { 0 };
                let index_mesh_offset = if i == 0 { 2 } else { 0 };
                let ib = make_index_buffer(None, 2 * 4);
                assert!(ib.update(&u16_bytes(&TI_INDICES), index_upload_offset));
                let bounds = Rect::bounds_or_empty(&[
                    Point::new(10.0, 10.0),
                    Point::new(20.0, 10.0),
                    Point::new(10.0, 20.0),
                ]);
                let result = Mesh::make_indexed(
                    Some(spec),
                    Mode::Triangles,
                    Some(vb),
                    TRI.len(),
                    0,
                    Some(ib),
                    TI_INDICES.len() + 1,
                    index_mesh_offset,
                    None,
                    &[],
                    bounds,
                );
                if !result.mesh.is_valid() {
                    eprintln!("Mesh creation failed: {}", result.error);
                    return DrawResult::Fail;
                }
                let paint = Paint::default();
                // The color will be transparent black. Set the blender to kDstOver so when
                // combined with the paint's opaque black we get opaque black.
                canvas.draw_mesh(&result.mesh, Blender::mode(BlendMode::DstOver), &paint);
                canvas.translate((bounds.width() + 10.0, 0.0));
            }
        }
        DrawResult::Ok
    }
}
crate::def_gm!(MeshZeroInitGM_ = "MeshZeroInitGM()", MeshZeroInitGm::new());

// Port of: gm/mesh.cpp#L1030-L1216 (chrome/m156)
struct PictureMeshGm {
    vb: Option<Arc<VertexBuffer>>,
    ib: Option<Arc<IndexBuffer>>,
    spec: Option<Arc<MeshSpecification>>,
    shader: Option<Shader>,
}

impl PictureMeshGm {
    const RECT: Rect = Rect::from_xywh(0.0, 0.0, 40.0, 40.0);

    fn new() -> Self {
        PictureMeshGm {
            vb: None,
            ib: None,
            spec: None,
            shader: None,
        }
    }
}

impl GM for PictureMeshGm {
    fn name(&self) -> String {
        "picture_mesh".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(390, 90)
    }

    // Port of: gm/mesh.cpp#L1041-L1094 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        let attributes = [Attribute {
            ty: AttributeType::Float2,
            offset: 0,
            name: "pos".to_owned(),
        }];
        let varyings = [Varying {
            ty: VaryingType::Float2,
            name: "coords".to_owned(),
        }];
        let vs = r"
                Varyings main(in const Attributes attributes) {
                    Varyings varyings;
                    varyings.position = attributes.pos;
                    return varyings;
                }
        ";
        let fs = format!(
            r"
                uniform float2 offset;
                float2 main(const Varyings varyings, out float4 color) {{
                    float2 tl = float2({:.6}, {:.6});
                    float2 wh = float2({:.6}, {:.6});
                    float2 c = tl + wh/2;
                    float  r = length(wh)/4;
                    color.rba = float3(1);
                    color.g = min(1, length(varyings.position - c + offset) / r);
                    return varyings.position;
                }}
        ",
            Self::RECT.x(),
            Self::RECT.y(),
            Self::RECT.width(),
            Self::RECT.height()
        );
        self.spec = make_spec(
            &attributes,
            8,
            &varyings,
            vs,
            &fs,
            Some(ColorSpace::new_srgb().with_color_spin()),
            AlphaType::Premul,
        );

        // Vertex {pos}: the first vertex is skipped by the indices.
        let quad: [[f32; 2]; 7] = [
            [1000.0, 1000.0], // skip
            [Self::RECT.left(), Self::RECT.top()],
            [Self::RECT.right(), Self::RECT.top()],
            [Self::RECT.left(), Self::RECT.bottom()],
            [Self::RECT.right(), Self::RECT.bottom()],
            [Self::RECT.left(), Self::RECT.bottom()],
            [Self::RECT.right(), Self::RECT.top()],
        ];
        let mut quad_bytes = Vec::new();
        for v in quad {
            quad_bytes.extend(f32_bytes(&v));
        }
        self.vb = Some(make_vertex_buffer(Some(&quad_bytes), quad_bytes.len()));
        let indices: [u16; 8] = [1000, 2000, 1, 2, 3, 4, 5, 6];
        let index_bytes = u16_bytes(&indices);
        self.ib = Some(make_index_buffer(Some(&index_bytes), index_bytes.len()));

        let mut random = Random::new(0);
        let mut colors = [Color4f::new(0.0, 0.0, 0.0, 1.0); 6];
        for color in colors.iter_mut().take(5) {
            *color = Color4f::new(random.next_f(), random.next_f(), random.next_f(), 1.0);
        }
        colors[5] = colors[0];
        let interpolation = Interpolation {
            color_space: GradientColorSpace::HSL,
            ..Interpolation::default()
        };
        self.shader = gradient_shaders::sweep_gradient(
            Self::RECT.center(),
            (0.0, 360.0),
            &Gradient::new(
                Colors::new(
                    &colors,
                    None,
                    TileMode::Repeat,
                    Some(ColorSpace::new_srgb()),
                ),
                interpolation,
            ),
            None,
        );
    }

    // Port of: gm/mesh.cpp#L1096-L1214 (chrome/m156)
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let mut paint = Paint::default();
        paint.set_shader(self.shader.clone());
        for picture in [false, true] {
            canvas.save();
            for gpu in [false, true] {
                // `CopyVertexBuffer(dc, …)` with no context makes a CPU copy of the buffer.
                let (vb, ib) = if gpu {
                    (
                        meshes::copy_vertex_buffer(self.vb.as_ref()),
                        meshes::copy_index_buffer(self.ib.as_ref()),
                    )
                } else {
                    (self.vb.clone(), self.ib.clone())
                };
                let (Some(vb), Some(ib)) = (vb, ib) else {
                    return DrawResult::Fail;
                };
                let mut offset = [8.0f32, 8.0];
                for i in 0..4 {
                    let uniforms = Data::new_copy(&f32_bytes(&offset));
                    let r = match i {
                        0 => Mesh::make(
                            self.spec.clone(),
                            Mode::Triangles,
                            Some(vb.clone()),
                            6,
                            8,
                            Some(uniforms),
                            &[],
                            Self::RECT,
                        ),
                        1 => Mesh::make(
                            self.spec.clone(),
                            Mode::TriangleStrip,
                            Some(vb.clone()),
                            4,
                            8,
                            Some(uniforms),
                            &[],
                            Self::RECT,
                        ),
                        2 => Mesh::make_indexed(
                            self.spec.clone(),
                            Mode::Triangles,
                            Some(vb.clone()),
                            7,
                            0,
                            Some(ib.clone()),
                            6,
                            2 * 2,
                            Some(uniforms),
                            &[],
                            Self::RECT,
                        ),
                        _ => Mesh::make_indexed(
                            self.spec.clone(),
                            Mode::TriangleStrip,
                            Some(vb.clone()),
                            7,
                            0,
                            Some(ib.clone()),
                            6,
                            2 * 2,
                            Some(uniforms),
                            &[],
                            Self::RECT,
                        ),
                    };
                    if !r.mesh.is_valid() {
                        *error_msg = r.error;
                        return DrawResult::Fail;
                    }
                    let draw = |c: &Canvas| {
                        c.draw_mesh(&r.mesh, Blender::mode(BlendMode::Difference), &paint);
                    };
                    if picture {
                        let mut recorder = skia_rust_core::picture_recorder::PictureRecorder::new();
                        let picture_canvas =
                            recorder.begin_recording(Rect::from_wh(390.0, 90.0), false);
                        draw(picture_canvas);
                        let Some(picture_out) = recorder.finish_recording_as_picture(None) else {
                            return DrawResult::Fail;
                        };
                        canvas.draw_picture(&picture_out, None, None);
                    } else {
                        draw(canvas);
                    }
                    offset[i % 2] *= -1.0;
                    canvas.translate((Self::RECT.width() + 10.0, 0.0));
                }
            }
            canvas.restore();
            canvas.translate((0.0, Self::RECT.height() + 10.0));
        }
        DrawResult::Ok
    }
}
crate::def_gm!(PictureMesh_ = "PictureMesh()", PictureMeshGm::new());

/// `MeshWithShadersGM::Type`.
// Port of: gm/mesh.cpp#L1219-L1224 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MeshWithShadersType {
    Image,
    PaintColor,
    PaintImage,
    Effects,
}

/// `MeshWithShadersGM::Vertex`: `{ float pos[2]; float uv[2]; }`.
// Port of: gm/mesh.cpp#L1385-L1388 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq)]
struct ShaderVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

impl ShaderVertex {
    /// `sizeof(Vertex)`.
    const STRIDE: usize = 16;

    /// The vertex bytes as the GM's `memcpy` lays them out.
    fn to_bytes(self) -> Vec<u8> {
        f32_bytes(&[self.pos[0], self.pos[1], self.uv[0], self.uv[1]])
    }
}

// Port of: gm/mesh.cpp#L1218-L1484 (chrome/m156), class MeshWithShadersGM
struct MeshWithShadersGm {
    kind: MeshWithShadersType,
    verts: Vec<ShaderVertex>,
    indices: Vec<u16>,
    spec: Option<Arc<MeshSpecification>>,
    shader1: Option<Shader>,
    shader2: Option<Shader>,
    paint_shader: Option<Shader>,
    color_filter: Option<ColorFilter>,
    blender: Option<Blender>,
    vb: Option<Arc<VertexBuffer>>,
    ib: Option<Arc<IndexBuffer>>,
}

impl MeshWithShadersGm {
    /// `kRect`.
    // Port of: gm/mesh.cpp#L1378-L1378 (chrome/m156)
    const RECT: Rect = Rect::from_ltrb(20.0, 20.0, 300.0, 300.0);
    /// `kUV`.
    // Port of: gm/mesh.cpp#L1379-L1379 (chrome/m156)
    const UV: Rect = Rect::from_ltrb(0.0, 0.0, 128.0, 128.0);
    /// `kMeshSize`.
    // Port of: gm/mesh.cpp#L1380-L1380 (chrome/m156)
    const MESH_SIZE: usize = 16;
    /// `kRippleSize`.
    // Port of: gm/mesh.cpp#L1381-L1381 (chrome/m156)
    const RIPPLE_SIZE: f32 = 6.0;

    // Port of: gm/mesh.cpp#L1227-L1248 (chrome/m156), MeshWithShadersGM(Type)
    fn new(kind: MeshWithShadersType) -> Self {
        let mut gm = Self {
            kind,
            verts: Vec::new(),
            indices: Vec::new(),
            spec: None,
            shader1: None,
            shader2: None,
            paint_shader: None,
            color_filter: None,
            blender: None,
            vb: None,
            ib: None,
        };
        // Create a grid of evenly spaced points for our mesh
        gm.animate_at(0.0);

        // Create an index buffer of triangles over our point mesh.
        for y in 0..Self::MESH_SIZE - 1 {
            for x in 0..Self::MESH_SIZE - 1 {
                let tl = u16::try_from(y * Self::MESH_SIZE + x).expect("small mesh");
                let tr = u16::try_from(y * Self::MESH_SIZE + x + 1).expect("small mesh");
                let bl = u16::try_from((y + 1) * Self::MESH_SIZE + x).expect("small mesh");
                let br = u16::try_from((y + 1) * Self::MESH_SIZE + x + 1).expect("small mesh");

                gm.indices.extend_from_slice(&[tl, tr, bl]);
                gm.indices.extend_from_slice(&[br, bl, tr]);
            }
        }
        gm
    }

    /// `ensureBuffers()`: makes the CPU buffers that do not exist yet.
    // Port of: gm/mesh.cpp#L1447-L1455 (chrome/m156), ensureBuffers
    fn ensure_buffers(&mut self) {
        if self.vb.is_none() {
            let bytes: Vec<u8> = self.verts.iter().flat_map(|v| v.to_bytes()).collect();
            self.vb = Some(make_vertex_buffer(Some(&bytes), bytes.len()));
        }
        if self.ib.is_none() {
            let bytes = u16_bytes(&self.indices);
            self.ib = Some(make_index_buffer(Some(&bytes), bytes.len()));
        }
    }

    // Port of: gm/mesh.cpp#L1251-L1262 (chrome/m156), onOnceBeforeDraw (the specification)
    fn make_spec(&mut self) {
        let attributes = [
            Attribute {
                ty: AttributeType::Float2,
                offset: 0,
                name: "position".to_owned(),
            },
            Attribute {
                ty: AttributeType::Float2,
                offset: 8,
                name: "uv".to_owned(),
            },
        ];
        let varyings = [Varying {
            ty: VaryingType::Float2,
            name: "uv".to_owned(),
        }];
        let vs = r"
                    Varyings main(const in Attributes attributes) {
                        Varyings varyings;
                        varyings.uv       = attributes.uv;
                        varyings.position = attributes.position;
                        return varyings;
                    }
            ";
        let fs = r"
                    uniform shader myShader1;
                    uniform shader myShader2;
                    uniform colorFilter myColorFilter;
                    uniform blender myBlend;

                    float2 main(const in Varyings varyings, out half4 color) {
                        half4 color1 = myShader1.eval(varyings.uv);
                        half4 color2 = myShader2.eval(varyings.uv);

                        // Apply a inverse color filter to the first image.
                        color1 = myColorFilter.eval(color1);

                        // Fade in the second image horizontally, leveraging the UVs.
                        color2 *= varyings.uv.x / 128.0;

                        // Combine the two images by using a blender (set to dst-over).
                        color = myBlend.eval(color1, color2);

                        return varyings.uv;
                    }
            ";
        self.spec = make_spec(
            &attributes,
            ShaderVertex::STRIDE,
            &varyings,
            vs,
            fs,
            // The five-argument `SkMeshSpecification::Make` uses the sRGB colour space.
            Some(ColorSpace::new_srgb()),
            AlphaType::Premul,
        );
    }

    /// `GetResourceAsImage(name)->makeShader(SkSamplingOptions(SkFilterMode::kLinear))`.
    fn resource_shader(name: &str) -> Shader {
        crate::tool_utils::get_resource_as_image(name)
            .expect(name)
            .to_shader(None, SamplingOptions::from(FilterMode::Linear), None)
            .expect("a linear image shader")
    }

    /// `onAnimate(nanos)`: the grid of vertices for the time `nanos`.
    // Port of: gm/mesh.cpp#L1288-L1322 (chrome/m156), onAnimate
    // The loops index `x_off` by `y` and `y_off` by `x`, as the C++ does.
    #[allow(clippy::needless_range_loop, clippy::cast_possible_truncation)] // mirrors the double-to-float stores of the vertices
    fn animate_at(&mut self, nanos: f64) {
        // `periodic` goes from zero to 2π every four seconds, then wraps around.
        let mut periodic = nanos / 4_000_000_000.;
        periodic -= periodic.floor();
        periodic *= 2.0 * std::f64::consts::PI;

        let mut x_off = [0.0_f64; Self::MESH_SIZE];
        let mut y_off = [0.0_f64; Self::MESH_SIZE];
        for index in 0..Self::MESH_SIZE {
            x_off[index] = periodic.sin() * f64::from(Self::RIPPLE_SIZE);
            y_off[index] = (periodic + 10.0).sin() * f64::from(Self::RIPPLE_SIZE);
            periodic += 0.8;
        }

        self.verts.clear();
        for y in 0..Self::MESH_SIZE {
            let yf = index_f32(y) / index_f32(Self::MESH_SIZE - 1); // yf = 0 .. 1
            for x in 0..Self::MESH_SIZE {
                let xf = index_f32(x) / index_f32(Self::MESH_SIZE - 1); // xf = 0 .. 1

                // `kRect.left() + xf * kRect.width() + xOff[y]`: float sums, then a double sum
                // that is stored as a float.
                let pos0 = (f64::from(Self::RECT.left() + xf * Self::RECT.width()) + x_off[y]) as f32;
                let pos1 = (f64::from(Self::RECT.top() + yf * Self::RECT.height()) + y_off[x]) as f32;
                let uv0 = Self::UV.left() + xf * Self::UV.width();
                let uv1 = Self::UV.top() + yf * Self::UV.height();
                self.verts.push(ShaderVertex {
                    pos: [pos0, pos1],
                    uv: [uv0, uv1],
                });
            }
        }
    }
}

impl GM for MeshWithShadersGm {
    // Port of: gm/mesh.cpp#L1349-L1360 (chrome/m156), getName
    fn name(&self) -> String {
        match self.kind {
            MeshWithShadersType::Image => "mesh_with_image",
            MeshWithShadersType::Effects => "mesh_with_effects",
            MeshWithShadersType::PaintColor => "mesh_with_paint_color",
            MeshWithShadersType::PaintImage => "mesh_with_paint_image",
        }
        .to_string()
    }

    // Port of: gm/mesh.cpp#L1266-L1266 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(320, 320)
    }

    // Port of: gm/mesh.cpp#L1251-L1347 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.make_spec();

        match self.kind {
            MeshWithShadersType::Image => {
                self.shader1 = Some(Self::resource_shader("images/mandrill_128.png"));
                self.shader2 = None;
                self.color_filter = None;
                self.blender = None;
                self.paint_shader = None;
            }
            MeshWithShadersType::Effects => {
                // uint8_t inverseTable[256]: inverseTable[index] = 255 - index
                let mut inverse_table = [0_u8; 256];
                for (index, entry) in inverse_table.iter_mut().enumerate() {
                    *entry = 255 - u8::try_from(index).expect("index < 256");
                }

                self.shader1 = Some(Self::resource_shader("images/mandrill_128.png"));
                self.shader2 = Some(Self::resource_shader("images/color_wheel.png"));
                self.color_filter = color_filters::table_argb(
                    None,
                    Some(&inverse_table),
                    Some(&inverse_table),
                    Some(&inverse_table),
                );
                self.blender = Some(Blender::mode(BlendMode::DstOver));
                self.paint_shader = None;
            }
            MeshWithShadersType::PaintColor => {
                self.shader1 = None;
                self.shader2 = Some(Self::resource_shader("images/mandrill_128.png"));
                self.color_filter = None;
                self.blender = Some(Blender::mode(BlendMode::Dst));
                self.paint_shader = Some(shaders::color(Color::GREEN));
            }
            MeshWithShadersType::PaintImage => {
                self.shader1 = Some(Self::resource_shader("images/color_wheel.png"));
                self.shader2 = None;
                self.color_filter = None;
                self.blender = None;
                self.paint_shader = Some(Self::resource_shader("images/mandrill_128.png"));
            }
        }
    }

    // Port of: gm/mesh.cpp#L1362-L1369 (chrome/m156), onGpuSetup (raster: no context, so only the
    // buffers are made)
    fn on_gpu_setup(&mut self, _canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        self.ensure_buffers();
        DrawResult::Ok
    }

    // Port of: gm/mesh.cpp#L1427-L1446 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let children = [
            self.shader1.clone().map_or(ChildPtr::Empty, ChildPtr::from),
            self.shader2.clone().map_or(ChildPtr::Empty, ChildPtr::from),
            self.color_filter
                .clone()
                .map_or(ChildPtr::Empty, ChildPtr::from),
            self.blender.clone().map_or(ChildPtr::Empty, ChildPtr::from),
        ];

        self.ensure_buffers();
        let (Some(vb), Some(ib)) = (self.vb.clone(), self.ib.clone()) else {
            return DrawResult::Fail;
        };
        let vertex_bytes: Vec<u8> = self.verts.iter().flat_map(|v| v.to_bytes()).collect();
        vb.update(&vertex_bytes, 0);

        let result = Mesh::make_indexed(
            self.spec.clone(),
            Mode::Triangles,
            Some(vb),
            self.verts.len(),
            0,
            Some(ib),
            self.indices.len(),
            0,
            None,
            &children,
            Self::RECT.with_outset((Self::RIPPLE_SIZE, Self::RIPPLE_SIZE)),
        );
        if !result.mesh.is_valid() {
            *error_msg = format!("Mesh creation failed: {}", result.error);
            return DrawResult::Fail;
        }

        let mut paint = Paint::default();
        paint.set_shader(self.paint_shader.clone());
        canvas.draw_mesh(&result.mesh, Blender::mode(BlendMode::DstOver), &paint);

        DrawResult::Ok
    }
}

// Port of: gm/mesh.cpp#L1486-L1489 (chrome/m156), DEF_GM(return new MeshWithShadersGM(...))
crate::def_gm!(
    MeshWithImage = "MeshWithShadersGM(MeshWithShadersGM::Type::kMeshWithImage)",
    MeshWithShadersGm::new(MeshWithShadersType::Image)
);
crate::def_gm!(
    MeshWithPaintColor = "MeshWithShadersGM(MeshWithShadersGM::Type::kMeshWithPaintColor)",
    MeshWithShadersGm::new(MeshWithShadersType::PaintColor)
);
crate::def_gm!(
    MeshWithPaintImage = "MeshWithShadersGM(MeshWithShadersGM::Type::kMeshWithPaintImage)",
    MeshWithShadersGm::new(MeshWithShadersType::PaintImage)
);
crate::def_gm!(
    MeshWithEffects = "MeshWithShadersGM(MeshWithShadersGM::Type::kMeshWithEffects)",
    MeshWithShadersGm::new(MeshWithShadersType::Effects)
);
