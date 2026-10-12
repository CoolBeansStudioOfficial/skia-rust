//! Black-box probe of the platform math library (the UCRT on Windows).
//!
//! `probe dump <outdir> <n>` evaluates every function the port needs on `n` reproducible inputs
//! per function through `std` (which calls the platform CRT on `*-windows-msvc`) and writes
//! `<fn>.bin`: 24-byte little-endian records `(a, b, result)` holding raw bit patterns (f32
//! values are zero-extended to u64). Clean-room evidence: docs/design/libm-cleanroom.md.
// Probe tool, not ported arithmetic: the casts build test inputs; short names are math variables.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::many_single_char_names
)]

use std::io::Write;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Input distributions: any finite bit pattern, uniform in [-lim, lim], log-uniform magnitude.
fn f32_in(r: &mut Rng, lim: f32, positive: bool) -> f32 {
    let v = match r.next() % 3 {
        0 => loop {
            let f = f32::from_bits(r.next() as u32);
            if f.is_finite() && f.abs() <= lim * 1e6 {
                break f;
            }
        },
        1 => ((r.unit() * 2.0 - 1.0) as f32) * lim,
        _ => {
            let e = (r.unit() * 40.0 - 20.0) as f32;
            let s = if r.next() & 1 == 0 { 1.0 } else { -1.0 };
            s * 2f32.powf(e).min(lim)
        }
    };
    if positive { v.abs() } else { v }
}
fn f64_in(r: &mut Rng, lim: f64, positive: bool) -> f64 {
    let v = match r.next() % 3 {
        0 => loop {
            let f = f64::from_bits(r.next());
            if f.is_finite() && f.abs() <= lim * 1e6 {
                break f;
            }
        },
        1 => (r.unit() * 2.0 - 1.0) * lim,
        _ => {
            let e = r.unit() * 80.0 - 40.0;
            let s = if r.next() & 1 == 0 { 1.0 } else { -1.0 };
            s * 2f64.powf(e).min(lim)
        }
    };
    if positive { v.abs() } else { v }
}

fn writer(dir: &str, name: &str) -> std::io::BufWriter<std::fs::File> {
    std::io::BufWriter::new(std::fs::File::create(format!("{dir}/{name}.bin")).unwrap())
}
fn rec(out: &mut impl Write, a: u64, b: u64, c: u64) {
    out.write_all(&a.to_le_bytes()).unwrap();
    out.write_all(&b.to_le_bytes()).unwrap();
    out.write_all(&c.to_le_bytes()).unwrap();
}

fn run1f(name: &str, dir: &str, n: usize, lim: f32, pos: bool, f: fn(f32) -> f32) {
    let mut r = Rng(0x1234_5678 ^ name.len() as u64);
    let mut out = writer(dir, name);
    for _ in 0..n {
        let x = f32_in(&mut r, lim, pos);
        let y = f(std::hint::black_box(x));
        rec(&mut out, u64::from(x.to_bits()), 0, u64::from(y.to_bits()));
    }
}
fn run1d(name: &str, dir: &str, n: usize, lim: f64, pos: bool, f: fn(f64) -> f64) {
    let mut r = Rng(0x8765_4321 ^ name.len() as u64);
    let mut out = writer(dir, name);
    for _ in 0..n {
        let x = f64_in(&mut r, lim, pos);
        let y = f(std::hint::black_box(x));
        rec(&mut out, x.to_bits(), 0, y.to_bits());
    }
}
fn run2f(name: &str, dir: &str, n: usize, f: fn(f32, f32) -> f32, g: fn(&mut Rng) -> (f32, f32)) {
    let mut r = Rng(0xABCD_EF01 ^ name.len() as u64);
    let mut out = writer(dir, name);
    for _ in 0..n {
        let (x, y) = g(&mut r);
        let z = f(std::hint::black_box(x), std::hint::black_box(y));
        rec(
            &mut out,
            u64::from(x.to_bits()),
            u64::from(y.to_bits()),
            u64::from(z.to_bits()),
        );
    }
}
fn run2d(name: &str, dir: &str, n: usize, f: fn(f64, f64) -> f64, g: fn(&mut Rng) -> (f64, f64)) {
    let mut r = Rng(0x0F0F_1234 ^ name.len() as u64);
    let mut out = writer(dir, name);
    for _ in 0..n {
        let (x, y) = g(&mut r);
        let z = f(std::hint::black_box(x), std::hint::black_box(y));
        rec(&mut out, x.to_bits(), y.to_bits(), z.to_bits());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 || args[1] != "dump" {
        eprintln!("usage: probe dump <outdir> <n>");
        return;
    }
    let dir = args[2].as_str();
    let n: usize = args[3].parse().unwrap();
    std::fs::create_dir_all(dir).unwrap();
    run1f("sinf", dir, n, 100.0, false, f32::sin);
    run1f("cosf", dir, n, 100.0, false, f32::cos);
    run1f("tanf", dir, n, 100.0, false, f32::tan);
    run1f("asinf", dir, n, 1.0, false, f32::asin);
    run1f("acosf", dir, n, 1.0, false, f32::acos);
    run1f("atanf", dir, n, 100.0, false, f32::atan);
    run1f("expf", dir, n, 100.0, false, f32::exp);
    run1f("logf", dir, n, 1000.0, true, f32::ln);
    run1f("log2f", dir, n, 1000.0, true, f32::log2);
    run1f("cbrtf", dir, n, 1000.0, false, f32::cbrt);
    run2f("atan2f", dir, n, f32::atan2, |r| {
        (f32_in(r, 100.0, false), f32_in(r, 100.0, false))
    });
    run2f("powf", dir, n, f32::powf, |r| {
        (f32_in(r, 50.0, true), (r.unit() * 40.0 - 20.0) as f32)
    });
    run1d("sin", dir, n, 100.0, false, f64::sin);
    run1d("cos", dir, n, 100.0, false, f64::cos);
    run1d("acos", dir, n, 1.0, false, f64::acos);
    run1d("cbrt", dir, n, 1000.0, false, f64::cbrt);
    run1d("exp", dir, n, 100.0, false, f64::exp);
    run1d("exp2", dir, n, 100.0, false, f64::exp2);
    run1d("log", dir, n, 1000.0, true, f64::ln);
    run2d("pow", dir, n, f64::powf, |r| {
        (f64_in(r, 50.0, true), r.unit() * 40.0 - 20.0)
    });
}
