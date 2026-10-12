//! Sparse strips: a CPU path rasterizer whose output Graphite draws on the GPU (Skia's
//! `src/gpu/graphite/sparse_strips`; `skia_enable_sparse_strips` is off in Skia's default builds).
//!
//! A path is flattened into a [`Polyline`](polyline::Polyline) ([`flatten`]), the lines are
//! tiled ([`tiler`]), and the sorted tiles are rasterized into the alpha values of the antialiased
//! boundary runs (`EndCaps`) and the solid inner spans (`WideTiles`) ([`make_strips`],
//! [`strip_generator`]). The alphas live in the [`alpha_atlas_manager`]'s pages.

pub mod alpha_atlas_manager;
pub mod config;
pub mod flatten;
pub mod make_strips;
pub mod msaa_lut;
pub mod polyline;
pub mod strip_generator;
pub mod strip_processor_scalar;
pub mod strip_processor_simd;
pub mod tiler;
pub mod types;
