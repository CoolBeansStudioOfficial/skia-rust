// Copyright 1995-2023 Mark Adler (zlib); port by The skia-rust Authors.
// Use of this source code is governed by the zlib licence in the LICENSE file.
// Port of: inflate.c#L83-L1266 (chromium zlib@646b7f56, zlib 1.3.0.1-motley)
// Ported from: inflate.c, inflate.h, zutil.h (ZSWAP32)

//! The inflate state machine: decompresses zlib (RFC 1950) and raw deflate (RFC 1951) streams.
//!
//! Scope of the port: the zlib and raw formats, which are what libpng and Skia use. Gzip
//! (`windowBits` 16..=47, `wrap & 2` in zlib) is not ported and is rejected by [`Inflate::new`].
//! Preset dictionaries (`inflateSetDictionary`) are not ported either: a stream that needs one
//! returns [`ReturnCode::NeedDict`] as in zlib, and the caller cannot continue it.
//!
//! The control flow follows zlib's `switch` over `state->mode`. Its `goto inf_leave` is `break
//! 'leave`, and a `break` out of the switch is `continue 'leave`. The `NEEDBITS` and `PULLBYTE`
//! macros return false instead of jumping, and the caller then leaves, so the labels stay in the
//! function body.

use crate::adler32::{ADLER32_INIT, adler32};
use crate::inffast::{INFLATE_FAST_MIN_INPUT, INFLATE_FAST_MIN_OUTPUT, inflate_fast};
use crate::inffixed::{DISTFIX, LENFIX};
use crate::inftrees::{CodeType, ENOUGH, inflate_table};
use crate::{Code, Flush, ReturnCode};

/// Port of `DEF_WBITS` (`MAX_WBITS`).
pub const DEF_WBITS: i32 = 15;

/// Port of `inflate_mode`, without the gzip-header states (see the module docs). The order is
/// zlib's, and the state machine compares modes by it (`mode < BAD`, `mode < CHECK`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Mode {
    Head,
    DictId,
    Dict,
    Type,
    TypeDo,
    Stored,
    CopyStored,
    Copy,
    Table,
    LenLens,
    CodeLens,
    LenStart,
    Len,
    LenExt,
    Dist,
    DistExt,
    Match,
    Lit,
    Check,
    Done,
    Bad,
}

/// Port of `ORDER`: the permutation of code-length code lengths (inflate.c#L608-L609).
const ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// Port of `struct inflate_state` (inflate.h#L82-L126), minus the gzip fields.
#[allow(clippy::struct_excessive_bools)] // mirrors zlib's flag fields
pub(crate) struct InflateState {
    pub(crate) mode: Mode,
    pub(crate) last: bool,
    /// 0 for raw deflate, 1 for zlib: bit 0 is the header, bit 2 the trailer check.
    pub(crate) wrap: u32,
    pub(crate) havedict: bool,
    pub(crate) sane: bool,
    pub(crate) dmax: u32,
    pub(crate) check: u32,
    pub(crate) total: u64,
    pub(crate) wbits: u32,
    pub(crate) wsize: u32,
    pub(crate) whave: u32,
    pub(crate) wnext: u32,
    pub(crate) window: Vec<u8>,
    pub(crate) hold: u64,
    pub(crate) bits: u32,
    pub(crate) length: u32,
    pub(crate) offset: u32,
    pub(crate) extra: u32,
    /// Index into `codes` of the literal/length table (`state->lencode`).
    pub(crate) lencode: usize,
    /// Index into `codes` of the distance table (`state->distcode`).
    pub(crate) distcode: usize,
    pub(crate) lenbits: u32,
    pub(crate) distbits: u32,
    pub(crate) ncode: usize,
    pub(crate) nlen: usize,
    pub(crate) ndist: usize,
    pub(crate) have: usize,
    /// Index into `codes` where the next table is built (`state->next`).
    pub(crate) next: usize,
    pub(crate) lens: [u16; 320],
    pub(crate) work: [u16; 288],
    pub(crate) codes: Vec<Code>,
    pub(crate) back: i32,
    pub(crate) was: u32,
    /// Port of `z_stream::msg`.
    pub(crate) msg: Option<&'static str>,
    /// Port of `z_stream::adler`.
    pub(crate) adler: u32,
    /// Port of `z_stream::total_in`.
    pub(crate) total_in: u64,
    /// Port of `z_stream::total_out`.
    pub(crate) total_out: u64,
    /// Port of `z_stream::data_type`.
    pub(crate) data_type: i32,
}

