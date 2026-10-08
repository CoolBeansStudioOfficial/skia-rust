// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/DescriptorTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::descriptor::{Descriptor, EFFECTS_TAG, REC_TAG};
use skia_rust_core::font_types::set_four_byte_tag;
use skia_rust_core::scaler_context::{SCALER_CONTEXT_REC_SIZE, ScalerContextRec};

use crate::{def_test, reporter_assert};

/// Converts a test size to the `u32` header field.
fn to_u32(n: usize) -> u32 {
    u32::try_from(n).expect("test sizes fit in u32")
}

// Port of: tests/DescriptorTest.cpp#L29-L35 (chrome/m156)
def_test!(Descriptor_empty, |reporter| {
    let size = Descriptor::compute_overhead(0);
    let desc = Descriptor::alloc(size);
    reporter_assert!(reporter, desc.is_valid());
    reporter_assert!(reporter, desc.length() == to_u32(size));
});

// Port of: tests/DescriptorTest.cpp#L37-L49 (chrome/m156)
def_test!(Descriptor_valid_simple, |reporter| {
    let size = Descriptor::compute_overhead(1) + SCALER_CONTEXT_REC_SIZE;
    let mut desc = Descriptor::alloc(size);
    let rec = ScalerContextRec::default();
    desc.add_entry(REC_TAG, SCALER_CONTEXT_REC_SIZE, Some(&rec.to_bytes()[..]));
    reporter_assert!(reporter, desc.is_valid());
    reporter_assert!(reporter, desc.length() == to_u32(size));

    desc.set_length_for_testing(to_u32(size - 4));
    reporter_assert!(reporter, !desc.is_valid());
});

// Port of: tests/DescriptorTest.cpp#L51-L64 (chrome/m156)
def_test!(Descriptor_valid_simple_extra_space, |reporter| {
    let extra_space = 100;
    let size = Descriptor::compute_overhead(1) + SCALER_CONTEXT_REC_SIZE;
    let mut desc = Descriptor::alloc(size + extra_space);
    let rec = ScalerContextRec::default();
    desc.add_entry(REC_TAG, SCALER_CONTEXT_REC_SIZE, Some(&rec.to_bytes()[..]));
    reporter_assert!(reporter, desc.is_valid());
    reporter_assert!(reporter, desc.length() == to_u32(size));

    desc.set_length_for_testing(to_u32(size - 4));
    reporter_assert!(reporter, !desc.is_valid());
});

// Port of: tests/DescriptorTest.cpp#L66-L82 (chrome/m156)
def_test!(Descriptor_valid_more_tags, |reporter| {
    let effect_size = 16;
    let test_size = 32;
    let size = Descriptor::compute_overhead(3) + SCALER_CONTEXT_REC_SIZE + effect_size + test_size;
    let mut desc = Descriptor::alloc(size);
    let rec = ScalerContextRec::default();
    desc.add_entry(REC_TAG, SCALER_CONTEXT_REC_SIZE, Some(&rec.to_bytes()[..]));
    desc.add_entry(EFFECTS_TAG, effect_size, None);
    desc.add_entry(set_four_byte_tag(b't', b'e', b's', b't'), test_size, None);
    reporter_assert!(reporter, desc.is_valid());
    reporter_assert!(reporter, desc.length() == to_u32(size));

    desc.set_length_for_testing(to_u32(size - 4));
    reporter_assert!(reporter, !desc.is_valid());
});

// Port of: tests/DescriptorTest.cpp#L84-L93 (chrome/m156)
def_test!(Descriptor_invalid_rec_size, |reporter| {
    let size = Descriptor::compute_overhead(1) + SCALER_CONTEXT_REC_SIZE - 4;
    let mut desc = Descriptor::alloc(size);
    let rec = ScalerContextRec::default().to_bytes();
    desc.add_entry(
        REC_TAG,
        SCALER_CONTEXT_REC_SIZE - 4,
        Some(&rec[..SCALER_CONTEXT_REC_SIZE - 4]),
    );
    reporter_assert!(reporter, desc.length() == to_u32(size));
    reporter_assert!(reporter, !desc.is_valid());
});

// Port of: tests/DescriptorTest.cpp#L95-L107 (chrome/m156)
def_test!(Descriptor_invalid_length, |reporter| {
    let size = Descriptor::compute_overhead(1);
    let effect_size = 1000;
    let mut desc = Descriptor::alloc(size);
    desc.add_entry(EFFECTS_TAG, effect_size, None);
    desc.set_length_for_testing(to_u32(size));
    reporter_assert!(reporter, !desc.is_valid());
    desc.set_length_for_testing(to_u32(size + effect_size));
    reporter_assert!(reporter, desc.is_valid());
});

// Port of: tests/DescriptorTest.cpp#L109-L125 (chrome/m156)
def_test!(Descriptor_entry_too_big, |reporter| {
    let size = Descriptor::compute_overhead(1) + 4;
    // Must be less than fLength, but big enough to be bigger then fLength when added.
    let effect_size = Descriptor::compute_overhead(1);
    let mut desc = Descriptor::alloc(size);
    desc.add_entry(EFFECTS_TAG, effect_size, None);
    desc.set_length_for_testing(to_u32(size));
    desc.set_count_for_testing(2);
    reporter_assert!(reporter, !desc.is_valid());
    desc.set_length_for_testing(to_u32(size));
    desc.set_count_for_testing(1);
    reporter_assert!(reporter, !desc.is_valid());
});

// Port of: tests/DescriptorTest.cpp#L127-L138 (chrome/m156)
def_test!(Descriptor_entry_over_end, |reporter| {
    let mut desc = Descriptor::alloc(36);
    // Make the start of the Entry be in the SkDescriptor, but the second half falls out side the
    // SkDescriptor. So: 12 (for descriptor) + 8 (for entry) + 12 (for entry length) = 32. An
    // An Entry is 8 bytes, so 4 bytes are < 36 and 4 bytes > 36.
    desc.add_entry(EFFECTS_TAG, 12, None);
    desc.set_length_for_testing(36);
    desc.set_count_for_testing(2);
    reporter_assert!(reporter, !desc.is_valid());
});
