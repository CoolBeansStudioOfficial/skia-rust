//! The CPU raster backend of skia-rust: blitters, scan converters, edges and clips.

pub mod alpha_runs;
pub mod analytic_edge;
pub mod blitter;
pub mod blitter_dump;
pub mod edge;
pub mod edge_builder;
pub mod raster_pipeline_blitter;
pub mod region_path;
pub mod scan;
pub mod scan_aaa_path;
pub mod scan_anti_path;
pub mod scan_antihair;
pub mod scan_clip;
pub mod scan_hairline;
pub mod scan_priv;

#[cfg(test)]
mod blitter_tests;
#[cfg(test)]
mod raster_pipeline_blitter_tests;
#[cfg(test)]
mod scan_aaa_tests;
#[cfg(test)]
mod scan_hairline_tests;
#[cfg(test)]
mod scan_tests;
