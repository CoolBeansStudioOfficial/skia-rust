// Copyright 1995-2023 Mark Adler (zlib); port by The skia-rust Authors.
// Use of this source code is governed by the zlib licence in the LICENSE file.
// Port of: inffast.c#L54-L310 (chromium zlib@646b7f56)
// Ported from: inffast.c, inffast.h

//! Fast inflate loop, used when there is plenty of input and output space.
//!
//! Pointers become indices: `in`/`last`/`end`/`beg`/`from` are positions in the input, output or
//! window buffer. The window case is a separate slice, so `from` also records which one it is.

use crate::inflate::{InflateState, Mode};

/// Port of `INFLATE_FAST_MIN_INPUT` (inffast.h#L17).
pub const INFLATE_FAST_MIN_INPUT: usize = 6;
/// Port of `INFLATE_FAST_MIN_OUTPUT` (inffast.h#L24).
pub const INFLATE_FAST_MIN_OUTPUT: usize = 258;

/// Where a match is copied from: the output so far, or the sliding window.
#[derive(Clone, Copy)]
enum From {
    Window(usize),
    Output(usize),
}

/// Copies one byte from `from` to `output[out]` and advances both (`*out++ = *from++`).
#[inline]
fn copy_byte(window: &[u8], output: &mut [u8], out: &mut usize, from: &mut From) {
    let byte = match *from {
        From::Window(i) => {
            *from = From::Window(i + 1);
            window[i]
        }
        From::Output(i) => {
            *from = From::Output(i + 1);
            output[i]
        }
    };
    output[*out] = byte;
    *out += 1;
}

