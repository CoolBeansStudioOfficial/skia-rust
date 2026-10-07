// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! [`Case`]: one raster pipeline run (buffers, stages with contexts, rects), and its text form,
//! which the C++ driver (`oracle/rp-diff/cpp/driver.cpp`) reads.
//!
//! # Text format
//! Line based, whitespace separated, `#` starts a comment line:
//!
//! ```text
//! case <name> <highp|auto> <run|compile>
//! buf <len> <stride> <origin> <hex|->        one per slot, in slot order; `-` = zeros
//! stage <op> <context>                       Skia's op name, in pipeline order
//! run <x> <y> <w> <h>                        one or more
//! end
//! ```
//!
//! - `highp` forces the highp pipeline (`gForceHighPrecisionRasterPipeline`); `auto` lets the
//!   builder pick lowp when every op has a lowp stage on the tier (never on `Scalar`).
//! - `run` builds the pipeline afresh for every rect (`SkRasterPipeline::run`, zeroed tail
//!   scratch each time); `compile` builds it once (`compile()`) and runs every rect on it, so
//!   tail scratch persists between rects (design §1.7).
//! - Buffer `stride` (pixels) and `origin` (bytes, may be negative) are what a `mem` context
//!   sees (Skia: `MemoryCtx { pixels = bytes + origin, stride }`).
//! - Contexts ([`Ctx`]): `-`, `ptr <slot> <byte offset>`, `mem <slot>`, `f32 <n> <bits>...`
//!   (hex `f32` bits; Skia: a `const float*`), `u8x4 <b0> <b1> <b2> <b3>` (packed into the
//!   pointer value), `branch <offset>`, `branch_eq <offset> <value> <slot> <byte offset>`,
//!   `uniform_color <r> <g> <b> <a> <r16> <g16> <b16> <a16>` (hex `f32` bits, then decimal),
//!   `gather <stride> <width bits> <height bits> <round_down 0|1> <len> <hex>` (a `GatherCtx`
//!   with its own copy of the pixels; `stride` is in pixels).
//!   `emboss <mul slot> <add slot>` (`EmbossCtx`: two `MemoryCtx` over buffers), `tables <hex>`
//!   (`TablesCtx`: four 256-byte tables r, g, b, a, 2048 hex digits).
//!
//! The output of a case is the bytes of all its buffers after the runs, concatenated in slot
//! order; results are compared (and stored) as [`fnv1a`] hashes of them.

use core::fmt::Write as _;

use skia_rust_simd::rp::Op;

/// FNV-1a (64-bit) over bytes: the hash of case texts and outputs.
#[must_use]
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// A buffer bound to one slot ([`skia_rust_simd::rp::MemSlot`]`(index)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Buffer {
    /// Initial bytes.
    pub bytes: Vec<u8>,
    /// Row stride in pixels, for `mem` contexts.
    pub stride: isize,
    /// Byte offset of pixel `(0, 0)`, for `mem` contexts.
    pub origin: isize,
}

impl Buffer {
    /// A buffer with these initial bytes (stride 0, origin 0).
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Buffer {
        Buffer {
            bytes,
            stride: 0,
            origin: 0,
        }
    }

    /// `len` zero bytes.
    #[must_use]
    pub fn zeroed(len: usize) -> Buffer {
        Buffer::new(vec![0; len])
    }

    /// Little-endian bytes of `words`.
    #[must_use]
    pub fn from_u32s(words: &[u32]) -> Buffer {
        Buffer::new(words.iter().flat_map(|w| w.to_le_bytes()).collect())
    }

    /// Little-endian bytes of `halves`.
    #[must_use]
    pub fn from_u16s(halves: &[u16]) -> Buffer {
        Buffer::new(halves.iter().flat_map(|w| w.to_le_bytes()).collect())
    }

    /// Sets the stride (pixels) and origin (bytes) seen by `mem` contexts.
    #[must_use]
    pub fn with_layout(mut self, stride: isize, origin: isize) -> Buffer {
        self.stride = stride;
        self.origin = origin;
        self
    }
}

