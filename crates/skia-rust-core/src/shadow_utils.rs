// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/utils/SkShadowUtils.h, src/utils/SkShadowUtils.cpp (raster path),
// src/core/SkDevice.cpp (SkDevice::drawShadow) (chrome/m156)

//! Shadow drawing (`SkShadowUtils`): ambient and spot shadows of a path under a disc or
//! directional light, drawn as tessellated meshes where possible and with a blur otherwise.

// The index and count casts here mirror the C++ int indices of the Skia code this file ports;
// every value stays inside the polygon or vertex count, which is bounded by u16::MAX for meshes.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::similar_names, // sNumer/tNumer etc. keep the C++ names
    clippy::many_single_char_names, // p0/p1/v0/v1 keep the C++ names for auditing the port
)]
use bitflags::bitflags;

use crate::blend_mode::BlendMode;
use crate::blender::Blender;
use crate::blur_types::BlurStyle;
use crate::canvas::Canvas;
use crate::color::Color;
use crate::color_filters;
use crate::draw_shadow_info::{
    DrawShadowRec, ambient_blur_radius, ambient_recip_alpha, get_directional_params,
    get_local_bounds, get_spot_params, get_spot_shadow_transform,
};
use crate::gaussian_color_filter::make_gaussian;
use crate::m44::M44;
use crate::mask_filter::MaskFilter;
use crate::matrix::Matrix;
use crate::paint::{Paint, Style};
use crate::path::Path;
use crate::point::{Point, Vector};
use crate::point3::Point3;
use crate::rect::Rect;
use crate::scalar::{SCALAR_NEARLY_ZERO, scalar};
use crate::shadow_tessellator::{make_ambient, make_spot};
use crate::vertices::Vertices;

bitflags! {
    /// Options of [`draw_shadow`] (`SkShadowFlags`).
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[doc(alias = "SkShadowFlags")]
    pub struct ShadowFlags: u32 {
        /// `kNone_ShadowFlag`.
        const NONE = 0x00;
        /// The occluding object is not opaque. Knowing that the occluder is opaque allows
        /// us to cull shadow geometry behind it and improve performance.
        #[doc(alias = "kTransparentOccluder_ShadowFlag")]
        const TRANSPARENT_OCCLUDER = 0x01;
        /// Don't try to use analytic shadows.
        #[doc(alias = "kGeometricOnly_ShadowFlag")]
        const GEOMETRIC_ONLY = 0x02;
        /// Light position represents a direction, light radius is blur radius at elevation 1.
        #[doc(alias = "kDirectionalLight_ShadowFlag")]
        const DIRECTIONAL_LIGHT = 0x04;
        /// Concave paths will only use blur to generate the shadow.
        #[doc(alias = "kConcaveBlurOnly_ShadowFlag")]
        const CONCAVE_BLUR_ONLY = 0x08;
        /// Mask for all shadow flags.
        #[doc(alias = "kAll_ShadowFlag")]
        const ALL = 0x0F;
    }
}

/// `SkDevice::drawShadow`'s argument check (`validate_rec`).
// Port of: src/core/SkDevice.cpp (validate_rec, chrome/m156)
fn validate_rec(rec: &DrawShadowRec) -> bool {
    rec.light_pos.is_finite() && rec.z_plane_params.is_finite() && rec.light_radius.is_finite()
}

/// `tilted`: whether the occluder's plane is tilted.
// Port of: src/utils/SkShadowUtils.cpp (tilted, chrome/m156)
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the NaN-aware negation mirrors SkScalarNearlyZero
fn tilted(z_plane_params: Point3) -> bool {
    !(z_plane_params.x.abs() <= SCALAR_NEARLY_ZERO)
        || !(z_plane_params.y.abs() <= SCALAR_NEARLY_ZERO)
}

/// The alpha byte of a color (`SkColorGetA`).
fn color_alpha(c: Color) -> u32 {
    (u32::from(c) >> 24) & 0xFF
}

