// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkSafeRange.h

//! `SkSafeRange`: checks that a series of operations are in range.

/// Always checks that a series of operations are in-range. The check is sticky: if any one
/// operation fails, the object remembers that and [`ok`](Self::ok) is false.
// Port of: src/core/SkSafeRange.h#L18-L46 (chrome/m156)
#[doc(alias = "SkSafeRange")]
#[derive(Clone, Copy, Debug)]
pub struct SafeRange {
    ok: bool,
}

impl Default for SafeRange {
    fn default() -> Self {
        SafeRange { ok: true }
    }
}

impl SafeRange {
    /// A range with no failure.
    #[must_use]
    pub fn new() -> SafeRange {
        SafeRange::default()
    }

    /// Whether every check passed (`ok`, and `operator bool`).
    #[must_use]
    pub fn ok(&self) -> bool {
        self.ok
    }

    /// Checks `0 <= value <= max`. On success returns `value`; on failure returns 0 and clears
    /// [`ok`](Self::ok) (`checkLE`).
    // Port of: src/core/SkSafeRange.h#L28-L36 (chrome/m156)
    #[doc(alias = "checkLE")]
    pub fn check_le(&mut self, mut value: u64, max: u64) -> u64 {
        if value > max {
            self.ok = false;
            value = 0;
        }
        value
    }

    /// Checks `value >= min`. On failure returns `min` and clears [`ok`](Self::ok)
    /// (`checkGE`).
    // Port of: src/core/SkSafeRange.h#L38-L44 (chrome/m156)
    #[doc(alias = "checkGE")]
    pub fn check_ge(&mut self, mut value: i32, min: i32) -> i32 {
        if value < min {
            self.ok = false;
            value = min;
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_are_sticky() {
        let mut safe = SafeRange::new();
        assert!(safe.ok());
        assert_eq!(safe.check_le(5, 10), 5);
        assert_eq!(safe.check_ge(3, 0), 3);
        assert!(safe.ok());
        assert_eq!(safe.check_ge(-1, 0), 0);
        assert!(!safe.ok());
        // Later successes do not clear the failure.
        assert_eq!(safe.check_le(1, 2), 1);
        assert!(!safe.ok());

        let mut safe = SafeRange::new();
        assert_eq!(safe.check_le(11, 10), 0);
        assert!(!safe.ok());
    }
}
