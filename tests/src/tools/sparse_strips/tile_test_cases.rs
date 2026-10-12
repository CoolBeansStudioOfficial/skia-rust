// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/sparse_strips/TileTestCases.h, .cpp (chrome/m156)

//! The tile test cases of the sparse strips `Tiler` and `Tile::ClipToTile` tests.
// The `double` literals of the C++ are converted to `float` as `f64 as f32`.
#![allow(
    clippy::cast_possible_truncation,
    clippy::excessive_precision,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use skia_rust_core::point::Point;
use skia_rust_gpu::graphite::sparse_strips::tiler::Tile;
use skia_rust_gpu::graphite::sparse_strips::types::{IntersectionBits, Line};

/// `TileTestCases::TileTestCase`.
// Port of: tests/graphite/sparse_strips/TileTestCases.h#L21-L31 (chrome/m156)
#[derive(Clone, Debug)]
pub struct TileTestCase {
    /// `fName`.
    pub name: &'static str,
    /// `fLines`.
    pub lines: Vec<Line>,
    /// `fExpected`.
    pub expected: Vec<Tile>,
}

impl TileTestCase {
    /// `TileTestCase(name, unscaledLines, expectedTiles, scale)`: scales the lines.
    // Port of: tests/graphite/sparse_strips/TileTestCases.cpp#L16-L25 (chrome/m156)
    #[must_use]
    pub fn new(
        name: &'static str,
        unscaled_lines: &[Line],
        expected_tiles: &[Tile],
        scale: f32,
    ) -> Self {
        let lines = unscaled_lines
            .iter()
            .map(|line| Line {
                p0: Point::new(line.p0.x * scale, line.p0.y * scale),
                p1: Point::new(line.p1.x * scale, line.p1.y * scale),
            })
            .collect();
        Self {
            name,
            lines,
            expected: expected_tiles.to_vec(),
        }
    }
}

