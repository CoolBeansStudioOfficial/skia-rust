// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/gpu/graphite/SerializationUtils.cpp (chrome/m156), src/gpu/graphite/SerializationUtils.h

//! Serialized pipeline keys: the byte form of a `GraphicsPipelineDesc` and `RenderPassDesc` that
//! the pipeline callbacks hand to the client, and that `PrecompileContext` reads back.
//!
//! Skia writes through `SkWStream` and reads through `SkStream`, both in native byte order. The
//! port keeps that: every integer is written and read with `to_ne_bytes`/`from_ne_bytes`, so the
//! bytes match Skia's on every platform.

use skia_rust_core::data::Data;
use skia_rust_core::font_types::set_four_byte_tag;
use skia_rust_core::swizzle::Swizzle;

use crate::graphite::caps::Caps;
use crate::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use crate::graphite::graphite_types::SampleCount;
use crate::graphite::paint_params_key::PaintParamsKey;
use crate::graphite::render_pass_desc::{AttachmentDesc, RenderPassDesc};
use crate::graphite::render_step::{NUM_RENDER_STEPS, RenderStepID};
use crate::graphite::resource_types::{LOAD_OP_COUNT, LoadOp, STORE_OP_COUNT, StoreOp};
use crate::graphite::shader_code_dictionary::ShaderCodeDictionary;
use crate::graphite::texture_format::{TEXTURE_FORMAT_COUNT, TextureFormat};
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;

// This is the main control to version the serialized Pipelines (c.f. stream_is_blob)
// Port of: src/gpu/graphite/SerializationUtils.cpp#L21 (chrome/m156)
const CURRENT_VERSION: u32 = 3;

// Port of: src/gpu/graphite/SerializationUtils.cpp#L25 (chrome/m156)
const MAGIC: [u8; 8] = *b"skiapipe";

// Port of: src/gpu/graphite/SerializationUtils.cpp#L233 (chrome/m156), `SK_BLOB_END_TAG`
const BLOB_END_TAG: u32 = set_four_byte_tag(b'e', b'n', b'd', b' ');

// The `RenderStepID` values in declaration order, so that `id as u32` indexes this table.
const RENDER_STEP_IDS: [RenderStepID; NUM_RENDER_STEPS] = [
    RenderStepID::Invalid,
    RenderStepID::CircularArc,
    RenderStepID::AnalyticRRect,
    RenderStepID::AnalyticBlur,
    RenderStepID::AnalyticRRectBlur,
    RenderStepID::PerEdgeAAQuad,
    RenderStepID::CoverBounds_NonAAFill,
    RenderStepID::CoverBounds_RegularCover,
    RenderStepID::CoverBounds_InverseCover,
    RenderStepID::CoverageMask,
    RenderStepID::BitmapText_Mask,
    RenderStepID::BitmapText_LCD,
    RenderStepID::BitmapText_Color,
    RenderStepID::MiddleOutFan_EvenOdd,
    RenderStepID::MiddleOutFan_Winding,
    RenderStepID::SDFTextLCD,
    RenderStepID::SDFText,
    RenderStepID::TessellateCurves_EvenOdd,
    RenderStepID::TessellateCurves_Winding,
    RenderStepID::TessellateStrokes_Fill,
    RenderStepID::TessellateStrokes_InverseFill,
    RenderStepID::TessellateWedges_Convex,
    RenderStepID::TessellateWedges_EvenOdd,
    RenderStepID::TessellateWedges_Winding,
    RenderStepID::Vertices_Pos,
    RenderStepID::Vertices_PosColor,
    RenderStepID::Vertices_PosTexCoords,
    RenderStepID::Vertices_PosColorTexCoords,
    RenderStepID::Mesh,
    RenderStepID::EndCap,
    RenderStepID::WideTile,
];

