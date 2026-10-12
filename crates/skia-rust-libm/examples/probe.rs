//! Black-box probe of the platform math library (the UCRT on Windows).
//!
//! `probe jobs <jobs-file> <outdir>` evaluates the platform function through `std` (which calls
//! the platform CRT on `*-windows-msvc`) on reproducible inputs and writes one `<tag>.bin` per job:
//! 24-byte little-endian records `(a, b, result)` holding raw bit patterns (f32 values are
//! zero-extended to u64). A job line is
//!
//! `tag fn n xmode xlo xhi [ymode ylo yhi]`
//!
//! `probe check <outdir> <fn>...` compares the crate's implementation against the platform
//! function on every f32 bit pattern (unary) and writes `<fn>.mismatch` (24-byte records
//! `(x bits, platform bits, ours bits)`) plus a summary on stdout.
//!
//! where `fn` is a C name (`sinf`, `cos`, ...), `xmode` is `lin` (uniform in value), `bits`
//! (uniform over the bit patterns between `xlo` and `xhi`, both of the same sign) or `symbits`
//! (as `bits` between `|xlo|` and `|xhi|` with a random sign). Clean-room evidence:
//! docs/design/libm-cleanroom.md.
// Probe tool, not ported arithmetic: the casts build test inputs; short names are math variables.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    clippy::cast_possible_wrap,
    clippy::float_cmp,
    clippy::type_complexity
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

struct Dist {
    mode: String,
    lo: f64,
    hi: f64,
}
impl Dist {
    fn parse(w: &[&str]) -> Dist {
        Dist {
            mode: w[0].to_string(),
            lo: w[1].parse().unwrap(),
            hi: w[2].parse().unwrap(),
        }
    }
    fn sample(&self, r: &mut Rng, is32: bool) -> f64 {
        match self.mode.as_str() {
            "lin" => self.lo + (self.hi - self.lo) * r.unit(),
            "bits" | "symbits" | "all" => {
                let sym = self.mode == "symbits";
                let (lo, hi) = if sym {
                    (self.lo.abs(), self.hi.abs())
                } else {
                    (self.lo, self.hi)
                };
                let neg = lo < 0.0 || (sym && r.next() & 1 == 1);
                let (a, b) = (lo.abs(), hi.abs());
                let (a, b) = if a <= b { (a, b) } else { (b, a) };
                let v = if is32 {
                    let (ia, ib) = ((a as f32).to_bits(), (b as f32).to_bits());
                    let span = u64::from(ib - ia) + 1;
                    f64::from(f32::from_bits(ia + (r.next() % span) as u32))
                } else {
                    let (ia, ib) = (a.to_bits(), b.to_bits());
                    f64::from_bits(ia + r.next() % (ib - ia + 1))
                };
                if neg { -v } else { v }
            }
            m => panic!("bad mode {m}"),
        }
    }
}

fn rec(out: &mut impl Write, a: u64, b: u64, c: u64) {
    out.write_all(&a.to_le_bytes()).unwrap();
    out.write_all(&b.to_le_bytes()).unwrap();
    out.write_all(&c.to_le_bytes()).unwrap();
}

fn un32(name: &str) -> fn(f32) -> f32 {
    match name {
        "sinf" => f32::sin,
        "cosf" => f32::cos,
        "tanf" => f32::tan,
        "asinf" => f32::asin,
        "acosf" => f32::acos,
        "atanf" => f32::atan,
        "expf" => f32::exp,
        "logf" => f32::ln,
        "log2f" => f32::log2,
        "cbrtf" => f32::cbrt,
        _ => panic!("unknown f32 function {name}"),
    }
}
fn un64(name: &str) -> fn(f64) -> f64 {
    match name {
        "sin" => f64::sin,
        "cos" => f64::cos,
        "acos" => f64::acos,
        "cbrt" => f64::cbrt,
        "exp" => f64::exp,
        "exp2" => f64::exp2,
        "log" => f64::ln,
        _ => panic!("unknown f64 function {name}"),
    }
}

