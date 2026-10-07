// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/AsADashTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, reporter_assert};
use skia_rust_core::path_effect::PathEffect;
use skia_rust_core::scalar::{scalar, scalar_mod};
use skia_rust_effects::corner_path_effect::CornerPathEffectExt;
use skia_rust_effects::dash_path_effect::DashPathEffectExt;

// Port of: tests/AsADashTest.cpp#L19-L25 (chrome/m156)
def_test!(AsADashTest_noneDash, |reporter| {
    let pe = PathEffect::corner_path(1.0).unwrap();

    let dash_info = pe.as_a_dash();
    reporter_assert!(reporter, dash_info.is_none());
});

// Port of: tests/AsADashTest.cpp#L27-L34 (chrome/m156)
def_test!(AsADashTest_nullInfo, |reporter| {
    let in_intervals: [scalar; 4] = [4.0, 2.0, 1.0, 3.0];
    let phase: scalar = 2.0;
    let pe = PathEffect::dash(&in_intervals, phase).unwrap();

    let dash_info = pe.as_a_dash();
    reporter_assert!(reporter, dash_info.is_some());
});

// Port of: tests/AsADashTest.cpp#L36-L52 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // exact float comparisons, as in C++
    AsADashTest_usingDash,
    |reporter| {
        let in_intervals: [scalar; 4] = [4.0, 2.0, 1.0, 3.0];
        let total_int_sum: scalar = 10.0;
        let phase: scalar = 2.0;

        let pe = PathEffect::dash(&in_intervals, phase).unwrap();

        let info = pe.as_a_dash();
        reporter_assert!(reporter, info.is_some());
        let info = info.unwrap();
        reporter_assert!(reporter, 4 == info.intervals.len());
        reporter_assert!(reporter, scalar_mod(phase, total_int_sum) == info.phase);

        reporter_assert!(reporter, in_intervals[0] == info.intervals[0]);
        reporter_assert!(reporter, in_intervals[1] == info.intervals[1]);
        reporter_assert!(reporter, in_intervals[2] == info.intervals[2]);
        reporter_assert!(reporter, in_intervals[3] == info.intervals[3]);
    }
);