// Reads a stream the way `SkStream`/`SkMemoryStream` does: every read is all or nothing (`None`
// where Skia returns `false`), and integers are in native byte order.
struct Reader<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    // `getPosition()`.
    fn position(&self) -> usize {
        self.position
    }

    // `getLength()`.
    fn length(&self) -> usize {
        self.data.len()
    }

    // `isAtEnd()`.
    fn is_at_end(&self) -> bool {
        self.position == self.data.len()
    }

    // `read(buffer, size) == size`: reads exactly `size` bytes or fails.
    fn read(&mut self, size: usize) -> Option<&'a [u8]> {
        let end = self.position.checked_add(size)?;
        if end > self.data.len() {
            return None;
        }
        let bytes = &self.data[self.position..end];
        self.position = end;
        Some(bytes)
    }

    // `skip(size)`: returns the number of bytes skipped, which is less than `size` at the end.
    fn skip(&mut self, size: usize) -> usize {
        let skipped = size.min(self.data.len() - self.position);
        self.position += skipped;
        skipped
    }

    // `readU32`.
    fn read_u32(&mut self) -> Option<u32> {
        let bytes = self.read(4)?;
        Some(u32::from_ne_bytes(bytes.try_into().ok()?))
    }

    // `readS32`.
    fn read_s32(&mut self) -> Option<i32> {
        let bytes = self.read(4)?;
        Some(i32::from_ne_bytes(bytes.try_into().ok()?))
    }

    // `readU16`.
    fn read_u16(&mut self) -> Option<u16> {
        let bytes = self.read(2)?;
        Some(u16::from_ne_bytes(bytes.try_into().ok()?))
    }

    // `readU8`.
    fn read_u8(&mut self) -> Option<u8> {
        let bytes = self.read(1)?;
        Some(bytes[0])
    }
}

// The writer side (`SkWStream` into an `SkDynamicMemoryWStream`); every write succeeds.
#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    // `write(buffer, size)`.
    fn write(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    // `write32`.
    fn write32(&mut self, value: u32) {
        self.write(&value.to_ne_bytes());
    }

    // `write16`.
    fn write16(&mut self, value: u16) {
        self.write(&value.to_ne_bytes());
    }

    // `write8`.
    fn write8(&mut self, value: u8) {
        self.write(&[value]);
    }
}

// `SkIsPow2(sampleCount) && sampleCount >= 1 && sampleCount <= 16`.
// Port of: src/gpu/graphite/SerializationUtils.cpp#L30-L32 (chrome/m156)
const fn is_valid_samplecount(sample_count: u32) -> bool {
    sample_count.is_power_of_two() && sample_count >= 1 && sample_count <= 16
}

// Port of: src/gpu/graphite/SerializationUtils.cpp#L34-L49 (chrome/m156)
fn stream_is_pipeline(stream: &mut Reader<'_>) -> bool {
    let Some(magic) = stream.read(MAGIC.len()) else {
        return false;
    };
    if magic != MAGIC {
        return false;
    }

    let Some(version) = stream.read_u32() else {
        return false;
    };
    version == CURRENT_VERSION
}

// Port of: src/gpu/graphite/SerializationUtils.cpp#L51-L80 (chrome/m156)
fn serialize_graphics_pipeline_desc(
    shader_code_dictionary: &ShaderCodeDictionary,
    stream: &mut Writer,
    pipeline_desc: GraphicsPipelineDesc,
) -> bool {
    let key_data = shader_code_dictionary.lookup(pipeline_desc.paint_params_id());
    let key = PaintParamsKey::new(&key_data);

    stream.write32(pipeline_desc.render_step_id() as u32);

    if !key.is_valid() {
        stream.write32(0);
        // Not all GraphicsPipeline have a valid PaintParamsKey
        return true;
    }

    let key_span = key.data();

    if !key.is_serializable(shader_code_dictionary) {
        return false;
    }

    stream.write32(u32::try_from(key_span.len()).unwrap_or(u32::MAX));
    for value in key_span {
        stream.write(&value.to_ne_bytes());
    }
    true
}

