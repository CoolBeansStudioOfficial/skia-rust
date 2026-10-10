// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/jsonreader/SkJSONReader.h, modules/jsonreader/SkJSONReader.cpp
// (chrome/m156). Skia's namespace is `skjson`; this module is `skottie::json`.
//
// Values are a Rust enum of facade types instead of Skia's 8-byte tagged records in an arena.
// The parser, the number conversions and the writer are the same algorithms, with the same
// arithmetic. Two representation differences are visible to callers:
//
//   - Strings are byte vectors (`StringValue::as_bytes`), not NUL-terminated arena slices.
//   - `DOM::root` is `Value::Null` when parsing fails, as in Skia.
//
// Skia's error messages are compiled out (`SK_JSON_REPORT_ERRORS` is not defined), so failures
// only produce the null root.

use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_core::string::str_append_scalar;
use skia_rust_core::utf::to_utf8;
use skia_rust_core::utils::parse::find_hex;

/// The kind of a [`Value`].
// Port of: modules/jsonreader/SkJSONReader.h#L59-L68 (chrome/m156)
#[doc(alias = "skjson::Value::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Null,
    Bool,
    Number,
    String,
    Array,
    Object,
}

/// A parsed JSON value. Each variant holds the facade type for its kind.
// Port of: modules/jsonreader/SkJSONReader.h#L57-L218 (chrome/m156)
#[doc(alias = "skjson::Value")]
#[derive(Debug, Clone)]
pub enum Value {
    Null(NullValue),
    Bool(BoolValue),
    Number(NumberValue),
    String(StringValue),
    Array(ArrayValue),
    Object(ObjectValue),
}

/// The facade types: a `Value` variant viewed as one concrete kind.
// Port of: the `T::kType` / `Value::as<T>()` facade pattern (modules/jsonreader/SkJSONReader.h#L77-L99)
pub trait Facade: Sized {
    /// The kind this facade represents.
    const KIND: Type;
    /// The facade view of `value`, if it is this kind.
    fn cast(value: &Value) -> Option<&Self>;
    /// The mutable facade view of `value`, if it is this kind.
    fn cast_mut(value: &mut Value) -> Option<&mut Self>;
}

static NULL_VALUE: Value = Value::Null(NullValue);

impl Value {
    /// The kind of this value.
    // Port of: modules/jsonreader/SkJSONReader.h#L365-L379 (chrome/m156) (`Value::getType`)
    #[doc(alias = "getType")]
    #[must_use]
    pub fn kind(&self) -> Type {
        match self {
            Self::Null(_) => Type::Null,
            Self::Bool(_) => Type::Bool,
            Self::Number(_) => Type::Number,
            Self::String(_) => Type::String,
            Self::Array(_) => Type::Array,
            Self::Object(_) => Type::Object,
        }
    }

    /// True if this value is of facade type `T`.
    // Port of: modules/jsonreader/SkJSONReader.h#L77 (chrome/m156) (`Value::is`)
    #[must_use]
    pub fn is<T: Facade>(&self) -> bool {
        T::cast(self).is_some()
    }

    /// The value viewed as facade type `T`, or `None` if it is another kind.
    // Port of: modules/jsonreader/SkJSONReader.h#L95-L99 (chrome/m156) (`operator const T*`)
    #[must_use]
    pub fn get<T: Facade>(&self) -> Option<&T> {
        T::cast(self)
    }

    /// The mutable view of this value as facade type `T`, or `None` if it is another kind.
    #[must_use]
    pub fn get_mut<T: Facade>(&mut self) -> Option<&mut T> {
        T::cast_mut(self)
    }

    /// The value viewed as facade type `T`.
    ///
    /// # Panics
    /// If the value is not of kind `T`. Skia asserts this (`SkASSERT(this->is<T>())`).
    // Port of: modules/jsonreader/SkJSONReader.h#L84-L90 (chrome/m156) (`Value::as`)
    #[doc(alias = "as")]
    #[must_use]
    pub fn as_type<T: Facade>(&self) -> &T {
        T::cast(self).expect("skjson::Value::as: value is not of the requested type")
    }

    /// The mutable view of this value as facade type `T`.
    ///
    /// # Panics
    /// If the value is not of kind `T`.
    #[must_use]
    pub fn as_type_mut<T: Facade>(&mut self) -> &mut T {
        T::cast_mut(self).expect("skjson::Value::as: value is not of the requested type")
    }

    /// The JSON text of this value, as `Value::toString` writes it.
    // Port of: modules/jsonreader/SkJSONReader.cpp#L978-L984 (chrome/m156) (`Value::toString`)
    #[doc(alias = "toString")]
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut stream = DynamicMemoryWStream::new();
        write_value(self, &mut stream);
        stream.detach_as_data().as_bytes().to_vec()
    }
}

