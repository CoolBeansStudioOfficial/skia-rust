// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkXmpTest.cpp (chrome/m156), all seven cases.
#![cfg(test)]
// The C++ asserts compare the parsed gainmap parameters with ==.
#![allow(clippy::float_cmp)]

use skia_rust_codec::xmp::Xmp;
use skia_rust_core::data::Data;
use skia_rust_core::gainmap_info::GainmapInfo;

use crate::{def_test, reporter_assert};

// Port of: tests/SkXmpTest.cpp#L21-L32 (chrome/m156)
def_test!(SkXmp_invalidXml, |r| {
    // Invalid truncated xml.
    let xmp_data = r#"
            <x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 6.0.0">
               <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
                        xmlns:"#;

    let app1_param = Data::new_copy(xmp_data.as_bytes());

    let xmp = Xmp::make(&app1_param);
    reporter_assert!(r, xmp.is_none());
});

// Port of: tests/SkXmpTest.cpp#L34-L57 (chrome/m156)
def_test!(SkXmp_xmpHdrgmAsFieldValue, |r| {
    // Expose HDRM values as fields. Also place the HDRGM namespace in the rdf:RDF node.
    let xmp_data = r#"
            <x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 6.0.0">
               <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
                        xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/">
                  <rdf:Description rdf:about="">
                     <hdrgm:Version>1.0</hdrgm:Version>
                     <hdrgm:GainMapMax>3</hdrgm:GainMapMax>
                     <hdrgm:HDRCapacityMax>4</hdrgm:HDRCapacityMax>
                  </rdf:Description>
               </rdf:RDF>
            </x:xmpmeta>"#;

    let app1_param = Data::new_copy(xmp_data.as_bytes());

    let Some(xmp) = Xmp::make(&app1_param) else {
        reporter_assert!(r, false);
        return;
    };

    let mut info = GainmapInfo::default();
    reporter_assert!(r, xmp.get_gainmap_info_hdrgm(&mut info));
    reporter_assert!(r, info.gainmap_ratio_max.r == 8.0);
    reporter_assert!(r, info.display_ratio_hdr == 16.0);
});

// Port of: tests/SkXmpTest.cpp#L59-L79 (chrome/m156)
def_test!(SkXmp_xmpHdrgmRequiresVersion, |r| {
    // Same as the above, except with Version being absent.
    let xmp_data = r#"
            <x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 6.0.0">
               <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
                        xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/">
                  <rdf:Description rdf:about="">
                     <hdrgm:GainMapMax>3</hdrgm:GainMapMax>
                     <hdrgm:HDRCapacityMax>4</hdrgm:HDRCapacityMax>
                  </rdf:Description>
               </rdf:RDF>
            </x:xmpmeta>"#;

    let app1_param = Data::new_copy(xmp_data.as_bytes());

    let Some(xmp) = Xmp::make(&app1_param) else {
        reporter_assert!(r, false);
        return;
    };

    let mut info = GainmapInfo::default();
    reporter_assert!(r, !xmp.get_gainmap_info_hdrgm(&mut info));
});

// Port of: tests/SkXmpTest.cpp#L81-L103 (chrome/m156)
def_test!(SkXmp_xmpHdrgmAsDescriptionPropertyAttributes, |r| {
    // Expose HDRGM values as attributes on an rdf:Description node.
    let xmp_data = r#"
            <x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 6.0.0">
               <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
                  <rdf:Description rdf:about=""
                        xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/"
                     hdrgm:Version="1.0"
                     hdrgm:GainMapMax="3"
                     hdrgm:HDRCapacityMax="4"/>
               </rdf:RDF>
            </x:xmpmeta>"#;

    let app1_param = Data::new_copy(xmp_data.as_bytes());

    let Some(xmp) = Xmp::make(&app1_param) else {
        reporter_assert!(r, false);
        return;
    };

    let mut info = GainmapInfo::default();
    reporter_assert!(r, xmp.get_gainmap_info_hdrgm(&mut info));
    reporter_assert!(r, info.gainmap_ratio_max.r == 8.0);
    reporter_assert!(r, info.display_ratio_hdr == 16.0);
});