// Port of: src/gpu/graphite/SerializationUtils.cpp#L82-L115 (chrome/m156)
fn deserialize_graphics_pipeline_desc(
    shader_code_dictionary: &ShaderCodeDictionary,
    stream: &mut Reader<'_>,
    pipeline_desc: &mut GraphicsPipelineDesc,
) -> bool {
    let Some(tmp) = stream.read_u32() else {
        return false;
    };

    if tmp as usize >= NUM_RENDER_STEPS {
        return false;
    }
    let render_step_id = RENDER_STEP_IDS[tmp as usize];

    let Some(tmp) = stream.read_u32() else {
        return false;
    };

    let mut paint_params_id = UniquePaintParamsID::invalid();
    if tmp != 0 {
        // `4 * tmp` is untrusted. Skia's C++ overflows here on a malicious stream; the port
        // rejects it instead, as it rejects any key the stream cannot hold.
        let Some(byte_count) = usize::try_from(tmp).ok().and_then(|n| n.checked_mul(4)) else {
            return false;
        };
        let Some(bytes) = stream.read(byte_count) else {
            return false;
        };
        let key_data: Vec<i32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| i32::from_ne_bytes(*chunk))
            .collect();

        let ppk = PaintParamsKey::new(&key_data);
        if !ppk.is_serializable(shader_code_dictionary) {
            return false;
        }

        paint_params_id = shader_code_dictionary.find_or_create(&ppk);
    }

    *pipeline_desc = GraphicsPipelineDesc::new(render_step_id, paint_params_id);
    true
}

// check a single block and, recursively, all its children
// Port of: src/gpu/graphite/SerializationUtils.cpp#L117-L168 (chrome/m156)
fn block_contains_ext_format(
    dict: &ShaderCodeDictionary,
    stream: &mut Reader<'_>,
    contains_ext_format: &mut bool,
) -> bool {
    let Some(code_snippet_id) = stream.read_s32() else {
        return false;
    };

    let Some(entry) = dict.get_entry(code_snippet_id) else {
        return false;
    };

    if entry.stores_sampler_desc_data() {
        let Some(data_length_encoded) = stream.read_s32() else {
            return false;
        };

        // This `dataLengthEncoded` is untrusted, so check that it doesn't overflow EncodeDataSize
        // and that it matches expectations of a valid length (i.e. it started out negative and is
        // now positive and less than the key data limit).
        if data_length_encoded >= 0
            || data_length_encoded
                < PaintParamsKey::encode_data_size(PaintParamsKey::EMBEDDED_DATA_SIZE_LIMIT)
        {
            // Would not produce a valid size after decoding
            return false;
        }

        let data_length = PaintParamsKey::encode_data_size(data_length_encoded);
        debug_assert!((0..=PaintParamsKey::EMBEDDED_DATA_SIZE_LIMIT).contains(&data_length));
        // `data_length` is non-negative here: `encode_data_size` maps the negative encoding back.
        let data_bytes = usize::try_from(data_length).unwrap_or(0) * 4;
        if stream.position() + data_bytes > stream.length() {
            return false;
        }

        // A SamplerDesc is serialized as either 1, 2, or 3 uint32_ts:
        //   0: the descriptor
        //   1: the format for immutable samplers
        //   2: the MSB for an external format (combined with the prior uint32_t)
        // The third uint32_t is only written for external formats so we can just use
        // the dataLength as a test.
        if data_length == 3 {
            *contains_ext_format = true;
        }

        if stream.skip(data_bytes) != data_bytes {
            return false;
        }
    }

    for _ in 0..entry.num_children {
        if !block_contains_ext_format(dict, stream, contains_ext_format) {
            return false;
        }
    }
    true
}

