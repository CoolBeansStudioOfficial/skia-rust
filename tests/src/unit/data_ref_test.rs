// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/DataRefTest.cpp (chrome/m156)

#![cfg(test)]

// The aliases keep Skia's names: `def_test!(Data, ..)` and `def_test!(DataTable, ..)` define
// functions with the names of the types.
use skia_rust_core::data::Data as SkData;
use skia_rust_core::data_table::DataTable as SkDataTable;
use skia_rust_core::stream::{FileWStream, WStream};

use crate::tmp_dir::get_tmp_dir;
use crate::{Reporter, def_test, errorf, reporter_assert};

// Port of: tests/DataRefTest.cpp#L24-L34 (chrome/m156)
#[allow(clippy::similar_names)] // mirrors the C++ names (mema/memb, sizea/sizeb)
fn test_is_equal(reporter: &mut Reporter, a: &SkDataTable, b: &SkDataTable) {
    reporter_assert!(reporter, a.count() == b.count());
    for i in 0..a.count() {
        let mema = a.at(i);
        let memb = b.at(i);
        let sizea = mema.len();
        let sizeb = memb.len();
        reporter_assert!(reporter, sizea == sizeb);
        reporter_assert!(reporter, mema == memb);
    }
}

// Port of: tests/DataRefTest.cpp#L36-L39 (chrome/m156)
fn test_datatable_is_empty(reporter: &mut Reporter, table: &SkDataTable) {
    reporter_assert!(reporter, table.is_empty());
    reporter_assert!(reporter, 0 == table.count());
}

// Port of: tests/DataRefTest.cpp#L41-L55 (chrome/m156)
fn test_emptytable(reporter: &mut Reporter) {
    let table0 = SkDataTable::make_empty();
    let table1 = SkDataTable::make_copy_arrays(&[]);
    let table2 = SkDataTable::make_copy_array(&[], 0, 0);
    let table3 = SkDataTable::make_array_proc(Vec::<u8>::new(), 0, 0);

    test_datatable_is_empty(reporter, &table0);
    test_datatable_is_empty(reporter, &table1);
    test_datatable_is_empty(reporter, &table2);
    test_datatable_is_empty(reporter, &table3);

    test_is_equal(reporter, &table0, &table1);
    test_is_equal(reporter, &table0, &table2);
    test_is_equal(reporter, &table0, &table3);
}

// Port of: tests/DataRefTest.cpp#L57-L68 (chrome/m156)
fn test_simpletable(reporter: &mut Reporter) {
    let idata: [i32; 6] = [1, 4, 9, 16, 25, 63];
    let icount = idata.len();
    let idata_bytes: Vec<u8> = idata.iter().flat_map(|i| i.to_ne_bytes()).collect();
    let itable = SkDataTable::make_copy_array(&idata_bytes, size_of::<i32>(), icount);
    reporter_assert!(reporter, itable.count() == icount);
    for (i, expected) in idata.iter().enumerate() {
        reporter_assert!(reporter, size_of::<i32>() == itable.at_size(i));
        // `*itable->atT<int>(i, &size)`: the entry's bytes read as an int.
        let entry = itable.at(i);
        let value = i32::from_ne_bytes(entry[..4].try_into().unwrap());
        reporter_assert!(reporter, value == *expected);
        let size = entry.len();
        reporter_assert!(reporter, size_of::<i32>() == size);
    }
}

// Port of: tests/DataRefTest.cpp#L70-L93 (chrome/m156)
fn test_vartable(reporter: &mut Reporter) {
    let str_: [&str; 7] = [
        "",
        "a",
        "be",
        "see",
        "deigh",
        "ef",
        "ggggggggggggggggggggggggggg",
    ];
    let count = str_.len();
    let sizes: Vec<usize> = str_.iter().map(|s| s.len() + 1).collect();
    // The entries include the trailing nul, as the C strings do.
    let arrays: Vec<Vec<u8>> = str_
        .iter()
        .map(|s| {
            let mut bytes = s.as_bytes().to_vec();
            bytes.push(0);
            bytes
        })
        .collect();
    let array_refs: Vec<&[u8]> = arrays.iter().map(Vec::as_slice).collect();

    let table = SkDataTable::make_copy_arrays(&array_refs);

    reporter_assert!(reporter, table.count() == count);
    for i in 0..count {
        reporter_assert!(reporter, table.at_size(i) == sizes[i]);
        // `!strcmp(table->atT<const char>(i, &size), str[i])`
        reporter_assert!(reporter, table.at_str(i) == str_[i]);
        let size = table.at(i).len();
        reporter_assert!(reporter, size == sizes[i]);

        let s = table.at_str(i);
        reporter_assert!(reporter, s.len() == str_[i].len());
    }
}

