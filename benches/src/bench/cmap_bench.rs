// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/CmapBench.cpp

//! `CMAPBench`: character-to-glyph lookups (`textToGlyphs`, `unicharsToGlyphs`, and the
//! `SkCharToGlyphCache` add and find paths) over 10 and 100 random code points (non-rendering).

use skia_rust_core::font::Font;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::random::Random;
use skia_rust_core::utf::Unichar;
use skia_rust_core::utils::char_to_glyph_cache::CharToGlyphCache;
use skia_rust_tools::font_tool_utils::default_typeface;

use crate::def_bench;
use crate::prelude::*;

/// `enum { NGLYPHS = 100 }`.
// Port of: bench/CmapBench.cpp#L18-L20 (chrome/m156)
const NGLYPHS: usize = 100;

/// `SMALL` and `BIG`: the number of code points a bench uses.
// Port of: bench/CmapBench.cpp#L114 and #L121 (chrome/m156)
const SMALL: usize = 10;
const BIG: usize = 100;

/// `TypefaceProc`: the function a `CMAPBench` times.
// Port of: bench/CmapBench.cpp#L32 (chrome/m156)
#[derive(Clone, Copy, Debug)]
enum TypefaceProc {
    /// `textToGlyphs_proc`.
    TextToGlyphs,
    /// `charsToGlyphs_proc`.
    CharsToGlyphs,
    /// `addcache_proc`.
    AddCache,
    /// `findcache_proc`.
    FindCache,
}

/// `class CMAPBench`.
// Port of: bench/CmapBench.cpp#L70-L110 (chrome/m156)
struct CmapBench {
    proc: TypefaceProc,
    /// `fName`: the base name and `_count`.
    name: String,
    /// `fText`: the `NGLYPHS` code points; only the first `count` are used.
    text: [Unichar; NGLYPHS],
    /// `fText` as the bytes `textToGlyphs` reads for `kUTF32` (native order; `count * 4` used).
    text_bytes: Vec<u8>,
    font: Font,
    cache: CharToGlyphCache,
    count: usize,
}

impl CmapBench {
    // Port of: bench/CmapBench.cpp#L70-L101 (chrome/m156)
    fn new(proc: TypefaceProc, name: &str, count: usize) -> Self {
        // SkASSERT(count <= NGLYPHS);
        debug_assert!(count <= NGLYPHS);

        let mut text = [0; NGLYPHS];
        let mut cache = CharToGlyphCache::new();
        // SkRandom rand;
        let mut rand = Random::default();
        for (i, uni) in text.iter_mut().enumerate().take(count) {
            // fText[i] = rand.nextU() & 0xFFFF;
            *uni = Unichar::try_from(rand.next_u() & 0xFFFF).expect("masked to 16 bits");
            // fCache.addCharAndGlyph(fText[i], i);
            cache.add_char_and_glyph(*uni, GlyphId::try_from(i).expect("below NGLYPHS"));
        }
        let mut font = Font::default();
        // fFont.setTypeface(ToolUtils::DefaultTypeface());
        font.set_typeface(Some(default_typeface()));

        let text_bytes = text.iter().flat_map(|u| u.to_ne_bytes()).collect();
        Self {
            proc,
            // SkStringPrintf(&fName, "%s_%d", name, count).
            name: format!("{name}_{count}"),
            text,
            text_bytes,
            font,
            cache,
            count,
        }
    }
}

impl Benchmark for CmapBench {
    // Port of: bench/CmapBench.cpp#L103-L108 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/CmapBench.cpp#L107-L109 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let r = Rec {
            cache: &self.cache,
            loops,
            font: &self.font,
            text: &self.text,
            text_bytes: &self.text_bytes,
            count: self.count,
        };
        match self.proc {
            TypefaceProc::TextToGlyphs => text_to_glyphs_proc(&r),
            TypefaceProc::CharsToGlyphs => chars_to_glyphs_proc(&r),
            TypefaceProc::AddCache => addcache_proc(&r),
            TypefaceProc::FindCache => findcache_proc(&r),
        }
    }
}

/// `struct Rec`: what a `TypefaceProc` gets.
// Port of: bench/CmapBench.cpp#L23-L29 (chrome/m156)
struct Rec<'a> {
    cache: &'a CharToGlyphCache,
    loops: i32,
    font: &'a Font,
    text: &'a [Unichar; NGLYPHS],
    text_bytes: &'a [u8],
    count: usize,
}