// Port of: src/gpu/graphite/SerializationUtils.cpp#L170-L200 (chrome/m156)
fn graphics_pipeline_desc_contains_ext_format(
    shader_code_dictionary: &ShaderCodeDictionary,
    stream: &mut Reader<'_>,
    contains_ext_format: &mut bool,
) -> bool {
    let Some(render_step_id) = stream.read_u32() else {
        return false;
    };
    if render_step_id as usize >= NUM_RENDER_STEPS {
        return false;
    }

    let Some(key_size) = stream.read_u32() else {
        return false;
    };

    if key_size != 0 {
        // The C++ `uint32_t` arithmetic here can wrap on an untrusted stream; a wrapped end is
        // reported as an error.
        let Some(end_of_key) = usize::try_from(key_size)
            .ok()
            .and_then(|n| n.checked_mul(4))
            .and_then(|n| n.checked_add(stream.position()))
        else {
            return false;
        };
        if end_of_key > stream.length() {
            return false;
        }

        while stream.position() < end_of_key {
            let Some(root_block_header) = stream.read_s32() else {
                return false;
            };
            if root_block_header >= 0 {
                return false;
            }
            if !block_contains_ext_format(shader_code_dictionary, stream, contains_ext_format) {
                return false;
            }
        }

        debug_assert_eq!(end_of_key, stream.position());
    }

    true
}

// Port of: src/gpu/graphite/SerializationUtils.cpp#L202-L209 (chrome/m156)
fn serialize_attachment_desc(stream: &mut Writer, attachment_desc: AttachmentDesc) {
    let tag = if attachment_desc.format == TextureFormat::Unsupported {
        set_four_byte_tag(TextureFormat::Unsupported as u8, 0, 0, 1)
    } else {
        set_four_byte_tag(
            attachment_desc.format as u8,
            attachment_desc.load_op as u8,
            attachment_desc.store_op as u8,
            attachment_desc.sample_count as u8,
        )
    };
    stream.write32(tag);
}

// `static_cast<LoadOp>(i)` for an index already checked against `LOAD_OP_COUNT`.
const fn load_op_from_index(index: u8) -> LoadOp {
    match index {
        0 => LoadOp::Load,
        1 => LoadOp::Clear,
        _ => LoadOp::Discard,
    }
}

// `static_cast<StoreOp>(i)` for an index already checked against `STORE_OP_COUNT`.
const fn store_op_from_index(index: u8) -> StoreOp {
    match index {
        0 => StoreOp::Store,
        _ => StoreOp::Discard,
    }
}

// `static_cast<SampleCount>(i)` for a value already checked by `is_valid_samplecount`.
const fn sample_count_from_u8(value: u8) -> SampleCount {
    match value {
        1 => SampleCount::One,
        2 => SampleCount::Two,
        4 => SampleCount::Four,
        8 => SampleCount::Eight,
        _ => SampleCount::Sixteen,
    }
}

// Port of: src/gpu/graphite/SerializationUtils.cpp#L211-L238 (chrome/m156)
fn deserialize_attachment_desc(stream: &mut Reader<'_>) -> Option<AttachmentDesc> {
    let tag = stream.read_u32()?;

    let format = ((tag >> 24) & 0xFF) as u8;
    let load_op = ((tag >> 16) & 0xFF) as u8;
    let store_op = ((tag >> 8) & 0xFF) as u8;
    let sample_count = (tag & 0xFF) as u8;

    if usize::from(format) >= TEXTURE_FORMAT_COUNT {
        return None;
    }
    if usize::from(load_op) >= LOAD_OP_COUNT {
        return None;
    }
    if usize::from(store_op) >= STORE_OP_COUNT {
        return None;
    }
    if !is_valid_samplecount(u32::from(sample_count)) {
        return None;
    }

    Some(AttachmentDesc {
        format: TextureFormat::ALL[usize::from(format)],
        load_op: load_op_from_index(load_op),
        store_op: store_op_from_index(store_op),
        sample_count: sample_count_from_u8(sample_count),
    })
}

// Port of: src/gpu/graphite/SerializationUtils.cpp#L240-L255 (chrome/m156)
fn serialize_render_pass_desc(stream: &mut Writer, render_pass_desc: &RenderPassDesc) -> bool {
    serialize_attachment_desc(stream, render_pass_desc.color_attachment);
    serialize_attachment_desc(stream, render_pass_desc.color_resolve_attachment);
    serialize_attachment_desc(stream, render_pass_desc.depth_stencil_attachment);

    stream.write16(render_pass_desc.write_swizzle.as_key());
    stream.write8(render_pass_desc.sample_count as u8);

    // Omit clear values for the various attachments as they do not effect structure.
    // Omit fDstReadStrategy from the serialization because it is not a part of RenderPassDesc
    // keys and does not impact pipeline creation. When deserializing, the strategy can be
    // obtained via caps->getDstReadStrategy().
    true
}

