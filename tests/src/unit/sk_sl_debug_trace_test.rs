// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkSLDebugTraceTest.cpp (chrome/m156)

//! `DebugTracePriv`: the source, the slot names and the component suffixes of a debug trace.
//!
//! Not ported yet, and left `todo` in the manifest: `DebugTracePrivWrite` and `DebugTracePrivRead`
//! (they need `SkSLTraceUtils`, which reads and writes JSON, and they store out-of-range
//! `NumberKind` values, which a Rust enum cannot hold).

use skia_rust_sksl::ir::NumberKind;
use skia_rust_sksl::position::Position;
use skia_rust_sksl::tracing::{DebugTracePriv, SlotDebugInfo};

use crate::{def_test, reporter_assert};

/// A `SlotDebugInfo` in the field order of the C++ aggregate initializers used by the test.
fn slot(name: &str, columns: u8, rows: u8, component_index: u8) -> SlotDebugInfo {
    SlotDebugInfo {
        name: name.to_owned(),
        columns,
        rows,
        component_index,
        group_index: i32::from(component_index),
        number_kind: NumberKind::Float,
        line: 0,
        pos: Position::default(),
        fn_return_value: -1,
    }
}

// Port of: tests/SkSLDebugTraceTest.cpp#L23-L35 (chrome/m156)
def_test!(DebugTracePrivSetSource, |r| {
    let mut i = DebugTracePriv::default();
    i.set_source(
        "DebugTracePriv::setSource unit test\n\t// first line\n\t// second line\n\t// third line"
            .as_bytes(),
    );

    reporter_assert!(r, i.source.len() == 4);
    reporter_assert!(r, i.source[0] == "DebugTracePriv::setSource unit test");
    reporter_assert!(r, i.source[1] == "\t// first line");
    reporter_assert!(r, i.source[2] == "\t// second line");
    reporter_assert!(r, i.source[3] == "\t// third line");
});

// Port of: tests/SkSLDebugTraceTest.cpp#L37-L45 (chrome/m156)
def_test!(DebugTracePrivSetSourceReplacesExistingText, |r| {
    let mut i = DebugTracePriv::default();
    i.set_source(b"One");
    i.set_source(b"Two");
    i.set_source(b"Three");

    reporter_assert!(r, i.source.len() == 1);
    reporter_assert!(r, i.source[0] == "Three");
});

// Port of: tests/SkSLDebugTraceTest.cpp#L143-L187 (chrome/m156)
def_test!(DebugTracePrivGetSlotComponentSuffix, |r| {
    // SlotDebugInfo fields:
    // - name
    // - columns
    // - rows
    // - componentIndex
    // - numberKind
    // - line
    // - fnReturnValue

    let i = DebugTracePriv {
        slot_info: vec![
            slot("s", 1, 1, 0),
            slot("v", 4, 1, 0),
            slot("v", 4, 1, 1),
            slot("v", 4, 1, 2),
            slot("v", 4, 1, 3),
            slot("m", 4, 4, 0),
            slot("m", 4, 4, 1),
            slot("m", 4, 4, 2),
            slot("m", 4, 4, 3),
            slot("m", 4, 4, 4),
            slot("m", 4, 4, 5),
            slot("m", 4, 4, 6),
            slot("m", 4, 4, 7),
            slot("m", 4, 4, 8),
            slot("m", 4, 4, 9),
            slot("m", 4, 4, 10),
            slot("m", 4, 4, 11),
            slot("m", 4, 4, 12),
            slot("m", 4, 4, 13),
            slot("m", 4, 4, 14),
            slot("m", 4, 4, 15),
        ],
        ..DebugTracePriv::default()
    };

    let expected: [&str; 21] = [
        "", ".x", ".y", ".z", ".w", "[0][0]", "[0][1]", "[0][2]", "[0][3]", "[1][0]", "[1][1]",
        "[1][2]", "[1][3]", "[2][0]", "[2][1]", "[2][2]", "[2][3]", "[3][0]", "[3][1]", "[3][2]",
        "[3][3]",
    ];

    reporter_assert!(r, i.slot_info.len() == expected.len());
    for (index, want) in expected.iter().enumerate() {
        reporter_assert!(r, *want == i.get_slot_component_suffix(index));
    }
});
