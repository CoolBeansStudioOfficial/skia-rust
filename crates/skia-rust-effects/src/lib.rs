//! Effects of skia-rust, ported from Skia's `src/effects` (and the pieces of `src/utils` they
//! use): path effects and mask filters so far. The `SkPathEffect` base, `MakeSum` and `MakeCompose`
//! live in `skia_rust_core::path_effect`, the `SkMaskFilter` base and the blur mask filter in
//! `skia_rust_core::{mask_filter, blur_mask_filter_impl}`.

pub mod corner_path_effect;
pub mod dash_impl;
pub mod dash_path;
pub mod dash_path_effect;
pub mod discrete_path_effect;
pub mod emboss_mask;
pub mod emboss_mask_filter;
pub mod line_2d_path_effect;
pub mod path_1d_path_effect;
pub mod path_2d_path_effect;
pub mod table_mask_filter;
pub mod trim_path_effect;
pub(crate) mod two_d_path_effect;

#[cfg(test)]
mod tests;
