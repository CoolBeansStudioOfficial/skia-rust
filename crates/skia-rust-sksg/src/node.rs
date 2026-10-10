// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGNode.h, modules/sksg/src/SkSGNode.cpp,
// modules/sksg/src/SkSGNodePriv.h (chrome/m156)
//
// `sksg::Node` is an `SkRefCnt` DAG whose nodes observe their descendants. An observer is a
// non-owning back pointer: Skia links it by raw pointer and unlinks it in the observer's
// destructor. Here the back pointer is a `Weak`, so a dropped observer is skipped during
// invalidation, which is the same behaviour without the unlink.
//
// Nodes are `Rc`s with `Cell`/`RefCell` state, not `Arc`s: the graph is mutable and
// single-threaded, as in Skia.

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::{Rc, Weak};

use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::Rect;

use crate::invalidation_controller::InvalidationController;

/// Traits of a node: how its damage propagates.
// Port of: modules/sksg/include/SkSGNode.h#L30-L37 (chrome/m156) (`Node::InvalTraits`)
pub mod inval_traits {
    /// Nodes with this trait never generate direct damage; the damage bubbles up to ancestors.
    pub const BUBBLE_DAMAGE: u32 = 1 << 0;
    /// Nodes with this trait obscure the descendants' damage and always override it.
    pub const OVERRIDE_DAMAGE: u32 = 1 << 1;
}

// Port of: modules/sksg/include/SkSGNode.h#L60-L73 (chrome/m156) (`Node::Flags`)
const INVALIDATED_FLAG: u32 = 1 << 0;
const DAMAGE_FLAG: u32 = 1 << 1;
const IN_TRAVERSAL_FLAG: u32 = 1 << 3;

/// The state every node carries (the `Node` base class fields).
// Port of: modules/sksg/include/SkSGNode.h#L75-L86 (chrome/m156)
#[derive(Debug)]
pub struct NodeCore {
    inval_traits: u32,
    /// The nodes that observe this one (their `fInvalObserver` / `fInvalObserverArray`).
    observers: RefCell<Vec<Weak<dyn Node>>>,
    /// This node's own weak reference, so it can register itself as an observer.
    self_ref: OnceCell<Weak<dyn Node>>,
    bounds: Cell<Rect>,
    flags: Cell<u32>,
    /// Flags for the subclasses (`fNodeFlags`).
    node_flags: Cell<u8>,
}

impl NodeCore {
    /// Port of `Node::Node(uint32_t invalTraits)`. The node starts invalidated, with large bounds.
    // Port of: modules/sksg/src/SkSGNode.cpp#L49-L55 (chrome/m156)
    #[must_use]
    pub fn new(inval_traits: u32, self_ref: Weak<dyn Node>) -> Self {
        let core = Self {
            inval_traits,
            observers: RefCell::new(Vec::new()),
            self_ref: OnceCell::new(),
            bounds: Cell::new(crate::util::make_large_s32()),
            flags: Cell::new(INVALIDATED_FLAG),
            node_flags: Cell::new(0),
        };
        let _ = core.self_ref.set(self_ref);
        core
    }

    /// True if the node or a descendant needs revalidation (`hasInval`).
    // Port of: modules/sksg/include/SkSGNode.h#L121-L121 (chrome/m156)
    #[must_use]
    pub fn has_inval(&self) -> bool {
        self.flags.get() & INVALIDATED_FLAG != 0
    }

    /// The last revalidated bounds (`bounds`). Valid only without pending invalidation.
    // Port of: modules/sksg/include/SkSGNode.h#L113-L118 (chrome/m156)
    #[must_use]
    pub fn bounds(&self) -> Rect {
        debug_assert!(!self.has_inval());
        self.bounds.get()
    }

    /// The subclass flags (`fNodeFlags`).
    #[must_use]
    pub fn node_flags(&self) -> u8 {
        self.node_flags.get()
    }

    /// Sets the subclass flags (`fNodeFlags`).
    pub fn set_node_flags(&self, flags: u8) {
        self.node_flags.set(flags);
    }
}

/// Sets a flag for the duration of a traversal, and clears it on drop unless it was already set.
// Port of: modules/sksg/src/SkSGNode.cpp#L23-L44 (chrome/m156) (`Node::ScopedFlag`)
struct ScopedFlag<'a> {
    core: &'a NodeCore,
    flag: u32,
    was_set: bool,
}

impl<'a> ScopedFlag<'a> {
    fn new(core: &'a NodeCore, flag: u32) -> Self {
        let was_set = core.flags.get() & flag != 0;
        core.flags.set(core.flags.get() | flag);
        Self {
            core,
            flag,
            was_set,
        }
    }

    fn was_set(&self) -> bool {
        self.was_set
    }
}

impl Drop for ScopedFlag<'_> {
    fn drop(&mut self) {
        if !self.was_set {
            self.core.flags.set(self.core.flags.get() & !self.flag);
        }
    }
}

