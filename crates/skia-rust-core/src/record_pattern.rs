// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRecordPattern.h

//! `SkRecordPattern`: patterns over the commands of a [`Record`], for the peephole optimizations
//! of [`record_opts`](crate::record_opts).
//!
//! First, some matchers. These match a single command in the record, and may hang onto some data
//! from it. skia-rust: Skia matchers keep a pointer to the command; here a matcher keeps the
//! command's *index* in the record (`index()`), and the command is read back with
//! [`Record::get`] or [`Record::mutate`]. A pattern is a tuple of matchers:
//! `Pattern<(Is<Save>, Greedy<Is<ClipRect>>, Is<Restore>)>`.

use std::marker::PhantomData;

use crate::record::Record;
use crate::records::{Command, RecordKind, tags};

/// A matcher of a single command (the functors `SkRecords::Is`, `IsDraw`, ... implement).
pub trait Matcher: Default {
    /// True if `command` (the one at `index` in the record) matches.
    fn matches(&mut self, command: &mut Command, index: usize) -> bool;

    /// The index of the command this matcher stored, if it matched one.
    fn index(&self) -> Option<usize>;
}

/// Matches a command of type `T`, and stores that command (`SkRecords::Is`).
// Port of: src/core/SkRecordPattern.h#L18-L39 (chrome/m156)
#[doc(alias = "SkRecords::Is")]
#[derive(Debug)]
pub struct Is<T: RecordKind> {
    index: Option<usize>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: RecordKind> Default for Is<T> {
    fn default() -> Self {
        Is {
            index: None,
            _marker: PhantomData,
        }
    }
}

impl<T: RecordKind> Matcher for Is<T> {
    fn matches(&mut self, command: &mut Command, index: usize) -> bool {
        if T::from_command(command).is_some() {
            self.index = Some(index);
            true
        } else {
            self.index = None;
            false
        }
    }

    fn index(&self) -> Option<usize> {
        self.index
    }
}

/// Matches any command that draws, and stores its paint (`SkRecords::IsDraw`). The paint is the
/// one of the command at [`index`](Matcher::index) ([`Command::paint_mut`]; `None` for a draw
/// without a paint).
// Port of: src/core/SkRecordPattern.h#L41-L68 (chrome/m156)
#[doc(alias = "SkRecords::IsDraw")]
#[derive(Debug, Default)]
pub struct IsDraw {
    index: Option<usize>,
}

impl Matcher for IsDraw {
    fn matches(&mut self, command: &mut Command, index: usize) -> bool {
        if command.tags() & tags::DRAW != 0 {
            self.index = Some(index);
            true
        } else {
            self.index = None;
            false
        }
    }

    fn index(&self) -> Option<usize> {
        self.index
    }
}

/// Matches any command that draws *once* (logically), and stores its paint
/// (`SkRecords::IsSingleDraw`).
// Port of: src/core/SkRecordPattern.h#L70-L103 (chrome/m156)
#[doc(alias = "SkRecords::IsSingleDraw")]
#[derive(Debug, Default)]
pub struct IsSingleDraw {
    index: Option<usize>,
}

impl Matcher for IsSingleDraw {
    fn matches(&mut self, command: &mut Command, index: usize) -> bool {
        let tags = command.tags();
        if tags & tags::DRAW != 0 && tags & tags::MULTI_DRAW == 0 {
            self.index = Some(index);
            true
        } else {
            self.index = None;
            false
        }
    }

    fn index(&self) -> Option<usize> {
        self.index
    }
}

/// Matches if `M` doesn't. Stores nothing (`SkRecords::Not`).
// Port of: src/core/SkRecordPattern.h#L105-L110 (chrome/m156)
#[doc(alias = "SkRecords::Not")]
#[derive(Debug)]
pub struct Not<M: Matcher>(PhantomData<fn() -> M>);

impl<M: Matcher> Default for Not<M> {
    fn default() -> Self {
        Not(PhantomData)
    }
}

impl<M: Matcher> Matcher for Not<M> {
    fn matches(&mut self, command: &mut Command, index: usize) -> bool {
        !M::default().matches(command, index)
    }

    fn index(&self) -> Option<usize> {
        None
    }
}

/// Matches if any of the matchers of the tuple `Ms` does. Stores nothing (`SkRecords::Or`).
// Port of: src/core/SkRecordPattern.h#L112-L121 (chrome/m156)
#[doc(alias = "SkRecords::Or")]
#[derive(Debug)]
pub struct Or<Ms>(PhantomData<fn() -> Ms>);

impl<Ms> Default for Or<Ms> {
    fn default() -> Self {
        Or(PhantomData)
    }
}

macro_rules! impl_or {
    ($($m:ident),+) => {
        impl<$($m: Matcher),+> Matcher for Or<($($m,)+)> {
            fn matches(&mut self, command: &mut Command, index: usize) -> bool {
                $( if $m::default().matches(command, index) { return true; } )+
                false
            }

            fn index(&self) -> Option<usize> {
                None
            }
        }
    };
}
impl_or!(A);
impl_or!(A, B);
impl_or!(A, B, C);
impl_or!(A, B, C, D);

/// A special matcher that greedily matches `M` 0 or more times. Stores nothing
/// (`SkRecords::Greedy`).
// Port of: src/core/SkRecordPattern.h#L124-L128 (chrome/m156)
#[doc(alias = "SkRecords::Greedy")]
#[derive(Debug)]
pub struct Greedy<M: Matcher>(M);

impl<M: Matcher> Default for Greedy<M> {
    fn default() -> Self {
        Greedy(M::default())
    }
}

/// One element of a pattern: a matcher, or a [`Greedy`] one.
pub trait PatternElement: Default {
    /// Tries to match at `i`; returns the index just past the match, or 0 (`matchFirst`).
    fn match_first(&mut self, record: &mut Record, i: usize) -> usize;

