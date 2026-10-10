// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/Time.cpp (chrome/m156)

use skia_rust_pdf::utils::get_date_time;

use crate::{def_test, errorf, reporter_assert};

// TODO(future generation): update these values.
const MINIMUM_SANE_YEAR: u16 = 1964;
const MAXIMUM_SANE_YEAR: u16 = 2064;

// Port of: tests/Time.cpp#L19-L61 (chrome/m156)
def_test!(SkPDFUtils_GetDateTime, |r| {
    let date_time = get_date_time();
    if date_time.year < MINIMUM_SANE_YEAR {
        errorf!(
            r,
            "SkPDFUtils::GetDateTime: {} (CurrentYear) < {} (MinimumSaneYear)",
            date_time.year,
            MINIMUM_SANE_YEAR
        );
    }
    if date_time.year > MAXIMUM_SANE_YEAR {
        errorf!(
            r,
            "SkPDFUtils::GetDateTime: {} (CurrentYear) > {} (MaximumSaneYear)",
            date_time.year,
            MAXIMUM_SANE_YEAR
        );
    }
    reporter_assert!(r, date_time.month >= 1);
    reporter_assert!(r, date_time.month <= 12);
    reporter_assert!(r, date_time.day >= 1);
    reporter_assert!(r, date_time.day <= 31);
    reporter_assert!(r, date_time.hour <= 23);
    reporter_assert!(r, date_time.minute <= 59);
    reporter_assert!(r, date_time.second <= 60); // leap seconds are 23:59:60
    // The westernmost timezone is -12:00. The easternmost timezone is +14:00.
    reporter_assert!(r, i32::from(date_time.time_zone_minutes).abs() <= 14 * 60);
    let time_stamp = date_time.to_iso8601();
    reporter_assert!(r, !time_stamp.is_empty());
});