/// Fluent key lookup, `v["foo"]["bar"]`: the member's value, or null if `self` is not an object
/// or has no such key.
// Port of: modules/jsonreader/SkJSONReader.h#L381-L383 (chrome/m156) (`Value::operator[]`)
impl std::ops::Index<&str> for Value {
    type Output = Value;

    fn index(&self, key: &str) -> &Value {
        match self {
            Self::Object(object) => object.get(key),
            _ => &NULL_VALUE,
        }
    }
}

macro_rules! facade_from {
    ($ty:ident, $variant:ident) => {
        impl From<$ty> for Value {
            fn from(value: $ty) -> Self {
                Self::$variant(value)
            }
        }

        impl Facade for $ty {
            const KIND: Type = Type::$variant;

            fn cast(value: &Value) -> Option<&Self> {
                match value {
                    Value::$variant(facade) => Some(facade),
                    _ => None,
                }
            }

            fn cast_mut(value: &mut Value) -> Option<&mut Self> {
                match value {
                    Value::$variant(facade) => Some(facade),
                    _ => None,
                }
            }
        }
    };
}

// The variant names of `Type` mirror `Value`'s; `Type::Null` and friends are used above.
facade_from!(NullValue, Null);
facade_from!(BoolValue, Bool);
facade_from!(NumberValue, Number);
facade_from!(StringValue, String);
facade_from!(ArrayValue, Array);
facade_from!(ObjectValue, Object);

/// The `null` literal.
// Port of: modules/jsonreader/SkJSONReader.h#L220-L225 (chrome/m156)
#[doc(alias = "skjson::NullValue")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NullValue;

/// A `true` or `false` literal.
// Port of: modules/jsonreader/SkJSONReader.h#L227-L237 (chrome/m156)
#[doc(alias = "skjson::BoolValue")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoolValue(bool);

impl BoolValue {
    /// Port of `BoolValue(bool)`.
    #[must_use]
    pub const fn new(value: bool) -> Self {
        Self(value)
    }

    /// The boolean (`*v` in Skia).
    #[must_use]
    pub const fn value(&self) -> bool {
        self.0
    }
}

/// A number: Skia stores it as an `int32_t` when the literal is integral and fits, and as a
/// `float` otherwise. [`NumberValue::value`] widens either to `f64`, as `operator*` does.
// Port of: modules/jsonreader/SkJSONReader.h#L239-L258 (chrome/m156)
#[doc(alias = "skjson::NumberValue")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumberValue(Repr);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Repr {
    Int(i32),
    Float(f32),
}

impl NumberValue {
    /// Port of `NumberValue(int32_t)`.
    #[must_use]
    pub const fn int(value: i32) -> Self {
        Self(Repr::Int(value))
    }

    /// Port of `NumberValue(float)`.
    #[must_use]
    pub const fn float(value: f32) -> Self {
        Self(Repr::Float(value))
    }

    /// The number as a `double` (`operator*`).
    #[must_use]
    pub fn value(&self) -> f64 {
        match self.0 {
            Repr::Int(i) => f64::from(i),
            Repr::Float(f) => f64::from(f),
        }
    }
}

/// A string. Its bytes are the decoded contents (escapes already processed).
// Port of: modules/jsonreader/SkJSONReader.h#L290-L326 (chrome/m156)
#[doc(alias = "skjson::StringValue")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringValue(Vec<u8>);

impl StringValue {
    /// Port of `StringValue(const char*, size_t, SkArenaAlloc&)`.
    #[must_use]
    pub fn new(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }

    /// The raw bytes of the string.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The length in bytes.
    #[must_use]
    pub fn size(&self) -> usize {
        self.0.len()
    }

    /// The string as UTF-8 text, or `None` if the bytes are not valid UTF-8.
    // Port of: modules/jsonreader/SkJSONReader.h#L190-L192 (chrome/m156) (`StringValue::str`)
    #[must_use]
    pub fn str(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }
}

/// A member of an object: a key and its value.
// Port of: modules/jsonreader/SkJSONReader.h#L328-L332 (chrome/m156) (`Member`)
#[doc(alias = "skjson::Member")]
#[derive(Debug, Clone)]
pub struct Member {
    pub key: StringValue,
    pub value: Value,
}

/// An array of values.
// Port of: modules/jsonreader/SkJSONReader.h#L285-L289 (chrome/m156) (`ArrayValue`)
#[doc(alias = "skjson::ArrayValue")]
#[derive(Debug, Clone, Default)]
pub struct ArrayValue(Vec<Value>);

impl ArrayValue {
    /// Port of `ArrayValue(const Value*, size_t, SkArenaAlloc&)`.
    #[must_use]
    pub fn new(items: Vec<Value>) -> Self {
        Self(items)
    }

    /// The number of elements (`size`).
    #[must_use]
    pub fn size(&self) -> usize {
        self.0.len()
    }

    /// The elements, in order (`begin`/`end`).
    #[must_use]
    pub fn items(&self) -> &[Value] {
        &self.0
    }
}

impl std::ops::Index<usize> for ArrayValue {
    type Output = Value;

