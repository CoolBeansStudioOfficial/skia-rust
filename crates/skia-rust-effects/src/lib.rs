//! Effects of skia-rust, ported from Skia's `src/effects` and `src/shaders` (and the pieces of
//! `src/utils` they use): path effects, mask filters and the gradient shaders so far. The
//! `SkPathEffect` base, `MakeSum` and `MakeCompose` live in `skia_rust_core::path_effect`, the
//! `SkMaskFilter` base and the blur mask filter in
//! `skia_rust_core::{mask_filter, blur_mask_filter_impl}`, the `SkShader` base in
//! `skia_rust_core::shader`.

pub mod blenders;
pub mod conical_gradient;
pub mod corner_path_effect;
pub mod dash_impl;
pub mod dash_path;
pub mod dash_path_effect;
pub mod discrete_path_effect;
pub mod emboss_mask;
pub mod emboss_mask_filter;
pub mod flattenable;
pub mod gainmap_shader;
pub mod gradient;
pub mod gradient_base_shader;
pub mod gradient_shader;
pub mod high_contrast_filter;
pub mod image_filters;
pub mod line_2d_path_effect;
pub mod linear_gradient;
pub mod luma_color_filter;
pub mod overdraw_color_filter;
pub mod path_1d_path_effect;
pub mod path_2d_path_effect;
pub mod perlin_noise_shader;
pub mod radial_gradient;
pub mod shader_mask_filter;
pub mod sweep_gradient;
pub mod table_mask_filter;
pub mod trim_path_effect;
pub(crate) mod two_d_path_effect;

#[cfg(test)]
mod tests;
