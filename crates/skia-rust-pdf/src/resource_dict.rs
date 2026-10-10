// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFResourceDict.{h,cpp} (chrome/m156)

//! The resource dictionary of a page or form XObject, and the names its entries are used by.

use skia_rust_core::stream::WStream;

use crate::types::{PdfArray, PdfDict, PdfIndirectReference};

/// `SkPDFResourceType`: the kinds of resource a content stream names.
// Port of: src/pdf/SkPDFResourceDict.h#L17-L24 (chrome/m156)
#[doc(alias = "SkPDFResourceType")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResourceType {
    /// `kExtGState`.
    ExtGState = 0,
    /// `kPattern`.
    Pattern = 1,
    /// `kXObject`.
    XObject = 2,
    /// `kFont`.
    Font = 3,
    // These additional types are defined by the spec, but not currently used by Skia: ColorSpace,
    // Shading, Properties
}

/// The prefix of the name of a resource of type `ty`, then the key in decimal.
// Port of: src/pdf/SkPDFResourceDict.cpp#L36-L47 (get_resource_name, chrome/m156)
fn resource_name_text(ty: ResourceType, key: i32) -> String {
    let prefix = match ty {
        ResourceType::ExtGState => 'G',
        ResourceType::Pattern => 'P',
        ResourceType::XObject => 'X',
        ResourceType::Font => 'F',
    };
    format!("{prefix}{key}")
}

/// `SkPDFWriteResourceName`: writes `/` and the name of the resource.
// Port of: src/pdf/SkPDFResourceDict.cpp#L49-L55 (chrome/m156)
#[doc(alias = "SkPDFWriteResourceName")]
pub fn write_resource_name(dst: &mut dyn WStream, ty: ResourceType, key: i32) {
    dst.write_text("/");
    dst.write_text(&resource_name_text(ty, key));
}

/// The key of the sub-dictionary of resources of type `ty`.
// Port of: src/pdf/SkPDFResourceDict.cpp#L57-L66 (resource_name, chrome/m156)
fn resource_type_key(ty: ResourceType) -> &'static str {
    match ty {
        ResourceType::ExtGState => "ExtGState",
        ResourceType::Pattern => "Pattern",
        ResourceType::XObject => "XObject",
        ResourceType::Font => "Font",
    }
}

// Port of: src/pdf/SkPDFResourceDict.cpp#L74-L85 (add_subdict, chrome/m156)
fn add_subdict(resource_list: &[PdfIndirectReference], ty: ResourceType, dst: &mut PdfDict) {
    if !resource_list.is_empty() {
        let mut resources = PdfDict::new(None);
        for reference in resource_list {
            resources.insert_ref_escaped_key(resource_name_text(ty, reference.value), *reference);
        }
        dst.insert_object(resource_type_key(ty), Box::new(resources));
    }
}

// Port of: src/pdf/SkPDFResourceDict.cpp#L87-L95 (make_proc_set, chrome/m156)
fn make_proc_set() -> PdfArray {
    const PROCS: [&str; 5] = ["PDF", "Text", "ImageB", "ImageC", "ImageI"];
    let mut proc_sets = PdfArray::new();
    proc_sets.reserve(PROCS.len());
    for proc in PROCS {
        proc_sets.append_name(proc);
    }
    proc_sets
}

/// `SkPDFMakeResourceDict`: a resource dictionary with the given resources, sorted by the caller.
// Port of: src/pdf/SkPDFResourceDict.cpp#L97-L108 (chrome/m156)
#[doc(alias = "SkPDFMakeResourceDict")]
#[must_use]
pub fn make_resource_dict(
    graphic_state_resources: &[PdfIndirectReference],
    shader_resources: &[PdfIndirectReference],
    x_object_resources: &[PdfIndirectReference],
    font_resources: &[PdfIndirectReference],
) -> PdfDict {
    let mut dict = PdfDict::new(None);
    dict.insert_object("ProcSet", Box::new(make_proc_set()));
    add_subdict(graphic_state_resources, ResourceType::ExtGState, &mut dict);
    add_subdict(shader_resources, ResourceType::Pattern, &mut dict);
    add_subdict(x_object_resources, ResourceType::XObject, &mut dict);
    add_subdict(font_resources, ResourceType::Font, &mut dict);
    dict
}
