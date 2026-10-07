// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tools/SkMetaData.h, tools/SkMetaData.cpp (chrome/m156)

//! `SkMetaData`: a map from string keys to arrays of POD values (`i32`, scalar, pointer,
//! `bool`).
//!
//! skia-rust: Skia's intrusive linked list of variable-sized records is a `Vec` of records
//! (front of the `Vec` is the head of the list), and `void*` values are addresses (`usize`).

use skia_rust_core::scalar::scalar;

/// `SkMetaData::Type`.
// Port of: tools/SkMetaData.h#L63-L70 (chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    S32,
    Scalar,
    Ptr,
    Bool,
}

#[derive(Debug, Clone)]
enum Values {
    S32(Vec<i32>),
    Scalar(Vec<scalar>),
    Ptr(Vec<usize>),
    Bool(Vec<bool>),
}

impl Values {
    fn ty(&self) -> Type {
        match self {
            Values::S32(_) => Type::S32,
            Values::Scalar(_) => Type::Scalar,
            Values::Ptr(_) => Type::Ptr,
            Values::Bool(_) => Type::Bool,
        }
    }

    fn count(&self) -> usize {
        match self {
            Values::S32(v) => v.len(),
            Values::Scalar(v) => v.len(),
            Values::Ptr(v) => v.len(),
            Values::Bool(v) => v.len(),
        }
    }
}

// Port of: tools/SkMetaData.h#L97-L112 (chrome/m156)
#[derive(Debug, Clone)]
struct Rec {
    name: String,
    values: Values,
}

/// A map from string keys to arrays of POD values.
// Port of: tools/SkMetaData.h#L17-L121 (chrome/m156)
#[doc(alias = "SkMetaData")]
#[derive(Debug, Default)]
pub struct MetaData {
    recs: Vec<Rec>,
}

