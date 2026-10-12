// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/include/ParagraphCache.h,
// modules/skparagraph/src/ParagraphCache.cpp

//! `ParagraphCache`: the shaped results of the paragraphs laid out with one font collection.

use std::hash::{Hash, Hasher};
use std::sync::{Mutex, MutexGuard, PoisonError};

use skia_rust_core::checksum::{GoodHash, hash32, mix as checksum_mix};
use skia_rust_core::float_bits::float_to_bits;
use skia_rust_core::floating_point::is_finite;
use skia_rust_core::lru_cache::LruCache;
use skia_rust_core::scalar::{scalar, scalar_round_to_scalar};
use skia_rust_unicode::unicode::{BidiRegion, CodeUnitFlags};

use crate::dart_types::RangeExtensions;
use crate::font_arguments::FontArguments;
use crate::paragraph_impl::Paragraph;
use crate::paragraph_style::ParagraphStyle;
use crate::run::{Cluster, Run};
use crate::text_style::{Block, Placeholder, PlaceholderAlignment, TextIndex};

// Port of: modules/skparagraph/src/ParagraphCache.cpp#L61-L73 (chrome/m156)
/// `relax(a)`: this rounding is done to match Flutter tests. Must be removed..
fn relax(a: scalar) -> i32 {
    if is_finite(a) {
        let threshold = (1 << 12) as scalar;
        float_to_bits(scalar_round_to_scalar(a * threshold) / threshold) as i32
    } else {
        float_to_bits(a) as i32
    }
}

// Port of: modules/skparagraph/src/ParagraphCache.cpp#L75-L77 (chrome/m156)
fn exactly_equal(x: scalar, y: scalar) -> bool {
    #[allow(clippy::float_cmp)] // mirrors `x == y || (x != x && y != y)`
    {
        x == y || (x.is_nan() && y.is_nan())
    }
}

/// `SkGoodHash()` of a `SkRange<size_t>` (16 bytes of object representation).
fn good_hash_range(range: &std::ops::Range<usize>) -> u32 {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&(range.start as u64).to_ne_bytes());
    bytes[8..].copy_from_slice(&(range.end as u64).to_ne_bytes());
    hash32(&bytes, 0)
}

/// `std::hash<std::optional<FontArguments>>`: only the equality of equal arguments matters to the
/// cache, so a deterministic hash of the contents stands in for the C++ library's.
fn hash_font_arguments(args: Option<&FontArguments>) -> usize {
    match args {
        None => 0,
        Some(args) => {
            let text = format!("{args:?}");
            hash32(text.as_bytes(), 0) as usize
        }
    }
}

// Port of: modules/skparagraph/src/ParagraphCache.cpp#L79-L106 (chrome/m156)
/// `ParagraphCacheKey`: what a cached paragraph is looked up by.
#[derive(Debug, Clone)]
struct ParagraphCacheKey {
    text: Vec<u8>,
    placeholders: Vec<Placeholder>,
    text_styles: Vec<Block>,
    paragraph_style: ParagraphStyle,
    hash: u32,
}

impl ParagraphCacheKey {
    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L84-L90 (chrome/m156)
    fn new(paragraph: &Paragraph) -> Self {
        let mut key = Self {
            text: paragraph.text.clone(),
            placeholders: paragraph.placeholders.clone(),
            text_styles: paragraph.text_styles.clone(),
            paragraph_style: paragraph.paragraph_style().clone(),
            hash: 0,
        };
        key.hash = key.compute_hash();
        key
    }

