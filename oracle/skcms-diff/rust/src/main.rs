// Rust twin of ../harness.cpp: prints the same report from the Rust skcms port (see the usage
// notes in harness.cpp). Tooling only: not part of the workspace.
use skia_rust_skcms as skcms;
use skcms::{AlphaFormat, Cicp, Curve, IccProfile, Matrix3x3, PixelFormat, TransferFunction};
use std::fmt::Write as _;

thread_local! {
    static RNG: std::cell::Cell<u32> = const { std::cell::Cell::new(1) };
    static CASE: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

fn rnd() -> u32 {
    RNG.with(|r| {
        let mut x = r.get();
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        r.set(x);
        x
    })
}

fn fnv(p: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in p {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn fbits(f: f32) -> u32 {
    f.to_bits()
}

fn bpp(fmt: i32) -> usize {
    match fmt >> 1 {
        0 | 1 => 1,
        2..=4 => 2,
        5 => 3,
        6..=8 => 4,
        9 => 6,
        10 => 8,
        11 => 6,
        12 => 8,
        13 => 6,
        14 => 8,
        15 => 6,
        16 => 8,
        17 => 12,
        18 => 16,
        19 => 4,
        20 => 8,
        _ => 0,
    }
}

fn rnd_float() -> f32 {
    (rnd() % 1400) as f32 / 1000.0f32 - 0.2f32
}

fn half_from_float(f: f32) -> u16 {
    let sem = f.to_bits();
    let s = sem & 0x80000000;
    let em = sem ^ s;
    if em < 0x38800000 {
        return (s >> 16) as u16;
    }
    ((s >> 16).wrapping_add(em >> 13).wrapping_sub((127 - 15) << 10)) as u16
}

fn make_pixels(fmt: i32, n: usize) -> Vec<u8> {
    let b = bpp(fmt);
    let mut v = vec![0u8; n * b + 8];
    let kind = fmt >> 1;
    for i in 0..n {
        let p = &mut v[i * b..(i + 1) * b];
        if kind == 17 || kind == 18 {
            for c in 0..b / 4 {
                p[4 * c..4 * c + 4].copy_from_slice(&rnd_float().to_ne_bytes());
            }
        } else if (13..=16).contains(&kind) {
            for c in 0..b / 2 {
                p[2 * c..2 * c + 2].copy_from_slice(&half_from_float(rnd_float()).to_ne_bytes());
            }
        } else {
            for c in 0..b {
                p[c] = rnd() as u8;
            }
        }
    }
    v
}

fn dump_tf(name: &str, tf: &TransferFunction) {
    print!("{}", fmt_tf(name, tf));
}

fn fmt_tf(name: &str, tf: &TransferFunction) -> String {
    format!(
        "{} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x}\n",
        name,
        fbits(tf.g),
        fbits(tf.a),
        fbits(tf.b),
        fbits(tf.c),
        fbits(tf.d),
        fbits(tf.e),
        fbits(tf.f)
    )
}

fn dump_mat(name: &str, m: &Matrix3x3) {
    print!("{name}");
    for r in 0..3 {
        for c in 0..3 {
            print!(" {:08x}", fbits(m.vals[r][c]));
        }
    }
    println!();
}

fn dump_curve(name: &str, c: &Curve) {
    match c {
        Curve::Parametric(tf) => dump_tf(name, tf),
        Curve::Table8 { entries, table } | Curve::Table16 { entries, table } => {
            let w = if matches!(c, Curve::Table8 { .. }) { 1 } else { 2 };
            let nbytes = std::cmp::min(8, *entries as usize * w);
            print!("{} T{} {}", name, w * 8, entries);
            for i in 0..nbytes {
                print!(" {:02x}", table.byte(i));
            }
            println!();
        }
    }
}

fn dump_profile(name: &str, p: &IccProfile) {
    println!(
        "PROFILE {} size={} dcs={:08x} pcs={:08x} tags={} trc={} xyz={} a2b={} b2a={} cicp={} hagc={}",
        name,
        p.size,
        p.data_color_space,
        p.pcs,
        p.tag_count,
        i32::from(p.has_trc),
        i32::from(p.has_to_xyzd50),
        i32::from(p.has_a2b),
        i32::from(p.has_b2a),
        i32::from(p.has_cicp),
        i32::from(p.has_hagc)
    );
    if p.has_trc {
        for i in 0..3 {
            dump_curve("  trc", &p.trc[i]);
        }
    }
    if p.has_to_xyzd50 {
        dump_mat("  xyz", &p.to_xyzd50);
    }
    if p.has_a2b {
        let a = &p.a2b;
        println!(
            "  a2b in={} grid={},{},{},{} mc={} out={} g8={} g16={}",
            a.input_channels,
            a.grid_points[0],
            a.grid_points[1],
            a.grid_points[2],
            a.grid_points[3],
            a.matrix_channels,
            a.output_channels,
            i32::from(a.grid_8.is_some()),
            i32::from(a.grid_16.is_some())
        );
        for i in 0..a.input_channels as usize {
            dump_curve("  a2b.in", &a.input_curves[i]);
        }
        for i in 0..a.matrix_channels as usize {
            dump_curve("  a2b.m", &a.matrix_curves[i]);
        }
        for i in 0..a.output_channels as usize {
            dump_curve("  a2b.out", &a.output_curves[i]);
        }
        print!("  a2b.matrix");
        for r in 0..3 {
            for c in 0..4 {
                print!(" {:08x}", fbits(a.matrix.vals[r][c]));
            }
        }
        println!();
        let g = a.grid_8.as_ref().or(a.grid_16.as_ref());
        if let Some(g) = g {
            println!("  a2b.grid {:02x} {:02x} {:02x} {:02x}", g.byte(0), g.byte(1), g.byte(2), g.byte(3));
        }
    }
    if p.has_b2a {
        let b = &p.b2a;
        println!(
            "  b2a in={} mc={} out={} grid={},{},{},{} g8={} g16={}",
            b.input_channels,
            b.matrix_channels,
            b.output_channels,
            b.grid_points[0],
            b.grid_points[1],
            b.grid_points[2],
            b.grid_points[3],
            i32::from(b.grid_8.is_some()),
            i32::from(b.grid_16.is_some())
        );
        for i in 0..b.input_channels as usize {
            dump_curve("  b2a.in", &b.input_curves[i]);
        }
        for i in 0..b.matrix_channels as usize {
            dump_curve("  b2a.m", &b.matrix_curves[i]);
        }
        for i in 0..b.output_channels as usize {
            dump_curve("  b2a.out", &b.output_curves[i]);
        }
        print!("  b2a.matrix");
        for r in 0..3 {
            for c in 0..4 {
                print!(" {:08x}", fbits(b.matrix.vals[r][c]));
            }
        }
        println!();
        let g = b.grid_8.as_ref().or(b.grid_16.as_ref());
        if let Some(g) = g {
            println!("  b2a.grid {:02x} {:02x} {:02x} {:02x}", g.byte(0), g.byte(1), g.byte(2), g.byte(3));
        }
    }
    if p.has_cicp {
        println!(
            "  cicp {} {} {} {}",
            p.cicp.color_primaries,
            p.cicp.transfer_characteristics,
            p.cicp.matrix_coefficients,
            p.cicp.video_full_range_flag
        );
    }
    if p.has_hagc {
        println!("  hagc {}", p.hagc.size);
    }
    println!("  channels={}", skcms::get_input_channel_count(p));
    if let Some(chad) = skcms::get_chad(p) {
        dump_mat("  chad", &chad);
    }
    if let Some(w) = skcms::get_wtpt(p) {
        println!("  wtpt {:08x} {:08x} {:08x}", fbits(w[0]), fbits(w[1]), fbits(w[2]));
    }
}

const ALL_FORMATS: [PixelFormat; 42] = [
    PixelFormat::A8,
    PixelFormat::A8Swap,
    PixelFormat::G8,
    PixelFormat::G8Swap,
    PixelFormat::Ga88,
    PixelFormat::Ga88Swap,
    PixelFormat::Rgb565,
    PixelFormat::Bgr565,
    PixelFormat::Abgr4444,
    PixelFormat::Argb4444,
    PixelFormat::Rgb888,
    PixelFormat::Bgr888,
    PixelFormat::Rgba8888,
    PixelFormat::Bgra8888,
    PixelFormat::Rgba8888SRgb,
    PixelFormat::Bgra8888SRgb,
    PixelFormat::Rgba1010102,
    PixelFormat::Bgra1010102,
    PixelFormat::Rgb161616Le,
    PixelFormat::Bgr161616Le,
    PixelFormat::Rgba16161616Le,
    PixelFormat::Bgra16161616Le,
    PixelFormat::Rgb161616Be,
    PixelFormat::Bgr161616Be,
    PixelFormat::Rgba16161616Be,
    PixelFormat::Bgra16161616Be,
    PixelFormat::RgbHhhNorm,
    PixelFormat::BgrHhhNorm,
    PixelFormat::RgbaHhhhNorm,
    PixelFormat::BgraHhhhNorm,
    PixelFormat::RgbHhh,
    PixelFormat::BgrHhh,
    PixelFormat::RgbaHhhh,
    PixelFormat::BgraHhhh,
    PixelFormat::RgbFff,
    PixelFormat::BgrFff,
    PixelFormat::RgbaFfff,
    PixelFormat::BgraFfff,
    PixelFormat::Rgb101010xXr,
    PixelFormat::Bgr101010xXr,
    PixelFormat::Rgba10101010Xr,
    PixelFormat::Bgra10101010Xr,
];

fn alpha(a: i32) -> AlphaFormat {
    match a {
        0 => AlphaFormat::Opaque,
        1 => AlphaFormat::Unpremul,
        _ => AlphaFormat::PremulAsEncoded,
    }
}


fn run_transform(
    tag: &str,
    src: Option<&IccProfile>,
    src_fmt: i32,
    src_alpha: i32,
    dst: Option<&IccProfile>,
    dst_fmt: i32,
    dst_alpha: i32,
    n: usize,
) {
    let case = CASE.with(|c| {
        c.set(c.get() + 1);
        c.get()
    });
    let mut state = 0x9e3779b9u32 ^ (case as u32).wrapping_mul(2654435761u32);
    if state == 0 {
        state = 1;
    }
    RNG.with(|r| r.set(state));
    let input = make_pixels(src_fmt, n);
    let mut out = vec![0xAAu8; n * bpp(dst_fmt) + 8];
    let ok = skcms::transform(
        &input,
        ALL_FORMATS[src_fmt as usize],
        alpha(src_alpha),
        src,
        &mut out,
        ALL_FORMATS[dst_fmt as usize],
        alpha(dst_alpha),
        dst,
        n,
    );
    print!(
        "T {} {}/{} -> {}/{} ok={} hash={:016x}",
        tag,
        src_fmt,
        src_alpha,
        dst_fmt,
        dst_alpha,
        i32::from(ok),
        if ok { fnv(&out[..n * bpp(dst_fmt)]) } else { 0 }
    );
    if std::env::var_os("FIRST").is_some() {
        print!(" in=");
        for k in 0..std::cmp::min(bpp(src_fmt) * 2, input.len()) {
            print!("{:02x}", input[k]);
        }
        print!(" out=");
        for k in 0..std::cmp::min(bpp(dst_fmt) * 3, out.len()) {
            print!("{:02x}", out[k]);
        }
    }
    println!();
}

fn make_profile(tf: &TransferFunction, m: &Matrix3x3) -> IccProfile {
    let mut p = IccProfile::new();
    p.set_transfer_function(tf);
    p.set_xyzd50(m);
    p
}

fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    let root = std::env::args().nth(1).unwrap();
    let root_norm = root.replace('\\', "/");
    // ---- transfer functions
    let mut tfs: Vec<(String, TransferFunction)> = Vec::new();
    tfs.push(("srgb".into(), *skcms::srgb_transfer_function()));
    tfs.push(("srgb_inv".into(), *skcms::srgb_inverse_transfer_function()));
    tfs.push(("identity".into(), *skcms::identity_transfer_function()));
    tfs.push(("gamma22".into(), TransferFunction::new(2.2, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0)));
    tfs.push(("gamma18".into(), TransferFunction::new(1.8, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0)));
    tfs.push((
        "rec709ish".into(),
        TransferFunction::new(2.22222, 0.909672, 0.0903276, 0.222222, 0.0812429, 0.0, 0.0),
    ));
    tfs.push((
        "smpte240".into(),
        TransferFunction::new(2.222222222222, 0.899626676224, 0.100373323776, 0.25, 0.091286342118, 0.0, 0.0),
    ));
    tfs.push(("bad_neg_a".into(), TransferFunction::new(2.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0)));
    tfs.push(("pq203".into(), TransferFunction::make_pq(203.0)));
    tfs.push(("pq100".into(), TransferFunction::make_pq(100.0)));
    tfs.push(("hlg203".into(), TransferFunction::make_hlg(203.0, 1000.0, 1.2)));
    tfs.push(("hlg12".into(), TransferFunction::make_hlg(1.0, 12.0, 1.0)));
    tfs.push((
        "pqish".into(),
        TransferFunction::make_pqish(
            -107.0 / 128.0,
            1.0,
            32.0 / 2523.0,
            2413.0 / 128.0,
            -2392.0 / 128.0,
            8192.0 / 1305.0,
        ),
    ));
    tfs.push((
        "hlgish".into(),
        TransferFunction::make_hlgish(2.0, 2.0, 1.0 / 0.17883277, 0.28466892, 0.55991073),
    ));
    tfs.push((
        "hlgish_k".into(),
        TransferFunction::make_scaled_hlgish(1.0 / 12.0, 2.0, 2.0, 1.0 / 0.17883277, 0.28466892, 0.55991073),
    ));
    let xs = [-0.5f32, -0.0, 0.0, 0.001, 0.04045, 0.2, 0.5, 0.75, 0.9999, 1.0, 1.5, 10.0];
    for (name, tf) in &tfs {
        dump_tf(&format!("TF {name}"), tf);
        println!("  type={}", tf.tf_type() as i32);
        print!("  eval");
        for &x in &xs {
            print!(" {:08x}", fbits(tf.eval(x)));
        }
        println!();
        if let Some(inv) = tf.invert() {
            dump_tf("  inv", &inv);
            print!("  inveval");
            for &x in &xs {
                print!(" {:08x}", fbits(inv.eval(x)));
            }
            println!();
        } else {
            println!("  inv fail");
        }
    }
    print!("powf");
    for x in [0.0f32, 0.1, 0.5, 1.0, 2.0, 7.5, 1000.0] {
        for y in [0.0f32, 0.4, 1.0, 2.2, 2.4, -1.0] {
            print!(" {:08x}", fbits(skcms::powf_(x, y)));
        }
    }
    println!();

    // ---- matrices
    let ms = [
        Matrix3x3 { vals: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] },
        Matrix3x3 {
            vals: [
                [0.436065674, 0.385147095, 0.143066406],
                [0.222488403, 0.716873169, 0.060607910],
                [0.013916016, 0.097076416, 0.714096069],
            ],
        },
        Matrix3x3 {
            vals: [
                [0.515102, 0.291965, 0.157153],
                [0.241182, 0.692236, 0.0665819],
                [-0.00104941, 0.0418818, 0.784378],
            ],
        },
        Matrix3x3 { vals: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]] },
        Matrix3x3 { vals: [[0.0; 3]; 3] },
        Matrix3x3 { vals: [[1e-30, 0.0, 0.0], [0.0, 1e-30, 0.0], [0.0, 0.0, 1e-30]] },
    ];
    for i in 0..6 {
        let inv = ms[i].invert();
        println!("MAT {} inv={}", i, i32::from(inv.is_some()));
        if let Some(inv) = inv {
            dump_mat("  inv", &inv);
        }
        for j in 0..6 {
            let c = ms[i].concat(&ms[j]);
            dump_mat("  concat", &c);
        }
    }
    let prim: [[f32; 8]; 7] = [
        [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290],
        [0.7347, 0.2653, 0.1596, 0.8404, 0.0366, 0.0001, 0.34567, 0.35850],
        [0.67, 0.33, 0.21, 0.71, 0.14, 0.08, 0.31006, 0.31616],
        [0.680, 0.320, 0.265, 0.690, 0.150, 0.060, 0.3127, 0.3290],
        [0.708, 0.292, 0.170, 0.797, 0.131, 0.046, 0.3127, 0.3290],
        [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0 / 3.0, 1.0 / 3.0],
        [1.5, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0 / 3.0, 1.0 / 3.0],
    ];
    for p in &prim {
        let m = skcms::primaries_to_xyzd50(p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7]);
        println!("PRIM ok={}", i32::from(m.is_some()));
        if let Some(m) = m {
            dump_mat("  m", &m);
        }
    }

    // ---- profiles
    let mut paths = Vec::new();
    walk(&std::path::Path::new(&root).join("icc_profiles"), &mut paths);
    let mut files: Vec<String> = Vec::new();
    for p in paths {
        let ext = p.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
        if ext == "icc" || ext == "icm" {
            let s = p.to_string_lossy().replace('\\', "/");
            files.push(s[root_norm.len() + 1..].to_string());
        }
    }
    files.sort();
    struct Loaded {
        name: String,
        p: IccProfile,
    }
    let mut loaded: Vec<Loaded> = Vec::new();
    for f in &files {
        let data = std::fs::read(format!("{root_norm}/{f}")).unwrap();
        let parsed = skcms::parse(&data);
        println!("FILE {} len={} parse={}", f, data.len(), i32::from(parsed.is_some()));
        if let Some(p) = parsed {
            dump_profile(f, &p);
            loaded.push(Loaded { name: f.clone(), p });
        }
    }

    // ---- profile comparisons, approximations
    for i in 0..loaded.len() {
        let l = &loaded[i];
        let m = &loaded[(i + 1) % loaded.len()];
        println!(
            "EQ {} srgb={} xyz={} next={} self={}",
            l.name,
            i32::from(skcms::approximately_equal_profiles(&l.p, skcms::srgb_profile())),
            i32::from(skcms::approximately_equal_profiles(&l.p, skcms::xyzd50_profile())),
            i32::from(skcms::approximately_equal_profiles(&l.p, &m.p)),
            i32::from(skcms::approximately_equal_profiles(&l.p, &l.p))
        );
        println!(
            "TRCS srgbinv={}",
            i32::from(skcms::trcs_are_approximate_inverse(&l.p, skcms::srgb_inverse_transfer_function()))
        );
        if l.p.has_trc {
            for c in 0..3 {
                if l.p.trc[c].table_entries() != 0 {
                    let a = skcms::approximate_curve(&l.p.trc[c]);
                    let mut s = String::new();
                    write!(s, "APPROX {} trc{} ok={}", l.name, c, i32::from(a.is_some())).unwrap();
                    if let Some((tf, err)) = &a {
                        write!(s, " err={:08x}", fbits(*err)).unwrap();
                        s.push_str(&fmt_tf(" tf", tf));
                    } else {
                        s.push('\n');
                    }
                    print!("{s}");
                    println!(
                        "  maxerr={:08x}",
                        fbits(skcms::max_roundtrip_error(&l.p.trc[c], skcms::srgb_inverse_transfer_function()))
                    );
                }
            }
        }
        let mut u = l.p.clone();
        let ok = skcms::make_usable_as_destination(&mut u);
        println!("USABLE {} ok={}", l.name, i32::from(ok));
        if ok {
            dump_profile(&format!("{}#usable", l.name), &u);
        }
        let mut u1 = l.p.clone();
        let ok = skcms::make_usable_as_destination_with_single_curve(&mut u1);
        println!("USABLE1 {} ok={}", l.name, i32::from(ok));
        if ok && u1.has_trc {
            if let Curve::Parametric(tf) = &u1.trc[0] {
                dump_tf("  tf0", tf);
            }
        }
    }

    // ---- transforms
    let srgb_p = skcms::srgb_profile().clone();
    let xyz_p = skcms::xyzd50_profile().clone();
    let p3 = ms[2];
    let p3_p = make_profile(&TransferFunction::new(2.2, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0), &p3);
    let p3srgb_p = make_profile(skcms::srgb_transfer_function(), &p3);
    let pq203 = TransferFunction::make_pq(203.0);
    let hlg203 = TransferFunction::make_hlg(203.0, 1000.0, 1.2);
    let mut pq_p = make_profile(&pq203, &p3);
    pq_p.has_cicp = true;
    pq_p.cicp = Cicp { color_primaries: 9, transfer_characteristics: 16, matrix_coefficients: 0, video_full_range_flag: 1 };
    let mut hlg_p = make_profile(&hlg203, &p3);
    hlg_p.has_cicp = true;
    hlg_p.cicp = Cicp { color_primaries: 9, transfer_characteristics: 18, matrix_coefficients: 0, video_full_range_flag: 1 };
    let dsts: Vec<(&str, Option<&IccProfile>)> = vec![
        ("srgb", Some(&srgb_p)),
        ("xyz", Some(&xyz_p)),
        ("p3g22", Some(&p3_p)),
        ("p3srgb", Some(&p3srgb_p)),
        ("pq", Some(&pq_p)),
        ("hlg", Some(&hlg_p)),
        ("null", None),
    ];
    for (dname, dp) in &dsts {
        for fmt in 0..42 {
            run_transform(&format!("sweepdst:{dname}"), Some(&srgb_p), 33, 1, *dp, fmt, (fmt + 1) % 3, 37);
        }
    }
    for fmt in 0..42 {
        for (dname, dp) in &dsts {
            run_transform(&format!("sweepsrc:{dname}"), Some(&p3srgb_p), fmt, fmt % 3, *dp, 27 - (fmt & 1), (fmt + 2) % 3, 41);
        }
    }
    for (sname, sp) in &dsts {
        for (dname, dp) in &dsts {
            run_transform(&format!("hdr:{sname}>{dname}"), *sp, 35, 0, *dp, 35, 0, 29);
            run_transform(&format!("hdr8:{sname}>{dname}"), *sp, 13, 1, *dp, 13, 2, 29);
        }
    }
    for (i, l) in loaded.iter().enumerate() {
        for (dname, dp) in &dsts {
            let fmts = [[12, 12], [10, 34], [35, 35], [25, 30], [18, 13]];
            for (k, f) in fmts.iter().enumerate() {
                run_transform(
                    &format!("src:{}>{}", l.name, dname),
                    Some(&l.p),
                    f[0],
                    ((i + k) % 3) as i32,
                    *dp,
                    f[1],
                    ((i + k + 1) % 3) as i32,
                    53,
                );
            }
        }
        let mut u = l.p.clone();
        let usable = skcms::make_usable_as_destination(&mut u);
        if usable {
            let fmts = [[12, 12], [35, 35], [34, 14], [19, 12], [35, 10]];
            for (k, f) in fmts.iter().enumerate() {
                run_transform(
                    &format!("dst:{}", l.name),
                    Some(&srgb_p),
                    f[0],
                    ((i + k) % 3) as i32,
                    Some(&u),
                    f[1],
                    ((i + k + 2) % 3) as i32,
                    53,
                );
            }
            run_transform(&format!("dstraw:{}", l.name), Some(&srgb_p), 35, 0, Some(&l.p), 35, 0, 31);
            run_transform(&format!("self:{}", l.name), Some(&l.p), 35, 0, Some(&l.p), 35, 0, 31);
            run_transform(&format!("gray:{}", l.name), Some(&srgb_p), 12, 1, Some(&u), 4, 2, 31);
            run_transform(&format!("gray8:{}", l.name), Some(&srgb_p), 12, 1, Some(&u), 2, 1, 31);
        }
    }
    println!("DONE");
}
