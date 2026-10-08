// Port of: wuffs-v0.3.c, the "gif" module (`wuffs_gif__decoder__*`, lines 31677-34471 of the
// release C file at google/wuffs-mirror-release-c@e3f919cc), restricted to the entry points
// SkWuffsCodec calls: `initialize`, `set_quirk_enabled`, `decode_image_config`,
// `decode_frame_config`, `decode_frame`, `restart_frame`, `num_animation_loops`,
// `num_decoded_frame_configs`, `num_decoded_frames` and `frame_dirty_rect`.
//
// Coroutines. Each Wuffs coroutine resumes at the `case N:` label recorded in its `p_*` field. In
// Rust that is a `loop { match coro { ... } }` in which every C `case N:` is an arm, and
// falling through a `WUFFS_BASE__COROUTINE_SUSPENSION_POINT(N)` is `coro = N; continue`. A
// suspension is `break` with the status and the current `coro` value. Locals that live across a
// suspension are fields named `s_<coroutine>_<name>`, as in the C `private_data`.
//
// Omitted, because SkWuffsCodec never reaches them: `tell_me_more` and the metadata reporting it
// drives (`set_report_metadata` is never called, so the ICCP/XMP branches of `decode_ae` can never
// run), `num_decoded_frame_configs` wrappers beyond the accessor, and the `workbuf` argument
// (`workbuf_len` is 0 for GIF). The quirk flags and the palette, interlace, disposal and blend
// logic are ported in full.

// The ported arithmetic is Wuffs' C integer arithmetic (uint8, uint16, uint32, uint64 and size_t
// conversions, with the truncating and sign-changing casts the C code relies on). Each cast mirrors
// one C cast, so the cast lints are allowed for the module.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless
)]
// Each coroutine is one state machine with one arm per C `case` label. Splitting a coroutine
// would move its resumption points out of the function that owns the state, so the length lint
// is allowed for the module.
#![allow(clippy::too_many_lines)]

use crate::base::{
    self, AnimationDisposal, FrameConfig, ImageConfig, IoBuffer, IoMeta, PixelBlend, PixelBuffer,
    PixelFormat, PixelSwizzler, RectIeU32, Status,
};
use crate::lzw::{LzwDecoder, Magic};
use crate::strings;

/// `WUFFS_GIF__QUIRK_DELAY_NUM_DECODED_FRAMES`
pub const QUIRK_DELAY_NUM_DECODED_FRAMES: u32 = 1_041_635_328;
/// `WUFFS_GIF__QUIRK_FIRST_FRAME_LOCAL_PALETTE_MEANS_BLACK_BACKGROUND`
pub const QUIRK_FIRST_FRAME_LOCAL_PALETTE_MEANS_BLACK_BACKGROUND: u32 = 1_041_635_329;
/// `WUFFS_GIF__QUIRK_HONOR_BACKGROUND_COLOR`
pub const QUIRK_HONOR_BACKGROUND_COLOR: u32 = 1_041_635_330;
/// `WUFFS_GIF__QUIRK_IGNORE_TOO_MUCH_PIXEL_DATA`: the quirk `SkWuffsCodec` enables.
pub const QUIRK_IGNORE_TOO_MUCH_PIXEL_DATA: u32 = 1_041_635_331;
/// `WUFFS_GIF__QUIRK_IMAGE_BOUNDS_ARE_STRICT`
pub const QUIRK_IMAGE_BOUNDS_ARE_STRICT: u32 = 1_041_635_332;
/// `WUFFS_GIF__QUIRK_REJECT_EMPTY_FRAME`
pub const QUIRK_REJECT_EMPTY_FRAME: u32 = 1_041_635_333;
/// `WUFFS_GIF__QUIRK_REJECT_EMPTY_PALETTE`
pub const QUIRK_REJECT_EMPTY_PALETTE: u32 = 1_041_635_334;

const QUIRKS_BASE: u32 = 1_041_635_328;
const QUIRKS_COUNT: u32 = 7;

/// `WUFFS_BASE__PIXEL_FORMAT__INDEXED__BGRA_BINARY`, the pixel format `decode_image_config`
/// reports. The C constant is written `2198077448` in the generated code.
const SOURCE_PIXFMT: u32 = 2_198_077_448;

/// `WUFFS_GIF__INTERLACE_START`
const INTERLACE_START: [u32; 5] = [4_294_967_295, 1, 2, 4, 0];
/// `WUFFS_GIF__INTERLACE_DELTA`
const INTERLACE_DELTA: [u8; 5] = [1, 2, 4, 8, 8];
/// `WUFFS_GIF__INTERLACE_COUNT`
const INTERLACE_COUNT: [u8; 5] = [0, 1, 2, 4, 8];
/// `WUFFS_GIF__ANIMEXTS1DOT0`
const ANIMEXTS1DOT0: &[u8; 11] = b"ANIMEXTS1.0";
/// `WUFFS_GIF__NETSCAPE2DOT0`
const NETSCAPE2DOT0: &[u8; 11] = b"NETSCAPE2.0";

/// The color Wuffs writes for a transparent first-frame background (`0x00000000` premultiplied
/// ARGB is `0`; `0xFF000000` is opaque black, `4278190080` in the C code).
const OPAQUE_BLACK_ARGB: u32 = 0xFF00_0000;

/// `WUFFS_GIF__ERROR__...`-free marker for "background colour is to be resolved" (`77` in the C
/// code).
const BACKGROUND_UNRESOLVED: u32 = 77;

/// Outcome of a `Status` returned by a sub-coroutine: the sub-coroutine returned a status other
/// than OK, so the caller suspends or exits.
fn non_ok(s: Status) -> bool {
    s.repr().is_some()
}

/// Port of the scratch-based little-endian 16-bit read in the C coroutines (`num_bits` stored in
/// the top byte of `scratch`). Returns `None` on a short read, leaving the state to resume from.
fn read_u16le_scratch(scratch: &mut u64, data: &[u8], iop: &mut usize, io2: usize) -> Option<u32> {
    loop {
        if *iop == io2 {
            return None;
        }
        let mut num_bits = (*scratch >> 56) as u32;
        *scratch <<= 8;
        *scratch >>= 8;
        *scratch |= u64::from(data[*iop]) << num_bits;
        *iop += 1;
        if num_bits == 8 {
            return Some(*scratch as u32);
        }
        num_bits += 8;
        *scratch |= u64::from(num_bits) << 56;
    }
}

/// Port of the scratch-based big-endian 24-bit read in the C coroutines (`num_bits` stored in the
/// low byte of `scratch`). Returns `None` on a short read.
fn read_u24be_scratch(scratch: &mut u64, data: &[u8], iop: &mut usize, io2: usize) -> Option<u32> {
    loop {
        if *iop == io2 {
            return None;
        }
        let mut num_bits = (*scratch & 0xFF) as u32;
        *scratch >>= 8;
        *scratch <<= 8;
        *scratch |= u64::from(data[*iop]) << (56 - num_bits);
        *iop += 1;
        if num_bits == 16 {
            return Some((*scratch >> 40) as u32);
        }
        num_bits += 8;
        *scratch |= u64::from(num_bits);
    }
}

/// Port of `wuffs_gif__decoder`. Construct one with [`GifDecoder::new`], which is
/// `wuffs_gif__decoder__initialize` with Skia's default options.
// The flags mirror the C `private_impl` bool fields one for one.
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct GifDecoder {
    magic: Magic,
    active_coroutine: u32,

    f_width: u32,
    f_height: u32,
    f_call_sequence: u8,
    f_quirks: [bool; 7],
    f_delayed_num_decoded_frames: bool,
    f_previous_lzw_decode_ended_abruptly: bool,
    f_seen_header: bool,
    f_has_global_palette: bool,
    f_interlace: u8,
    f_seen_num_animation_loops_value: bool,
    f_num_animation_loops_value: u32,
    f_background_color_u32_argb_premul: u32,
    f_black_color_u32_argb_premul: u32,
    f_gc_has_transparent_index: bool,
    f_gc_transparent_index: u8,
    f_gc_disposal: u8,
    f_gc_duration: u64,
    f_frame_config_io_position: u64,
    f_num_decoded_frame_configs_value: u64,
    f_num_decoded_frames_value: u64,
    f_frame_rect_x0: u32,
    f_frame_rect_y0: u32,
    f_frame_rect_x1: u32,
    f_frame_rect_y1: u32,
    f_dst_x: u32,
    f_dst_y: u32,
    f_dirty_max_excl_y: u32,
    f_compressed_ri: u64,
    f_compressed_wi: u64,
    f_swizzler: PixelSwizzler,

    // Coroutine resumption points (`p_*`).
    p_decode_image_config: u32,
    p_do_decode_image_config: u32,
    p_decode_frame_config: u32,
    p_do_decode_frame_config: u32,
    p_skip_frame: u32,
    p_decode_frame: u32,
    p_do_decode_frame: u32,
    p_decode_up_to_id_part1: u32,
    p_decode_header: u32,
    p_decode_lsd: u32,
    p_decode_extension: u32,
    p_skip_blocks: u32,
    p_decode_ae: u32,
    p_decode_gc: u32,
    p_decode_id_part0: u32,
    p_decode_id_part1: u32,
    p_decode_id_part2: u32,

    // `private_data`.
    f_compressed: Box<[u8; 4096]>,
    f_palettes: Box<[[u8; 1024]; 2]>,
    f_dst_palette: Box<[u8; 1024]>,
    f_lzw: LzwDecoder,

    // Coroutine locals that live across suspensions (`s_*`).
    s_do_decode_frame_config_v_background_color: u32,
    s_skip_frame_scratch: u64,
    s_decode_header_v_c: [u8; 6],
    s_decode_header_v_i: u32,
    s_decode_lsd_v_flags: u8,
    s_decode_lsd_v_background_color_index: u8,
    s_decode_lsd_v_num_palette_entries: u32,
    s_decode_lsd_v_i: u32,
    s_decode_lsd_scratch: u64,
    s_skip_blocks_scratch: u64,
    s_decode_ae_v_block_size: u8,
    s_decode_ae_v_is_animexts: bool,
    s_decode_ae_v_is_netscape: bool,
    s_decode_ae_scratch: u64,
    s_decode_gc_scratch: u64,
    s_decode_id_part0_scratch: u64,
    s_decode_id_part1_v_which_palette: u8,
    s_decode_id_part1_v_num_palette_entries: u32,
    s_decode_id_part1_v_i: u32,
    s_decode_id_part1_scratch: u64,
    s_decode_id_part2_v_block_size: u64,
    s_decode_id_part2_v_need_block_size: bool,
}

