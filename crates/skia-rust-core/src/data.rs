// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkData.h, src/core/SkData.cpp

//! `Data`: an immutable, cheaply clonable (shared) byte buffer.

use std::ffi::CStr;
use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use crate::stream::Stream;
use crate::stream_priv;

/// How a [`Data`]'s bytes are held.
///
/// `SkData` holds a pointer, a length and an optional release proc. Safe Rust cannot hold a
/// borrowed pointer, so the possible owners are made explicit instead of a release proc.
enum Storage {
    /// `SkData::MakeEmpty()`.
    Empty,
    /// `SkData::MakeWithoutCopy()`: memory that outlives the program.
    Static(&'static [u8]),
    /// `MakeWithCopy`, `MakeUninitialized`, `MakeFromMalloc`: an owned, writable buffer.
    Vec(Vec<u8>),
    /// `MakeWithProc`: any owner; dropping it is the release proc.
    Owner(Box<dyn AsRef<[u8]> + Send + Sync>),
    /// `shareSubset`: a reference to a range of another `Data`.
    Subset {
        parent: Data,
        offset: usize,
        length: usize,
    },
}

/// `Data` holds an immutable data buffer. It can be created to allocate its own buffer for the
/// contents, or to share a buffer owned by someone else. The size and address of the contents
/// never change for the lifetime of the data object. Cloning is cheap (a reference count).
// Port of: include/core/SkData.h#L26-L223 (chrome/m156)
#[doc(alias = "SkData")]
#[derive(Clone)]
pub struct Data(Arc<Storage>);

impl Default for Data {
    fn default() -> Self {
        Self::new_empty()
    }
}

impl fmt::Debug for Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Data").field("size", &self.size()).finish()
    }
}

impl Deref for Data {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsRef<[u8]> for Data {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

/// Returns true if this and `other` are the same size, and contain the same contents. All empty
/// objects compare as equal.
// Port of: src/core/SkData.cpp#L42-L47 (chrome/m156)
impl PartialEq for Data {
    fn eq(&self, other: &Self) -> bool {
        if Arc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for Data {}

impl Data {
    fn from_storage(storage: Storage) -> Self {
        Self(Arc::new(storage))
    }

    /// Returns the number of bytes stored.
    // Port of: include/core/SkData.h#L53 (chrome/m156)
    #[must_use]
    pub fn size(&self) -> usize {
        self.as_bytes().len()
    }

    /// Returns true if there are no bytes stored.
    // Port of: include/core/SkData.h#L60 (chrome/m156)
    #[doc(alias = "empty")]
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }

    /// The stored bytes.
    // Port of: include/core/SkData.h#L58-L64 (chrome/m156)
    #[doc(alias = "data")]
    #[doc(alias = "bytes")]
    #[doc(alias = "byteSpan")]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match &*self.0 {
            Storage::Empty => &[],
            Storage::Static(bytes) => bytes,
            Storage::Vec(bytes) => bytes,
            Storage::Owner(owner) => (**owner).as_ref(),
            Storage::Subset {
                parent,
                offset,
                length,
            } => &parent.as_bytes()[*offset..*offset + *length],
        }
    }

    /// Writable access to the contents, available only while this is the only reference to a
    /// buffer that `Data` allocated itself (`new_uninitialized`, `new_zero_initialized`,
    /// `new_copy`, `new_from_vec`, ...).
    ///
    /// Skia's `writable_data()` is unchecked ("use with caution"); safe Rust can only offer it
    /// while no other `Data` shares the buffer.
    // Port of: include/core/SkData.h#L70-L72 (chrome/m156)
    pub fn writable_data(&mut self) -> Option<&mut [u8]> {
        match Arc::get_mut(&mut self.0)? {
            Storage::Empty => Some(&mut []),
            Storage::Vec(bytes) => Some(bytes.as_mut_slice()),
            _ => None,
        }
    }

    /// Returns a new empty dataref.
    // Port of: src/core/SkData.cpp#L130-L133 (chrome/m156)
    #[doc(alias = "MakeEmpty")]
    #[must_use]
    pub fn new_empty() -> Self {
        Self::from_storage(Storage::Empty)
    }

