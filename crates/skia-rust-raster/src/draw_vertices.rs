// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkDraw_vertices.cpp

//! `skcpu::Draw::drawVertices` and `drawFixedVertices`: draws the triangles of a
//! [`Vertices`] through a raster pipeline blitter, with per-vertex colors
//! ([`TriColorShader`]) and/or the paint's shader (mapped by the texture coordinates with a
//! [`TransformShader`]).
//!
//! skia-rust:
//! * Skia builds the shader tree and the blitter once, and updates the matrices of the
//!   `SkTriColorShader` and `SkTransformShader` in place between triangles (the pipeline's
//!   contexts point at them). Pipeline contexts are immutable here, so each triangle builds its
//!   own copy of the (identical) shader tree with its matrices and a blitter for it. The stages
//!   and the arithmetic are the same, so the pixels are too.
//! * Skia's `drawFixedVertices` takes the arena the shaders are allocated in
//!   (`outerAlloc`); here the shaders are values and each triangle's blitter has its own arena.
//! * Blenders that are not blend modes need `SkRuntimeEffect` (`SkShaders::Blend(sk_sp<SkBlender>,
//!   ...)`), which is not ported: such a draw with vertex colors draws nothing.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::color::{Color, PMColor4f};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::convert_pixels::convert_pixels;
use skia_rust_core::floating_point::{ieee_float_divide, is_finite_array};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::Point;
use skia_rust_core::point3::Point3;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::{self, TransformShader, TriColorShader};
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::vert_state::VertState;
use skia_rust_core::vertices::Vertices;

use crate::blitter::Blitter;
use crate::draw::Draw;
use crate::raster_clip::RasterClip;
use crate::raster_pipeline_blitter::{RasterPipelineBlitter, create_raster_pipeline_blitter};
use crate::scan::fill_triangle as scan_fill_triangle;

// Port of: src/core/SkDraw_vertices.cpp#L51-L62 (chrome/m156)
fn texture_to_matrix(state: &VertState<'_>, verts: &[Point], texs: &[Point]) -> Option<Matrix> {
    let src = [verts[state.f0], verts[state.f1], verts[state.f2]];
    let dst = [texs[state.f0], texs[state.f1], texs[state.f2]];
    Matrix::poly_to_poly(&src, &dst)
}

/// Converts the `SkColor`s into float colors. The conversion depends on some conditions:
/// - If the pixmap has a dst colorspace, we have to be "color-correct". Do we map into
///   dst-colorspace before or after we interpolate?
/// - We have to decide when to apply per-color alpha (before or after we interpolate)
///
/// For now, we will take a simple approach, but recognize this is just a start:
/// - convert colors into dst colorspace before interpolation (matches gradients)
/// - apply per-color alpha before interpolation (matches old version of vertices)
// Port of: src/core/SkDraw_vertices.cpp#L64-L88 (chrome/m156)
fn convert_colors(
    src: &[Color],
    device_cs: Option<ColorSpace>,
    skip_color_xform: bool,
) -> Vec<PMColor4f> {
    let count = src.len();
    let width = i32::try_from(count).expect("a vertices object has at most i32::MAX vertices");

    // Passing `nullptr` for the destination CS effectively disables color conversion.
    let dst_cs = if skip_color_xform { None } else { device_cs };
    let src_info = ImageInfo::new(
        (width, 1),
        ColorType::BGRA8888,
        AlphaType::Unpremul,
        Some(ColorSpace::new_srgb()),
    );
    let dst_info = ImageInfo::new((width, 1), ColorType::RGBAF32, AlphaType::Premul, dst_cs);
    let src_bytes: Vec<u8> = src
        .iter()
        .flat_map(|&c| u32::from(c).to_le_bytes())
        .collect();
    let mut dst_bytes = vec![0u8; count * 16];
    // SkAssertResult
    assert!(convert_pixels(
        &dst_info,
        &mut dst_bytes,
        0,
        &src_info,
        &src_bytes,
        0
    ));
    dst_bytes
        .as_chunks::<16>()
        .0
        .iter()
        .map(|px| {
            let c = |i: usize| f32::from_ne_bytes([px[i], px[i + 1], px[i + 2], px[i + 3]]);
            PMColor4f {
                r: c(0),
                g: c(4),
                b: c(8),
                a: c(12),
            }
        })
        .collect()
}

