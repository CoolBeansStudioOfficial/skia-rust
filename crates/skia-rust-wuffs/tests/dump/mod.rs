// Copyright 2017 The Wuffs Authors (the Wuffs sources this file ports).
// Modifications (the Rust port) Copyright (C) 2025 The skia-rust Authors.
// Licensed under the Apache License, Version 2.0; see the LICENSE file of this crate. This file
// is modified from the Wuffs sources.
//! The differential driver: the Rust side of `oracle/codec-diff/wuffs/wuffsdump.c`. It drives the
//! GIF decoder the way `SkWuffsCodec` does (`third_party/skia/src/codec/SkWuffsCodec.cpp`), and
//! writes the same text as the C driver. Any difference between the two outputs is a bug in the
//! port.

// The driver mirrors the C driver's integer conversions (u64 positions and sizes as `usize` on a
// 64-bit host), so the truncation lint is allowed for this test support module.
#![allow(clippy::cast_possible_truncation)]

//!
//! Per file, per variant and per read chunk size, it decodes the image config, the frame configs
//! (the frame count), and each of the first eight frames in three pixel formats, refilling the
//! io buffer on short reads. A sweep over every truncation point of the small files reports one
//! line per cut.

use std::cell::{Cell, RefCell};

use skia_rust_wuffs::gif::QUIRK_IGNORE_TOO_MUCH_PIXEL_DATA;
use skia_rust_wuffs::strings;
use skia_rust_wuffs::{
    FrameConfig, GifDecoder, ImageConfig, IoBuffer, IoMeta, PixelBlend, PixelBuffer, PixelConfig,
    Status, Table,
};

const IO_BUFFER_SIZE: usize = 4096;
const MAX_FRAMES: usize = 64;
const MAX_DECODED_FRAMES: usize = 8;
const SWEEP_MAX_LEN: usize = 512;

thread_local! {
    /// The dump being written. `run` returns it.
    static OUTPUT: RefCell<String> = const { RefCell::new(String::new()) };
    /// Set during the sweep: suppresses the per-step lines, as the C driver's `quiet`.
    static QUIET: Cell<bool> = const { Cell::new(false) };
}

/// Appends one line to the dump, unless the sweep is running quietly.
fn emit(line: &str) {
    if !QUIET.with(Cell::get) {
        OUTPUT.with(|o| {
            let mut o = o.borrow_mut();
            o.push_str(line);
            o.push('\n');
        });
    }
}

macro_rules! out {
    ($($arg:tt)*) => {
        emit(&format!($($arg)*))
    };
}

/// Port of the C `stream_t`: a memory stream that returns at most `chunk` bytes per read.
struct Stream<'a> {
    data: &'a [u8],
    pos: usize,
    chunk: usize,
}

/// The driver's state, as `session_t` in the C driver.
struct Session<'a> {
    dec: GifDecoder,
    io: IoBuffer,
    s: Stream<'a>,
    closed_on_eof: bool,
    width: u32,
    height: u32,
    first_io: u64,
    frame_io: Vec<u64>,
    frame_count: usize,
    /// `SkWuffsCodec`'s `fDecoderIsSuspended`.
    suspended: bool,
    /// The status and hash of frame 0 (BGRA, SRC) from the last `run_decode`.
    frame0_status: &'static str,
    frame0_hash: u64,
}

impl<'a> Session<'a> {
    fn new(data: &'a [u8], chunk: usize, closed_on_eof: bool) -> Self {
        let mut dec = GifDecoder::new();
        dec.set_quirk_enabled(QUIRK_IGNORE_TOO_MUCH_PIXEL_DATA, true);
        Session {
            dec,
            io: IoBuffer::with_capacity(IO_BUFFER_SIZE),
            s: Stream {
                data,
                pos: 0,
                chunk,
            },
            closed_on_eof,
            width: 0,
            height: 0,
            first_io: 0,
            frame_io: vec![0; MAX_FRAMES],
            frame_count: 0,
            suspended: false,
            frame0_status: "skip",
            frame0_hash: 0,
        }
    }
}