impl InflateState {
    /// Port of `inflateStateCheck`'s state after `inflateInit2_` (inflate.c#L178-L217).
    fn new(wrap: u32, windowbits: u32) -> Self {
        Self {
            mode: Mode::Head,
            last: false,
            wrap,
            havedict: false,
            sane: true,
            dmax: 32768,
            check: ADLER32_INIT,
            total: 0,
            wbits: windowbits,
            wsize: 0,
            whave: 0,
            wnext: 0,
            window: Vec::new(),
            hold: 0,
            bits: 0,
            length: 0,
            offset: 0,
            extra: 0,
            lencode: 0,
            distcode: 0,
            lenbits: 0,
            distbits: 0,
            ncode: 0,
            nlen: 0,
            ndist: 0,
            have: 0,
            next: 0,
            lens: [0; 320],
            work: [0; 288],
            codes: vec![Code::default(); ENOUGH],
            back: -1,
            was: 0,
            msg: None,
            adler: 0,
            total_in: 0,
            total_out: 0,
            data_type: 0,
        }
    }
}

/// Port of `fixedtables` (inflate.c#L253-L292): points the decoder at the fixed Huffman tables.
///
/// zlib keeps them in static arrays. Here they are copied to the front of `codes`, which is free
/// while a fixed block is decoded: a dynamic block rebuilds `codes` from index 0 before it uses it.
fn fixedtables(state: &mut InflateState) {
    state.codes[..LENFIX.len()].copy_from_slice(&LENFIX);
    state.codes[LENFIX.len()..LENFIX.len() + DISTFIX.len()].copy_from_slice(&DISTFIX);
    state.lencode = 0;
    state.lenbits = 9;
    state.distcode = LENFIX.len();
    state.distbits = 5;
}

/// Port of `updatewindow` (inflate.c#L369-L413): copies the last `copy` bytes written to the
/// output (`end` is the output so far) into the sliding window.
// Port of: inflate.c#L369-L413 (chromium zlib@646b7f56)
#[allow(clippy::cast_possible_truncation)] // zlib's window fields are unsigned; wsize <= 32768
fn updatewindow(state: &mut InflateState, end: &[u8], mut copy: usize) {
    if state.window.is_empty() {
        state.window = vec![0; 1usize << state.wbits];
    }
    // Port of: inflate.c#L384-L389
    if state.wsize == 0 {
        state.wsize = 1 << state.wbits;
        state.wnext = 0;
        state.whave = 0;
    }
    let wsize = state.wsize as usize;
    let end_pos = end.len();
    if copy >= wsize {
        state.window[..wsize].copy_from_slice(&end[end_pos - wsize..end_pos]);
        state.wnext = 0;
        state.whave = state.wsize;
    } else {
        let wnext = state.wnext as usize;
        let mut dist = wsize - wnext;
        if dist > copy {
            dist = copy;
        }
        state.window[wnext..wnext + dist]
            .copy_from_slice(&end[end_pos - copy..end_pos - copy + dist]);
        copy -= dist;
        if copy != 0 {
            state.window[..copy].copy_from_slice(&end[end_pos - copy..end_pos]);
            state.wnext = copy as u32;
            state.whave = state.wsize;
        } else {
            state.wnext += dist as u32;
            if state.wnext == state.wsize {
                state.wnext = 0;
            }
            if state.whave < state.wsize {
                state.whave += dist as u32;
            }
        }
    }
}

/// The result of one [`Inflate::inflate`] call: zlib's return code, and how many input bytes were
/// consumed and output bytes produced (the `avail_in` and `avail_out` changes in zlib).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inflated {
    /// The return code (`Z_OK`, `Z_STREAM_END`, `Z_DATA_ERROR`, ...).
    pub ret: ReturnCode,
    /// Bytes taken from the front of the input.
    pub consumed: usize,
    /// Bytes written to the front of the output.
    pub produced: usize,
}

/// An inflate stream: the state behind `z_streamp->state`, plus the `z_stream` fields that zlib
/// keeps next to it (`msg`, `adler`, `total_in`, `total_out`, `data_type`).
///
/// zlib's `next_in` and `next_out` pointers are replaced by the slices passed to
/// [`Inflate::inflate`]: each call reads from the front of the input and writes at the front of
/// the output, and reports how far it got.
pub struct Inflate {
    state: Box<InflateState>,
}

