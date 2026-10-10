// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/docs/SkPDFDocument.h#L76-L88, src/pdf/SkPDFDocument.cpp#L737-L750 (chrome/m156)

//! `SkPDF::DateTime`: a timestamp for the document metadata.

use std::fmt::Write as _;

/// `SkPDF::DateTime`.
// Port of: include/docs/SkPDFDocument.h#L76-L88 (chrome/m156)
#[doc(alias = "SkPDF::DateTime")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DateTime {
    /// The number of minutes that this is ahead of or behind UTC.
    pub time_zone_minutes: i16,
    /// e.g. 2005.
    pub year: u16,
    /// 1..12.
    pub month: u8,
    /// 0..6, 0 is Sunday.
    pub day_of_week: u8,
    /// 1..31.
    pub day: u8,
    /// 0..23.
    pub hour: u8,
    /// 0..59.
    pub minute: u8,
    /// 0..59.
    pub second: u8,
}

impl DateTime {
    /// `SkPDF::DateTime::toISO8601`: `YYYY-MM-DDThh:mm:ss+hh:mm`.
    // Port of: src/pdf/SkPDFDocument.cpp#L737-L750 (chrome/m156)
    #[doc(alias = "toISO8601")]
    #[must_use]
    pub fn to_iso8601(&self) -> String {
        let time_zone_minutes = i32::from(self.time_zone_minutes);
        let timezone_sign = if time_zone_minutes >= 0 { '+' } else { '-' };
        let time_zone_hours = time_zone_minutes.abs() / 60;
        let time_zone_minutes = time_zone_minutes.abs() % 60;
        let mut dst = String::new();
        // `printf` into a `SkString`: the widths are minimums, so nothing is truncated.
        let _ = write!(
            dst,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}{:02}:{:02}",
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
            timezone_sign,
            time_zone_hours,
            time_zone_minutes
        );
        dst
    }
}
