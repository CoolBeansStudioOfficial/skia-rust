//! Replays the demuxer corpus (`corpus/` and `corpus-anim/` of `oracle/codec-diff/libwebp`,
//! with the animated and truncated fixtures of `make_anim.py`) through `demux_internal` in the
//! complete and the partial mode, and compares the result line by line with
//! `expected_demux.txt`, the output of `webpdemux.c` (built from the pinned libwebp 1.4.0).

mod common;

use std::fmt::Write as _;

use common::oracle_dir;
use skia_rust_libwebp::demux::{ChunkIterator, DemuxState, Demuxer, FrameIter, demux_internal};

/// FNV-1a 64 over `bytes`, as `Fnv1a` in `webpdemux.c`.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 1_469_598_103_934_665_603;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(1_099_511_628_211);
    }
    h
}

/// The `WebPDemuxState` numbering of `webp/demux.h`.
fn state_code(s: DemuxState) -> i32 {
    match s {
        DemuxState::ParseError => -1,
        DemuxState::ParsingHeader => 0,
        DemuxState::ParsedHeader => 1,
        DemuxState::Done => 2,
    }
}

/// The lines `webpdemux.c` prints for one mode of one file.
fn report(out: &mut String, base: &str, mode: &str, data: &[u8], partial: bool) {
    let (parsed, state) = demux_internal(data, partial);
    let Some(dmux) = parsed else {
        writeln!(out, "{base} {mode} state={} demux=0", state_code(state)).expect("write");
        return;
    };
    writeln!(
        out,
        "{base} {mode} state={} demux=1 flags={} canvas={}x{} frames={} loop={} bgcolor={:08x}",
        state_code(state),
        dmux.get_i(skia_rust_libwebp::demux::FormatFeature::FormatFlags),
        dmux.get_i(skia_rust_libwebp::demux::FormatFeature::CanvasWidth),
        dmux.get_i(skia_rust_libwebp::demux::FormatFeature::CanvasHeight),
        dmux.get_i(skia_rust_libwebp::demux::FormatFeature::FrameCount),
        dmux.get_i(skia_rust_libwebp::demux::FormatFeature::LoopCount),
        dmux.get_i(skia_rust_libwebp::demux::FormatFeature::BackgroundColor),
    )
    .expect("write");
    let mut frame = dmux.get_frame(1);
    while let Some(f) = frame {
        write_frame(out, base, mode, &f);
        frame = f.next_frame();
    }
    let last = dmux.get_frame(0).map_or(-1, |f| f.frame_num);
    writeln!(out, "{base} {mode} last frame={last}").expect("write");
    write_chunks(out, base, mode, &dmux, "ICCP");
    write_chunks(out, base, mode, &dmux, "EXIF");
    write_chunks(out, base, mode, &dmux, "XMP ");
}

fn write_frame(out: &mut String, base: &str, mode: &str, f: &FrameIter<'_, '_>) {
    writeln!(
        out,
        "{base} {mode} frame={} x={} y={} w={} h={} dur={} dispose={} blend={} complete={} \
         alpha={} size={} fnv={:016x}",
        f.frame_num,
        f.x_offset,
        f.y_offset,
        f.width,
        f.height,
        f.duration,
        f.dispose_method as i32,
        f.blend_method as i32,
        i32::from(f.complete),
        i32::from(f.has_alpha),
        f.fragment.len(),
        fnv1a(f.fragment),
    )
    .expect("write");
}

fn write_chunks(out: &mut String, base: &str, mode: &str, dmux: &Demuxer<'_>, fourcc: &str) {
    let tag: [u8; 4] = fourcc.as_bytes().try_into().expect("four-byte FourCC");
    let mut chunk: Option<ChunkIterator<'_, '_>> = dmux.get_chunk(tag, 1);
    while let Some(c) = chunk {
        writeln!(
            out,
            "{base} {mode} chunk={fourcc} {}/{} size={} fnv={:016x}",
            c.chunk_num,
            c.num_chunks,
            c.chunk.len(),
            fnv1a(c.chunk),
        )
        .expect("write");
        chunk = c.next_chunk();
    }
}

/// Finds a fixture by its base name in the corpus directories.
fn read_fixture(base: &str) -> Vec<u8> {
    let dir = oracle_dir();
    for sub in ["corpus", "corpus-anim"] {
        let path = dir.join(sub).join(base);
        if let Ok(data) = std::fs::read(&path) {
            return data;
        }
    }
    panic!("no fixture {base}");
}

#[test]
fn demux_replay_matches_c_reference() {
    let expected = std::fs::read_to_string(oracle_dir().join("expected_demux.txt"))
        .expect("read expected_demux.txt");
    // The files, in the order they appear in the expected output.
    let mut files: Vec<&str> = Vec::new();
    for l in expected.lines() {
        let base = l.split(' ').next().expect("basename");
        if !files.contains(&base) {
            files.push(base);
        }
    }
    let mut actual = String::new();
    for base in files {
        let data = read_fixture(base);
        report(&mut actual, base, "full", &data, false);
        report(&mut actual, base, "partial", &data, true);
    }
    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    let mismatches: Vec<String> = expected_lines
        .iter()
        .zip(actual_lines.iter())
        .filter(|(e, a)| e != a)
        .take(20)
        .map(|(e, a)| format!("expected: {e}\n  actual: {a}"))
        .collect();
    assert!(
        mismatches.is_empty() && expected_lines.len() == actual_lines.len(),
        "{} of {} lines differ (first 20 shown; {} vs {} lines):\n{}",
        expected_lines.len().max(actual_lines.len()) - mismatches.len(),
        expected_lines.len(),
        expected_lines.len(),
        actual_lines.len(),
        mismatches.join("\n")
    );
}