    /// Creates a new dataref by copying the specified data.
    // Port of: src/core/SkData.cpp#L110-L148 (chrome/m156)
    #[doc(alias = "MakeWithCopy")]
    #[must_use]
    pub fn new_copy(data: &[u8]) -> Self {
        if data.is_empty() {
            return Self::new_empty();
        }
        Self::from_storage(Storage::Vec(data.to_vec()))
    }

    /// Creates a new data of `length` bytes. Skia leaves the contents uninitialised; safe Rust
    /// cannot expose uninitialised memory, so the contents are zeroed. Fill them through
    /// [`Data::writable_data`] before sharing the `Data`.
    // Port of: src/core/SkData.cpp#L110-L152 (chrome/m156)
    #[doc(alias = "MakeUninitialized")]
    #[must_use]
    pub fn new_uninitialized(length: usize) -> Self {
        Self::new_zero_initialized(length)
    }

    /// Creates a new data with zero-initialized contents.
    // Port of: src/core/SkData.cpp#L154-L160 (chrome/m156)
    #[doc(alias = "MakeZeroInitialized")]
    #[must_use]
    pub fn new_zero_initialized(length: usize) -> Self {
        if length == 0 {
            return Self::new_empty();
        }
        Self::from_storage(Storage::Vec(vec![0; length]))
    }

    /// Creates a new dataref copying the c-string's bytes *and* its terminating nul, so `size()`
    /// is `strlen(cstr) + 1`. `None` is treated the same as `""`.
    // Port of: src/core/SkData.cpp#L202-L211 (chrome/m156)
    #[doc(alias = "MakeWithCString")]
    #[must_use]
    pub fn new_with_cstring(cstr: Option<&CStr>) -> Self {
        match cstr {
            None => Self::new_copy(&[0]),
            Some(cstr) => Self::new_copy(cstr.to_bytes_with_nul()),
        }
    }

    /// Creates a new dataref from memory that outlives the program, without copying it.
    // Port of: include/core/SkData.h#L139-L141 (chrome/m156)
    #[doc(alias = "MakeWithoutCopy")]
    #[must_use]
    pub fn new_static(data: &'static [u8]) -> Self {
        Self::from_storage(Storage::Static(data))
    }

    /// Creates a new dataref taking ownership of `data` (Skia's `MakeFromMalloc`).
    // Port of: src/core/SkData.cpp#L140-L143 (chrome/m156)
    #[doc(alias = "MakeFromMalloc")]
    #[must_use]
    pub fn new_from_vec(data: Vec<u8>) -> Self {
        Self::from_storage(Storage::Vec(data))
    }

    /// Creates a new dataref over the bytes of `owner`, which is dropped when the last reference
    /// to the `Data` goes away. This replaces Skia's `MakeWithProc(ptr, length, proc, ctx)`:
    /// the owner's `Drop` is the release proc.
    // Port of: src/core/SkData.cpp#L162-L165 (chrome/m156)
    #[doc(alias = "MakeWithProc")]
    pub fn new_with_owner<T: AsRef<[u8]> + Send + Sync + 'static>(owner: T) -> Self {
        Self::from_storage(Storage::Owner(Box::new(owner)))
    }

    /// Creates a new dataref holding the contents of a whole file, or `None` if it cannot be read
    /// (including when it is empty: Skia's mapping of an empty file fails). The file's position
    /// is left unchanged.
    ///
    /// Skia maps the file into memory (`MakeFromFILE` / `MakeFromFD`, which share this one
    /// Rust entry point because [`File`] is both); safe Rust reads it instead.
    // Port of: src/core/SkData.cpp#L173-L200 (chrome/m156)
    #[doc(alias = "MakeFromFILE")]
    #[doc(alias = "MakeFromFD")]
    #[must_use]
    pub fn new_from_file(file: &File) -> Option<Self> {
        let mut file = file;
        let position = file.stream_position().ok()?;
        file.seek(SeekFrom::Start(0)).ok()?;
        let mut contents = Vec::new();
        let read = file.read_to_end(&mut contents);
        file.seek(SeekFrom::Start(position)).ok()?;
        read.ok()?;
        if contents.is_empty() {
            return None;
        }
        Some(Self::new_from_vec(contents))
    }

    /// Creates a new dataref holding the file with the specified path, or `None` if the file
    /// cannot be opened.
    // Port of: src/core/SkData.cpp#L183-L191 (chrome/m156)
    #[doc(alias = "MakeFromFileName")]
    pub fn from_filename(path: impl AsRef<Path>) -> Option<Self> {
        let file = File::open(path).ok()?;
        Self::new_from_file(&file)
    }

