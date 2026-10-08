// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkSLDebugTraceTest.cpp (chrome/m156)

//! `DebugTracePriv`: the source, the slot names and the component suffixes of a debug trace, and
//! the JSON round trip of a trace (`SkSLTraceUtils::WriteTrace` / `ReadTrace`).

use std::sync::Mutex;

use skia_rust_sksl::ir::NumberKind;
use skia_rust_sksl::position::Position;
use skia_rust_sksl::tracing::{
    DebugTracePriv, FunctionDebugInfo, SlotDebugInfo, TraceInfo, TraceOp,
};

use crate::tools::sksl_trace_utils::{read_trace, write_trace};
use crate::{def_test, reporter_assert};

/// A `SlotDebugInfo` in the field order of the C++ aggregate initializers used by the test.
fn slot(name: &str, columns: u8, rows: u8, component_index: u8) -> SlotDebugInfo {
    SlotDebugInfo {
        name: name.to_owned(),
        columns,
        rows,
        component_index,
        group_index: i32::from(component_index),
        number_kind_raw: NumberKind::Float as i32,
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

// Port of: tests/SkSLDebugTraceTest.cpp#L47-L83 (chrome/m156)
def_test!(DebugTracePrivWrite, |r| {
    let i = DebugTracePriv {
        source: vec![
            "\t// first line".to_owned(),
            "// \"second line\"".to_owned(),
            "//\\\\//\\\\ third line".to_owned(),
        ],
        slot_info: vec![
            SlotDebugInfo {
                name: "SkSL_DebugTrace".to_owned(),
                columns: 1,
                rows: 2,
                component_index: 3,
                group_index: 4,
                number_kind_raw: 5,
                line: 6,
                pos: Position::default(),
                fn_return_value: -1,
            },
            SlotDebugInfo {
                name: "Unit_Test".to_owned(),
                columns: 6,
                rows: 7,
                component_index: 8,
                group_index: 8,
                number_kind_raw: 10,
                line: 11,
                pos: Position::default(),
                fn_return_value: 12,
            },
        ],
        func_info: vec![FunctionDebugInfo {
            name: "void testFunc();".to_owned(),
        }],
        trace_info: Mutex::new(vec![
            TraceInfo {
                op: TraceOp::Enter,
                data: [0, 0],
            },
            TraceInfo {
                op: TraceOp::Line,
                data: [5, 0],
            },
            TraceInfo {
                op: TraceOp::Var,
                data: [10, 15],
            },
            TraceInfo {
                op: TraceOp::Exit,
                data: [20, 0],
            },
        ]),
        ..DebugTracePriv::default()
    };
    let actual = write_trace(&i);

    let expected = concat!(
        r#"{"version":"20220209","source":["\t// first line","// \"second line\"","//\\\\//\\"#,
        r#"\\ third line"],"slots":[{"name":"SkSL_DebugTrace","columns":1,"rows":2,"index":3,"#,
        r#""groupIdx":4,"kind":5,"line":6},{"name":"Unit_Test","columns":6,"rows":7,"index":8"#,
        r#","kind":10,"line":11,"retval":12}],"functions":[{"name":"void testFunc();"}],"trac"#,
        r#"e":[[2],[0,5],[1,10,15],[3,20]]}"#,
    );

    reporter_assert!(r, actual == expected);
});

// Port of: tests/SkSLDebugTraceTest.cpp#L85-L141 (chrome/m156)
def_test!(DebugTracePrivRead, |r| {
    let json = concat!(
        r#"{"version":"20220209","source":["\t// first line","// \"second line\"","//\\\\//\\"#,
        r#"\\ third line"],"slots":[{"name":"SkSL_DebugTrace","columns":1,"rows":2,"index":3,"#,
        r#""groupIdx":4,"kind":5,"line":6},{"name":"Unit_Test","columns":6,"rows":7,"index":8"#,
        r#","kind":10,"line":11,"retval":12}],"functions":[{"name":"void testFunc();"}],"trac"#,
        r#"e":[[2],[0,5],[1,10,15],[3,20]]}"#,
    );

    let trace = read_trace(json.as_bytes());
    reporter_assert!(r, trace.is_some());
    let Some(trace) = trace else {
        return;
    };
    let ops = trace.trace_ops();

    reporter_assert!(r, trace.source.len() == 3);
    reporter_assert!(r, trace.slot_info.len() == 2);
    reporter_assert!(r, trace.func_info.len() == 1);
    reporter_assert!(r, ops.len() == 4);

    reporter_assert!(r, trace.source[0] == "\t// first line");
    reporter_assert!(r, trace.source[1] == "// \"second line\"");
    reporter_assert!(r, trace.source[2] == "//\\\\//\\\\ third line");

    reporter_assert!(r, trace.slot_info[0].name == "SkSL_DebugTrace");
    reporter_assert!(r, trace.slot_info[0].columns == 1);
    reporter_assert!(r, trace.slot_info[0].rows == 2);
    reporter_assert!(r, trace.slot_info[0].component_index == 3);
    reporter_assert!(r, trace.slot_info[0].group_index == 4);
    reporter_assert!(r, trace.slot_info[0].number_kind_raw == 5);
    reporter_assert!(r, trace.slot_info[0].line == 6);
    reporter_assert!(r, trace.slot_info[0].fn_return_value == -1);

    reporter_assert!(r, trace.slot_info[1].name == "Unit_Test");
    reporter_assert!(r, trace.slot_info[1].columns == 6);
    reporter_assert!(r, trace.slot_info[1].rows == 7);
    reporter_assert!(r, trace.slot_info[1].component_index == 8);
    reporter_assert!(r, trace.slot_info[1].group_index == 8);
    reporter_assert!(r, trace.slot_info[1].number_kind_raw == 10);
    reporter_assert!(r, trace.slot_info[1].line == 11);
    reporter_assert!(r, trace.slot_info[1].fn_return_value == 12);

    reporter_assert!(r, trace.func_info[0].name == "void testFunc();");

    reporter_assert!(r, ops[0].op == TraceOp::Enter);
    reporter_assert!(r, ops[0].data[0] == 0);
    reporter_assert!(r, ops[0].data[1] == 0);

    reporter_assert!(r, ops[1].op == TraceOp::Line);
    reporter_assert!(r, ops[1].data[0] == 5);
    reporter_assert!(r, ops[1].data[1] == 0);

    reporter_assert!(r, ops[2].op == TraceOp::Var);
    reporter_assert!(r, ops[2].data[0] == 10);
    reporter_assert!(r, ops[2].data[1] == 15);

    reporter_assert!(r, ops[3].op == TraceOp::Exit);
    reporter_assert!(r, ops[3].data[0] == 20);
    reporter_assert!(r, ops[3].data[1] == 0);
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
