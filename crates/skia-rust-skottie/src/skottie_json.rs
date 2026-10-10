// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/SkottieJson.h, modules/skottie/src/SkottieJson.cpp
// (chrome/m156)
//
// The `Parse<T>` family: typed readers over [`json::Value`]. Like Skia's, [`Parse::parse`] writes
// its output in place and may leave a partially written value behind when it fails (a `Vec2`
// whose `x` parsed but whose `y` did not), which the callers rely on.

use skia_rust_core::m44::V2;
use skia_rust_core::point::Point;

use crate::json::{ArrayValue, BoolValue, NumberValue, ObjectValue, StringValue, Value};
use crate::skottie_value::VectorValue;

/// The typed views of a [`Value`]: Skia's implicit `Value` -> `const T*` conversions, where a
/// value of another kind converts to a null pointer.
pub trait ValueExt {
    /// The value as an object, if it is one.
    fn as_object(&self) -> Option<&ObjectValue>;
    /// The value as an array, if it is one.
    fn as_array(&self) -> Option<&ArrayValue>;
    /// The value as a string, if it is one.
    fn as_string(&self) -> Option<&StringValue>;
    /// The value as a number, if it is one.
    fn as_number(&self) -> Option<&NumberValue>;
    /// The value as a boolean, if it is one.
    fn as_bool(&self) -> Option<&BoolValue>;
}

impl ValueExt for Value {
    fn as_object(&self) -> Option<&ObjectValue> {
        self.get::<ObjectValue>()
    }

    fn as_array(&self) -> Option<&ArrayValue> {
        self.get::<ArrayValue>()
    }

    fn as_string(&self) -> Option<&StringValue> {
        self.get::<StringValue>()
    }

    fn as_number(&self) -> Option<&NumberValue> {
        self.get::<NumberValue>()
    }

    fn as_bool(&self) -> Option<&BoolValue> {
        self.get::<BoolValue>()
    }
}

/// A piece of JSON that a log message can refer to.
pub trait LogJson {
    /// The JSON text (`Value::toString`).
    fn json_text(&self) -> Vec<u8>;
}

impl LogJson for Value {
    fn json_text(&self) -> Vec<u8> {
        self.to_bytes()
    }
}

impl LogJson for ObjectValue {
    fn json_text(&self) -> Vec<u8> {
        self.to_bytes()
    }
}

/// The text of a JSON string value, as Skia reads it through `const char*` (`StringValue::begin`).
// Port of: modules/jsonreader/SkJSONReader.h#L249-L263 (chrome/m156) (`StringValue::begin`)
#[must_use]
pub fn string_text(value: &StringValue) -> String {
    String::from_utf8_lossy(value.as_bytes()).into_owned()
}

/// A type that can be parsed from a JSON [`Value`] (`Parse<T>`).
// Port of: modules/skottie/src/SkottieJson.h#L19-L20 (chrome/m156) (`Parse`)
pub trait Parse: Sized {
    /// Parses `value` into `out`, which may be partially written on failure.
    fn parse(value: &Value, out: &mut Self) -> bool;
}

/// `Parse<T>(v, &res)` returning the result by value; `None` on failure.
#[must_use]
pub fn parse_value<T: Parse + Default>(value: &Value) -> Option<T> {
    let mut out = T::default();
    T::parse(value, &mut out).then_some(out)
}

/// `ParseDefault<T>`: the parsed value, or `default` if the parse fails.
// Port of: modules/skottie/src/SkottieJson.h#L22-L29 (chrome/m156) (`ParseDefault`)
#[must_use]
pub fn parse_default<T: Parse + Default>(value: &Value, default: T) -> T {
    let mut res = T::default();
    if T::parse(value, &mut res) {
        res
    } else {
        default
    }
}

/// The `sid` string of `jobj`, if it has one (`ParseSlotID`).
// Port of: modules/skottie/src/SkottieJson.cpp#L131-L138 (chrome/m156) (`ParseSlotID`)
#[must_use]
pub fn parse_slot_id(jobj: Option<&ObjectValue>) -> Option<&StringValue> {
    jobj?.get("sid").as_string()
}