    // Port of: modules/jsonreader/SkJSONReader.h#L255-L282 (chrome/m156) (`VectorValue::operator[]`)
    fn index(&self, i: usize) -> &Value {
        &self.0[i]
    }
}

/// An object: its members in document order. Duplicate keys are kept; lookups return the last.
// Port of: modules/jsonreader/SkJSONReader.h#L333-L350 (chrome/m156)
#[doc(alias = "skjson::ObjectValue")]
#[derive(Debug, Clone, Default)]
pub struct ObjectValue(Vec<Member>);

impl ObjectValue {
    /// Port of `ObjectValue(const Member*, size_t, SkArenaAlloc&)`.
    #[must_use]
    pub fn new(members: Vec<Member>) -> Self {
        Self(members)
    }

    /// The number of members (`size`).
    #[must_use]
    pub fn size(&self) -> usize {
        self.0.len()
    }

    /// The members, in document order (`begin`/`end`).
    #[must_use]
    pub fn members(&self) -> &[Member] {
        &self.0
    }

    /// The JSON text of this object, as `Value::toString` writes it.
    // Port of: modules/jsonreader/SkJSONReader.cpp#L978-L984 (chrome/m156) (`Value::toString`)
    #[doc(alias = "toString")]
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut stream = DynamicMemoryWStream::new();
        write_pending(PendingRoot::Object(self), &mut stream);
        stream.detach_as_data().as_bytes().to_vec()
    }

    /// The value for `key`, or null if there is none (`operator[]`).
    // Port of: modules/jsonreader/SkJSONReader.h#L337-L340 (chrome/m156)
    #[doc(alias = "operator[]")]
    #[must_use]
    pub fn get(&self, key: &str) -> &Value {
        self.find(key).map_or(&NULL_VALUE, |member| &member.value)
    }

    /// The member for `key`. A duplicate key resolves to the last member (`find`).
    // Port of: modules/jsonreader/SkJSONReader.cpp#L230-L243 (chrome/m156) (`ObjectValue::find`)
    fn find(&self, key: &str) -> Option<&Member> {
        self.0
            .iter()
            .rev()
            .find(|member| member.key.as_bytes() == key.as_bytes())
    }

    /// The value for `key`, inserting a null member at the end if there is none.
    // Port of: modules/jsonreader/SkJSONReader.cpp#L245-L260 (chrome/m156) (`ObjectValue::writable`)
    pub fn writable(&mut self, key: &str) -> &mut Value {
        if let Some(index) = self
            .0
            .iter()
            .rposition(|m| m.key.as_bytes() == key.as_bytes())
        {
            return &mut self.0[index].value;
        }
        self.0.push(Member {
            key: StringValue::new(key.as_bytes()),
            value: NullValue.into(),
        });
        let last = self.0.len() - 1;
        &mut self.0[last].value
    }
}

/// A parsed document.
// Port of: modules/jsonreader/SkJSONReader.h#L352-L359 (chrome/m156)
#[doc(alias = "skjson::DOM")]
#[derive(Debug, Clone)]
pub struct DOM {
    root: Value,
}

impl DOM {
    /// Parses `data`. On failure the root is `Value::Null`.
    // Port of: modules/jsonreader/SkJSONReader.cpp#L988-L992 (chrome/m156) (`DOM::DOM`)
    #[doc(alias = "DOM")]
    #[must_use]
    pub fn new(data: &[u8]) -> Self {
        Self {
            root: Parser::parse(data),
        }
    }

    /// The root value.
    #[must_use]
    pub fn root(&self) -> &Value {
        &self.root
    }

    /// The root value, for in-place edits.
    #[must_use]
    pub fn root_mut(&mut self) -> &mut Value {
        &mut self.root
    }

    /// Writes the document as compact JSON (`DOM::write`).
    // Port of: modules/jsonreader/SkJSONReader.cpp#L990 (chrome/m156) (`DOM::write`)
    pub fn write(&self, stream: &mut dyn WStream) {
        write_value(&self.root, stream);
    }
}

// Port of: modules/jsonreader/SkJSONReader.cpp#L276-L292 (chrome/m156) (`g_token_flags`)
// bit 0 (0x01) - plain ASCII string character
// bit 1 (0x02) - whitespace
// bit 2 (0x04) - string terminator (" \ \0 [control chars] and } ])
// bit 3 (0x08) - 0-9
// bit 4 (0x10) - 0-9 e E .
// bit 5 (0x20) - scope terminator (} ])
#[rustfmt::skip]
const TOKEN_FLAGS: [u8; 256] = [
 // 0    1    2    3    4    5    6    7      8    9    A    B    C    D    E    F
    4,   4,   4,   4,   4,   4,   4,   4,     4,   6,   6,   4,   4,   6,   4,   4, // 0
    4,   4,   4,   4,   4,   4,   4,   4,     4,   4,   4,   4,   4,   4,   4,   4, // 1
    3,   1,   4,   1,   1,   1,   1,   1,     1,   1,   1,   1,   1,   1, 0x11,   1, // 2
 0x19,0x19,0x19,0x19,0x19,0x19,0x19,0x19,  0x19,0x19,   1,   1,   1,   1,   1,   1, // 3
    1,   1,   1,   1,   1, 0x11,   1,   1,     1,   1,   1,   1,   1,   1,   1,   1, // 4
    1,   1,   1,   1,   1,   1,   1,   1,     1,   1,   1,   1,   4, 0x25,   1,   1, // 5
    1,   1,   1,   1,   1, 0x11,   1,   1,     1,   1,   1,   1,   1,   1,   1,   1, // 6
    1,   1,   1,   1,   1,   1,   1,   1,     1,   1,   1,   1,   1, 0x25,   1,   1, // 7
 // 128-255
    0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,
];

