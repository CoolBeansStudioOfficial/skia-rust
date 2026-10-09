// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The ported benchmarks: one module per `bench/<File>.cpp` (`PathBench` → `path_bench`).

pub mod bezier_bench;
pub mod canvas_save_restore_bench;
pub mod clear_bench;
pub mod color_priv_bench;
pub mod control_bench;
pub mod cubic_map_bench;
pub mod dash_bench;
pub mod find_cubic_convex180_chops_bench;
pub mod fs_rect_bench;
pub mod geometry_bench;
pub mod interp_bench;
pub mod line_bench;
pub mod math_bench;
pub mod matrix44_bench;
pub mod matrix_bench;
pub mod memset_bench;
pub mod path_iter_bench;
pub mod quick_reject_bench;
pub mod r_tree_bench;
pub mod region_bench;
pub mod region_contain_bench;
pub mod stream_bench;
pub mod table_bench;
pub mod writer_bench;
