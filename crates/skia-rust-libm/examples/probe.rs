//! Black-box probe of the platform math library (the UCRT on Windows).
//!
//! `probe jobs <jobs-file> <outdir>` evaluates the platform function through `std` (which calls
//! the platform CRT on `*-windows-msvc`) on reproducible inputs and writes one `<tag>.bin` per job:
//! 24-byte little-endian records `(a, b, result)` holding raw bit patterns (f32 values are
//! zero-extended to u64). A job line is
//!
//! `tag fn n xmode xlo xhi [ymode ylo yhi]`
//!
//! where `fn` is a C name (`sinf`, `cos`, ...), `xmode` is `lin` (uniform in value), `bits`
//! (uniform over the bit patterns between `xlo` and `xhi`, both of the same sign) or `symbits`
//! (as `bits` between `|xlo|` and `|xhi|` with a random sign). Clean-room evidence:
//! docs/design/libm-cleanroom.md.
// Probe tool, not ported arithmetic: the casts build test inputs; short names are math variables.
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::many_single_char_names)]

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
        Dist { mode: w[0].to_string(), lo: w[1].parse().unwrap(), hi: w[2].parse().unwrap() }
    }
    fn sample(&self, r: &mut Rng, is32: bool) -> f64 {
        match self.mode.as_str() {
            "lin" => self.lo + (self.hi - self.lo) * r.unit(),
            "bits" | "symbits" => {
                let sym = self.mode == "symbits";
                let (lo, hi) = if sym { (self.lo.abs(), self.hi.abs()) } else { (self.lo, self.hi) };
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
    let mut out = std::io::BufWriter::new(std::fs::File::create(format!("{dir}/{tag}.bin")).unwrap());
    let is32 = name.ends_with('f');
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
            rec(&mut out, u64::from(x.to_bits()), u64::from(y.to_bits()), u64::from(z.to_bits()));
        } else {
            let z = match name {
                "pow" => std::hint::black_box(x).powf(std::hint::black_box(y)),
                _ => un64(name)(std::hint::black_box(x)),
            };
            rec(&mut out, x.to_bits(), y.to_bits(), z.to_bits());
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 || args[1] != "jobs" {
        eprintln!("usage: probe jobs <jobs-file> <outdir>");
        return;
    }
    std::fs::create_dir_all(&args[3]).unwrap();
    let text = std::fs::read_to_string(&args[2]).unwrap();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        run_job(line, &args[3]);
    }
}