/// `fill_shadow_rec`: packs the arguments into a [`DrawShadowRec`], moving the light into local
/// space for point lights.
// Port of: src/utils/SkShadowUtils.cpp#L664-L684 (chrome/m156)
fn fill_shadow_rec(
    z_plane_params: Point3,
    light_pos: Point3,
    light_radius: scalar,
    ambient_color: Color,
    spot_color: Color,
    flags: ShadowFlags,
    ctm: &Matrix,
) -> Option<DrawShadowRec> {
    let mut pt = Point::new(light_pos.x, light_pos.y);
    if !flags.contains(ShadowFlags::DIRECTIONAL_LIGHT) {
        // If light position is in device space, need to transform to local space
        // before applying to SkCanvas.
        let inverse = ctm.invert()?;
        pt = inverse.map_point(pt);
    }
    Some(DrawShadowRec {
        z_plane_params,
        light_pos: Point3::new(pt.x, pt.y, light_pos.z),
        light_radius,
        ambient_color,
        spot_color,
        flags: flags.bits(),
    })
}

/// Draws an offset spot shadow and outlining ambient shadow for `path` using a disc light, on
/// the canvas (`SkShadowUtils::DrawShadow`).
// Port of: src/utils/SkShadowUtils.cpp#L686-L697 (chrome/m156)
#[doc(alias = "DrawShadow")]
#[allow(clippy::too_many_arguments)] // mirrors SkShadowUtils::DrawShadow's signature
pub fn draw_shadow(
    canvas: &Canvas,
    path: &Path,
    z_plane_params: impl Into<Point3>,
    light_pos: impl Into<Point3>,
    light_radius: scalar,
    ambient_color: impl Into<Color>,
    spot_color: impl Into<Color>,
    flags: impl Into<Option<ShadowFlags>>,
) {
    let flags = flags.into().unwrap_or(ShadowFlags::NONE);
    let ctm = canvas.total_matrix();
    let Some(rec) = fill_shadow_rec(
        z_plane_params.into(),
        light_pos.into(),
        light_radius,
        ambient_color.into(),
        spot_color.into(),
        flags,
        &ctm,
    ) else {
        return;
    };
    private_draw_shadow_rec(canvas, path, &rec);
}

/// Generates the bounding box for shadows relative to `path`, including both the ambient and spot
/// shadow bounds (`SkShadowUtils::GetLocalBounds`). Returns `None` if the shadow cannot be placed.
// Port of: src/utils/SkShadowUtils.cpp#L699-L713 (chrome/m156)
#[doc(alias = "GetLocalBounds")]
#[must_use]
pub fn local_bounds(
    ctm: &Matrix,
    path: &Path,
    z_plane_params: impl Into<Point3>,
    light_pos: impl Into<Point3>,
    light_radius: scalar,
    flags: impl Into<Option<ShadowFlags>>,
) -> Option<Rect> {
    let flags = flags.into().unwrap_or(ShadowFlags::NONE);
    let rec = fill_shadow_rec(
        z_plane_params.into(),
        light_pos.into(),
        light_radius,
        Color::BLACK,
        Color::BLACK,
        flags,
        ctm,
    )?;
    Some(get_local_bounds(path, &rec, ctm))
}

