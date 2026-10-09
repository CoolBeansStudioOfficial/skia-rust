// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/SkLog.h

//! `SKIA_LOG_E` and `SKIA_LOG_W`: the diagnostics the GPU code prints, to standard error.

/// `SKIA_LOG_E(fmt, ...)`.
// Port of: include/private/SkLog.h#L43 (chrome/m156)
macro_rules! skia_log_e {
    ($($arg:tt)*) => {
        eprintln!("[skia] ** ERROR ** {}", format_args!($($arg)*))
    };
}

/// `SKIA_LOG_W(fmt, ...)`.
// Port of: include/private/SkLog.h#L44 (chrome/m156)
macro_rules! skia_log_w {
    ($($arg:tt)*) => {
        eprintln!("[skia] WARNING - {}", format_args!($($arg)*))
    };
}

pub(crate) use skia_log_e;
pub(crate) use skia_log_w;
