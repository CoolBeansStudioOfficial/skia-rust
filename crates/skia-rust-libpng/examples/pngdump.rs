//! Prints the libpng read path's behaviour for PNG files, in the format of `oracle/codec-diff/png/
//! pngdump.c`. The two outputs must be identical; a difference is a bug in the port.
//!
//! Usage: `cargo run -p skia-rust-libpng --example pngdump -- FILE...`

use std::cell::RefCell;
use std::rc::Rc;

use skia_rust_libpng::{
    PNG_HANDLE_CHUNK_ALWAYS, PNG_MAXIMUM_INFLATE_WINDOW, PngError, PngInfo, PngResult, PngStruct,
    ProgressiveHandler, create_info_struct, get_ihdr, get_valid, info,
};

/// State shared between the handler (owned by the reader) and this driver.
#[derive(Default)]
struct Shared {
    interlaced: bool,
    height: usize,
    rowbytes: usize,
    buf: Vec<u8>,
    rows: usize,
}

/// Port of the callbacks `SkPngCodec` installs: `AllRowsCallback` or `InterlacedRowCallback`.
struct Handler(Rc<RefCell<Shared>>);

impl ProgressiveHandler for Handler {
    fn info(&mut self, _png: &mut PngStruct, _info: &mut PngInfo) -> PngResult<()> {
        Ok(())
    }

    fn row(
        &mut self,
        png: &mut PngStruct,
        row: Option<&[u8]>,
        row_num: u32,
        pass: i32,
    ) -> PngResult<()> {
        let mut s = self.0.borrow_mut();
        if s.interlaced {
            if row_num as usize >= s.height {
                return Ok(());
            }
            let off = row_num as usize * s.rowbytes;
            let rb = s.rowbytes;
            png.progressive_combine_row(&mut s.buf[off..off + rb], row)?;
            return Ok(());
        }
        let Some(row) = row else {
            return Err(PngError::Error("NULL row in a non-interlaced image".into()));
        };
        let rb = s.rowbytes;
        println!(
            "row {row_num} {pass} {:016x}",
            fnv_update(fnv_init(), &row[..rb])
        );
        s.rows += 1;
        Ok(())
    }

    fn end(&mut self, _png: &mut PngStruct, _info: &mut PngInfo) -> PngResult<()> {
        Ok(())
    }
}

/// Port of `ffnv`'s hash in pngdump.c: FNV-1a 64.
fn fnv_init() -> u64 {
    1_469_598_103_934_665_603
}

fn fnv_update(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(1_099_511_628_211);
    }
    h
}

/// Returns the byte offset of the first IDAT chunk, as `first_idat` in pngdump.c does.
fn first_idat(data: &[u8]) -> Option<usize> {
    let mut pos = 8usize;
    while pos + 8 <= data.len() {
        let l = usize::try_from(u32::from_be_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
        ]))
        .unwrap_or(usize::MAX);
        if &data[pos + 4..pos + 8] == b"IDAT" {
            return Some(pos);
        }
        if pos.saturating_add(12).saturating_add(l) > data.len() {
            break;
        }
        pos += 12 + l;
    }
    None
}

/// Feeds `data[from..to)` in pieces of `piece` bytes, as `feed` does in pngdump.c.
fn feed(
    png: &mut PngStruct,
    info: &mut PngInfo,
    data: &[u8],
    from: usize,
    to: usize,
    piece: usize,
) -> bool {
    let mut pos = from;
    while pos < to {
        let n = (to - pos).min(piece);
        if let Err(e) = png.process_data(info, &data[pos..pos + n]) {
            eprintln!("error: {e}");
            return false;
        }
        pos += n;
    }
    true
}

/// Port of `setup_transforms` in pngdump.c (`SkPngCodec`'s infoCallback). Returns the pass count.
fn setup_transforms(png: &mut PngStruct, info: &PngInfo) -> u32 {
    let (_w, _h, mut bd, ct, _il, _cm, _fm) = get_ihdr(info);
    if bd == 16 && (ct == 0 || ct == 4) {
        bd = 8;
        png.set_strip_16();
    }
    match ct {
        3 => {
            if bd < 8 {
                png.set_packing();
            }
        }
        2 => {
            if get_valid(info, info::TRNS) {
                png.set_trns_to_alpha();
            }
        }
        0 => {
            if bd < 8 {
                png.set_expand_gray_1_2_4_to_8();
            }
            if get_valid(info, info::TRNS) {
                png.set_trns_to_alpha();
            }
        }
        _ => {}
    }
    png.set_interlace_handling()
}

fn run(path: &str, data: &[u8], piece: usize) {
    println!("== {path} piece={piece}");
    let Some(idat) = first_idat(data) else {
        println!("header: incomplete\nresult: error");
        return;
    };
    let mut png = PngStruct::new_read();
    png.set_option(PNG_MAXIMUM_INFLATE_WINDOW, true);
    let mut info = create_info_struct();
    png.set_keep_unknown_chunks(PNG_HANDLE_CHUNK_ALWAYS, &[]);
    if !feed(&mut png, &mut info, data, 0, idat, piece) {
        println!("header: error\nresult: error");
        return;
    }
    println!("header: ok");
    let passes = setup_transforms(&mut png, &info);
    if png.read_update_info(&mut info).is_err() {
        println!("update: error\nresult: error");
        return;
    }
    let rowbytes = info.rowbytes;
    let height = info.height as usize;
    println!(
        "info: {} {} {} {} {} {}",
        info.width, info.height, info.bit_depth, info.color_type, rowbytes, info.channels
    );
    let shared = Rc::new(RefCell::new(Shared {
        interlaced: passes > 1,
        height,
        rowbytes,
        buf: if passes > 1 {
            vec![0u8; height * rowbytes]
        } else {
            Vec::new()
        },
        rows: 0,
    }));
    png.set_progressive_read_fn(Some(Box::new(Handler(Rc::clone(&shared)))));
    let ok = feed(&mut png, &mut info, data, idat, data.len(), piece);
    let s = shared.borrow();
    if s.interlaced {
        println!("interlace-hash: {:016x}", fnv_update(fnv_init(), &s.buf));
    }
    println!(
        "rows: {}\nresult: {}",
        s.rows,
        if ok { "ok" } else { "error" }
    );
}

fn main() {
    for path in std::env::args().skip(1) {
        match std::fs::read(&path) {
            Ok(data) => {
                for piece in [1usize, 7, 4096] {
                    run(&path, &data, piece);
                }
            }
            Err(_) => println!("== {path}: cannot open"),
        }
    }
}