/// Port of `inflate_fast`: decodes symbols until input or output runs low, or the block ends.
///
/// `ip` is the input position and `op` the output position, both updated on return, as
/// `strm->next_in`/`next_out` are in C. `beg` is the output position where the current
/// `inflate()` call started. On a corrupt stream the state's mode becomes `Bad` and the message is
/// returned. The caller has ensured at least `INFLATE_FAST_MIN_INPUT` input bytes and
/// `INFLATE_FAST_MIN_OUTPUT` output bytes remain.
// Port of: inffast.c#L54-L310 (chromium zlib@646b7f56)
#[allow(clippy::too_many_lines)] // mirrors inflate_fast as one function, as in zlib
#[allow(clippy::cast_possible_truncation)] // the C code narrows to unsigned char on purpose
pub fn inflate_fast(
    state: &mut InflateState,
    input: &[u8],
    ip: &mut usize,
    output: &mut [u8],
    op: &mut usize,
    beg: usize,
) -> Option<&'static str> {
    let mut msg: Option<&'static str> = None;
    // Port of: inffast.c#L82-L100 (load the state; `last` and `end` keep zlib's margins)
    let mut inp = *ip;
    let last = input.len() - (INFLATE_FAST_MIN_INPUT - 1);
    let mut out = *op;
    let end = output.len() - (INFLATE_FAST_MIN_OUTPUT - 1);
    let wsize = state.wsize as usize;
    let whave = state.whave as usize;
    let wnext = state.wnext as usize;
    let mut hold: u64 = state.hold;
    let mut bits: u32 = state.bits;
    let lcode = state.lencode;
    let dcode = state.distcode;
    let lmask: u64 = (1u64 << state.lenbits) - 1;
    let dmask: u64 = (1u64 << state.distbits) - 1;

    // Port of: inffast.c#L104-L291 (the main loop)
    'main: loop {
        if bits < 15 {
            hold += u64::from(input[inp]) << bits;
            inp += 1;
            bits += 8;
            hold += u64::from(input[inp]) << bits;
            inp += 1;
            bits += 8;
        }
        let mut here = state.codes[lcode + (hold & lmask) as usize];
        'dolen: loop {
            let mut opc = u32::from(here.bits);
            hold >>= opc;
            bits -= opc;
            opc = u32::from(here.op);
            if opc == 0 {
                // literal
                output[out] = here.val as u8;
                out += 1;
            } else if opc & 16 != 0 {
                // length base
                let mut len = u32::from(here.val);
                opc &= 15;
                if opc != 0 {
                    if bits < opc {
                        hold += u64::from(input[inp]) << bits;
                        inp += 1;
                        bits += 8;
                    }
                    len += (hold as u32) & ((1u32 << opc) - 1);
                    hold >>= opc;
                    bits -= opc;
                }
                if bits < 15 {
                    hold += u64::from(input[inp]) << bits;
                    inp += 1;
                    bits += 8;
                    hold += u64::from(input[inp]) << bits;
                    inp += 1;
                    bits += 8;
                }
                here = state.codes[dcode + (hold & dmask) as usize];
                'dodist: loop {
                    opc = u32::from(here.bits);
                    hold >>= opc;
                    bits -= opc;
                    opc = u32::from(here.op);
                    if opc & 16 != 0 {
                        // distance base
                        let mut dist = u32::from(here.val);
                        opc &= 15;
                        if bits < opc {
                            hold += u64::from(input[inp]) << bits;
                            inp += 1;
                            bits += 8;
                            if bits < opc {
                                hold += u64::from(input[inp]) << bits;
                                inp += 1;
                                bits += 8;
                            }
                        }
                        dist += (hold as u32) & ((1u32 << opc) - 1);
                        hold >>= opc;
                        bits -= opc;
                        let mut back = out - beg;
                        let mut len_left = len as usize;
                        let dist = dist as usize;
                        // Port of: inffast.c#L170-L265 (copy the match)
                        if dist > back {
                            back = dist - back;
                            // INFLATE_ALLOW_INVALID_DISTANCE_TOOFAR_ARRR is not defined in the
                            // oracle build, so the `!sane` path that follows is not ported.
                            if back > whave && state.sane {
                                msg = Some("invalid distance too far back");
                                state.mode = Mode::Bad;
                                break 'main;
                            }
                            let window = &state.window;
                            let mut from;
                            if wnext == 0 {
                                from = From::Window(wsize - back);
                                if back < len_left {
                                    len_left -= back;
                                    for _ in 0..back {
                                        copy_byte(window, output, &mut out, &mut from);
                                    }
                                    from = From::Output(out - dist);
                                }
                            } else if wnext < back {
                                from = From::Window(wsize + wnext - back);
                                let mut back2 = back - wnext;
                                if back2 < len_left {
                                    len_left -= back2;
                                    for _ in 0..back2 {
                                        copy_byte(window, output, &mut out, &mut from);
                                    }
                                    from = From::Window(0);
                                    if wnext < len_left {
                                        back2 = wnext;
                                        len_left -= back2;
                                        for _ in 0..back2 {
                                            copy_byte(window, output, &mut out, &mut from);
                                        }
                                        from = From::Output(out - dist);
                                    }
                                }
                            } else {
                                from = From::Window(wnext - back);
                                if back < len_left {
                                    len_left -= back;
                                    for _ in 0..back {
                                        copy_byte(window, output, &mut out, &mut from);
                                    }
                                    from = From::Output(out - dist);
                                }
                            }
                            while len_left > 2 {
                                copy_byte(window, output, &mut out, &mut from);
                                copy_byte(window, output, &mut out, &mut from);
                                copy_byte(window, output, &mut out, &mut from);
                                len_left -= 3;
                            }
                            if len_left != 0 {
                                copy_byte(window, output, &mut out, &mut from);
                                if len_left > 1 {
                                    copy_byte(window, output, &mut out, &mut from);
                                }
                            }
                        } else {
                            let mut from = From::Output(out - dist);
                            loop {
                                copy_byte(&state.window, output, &mut out, &mut from);
                                copy_byte(&state.window, output, &mut out, &mut from);
                                copy_byte(&state.window, output, &mut out, &mut from);
                                len_left -= 3;
                                if len_left <= 2 {
                                    break;
                                }
                            }
                            if len_left != 0 {
                                copy_byte(&state.window, output, &mut out, &mut from);
                                if len_left > 1 {
                                    copy_byte(&state.window, output, &mut out, &mut from);
                                }
                            }
                        }
                        break 'dodist;
                    } else if opc & 64 != 0 {
                        msg = Some("invalid distance code");
                        state.mode = Mode::Bad;
                        break 'main;
                    }
                    // Here `opc` has neither bit 16 nor bit 64: it is a second-level distance
                    // table, so look up again in it (the loop repeats with the new entry).
                    here = state.codes
                        [dcode + usize::from(here.val) + (hold & ((1u64 << opc) - 1)) as usize];
                }
            } else if opc & 64 == 0 {
                // second-level length table
                here = state.codes
                    [lcode + usize::from(here.val) + (hold & ((1u64 << opc) - 1)) as usize];
                continue 'dolen;
            } else if opc & 32 != 0 {
                // end of block
                state.mode = Mode::Type;
                break 'main;
            } else {
                msg = Some("invalid literal/length code");
                state.mode = Mode::Bad;
                break 'main;
            }
            break 'dolen;
        }
        if !(inp < last && out < end) {
            break 'main;
        }
    }

    // Port of: inffast.c#L293-L309 (return unused bytes to the input, store the state)
    let len = (bits >> 3) as usize;
    inp -= len;
    bits -= len as u32 * 8;
    hold &= (1u64 << bits) - 1;
    *ip = inp;
    *op = out;
    state.hold = hold;
    state.bits = bits;
    msg
}
