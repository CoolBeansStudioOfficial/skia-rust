// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The command trace (`docs/design/gpu.md` §3.2), behind the `trace` cargo feature.
//!
//! The oracle's `DawnCommandBuffer`, `DawnResourceProvider` and `DawnQueueManager` write a trace
//! of everything Graphite asks the GPU to do; the wgpu backend emits the same kinds of records
//! from the same places, so a trace of our port can be diffed against the oracle's (the GPU
//! counterpart of `rp-dump`/`rp-diff`):
//!
//! - every resource creation (`create_buffer`, `create_texture`, `create_sampler`);
//! - every pipeline (`create_pipeline`: label and the hashes of its WGSL);
//! - every mapped buffer flush and queue write (`flush_mapped_buffer`, `write_buffer`: length and
//!   a hash of the bytes, which go to [`TraceSink::blob`] once per hash);
//! - every render or compute pass (`begin_render_pass`, `end_render_pass`, `begin_compute_pass`,
//!   `end_compute_pass`) and its commands (`set_pipeline`, `set_bind_group`,
//!   `set_vertex_buffer`, `set_index_buffer`, `set_scissor_rect`, `set_viewport`,
//!   `set_immediates`, `set_blend_constant`, `draw*`, `dispatch*`);
//! - copies and clears (`copy_*`, `clear_buffer`);
//! - the submission (`submit`) and the bytes of a buffer read back by the CPU (`map_read`).
//!
//! A record is an operation name and an ordered list of fields; [`JsonLinesSink`] writes one JSON
//! object per line. Resources are named by a trace id that their creation record carries.

use std::fmt::Write as _;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

/// A field value of a [`Record`].
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// An unsigned integer.
    U(u64),
    /// A signed integer.
    I(i64),
    /// A float.
    F(f64),
    /// A string.
    S(String),
    /// A list.
    List(Vec<Value>),
}

/// One record of the trace.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// The operation.
    pub op: &'static str,
    /// The fields, in the order they were added.
    pub fields: Vec<(&'static str, Value)>,
}

impl Record {
    /// A record of operation `op`.
    #[must_use]
    pub fn new(op: &'static str) -> Self {
        Self {
            op,
            fields: Vec::new(),
        }
    }

    /// Adds an unsigned field.
    #[must_use]
    pub fn u(mut self, name: &'static str, value: impl Into<u64>) -> Self {
        self.fields.push((name, Value::U(value.into())));
        self
    }

    /// Adds a signed field.
    #[must_use]
    pub fn i(mut self, name: &'static str, value: impl Into<i64>) -> Self {
        self.fields.push((name, Value::I(value.into())));
        self
    }

    /// Adds a float field.
    #[must_use]
    pub fn f(mut self, name: &'static str, value: impl Into<f64>) -> Self {
        self.fields.push((name, Value::F(value.into())));
        self
    }

    /// Adds a string field.
    #[must_use]
    pub fn s(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.fields.push((name, Value::S(value.into())));
        self
    }

    /// Adds a list of unsigned numbers.
    #[must_use]
    pub fn list(mut self, name: &'static str, values: impl IntoIterator<Item = u64>) -> Self {
        self.fields.push((
            name,
            Value::List(values.into_iter().map(Value::U).collect()),
        ));
        self
    }

    /// Adds a list of strings.
    #[must_use]
    pub fn strings(mut self, name: &'static str, values: impl IntoIterator<Item = String>) -> Self {
        self.fields.push((
            name,
            Value::List(values.into_iter().map(Value::S).collect()),
        ));
        self
    }

    /// The record as a JSON object (`{"op":…,field:…}`).
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut out = format!("{{\"op\":{}", json_string(self.op));
        for (name, value) in &self.fields {
            out.push(',');
            out.push_str(&json_string(name));
            out.push(':');
            value.write_json(&mut out);
        }
        out.push('}');
        out
    }
}

impl Value {
    fn write_json(&self, out: &mut String) {
        match self {
            Value::U(value) => out.push_str(&value.to_string()),
            Value::I(value) => out.push_str(&value.to_string()),
            // Rust's shortest round-trip formatting; non-finite floats are not JSON numbers.
            Value::F(value) if value.is_finite() => {
                let _ = write!(out, "{value:?}");
            }
            Value::F(value) => out.push_str(&json_string(&format!("{value}"))),
            Value::S(value) => out.push_str(&json_string(value)),
            Value::List(values) => {
                out.push('[');
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    value.write_json(out);
                }
                out.push(']');
            }
        }
    }
}

fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// FNV-1a over `bytes`: the hash that names a blob in the trace.
#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// A new trace id for a resource.
#[must_use]
pub fn next_trace_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Where the records go.
pub trait TraceSink: Send + std::fmt::Debug {
    /// A record, in the order the operations were recorded.
    fn record(&mut self, record: &Record);

    /// The bytes of the blob with the given hash, delivered the first time the hash is used.
    fn blob(&mut self, _hash: u64, _bytes: &[u8]) {}
}

/// A [`TraceSink`] that writes one JSON object per line and keeps no blobs.
pub struct JsonLinesSink<W: Write + Send>(pub W);

impl<W: Write + Send> std::fmt::Debug for JsonLinesSink<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsonLinesSink").finish_non_exhaustive()
    }
}

impl<W: Write + Send> TraceSink for JsonLinesSink<W> {
    fn record(&mut self, record: &Record) {
        // A trace that cannot be written is lost, but must not fail the rendering.
        let _ = writeln!(self.0, "{}", record.to_json());
    }
}

/// A [`TraceSink`] that keeps the records (and blobs) in memory.
#[derive(Debug, Default)]
pub struct MemorySink {
    /// The records.
    pub records: std::sync::Arc<std::sync::Mutex<Vec<Record>>>,
    /// The blobs by hash.
    pub blobs: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<u64, Vec<u8>>>>,
}

impl TraceSink for MemorySink {
    fn record(&mut self, record: &Record) {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(record.clone());
    }

    fn blob(&mut self, hash: u64, bytes: &[u8]) {
        self.blobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(hash)
            .or_insert_with(|| bytes.to_vec());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_json_objects() {
        let record = Record::new("draw")
            .u("vertex_count", 6_u32)
            .i("base_vertex", -1_i32)
            .f("depth", 0.5_f32)
            .s("label", "a \"b\"\n")
            .list("offsets", [0, 16]);
        assert_eq!(
            record.to_json(),
            "{\"op\":\"draw\",\"vertex_count\":6,\"base_vertex\":-1,\"depth\":0.5,\
             \"label\":\"a \\\"b\\\"\\n\",\"offsets\":[0,16]}"
        );
    }

    #[test]
    fn hashes_are_fnv1a() {
        assert_eq!(hash_bytes(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(hash_bytes(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