fn token_flags(c: u8) -> u8 {
    TOKEN_FLAGS[usize::from(c)]
}

fn is_ws(c: u8) -> bool {
    token_flags(c) & 0x02 != 0
}

fn is_eostring(c: u8) -> bool {
    token_flags(c) & 0x04 != 0
}

fn is_digit(c: u8) -> bool {
    token_flags(c) & 0x08 != 0
}

fn is_numeric(c: u8) -> bool {
    token_flags(c) & 0x10 != 0
}

fn is_eoscope(c: u8) -> bool {
    token_flags(c) & 0x20 != 0
}

// The largest absolute int32 value the fast path can accumulate before risking overflow *on the
// next digit* (214748363).
// Port of: modules/jsonreader/SkJSONReader.cpp#L813-L814 (chrome/m156)
const K_MAX_INT32: i32 = (i32::MAX - 9) / 10;

/// The C++ `int` to `float` conversion of the fast path (`sign * f`, `n32 * pow10`). Like Skia's,
/// it rounds to nearest, and the inputs are at most 9 digits so it is exact.
#[allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float conversion of the C++
fn int_as_f32(value: i32) -> f32 {
    value as f32
}

/// The C++ `double` to `float` conversion of `writeScalarAsText(*number)`.
#[allow(clippy::cast_possible_truncation)] // mirrors the implicit double -> float (SkScalar)
fn f64_as_f32(value: f64) -> f32 {
    value as f32
}

/// `10^exp` for `exp <= 0`, from the float table; `10^exp` by `powf` below the table.
// Port of: modules/jsonreader/SkJSONReader.cpp#L305-L325 (chrome/m156) (`pow10`)
fn pow10(exp: i32) -> f32 {
    // Port of: modules/jsonreader/SkJSONReader.cpp#L306-L316 (chrome/m156) (`g_pow10_table`)
    const G_POW10_TABLE: [f32; 63] = [
        1e-31, 1e-30, 1e-29, 1e-28, 1e-27, 1e-26, 1e-25, 1e-24, 1e-23, 1e-22, 1e-21, 1e-20, 1e-19,
        1e-18, 1e-17, 1e-16, 1e-15, 1e-14, 1e-13, 1e-12, 1e-11, 1e-10, 1e-9, 1e-8, 1e-7, 1e-6,
        1e-5, 1e-4, 1e-3, 1e-2, 1e-1, 1e+0, 1e+1, 1e+2, 1e+3, 1e+4, 1e+5, 1e+6, 1e+7, 1e+8, 1e+9,
        1e+10, 1e+11, 1e+12, 1e+13, 1e+14, 1e+15, 1e+16, 1e+17, 1e+18, 1e+19, 1e+20, 1e+21, 1e+22,
        1e+23, 1e+24, 1e+25, 1e+26, 1e+27, 1e+28, 1e+29, 1e+30, 1e+31,
    ];
    const K_EXP_OFFSET: i32 = 31;
    debug_assert!(exp <= 0);
    if exp >= -K_EXP_OFFSET {
        G_POW10_TABLE[usize::try_from(exp + K_EXP_OFFSET).unwrap_or(0)]
    } else {
        // skia-rust: libm (`std::pow(10.0f, float)`, PORTING §5.7)
        10.0_f32.powf(int_as_f32(exp))
    }
}