// Port of: tests/DataRefTest.cpp#L95-L111 (chrome/m156)
fn test_globaltable(reporter: &mut Reporter) {
    let g_data: [i32; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
    let count = g_data.len();
    let g_data_bytes: Vec<u8> = g_data.iter().flat_map(|i| i.to_ne_bytes()).collect();

    // `MakeArrayProc(gData, ..., nullptr, nullptr)`: no free proc, the table just owns the bytes.
    let table = SkDataTable::make_array_proc(g_data_bytes, size_of::<i32>(), count);

    reporter_assert!(reporter, table.count() == count);
    for i in 0..count {
        reporter_assert!(reporter, table.at_size(i) == size_of::<i32>());
        // `*table->atT<const char>(i, &size) == i`: the first byte of the entry (little endian).
        let entry = table.at(i);
        reporter_assert!(
            reporter,
            i32::from(entry[0].cast_signed()) == i32::try_from(i).unwrap()
        );
        let size = entry.len();
        reporter_assert!(reporter, size_of::<i32>() == size);
    }
}

// Port of: tests/DataRefTest.cpp#L113-L118 (chrome/m156)
def_test!(DataTable, |reporter| {
    test_emptytable(reporter);
    test_simpletable(reporter);
    test_vartable(reporter);
    test_globaltable(reporter);
});

// skia-rust: not expressible in Rust: `gGlobal` and `delete_int_proc` (Skia's `MakeWithProc`
// release proc, which frees the `new int[N]` allocation and asserts its context); a Rust owner
// frees itself when the `Data` is dropped.

// Port of: tests/DataRefTest.cpp#L128-L130 (chrome/m156)
fn assert_len(reporter: &mut Reporter, r#ref: &SkData, len: usize) {
    reporter_assert!(reporter, r#ref.size() == len);
}

// Port of: tests/DataRefTest.cpp#L132-L136 (chrome/m156)
fn assert_data(reporter: &mut Reporter, r#ref: &SkData, data: &[u8], len: usize) {
    reporter_assert!(reporter, r#ref.size() == len);
    reporter_assert!(reporter, r#ref.as_bytes()[..len] == data[..len]);
}

// Port of: tests/DataRefTest.cpp#L138-L150 (chrome/m156)
fn test_cstring(reporter: &mut Reporter) {
    let str_ = c"Hello world";
    let len = str_.to_bytes().len();

    let r0 = SkData::new_copy(&str_.to_bytes_with_nul()[..=len]);
    let r1 = SkData::new_with_cstring(Some(str_));

    reporter_assert!(reporter, r0.equals(Some(&r1)));

    let r2 = SkData::new_with_cstring(None);
    reporter_assert!(reporter, 1 == r2.size());
    reporter_assert!(reporter, 0 == r2.as_bytes()[0]);
}

// Port of: tests/DataRefTest.cpp#L152-L181 (chrome/m156)
fn test_files(reporter: &mut Reporter) {
    let Some(tmp_dir) = get_tmp_dir() else {
        return;
    };

    let path = tmp_dir.join("data_test");

    let s = b"abcdefghijklmnopqrstuvwxyz";
    {
        let mut writer = FileWStream::new(&path);
        if !writer.is_valid() {
            errorf!(reporter, "Failed to create tmp file {}\n", path.display());
            return;
        }
        writer.write(s);
    }

    let file = std::fs::File::open(&path).unwrap();
    let r1 = SkData::new_from_file(&file);
    reporter_assert!(reporter, r1.is_some());
    let r1 = r1.unwrap();
    reporter_assert!(reporter, r1.size() == 26);
    reporter_assert!(reporter, r1.as_bytes()[..26] == s[..26]);

    // `SkData::MakeFromFD(sk_fileno(file))`: a `File` is both a FILE and a file descriptor, so
    // `new_from_file` serves both.
    let r2 = SkData::new_from_file(&file);
    reporter_assert!(reporter, r2.is_some());
    let r2 = r2.unwrap();
    reporter_assert!(reporter, r2.size() == 26);
    reporter_assert!(reporter, r2.as_bytes()[..26] == s[..26]);
}

// Port of: tests/DataRefTest.cpp#L183-L188 (chrome/m156)
// `SkSpan` equality in this test file is "same size and same address".
fn span_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && std::ptr::eq(a.as_ptr(), b.as_ptr())
}

// Port of: tests/DataRefTest.cpp#L190-L195 (chrome/m156)
fn deep_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.is_empty() || a[..a.len()] == b[..a.len()]
}

struct Subset {
    offset: usize,
    length: usize,
}

// Port of: tests/DataRefTest.cpp#L197-L237 (chrome/m156)
fn test_subsets(reporter: &mut Reporter) {
    // `MakeWithoutCopy(array, ..)` needs memory that outlives the `Data`.
    static ARRAY: [u8; 4] = [1, 2, 3, 4];
    let src = SkData::new_static(&ARRAY);

    let bad_subsets = [
        Subset {
            offset: 5,
            length: 0,
        },
        Subset {
            offset: 4,
            length: 2,
        },
        Subset {
            offset: 0,
            length: 5,
        },
        Subset {
            offset: 3,
            length: 2,
        },
    ];
    for s in &bad_subsets {
        reporter_assert!(reporter, src.share_subset(s.offset, s.length).is_none());
        reporter_assert!(reporter, src.copy_subset(s.offset, s.length).is_none());
    }

    let empty_subsets = [
        Subset {
            offset: 0,
            length: 0,
        },
        Subset {
            offset: 2,
            length: 0,
        },
        Subset {
            offset: 4,
            length: 0,
        },
    ];
    for s in &empty_subsets {
        reporter_assert!(
            reporter,
            src.share_subset(s.offset, s.length).unwrap().is_empty()
        );
        reporter_assert!(
            reporter,
            src.copy_subset(s.offset, s.length).unwrap().is_empty()
        );
    }

    let assert_shared = |reporter: &mut Reporter, data: SkData, src: &[u8]| {
        reporter_assert!(reporter, span_eq(data.as_bytes(), src));
    };
    let assert_copied = |reporter: &mut Reporter, data: SkData, src: &[u8]| {
        reporter_assert!(reporter, !span_eq(data.as_bytes(), src));
        reporter_assert!(reporter, deep_equal(data.as_bytes(), src));
    };

    let nonempty_subsets = [
        Subset {
            offset: 0,
            length: 4,
        },
        Subset {
            offset: 2,
            length: 2,
        },
        Subset {
            offset: 0,
            length: 2,
        },
        Subset {
            offset: 1,
            length: 2,
        },
    ];
    for s in &nonempty_subsets {
        let src_span = &src.as_bytes()[s.offset..s.offset + s.length];
        assert_shared(
            reporter,
            src.share_subset(s.offset, s.length).unwrap(),
            src_span,
        );
        assert_copied(
            reporter,
            src.copy_subset(s.offset, s.length).unwrap(),
            src_span,
        );
    }
}

// Port of: tests/DataRefTest.cpp#L239-L258 (chrome/m156)
#[allow(clippy::eq_op)] // `*d == *d` is the point of the assertion
fn test_copies(reporter: &mut Reporter) {
    let mut array = [0i32; 10];
    for (i, item) in array.iter_mut().enumerate() {
        *item = i32::try_from(i).unwrap();
    }
    let array_bytes: Vec<u8> = array.iter().flat_map(|i| i.to_ne_bytes()).collect();

    let d = SkData::new_copy(&array_bytes);
    reporter_assert!(reporter, d.size() == array_bytes.len());
    reporter_assert!(
        reporter,
        array_bytes[..] == d.as_bytes()[..array_bytes.len()]
    );

    let d1 = d.share_subset(8, 16).unwrap(); // 2, 3, 4, 5
    reporter_assert!(reporter, d1.size() == 16);
    reporter_assert!(
        reporter,
        std::ptr::eq(d1.as_bytes().as_ptr(), d.as_bytes()[8..].as_ptr())
    );

    let d2 = d.copy_subset(8, 16).unwrap();
    reporter_assert!(reporter, d2.size() == 16);
    reporter_assert!(
        reporter,
        d2.as_bytes()[..d2.size()] == array_bytes[8..8 + d2.size()]
    );

    reporter_assert!(reporter, d == d);
    reporter_assert!(reporter, d != d1);
    reporter_assert!(reporter, d1 == d2);
}

// Port of: tests/DataRefTest.cpp#L260-L287 (chrome/m156)
def_test!(Data, |reporter| {
    const N: usize = 10;
    let str_ = "We the people, in order to form a more perfect union.";

    let r0 = SkData::new_empty();
    let r1 = SkData::new_copy(str_.as_bytes());
    let r2 = SkData::new_with_owner(vec![0u8; N * size_of::<i32>()]);
    let r3 = SkData::new_subset(&r1, 7, 6);

    assert_len(reporter, &r0, 0);
    assert_len(reporter, &r1, str_.len());
    assert_len(reporter, &r2, N * size_of::<i32>());
    assert_len(reporter, &r3, 6);

    assert_data(reporter, &r1, str_.as_bytes(), str_.len());
    assert_data(reporter, &r3, b"people", 6);

    let mut tmp = SkData::new_subset(&r1, str_.len(), 10);
    assert_len(reporter, &tmp, 0);
    tmp = SkData::new_subset(&r1, 0, 0);
    assert_len(reporter, &tmp, 0);

    test_cstring(reporter);
    test_files(reporter);

    test_subsets(reporter);
    test_copies(reporter);
});

// Port of: tests/DataRefTest.cpp#L289-L306 (chrome/m156)
def_test!(Data_empty, |reporter| {
    let array = [
        SkData::new_empty(),
        SkData::new_uninitialized(0),
        // `MakeFromMalloc(sk_malloc_throw(0), 0)`
        SkData::new_from_vec(Vec::new()),
        SkData::new_copy(b""),
        // `MakeWithProc(nullptr, 0, [](const void*, void*) {}, nullptr)`
        SkData::new_with_owner(Vec::<u8>::new()),
        SkData::new_static(&[]),
    ];
    let n = array.len();

    for i in 0..n {
        reporter_assert!(reporter, array[i].size() == 0);
        for j in 0..n {
            reporter_assert!(reporter, array[i].equals(Some(&array[j])));
        }
    }
});
