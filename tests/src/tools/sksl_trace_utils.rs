// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tools/sksltrace/SkSLTraceUtils.{h,cpp} (chrome/m156)

//! Reads and writes the JSON form of a `DebugTracePriv` (the `sksltrace` tool's format).

use std::sync::Arc;

use skia_rust_sksl::tracing::{DebugTracePriv, FunctionDebugInfo, SlotDebugInfo, TraceInfo, TraceOp};

use super::json::{self, JsonValue, JsonWriter};

const TRACE_VERSION: &str = "20220209";

/// `SkSLTraceUtils::WriteTrace`: the trace as JSON text (`SkJSONWriter` kFast output).
// Port of: tools/sksltrace/SkSLTraceUtils.cpp#L26-L92 (chrome/m156)
#[must_use]
pub fn write_trace(src: &DebugTracePriv) -> String {
    let mut json = JsonWriter::new();

    json.begin_object(None); // root
    json.append_string(Some("version"), TRACE_VERSION);
    json.begin_array(Some("source"));

    for line in &src.source {
        json.append_string(None, line);
    }

    json.end_array(); // code
    json.begin_array(Some("slots"));

    for info in &src.slot_info {
        json.begin_object(None);
        json.append_string(Some("name"), &info.name);
        json.append_s32(Some("columns"), i32::from(info.columns));
        json.append_s32(Some("rows"), i32::from(info.rows));
        json.append_s32(Some("index"), i32::from(info.component_index));
        if info.group_index != i32::from(info.component_index) {
            json.append_s32(Some("groupIdx"), info.group_index);
        }
        json.append_s32(Some("kind"), info.number_kind_raw);
        json.append_s32(Some("line"), info.line);
        if info.fn_return_value >= 0 {
            json.append_s32(Some("retval"), info.fn_return_value);
        }
        json.end_object();
    }

    json.end_array(); // slots
    json.begin_array(Some("functions"));

    for info in &src.func_info {
        json.begin_object(None);
        json.append_string(Some("name"), &info.name);
        json.end_object();
    }

    json.end_array(); // functions
    json.begin_array(Some("trace"));

    for trace in src.trace_ops() {
        json.begin_array(None);
        json.append_s32(None, trace.op.as_raw());

        // Skip trailing zeros in the data (since most ops only use one value).
        let data_len = trace.data.iter().rposition(|&v| v != 0).map_or(0, |i| i + 1);
        for value in &trace.data[..data_len] {
            json.append_s32(None, *value);
        }
        json.end_array();
    }

    json.end_array(); // trace
    json.end_object(); // root
    json.finish()
}

/// `SkSLTraceUtils::ReadTrace`: parses a trace written by [`write_trace`]. `None` if the text is
/// not a trace of the current version.
///
/// Trace ops whose raw value is not a `TraceOp` enumerator fail the read: the C++ code stores
/// the value in the enum regardless, which a Rust enum cannot do.
// Port of: tools/sksltrace/SkSLTraceUtils.cpp#L94-L211 (chrome/m156)
#[must_use]
pub fn read_trace(text: &[u8]) -> Option<Arc<DebugTracePriv>> {
    let root = json::parse(text)?;
    if !matches!(root, JsonValue::Object(_)) {
        return None;
    }

    let version = root.get("version")?.as_str()?;
    if version != TRACE_VERSION {
        return None;
    }

    let source = root.get("source")?.as_array()?;

    let mut dst = DebugTracePriv::default();
    for line in source {
        dst.source.push(line.as_str()?.to_owned());
    }

    let slots = root.get("slots")?.as_array()?;
    for element in slots {
        // Populate the SlotInfo with our JSON data.
        let name = element.get("name").and_then(JsonValue::as_str);
        let columns = element.get("columns").and_then(JsonValue::as_number);
        let rows = element.get("rows").and_then(JsonValue::as_number);
        let index = element.get("index").and_then(JsonValue::as_number);
        let group_idx = element.get("groupIdx").and_then(JsonValue::as_number);
        let kind = element.get("kind").and_then(JsonValue::as_number);
        let line = element.get("line").and_then(JsonValue::as_number);
        let retval = element.get("retval").and_then(JsonValue::as_number);
        let (Some(name), Some(columns), Some(rows), Some(index), Some(kind), Some(line)) =
            (name, columns, rows, index, kind, line)
        else {
            return None;
        };

        // The C++ code stores each number in its field's type (`uint8_t` for the dimensions,
        // `int32_t` for the rest), so the casts convert the same way for in-range values.
        let component_index = index as u8;
        let info = SlotDebugInfo {
            name: name.to_owned(),
            columns: columns as u8,
            rows: rows as u8,
            component_index,
            group_index: group_idx.map_or(i32::from(component_index), |g| g as i32),
            number_kind_raw: kind as i32,
            line: line as i32,
            fn_return_value: retval.map_or(-1, |r| r as i32),
            ..SlotDebugInfo::default()
        };
        dst.slot_info.push(info);
    }

    let functions = root.get("functions")?.as_array()?;
    for element in functions {
        let name = element.get("name")?.as_str()?;
        dst.func_info.push(FunctionDebugInfo {
            name: name.to_owned(),
        });
    }

    let trace = root.get("trace")?.as_array()?;
    let mut trace_info = Vec::with_capacity(trace.len());
    for element in trace {
        let items = element.as_array()?;
        if items.is_empty() || items.len() > 1 + 2 {
            return None;
        }
        let op = TraceOp::from_raw(items[0].as_number()? as i32)?;
        let mut data = [0i32; 2];
        for (slot, item) in data.iter_mut().zip(&items[1..]) {
            *slot = item.as_number()? as i32;
        }
        trace_info.push(TraceInfo { op, data });
    }
    *dst.trace_info.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = trace_info;

    Some(Arc::new(dst))
}