// Port of: src/core/SkDraw_vertices.cpp#L90-L96 (chrome/m156)
fn compute_is_opaque(colors: &[Color]) -> bool {
    let mut c: u32 = !0;
    for &color in colors {
        c &= u32::from(color);
    }
    c >> 24 == 0xFF
}

// Port of: src/core/SkDraw_vertices.cpp#L98-L104 (chrome/m156)
fn fill_triangle_2(
    state: &VertState<'_>,
    blitter: &mut dyn Blitter,
    rc: &RasterClip,
    dev2: &[Point],
) {
    let tmp = [dev2[state.f0], dev2[state.f1], dev2[state.f2]];
    scan_fill_triangle(&tmp, rc, blitter);
}

const MAX_CLIPPED_TRIANGLE_POINT_COUNT: usize = 4;

/// `tol` of `fill_triangle_3`: the nudge away from zero, to keep the numerics nice.
const TOL: f32 = 0.05;

// Port of: src/core/SkDraw_vertices.cpp#L106-L176 (chrome/m156)
fn fill_triangle_3(
    state: &VertState<'_>,
    blitter: &mut dyn Blitter,
    rc: &RasterClip,
    dev3: &[Point3],
) {
    // Compute the crossing point (across zero) for the two values, expressed as a
    // normalized 0...1 value. If curr is 0, returns 0. If next is 0, returns 1.
    let compute_t = |curr: f32, next: f32| {
        // Check that 0 is between next and curr.
        debug_assert!((next <= 0.0 && 0.0 < curr) || (curr <= 0.0 && 0.0 < next));
        let t = curr / (curr - next);
        debug_assert!((0.0..=1.0).contains(&t));
        t
    };

    let lerp = |curr: Point3, next: Point3, t: f32| curr + t * (next - curr);

    // tol is the nudge away from zero, to keep the numerics nice.
    // Think of it as our near-clipping-plane (or w-plane).
    let clip = |curr: Point3, next: Point3| {
        // Return the point between curr and next where the fZ value crosses tol.
        // To be (really) perspective correct, we should be computing based on 1/Z, not Z.
        // For now, this is close enough (and faster).
        lerp(curr, next, compute_t(curr.z - TOL, next.z - TOL))
    };

    // Clip a triangle (based on its homogeneous W values), and return the projected polygon.
    // Since we only clip against one "edge"/plane, the max number of points in the clipped
    // polygon is 4.
    let clip_triangle = |dst: &mut [Point; MAX_CLIPPED_TRIANGLE_POINT_COUNT],
                         idx: [usize; 3],
                         pts: &[Point3]|
     -> usize {
        let mut out_points = [Point3::default(); MAX_CLIPPED_TRIANGLE_POINT_COUNT];
        let mut out_p = 0;

        for i in 0..3 {
            let curr = idx[i];
            let next = idx[(i + 1) % 3];
            if pts[curr].z > TOL {
                out_points[out_p] = pts[curr];
                out_p += 1;
                if pts[next].z <= TOL {
                    // curr is IN, next is OUT
                    out_points[out_p] = clip(pts[curr], pts[next]);
                    out_p += 1;
                }
            } else if pts[next].z > TOL {
                // curr is OUT, next is IN
                out_points[out_p] = clip(pts[curr], pts[next]);
                out_p += 1;
            }
        }

        let count = out_p;
        debug_assert!(count == 0 || count == 3 || count == 4);
        for i in 0..count {
            let scale = ieee_float_divide(1.0, out_points[i].z);
            dst[i].set(out_points[i].x * scale, out_points[i].y * scale);
        }
        count
    };

    let mut tmp = [Point::default(); MAX_CLIPPED_TRIANGLE_POINT_COUNT];
    let idx = [state.f0, state.f1, state.f2];
    let n = clip_triangle(&mut tmp, idx, dev3);
    if n != 0 {
        // TODO: SkScan::FillConvexPoly(tmp, n, ...);
        debug_assert!(n == 3 || n == 4);
        scan_fill_triangle(&tmp, rc, blitter);
        if n == 4 {
            tmp[1] = tmp[2];
            tmp[2] = tmp[3];
            scan_fill_triangle(&tmp, rc, blitter);
        }
    }
}