/// A stage's context, serialized. See the [module docs](self) for the C++ side of each.
#[derive(Clone, Debug, PartialEq)]
pub enum Ctx {
    /// No context (`nullptr`).
    None,
    /// A pointer into a buffer: [`skia_rust_simd::rp::MemPtr`].
    Ptr {
        /// Buffer slot.
        slot: u16,
        /// Byte offset.
        offset: u32,
    },
    /// Pixel memory: [`skia_rust_simd::rp::MemoryCtx`] (the buffer's stride and origin).
    Mem {
        /// Buffer slot.
        slot: u16,
    },
    /// Constant floats (`const float*` in Skia; `f32`, `[f32; N]`, `&[f32; N]`, `&Cell<f32>`,
    /// `&TransferFunction` in Rust).
    F32(Vec<f32>),
    /// Four bytes packed into the context pointer (`swizzle`).
    U8x4([u8; 4]),
    /// `BranchCtx` (and `BranchIfAllLanesActiveCtx`, whose tail pointer the builder sets).
    Branch {
        /// Program-counter offset.
        offset: i32,
    },
    /// `BranchIfEqualCtx`.
    BranchEq {
        /// Program-counter offset.
        offset: i32,
        /// Compared value.
        value: i32,
        /// Buffer slot of the `N` ints.
        slot: u16,
        /// Byte offset of the ints.
        byte_offset: u32,
    },
    /// `EmbossCtx`: two A8 memory contexts.
    Emboss {
        /// Buffer slot of the multiplier plane.
        mul: u16,
        /// Buffer slot of the addend plane.
        add: u16,
    },
    /// `TablesCtx`: four 256-byte tables (`r`, `g`, `b`, `a`), 1024 bytes in all.
    Tables(Vec<u8>),
    /// `UniformColorCtx`.
    UniformColor {
        /// `r, g, b, a`.
        rgba: [f32; 4],
        /// `rgba[4]` (`[0, 255]` in 16-bit lanes).
        rgba16: [u16; 4],
    },
    /// `GatherCtx`, with its own copy of the pixels (gathers are never patched).
    Gather {
        /// The pixels, from `(0, 0)`.
        pixels: Vec<u8>,
        /// Row stride in pixels.
        stride: i32,
        /// `GatherCtx::width`.
        width: f32,
        /// `GatherCtx::height`.
        height: f32,
        /// `GatherCtx::round_down_at_integer`.
        round_down_at_integer: bool,
    },
}

/// One stage: an op and its context.
#[derive(Clone, Debug, PartialEq)]
pub struct StageSpec {
    /// The op.
    pub op: Op,
    /// Its context.
    pub ctx: Ctx,
}

impl StageSpec {
    /// A stage without a context.
    #[must_use]
    pub fn new(op: Op) -> StageSpec {
        StageSpec { op, ctx: Ctx::None }
    }

    /// A stage with a context.
    #[must_use]
    pub fn with(op: Op, ctx: Ctx) -> StageSpec {
        StageSpec { op, ctx }
    }
}

/// A run rect: `start_pipeline(x, y, x + w, y + h)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    /// Left.
    pub x: usize,
    /// Top.
    pub y: usize,
    /// Width.
    pub w: usize,
    /// Height.
    pub h: usize,
}

impl Rect {
    /// `(x, y, w, h)`.
    #[must_use]
    pub const fn new(x: usize, y: usize, w: usize, h: usize) -> Rect {
        Rect { x, y, w, h }
    }
}

/// One rp-diff case.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// Unique name, `<group>/<detail>/...` (no whitespace).
    pub name: String,
    /// Force the highp pipeline.
    pub force_highp: bool,
    /// Compile once and run every rect on it (else build per run).
    pub compiled: bool,
    /// Buffers by slot.
    pub buffers: Vec<Buffer>,
    /// Stages in order.
    pub stages: Vec<StageSpec>,
    /// Rects, run in order.
    pub runs: Vec<Rect>,
}

impl Case {
    /// The case's text block (see the [module docs](self)).
    ///
    /// # Panics
    /// If the name contains whitespace.
    #[must_use]
    pub fn to_text(&self) -> String {
        assert!(
            !self.name.is_empty() && !self.name.contains(char::is_whitespace),
            "rp-diff case names must be non-empty without whitespace: {:?}",
            self.name
        );
        let mut s = String::new();
        let precision = if self.force_highp { "highp" } else { "auto" };
        let mode = if self.compiled { "compile" } else { "run" };
        let _ = writeln!(s, "case {} {precision} {mode}", self.name);
        for b in &self.buffers {
            let _ = write!(s, "buf {} {} {} ", b.bytes.len(), b.stride, b.origin);
            if b.bytes.iter().all(|&x| x == 0) {
                s.push('-');
            } else {
                for x in &b.bytes {
                    let _ = write!(s, "{x:02x}");
                }
            }
            s.push('\n');
        }
        for st in &self.stages {
            let _ = write!(s, "stage {} ", st.op.name());
            write_ctx(&mut s, &st.ctx);
            s.push('\n');
        }
        for r in &self.runs {
            let _ = writeln!(s, "run {} {} {} {}", r.x, r.y, r.w, r.h);
        }
        s.push_str("end\n");
        s
    }

    /// [`fnv1a`] of [`to_text`](Self::to_text): identifies the case's exact content, so stored
    /// results of an edited case are detected as stale.
    #[must_use]
    pub fn hash(&self) -> u64 {
        fnv1a(self.to_text().as_bytes())
    }