/// `std::from_chars(p, end, float)` in the general format, on the bytes `s` (the text from the
/// token to `end`). Returns the number of bytes consumed and the value.
///
/// It accepts an optional `-`, `inf`/`infinity` and `nan`/`nan(...)` (case-insensitive), and a
/// decimal `digits[.digits][e[+-]digits]` where the exponent is consumed only if it has digits.
/// Out-of-range values fail as `result_out_of_range` does in libstdc++: overflow to infinity, and
/// underflow of nonzero digits to zero.
// Port of: the `std::from_chars` fallback of modules/jsonreader/SkJSONReader.cpp#L871-L891 (chrome/m156)
fn from_chars_f32(s: &[u8]) -> Option<(usize, f32)> {
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let matches_ci = |i: usize, word: &[u8]| {
        word.iter()
            .enumerate()
            .all(|(k, w)| at(i + k).eq_ignore_ascii_case(w))
    };

    let neg = at(0) == b'-';
    let start = usize::from(neg);

    if matches_ci(start, b"inf") {
        let consumed = if matches_ci(start, b"infinity") {
            start + 8
        } else {
            start + 3
        };
        return Some((
            consumed,
            if neg {
                f32::NEG_INFINITY
            } else {
                f32::INFINITY
            },
        ));
    }
    if matches_ci(start, b"nan") {
        let mut consumed = start + 3;
        if at(consumed) == b'(' {
            let mut j = consumed + 1;
            while at(j).is_ascii_alphanumeric() || at(j) == b'_' {
                j += 1;
            }
            if at(j) == b')' {
                consumed = j + 1;
            }
        }
        let nan = if neg { -f32::NAN } else { f32::NAN };
        return Some((consumed, nan));
    }

    let int_start = start;
    let mut i = start;
    while is_digit(at(i)) {
        i += 1;
    }
    let mut nonzero = s[int_start..i].iter().any(|&c| c != b'0');
    let int_digits = i - int_start;
    let mut frac_digits = 0;
    if at(i) == b'.' {
        let mut j = i + 1;
        while is_digit(at(j)) {
            nonzero |= at(j) != b'0';
            j += 1;
            frac_digits += 1;
        }
        i = j;
    }
    if int_digits + frac_digits == 0 {
        return None;
    }
    if at(i) == b'e' || at(i) == b'E' {
        let mut j = i + 1;
        if at(j) == b'+' || at(j) == b'-' {
            j += 1;
        }
        if is_digit(at(j)) {
            while is_digit(at(j)) {
                j += 1;
            }
            i = j;
        }
    }

    let text = std::str::from_utf8(&s[..i]).ok()?;
    let value: f32 = text.parse().ok()?;
    if value.is_infinite() || (value == 0.0 && nonzero) {
        return None;
    }
    Some((i, value))
}

/// The parser's open scopes. Skia keeps these on one value stack with placeholders; the scope
/// stack here holds the same information.
enum Scope {
    Object {
        members: Vec<Member>,
        key: Option<StringValue>,
    },
    Array {
        items: Vec<Value>,
    },
}

/// The control states of Skia's `goto` parser (`DOMParser::parse`).
#[derive(Clone, Copy)]
enum State {
    MatchObject,
    MatchObjectKey,
    MatchValue,
    MatchPostValue,
    MatchArray,
    PopObject,
    PopArray,
    PopCommon,
}

// Port of: modules/jsonreader/SkJSONReader.cpp#L327-L332 (chrome/m156) (`DOMParser`)
// Lexer/parser inspired by rapidjson, sajson and pjson.
struct Parser<'a> {
    data: &'a [u8],
    scopes: Vec<Scope>,
    root: Option<Value>,
}

impl<'a> Parser<'a> {
    /// Bytes past the end read as NUL, which is how Skia's inputs (C strings) end.
    fn at(&self, i: usize) -> u8 {
        self.data.get(i).copied().unwrap_or(0)
    }

    fn skip_ws(&self, mut p: usize) -> usize {
        while is_ws(self.at(p)) {
            p += 1;
        }
        p
    }

