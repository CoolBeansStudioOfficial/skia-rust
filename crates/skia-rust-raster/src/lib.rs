//! The CPU raster backend of skia-rust: blitters, scan converters, edges and clips.

pub mod alpha_runs;
pub mod blitter;
pub mod edge;
pub mod edge_builder;
pub mod region_path;
pub mod scan;
pub mod scan_priv;

#[cfg(test)]
mod blitter_tests;
#[cfg(test)]
mod scan_tests;
