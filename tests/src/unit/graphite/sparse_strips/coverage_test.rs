// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/sparse_strips/CoverageTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::items_after_statements,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_gpu::graphite::geom::end_caps::EndCaps;
use skia_rust_gpu::graphite::geom::wide_tiles::WideTiles;
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::sparse_strips::alpha_atlas_manager::AlphaAtlasManager;
use skia_rust_gpu::graphite::sparse_strips::flatten::Flatten;
use skia_rust_gpu::graphite::sparse_strips::make_strips::MakeStrips;
use skia_rust_gpu::graphite::sparse_strips::msaa_lut::{MsaaLut, generate_msaa_lut};
use skia_rust_gpu::graphite::sparse_strips::polyline::Polyline;
use skia_rust_gpu::graphite::sparse_strips::tiler::Tiles;
use skia_rust_gpu::graphite::sparse_strips::types::{FlattenMode, MsaaExactMaskObserver};
use skia_rust_simd::vx::Int8;

use crate::tools::sparse_strips::coverage_test_utils::{
    point_in_polygon, print_coverage_diagnostics,
};
use crate::tools::sparse_strips::skp_validator::SkpValidator;
use crate::{Reporter, def_graphite_test_for_all_contexts, infof, reporter_assert};

/// `CoverageTestRunner<kTileWidth, kTileHeight>::StripFunc`.
// Port of: tests/graphite/sparse_strips/CoverageTest.cpp#L58-L65 (chrome/m156)
type StripFunc<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16> = for<'a> fn(
    &Tiles<K_TILE_WIDTH, K_TILE_HEIGHT>,
    &mut WideTiles,
    &mut EndCaps,
    &mut AlphaAtlasManager,
    &Recorder,
    bool,
    &'a Polyline,
    &'a [u8],
    Option<MsaaExactMaskObserver<'a>>,
);

/// `CoverageTestRunner<kTileWidth, kTileHeight>`.
// Port of: tests/graphite/sparse_strips/CoverageTest.cpp#L48-L277 (chrome/m156)
struct CoverageTestRunner<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16> {
    func: StripFunc<K_TILE_WIDTH, K_TILE_HEIGHT>,
    impl_name: &'static str,
}