/// Color values for one-pass tonal alpha (`SkShadowUtils::ComputeTonalColors`). Returns
/// `(ambient, spot)`.
// Port of: src/utils/SkShadowUtils.cpp#L461-L529 (chrome/m156)
#[doc(alias = "ComputeTonalColors")]
#[must_use]
pub fn compute_tonal_colors(in_ambient_color: Color, in_spot_color: Color) -> (Color, Color) {
    // For tonal color we only compute color values for the spot shadow.
    // The ambient shadow is greyscale only.

    // Ambient
    let a_alpha = color_alpha(in_ambient_color);
    let out_ambient_color = Color::from_argb(a_alpha as u8, 0, 0, 0);

    // Spot
    let c = u32::from(in_spot_color);
    let spot_r = ((c >> 16) & 0xFF) as i32;
    let spot_g = ((c >> 8) & 0xFF) as i32;
    let spot_b = (c & 0xFF) as i32;
    let max = spot_r.max(spot_g).max(spot_b);
    let min = spot_r.min(spot_g).min(spot_b);
    let luminance: scalar = 0.5 * (max + min) as scalar / 255.0;
    let orig_a: scalar = color_alpha(in_spot_color) as scalar / 255.0;

    // We compute a color alpha value based on the luminance of the color, scaled by an
    // adjusted alpha value. We want the following properties to match the UX examples
    // (assuming a = 0.25) and to ensure that we have reasonable results when the color
    // is black and/or the alpha is 0:
    //     f(0, a) = 0
    //     f(luminance, 0) = 0
    //     f(1, 0.25) = .5
    //     f(0.5, 0.25) = .4
    //     f(1, 1) = 1
    // The following functions match this as closely as possible.
    let alpha_adjust: scalar = (2.6 + (-2.66667 + 1.06667 * orig_a) * orig_a) * orig_a;
    let mut color_alpha_v: scalar =
        (3.544_762 + (-4.891_428 + 2.3466 * luminance) * luminance) * luminance;
    color_alpha_v = (alpha_adjust * color_alpha_v).clamp(0.0, 1.0);

    // Similarly, we set the greyscale alpha based on luminance and alpha so that
    //     f(0, a) = a
    //     f(luminance, 0) = 0
    //     f(1, 0.25) = 0.15
    let grey_alpha = (orig_a * (1.0 - 0.4 * luminance)).clamp(0.0, 1.0);

    // The final color we want to emulate is generated by rendering a color shadow (C_rgb) using an
    // alpha computed from the color's luminance (C_a), and then a black shadow with alpha (S_a)
    // which is an adjusted value of 'a'.  Assuming SrcOver, a background color of B_rgb, and
    // ignoring edge falloff, this becomes
    //
    //      (C_a - S_a*C_a)*C_rgb + (1 - (S_a + C_a - S_a*C_a))*B_rgb
    //
    // Assuming premultiplied alpha, this means we scale the color by (C_a - S_a*C_a) and
    // set the alpha to (S_a + C_a - S_a*C_a).
    let color_scale = color_alpha_v * (1.0 - grey_alpha);
    let tonal_alpha = color_scale + grey_alpha;
    let un_premul_scale = color_scale / tonal_alpha;
    let out_spot_color = Color::from_argb(
        (tonal_alpha * 255.999) as u8,
        (un_premul_scale * spot_r as scalar) as u8,
        (un_premul_scale * spot_g as scalar) as u8,
        (un_premul_scale * spot_b as scalar) as u8,
    );
    (out_ambient_color, out_spot_color)
}

/// Where a shadow mesh is drawn: the device translation applied to the cached mesh and the
/// vertices (`factory.makeVertices` in the uncached path).
struct ShadowMesh {
    vertices: Option<Vertices>,
    translate: Vector,
}

/// `AmbientVerticesFactory::makeVertices` and the uncached ambient path.
// Port of: src/utils/SkShadowUtils.cpp#L58-L76 (chrome/m156)
fn make_ambient_mesh(
    path: &Path,
    ctm: &Matrix,
    occluder_height: scalar,
    transparent: bool,
    offset: Vector,
) -> ShadowMesh {
    let z_params = Point3::new(0.0, 0.0, occluder_height);
    // pick a canonical place to generate shadow
    let mut no_trans = ctm.clone();
    if !ctm.has_perspective() {
        no_trans.set_translate_x(0.0);
        no_trans.set_translate_y(0.0);
    }
    ShadowMesh {
        vertices: make_ambient(path, &no_trans, z_params, transparent),
        translate: offset,
    }
}

/// Occluder classes of the spot shadow (`SpotVerticesFactory::OccluderType`).
// Port of: src/utils/SkShadowUtils.cpp#L101-L115 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OccluderType {
    PointTransparent,
    PointOpaquePartialUmbra,
    PointOpaqueNoUmbra,
    Directional,
    DirectionalTransparent,
}