/// The base of every sksg node: the state of [`NodeCore`] plus the `revalidate`/`invalidate`
/// traversals. Subclasses implement [`Node::on_revalidate`].
// Port of: modules/sksg/include/SkSGNode.h#L16-L58 (chrome/m156) (`class Node`)
#[doc(alias = "sksg::Node")]
pub trait Node: std::fmt::Debug + 'static {
    /// The base state of the node.
    fn core(&self) -> &NodeCore;

    /// Recomputes the cached properties of the node and returns its bounds in local coordinates.
    // Port of: modules/sksg/include/SkSGNode.h#L49-L51 (chrome/m156) (`Node::onRevalidate`)
    #[doc(alias = "onRevalidate")]
    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect;

    /// Traverses the DAG and revalidates the dependent or invalidated nodes. Returns the bounding
    /// box of the DAG fragment.
    // Port of: modules/sksg/src/SkSGNode.cpp#L118-L150 (chrome/m156) (`Node::revalidate`)
    fn revalidate(&self, mut ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        let core = self.core();
        let guard = ScopedFlag::new(core, IN_TRAVERSAL_FLAG);
        if guard.was_set() {
            return core.bounds.get();
        }
        if !core.has_inval() {
            return core.bounds.get();
        }

        let generate_damage = ic.is_some()
            && (core.flags.get() & DAMAGE_FLAG != 0
                || core.inval_traits & inval_traits::OVERRIDE_DAMAGE != 0);
        if generate_damage {
            // Revalidate and emit damage for old-bounds, new-bounds.
            let prev_bounds = core.bounds.get();
            let ic_override = if core.inval_traits & inval_traits::OVERRIDE_DAMAGE != 0 {
                None
            } else {
                ic.as_deref_mut()
            };
            let new_bounds = self.on_revalidate(ic_override, ctm);
            core.bounds.set(new_bounds);
            if let Some(ic) = ic {
                ic.inval(prev_bounds, ctm);
                if new_bounds != prev_bounds {
                    ic.inval(new_bounds, ctm);
                }
            }
        } else {
            // Trivial transitive revalidation.
            let new_bounds = self.on_revalidate(ic, ctm);
            core.bounds.set(new_bounds);
        }

        core.flags
            .set(core.flags.get() & !(INVALIDATED_FLAG | DAMAGE_FLAG));
        core.bounds.get()
    }

    /// Tags this node for invalidation and, if `damage` is set, for damage.
    // Port of: modules/sksg/include/SkSGNode.h#L23-L25 (chrome/m156) (`Node::invalidate`)
    fn invalidate(&self) {
        self.invalidate_bubbling(true);
    }

    /// `invalidate(bool damageBubbling)`: the recursive form of [`Node::invalidate`].
    // Port of: modules/sksg/src/SkSGNode.cpp#L100-L116 (chrome/m156) (`Node::invalidate`)
    #[doc(hidden)]
    fn invalidate_bubbling(&self, damage_bubbling: bool) {
        let core = self.core();
        let guard = ScopedFlag::new(core, IN_TRAVERSAL_FLAG);
        if guard.was_set() {
            return;
        }
        if core.has_inval() && (!damage_bubbling || core.flags.get() & DAMAGE_FLAG != 0) {
            // All done.
            return;
        }
        let mut damage_bubbling = damage_bubbling;
        if damage_bubbling && core.inval_traits & inval_traits::BUBBLE_DAMAGE == 0 {
            // Found a damage observer.
            core.flags.set(core.flags.get() | DAMAGE_FLAG);
            damage_bubbling = false;
        }
        core.flags.set(core.flags.get() | INVALIDATED_FLAG);
        // Copy the observers first: an observer may run code that changes the list.
        let observers: Vec<Weak<dyn Node>> = core.observers.borrow().clone();
        for observer in observers {
            if let Some(observer) = observer.upgrade() {
                observer.invalidate_bubbling(damage_bubbling);
            }
        }
    }

    /// Registers `self` to receive invalidation events from `child` (`observeInval`).
    // Port of: modules/sksg/src/SkSGNode.cpp#L63-L84 (chrome/m156) (`Node::observeInval`)
    fn observe_inval(&self, child: &dyn Node) {
        let observer = self
            .core()
            .self_ref
            .get()
            .expect("node has a self reference")
            .clone();
        let child_core = child.core();
        let mut observers = child_core.observers.borrow_mut();
        // No duplicate observers.
        debug_assert!(!observers.iter().any(|o| Weak::ptr_eq(o, &observer)));
        observers.push(observer);
    }

    /// Unregisters `self` from `child` (`unobserveInval`).
    // Port of: modules/sksg/src/SkSGNode.cpp#L86-L98 (chrome/m156) (`Node::unobserveInval`)
    fn unobserve_inval(&self, child: &dyn Node) {
        let observer = self
            .core()
            .self_ref
            .get()
            .expect("node has a self reference");
        child
            .core()
            .observers
            .borrow_mut()
            .retain(|o| !Weak::ptr_eq(o, observer));
    }
}

/// Builds the self reference a node's [`NodeCore`] keeps, for use inside `Rc::new_cyclic`.
#[must_use]
pub fn self_ref<T: Node>(weak: &Weak<T>) -> Weak<dyn Node> {
    weak.clone()
}

/// `SkSGNodePriv::HasInval`: true if the node needs revalidation.
// Port of: modules/sksg/src/SkSGNodePriv.h#L14-L17 (chrome/m156)
#[must_use]
pub fn has_inval(node: &Rc<dyn Node>) -> bool {
    node.core().has_inval()
}
