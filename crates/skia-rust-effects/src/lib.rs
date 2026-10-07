//! Effects of skia-rust, ported from Skia's `src/effects` (and the pieces of `src/utils` they
//! use): path effects so far. The `SkPathEffect` base lives in `skia_rust_core::path_effect`.

pub mod corner_path_effect;
pub mod dash_impl;
pub mod dash_path;
pub mod dash_path_effect;

#[cfg(test)]
mod tests;