impl std::fmt::Debug for Inflate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inflate")
            .field("mode", &self.state.mode)
            .field("wbits", &self.state.wbits)
            .field("total_in", &self.state.total_in)
            .field("total_out", &self.state.total_out)
            .finish_non_exhaustive()
    }
}

impl Inflate {
    /// Port of `inflateInit2_` (inflate.c#L178-L217) for the zlib and raw formats.
    ///
    /// `window_bits` is `15` for zlib streams, `-15` for raw deflate, and `0` to take the window
    /// size from the stream header.
    ///
    /// # Errors
    /// `StreamError` for the other values, including gzip.
    #[doc(alias = "inflateInit2")]
    pub fn new(window_bits: i32) -> Result<Self, ReturnCode> {
        let mut state = Box::new(InflateState::new(0, 0));
        Self::reset2(&mut state, window_bits)?;
        Ok(Self { state })
    }

    /// Port of `inflateInit_`: `inflateInit2` with the default 15-bit window.
    ///
    /// # Errors
    /// Never in practice: 15 is a valid window size. Kept for the zlib API shape.
    #[doc(alias = "inflateInit")]
    pub fn new_default() -> Result<Self, ReturnCode> {
        Self::new(DEF_WBITS)
    }

    /// Port of `inflateReset2` (inflate.c#L141-L176): sets the window size and format, then resets.
    // `window_bits` is checked to be non-negative before the casts that narrow it.
    #[allow(clippy::cast_sign_loss)]
    fn reset2(state: &mut InflateState, window_bits: i32) -> Result<(), ReturnCode> {
        let wrap;
        let mut wb = window_bits;
        if wb < 0 {
            if wb < -15 {
                return Err(ReturnCode::StreamError);
            }
            wrap = 0;
            wb = -wb;
        } else {
            wrap = ((wb >> 4) + 5) as u32;
            if wb >= 16 {
                // Gzip (`wrap & 2`) and auto-detection (`windowBits >= 32`) are not ported.
                return Err(ReturnCode::StreamError);
            }
        }
        if wb != 0 && !(8..=15).contains(&wb) {
            return Err(ReturnCode::StreamError);
        }
        if !state.window.is_empty() && state.wbits != wb as u32 {
            state.window = Vec::new();
        }
        state.wrap = wrap;
        state.wbits = wb as u32;
        // Port of: inflateReset (inflate.c#L130-L139)
        state.wsize = 0;
        state.whave = 0;
        state.wnext = 0;
        Self::reset_keep(state);
        Ok(())
    }

    /// Port of `inflateResetKeep` (inflate.c#L106-L128).
    fn reset_keep(state: &mut InflateState) {
        state.total_in = 0;
        state.total_out = 0;
        state.total = 0;
        state.msg = None;
        if state.wrap != 0 {
            state.adler = state.wrap & 1;
        }
        state.mode = Mode::Head;
        state.last = false;
        state.havedict = false;
        state.dmax = 32768;
        state.hold = 0;
        state.bits = 0;
        state.lencode = 0;
        state.distcode = 0;
        state.next = 0;
        state.sane = true;
        state.back = -1;
    }

    /// Port of `inflateReset` (inflate.c#L130-L139): keeps the window size and format.
    #[doc(alias = "inflateReset")]
    pub fn reset(&mut self) -> ReturnCode {
        self.state.wsize = 0;
        self.state.whave = 0;
        self.state.wnext = 0;
        Self::reset_keep(&mut self.state);
        ReturnCode::Ok
    }

