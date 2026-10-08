// Port of: wuffs-v0.3.c, the "lzw" module (`wuffs_lzw__decoder__*`, lines 31143-31675 of the
// release C file at google/wuffs-mirror-release-c@e3f919cc), as driven by the GIF decoder.
//
// The decoder is a coroutine (`transform_io`, `write_to`). Each Wuffs coroutine keeps its
// resumption point in `p_*` and its locals in fields, and the Rust function is a `loop { match }`
// over the same resumption points, so a short read or short write resumes exactly where the C
// code does.
//
// Omitted: `workbuf_len` and `set_quirk_enabled` (no-ops in the C code, which Skia never needs to
// distinguish), the `io_transformer` vtable, and the `alloc` functions.

// The ported arithmetic is Wuffs' C integer arithmetic (uint8, uint16, uint32, uint64 and size_t
// conversions, with the truncating and sign-changing casts the C code relies on). Each cast mirrors
// one C cast, so the cast lints are allowed for the module.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless
)]
// `read_from` and `transform_io` are long because they are the C functions, one step per branch
// of the C code, and the lint is allowed for the module for the same reason as the casts.
#![allow(clippy::too_many_lines)]

/// A boxed array of `N` copies of `value`. `Box::new([value; N])` would build the array on the
/// stack first, which is 32 KiB for the suffix table.
fn boxed_array<T: Copy, const N: usize>(value: T) -> Box<[T; N]> {
    vec![value; N]
        .into_boxed_slice()
        .try_into()
        .unwrap_or_else(|_| unreachable!("the vector has N elements"))
}

use crate::base::{IoMeta, Status};
use crate::strings;

/// Port of the `private_impl.magic` field: a freshly zeroed decoder, a usable one, or one that an
/// error disabled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Magic {
    /// `magic == 0`: `initialize` has not been called.
    #[default]
    Uninitialized,
    /// `WUFFS_BASE__MAGIC`
    Valid,
    /// `WUFFS_BASE__DISABLED`
    Disabled,
}

/// Port of `wuffs_lzw__decoder`'s `private_impl` fields, the ones `initialize` resets with
/// `LEAVE_INTERNAL_BUFFERS_UNINITIALIZED`.
#[derive(Debug)]
pub(crate) struct LzwImpl {
    pub(crate) magic: Magic,
    pub(crate) active_coroutine: u32,
    pub(crate) f_set_literal_width_arg: u32,
    pub(crate) f_literal_width: u32,
    pub(crate) f_clear_code: u32,
    pub(crate) f_end_code: u32,
    pub(crate) f_save_code: u32,
    pub(crate) f_prev_code: u32,
    pub(crate) f_width: u32,
    pub(crate) f_bits: u32,
    pub(crate) f_n_bits: u32,
    pub(crate) f_output_ri: u32,
    pub(crate) f_output_wi: u32,
    pub(crate) f_read_from_return_value: u32,
    pub(crate) f_prefixes: Box<[u16; 4096]>,
    pub(crate) p_transform_io: u32,
    pub(crate) p_write_to: u32,
}

impl Default for LzwImpl {
    fn default() -> Self {
        LzwImpl {
            magic: Magic::Uninitialized,
            active_coroutine: 0,
            f_set_literal_width_arg: 0,
            f_literal_width: 0,
            f_clear_code: 0,
            f_end_code: 0,
            f_save_code: 0,
            f_prev_code: 0,
            f_width: 0,
            f_bits: 0,
            f_n_bits: 0,
            f_output_ri: 0,
            f_output_wi: 0,
            f_read_from_return_value: 0,
            f_prefixes: Box::new([0; 4096]),
            p_transform_io: 0,
            p_write_to: 0,
        }
    }
}

