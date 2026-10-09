//! Sanity checks for the `src/gpu` helpers ported in G1 part 2. These are not 1:1 ports of Skia
//! tests (none of Skia's tests for these files can run without a GPU); they pin the packing,
//! hashing and encoding rules that the ports must keep.

use skia_rust_core::color_type::ColorType;
use skia_rust_gpu::gpu::blur_utils::{compute_integral_table_width, create_circle_profile};
use skia_rust_gpu::gpu::buffer_writer::{BufferWriter, VertexWriter};
use skia_rust_gpu::gpu::dither_utils::{dither_range_for_config, make_dither_lut};
use skia_rust_gpu::gpu::gradient_bitmap::encode_gradient_stop_to_half;
use skia_rust_gpu::gpu::key_builder::{KeyBuilder, StringKeyBuilder};
use skia_rust_gpu::gpu::resource_key::{
    ScratchKey, ScratchKeyBuilder, UniqueKey, UniqueKeyBuilder, resource_key_hash,
};

#[test]
fn key_builder_packs_lsb_first_and_flushes() {
    let mut data = Vec::new();
    {
        let mut kb = KeyBuilder::new(&mut data);
        kb.add_bits(3, 0b101, "a");
        kb.add_bool(true, "b");
        // Fills bits 4..31 of the first word exactly.
        kb.add_bits(27, 0x0012_3456, "c");
        // Starts at bit 31: its low bit lands in the first word, the rest in the second.
        kb.add_bits(12, 0xabd, "d");
        kb.flush();
    }
    let expected0 = 0b101 | (1 << 3) | (0x0012_3456_u32 << 4) | (1 << 31);
    let expected1 = 0xabd >> 1;
    assert_eq!(data, vec![expected0, expected1]);
}

#[test]
fn string_key_builder_describes_fields() {
    let mut data = Vec::new();
    let description;
    {
        let mut kb = StringKeyBuilder::new(&mut data);
        kb.append_comment("header");
        kb.add32(7, "seven");
        kb.add_bytes(&[1, 2], "bytes");
        kb.flush();
        description = kb.description().to_string();
    }
    assert_eq!(description, "header\nseven: 7\nbytes: 1\nbytes: 2\n");
    assert_eq!(data[0], 7);
}

#[test]
fn scratch_key_hash_covers_domain_and_data() {
    let ty = ScratchKey::generate_resource_type();
    let mut key = ScratchKey::new();
    {
        let mut b = ScratchKeyBuilder::new(&mut key, ty, 2);
        b[0] = 0xdead_beef;
        b[1] = 42;
    }
    assert!(key.is_valid());
    assert_eq!(key.resource_type(), ty);
    assert_eq!(key.size(), 4 * (2 + 2));
    // The hash is over every word after the hash word.
    assert_ne!(key.hash(), 0);
    let mut same = ScratchKey::new();
    {
        let mut b = ScratchKeyBuilder::new(&mut same, ty, 2);
        b[0] = 0xdead_beef;
        b[1] = 42;
    }
    assert_eq!(key, same);
    let mut other = ScratchKey::new();
    {
        let mut b = ScratchKeyBuilder::new(&mut other, ty, 2);
        b[0] = 0xdead_beef;
        b[1] = 43;
    }
    assert_ne!(key, other);
    assert_ne!(key.hash(), other.hash());
}

#[test]
fn unique_key_domains_are_fresh_and_wrapping_appends_inner_key() {
    let d1 = UniqueKey::generate_domain();
    let d2 = UniqueKey::generate_domain();
    assert_ne!(d1, d2);

    let mut inner = UniqueKey::new();
    {
        let mut b = UniqueKeyBuilder::new(&mut inner, d1, 1, Some("inner"));
        b[0] = 9;
    }
    assert_eq!(inner.tag(), Some("inner"));

    let mut outer = UniqueKey::new();
    {
        let mut b = UniqueKeyBuilder::new_wrapping(&mut outer, &inner, d2, 1, None);
        b[0] = 5;
    }
    // Extra word, then the inner domain, then the inner data.
    assert_eq!(outer.data(), &[5, u32::from(d1), 9]);
    assert_eq!(outer.tag(), None);
}