    /// The message of the last error (`z_stream::msg`).
    #[must_use]
    pub fn msg(&self) -> Option<&'static str> {
        self.state.msg
    }

    /// The running checksum of the output (`z_stream::adler`).
    #[must_use]
    pub fn adler(&self) -> u32 {
        self.state.adler
    }

    /// Total input bytes consumed so far (`z_stream::total_in`).
    #[must_use]
    pub fn total_in(&self) -> u64 {
        self.state.total_in
    }

    /// Total output bytes produced so far (`z_stream::total_out`).
    #[must_use]
    pub fn total_out(&self) -> u64 {
        self.state.total_out
    }

    /// The `data_type` flags of the last call (`z_stream::data_type`).
    #[must_use]
    pub fn data_type(&self) -> i32 {
        self.state.data_type
    }

    /// Port of `inflate` (inflate.c#L591-L1265): decompresses from `input` into `output` as far
    /// as the buffers allow. Returns `Ok` when more input or output is needed, `StreamEnd` when
    /// the stream is complete, `DataError` for a corrupt stream and `BufError` when no progress
    /// was possible. The result says how many input and output bytes were used.
    // Port of: inflate.c#L591-L1265 (chromium zlib@646b7f56)
    #[allow(clippy::too_many_lines)] // the state machine is one function in zlib too
    #[allow(clippy::cognitive_complexity)] // the state machine is one function in zlib too
    #[allow(clippy::cast_possible_truncation)] // hold and bit counts are narrowed as zlib does
    #[allow(clippy::cast_sign_loss)] // counts are non-negative where zlib uses unsigned
    #[allow(clippy::similar_names)] // zlib's names: `have`, `left`, `hold`, `here`, `last`
    pub fn inflate(&mut self, input: &[u8], output: &mut [u8], flush: Flush) -> Inflated {
        let st: &mut InflateState = &mut self.state;
        if st.mode == Mode::Type {
            st.mode = Mode::TypeDo;
        }

        // Port of: LOAD() (inflate.c#L445-L453)
        let mut ip: usize = 0;
        let mut have: usize = input.len();
        let mut op: usize = 0;
        let mut left: usize = output.len();
        let mut hold: u64 = st.hold;
        let mut bits: u32 = st.bits;
        let mut out = left; // zlib's `out`
        let mut ret = ReturnCode::Ok;
        // Set by the paths that return without the epilogue (`return Z_NEED_DICT`).
        let mut early: Option<ReturnCode> = None;

        // The NEEDBITS/PULLBYTE/BITS/DROPBITS/BYTEBITS helpers (inflate.c#L467-L507). They are
        // macros so that they can refer to the locals above. The ones that can leave return false.
        macro_rules! pull_byte {
            () => {{
                if have == 0 {
                    false
                } else {
                    have -= 1;
                    hold += u64::from(input[ip]) << bits;
                    ip += 1;
                    bits += 8;
                    true
                }
            }};
        }
        macro_rules! need_bits {
            ($n:expr) => {{
                let n: u32 = $n;
                let mut ok = true;
                while bits < n {
                    if !pull_byte!() {
                        ok = false;
                        break;
                    }
                }
                ok
            }};
        }
        macro_rules! bits_of {
            ($n:expr) => {
                ((hold as u32) & ((1u32 << ($n)) - 1))
            };
        }
        macro_rules! drop_bits {
            ($n:expr) => {{
                let n: u32 = $n;
                hold >>= n;
                bits -= n;
            }};
        }
        macro_rules! byte_bits {
            () => {{
                hold >>= bits & 7;
                bits -= bits & 7;
            }};
        }
        macro_rules! init_bits {
            () => {{
                hold = 0;
                bits = 0;
            }};
        }

        'leave: loop {
            match st.mode {
                // Port of: inflate.c#L623-L670 (zlib header)
                Mode::Head => {
                    if st.wrap == 0 {
                        st.mode = Mode::TypeDo;
                        continue 'leave;
                    }
                    if !need_bits!(16) {
                        break 'leave;
                    }
                    if !((u64::from(bits_of!(8)) << 8) + (hold >> 8)).is_multiple_of(31) {
                        st.msg = Some("incorrect header check");
                        st.mode = Mode::Bad;
                        continue 'leave;
                    }
                    if bits_of!(4) != 8 {
                        st.msg = Some("unknown compression method");
                        st.mode = Mode::Bad;
                        continue 'leave;
                    }
                    drop_bits!(4);
                    let len = bits_of!(4) + 8;
                    if st.wbits == 0 {
                        st.wbits = len;
                    }
                    if len > 15 || len > st.wbits {
                        st.msg = Some("invalid window size");
                        st.mode = Mode::Bad;
                        continue 'leave;
                    }
                    st.dmax = 1 << len;
                    st.adler = ADLER32_INIT;
                    st.check = ADLER32_INIT;
                    st.mode = if hold & 0x200 != 0 {
                        Mode::DictId
                    } else {
                        Mode::Type
                    };
                    init_bits!();
                }
                // Port of: inflate.c#L811-L815
                Mode::DictId => {
                    if !need_bits!(32) {
                        break 'leave;
                    }
                    let v = (hold as u32).swap_bytes();
                    st.adler = v;
                    st.check = v;
                    init_bits!();
                    st.mode = Mode::Dict;
                }
                // Port of: inflate.c#L817-L823
                Mode::Dict => {
                    if !st.havedict {
                        early = Some(ReturnCode::NeedDict);
                        break 'leave;
                    }
                    st.adler = ADLER32_INIT;
                    st.check = ADLER32_INIT;
                    st.mode = Mode::Type;
                }
                // Port of: inflate.c#L825-L827 (`case TYPE:` falls through to `case TYPEDO:`)
                Mode::Type => {
                    if matches!(flush, Flush::Block | Flush::Trees) {
                        break 'leave;
                    }
                    st.mode = Mode::TypeDo;
                }
                // Port of: inflate.c#L828-L863 (block header)
                Mode::TypeDo => {
                    if st.last {
                        byte_bits!();
                        st.mode = Mode::Check;
                        continue 'leave;
                    }
                    if !need_bits!(3) {
                        break 'leave;
                    }
                    st.last = bits_of!(1) != 0;
                    drop_bits!(1);
                    match bits_of!(2) {
                        0 => st.mode = Mode::Stored,
                        1 => {
                            fixedtables(st);
                            st.mode = Mode::LenStart;
                            if flush == Flush::Trees {
                                drop_bits!(2);
                                break 'leave;
                            }
                        }
                        2 => st.mode = Mode::Table,
                        _ => {
                            st.msg = Some("invalid block type");
                            st.mode = Mode::Bad;
                        }
                    }
                    drop_bits!(2);
                }
                // Port of: inflate.c#L864-L878 (stored block header)
                Mode::Stored => {
                    byte_bits!();
                    if !need_bits!(32) {
                        break 'leave;
                    }
                    if (hold & 0xffff) != ((hold >> 16) ^ 0xffff) {
                        st.msg = Some("invalid stored block lengths");
                        st.mode = Mode::Bad;
                        continue 'leave;
                    }
                    st.length = (hold as u32) & 0xffff;
                    init_bits!();
                    st.mode = Mode::CopyStored;
                    if flush == Flush::Trees {
                        break 'leave;
                    }
                }
                // Port of: inflate.c#L879-L880 (`case COPY_:` falls through to `case COPY:`)
                Mode::CopyStored => st.mode = Mode::Copy,
                // Port of: inflate.c#L882-L898 (stored block data)
                Mode::Copy => {
                    let mut copy = st.length as usize;
                    if copy != 0 {
                        if copy > have {
                            copy = have;
                        }
                        if copy > left {
                            copy = left;
                        }
                        if copy == 0 {
                            break 'leave;
                        }
                        output[op..op + copy].copy_from_slice(&input[ip..ip + copy]);
                        have -= copy;
                        ip += copy;
                        left -= copy;
                        op += copy;
                        st.length -= copy as u32;
                        continue 'leave;
                    }
                    st.mode = Mode::Type;
                }
                // Port of: inflate.c#L899-L917 (dynamic block header)
                Mode::Table => {
                    if !need_bits!(14) {
                        break 'leave;
                    }
                    st.nlen = (bits_of!(5) + 257) as usize;
                    drop_bits!(5);
                    st.ndist = (bits_of!(5) + 1) as usize;
                    drop_bits!(5);
                    st.ncode = (bits_of!(4) + 4) as usize;
                    drop_bits!(4);
                    if st.nlen > 286 || st.ndist > 30 {
                        st.msg = Some("too many length or distance symbols");
                        st.mode = Mode::Bad;
                        continue 'leave;
                    }
                    st.have = 0;
                    st.mode = Mode::LenLens;
                }
                // Port of: inflate.c#L918-L939 (code-length code lengths)
                Mode::LenLens => {
                    while st.have < st.ncode {
                        if !need_bits!(3) {
                            break 'leave;
                        }
                        st.lens[ORDER[st.have]] = bits_of!(3) as u16;
                        st.have += 1;
                        drop_bits!(3);
                    }
                    while st.have < 19 {
                        st.lens[ORDER[st.have]] = 0;
                        st.have += 1;
                    }
                    st.next = 0;
                    st.lencode = st.next;
                    st.lenbits = 7;
                    let mut next = st.next;
                    let ret_code = inflate_table(
                        CodeType::Codes,
                        &st.lens,
                        19,
                        &mut st.codes,
                        &mut next,
                        &mut st.lenbits,
                        &mut st.work,
                    );
                    st.next = next;
                    if ret_code != 0 {
                        st.msg = Some("invalid code lengths set");
                        st.mode = Mode::Bad;
                        continue 'leave;
                    }
                    st.have = 0;
                    st.mode = Mode::CodeLens;
                }
                // Port of: inflate.c#L940-L1023 (literal/length and distance code lengths)
                Mode::CodeLens => {
                    let total = st.nlen + st.ndist;
                    let mut bad = false;
                    'codelens: while st.have < total {
                        let here: Code = loop {
                            let candidate = st.codes[st.lencode + bits_of!(st.lenbits) as usize];
                            if u32::from(candidate.bits) <= bits {
                                break candidate;
                            }
                            if !pull_byte!() {
                                break 'leave;
                            }
                        };
                        if here.val < 16 {
                            drop_bits!(u32::from(here.bits));
                            st.lens[st.have] = here.val;
                            st.have += 1;
                        } else {
                            let len: u16;
                            let copy: u32;
                            if here.val == 16 {
                                if !need_bits!(u32::from(here.bits) + 2) {
                                    break 'leave;
                                }
                                drop_bits!(u32::from(here.bits));
                                if st.have == 0 {
                                    st.msg = Some("invalid bit length repeat");
                                    st.mode = Mode::Bad;
                                    bad = true;
                                    break 'codelens;
                                }
                                len = st.lens[st.have - 1];
                                copy = 3 + bits_of!(2);
                                drop_bits!(2);
                            } else if here.val == 17 {
                                if !need_bits!(u32::from(here.bits) + 3) {
                                    break 'leave;
                                }
                                drop_bits!(u32::from(here.bits));
                                len = 0;
                                copy = 3 + bits_of!(3);
                                drop_bits!(3);
                            } else {
                                if !need_bits!(u32::from(here.bits) + 7) {
                                    break 'leave;
                                }
                                drop_bits!(u32::from(here.bits));
                                len = 0;
                                copy = 11 + bits_of!(7);
                                drop_bits!(7);
                            }
                            if st.have + copy as usize > total {
                                st.msg = Some("invalid bit length repeat");
                                st.mode = Mode::Bad;
                                bad = true;
                                break 'codelens;
                            }
                            for _ in 0..copy {
                                st.lens[st.have] = len;
                                st.have += 1;
                            }
                        }
                    }
                    // Port of: inflate.c#L989 (`if (state->mode == BAD) break;`)
                    if bad || st.mode == Mode::Bad {
                        continue 'leave;
                    }
                    // Port of: inflate.c#L992-L997 (the end-of-block code must be present)
                    if st.lens[256] == 0 {
                        st.msg = Some("invalid code -- missing end-of-block");
                        st.mode = Mode::Bad;
                        continue 'leave;
                    }
                    // Port of: inflate.c#L1000-L1020 (build the literal/length and distance tables)
                    st.next = 0;
                    st.lencode = st.next;
                    st.lenbits = 10;
                    let mut next = st.next;
                    let nlen = st.nlen;
                    let ret_code = inflate_table(
                        CodeType::Lens,
                        &st.lens,
                        nlen,
                        &mut st.codes,
                        &mut next,
                        &mut st.lenbits,
                        &mut st.work,
                    );
                    if ret_code != 0 {
                        st.msg = Some("invalid literal/lengths set");
                        st.mode = Mode::Bad;
                        st.next = next;
                        continue 'leave;
                    }
                    st.distcode = next;
                    st.distbits = 9;
                    let ndist = st.ndist;
                    let ret_code = inflate_table(
                        CodeType::Dists,
                        &st.lens[nlen..],
                        ndist,
                        &mut st.codes,
                        &mut next,
                        &mut st.distbits,
                        &mut st.work,
                    );
                    st.next = next;
                    if ret_code != 0 {
                        st.msg = Some("invalid distances set");
                        st.mode = Mode::Bad;
                        continue 'leave;
                    }
                    st.mode = Mode::LenStart;
                    if flush == Flush::Trees {
                        break 'leave;
                    }
                }
                // Port of: inflate.c#L1024-L1026 (`case LEN_:` falls through to `case LEN:`)
                Mode::LenStart => st.mode = Mode::Len,
                // Port of: inflate.c#L1027-L1077 (length/literal code)
                Mode::Len => {
                    if have >= INFLATE_FAST_MIN_INPUT && left >= INFLATE_FAST_MIN_OUTPUT {
                        // RESTORE(); inflate_fast(strm, out); LOAD();
                        st.hold = hold;
                        st.bits = bits;
                        let fast_msg = inflate_fast(st, input, &mut ip, output, &mut op, 0);
                        hold = st.hold;
                        bits = st.bits;
                        have = input.len() - ip;
                        left = output.len() - op;
                        if let Some(m) = fast_msg {
                            st.msg = Some(m);
                        }
                        if st.mode == Mode::Type {
                            st.back = -1;
                        }
                        continue 'leave;
                    }
                    st.back = 0;
                    let mut here: Code = loop {
                        let candidate = st.codes[st.lencode + bits_of!(st.lenbits) as usize];
                        if u32::from(candidate.bits) <= bits {
                            break candidate;
                        }
                        if !pull_byte!() {
                            break 'leave;
                        }
                    };
                    if here.op != 0 && (here.op & 0xf0) == 0 {
                        let last = here;
                        here = loop {
                            let candidate = st.codes[st.lencode
                                + usize::from(last.val)
                                + (bits_of!(u32::from(last.bits) + u32::from(last.op)) >> last.bits)
                                    as usize];
                            if u32::from(last.bits) + u32::from(candidate.bits) <= bits {
                                break candidate;
                            }
                            if !pull_byte!() {
                                break 'leave;
                            }
                        };
                        drop_bits!(u32::from(last.bits));
                        st.back += i32::from(last.bits);
                    }
                    drop_bits!(u32::from(here.bits));
                    st.back += i32::from(here.bits);
                    st.length = u32::from(here.val);
                    if here.op == 0 {
                        st.mode = Mode::Lit;
                        continue 'leave;
                    }
                    if here.op & 32 != 0 {
                        st.back = -1;
                        st.mode = Mode::Type;
                        continue 'leave;
                    }
                    if here.op & 64 != 0 {
                        st.msg = Some("invalid literal/length code");
                        st.mode = Mode::Bad;
                        continue 'leave;
                    }
                    st.extra = u32::from(here.op) & 15;
                    st.mode = Mode::LenExt;
                }
                // Port of: inflate.c#L1078-L1088 (extra length bits)
                Mode::LenExt => {
                    if st.extra != 0 {
                        if !need_bits!(st.extra) {
                            break 'leave;
                        }
                        st.length += bits_of!(st.extra);
                        drop_bits!(st.extra);
                        st.back += st.extra.cast_signed();
                    }
                    st.was = st.length;
                    st.mode = Mode::Dist;
                }
                // Port of: inflate.c#L1089-L1116 (distance code)
                Mode::Dist => {
                    let mut here: Code = loop {
                        let candidate = st.codes[st.distcode + bits_of!(st.distbits) as usize];
                        if u32::from(candidate.bits) <= bits {
                            break candidate;
                        }
                        if !pull_byte!() {
                            break 'leave;
                        }
                    };
                    if (here.op & 0xf0) == 0 {
                        let last = here;
                        here = loop {
                            let candidate = st.codes[st.distcode
                                + usize::from(last.val)
                                + (bits_of!(u32::from(last.bits) + u32::from(last.op)) >> last.bits)
                                    as usize];
                            if u32::from(last.bits) + u32::from(candidate.bits) <= bits {
                                break candidate;
                            }
                            if !pull_byte!() {
                                break 'leave;
                            }
                        };
                        drop_bits!(u32::from(last.bits));
                        st.back += i32::from(last.bits);
                    }
                    drop_bits!(u32::from(here.bits));
                    st.back += i32::from(here.bits);
                    if here.op & 64 != 0 {
                        st.msg = Some("invalid distance code");
                        st.mode = Mode::Bad;
                        continue 'leave;
                    }
                    st.offset = u32::from(here.val);
                    st.extra = u32::from(here.op) & 15;
                    st.mode = Mode::DistExt;
                }
                // Port of: inflate.c#L1117-L1133 (extra distance bits)
                Mode::DistExt => {
                    if st.extra != 0 {
                        if !need_bits!(st.extra) {
                            break 'leave;
                        }
                        st.offset += bits_of!(st.extra);
                        drop_bits!(st.extra);
                        st.back += st.extra.cast_signed();
                    }
                    st.mode = Mode::Match;
                }
                // Port of: inflate.c#L1134-L1178 (copy the match from the output or the window)
                Mode::Match => {
                    if left == 0 {
                        break 'leave;
                    }
                    let mut copy = out - left;
                    // `from` is an index into the window (`from_window`) or into the output.
                    let from_window: bool;
                    let mut from: usize;
                    if st.offset as usize > copy {
                        copy = st.offset as usize - copy;
                        // INFLATE_ALLOW_INVALID_DISTANCE_TOOFAR_ARRR is not defined in the oracle
                        // build, and `sane` is always set here, so the `!sane` path is not ported.
                        if copy > st.whave as usize && st.sane {
                            st.msg = Some("invalid distance too far back");
                            st.mode = Mode::Bad;
                            continue 'leave;
                        }
                        from_window = true;
                        if copy > st.wnext as usize {
                            copy -= st.wnext as usize;
                            from = st.wsize as usize - copy;
                        } else {
                            from = st.wnext as usize - copy;
                        }
                        if copy > st.length as usize {
                            copy = st.length as usize;
                        }
                    } else {
                        from_window = false;
                        from = op - st.offset as usize;
                        copy = st.length as usize;
                    }
                    if copy > left {
                        copy = left;
                    }
                    left -= copy;
                    st.length -= copy as u32;
                    for _ in 0..copy {
                        let byte = if from_window {
                            st.window[from]
                        } else {
                            output[from]
                        };
                        output[op] = byte;
                        op += 1;
                        from += 1;
                    }
                    if st.length == 0 {
                        st.mode = Mode::Len;
                    }
                }
                // Port of: inflate.c#L1179-L1184 (a literal)
                Mode::Lit => {
                    if left == 0 {
                        break 'leave;
                    }
                    output[op] = st.length as u8;
                    op += 1;
                    left -= 1;
                    st.mode = Mode::Len;
                }
                // Port of: inflate.c#L1185-L1206 (the zlib trailer check)
                Mode::Check => {
                    if st.wrap != 0 {
                        if !need_bits!(32) {
                            break 'leave;
                        }
                        out -= left;
                        st.total_out += out as u64;
                        st.total += out as u64;
                        if (st.wrap & 4) != 0 && out != 0 {
                            st.check = adler32(st.check, &output[op - out..op]);
                            st.adler = st.check;
                        }
                        out = left;
                        if (st.wrap & 4) != 0 && (hold as u32).swap_bytes() != st.check {
                            st.msg = Some("incorrect data check");
                            st.mode = Mode::Bad;
                            continue 'leave;
                        }
                        init_bits!();
                    }
                    st.mode = Mode::Done;
                }
                // Port of: inflate.c#L1224-L1229
                Mode::Done => {
                    ret = ReturnCode::StreamEnd;
                    break 'leave;
                }
                Mode::Bad => {
                    ret = ReturnCode::DataError;
                    break 'leave;
                }
            }
        }

        // Port of: inf_leave (inflate.c#L1244-L1265). Save the state; `early` (NEED_DICT) returns
        // without the epilogue, as `return Z_NEED_DICT` does in zlib.
        st.hold = hold;
        st.bits = bits;
        if let Some(r) = early {
            return Inflated {
                ret: r,
                consumed: ip,
                produced: op,
            };
        }
        // `strm->avail_out` is `left` here, so `out != strm->avail_out` is `out != left`.
        if st.wsize != 0
            || (out != left
                && st.mode < Mode::Bad
                && (st.mode < Mode::Check || flush != Flush::Finish))
        {
            // `strm->next_out` is `output[op..]` in zlib, so the output so far is `output[..op]`.
            updatewindow(st, &output[..op], out - left);
        }
        // `in -= strm->avail_in` and `out -= strm->avail_out` (the bytes this call consumed and
        // produced).
        let consumed = ip;
        let produced = out - left;
        st.total_in += consumed as u64;
        st.total_out += produced as u64;
        st.total += produced as u64;
        if (st.wrap & 4) != 0 && produced != 0 {
            st.check = adler32(st.check, &output[op - produced..op]);
            st.adler = st.check;
        }
        st.data_type = bits.cast_signed()
            + if st.last { 64 } else { 0 }
            + if st.mode == Mode::Type { 128 } else { 0 }
            + if st.mode == Mode::LenStart || st.mode == Mode::CopyStored {
                256
            } else {
                0
            };
        if ((consumed == 0 && produced == 0) || flush == Flush::Finish) && ret == ReturnCode::Ok {
            ret = ReturnCode::BufError;
        }
        // The caller sees the bytes this call wrote, which is `op`. `produced` above is zlib's
        // own accounting, which the CHECK state resets once the trailer is read.
        Inflated {
            ret,
            consumed,
            produced: op,
        }
    }
}
