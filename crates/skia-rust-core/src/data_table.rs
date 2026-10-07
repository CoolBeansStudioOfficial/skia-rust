// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkDataTable.h, src/core/SkDataTable.cpp

//! `DataTable`: an immutable table of byte entries.

use std::sync::Arc;

/// Where a table's entries live.
enum Storage {
    /// An empty table.
    Empty,
    /// `fElemSize != 0`: `count` entries of `elem_size` bytes, contiguous in `elems`.
    Elems {
        elems: Box<dyn AsRef<[u8]> + Send + Sync>,
        elem_size: usize,
    },
    /// `fElemSize == 0`: a directory of `(offset, size)` ranges into `bytes`.
    Dir {
        bytes: Vec<u8>,
        dir: Vec<(usize, usize)>,
    },
}

struct Inner {
    count: usize,
    storage: Storage,
}

/// Like [`Data`](crate::data::Data), `DataTable` holds an immutable data buffer. The data buffer
/// is organized into a table of entries, each with a length, so the entries are not required to
/// all be the same size.
// Port of: include/core/SkDataTable.h#L23-L120 (chrome/m156)
#[doc(alias = "SkDataTable")]
#[derive(Clone)]
pub struct DataTable(Arc<Inner>);

impl std::fmt::Debug for DataTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataTable")
            .field("count", &self.0.count)
            .finish()
    }
}

impl Default for DataTable {
    fn default() -> Self {
        Self::make_empty()
    }
}

impl DataTable {
    /// Returns true if the table is empty (i.e. has no entries).
    // Port of: include/core/SkDataTable.h#L28 (chrome/m156)
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        0 == self.0.count
    }

    /// Returns the number of entries in the table. 0 for an empty table.
    // Port of: include/core/SkDataTable.h#L33 (chrome/m156)
    #[must_use]
    pub fn count(&self) -> usize {
        self.0.count
    }

    /// Returns the size of the `index`'th entry in the table.
    ///
    /// # Panics
    /// If `index >= count()`.
    // Port of: src/core/SkDataTable.cpp#L56-L64 (chrome/m156)
    #[doc(alias = "atSize")]
    #[must_use]
    pub fn at_size(&self, index: usize) -> usize {
        self.at(index).len()
    }

    /// Returns the data of the `index`'th entry in the table. Its length is the entry's size.
    ///
    /// # Panics
    /// If `index >= count()`.
    // Port of: src/core/SkDataTable.cpp#L66-L80 (chrome/m156)
    #[doc(alias = "atT")]
    #[must_use]
    pub fn at(&self, index: usize) -> &[u8] {
        assert!(index < self.0.count);
        match &self.0.storage {
            Storage::Empty => unreachable!("an empty table has no entries"),
            Storage::Elems { elems, elem_size } => {
                &(**elems).as_ref()[index * elem_size..(index + 1) * elem_size]
            }
            Storage::Dir { bytes, dir } => {
                let (offset, size) = dir[index];
                &bytes[offset..offset + size]
            }
        }
    }

    /// Returns the `index`'th entry as a string, assuming that the trailing nul byte had been
    /// copied into the table as well. The nul is not part of the returned string.
    ///
    /// # Panics
    /// If `index >= count()`, or if the entry is not a nul-terminated UTF-8 string.
    // Port of: include/core/SkDataTable.h#L59-L64 (chrome/m156)
    #[doc(alias = "atStr")]
    #[must_use]
    pub fn at_str(&self, index: usize) -> &str {
        let entry = self.at(index);
        let (nul, text) = entry.split_last().expect("entry has a trailing nul");
        assert_eq!(*nul, 0, "entry is nul-terminated");
        let text = std::str::from_utf8(text).expect("entry is UTF-8");
        debug_assert!(!text.contains('\0'));
        text
    }

    /// Returns the empty table.
    // Port of: src/core/SkDataTable.cpp#L84-L87 (chrome/m156)
    #[doc(alias = "MakeEmpty")]
    #[must_use]
    pub fn make_empty() -> Self {
        Self(Arc::new(Inner {
            count: 0,
            storage: Storage::Empty,
        }))
    }

    /// Returns a new `DataTable` that contains a copy of the data stored in each "array". The
    /// sizes are the lengths of the slices.
    // Port of: src/core/SkDataTable.cpp#L89-L113 (chrome/m156)
    #[doc(alias = "MakeCopyArrays")]
    #[must_use]
    pub fn make_copy_arrays(arrays: &[&[u8]]) -> Self {
        if arrays.is_empty() {
            return Self::make_empty();
        }

        let data_size: usize = arrays.iter().map(|array| array.len()).sum();
        let mut bytes = Vec::with_capacity(data_size);
        let mut dir = Vec::with_capacity(arrays.len());
        for array in arrays {
            dir.push((bytes.len(), array.len()));
            bytes.extend_from_slice(array);
        }

        Self(Arc::new(Inner {
            count: arrays.len(),
            storage: Storage::Dir { bytes, dir },
        }))
    }

    /// Returns a new table that contains a copy of the data in `array`: `count` elements of
    /// `elem_size` bytes each, contiguous.
    ///
    /// # Panics
    /// If `array` is shorter than `elem_size * count`.
    // Port of: src/core/SkDataTable.cpp#L115-L125 (chrome/m156)
    #[doc(alias = "MakeCopyArray")]
    #[must_use]
    pub fn make_copy_array(array: &[u8], elem_size: usize, count: usize) -> Self {
        if count == 0 {
            return Self::make_empty();
        }

        let buffer_size = elem_size * count;
        let buffer = array[..buffer_size].to_vec();
        Self(Arc::new(Inner {
            count,
            storage: Storage::Elems {
                elems: Box::new(buffer),
                elem_size,
            },
        }))
    }

    /// Returns a new table over `count` elements of `elem_size` bytes each, contiguous in
    /// `array`, which is dropped when the table is. This replaces Skia's
    /// `MakeArrayProc(array, elemSize, count, freeProc, context)`: the owner's `Drop` is the
    /// free proc.
    ///
    /// # Panics
    /// If `array` is shorter than `elem_size * count`.
    // Port of: src/core/SkDataTable.cpp#L127-L133 (chrome/m156)
    #[doc(alias = "MakeArrayProc")]
    pub fn make_array_proc<T: AsRef<[u8]> + Send + Sync + 'static>(
        array: T,
        elem_size: usize,
        count: usize,
    ) -> Self {
        if count == 0 {
            return Self::make_empty();
        }
        assert!(array.as_ref().len() >= elem_size * count);
        Self(Arc::new(Inner {
            count,
            storage: Storage::Elems {
                elems: Box::new(array),
                elem_size,
            },
        }))
    }
}
