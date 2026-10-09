// Copyright 2017 The Wuffs Authors (the Wuffs sources this file ports).
// Modifications (the Rust port) Copyright (C) 2025 The skia-rust Authors.
// Licensed under the Apache License, Version 2.0; see the LICENSE file of this crate. This file
// is modified from the Wuffs sources.
// Port of: wuffs-v0.3.c, the status code strings of the base, gif and lzw modules
// (`wuffs_base__note__*`, `wuffs_base__suspension__*`, `wuffs_base__error__*`,
// `wuffs_gif__error__*` and `wuffs_lzw__error__*`). The text is byte-for-byte the C text, because
// SkWuffsCodec prints it and the differential harness compares it.

/// `wuffs_base__note__end_of_data`
pub const NOTE_END_OF_DATA: &str = "@base: end of data";

/// `wuffs_base__suspension__short_read`
pub const SUSPENSION_SHORT_READ: &str = "$base: short read";
/// `wuffs_base__suspension__short_write`
pub const SUSPENSION_SHORT_WRITE: &str = "$base: short write";
/// `wuffs_base__suspension__mispositioned_read`
pub const SUSPENSION_MISPOSITIONED_READ: &str = "$base: mispositioned read";

/// `wuffs_base__error__bad_argument`
pub const ERROR_BAD_ARGUMENT: &str = "#base: bad argument";
/// `wuffs_base__error__bad_argument_length_too_short`
pub const ERROR_BAD_ARGUMENT_LENGTH_TOO_SHORT: &str = "#base: bad argument (length too short)";
/// `wuffs_base__error__bad_call_sequence`
pub const ERROR_BAD_CALL_SEQUENCE: &str = "#base: bad call sequence";
/// `wuffs_base__error__bad_receiver`
pub const ERROR_BAD_RECEIVER: &str = "#base: bad receiver";
/// `wuffs_base__error__bad_restart`
pub const ERROR_BAD_RESTART: &str = "#base: bad restart";
/// `wuffs_base__error__cannot_return_a_suspension`
pub const ERROR_CANNOT_RETURN_A_SUSPENSION: &str = "#base: cannot return a suspension";
/// `wuffs_base__error__disabled_by_previous_error`
pub const ERROR_DISABLED_BY_PREVIOUS_ERROR: &str = "#base: disabled by previous error";
/// `wuffs_base__error__interleaved_coroutine_calls`
pub const ERROR_INTERLEAVED_COROUTINE_CALLS: &str = "#base: interleaved coroutine calls";
/// `wuffs_base__error__initialize_not_called`
pub const ERROR_INITIALIZE_NOT_CALLED: &str = "#base: initialize not called";
/// `wuffs_base__error__not_enough_data`
pub const ERROR_NOT_ENOUGH_DATA: &str = "#base: not enough data";
/// `wuffs_base__error__too_much_data`
pub const ERROR_TOO_MUCH_DATA: &str = "#base: too much data";
/// `wuffs_base__error__unsupported_option`
pub const ERROR_UNSUPPORTED_OPTION: &str = "#base: unsupported option";
/// `wuffs_base__error__unsupported_pixel_swizzler_option`
pub const ERROR_UNSUPPORTED_PIXEL_SWIZZLER_OPTION: &str =
    "#base: unsupported pixel swizzler option";

/// `wuffs_gif__error__bad_extension_label`
pub const GIF_ERROR_BAD_EXTENSION_LABEL: &str = "#gif: bad extension label";
/// `wuffs_gif__error__bad_frame_size`
pub const GIF_ERROR_BAD_FRAME_SIZE: &str = "#gif: bad frame size";
/// `wuffs_gif__error__bad_graphic_control`
pub const GIF_ERROR_BAD_GRAPHIC_CONTROL: &str = "#gif: bad graphic control";
/// `wuffs_gif__error__bad_header`
pub const GIF_ERROR_BAD_HEADER: &str = "#gif: bad header";
/// `wuffs_gif__error__bad_literal_width`
pub const GIF_ERROR_BAD_LITERAL_WIDTH: &str = "#gif: bad literal width";
/// `wuffs_gif__error__bad_palette`
pub const GIF_ERROR_BAD_PALETTE: &str = "#gif: bad palette";
/// `wuffs_gif__error__truncated_input`
pub const GIF_ERROR_TRUNCATED_INPUT: &str = "#gif: truncated input";
/// `wuffs_gif__error__internal_error_inconsistent_ri_wi`
pub const GIF_ERROR_INTERNAL_INCONSISTENT_RI_WI: &str = "#gif: internal error: inconsistent ri/wi";

/// `wuffs_lzw__error__bad_code`
pub const LZW_ERROR_BAD_CODE: &str = "#lzw: bad code";
/// `wuffs_lzw__error__truncated_input`
pub const LZW_ERROR_TRUNCATED_INPUT: &str = "#lzw: truncated input";
/// `wuffs_lzw__error__internal_error_inconsistent_i_o`
pub const LZW_ERROR_INTERNAL_INCONSISTENT_I_O: &str = "#lzw: internal error: inconsistent I/O";
