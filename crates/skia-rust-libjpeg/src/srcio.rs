//! The `INPUT_VARS` / `INPUT_BYTE` / `INPUT_SYNC` macros of `jdmarker.c`, as functions.
//!
//! libjpeg copies `next_input_byte` and `bytes_in_buffer` into locals, reads through the locals,
//! and writes them back only at `INPUT_SYNC`. A function that returns "suspended" before its sync
//! therefore leaves the source where it was at the last sync, and the function restarts from
//! there when it is called again. [`Local`] is that copy. The buffer itself lives in
//! `Decompress::srcbuf`, so a `fill_input_buffer` that replaces it is seen through
//! [`Decompress::make_byte_avail`], which reloads the local copy (`INPUT_RELOAD`).

use crate::Decompress;
use crate::error::{Error, Result};

/// The locals of `INPUT_VARS(cinfo)`: a copy of the source's `next_input_byte` and
/// `bytes_in_buffer`, synchronised back with [`Decompress::input_sync`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Local {
    pub(crate) next: usize,
    pub(crate) bytes: usize,
}

impl Decompress {
    /// `INPUT_VARS(cinfo)`: copies the source's state into locals.
    pub(crate) fn input_vars(&self) -> Local {
        Local {
            next: self.srcbuf.next,
            bytes: self.srcbuf.bytes_in_buffer,
        }
    }

    /// `INPUT_SYNC(cinfo)`: writes the locals back to the source.
    pub(crate) fn input_sync(&mut self, l: &Local) {
        self.srcbuf.next = l.next;
        self.srcbuf.bytes_in_buffer = l.bytes;
    }

    /// Calls the source's `fill_input_buffer`. On failure the buffer is reset to empty, as
    /// Skia's wrapper does (`next_input_byte = nullptr; bytes_in_buffer = 0`).
    pub(crate) fn fill_input(&mut self) -> Result<bool> {
        let src = self
            .source
            .as_mut()
            .ok_or(Error::Internal("no data source"))?;
        if src.fill_input_buffer(&mut self.srcbuf) {
            Ok(true)
        } else {
            self.srcbuf.next = 0;
            self.srcbuf.bytes_in_buffer = 0;
            Ok(false)
        }
    }

    /// `MAKE_BYTE_AVAIL(cinfo, action)`: refills when the local count is zero. Returns
    /// `Ok(false)` where libjpeg runs `action` (a suspension).
    pub(crate) fn make_byte_avail(&mut self, l: &mut Local) -> Result<bool> {
        if l.bytes == 0 {
            if !self.fill_input()? {
                return Ok(false);
            }
            // INPUT_RELOAD
            l.next = self.srcbuf.next;
            l.bytes = self.srcbuf.bytes_in_buffer;
        }
        Ok(true)
    }

    /// `INPUT_BYTE(cinfo, V, action)`. `Ok(None)` is a suspension.
    pub(crate) fn input_byte(&mut self, l: &mut Local) -> Result<Option<u8>> {
        if !self.make_byte_avail(l)? {
            return Ok(None);
        }
        l.bytes -= 1;
        let v = self.srcbuf.data[l.next];
        l.next += 1;
        Ok(Some(v))
    }

    /// `INPUT_2BYTES(cinfo, V, action)`: big-endian 16-bit value. `Ok(None)` is a suspension.
    pub(crate) fn input_2bytes(&mut self, l: &mut Local) -> Result<Option<i32>> {
        let Some(hi) = self.input_byte(l)? else {
            return Ok(None);
        };
        let Some(lo) = self.input_byte(l)? else {
            return Ok(None);
        };
        Ok(Some((i32::from(hi) << 8) + i32::from(lo)))
    }

    /// `skip_input_data(cinfo, num_bytes)`: Skia's wrapper. A failure is fatal.
    pub(crate) fn skip_input_data(&mut self, num_bytes: i64) -> Result<()> {
        // Callers sync before calling, so `srcbuf` is current.
        let n = usize::try_from(num_bytes).map_err(|_| Error::SourceSkipFailed)?;
        let src = self
            .source
            .as_mut()
            .ok_or(Error::Internal("no data source"))?;
        if !src.skip_input_bytes(n, &mut self.srcbuf) {
            self.srcbuf.next = 0;
            self.srcbuf.bytes_in_buffer = 0;
            return Err(Error::SourceSkipFailed);
        }
        Ok(())
    }
}