    fn text(&self) -> &[u8] {
        &self.text
    }

    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L144-L150 (chrome/m156)
    fn mix(mut hash: u32, data: u32) -> u32 {
        hash = hash.wrapping_add(data);
        hash = hash.wrapping_add(hash << 10);
        hash ^= hash >> 6;
        hash
    }

    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L152-L207 (chrome/m156)
    fn compute_hash(&self) -> u32 {
        let mut hash: u32 = 0;
        for ph in &self.placeholders {
            if ph.range.width() == 0 {
                continue;
            }
            hash = Self::mix(hash, good_hash_range(&ph.range));
            hash = Self::mix(hash, relax(ph.style.height).good_hash());
            hash = Self::mix(hash, relax(ph.style.width).good_hash());
            hash = Self::mix(hash, (ph.style.alignment as i32).good_hash());
            hash = Self::mix(hash, (ph.style.baseline as i32).good_hash());
            if ph.style.alignment == PlaceholderAlignment::Baseline {
                hash = Self::mix(hash, relax(ph.style.baseline_offset).good_hash());
            }
        }

        for ts in &self.text_styles {
            if ts.style.is_placeholder() {
                continue;
            }
            hash = Self::mix(hash, relax(ts.style.letter_spacing()).good_hash());
            hash = Self::mix(hash, relax(ts.style.word_spacing()).good_hash());
            hash = Self::mix(hash, ts.style.locale().good_hash());
            hash = Self::mix(hash, relax(ts.style.height()).good_hash());
            hash = Self::mix(hash, relax(ts.style.baseline_shift()).good_hash());
            for ff in ts.style.font_families() {
                hash = Self::mix(hash, ff.good_hash());
            }
            for ff in ts.style.font_features() {
                hash = Self::mix(hash, ff.value.good_hash());
                hash = Self::mix(hash, ff.name.good_hash());
            }
            hash = Self::mix(hash, hash_font_arguments(ts.style.font_arguments()) as u32);
            hash = Self::mix(hash, font_style_hash(ts.style.font_style()));
            hash = Self::mix(hash, relax(ts.style.font_size()).good_hash());
            hash = Self::mix(hash, good_hash_range(&ts.range));
        }

        hash = Self::mix(hash, relax(self.paragraph_style.height()).good_hash());
        hash = Self::mix(
            hash,
            (self.paragraph_style.text_direction() as i32).good_hash(),
        );
        hash = Self::mix(
            hash,
            i32::from(self.paragraph_style.replace_tab_characters()).good_hash(),
        );

        let strut_style = self.paragraph_style.strut_style();
        if strut_style.strut_enabled() {
            hash = Self::mix(hash, relax(strut_style.height()).good_hash());
            hash = Self::mix(hash, relax(strut_style.leading()).good_hash());
            hash = Self::mix(hash, relax(strut_style.font_size()).good_hash());
            // `SkGoodHash()(bool)`: one byte.
            hash = Self::mix(hash, hash32(&[u8::from(strut_style.height_override())], 0));
            hash = Self::mix(hash, font_style_hash(strut_style.font_style()));
            hash = Self::mix(
                hash,
                hash32(&[u8::from(strut_style.force_strut_height())], 0),
            );
            for ff in strut_style.font_families() {
                hash = Self::mix(hash, ff.good_hash());
            }
        }

        hash = Self::mix(hash, self.text.good_hash());
        let _ = checksum_mix;
        hash
    }
}

/// `SkGoodHash()(SkFontStyle)`: four bytes, mixed.
fn font_style_hash(style: skia_rust_core::font_style::FontStyle) -> u32 {
    let weight = *style.weight() as u32 & 0xFFFF;
    let width = *style.width() as u32 & 0xFF;
    let slant = style.slant() as u32 & 0xFF;
    (weight | (width << 16) | (slant << 24)).good_hash()
}

impl PartialEq for ParagraphCacheKey {
    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L213-L289 (chrome/m156), and the hash
    // comparison of `THashTable`, which compares the hash first.
    fn eq(&self, other: &Self) -> bool {
        if self.hash != other.hash {
            return false;
        }
        if self.text.len() != other.text.len() {
            return false;
        }
        if self.placeholders.len() != other.placeholders.len() {
            return false;
        }
        if self.text != other.text {
            return false;
        }
        if self.text_styles.len() != other.text_styles.len() {
            return false;
        }
        // There is no need to compare default paragraph styles - they are included into fTextStyles
        if !exactly_equal(
            self.paragraph_style.height(),
            other.paragraph_style.height(),
        ) {
            return false;
        }
        if self.paragraph_style.text_direction() != other.paragraph_style.text_direction() {
            return false;
        }
        if self.paragraph_style.strut_style() != other.paragraph_style.strut_style() {
            return false;
        }
        if self.paragraph_style.replace_tab_characters()
            != other.paragraph_style.replace_tab_characters()
        {
            return false;
        }

        for i in 0..self.text_styles.len() {
            let tsa = &self.text_styles[i];
            let tsb = &other.text_styles[i];
            if tsa.style.is_placeholder() {
                continue;
            }
            if !tsa.style.equals_by_fonts(&tsb.style) {
                return false;
            }
            if tsa.range.width() != tsb.range.width() {
                return false;
            }
            if tsa.range.start != tsb.range.start {
                return false;
            }
        }
        for i in 0..self.placeholders.len() {
            let tsa = &self.placeholders[i];
            let tsb = &other.placeholders[i];
            if tsa.range.width() == 0 && tsb.range.width() == 0 {
                continue;
            }
            if !tsa.style.equals(&tsb.style) {
                return false;
            }
            if tsa.range.width() != tsb.range.width() {
                return false;
            }
            if tsa.range.start != tsb.range.start {
                return false;
            }
        }
        true
    }
}

impl Eq for ParagraphCacheKey {}

impl Hash for ParagraphCacheKey {
    // `KeyHash::operator()` is `key.hash()`.
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(self.hash);
    }
}