// Port of: src/gpu/graphite/SerializationUtils.cpp#L257-L285 (chrome/m156)
fn deserialize_render_pass_desc(
    caps: &dyn Caps,
    stream: &mut Reader<'_>,
    render_pass_desc: &mut RenderPassDesc,
) -> bool {
    let Some(color_attachment) = deserialize_attachment_desc(stream) else {
        return false;
    };
    let Some(color_resolve_attachment) = deserialize_attachment_desc(stream) else {
        return false;
    };
    let Some(depth_stencil_attachment) = deserialize_attachment_desc(stream) else {
        return false;
    };
    render_pass_desc.color_attachment = color_attachment;
    render_pass_desc.color_resolve_attachment = color_resolve_attachment;
    render_pass_desc.depth_stencil_attachment = depth_stencil_attachment;

    let Some(swizzle) = stream.read_u16() else {
        return false;
    };
    render_pass_desc.write_swizzle = Swizzle::from_key(swizzle);

    let Some(sample_count) = stream.read_u8() else {
        return false;
    };
    if !is_valid_samplecount(u32::from(sample_count)) {
        return false;
    }
    render_pass_desc.sample_count = sample_count_from_u8(sample_count);

    // RenderPassDesc dst read strategy is not serialized as it is not something we key on and does
    // not impact pipeline creation. When deserializing, simply query Caps again for a
    // DstReadStrategy. Leave clear color/depth/stencil as their default values.
    render_pass_desc.dst_read_strategy = caps.get_dst_read_strategy();

    true
}

// Port of: src/gpu/graphite/SerializationUtils.cpp#L287-L302 (chrome/m156)
fn render_pass_contains_ext_format(
    caps: &dyn Caps,
    stream: &mut Reader<'_>,
    contains_ext_format: &mut bool,
) -> bool {
    let mut render_pass_desc = RenderPassDesc::default();

    if !deserialize_render_pass_desc(caps, stream, &mut render_pass_desc) {
        return false;
    }

    if render_pass_desc.color_attachment.format == TextureFormat::External
        || render_pass_desc.color_resolve_attachment.format == TextureFormat::External
        || render_pass_desc.depth_stencil_attachment.format == TextureFormat::External
    {
        *contains_ext_format = true;
    }

    true
}

// Port of: src/gpu/graphite/SerializationUtils.cpp#L304-L326 (chrome/m156), `SerializePipelineDesc`
fn serialize_pipeline_desc(
    shader_code_dictionary: &ShaderCodeDictionary,
    stream: &mut Writer,
    pipeline_desc: GraphicsPipelineDesc,
    render_pass_desc: &RenderPassDesc,
) -> bool {
    stream.write(&MAGIC);
    stream.write32(CURRENT_VERSION);

    if !serialize_graphics_pipeline_desc(shader_code_dictionary, stream, pipeline_desc) {
        return false;
    }

    if !serialize_render_pass_desc(stream, render_pass_desc) {
        return false;
    }

    stream.write32(BLOB_END_TAG);
    true
}

// Port of: src/gpu/graphite/SerializationUtils.cpp#L328-L351 (chrome/m156), `DeserializePipelineDesc`
fn deserialize_pipeline_desc(
    caps: &dyn Caps,
    shader_code_dictionary: &ShaderCodeDictionary,
    stream: &mut Reader<'_>,
    pipeline_desc: &mut GraphicsPipelineDesc,
    render_pass_desc: &mut RenderPassDesc,
) -> bool {
    if !stream_is_pipeline(stream) {
        return false;
    }

    if !deserialize_graphics_pipeline_desc(shader_code_dictionary, stream, pipeline_desc) {
        return false;
    }

    if !deserialize_render_pass_desc(caps, stream, render_pass_desc) {
        return false;
    }

    let Some(tag) = stream.read_u32() else {
        return false;
    };

    tag == BLOB_END_TAG
}

