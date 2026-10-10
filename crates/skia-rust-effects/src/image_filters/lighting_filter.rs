// Copyright 2012 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkLightingImageFilter.cpp

//! `SkLightingImageFilter`: diffuse and specular lighting of the alpha channel of its input, as a
//! normal map (the `Normal` known runtime effect) lit by the `Lighting` known runtime effect.
//!
//! Skia keeps the x/y and z components of positions and directions as separate parameter-space
//! types (`ZValue`); here they stay plain scalars, mapped with the same `Mapping` helpers.

#![allow(clippy::similar_names)] // Keeps the C++ light/material field names (kd, ks, ...).

use skia_rust_core::color::Color;
use skia_rust_core::floating_point::float_midpoint;
use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter_result::{Builder, FilterResult, ShaderFlags, default_sampling};
use skia_rust_core::image_filter_types::{Context, Mapping};
use skia_rust_core::known_runtime_effects::{StableKey, get_known_runtime_effect};
use skia_rust_core::m44::V3;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::point3::Point3;
use skia_rust_core::rect::{Contains, IRect, Rect, rect_priv};
use skia_rust_core::runtime_effect::RuntimeEffectBuilder;
use skia_rust_core::scalar::{degrees_to_radians, scalar_cos};
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;

use crate::image_filters::crop_filter::crop;

/// `Light::Type`.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L44-L50 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LightType {
    Distant,
    Point,
    Spot,
}

/// `Light`. The location and direction components a light type does not use are zero, as in C++.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L40-L109 (chrome/m156)
#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_field_names)] // mirrors the C++ Light fields (fLightType, fLightColor)
struct Light {
    light_type: LightType,
    light_color: Color,
    /// `fLocationXY` and `fLocationZ` (spot and point lights only).
    location: Point3,
    /// `fDirectionXY` and `fDirectionZ` (spot and distant lights only).
    direction: Point3,
    falloff_exponent: f32,
    cos_cutoff_angle: f32,
}

impl Light {
    // Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L63-L74 (chrome/m156)
    fn point(color: Color, location: Point3) -> Light {
        Light {
            light_type: LightType::Point,
            light_color: color,
            location,
            direction: Point3::default(),
            falloff_exponent: 0.0,
            cos_cutoff_angle: 0.0,
        }
    }

    // Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L76-L86 (chrome/m156)
    fn distant(color: Color, direction: Point3) -> Light {
        Light {
            light_type: LightType::Distant,
            light_color: color,
            location: Point3::default(),
            direction,
            falloff_exponent: 0.0,
            cos_cutoff_angle: 0.0,
        }
    }

    // Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L88-L99 (chrome/m156)
    fn spot(
        color: Color,
        location: Point3,
        direction: Point3,
        falloff_exponent: f32,
        cos_cutoff_angle: f32,
    ) -> Light {
        Light {
            light_type: LightType::Spot,
            light_color: color,
            location,
            direction,
            falloff_exponent,
            cos_cutoff_angle,
        }
    }
}

/// `Material::Type`. The discriminants are the values written to the `materialAndLightType`
/// uniform (`static_cast<float>(matType)`).
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L111-L117 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaterialType {
    Diffuse = 0,
    Specular = 1,
    // Constructed by `SkEmbossMaskFilter::LegacySpecular`, which is not ported yet.
    #[allow(dead_code)] // see above
    EmbossSpecular = 2,
}

/// `Material`.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L119-L142 (chrome/m156)
#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_field_names)] // mirrors the C++ Material fields (fType, fSurfaceDepth)
struct Material {
    material_type: MaterialType,
    /// `fSurfaceDepth`, a z value in parameter space.
    surface_depth: f32,
    /// `fK`, the reflectance coefficient.
    k: f32,
    /// `fShininess` (specular only).
    shininess: f32,
}

impl Material {
    // Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L128-L130 (chrome/m156)
    fn diffuse(k: f32, surface_depth: f32) -> Material {
        Material {
            material_type: MaterialType::Diffuse,
            surface_depth,
            k,
            shininess: 0.0,
        }
    }

    // Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L132-L134 (chrome/m156)
    fn specular(k: f32, shininess: f32, surface_depth: f32) -> Material {
        Material {
            material_type: MaterialType::Specular,
            surface_depth,
            k,
            shininess,
        }
    }
}

/// `LayerSpace<ZValue>::Map`: a z value mapped to layer space, scaled as the average of the x and
/// y scale factors of the mapping.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L36-L41 (chrome/m156)
fn map_z_to_layer(mapping: &Mapping, z: f32) -> f32 {
    // See the comment on ZValue for the rationale.
    let z2d = mapping.param_to_layer_vector(Vector::new(z, z));
    float_midpoint(z2d.x, z2d.y)
}

