//! Core types of skia-rust, ported from Skia's `include/core` and `src/core`.

pub mod align;
pub mod alpha_type;
pub mod arc;
pub mod arena_alloc;
pub mod bezier_curves;
pub mod bitmap;
pub mod blend_mode;
pub mod blend_mode_blender;
#[doc(hidden)]
pub mod blend_mode_priv;
pub mod blender;
pub mod buffer;
pub mod canvas;
pub mod checksum;
pub mod clip_op;
pub mod clip_stack;
pub mod color;
pub mod color_data;
pub mod color_filter;
pub mod color_filters;
#[doc(hidden)]
pub mod color_priv;
pub mod color_space;
#[doc(hidden)]
pub mod color_space_priv;
pub mod color_space_xform_steps;
pub mod color_type;
pub mod contour_measure;
#[doc(hidden)]
pub mod convert_pixels;
pub mod cubic_clipper;
pub mod cubic_map;
pub mod cubics;
pub mod data;
pub mod data_table;
pub mod device;
pub mod draw_procs;
pub mod draw_types;
#[doc(hidden)]
pub mod edge_clipper;
pub mod effect_priv;
pub mod endian;
pub mod fdot6;
pub mod fixed;
pub mod float_bits;
pub mod floating_point;
pub mod front_buffered_stream;
pub mod geometry;
pub mod half;
pub mod id_change_listener;
pub mod image_filter;
pub mod image_filter_types;
pub mod image_info;
#[doc(hidden)]
pub mod image_info_priv;
#[doc(hidden)]
pub mod line_clipper;
pub mod m44;
pub mod malloc_pixel_ref;
pub mod mask;
pub mod mask_filter;
pub mod math;
#[doc(hidden)]
pub mod math_priv;
pub mod matrix;
pub mod matrix_invert;
#[doc(hidden)]
pub mod matrix_priv;
#[doc(hidden)]
pub mod matrix_utils;
pub mod paint;
#[doc(hidden)]
pub mod paint_priv;
pub mod path;
pub mod path_builder;
pub mod path_data;
mod path_dump;
pub mod path_effect;
#[doc(hidden)]
pub mod path_enums;
mod path_interpolate;
pub mod path_iter;
#[doc(hidden)]
pub mod path_makers;
pub mod path_measure;
#[doc(hidden)]
pub mod path_priv;
pub mod path_raw;
#[doc(hidden)]
pub mod path_raw_shapes;
#[doc(hidden)]
pub mod path_ref;
mod path_serial;
pub mod path_types;
pub mod path_utils;
pub mod pixel_ref;
#[doc(hidden)]
pub mod pixel_ref_priv;
pub mod pixmap;
pub mod point;
pub mod point3;
pub mod quads;
pub mod random;
pub mod raster_pipeline;
pub mod raster_pipeline_context_utils;
#[doc(hidden)]
pub mod read_pixels_rec;
pub mod rect;
pub mod region;
pub mod region_path;
pub mod rrect;
pub mod safe32;
pub mod safe_math;
pub mod scalar;
pub mod shader;
pub mod shaders;
pub mod size;
pub mod special_image;
pub mod stream;
#[doc(hidden)]
pub mod stream_priv;
pub mod string;
#[doc(hidden)]
pub mod string_utils;
pub mod stroke;
pub mod stroke_rec;
#[doc(hidden)]
pub mod stroker_priv;
pub mod surface_props;
pub mod swizzle;
pub mod t_fits_in;
pub mod t_pin;
pub mod t_sort;
pub mod tessellation;
pub mod tile_mode;
pub mod to;
pub mod un_pre_multiply;
pub mod utf;
pub mod utils;
#[doc(hidden)]
pub mod write_pixels_rec;
