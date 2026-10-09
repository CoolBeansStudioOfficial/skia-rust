//! Image filters (`SkImageFilters`), ported from Skia's `src/effects/imagefilters`.
//!
//! Each filter is an [`ImageFilterBase`](skia_rust_core::image_filter::ImageFilterBase) built by
//! a factory here (`Offset`, `Merge`, ...). Crop rectangles are `Option<Rect>`: `None` is Skia's
//! `CropRect` with no crop.
//!
//! Ported so far: `MatrixTransform`/`Offset`, `Merge`, `Compose`, `Crop`/`Empty`/`Tile`,
//! `ColorFilter` (without the composition of two color filters), `Image` and `Blend` (blend
//! modes and custom blenders; not the arithmetic blender), `Blur`, `DropShadow`, `Shader` and
//! `Picture`, and the known-runtime-effect filters `Dilate`/`Erode` (morphology),
//! `MatrixConvolution` and `DisplacementMap`. The lighting, magnifier and arithmetic filters are
//! not ported yet.

pub mod blend_filter;
pub mod blur_filter;
pub mod color_filter_filter;
pub mod compose_filter;
pub mod crop_filter;
pub mod displacement_map_filter;
pub mod drop_shadow_filter;
pub mod image_source_filter;
pub mod matrix_convolution_filter;
pub mod matrix_transform_filter;
pub mod merge_filter;
pub mod morphology_filter;
pub mod picture_filter;
pub mod shader_filter;

pub use blend_filter::{blend, blend_with_blender};
pub use blur_filter::blur;
pub use color_filter_filter::color_filter;
pub use compose_filter::compose;
pub use crop_filter::{crop, empty, tile};
pub use displacement_map_filter::displacement_map;
pub use drop_shadow_filter::{drop_shadow, drop_shadow_only};
pub use image_source_filter::{image, image_sampled};
pub use matrix_convolution_filter::matrix_convolution;
pub use matrix_transform_filter::{matrix_transform, offset};
pub use merge_filter::merge;
pub use morphology_filter::{dilate, erode};
pub use picture_filter::picture;
pub use shader_filter::{Dither, shader};