/// Port of `wuffs_lzw__decoder`. The `private_data` arrays are kept across
/// `initialize_leave_internal_buffers_uninitialized`, as in the C code.
#[derive(Debug)]
pub struct LzwDecoder {
    pub(crate) imp: LzwImpl,
    /// Port of `private_data.f_suffixes`.
    pub(crate) f_suffixes: Box<[[u8; 8]; 4096]>,
    /// Port of `private_data.f_lm1s`.
    pub(crate) f_lm1s: Box<[u16; 4096]>,
    /// Port of `private_data.f_output`. It is 8199 bytes, not 8192, because the copy in
    /// `read_from` writes eight bytes at a time.
    pub(crate) f_output: Box<[u8; 8199]>,
}

impl Default for LzwDecoder {
    fn default() -> Self {
        LzwDecoder {
            imp: LzwImpl::default(),
            f_suffixes: boxed_array([0; 8]),
            f_lm1s: Box::new([0; 4096]),
            f_output: Box::new([0; 8199]),
        }
    }
}

// Coroutine resumption points of `transform_io`.
const TRANSFORM_IO_START: u32 = 0;
const TRANSFORM_IO_AFTER_WRITE: u32 = 1;
const TRANSFORM_IO_AFTER_SHORT_READ: u32 = 2;
const TRANSFORM_IO_LOOP: u32 = 3;
const TRANSFORM_IO_DISPATCH: u32 = 4;

impl LzwDecoder {
    /// Port of `wuffs_lzw__decoder__initialize` with `WUFFS_INITIALIZE__DEFAULT_OPTIONS`: the
    /// whole struct is zeroed, then marked valid.
    #[must_use]
    pub fn new() -> Self {
        let mut d = LzwDecoder::default();
        d.imp.magic = Magic::Valid;
        d
    }

    /// Port of `wuffs_lzw__decoder__initialize` with `WUFFS_INITIALIZE__LEAVE_INTERNAL_BUFFERS_UNINITIALIZED`:
    /// the `private_impl` fields are reset, and `private_data` is left alone.
    pub(crate) fn initialize_leave_internal_buffers(&mut self) {
        self.imp = LzwImpl {
            magic: Magic::Valid,
            ..LzwImpl::default()
        };
    }

    /// Port of `wuffs_lzw__decoder__set_literal_width`.
    pub(crate) fn set_literal_width(&mut self, a_lw: u32) {
        if self.imp.magic != Magic::Valid {
            return;
        }
        if a_lw > 8 {
            self.imp.magic = Magic::Disabled;
            return;
        }
        self.imp.f_set_literal_width_arg = a_lw + 1;
    }

    /// Port of `wuffs_lzw__decoder__flush`: returns the decoded bytes that are ready and resets
    /// the output buffer. Returns nothing when the decoder is not usable.
    pub(crate) fn flush(&mut self) -> Vec<u8> {
        if self.imp.magic != Magic::Valid && self.imp.magic != Magic::Disabled {
            return Vec::new();
        }
        let mut v = Vec::new();
        if self.imp.f_output_ri <= self.imp.f_output_wi {
            let ri = self.imp.f_output_ri as usize;
            let wi = self.imp.f_output_wi as usize;
            v.extend_from_slice(&self.f_output[ri..wi]);
        }
        self.imp.f_output_ri = 0;
        self.imp.f_output_wi = 0;
        v
    }

