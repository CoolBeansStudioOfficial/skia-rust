//! Ports of `skia/tests/*.cpp`, one module per Skia file (see the crate docs for naming).

#[cfg(test)]
pub mod aa_clip_test;
#[cfg(test)]
pub mod android_codec_test;
#[cfg(test)]
pub mod as_a_dash_test;
#[cfg(test)]
pub mod bad_ico_test;
#[cfg(test)]
pub mod bitmap_copy_test;
#[cfg(test)]
pub mod bitmap_get_color_test;
#[cfg(test)]
pub mod bitmap_test;
#[cfg(test)]
pub mod blend_test;
#[cfg(test)]
pub mod blit_mask_clip;
#[cfg(test)]
pub mod blur_test;
#[cfg(test)]
pub mod cached_data_test;
#[cfg(test)]
pub mod canvas_test;
#[cfg(test)]
pub mod capped_hairlines_test;
#[cfg(test)]
pub mod char_to_glyph_cache;
#[cfg(test)]
pub mod checksum_test;
#[cfg(test)]
pub mod clip_cubic_test;
#[cfg(test)]
pub mod clip_stack_test;
#[cfg(test)]
pub mod clipper_test;
#[cfg(test)]
pub mod codec_anim_test;
#[cfg(test)]
pub mod codec_exact_read_test;
#[cfg(test)]
pub mod codec_partial_test;
#[cfg(test)]
pub mod codec_test;
#[cfg(test)]
pub mod color_filter_test;
#[cfg(test)]
pub mod color_matrix_test;
#[cfg(test)]
pub mod color_priv_test;
#[cfg(test)]
pub mod color_space_test;
#[cfg(test)]
pub mod color_test;
#[cfg(test)]
pub mod convert_pixels_test;
#[cfg(test)]
pub mod core_blitters_test;
#[cfg(test)]
pub mod cubic_map_test;
#[cfg(test)]
pub mod cubic_roots_test;
#[cfg(test)]
pub mod cull_test_test;
#[cfg(test)]
pub mod dash_path_effect_test;
#[cfg(test)]
pub mod data_ref_test;
#[cfg(test)]
pub mod descriptor_test;
#[cfg(test)]
pub mod direct_mask_limit_test;
pub mod discardable_memory_test;
#[cfg(test)]
pub mod discardable_memory_test;
#[cfg(test)]
pub mod draw_bitmap_rect_test;
#[cfg(test)]
pub mod draw_path_test;
#[cfg(test)]
pub mod draw_text_test;
#[cfg(test)]
pub mod edge_test;
#[cfg(test)]
pub mod encode_test;
pub mod exif_test;
#[cfg(test)]
pub mod extended_sk_color_type_tests;
#[cfg(test)]
pub mod f16_draw_test;
#[cfg(test)]
pub mod f16_stages_test;
#[cfg(test)]
pub mod fill_path_test;
#[cfg(test)]
pub mod find_cubic_convex180_chops_test;
#[cfg(test)]
pub mod float16_test;
#[cfg(test)]
pub mod floating_point_test;
#[cfg(test)]
pub mod font_host_stream_test;
#[cfg(test)]
pub mod font_host_test;
#[cfg(test)]
pub mod font_mgr_test;
#[cfg(test)]
pub mod font_names_test;
#[cfg(test)]
pub mod font_scanner_fontations_test;
#[cfg(test)]
pub mod font_test;
#[cfg(test)]
pub mod fontations_test;
#[cfg(test)]
pub mod front_buffered_stream_test;
#[cfg(test)]
pub mod geometry_test;
#[cfg(test)]
pub mod gif_test;
#[cfg(test)]
pub mod gradient_test;
#[cfg(test)]
pub mod graphite;
#[cfg(test)]
pub mod high_contrast_filter_test;
#[cfg(test)]
pub mod hsv_round_trip_test;
#[cfg(test)]
pub mod icc_test;
#[cfg(test)]
pub mod image_bitmap_test;
#[cfg(test)]
pub mod image_filter_test;
#[cfg(test)]
pub mod image_from565_bitmap;
#[cfg(test)]
pub mod image_is_opaque_test;
#[cfg(test)]
pub mod image_new_shader_test;
#[cfg(test)]
pub mod image_test;
#[cfg(test)]
pub mod inf_rect_test;
#[cfg(test)]
pub mod invalid_indexed_png_test;
#[cfg(test)]
pub mod m44_test;
#[cfg(test)]
pub mod malloc_pixel_ref_test;
#[cfg(test)]
pub mod math_test;
#[cfg(test)]
pub mod matrix_test;
#[cfg(test)]
pub mod md5_test;
#[cfg(test)]
pub mod memset_test;
#[cfg(test)]
pub mod mesh_test;
#[cfg(test)]
pub mod meta_data_test;
#[cfg(test)]
pub mod mip_map_test;
#[cfg(test)]
pub mod nonlinear_blending_test;
#[cfg(test)]
pub mod paint_test;
#[cfg(test)]
pub mod parametric_stage_test;
#[cfg(test)]
pub mod parse_path_test;
#[cfg(test)]
pub mod path_builder_test;
#[cfg(test)]
pub mod path_coverage_test;
#[cfg(test)]
pub mod path_data_test;
#[cfg(test)]
pub mod path_measure_test;
#[cfg(test)]
pub mod path_ops_as_winding_test;
#[cfg(test)]
pub mod path_ops_battles;
#[cfg(test)]
pub mod path_ops_bounds_test;
#[cfg(test)]
pub mod path_ops_build_use_test;
#[cfg(test)]
pub mod path_ops_builder_conic_test;
#[cfg(test)]
pub mod path_ops_builder_test;
#[cfg(test)]
pub mod path_ops_chalkboard_test;
#[cfg(test)]
pub mod path_ops_conic_intersection_test;
#[cfg(test)]
pub mod path_ops_conic_line_intersection_test;
#[cfg(test)]
pub mod path_ops_conic_quad_intersection_test;
#[cfg(test)]
pub mod path_ops_cubic_conic_intersection_test;
#[cfg(test)]
pub mod path_ops_cubic_intersection_test;
#[cfg(test)]
pub mod path_ops_cubic_intersection_test_data;
#[cfg(test)]
pub mod path_ops_cubic_line_intersection_ideas;
#[cfg(test)]
pub mod path_ops_cubic_line_intersection_test;
#[cfg(test)]
pub mod path_ops_cubic_quad_intersection_test;
#[cfg(test)]
pub mod path_ops_cubic_reduce_order_test;
#[cfg(test)]
pub mod path_ops_d_cubic_test;
#[cfg(test)]
pub mod path_ops_d_line_test;
#[cfg(test)]
pub mod path_ops_d_point_test;
#[cfg(test)]
pub mod path_ops_d_rect_test;
#[cfg(test)]
pub mod path_ops_d_vector_test;
#[cfg(test)]
pub mod path_ops_extended_test;
#[cfg(test)]
pub mod path_ops_fuzz763_test;
#[cfg(test)]
pub mod path_ops_inverse_test;
#[cfg(test)]
pub mod path_ops_issue3651;
#[cfg(test)]
pub mod path_ops_line_intersection_test;
#[cfg(test)]
pub mod path_ops_line_parameteters_test;
#[cfg(test)]
pub mod path_ops_op_circle_threaded_test;
#[cfg(test)]
pub mod path_ops_op_cubic_threaded_test;
#[cfg(test)]
pub mod path_ops_op_loop_threaded_test;
#[cfg(test)]
pub mod path_ops_op_rect_threaded_test;
#[cfg(test)]
pub mod path_ops_op_test;
#[cfg(test)]
pub mod path_ops_quad_intersection_test;
#[cfg(test)]
pub mod path_ops_quad_intersection_test_data;
#[cfg(test)]
pub mod path_ops_quad_line_intersection_test;
#[cfg(test)]
pub mod path_ops_quad_line_intersection_threaded_test;
#[cfg(test)]
pub mod path_ops_quad_reduce_order_test;
#[cfg(test)]
pub mod path_ops_simplify_degenerate_threaded_test;
#[cfg(test)]
pub mod path_ops_simplify_fail_test;
#[cfg(test)]
pub mod path_ops_simplify_quad_threaded_test;
#[cfg(test)]
pub mod path_ops_simplify_quadralaterals_threaded_test;
#[cfg(test)]
pub mod path_ops_simplify_rect_threaded_test;
#[cfg(test)]
pub mod path_ops_simplify_test;
#[cfg(test)]
pub mod path_ops_simplify_triangles_threaded_test;
#[cfg(test)]
pub mod path_ops_skp_test;
#[cfg(test)]
pub mod path_ops_test_common;
#[cfg(test)]
pub mod path_ops_three_way_test;
#[cfg(test)]
pub mod path_ops_tiger_test;
#[cfg(test)]
pub mod path_ops_tight_bounds_test;
#[cfg(test)]
pub mod path_ops_types_test;
#[cfg(test)]
pub mod path_raw_shapes_test;
#[cfg(test)]
pub mod path_raw_test;
#[cfg(test)]
pub mod path_test;
#[cfg(test)]
pub mod picture_bbh_test;
#[cfg(test)]
pub mod picture_test;
#[cfg(test)]
pub mod pixel_ref_test;
#[cfg(test)]
pub mod pixels_rec_test;
#[cfg(test)]
pub mod point3_test;
#[cfg(test)]
pub mod point_test;
#[cfg(test)]
pub mod poly_utils_test;
#[cfg(test)]
pub mod pre_chop_path_curves_test;
#[cfg(test)]
pub mod premul_alpha_round_trip_test;
#[cfg(test)]
pub mod quad_roots_test;
#[cfg(test)]
pub mod quick_reject_test;
#[cfg(test)]
pub mod r_rect_in_path_test;
#[cfg(test)]
pub mod r_tree_test;
#[cfg(test)]
pub mod random_test;
#[cfg(test)]
pub mod raster_pipeline_builder_test;
#[cfg(test)]
pub mod raster_pipeline_code_generator_test;
#[cfg(test)]
pub mod read_pixels_test;
#[cfg(test)]
pub mod record_draw_test;
#[cfg(test)]
pub mod record_opts_test;
#[cfg(test)]
pub mod record_pattern_test;
#[cfg(test)]
pub mod record_test;
#[cfg(test)]
pub mod record_test_utils;
#[cfg(test)]
pub mod recorder_test;
#[cfg(test)]
pub mod rect_test;
#[cfg(test)]
pub mod region_test;
#[cfg(test)]
pub mod round_rect_test;
#[cfg(test)]
pub mod runtime_blend_test;
#[cfg(test)]
pub mod safe_math_test;
#[cfg(test)]
pub mod scalar_test;
#[cfg(test)]
pub mod serialization_test;
#[cfg(test)]
pub mod shader_test;
pub mod shadow_test;
#[cfg(test)]
pub mod simplify_paint_test;
#[cfg(test)]
pub mod size_test;
#[cfg(test)]
pub mod sk_color4f_test;
#[cfg(test)]
pub mod sk_color_space_xform_steps_test;
#[cfg(test)]
pub mod sk_font_metrics_priv_test;
#[cfg(test)]
pub mod sk_gauss_filter_test;
#[cfg(test)]
pub mod sk_glyph_test;
#[cfg(test)]
pub mod sk_image_test;
#[cfg(test)]
pub mod sk_path_range_iter_test;
#[cfg(test)]
pub mod sk_raster_pipeline_opts_test;
#[cfg(test)]
pub mod sk_raster_pipeline_test;
#[cfg(test)]
pub mod sk_remote_glyph_cache_test;
#[cfg(test)]
pub mod sk_resource_cache_test;
#[cfg(test)]
pub mod sk_runtime_effect_test;
#[cfg(test)]
pub mod sk_sl_debug_trace_player_test;
#[cfg(test)]
pub mod sk_sl_debug_trace_test;
#[cfg(test)]
pub mod sk_sl_memory_layout_test;
#[cfg(test)]
pub mod sk_sl_pipeline_stage_testbed;
#[cfg(test)]
pub mod sk_sl_test;
#[cfg(test)]
pub mod sk_sl_type_test;
#[cfg(test)]
pub mod sk_sles2_conformance_test;
#[cfg(test)]
pub mod sk_slwgsl_testbed;
#[cfg(test)]
pub mod sk_strike_cache_test;
#[cfg(test)]
pub mod sk_strike_test;
#[cfg(test)]
pub mod sk_utf_test;
#[cfg(test)]
pub mod sk_vx_test;
#[cfg(test)]
pub mod src_over_test;
#[cfg(test)]
pub mod stream_test;
#[cfg(test)]
pub mod stroke_test;
#[cfg(test)]
pub mod stroker_test;
#[cfg(test)]
pub mod surface_test;
#[cfg(test)]
pub mod swizzler_test;
#[cfg(test)]
pub mod text_blob_test;
#[cfg(test)]
pub mod typeface_test;
#[cfg(test)]
pub mod vertices_test;
#[cfg(test)]
pub mod wangs_formula_test;
#[cfg(test)]
pub mod webp_test;
#[cfg(test)]
pub mod write_pixels_test;
#[cfg(test)]
pub mod yuv_cache_test;
#[cfg(test)]
pub mod yuv_test;
