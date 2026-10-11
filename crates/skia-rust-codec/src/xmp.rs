// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkXmp.cpp (chrome/m156), the declarations in include/private/SkXmp.h.
//
// The XML is read with the DOM in `skia_rust_core::xml` (docs/design/codecs.md Q4). The
// `SkCodecPrintf` diagnostics are not ported: they are debug output and do not change results.

use skia_rust_core::color::Color4f;
use skia_rust_core::data::Data;
use skia_rust_core::gainmap_info::{BaseImageType, GainmapInfo, GainmapType};
use skia_rust_core::utils::parse;
use skia_rust_core::xml::{Dom, Node, NodeType};

const XMLNS_PREFIX: &str = "xmlns:";
const XMLNS_PREFIX_LENGTH: usize = 6;

// Port of: src/codec/SkXmp.cpp#L27-L32 (chrome/m156)
fn get_namespace_prefix(name: &str) -> Option<&str> {
    if name.len() <= XMLNS_PREFIX_LENGTH {
        return None;
    }
    name.get(XMLNS_PREFIX_LENGTH..)
}

// Port of: src/codec/SkXmp.cpp#L48-L63 (chrome/m156), `get_unique_child_text`.
//
// Given a node, see if that node has only one child with the indicated name. If so, see if that
// child has only a single child of its own, and that child is text. If all of that is the case
// then return the text, otherwise return None.
fn get_unique_child_text<'a>(dom: &'a Dom, node: Node, child_name: &str) -> Option<&'a str> {
    // Fail if there are multiple children with childName.
    if dom.count_children(node, Some(child_name)) != 1 {
        return None;
    }
    let child = dom.get_first_child(node, Some(child_name))?;
    // Fail if the child has any children besides text.
    if dom.count_children(child, None) != 1 {
        return None;
    }
    let grand_child = dom.get_first_child(child, None)?;
    if dom.get_type(grand_child) != NodeType::Text {
        return None;
    }
    Some(dom.get_name(grand_child))
}

// Port of: src/codec/SkXmp.cpp#L74-L101 (chrome/m156), `get_typed_child`.
//
// If there exists a child node with name |prefix| + ":" + |type|, then return that child.
//
// If there exists a child node with name "rdf:type" that has attribute "rdf:resource" with value
// of |type|, then if there also exists a child node with name "rdf:value" with attribute
// "rdf:parseType" of "Resource", then return that child node with name "rdf:value".
fn get_typed_child(dom: &Dom, node: Node, prefix: &str, typ: &str) -> Option<Node> {
    let name = format!("{prefix}:{typ}");
    if let Some(child) = dom.get_first_child(node, Some(&name)) {
        return Some(child);
    }

    let type_child = dom.get_first_child(node, Some("rdf:type"))?;
    let type_child_resource = dom.find_attr(type_child, "rdf:resource")?;
    if type_child_resource != typ {
        return None;
    }

    let value_child = dom.get_first_child(node, Some("rdf:value"))?;
    let value_child_parse_type = dom.find_attr(value_child, "rdf:parseType")?;
    if value_child_parse_type != "Resource" {
        return None;
    }
    Some(value_child)
}

// Port of: src/codec/SkXmp.cpp#L110-L131 (chrome/m156), `get_attr`.
//
// This will first look for an attribute with the name |prefix| + ":" + |key|, and return the value
// for that attribute. This will then look for a child node of name |prefix| + ":" + |key|, and
// return the field value for that child.
fn get_attr<'a>(dom: &'a Dom, node: Node, prefix: &str, key: &str) -> Option<&'a str> {
    let name = format!("{prefix}:{key}");
    if let Some(attr) = dom.find_attr(node, &name) {
        return Some(attr);
    }
    get_unique_child_text(dom, node, &name)
}

