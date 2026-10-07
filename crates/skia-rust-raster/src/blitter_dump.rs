// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! A debug blitter that records (and optionally forwards) every call a scan converter makes.
//!
//! When a GM's hash differs from the oracle's, the first thing to compare is the sequence of
//! blitter calls the scan converter produced. [`DumpBlitter`] records each call as a
//! [`BlitCall`] and renders the list with [`DumpBlitter::dump`], one call per line:
//!
//! ```text
//! blit_h(x=3, y=7, w=5)
//! blit_anti_h(x=0, y=2, [1x128, 4x255])
//! blit_v(x=1, y=0, h=3, a=64)
//! blit_rect(x=2, y=2, w=3, h=1)
//! ```
//!
//! The calls are recorded at the level the scan converter makes them (`blit_anti_h2` is
//! recorded as such, not as the two `blit_anti_h` calls its default expands to), which is what
//! the C++ code reads like. With an inner blitter, every call is forwarded unchanged.

use std::fmt::Write as _;

use skia_rust_core::color::Alpha;
use skia_rust_core::mask::Mask;
use skia_rust_core::rect::IRect;

use crate::blitter::{BlitMemory, Blitter};

/// One recorded blitter call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlitCall {
    /// `blit_h(x, y, width)`.
    H { x: i32, y: i32, width: i32 },
    /// `blit_anti_h(x, y, ..)`, with the sparse run encoding decoded to `(run length, alpha)`.
    AntiH {
        x: i32,
        y: i32,
        runs: Vec<(i32, Alpha)>,
    },
    /// `blit_v(x, y, height, alpha)`.
    V {
        x: i32,
        y: i32,
        height: i32,
        alpha: Alpha,
    },
    /// `blit_rect(x, y, width, height)`.
    Rect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
    /// `blit_anti_rect(x, y, width, height, left_alpha, right_alpha)`.
    AntiRect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        left_alpha: Alpha,
        right_alpha: Alpha,
    },
    /// `blit_anti_h2(x, y, a0, a1)`.
    AntiH2 { x: i32, y: i32, a0: u32, a1: u32 },
    /// `blit_anti_v2(x, y, a0, a1)`.
    AntiV2 { x: i32, y: i32, a0: u32, a1: u32 },
    /// `blit_mask(mask, clip)`: the mask's bounds and the clip.
    Mask { bounds: IRect, clip: IRect },
}

impl std::fmt::Display for BlitCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlitCall::H { x, y, width } => write!(f, "blit_h(x={x}, y={y}, w={width})"),
            BlitCall::AntiH { x, y, runs } => {
                write!(f, "blit_anti_h(x={x}, y={y}, [")?;
                for (i, (n, a)) in runs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{n}x{a}")?;
                }
                write!(f, "])")
            }
            BlitCall::V {
                x,
                y,
                height,
                alpha,
            } => write!(f, "blit_v(x={x}, y={y}, h={height}, a={alpha})"),
            BlitCall::Rect {
                x,
                y,
                width,
                height,
            } => write!(f, "blit_rect(x={x}, y={y}, w={width}, h={height})"),
            BlitCall::AntiRect {
                x,
                y,
                width,
                height,
                left_alpha,
                right_alpha,
            } => write!(
                f,
                "blit_anti_rect(x={x}, y={y}, w={width}, h={height}, la={left_alpha}, ra={right_alpha})"
            ),
            BlitCall::AntiH2 { x, y, a0, a1 } => {
                write!(f, "blit_anti_h2(x={x}, y={y}, a0={a0}, a1={a1})")
            }
            BlitCall::AntiV2 { x, y, a0, a1 } => {
                write!(f, "blit_anti_v2(x={x}, y={y}, a0={a0}, a1={a1})")
            }
            BlitCall::Mask { bounds, clip } => write!(
                f,
                "blit_mask(bounds=[{},{},{},{}], clip=[{},{},{},{}])",
                bounds.left,
                bounds.top,
                bounds.right,
                bounds.bottom,
                clip.left,
                clip.top,
                clip.right,
                clip.bottom
            ),
        }
    }
}