#[test]
fn resource_key_hash_matches_its_bytes() {
    let data = [1u32, 2, 3];
    let mut bytes = Vec::new();
    for w in data {
        bytes.extend_from_slice(&w.to_ne_bytes());
    }
    assert_eq!(
        resource_key_hash(&data),
        skia_rust_core::checksum::hash32(&bytes, 0)
    );
}

#[test]
fn encode_gradient_stop_round_trips() {
    // frexp(0.5) = (0.5, 0); frexp(0.75) = (0.75, 0); frexp(0.1) = (0.8, -3)
    assert_eq!(encode_gradient_stop_to_half(0.5), Some((0.5, 0.0)));
    assert_eq!(encode_gradient_stop_to_half(0.75), Some((0.75, 0.0)));
    assert_eq!(encode_gradient_stop_to_half(0.0), Some((0.0, 0.0)));
    let (m, e) = encode_gradient_stop_to_half(0.1).expect("0.1 encodes");
    assert!((m - 0.8).abs() < 1e-6);
    assert_eq!(e, -3.0);
}

#[test]
fn dither_tables() {
    assert_eq!(dither_range_for_config(ColorType::RGB565), 1.0 / 63.0);
    assert_eq!(dither_range_for_config(ColorType::RGBA8888), 1.0 / 255.0);
    assert_eq!(dither_range_for_config(ColorType::RGBAF32), 0.0);

    let lut = make_dither_lut();
    assert_eq!(lut.width(), 8);
    assert_eq!(lut.height(), 8);
    // m = 0 for (0, 0): value = -63/128, and (value + 0.5) * 255 + 0.5 truncates to 2.
    assert_eq!(lut.get_addr8(0, 0), 2);
}

#[test]
fn integral_table_width_bins_by_power_of_two() {
    assert_eq!(compute_integral_table_width(6.0), 32);
    assert_eq!(compute_integral_table_width(20.0), 64);
    assert_eq!(compute_integral_table_width(f32::NAN), 0);
    assert_eq!(compute_integral_table_width(f32::INFINITY), 0);
}

#[test]
fn circle_profile_is_monotone_and_zero_at_the_tail() {
    let profile = create_circle_profile(2.0, 5.0, 32);
    assert_eq!(profile.width(), 32);
    let mut prev = 255u8;
    for x in 0..32 {
        let v = profile.get_addr8(x, 0);
        assert!(v <= prev, "profile must not increase at {x}");
        prev = v;
    }
    assert_eq!(profile.get_addr8(31, 0), 0);
}

#[test]
fn buffer_writer_splits_and_marks() {
    let mut bytes = [0u8; 12];
    let mut w = BufferWriter::new(&mut bytes);
    let start = w.mark(0);
    w.write_bytes(&[1, 2, 3]);
    w.zero_bytes(1);
    assert_eq!(w.mark(0) - start, 4);
    // Splits the remaining 8 bytes: `w` keeps the next 2, the tail gets the other 6.
    let mut tail = w.make_offset(2);
    w.write_bytes(&[7, 8]);
    tail.write_bytes(&[9; 6]);
    assert_eq!(bytes, [1, 2, 3, 0, 7, 8, 9, 9, 9, 9, 9, 9]);
}

#[test]
#[should_panic(expected = "past the end")]
fn buffer_writer_panics_past_the_end() {
    let mut bytes = [0u8; 2];
    let mut w = BufferWriter::new(&mut bytes);
    w.write_bytes(&[1, 2, 3]);
}

#[test]
fn vertex_writer_chains_values() {
    let mut bytes = [0u8; 12];
    let mut vw = VertexWriter::new(&mut bytes);
    vw.put(&1u16).put(&2u16).put(&3u32);
    assert_eq!(bytes[0..2], 1u16.to_ne_bytes());
    assert_eq!(bytes[2..4], 2u16.to_ne_bytes());
    assert_eq!(bytes[4..8], 3u32.to_ne_bytes());
}
