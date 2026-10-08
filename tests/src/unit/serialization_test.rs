// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SerializationTest.cpp (chrome/m156)

use std::sync::Mutex;

use skia_rust_core::data::Data;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_descriptor::{FactoryId, FontDescriptor};
use skia_rust_core::font_mgr::TypefaceDecoder;
use skia_rust_core::font_types::set_four_byte_tag;
use skia_rust_core::stream::{DynamicMemoryWStream, MemoryStream, StreamAsset};
use skia_rust_core::typeface::{SerializeBehavior, Typeface};
use skia_rust_effects::dash_path_effect;
use skia_rust_tools::font_tool_utils::{default_typeface, test_font_mgr, with_typeface_decoders};

use crate::{def_font_test, def_test, errorf, reporter_assert};

// Port of: tests/SerializationTest.cpp#L1190-L1204 (chrome/m156), WriteBuffer_external_memory_flattenable
def_test!(WriteBuffer_external_memory_flattenable, |reporter| {
    let intervals = [1.0, 1.0];
    let path_effect = dash_path_effect::new(&intervals, 0.0).expect("a valid dash effect");
    // SkAlign4 of the serialized size.
    let path_size = (path_effect.serialize().size() + 3) & !3;
    reporter_assert!(reporter, path_size > 4);

    // Too small external storage: nothing is written.
    let mut storage = vec![0u8; path_size - 4];
    reporter_assert!(reporter, path_effect.serialize_into(&mut storage) == 0);

    let mut storage = vec![0u8; path_size];
    reporter_assert!(reporter, path_effect.serialize_into(&mut storage) != 0);
});

/// `Typeface::make_deserialize` with a sanitizer closure, as `SkTypeface::MakeDeserialize`.
fn deserialize_with(
    data: &Data,
    sanitizer: Option<&dyn Fn(Data) -> Option<Data>>,
) -> Option<Typeface> {
    let mut stream = MemoryStream::make_copy(data.as_bytes());
    Typeface::make_deserialize(&mut *stream, Some(&test_font_mgr()), sanitizer)
}

// Port of: tests/SerializationTest.cpp#L685-L716 (chrome/m156), Serialization_Typeface_Sanitizer
def_font_test!(Serialization_Typeface_Sanitizer, |reporter| {
    let typeface = default_typeface();
    let Some(serialized_data) = typeface.serialize(SerializeBehavior::DoIncludeData) else {
        errorf!(reporter, "serialize typeface");
        return;
    };

    {
        // Default (no sanitizer) behavior
        let clone = deserialize_with(&serialized_data, None);
        reporter_assert!(reporter, clone.is_some());
    }

    {
        // Sanitizer succeeds (no data change necessary)
        let pass_through = |data: Data| Some(Data::new_copy(data.as_bytes()));
        let clone = deserialize_with(&serialized_data, Some(&pass_through));
        reporter_assert!(reporter, clone.is_some());
    }

    {
        // Pretend sanitizer rejects data
        let reject = |_data: Data| -> Option<Data> { None };
        let clone = deserialize_with(&serialized_data, Some(&reject));
        reporter_assert!(reporter, clone.is_none());
    }
});

/// `SpyDecoder::kSpyFactoryId`: `'spyf'`.
// Port of: tests/SerializationTest.cpp#L720-L723 (chrome/m156)
const SPY_FACTORY_ID: FactoryId = set_four_byte_tag(b's', b'p', b'y', b'f');

/// The bytes that `SpyDecoder` received: `SpyDecoder::fData` (the decoder runs on the test's
/// thread, and only this test uses the spy).
static SPY_DATA: Mutex<Option<Vec<u8>>> = Mutex::new(None);

/// `SpyDecoder`'s factory: it keeps the stream's bytes and returns the default typeface.
// Port of: tests/SerializationTest.cpp#L724-L738 (chrome/m156)
// The `Option` is the `TypefaceDecoder::make_from_stream` signature, which can fail.
#[allow(clippy::unnecessary_wraps)]
fn spy_make_from_stream(
    mut stream: Box<dyn StreamAsset>,
    _args: &FontArguments<'_, '_>,
) -> Option<Typeface> {
    let mut data = vec![0u8; stream.get_length()];
    let read = stream.read(&mut data);
    data.truncate(read);
    if let Ok(mut spy) = SPY_DATA.lock() {
        *spy = Some(data);
    }
    // Return a valid typeface to avoid decoding failure
    Some(default_typeface())
}

// Port of: tests/SerializationTest.cpp#L750-L781 (chrome/m156), Serialization_Typeface_Sanitizer_Spy
def_font_test!(Serialization_Typeface_Sanitizer_Spy, |reporter| {
    let spy_decoder = TypefaceDecoder {
        factory_id: SPY_FACTORY_ID,
        make_from_stream: spy_make_from_stream,
    };
    let mgr = with_typeface_decoders(test_font_mgr(), vec![spy_decoder]);

    // Construct the serialized stream manually with our spy factory id and custom font bytes
    let mut desc = FontDescriptor::new();
    desc.set_factory_id(SPY_FACTORY_ID);
    desc.set_family_name("SpyFont");
    let original_stream: Box<dyn StreamAsset> = MemoryStream::make_copy(b"original_bytes");
    desc.set_stream(Some(original_stream));

    let mut wstream = DynamicMemoryWStream::new();
    reporter_assert!(reporter, desc.serialize(&mut wstream));
    let serialized_data = wstream.detach_as_data();

    // Deserialize using a mutating sanitizer callback
    let mutating_sanitizer = |_data: Data| Some(Data::new_copy(b"sanitized_bytes\0"));
    let mut stream = MemoryStream::make_copy(serialized_data.as_bytes());
    let clone = Typeface::make_deserialize(&mut *stream, Some(&mgr), Some(&mutating_sanitizer));
    reporter_assert!(reporter, clone.is_some());

    // Verify end-to-end that the decoder factory received the sanitized bytes, not the
    // original bytes
    let spy_data = SPY_DATA.lock().ok().and_then(|spy| spy.clone());
    reporter_assert!(reporter, spy_data.is_some());
    reporter_assert!(
        reporter,
        spy_data.as_deref() == Some(&b"sanitized_bytes\0"[..])
    );
});
