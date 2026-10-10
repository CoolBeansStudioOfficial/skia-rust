// Copyright © 2007,2008,2009,2010  Red Hat, Inc.
// Copyright © 2012,2018  Google, Inc.
// Copyright © 2019  Facebook, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-serialize.hh (harfbuzz 9cb1fee5)

//! The object serializer of `HarfBuzz`: objects are built front to back, packed (and shared when
//! their bytes and links are equal) from the back of the buffer towards the front, and their
//! offsets are resolved when the root object is finished. The resulting layout, not only the
//! table contents, decides the output bytes, so it is ported rather than replaced.
//!
//! Where `HarfBuzz` keeps every object in one flat buffer (`head` .. `tail`), each object here has
//! its own `Vec<u8>`. `HarfBuzz` only ever builds the innermost object at `head`, so the layouts are
//! the same: the root object comes first in the output, followed by the packed objects from the
//! last packed to the first packed.

// The serializer API is ported whole; the layout tables use the parts the small tables do not.
#![allow(dead_code)]

use std::collections::HashMap;

/// Port of `hb_serialize_error_t` (hb-serialize.hh#L48-L56).
pub(crate) const ERROR_OTHER: u32 = 0x1;
pub(crate) const ERROR_OFFSET_OVERFLOW: u32 = 0x2;
pub(crate) const ERROR_INT_OVERFLOW: u32 = 0x8;

/// Index of a packed object. Index 0 is the nil object (`HarfBuzz`'s `objidx_t`).
pub(crate) type ObjIdx = usize;

/// Port of `hb_serialize_context_t::whence_t` (hb-serialize.hh#L67-L71).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Whence {
    /// Relative to the head of the object that holds the offset.
    Head,
    /// Relative to the tail of the object that holds the offset.
    Tail,
    /// From the start of the serialized table.
    Absolute,
}

/// Port of `hb_serialize_context_t::object_t::link_t` (hb-serialize.hh#L134-L162).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Link {
    pub width: u8,
    pub is_signed: bool,
    pub whence: Whence,
    pub bias: u32,
    pub position: u32,
    pub objidx: ObjIdx,
}

/// Port of `hb_serialize_context_t::object_t` (hb-serialize.hh#L74-L180).
#[derive(Clone, Default, Debug)]
struct Object {
    data: Vec<u8>,
    real_links: Vec<Link>,
    virtual_links: Vec<Link>,
}

/// Port of `hb_serialize_context_t::snapshot_t` (hb-serialize.hh#L182-L190).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Snapshot {
    head: usize,
    packed_len: usize,
    tail_bytes: usize,
    num_real_links: usize,
    num_virtual_links: usize,
    errors: u32,
    depth: usize,
}

/// Port of `hb_serialize_context_t` (hb-serialize.hh#L58-L810).
#[derive(Default, Debug)]
pub(crate) struct Serializer {
    /// The stack of objects under construction (`current` and its `next` chain).
    stack: Vec<Object>,
    /// `packed`: object 0 is the nil object.
    packed: Vec<Option<Object>>,
    /// `packed_map`.
    packed_map: HashMap<(Vec<u8>, Vec<Link>), ObjIdx>,
    /// `end - tail`: the bytes of all packed objects.
    tail_bytes: usize,
    errors: u32,
    /// The bytes of the finished table (`copy_bytes`).
    output: Vec<u8>,
}

impl Serializer {
    /// Port of the constructor and `reset()` (hb-serialize.hh#L196-L260).
    pub(crate) fn new() -> Self {
        Serializer {
            packed: vec![None],
            ..Default::default()
        }
    }

    pub(crate) fn in_error(&self) -> bool {
        self.errors != 0
    }

    pub(crate) fn offset_overflow(&self) -> bool {
        self.errors & ERROR_OFFSET_OVERFLOW != 0
    }

    pub(crate) fn only_overflow(&self) -> bool {
        self.errors == ERROR_OFFSET_OVERFLOW || self.errors == ERROR_INT_OVERFLOW
    }

    /// Port of `err()` (hb-serialize.hh#L460-L462). Always returns `false`.
    pub(crate) fn err(&mut self, err_type: u32) -> bool {
        self.errors |= err_type;
        false
    }

    /// Port of `check_success()`.
    pub(crate) fn check_success(&mut self, success: bool) -> bool {
        self.errors == 0 && (success || self.err(ERROR_OTHER))
    }

    /// Port of `check_assign()` for a field of `bits` bits: an error when `value` does not fit.
    pub(crate) fn check_fits(&mut self, value: u64, bits: u32, err_type: u32) -> bool {
        if value >> bits != 0 {
            self.err(err_type)
        } else {
            true
        }
    }

