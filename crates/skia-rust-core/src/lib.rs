//! Core types of skia-rust, ported from Skia's `include/core` and `src/core`.

pub mod align;
pub mod alpha_type;
pub mod annotation;
pub mod arc;
pub mod arena_alloc;
pub mod base64;
pub mod bbh_factory;
pub mod bezier_curves;
pub mod bitmap;
pub mod bitmap_cache;
#[doc(hidden)]
pub mod bitmap_proc_state_priv;
pub mod blend_mode;
pub mod blend_mode_blender;
#[doc(hidden)]
pub mod blend_mode_priv;
pub mod blender;
pub mod blur_engine;
pub mod blur_mask;
pub mod blur_mask_filter_impl;
pub mod blur_types;
pub mod buffer;
pub mod cached_data;
pub mod canvas;
#[doc(hidden)]
pub mod canvas_priv;
pub mod capabilities;
pub mod checksum;
pub mod clip_op;
pub mod clip_stack;
pub mod color;
pub mod color_data;
pub mod color_filter;
pub mod color_filters;
pub mod color_matrix;
#[doc(hidden)]
pub mod color_priv;
pub mod color_space;
#[doc(hidden)]
pub mod color_space_priv;
pub mod color_space_xform_color_filter;
pub mod color_space_xform_steps;
pub mod color_table;
pub mod color_type;
pub mod compose_color_filter;
pub mod compressed_data_utils;
pub mod contour_measure;
#[doc(hidden)]
pub mod convert_pixels;
pub mod cubic_clipper;
pub mod cubic_map;
pub mod cubics;
pub mod data;
pub mod data_table;
pub mod descriptor;
pub mod device;
pub mod discardable_memory;
pub mod discardable_memory_pool;
pub mod distance_field_gen;
pub mod draw_procs;
pub mod draw_shadow_info;
pub mod draw_types;
pub mod drawable;
#[doc(hidden)]
pub mod edge_clipper;
pub mod effect_priv;
pub mod encoded_image_format;
pub mod encoded_origin;
pub mod endian;
pub mod executor;
pub mod fdot6;
pub mod fixed;
pub mod flattenable;
pub mod float_bits;
pub mod floating_point;
pub mod font;
pub mod font_arguments;
pub mod font_descriptor;
pub mod font_metrics;
#[doc(hidden)]
pub mod font_metrics_priv;
pub mod font_mgr;
pub mod font_parameters;
#[doc(hidden)]
pub mod font_priv;
pub mod font_stream;
pub mod font_style;
pub mod font_types;
pub mod front_buffered_stream;
pub mod gauss_filter;
pub mod gaussian_color_filter;
pub mod geometry;
pub mod glyph;
mod glyph_intercepts;
pub mod glyph_run;
pub mod graphics;
pub mod half;
pub mod id_change_listener;
pub mod image;
pub mod image_base;
pub mod image_filter;
pub mod image_filter_result;
pub mod image_filter_types;
pub mod image_generator;
pub mod image_info;
#[doc(hidden)]
pub mod image_info_priv;
pub mod image_lazy;
pub mod image_raster;
pub mod images;
pub mod known_runtime_effects;
#[doc(hidden)]
pub mod lattice_iter;
#[doc(hidden)]
pub mod line_clipper;
pub mod local_matrix_image_filter;
#[doc(hidden)]
pub mod lru_cache;
pub mod m44;
pub mod malloc_pixel_ref;
pub mod mask;
pub mod mask_blur_filter;
pub mod mask_filter;
pub mod mask_gamma;
pub mod math;
#[doc(hidden)]
pub mod math_priv;
pub mod matrix;
pub mod matrix_color_filter;
pub mod matrix_invert;
#[doc(hidden)]
pub mod matrix_priv;
#[doc(hidden)]
pub mod matrix_utils;
pub mod md5;
pub mod mesh;
pub mod mipmap;
pub mod mipmap_accessor;
pub mod mipmap_builder;
pub mod os_path;
pub mod packed_glyph_id;
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
pub mod picture;
#[doc(hidden)]
pub(crate) mod picture_data;
pub(crate) mod picture_flat;
pub(crate) mod picture_playback;
pub mod picture_priv;
pub(crate) mod picture_record;
pub mod picture_recorder;
pub mod pixel_ref;
#[doc(hidden)]
pub mod pixel_ref_priv;
pub mod pixmap;
pub mod point;
pub mod point3;
pub mod quads;
pub mod r_tree;
pub mod random;
pub mod raster_pipeline;
pub mod raster_pipeline_context_utils;
pub mod read_buffer;
#[doc(hidden)]
pub mod read_pixels_rec;
pub mod record;
pub mod record_canvas;
pub mod record_draw;
pub mod record_opts;
pub mod record_pattern;
pub mod records;
pub mod rect;
pub mod region;
pub mod region_path;
pub mod resource_cache;
pub mod rrect;
pub mod rsxform;
pub mod runtime_blender;
pub mod runtime_color_filter;
pub mod runtime_effect;
#[doc(hidden)]
pub mod runtime_effect_priv;
pub mod safe32;
pub mod safe_math;
pub mod safe_range;
pub mod sampling_options;
#[doc(hidden)]
pub mod sampling_priv;
pub mod scalar;
pub mod scaler_context;
pub mod serial_procs;
pub mod sfnt;
pub mod shader;
pub mod shader_blur_algorithm;
pub mod shaders;
pub mod shadow_tessellator;
pub mod shadow_utils;
pub mod size;
pub mod slug;
pub mod special_image;
pub mod stream;
#[doc(hidden)]
pub mod stream_priv;
#[doc(hidden)]
pub mod strike;
#[doc(hidden)]
pub mod strike_cache;
#[doc(hidden)]
pub mod strike_spec;
pub mod string;
#[doc(hidden)]
pub mod string_utils;
pub mod stroke;
pub mod stroke_rec;
#[doc(hidden)]
pub mod stroker_priv;
pub mod surface_props;
pub mod swizzle;
pub mod t_dp_queue;
pub mod t_fits_in;
pub mod t_hash;
pub mod t_pin;
pub mod t_sort;
pub mod table_color_filter;
pub mod tessellation;
pub mod text_blob;
pub mod texture_compression_type;
pub mod tile_mode;
pub mod tiled_image_utils;
pub mod to;
pub mod typeface;
pub mod typeface_cache;
pub mod un_pre_multiply;
pub mod utf;
pub mod utils;
pub mod vert_state;
pub mod vertices;
pub mod working_format_color_filter;
pub mod write_buffer;
#[doc(hidden)]
pub mod write_pixels_rec;
pub mod xml;
pub mod yuv_math;
pub mod yuv_planes_cache;
pub mod yuva_info;
pub mod yuva_pixmaps;