    /// Port of `DOMParser::parse`.
    // Port of: modules/jsonreader/SkJSONReader.cpp#L334-L498 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one state per `goto` label of the C++ parser, kept together
    fn parse(data: &'a [u8]) -> Value {
        if data.is_empty() {
            return NullValue.into();
        }
        let mut parser = Parser {
            data,
            scopes: Vec::new(),
            root: None,
        };
        // We're only checking for end-of-stream on object/array close, so trim the tail.
        let mut stop = data.len() - 1;
        while stop > 0 && is_ws(parser.at(stop)) {
            stop -= 1;
        }
        let p_stop = stop;
        if !is_eoscope(parser.at(p_stop)) {
            return NullValue.into();
        }
        let mut p = parser.skip_ws(0);
        let mut state = match parser.at(p) {
            b'{' => State::MatchObject,
            b'[' => State::MatchArray,
            _ => return NullValue.into(),
        };
        loop {
            state = match state {
                State::MatchObject => {
                    p = parser.skip_ws(p + 1);
                    parser.scopes.push(Scope::Object {
                        members: Vec::new(),
                        key: None,
                    });
                    if parser.at(p) == b'}' {
                        State::PopObject
                    } else {
                        State::MatchObjectKey
                    }
                }
                State::MatchObjectKey => {
                    p = parser.skip_ws(p);
                    if parser.at(p) != b'"' {
                        return NullValue.into();
                    }
                    let Some((next, key)) = parser.match_string(p, p_stop) else {
                        return NullValue.into();
                    };
                    p = next;
                    if let Some(Scope::Object { key: pending, .. }) = parser.scopes.last_mut() {
                        *pending = Some(StringValue(key));
                    }
                    p = parser.skip_ws(p);
                    if parser.at(p) != b':' {
                        return NullValue.into();
                    }
                    p += 1;
                    State::MatchValue
                }
                State::MatchValue => {
                    p = parser.skip_ws(p);
                    match parser.at(p) {
                        0 => return NullValue.into(),
                        // The openers continue in their own states, without a post-value check.
                        b'[' => State::MatchArray,
                        b'{' => State::MatchObject,
                        c => {
                            match c {
                                b'"' => {
                                    let Some((next, s)) = parser.match_string(p, p_stop) else {
                                        return NullValue.into();
                                    };
                                    p = next;
                                    parser.push_value(StringValue(s).into());
                                }
                                b'f' | b'n' | b't' => {
                                    let (literal, value): (&[u8], Value) = match c {
                                        b'f' => (b"false", BoolValue::new(false).into()),
                                        b'n' => (b"null", NullValue.into()),
                                        _ => (b"true", BoolValue::new(true).into()),
                                    };
                                    let Some(next) = parser.match_literal(p, literal) else {
                                        return NullValue.into();
                                    };
                                    p = next;
                                    parser.push_value(value);
                                }
                                _ => {
                                    let Some((next, number)) = parser.match_number(p, p_stop)
                                    else {
                                        return NullValue.into();
                                    };
                                    p = next;
                                    parser.push_value(number.into());
                                }
                            }
                            State::MatchPostValue
                        }
                    }
                }
                State::MatchPostValue => {
                    p = parser.skip_ws(p);
                    match parser.at(p) {
                        b',' => {
                            p += 1;
                            if matches!(parser.scopes.last(), Some(Scope::Object { .. })) {
                                State::MatchObjectKey
                            } else {
                                State::MatchValue
                            }
                        }
                        b']' => State::PopArray,
                        b'}' => State::PopObject,
                        _ => return NullValue.into(),
                    }
                }
                State::MatchArray => {
                    p = parser.skip_ws(p + 1);
                    parser.scopes.push(Scope::Array { items: Vec::new() });
                    if parser.at(p) == b']' {
                        State::PopArray
                    } else {
                        State::MatchValue
                    }
                }
                State::PopObject => {
                    if matches!(parser.scopes.last(), Some(Scope::Array { .. })) {
                        return NullValue.into();
                    }
                    if let Some(Scope::Object { members, .. }) = parser.scopes.pop() {
                        parser.push_value(ObjectValue::new(members).into());
                    }
                    State::PopCommon
                }
                State::PopArray => {
                    if matches!(parser.scopes.last(), Some(Scope::Object { .. })) {
                        return NullValue.into();
                    }
                    if let Some(Scope::Array { items }) = parser.scopes.pop() {
                        parser.push_value(ArrayValue::new(items).into());
                    }
                    State::PopCommon
                }
                State::PopCommon => {
                    if parser.scopes.is_empty() {
                        // Success condition: parsed the top-level element, reached the stop token.
                        return if p == p_stop {
                            parser.root.unwrap_or_else(|| NullValue.into())
                        } else {
                            NullValue.into()
                        };
                    }
                    if p == p_stop {
                        return NullValue.into();
                    }
                    p += 1;
                    State::MatchPostValue
                }
            };
        }
    }

    /// Adds a completed value to the innermost scope, or makes it the root.
    fn push_value(&mut self, value: Value) {
        match self.scopes.last_mut() {
            Some(Scope::Object { members, key }) => {
                let key = key.take().unwrap_or_else(|| StringValue::new(b""));
                members.push(Member { key, value });
            }
            Some(Scope::Array { items }) => items.push(value),
            None => self.root = Some(value),
        }
    }

    /// `true`/`false`/`null`: the literal must match exactly.
    // Port of: modules/jsonreader/SkJSONReader.cpp#L638-L669 (chrome/m156) (`matchTrue`, `matchFalse`, `matchNull`)
    fn match_literal(&self, p: usize, literal: &[u8]) -> Option<usize> {
        literal
            .iter()
            .enumerate()
            .all(|(k, &c)| self.at(p + k) == c)
            .then_some(p + literal.len())
    }

