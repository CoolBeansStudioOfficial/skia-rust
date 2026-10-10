// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkClusterator.{h,cpp} (chrome/m156)

//! `SkClusterator`: given a glyph run with the text it was shaped from, iterates over the
//! clusters (the groups of glyphs that stand for the same text).

use skia_rust_core::glyph_run::GlyphRun;

/// `SkClusterator::Cluster`: the text of a cluster (`None` when the run has no text), and the
/// glyphs it makes.
///
/// Two clusters are equal when their text is the same bytes of the same run, as Skia compares
/// the pointers.
// Port of: src/pdf/SkClusterator.h#L23-L34 (chrome/m156)
#[doc(alias = "SkClusterator::Cluster")]
#[derive(Debug, Clone, Copy)]
pub struct Cluster<'a> {
    /// `fUtf8Text`.
    pub utf8_text: Option<&'a [u8]>,
    /// `fTextByteLength`.
    pub text_byte_length: u32,
    /// `fGlyphIndex`.
    pub glyph_index: u32,
    /// `fGlyphCount`.
    pub glyph_count: u32,
}

impl Cluster<'_> {
    /// `explicit operator bool`: whether the cluster has glyphs.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.glyph_count != 0
    }
}

impl PartialEq for Cluster<'_> {
    // Port of: src/pdf/SkClusterator.h#L29-L33 (chrome/m156)
    fn eq(&self, o: &Self) -> bool {
        let same_text = match (self.utf8_text, o.utf8_text) {
            (None, None) => true,
            (Some(a), Some(b)) => std::ptr::eq(a.as_ptr(), b.as_ptr()),
            _ => false,
        };
        same_text
            && self.text_byte_length == o.text_byte_length
            && self.glyph_index == o.glyph_index
            && self.glyph_count == o.glyph_count
    }
}

/// `SkClusterator`.
// Port of: src/pdf/SkClusterator.h#L19-L45 (chrome/m156)
#[doc(alias = "SkClusterator")]
#[derive(Debug)]
pub struct Clusterator<'a> {
    clusters: Option<&'a [u32]>,
    utf8_text: Option<&'a [u8]>,
    glyph_count: u32,
    text_byte_length: u32,
    reversed_chars: bool,
    current_glyph_index: u32,
}

// Port of: src/pdf/SkClusterator.cpp#L16-L28 (chrome/m156)
fn is_reversed(clusters: &[u32], count: u32) -> bool {
    // "ReversedChars" is how PDF deals with RTL text.
    // return true if more than one cluster and monotonicly decreasing to zero.
    if count < 2 || clusters[0] == 0 || clusters[(count - 1) as usize] != 0 {
        return false;
    }
    for i in 0..(count - 1) as usize {
        if clusters[i + 1] > clusters[i] {
            return false;
        }
    }
    true
}

impl<'a> Clusterator<'a> {
    /// `SkClusterator(const sktext::GlyphRun&)`.
    // Port of: src/pdf/SkClusterator.cpp#L30-L43 (chrome/m156)
    #[must_use]
    #[allow(clippy::missing_panics_doc)] // asserts the C++ preconditions
    pub fn new(run: &'a GlyphRun) -> Self {
        let clusters = if run.clusters().is_empty() {
            None
        } else {
            Some(run.clusters())
        };
        let utf8_text = if run.text().is_empty() {
            None
        } else {
            Some(run.text())
        };
        let glyph_count = u32::try_from(run.glyph_ids().len()).expect("fits in 32 bits");
        let text_byte_length = u32::try_from(run.text().len()).expect("fits in 32 bits");
        let reversed_chars = clusters.is_some_and(|c| is_reversed(c, glyph_count));
        if clusters.is_some() {
            debug_assert!(utf8_text.is_some() && text_byte_length > 0 && glyph_count > 0);
        } else {
            debug_assert!(utf8_text.is_none() && text_byte_length == 0);
        }
        Self {
            clusters,
            utf8_text,
            glyph_count,
            text_byte_length,
            reversed_chars,
            current_glyph_index: 0,
        }
    }

    /// `glyphCount`.
    #[must_use]
    pub fn glyph_count(&self) -> u32 {
        self.glyph_count
    }

    /// `reversedChars`.
    #[must_use]
    pub fn reversed_chars(&self) -> bool {
        self.reversed_chars
    }

    /// `next`: the next cluster, or one with no glyphs when done.
    // Port of: src/pdf/SkClusterator.cpp#L45-L65 (chrome/m156)
    #[allow(clippy::should_implement_trait)] // mirrors SkClusterator::next, which returns a Cluster, not an Option
    pub fn next(&mut self) -> Cluster<'a> {
        if self.current_glyph_index >= self.glyph_count {
            return Cluster {
                utf8_text: None,
                text_byte_length: 0,
                glyph_index: 0,
                glyph_count: 0,
            };
        }
        let (Some(clusters), Some(utf8_text)) = (self.clusters, self.utf8_text) else {
            let glyph_index = self.current_glyph_index;
            self.current_glyph_index += 1;
            return Cluster {
                utf8_text: None,
                text_byte_length: 0,
                glyph_index,
                glyph_count: 1,
            };
        };
        let cluster_glyph_index = self.current_glyph_index;
        let cluster = clusters[cluster_glyph_index as usize];
        loop {
            self.current_glyph_index += 1;
            if !(self.current_glyph_index < self.glyph_count
                && cluster == clusters[self.current_glyph_index as usize])
            {
                break;
            }
        }
        let cluster_glyph_count = self.current_glyph_index - cluster_glyph_index;
        let mut cluster_end = self.text_byte_length;
        for &c in clusters.iter().take(self.glyph_count as usize) {
            if c > cluster && c < cluster_end {
                cluster_end = c;
            }
        }
        let cluster_len = cluster_end - cluster;
        Cluster {
            utf8_text: Some(&utf8_text[cluster as usize..(cluster + cluster_len) as usize]),
            text_byte_length: cluster_len,
            glyph_index: cluster_glyph_index,
            glyph_count: cluster_glyph_count,
        }
    }
}