// Port of: modules/skparagraph/src/ParagraphCache.cpp#L108-L142 (chrome/m156)
/// `ParagraphCacheValue`: the shaped results of a paragraph.
#[derive(Debug)]
struct ParagraphCacheValue {
    key: ParagraphCacheKey,
    // Shaped results
    runs: Vec<Run>,
    clusters: Vec<Cluster>,
    clusters_index_from_code_unit: Vec<usize>,
    // ICU results
    code_unit_properties: Vec<CodeUnitFlags>,
    words: Vec<usize>,
    bidi_regions: Vec<BidiRegion>,
    has_line_breaks: bool,
    has_whitespaces_inside: bool,
    trailing_spaces: TextIndex,
}

type Checker = Box<dyn Fn(&Paragraph, &str, bool) + Send + Sync>;

// Port of: modules/skparagraph/src/ParagraphCache.cpp#L22-L58 (chrome/m156)
struct Cache {
    lru_cache_map: LruCache<ParagraphCacheKey, Box<ParagraphCacheValue>>,
    cache_is_on: bool,
    /// `fLastCachedValue`: the text of the last value that was added (the value itself may
    /// have been evicted since, which C++ would turn into a dangling pointer).
    last_cached_text: Option<Vec<u8>>,
    total_requests: i32,
    cache_misses: i32,
    hash_misses: i32,
}

const K_MAX_ENTRIES: usize = 128;

impl Cache {
    fn new() -> Self {
        Self {
            lru_cache_map: LruCache::new(K_MAX_ENTRIES),
            cache_is_on: true,
            last_cached_text: None,
            total_requests: 0,
            cache_misses: 0,
            hash_misses: 0,
        }
    }

    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L36-L44 (chrome/m156)
    fn print_statistics(&self) {
        println!("--- Paragraph Cache ---");
        println!("Total requests: {}", self.total_requests);
        println!("Cache misses: {}", self.cache_misses);
        println!(
            "Cache miss %: {}",
            if self.total_requests > 0 {
                100.0 * self.cache_misses as f32 / self.total_requests as f32
            } else {
                0.0
            }
        );
        let cache_hits = self.total_requests - self.cache_misses;
        println!(
            "Hash miss %: {}",
            if cache_hits > 0 {
                100.0 * self.hash_misses as f32 / cache_hits as f32
            } else {
                0.0
            }
        );
        println!("---------------------");
    }
}

// Port of: modules/skparagraph/include/ParagraphCache.h#L20-L48 (chrome/m156)
/// `ParagraphCache`: caches the shaped text of paragraphs by their text, styles and fonts.
pub struct ParagraphCache {
    /// `fParagraphMutex` guards the cache.
    cache: Mutex<Cache>,
    checker: Mutex<Checker>,
}

impl std::fmt::Debug for ParagraphCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParagraphCache").finish_non_exhaustive()
    }
}

