// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkJpegXmpTest.cpp (chrome/m156), all three cases.
#![cfg(test)]
// The C++ asserts compare the parsed gainmap parameters with ==.
#![allow(clippy::float_cmp)]

use skia_rust_codec::jpeg_xmp::make_xmp;
use skia_rust_core::data::Data;
use skia_rust_core::gainmap_info::GainmapInfo;
use skia_rust_core::md5::Md5;

use crate::{def_test, reporter_assert};

// Port of: tests/SkJpegXmpTest.cpp#L16-L42 (chrome/m156)
def_test!(SkJpegXmp_standardXmp, |r| {
    let xmp_data = concat!(
        "http://ns.adobe.com/xap/1.0/\0",
        r#"
            <x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 6.0.0">
               <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
                        xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/">
                  <rdf:Description rdf:about="">
                     <hdrgm:Version>1.0</hdrgm:Version>
                     <hdrgm:GainMapMax>3</hdrgm:GainMapMax>
                     <hdrgm:HDRCapacityMax>4</hdrgm:HDRCapacityMax>
                  </rdf:Description>
               </rdf:RDF>
            </x:xmpmeta>"#
    );

    let app1_params = vec![Data::new_copy(xmp_data.as_bytes())];

    let Some(xmp) = make_xmp(&app1_params) else {
        reporter_assert!(r, false);
        return;
    };
    let mut info = GainmapInfo::default();
    reporter_assert!(r, xmp.get_gainmap_info_adobe(Some(&mut info)));
    reporter_assert!(r, info.gainmap_ratio_max.r == 8.0);
    reporter_assert!(r, info.display_ratio_hdr == 16.0);
});

// Port of: tests/SkJpegXmpTest.cpp#L44-L81 (chrome/m156)
def_test!(SkJpegXmp_defaultValues, |r| {
    let xmp_data = concat!(
        "http://ns.adobe.com/xap/1.0/\0",
        r#"
            <x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 6.0.0">
               <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
                        xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/">
                  <rdf:Description rdf:about="" hdrgm:Version="1.0">
                  </rdf:Description>
               </rdf:RDF>
            </x:xmpmeta>"#
    );

    let app1_params = vec![Data::new_copy(xmp_data.as_bytes())];

    let Some(xmp) = make_xmp(&app1_params) else {
        reporter_assert!(r, false);
        return;
    };
    let mut info = GainmapInfo::default();
    reporter_assert!(r, xmp.get_gainmap_info_adobe(Some(&mut info)));
    reporter_assert!(r, info.gainmap_ratio_min.r == 1.0);
    reporter_assert!(r, info.gainmap_ratio_min.g == 1.0);
    reporter_assert!(r, info.gainmap_ratio_min.b == 1.0);
    reporter_assert!(r, info.gainmap_ratio_max.r == 2.0);
    reporter_assert!(r, info.gainmap_ratio_max.g == 2.0);
    reporter_assert!(r, info.gainmap_ratio_max.b == 2.0);
    reporter_assert!(r, info.gainmap_gamma.r == 1.0);
    reporter_assert!(r, info.gainmap_gamma.g == 1.0);
    reporter_assert!(r, info.gainmap_gamma.b == 1.0);
    reporter_assert!(r, info.epsilon_sdr.r == 1.0 / 64.0);
    reporter_assert!(r, info.epsilon_sdr.g == 1.0 / 64.0);
    reporter_assert!(r, info.epsilon_sdr.b == 1.0 / 64.0);
    reporter_assert!(r, info.epsilon_hdr.g == 1.0 / 64.0);
    reporter_assert!(r, info.epsilon_hdr.r == 1.0 / 64.0);
    reporter_assert!(r, info.epsilon_hdr.b == 1.0 / 64.0);
    reporter_assert!(r, info.display_ratio_sdr == 1.0);
    reporter_assert!(r, info.display_ratio_hdr == 2.0);
});