// Port of: modules/skottie/src/SkottieJson.cpp#L25-L40 (chrome/m156) (`Parse<SkScalar>`)
impl Parse for f32 {
    fn parse(v: &Value, s: &mut Self) -> bool {
        // Some versions wrap values as single-element arrays.
        if let Some(array) = v.as_array()
            && array.size() > 0
        {
            return Self::parse(&array[0], s);
        }

        if let Some(num) = v.as_number() {
            #[allow(clippy::cast_possible_truncation)] // mirrors static_cast<SkScalar>(double)
            let value = num.value() as f32;
            *s = value;
            return true;
        }

        false
    }
}

// Port of: modules/skottie/src/SkottieJson.cpp#L42-L56 (chrome/m156) (`Parse<bool>`)
impl Parse for bool {
    #[allow(clippy::float_cmp)] // SkToBool(double) is `x != 0`
    fn parse(v: &Value, b: &mut Self) -> bool {
        match v {
            Value::Number(num) => {
                *b = num.value() != 0.0;
                true
            }
            Value::Bool(value) => {
                *b = value.value();
                true
            }
            _ => false,
        }
    }
}

/// `ParseIntegral<T>`: a number that fits in `T`, truncated.
// Port of: modules/skottie/src/SkottieJson.cpp#L58-L70 (chrome/m156) (`ParseIntegral`)
macro_rules! parse_integral {
    ($ty:ty) => {
        impl Parse for $ty {
            fn parse(v: &Value, result: &mut Self) -> bool {
                if let Some(num) = v.as_number() {
                    let dbl = num.value();
                    #[allow(clippy::cast_precision_loss, clippy::cast_lossless)]
                    // mirrors static_cast<double>(max)
                    if dbl > <$ty>::MAX as f64 || dbl < <$ty>::MIN as f64 {
                        return false;
                    }

                    // In range (checked above): the cast truncates, as static_cast<T>(double).
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let value = dbl as $ty;
                    *result = value;
                    return true;
                }

                false
            }
        }
    };
}

parse_integral!(i32);
parse_integral!(usize);

// Port of: modules/skottie/src/SkottieJson.cpp#L82-L90 (chrome/m156) (`Parse<SkString>`)
impl Parse for String {
    fn parse(v: &Value, s: &mut Self) -> bool {
        if let Some(sv) = v.as_string() {
            *s = string_text(sv);
            return true;
        }

        false
    }
}

// Port of: modules/skottie/src/SkottieJson.cpp#L92-L103 (chrome/m156) (`Parse<SkV2>`)
impl Parse for V2 {
    fn parse(v: &Value, v2: &mut Self) -> bool {
        let Some(av) = v.as_array() else {
            return false;
        };

        // We need at least two scalars (BM sometimes exports a third value == 0).
        av.size() >= 2 && f32::parse(&av[0], &mut v2.x) && f32::parse(&av[1], &mut v2.y)
    }
}

// Port of: modules/skottie/src/SkottieJson.cpp#L105-L114 (chrome/m156) (`Parse<SkPoint>`)
impl Parse for Point {
    fn parse(v: &Value, pt: &mut Self) -> bool {
        let Some(ov) = v.as_object() else {
            return false;
        };

        f32::parse(ov.get("x"), &mut pt.x) && f32::parse(ov.get("y"), &mut pt.y)
    }
}

// Port of: modules/skottie/src/SkottieJson.cpp#L116-L129 (chrome/m156) (`Parse<VectorValue>`)
impl Parse for VectorValue {
    fn parse(v: &Value, vec: &mut Self) -> bool {
        let Some(av) = v.as_array() else {
            return false;
        };

        vec.resize(av.size(), 0.0);
        for i in 0..av.size() {
            if !f32::parse(&av[i], &mut vec[i]) {
                return false;
            }
        }

        true
    }
}
