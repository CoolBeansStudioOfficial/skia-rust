// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SerializationTest.cpp (chrome/m156)

use crate::{def_test, reporter_assert};
use skia_rust_effects::dash_path_effect;

// Port of: tests/SerializationTest.cpp#L1190-L1204 (chrome/m156), WriteBuffer_external_memory_flattenable
def_test!(WriteBuffer_external_memory_flattenable, |reporter| {
    let intervals = [1.0, 1.0];
    let path_effect = dash_path_effect::new(&intervals, 0.0).expect("a valid dash effect");
    // SkAlign4 of the serialized size.
    let path_size = (path_effect.serialize().size() + 3) & !3;
    reporter_assert!(reporter, path_size > 4);

    // Too small external storage: nothing is written.
    let mut storage = vec![0u8; path_size - 4];
    reporter_assert!(reporter, path_effect.serialize_into(&mut storage) == 0);

    let mut storage = vec![0u8; path_size];
    reporter_assert!(reporter, path_effect.serialize_into(&mut storage) != 0);
});
