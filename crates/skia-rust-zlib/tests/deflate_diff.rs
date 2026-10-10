// Copyright 1995-2023 Jean-loup Gailly and Mark Adler (zlib); port by The skia-rust Authors.
// Use of this source code is governed by the zlib licence in the LICENSE file.
// Port of: the call pattern and corpus of oracle/codec-diff/zlib/zlibdump.c (chromium
// zlib@646b7f56), replayed against the Rust deflate. The expected output is
// oracle/codec-diff/zlib/expected.txt, printed by the C harness built from the pinned sources.

// The corpus generator and the case table mirror zlibdump.c line for line, so the C names (`b`,
// `i`, `n`, `v`), the C integer conversions and the C literal arithmetic are kept as they are.
#![allow(clippy::many_single_char_names)] // zlibdump.c's names
#![allow(clippy::cast_possible_truncation)] // C's unsigned byte and int conversions
#![allow(clippy::manual_is_multiple_of)] // zlibdump.c's `rnd() % n == 0`

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use skia_rust_zlib::{Deflate, Flush, Inflate, ReturnCode};

/// Port of the `rng_state` of zlibdump.c: a 32-bit LCG that is reseeded by each generator call.
struct Rng(u32);

impl Rng {
    /// Port of `rnd`.
    fn rnd(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (self.0 >> 16) & 0x7fff
    }
}

/// Port of `kWords`.
const WORDS: [&str; 15] = [
    "the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog", "zlib", "deflate", "png",
    "hash", "chain", "match", "literal",
];

/// Port of `gen_input`: the corpus of kind 0 (text), 1 (random), 2 (runs), 3 (gradient rows) and
/// 4 (mixed segments). The generator state is shared with its recursive calls, as in C.
fn gen_input(rng: &mut Rng, kind: u32, n: usize, seed: u32) -> Vec<u8> {
    let mut b = vec![0u8; n];
    let mut i = 0usize;
    rng.0 = seed;
    match kind {
        0 => {
            while i < n {
                let w = WORDS[(rng.rnd() % 15) as usize];
                for &c in w.as_bytes() {
                    if i >= n {
                        break;
                    }
                    b[i] = c;
                    i += 1;
                }
                if i < n {
                    b[i] = if rng.rnd().is_multiple_of(10) {
                        b'\n'
                    } else {
                        b' '
                    };
                    i += 1;
                }
            }
        }
        1 => {
            for x in &mut b {
                *x = (rng.rnd() & 0xff) as u8;
            }
        }
        2 => {
            while i < n {
                let v = (rng.rnd() % 4) as u8;
                let run = 1 + rng.rnd() as usize % 300;
                for _ in 0..run {
                    if i >= n {
                        break;
                    }
                    b[i] = v;
                    i += 1;
                }
            }
        }
        3 => {
            for (i, x) in b.iter_mut().enumerate() {
                let xx = i % 257;
                let y = i / 257;
                let mut v = (xx + y * 3) as u32;
                if rng.rnd().is_multiple_of(4) {
                    v += 1;
                }
                *x = v as u8;
            }
        }
        _ => {
            let mut seg: u32 = 0;
            let mut k: u32 = 0;
            while i < n {
                let mut len = 500 + rng.rnd() as usize % 3000;
                if len > n - i {
                    len = n - i;
                }
                let sub_kind = match k % 3 {
                    0 => 0,
                    1 => 2,
                    _ => 1,
                };
                let part = gen_input(rng, sub_kind, len, seed.wrapping_add(seg));
                b[i..i + len].copy_from_slice(&part);
                i += len;
                seg += 1;
                k += 1;
            }
        }
    }
    b
}