/// `SpotVerticesFactory::makeVertices` and the uncached spot path.
// Port of: src/utils/SkShadowUtils.cpp#L117-L165 (chrome/m156)
#[allow(clippy::too_many_arguments)] // carries the SpotVerticesFactory fields
fn make_spot_mesh(
    path: &Path,
    ctm: &Matrix,
    occluder_height: scalar,
    dev_light_pos: Point3,
    light_radius: scalar,
    local_center: Point,
    occluder_type: OccluderType,
    offset: Vector,
) -> ShadowMesh {
    let transparent = occluder_type == OccluderType::PointTransparent
        || occluder_type == OccluderType::DirectionalTransparent;
    let directional = occluder_type == OccluderType::Directional
        || occluder_type == OccluderType::DirectionalTransparent;
    let z_params = Point3::new(0.0, 0.0, occluder_height);
    if directional {
        ShadowMesh {
            vertices: make_spot(
                path,
                ctm,
                z_params,
                dev_light_pos,
                light_radius,
                transparent,
                true,
            ),
            translate: Vector::new(0.0, 0.0),
        }
    } else if ctm.has_perspective() || occluder_type == OccluderType::PointOpaquePartialUmbra {
        ShadowMesh {
            vertices: make_spot(
                path,
                ctm,
                z_params,
                dev_light_pos,
                light_radius,
                transparent,
                false,
            ),
            translate: Vector::new(0.0, 0.0),
        }
    } else {
        // pick a canonical place to generate shadow, with light centered over path
        let mut no_trans = ctm.clone();
        no_trans.set_translate_x(0.0);
        no_trans.set_translate_y(0.0);
        let dev_center = no_trans.map_point(local_center);
        let center_light_pos = Point3::new(dev_center.x, dev_center.y, dev_light_pos.z);
        ShadowMesh {
            vertices: make_spot(
                path,
                &no_trans,
                z_params,
                center_light_pos,
                light_radius,
                transparent,
                false,
            ),
            translate: offset,
        }
    }
}