/// `SkLightingImageFilter::requiredInput`: one pixel of padding for the 3x3 Sobel kernel.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L199-L204 (chrome/m156)
fn required_input(desired_output: IRect) -> IRect {
    desired_output.with_outset((1, 1))
}

/// Sets child `name` to `shader`, or to null when there is none.
fn set_child(builder: &mut RuntimeEffectBuilder, name: &str, shader: Option<Shader>) {
    let mut child = builder.child(name);
    match shader {
        Some(shader) => {
            child.assign(shader);
        }
        None => {
            child.assign_null();
        }
    }
}

/// `make_normal_shader`: a Sobel filter on the alpha channel of the input, using `edge_bounds` to
/// decide how to modify the kernel weights.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L143-L156 (chrome/m156)
fn make_normal_shader(
    alpha_map: Option<Shader>,
    edge_bounds: IRect,
    surface_depth: f32,
) -> Option<Shader> {
    let normal_effect = get_known_runtime_effect(StableKey::Normal)?;
    let mut builder = RuntimeEffectBuilder::new(normal_effect.clone());
    set_child(&mut builder, "alphaMap", alpha_map);
    let edge = Rect::from_irect(edge_bounds).with_inset((0.5, 0.5));
    builder
        .uniform("edgeBounds")
        .set_f32(&[edge.left(), edge.top(), edge.right(), edge.bottom()]);
    builder
        .uniform("negSurfaceDepth")
        .set_f32(&[-surface_depth]);
    builder.make_shader(None)
}

/// The light in layer space: the parameters the lighting shader takes.
#[derive(Clone, Copy, Debug)]
struct LayerLight {
    light_type: LightType,
    light_color: Color,
    location_xy: Point,
    location_z: f32,
    direction_xy: Vector,
    direction_z: f32,
    falloff_exponent: f32,
    cos_cutoff_angle: f32,
}

/// The material in layer space.
#[derive(Clone, Copy, Debug)]
struct LayerMaterial {
    material_type: MaterialType,
    surface_depth: f32,
    k: f32,
    shininess: f32,
}

/// `make_lighting_shader`: the `Lighting` known runtime effect over the normal map.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L158-L210 (chrome/m156)
fn make_lighting_shader(
    normal_map: Option<Shader>,
    light: LayerLight,
    material: LayerMaterial,
) -> Option<Shader> {
    let lighting_effect = get_known_runtime_effect(StableKey::Lighting)?;
    let mut builder = RuntimeEffectBuilder::new(lighting_effect.clone());
    set_child(&mut builder, "normalMap", normal_map);

    let light_type_value = match light.light_type {
        LightType::Point => 0.0,
        LightType::Distant => -1.0,
        LightType::Spot => 1.0,
    };
    // The `static_cast<float>(matType)` of the C++ enum values.
    let material_type_value = match material.material_type {
        MaterialType::Diffuse => 0.0,
        MaterialType::Specular => 1.0,
        MaterialType::EmbossSpecular => 2.0,
    };
    builder.uniform("materialAndLightType").set_f32(&[
        material.surface_depth,
        material.shininess,
        material_type_value,
        light_type_value,
    ]);
    builder.uniform("lightPosAndSpotFalloff").set_f32(&[
        light.location_xy.x,
        light.location_xy.y,
        light.location_z,
        light.falloff_exponent,
    ]);

    // Pre-normalize the light direction, but this can be (0,0,0) for point lights, which won't use
    // the uniform anyways. Avoid a division by 0 to keep ASAN happy or in the event that a spot/dir
    // light have bad user input.
    let dir = V3::new(
        light.direction_xy.x,
        light.direction_xy.y,
        light.direction_z,
    );
    let dir_len = dir.length();
    // C++ tests the float for truthiness: only an exact zero is false, so NaN takes the
    // reciprocal branch.
    #[allow(clippy::float_cmp)] // mirrors the C++ truthiness test of a float
    let inv_dir_len = if dir_len == 0.0 { 0.0 } else { 1.0 / dir_len };
    builder.uniform("lightDirAndSpotCutoff").set_f32(&[
        inv_dir_len * dir.x,
        inv_dir_len * dir.y,
        inv_dir_len * dir.z,
        light.cos_cutoff_angle,
    ]);

    // Historically, the Skia lighting image filter did not apply any color space transformation to
    // the light's color. The material's K is applied up front, since no color space transforms
    // need to be performed on the original light color.
    let color_scale = material.k / 255.0;
    builder.uniform("lightColor").set_f32(&[
        f32::from(light.light_color.r()) * color_scale,
        f32::from(light.light_color.g()) * color_scale,
        f32::from(light.light_color.b()) * color_scale,
    ]);

    builder.make_shader(None)
}