/// Port of `fnv1a`.
fn fnv1a(p: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &x in p {
        h ^= u64::from(x);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// The parameters of one case.
#[derive(Clone, Copy)]
struct Params {
    level: i32,
    wbits: i32,
    mem: i32,
    strategy: i32,
    feed: usize,
    obsz: usize,
}

/// Port of `compress_case`: the call pattern of the C harness. Input is fed `feed` bytes per call
/// with `Z_NO_FLUSH` into `obsz`-byte output buffers, then the stream is finished with `Z_FINISH`.
fn compress_case(input: &[u8], p: Params) -> Vec<u8> {
    let mut d =
        Deflate::new(p.level, p.wbits, p.mem, p.strategy).expect("valid deflate parameters");
    let mut out = Vec::new();
    let mut obuf = vec![0u8; p.obsz];
    let mut pos = 0usize;
    while pos < input.len() {
        let take = (input.len() - pos).min(p.feed);
        let mut chunk = &input[pos..pos + take];
        pos += take;
        loop {
            let r = d.deflate(chunk, &mut obuf, Flush::NoFlush);
            assert_eq!(r.ret, ReturnCode::Ok);
            out.extend_from_slice(&obuf[..r.produced]);
            chunk = &chunk[r.consumed..];
            if chunk.is_empty() && r.produced != p.obsz {
                break;
            }
        }
    }
    loop {
        let r = d.deflate(&[], &mut obuf, Flush::Finish);
        out.extend_from_slice(&obuf[..r.produced]);
        if r.ret != ReturnCode::Ok {
            assert_eq!(r.ret, ReturnCode::StreamEnd);
            break;
        }
    }
    out
}

/// Port of `run_case`: compresses one generated input, checks the stream by inflating it, and
/// appends the line the C harness prints.
fn run_case(out: &mut String, kind: u32, n: usize, seed: u32, p: Params) {
    let mut rng = Rng(0);
    let input = gen_input(&mut rng, kind, n, seed);
    let compressed = compress_case(&input, p);

    // The stream must decode to the input (an independent check of the port's output).
    let mut inflate = Inflate::new(15).expect("inflate window");
    let mut decoded = vec![0u8; n.max(1)];
    let r = inflate.inflate(&compressed, &mut decoded, Flush::Finish);
    assert_eq!(
        r.ret,
        ReturnCode::StreamEnd,
        "in={kind}:{n} lvl={}",
        p.level
    );
    assert_eq!(
        &decoded[..r.produced],
        &input[..],
        "in={kind}:{n} lvl={}",
        p.level
    );

    writeln!(
        out,
        "in={kind}:{n} lvl={} wbits={} mem={} strat={} feed={} obuf={} len={} fnv={:016x}",
        p.level,
        p.wbits,
        p.mem,
        p.strategy,
        p.feed,
        p.obsz,
        compressed.len(),
        fnv1a(&compressed),
    )
    .expect("write to string");
}

/// The sizes of zlibdump.c's `kSizes`.
const SIZES: [usize; 7] = [0, 1, 3, 300, 4096, 40000, 70000];

/// Port of `main` of zlibdump.c: the same cases, in the same order.
fn dump() -> String {
    let mut out = String::new();
    let p = |level, wbits, mem, strategy, feed, obsz| Params {
        level,
        wbits,
        mem,
        strategy,
        feed,
        obsz,
    };
    // A: every level, default strategy and window, chunked calls.
    for kind in 0..5u32 {
        for (si, &size) in SIZES.iter().enumerate() {
            for level in 0..=9 {
                run_case(
                    &mut out,
                    kind,
                    size,
                    1000 + (kind * 17 + si as u32),
                    p(level, 15, 8, 0, 4096, 8192),
                );
            }
        }
    }
    // B: one whole-input call per case, at levels 6 and 9.
    for kind in 0..5u32 {
        for (si, &size) in SIZES.iter().enumerate() {
            let seed = 2000 + (kind * 17 + si as u32);
            let feed = size.max(1);
            run_case(&mut out, kind, size, seed, p(6, 15, 8, 0, feed, 1 << 20));
            run_case(&mut out, kind, size, seed, p(9, 15, 8, 0, feed, 1 << 20));
        }
    }
    // C: strategies, memory levels and window sizes.
    for strategy in 1..=4 {
        run_case(&mut out, 0, 40000, 3000, p(6, 15, 8, strategy, 4096, 8192));
        run_case(&mut out, 3, 40000, 3001, p(1, 15, 8, strategy, 4096, 8192));
        run_case(&mut out, 2, 300, 3002, p(9, 15, 8, strategy, 4096, 8192));
    }
    run_case(&mut out, 0, 40000, 3100, p(6, 15, 1, 0, 4096, 8192));
    run_case(&mut out, 0, 40000, 3100, p(6, 15, 9, 0, 4096, 8192));
    run_case(&mut out, 3, 40000, 3101, p(4, 15, 5, 0, 4096, 8192));
    run_case(&mut out, 0, 40000, 3200, p(6, 9, 8, 0, 4096, 8192));
    run_case(&mut out, 3, 40000, 3201, p(6, 12, 8, 0, 4096, 8192));
    run_case(&mut out, 0, 70000, 3202, p(9, 9, 8, 0, 4096, 8192));
    // D: one byte per call, and small output buffers.
    run_case(&mut out, 0, 300, 4000, p(6, 15, 8, 0, 1, 8192));
    run_case(&mut out, 3, 300, 4001, p(9, 15, 8, 0, 1, 8192));
    run_case(&mut out, 1, 4096, 4002, p(1, 15, 8, 0, 4096, 7));
    run_case(&mut out, 0, 40000, 4003, p(6, 15, 8, 0, 4096, 1000));
    out
}

#[test]
fn deflate_matches_chromium_zlib_output() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expected_path = manifest.join("../../oracle/codec-diff/zlib/expected.txt");
    let expected = fs::read_to_string(&expected_path)
        .unwrap_or_else(|e| panic!("{}: {e}", expected_path.display()));
    let got = dump();
    if got != expected {
        let first = match got.lines().zip(expected.lines()).find(|(g, e)| g != e) {
            Some((g, e)) => format!("expected: {e}\n  actual: {g}"),
            None => "line counts differ".to_string(),
        };
        panic!("deflate output differs from the pinned zlib: {first}");
    }
}