    /// The index of the command this element stored (`get`).
    fn stored_index(&self) -> Option<usize>;
}

// If first isn't a Greedy, try to match at i once.
// Port of: src/core/SkRecordPattern.h#L190-L197 (chrome/m156)
impl<M: Matcher> PatternElement for M {
    fn match_first(&mut self, record: &mut Record, i: usize) -> usize {
        if i < record.count() && record.mutate(i, |command| self.matches(command, i)) {
            return i + 1;
        }
        0
    }

    fn stored_index(&self) -> Option<usize> {
        self.index()
    }
}

// If first is a Greedy, walk i until it doesn't match.
// Port of: src/core/SkRecordPattern.h#L199-L209 (chrome/m156)
impl<M: Matcher> PatternElement for Greedy<M> {
    fn match_first(&mut self, record: &mut Record, mut i: usize) -> usize {
        while i < record.count() {
            if !record.mutate(i, |command| self.0.matches(command, i)) {
                return i;
            }
            i += 1;
        }
        0
    }

    fn stored_index(&self) -> Option<usize> {
        None
    }
}

/// The matchers of a [`Pattern`]: a tuple of [`PatternElement`]s.
pub trait PatternElements: Default {
    /// Matches each element in order from `i`; returns the index just past the end, or 0.
    fn match_all(&mut self, record: &mut Record, i: usize) -> usize;

    /// The index stored by the `n`-th element (0 is `first`).
    fn index_at(&self, n: usize) -> Option<usize>;
}

macro_rules! impl_elements {
    ($(($idx:tt, $e:ident)),+) => {
        impl<$($e: PatternElement),+> PatternElements for ($($e,)+) {
            fn match_all(&mut self, record: &mut Record, mut i: usize) -> usize {
                $(
                    i = self.$idx.match_first(record, i);
                    if i == 0 {
                        return 0;
                    }
                )+
                i
            }

            fn index_at(&self, n: usize) -> Option<usize> {
                match n {
                    $($idx => self.$idx.stored_index(),)+
                    _ => None,
                }
            }
        }
    };
}
impl_elements!((0, A));
impl_elements!((0, A), (1, B));
impl_elements!((0, A), (1, B), (2, C));
impl_elements!((0, A), (1, B), (2, C), (3, D));
impl_elements!((0, A), (1, B), (2, C), (3, D), (4, E));
impl_elements!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F));
impl_elements!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G));

/// Pattern matches each of its matchers in order (`SkRecords::Pattern`).
///
/// This is the main entry point to pattern matching, and so provides a couple of extra API bits:
///  - [`search`](Pattern::search) scans through the record to look for matches;
///  - `first`, `second`, `third`, ... return the data stored by their respective matchers in the
///    pattern.
// Port of: src/core/SkRecordPattern.h#L130-L213 (chrome/m156)
#[doc(alias = "SkRecords::Pattern")]
#[derive(Debug, Default)]
pub struct Pattern<Ms: PatternElements> {
    elements: Ms,
}

impl<Ms: PatternElements> Pattern<Ms> {
    /// If this pattern matches the record starting from `i`, returns the index just past the end
    /// of the pattern, otherwise returns 0 (`match`).
    #[doc(alias = "match")]
    pub fn match_at(&mut self, record: &mut Record, i: usize) -> usize {
        self.elements.match_all(record, i)
    }

    /// Starting from `*end`, walks through the record to find the first span matching this
    /// pattern. If there is no such span, returns false. If there is, returns true and sets
    /// `[*begin, *end)`.
    pub fn search(&mut self, record: &mut Record, begin: &mut usize, end: &mut usize) -> bool {
        *begin = *end;
        while *begin < record.count() {
            *end = self.match_at(record, *begin);
            if *end != 0 {
                return true;
            }
            *begin += 1;
        }
        false
    }

    /// The index of the command stored by the first matcher.
    #[must_use]
    pub fn first_index(&self) -> Option<usize> {
        self.elements.index_at(0)
    }

    /// The index of the command stored by the second matcher.
    #[must_use]
    pub fn second_index(&self) -> Option<usize> {
        self.elements.index_at(1)
    }

    /// The index of the command stored by the third matcher.
    #[must_use]
    pub fn third_index(&self) -> Option<usize> {
        self.elements.index_at(2)
    }

    /// The index of the command stored by the fourth matcher.
    #[must_use]
    pub fn fourth_index(&self) -> Option<usize> {
        self.elements.index_at(3)
    }

    /// The command stored by the first matcher, if it is a `T` (`first<T>`).
    #[must_use]
    pub fn first<'r, T: RecordKind>(&self, record: &'r Record) -> Option<&'r T> {
        self.first_index()
            .and_then(|i| T::from_command(record.get(i)))
    }

    /// The command stored by the second matcher, if it is a `T` (`second<T>`).
    #[must_use]
    pub fn second<'r, T: RecordKind>(&self, record: &'r Record) -> Option<&'r T> {
        self.second_index()
            .and_then(|i| T::from_command(record.get(i)))
    }

    /// The command stored by the third matcher, if it is a `T` (`third<T>`).
    #[must_use]
    pub fn third<'r, T: RecordKind>(&self, record: &'r Record) -> Option<&'r T> {
        self.third_index()
            .and_then(|i| T::from_command(record.get(i)))
    }

    /// The command stored by the fourth matcher, if it is a `T` (`fourth<T>`).
    #[must_use]
    pub fn fourth<'r, T: RecordKind>(&self, record: &'r Record) -> Option<&'r T> {
        self.fourth_index()
            .and_then(|i| T::from_command(record.get(i)))
    }
}