    /// Port of `start_serialize()` (hb-serialize.hh#L289-L300).
    pub(crate) fn start_serialize(&mut self) {
        assert!(self.stack.is_empty());
        self.push();
    }

    /// Port of `push()` (hb-serialize.hh#L340-L358).
    pub(crate) fn push(&mut self) {
        if self.in_error() {
            return;
        }
        self.stack.push(Object::default());
    }

    fn cur(&mut self) -> &mut Object {
        self.stack.last_mut().expect("no current object")
    }

    /// Port of `length()` (hb-serialize.hh#L525-L529): the bytes of the current object.
    pub(crate) fn length(&self) -> usize {
        self.stack.last().map_or(0, |o| o.data.len())
    }

    /// Port of `snapshot()` (hb-serialize.hh#L192-L198).
    pub(crate) fn snapshot(&self) -> Snapshot {
        let cur = self.stack.last();
        Snapshot {
            head: cur.map_or(0, |o| o.data.len()),
            packed_len: self.packed.len(),
            tail_bytes: self.tail_bytes,
            num_real_links: cur.map_or(0, |o| o.real_links.len()),
            num_virtual_links: cur.map_or(0, |o| o.virtual_links.len()),
            errors: self.errors,
            depth: self.stack.len(),
        }
    }

    /// Port of `revert(snapshot_t)` and `discard_stale_objects()` (hb-serialize.hh#L424-L470).
    pub(crate) fn revert(&mut self, snap: Snapshot) {
        // Overflows that happened after the snapshot will be erased by the revert.
        if self.in_error() && !self.only_overflow() {
            return;
        }
        assert_eq!(snap.depth, self.stack.len());
        if let Some(cur) = self.stack.last_mut() {
            cur.real_links.truncate(snap.num_real_links);
            cur.virtual_links.truncate(snap.num_virtual_links);
            cur.data.truncate(snap.head);
        }
        self.errors = snap.errors;
        if self.in_error() {
            return;
        }
        while self.packed.len() > snap.packed_len {
            let obj = self.packed.pop().flatten().expect("packed object");
            self.packed_map.remove(&(obj.data, obj.real_links));
        }
        self.tail_bytes = snap.tail_bytes;
    }

    /// Port of `pop_discard()` (hb-serialize.hh#L360-L376).
    pub(crate) fn pop_discard(&mut self) {
        if self.stack.is_empty() {
            return;
        }
        if self.in_error() && !self.only_overflow() {
            return;
        }
        self.stack.pop();
    }

    /// Port of `pop_pack()` (hb-serialize.hh#L378-L440). Returns 0 for an empty object.
    pub(crate) fn pop_pack(&mut self, share: bool) -> ObjIdx {
        if self.stack.is_empty() {
            return 0;
        }
        if self.in_error() && !self.only_overflow() {
            return 0;
        }
        let obj = self.stack.pop().expect("object");
        let len = obj.data.len();
        if len == 0 {
            debug_assert!(obj.real_links.is_empty() && obj.virtual_links.is_empty());
            return 0;
        }
        if share {
            let key = (obj.data, obj.real_links);
            if let Some(&objidx) = self.packed_map.get(&key) {
                // `merge_virtual_links`
                let to = self.packed[objidx].as_mut().expect("packed object");
                to.virtual_links.extend(obj.virtual_links);
                return objidx;
            }
            let objidx = self.packed.len();
            self.packed_map.insert(key.clone(), objidx);
            self.packed.push(Some(Object {
                data: key.0,
                real_links: key.1,
                virtual_links: obj.virtual_links,
            }));
            self.tail_bytes += len;
            return objidx;
        }
        self.tail_bytes += len;
        self.packed.push(Some(obj));
        self.packed.len() - 1
    }

    /// Port of `allocate_size(size, clear = true)`: appends `size` zero bytes and returns their
    /// position in the current object.
    pub(crate) fn allocate(&mut self, size: usize) -> usize {
        let cur = self.cur();
        let pos = cur.data.len();
        cur.data.resize(pos + size, 0);
        pos
    }

    /// Port of `embed(const char *, unsigned)`: appends the bytes and returns their position.
    pub(crate) fn embed(&mut self, bytes: &[u8]) -> usize {
        let cur = self.cur();
        let pos = cur.data.len();
        cur.data.extend_from_slice(bytes);
        pos
    }

    pub(crate) fn embed_u8(&mut self, v: u8) -> usize {
        self.embed(&[v])
    }

    pub(crate) fn embed_u16(&mut self, v: u16) -> usize {
        self.embed(&v.to_be_bytes())
    }

    pub(crate) fn embed_u32(&mut self, v: u32) -> usize {
        self.embed(&v.to_be_bytes())
    }

