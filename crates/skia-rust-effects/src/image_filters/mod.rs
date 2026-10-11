//! Image filters (`SkImageFilters`), ported from Skia's `src/effects/imagefilters`.
//!
//! Each filter is an [`ImageFilterBase`](skia_rust_core::image_filter::ImageFilterBase) built by
//! a factory here (`Offset`, `Merge`, ...). Crop rectangles are `Option<Rect>`: `None` is Skia's
//! `CropRect` with no crop.
//!
//! Ported so far: `MatrixTransform`/`Offset`, `Merge`, `Compose`, `Crop`/`Empty`/`Tile`,
//! `ColorFilter` (without the composition of two color filters), `Image`, `Blend` (blend modes,
//! custom blenders and the arithmetic blender), `Blur`, `DropShadow`, `Shader`, `Picture`,
//! `Lighting` (distant, point and spot lights, diffuse and specular), `Magnifier`, and the
//! known-runtime-effect filters `Dilate`/`Erode` (morphology), `MatrixConvolution` and
//! `DisplacementMap`.

pub mod blend_filter;
pub mod blur_filter;
pub mod color_filter_filter;
pub mod compose_filter;
pub mod crop_filter;
pub mod displacement_map_filter;
pub mod drop_shadow_filter;
pub mod image_source_filter;
pub mod lighting_filter;
pub mod magnifier_filter;
pub mod matrix_convolution_filter;
pub mod matrix_transform_filter;
pub mod merge_filter;
pub mod morphology_filter;
pub mod picture_filter;
pub mod runtime_image_filter;
pub mod shader_filter;

pub use blend_filter::{arithmetic, blend, blend_with_blender};
pub use blur_filter::blur;
pub use color_filter_filter::color_filter;
pub use compose_filter::compose;
pub use crop_filter::{crop, empty, tile};
pub use displacement_map_filter::displacement_map;
pub use drop_shadow_filter::{drop_shadow, drop_shadow_only};
pub use image_source_filter::{image, image_sampled};
pub use lighting_filter::{
    distant_lit_diffuse, distant_lit_specular, point_lit_diffuse, point_lit_specular,
    spot_lit_diffuse, spot_lit_specular,
};
pub use magnifier_filter::magnifier;
pub use matrix_convolution_filter::matrix_convolution;
pub use matrix_transform_filter::{matrix_transform, offset};
pub use merge_filter::merge;
pub use morphology_filter::{dilate, erode};
pub use picture_filter::picture;
pub use runtime_image_filter::{runtime_shader, runtime_shader_children};
pub use shader_filter::{Dither, shader};