    /// Attempts to read `size` bytes into a `Data`. Returns `None` if the read fails. Either way
    /// the stream's cursor may have been changed.
    // Port of: src/core/SkData.cpp#L215-L226 (chrome/m156)
    #[doc(alias = "MakeFromStream")]
    pub fn from_stream(stream: &mut dyn Stream, size: usize) -> Option<Self> {
        // reduce the chance of OOM by checking that the stream has enough bytes to read from
        // before allocating that potentially large buffer.
        if stream_priv::remaining_length_is_below(stream, size) {
            return None;
        }
        let mut data = Self::new_uninitialized(size);
        let buffer = data.writable_data()?;
        if stream.read(buffer) != size {
            return None;
        }
        Some(data)
    }

    /// Returns a data that is a reference to a subset of the original data. This never makes a
    /// deep copy of the contents, but retains a reference to the original data object.
    ///
    /// If `offset + length > self.size()`, returns `None`.
    // Port of: src/core/SkData.cpp#L74-L98 (chrome/m156)
    #[doc(alias = "shareSubset")]
    #[must_use]
    pub fn share_subset(&self, offset: usize, length: usize) -> Option<Self> {
        let size = self.size();
        if offset > size || length > size - offset {
            return None;
        }

        if offset == 0 && length == size {
            return Some(self.clone());
        }

        if length == 0 {
            return Some(Self::new_empty());
        }

        Some(Self::from_storage(Storage::Subset {
            parent: self.clone(),
            offset,
            length,
        }))
    }

    /// Attempts to create a deep copy of a range of the original data.
    ///
    /// If `offset + length > self.size()`, returns `None`.
    // Port of: src/core/SkData.cpp#L100-L104 (chrome/m156)
    #[doc(alias = "copySubset")]
    #[must_use]
    pub fn copy_subset(&self, offset: usize, length: usize) -> Option<Self> {
        let size = self.size();
        if offset > size || length > size - offset {
            return None;
        }
        Some(Self::new_copy(&self.as_bytes()[offset..offset + length]))
    }

    /// Like [`Data::share_subset`], but an out-of-range request gives an empty `Data` instead of
    /// `None` (Skia's deprecated `MakeSubset`).
    // Port of: include/core/SkData.h#L186-L191 (chrome/m156)
    #[doc(alias = "MakeSubset")]
    #[must_use]
    pub fn new_subset(data: &Data, offset: usize, length: usize) -> Self {
        data.share_subset(offset, length)
            .unwrap_or_else(Self::new_empty)
    }

    /// Copies a range of the data into a caller-provided buffer. Returns the actual number of
    /// bytes copied, after clamping `offset` and `length` to the size of this data. If `buffer`
    /// is `None`, only the computed number of bytes is returned.
    ///
    /// # Panics
    /// If `buffer` is provided but shorter than the number of bytes to copy.
    // Port of: src/core/SkData.cpp#L49-L64 (chrome/m156)
    #[doc(alias = "copyRange")]
    #[must_use]
    pub fn copy_range(&self, offset: usize, length: usize, buffer: Option<&mut [u8]>) -> usize {
        let mut available = self.size();
        if offset >= available || 0 == length {
            return 0;
        }
        available -= offset;
        let length = length.min(available);
        debug_assert!(length > 0);

        if let Some(buffer) = buffer {
            buffer[..length].copy_from_slice(&self.as_bytes()[offset..offset + length]);
        }
        length
    }

    /// Calls `==`, but first checks if `other` is `None` (in which case it returns false).
    // Port of: include/core/SkData.h#L38-L40 (chrome/m156)
    #[must_use]
    pub fn equals(&self, other: Option<&Data>) -> bool {
        other.is_some_and(|other| self == other)
    }

    /// Returns true if both arguments are the same size and contain the same bytes, or if both
    /// arguments are `None`.
    // Port of: include/core/SkData.h#L46-L48 (chrome/m156)
    #[doc(alias = "Equals")]
    #[must_use]
    pub fn equals_opt(a: Option<&Data>, b: Option<&Data>) -> bool {
        match a {
            None => b.is_none(),
            Some(a) => a.equals(b),
        }
    }
}