/// Port of `fill_buffer` in SkWuffsCodec.cpp, with the harness's `closed_on_eof` switch.
fn fill(se: &mut Session<'_>) -> bool {
    se.io.compact();
    let wi = se.io.meta.wi;
    let space = se.io.data.len() - wi;
    let remaining = se.s.data.len() - se.s.pos;
    let n = space.min(remaining).min(se.s.chunk);
    se.io.data[wi..wi + n].copy_from_slice(&se.s.data[se.s.pos..se.s.pos + n]);
    se.s.pos += n;
    se.io.meta.wi += n;
    se.io.meta.closed = se.closed_on_eof && n == 0;
    n > 0
}

/// After a failed `fill`, a client that has set `closed` calls the decoder once more, so the
/// decoder reports the truncation. Otherwise the input is incomplete and the caller stops.
fn retry(se: &mut Session<'_>) -> bool {
    fill(se) || se.io.meta.closed
}

/// Port of `seek_buffer` in SkWuffsCodec.cpp.
fn seek(se: &mut Session<'_>, pos: u64) -> bool {
    let meta_pos = se.io.meta.pos;
    if pos >= meta_pos && pos - meta_pos <= se.io.meta.wi as u64 {
        se.io.meta.ri = (pos - meta_pos) as usize;
        return true;
    }
    // SkMemoryStream::seek clamps the position to the length.
    let pos = pos.min(se.s.data.len() as u64);
    se.s.pos = pos as usize;
    se.io.meta = IoMeta {
        wi: 0,
        ri: 0,
        pos,
        closed: false,
    };
    true
}

/// The status text the driver prints: `ok`, or the message with its prefix.
fn status_text(st: Status) -> &'static str {
    st.repr().unwrap_or("ok")
}

/// FNV-1a 64, as the C driver's `fnv1a`.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 1_469_598_103_934_665_603;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(1_099_511_628_211);
    }
    h
}

/// Port of `SkWuffsCodec::resetDecoder`.
fn reset_decoder(se: &mut Session<'_>) -> bool {
    se.s.pos = 0;
    se.io.meta = IoMeta::default();
    se.dec = GifDecoder::new();
    se.dec
        .set_quirk_enabled(QUIRK_IGNORE_TOO_MUCH_PIXEL_DATA, true);
    loop {
        let st = se.dec.decode_image_config(None, &mut se.io);
        let Some(msg) = st.repr() else {
            break;
        };
        if msg != strings::SUSPENSION_SHORT_READ {
            out!("reset {msg}");
            return false;
        }
        if !retry(se) {
            out!("reset incomplete");
            return false;
        }
    }
    se.suspended = false;
    true
}

/// Port of `SkWuffsCodec::seekFrame`.
fn seek_frame(se: &mut Session<'_>, index: usize) -> bool {
    if se.suspended && !reset_decoder(se) {
        return false;
    }
    let pos = if index == 0 {
        se.first_io
    } else {
        se.frame_io[index]
    };
    if !seek(se, pos) {
        out!("seek failed");
        return false;
    }
    let st = se
        .dec
        .restart_frame(index as u64, se.io.reader_io_position());
    if let Some(msg) = st.repr() {
        out!("restart {index} {msg}");
        return false;
    }
    true
}

/// Port of `SkWuffsCodec::decodeFrameConfig` as the C driver's `decode_frame_config_loop`.
/// Returns the status and its text, where the text is `incomplete` when input ran out.
fn decode_frame_config_loop(se: &mut Session<'_>, fc: &mut FrameConfig) -> (Status, &'static str) {
    let st = loop {
        let st = se.dec.decode_frame_config(Some(&mut *fc), &mut se.io);
        if st.repr() == Some(strings::SUSPENSION_SHORT_READ) && retry(se) {
            continue;
        }
        break st;
    };
    se.suspended = !st.is_complete();
    let text = if st.repr() == Some(strings::SUSPENSION_SHORT_READ) {
        "incomplete"
    } else {
        status_text(st)
    };
    (st, text)
}

