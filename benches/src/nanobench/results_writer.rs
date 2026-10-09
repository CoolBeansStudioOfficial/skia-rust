// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ResultsWriter.h (NanoJSONResultsWriter)

//! `NanoJSONResultsWriter`: nanobench's `--outResultsFile` JSON.
//!
//! ```text
//! { "key": {...}, "options": {...},
//!   "results": { "<uniqueName>_<w>_<h>": { "<config>": {
//!       "options": {"name": ...}, "min_ms": ..., "min_ratio": ..., "samples": [...] } } } }
//! ```

use serde_json::{Map, Value, json};

/// One `(benchmark, config)` result.
#[derive(Debug)]
pub struct BenchResult<'a> {
    /// `<uniqueName>_<w>_<h>` ([`NanoJsonResultsWriter::bench_id`]).
    pub bench_id: &'a str,
    pub config: &'a str,
    /// `bench->getName()`.
    pub name: &'a str,
    /// Free-form option strings (nanobench's schema allows any), e.g. `manifest_id`, `loops`.
    pub options: &'a [(&'a str, String)],
    pub min_ms: f64,
    /// `median / min`.
    pub min_ratio: f64,
    pub samples: &'a [f64],
}

/// Collects results and renders nanobench's JSON.
#[derive(Debug, Default)]
pub struct NanoJsonResultsWriter {
    key: Map<String, Value>,
    options: Map<String, Value>,
    results: Map<String, Value>,
}

impl NanoJsonResultsWriter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An entry of the top-level `key` block.
    pub fn add_key(&mut self, k: &str, v: &str) {
        self.key.insert(k.to_owned(), json!(v));
    }

    /// An entry of the top-level `options` block.
    pub fn add_option(&mut self, k: &str, v: &str) {
        self.options.insert(k.to_owned(), json!(v));
    }

    /// `beginBench(name, x, y)`: the result id is `<name>_<x>_<y>`.
    // Port of: bench/ResultsWriter.h#L45-L48 (chrome/m156)
    #[must_use]
    pub fn bench_id(name: &str, x: i32, y: i32) -> String {
        format!("{name}_{x}_{y}")
    }

    /// Adds one result. Like `appendMetric`, a metric that is NaN or infinite is not recorded.
    // Port of: bench/ResultsWriter.h#L52-L57 (chrome/m156)
    pub fn add_result(&mut self, r: &BenchResult<'_>) {
        let mut options = Map::new();
        options.insert("name".to_owned(), json!(r.name));
        for (k, v) in r.options {
            options.insert((*k).to_owned(), json!(v));
        }
        let mut result = Map::new();
        result.insert("options".to_owned(), Value::Object(options));
        if r.min_ms.is_finite() {
            result.insert("min_ms".to_owned(), json!(r.min_ms));
        }
        if r.min_ratio.is_finite() {
            result.insert("min_ratio".to_owned(), json!(r.min_ratio));
        }
        result.insert("samples".to_owned(), json!(r.samples));
        let bench = self
            .results
            .entry(r.bench_id.to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(bench) = bench {
            bench.insert(r.config.to_owned(), Value::Object(result));
        }
    }

    /// The whole document.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "key": self.key,
            "options": self.options,
            "results": self.results,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_nanobench() {
        let mut w = NanoJsonResultsWriter::new();
        w.add_key("impl", "skia-rust");
        let id = NanoJsonResultsWriter::bench_id("math_noOp", 640, 480);
        assert_eq!(id, "math_noOp_640_480");
        w.add_result(&BenchResult {
            bench_id: &id,
            config: "nonrendering",
            name: "math_noOp",
            options: &[("loops", "7".to_owned())],
            min_ms: 1.0,
            min_ratio: 1.5,
            samples: &[1.0, 2.0],
        });
        let j = w.to_json();
        let r = &j["results"]["math_noOp_640_480"]["nonrendering"];
        assert_eq!(r["options"]["name"], "math_noOp");
        assert_eq!(r["options"]["loops"], "7");
        assert_eq!(r["min_ms"], 1.0);
        assert_eq!(r["samples"][1], 2.0);
        assert_eq!(j["key"]["impl"], "skia-rust");
    }

    #[test]
    fn non_finite_metrics_are_not_recorded() {
        let mut w = NanoJsonResultsWriter::new();
        w.add_result(&BenchResult {
            bench_id: "b_1_1",
            config: "8888",
            name: "b",
            options: &[],
            min_ms: f64::NAN,
            min_ratio: f64::INFINITY,
            samples: &[1.0],
        });
        let j = w.to_json();
        let r = &j["results"]["b_1_1"]["8888"];
        assert!(r.get("min_ms").is_none());
        assert!(r.get("min_ratio").is_none());
    }
}
