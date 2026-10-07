// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! A blitter that records every call it receives as a line of text, for debugging the scan
//! converters (not a Skia class).
//!
//! The format is the one `oracle/scan-aaa` prints from real Skia, so a scan conversion can be
//! compared call by call with the oracle: wrap the blitter a GM draws into in a [`DumpBlitter`]
//! (with [`DumpBlitter::forwarding`]) and diff its [`DumpBlitter::text`] against the oracle's.
//!
//! Every virtual `SkBlitter` method is overridden, so calls are recorded as the scan converter
//! made them (Skia's default implementations, which decompose e.g. `blitAntiRect` into other
//! calls, never run). One line per call:
//!
//! ```text
//! blitH x y width
//! blitAntiH x y alpha:run alpha:run ...
//! blitV x y height alpha
//! blitRect x y width height
//! blitAntiRect x y width height leftAlpha rightAlpha
//! blitAntiH2 x y a0 a1
//! blitAntiV2 x y a0 a1
//! blitMask format l t r b clip l t r b
//!  row y: aa bb cc ...      (A8 masks: one line per row of the mask bounds, hex)
//! ```

use std::fmt::Write;

use skia_rust_core::color::Alpha;
use skia_rust_core::mask::{Mask, MaskFormat};
use skia_rust_core::rect::IRect;

use crate::blitter::{BlitMemory, Blitter};

/// Records each blitter call as a line of text, optionally forwarding it to another blitter.
#[derive(Debug, Default)]
pub struct DumpBlitter<'a> {
    text: String,
    inner: Option<&'a mut dyn Blitter>,
    memory: BlitMemory,
}

impl<'a> DumpBlitter<'a> {
    /// A blitter that only records.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A blitter that records each call, then forwards it to `inner`.
    #[must_use]
    pub fn forwarding(inner: &'a mut dyn Blitter) -> Self {
        DumpBlitter {
            text: String::new(),
            inner: Some(inner),
            memory: BlitMemory::default(),
        }
    }

    /// The calls recorded so far, one per line.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Takes the calls recorded so far.
    pub fn take_text(&mut self) -> String {
        std::mem::take(&mut self.text)
    }

    fn line(&mut self, args: std::fmt::Arguments<'_>) {
        let _ = self.text.write_fmt(args);
        self.text.push('\n');
    }
}

/// Formats the runs of a `blitAntiH` call as ` alpha:run` pairs.
#[must_use]
pub fn format_runs(antialias: &[Alpha], runs: &[i16]) -> String {
    let mut s = String::new();
    let mut i = 0usize;
    while runs[i] > 0 {
        let _ = write!(s, " {}:{}", antialias[i], runs[i]);
        i += usize::try_from(runs[i]).expect("positive run");
    }
    s
}

fn format_name(format: MaskFormat) -> &'static str {
    match format {
        MaskFormat::BW => "BW",
        MaskFormat::A8 => "A8",
        MaskFormat::ThreeD => "3D",
        MaskFormat::Argb32 => "ARGB32",
        MaskFormat::Lcd16 => "LCD16",
        MaskFormat::Sdf => "SDF",
    }
}

/// Formats a `blitMask` call (its first line and, for A8 masks, the rows).
#[must_use]
#[allow(clippy::cast_sign_loss)] // rows and columns are inside the mask bounds
pub fn format_mask(mask: &Mask<'_>, clip: &IRect) -> String {
    let b = mask.bounds;
    let mut s = format!(
        "blitMask {} {} {} {} {} clip {} {} {} {}",
        format_name(mask.format),
        b.left,
        b.top,
        b.right,
        b.bottom,
        clip.left,
        clip.top,
        clip.right,
        clip.bottom
    );
    if mask.format == MaskFormat::A8 {
        for y in b.top..b.bottom {
            let _ = write!(s, "\n row {y}:");
            let row = (y - b.top) as usize * mask.row_bytes as usize;
            for x in 0..b.width() as usize {
                let _ = write!(s, " {:02x}", mask.image[row + x]);
            }
        }
    }
    s
}

impl Blitter for DumpBlitter<'_> {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        self.line(format_args!("blitH {x} {y} {width}"));
        if let Some(b) = self.inner.as_deref_mut() {
            b.blit_h(x, y, width);
        }
    }

    fn blit_anti_h(&mut self, x: i32, y: i32, antialias: &mut [Alpha], runs: &mut [i16]) {
        let r = format_runs(antialias, runs);
        self.line(format_args!("blitAntiH {x} {y}{r}"));
        if let Some(b) = self.inner.as_deref_mut() {
            b.blit_anti_h(x, y, antialias, runs);
        }
    }

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        self.line(format_args!("blitV {x} {y} {height} {alpha}"));
        if let Some(b) = self.inner.as_deref_mut() {
            b.blit_v(x, y, height, alpha);
        }
    }

    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.line(format_args!("blitRect {x} {y} {width} {height}"));
        if let Some(b) = self.inner.as_deref_mut() {
            b.blit_rect(x, y, width, height);
        }
    }

    fn blit_anti_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        left_alpha: Alpha,
        right_alpha: Alpha,
    ) {
        self.line(format_args!(
            "blitAntiRect {x} {y} {width} {height} {left_alpha} {right_alpha}"
        ));
        if let Some(b) = self.inner.as_deref_mut() {
            b.blit_anti_rect(x, y, width, height, left_alpha, right_alpha);
        }
    }

    fn blit_mask(&mut self, mask: &Mask<'_>, clip: &IRect) {
        let m = format_mask(mask, clip);
        self.line(format_args!("{m}"));
        if let Some(b) = self.inner.as_deref_mut() {
            b.blit_mask(mask, clip);
        }
    }

    fn blit_anti_h2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        self.line(format_args!("blitAntiH2 {x} {y} {a0} {a1}"));
        if let Some(b) = self.inner.as_deref_mut() {
            b.blit_anti_h2(x, y, a0, a1);
        }
    }

    fn blit_anti_v2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        self.line(format_args!("blitAntiV2 {x} {y} {a0} {a1}"));
        if let Some(b) = self.inner.as_deref_mut() {
            b.blit_anti_v2(x, y, a0, a1);
        }
    }

    fn request_rows_preserved(&self) -> i32 {
        self.inner
            .as_deref()
            .map_or(1, Blitter::request_rows_preserved)
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.memory
    }
}
