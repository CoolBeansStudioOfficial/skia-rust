//! Tests of the progressive read path on a tiny PNG generated for these tests: 2x2 RGB, 8 bits
//! per channel, filter type 0. The expected rows are the stored pixel bytes, compared exactly.

use std::sync::{Arc, Mutex};

use skia_rust_libpng::{
    PNG_HANDLE_CHUNK_ALWAYS, PNG_MAXIMUM_INFLATE_WINDOW, PngError, PngInfo, PngResult, PngStruct,
    ProgressiveHandler, create_info_struct, get_ihdr,
};

/// A 2x2 RGB PNG: IHDR, one stored-deflate IDAT, IEND.
const PNG_2X2: [u8; 82] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x02, 0x00, 0x00, 0x00, 0xfd, 0xd4, 0x9a,
    0x73, 0x00, 0x00, 0x00, 0x19, 0x49, 0x44, 0x41, 0x54, 0x78, 0x01, 0x01, 0x0e, 0x00, 0xf1, 0xff,
    0x00, 0x0a, 0x14, 0x1e, 0x28, 0x32, 0x3c, 0x00, 0x46, 0x50, 0x5a, 0x64, 0x6e, 0x78, 0x0f, 0x18,
    0x03, 0x0d, 0x02, 0x25, 0xc5, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42,
    0x60, 0x82,
];

/// The two rows of the image, without their filter bytes.
const ROW0: [u8; 6] = [10, 20, 30, 40, 50, 60];
const ROW1: [u8; 6] = [70, 80, 90, 100, 110, 120];

/// One row as the callback received it: (row number, the row's bytes).
type DeliveredRow = (u32, Vec<u8>);

/// The rows delivered so far.
type Rows = Arc<Mutex<Vec<DeliveredRow>>>;

/// Records every row the reader delivers, as the codec's row callback does.
struct Collect(Rows);

impl ProgressiveHandler for Collect {
    fn info(&mut self, _png: &mut PngStruct, _info: &mut PngInfo) -> PngResult<()> {
        Ok(())
    }

    fn row(
        &mut self,
        _png: &mut PngStruct,
        row: Option<&[u8]>,
        row_num: u32,
        _pass: i32,
    ) -> PngResult<()> {
        let row = row.expect("a non-interlaced image has no empty rows");
        self.0.lock().unwrap().push((row_num, row[..6].to_vec()));
        Ok(())
    }

    fn end(&mut self, _png: &mut PngStruct, _info: &mut PngInfo) -> PngResult<()> {
        Ok(())
    }
}

/// Reads `data` with `piece`-byte feeds, the way the codec drives the reader. Returns the header,
/// the rows delivered, and the first error if there was one.
fn decode(data: &[u8], piece: usize) -> (PngInfo, Vec<DeliveredRow>, Option<PngError>) {
    let mut png = PngStruct::new_read();
    assert!(png.set_option(PNG_MAXIMUM_INFLATE_WINDOW, true) >= 0);
    let mut info = create_info_struct();
    png.set_keep_unknown_chunks(PNG_HANDLE_CHUNK_ALWAYS, &[]);
    let idat = data
        .windows(4)
        .position(|w| w == b"IDAT")
        .expect("the test image has an IDAT")
        - 4;
    let mut error = None;
    for part in data[..idat].chunks(piece) {
        if let Err(e) = png.process_data(&mut info, part) {
            error = Some(e);
            break;
        }
    }
    if error.is_none() {
        png.read_update_info(&mut info)
            .expect("the header is valid");
        let rows: Rows = Arc::new(Mutex::new(Vec::new()));
        png.set_progressive_read_fn(Some(Box::new(Collect(Arc::clone(&rows)))));
        for part in data[idat..].chunks(piece) {
            if let Err(e) = png.process_data(&mut info, part) {
                error = Some(e);
                break;
            }
        }
        let rows = rows.lock().unwrap().clone();
        return (info, rows, error);
    }
    (info, Vec::new(), error)
}

#[test]
fn decodes_every_row_of_a_2x2_rgb_image_at_every_feed_size() {
    for piece in [1usize, 7, 4096] {
        let (info, rows, error) = decode(&PNG_2X2, piece);
        assert_eq!(error, None, "piece {piece}");
        assert_eq!(get_ihdr(&info), (2, 2, 8, 2, 0, 0, 0), "piece {piece}");
        assert_eq!(info.rowbytes, 6, "piece {piece}");
        assert_eq!(
            rows,
            vec![(0, ROW0.to_vec()), (1, ROW1.to_vec())],
            "piece {piece}"
        );
    }
}

#[test]
fn rejects_a_corrupt_ihdr_crc() {
    let mut data = PNG_2X2;
    // The IHDR CRC is the four bytes after the 13 IHDR data bytes, at offset 29.
    data[29] ^= 0x01;
    let (_info, _rows, error) = decode(&data, 4096);
    assert!(
        error.is_some(),
        "a critical chunk with a bad CRC is an error"
    );
}

#[test]
fn rejects_idat_before_ihdr() {
    // Drop the IHDR chunk (8 signature bytes, then 25 bytes of IHDR with its length and CRC).
    let mut data = PNG_2X2[..8].to_vec();
    data.extend_from_slice(&PNG_2X2[33..]);
    let (_info, _rows, error) = decode(&data, 4096);
    assert_eq!(
        error,
        Some(PngError::Error("Missing IHDR before IDAT".into()))
    );
}