fn run_job(line: &str, dir: &str) {
    let w: Vec<&str> = line.split_whitespace().collect();
    let (tag, name, n) = (w[0], w[1], w[2].parse::<usize>().unwrap());
    let xd = Dist::parse(&w[3..6]);
    let yd = (w.len() >= 9).then(|| Dist::parse(&w[6..9]));
    let mut r = Rng(0x1234_5678_9ABC_DEF0 ^ n as u64 ^ (tag.len() as u64) << 40);
    let is32 = name.ends_with('f');
    if xd.mode == "all" {
        // every non-negative f32 from bits(lo) to bits(hi): a bare array of u32 results
        let f = un32(name);
        let mut out = writer_at(&format!("{dir}/{tag}.all"));
        for b in (xd.lo as f32).to_bits()..=(xd.hi as f32).to_bits() {
            let z = f(std::hint::black_box(f32::from_bits(b)));
            out.write_all(&z.to_bits().to_le_bytes()).unwrap();
        }
        return;
    }
    if is32 && yd.is_none() && name != "powf" && name != "atan2f" {
        // compact: (x bits, result bits) as u32 pairs
        let f = un32(name);
        let mut out = writer_at(&format!("{dir}/{tag}.xr"));
        for _ in 0..n {
            let x = xd.sample(&mut r, true) as f32;
            let z = f(std::hint::black_box(x));
            out.write_all(&x.to_bits().to_le_bytes()).unwrap();
            out.write_all(&z.to_bits().to_le_bytes()).unwrap();
        }
        return;
    }
    let mut out = writer_at(&format!("{dir}/{tag}.bin"));
    for _ in 0..n {
        let x = xd.sample(&mut r, is32);
        let y = yd.as_ref().map_or(0.0, |d| d.sample(&mut r, is32));
        if is32 {
            let (x, y) = (x as f32, y as f32);
            let z = match name {
                "atan2f" => std::hint::black_box(x).atan2(std::hint::black_box(y)),
                "powf" => std::hint::black_box(x).powf(std::hint::black_box(y)),
                _ => un32(name)(std::hint::black_box(x)),
            };
            rec(
                &mut out,
                u64::from(x.to_bits()),
                u64::from(y.to_bits()),
                u64::from(z.to_bits()),
            );
        } else {
            let z = match name {
                "pow" => std::hint::black_box(x).powf(std::hint::black_box(y)),
                _ => un64(name)(std::hint::black_box(x)),
            };
            rec(&mut out, x.to_bits(), y.to_bits(), z.to_bits());
        }
    }
}

fn ours32(name: &str) -> Option<fn(f32) -> f32> {
    Some(match name {
        "sinf" => skia_rust_libm::sinf,
        "cosf" => skia_rust_libm::cosf,
        "tanf" => skia_rust_libm::tanf,
        _ => return None,
    })
}

type Mismatch = (u32, u32, u32);