impl Default for GifDecoder {
    fn default() -> Self {
        GifDecoder::new()
    }
}

impl GifDecoder {
    /// Port of `wuffs_gif__decoder__initialize` with `WUFFS_INITIALIZE__DEFAULT_OPTIONS`.
    #[must_use]
    pub fn new() -> Self {
        GifDecoder {
            magic: Magic::Valid,
            active_coroutine: 0,
            f_width: 0,
            f_height: 0,
            f_call_sequence: 0,
            f_quirks: [false; 7],
            f_delayed_num_decoded_frames: false,
            f_previous_lzw_decode_ended_abruptly: false,
            f_seen_header: false,
            f_has_global_palette: false,
            f_interlace: 0,
            f_seen_num_animation_loops_value: false,
            f_num_animation_loops_value: 0,
            f_background_color_u32_argb_premul: 0,
            f_black_color_u32_argb_premul: 0,
            f_gc_has_transparent_index: false,
            f_gc_transparent_index: 0,
            f_gc_disposal: 0,
            f_gc_duration: 0,
            f_frame_config_io_position: 0,
            f_num_decoded_frame_configs_value: 0,
            f_num_decoded_frames_value: 0,
            f_frame_rect_x0: 0,
            f_frame_rect_y0: 0,
            f_frame_rect_x1: 0,
            f_frame_rect_y1: 0,
            f_dst_x: 0,
            f_dst_y: 0,
            f_dirty_max_excl_y: 0,
            f_compressed_ri: 0,
            f_compressed_wi: 0,
            f_swizzler: PixelSwizzler::default(),
            p_decode_image_config: 0,
            p_do_decode_image_config: 0,
            p_decode_frame_config: 0,
            p_do_decode_frame_config: 0,
            p_skip_frame: 0,
            p_decode_frame: 0,
            p_do_decode_frame: 0,
            p_decode_up_to_id_part1: 0,
            p_decode_header: 0,
            p_decode_lsd: 0,
            p_decode_extension: 0,
            p_skip_blocks: 0,
            p_decode_ae: 0,
            p_decode_gc: 0,
            p_decode_id_part0: 0,
            p_decode_id_part1: 0,
            p_decode_id_part2: 0,
            f_compressed: Box::new([0; 4096]),
            f_palettes: Box::new([[0; 1024]; 2]),
            f_dst_palette: Box::new([0; 1024]),
            f_lzw: LzwDecoder::new(),
            s_do_decode_frame_config_v_background_color: 0,
            s_skip_frame_scratch: 0,
            s_decode_header_v_c: [0; 6],
            s_decode_header_v_i: 0,
            s_decode_lsd_v_flags: 0,
            s_decode_lsd_v_background_color_index: 0,
            s_decode_lsd_v_num_palette_entries: 0,
            s_decode_lsd_v_i: 0,
            s_decode_lsd_scratch: 0,
            s_skip_blocks_scratch: 0,
            s_decode_ae_v_block_size: 0,
            s_decode_ae_v_is_animexts: false,
            s_decode_ae_v_is_netscape: false,
            s_decode_ae_scratch: 0,
            s_decode_gc_scratch: 0,
            s_decode_id_part0_scratch: 0,
            s_decode_id_part1_v_which_palette: 0,
            s_decode_id_part1_v_num_palette_entries: 0,
            s_decode_id_part1_v_i: 0,
            s_decode_id_part1_scratch: 0,
            s_decode_id_part2_v_block_size: 0,
            s_decode_id_part2_v_need_block_size: false,
        }
    }

    /// Port of `wuffs_gif__decoder__set_quirk_enabled`. `quirk` is one of the `QUIRK_*`
    /// constants; quirks set after the first call are ignored, as in the C code.
    pub fn set_quirk_enabled(&mut self, quirk: u32, enabled: bool) {
        if self.magic != Magic::Valid {
            return;
        }
        if self.f_call_sequence == 0 && quirk >= QUIRKS_BASE {
            let q = quirk - QUIRKS_BASE;
            if q < QUIRKS_COUNT {
                self.f_quirks[q as usize] = enabled;
            }
        }
    }

    /// Port of `wuffs_gif__decoder__num_animation_loops`.
    #[must_use]
    pub fn num_animation_loops(&self) -> u32 {
        if self.magic != Magic::Valid && self.magic != Magic::Disabled {
            return 0;
        }
        if self.f_seen_num_animation_loops_value {
            return self.f_num_animation_loops_value;
        }
        if self.f_num_decoded_frame_configs_value > 1 {
            return 1;
        }
        0
    }

    /// Port of `wuffs_gif__decoder__num_decoded_frame_configs`.
    #[must_use]
    pub fn num_decoded_frame_configs(&self) -> u64 {
        if self.magic != Magic::Valid && self.magic != Magic::Disabled {
            return 0;
        }
        self.f_num_decoded_frame_configs_value
    }

    /// Port of `wuffs_gif__decoder__num_decoded_frames`.
    #[must_use]
    pub fn num_decoded_frames(&self) -> u64 {
        if self.magic != Magic::Valid && self.magic != Magic::Disabled {
            return 0;
        }
        self.f_num_decoded_frames_value
    }

    /// Port of `wuffs_gif__decoder__frame_dirty_rect`.
    #[must_use]
    pub fn frame_dirty_rect(&self) -> RectIeU32 {
        if self.magic != Magic::Valid && self.magic != Magic::Disabled {
            return RectIeU32::default();
        }
        RectIeU32::new(
            self.f_frame_rect_x0.min(self.f_width),
            self.f_frame_rect_y0.min(self.f_height),
            self.f_frame_rect_x1.min(self.f_width),
            self.f_dirty_max_excl_y.min(self.f_height),
        )
    }

    /// Port of `wuffs_gif__decoder__restart_frame`: after `index` frames, resumes at the frame
    /// whose data starts at `io_position`.
    pub fn restart_frame(&mut self, index: u64, io_position: u64) -> Status {
        if self.magic != Magic::Valid {
            return Status::new(if self.magic == Magic::Disabled {
                strings::ERROR_DISABLED_BY_PREVIOUS_ERROR
            } else {
                strings::ERROR_INITIALIZE_NOT_CALLED
            });
        }
        if self.f_call_sequence < 32 {
            return Status::new(strings::ERROR_BAD_CALL_SEQUENCE);
        } else if io_position == 0 {
            return Status::new(strings::ERROR_BAD_ARGUMENT);
        }
        self.f_delayed_num_decoded_frames = false;
        self.f_frame_config_io_position = io_position;
        self.f_num_decoded_frame_configs_value = index;
        self.f_num_decoded_frames_value = index;
        self.reset_gc();
        self.f_call_sequence = 40;
        Status::OK
    }

    /// Port of `wuffs_gif__decoder__decode_image_config`. Fills `dst` (when given) with the
    /// canvas size and the first frame's opacity, and suspends with `short read` when `src` runs
    /// out before the header and the first frame's introduction are read.
    pub fn decode_image_config(
        &mut self,
        dst: Option<&mut ImageConfig>,
        src: &mut IoBuffer,
    ) -> Status {
        if let Some(s) = self.check_entry(1) {
            return s;
        }
        let v = self.do_decode_image_config(dst, src);
        self.finish_wrapper(v, src, 1)
    }

    /// Port of `wuffs_gif__decoder__decode_frame_config`.
    pub fn decode_frame_config(
        &mut self,
        dst: Option<&mut FrameConfig>,
        src: &mut IoBuffer,
    ) -> Status {
        if let Some(s) = self.check_entry(3) {
            return s;
        }
        let v = self.do_decode_frame_config(dst, src);
        self.finish_wrapper(v, src, 3)
    }

    /// Port of `wuffs_gif__decoder__decode_frame`: decodes the next frame into `dst`, blending
    /// with `blend`. The `workbuf` argument of the C API is not needed for GIF.
    pub fn decode_frame(
        &mut self,
        dst: &mut PixelBuffer<'_>,
        src: &mut IoBuffer,
        blend: PixelBlend,
    ) -> Status {
        if let Some(s) = self.check_entry(4) {
            return s;
        }
        let v = self.do_decode_frame(dst, src, blend);
        self.finish_wrapper(v, src, 4)
    }

    /// The entry checks shared by the public coroutine wrappers. `active` is the coroutine id the
    /// wrapper records in `active_coroutine` (1 for image config, 3 for frame config, 4 for frame).
    fn check_entry(&mut self, active: u32) -> Option<Status> {
        if self.magic != Magic::Valid {
            return Some(Status::new(if self.magic == Magic::Disabled {
                strings::ERROR_DISABLED_BY_PREVIOUS_ERROR
            } else {
                strings::ERROR_INITIALIZE_NOT_CALLED
            }));
        }
        if self.active_coroutine != 0 && self.active_coroutine != active {
            self.magic = Magic::Disabled;
            return Some(Status::new(strings::ERROR_INTERLEAVED_COROUTINE_CALLS));
        }
        self.active_coroutine = 0;
        None
    }