    /// Port of `align()` (hb-serialize.hh#L531-L536).
    pub(crate) fn align(&mut self, alignment: usize) {
        let l = self.length() % alignment;
        if l != 0 {
            self.allocate(alignment - l);
        }
    }

    /// The bytes of the current object, to patch a field in place.
    pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.cur().data
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.stack.last().expect("no current object").data
    }

    pub(crate) fn set_u16(&mut self, pos: usize, v: u16) {
        self.cur().data[pos..pos + 2].copy_from_slice(&v.to_be_bytes());
    }

    pub(crate) fn set_u32(&mut self, pos: usize, v: u32) {
        self.cur().data[pos..pos + 4].copy_from_slice(&v.to_be_bytes());
    }

    /// Port of `add_link()` (hb-serialize.hh#L511-L545). `pos` is the position of the offset
    /// field in the current object.
    pub(crate) fn add_link(
        &mut self,
        pos: usize,
        width: u8,
        objidx: ObjIdx,
        whence: Whence,
        bias: u32,
    ) {
        if self.in_error() || objidx == 0 {
            return;
        }
        self.cur().real_links.push(Link {
            width,
            is_signed: false,
            whence,
            bias,
            position: pos as u32,
            objidx,
        });
    }

    /// Port of `add_virtual_link()` (hb-serialize.hh#L465-L478).
    pub(crate) fn add_virtual_link(&mut self, objidx: ObjIdx) {
        if self.in_error() || objidx == 0 {
            return;
        }
        self.cur().virtual_links.push(Link {
            width: 0,
            is_signed: false,
            whence: Whence::Head,
            bias: 0,
            position: 0,
            objidx,
        });
    }

    /// `end - tail` of `HarfBuzz`: the total size of the packed objects.
    pub(crate) fn tail_bytes(&self) -> usize {
        self.tail_bytes
    }

    /// Port of `end_serialize()` (hb-serialize.hh#L302-L330) followed by `copy_bytes()`
    /// (hb-serialize.hh#L715-L730). Returns `None` on an error. An offset overflow also returns
    /// `None`: `HarfBuzz` would then run the repacker (`hb-repacker.hh`), which is not ported.
    pub(crate) fn end_serialize(&mut self) -> Option<Vec<u8>> {
        if self.in_error() {
            return None;
        }
        assert_eq!(self.stack.len(), 1);
        // Only "pack" if there exist other objects.
        if self.packed.len() <= 1 {
            return self.stack.pop().map(|o| o.data);
        }
        let root = self.pop_pack(false);
        self.resolve_links(root)
    }

    /// Port of `resolve_links()` (hb-serialize.hh#L480-L525), over the final layout: packed
    /// objects from the last to the first.
    fn resolve_links(&mut self, root: ObjIdx) -> Option<Vec<u8>> {
        let n = self.packed.len();
        let mut start = vec![0usize; n];
        let mut pos = 0usize;
        for idx in (1..n).rev() {
            start[idx] = pos;
            pos += self.packed[idx].as_ref().expect("packed").data.len();
        }
        debug_assert_eq!(root, n - 1);
        let mut out = Vec::with_capacity(pos);
        for idx in (1..n).rev() {
            out.extend_from_slice(&self.packed[idx].as_ref().expect("packed").data);
        }
        for parent_idx in 1..n {
            let parent = self.packed[parent_idx].as_ref().expect("packed");
            let parent_len = parent.data.len();
            for link in &parent.real_links {
                if link.objidx == 0 || link.objidx >= n {
                    self.errors |= ERROR_OTHER;
                    return None;
                }
                let child_start = start[link.objidx];
                let offset: usize = match link.whence {
                    Whence::Head => child_start.wrapping_sub(start[parent_idx]),
                    Whence::Tail => child_start.wrapping_sub(start[parent_idx] + parent_len),
                    Whence::Absolute => child_start,
                };
                let offset = offset.wrapping_sub(link.bias as usize);
                let p = start[parent_idx] + link.position as usize;
                let (width, signed) = (link.width, link.is_signed);
                let fits = match (width, signed) {
                    (2, false) => offset <= 0xFFFF,
                    (2, true) => offset <= 0x7FFF,
                    (3, false) => offset <= 0xFF_FFFF,
                    (4, false) => offset <= 0xFFFF_FFFF,
                    (4, true) => offset <= 0x7FFF_FFFF,
                    _ => false,
                };
                if !fits {
                    // `check_assign (off, offset, HB_SERIALIZE_ERROR_OFFSET_OVERFLOW)`
                    self.errors |= ERROR_OFFSET_OVERFLOW;
                    return None;
                }
                let bytes = (offset as u32).to_be_bytes();
                out[p..p + usize::from(width)].copy_from_slice(&bytes[4 - usize::from(width)..]);
            }
        }
        Some(out)
    }
}
