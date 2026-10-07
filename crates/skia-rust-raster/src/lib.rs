//! The CPU raster backend of skia-rust: blitters, scan converters, edges and clips.

pub mod alpha_runs;
pub mod blitter;

#[cfg(test)]
mod blitter_tests;