impl<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16>
    CoverageTestRunner<K_TILE_WIDTH, K_TILE_HEIGHT>
{
    const K_TILE_WIDTH_F: f32 = K_TILE_WIDTH as f32;
    const K_TILE_HEIGHT_F: f32 = K_TILE_HEIGHT as f32;
    const K_VIEWPORT_WIDTH: u32 = 400;
    const K_VIEWPORT_HEIGHT: u32 = 400;
    const K_VIEWPORT_WIDTH_F: f32 = Self::K_VIEWPORT_WIDTH as f32;
    const K_VIEWPORT_HEIGHT_F: f32 = Self::K_VIEWPORT_HEIGHT as f32;

    // Port of: tests/graphite/sparse_strips/CoverageTest.cpp#L62-L75 (chrome/m156)
    fn run_scalar_winding<'a>(
        tile_container: &Tiles<K_TILE_WIDTH, K_TILE_HEIGHT>,
        wides: &mut WideTiles,
        ends: &mut EndCaps,
        atlas_manager: &mut AlphaAtlasManager,
        recorder: &Recorder,
        is_inverse: bool,
        polyline: &'a Polyline,
        mask_lut: &'a [u8],
        observer: Option<MsaaExactMaskObserver<'a>>,
    ) {
        let fill_type = if is_inverse {
            PathFillType::InverseWinding
        } else {
            PathFillType::Winding
        };
        MakeStrips::msaa_scalar::<K_TILE_WIDTH, K_TILE_HEIGHT>(
            tile_container,
            wides,
            ends,
            atlas_manager,
            recorder,
            fill_type,
            polyline,
            mask_lut,
            Self::K_VIEWPORT_WIDTH as u16,
            Self::K_VIEWPORT_HEIGHT as u16,
            observer,
        );
    }

    // Port of: tests/graphite/sparse_strips/CoverageTest.cpp#L77-L90 (chrome/m156)
    fn run_simd_winding<'a>(
        tile_container: &Tiles<K_TILE_WIDTH, K_TILE_HEIGHT>,
        wides: &mut WideTiles,
        ends: &mut EndCaps,
        atlas_manager: &mut AlphaAtlasManager,
        recorder: &Recorder,
        is_inverse: bool,
        polyline: &'a Polyline,
        mask_lut: &'a [u8],
        observer: Option<MsaaExactMaskObserver<'a>>,
    ) {
        let fill_type = if is_inverse {
            PathFillType::InverseWinding
        } else {
            PathFillType::Winding
        };
        MakeStrips::msaa_simd::<K_TILE_WIDTH, K_TILE_HEIGHT>(
            tile_container,
            wides,
            ends,
            atlas_manager,
            recorder,
            fill_type,
            polyline,
            mask_lut,
            Self::K_VIEWPORT_WIDTH as u16,
            Self::K_VIEWPORT_HEIGHT as u16,
            observer,
        );
    }

    // Port of: tests/graphite/sparse_strips/CoverageTest.cpp#L99 (chrome/m156)
    fn new(func: StripFunc<K_TILE_WIDTH, K_TILE_HEIGHT>, impl_name: &'static str) -> Self {
        Self { func, impl_name }
    }

    // Port of: tests/graphite/sparse_strips/CoverageTest.cpp#L94-L189 (chrome/m156)
    fn run_all(&self, reporter: &mut Reporter, recorder: &Recorder) {
        let lut = generate_msaa_lut::<u8>();
        const K_ERROR_LIMIT: usize = 3;
        let mut minor_error_count = [0u32; K_ERROR_LIMIT];
        let mut total_tests_run = 0;

        struct TestCase {
            path: Path,
            name: &'static str,
        }
        let mut base_geometries: Vec<TestCase> = Vec::new();

        let mut add_rect = |w: f32, h: f32, name: &'static str| {
            base_geometries.push(TestCase {
                path: PathBuilder::new()
                    .add_rect(Rect::from_wh(w, h), None, None)
                    .detach(),
                name,
            });
        };

        add_rect(
            Self::K_TILE_WIDTH_F * 0.5,
            Self::K_TILE_HEIGHT_F * 0.5,
            "Rect(Small)",
        );
        add_rect(
            Self::K_TILE_WIDTH_F,
            Self::K_TILE_HEIGHT_F,
            "Rect(ExactTile)",
        );
        add_rect(
            Self::K_TILE_WIDTH_F * 2.5,
            Self::K_TILE_HEIGHT_F * 1.5,
            "Rect(MultiTile)",
        );
        add_rect(Self::K_TILE_WIDTH_F * 4.0, 0.2, "Rect(HorizSliver)");
        add_rect(0.2, Self::K_TILE_HEIGHT_F * 4.0, "Rect(VertSliver)");

        base_geometries.push(TestCase {
            path: PathBuilder::new()
                .add_circle(
                    (Self::K_TILE_WIDTH_F * 1.5, Self::K_TILE_HEIGHT_F * 1.5),
                    Self::K_TILE_WIDTH_F * 1.2,
                    None,
                )
                .detach(),
            name: "Circle",
        });

        base_geometries.push(TestCase {
            path: PathBuilder::new()
                .add_oval(
                    Rect::from_wh(f32::from(K_TILE_WIDTH) * 4.0, 0.5),
                    None,
                    None,
                )
                .detach(),
            name: "ThinOval",
        });

        let mut inset = PathBuilder::new();
        inset.add_rect(
            Rect::from_wh(Self::K_TILE_WIDTH_F * 3.0, Self::K_TILE_HEIGHT_F * 3.0),
            PathDirection::CW,
            None,
        );
        inset.add_rect(
            Rect::from_xywh(
                Self::K_TILE_WIDTH_F,
                Self::K_TILE_HEIGHT_F,
                Self::K_TILE_WIDTH_F,
                Self::K_TILE_HEIGHT_F,
            ),
            PathDirection::CCW,
            None,
        );
        base_geometries.push(TestCase {
            path: inset.detach(),
            name: "InsetRect",
        });

        // Tile-relative alignments (dx, dy)
        let alignments = [
            Point::new(0.0, 0.0),                         // Top & Left aligned
            Point::new(0.0, Self::K_TILE_HEIGHT_F * 0.5), // Left aligned, offset top
            Point::new(Self::K_TILE_WIDTH_F * 0.5, 0.0),  // Top aligned, offset left
            Point::new(Self::K_TILE_WIDTH_F * 0.33, Self::K_TILE_HEIGHT_F * 0.33), // Strictly inside
            Point::new(Self::K_TILE_WIDTH_F - 0.01, Self::K_TILE_HEIGHT_F - 0.01), // Right on a tile boundary edge
        ];

        for geom in &base_geometries {
            // Progressively rotate the geometry
            for angle_deg in 0..360 {
                let angle = angle_deg as f32;

                for alignment in &alignments {
                    let mut rot_matrix = Matrix::new_identity();
                    rot_matrix.set_rotate(angle, None);
                    let rotated_bounds = *geom.path.make_transform(&rot_matrix).bounds();
                    let mut trans_matrix = Matrix::new_identity();
                    trans_matrix.set_translate((
                        alignment.x - rotated_bounds.left,
                        alignment.y - rotated_bounds.top,
                    ));
                    let ctm = Matrix::concat(&trans_matrix, &rot_matrix);
                    let device_space_path = geom.path.make_transform(&ctm);

                    let test_name = format!(
                        "{} - {} Rot({:.1}) Align({:.2},{:.2})",
                        self.impl_name, geom.name, angle, alignment.x, alignment.y
                    );

                    if !self.run_single_test(
                        reporter,
                        recorder,
                        &device_space_path,
                        &test_name,
                        &lut,
                        &mut minor_error_count,
                    ) {
                        return;
                    }
                    total_tests_run += 1;
                }
            }
        }

        infof!(
            reporter,
            "[{} ({}x{})] Coverage LUT Test Complete. Ran {} variants. Minor Error Summary: \
             1-sample: {}, 2-sample: {}, 3-sample: {}\n",
            self.impl_name,
            K_TILE_WIDTH,
            K_TILE_HEIGHT,
            total_tests_run,
            minor_error_count[0],
            minor_error_count[1],
            minor_error_count[2]
        );
    }

    // Port of: tests/graphite/sparse_strips/CoverageTest.cpp#L195-L294 (chrome/m156)
    fn run_single_test(
        &self,
        reporter: &mut Reporter,
        recorder: &Recorder,
        path: &Path,
        name: &str,
        lut: &[u8],
        minor_error_count: &mut [u32; 3],
    ) -> bool {
        let mut flattener = Flatten::new();
        let mut polyline = Polyline::new();
        flattener.process_paths(
            FlattenMode::Simd,
            path,
            &Matrix::new_identity(),
            Self::K_VIEWPORT_WIDTH_F,
            Self::K_VIEWPORT_HEIGHT_F,
            &mut polyline,
        );

        let mut tiler = Tiles::<K_TILE_WIDTH, K_TILE_HEIGHT>::new();
        tiler.make_tiles_msaa(
            &polyline,
            Self::K_VIEWPORT_WIDTH as u16,
            Self::K_VIEWPORT_HEIGHT as u16,
        );
        tiler.sort_tiles();

        let mut wides = WideTiles::new();
        let mut ends = EndCaps::new();
        let mut exact_masks: Vec<u8> = Vec::new();
        let mut atlas_manager = AlphaAtlasManager::new();

        {
            let mut observer = |exact_mask: u8, _winding: Int8| exact_masks.push(exact_mask);
            (self.func)(
                &tiler,
                &mut wides,
                &mut ends,
                &mut atlas_manager,
                recorder,
                /* is_inverse = */ false,
                &polyline,
                lut,
                Some(&mut observer),
            );
        }

        if ends.is_empty() {
            let buffer_size_match = exact_masks.is_empty();
            reporter_assert!(
                reporter,
                buffer_size_match,
                "[{}] No endcaps but observer has data.",
                name
            );
            return buffer_size_match;
        }

        let mut mask_idx: usize = 0;

        for cap in ends.caps() {
            let spanned_tiles = cap.width / K_TILE_WIDTH;
            let mut curr_x = cap.x;
            let curr_y = cap.y;

            for _s in 0..spanned_tiles {
                let tile_start_idx = mask_idx;
                for y in 0..i32::from(K_TILE_HEIGHT) {
                    for x in 0..i32::from(K_TILE_WIDTH) {
                        let mut expected_mask: u8 = 0;
                        let mut expected_samples = 0;
                        for k in 0..8usize {
                            if point_in_polygon(
                                Point::new(
                                    (i32::from(curr_x) + x) as f32
                                        + (f32::from(MsaaLut::<u8>::K_PATTERN[k]) + 0.5) / 8.0,
                                    (i32::from(curr_y) + y) as f32 + (k as f32 + 0.5) / 8.0,
                                ),
                                &polyline,
                            ) {
                                expected_samples += 1;
                                expected_mask |= 1 << k;
                            }
                        }

                        let actual_mask = if mask_idx < exact_masks.len() {
                            exact_masks[mask_idx]
                        } else {
                            0
                        };

                        let mut sample_diff = 0;
                        let mut actual_samples = 0;
                        for k in 0..8 {
                            if actual_mask & (1 << k) != 0 {
                                actual_samples += 1;
                            }
                            if (expected_mask & (1 << k)) != (actual_mask & (1 << k)) {
                                sample_diff += 1;
                            }
                        }

                        if sample_diff > 3 {
                            print_coverage_diagnostics(
                                reporter,
                                &polyline,
                                &tiler,
                                i32::from(curr_x),
                                i32::from(curr_y),
                                &exact_masks,
                                tile_start_idx,
                            );
                            reporter_assert!(
                                reporter,
                                false,
                                "[{}] Fail at tile({},{}). Exp {}, Got {} samples",
                                name,
                                curr_x / K_TILE_WIDTH,
                                curr_y / K_TILE_HEIGHT,
                                expected_samples,
                                actual_samples
                            );
                            return false;
                        } else if sample_diff > 0 {
                            minor_error_count[sample_diff - 1] += 1;
                        }

                        mask_idx += 1;
                    }
                }
                curr_x += K_TILE_WIDTH;
            }
        }

        let buffer_size_match = mask_idx == exact_masks.len();
        reporter_assert!(
            reporter,
            buffer_size_match,
            "[{}] Checked {} mask bytes but observer size is {}",
            name,
            mask_idx,
            exact_masks.len()
        );
        buffer_size_match
    }
}

