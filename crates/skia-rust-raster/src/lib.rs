//! The CPU raster backend of skia-rust: blitters, scan converters, edges and clips.

pub mod alpha_runs;
pub mod analytic_edge;
pub mod blitter;
#[doc(hidden)]
pub mod dump_blitter;
pub mod edge;
pub mod edge_builder;
pub mod scan;
pub mod scan_aaa_path;
pub mod scan_anti_path;
pub mod scan_antihair;
pub mod scan_priv;

#[cfg(test)]
mod blitter_tests;
#[cfg(test)]
mod scan_aaa_tests;