    /// Unescapes `data[begin..end]`. `None` for an invalid escape.
    // Port of: modules/jsonreader/SkJSONReader.cpp#L671-L716 (chrome/m156) (`unescapeString`)
    fn unescape_string(&self, begin: usize, end: usize) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        let mut p = begin;
        while p != end {
            if self.at(p) != b'\\' {
                out.push(self.at(p));
                p += 1;
                continue;
            }
            p += 1;
            if p == end {
                return None;
            }
            match self.at(p) {
                b'"' => out.push(b'"'),
                b'\\' => out.push(b'\\'),
                b'/' => out.push(b'/'),
                b'b' => out.push(0x08),
                b'f' => out.push(0x0C),
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'u' => {
                    if p + 4 >= end {
                        return None;
                    }
                    let hex = [
                        self.at(p + 1),
                        self.at(p + 2),
                        self.at(p + 3),
                        self.at(p + 4),
                        0,
                    ];
                    // The C string must end right after the four digits (`!eos || *eos`).
                    let Some((4, hexed)) = find_hex(&hex, 0) else {
                        return None;
                    };
                    let mut utf8 = [0_u8; 4];
                    let len = to_utf8(hexed.cast_signed(), Some(&mut utf8));
                    out.extend_from_slice(&utf8[..len]);
                    p += 4;
                }
                _ => return None,
            }
            p += 1;
        }
        Some(out)
    }

    /// Parses the string at `p` (its opening quote). Returns the position after the closing
    /// quote and the decoded bytes.
    // Port of: modules/jsonreader/SkJSONReader.cpp#L719-L767 (chrome/m156) (`matchString`)
    fn match_string(&self, mut p: usize, p_stop: usize) -> Option<(usize, Vec<u8>)> {
        let s_begin = p + 1;
        let mut requires_unescape = false;
        loop {
            // Consume string chars. This is the fast path.
            p += 1;
            while !is_eostring(self.at(p)) {
                p += 1;
            }
            if self.at(p) == b'"' {
                let bytes = if requires_unescape {
                    self.unescape_string(s_begin, p)?
                } else {
                    self.data[s_begin..p].to_vec()
                };
                return Some((p + 1, bytes));
            }
            if self.at(p) == b'\\' {
                requires_unescape = true;
                p += 1;
            } else if !is_eoscope(self.at(p)) {
                // Invalid/unexpected char.
                // End-of-scope chars are special: they tag the end of the input, so they are
                // consumed here and the loop condition checks for the end.
                break;
            }
            if p == p_stop {
                break;
            }
        }
        // Premature end-of-input, or illegal string char.
        None
    }

    /// The `[-]digits[.digits]` fast path with a `float` accumulator, after a decimal point.
    // Port of: modules/jsonreader/SkJSONReader.cpp#L769-L789 (chrome/m156) (`matchFastFloatDecimalPart`)
    fn match_fast_float_decimal_part(
        &self,
        mut p: usize,
        sign: i32,
        mut f: f32,
        mut exp: i32,
    ) -> Option<(usize, NumberValue)> {
        loop {
            if !is_digit(self.at(p)) {
                break;
            }
            f = f * 10.0 + f32::from(self.at(p) - b'0');
            p += 1;
            exp -= 1;
            if !is_digit(self.at(p)) {
                break;
            }
            f = f * 10.0 + f32::from(self.at(p) - b'0');
            p += 1;
            exp -= 1;
        }
        let decimal_scale = pow10(exp);
        if is_numeric(self.at(p)) || decimal_scale == 0.0 {
            // Malformed input, or an (unsupported) exponent, or a collapsed decimal factor.
            return None;
        }
        Some((p, NumberValue::float(int_as_f32(sign) * f * decimal_scale)))
    }

    /// The integral-part fast path with a `float` accumulator.
    // Port of: modules/jsonreader/SkJSONReader.cpp#L791-L807 (chrome/m156) (`matchFastFloatPart`)
    fn match_fast_float_part(
        &self,
        mut p: usize,
        sign: i32,
        mut f: f32,
    ) -> Option<(usize, NumberValue)> {
        loop {
            if !is_digit(self.at(p)) {
                break;
            }
            f = f * 10.0 + f32::from(self.at(p) - b'0');
            p += 1;
            if !is_digit(self.at(p)) {
                break;
            }
            f = f * 10.0 + f32::from(self.at(p) - b'0');
            p += 1;
        }
        if !is_numeric(self.at(p)) {
            // Matched (integral) float.
            return Some((p, NumberValue::float(int_as_f32(sign) * f)));
        }
        if self.at(p) == b'.' {
            self.match_fast_float_decimal_part(p + 1, sign, f, 0)
        } else {
            None
        }
    }

    /// The fast path for numbers: an `int32_t` when there is no fraction and no overflow, and a
    /// `float` accumulator otherwise.
    // Port of: modules/jsonreader/SkJSONReader.cpp#L809-L869 (chrome/m156) (`matchFast32OrFloat`)
    fn match_fast_32_or_float(&self, mut p: usize) -> Option<(usize, NumberValue)> {
        let mut sign = 1_i32;
        if self.at(p) == b'-' {
            sign = -1;
            p += 1;
        }
        let digits_start = p;
        let mut n32: i32 = 0;

        if is_digit(self.at(p)) {
            n32 = i32::from(self.at(p) - b'0');
            p += 1;
            loop {
                if !is_digit(self.at(p)) || n32 > K_MAX_INT32 {
                    break;
                }
                n32 = n32 * 10 + i32::from(self.at(p) - b'0');
                p += 1;
            }
        }
        if !is_numeric(self.at(p)) {
            // Did we actually match any digits?
            if p > digits_start {
                return Some((p, NumberValue::int(sign * n32)));
            }
            return None;
        }
        if self.at(p) == b'.' {
            p += 1;
            let decimals_start = p;
            let mut exp = 0_i32;
            loop {
                if !is_digit(self.at(p)) || n32 > K_MAX_INT32 {
                    break;
                }
                n32 = n32 * 10 + i32::from(self.at(p) - b'0');
                exp -= 1;
                p += 1;
                if !is_digit(self.at(p)) || n32 > K_MAX_INT32 {
                    break;
                }
                n32 = n32 * 10 + i32::from(self.at(p) - b'0');
                exp -= 1;
                p += 1;
            }
            if !is_numeric(self.at(p)) {
                // Did we actually match any digits?
                if p > decimals_start {
                    return Some((p, NumberValue::float(int_as_f32(sign * n32) * pow10(exp))));
                }
                return None;
            }
            if n32 > K_MAX_INT32 {
                // we ran out on n32 bits
                return self.match_fast_float_decimal_part(p, sign, int_as_f32(n32), exp);
            }
        }
        self.match_fast_float_part(p, sign, int_as_f32(n32))
    }

    /// Parses a number starting at `p`: the fast path, then the `from_chars` fallback over the
    /// text up to `p_stop`.
    // Port of: modules/jsonreader/SkJSONReader.cpp#L871-L891 (chrome/m156) (`matchNumber`)
    fn match_number(&self, p: usize, p_stop: usize) -> Option<(usize, NumberValue)> {
        if let Some(fast) = self.match_fast_32_or_float(p) {
            return Some(fast);
        }
        // slow fallback
        let text = self.data.get(p..=p_stop).unwrap_or(&[]);
        let (consumed, value) = from_chars_f32(text)?;
        Some((p + consumed, NumberValue::float(value)))
    }
}