    /// Port of `wuffs_lzw__decoder__transform_io` on raw buffers: `src` is `src_data[..src_wi]`
    /// with read index `src_meta.ri`, and `dst` is `dst_data` with write index `dst_meta.wi`.
    /// The public `io_buffer` wrapper is not ported, because the GIF decoder is the only caller
    /// and it passes buffers it owns.
    pub(crate) fn transform_io(
        &mut self,
        dst_data: &mut [u8],
        dst_meta: &mut IoMeta,
        src_data: &[u8],
        src_meta: &mut IoMeta,
    ) -> Status {
        if self.imp.magic != Magic::Valid {
            return Status::new(if self.imp.magic == Magic::Disabled {
                strings::ERROR_DISABLED_BY_PREVIOUS_ERROR
            } else {
                strings::ERROR_INITIALIZE_NOT_CALLED
            });
        }
        if self.imp.active_coroutine != 0 && self.imp.active_coroutine != 1 {
            self.imp.magic = Magic::Disabled;
            return Status::new(strings::ERROR_INTERLEAVED_COROUTINE_CALLS);
        }
        self.imp.active_coroutine = 0;

        let mut coro = self.imp.p_transform_io;
        let status: Status = 'co: loop {
            match coro {
                TRANSFORM_IO_START => {
                    self.imp.f_literal_width = 8;
                    if self.imp.f_set_literal_width_arg > 0 {
                        self.imp.f_literal_width = self.imp.f_set_literal_width_arg - 1;
                    }
                    self.imp.f_clear_code = 1u32 << self.imp.f_literal_width;
                    self.imp.f_end_code = self.imp.f_clear_code + 1;
                    self.imp.f_save_code = self.imp.f_end_code;
                    self.imp.f_prev_code = self.imp.f_end_code;
                    self.imp.f_width = self.imp.f_literal_width + 1;
                    self.imp.f_bits = 0;
                    self.imp.f_n_bits = 0;
                    self.imp.f_output_ri = 0;
                    self.imp.f_output_wi = 0;
                    let mut v_i: u32 = 0;
                    while v_i < self.imp.f_clear_code {
                        self.f_lm1s[v_i as usize] = 0;
                        self.f_suffixes[v_i as usize][0] = v_i as u8;
                        v_i += 1;
                    }
                    coro = TRANSFORM_IO_LOOP;
                }
                TRANSFORM_IO_LOOP => {
                    // label__0__continue
                    self.read_from(src_data, src_meta);
                    if self.imp.f_output_wi > 0 {
                        coro = TRANSFORM_IO_AFTER_WRITE;
                    } else {
                        coro = TRANSFORM_IO_DISPATCH;
                    }
                }
                TRANSFORM_IO_AFTER_WRITE => {
                    let status = self.write_to(dst_data, dst_meta);
                    if status.repr().is_some() {
                        break 'co status;
                    }
                    coro = TRANSFORM_IO_DISPATCH;
                }
                TRANSFORM_IO_AFTER_SHORT_READ => {
                    // The end of the loop body after the short-read suspension.
                    coro = TRANSFORM_IO_LOOP;
                }
                TRANSFORM_IO_DISPATCH => match self.imp.f_read_from_return_value {
                    0 => break 'co Status::OK,
                    1 => coro = TRANSFORM_IO_LOOP,
                    2 => {
                        coro = TRANSFORM_IO_AFTER_SHORT_READ;
                        break 'co Status::new(strings::SUSPENSION_SHORT_READ);
                    }
                    3 => {
                        self.imp.magic = Magic::Disabled;
                        return Status::new(strings::LZW_ERROR_TRUNCATED_INPUT);
                    }
                    4 => {
                        self.imp.magic = Magic::Disabled;
                        return Status::new(strings::LZW_ERROR_BAD_CODE);
                    }
                    _ => {
                        self.imp.magic = Magic::Disabled;
                        return Status::new(strings::LZW_ERROR_INTERNAL_INCONSISTENT_I_O);
                    }
                },
                _ => unreachable!("invalid transform_io resumption point {coro}"),
            }
        };

