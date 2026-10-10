// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkDOMTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::stream::MemoryStream;
use skia_rust_core::xml::{Dom as SkDOM, Node, NodeType};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/SkDOMTest.cpp#L16-L25 (chrome/m156)
fn check_node(
    r: &mut Reporter,
    dom: &SkDOM,
    node: Option<Node>,
    expected_name: &str,
    expected_type: NodeType,
) -> Option<Node> {
    reporter_assert!(r, node.is_some());
    if let Some(node) = node {
        reporter_assert!(r, dom.get_name(node) == expected_name);
        reporter_assert!(r, dom.get_type(node) == expected_type);
    }
    node
}

// Port of: tests/SkDOMTest.cpp#L26-L84 (chrome/m156)
def_test!(SkDOM_test, |r| {
    const G_DOC: &str = "<root a='1' b='2'>\
            <elem1 c='3' />\
            <elem2 d='4' />\
            <elem3 e='5'>\
                <subelem1>Some text.</subelem1>\
                <subelem2 f='6' g='7'/>\
                <subelem3>Some more text.</subelem3>\
            </elem3>\
            <elem4 h='8'/>\
        </root>";

    let mut doc_stream = MemoryStream::make_copy(G_DOC.as_bytes());

    let mut dom = SkDOM::new();
    reporter_assert!(r, dom.root_node().is_none());

    let root = dom.build(&mut *doc_stream, None);
    reporter_assert!(r, root.is_some() && dom.root_node() == root);
    let root = root.unwrap();

    let v = dom.find_attr(root, "a");
    reporter_assert!(r, v == Some("1"));
    let v = dom.find_attr(root, "b");
    reporter_assert!(r, v == Some("2"));
    let v = dom.find_attr(root, "c");
    reporter_assert!(r, v.is_none());

    reporter_assert!(r, dom.get_first_child(root, Some("elem1")).is_some());
    reporter_assert!(r, dom.get_first_child(root, Some("subelem1")).is_none());

    {
        let elem1 = check_node(
            r,
            &dom,
            dom.get_first_child(root, None),
            "elem1",
            NodeType::Element,
        );
        let elem2 = check_node(
            r,
            &dom,
            dom.get_next_sibling(elem1.unwrap(), None),
            "elem2",
            NodeType::Element,
        );
        let elem3 = check_node(
            r,
            &dom,
            dom.get_next_sibling(elem2.unwrap(), None),
            "elem3",
            NodeType::Element,
        );
        {
            let subelem1 = check_node(
                r,
                &dom,
                dom.get_first_child(elem3.unwrap(), None),
                "subelem1",
                NodeType::Element,
            );
            {
                check_node(
                    r,
                    &dom,
                    dom.get_first_child(subelem1.unwrap(), None),
                    "Some text.",
                    NodeType::Text,
                );
            }
            let subelem2 = check_node(
                r,
                &dom,
                dom.get_next_sibling(subelem1.unwrap(), None),
                "subelem2",
                NodeType::Element,
            );
            let subelem3 = check_node(
                r,
                &dom,
                dom.get_next_sibling(subelem2.unwrap(), None),
                "subelem3",
                NodeType::Element,
            );
            {
                check_node(
                    r,
                    &dom,
                    dom.get_first_child(subelem3.unwrap(), None),
                    "Some more text.",
                    NodeType::Text,
                );
            }
        }
        check_node(
            r,
            &dom,
            dom.get_next_sibling(elem3.unwrap(), None),
            "elem4",
            NodeType::Element,
        );
    }
});