impl MetaData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // Port of: tools/SkMetaData.cpp#L10-L19 (chrome/m156)
    pub fn reset(&mut self) {
        self.recs.clear();
    }

    // Port of: tools/SkMetaData.cpp#L108-L131 (chrome/m156)
    fn find(&self, name: &str, ty: Type) -> Option<&Values> {
        self.recs
            .iter()
            .find(|rec| rec.values.ty() == ty && rec.name == name)
            .map(|rec| &rec.values)
    }

    // Port of: tools/SkMetaData.cpp#L50-L106 (chrome/m156)
    //
    // skia-rust: when the old record is replaced because the count changed, the new one takes
    // its place in the list (Skia drops the new record if the old one was the head).
    fn set(&mut self, name: &str, values: Values) {
        let found = self
            .recs
            .iter()
            .position(|rec| rec.values.ty() == values.ty() && rec.name == name);
        match found {
            Some(index) => self.recs[index].values = values,
            // Adding a new one, stick it at head.
            None => self.recs.insert(
                0,
                Rec {
                    name: name.to_owned(),
                    values,
                },
            ),
        }
    }

    // Port of: tools/SkMetaData.cpp#L208-L216 (chrome/m156)
    fn remove(&mut self, name: &str, ty: Type) -> bool {
        let Some(index) = self
            .recs
            .iter()
            .position(|rec| rec.values.ty() == ty && rec.name == name)
        else {
            return false;
        };
        self.recs.remove(index);
        true
    }

    // Port of: tools/SkMetaData.cpp#L62-L74 (chrome/m156)
    #[doc(alias = "findS32")]
    #[must_use]
    pub fn find_s32(&self, name: &str) -> Option<i32> {
        match self.find(name, Type::S32)? {
            Values::S32(v) => {
                debug_assert_eq!(v.len(), 1);
                Some(v[0])
            }
            _ => unreachable!(),
        }
    }

    // Port of: tools/SkMetaData.cpp#L76-L88 (chrome/m156)
    #[doc(alias = "findScalar")]
    #[must_use]
    pub fn find_scalar(&self, name: &str) -> Option<scalar> {
        match self.find(name, Type::Scalar)? {
            Values::Scalar(v) => {
                debug_assert_eq!(v.len(), 1);
                Some(v[0])
            }
            _ => unreachable!(),
        }
    }

    // Port of: tools/SkMetaData.cpp#L90-L103 (chrome/m156)
    #[doc(alias = "findScalars")]
    #[must_use]
    pub fn find_scalars(&self, name: &str) -> Option<&[scalar]> {
        match self.find(name, Type::Scalar)? {
            Values::Scalar(v) => Some(v),
            _ => unreachable!(),
        }
    }

    // Port of: tools/SkMetaData.cpp#L105-L116 (chrome/m156)
    #[doc(alias = "findPtr")]
    #[must_use]
    pub fn find_ptr(&self, name: &str) -> Option<usize> {
        match self.find(name, Type::Ptr)? {
            Values::Ptr(v) => {
                debug_assert_eq!(v.len(), 1);
                Some(v[0])
            }
            _ => unreachable!(),
        }
    }

    // Port of: tools/SkMetaData.cpp#L118-L130 (chrome/m156)
    #[doc(alias = "findBool")]
    #[must_use]
    pub fn find_bool(&self, name: &str) -> Option<bool> {
        match self.find(name, Type::Bool)? {
            Values::Bool(v) => {
                debug_assert_eq!(v.len(), 1);
                Some(v[0])
            }
            _ => unreachable!(),
        }
    }

    // Port of: tools/SkMetaData.h#L31-L46 (chrome/m156)
    #[doc(alias = "hasS32")]
    #[must_use]
    pub fn has_s32(&self, name: &str, value: i32) -> bool {
        self.find_s32(name) == Some(value)
    }

    #[doc(alias = "hasScalar")]
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors `v == value` in SkMetaData::hasScalar
    pub fn has_scalar(&self, name: &str, value: scalar) -> bool {
        self.find_scalar(name).is_some_and(|v| v == value)
    }

    #[doc(alias = "hasPtr")]
    #[must_use]
    pub fn has_ptr(&self, name: &str, value: usize) -> bool {
        self.find_ptr(name) == Some(value)
    }

    #[doc(alias = "hasBool")]
    #[must_use]
    pub fn has_bool(&self, name: &str, value: bool) -> bool {
        self.find_bool(name) == Some(value)
    }

    // Port of: tools/SkMetaData.cpp#L21-L48 (chrome/m156)
    #[doc(alias = "setS32")]
    pub fn set_s32(&mut self, name: &str, value: i32) {
        self.set(name, Values::S32(vec![value]));
    }

    #[doc(alias = "setScalar")]
    pub fn set_scalar(&mut self, name: &str, value: scalar) {
        self.set(name, Values::Scalar(vec![value]));
    }

    /// Sets an array of scalars.
    ///
    /// # Panics
    /// If `values` is empty.
    #[doc(alias = "setScalars")]
    pub fn set_scalars(&mut self, name: &str, values: &[scalar]) {
        assert_ne!(values.len(), 0);
        self.set(name, Values::Scalar(values.to_vec()));
    }

    #[doc(alias = "setPtr")]
    pub fn set_ptr(&mut self, name: &str, value: usize) {
        self.set(name, Values::Ptr(vec![value]));
    }

    #[doc(alias = "setBool")]
    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.set(name, Values::Bool(vec![value]));
    }

    // Port of: tools/SkMetaData.cpp#L218-L236 (chrome/m156)
    #[doc(alias = "removeS32")]
    pub fn remove_s32(&mut self, name: &str) -> bool {
        self.remove(name, Type::S32)
    }

    #[doc(alias = "removeScalar")]
    pub fn remove_scalar(&mut self, name: &str) -> bool {
        self.remove(name, Type::Scalar)
    }

    #[doc(alias = "removePtr")]
    pub fn remove_ptr(&mut self, name: &str) -> bool {
        self.remove(name, Type::Ptr)
    }

    #[doc(alias = "removeBool")]
    pub fn remove_bool(&mut self, name: &str) -> bool {
        self.remove(name, Type::Bool)
    }
}

/// Iterates over the entries of a [`MetaData`].
// Port of: tools/SkMetaData.h#L78-L95 (chrome/m156)
#[doc(alias = "SkMetaData::Iter")]
#[derive(Debug)]
pub struct Iter<'a> {
    recs: std::slice::Iter<'a, Rec>,
}

impl<'a> Iter<'a> {
    // Port of: tools/SkMetaData.cpp#L240-L246 (chrome/m156)
    #[must_use]
    pub fn new(metadata: &'a MetaData) -> Self {
        Self {
            recs: metadata.recs.iter(),
        }
    }

    /// Each time `next` is called, it returns the name of the next data element, its type and
    /// the number of data values, or `None` when there are no more elements.
    // Port of: tools/SkMetaData.cpp#L248-L264 (chrome/m156)
    #[allow(clippy::should_implement_trait)] // mirrors SkMetaData::Iter::next(Type*, int*)
    pub fn next(&mut self) -> Option<(&'a str, Type, usize)> {
        let rec = self.recs.next()?;
        Some((rec.name.as_str(), rec.values.ty(), rec.values.count()))
    }
}