    /// The tail of each public wrapper: a truncated input (short read after `closed`) becomes an
    /// error, a suspension is recorded in `active_coroutine` and the wrapper's resumption point,
    /// and an error disables the decoder. The wrapper's only resumption point is case 1, which
    /// calls the `do_` coroutine again, so the point value is 1 whenever it suspends.
    fn finish_wrapper(&mut self, v: Status, src: &IoBuffer, active: u32) -> Status {
        let status = if v.repr() == Some(strings::SUSPENSION_SHORT_READ) && src.meta.closed {
            Status::new(strings::GIF_ERROR_TRUNCATED_INPUT)
        } else {
            v
        };
        let susp = status.is_suspension();
        let p = u32::from(susp);
        match active {
            1 => self.p_decode_image_config = p,
            3 => self.p_decode_frame_config = p,
            _ => self.p_decode_frame = p,
        }
        self.active_coroutine = if susp { active } else { 0 };
        if status.is_error() {
            self.magic = Magic::Disabled;
        }
        status
    }

    // ---------------------------------------------------------------------------------------
    // Coroutines.

    /// Port of `wuffs_gif__decoder__do_decode_image_config`.
    fn do_decode_image_config(
        &mut self,
        dst: Option<&mut ImageConfig>,
        src: &mut IoBuffer,
    ) -> Status {
        let mut coro = self.p_do_decode_image_config;
        let status = 'co: loop {
            match coro {
                0 => {
                    if self.f_call_sequence != 0 {
                        break 'co Status::new(strings::ERROR_BAD_CALL_SEQUENCE);
                    } else if !self.f_seen_header {
                        coro = 1;
                    } else {
                        coro = 3;
                    }
                }
                1 => {
                    let st = self.decode_header(src);
                    if non_ok(st) {
                        break 'co st;
                    }
                    coro = 2;
                }
                2 => {
                    let st = self.decode_lsd(src);
                    if non_ok(st) {
                        break 'co st;
                    }
                    self.f_seen_header = true;
                    coro = 3;
                }
                3 => {
                    let st = self.decode_up_to_id_part1(src);
                    if non_ok(st) {
                        break 'co st;
                    }
                    let mut v_ffio = !self.f_gc_has_transparent_index;
                    if !self.f_quirks[2] {
                        v_ffio = v_ffio
                            && self.f_frame_rect_x0 == 0
                            && self.f_frame_rect_y0 == 0
                            && self.f_frame_rect_x1 == self.f_width
                            && self.f_frame_rect_y1 == self.f_height;
                    } else if v_ffio {
                        self.f_black_color_u32_argb_premul = OPAQUE_BLACK_ARGB;
                    }
                    if self.f_background_color_u32_argb_premul == BACKGROUND_UNRESOLVED {
                        self.f_background_color_u32_argb_premul =
                            self.f_black_color_u32_argb_premul;
                    }
                    if let Some(d) = dst {
                        d.set(
                            SOURCE_PIXFMT,
                            0,
                            self.f_width,
                            self.f_height,
                            self.f_frame_config_io_position,
                            v_ffio,
                        );
                    }
                    if self.f_call_sequence == 0 {
                        self.f_call_sequence = 32;
                    }
                    break 'co Status::OK;
                }
                _ => unreachable!("invalid do_decode_image_config resumption point {coro}"),
            }
        };
        self.p_do_decode_image_config = if status.is_suspension() { coro } else { 0 };
        status
    }

    /// Port of `wuffs_gif__decoder__do_decode_frame_config`.
    fn do_decode_frame_config(
        &mut self,
        dst: Option<&mut FrameConfig>,
        src: &mut IoBuffer,
    ) -> Status {
        let mut iop = src.meta.ri;
        let io2 = src.meta.wi;
        let mut coro = self.p_do_decode_frame_config;
        if coro == 0 {
            self.s_do_decode_frame_config_v_background_color = 0;
        }
        let mut v_background_color = self.s_do_decode_frame_config_v_background_color;
        let status = 'co: loop {
            match coro {
                0 => {
                    self.f_dirty_max_excl_y = 0;
                    if (self.f_call_sequence & 16) != 0 {
                        break 'co Status::new(strings::ERROR_BAD_CALL_SEQUENCE);
                    } else if self.f_call_sequence == 32 {
                        // Nothing to do: the previous frame was fully read.
                    } else if self.f_call_sequence < 32 {
                        // SUSPENSION_POINT(1)
                        coro = 1;
                        continue;
                    } else if self.f_call_sequence == 40 {
                        if self.f_frame_config_io_position
                            != src.meta.pos.saturating_add(iop as u64)
                        {
                            break 'co Status::new(strings::ERROR_BAD_RESTART);
                        }
                    } else if self.f_call_sequence == 64 {
                        // SUSPENSION_POINT(2)
                        coro = 2;
                        continue;
                    } else {
                        break 'co Status::new(strings::NOTE_END_OF_DATA);
                    }
                    coro = 5;
                }
                1 => {
                    src.meta.ri = iop;
                    let st = self.do_decode_image_config(None, src);
                    iop = src.meta.ri;
                    if non_ok(st) {
                        break 'co st;
                    }
                    coro = 5;
                }
                2 => {
                    src.meta.ri = iop;
                    let st = self.skip_frame(src);
                    iop = src.meta.ri;
                    if non_ok(st) {
                        break 'co st;
                    }
                    if self.f_call_sequence >= 96 {
                        break 'co Status::new(strings::NOTE_END_OF_DATA);
                    }
                    coro = 5;
                }
                5 => {
                    // The block after the if/else chain above.
                    if self.f_num_decoded_frame_configs_value > 0 || self.f_call_sequence == 40 {
                        coro = 3;
                    } else {
                        coro = 6;
                    }
                }
                3 => {
                    src.meta.ri = iop;
                    let st = self.decode_up_to_id_part1(src);
                    iop = src.meta.ri;
                    if non_ok(st) {
                        break 'co st;
                    }
                    if self.f_call_sequence >= 96 {
                        break 'co Status::new(strings::NOTE_END_OF_DATA);
                    }
                    coro = 6;
                }
                6 => {
                    v_background_color = self.f_black_color_u32_argb_premul;
                    if !self.f_gc_has_transparent_index {
                        v_background_color = self.f_background_color_u32_argb_premul;
                        if self.f_quirks[1] && self.f_num_decoded_frame_configs_value == 0 {
                            coro = 4;
                            continue;
                        }
                    }
                    coro = 7;
                }
                4 => {
                    // The `while (io2 - iop <= 0)` loop; resumption re-checks the condition.
                    if io2 <= iop {
                        self.s_do_decode_frame_config_v_background_color = v_background_color;
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let v_flags = src.data[iop];
                    if (v_flags & 128) != 0 {
                        v_background_color = self.f_black_color_u32_argb_premul;
                    }
                    coro = 7;
                }
                7 => {
                    if let Some(d) = dst {
                        let bounds = RectIeU32::new(
                            self.f_frame_rect_x0.min(self.f_width),
                            self.f_frame_rect_y0.min(self.f_height),
                            self.f_frame_rect_x1.min(self.f_width),
                            self.f_frame_rect_y1.min(self.f_height),
                        );
                        d.set(
                            bounds,
                            self.f_gc_duration as i64,
                            self.f_num_decoded_frame_configs_value,
                            self.f_frame_config_io_position,
                            disposal_from_gc(self.f_gc_disposal),
                            !self.f_gc_has_transparent_index,
                            false,
                            v_background_color,
                        );
                    }
                    self.f_num_decoded_frame_configs_value =
                        self.f_num_decoded_frame_configs_value.saturating_add(1);
                    self.f_call_sequence = 64;
                    break 'co Status::OK;
                }
                _ => unreachable!("invalid do_decode_frame_config resumption point {coro}"),
            }
        };
        self.s_do_decode_frame_config_v_background_color = v_background_color;
        self.p_do_decode_frame_config = if status.is_suspension() { coro } else { 0 };
        src.meta.ri = iop;
        status
    }

    /// Port of `wuffs_gif__decoder__skip_frame`.
    fn skip_frame(&mut self, src: &mut IoBuffer) -> Status {
        let mut iop = src.meta.ri;
        let io2 = src.meta.wi;
        let mut coro = self.p_skip_frame;
        let status = 'co: loop {
            match coro {
                0 => {
                    coro = 1;
                }
                1 => {
                    // SUSPENSION_POINT(1)
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let v_flags = src.data[iop];
                    iop += 1;
                    if (v_flags & 128) != 0 {
                        self.s_skip_frame_scratch = u64::from(3u32 << (1 + (v_flags & 7)));
                        coro = 2;
                    } else {
                        coro = 3;
                    }
                }
                2 => {
                    // SUSPENSION_POINT(2)
                    if self.s_skip_frame_scratch > (io2 - iop) as u64 {
                        self.s_skip_frame_scratch -= (io2 - iop) as u64;
                        iop = io2;
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    iop += self.s_skip_frame_scratch as usize;
                    coro = 3;
                }
                3 => {
                    // SUSPENSION_POINT(3)
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let v_lw = src.data[iop];
                    iop += 1;
                    if v_lw > 8 {
                        // `goto exit`: the error leaves the coroutine state alone.
                        src.meta.ri = iop;
                        return Status::new(strings::GIF_ERROR_BAD_LITERAL_WIDTH);
                    }
                    coro = 4;
                }
                4 => {
                    // SUSPENSION_POINT(4)
                    src.meta.ri = iop;
                    let st = self.skip_blocks(src);
                    iop = src.meta.ri;
                    if non_ok(st) {
                        break 'co st;
                    }
                    if self.f_quirks[0] {
                        self.f_delayed_num_decoded_frames = true;
                    } else {
                        self.f_num_decoded_frames_value =
                            self.f_num_decoded_frames_value.saturating_add(1);
                    }
                    self.reset_gc();
                    self.f_call_sequence = 32;
                    break 'co Status::OK;
                }
                _ => unreachable!("invalid skip_frame resumption point {coro}"),
            }
        };
        self.p_skip_frame = if status.is_suspension() { coro } else { 0 };
        src.meta.ri = iop;
        status
    }

    /// Port of `wuffs_gif__decoder__do_decode_frame`.
    fn do_decode_frame(
        &mut self,
        dst: &mut PixelBuffer<'_>,
        src: &mut IoBuffer,
        blend: PixelBlend,
    ) -> Status {
        let mut coro = self.p_do_decode_frame;
        let status = 'co: loop {
            match coro {
                0 => match self.f_call_sequence {
                    64 => coro = 2,
                    0..64 => coro = 1,
                    _ => break 'co Status::new(strings::NOTE_END_OF_DATA),
                },
                1 => {
                    let st = self.do_decode_frame_config(None, src);
                    if non_ok(st) {
                        break 'co st;
                    }
                    coro = 2;
                }
                2 => {
                    if self.f_quirks[5]
                        && (self.f_frame_rect_x0 == self.f_frame_rect_x1
                            || self.f_frame_rect_y0 == self.f_frame_rect_y1)
                    {
                        // `goto exit` with an error: nothing is saved.
                        return Status::new(strings::GIF_ERROR_BAD_FRAME_SIZE);
                    }
                    coro = 3;
                }
                3 => {
                    let st = self.decode_id_part1(dst, src, blend);
                    if non_ok(st) {
                        break 'co st;
                    }
                    coro = 4;
                }
                4 => {
                    let st = self.decode_id_part2(dst, src);
                    if non_ok(st) {
                        break 'co st;
                    }
                    self.f_num_decoded_frames_value =
                        self.f_num_decoded_frames_value.saturating_add(1);
                    self.reset_gc();
                    self.f_call_sequence = 32;
                    break 'co Status::OK;
                }
                _ => unreachable!("invalid do_decode_frame resumption point {coro}"),
            }
        };
        self.p_do_decode_frame = if status.is_suspension() { coro } else { 0 };
        status
    }

    /// Port of `wuffs_gif__decoder__reset_gc`.
    fn reset_gc(&mut self) {
        self.f_gc_has_transparent_index = false;
        self.f_gc_transparent_index = 0;
        self.f_gc_disposal = 0;
        self.f_gc_duration = 0;
    }

    /// Port of `wuffs_gif__decoder__decode_up_to_id_part1`: skips extensions until an image
    /// descriptor (`0x2C`) or the trailer (anything else) is found.
    fn decode_up_to_id_part1(&mut self, src: &mut IoBuffer) -> Status {
        let mut iop = src.meta.ri;
        let io2 = src.meta.wi;
        let mut coro = self.p_decode_up_to_id_part1;
        if coro == 0
            && (self.f_frame_config_io_position == 0 || self.f_num_decoded_frame_configs_value > 0)
        {
            self.f_frame_config_io_position = src.meta.pos.saturating_add(iop as u64);
        }
        let status = 'co: loop {
            match coro {
                0 => {
                    coro = 1;
                }
                1 => {
                    // SUSPENSION_POINT(1), the top of the `while (true)` loop.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let v_block_type = src.data[iop];
                    iop += 1;
                    if v_block_type == 33 {
                        coro = 2;
                    } else if v_block_type == 44 {
                        if self.f_delayed_num_decoded_frames {
                            self.f_delayed_num_decoded_frames = false;
                            self.f_num_decoded_frames_value =
                                self.f_num_decoded_frames_value.saturating_add(1);
                        }
                        coro = 3;
                    } else {
                        if self.f_delayed_num_decoded_frames {
                            self.f_delayed_num_decoded_frames = false;
                            self.f_num_decoded_frames_value =
                                self.f_num_decoded_frames_value.saturating_add(1);
                        }
                        self.f_call_sequence = 96;
                        break 'co Status::OK;
                    }
                }
                2 => {
                    // SUSPENSION_POINT(2)
                    src.meta.ri = iop;
                    let st = self.decode_extension(src);
                    iop = src.meta.ri;
                    if non_ok(st) {
                        break 'co st;
                    }
                    coro = 1;
                }
                3 => {
                    // SUSPENSION_POINT(3)
                    src.meta.ri = iop;
                    let st = self.decode_id_part0(src);
                    iop = src.meta.ri;
                    if non_ok(st) {
                        break 'co st;
                    }
                    break 'co Status::OK;
                }
                _ => unreachable!("invalid decode_up_to_id_part1 resumption point {coro}"),
            }
        };
        self.p_decode_up_to_id_part1 = if status.is_suspension() { coro } else { 0 };
        src.meta.ri = iop;
        status
    }

    /// Port of `wuffs_gif__decoder__decode_header`: the six-byte signature, `GIF87a` or
    /// `GIF89a`.
    fn decode_header(&mut self, src: &mut IoBuffer) -> Status {
        let mut iop = src.meta.ri;
        let io2 = src.meta.wi;
        let mut coro = self.p_decode_header;
        if coro == 0 {
            self.s_decode_header_v_c = [0; 6];
            self.s_decode_header_v_i = 0;
        }
        let status = 'co: loop {
            match coro {
                0 => {
                    // `while (v_i < 6)`: the condition, checked on entry and after each byte.
                    if self.s_decode_header_v_i < 6 {
                        coro = 1;
                    } else {
                        let v_c = self.s_decode_header_v_c;
                        if v_c[0] != 71
                            || v_c[1] != 73
                            || v_c[2] != 70
                            || v_c[3] != 56
                            || (v_c[4] != 55 && v_c[4] != 57)
                            || v_c[5] != 97
                        {
                            break 'co Status::new(strings::GIF_ERROR_BAD_HEADER);
                        }
                        break 'co Status::OK;
                    }
                }
                1 => {
                    // SUSPENSION_POINT(1), the body of the `while` loop.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let i = self.s_decode_header_v_i as usize;
                    self.s_decode_header_v_c[i] = src.data[iop];
                    iop += 1;
                    self.s_decode_header_v_i += 1;
                    coro = 0;
                }
                _ => unreachable!("invalid decode_header resumption point {coro}"),
            }
        };
        self.p_decode_header = if status.is_suspension() { coro } else { 0 };
        src.meta.ri = iop;
        status
    }

    /// Port of `wuffs_gif__decoder__decode_lsd`: the logical screen descriptor and the global
    /// palette.
    fn decode_lsd(&mut self, src: &mut IoBuffer) -> Status {
        let mut iop = src.meta.ri;
        let io2 = src.meta.wi;
        let mut coro = self.p_decode_lsd;
        if coro == 0 {
            self.s_decode_lsd_v_flags = 0;
            self.s_decode_lsd_v_background_color_index = 0;
            self.s_decode_lsd_v_num_palette_entries = 0;
            self.s_decode_lsd_v_i = 0;
        }
        let status = 'co: loop {
            match coro {
                0 => coro = 1,
                1 => {
                    // Width: u16le. Fast path if two bytes are buffered, else the scratch loop.
                    if io2 - iop >= 2 {
                        self.f_width =
                            u32::from(u16::from_le_bytes([src.data[iop], src.data[iop + 1]]));
                        iop += 2;
                        coro = 3;
                    } else {
                        self.s_decode_lsd_scratch = 0;
                        coro = 2;
                    }
                }
                2 => match read_u16le_scratch(
                    &mut self.s_decode_lsd_scratch,
                    &src.data,
                    &mut iop,
                    io2,
                ) {
                    Some(t) => {
                        self.f_width = t;
                        coro = 3;
                    }
                    None => break 'co Status::new(strings::SUSPENSION_SHORT_READ),
                },
                3 => {
                    // Height: u16le.
                    if io2 - iop >= 2 {
                        self.f_height =
                            u32::from(u16::from_le_bytes([src.data[iop], src.data[iop + 1]]));
                        iop += 2;
                        coro = 5;
                    } else {
                        self.s_decode_lsd_scratch = 0;
                        coro = 4;
                    }
                }
                4 => match read_u16le_scratch(
                    &mut self.s_decode_lsd_scratch,
                    &src.data,
                    &mut iop,
                    io2,
                ) {
                    Some(t) => {
                        self.f_height = t;
                        coro = 5;
                    }
                    None => break 'co Status::new(strings::SUSPENSION_SHORT_READ),
                },
                5 => {
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    self.s_decode_lsd_v_flags = src.data[iop];
                    iop += 1;
                    coro = 6;
                }
                6 => {
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    self.s_decode_lsd_v_background_color_index = src.data[iop];
                    iop += 1;
                    coro = 7;
                }
                7 => {
                    // The packed aspect ratio byte, ignored.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    iop += 1;
                    self.s_decode_lsd_v_i = 0;
                    self.f_has_global_palette = (self.s_decode_lsd_v_flags & 128) != 0;
                    if self.f_has_global_palette {
                        self.s_decode_lsd_v_num_palette_entries =
                            1u32 << (1 + (self.s_decode_lsd_v_flags & 7));
                        coro = 8;
                    } else {
                        coro = 12;
                    }
                }
                8 => {
                    // The global palette loop condition.
                    if self.s_decode_lsd_v_i < self.s_decode_lsd_v_num_palette_entries {
                        coro = 9;
                    } else {
                        coro = 13;
                    }
                }
                9 => {
                    // One palette entry: u24be, fast path or scratch loop.
                    if io2 - iop >= 3 {
                        let t = (u32::from(src.data[iop]) << 16)
                            | (u32::from(src.data[iop + 1]) << 8)
                            | u32::from(src.data[iop + 2]);
                        iop += 3;
                        self.store_lsd_palette_entry(t);
                        coro = 8;
                    } else {
                        self.s_decode_lsd_scratch = 0;
                        coro = 10;
                    }
                }
                10 => match read_u24be_scratch(
                    &mut self.s_decode_lsd_scratch,
                    &src.data,
                    &mut iop,
                    io2,
                ) {
                    Some(t) => {
                        self.store_lsd_palette_entry(t);
                        coro = 8;
                    }
                    None => break 'co Status::new(strings::SUSPENSION_SHORT_READ),
                },
                12 => {
                    // No global palette: `v_i` is 0 and the default palette is written below.
                    coro = 14;
                }
                13 => {
                    // After the global palette: the background colour when honouring it.
                    if self.f_quirks[2] {
                        let idx = self.s_decode_lsd_v_background_color_index;
                        let n = self.s_decode_lsd_v_num_palette_entries;
                        if idx != 0 && u32::from(idx) < n {
                            let j = 4 * usize::from(idx);
                            let p = &self.f_palettes[0];
                            self.f_background_color_u32_argb_premul = u32::from(p[j])
                                | (u32::from(p[j + 1]) << 8)
                                | (u32::from(p[j + 2]) << 16)
                                | (u32::from(p[j + 3]) << 24);
                        } else {
                            self.f_background_color_u32_argb_premul = BACKGROUND_UNRESOLVED;
                        }
                    }
                    coro = 14;
                }
                14 => {
                    // The default palette fill: `while (v_i < 256)`.
                    let mut v_i = if self.f_has_global_palette {
                        self.s_decode_lsd_v_i
                    } else {
                        0
                    };
                    while v_i < 256 {
                        let i = 4 * v_i as usize;
                        self.f_palettes[0][i] = 0;
                        self.f_palettes[0][i + 1] = 0;
                        self.f_palettes[0][i + 2] = 0;
                        self.f_palettes[0][i + 3] = 255;
                        v_i += 1;
                    }
                    break 'co Status::OK;
                }
                _ => unreachable!("invalid decode_lsd resumption point {coro}"),
            }
        };
        self.p_decode_lsd = if status.is_suspension() { coro } else { 0 };
        src.meta.ri = iop;
        status
    }

    /// One entry of the global palette in `decode_lsd`: stores the colour and advances the
    /// index. Shared by the fast and scratch paths.
    fn store_lsd_palette_entry(&mut self, t: u32) {
        let v_argb = t | 0xFF00_0000;
        let i = self.s_decode_lsd_v_i as usize;
        let p = &mut self.f_palettes[0];
        p[4 * i] = (v_argb & 255) as u8;
        p[4 * i + 1] = ((v_argb >> 8) & 255) as u8;
        p[4 * i + 2] = ((v_argb >> 16) & 255) as u8;
        p[4 * i + 3] = ((v_argb >> 24) & 255) as u8;
        self.s_decode_lsd_v_i += 1;
    }

    /// Port of `wuffs_gif__decoder__decode_extension`.
    fn decode_extension(&mut self, src: &mut IoBuffer) -> Status {
        let mut iop = src.meta.ri;
        let io2 = src.meta.wi;
        let mut coro = self.p_decode_extension;
        let status = 'co: loop {
            match coro {
                0 => coro = 1,
                1 => {
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let v_label = src.data[iop];
                    iop += 1;
                    if v_label == 249 {
                        coro = 2;
                    } else if v_label == 255 {
                        coro = 3;
                    } else {
                        coro = 4;
                    }
                }
                2 => {
                    // Graphic control extension.
                    src.meta.ri = iop;
                    let st = self.decode_gc(src);
                    iop = src.meta.ri;
                    if non_ok(st) {
                        break 'co st;
                    }
                    break 'co Status::OK;
                }
                3 => {
                    // Application extension.
                    src.meta.ri = iop;
                    let st = self.decode_ae(src);
                    iop = src.meta.ri;
                    if non_ok(st) {
                        break 'co st;
                    }
                    break 'co Status::OK;
                }
                4 => {
                    // Any other extension: skipped.
                    src.meta.ri = iop;
                    let st = self.skip_blocks(src);
                    iop = src.meta.ri;
                    if non_ok(st) {
                        break 'co st;
                    }
                    break 'co Status::OK;
                }
                _ => unreachable!("invalid decode_extension resumption point {coro}"),
            }
        };
        self.p_decode_extension = if status.is_suspension() { coro } else { 0 };
        src.meta.ri = iop;
        status
    }

    /// Port of `wuffs_gif__decoder__skip_blocks`: skips data sub-blocks up to the zero-length
    /// terminator.
    fn skip_blocks(&mut self, src: &mut IoBuffer) -> Status {
        let mut iop = src.meta.ri;
        let io2 = src.meta.wi;
        let mut coro = self.p_skip_blocks;
        let status = 'co: loop {
            match coro {
                0 => coro = 1,
                1 => {
                    // SUSPENSION_POINT(1), the top of the `while (true)` loop.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let v_block_size = src.data[iop];
                    iop += 1;
                    if v_block_size == 0 {
                        break 'co Status::OK;
                    }
                    self.s_skip_blocks_scratch = u64::from(v_block_size);
                    coro = 2;
                }
                2 => {
                    // SUSPENSION_POINT(2): skip the block's bytes.
                    if self.s_skip_blocks_scratch > (io2 - iop) as u64 {
                        self.s_skip_blocks_scratch -= (io2 - iop) as u64;
                        iop = io2;
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    iop += self.s_skip_blocks_scratch as usize;
                    coro = 1;
                }
                _ => unreachable!("invalid skip_blocks resumption point {coro}"),
            }
        };
        self.p_skip_blocks = if status.is_suspension() { coro } else { 0 };
        src.meta.ri = iop;
        status
    }

    /// Port of `wuffs_gif__decoder__decode_ae`: the application extension. Only the NETSCAPE and
    /// ANIMEXTS loop-count blocks have an effect. The ICCP and XMP metadata branches are not
    /// ported (see the module comment).
    fn decode_ae(&mut self, src: &mut IoBuffer) -> Status {
        let mut iop = src.meta.ri;
        let io2 = src.meta.wi;
        let mut coro = self.p_decode_ae;
        if coro == 0 {
            self.s_decode_ae_v_block_size = 0;
            self.s_decode_ae_v_is_animexts = false;
            self.s_decode_ae_v_is_netscape = false;
        }
        let status = 'co: loop {
            match coro {
                0 => coro = 1,
                1 => {
                    // SUSPENSION_POINT(1): the block size.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    self.s_decode_ae_v_block_size = src.data[iop];
                    iop += 1;
                    if self.s_decode_ae_v_block_size == 0 {
                        break 'co Status::OK;
                    }
                    if self.s_decode_ae_v_block_size == 11 {
                        self.s_decode_ae_v_is_animexts = true;
                        self.s_decode_ae_v_is_netscape = true;
                        self.s_decode_ae_v_block_size = 0;
                        coro = 3;
                    } else {
                        self.s_decode_ae_scratch = u64::from(self.s_decode_ae_v_block_size);
                        coro = 2;
                    }
                }
                2 => {
                    // SUSPENSION_POINT(2): skip an application block that is not 11 bytes long.
                    if self.s_decode_ae_scratch > (io2 - iop) as u64 {
                        self.s_decode_ae_scratch -= (io2 - iop) as u64;
                        iop = io2;
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    iop += self.s_decode_ae_scratch as usize;
                    coro = 10;
                }
                3 => {
                    // The `while (v_block_size < 11)` condition.
                    if self.s_decode_ae_v_block_size < 11 {
                        coro = 30;
                    } else if self.s_decode_ae_v_is_animexts || self.s_decode_ae_v_is_netscape {
                        coro = 4;
                    } else {
                        coro = 10;
                    }
                }
                30 => {
                    // SUSPENSION_POINT(3): one byte of the identifier.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let v_c = src.data[iop];
                    iop += 1;
                    let i = usize::from(self.s_decode_ae_v_block_size);
                    self.s_decode_ae_v_is_animexts =
                        self.s_decode_ae_v_is_animexts && v_c == ANIMEXTS1DOT0[i];
                    self.s_decode_ae_v_is_netscape =
                        self.s_decode_ae_v_is_netscape && v_c == NETSCAPE2DOT0[i];
                    self.s_decode_ae_v_block_size += 1;
                    coro = 3;
                }
                4 => {
                    // SUSPENSION_POINT(4): the authentication code length.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    self.s_decode_ae_v_block_size = src.data[iop];
                    iop += 1;
                    if self.s_decode_ae_v_block_size == 3 {
                        coro = 6;
                    } else {
                        self.s_decode_ae_scratch = u64::from(self.s_decode_ae_v_block_size);
                        coro = 5;
                    }
                }
                5 => {
                    // SUSPENSION_POINT(5): skip the authentication code.
                    if self.s_decode_ae_scratch > (io2 - iop) as u64 {
                        self.s_decode_ae_scratch -= (io2 - iop) as u64;
                        iop = io2;
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    iop += self.s_decode_ae_scratch as usize;
                    coro = 10;
                }
                6 => {
                    // SUSPENSION_POINT(6): the sub-block id, which must be 1.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let v_c = src.data[iop];
                    iop += 1;
                    if v_c == 1 {
                        coro = 8;
                    } else {
                        self.s_decode_ae_scratch = 2;
                        coro = 7;
                    }
                }
                7 => {
                    // SUSPENSION_POINT(7): skip the two bytes of an unknown sub-block.
                    if self.s_decode_ae_scratch > (io2 - iop) as u64 {
                        self.s_decode_ae_scratch -= (io2 - iop) as u64;
                        iop = io2;
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    iop += self.s_decode_ae_scratch as usize;
                    coro = 10;
                }
                8 => {
                    // SUSPENSION_POINT(8): the loop count, u16le.
                    if io2 - iop >= 2 {
                        let t = u32::from(u16::from_le_bytes([src.data[iop], src.data[iop + 1]]));
                        iop += 2;
                        self.store_loop_count(t);
                        coro = 10;
                    } else {
                        self.s_decode_ae_scratch = 0;
                        coro = 9;
                    }
                }
                9 => match read_u16le_scratch(
                    &mut self.s_decode_ae_scratch,
                    &src.data,
                    &mut iop,
                    io2,
                ) {
                    Some(t) => {
                        self.store_loop_count(t);
                        coro = 10;
                    }
                    None => break 'co Status::new(strings::SUSPENSION_SHORT_READ),
                },
                10 => {
                    // label__goto_done__break: skip the rest of the extension.
                    src.meta.ri = iop;
                    coro = 11;
                }
                11 => {
                    // SUSPENSION_POINT(10): skip_blocks.
                    let st = self.skip_blocks(src);
                    iop = src.meta.ri;
                    if non_ok(st) {
                        break 'co st;
                    }
                    break 'co Status::OK;
                }
                _ => unreachable!("invalid decode_ae resumption point {coro}"),
            }
        };
        self.p_decode_ae = if status.is_suspension() { coro } else { 0 };
        src.meta.ri = iop;
        status
    }

    /// The loop-count part of `decode_ae`, shared by the fast and scratch reads.
    fn store_loop_count(&mut self, t: u32) {
        self.f_num_animation_loops_value = t;
        self.f_seen_num_animation_loops_value = true;
        if self.f_num_animation_loops_value > 0 && self.f_num_animation_loops_value <= 65535 {
            self.f_num_animation_loops_value += 1;
        }
    }

    /// Port of `wuffs_gif__decoder__decode_gc`: the graphic control extension.
    fn decode_gc(&mut self, src: &mut IoBuffer) -> Status {
        let mut iop = src.meta.ri;
        let io2 = src.meta.wi;
        let mut coro = self.p_decode_gc;
        let status = 'co: loop {
            match coro {
                0 => coro = 1,
                1 => {
                    // The block size, which must be 4.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let v_c = src.data[iop];
                    iop += 1;
                    if v_c != 4 {
                        src.meta.ri = iop;
                        return Status::new(strings::GIF_ERROR_BAD_GRAPHIC_CONTROL);
                    }
                    coro = 2;
                }
                2 => {
                    // The packed flags.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let mut v_flags = src.data[iop];
                    iop += 1;
                    self.f_gc_has_transparent_index = (v_flags & 1) != 0;
                    v_flags = (v_flags >> 2) & 7;
                    self.f_gc_disposal = if v_flags == 2 {
                        1
                    } else if v_flags == 3 || v_flags == 4 {
                        2
                    } else {
                        0
                    };
                    coro = 3;
                }
                3 => {
                    // The delay time in centiseconds, u16le.
                    if io2 - iop >= 2 {
                        let t = u16::from_le_bytes([src.data[iop], src.data[iop + 1]]);
                        iop += 2;
                        self.store_gc_duration(t);
                        coro = 5;
                    } else {
                        self.s_decode_gc_scratch = 0;
                        coro = 4;
                    }
                }
                4 => match read_u16le_scratch(
                    &mut self.s_decode_gc_scratch,
                    &src.data,
                    &mut iop,
                    io2,
                ) {
                    Some(t) => {
                        self.store_gc_duration(t as u16);
                        coro = 5;
                    }
                    None => break 'co Status::new(strings::SUSPENSION_SHORT_READ),
                },
                5 => {
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    self.f_gc_transparent_index = src.data[iop];
                    iop += 1;
                    coro = 6;
                }
                6 => {
                    // The block terminator, which must be 0.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let v_c = src.data[iop];
                    iop += 1;
                    if v_c != 0 {
                        src.meta.ri = iop;
                        return Status::new(strings::GIF_ERROR_BAD_GRAPHIC_CONTROL);
                    }
                    break 'co Status::OK;
                }
                _ => unreachable!("invalid decode_gc resumption point {coro}"),
            }
        };
        self.p_decode_gc = if status.is_suspension() { coro } else { 0 };
        src.meta.ri = iop;
        status
    }

    fn store_gc_duration(&mut self, centiseconds: u16) {
        self.f_gc_duration = u64::from(centiseconds) * 7_056_000;
    }

    /// Port of `wuffs_gif__decoder__decode_id_part0`: the image descriptor's rectangle.
    fn decode_id_part0(&mut self, src: &mut IoBuffer) -> Status {
        let mut iop = src.meta.ri;
        let io2 = src.meta.wi;
        let mut coro = self.p_decode_id_part0;
        let status = 'co: loop {
            match coro {
                0 => coro = 1,
                1 => {
                    // Left: u16le into f_frame_rect_x0.
                    if io2 - iop >= 2 {
                        self.f_frame_rect_x0 =
                            u32::from(u16::from_le_bytes([src.data[iop], src.data[iop + 1]]));
                        iop += 2;
                        coro = 3;
                    } else {
                        self.s_decode_id_part0_scratch = 0;
                        coro = 2;
                    }
                }
                2 => match read_u16le_scratch(
                    &mut self.s_decode_id_part0_scratch,
                    &src.data,
                    &mut iop,
                    io2,
                ) {
                    Some(t) => {
                        self.f_frame_rect_x0 = t;
                        coro = 3;
                    }
                    None => break 'co Status::new(strings::SUSPENSION_SHORT_READ),
                },
                3 => {
                    if io2 - iop >= 2 {
                        self.f_frame_rect_y0 =
                            u32::from(u16::from_le_bytes([src.data[iop], src.data[iop + 1]]));
                        iop += 2;
                        coro = 5;
                    } else {
                        self.s_decode_id_part0_scratch = 0;
                        coro = 4;
                    }
                }
                4 => match read_u16le_scratch(
                    &mut self.s_decode_id_part0_scratch,
                    &src.data,
                    &mut iop,
                    io2,
                ) {
                    Some(t) => {
                        self.f_frame_rect_y0 = t;
                        coro = 5;
                    }
                    None => break 'co Status::new(strings::SUSPENSION_SHORT_READ),
                },
                5 => {
                    // Width: stored as the right edge, x1 = x0 + width.
                    if io2 - iop >= 2 {
                        self.f_frame_rect_x1 =
                            u32::from(u16::from_le_bytes([src.data[iop], src.data[iop + 1]]));
                        iop += 2;
                        coro = 7;
                    } else {
                        self.s_decode_id_part0_scratch = 0;
                        coro = 6;
                    }
                }
                6 => match read_u16le_scratch(
                    &mut self.s_decode_id_part0_scratch,
                    &src.data,
                    &mut iop,
                    io2,
                ) {
                    Some(t) => {
                        self.f_frame_rect_x1 = t;
                        coro = 7;
                    }
                    None => break 'co Status::new(strings::SUSPENSION_SHORT_READ),
                },
                7 => {
                    self.f_frame_rect_x1 = self.f_frame_rect_x1.wrapping_add(self.f_frame_rect_x0);
                    if io2 - iop >= 2 {
                        self.f_frame_rect_y1 =
                            u32::from(u16::from_le_bytes([src.data[iop], src.data[iop + 1]]));
                        iop += 2;
                        coro = 9;
                    } else {
                        self.s_decode_id_part0_scratch = 0;
                        coro = 8;
                    }
                }
                8 => match read_u16le_scratch(
                    &mut self.s_decode_id_part0_scratch,
                    &src.data,
                    &mut iop,
                    io2,
                ) {
                    Some(t) => {
                        self.f_frame_rect_y1 = t;
                        coro = 9;
                    }
                    None => break 'co Status::new(strings::SUSPENSION_SHORT_READ),
                },
                9 => {
                    self.f_frame_rect_y1 = self.f_frame_rect_y1.wrapping_add(self.f_frame_rect_y0);
                    self.f_dst_x = self.f_frame_rect_x0;
                    self.f_dst_y = self.f_frame_rect_y0;
                    if self.f_num_decoded_frame_configs_value == 0 && !self.f_quirks[4] {
                        self.f_width = self.f_width.max(self.f_frame_rect_x1);
                        self.f_height = self.f_height.max(self.f_frame_rect_y1);
                    }
                    break 'co Status::OK;
                }
                _ => unreachable!("invalid decode_id_part0 resumption point {coro}"),
            }
        };
        self.p_decode_id_part0 = if status.is_suspension() { coro } else { 0 };
        src.meta.ri = iop;
        status
    }

    /// Port of `wuffs_gif__decoder__decode_id_part1`: the local palette or the transparent
    /// colour, the interlace flag and the LZW minimum code size; then the swizzler setup.
    fn decode_id_part1(
        &mut self,
        dst: &mut PixelBuffer<'_>,
        src: &mut IoBuffer,
        blend: PixelBlend,
    ) -> Status {
        let mut iop = src.meta.ri;
        let io2 = src.meta.wi;
        let mut coro = self.p_decode_id_part1;
        let status = 'co: loop {
            match coro {
                0 => coro = 1,
                1 => {
                    // The image flags.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let v_flags = src.data[iop];
                    iop += 1;
                    self.f_interlace = if (v_flags & 64) != 0 { 4 } else { 0 };
                    self.s_decode_id_part1_v_which_palette = 1;
                    if (v_flags & 128) != 0 {
                        self.s_decode_id_part1_v_num_palette_entries = 1u32 << (1 + (v_flags & 7));
                        self.s_decode_id_part1_v_i = 0;
                        coro = 2;
                    } else if self.f_quirks[6] && !self.f_has_global_palette {
                        src.meta.ri = iop;
                        return Status::new(strings::GIF_ERROR_BAD_PALETTE);
                    } else if self.f_gc_has_transparent_index {
                        let (a, b) = self.f_palettes.split_at_mut(1);
                        b[0].copy_from_slice(&a[0]);
                        coro = 7;
                    } else {
                        self.s_decode_id_part1_v_which_palette = 0;
                        coro = 7;
                    }
                }
                2 => {
                    // The local palette loop condition.
                    if self.s_decode_id_part1_v_i < self.s_decode_id_part1_v_num_palette_entries {
                        coro = 3;
                    } else {
                        coro = 6;
                    }
                }
                3 => {
                    // One local palette entry: u24be.
                    if io2 - iop >= 3 {
                        let t = (u32::from(src.data[iop]) << 16)
                            | (u32::from(src.data[iop + 1]) << 8)
                            | u32::from(src.data[iop + 2]);
                        iop += 3;
                        self.store_local_palette_entry(t);
                        coro = 2;
                    } else {
                        self.s_decode_id_part1_scratch = 0;
                        coro = 4;
                    }
                }
                4 => match read_u24be_scratch(
                    &mut self.s_decode_id_part1_scratch,
                    &src.data,
                    &mut iop,
                    io2,
                ) {
                    Some(t) => {
                        self.store_local_palette_entry(t);
                        coro = 2;
                    }
                    None => break 'co Status::new(strings::SUSPENSION_SHORT_READ),
                },
                6 => {
                    // Fill the rest of the local palette with opaque black.
                    let mut v_i = self.s_decode_id_part1_v_i;
                    while v_i < 256 {
                        let i = 4 * v_i as usize;
                        self.f_palettes[1][i] = 0;
                        self.f_palettes[1][i + 1] = 0;
                        self.f_palettes[1][i + 2] = 0;
                        self.f_palettes[1][i + 3] = 255;
                        v_i += 1;
                    }
                    self.s_decode_id_part1_v_i = v_i;
                    coro = 7;
                }
                7 => {
                    if self.f_gc_has_transparent_index {
                        let t = 4 * usize::from(self.f_gc_transparent_index);
                        self.f_palettes[1][t] = 0;
                        self.f_palettes[1][t + 1] = 0;
                        self.f_palettes[1][t + 2] = 0;
                        self.f_palettes[1][t + 3] = 0;
                    }
                    let which = usize::from(self.s_decode_id_part1_v_which_palette);
                    let v_status = base::swizzler_prepare(
                        &mut self.f_swizzler,
                        dst.pixel_format(),
                        &mut self.f_dst_palette[..],
                        PixelFormat(SOURCE_PIXFMT),
                        &self.f_palettes[which][..],
                        blend,
                    );
                    if !v_status.is_ok() {
                        if v_status.is_error() {
                            src.meta.ri = iop;
                            return v_status;
                        } else if v_status.is_suspension() {
                            src.meta.ri = iop;
                            return Status::new(strings::ERROR_CANNOT_RETURN_A_SUSPENSION);
                        }
                        break 'co v_status;
                    }
                    if self.f_previous_lzw_decode_ended_abruptly {
                        self.f_lzw.initialize_leave_internal_buffers();
                    }
                    coro = 8;
                }
                8 => {
                    // The LZW literal width.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    let v_lw = src.data[iop];
                    iop += 1;
                    if v_lw > 8 {
                        src.meta.ri = iop;
                        return Status::new(strings::GIF_ERROR_BAD_LITERAL_WIDTH);
                    }
                    self.f_lzw.set_literal_width(u32::from(v_lw));
                    self.f_previous_lzw_decode_ended_abruptly = true;
                    break 'co Status::OK;
                }
                _ => unreachable!("invalid decode_id_part1 resumption point {coro}"),
            }
        };
        self.p_decode_id_part1 = if status.is_suspension() { coro } else { 0 };
        src.meta.ri = iop;
        status
    }

    /// One local palette entry in `decode_id_part1`, shared by the fast and scratch reads.
    fn store_local_palette_entry(&mut self, t: u32) {
        let v_argb = t | 0xFF00_0000;
        let i = self.s_decode_id_part1_v_i as usize;
        let p = &mut self.f_palettes[1];
        p[4 * i] = (v_argb & 255) as u8;
        p[4 * i + 1] = ((v_argb >> 8) & 255) as u8;
        p[4 * i + 2] = ((v_argb >> 16) & 255) as u8;
        p[4 * i + 3] = ((v_argb >> 24) & 255) as u8;
        self.s_decode_id_part1_v_i += 1;
    }

    /// Port of `wuffs_gif__decoder__decode_id_part2`: the LZW-compressed image data, in
    /// sub-blocks, decoded row by row into `dst`.
    fn decode_id_part2(&mut self, dst: &mut PixelBuffer<'_>, src: &mut IoBuffer) -> Status {
        let mut iop = src.meta.ri;
        let io2 = src.meta.wi;
        let mut coro = self.p_decode_id_part2;
        if coro == 0 {
            self.s_decode_id_part2_v_block_size = 0;
            self.s_decode_id_part2_v_need_block_size = true;
        }
        let status = 'co: loop {
            match coro {
                0 => {
                    self.s_decode_id_part2_v_need_block_size = true;
                    coro = 10;
                }
                10 => {
                    // label__outer__continue
                    if self.s_decode_id_part2_v_need_block_size {
                        self.s_decode_id_part2_v_need_block_size = false;
                        coro = 1;
                    } else {
                        coro = 11;
                    }
                }
                1 => {
                    // SUSPENSION_POINT(1): the sub-block size.
                    if iop == io2 {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    self.s_decode_id_part2_v_block_size = u64::from(src.data[iop]);
                    iop += 1;
                    coro = 11;
                }
                11 => {
                    if self.s_decode_id_part2_v_block_size == 0 {
                        coro = 20;
                    } else {
                        coro = 2;
                    }
                }
                2 => {
                    // SUSPENSION_POINT_MAYBE_SUSPEND(2): wait for input.
                    if io2 == iop {
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    coro = 12;
                }
                12 => {
                    if self.f_compressed_ri == self.f_compressed_wi {
                        self.f_compressed_ri = 0;
                        self.f_compressed_wi = 0;
                    }
                    coro = 13;
                }
                13 => {
                    // The `while (compressed_wi <= 3841)` loop.
                    if self.f_compressed_wi > 3841 {
                        coro = 14;
                        continue;
                    }
                    let v_n_compressed =
                        self.s_decode_id_part2_v_block_size.min((io2 - iop) as u64);
                    if v_n_compressed == 0 {
                        coro = 14;
                        continue;
                    }
                    let wi = self.f_compressed_wi as usize;
                    let n = v_n_compressed.min(4_294_967_295) as u32;
                    let v_n_copied = limited_copy_u32_to_slice(
                        &src.data,
                        &mut iop,
                        io2,
                        n,
                        &mut self.f_compressed[wi..],
                    );
                    self.f_compressed_wi =
                        self.f_compressed_wi.saturating_add(u64::from(v_n_copied));
                    self.s_decode_id_part2_v_block_size = self
                        .s_decode_id_part2_v_block_size
                        .saturating_sub(u64::from(v_n_copied));
                    if self.s_decode_id_part2_v_block_size > 0 {
                        coro = 14;
                        continue;
                    }
                    if io2 <= iop {
                        self.s_decode_id_part2_v_need_block_size = true;
                        coro = 14;
                        continue;
                    }
                    self.s_decode_id_part2_v_block_size = u64::from(src.data[iop]);
                    iop += 1;
                    // Stay in the loop (`while` condition at arm 13).
                }
                14 => {
                    // label__0__break and label__inner__continue: the LZW step.
                    if self.f_compressed_ri > self.f_compressed_wi || self.f_compressed_wi > 4096 {
                        break 'co Status::new(strings::GIF_ERROR_INTERNAL_INCONSISTENT_RI_WI);
                    }
                    let ri = self.f_compressed_ri as usize;
                    let wi = self.f_compressed_wi as usize;
                    let mut u_r = IoBuffer {
                        data: Vec::new(),
                        meta: IoMeta::default(),
                    };
                    u_r.meta.wi = wi - ri;
                    u_r.meta.ri = 0;
                    u_r.meta.closed = false;
                    let mut empty_dst_meta = IoMeta::default();
                    let v_lzw_status = self.f_lzw.transform_io(
                        &mut [],
                        &mut empty_dst_meta,
                        &self.f_compressed[ri..wi],
                        &mut u_r.meta,
                    );
                    let consumed = u_r.meta.ri as u64;
                    self.f_compressed_ri = self.f_compressed_ri.saturating_add(consumed);
                    let v_uncompressed = self.f_lzw.flush();
                    if !v_uncompressed.is_empty() {
                        let v_copy_status = self.copy_to_image_buffer(dst, &v_uncompressed);
                        if v_copy_status.is_error() {
                            break 'co v_copy_status;
                        }
                    }
                    if v_lzw_status.is_ok() {
                        self.f_previous_lzw_decode_ended_abruptly = false;
                        if self.s_decode_id_part2_v_need_block_size
                            || self.s_decode_id_part2_v_block_size > 0
                        {
                            coro = 3;
                        } else {
                            coro = 20;
                        }
                        continue;
                    } else if v_lzw_status.repr() == Some(strings::SUSPENSION_SHORT_READ) {
                        coro = 10;
                        continue;
                    } else if v_lzw_status.repr() == Some(strings::SUSPENSION_SHORT_WRITE) {
                        coro = 14;
                        continue;
                    } else if self.f_quirks[3]
                        && self.f_dst_y >= self.f_frame_rect_y1
                        && self.f_interlace == 0
                    {
                        if self.s_decode_id_part2_v_need_block_size
                            || self.s_decode_id_part2_v_block_size > 0
                        {
                            coro = 5;
                        } else {
                            coro = 20;
                        }
                        continue;
                    }
                    if v_lzw_status.is_error() {
                        break 'co v_lzw_status;
                    } else if v_lzw_status.is_suspension() {
                        break 'co Status::new(strings::ERROR_CANNOT_RETURN_A_SUSPENSION);
                    }
                    break 'co v_lzw_status;
                }
                3 => {
                    // SUSPENSION_POINT(3): skip the remainder of the block.
                    let v = self.s_decode_id_part2_v_block_size;
                    if v > (io2 - iop) as u64 {
                        self.s_decode_id_part2_v_block_size = v - (io2 - iop) as u64;
                        iop = io2;
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    iop += v as usize;
                    coro = 4;
                }
                4 => {
                    // SUSPENSION_POINT(4): skip_blocks for the rest of the sub-blocks.
                    src.meta.ri = iop;
                    let st = self.skip_blocks(src);
                    iop = src.meta.ri;
                    if non_ok(st) {
                        break 'co st;
                    }
                    coro = 20;
                }
                5 => {
                    // SUSPENSION_POINT(5): the quirk path, same as 3.
                    let v = self.s_decode_id_part2_v_block_size;
                    if v > (io2 - iop) as u64 {
                        self.s_decode_id_part2_v_block_size = v - (io2 - iop) as u64;
                        iop = io2;
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    iop += v as usize;
                    coro = 6;
                }
                6 => {
                    // SUSPENSION_POINT(6): the quirk path, same as 4.
                    src.meta.ri = iop;
                    let st = self.skip_blocks(src);
                    iop = src.meta.ri;
                    if non_ok(st) {
                        break 'co st;
                    }
                    coro = 20;
                }
                20 => {
                    // label__outer__break
                    self.f_compressed_ri = 0;
                    self.f_compressed_wi = 0;
                    if self.f_dst_y < self.f_frame_rect_y1
                        && self.f_frame_rect_x0 != self.f_frame_rect_x1
                        && self.f_frame_rect_y0 != self.f_frame_rect_y1
                    {
                        break 'co Status::new(strings::ERROR_NOT_ENOUGH_DATA);
                    }
                    break 'co Status::OK;
                }
                _ => unreachable!("invalid decode_id_part2 resumption point {coro}"),
            }
        };
        self.p_decode_id_part2 = if status.is_suspension() { coro } else { 0 };
        src.meta.ri = iop;
        status
    }

    /// Port of `wuffs_gif__decoder__copy_to_image_buffer`: places decoded indices into `pb`,
    /// row by row, handling the frame rectangle, interlacing and the swizzler.
    fn copy_to_image_buffer(&mut self, pb: &mut PixelBuffer<'_>, a_src: &[u8]) -> Status {
        let pixfmt = pb.pixel_format();
        let bits_per_pixel = pixfmt.bits_per_pixel();
        if (bits_per_pixel & 7) != 0 {
            return Status::new(strings::ERROR_UNSUPPORTED_OPTION);
        }
        let bytes_per_pixel = u64::from(bits_per_pixel >> 3);
        let width_in_bytes = u64::from(self.f_width) * bytes_per_pixel;
        let (tab_width, tab_height, tab_stride) = pb.plane0_dims().unwrap_or((0, 0, 0));

        let src_len = a_src.len();
        let mut src_ri: usize = 0;
        'cont: while src_ri < src_len {
            let v_src = &a_src[src_ri..];
            if self.f_dst_y >= self.f_frame_rect_y1 {
                if self.f_quirks[3] {
                    return Status::OK;
                }
                return Status::new(strings::ERROR_TOO_MUCH_DATA);
            }
            // The row as (start, len) within the plane. An out-of-range row is empty.
            let (row_start, row_len) = row_range(tab_width, tab_height, tab_stride, self.f_dst_y);
            let mut dst_len = row_len;
            if self.f_dst_y >= self.f_height {
                dst_len = 0;
            } else if width_in_bytes < dst_len as u64 {
                dst_len = width_in_bytes as usize;
            }
            let v_i = u64::from(self.f_dst_x) * bytes_per_pixel;
            if v_i < dst_len as u64 {
                let v_j = u64::from(self.f_frame_rect_x1) * bytes_per_pixel;
                let (lo, hi) = if v_i <= v_j && v_j <= dst_len as u64 {
                    (v_i as usize, v_j as usize)
                } else {
                    (v_i as usize, dst_len)
                };
                let dst_start = row_start + lo;
                let dst_end = row_start + hi;
                let plane = pb
                    .plane0_mut()
                    .expect("a row with a non-zero length implies a plane");
                let n = base::swizzle_interleaved_from_slice(
                    &self.f_swizzler,
                    &mut plane.data[dst_start..dst_end],
                    &self.f_dst_palette[..],
                    v_src,
                );
                src_ri += n as usize;
                self.f_dst_x = self.f_dst_x.saturating_add(n as u32);
                self.f_dirty_max_excl_y =
                    self.f_dirty_max_excl_y.max(self.f_dst_y.saturating_add(1));
            }
            if self.f_frame_rect_x1 <= self.f_dst_x {
                self.f_dst_x = self.f_frame_rect_x0;
                if self.f_interlace == 0 {
                    self.f_dst_y = self.f_dst_y.saturating_add(1);
                    continue 'cont;
                }
                if self.f_num_decoded_frames_value == 0
                    && !self.f_gc_has_transparent_index
                    && self.f_interlace > 1
                {
                    // Replicate the row above into the rows the pass skips.
                    let src_row = row_range(tab_width, tab_height, tab_stride, self.f_dst_y);
                    let mut y0 = self.f_dst_y.saturating_add(1);
                    let mut y1 = self
                        .f_dst_y
                        .saturating_add(u32::from(INTERLACE_COUNT[self.f_interlace as usize]));
                    y1 = y1.min(self.f_frame_rect_y1);
                    let plane = pb.plane0_mut().expect("a replicated row implies a plane");
                    while y0 < y1 {
                        let dst_row = row_range(tab_width, tab_height, tab_stride, y0);
                        let n = dst_row.1.min(src_row.1);
                        if n > 0 {
                            plane.data.copy_within(src_row.0..src_row.0 + n, dst_row.0);
                        }
                        y0 += 1;
                    }
                    self.f_dirty_max_excl_y = self.f_dirty_max_excl_y.max(y1);
                }
                self.f_dst_y = self
                    .f_dst_y
                    .saturating_add(u32::from(INTERLACE_DELTA[self.f_interlace as usize]));
                while self.f_interlace > 0 && self.f_dst_y >= self.f_frame_rect_y1 {
                    self.f_interlace -= 1;
                    self.f_dst_y = self
                        .f_frame_rect_y0
                        .saturating_add(INTERLACE_START[self.f_interlace as usize]);
                }
                continue 'cont;
            }
            if src_len == src_ri {
                break 'cont;
            } else if src_len < src_ri {
                return Status::new(strings::GIF_ERROR_INTERNAL_INCONSISTENT_RI_WI);
            }
            let mut v_n = u64::from(self.f_frame_rect_x1 - self.f_dst_x);
            v_n = v_n.min((src_len - src_ri) as u64);
            src_ri += v_n as usize;
            self.f_dst_x = self.f_dst_x.saturating_add(v_n as u32);
            if self.f_frame_rect_x1 <= self.f_dst_x {
                self.f_dst_x = self.f_frame_rect_x0;
                self.f_dst_y = self
                    .f_dst_y
                    .saturating_add(u32::from(INTERLACE_DELTA[self.f_interlace as usize]));
                while self.f_interlace > 0 && self.f_dst_y >= self.f_frame_rect_y1 {
                    self.f_interlace -= 1;
                    self.f_dst_y = self
                        .f_frame_rect_y0
                        .saturating_add(INTERLACE_START[self.f_interlace as usize]);
                }
                continue 'cont;
            }
            if src_ri != src_len {
                return Status::new(strings::GIF_ERROR_INTERNAL_INCONSISTENT_RI_WI);
            }
            break 'cont;
        }
        Status::OK
    }
}

/// `wuffs_base__table_u8__row_u32` on a plane described by `(width, height, stride)`: the start
/// and length of row `y`, with an empty row when `y` is out of range.
fn row_range(width: usize, height: usize, stride: usize, y: u32) -> (usize, usize) {
    if (y as usize) < height {
        (stride * (y as usize), width)
    } else {
        (0, 0)
    }
}

/// `wuffs_base__io_reader__limited_copy_u32_to_slice` on a buffer whose read index is `iop` and
/// whose end is `io2`: copies at most `length` bytes into `dst` and returns the count.
fn limited_copy_u32_to_slice(
    data: &[u8],
    iop: &mut usize,
    io2: usize,
    length: u32,
    dst: &mut [u8],
) -> u32 {
    let mut n = dst.len();
    if n > length as usize {
        n = length as usize;
    }
    if n > io2 - *iop {
        n = io2 - *iop;
    }
    if n > 0 {
        dst[..n].copy_from_slice(&data[*iop..*iop + n]);
        *iop += n;
    }
    n as u32
}

/// `wuffs_gif__decoder__do_decode_frame_config`'s `frame_config.set(...)` disposal argument.
fn disposal_from_gc(v: u8) -> AnimationDisposal {
    match v {
        1 => AnimationDisposal::RestoreBackground,
        2 => AnimationDisposal::RestorePrevious,
        _ => AnimationDisposal::None,
    }
}