impl Default for ParagraphCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ParagraphCache {
    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L291-L298 (chrome/m156)
    /// `ParagraphCache()`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(Cache::new()),
            checker: Mutex::new(Box::new(|_paragraph, _label, _flag| {})),
        }
    }

    fn cache(&self) -> MutexGuard<'_, Cache> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn check(&self, paragraph: &Paragraph, label: &str, flag: bool) {
        let checker = self.checker.lock().unwrap_or_else(PoisonError::into_inner);
        checker(paragraph, label, flag);
    }

    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L300 (chrome/m156)
    /// `turnOn(value)`.
    #[doc(alias = "turnOn")]
    pub fn turn_on(&self, value: bool) {
        self.cache().cache_is_on = value;
    }

    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L301 (chrome/m156)
    /// `count()`.
    #[must_use]
    pub fn count(&self) -> i32 {
        i32::try_from(self.cache().lru_cache_map.count()).unwrap_or(i32::MAX)
    }

    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L303-L324 (chrome/m156)
    /// `updateTo(paragraph, entry)`.
    fn update_to(paragraph: &mut Paragraph, entry: &ParagraphCacheValue) {
        paragraph.runs.clear();
        paragraph.runs.clone_from(&entry.runs);
        paragraph.clusters.clone_from(&entry.clusters);
        paragraph
            .clusters_index_from_code_unit
            .clone_from(&entry.clusters_index_from_code_unit);
        paragraph
            .code_unit_properties
            .clone_from(&entry.code_unit_properties);
        paragraph.words.clone_from(&entry.words);
        paragraph.bidi_regions.clone_from(&entry.bidi_regions);
        paragraph.has_line_breaks = entry.has_line_breaks;
        paragraph.has_whitespaces_inside = entry.has_whitespaces_inside;
        paragraph.trailing_spaces = entry.trailing_spaces;
    }

    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L326-L328 (chrome/m156)
    /// `printStatistics()`.
    #[doc(alias = "printStatistics")]
    pub fn print_statistics(&self) {
        self.cache().print_statistics();
    }

    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L330-L332 (chrome/m156)
    /// `abandon()`.
    pub fn abandon(&self) {
        self.reset();
    }

    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L334-L345 (chrome/m156)
    /// `reset()`.
    pub fn reset(&self) {
        let mut cache = self.cache();
        cache.total_requests = 0;
        cache.cache_misses = 0;
        cache.hash_misses = 0;

        cache.lru_cache_map.reset();
        cache.last_cached_text = None;
    }

    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L347-L371 (chrome/m156)
    /// `findParagraph(paragraph)`: fills the shaped results of `paragraph` from the cache, if
    /// they are there.
    #[doc(alias = "findParagraph")]
    pub fn find_paragraph(&self, paragraph: &mut Paragraph) -> bool {
        let mut cache = self.cache();
        if !cache.cache_is_on {
            return false;
        }
        cache.total_requests += 1;
        let key = ParagraphCacheKey::new(paragraph);
        let Some(entry) = cache.lru_cache_map.find(&key) else {
            // We have a cache miss
            cache.cache_misses += 1;
            self.check(paragraph, "missingParagraph", true);
            return false;
        };
        Self::update_to(paragraph, entry);
        self.check(paragraph, "foundParagraph", true);
        true
    }

    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L373-L399 (chrome/m156)
    /// `updateParagraph(paragraph)`: adds the shaped results of `paragraph` to the cache.
    #[doc(alias = "updateParagraph")]
    pub fn update_paragraph(&self, paragraph: &mut Paragraph) -> bool {
        let mut cache = self.cache();
        if !cache.cache_is_on {
            return false;
        }
        cache.total_requests += 1;
        let key = ParagraphCacheKey::new(paragraph);
        if cache.lru_cache_map.find(&key).is_none() {
            // isTooMuchMemoryWasted(paragraph) not needed for now
            if Self::is_possibly_text_editing_in(&cache, paragraph) {
                // Skip this paragraph
                return false;
            }
            let value = Box::new(ParagraphCacheValue {
                key: key.clone(),
                runs: paragraph.runs.clone(),
                clusters: paragraph.clusters.clone(),
                clusters_index_from_code_unit: paragraph.clusters_index_from_code_unit.clone(),
                code_unit_properties: paragraph.code_unit_properties.clone(),
                words: paragraph.words.clone(),
                bidi_regions: paragraph.bidi_regions.clone(),
                has_line_breaks: paragraph.has_line_breaks,
                has_whitespaces_inside: paragraph.has_whitespaces_inside,
                trailing_spaces: paragraph.trailing_spaces,
            });
            cache.last_cached_text = Some(value.key.text().to_vec());
            cache.lru_cache_map.insert(key, value);
            self.check(paragraph, "addedParagraph", true);
            true
        } else {
            // We do not have to update the paragraph
            false
        }
    }

    // Port of: modules/skparagraph/src/ParagraphCache.cpp#L401-L428 (chrome/m156)
    /// Special situation: (very) long paragraph that is close to the last formatted paragraph.
    fn is_possibly_text_editing_in(cache: &Cache, paragraph: &Paragraph) -> bool {
        const NOCACHE_PREFIX_LENGTH: usize = 40;
        let Some(last_text) = &cache.last_cached_text else {
            return false;
        };
        let text = &paragraph.text;
        if last_text.len() < NOCACHE_PREFIX_LENGTH || text.len() < NOCACHE_PREFIX_LENGTH {
            // Either last text or the current are too short
            return false;
        }

        if last_text[..NOCACHE_PREFIX_LENGTH] == text[..NOCACHE_PREFIX_LENGTH] {
            // Texts have the same starts
            return true;
        }

        if last_text[last_text.len() - NOCACHE_PREFIX_LENGTH..]
            == text[text.len() - NOCACHE_PREFIX_LENGTH..]
        {
            // Texts have the same ends
            return true;
        }

        // It does not look like editing the text
        false
    }

    /// `isPossiblyTextEditing(paragraph)`.
    #[doc(alias = "isPossiblyTextEditing")]
    pub fn is_possibly_text_editing(&self, paragraph: &Paragraph) -> bool {
        Self::is_possibly_text_editing_in(&self.cache(), paragraph)
    }

    /// `setChecker(checker)`: a function called with the paragraph, a label and a flag at each
    /// cache event (for testing).
    #[doc(alias = "setChecker")]
    pub fn set_checker(&self, checker: impl Fn(&Paragraph, &str, bool) + Send + Sync + 'static) {
        *self.checker.lock().unwrap_or_else(PoisonError::into_inner) = Box::new(checker);
    }
}