fn check32(name: &str, dir: &str) {
    let (Some(ours), platform) = (ours32(name), un32(name)) else {
        println!("{name}: no implementation");
        return;
    };
    let threads = std::thread::available_parallelism().map_or(4, usize::from);
    let chunk = (1u64 << 32).div_ceil(threads as u64);
    let results: Vec<(u64, Vec<Mismatch>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads as u64)
            .map(|t| {
                sc.spawn(move || {
                    let (lo, hi) = (t * chunk, ((t + 1) * chunk).min(1 << 32));
                    let mut count = 0u64;
                    let mut list = Vec::new();
                    for b in lo..hi {
                        let x = f32::from_bits(b as u32);
                        let (p, o) = (
                            platform(std::hint::black_box(x)).to_bits(),
                            ours(x).to_bits(),
                        );
                        if p != o {
                            count += 1;
                            if list.len() < 2_000_000 / threads {
                                list.push((b as u32, p, o));
                            }
                        }
                    }
                    (count, list)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let total: u64 = results.iter().map(|r| r.0).sum();
    let mut all: Vec<Mismatch> = results.into_iter().flat_map(|r| r.1).collect();
    all.sort_unstable();
    println!("{name}: {total} mismatches of 4294967296 bit patterns");
    let ord = |b: u32| -> i64 {
        if b >> 31 == 1 {
            -i64::from(b & 0x7FFF_FFFF)
        } else {
            i64::from(b)
        }
    };
    let far = all
        .iter()
        .filter(|&&(x, p, o)| {
            f32::from_bits(x).is_finite()
                && !f32::from_bits(p).is_nan()
                && (ord(p) - ord(o)).abs() > 1
        })
        .count();
    println!(
        "  of the listed mismatches, {far} differ by more than 1 ulp (or are NaN-ness differences)"
    );
    let mut hist = std::collections::BTreeMap::new();
    for &(x, _, _) in &all {
        *hist.entry((x >> 23) & 0x1ff).or_insert(0u32) += 1;
    }
    println!("  by sign+exponent (biased, 0x100 = negative): {hist:?}");
    for &(x, p, o) in all.iter().take(12) {
        println!(
            "  x={x:#010x} ({:e}) platform={p:#010x} ours={o:#010x}",
            f32::from_bits(x)
        );
    }
    let mut out = writer_at(&format!("{dir}/{name}.mismatch"));
    for (x, p, o) in all {
        rec(&mut out, u64::from(x), u64::from(p), u64::from(o));
    }
}

/// For each binade, finds the inputs where the quadrant choice (double-precision product vs exact
/// expansion) changes the result, and counts which side the platform follows.
fn nsel(name: &str) {
    let (platform, var): (fn(f32) -> f32, fn(f32, f64, bool) -> f32) = match name {
        "sinf" => (f32::sin, skia_rust_libm::probe_sinf),
        _ => (f32::cos, skia_rust_libm::probe_cosf),
    };
    for exp in 126u32..=190 {
        let (mut d, mut dbl, mut exact, mut neither, mut fused) = (0u32, 0u32, 0u32, 0u32, 0u32);
        let mut sample = Vec::new();
        for mant in 0u32..(1 << 23) {
            let x = f32::from_bits((exp << 23) | mant);
            if f64::from(x) <= 0.8 {
                continue;
            }
            let a = var(x, f64::INFINITY, false);
            let b = var(x, 0.0, false);
            let c = var(x, f64::INFINITY, true);
            if a == b && c == b {
                continue;
            }
            d += 1;
            let p = platform(std::hint::black_box(x));
            if p == a {
                dbl += 1;
            } else if p == b {
                exact += 1;
            } else {
                neither += 1;
            }
            if a != c {
                fused += 1;
            }
            if sample.len() < 3 {
                sample.push(format!(
                    "{:#x}:p={:#x},dbl={:#x},ex={:#x},fma={:#x}",
                    x.to_bits(),
                    p.to_bits(),
                    a.to_bits(),
                    b.to_bits(),
                    c.to_bits()
                ));
            }
        }
        println!(
            "{name} binade {} ({:e}): differing {d}: platform=double-n {dbl}, =exact-n {exact}, neither {neither}; fma-n differs {fused} {sample:?}",
            exp as i32 - 127,
            f64::from(f32::from_bits(exp << 23))
        );
    }
}

fn writer_at(path: &str) -> std::io::BufWriter<std::fs::File> {
    std::io::BufWriter::new(std::fs::File::create(path).unwrap())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "nsel" {
        for name in &args[2..] {
            nsel(name);
        }
        return;
    }
    if args.len() >= 4 && args[1] == "check" {
        std::fs::create_dir_all(&args[2]).unwrap();
        for name in &args[3..] {
            check32(name, &args[2]);
        }
        return;
    }
    if args.len() < 4 || args[1] != "jobs" {
        eprintln!("usage: probe jobs <jobs-file> <outdir>");
        return;
    }
    std::fs::create_dir_all(&args[3]).unwrap();
    let text = std::fs::read_to_string(&args[2]).unwrap();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        run_job(line, &args[3]);
    }
}