// Decodes the sparse run encoding into (run length, alpha) pairs.
fn decode_runs(aa: &[Alpha], runs: &[i16]) -> Vec<(i32, Alpha)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < runs.len() && runs[i] > 0 {
        out.push((i32::from(runs[i]), aa[i]));
        i += usize::try_from(runs[i]).expect("positive run");
    }
    out
}

/// A [`Blitter`] that records every call; see the module documentation.
#[derive(Default)]
pub struct DumpBlitter<'a> {
    /// The calls made so far.
    pub calls: Vec<BlitCall>,
    inner: Option<&'a mut dyn Blitter>,
    mem: BlitMemory,
}

impl std::fmt::Debug for DumpBlitter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DumpBlitter")
            .field("calls", &self.calls)
            .field("forwarding", &self.inner.is_some())
            .finish()
    }
}

impl<'a> DumpBlitter<'a> {
    /// A recorder that only records.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A recorder that also forwards every call to `inner`.
    #[must_use]
    pub fn wrapping(inner: &'a mut dyn Blitter) -> Self {
        DumpBlitter {
            calls: Vec::new(),
            inner: Some(inner),
            mem: BlitMemory::default(),
        }
    }

    /// The recorded calls, one per line.
    #[must_use]
    pub fn dump(&self) -> String {
        let mut s = String::new();
        for c in &self.calls {
            let _ = writeln!(s, "{c}");
        }
        s
    }
}

impl Blitter for DumpBlitter<'_> {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        self.calls.push(BlitCall::H { x, y, width });
        if let Some(inner) = self.inner.as_deref_mut() {
            inner.blit_h(x, y, width);
        }
    }

    fn blit_anti_h(&mut self, x: i32, y: i32, antialias: &mut [Alpha], runs: &mut [i16]) {
        self.calls.push(BlitCall::AntiH {
            x,
            y,
            runs: decode_runs(antialias, runs),
        });
        if let Some(inner) = self.inner.as_deref_mut() {
            inner.blit_anti_h(x, y, antialias, runs);
        }
    }

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        self.calls.push(BlitCall::V {
            x,
            y,
            height,
            alpha,
        });
        if let Some(inner) = self.inner.as_deref_mut() {
            inner.blit_v(x, y, height, alpha);
        }
    }

    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.calls.push(BlitCall::Rect {
            x,
            y,
            width,
            height,
        });
        if let Some(inner) = self.inner.as_deref_mut() {
            inner.blit_rect(x, y, width, height);
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
        self.calls.push(BlitCall::AntiRect {
            x,
            y,
            width,
            height,
            left_alpha,
            right_alpha,
        });
        if let Some(inner) = self.inner.as_deref_mut() {
            inner.blit_anti_rect(x, y, width, height, left_alpha, right_alpha);
        }
    }

    fn blit_anti_h2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        self.calls.push(BlitCall::AntiH2 { x, y, a0, a1 });
        if let Some(inner) = self.inner.as_deref_mut() {
            inner.blit_anti_h2(x, y, a0, a1);
        }
    }

    fn blit_anti_v2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        self.calls.push(BlitCall::AntiV2 { x, y, a0, a1 });
        if let Some(inner) = self.inner.as_deref_mut() {
            inner.blit_anti_v2(x, y, a0, a1);
        }
    }

    fn blit_mask(&mut self, mask: &Mask<'_>, clip: &IRect) {
        self.calls.push(BlitCall::Mask {
            bounds: mask.bounds,
            clip: *clip,
        });
        if let Some(inner) = self.inner.as_deref_mut() {
            inner.blit_mask(mask, clip);
        }
    }

    fn request_rows_preserved(&self) -> i32 {
        self.inner
            .as_deref()
            .map_or(1, Blitter::request_rows_preserved)
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.mem
    }
}
