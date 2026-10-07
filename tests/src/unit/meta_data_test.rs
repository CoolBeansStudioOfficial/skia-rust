// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/MetaDataTest.cpp (chrome/m156)

#![cfg(test)]

use crate::tools::sk_meta_data::{Iter, MetaData, Type};
use crate::{def_test, reporter_assert};

struct Elems {
    name: &'static str,
    ty: Type,
    count: usize,
}

// Port of: tests/MetaDataTest.cpp#L17-L86 (chrome/m156)
def_test!(MetaData, |reporter| {
    let mut m1 = MetaData::new();

    reporter_assert!(reporter, m1.find_s32("int").is_none());
    reporter_assert!(reporter, m1.find_scalar("scalar").is_none());
    reporter_assert!(reporter, !m1.remove_s32("int"));
    reporter_assert!(reporter, !m1.remove_scalar("scalar"));

    m1.set_s32("int", 12345);
    m1.set_scalar("scalar", 1.0 * 42.0);
    // `m1.setPtr("ptr", &m1)`: the pointer is stored as an address.
    let m1_address = std::ptr::from_ref(&m1) as usize;
    m1.set_ptr("ptr", m1_address);
    m1.set_bool("true", true);
    m1.set_bool("false", false);

    m1.set_scalar("scalar", 1.0 / 2.0);

    reporter_assert!(reporter, m1.find_s32("int") == Some(12345));
    reporter_assert!(reporter, m1.find_scalar("scalar") == Some(1.0 / 2.0));
    reporter_assert!(reporter, m1.has_bool("true", true));
    reporter_assert!(reporter, m1.has_bool("false", false));

    let mut iter = Iter::new(&m1);

    let g_elems = [
        Elems {
            name: "int",
            ty: Type::S32,
            count: 1,
        },
        Elems {
            name: "scalar",
            ty: Type::Scalar,
            count: 1,
        },
        Elems {
            name: "ptr",
            ty: Type::Ptr,
            count: 1,
        },
        Elems {
            name: "true",
            ty: Type::Bool,
            count: 1,
        },
        Elems {
            name: "false",
            ty: Type::Bool,
            count: 1,
        },
    ];

    let mut loop_count = 0usize;
    while let Some((name, t, count)) = iter.next() {
        let mut matches = 0;
        for elem in &g_elems {
            if name == elem.name {
                matches += 1;
                reporter_assert!(reporter, elem.ty == t);
                reporter_assert!(reporter, elem.count == count);
            }
        }
        reporter_assert!(reporter, matches == 1);
        loop_count += 1;
    }
    reporter_assert!(reporter, loop_count == g_elems.len());

    reporter_assert!(reporter, m1.remove_s32("int"));
    reporter_assert!(reporter, m1.remove_scalar("scalar"));
    reporter_assert!(reporter, m1.remove_bool("true"));
    reporter_assert!(reporter, m1.remove_bool("false"));

    reporter_assert!(reporter, m1.find_s32("int").is_none());
    reporter_assert!(reporter, m1.find_scalar("scalar").is_none());
    reporter_assert!(reporter, m1.find_bool("true").is_none());
    reporter_assert!(reporter, m1.find_bool("false").is_none());
});