// Port of: src/gpu/graphite/SerializationUtils.cpp#L353-L382 (chrome/m156),
// `serialized_key_contains_ext_format`
fn serialized_key_contains_ext_format(
    caps: &dyn Caps,
    shader_code_dictionary: &ShaderCodeDictionary,
    stream: &mut Reader<'_>,
    contains_ext_format: &mut bool,
) -> bool {
    if !stream_is_pipeline(stream) {
        return false;
    }

    if !graphics_pipeline_desc_contains_ext_format(
        shader_code_dictionary,
        stream,
        contains_ext_format,
    ) {
        return false;
    }

    if !render_pass_contains_ext_format(caps, stream, contains_ext_format) {
        return false;
    }

    let Some(tag) = stream.read_u32() else {
        return false;
    };
    if tag != BLOB_END_TAG {
        return false;
    }

    debug_assert!(stream.is_at_end());

    true
}

/// `PipelineDescToData`: serializes a pipeline description and its render pass into a key that
/// `data_to_pipeline_desc` reads back. `None` if the paint key cannot be serialized.
// Port of: src/gpu/graphite/SerializationUtils.cpp#L384-L416 (chrome/m156)
#[doc(alias = "PipelineDescToData")]
#[must_use]
pub fn pipeline_desc_to_data(
    _caps: &dyn Caps,
    shader_code_dictionary: &ShaderCodeDictionary,
    pipeline_desc: &GraphicsPipelineDesc,
    render_pass_desc: &RenderPassDesc,
) -> Option<Data> {
    let mut stream = Writer::default();

    if !serialize_pipeline_desc(
        shader_code_dictionary,
        &mut stream,
        *pipeline_desc,
        render_pass_desc,
    ) {
        return None;
    }

    Some(Data::new_from_vec(stream.bytes))
}

/// `DataToPipelineDesc`: reads a serialized pipeline key back into a pipeline description and
/// render pass. Registers the paint key with `shader_code_dictionary`, so that the description
/// gets this session's `UniquePaintParamsID`. `None` for a missing or malformed key.
// Port of: src/gpu/graphite/SerializationUtils.cpp#L418-L438 (chrome/m156)
#[doc(alias = "DataToPipelineDesc")]
#[must_use]
pub fn data_to_pipeline_desc(
    caps: &dyn Caps,
    shader_code_dictionary: &ShaderCodeDictionary,
    data: Option<&Data>,
) -> Option<(GraphicsPipelineDesc, RenderPassDesc)> {
    let data = data?;
    let mut stream = Reader::new(data.as_bytes());

    let mut pipeline_desc = GraphicsPipelineDesc::default();
    let mut render_pass_desc = RenderPassDesc::default();
    if !deserialize_pipeline_desc(
        caps,
        shader_code_dictionary,
        &mut stream,
        &mut pipeline_desc,
        &mut render_pass_desc,
    ) {
        return None;
    }

    Some((pipeline_desc, render_pass_desc))
}

/// `DataContainsExternalFormat`: `None` if `data` is missing or malformed; otherwise whether the
/// key uses an external texture format.
// Port of: src/gpu/graphite/SerializationUtils.cpp#L440-L450 (chrome/m156)
#[doc(alias = "DataContainsExternalFormat")]
#[must_use]
pub fn data_contains_external_format(
    caps: &dyn Caps,
    shader_code_dictionary: &ShaderCodeDictionary,
    data: Option<&Data>,
) -> Option<bool> {
    let data = data?;
    let mut stream = Reader::new(data.as_bytes());

    let mut contains_ext_format = false;
    if !serialized_key_contains_ext_format(
        caps,
        shader_code_dictionary,
        &mut stream,
        &mut contains_ext_format,
    ) {
        return None;
    }
    Some(contains_ext_format)
}