    /// The length of the case's output (all buffers).
    #[must_use]
    pub fn output_len(&self) -> usize {
        self.buffers.iter().map(|b| b.bytes.len()).sum()
    }
}

fn write_ctx(s: &mut String, ctx: &Ctx) {
    let _ = match ctx {
        Ctx::None => write!(s, "-"),
        Ctx::Ptr { slot, offset } => write!(s, "ptr {slot} {offset}"),
        Ctx::Mem { slot } => write!(s, "mem {slot}"),
        Ctx::F32(v) => {
            let _ = write!(s, "f32 {}", v.len());
            for f in v {
                let _ = write!(s, " {:08x}", f.to_bits());
            }
            Ok(())
        }
        Ctx::U8x4(b) => write!(s, "u8x4 {} {} {} {}", b[0], b[1], b[2], b[3]),
        Ctx::Branch { offset } => write!(s, "branch {offset}"),
        Ctx::BranchEq {
            offset,
            value,
            slot,
            byte_offset,
        } => write!(s, "branch_eq {offset} {value} {slot} {byte_offset}"),
        Ctx::Emboss { mul, add } => write!(s, "emboss {mul} {add}"),
        Ctx::Tables(t) => {
            assert_eq!(t.len(), 1024, "a TablesCtx holds four 256-byte tables");
            let _ = write!(s, "tables ");
            for b in t {
                let _ = write!(s, "{b:02x}");
            }
            Ok(())
        }
        Ctx::UniformColor { rgba, rgba16 } => {
            let _ = write!(s, "uniform_color");
            for f in rgba {
                let _ = write!(s, " {:08x}", f.to_bits());
            }
            for h in rgba16 {
                let _ = write!(s, " {h}");
            }
            Ok(())
        }
        Ctx::Gather {
            pixels,
            stride,
            width,
            height,
            round_down_at_integer,
        } => {
            let _ = write!(
                s,
                "gather {stride} {:08x} {:08x} {} {} ",
                width.to_bits(),
                height.to_bits(),
                u8::from(*round_down_at_integer),
                pixels.len()
            );
            for x in pixels {
                let _ = write!(s, "{x:02x}");
            }
            Ok(())
        }
    };
}

/// The text of a whole case file.
#[must_use]
pub fn cases_to_text(cases: &[Case]) -> String {
    let mut s = String::from(
        "# oracle/rp-diff cases (generated from oracle/rp-diff/src/cases.rs; see case.rs)\n",
    );
    for c in cases {
        s.push_str(&c.to_text());
    }
    s
}

/// Matches `name` against a glob where `*` matches any run of characters (including `/`).
#[must_use]
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !name.starts_with(first) || name.len() < first.len() + last.len() {
        return false;
    }
    let mut rest = &name[first.len()..name.len() - last.len()];
    if !name.ends_with(last) {
        return false;
    }
    for mid in &parts[1..parts.len() - 1] {
        match rest.find(mid) {
            Some(i) => rest = &rest[i + mid.len()..],
            None => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob() {
        assert!(glob_match("srcover/*", "srcover/auto/w4"));
        assert!(glob_match("*", "x"));
        assert!(glob_match("*w4", "srcover/auto/w4"));
        assert!(glob_match("s*/a*/w*", "srcover/auto/w4"));
        assert!(!glob_match("srcover/*", "seed_shader/x"));
        assert!(!glob_match("a*a", "a"));
        assert!(glob_match("exact", "exact"));
        assert!(!glob_match("exact", "exact2"));
    }

    #[test]
    fn text_form() {
        let c = Case {
            name: "t/1".into(),
            force_highp: true,
            compiled: false,
            buffers: vec![
                Buffer::new(vec![1, 0xab]),
                Buffer::zeroed(3).with_layout(4, -8),
            ],
            stages: vec![
                StageSpec::with(Op::LoadSrc, Ctx::Ptr { slot: 0, offset: 4 }),
                StageSpec::new(Op::Srcover),
                StageSpec::with(Op::Jump, Ctx::Branch { offset: 2 }),
                StageSpec::with(Op::SetRgb, Ctx::F32(vec![1.0, -0.0])),
            ],
            runs: vec![Rect::new(1, 2, 3, 4)],
        };
        assert_eq!(
            c.to_text(),
            "case t/1 highp run\nbuf 2 0 0 01ab\nbuf 3 4 -8 -\nstage load_src ptr 0 4\n\
             stage srcover -\nstage jump branch 2\nstage set_rgb f32 2 3f800000 80000000\n\
             run 1 2 3 4\nend\n"
        );
        assert_eq!(c.output_len(), 5);
    }
}
