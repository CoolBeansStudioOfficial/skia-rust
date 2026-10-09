// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/MemsetBench.cpp

//! `SkOpts::memset16/32/64` over buffers of 16 bytes to 64 KiB (non-rendering benches).

use skia_rust_simd::memset::{memset16, memset32, memset64};

use crate::def_bench;
use crate::prelude::*;

/// The element type `T` of `MemsetBench<T>`.
#[derive(Clone, Copy, Debug)]
enum Width {
    U16,
    U32,
    U64,
}

/// The buffer `AutoTMalloc<T>` of `MemsetBench<T>`; uninitialized in C++, zeroed here.
enum Buffer {
    U16(Vec<u16>),
    U32(Vec<u32>),
    U64(Vec<u64>),
}

/// `template <typename T> class MemsetBench`.
// Port of: bench/MemsetBench.cpp#L11-L28 (chrome/m156)
struct MemsetBench {
    /// `fN(bytes / sizeof(T))`.
    n: usize,
    buffer: Buffer,
    name: String,
}

impl MemsetBench {
    // Port of: bench/MemsetBench.cpp#L14-L18 (chrome/m156)
    fn new(width: Width, bytes: usize) -> Self {
        let (bits, n) = match width {
            Width::U16 => (16, bytes / 2),
            Width::U32 => (32, bytes / 4),
            Width::U64 => (64, bytes / 8),
        };
        let buffer = match width {
            Width::U16 => Buffer::U16(vec![0; n]),
            Width::U32 => Buffer::U32(vec![0; n]),
            Width::U64 => Buffer::U64(vec![0; n]),
        };
        // SkStringPrintf("memset%zu_%zu", sizeof(T)*8, bytes)
        Self {
            n,
            buffer,
            name: format!("memset{bits}_{bytes}"),
        }
    }
}

impl Benchmark for MemsetBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        match &mut self.buffer {
            // SkOpts::memset64(fBuffer.get(), 0xFACEFACEFACEFACE, fN);
            Buffer::U64(buf) => {
                for _ in 0..1000 * loops {
                    memset64(buf, 0xFACE_FACE_FACE_FACE, self.n);
                }
            }
            // SkOpts::memset32(fBuffer.get(), 0xFACEB004, fN);
            Buffer::U32(buf) => {
                for _ in 0..1000 * loops {
                    memset32(buf, 0xFACE_B004, self.n);
                }
            }
            // SkOpts::memset16(fBuffer.get(), 0x4973, fN);
            Buffer::U16(buf) => {
                for _ in 0..1000 * loops {
                    memset16(buf, 0x4973, self.n);
                }
            }
        }
    }
}

/// Registers `DEF_BENCH(return (new MemsetBench<T>(bytes)))`.
macro_rules! def_memset_bench {
    ($test:ident, $name:literal, $width:expr, $bytes:literal) => {
        def_bench!($test = $name, MemsetBench::new($width, $bytes));
    };
}

// Port of: bench/MemsetBench.cpp#L51-L59 (chrome/m156)
def_memset_bench!(
    memset_uint64_16,
    "(new MemsetBench<uint64_t>(16))",
    Width::U64,
    16
);
def_memset_bench!(
    memset_uint64_64,
    "(new MemsetBench<uint64_t>(64))",
    Width::U64,
    64
);
def_memset_bench!(
    memset_uint64_256,
    "(new MemsetBench<uint64_t>(256))",
    Width::U64,
    256
);
def_memset_bench!(
    memset_uint64_512,
    "(new MemsetBench<uint64_t>(512))",
    Width::U64,
    512
);
def_memset_bench!(
    memset_uint64_768,
    "(new MemsetBench<uint64_t>(768))",
    Width::U64,
    768
);
def_memset_bench!(
    memset_uint64_1024,
    "(new MemsetBench<uint64_t>(1024))",
    Width::U64,
    1024
);
def_memset_bench!(
    memset_uint64_2048,
    "(new MemsetBench<uint64_t>(2048))",
    Width::U64,
    2048
);
def_memset_bench!(
    memset_uint64_4096,
    "(new MemsetBench<uint64_t>(4096))",
    Width::U64,
    4096
);
def_memset_bench!(
    memset_uint64_65536,
    "(new MemsetBench<uint64_t>(65536))",
    Width::U64,
    65536
);

// Port of: bench/MemsetBench.cpp#L61-L69 (chrome/m156)
def_memset_bench!(
    memset_uint32_16,
    "(new MemsetBench<uint32_t>(16))",
    Width::U32,
    16
);
def_memset_bench!(
    memset_uint32_64,
    "(new MemsetBench<uint32_t>(64))",
    Width::U32,
    64
);
def_memset_bench!(
    memset_uint32_256,
    "(new MemsetBench<uint32_t>(256))",
    Width::U32,
    256
);
def_memset_bench!(
    memset_uint32_512,
    "(new MemsetBench<uint32_t>(512))",
    Width::U32,
    512
);
def_memset_bench!(
    memset_uint32_768,
    "(new MemsetBench<uint32_t>(768))",
    Width::U32,
    768
);
def_memset_bench!(
    memset_uint32_1024,
    "(new MemsetBench<uint32_t>(1024))",
    Width::U32,
    1024
);
def_memset_bench!(
    memset_uint32_2048,
    "(new MemsetBench<uint32_t>(2048))",
    Width::U32,
    2048
);
def_memset_bench!(
    memset_uint32_4096,
    "(new MemsetBench<uint32_t>(4096))",
    Width::U32,
    4096
);
def_memset_bench!(
    memset_uint32_65536,
    "(new MemsetBench<uint32_t>(65536))",
    Width::U32,
    65536
);

// Port of: bench/MemsetBench.cpp#L71-L79 (chrome/m156)
def_memset_bench!(
    memset_uint16_16,
    "(new MemsetBench<uint16_t>(16))",
    Width::U16,
    16
);
def_memset_bench!(
    memset_uint16_64,
    "(new MemsetBench<uint16_t>(64))",
    Width::U16,
    64
);
def_memset_bench!(
    memset_uint16_256,
    "(new MemsetBench<uint16_t>(256))",
    Width::U16,
    256
);
def_memset_bench!(
    memset_uint16_512,
    "(new MemsetBench<uint16_t>(512))",
    Width::U16,
    512
);
def_memset_bench!(
    memset_uint16_768,
    "(new MemsetBench<uint16_t>(768))",
    Width::U16,
    768
);
def_memset_bench!(
    memset_uint16_1024,
    "(new MemsetBench<uint16_t>(1024))",
    Width::U16,
    1024
);
def_memset_bench!(
    memset_uint16_2048,
    "(new MemsetBench<uint16_t>(2048))",
    Width::U16,
    2048
);
def_memset_bench!(
    memset_uint16_4096,
    "(new MemsetBench<uint16_t>(4096))",
    Width::U16,
    4096
);
def_memset_bench!(
    memset_uint16_65536,
    "(new MemsetBench<uint16_t>(65536))",
    Width::U16,
    65536
);