/// `TileTestCases::Get(scale, viewportDim)`.
// Port of: tests/graphite/sparse_strips/TileTestCases.cpp#L28-L486 (chrome/m156)
#[must_use]
pub fn get(scale: f32, viewport_dim: u16) -> Vec<TileTestCase> {
    const T: u32 = IntersectionBits::T;
    const B: u32 = IntersectionBits::B;
    const L: u32 = IntersectionBits::L;
    const R: u32 = IntersectionBits::R;
    const W: u32 = IntersectionBits::W;

    let unscaled_dim = f32::from(viewport_dim);

    // `{{x0, y0}, {x1, y1}}`
    let l = |x0: f32, y0: f32, x1: f32, y1: f32| Line {
        p0: Point::new(x0, y0),
        p1: Point::new(x1, y1),
    };

    vec![
        TileTestCase::new(
            "CulledLines",
            &[
                l(1.0, -7.0, 3.0, -1.0),
                l(1.0, -11.0, 3.0, -1.0),
                l(unscaled_dim + 1.0, 50.0, unscaled_dim + 3.0, 70.0),
                l(1.0, unscaled_dim + 1.0, 3.0, unscaled_dim + 7.0),
                l(1.0, unscaled_dim + 1.0, 3.0, unscaled_dim + 13.0),
            ],
            &[],
            scale,
        ),
        TileTestCase::new(
            "SlopedLineCrossingTop",
            &[
                l(-2.0, -3.0, 2.0, 1.0),
                l(6.0, -1.0, 5.0, 2.0),
                l(9.0, -10.0, 10.0, 3.0),
                l(2.0, 1.0, -2.0, -3.0),
            ],
            &[
                Tile::new(0, 0, 0, W | T),
                Tile::new(1, 0, 1, W | T),
                Tile::new(2, 0, 2, W | T),
                Tile::new(0, 0, 3, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "SlopedLineCrossingBot",
            &[
                l(5.0, unscaled_dim + 3.0, 6.0, unscaled_dim - 2.0),
                l(10.0, unscaled_dim + 1.0, 9.0, unscaled_dim - 1.0),
                l(2.0, unscaled_dim - 2.0, 3.0, unscaled_dim + 3.0),
            ],
            &[
                Tile::new(1, 24, 0, B),
                Tile::new(2, 24, 1, B),
                Tile::new(0, 24, 2, B),
            ],
            scale,
        ),
        TileTestCase::new(
            "SlopedLineCrossingTopMultiTile",
            &[l(1.0, -5.0, 6.0, 7.0), l(2.5, -10.0, 3.5, 6.0)],
            &[
                Tile::new(0, 0, 0, W | T | R),
                Tile::new(1, 0, 0, L | B),
                Tile::new(1, 1, 0, W | T),
                Tile::new(0, 0, 1, W | T | B),
                Tile::new(0, 1, 1, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "SlopedLineCrossingBotMultiTile",
            &[
                l(12.0, unscaled_dim + 10.0, 2.0, 94.0),
                l(1.5, unscaled_dim + 5.0, 3.5, 94.0),
            ],
            &[
                Tile::new(0, 23, 0, B),
                Tile::new(0, 24, 0, W | T | R),
                Tile::new(1, 24, 0, B | L),
                Tile::new(0, 23, 1, B),
                Tile::new(0, 24, 1, W | T | B),
            ],
            scale,
        ),
        TileTestCase::new(
            "SlopedLineCrossingRight",
            &[
                l(97.0, 1.0, unscaled_dim + 1.0, 2.0),
                l(93.0, 1.0, unscaled_dim + 5.0, 2.0),
            ],
            &[
                Tile::new(24, 0, 0, R),
                Tile::new(23, 0, 1, R),
                Tile::new(24, 0, 1, R | L),
            ],
            scale,
        ),
        TileTestCase::new(
            "SlopedLineCrossingLeft",
            &[
                l(-5.0, 1.0, 1.0, 2.0),
                l(-5.0, 1.0, 5.0, 2.0),
                l(-5.0, 1.0, 13.0, 9.0),
            ],
            &[
                Tile::new(0, 0, 0, L),
                Tile::new(0, 0, 1, L | R),
                Tile::new(1, 0, 1, L),
                Tile::new(0, 0, 2, L | B),
                Tile::new(0, 1, 2, W | R | T),
                Tile::new(1, 1, 2, R | L),
                Tile::new(2, 1, 2, L | B),
                Tile::new(2, 2, 2, W | R | T),
                Tile::new(3, 2, 2, L),
            ],
            scale,
        ),
        TileTestCase::new(
            "HorizontalLineAboveViewport",
            &[l(10.0, -5.0, 90.0, -5.0)],
            &[],
            scale,
        ),
        TileTestCase::new(
            "HorizontalLineBelowViewport",
            &[l(10.0, unscaled_dim + 5.0, 90.0, unscaled_dim + 5.0)],
            &[],
            scale,
        ),
        TileTestCase::new(
            "HorizontalLineCrossingLeftViewport",
            &[l(-10.0, 10.0, 10.0, 10.0)],
            &[
                Tile::new(0, 2, 0, L | R),
                Tile::new(1, 2, 0, L | R),
                Tile::new(2, 2, 0, L),
            ],
            scale,
        ),
        TileTestCase::new(
            "HorizontalLineCrossingRightViewport",
            &[l(unscaled_dim - 5.0, 10.0, unscaled_dim + 5.0, 10.0)],
            &[Tile::new(23, 2, 0, R), Tile::new(24, 2, 0, L | R)],
            scale,
        ),
        TileTestCase::new(
            "VerticalLinesOutsideViewport",
            &[
                l(1.0, -5.0, 1.0, -1.0),
                l(1.0, unscaled_dim + 1.0, 1.0, unscaled_dim + 5.0),
            ],
            &[],
            scale,
        ),
        TileTestCase::new(
            "VerticalLineCrossingTopViewport",
            &[
                l(1.0, -7.0, 1.0, 3.0),
                l(1.0, -7.0, 1.0, 7.0),
                l(1.0, -7.0, 1.0, 8.0),
            ],
            &[
                Tile::new(0, 0, 0, W | T),
                Tile::new(0, 0, 1, W | B | T),
                Tile::new(0, 1, 1, W | T),
                Tile::new(0, 0, 2, W | B | T),
                Tile::new(0, 1, 2, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "VerticalLineCrossingBotViewport",
            &[
                l(1.0, unscaled_dim - 1.0, 1.0, unscaled_dim + 5.0),
                l(1.0, unscaled_dim - 5.0, 1.0, unscaled_dim + 5.0),
            ],
            &[
                Tile::new(0, 24, 0, B),
                Tile::new(0, 23, 1, B),
                Tile::new(0, 24, 1, W | T | B),
            ],
            scale,
        ),
        TileTestCase::new(
            "ClipTopLeftCorner",
            &[l(-1.0, 2.0, 2.0, -1.0)],
            &[Tile::new(0, 0, 0, W | L | T)],
            scale,
        ),
        TileTestCase::new(
            "ClipBottomRightCorner",
            &[l(
                unscaled_dim + 1.0,
                unscaled_dim - 2.0,
                unscaled_dim - 2.0,
                unscaled_dim + 1.0,
            )],
            &[Tile::new(24, 24, 0, R | B)],
            scale,
        ),
        TileTestCase::new(
            "HorizontalLineLeftToRightThreeTile",
            &[l(1.5, 1.0, 8.5, 1.0)],
            &[
                Tile::new(0, 0, 0, R),
                Tile::new(1, 0, 0, R | L),
                Tile::new(2, 0, 0, L),
            ],
            scale,
        ),
        TileTestCase::new(
            "HorizontalLineRightToLeftThreeTile",
            &[l(8.5, 1.0, 1.5, 1.0)],
            &[
                Tile::new(0, 0, 0, R),
                Tile::new(1, 0, 0, R | L),
                Tile::new(2, 0, 0, L),
            ],
            scale,
        ),
        TileTestCase::new(
            "HorizontalLineMultiTile",
            &[l(1.5, 1.0, 12.5, 1.0)],
            &[
                Tile::new(0, 0, 0, R),
                Tile::new(1, 0, 0, R | L),
                Tile::new(2, 0, 0, R | L),
                Tile::new(3, 0, 0, L),
            ],
            scale,
        ),
        TileTestCase::new(
            "VerticalLineDownThreeTile",
            &[l(1.0, 1.5, 1.0, 8.5)],
            &[
                Tile::new(0, 0, 0, B),
                Tile::new(0, 1, 0, W | T | B),
                Tile::new(0, 2, 0, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "VerticalLineDownMultiTile",
            &[l(1.0, 1.0, 1.0, 13.0)],
            &[
                Tile::new(0, 0, 0, B),
                Tile::new(0, 1, 0, W | T | B),
                Tile::new(0, 2, 0, W | T | B),
                Tile::new(0, 3, 0, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "VerticalLineUpThreeTile",
            &[l(1.0, 13.0, 1.0, 1.0)],
            &[
                Tile::new(0, 0, 0, B),
                Tile::new(0, 1, 0, W | T | B),
                Tile::new(0, 2, 0, W | T | B),
                Tile::new(0, 3, 0, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "VerticalLineUpMultiTile",
            &[l(1.0, 8.5, 1.0, 1.5)],
            &[
                Tile::new(0, 0, 0, B),
                Tile::new(0, 1, 0, W | T | B),
                Tile::new(0, 2, 0, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "VerticalLineTouchingBot",
            &[l(1.0, 1.0, 1.0, 8.0)],
            &[Tile::new(0, 0, 0, B), Tile::new(0, 1, 0, W | T)],
            scale,
        ),
        TileTestCase::new(
            "VerticalLineTouchingTop",
            &[l(1.0, 0.0, 1.0, 7.0)],
            &[Tile::new(0, 0, 0, W | B), Tile::new(0, 1, 0, W | T)],
            scale,
        ),
        TileTestCase::new(
            "VerticalLineTouchingLeft",
            &[l(4.0, 0.0, 4.0, 7.0)],
            &[Tile::new(1, 0, 0, W | B), Tile::new(1, 1, 0, W | T)],
            scale,
        ),
        TileTestCase::new(
            "TopLeftToBottomRight",
            &[l(1.0, 1.0, 11.0, 9.0)],
            &[
                Tile::new(0, 0, 0, R),
                Tile::new(1, 0, 0, L | B),
                Tile::new(1, 1, 0, W | R | T),
                Tile::new(2, 1, 0, L | B),
                Tile::new(2, 2, 0, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "BottomRightToTopLeft",
            &[l(11.0, 9.0, 1.0, 1.0)],
            &[
                Tile::new(0, 0, 0, R),
                Tile::new(1, 0, 0, L | B),
                Tile::new(1, 1, 0, W | R | T),
                Tile::new(2, 1, 0, L | B),
                Tile::new(2, 2, 0, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "BottomLeftToTopRight",
            &[l(2.0, 11.0, 14.0, 6.0)],
            &[
                Tile::new(2, 1, 0, R | B),
                Tile::new(3, 1, 0, L),
                Tile::new(0, 2, 0, R),
                Tile::new(1, 2, 0, R | L),
                Tile::new(2, 2, 0, W | L | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "TopRightToBottomLeft",
            &[l(14.0, 6.0, 2.0, 11.0)],
            &[
                Tile::new(2, 1, 0, R | B),
                Tile::new(3, 1, 0, L),
                Tile::new(0, 2, 0, R),
                Tile::new(1, 2, 0, R | L),
                Tile::new(2, 2, 0, W | L | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "TwoLinesInSingleTile",
            &[l(1.0, 3.0, 3.0, 3.0), l(3.0, 3.0, 0.0, 1.0)],
            &[Tile::new(0, 0, 0, 0), Tile::new(0, 0, 1, 0)],
            scale,
        ),
        TileTestCase::new(
            "DiagonalCrossCorner",
            &[l(3.0, 5.0, 5.0, 3.0)],
            &[
                Tile::new(1, 0, 0, L),
                Tile::new(0, 1, 0, R),
                Tile::new(1, 1, 0, W | L | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "DiagonalCrossCornerTwo",
            &[l(7.9, 7.9, 0.1, 0.1)],
            &[
                Tile::new(0, 0, 0, R),
                Tile::new(1, 0, 0, L),
                Tile::new(1, 1, 0, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "DiagonalDownSlopeTiles",
            &[l(5.0, 5.0, 9.0, 9.0)],
            &[
                Tile::new(1, 1, 0, R),
                Tile::new(2, 1, 0, L),
                Tile::new(2, 2, 0, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "DiagonalUpSlopeTiles",
            &[l(5.0, 9.0, 9.0, 5.0)],
            &[
                Tile::new(1, 1, 0, R | B),
                Tile::new(2, 1, 0, L),
                Tile::new(1, 2, 0, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "DiagonalDownOneTile",
            &[l(0.0, 0.0, 4.0, 4.0)],
            &[Tile::new(0, 0, 0, W | R), Tile::new(1, 0, 0, L)],
            scale,
        ),
        TileTestCase::new(
            "DiagonalUpOneTile",
            &[l(0.0, 4.0, 4.0, 0.0)],
            &[Tile::new(0, 0, 0, R), Tile::new(1, 0, 0, W | L)],
            scale,
        ),
        TileTestCase::new(
            "DiagonalDownTwoTile",
            &[l(0.0, 0.0, 8.0, 8.0)],
            &[
                Tile::new(0, 0, 0, W | R),
                Tile::new(1, 0, 0, L),
                Tile::new(1, 1, 0, W | R | T),
                Tile::new(2, 1, 0, L),
            ],
            scale,
        ),
        TileTestCase::new(
            "DiagonalUpTwoTile",
            &[l(0.0, 8.0, 8.0, 0.0)],
            &[
                Tile::new(1, 0, 0, R | L),
                Tile::new(2, 0, 0, W | L),
                Tile::new(0, 1, 0, R),
                Tile::new(1, 1, 0, W | L | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "SlopedEndingRight",
            &[l(1.0, 1.0, 8.0, 2.0)],
            &[
                Tile::new(0, 0, 0, R),
                Tile::new(1, 0, 0, R | L),
                Tile::new(2, 0, 0, L),
            ],
            scale,
        ),
        TileTestCase::new(
            "SlopedTouchingTop",
            &[l(0.0, 8.0, 4.0, 0.0)],
            &[
                Tile::new(0, 0, 0, R | B),
                Tile::new(1, 0, 0, W | L),
                Tile::new(0, 1, 0, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "SlopedTouchingBot",
            &[l(0.0, 0.0, 4.0, 8.0)],
            &[
                Tile::new(0, 0, 0, W | B),
                Tile::new(0, 1, 0, W | R | T),
                Tile::new(1, 1, 0, L),
            ],
            scale,
        ),
        TileTestCase::new(
            "SlopedDownThree",
            &[l(1.0, 1.0, 5.0, 11.0)],
            &[
                Tile::new(0, 0, 0, B),
                Tile::new(0, 1, 0, W | T | B),
                Tile::new(0, 2, 0, W | T | R),
                Tile::new(1, 2, 0, L),
            ],
            scale,
        ),
        TileTestCase::new(
            "SameTile",
            &[l(1.0, 1.0, 3.0, 3.0)],
            &[Tile::new(0, 0, 0, 0)],
            scale,
        ),
        TileTestCase::new(
            "SameTileLeft",
            &[l(0.0, 1.0, 3.0, 1.0)],
            &[Tile::new(0, 0, 0, 0)],
            scale,
        ),
        TileTestCase::new(
            "SameTileTop",
            &[l(1.0, 0.0, 1.0, 3.0)],
            &[Tile::new(0, 0, 0, W)],
            scale,
        ),
        TileTestCase::new(
            "SameTileRight",
            &[l(1.0, 1.0, 4.0, 1.0)],
            &[Tile::new(0, 0, 0, R), Tile::new(1, 0, 0, L)],
            scale,
        ),
        TileTestCase::new(
            "SameTileBottom",
            &[l(1.0, 1.0, 1.0, 4.0), l(1.0, 1.0, 2.0, 4.0)],
            &[Tile::new(0, 0, 0, 0), Tile::new(0, 0, 1, 0)],
            scale,
        ),
        TileTestCase::new(
            "SameTileTopLeft",
            &[l(0.0, 1.0, 1.0, 0.0), l(0.0, 0.0001, 0.0001, 0.0)],
            &[Tile::new(0, 0, 0, W), Tile::new(0, 0, 1, W)],
            scale,
        ),
        TileTestCase::new(
            "ExactRightEdgeCoincidence",
            &[l(4.0, 1.0, 4.0, 3.0)],
            &[Tile::new(1, 0, 0, 0)],
            scale,
        ),
        TileTestCase::new(
            "ExactBottomEdgeCoincidence",
            &[l(1.0, 4.0, 3.0, 4.0)],
            &[],
            scale,
        ),
        TileTestCase::new(
            "Tricky Precision",
            &[l(
                0.000000_f64 as f32,
                15.856406_f64 as f32,
                8.000000_f64 as f32,
                2.000000_f64 as f32,
            )],
            &[
                Tile::new(1, 0, 0, B | R),
                Tile::new(2, 0, 0, L),
                Tile::new(1, 1, 0, W | T | B),
                Tile::new(0, 2, 0, B | R),
                Tile::new(1, 2, 0, W | T | L),
                Tile::new(0, 3, 0, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "BackwardsDiagonal",
            &[l(8.0, 8.0, 0.0, 0.0)],
            &[
                Tile::new(0, 0, 0, W | R),
                Tile::new(1, 0, 0, L),
                Tile::new(1, 1, 0, W | T | R),
                Tile::new(2, 1, 0, L),
            ],
            scale,
        ),
        TileTestCase::new("PointLineInside", &[l(2.0, 2.0, 2.0, 2.0)], &[], scale),
        TileTestCase::new("PointLineOnCorner", &[l(4.0, 4.0, 4.0, 4.0)], &[], scale),
        TileTestCase::new(
            "NearVerticalGrazing",
            &[l(3.999, 0.0, 3.998, 8.0)],
            &[Tile::new(0, 0, 0, W | B), Tile::new(0, 1, 0, W | T)],
            scale,
        ),
        TileTestCase::new(
            "HighSlopeNeedle",
            &[l(2.0, -100.0, 2.01, 11.0)],
            &[
                Tile::new(0, 0, 0, W | T | B),
                Tile::new(0, 1, 0, W | T | B),
                Tile::new(0, 2, 0, W | T),
            ],
            scale,
        ),
        TileTestCase::new(
            "NearParallelHorizontal",
            &[l(-5.0, 2.0, 15.0, 2.0001)],
            &[
                Tile::new(0, 0, 0, L | R),
                Tile::new(1, 0, 0, L | R),
                Tile::new(2, 0, 0, L | R),
                Tile::new(3, 0, 0, L),
            ],
            scale,
        ),
    ]
}
