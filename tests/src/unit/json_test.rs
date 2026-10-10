// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/JSONTest.cpp (chrome/m156)
//
// Not ported as is:
// - `SkAutoLocaleSetter commaLocale("de_DE.UTF-8")` in `JSON_ParseNumber`: Rust's float parsing
//   never reads the C locale, so there is nothing to set. The assertions are unchanged.
// - `vec.begin() != nullptr` and `vec.end() == vec.begin() + n` in `check_vector`: pointer
//   checks of the C++ iterators. The length check above them covers the same contract, and
//   slices are never null.

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::scalar::Scalar;
use skia_rust_core::scalar::scalar;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_skottie::json::{
    ArrayValue, BoolValue, DOM, Facade, Member, NullValue, NumberValue, ObjectValue, StringValue,
    Value,
};

/// The bytes of a C string: everything before the first NUL, as `strlen` sees them.
fn c_str_bytes(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())]
}

// Port of: tests/JSONTest.cpp#L23-L153 (chrome/m156) (the `JSON_Parse` table)
def_test!(JSON_Parse, |reporter| {
    let g_tests: &[(&str, Option<&str>)] = &[
        ("", None),
        ("[", None),
        ("]", None),
        ("[[]", None),
        ("[]]", None),
        ("[]f", None),
        ("{", None),
        ("}", None),
        ("{{}", None),
        ("{}}", None),
        ("{}f", None),
        ("{]", None),
        ("[}", None),
        ("{\"}", None),
        ("[\"]", None),
        ("1", None),
        ("true", None),
        ("false", None),
        ("null", None),
        ("[nulll]", None),
        ("[false2]", None),
        ("[true:]", None),
        ("[1 2]", None),
        ("[1,,2]", None),
        ("[1,2,]", None),
        ("[,1,2]", None),
        ("[ \"foo", None),
        ("[ \"fo\0o\" ]", None),
        ("{\"\":{}", None),
        ("{ null }", None),
        ("{ \"k\" : }", None),
        ("{ : null }", None),
        ("{ \"k\" : : null }", None),
        ("{ \"k\" : null , }", None),
        ("{ \"k\" : null \"k\" : 1 }", None),
        (r#"["\)"#, None),
        (r#"["\]"#, None),
        (r#"["\"]"#, None),
        (r#"["\z"]"#, None),
        (r#"["\u"]"#, None),
        (r#"["\u0"]"#, None),
        (r#"["\u00"]"#, None),
        (r#"["\u000"]"#, None),
        ("[]", Some("[]")),
        (" \n\r\t [ \n\r\t ] \n\r\t ", Some("[]")),
        ("[[]]", Some("[[]]")),
        ("[ null ]", Some("[null]")),
        ("[ true ]", Some("[true]")),
        ("[ false ]", Some("[false]")),
        ("[ 0 ]", Some("[0]")),
        ("[ 1 ]", Some("[1]")),
        ("[ 1.248 ]", Some("[1.248]")),
        ("[ \"\" ]", Some("[\"\"]")),
        ("[ \"foo{bar}baz\" ]", Some("[\"foo{bar}baz\"]")),
        ("[ \" f o o \" ]", Some("[\" f o o \"]")),
        ("[ \"123456\" ]", Some("[\"123456\"]")),
        ("[ \"1234567\" ]", Some("[\"1234567\"]")),
        ("[ \"12345678\" ]", Some("[\"12345678\"]")),
        ("[ \"123456789\" ]", Some("[\"123456789\"]")),
        (
            "[ null , true, false,0,12.8 ]",
            Some("[null,true,false,0,12.8]"),
        ),
        ("{}", Some("{}")),
        (" \n\r\t { \n\r\t } \n\r\t ", Some("{}")),
        ("{ \"k\" : null }", Some("{\"k\":null}")),
        ("{ \"foo{\" : \"bar}baz\" }", Some("{\"foo{\":\"bar}baz\"}")),
        (
            "{ \"k1\" : null, \"k2 \":0 }",
            Some("{\"k1\":null,\"k2 \":0}"),
        ),
        (
            "{ \"k1\" : null, \"k1\":0 }",
            Some("{\"k1\":null,\"k1\":0}"),
        ),
        // Rust's line continuation drops the leading blanks of each line; the blanks between
        // tokens do not change what the parser sees.
        (
            "{ \"k1\" : null,                   \n\
             \"k2\" : 0,                      \n\
             \"k3\" : [                       \n\
                        true,                 \r\n\
                        { \"kk1\" : \"foo\" , \n\
                          \"kk2\" : \"bar\" , \n\
                          \"kk3\" : 1.28 ,    \n\
                          \"kk4\" : [ 42 ]    \n\
                        } ,                   \n\
                        \"boo\" ,             \n\
                        null                  \n\
                      ]                       \n\
           }",
            Some(
                "{\"k1\":null,\"k2\":0,\"k3\":[true,\
                 {\"kk1\":\"foo\",\"kk2\":\"bar\",\"kk3\":1.28,\"kk4\":[42]},\"boo\",null]}",
            ),
        ),
        (r#"["\""]"#, Some("[\"\"\"]")),
        (r#"["\\"]"#, Some("[\"\\\"]")),
        (r#"["\/"]"#, Some("[\"/\"]")),
        (r#"["\b"]"#, Some("[\"\x08\"]")),
        (r#"["\f"]"#, Some("[\"\x0c\"]")),
        (r#"["\n"]"#, Some("[\"\n\"]")),
        (r#"["\r"]"#, Some("[\"\r\"]")),
        (r#"["\t"]"#, Some("[\"\t\"]")),
        (r#"["ሴ"]"#, Some("[\"\u{1234}\"]")),
        (r#"["foo\"bar"]"#, Some("[\"foo\"bar\"]")),
        (r#"["foo\\bar"]"#, Some("[\"foo\\bar\"]")),
        (r#"["foo\/bar"]"#, Some("[\"foo/bar\"]")),
        (r#"["foo\bbar"]"#, Some("[\"foo\x08bar\"]")),
        (r#"["foo\fbar"]"#, Some("[\"foo\x0cbar\"]")),
        (r#"["foo\nbar"]"#, Some("[\"foo\nbar\"]")),
        (r#"["foo\rbar"]"#, Some("[\"foo\rbar\"]")),
        (r#"["foo\tbar"]"#, Some("[\"foo\tbar\"]")),
        (r#"["fooሴbar"]"#, Some("[\"foo\u{1234}bar\"]")),
    ];
    for (input, out) in g_tests {
        let input = c_str_bytes(input.as_bytes());
        let dom = DOM::new(input);
        let success = !dom.root().is::<NullValue>();
        reporter_assert!(reporter, success == out.is_some());
        let Some(out) = out else { continue };
        let mut stream = DynamicMemoryWStream::new();
        dom.write(&mut stream);
        stream.write8(0);
        let data = stream.detach_as_data();
        reporter_assert!(reporter, c_str_bytes(data.as_bytes()) == out.as_bytes());
    }
});

fn check_primitive_bool(reporter: &mut Reporter, v: &Value, pv: bool, is_type: bool) {
    reporter_assert!(reporter, v.is::<BoolValue>() == is_type);
    let cast_t = v.get::<BoolValue>();
    reporter_assert!(reporter, cast_t.is_some() == is_type);
    if let Some(cast_t) = cast_t.filter(|_| is_type) {
        reporter_assert!(reporter, std::ptr::eq(v.as_type::<BoolValue>(), cast_t));
        reporter_assert!(reporter, v.as_type::<BoolValue>().value() == pv);
    }
}

#[allow(clippy::float_cmp)] // the C++ check compares the parsed double with ==
fn check_primitive_number(reporter: &mut Reporter, v: &Value, pv: f64, is_type: bool) {
    reporter_assert!(reporter, v.is::<NumberValue>() == is_type);
    let cast_t = v.get::<NumberValue>();
    reporter_assert!(reporter, cast_t.is_some() == is_type);
    if let Some(cast_t) = cast_t.filter(|_| is_type) {
        reporter_assert!(reporter, std::ptr::eq(v.as_type::<NumberValue>(), cast_t));
        reporter_assert!(reporter, v.as_type::<NumberValue>().value() == pv);
    }
}

fn check_vector<T: Facade + VectorLen>(
    reporter: &mut Reporter,
    v: &Value,
    expected_size: usize,
    is_vector: bool,
) {
    reporter_assert!(reporter, v.is::<T>() == is_vector);
    let cast_t = v.get::<T>();
    reporter_assert!(reporter, cast_t.is_some() == is_vector);
    if let Some(vec) = cast_t.filter(|_| is_vector) {
        reporter_assert!(reporter, std::ptr::eq(v.as_type::<T>(), vec));
        reporter_assert!(reporter, vec.len() == expected_size);
    }
}

/// The element count of a vector-like facade (`size()`).
trait VectorLen {
    fn len(&self) -> usize;
}

impl VectorLen for StringValue {
    fn len(&self) -> usize {
        self.size()
    }
}

impl VectorLen for ArrayValue {
    fn len(&self) -> usize {
        self.size()
    }
}

impl VectorLen for ObjectValue {
    fn len(&self) -> usize {
        self.size()
    }
}

fn check_string(reporter: &mut Reporter, v: &Value, s: Option<&str>) {
    check_vector::<StringValue>(reporter, v, s.map_or(0, str::len), s.is_some());
    if let Some(s) = s {
        reporter_assert!(reporter, v.as_type::<StringValue>().str() == Some(s));
        reporter_assert!(
            reporter,
            v.as_type::<StringValue>().as_bytes() == s.as_bytes()
        );
    }
}

// Port of: tests/JSONTest.cpp#L193-L340 (chrome/m156)
def_test!(JSON_DOM_visit, |reporter| {
    let json = "{     \n\
        \"k1\": null,                     \n\
        \"k2\": false,                    \n\
        \"k3\": true,                     \n\
        \"k4\": 42,                       \n\
        \"k5\": .75,                      \n\
        \"k6\": \"foo\",                  \n\
        \"k6b\": \"this string is long\", \n\
        \"k7\": [ 1, true, \"bar\" ],     \n\
        \"k8\": { \"kk1\": 2, \"kk2\": false, \"kk1\": \"baz\" } \n\
    }";
    let dom = DOM::new(json.as_bytes());
    let jroot = dom.root().as_type::<ObjectValue>();
    reporter_assert!(reporter, dom.root().is::<ObjectValue>());
    {
        let v = jroot.get("k1");
        reporter_assert!(reporter, v.is::<NullValue>());
        check_primitive_bool(reporter, v, false, false);
        check_primitive_number(reporter, v, 0.0, false);
        check_string(reporter, v, None);
        check_vector::<ArrayValue>(reporter, v, 0, false);
        check_vector::<ObjectValue>(reporter, v, 0, false);
    }
    {
        let v = jroot.get("k2");
        reporter_assert!(reporter, !v.is::<NullValue>());
        check_primitive_bool(reporter, v, false, true);
        check_primitive_number(reporter, v, 0.0, false);
        check_string(reporter, v, None);
        check_vector::<ArrayValue>(reporter, v, 0, false);
        check_vector::<ObjectValue>(reporter, v, 0, false);
    }
    {
        let v = jroot.get("k3");
        reporter_assert!(reporter, !v.is::<NullValue>());
        check_primitive_bool(reporter, v, true, true);
        check_primitive_number(reporter, v, 0.0, false);
        check_string(reporter, v, None);
        check_vector::<ArrayValue>(reporter, v, 0, false);
        check_vector::<ObjectValue>(reporter, v, 0, false);
    }
    {
        let v = jroot.get("k4");
        reporter_assert!(reporter, !v.is::<NullValue>());
        check_primitive_bool(reporter, v, false, false);
        check_primitive_number(reporter, v, 42.0, true);
        check_string(reporter, v, None);
        check_vector::<ArrayValue>(reporter, v, 0, false);
        check_vector::<ObjectValue>(reporter, v, 0, false);
    }
    {
        let v = jroot.get("k5");
        reporter_assert!(reporter, !v.is::<NullValue>());
        check_primitive_bool(reporter, v, false, false);
        check_primitive_number(reporter, v, f64::from(0.75_f32), true);
        check_string(reporter, v, None);
        check_vector::<ArrayValue>(reporter, v, 0, false);
        check_vector::<ObjectValue>(reporter, v, 0, false);
    }
    {
        let v = jroot.get("k6");
        reporter_assert!(reporter, !v.is::<NullValue>());
        check_primitive_bool(reporter, v, false, false);
        check_primitive_number(reporter, v, 0.0, false);
        check_string(reporter, v, Some("foo"));
        check_vector::<ArrayValue>(reporter, v, 0, false);
        check_vector::<ObjectValue>(reporter, v, 0, false);
    }
    {
        let v = jroot.get("k6b");
        reporter_assert!(reporter, !v.is::<NullValue>());
        check_primitive_bool(reporter, v, false, false);
        check_primitive_number(reporter, v, 0.0, false);
        check_string(reporter, v, Some("this string is long"));
        check_vector::<ArrayValue>(reporter, v, 0, false);
        check_vector::<ObjectValue>(reporter, v, 0, false);
    }
    {
        let v = jroot.get("k7");
        reporter_assert!(reporter, !v.is::<NullValue>());
        check_primitive_bool(reporter, v, false, false);
        check_primitive_number(reporter, v, 0.0, false);
        check_string(reporter, v, None);
        check_vector::<ObjectValue>(reporter, v, 0, false);
        check_vector::<ArrayValue>(reporter, v, 3, true);
        let items = v.as_type::<ArrayValue>();
        check_primitive_number(reporter, &items[0], 1.0, true);
        check_primitive_bool(reporter, &items[1], true, true);
        check_vector::<StringValue>(reporter, &items[2], 3, true);
    }
    {
        let v = jroot.get("k8");
        reporter_assert!(reporter, !v.is::<NullValue>());
        check_primitive_bool(reporter, v, false, false);
        check_primitive_number(reporter, v, 0.0, false);
        check_string(reporter, v, None);
        check_vector::<ArrayValue>(reporter, v, 0, false);
        check_vector::<ObjectValue>(reporter, v, 3, true);
        let object = v.as_type::<ObjectValue>();
        let m0 = &object.members()[0];
        check_string(reporter, &Value::String(m0.key.clone()), Some("kk1"));
        check_primitive_number(reporter, &m0.value, 2.0, true);
        let m1 = &object.members()[1];
        check_string(reporter, &Value::String(m1.key.clone()), Some("kk2"));
        check_primitive_bool(reporter, &m1.value, false, true);
        let m2 = &object.members()[2];
        check_string(reporter, &Value::String(m2.key.clone()), Some("kk1"));
        check_string(reporter, &m2.value, Some("baz"));
        reporter_assert!(reporter, object.get("").is::<NullValue>());
        reporter_assert!(reporter, object.get("nosuchkey").is::<NullValue>());
        check_string(reporter, object.get("kk1"), Some("baz"));
        check_primitive_bool(reporter, object.get("kk2"), false, true);
    }
});

fn check_value<T: Facade>(reporter: &mut Reporter, v: &Value, expected_string: &str) {
    reporter_assert!(reporter, v.is::<T>());
    let cast_t = v.get::<T>();
    reporter_assert!(
        reporter,
        cast_t.is_some_and(|c| std::ptr::eq(c, v.as_type::<T>()))
    );
    let vstr = v.to_bytes();
    reporter_assert!(reporter, vstr == expected_string.as_bytes());
}

// Port of: tests/JSONTest.cpp#L353-L436 (chrome/m156)
def_test!(JSON_DOM_build, |reporter| {
    let v0: Value = NullValue.into();
    check_value::<NullValue>(reporter, &v0, "null");
    let v1: Value = BoolValue::new(true).into();
    check_value::<BoolValue>(reporter, &v1, "true");
    let v2: Value = BoolValue::new(false).into();
    check_value::<BoolValue>(reporter, &v2, "false");
    let v3: Value = NumberValue::int(0).into();
    check_value::<NumberValue>(reporter, &v3, "0");
    let v4: Value = NumberValue::int(42).into();
    check_value::<NumberValue>(reporter, &v4, "42");
    let v5: Value = NumberValue::float(42.75).into();
    check_value::<NumberValue>(reporter, &v5, "42.75");
    let v6: Value = StringValue::new(b"").into();
    check_value::<StringValue>(reporter, &v6, "\"\"");
    let v7: Value = StringValue::new(b" foo ").into();
    check_value::<StringValue>(reporter, &v7, "\" foo \"");
    let v8: Value = StringValue::new(b" foo bar baz ").into();
    check_value::<StringValue>(reporter, &v8, "\" foo bar baz \"");
    let v9: Value = ArrayValue::new(Vec::new()).into();
    check_value::<ArrayValue>(reporter, &v9, "[]");
    let values0 = vec![v0.clone(), v3.clone(), v9.clone()];
    let v10: Value = ArrayValue::new(values0).into();
    check_value::<ArrayValue>(reporter, &v10, "[null,0,[]]");
    let v11: Value = ObjectValue::new(Vec::new()).into();
    check_value::<ObjectValue>(reporter, &v11, "{}");
    let members0 = vec![
        Member {
            key: StringValue::new(b"key_0"),
            value: v1.clone(),
        },
        Member {
            key: StringValue::new(b"key_1"),
            value: v4.clone(),
        },
        Member {
            key: StringValue::new(b"key_2"),
            value: v11.clone(),
        },
    ];
    let v12: Value = ObjectValue::new(members0).into();
    check_value::<ObjectValue>(reporter, &v12, "{\"key_0\":true,\"key_1\":42,\"key_2\":{}}");
    let values1 = vec![v2.clone(), v6.clone(), v12.clone()];
    let v13: Value = ArrayValue::new(values1).into();
    check_value::<ArrayValue>(
        reporter,
        &v13,
        "[false,\"\",{\"key_0\":true,\"key_1\":42,\"key_2\":{}}]",
    );
    let members1 = vec![
        Member {
            key: StringValue::new(b"key_00"),
            value: v5.clone(),
        },
        Member {
            key: StringValue::new(b"key_01"),
            value: v7.clone(),
        },
        Member {
            key: StringValue::new(b"key_02"),
            value: v13.clone(),
        },
    ];
    let v14: Value = ObjectValue::new(members1).into();
    check_value::<ObjectValue>(
        reporter,
        &v14,
        "{\"key_00\":42.75,\"key_01\":\" foo \",\"key_02\":[false,\"\",{\"key_0\":true,\"key_1\":42,\"key_2\":{}}]}",
    );
});

// Port of: tests/JSONTest.cpp#L438-L498 (chrome/m156)
def_test!(JSON_ParseNumber, |reporter| {
    // (string, value, tolerance)
    let g_tests: &[(&str, scalar, scalar)] = &[
        ("0", 0.0, 0.0),
        ("1", 1.0, 0.0),
        ("00000000", 0.0, 0.0),
        ("00000001", 1.0, 0.0),
        ("0.001", 0.001, 0.0),
        ("1.001", 1.001, 0.0),
        ("0.000001", 0.000_001, 0.0),
        ("1.000001", 1.000_001, 0.0),
        ("1000.000001", 1_000.000_001, 0.0),
        ("0.0000000001", 0.000_000_000_1, 0.0),
        ("1.0000000001", 1.000_000_000_1, 0.0),
        ("1000.0000000001", 1_000.000_000_000_1, 0.0),
        (
            "20.001111814444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444473",
            20.001,
            0.001,
        ),
        ("1e0", 1.0, 0.0),
        ("1e1", 10.0, 0.0),
        ("1e+1", 10.0, 0.0),
        ("1e-1", 0.1, 0.0),
        ("-1e1", -10.0, 0.0),
        ("-1e-1", -0.1, 0.0),
        ("3.14e2", 314.0, 0.0),
        ("3.14e-2", 0.0314, 0.0),
        ("1.23E4", 12_300.0, 0.0),
        ("1.23E-4", 0.000_123, 0.0),
        ("0e0", 0.0, 0.0),
        ("5E+0", 5.0, 0.0),
        ("1.0e+2", 100.0, 0.0),
        ("2.5e-3", 0.0025, 0.0),
        ("9.99e10", 9.99e10, 0.0),
        ("-7.5E-5", -7.5e-5, 0.0),
    ];
    // skia-rust: not expressible in Rust: `SkAutoLocaleSetter("de_DE.UTF-8")`. Number parsing
    // here never reads the locale, so the test runs in the same conditions without it.
    for (string, value, tolerance) in g_tests {
        let json = format!("{{ \"key\": {string} }}");
        let dom = DOM::new(json.as_bytes());
        let jroot = dom.root().get::<ObjectValue>();
        reporter_assert!(reporter, jroot.is_some());
        let jnumber = jroot.map(|root| root.get("key"));
        reporter_assert!(reporter, jnumber.is_some());
        let jnumber = jnumber.and_then(|v| v.get::<NumberValue>());
        reporter_assert!(reporter, jnumber.is_some());
        if let Some(jnumber) = jnumber {
            // `SkScalarNearlyEqual(**jnumber, ...)` takes the double as an SkScalar.
            let number = f64_as_scalar(jnumber.value());
            reporter_assert!(
                reporter,
                <scalar as Scalar>::nearly_equal(number, *value, *tolerance)
            );
        }
    }
});

/// The `double` to `SkScalar` conversion of the call to `SkScalarNearlyEqual`.
#[allow(clippy::cast_possible_truncation)] // mirrors the implicit double -> float (SkScalar)
fn f64_as_scalar(value: f64) -> scalar {
    value as scalar
}

// Port of: tests/JSONTest.cpp#L500-L513 (chrome/m156)
def_test!(JSON_Lookup, |r| {
    let json = r#"{"foo": { "bar": { "baz": 100 }}}"#;
    let dom = DOM::new(json.as_bytes());
    let root = dom.root();
    reporter_assert!(r, root.is::<ObjectValue>());
    reporter_assert!(r, root["foo"].is::<ObjectValue>());
    reporter_assert!(r, root["foo"]["bar"].is::<ObjectValue>());
    reporter_assert!(r, root["foo"]["bar"]["baz"].is::<NumberValue>());
    reporter_assert!(r, root["foozz"].is::<NullValue>());
    reporter_assert!(r, root["foozz"]["barzz"].is::<NullValue>());
    reporter_assert!(r, root["foozz"]["barzz"]["bazzz"].is::<NullValue>());
});

// Port of: tests/JSONTest.cpp#L515-L560 (chrome/m156)
// the C++ test compares the parsed values with ==
def_test!(
    #[allow(clippy::float_cmp)]
    JSON_Writable,
    |r| {
        let json = r#"{"null": null, "num": 100}"#;
        let mut dom = DOM::new(json.as_bytes());
        reporter_assert!(r, dom.root().is::<ObjectValue>());
        {
            let root = dom.root_mut().as_type_mut::<ObjectValue>();
            reporter_assert!(r, root.get("null").is::<NullValue>());
            let w1 = root.writable("null");
            reporter_assert!(r, w1.is::<NullValue>());
            *w1 = NumberValue::int(42).into();
            reporter_assert!(r, root.get("null").is::<NumberValue>());
            reporter_assert!(r, root.get("null").as_type::<NumberValue>().value() == 42.0);
            reporter_assert!(r, root.get("num").is::<NumberValue>());
            let w2 = root.writable("num");
            reporter_assert!(r, w2.is::<NumberValue>());
            *w2 = StringValue::new(b"foo").into();
            reporter_assert!(r, root.get("num").is::<StringValue>());
            reporter_assert!(
                r,
                root.get("num").as_type::<StringValue>().str() == Some("foo")
            );
            // new/insert semantics
            reporter_assert!(r, root.get("new").is::<NullValue>());
            reporter_assert!(r, root.size() == 2);
            let w3 = root.writable("new");
            reporter_assert!(r, w3.is::<NullValue>());
            *w3 = BoolValue::new(true).into();
            reporter_assert!(r, root.size() == 3);
            reporter_assert!(r, root.get("new").is::<BoolValue>());
            reporter_assert!(r, root.get("new").as_type::<BoolValue>().value());
            *root.writable("newobj") = ObjectValue::new(Vec::new()).into();
            reporter_assert!(r, root.size() == 4);
            reporter_assert!(r, root.get("newobj").is::<ObjectValue>());
            let newobj = root.writable("newobj").as_type_mut::<ObjectValue>();
            reporter_assert!(r, newobj.size() == 0);
            *newobj.writable("newprop") = NumberValue::int(-1).into();
            reporter_assert!(r, newobj.size() == 1);
        }
        let root = dom.root();
        reporter_assert!(r, root["newobj"]["newprop"].is::<NumberValue>());
        reporter_assert!(
            r,
            root["newobj"]["newprop"].as_type::<NumberValue>().value() == -1.0
        );
        reporter_assert!(
            r,
            root.to_bytes()
                == br#"{"null":42,"num":"foo","new":true,"newobj":{"newprop":-1}}"#.to_vec()
        );
    }
);
