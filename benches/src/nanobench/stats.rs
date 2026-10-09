// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/Stats.h

//! `Stats`: min, max, mean, variance and median of a bench's samples.

/// Port of `Stats` (without the console plot).
// Port of: tools/Stats.h#L22-L82 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub min: f64,
    pub max: f64,
    /// Estimate of population mean.
    pub mean: f64,
    /// Estimate of population variance.
    pub var: f64,
    pub median: f64,
}

impl Stats {
    /// `Stats(samples, want_plot)`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // mirrors `sum / n` with an int n
    pub fn new(samples: &[f64]) -> Self {
        let n = samples.len();
        if n == 0 {
            return Self {
                min: 0.0,
                max: 0.0,
                mean: 0.0,
                var: 0.0,
                median: 0.0,
            };
        }

        let mut min = samples[0];
        let mut max = samples[0];
        for &s in samples {
            if s < min {
                min = s;
            }
            if s > max {
                max = s;
            }
        }

        let mut sum = 0.0;
        for &s in samples {
            sum += s;
        }
        let mean = sum / n as f64;

        let mut err = 0.0;
        for &s in samples {
            err += (s - mean) * (s - mean);
        }
        // sk_ieee_double_divide(err, n-1): IEEE division, so 0/0 is NaN and x/0 is infinity.
        let var = err / (n as f64 - 1.0);

        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[n / 2];

        Self {
            min,
            max,
            mean,
            var,
            median,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)] // exact small values
    fn median_is_sorted_n_over_2() {
        let s = Stats::new(&[4.0, 1.0, 3.0, 2.0]);
        assert_eq!(s.min, 1.0);
        assert_eq!(s.max, 4.0);
        assert_eq!(s.median, 3.0); // sorted[4 / 2]
        assert_eq!(s.mean, 2.5);
        assert!((s.var - 5.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn one_sample_has_nan_variance() {
        assert!(Stats::new(&[2.0]).var.is_nan());
        assert_eq!(Stats::new(&[]).median, 0.0);
    }
}
