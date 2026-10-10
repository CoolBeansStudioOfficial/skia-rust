// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFFormXObject.{h,cpp} (chrome/m156)

//! `SkPDFMakeFormXObject`: a form XObject, the PDF object a layer, a soft mask or a pattern cell
//! is drawn into.

use skia_rust_core::matrix::Matrix;

use crate::document::DocHandle;
use crate::types::{PdfArray, PdfDict, PdfIndirectReference, PdfParentTreeKey};
use crate::utils::matrix_to_array;

/// `SkPDFMakeFormXObject`: writes `content` as a transparency-group form XObject with the given
/// bounding box and resources.
// Port of: src/pdf/SkPDFFormXObject.cpp#L17-L53 (chrome/m156)
#[doc(alias = "SkPDFMakeFormXObject")]
#[must_use]
pub fn make_form_x_object(
    doc: &DocHandle,
    content: &[u8],
    struct_parents_key: PdfParentTreeKey,
    media_box: PdfArray,
    resource_dict: PdfDict,
    inverse_transform: &Matrix,
    color_space: Option<&str>,
) -> PdfIndirectReference {
    let mut dict = PdfDict::new(None);
    dict.insert_name("Type", "XObject");
    dict.insert_name("Subtype", "Form");
    if !inverse_transform.is_identity() {
        dict.insert_object("Matrix", Box::new(matrix_to_array(inverse_transform)));
    }
    dict.insert_object("Resources", Box::new(resource_dict));
    dict.insert_object("BBox", Box::new(media_box));

    if struct_parents_key.is_valid() {
        dict.insert_int("StructParents", struct_parents_key.value);
    }
    // "StructParent" is not supported in favor of using `Do` in a marked-content sequence.

    // Right now FormXObject is only used for saveLayer, which implies
    // isolated blending.  Do this conditionally if that changes.
    // TODO(halcanary): Is this comment obsolete, since we use it for
    // alpha masks?
    let mut group = PdfDict::new(Some("Group"));
    group.insert_name("S", "Transparency");
    if let Some(color_space) = color_space {
        group.insert_name("CS", color_space);
    }
    group.insert_bool("I", true); // Isolated.
    dict.insert_object("Group", Box::new(group));
    let xobject = doc.stream_out(Some(dict), content, true);
    if struct_parents_key.is_valid() {
        doc.with(|d| {
            d.struct_tree
                .set_content_stream_ref_for_struct_parents_key(struct_parents_key, xobject);
        });
    }
    xobject
}