/// `SkDevice::drawShadow` for the raster device: draws the ambient and spot shadows of `rec` on
/// `canvas`, with meshes where the tessellator succeeds and blurs otherwise.
// Port of: src/utils/SkShadowUtils.cpp#L715-L905 (SkDevice::drawShadow, chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the length of SkDevice::drawShadow
pub(crate) fn private_draw_shadow_rec(canvas: &Canvas, path: &Path, rec: &DrawShadowRec) {
    if !validate_rec(rec) {
        return;
    }

    let view_matrix: Matrix = canvas.with_top_device(|d| d.state().local_to_device().clone());

    let transparent = rec.flags & ShadowFlags::TRANSPARENT_OCCLUDER.bits() != 0;
    let use_blur = rec.flags & ShadowFlags::CONCAVE_BLUR_ONLY.bits() != 0 && !path.is_convex();
    let uncached = tilted(rec.z_plane_params) || path.is_volatile();
    let directional = rec.flags & ShadowFlags::DIRECTIONAL_LIGHT.bits() != 0;

    let z_plane_params = rec.z_plane_params;
    let mut dev_light_pos = rec.light_pos;
    if !directional {
        let p = view_matrix.map_point(Point::new(dev_light_pos.x, dev_light_pos.y));
        dev_light_pos.x = p.x;
        dev_light_pos.y = p.y;
    }
    let light_radius = rec.light_radius;

    if color_alpha(rec.ambient_color) > 0 {
        let prev = set_device_transform(canvas, &M44::default());
        let mut success = false;
        let uncached_vertices = if uncached && !use_blur {
            make_ambient(path, &view_matrix, z_plane_params, transparent)
        } else {
            None
        };
        if let Some(vertices) = uncached_vertices {
            draw_mesh(
                canvas,
                &vertices,
                rec.ambient_color,
                Vector::new(0.0, 0.0),
                false,
            );
            success = true;
        }

        if !success && !use_blur {
            let offset = if view_matrix.has_perspective() {
                Vector::new(0.0, 0.0)
            } else {
                Vector::new(view_matrix.translate_x(), view_matrix.translate_y())
            };
            let mesh = make_ambient_mesh(path, &view_matrix, z_plane_params.z, transparent, offset);
            success = draw_mesh_shadow(
                canvas,
                mesh,
                rec.ambient_color,
                view_matrix.has_perspective(),
            );
        }

        // All else has failed, draw with blur
        if !success {
            // Pretransform the path to avoid transforming the stroke, below.
            let mut dev_space_path = path.make_transform(&canvas.local_to_device_as_3x3());
            dev_space_path.set_is_volatile(true);

            // The tesselator outsets by AmbientBlurRadius (or 'r') to get the outer ring of
            // the tesselation, and sets the alpha on the path to 1/AmbientRecipAlpha (or 'a').
            //
            // We want to emulate this with a blur. The full blur width (2*blurRadius or 'f')
            // can be calculated by interpolating, which gives blurRadius = f/2 with f = r/a.
            //
            // We outset the path to place the new edge in the center of the blur region
            // (o = r - f/2) by stroking with strokeWidth o/2.
            let dev_space_outset = ambient_blur_radius(z_plane_params.z);
            let one_over_a = ambient_recip_alpha(z_plane_params.z);
            let blur_radius = 0.5 * dev_space_outset * one_over_a;
            let stroke_width = 0.5 * (dev_space_outset - blur_radius);

            // Now draw with blur
            canvas.save();
            canvas.set_matrix(&M44::default());
            let mut paint = Paint::default();
            paint.set_color(rec.ambient_color);
            paint.set_stroke_width(stroke_width);
            paint.set_style(Style::StrokeAndFill);
            let sigma = crate::blur_mask::BlurMask::convert_radius_to_sigma(blur_radius);
            paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, sigma, false));
            canvas.draw_path(&dev_space_path, &paint);
            canvas.restore();
        }
        restore_device_transform(canvas, prev);
    }

    if color_alpha(rec.spot_color) > 0 {
        let prev = set_device_transform(canvas, &M44::default());
        let mut success = false;
        let uncached_vertices = if uncached && !use_blur {
            make_spot(
                path,
                &view_matrix,
                z_plane_params,
                dev_light_pos,
                light_radius,
                transparent,
                directional,
            )
        } else {
            None
        };
        if let Some(vertices) = uncached_vertices {
            draw_mesh(
                canvas,
                &vertices,
                rec.spot_color,
                Vector::new(0.0, 0.0),
                false,
            );
            success = true;
        }

        if !success && !use_blur {
            let center = Point::new(path.bounds().center_x(), path.bounds().center_y());
            let local_center = center;
            let center_dev = view_matrix.map_point(center);
            let (radius, scale, mut occluder_height_factor_offset) = if directional {
                get_directional_params(
                    z_plane_params.z,
                    dev_light_pos.x,
                    dev_light_pos.y,
                    dev_light_pos.z,
                    light_radius,
                )
            } else {
                get_spot_params(
                    z_plane_params.z,
                    dev_light_pos.x - center_dev.x,
                    dev_light_pos.y - center_dev.y,
                    dev_light_pos.z,
                    light_radius,
                )
            };

            let dev_bounds = view_matrix.map_rect(path.bounds()).0;
            let occluder_type = if transparent
                || occluder_height_factor_offset.x.abs() > 0.5 * dev_bounds.width()
                || occluder_height_factor_offset.y.abs() > 0.5 * dev_bounds.height()
            {
                // if the translation of the shadow is big enough we're going to end up
                // filling the entire umbra, we can treat these as all the same
                if directional {
                    OccluderType::DirectionalTransparent
                } else {
                    OccluderType::PointTransparent
                }
            } else if directional {
                OccluderType::Directional
            } else if occluder_height_factor_offset.length() * scale + scale < radius {
                // if we don't translate more than the blur distance, can assume umbra is covered
                OccluderType::PointOpaqueNoUmbra
            } else if path.is_convex() {
                OccluderType::PointOpaquePartialUmbra
            } else {
                OccluderType::PointTransparent
            };
            // need to add this after we classify the shadow
            occluder_height_factor_offset.x += view_matrix.translate_x();
            occluder_height_factor_offset.y += view_matrix.translate_y();

            let mesh = make_spot_mesh(
                path,
                &view_matrix,
                z_plane_params.z,
                dev_light_pos,
                light_radius,
                local_center,
                occluder_type,
                occluder_height_factor_offset,
            );
            success = draw_mesh_shadow(canvas, mesh, rec.spot_color, view_matrix.has_perspective());
        }

        // All else has failed, draw with blur
        if !success {
            let Some((shadow_matrix, radius)) = get_spot_shadow_transform(
                dev_light_pos,
                light_radius,
                &canvas.local_to_device_as_3x3(),
                z_plane_params,
                path.bounds(),
                directional,
            ) else {
                restore_device_transform(canvas, prev);
                return;
            };
            canvas.save();

            // And draw with blur
            canvas.set_matrix(&M44::from(shadow_matrix));
            let mut paint = Paint::default();
            paint.set_color(rec.spot_color);
            let sigma = crate::blur_mask::BlurMask::convert_radius_to_sigma(radius);
            paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, sigma, false));
            canvas.draw_path(path, &paint);
            canvas.restore();
        }
        restore_device_transform(canvas, prev);
    }
}