/// The lighting image filter (`SkLightingImageFilter`).
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L144-L183 (chrome/m156)
#[doc(alias = "SkLightingImageFilter")]
#[derive(Debug)]
pub struct LightingImageFilter {
    common: ImageFilterCommon,
    light: Light,
    material: Material,
}

impl ImageFilterBase for LightingImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L171 (chrome/m156)
    fn on_affects_transparent_black(&self) -> bool {
        true
    }

    // Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L442-L503 (chrome/m156)
    fn on_filter_image(&self, ctx: &Context<'_>) -> FilterResult {
        let mapping = ctx.mapping();

        // Map lighting and material parameters into layer space
        let surface_depth = map_z_to_layer(mapping, self.material.surface_depth);
        let light_location_xy =
            mapping.param_to_layer_point(Point::new(self.light.location.x, self.light.location.y));
        let light_location_z = map_z_to_layer(mapping, self.light.location.z);
        let light_dir_xy = mapping
            .param_to_layer_vector(Vector::new(self.light.direction.x, self.light.direction.y));
        let light_dir_z = map_z_to_layer(mapping, self.light.direction.z);

        // The normal map is determined by a 3x3 kernel, so we request a 1px outset of what should
        // be filled by the lighting equation. See the C++ comment for the boundary conditions.
        let required = required_input(ctx.desired_output());
        let child_output = self.get_child_output(0, &ctx.with_new_desired_output(required));

        let mut clamp_rect = required; // effectively no clamping of normals
        if !child_output.layer_bounds().contains(required) {
            // Adjust clampRect edges to desiredOutput if the actual child output matched the
            // lighting output size (typical SVG case). Otherwise leave coordinates alone to use
            // decal tiling automatically for the pixels outside the child image.
            let edge_clamp = |actual_edge: i32, requested_edge: i32, output_edge: i32| {
                if actual_edge == output_edge {
                    output_edge
                } else {
                    requested_edge
                }
            };
            let input_rect = child_output.layer_bounds();
            let clamp_to = ctx.desired_output();
            clamp_rect = IRect::new(
                edge_clamp(input_rect.left(), required.left(), clamp_to.left()),
                edge_clamp(input_rect.top(), required.top(), clamp_to.top()),
                edge_clamp(input_rect.right(), required.right(), clamp_to.right()),
                edge_clamp(input_rect.bottom(), required.bottom(), clamp_to.bottom()),
            );
        }

        let light = LayerLight {
            light_type: self.light.light_type,
            light_color: self.light.light_color,
            location_xy: light_location_xy,
            location_z: light_location_z,
            direction_xy: light_dir_xy,
            direction_z: light_dir_z,
            falloff_exponent: self.light.falloff_exponent,
            cos_cutoff_angle: self.light.cos_cutoff_angle,
        };
        let material = LayerMaterial {
            material_type: self.material.material_type,
            surface_depth,
            k: self.material.k,
            shininess: self.material.shininess,
        };

        let mut builder = Builder::new(ctx);
        builder.add(
            child_output,
            Some(clamp_rect),
            ShaderFlags::SAMPLED_REPEATEDLY,
            default_sampling(),
        );
        builder.eval(
            |input| {
                let normals = make_normal_shader(input[0].clone(), clamp_rect, surface_depth);
                make_lighting_shader(normals, light, material)
            },
            None,
            false,
        )
    }

    // Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L505-L511 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        let required = required_input(desired_output);
        self.get_child_input_layer_bounds(0, mapping, required, content_bounds)
    }

    // Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L513-L521 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        _mapping: &Mapping,
        _content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        // The lighting equation is defined on the entire plane, even if the input image that
        // defines the normal map is bounded. So the output is unbounded.
        None
    }

    // Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L523-L525 (chrome/m156)
    fn compute_fast_bounds(&self, _src: &Rect) -> Rect {
        rect_priv::make_large_s32()
    }
}