// Port of: bench/CmapBench.cpp#L34-L41 (chrome/m156)
fn text_to_glyphs_proc(r: &Rec<'_>) {
    let mut glyphs = [0; NGLYPHS];
    // SkASSERT(r.fCount <= NGLYPHS);
    debug_assert!(r.count <= NGLYPHS);

    for _ in 0..r.loops {
        // r.fFont.textToGlyphs(r.fText, r.fCount*4, SkTextEncoding::kUTF32, glyphs);
        let _ = r.font.text_to_glyphs(
            &r.text_bytes[..r.count * 4],
            TextEncoding::UTF32,
            &mut glyphs,
        );
    }
}

// Port of: bench/CmapBench.cpp#L43-L51 (chrome/m156)
fn chars_to_glyphs_proc(r: &Rec<'_>) {
    let mut glyphs = [0; NGLYPHS];
    // SkASSERT(r.fCount <= NGLYPHS);
    debug_assert!(r.count <= NGLYPHS);

    // SkTypeface* face = r.fFont.getTypeface();
    let face = r.font.typeface();
    for _ in 0..r.loops {
        // face->unicharsToGlyphs({r.fText, (size_t)r.fCount}, glyphs);
        face.unichars_to_glyphs(&r.text[..r.count], &mut glyphs);
    }
}

// Port of: bench/CmapBench.cpp#L53-L60 (chrome/m156)
fn addcache_proc(r: &Rec<'_>) {
    for _ in 0..r.loops {
        // SkCharToGlyphCache cache;
        let mut cache = CharToGlyphCache::new();
        for (i, &uni) in r.text.iter().enumerate().take(r.count) {
            // cache.addCharAndGlyph(r.fText[i], i);
            cache.add_char_and_glyph(uni, GlyphId::try_from(i).expect("below NGLYPHS"));
        }
    }
}

// Port of: bench/CmapBench.cpp#L62-L68 (chrome/m156)
fn findcache_proc(r: &Rec<'_>) {
    for _ in 0..r.loops {
        for &uni in r.text.iter().take(r.count) {
            // r.fCache.findGlyphIndex(r.fText[i]);
            let _ = r.cache.find_glyph_index(uni);
        }
    }
}

/// `DEF_BENCH(return new CMAPBench(proc, name, count);)`.
macro_rules! def_cmap_bench {
    ($test:ident, $name:literal, $proc:expr, $base:literal, $count:expr) => {
        def_bench!($test = $name, CmapBench::new($proc, $base, $count));
    };
}

// Port of: bench/CmapBench.cpp#L116-L119 (chrome/m156)
def_cmap_bench!(
    cmap_text_to_glyphs_small,
    "CMAPBench(textToGlyphs_proc, \"font_charToGlyph\", SMALL)",
    TypefaceProc::TextToGlyphs,
    "font_charToGlyph",
    SMALL
);
def_cmap_bench!(
    cmap_chars_to_glyphs_small,
    "CMAPBench(charsToGlyphs_proc, \"face_charToGlyph\", SMALL)",
    TypefaceProc::CharsToGlyphs,
    "face_charToGlyph",
    SMALL
);
def_cmap_bench!(
    cmap_addcache_small,
    "CMAPBench(addcache_proc, \"addcache_charToGlyph\", SMALL)",
    TypefaceProc::AddCache,
    "addcache_charToGlyph",
    SMALL
);
def_cmap_bench!(
    cmap_findcache_small,
    "CMAPBench(findcache_proc, \"findcache_charToGlyph\", SMALL)",
    TypefaceProc::FindCache,
    "findcache_charToGlyph",
    SMALL
);

// Port of: bench/CmapBench.cpp#L123-L126 (chrome/m156)
def_cmap_bench!(
    cmap_text_to_glyphs_big,
    "CMAPBench(textToGlyphs_proc, \"font_charToGlyph\", BIG)",
    TypefaceProc::TextToGlyphs,
    "font_charToGlyph",
    BIG
);
def_cmap_bench!(
    cmap_chars_to_glyphs_big,
    "CMAPBench(charsToGlyphs_proc, \"face_charToGlyph\", BIG)",
    TypefaceProc::CharsToGlyphs,
    "face_charToGlyph",
    BIG
);
def_cmap_bench!(
    cmap_addcache_big,
    "CMAPBench(addcache_proc, \"addcache_charToGlyph\", BIG)",
    TypefaceProc::AddCache,
    "addcache_charToGlyph",
    BIG
);
def_cmap_bench!(
    cmap_findcache_big,
    "CMAPBench(findcache_proc, \"findcache_charToGlyph\", BIG)",
    TypefaceProc::FindCache,
    "findcache_charToGlyph",
    BIG
);
