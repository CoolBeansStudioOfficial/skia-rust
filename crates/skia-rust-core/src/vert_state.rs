// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkVertState.h, src/core/SkVertState.cpp

//! `VertState`: a helper for `drawVertices` that iterates over the triangles of a set of
//! vertices, given a [`VertexMode`] and (optionally) an index array.

use crate::vertices::VertexMode;

/// A function that advances a [`VertState`] to the next triangle (`VertState::Proc`); false when
/// there are no more.
// Port of: src/core/SkVertState.h#L39 (chrome/m156)
pub type Proc = fn(&mut VertState<'_>) -> bool;

/// Iterates over the triangles to be rendered for a [`VertexMode`] and an optional index array.
/// It does not copy the index array.
///
/// skia-rust: the vertex indices `f0`, `f1` and `f2` are `usize` (they only index arrays);
/// Skia's are `int`.
// Port of: src/core/SkVertState.h#L22-L60 (chrome/m156)
#[derive(Debug)]
pub struct VertState<'a> {
    /// The first vertex of the current triangle (`f0`).
    pub f0: usize,
    /// The second vertex of the current triangle (`f1`).
    pub f1: usize,
    /// The third vertex of the current triangle (`f2`).
    pub f2: usize,
    count: usize,
    curr_index: usize,
    indices: Option<&'a [u16]>,
}

impl<'a> VertState<'a> {
    /// Constructs a `VertState` from a vertex count, index array and index count. If the
    /// vertices are unindexed pass `None` for `indices`.
    // Port of: src/core/SkVertState.h#L28-L37 (chrome/m156)
    #[must_use]
    pub fn new(v_count: usize, indices: Option<&'a [u16]>, index_count: usize) -> VertState<'a> {
        VertState {
            f0: 0,
            f1: 0,
            f2: 0,
            count: if indices.is_some() {
                index_count
            } else {
                v_count
            },
            curr_index: 0,
            indices,
        }
    }

    /// Chooses an appropriate function to traverse the vertices (`chooseProc`).
    // Port of: src/core/SkVertState.cpp#L95-L106 (chrome/m156)
    #[doc(alias = "chooseProc")]
    #[must_use]
    pub fn choose_proc(&self, mode: VertexMode) -> Proc {
        match mode {
            VertexMode::Triangles => {
                if self.indices.is_some() {
                    Self::triangles_x
                } else {
                    Self::triangles
                }
            }
            VertexMode::TriangleStrip => {
                if self.indices.is_some() {
                    Self::triangle_strip_x
                } else {
                    Self::triangle_strip
                }
            }
            VertexMode::TriangleFan => {
                if self.indices.is_some() {
                    Self::triangle_fan_x
                } else {
                    Self::triangle_fan
                }
            }
        }
    }

    /// `currIndex`.
    // Port of: src/core/SkVertState.h#L44 (chrome/m156)
    #[doc(alias = "currIndex")]
    #[must_use]
    pub fn curr_index(&self) -> usize {
        self.curr_index
    }

    /// The indices of `triangles_x` and friends (`fIndices` of an indexed state).
    fn idx(&self, i: usize) -> usize {
        usize::from(self.indices.expect("an indexed VertState proc")[i])
    }

    // Port of: src/core/SkVertState.cpp#L10-L20 (chrome/m156)
    fn triangles(state: &mut VertState<'_>) -> bool {
        let index = state.curr_index;
        if index + 3 > state.count {
            return false;
        }
        state.f0 = index;
        state.f1 = index + 1;
        state.f2 = index + 2;
        state.curr_index = index + 3;
        true
    }

    // Port of: src/core/SkVertState.cpp#L22-L33 (chrome/m156)
    fn triangles_x(state: &mut VertState<'_>) -> bool {
        let index = state.curr_index;
        if index + 3 > state.count {
            return false;
        }
        state.f0 = state.idx(index);
        state.f1 = state.idx(index + 1);
        state.f2 = state.idx(index + 2);
        state.curr_index = index + 3;
        true
    }

    // Port of: src/core/SkVertState.cpp#L35-L50 (chrome/m156)
    fn triangle_strip(state: &mut VertState<'_>) -> bool {
        let index = state.curr_index;
        if index + 3 > state.count {
            return false;
        }
        state.f2 = index + 2;
        if index & 1 != 0 {
            state.f0 = index + 1;
            state.f1 = index;
        } else {
            state.f0 = index;
            state.f1 = index + 1;
        }
        state.curr_index = index + 1;
        true
    }

    // Port of: src/core/SkVertState.cpp#L52-L68 (chrome/m156)
    fn triangle_strip_x(state: &mut VertState<'_>) -> bool {
        let index = state.curr_index;
        if index + 3 > state.count {
            return false;
        }
        state.f2 = state.idx(index + 2);
        if index & 1 != 0 {
            state.f0 = state.idx(index + 1);
            state.f1 = state.idx(index);
        } else {
            state.f0 = state.idx(index);
            state.f1 = state.idx(index + 1);
        }
        state.curr_index = index + 1;
        true
    }

    // Port of: src/core/SkVertState.cpp#L70-L80 (chrome/m156)
    fn triangle_fan(state: &mut VertState<'_>) -> bool {
        let index = state.curr_index;
        if index + 3 > state.count {
            return false;
        }
        state.f0 = 0;
        state.f1 = index + 1;
        state.f2 = index + 2;
        state.curr_index = index + 1;
        true
    }

    // Port of: src/core/SkVertState.cpp#L82-L93 (chrome/m156)
    fn triangle_fan_x(state: &mut VertState<'_>) -> bool {
        let index = state.curr_index;
        if index + 3 > state.count {
            return false;
        }
        state.f0 = state.idx(0);
        state.f1 = state.idx(index + 1);
        state.f2 = state.idx(index + 2);
        state.curr_index = index + 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangles(mut state: VertState<'_>, mode: VertexMode) -> Vec<[usize; 3]> {
        let proc = state.choose_proc(mode);
        let mut out = Vec::new();
        while proc(&mut state) {
            out.push([state.f0, state.f1, state.f2]);
        }
        out
    }

    #[test]
    fn unindexed_modes() {
        let state = || VertState::new(5, None, 0);
        assert_eq!(
            triangles(state(), VertexMode::Triangles),
            vec![[0, 1, 2]] // 5 vertices make one triangle; the last two are dropped
        );
        assert_eq!(
            triangles(state(), VertexMode::TriangleStrip),
            vec![[0, 1, 2], [2, 1, 3], [2, 3, 4]]
        );
        assert_eq!(
            triangles(state(), VertexMode::TriangleFan),
            vec![[0, 1, 2], [0, 2, 3], [0, 3, 4]]
        );
    }

    #[test]
    fn indexed_modes() {
        let indices = [4u16, 3, 2, 1, 0];
        let state = || VertState::new(5, Some(&indices), 5);
        assert_eq!(triangles(state(), VertexMode::Triangles), vec![[4, 3, 2]]);
        assert_eq!(
            triangles(state(), VertexMode::TriangleStrip),
            vec![[4, 3, 2], [2, 3, 1], [2, 1, 0]]
        );
        assert_eq!(
            triangles(state(), VertexMode::TriangleFan),
            vec![[4, 3, 2], [4, 2, 1], [4, 1, 0]]
        );
        // The index count, not the slice length, bounds the iteration.
        let state = VertState::new(5, Some(&indices), 3);
        assert_eq!(triangles(state, VertexMode::Triangles), vec![[4, 3, 2]]);
    }
}