/// Writes `value` as compact JSON, iteratively.
// Port of: modules/jsonreader/SkJSONReader.cpp#L894-L974 (chrome/m156) (`Write`)
fn write_value(value: &Value, stream: &mut dyn WStream) {
    write_pending(PendingRoot::Value(value), stream);
}

/// What `write_pending` starts from.
#[derive(Clone, Copy)]
enum PendingRoot<'a> {
    Value(&'a Value),
    Object(&'a ObjectValue),
}

/// Writes a root value or object as compact JSON, iteratively.
fn write_pending(root: PendingRoot<'_>, stream: &mut dyn WStream) {
    enum Pending<'a> {
        Value(&'a Value),
        Object(&'a ObjectValue),
        Key(&'a StringValue),
        ArrayClose,
        ObjectClose,
        ListSeparator,
        KeySeparator,
    }

    let mut pending = vec![match root {
        PendingRoot::Value(value) => Pending::Value(value),
        PendingRoot::Object(object) => Pending::Object(object),
    }];
    while let Some(item) = pending.pop() {
        match item {
            Pending::ArrayClose => {
                stream.write_text("]");
            }
            Pending::ObjectClose => {
                stream.write_text("}");
            }
            Pending::ListSeparator => {
                stream.write_text(",");
            }
            Pending::KeySeparator => {
                stream.write_text(":");
            }
            Pending::Key(key) => write_quoted(stream, key.as_bytes()),
            Pending::Object(object) => {
                stream.write_text("{");
                // "key: val, key: val, .. }" in reverse order
                pending.push(Pending::ObjectClose);
                let mut last_member = true;
                for member in object.members().iter().rev() {
                    if !last_member {
                        pending.push(Pending::ListSeparator);
                    }
                    pending.push(Pending::Value(&member.value));
                    pending.push(Pending::KeySeparator);
                    pending.push(Pending::Key(&member.key));
                    last_member = false;
                }
            }
            Pending::Value(val) => match val {
                Value::Null(_) => {
                    stream.write_text("null");
                }
                Value::Bool(b) => {
                    stream.write_text(if b.value() { "true" } else { "false" });
                }
                Value::Number(n) => {
                    // SkScalar (float) parameter: the double rounds to float.
                    let mut text = String::new();
                    str_append_scalar(&mut text, f64_as_f32(n.value()));
                    stream.write_text(&text);
                }
                Value::String(s) => write_quoted(stream, s.as_bytes()),
                Value::Array(array) => {
                    stream.write_text("[");
                    // "val, val, .. ]" in reverse order
                    pending.push(Pending::ArrayClose);
                    let mut last_value = true;
                    for item in array.items().iter().rev() {
                        if !last_value {
                            pending.push(Pending::ListSeparator);
                        }
                        pending.push(Pending::Value(item));
                        last_value = false;
                    }
                }
                Value::Object(object) => pending.push(Pending::Object(object)),
            },
        }
    }
}

/// Writes a string between quotes, without escaping (Skia writes the stored text as is).
fn write_quoted(stream: &mut dyn WStream, bytes: &[u8]) {
    stream.write8(b'"');
    stream.write(bytes);
    stream.write8(b'"');
}