// Port of: src/core/SkDraw_vertices.cpp#L178-L185 (chrome/m156)
fn fill_triangle(
    state: &VertState<'_>,
    blitter: &mut dyn Blitter,
    rc: &RasterClip,
    dev2: Option<&[Point]>,
    dev3: Option<&[Point3]>,
) {
    if let Some(dev3) = dev3 {
        fill_triangle_3(state, blitter, rc, dev3);
    } else if let Some(dev2) = dev2 {
        fill_triangle_2(state, blitter, rc, dev2);
    }
}

/// What a vertices draw varies per triangle: the shader tree, built from the triangle's
/// [`TriColorShader`] and [`TransformShader`].
struct ShaderTree<'p> {
    /// `triColorShader != nullptr`: whether there are (kept) vertex colors.
    has_colors: bool,
    blender_is_dst: bool,
    blender: &'p Blender,
    paint: &'p Paint,
    /// `paintShader` (before the transform shader wraps it).
    paint_shader: Option<Shader>,
}

impl ShaderTree<'_> {
    /// `applyShaderColorBlend(shader)`: combines the per-vertex colors with `shader` using the
    /// blender. `None` if there is no shader at all, or the blender is not supported.
    // Port of: src/core/SkDraw_vertices.cpp#L248-L276 (chrome/m156)
    fn apply_shader_color_blend(
        &self,
        shader: Option<Shader>,
        tri_color_shader: Option<TriColorShader>,
    ) -> Result<Option<Shader>, ()> {
        let Some(tri_color_shader) = tri_color_shader else {
            debug_assert!(!self.has_colors);
            return Ok(shader);
        };
        let tri_color_shader = Shader::from_base(tri_color_shader);
        if self.blender_is_dst {
            return Ok(Some(tri_color_shader));
        }
        let shader_with_which_to_blend = match shader {
            // When there is no shader then the blender applies to the vertex colors and opaque
            // paint color.
            None => shaders::color_in_space(self.paint.color4f().to_opaque(), None)
                .expect("an opaque paint color is finite"),
            Some(shader) => shader,
        };
        match shaders::blend_blender(self.blender, tri_color_shader, shader_with_which_to_blend) {
            Some(shader) => Ok(Some(shader)),
            // A blender that is not a blend mode needs a runtime effect.
            None => Err(()),
        }
    }

    /// The paint that draws one triangle: `paint` with the shader tree built from the
    /// triangle's shaders. `Err` if the shader cannot be made.
    fn final_paint(
        &self,
        tri_color_shader: Option<TriColorShader>,
        transform_shader: Option<TransformShader>,
    ) -> Result<Paint, ()> {
        let paint_shader = match transform_shader {
            Some(transform_shader) => Some(Shader::from_base(transform_shader)),
            None => self.paint_shader.clone(),
        };
        let blender_shader = self.apply_shader_color_blend(paint_shader, tri_color_shader)?;

        let mut final_paint = self.paint.clone();
        final_paint.set_shader(blender_shader);
        Ok(final_paint)
    }
}