        // `ok:` and `suspend:` of the C code.
        if status.is_suspension() {
            self.imp.p_transform_io = coro;
            self.imp.active_coroutine = 1;
        } else {
            self.imp.p_transform_io = 0;
            self.imp.active_coroutine = 0;
        }
        if status.is_error() {
            self.imp.magic = Magic::Disabled;
        }
        status
    }

    /// Port of `wuffs_lzw__decoder__read_from`: decodes as many codes as fit in the output
    /// buffer, reading bits from `src_data[src_meta.ri..src_meta.wi]`. Sets
    /// `f_read_from_return_value` to say why it stopped.
    fn read_from(&mut self, src_data: &[u8], src_meta: &mut IoMeta) {
        let io1 = src_meta.ri;
        let mut iop = io1;
        let io2 = src_meta.wi;
        let closed = src_meta.closed;

        let v_clear_code = self.imp.f_clear_code;
        let v_end_code = self.imp.f_end_code;
        let mut v_save_code = self.imp.f_save_code;
        let mut v_prev_code = self.imp.f_prev_code;
        let mut v_width = self.imp.f_width;
        let mut v_bits = self.imp.f_bits;
        let mut v_n_bits = self.imp.f_n_bits;
        let mut v_output_wi = self.imp.f_output_wi;

        'main: loop {
            if v_n_bits < v_width {
                if io2 - iop >= 4 {
                    let x = u32::from_le_bytes([
                        src_data[iop],
                        src_data[iop + 1],
                        src_data[iop + 2],
                        src_data[iop + 3],
                    ]);
                    v_bits |= x << v_n_bits;
                    iop += ((31 - v_n_bits) >> 3) as usize;
                    v_n_bits |= 0x18;
                } else if io2 <= iop {
                    self.imp.f_read_from_return_value = if closed { 3 } else { 2 };
                    break 'main;
                } else {
                    v_bits |= u32::from(src_data[iop]) << v_n_bits;
                    iop += 1;
                    v_n_bits += 8;
                    if v_n_bits >= v_width {
                        // Nothing more to read.
                    } else if io2 <= iop {
                        self.imp.f_read_from_return_value = if closed { 3 } else { 2 };
                        break 'main;
                    } else {
                        v_bits |= u32::from(src_data[iop]) << v_n_bits;
                        iop += 1;
                        v_n_bits += 8;
                        if v_n_bits < v_width {
                            self.imp.f_read_from_return_value = 5;
                            break 'main;
                        }
                    }
                }
            }
            let v_code = v_bits & ((1u32 << v_width) - 1);
            v_bits >>= v_width;
            v_n_bits -= v_width;

            if v_code < v_clear_code {
                self.f_output[v_output_wi as usize] = v_code as u8;
                v_output_wi = (v_output_wi + 1) & 8191;
                if v_save_code <= 4095 {
                    let v_lm1_a = (self.f_lm1s[v_prev_code as usize].wrapping_add(1)) & 4095;
                    self.f_lm1s[v_save_code as usize] = v_lm1_a;
                    if v_lm1_a.is_multiple_of(8) {
                        self.imp.f_prefixes[v_save_code as usize] = v_prev_code as u16;
                        self.f_suffixes[v_save_code as usize][0] = v_code as u8;
                    } else {
                        self.imp.f_prefixes[v_save_code as usize] =
                            self.imp.f_prefixes[v_prev_code as usize];
                        self.f_suffixes[v_save_code as usize] =
                            self.f_suffixes[v_prev_code as usize];
                        self.f_suffixes[v_save_code as usize][(v_lm1_a % 8) as usize] =
                            v_code as u8;
                    }
                    v_save_code += 1;
                    if v_width < 12 {
                        v_width += 1 & (v_save_code >> v_width);
                    }
                    v_prev_code = v_code;
                }
            } else if v_code <= v_end_code {
                if v_code == v_end_code {
                    self.imp.f_read_from_return_value = 0;
                    break 'main;
                }
                v_save_code = v_end_code;
                v_prev_code = v_end_code;
                v_width = self.imp.f_literal_width + 1;
            } else if v_code <= v_save_code {
                let mut v_c = v_code;
                if v_code == v_save_code {
                    v_c = v_prev_code;
                }
                let lm1 = u32::from(self.f_lm1s[v_c as usize]);
                let mut v_o = (v_output_wi + (lm1 & 0xFFFF_FFF8)) & 8191;
                v_output_wi = (v_output_wi + 1 + lm1) & 8191;
                let mut v_steps = lm1 >> 3;
                loop {
                    let o = v_o as usize;
                    self.f_output[o..o + 8].copy_from_slice(&self.f_suffixes[v_c as usize]);
                    if v_steps == 0 {
                        break;
                    }
                    v_steps -= 1;
                    v_o = v_o.wrapping_sub(8) & 8191;
                    v_c = u32::from(self.imp.f_prefixes[v_c as usize]);
                }
                let v_first_byte = self.f_suffixes[v_c as usize][0];
                if v_code == v_save_code {
                    self.f_output[v_output_wi as usize] = v_first_byte;
                    v_output_wi = (v_output_wi + 1) & 8191;
                }
                if v_save_code <= 4095 {
                    let v_lm1_b = (self.f_lm1s[v_prev_code as usize].wrapping_add(1)) & 4095;
                    self.f_lm1s[v_save_code as usize] = v_lm1_b;
                    if v_lm1_b.is_multiple_of(8) {
                        self.imp.f_prefixes[v_save_code as usize] = v_prev_code as u16;
                        self.f_suffixes[v_save_code as usize][0] = v_first_byte;
                    } else {
                        self.imp.f_prefixes[v_save_code as usize] =
                            self.imp.f_prefixes[v_prev_code as usize];
                        self.f_suffixes[v_save_code as usize] =
                            self.f_suffixes[v_prev_code as usize];
                        self.f_suffixes[v_save_code as usize][(v_lm1_b % 8) as usize] =
                            v_first_byte;
                    }
                    v_save_code += 1;
                    if v_width < 12 {
                        v_width += 1 & (v_save_code >> v_width);
                    }
                    v_prev_code = v_code;
                }
            } else {
                self.imp.f_read_from_return_value = 4;
                break 'main;
            }
            if v_output_wi > 4095 {
                self.imp.f_read_from_return_value = 1;
                break 'main;
            }
        }

        if self.imp.f_read_from_return_value != 2 {
            while v_n_bits >= 8 {
                v_n_bits -= 8;
                if iop > io1 {
                    iop -= 1;
                } else {
                    self.imp.f_read_from_return_value = 5;
                    break;
                }
            }
        }
        self.imp.f_save_code = v_save_code;
        self.imp.f_prev_code = v_prev_code;
        self.imp.f_width = v_width;
        self.imp.f_bits = v_bits;
        self.imp.f_n_bits = v_n_bits;
        self.imp.f_output_wi = v_output_wi;
        src_meta.ri = iop;
    }

    /// Port of `wuffs_lzw__decoder__write_to`: copies pending output into `dst_data` from
    /// `dst_meta.wi`. Suspends with `short write` when the destination fills up. Entry and
    /// resumption both re-check the C `while` condition, so there is no separate resumption
    /// point to dispatch on.
    fn write_to(&mut self, dst_data: &mut [u8], dst_meta: &mut IoMeta) -> Status {
        let mut iop = dst_meta.wi;
        let io2 = if dst_meta.closed { iop } else { dst_data.len() };
        let status = if self.imp.f_output_wi > 0 {
            if self.imp.f_output_ri > self.imp.f_output_wi {
                Status::new(strings::LZW_ERROR_INTERNAL_INCONSISTENT_I_O)
            } else {
                let ri = self.imp.f_output_ri as usize;
                let wi = self.imp.f_output_wi as usize;
                let s_len = wi - ri;
                let n = s_len.min(io2 - iop);
                dst_data[iop..iop + n].copy_from_slice(&self.f_output[ri..ri + n]);
                iop += n;
                if n == s_len {
                    self.imp.f_output_ri = 0;
                    self.imp.f_output_wi = 0;
                    Status::OK
                } else {
                    self.imp.f_output_ri = (self.imp.f_output_ri + n as u32) & 8191;
                    Status::new(strings::SUSPENSION_SHORT_WRITE)
                }
            }
        } else {
            Status::OK
        };
        dst_meta.wi = iop;
        self.imp.p_write_to = u32::from(status.is_suspension());
        status
    }
}
