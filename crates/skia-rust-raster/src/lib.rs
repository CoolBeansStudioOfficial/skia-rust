//! The CPU raster backend of skia-rust: blitters, scan converters, edges and clips.

pub mod aa_clip;
pub mod alpha_runs;
pub mod analytic_edge;
pub mod auto_blitter_choose;
pub mod bitmap_device;
pub mod blit_row;
pub mod blitter;
pub mod blitter_a8;
pub mod blitter_choose;
pub mod blitter_dump;
pub mod blur_engine;
pub mod core_blitters;
pub mod draw;
pub mod draw_atlas;
pub mod draw_vertices;
pub mod edge;
pub mod edge_builder;
pub mod glyph_image;
pub mod glyph_run_painter;
pub mod image_filter_backend;
pub mod image_picture;
pub mod images;
pub mod mask_filter_base;
mod pixel_rows;
pub mod pixmap_draw;
pub mod raster_canvas;
pub mod raster_clip;
pub mod raster_clip_stack;
pub mod raster_pipeline_blitter;
pub mod region_path;
pub mod scan;
pub mod scan_aaa_path;
pub mod scan_anti_path;
pub mod scan_antihair;
pub mod scan_hairline;
pub mod scan_priv;
pub mod sprite_blitter;
pub mod surface;
pub mod surfaces;

#[cfg(test)]
mod aa_clip_region_tests;
#[cfg(test)]
mod aa_clip_tests;
#[cfg(test)]
mod blitter_tests;
#[cfg(test)]
mod blitters_tests;
#[cfg(test)]
mod canvas_tests;
#[cfg(test)]
mod draw_tests;
#[cfg(test)]
mod draw_vertices_tests;
#[cfg(test)]
mod legacy_blitters_tests;
#[cfg(test)]
mod raster_pipeline_blitter_tests;
#[cfg(test)]
mod scan_aaa_tests;
#[cfg(test)]
mod scan_hairline_tests;
#[cfg(test)]
mod scan_tests;