impl Draw<'_> {
    /// Draws the triangles of `vertices` with `paint`; `blender` combines the per-vertex
    /// colors with the paint's shader or opaque color (`drawVertices`). If `skip_color_xform`
    /// is true the vertex colors are assumed to be in the destination color space already.
    // Port of: src/core/SkDraw_vertices.cpp#L311-L354 (chrome/m156)
    #[doc(alias = "drawVertices")]
    pub fn draw_vertices(
        &mut self,
        vertices: &Vertices,
        blender: &Blender,
        paint: &Paint,
        skip_color_xform: bool,
    ) {
        let vertex_count = vertices.vertex_count();
        let index_count = vertices.index_count();

        // abort early if there is nothing to draw
        if vertex_count < 3 || (index_count > 0 && index_count < 3) || self.rc.is_empty() {
            return;
        }
        let Some(ctm_inv) = self.ctm.invert() else {
            return;
        };

        let positions = vertices.positions();
        let mut dev2 = None;
        let mut dev3 = None;

        if self.ctm.has_perspective() {
            let mut mapped = vec![Point3::default(); vertex_count];
            self.ctm
                .map_points_to_homogeneous(&mut mapped, &positions[..vertex_count]);
            // similar to the bounds check for 2d points (below)
            let flat: Vec<f32> = mapped.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
            if !is_finite_array(&flat) {
                return;
            }
            dev3 = Some(mapped);
        } else {
            let mut mapped = vec![Point::default(); vertex_count];
            self.ctm.map_points(&mut mapped, &positions[..vertex_count]);

            if Rect::bounds_or_empty(&mapped).is_empty() {
                return;
            }
            dev2 = Some(mapped);
        }

        self.draw_fixed_vertices(
            vertices,
            blender,
            paint,
            &ctm_inv,
            dev2.as_deref(),
            dev3.as_deref(),
            skip_color_xform,
        );
    }

    /// Draws the triangles of `vertices` whose positions mapped by the CTM are `dev2` (or, with
    /// perspective, whose homogeneous coordinates are `dev3`); `ctm_inverse` is the inverse of the
    /// CTM (`drawFixedVertices`).
    // Port of: src/core/SkDraw_vertices.cpp#L187-L309 (chrome/m156)
    #[doc(alias = "drawFixedVertices")]
    #[allow(clippy::too_many_arguments)] // mirrors drawFixedVertices
    #[allow(clippy::too_many_lines)] // mirrors drawFixedVertices
    pub fn draw_fixed_vertices(
        &mut self,
        vertices: &Vertices,
        blender: &Blender,
        paint: &Paint,
        ctm_inverse: &Matrix,
        dev2: Option<&[Point]>,
        dev3: Option<&[Point3]>,
        skip_color_xform: bool,
    ) {
        let vertex_count = vertices.vertex_count();
        let index_count = vertices.index_count();
        let positions = vertices.positions();
        let mut tex_coords = vertices.tex_coords();
        let indices = vertices.indices();
        let mut colors = vertices.colors();

        let mut paint_shader = paint.shader();

        // `if (paintShader) { if (!texCoords) texCoords = positions; } else texCoords = nullptr;`
        // `texCoords && texCoords != positions` below is "there are separate texture coords".
        if paint_shader.is_none() {
            tex_coords = None;
        }

        let mut blender_is_dst = false;
        // We can simplify things for certain blend modes. This is for speed, and SkShader_Blend
        // itself insists we don't pass kSrc or kDst to it.
        if let Some(bm) = blender.as_base().as_blend_mode()
            && colors.is_some()
        {
            match bm {
                BlendMode::Src => colors = None,
                BlendMode::Dst => {
                    blender_is_dst = true;
                    tex_coords = None;
                    paint_shader = None;
                }
                _ => {}
            }
        }

        // There is a paintShader iff there is texCoords. (`paintShader` without separate texture
        // coords uses the positions.)
        debug_assert!(!(tex_coords.is_some() && paint_shader.is_none()));

        // Explicit texture coords can't contain perspective - only the CTM can.
        let use_perspective = self.ctm.has_perspective();

        let mut dst_colors = Vec::new();
        let mut tri_color_template = None;
        if let Some(colors) = colors {
            dst_colors = convert_colors(
                &colors[..vertex_count],
                self.dst.color_space(),
                skip_color_xform,
            );
            tri_color_template = Some(TriColorShader::new(
                compute_is_opaque(&colors[..vertex_count]),
                use_perspective,
            ));
        }

        // If there are separate texture coords then we need to insert a transform shader to
        // update a matrix derived from each triangle's coords. In that case we will fold the CTM
        // into each update and use an identity matrix.
        let transform_template = match (&paint_shader, tex_coords) {
            (Some(shader), Some(_)) => Some(TransformShader::new(shader.clone(), use_perspective)),
            _ => None,
        };
        let identity = Matrix::new_identity();
        let ctm = if transform_template.is_some() {
            &identity
        } else {
            self.ctm
        };

        let tree = ShaderTree {
            has_colors: tri_color_template.is_some(),
            blender_is_dst,
            blender,
            paint,
            paint_shader,
        };

        let mut state = VertState::new(vertex_count, indices, index_count);
        let vert_proc = state.choose_proc(vertices.mode());
        let props = self.props.copied().unwrap_or_default();
        let rc = self.rc;

        // Skia makes the blitter before the loop and draws nothing if it can't. The shader tree
        // has the same structure for every triangle, so this makes sure the blitter can be made.
        {
            let Ok(first_paint) =
                tree.final_paint(tri_color_template.clone(), transform_template.clone())
            else {
                return;
            };
            let alloc = ArenaAlloc::new();
            let blitter = make_blitter(
                self.dst.reborrow_mut(),
                rc,
                &first_paint,
                ctm,
                &alloc,
                &props,
            );
            if blitter.is_none() {
                return;
            }
        }

        while vert_proc(&mut state) {
            let mut tri_color_shader = tri_color_template.clone();
            if let Some(tri_color_shader) = &mut tri_color_shader
                && !tri_color_shader.update(
                    ctm_inverse,
                    positions,
                    &dst_colors,
                    state.f0,
                    state.f1,
                    state.f2,
                )
            {
                continue;
            }

            let mut transform_shader = transform_template.clone();
            if let (Some(transform_shader), Some(tex_coords)) = (&mut transform_shader, tex_coords)
            {
                let Some(local_m) = texture_to_matrix(&state, positions, tex_coords) else {
                    continue;
                };
                if !transform_shader.update(&Matrix::concat(&local_m, ctm_inverse)) {
                    continue;
                }
            }

            let Ok(final_paint) = tree.final_paint(tri_color_shader, transform_shader) else {
                return;
            };
            let alloc = ArenaAlloc::new();
            let Some(mut blitter) = make_blitter(
                self.dst.reborrow_mut(),
                rc,
                &final_paint,
                ctm,
                &alloc,
                &props,
            ) else {
                return;
            };
            fill_triangle(&state, &mut blitter, rc, dev2, dev3);
        }
    }
}

/// `SkCreateRasterPipelineBlitter(fDst, finalPaint, *ctm, outerAlloc, fRC->clipShader(), props,
/// SkRect::MakeEmpty())`.
fn make_blitter<'b>(
    dst: Pixmap<'b>,
    rc: &RasterClip,
    paint: &Paint,
    ctm: &Matrix,
    alloc: &'b ArenaAlloc,
    props: &SurfaceProps,
) -> Option<RasterPipelineBlitter<'b>> {
    create_raster_pipeline_blitter(
        dst,
        paint,
        ctm,
        alloc,
        rc.clip_shader(),
        props,
        &Rect::new_empty(),
    )
}