// Test mixed list and non-list entries.
// Port of: tests/SkXmpTest.cpp#L106-L167 (chrome/m156)
def_test!(SkXmp_xmpHdrgmList, |r| {
    let xmp_data = r#"
            <x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 6.0.0">
               <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
                        xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/">
                  <rdf:Description rdf:about=""
                     hdrgm:Version="1.0"
                     hdrgm:GainMapMin="2.0"
                     hdrgm:OffsetSDR="0.1">
                     <hdrgm:GainMapMax>
                       <rdf:Seq>
                         <rdf:li>3</rdf:li>
                         <rdf:li>4</rdf:li>
                         <rdf:li>5</rdf:li>
                       </rdf:Seq>
                     </hdrgm:GainMapMax>
                     <hdrgm:Gamma>
                       1.2
                     </hdrgm:Gamma>
                     <hdrgm:OffsetHDR>
                       <rdf:Seq>
                         <rdf:li>
                           0.2
                         </rdf:li>
                         <rdf:li>
                           0.3
                         </rdf:li>
                         <rdf:li>
                           0.4
                         </rdf:li>
                       </rdf:Seq>
                     </hdrgm:OffsetHDR>
                  </rdf:Description>
               </rdf:RDF>
            </x:xmpmeta>"#;

    let app1_param = Data::new_copy(xmp_data.as_bytes());

    let Some(xmp) = Xmp::make(&app1_param) else {
        reporter_assert!(r, false);
        return;
    };

    let mut info = GainmapInfo::default();
    reporter_assert!(r, xmp.get_gainmap_info_hdrgm(&mut info));
    reporter_assert!(r, info.gainmap_ratio_min.r == 4.0);
    reporter_assert!(r, info.gainmap_ratio_min.g == 4.0);
    reporter_assert!(r, info.gainmap_ratio_min.b == 4.0);
    reporter_assert!(r, info.gainmap_ratio_max.r == 8.0);
    reporter_assert!(r, info.gainmap_ratio_max.g == 16.0);
    reporter_assert!(r, info.gainmap_ratio_max.b == 32.0);

    reporter_assert!(r, info.gainmap_gamma.r == 1.0f32 / 1.2f32);
    reporter_assert!(r, info.gainmap_gamma.g == 1.0f32 / 1.2f32);
    reporter_assert!(r, info.gainmap_gamma.b == 1.0f32 / 1.2f32);

    reporter_assert!(r, info.epsilon_sdr.r == 0.1f32);
    reporter_assert!(r, info.epsilon_sdr.g == 0.1f32);
    reporter_assert!(r, info.epsilon_sdr.b == 0.1f32);

    reporter_assert!(r, info.epsilon_hdr.r == 0.2f32);
    reporter_assert!(r, info.epsilon_hdr.g == 0.3f32);
    reporter_assert!(r, info.epsilon_hdr.b == 0.4f32);
});

// Port of: tests/SkXmpTest.cpp#L169-L205 (chrome/m156)
def_test!(SkXmp_xmpContainerTypedNode, |r| {
    // Container and Item using a node of type Container:Item.
    let xmp_data = r#"
            <x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 5.5.0">
             <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
              <rdf:Description rdf:about=""
                xmlns:Container="http://ns.google.com/photos/1.0/container/"
                xmlns:Item="http://ns.google.com/photos/1.0/container/item/">
               <Container:Directory>
                <rdf:Seq>
                 <rdf:li rdf:parseType="Resource">
                  <Container:Item>
                   <Item:Mime>image/jpeg</Item:Mime>
                   <Item:Semantic>Primary</Item:Semantic>
                  </Container:Item>
                 </rdf:li>
                 <rdf:li rdf:parseType="Resource">
                  <Container:Item
                     Item:Semantic="GainMap"
                     Item:Mime="image/jpeg"
                     Item:Length="49035"/>
                 </rdf:li>
                </rdf:Seq>
               </Container:Directory>
              </rdf:Description>
             </rdf:RDF>
            </x:xmpmeta>"#;
    let app1_param = Data::new_copy(xmp_data.as_bytes());

    let Some(xmp) = Xmp::make(&app1_param) else {
        reporter_assert!(r, false);
        return;
    };

    let Some((_offset, size)) = xmp.container_gainmap_location() else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, size == 49035);
});

// Port of: tests/SkXmpTest.cpp#L207-L246 (chrome/m156)
def_test!(SkXmp_xmpContainerTypedNodeRdfEquivalent, |r| {
    // Container and Item using rdf:value and rdf:type pairs.
    let xmp_data = r#"
            <x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 5.5.0">
             <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
              <rdf:Description rdf:about=""
                xmlns:Container="http://ns.google.com/photos/1.0/container/"
                xmlns:Item="http://ns.google.com/photos/1.0/container/item/">
               <Container:Directory>
                <rdf:Seq>
                 <rdf:li rdf:parseType="Resource">
                  <rdf:value rdf:parseType="Resource">
                   <Item:Mime>image/jpeg</Item:Mime>
                   <Item:Semantic>Primary</Item:Semantic>
                  </rdf:value>
                  <rdf:type rdf:resource="Item"/>
                 </rdf:li>
                 <rdf:li rdf:parseType="Resource">
                  <rdf:value rdf:parseType="Resource">
                   <Item:Semantic>GainMap</Item:Semantic>
                   <Item:Mime>image/jpeg</Item:Mime>
                   <Item:Length>49035</Item:Length>
                  </rdf:value>
                  <rdf:type rdf:resource="Item"/>
                 </rdf:li>
                </rdf:Seq>
               </Container:Directory>
              </rdf:Description>
             </rdf:RDF>
            </x:xmpmeta>"#;
    let app1_param = Data::new_copy(xmp_data.as_bytes());

    let Some(xmp) = Xmp::make(&app1_param) else {
        reporter_assert!(r, false);
        return;
    };

    let Some((_offset, size)) = xmp.container_gainmap_location() else {
        reporter_assert!(r, false);
        return;
    };
    reporter_assert!(r, size == 49035);
});