fn print_frame_config(index: usize, fc: &FrameConfig) {
    let r = fc.bounds;
    out!(
        "frame {index} ok bounds={},{},{},{} dur={} idx={} io={} disposal={} opaque={} overwrite={} bg={:08x}",
        r.min_incl_x,
        r.min_incl_y,
        r.max_excl_x,
        r.max_excl_y,
        fc.duration,
        fc.index,
        fc.io_position,
        fc.disposal.repr(),
        u8::from(fc.opaque_within_bounds),
        u8::from(fc.overwrite_instead_of_blend),
        fc.background_color,
    );
}

/// Port of `run_config`. Returns `ok`, `incomplete`, `invalid`, `seek` or the failure's message.
fn run_config(se: &mut Session<'_>) -> &'static str {
    let mut imgcfg = ImageConfig::default();
    loop {
        let st = se.dec.decode_image_config(Some(&mut imgcfg), &mut se.io);
        let Some(msg) = st.repr() else {
            break;
        };
        if msg != strings::SUSPENSION_SHORT_READ {
            out!("image {msg}");
            return msg;
        }
        if !retry(se) {
            out!("image incomplete");
            return "incomplete";
        }
    }
    se.width = imgcfg.pixcfg.width;
    se.height = imgcfg.pixcfg.height;
    se.first_io = imgcfg.first_frame_io_position;
    if se.width == 0 || se.height == 0 {
        out!("image invalid dimensions");
        return "invalid";
    }
    out!(
        "image ok width={} height={} first_io={} opaque={} pixfmt={:08x}",
        se.width,
        se.height,
        se.first_io,
        u8::from(imgcfg.first_frame_is_opaque),
        imgcfg.pixcfg.pixfmt.0,
    );

    if !seek_frame(se, 0) {
        return "seek";
    }
    for k in 0..MAX_FRAMES {
        let mut fc = FrameConfig::default();
        let (fst, text) = decode_frame_config_loop(se, &mut fc);
        if fst.repr().is_none() {
            print_frame_config(k, &fc);
            se.frame_io[k] = fc.io_position;
            se.frame_count = k + 1;
            continue;
        }
        if fst.repr() == Some(strings::NOTE_END_OF_DATA) {
            out!("frame {k} end");
        } else {
            out!("frame {k} {text}");
        }
        break;
    }
    out!(
        "frames n={} loops={}",
        se.frame_count,
        se.dec.num_animation_loops()
    );
    "ok"
}

/// Port of `run_decode`.
fn run_decode(se: &mut Session<'_>) {
    const FMTS: [(u32, &str, usize); 3] = [
        (0x8100_8888, "bgra", 4),
        (0xA100_8888, "rgba", 4),
        (0x8000_0565, "565", 2),
    ];
    let n = se.frame_count.min(MAX_DECODED_FRAMES);
    se.frame0_status = "skip";
    se.frame0_hash = 0;
    for i in 0..n {
        for (c, (fmt_repr, fmt_name, bpp)) in FMTS.into_iter().enumerate() {
            let blend = if i == 0 {
                PixelBlend::Src
            } else {
                PixelBlend::SrcOver
            };
            let blend_name = if i == 0 { "src" } else { "srcover" };
            if !seek_frame(se, i) {
                continue;
            }
            let mut fc = FrameConfig::default();
            let (fst, text) = decode_frame_config_loop(se, &mut fc);
            out!("decode {i} {fmt_name} {blend_name} frameconfig={text}");
            if fst.repr().is_some() {
                continue;
            }
            let width = se.width as usize;
            let height = se.height as usize;
            let stride = width * bpp;
            let mut pixels = vec![0u8; stride * height];
            let mut pc = PixelConfig::default();
            pc.set(fmt_repr, 0, se.width, se.height);
            let st = {
                let mut pb = PixelBuffer::null();
                let pst = pb.set_from_table(
                    &pc,
                    Table {
                        data: &mut pixels[..],
                        width: width * bpp,
                        height,
                        stride,
                    },
                );
                if let Some(msg) = pst.repr() {
                    out!("set_from_table {msg}");
                    continue;
                }
                loop {
                    let st = se.dec.decode_frame(&mut pb, &mut se.io, blend);
                    if st.repr() == Some(strings::SUSPENSION_SHORT_READ) && retry(se) {
                        continue;
                    }
                    break st;
                }
            };
            se.suspended = !st.is_complete();
            let status = if st.repr() == Some(strings::SUSPENSION_SHORT_READ) {
                "incomplete"
            } else {
                status_text(st)
            };
            let hash = fnv1a(&pixels);
            if i == 0 && c == 0 {
                se.frame0_status = status;
                se.frame0_hash = hash;
            }
            let d = se.dec.frame_dirty_rect();
            out!(
                "decode {i} {fmt_name} {blend_name} status={status} hash={hash:016x} dirty={},{},{},{} nconf={} nframes={}",
                d.min_incl_x,
                d.min_incl_y,
                d.max_excl_x,
                d.max_excl_y,
                se.dec.num_decoded_frame_configs(),
                se.dec.num_decoded_frames(),
            );
        }
    }
}

/// Port of `run_variant`.
fn run_variant(name: &str, data: &[u8], chunk: usize, variant: &str, closed_on_eof: bool) {
    out!(
        "== {name} chunk={chunk} variant={variant} len={}",
        data.len()
    );
    let mut se = Session::new(data, chunk, closed_on_eof);
    run_config(&mut se);
    run_decode(&mut se);
}

/// Port of `sweep_cut`: one line per truncation point.
fn sweep_cut(data: &[u8], cut: usize, closed_on_eof: bool) {
    QUIET.with(|q| q.set(true));
    let mut se = Session::new(&data[..cut], 4096, closed_on_eof);
    let cfg = run_config(&mut se);
    if cfg == "ok" && se.frame_count > 0 {
        run_decode(&mut se);
    }
    QUIET.with(|q| q.set(false));
    out!(
        "cut {cut} closed={} config={cfg} frame0={} hash={:016x}",
        u8::from(closed_on_eof),
        se.frame0_status,
        se.frame0_hash,
    );
}

/// Dumps every file in `files` (`(basename, bytes)`), in order, as the C driver does, and returns
/// the text.
#[must_use]
pub fn dump(files: &[(&str, Vec<u8>)]) -> String {
    OUTPUT.with(|o| o.borrow_mut().clear());
    for (base, data) in files {
        // One byte at a time for small files (every suspension point), then 7 bytes, then a full
        // 4096-byte read.
        let chunks: [usize; 3] = [1, 7, 4096];
        let nchunks = if data.len() <= 8192 {
            3
        } else if data.len() <= 102_400 {
            2
        } else {
            1
        };
        for &chunk in &chunks[..nchunks] {
            run_variant(base, data, chunk, "full", false);
            if chunk == 4096 {
                let third = data.len() / 3;
                run_variant(base, &data[..third], chunk, "trunc1of3", false);
                run_variant(base, &data[..third], chunk, "trunc1of3closed", true);
                run_variant(
                    base,
                    &data[..(2 * data.len()) / 3],
                    chunk,
                    "trunc2of3closed",
                    true,
                );
                if !data.is_empty() {
                    let mut corrupt = data.clone();
                    corrupt[data.len() / 2] ^= 0x5A;
                    run_variant(base, &corrupt, chunk, "corrupt", false);
                }
            }
        }
        if data.len() <= SWEEP_MAX_LEN {
            out!("== {base} sweep len={}", data.len());
            for cut in 0..=data.len() {
                sweep_cut(data, cut, false);
                sweep_cut(data, cut, true);
            }
        }
    }
    OUTPUT.with(|o| o.borrow().clone())
}
