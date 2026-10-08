//! The input source interface (`struct jpeg_source_mgr` in `jpeglib.h`).
//!
//! libjpeg reads compressed bytes through three callbacks. In C they receive the decompressor and
//! read and write `next_input_byte` and `bytes_in_buffer` directly. Here the same pair lives in
//! [`SrcBuf`], which the decoder owns and passes to each callback, so the callbacks can replace
//! the buffer exactly as Skia's `SkJpegSourceMgr` does.
//!
//! Suspension: `fill_input_buffer` returning `false` means "no more data right now". The decoder
//! then returns the suspended result (`JPEG_SUSPENDED`) and keeps its state, so a later call can
//! resume it. `skip_input_bytes` returning `false` is fatal (Skia calls `error_exit`).

/// The `next_input_byte` / `bytes_in_buffer` pair of `jpeg_source_mgr`.
///
/// The valid bytes are `data[next..next + bytes_in_buffer]`. Callbacks may replace `data` and
/// reset `next`, which is what a buffered source does on refill.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SrcBuf {
    /// The buffer that `next` and `bytes_in_buffer` index into.
    pub data: Vec<u8>,
    /// Index of the next unread byte in `data`.
    pub next: usize,
    /// Number of unread bytes starting at `next`.
    pub bytes_in_buffer: usize,
}

impl SrcBuf {
    /// The unread bytes, `next_input_byte[0..bytes_in_buffer]`.
    pub fn unread(&self) -> &[u8] {
        &self.data[self.next..self.next + self.bytes_in_buffer]
    }
}

/// A libjpeg input source. Skia's `SkJpegSourceMgr` implements this for memory-backed and
/// buffered streams; the differential harness implements it for chunked input.
pub trait JpegSource {
    /// `init_source`: called once when the decoder starts reading. Sets the initial buffer.
    fn init_source(&mut self, buf: &mut SrcBuf);

    /// `fill_input_buffer`: the buffer is empty. Replace it with more data and return `true`, or
    /// return `false` to suspend. The decoder resets the buffer to empty after a `false`.
    fn fill_input_buffer(&mut self, buf: &mut SrcBuf) -> bool;

    /// `skip_input_data`: discard `bytes_to_skip` bytes. Returning `false` is a fatal error.
    fn skip_input_bytes(&mut self, bytes_to_skip: usize, buf: &mut SrcBuf) -> bool;
}