// Port of: src/codec/SkXmp.cpp#L134-L147 (chrome/m156), `get_attr_bool`.
fn get_attr_bool(dom: &Dom, node: Node, prefix: &str, key: &str) -> Option<bool> {
    let attr = get_attr(dom, node, prefix, key)?;
    match parse::find_list(attr, "False,True") {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

// Port of: src/codec/SkXmp.cpp#L149-L162 (chrome/m156), `get_attr_int32`.
fn get_attr_int32(dom: &Dom, node: Node, prefix: &str, key: &str) -> Option<i32> {
    let attr = get_attr(dom, node, prefix, key)?;
    parse::find_s32(attr.as_bytes(), 0).map(|(_, value)| value)
}

// Port of: src/codec/SkXmp.cpp#L164-L177 (chrome/m156), `get_attr_float`.
fn get_attr_float(dom: &Dom, node: Node, prefix: &str, key: &str) -> Option<f32> {
    let attr = get_attr(dom, node, prefix, key)?;
    parse::find_scalar(attr.as_bytes(), 0).map(|(_, value)| value)
}

// Port of: src/codec/SkXmp.cpp#L180-L230 (chrome/m156), `get_attr_float3_as_list`.
//
// Perform get_attr and parse the result as three comma-separated floats. Return the result as a
// Color4f with the alpha component set to 1.
fn get_attr_float3_as_list(dom: &Dom, node: Node, prefix: &str, key: &str) -> Option<Color4f> {
    let name = format!("{prefix}:{key}");

    // Fail if there are multiple children with childName.
    if dom.count_children(node, Some(&name)) != 1 {
        return None;
    }
    // Find the child.
    let child = dom.get_first_child(node, Some(&name))?;

    // Search for the rdf:Seq child.
    let seq = dom.get_first_child(child, Some("rdf:Seq"))?;

    let mut count = 0usize;
    let mut values = [0.0f32; 3];
    let mut li_node = dom.get_first_child(seq, Some("rdf:li"));
    while let Some(li) = li_node {
        if count > 2 {
            // "Too many items in list."
            return None;
        }
        if dom.count_children(li, None) != 1 {
            // "Item can only have one child."
            return None;
        }
        let li_text_node = dom.get_first_child(li, None)?;
        if dom.get_type(li_text_node) != NodeType::Text {
            // "Item's only child must be text."
            return None;
        }
        let li_text = dom.get_name(li_text_node);
        let (_, value) = parse::find_scalar(li_text.as_bytes(), 0)?;
        values[count] = value;
        count += 1;
        li_node = dom.get_next_sibling(li, Some("rdf:li"));
    }
    if count < 3 {
        // "List didn't have enough items."
        return None;
    }
    Some(Color4f {
        r: values[0],
        g: values[1],
        b: values[2],
        a: 1.0,
    })
}

// Port of: src/codec/SkXmp.cpp#L232-L243 (chrome/m156), `get_attr_float3`.
fn get_attr_float3(dom: &Dom, node: Node, prefix: &str, key: &str) -> Option<Color4f> {
    if let Some(list) = get_attr_float3_as_list(dom, node, prefix, key) {
        return Some(list);
    }
    let value = get_attr_float(dom, node, prefix, key)?;
    Some(Color4f {
        r: value,
        g: value,
        b: value,
        a: 1.0,
    })
}

// Port of: src/codec/SkXmp.cpp#L245-L263 (chrome/m156), `find_uri_namespaces` on one node.
//
// Search all attributes for xmlns:NAMESPACEi="URIi".
fn find_uri_namespaces_on_node<'a>(
    dom: &'a Dom,
    node: Node,
    uris: &[&str],
    out_namespaces: &mut [Option<&'a str>],
) {
    for (attr_name, attr_value) in dom.attrs(node) {
        // Make sure the name starts with "xmlns:".
        if attr_name.len() <= XMLNS_PREFIX_LENGTH || !attr_name.starts_with(XMLNS_PREFIX) {
            continue;
        }
        // Search for a requested URI that matches.
        for (i, uri) in uris.iter().enumerate() {
            if attr_value == *uri {
                out_namespaces[i] = Some(attr_name);
            }
        }
    }
}

// Port of: src/codec/SkXmp.cpp#L268-L305 (chrome/m156), the single-DOM `find_uri_namespaces`.
//
// Returns the first rdf:Description that carries every requested namespace, with the namespace
// attribute names in `uris` order.
fn find_uri_namespaces<'a>(dom: &'a Dom, uris: &[&str]) -> Option<(Node, Vec<&'a str>)> {
    let root = dom.root_node()?;

    // Ensure that the root node identifies itself as XMP metadata.
    if dom.get_name(root) != "x:xmpmeta" {
        return None;
    }

    // Iterate the children with name rdf:RDF.
    let mut rdf = dom.get_first_child(root, Some("rdf:RDF"));
    while let Some(rdf_node) = rdf {
        let mut rdf_namespaces = vec![None; uris.len()];
        find_uri_namespaces_on_node(dom, rdf_node, uris, &mut rdf_namespaces);

        // Iterate the children with name rdf:Description.
        let mut desc = dom.get_first_child(rdf_node, Some("rdf:Description"));
        while let Some(desc_node) = desc {
            let mut desc_namespaces = rdf_namespaces.clone();
            find_uri_namespaces_on_node(dom, desc_node, uris, &mut desc_namespaces);

            // If we have a match for all the requested URIs, return.
            if desc_namespaces.iter().all(Option::is_some) {
                let found = desc_namespaces.into_iter().flatten().collect();
                return Some((desc_node, found));
            }
            desc = dom.get_next_sibling(desc_node, Some("rdf:Description"));
        }
        rdf = dom.get_next_sibling(rdf_node, Some("rdf:RDF"));
    }
    None
}

// Port of: src/codec/SkXmp.cpp#L334-L348 (chrome/m156), `SkXmpImpl`.
//
// The standard and extended DOMs are kept apart, and searched in order, as XMP Part 3 describes.
#[doc(alias = "SkXmp")]
#[derive(Debug, Default)]
pub struct Xmp {
    standard_dom: Dom,
    extended_dom: Dom,
}

impl Xmp {
    // Port of: src/codec/SkXmp.cpp#L658-L664 (chrome/m156), `SkXmp::Make(sk_sp<SkData>)`.
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(xmp_data: &Data) -> Option<Self> {
        let mut xmp = Self::default();
        if !parse_dom(&mut xmp.standard_dom, xmp_data) {
            return None;
        }
        Some(xmp)
    }

    // Port of: src/codec/SkXmp.cpp#L666-L677 (chrome/m156), `SkXmp::Make(standard, extended)`.
    //
    // Parsing the extended XMP is attempted, but its failure is ignored: the standard XMP is
    // still returned.
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make_with_extended(xmp_standard: &Data, xmp_extended: &Data) -> Option<Self> {
        let mut xmp = Self::default();
        if !parse_dom(&mut xmp.standard_dom, xmp_standard) {
            return None;
        }
        // Try to parse extended xmp but ignore the return value.
        let _ = parse_dom(&mut xmp.extended_dom, xmp_extended);
        Some(xmp)
    }

    // Port of: src/codec/SkXmp.cpp#L380-L397 (chrome/m156), `findUriNamespaces`.
    //
    // See XMP Specification Part 3: Storage in files, Section 1.1.3.1: Extended XMP in JPEG. A
    // JPEG reader must recompose the StandardXMP and ExtendedXMP into a single data model tree.
    // This code does not do that. Instead, it maintains the two separate trees and searches them
    // sequentially.
    fn find_uri_namespaces_in_xmp<'a>(
        &'a self,
        uris: &[&str],
    ) -> Option<(&'a Dom, Node, Vec<&'a str>)> {
        if let Some((node, namespaces)) = find_uri_namespaces(&self.standard_dom, uris) {
            return Some((&self.standard_dom, node, namespaces));
        }
        let (node, namespaces) = find_uri_namespaces(&self.extended_dom, uris)?;
        Some((&self.extended_dom, node, namespaces))
    }

    // Port of: src/codec/SkXmp.cpp#L349-L361 (chrome/m156), `getExtendedXmpGuid`.
    //
    // Return the GUID (the MD5 hash) of an Extended XMP if present, or None otherwise.
    #[doc(alias = "getExtendedXmpGuid")]
    #[must_use]
    pub fn extended_xmp_guid(&self) -> Option<&str> {
        let uris = ["http://ns.adobe.com/xmp/note/"];
        let (extended_node, namespaces) = find_uri_namespaces(&self.standard_dom, &uris)?;
        let xmp_note_prefix = get_namespace_prefix(namespaces[0])?;
        get_attr(
            &self.standard_dom,
            extended_node,
            xmp_note_prefix,
            "HasExtendedXMP",
        )
    }

    // Port of: src/codec/SkXmp.cpp#L363-L423 (chrome/m156), `getContainerGainmapLocation`.
    //
    // If this includes GContainer metadata and the GContainer contains an item with semantic
    // GainMap and Mime of image/jpeg, then return the item's offset (from the end of the primary
    // JPEG image's EndOfImage) and its size.
    #[doc(alias = "getContainerGainmapLocation")]
    #[must_use]
    pub fn container_gainmap_location(&self) -> Option<(usize, usize)> {
        // Find a node that matches the requested namespaces and URIs.
        let uris = [
            "http://ns.google.com/photos/1.0/container/",
            "http://ns.google.com/photos/1.0/container/item/",
        ];
        let (dom, node, namespaces) = self.find_uri_namespaces_in_xmp(&uris)?;
        let container_prefix = get_namespace_prefix(namespaces[0])?;
        let item_prefix = get_namespace_prefix(namespaces[1])?;

        // The node must have a Container:Directory child.
        let directory = get_typed_child(dom, node, container_prefix, "Directory")?;

        // That Container:Directory must have a sequence of items.
        let seq = dom.get_first_child(directory, Some("rdf:Seq"))?;

        // Iterate through the items in the Container:Directory's sequence. Keep a running sum of
        // the Item:Length of all items that appear before the GainMap.
        let mut is_first_item = true;
        let mut offset: usize = 0;
        let mut li_node = dom.get_first_child(seq, Some("rdf:li"));
        while let Some(li) = li_node {
            // Each list item must contain a Container:Item.
            let item = get_typed_child(dom, li, container_prefix, "Item")?;
            // A Semantic is required for every item.
            let item_semantic = get_attr(dom, item, item_prefix, "Semantic")?;
            // A Mime is required for every item.
            let item_mime = get_attr(dom, item, item_prefix, "Mime")?;

            if is_first_item {
                is_first_item = false;
                // The first item must be Primary.
                if item_semantic != "Primary" {
                    return None;
                }
                // The first item has mime type image/jpeg (we are decoding a jpeg).
                if item_mime != "image/jpeg" {
                    return None;
                }
                // The first media item can contain a Padding attribute, which specifies additional
                // padding between the end of the encoded primary image and the beginning of the
                // next media item. Only the first media item can contain a Padding attribute.
                if let Some(padding) = get_attr_int32(dom, item, item_prefix, "Padding") {
                    // Padding must be non-negative.
                    offset += usize::try_from(padding).ok()?;
                }
            } else {
                // A Length is required for all non-Primary items, and must be non-negative.
                let length =
                    usize::try_from(get_attr_int32(dom, item, item_prefix, "Length")?).ok()?;
                // If this is not the recovery map, then read past it.
                if item_semantic == "GainMap" {
                    // The recovery map must have mime type image/jpeg in this implementation.
                    if item_mime != "image/jpeg" {
                        return None;
                    }
                    // Populate the location in the file at which to find the gainmap image.
                    return Some((offset, length));
                }
                offset += length;
            }
            li_node = dom.get_next_sibling(li, Some("rdf:li"));
        }
        None
    }

    // Port of: src/codec/SkXmp.cpp#L425-L461 (chrome/m156), `getGainmapInfoApple`.
    //
    // Return true if the specified XMP metadata identifies this image as an HDR gainmap. If the
    // image specifies an Apple HDRGainMap, then `info` is populated with gainmap parameters that
    // approximate the math specified by Apple.
    #[doc(alias = "getGainmapInfoApple")]
    pub fn get_gainmap_info_apple(&self, exif_hdr_headroom: f32, info: &mut GainmapInfo) -> bool {
        // Find a node that matches the requested namespaces and URIs.
        let uris = [
            "http://ns.apple.com/pixeldatainfo/1.0/",
            "http://ns.apple.com/HDRGainMap/1.0/",
        ];
        let Some((dom, node, namespaces)) = self.find_uri_namespaces_in_xmp(&uris) else {
            return false;
        };
        let Some(adpi_prefix) = get_namespace_prefix(namespaces[0]) else {
            return false;
        };
        let Some(hdr_gainmap_prefix) = get_namespace_prefix(namespaces[1]) else {
            return false;
        };

        let Some(auxiliary_image_type) = get_attr(dom, node, adpi_prefix, "AuxiliaryImageType")
        else {
            return false;
        };
        if auxiliary_image_type != "urn:com:apple:photo:2020:aux:hdrgainmap" {
            return false;
        }

        // Require that the gainmap version be present, but do not require a specific version.
        if get_attr_int32(dom, node, hdr_gainmap_prefix, "HDRGainMapVersion").is_none() {
            return false;
        }

        // If the XMP also specifies a HDRGainMapHeadroom parameter, then prefer that parameter to
        // the parameter specified in the base image Exif.
        let mut hdr_headroom = exif_hdr_headroom;
        if let Some(xmp_hdr_headroom) =
            get_attr_float(dom, node, hdr_gainmap_prefix, "HDRGainMapHeadroom")
        {
            hdr_headroom = xmp_hdr_headroom;
        }

        // This node will often have StoredFormat and NativeFormat children that have inner text
        // that specifies the integer 'L008' (also known as kCVPixelFormatType_OneComponent8).
        info.gainmap_ratio_min = Color4f {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        info.gainmap_ratio_max = Color4f {
            r: hdr_headroom,
            g: hdr_headroom,
            b: hdr_headroom,
            a: 1.0,
        };
        info.gainmap_gamma = Color4f {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        info.epsilon_sdr = Color4f {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        info.epsilon_hdr = Color4f {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        info.display_ratio_sdr = 1.0;
        info.display_ratio_hdr = hdr_headroom;
        info.base_image_type = BaseImageType::Sdr;
        info.gainmap_type = GainmapType::Apple;
        true
    }

    // Port of: src/codec/SkXmp.cpp#L463-L525 (chrome/m156), `getGainmapInfoAdobe`.
    //
    // Extract gainmap parameters from http://ns.adobe.com/hdr-gain-map/1.0/. When `out` is None,
    // only the presence of a valid description is checked.
    #[doc(alias = "getGainmapInfoAdobe")]
    #[doc(alias = "getGainmapInfoHDRGM")]
    #[must_use]
    #[allow(clippy::similar_names)] // offset_sdr / offset_hdr mirror the OffsetSDR / OffsetHDR keys
    pub fn get_gainmap_info_adobe(&self, out: Option<&mut GainmapInfo>) -> bool {
        // Find a node that matches the requested namespace and URI.
        let uris = ["http://ns.adobe.com/hdr-gain-map/1.0/"];
        let Some((dom, node, namespaces)) = self.find_uri_namespaces_in_xmp(&uris) else {
            return false;
        };
        let Some(hdrgm_prefix) = get_namespace_prefix(namespaces[0]) else {
            return false;
        };

        // Require that hdrgm:Version="1.0" be present.
        let Some(version) = get_attr(dom, node, hdrgm_prefix, "Version") else {
            return false;
        };
        if version != "1.0" {
            return false;
        }

        // Initialize the parameters to their defaults.
        let mut base_rendition_is_hdr = false;
        let mut gain_map_min = Color4f {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }; // log2 value
        let mut gain_map_max = Color4f {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }; // log2 value
        let mut gamma = Color4f {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let mut offset_sdr = Color4f {
            r: 1.0 / 64.0,
            g: 1.0 / 64.0,
            b: 1.0 / 64.0,
            a: 0.0,
        };
        let mut offset_hdr = offset_sdr;
        let mut hdr_capacity_min: f32 = 0.0; // log2 value
        let mut hdr_capacity_max: f32 = 1.0; // log2 value

        // Read all parameters that are present.
        if let Some(value) = get_attr_bool(dom, node, hdrgm_prefix, "BaseRenditionIsHDR") {
            base_rendition_is_hdr = value;
        }
        if let Some(value) = get_attr_float3(dom, node, hdrgm_prefix, "GainMapMin") {
            gain_map_min = value;
        }
        if let Some(value) = get_attr_float3(dom, node, hdrgm_prefix, "GainMapMax") {
            gain_map_max = value;
        }
        if let Some(value) = get_attr_float3(dom, node, hdrgm_prefix, "Gamma") {
            gamma = value;
        }
        if let Some(value) = get_attr_float3(dom, node, hdrgm_prefix, "OffsetSDR") {
            offset_sdr = value;
        }
        if let Some(value) = get_attr_float3(dom, node, hdrgm_prefix, "OffsetHDR") {
            offset_hdr = value;
        }
        if let Some(value) = get_attr_float(dom, node, hdrgm_prefix, "HDRCapacityMin") {
            hdr_capacity_min = value;
        }
        if let Some(value) = get_attr_float(dom, node, hdrgm_prefix, "HDRCapacityMax") {
            hdr_capacity_max = value;
        }

        // Translate all parameters to SkGainmapInfo's expected format.
        let Some(out) = out else {
            return true;
        };
        let k_log2 = 2.0f32.ln();
        out.gainmap_ratio_min = Color4f {
            r: (gain_map_min.r * k_log2).exp(),
            g: (gain_map_min.g * k_log2).exp(),
            b: (gain_map_min.b * k_log2).exp(),
            a: 1.0,
        };
        out.gainmap_ratio_max = Color4f {
            r: (gain_map_max.r * k_log2).exp(),
            g: (gain_map_max.g * k_log2).exp(),
            b: (gain_map_max.b * k_log2).exp(),
            a: 1.0,
        };
        out.gainmap_gamma = Color4f {
            r: 1.0 / gamma.r,
            g: 1.0 / gamma.g,
            b: 1.0 / gamma.b,
            a: 1.0,
        };
        out.epsilon_sdr = offset_sdr;
        out.epsilon_hdr = offset_hdr;
        out.display_ratio_sdr = (hdr_capacity_min * k_log2).exp();
        out.display_ratio_hdr = (hdr_capacity_max * k_log2).exp();
        out.base_image_type = if base_rendition_is_hdr {
            BaseImageType::Hdr
        } else {
            BaseImageType::Sdr
        };
        true
    }
}

// Port of: src/codec/SkXmp.cpp#L637-L645 (chrome/m156), `SkXmpImpl::parseDom`.
fn parse_dom(dom: &mut Dom, xmp_data: &Data) -> bool {
    dom.build_from_bytes(xmp_data.as_bytes()).is_some()
}