// Port of: tests/SkJpegXmpTest.cpp#L83-L86 (chrome/m156), `uint32_to_string`.
fn uint32_to_string(v: u32) -> Vec<u8> {
    v.to_be_bytes().to_vec()
}

// Port of: tests/SkJpegXmpTest.cpp#L88-L92 (chrome/m156), `standard_xmp_with_header`.
fn standard_xmp_with_header(digest: &skia_rust_core::md5::Digest, data: &str) -> Vec<u8> {
    let guid = digest.to_hex_string();
    let data_with_guid = data.replace("$GUID", &guid);
    let mut out = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
    out.extend_from_slice(data_with_guid.as_bytes());
    out
}

// Port of: tests/SkJpegXmpTest.cpp#L94-L100 (chrome/m156), `extended_xmp_with_header`.
fn extended_xmp_with_header(
    digest: &skia_rust_core::md5::Digest,
    size: u32,
    offset: u32,
    data: &str,
) -> Vec<u8> {
    let mut out = b"http://ns.adobe.com/xmp/extension/\0".to_vec();
    out.extend_from_slice(digest.to_hex_string().as_bytes());
    out.extend_from_slice(&uint32_to_string(size));
    out.extend_from_slice(&uint32_to_string(offset));
    out.extend_from_slice(data.as_bytes());
    out
}

// Port of: tests/SkJpegXmpTest.cpp#L102-L161 (chrome/m156)
def_test!(SkJpegXmp_readExtendedXmp, |r| {
    let standard_xmp_data = r#"
            <x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 6.0.0">
               <rdf:RDF xmlns:xmpNote="http://ns.adobe.com/xmp/note/">
                  <rdf:Description rdf:about="">
                     <xmpNote:HasExtendedXMP>$GUID</xmpNote:HasExtendedXMP>
                  </rdf:Description>
               </rdf:RDF>
            </x:xmpmeta>"#;

    let extended_xmp_data1 = r#"
        <x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 6.0.0">
            <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
                    xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/">
                <rdf:Description rdf:about="">
                     <hdrgm:Version>1.0</hdrgm:Version>
                     <hdrgm:GainMapMax>3</hdrgm:GainMapMax>
                    <hdrgm:HDRCapacityMax>4</hdrgm:HDRCapacityMax>"#;
    let extended_xmp_data2 = r#"
                </rdf:Description>
            </rdf:RDF>
        </x:xmpmeta>"#;

    let total_extended_xmp_size = (extended_xmp_data1.len() + extended_xmp_data2.len()) as u32;
    let mut md5 = Md5::new();
    md5.write_bytes(extended_xmp_data1.as_bytes());
    md5.write_bytes(extended_xmp_data2.as_bytes());
    let digest = md5.finish();

    let standard_xmp_data_with_header = standard_xmp_with_header(&digest, standard_xmp_data);

    let offset1 = 0;
    let extended_xmp_data1_with_header = extended_xmp_with_header(
        &digest,
        total_extended_xmp_size,
        offset1,
        extended_xmp_data1,
    );

    let offset2 = extended_xmp_data1.len() as u32;
    let extended_xmp_data2_with_header = extended_xmp_with_header(
        &digest,
        total_extended_xmp_size,
        offset2,
        extended_xmp_data2,
    );

    let app1_params = vec![
        Data::new_copy(&standard_xmp_data_with_header),
        Data::new_copy(&extended_xmp_data1_with_header),
        Data::new_copy(&extended_xmp_data2_with_header),
    ];

    let Some(xmp) = make_xmp(&app1_params) else {
        reporter_assert!(r, false);
        return;
    };
    let mut info = GainmapInfo::default();
    reporter_assert!(r, xmp.get_gainmap_info_adobe(Some(&mut info)));
    reporter_assert!(r, info.gainmap_ratio_max.r == 8.0);
    reporter_assert!(r, info.display_ratio_hdr == 16.0);
});