/// `make_lighting`: validates the parameters, then wraps `input` in the lighting filter, clipped
/// by `crop_rect` on both sides when there is one.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L212-L244 (chrome/m156)
fn make_lighting(
    light: Light,
    material: Material,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    // According to the spec, ks and kd can be any non-negative number:
    // http://www.w3.org/TR/SVG/filters.html#feSpecularLightingElement
    if !material.k.is_finite()
        || !material.shininess.is_finite()
        || !material.surface_depth.is_finite()
        || material.k < 0.0
    {
        return None;
    }

    // Ensure light values are finite, and the cosine should be between -1 and 1
    if !Point::new(light.location.x, light.location.y).is_finite()
        || !Vector::new(light.direction.x, light.direction.y).is_finite()
        || !light.falloff_exponent.is_finite()
        || !light.cos_cutoff_angle.is_finite()
        || !light.location.z.is_finite()
        || !light.direction.z.is_finite()
        || light.cos_cutoff_angle < -1.0
        || light.cos_cutoff_angle > 1.0
    {
        return None;
    }

    // If a crop rect is provided, it clamps both the input (to better match the SVG's normal
    // boundary condition spec) and the output (because otherwise it has infinite bounds).
    let mut filter = input;
    if let Some(rect) = &crop_rect {
        filter = crop(rect, TileMode::Decal, filter);
    }
    let filter = Some(ImageFilter::from_base(LightingImageFilter {
        common: ImageFilterCommon::new(vec![filter], None),
        light,
        material,
    }));
    match &crop_rect {
        Some(rect) => crop(rect, TileMode::Decal, filter),
        None => filter,
    }
}

/// `SkImageFilters::DistantLitDiffuse`.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L246-L252 (chrome/m156)
#[doc(alias = "DistantLitDiffuse")]
#[must_use]
pub fn distant_lit_diffuse(
    direction: Point3,
    light_color: Color,
    surface_scale: f32,
    kd: f32,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    make_lighting(
        Light::distant(light_color, direction),
        Material::diffuse(kd, surface_scale),
        input,
        crop_rect,
    )
}

/// `SkImageFilters::PointLitDiffuse`.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L254-L260 (chrome/m156)
#[doc(alias = "PointLitDiffuse")]
#[must_use]
pub fn point_lit_diffuse(
    location: Point3,
    light_color: Color,
    surface_scale: f32,
    kd: f32,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    make_lighting(
        Light::point(light_color, location),
        Material::diffuse(kd, surface_scale),
        input,
        crop_rect,
    )
}

/// `SkImageFilters::SpotLitDiffuse`.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L262-L271 (chrome/m156)
#[doc(alias = "SpotLitDiffuse")]
#[must_use]
#[allow(clippy::too_many_arguments)] // mirrors SkImageFilters::SpotLitDiffuse's signature
pub fn spot_lit_diffuse(
    location: Point3,
    target: Point3,
    falloff_exponent: f32,
    cutoff_angle: f32,
    light_color: Color,
    surface_scale: f32,
    kd: f32,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    let dir = sub(target, location);
    let cos_cutoff_angle = scalar_cos(degrees_to_radians(cutoff_angle));
    make_lighting(
        Light::spot(
            light_color,
            location,
            dir,
            falloff_exponent,
            cos_cutoff_angle,
        ),
        Material::diffuse(kd, surface_scale),
        input,
        crop_rect,
    )
}

/// `SkImageFilters::DistantLitSpecular`.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L296-L302 (chrome/m156)
#[doc(alias = "DistantLitSpecular")]
#[must_use]
pub fn distant_lit_specular(
    direction: Point3,
    light_color: Color,
    surface_scale: f32,
    ks: f32,
    shininess: f32,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    make_lighting(
        Light::distant(light_color, direction),
        Material::specular(ks, shininess, surface_scale),
        input,
        crop_rect,
    )
}

/// `SkImageFilters::PointLitSpecular`.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L304-L310 (chrome/m156)
#[doc(alias = "PointLitSpecular")]
#[must_use]
pub fn point_lit_specular(
    location: Point3,
    light_color: Color,
    surface_scale: f32,
    ks: f32,
    shininess: f32,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    make_lighting(
        Light::point(light_color, location),
        Material::specular(ks, shininess, surface_scale),
        input,
        crop_rect,
    )
}

/// `SkImageFilters::SpotLitSpecular`.
// Port of: src/effects/imagefilters/SkLightingImageFilter.cpp#L312-L321 (chrome/m156)
#[doc(alias = "SpotLitSpecular")]
#[must_use]
#[allow(clippy::too_many_arguments)] // mirrors SkImageFilters::SpotLitSpecular's signature
pub fn spot_lit_specular(
    location: Point3,
    target: Point3,
    falloff_exponent: f32,
    cutoff_angle: f32,
    light_color: Color,
    surface_scale: f32,
    ks: f32,
    shininess: f32,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    let dir = sub(target, location);
    let cos_cutoff_angle = scalar_cos(degrees_to_radians(cutoff_angle));
    make_lighting(
        Light::spot(
            light_color,
            location,
            dir,
            falloff_exponent,
            cos_cutoff_angle,
        ),
        Material::specular(ks, shininess, surface_scale),
        input,
        crop_rect,
    )
}

/// `SkPoint3 operator-`: component-wise difference.
fn sub(a: Point3, b: Point3) -> Point3 {
    Point3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}