/// Draws `mesh` with its translation applied to the device transform, and reports success.
/// `None` vertices count as failure, so the caller falls back to blur.
// Port of: src/utils/SkShadowUtils.cpp#L747-L778 (drawVertsProc, chrome/m156)
fn draw_mesh_shadow(
    canvas: &Canvas,
    mesh: ShadowMesh,
    color: Color,
    has_perspective: bool,
) -> bool {
    let Some(vertices) = mesh.vertices else {
        return false;
    };
    draw_mesh(canvas, &vertices, color, mesh.translate, has_perspective);
    true
}

/// `drawVertsProc` and the gaussian-filtered vertex draw: runs the vertex color through the
/// Gaussian color filter and modulates it against `color`, then blends with `kDst` so the mesh
/// alpha is what remains.
// Port of: src/utils/SkShadowUtils.cpp#L392-L400 and #L747-L778 (chrome/m156)
fn draw_mesh(
    canvas: &Canvas,
    vertices: &Vertices,
    color: Color,
    translate: Vector,
    has_perspective: bool,
) {
    if vertices.vertex_count() == 0 {
        return;
    }
    let mut paint = Paint::default();
    // Run the vertex color through a GaussianColorFilter and then modulate the grayscale result
    // of that against our 'color' param.
    if let Some(blend) = color_filters::blend(color_to_color4f(color), None, BlendMode::Modulate) {
        paint.set_color_filter(blend.composed(Some(make_gaussian())));
    }
    // For perspective shadows we've already computed the shadow in world space, and we can't
    // translate it without changing it. Otherwise we concat the change in translation.
    let transform = if has_perspective {
        M44::default()
    } else {
        let t = M44::translate(translate.x, translate.y, 0.0);
        let local = canvas.with_top_device(|d| *d.state().local_to_device44());
        M44::concat(&local, &t)
    };
    let prev = set_device_transform(canvas, &transform);
    // The vertex colors for a tesselated shadow polygon are always either opaque black or
    // transparent and their real contribution to the final blended color is via their alpha. We
    // can skip expensive per-vertex color conversion for this.
    canvas.with_top_device(|d| {
        d.draw_vertices(vertices, Blender::mode(BlendMode::Dst), &paint, true);
    });
    restore_device_transform(canvas, prev);
}

/// `Color` to `Color4f` for the color filter constructor.
fn color_to_color4f(c: Color) -> crate::color::Color4f {
    crate::color::Color4f::from(c)
}

/// Sets the device's local-to-device transform and returns the previous one
/// (`SkAutoDeviceTransformRestore`).
fn set_device_transform(canvas: &Canvas, m: &M44) -> M44 {
    canvas.with_top_device(|d| {
        let prev = *d.state().local_to_device44();
        d.state_mut().set_local_to_device(m);
        prev
    })
}

/// Restores a device transform saved by [`set_device_transform`].
fn restore_device_transform(canvas: &Canvas, prev: M44) {
    canvas.with_top_device(|d| d.state_mut().set_local_to_device(&prev));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tonal_colors_of_opaque_black_spot_stay_black() {
        // Opaque black has luminance 0 and alpha 1, so the grey alpha is 1 and the colour scale 0:
        // the tonal spot is opaque black.
        let (ambient, spot) = compute_tonal_colors(Color::BLACK, Color::BLACK);
        assert_eq!(color_alpha(spot), 255);
        assert_eq!(u32::from(spot) & 0x00FF_FFFF, 0);
        assert_eq!(color_alpha(ambient), 255);
    }
}