def_graphite_test_for_all_contexts!(SparseStrips_CoverageScalar_4x4, |reporter, context| {
    // Port of: tests/graphite/sparse_strips/CoverageTest.cpp#L297-L305 (chrome/m156)
    let recorder = context.make_recorder(None);
    let scalar_runner =
        CoverageTestRunner::<4, 4>::new(CoverageTestRunner::<4, 4>::run_scalar_winding, "Scalar");
    scalar_runner.run_all(reporter, &recorder);
});

def_graphite_test_for_all_contexts!(SparseStrips_CoverageSIMD_4x4, |reporter, context| {
    // Port of: tests/graphite/sparse_strips/CoverageTest.cpp#L307-L315 (chrome/m156)
    let recorder = context.make_recorder(None);
    let simd_runner =
        CoverageTestRunner::<4, 4>::new(CoverageTestRunner::<4, 4>::run_simd_winding, "SIMD");
    simd_runner.run_all(reporter, &recorder);
});

def_graphite_test_for_all_contexts!(SparseStrips_CoverageSIMD_8x8, |reporter, context| {
    // Port of: tests/graphite/sparse_strips/CoverageTest.cpp#L317-L325 (chrome/m156)
    let recorder = context.make_recorder(None);
    let simd_runner =
        CoverageTestRunner::<8, 8>::new(CoverageTestRunner::<8, 8>::run_simd_winding, "SIMD");
    simd_runner.run_all(reporter, &recorder);
});

def_graphite_test_for_all_contexts!(SparseStrips_Coverage_SKP_SIMD_4x4, |reporter, context| {
    // Port of: tests/graphite/sparse_strips/CoverageTest.cpp#L327-L334 (chrome/m156)
    let recorder = context.make_recorder(None);
    let lut = generate_msaa_lut::<u8>();
    SkpValidator::validate_skp::<4, 4>(reporter, &recorder, "skps/desk_tiger8svg.skp", &lut);
});

def_graphite_test_for_all_contexts!(SparseStrips_Coverage_SKP_SIMD_8x8, |reporter, context| {
    // Port of: tests/graphite/sparse_strips/CoverageTest.cpp#L336-L343 (chrome/m156)
    let recorder = context.make_recorder(None);
    let lut = generate_msaa_lut::<u8>();
    SkpValidator::validate_skp::<8, 8>(reporter, &recorder, "skps/desk_tiger8svg.skp", &lut);
});
