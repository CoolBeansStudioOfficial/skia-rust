#!/usr/bin/env python3
# Copyright 2026 The skia-rust Authors
# Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
"""Generates oracle/shaper-diff/corpus/ from the pinned Skia tree (docs/design/modules.md §2.4, item 4).

    python3 -I oracle/shaper-diff/gen_corpus.py <skia-checkout> <corpus-dir>

Sources:
  text-<name>.txt     resources/text/<name>.txt, copied unchanged (the 34 script samples).
  tests-strings.txt   the string literals of the shaper, paragraph, skottie-text and unicode tests
                      and the paragraph GM (one line per literal, escapes decoded; only literals
                      with a space or a non-ASCII character, deduplicated, in file order).
  skottie-text.txt    the text of every text layer ("t" of a text document) of resources/skottie/*.json.
  index.txt           the files above, sorted: the order both drivers use.
"""
import json
import os
import re
import shutil
import sys

SOURCES = [
    "modules/skshaper/tests/ShaperTest.cpp",
    "modules/skparagraph/tests/SkParagraphTest.cpp",
    "modules/skparagraph/tests/SkShaperJSONWriterTest.cpp",
    "modules/skparagraph/gm/simple_gm.cpp",
    "modules/skottie/tests/Shaper.cpp",
    "modules/skottie/tests/Text.cpp",
    "modules/skottie/tests/SkottieTest.cpp",
    "modules/skunicode/tests/SkUnicodeTest.cpp",
]

LITERAL = re.compile(r'(?:u8|u|U|L)?"((?:[^"\\\n]|\\.)*)"')
ESCAPES = {"n": "\n", "t": "\t", "r": "\r", "\\": "\\", '"': '"', "'": "'", "0": "\0", "a": "\a",
           "b": "\b", "f": "\f", "v": "\v", "?": "?"}


def decode(body):
    """Decodes a C++ string literal body into text (\\x and octal escapes are UTF-8 bytes)."""
    out = bytearray()
    i = 0
    while i < len(body):
        c = body[i]
        if c != "\\":
            out += c.encode("utf-8")
            i += 1
            continue
        i += 1
        e = body[i]
        if e in "uU":
            n = 4 if e == "u" else 8
            out += chr(int(body[i + 1:i + 1 + n], 16)).encode("utf-8")
            i += 1 + n
        elif e == "x":
            j = i + 1
            while j < len(body) and body[j] in "0123456789abcdefABCDEF":
                j += 1
            out.append(int(body[i + 1:j], 16) & 0xFF)
            i = j
        elif e in "01234567":
            j = i
            while j < len(body) and j < i + 3 and body[j] in "01234567":
                j += 1
            out.append(int(body[i:j], 8) & 0xFF)
            i = j
        else:
            out += ESCAPES.get(e, e).encode("utf-8")
            i += 1
    return out.decode("utf-8", errors="replace")


def literals(path):
    text = open(path, encoding="utf-8").read()
    text = re.sub(r"//[^\n]*", "", text)
    out = []
    pos = 0
    pending = None
    for m in LITERAL.finditer(text):
        # adjacent literals (only whitespace between) are one literal
        if pending is not None and text[pos:m.start()].strip() == "":
            pending += decode(m.group(1))
        else:
            if pending is not None:
                out.append(pending)
            pending = decode(m.group(1))
        pos = m.end()
    if pending is not None:
        out.append(pending)
    return out


def keep(s):
    if len(s) < 3 or "\0" in s:
        return False
    if any(ord(ch) > 127 for ch in s):
        return True
    return " " in s and any(ch.isalpha() for ch in s) and "%" not in s and "{" not in s


def walk_text_docs(node, out):
    if isinstance(node, dict):
        t = node.get("t")
        if isinstance(t, str) and "f" in node and "s" in node:
            out.append(t)
        for v in node.values():
            walk_text_docs(v, out)
    elif isinstance(node, list):
        for v in node:
            walk_text_docs(v, out)


def main():
    skia, dest = sys.argv[1], sys.argv[2]
    shutil.rmtree(dest, ignore_errors=True)
    os.makedirs(dest)
    names = []
    text_dir = os.path.join(skia, "resources", "text")
    for f in sorted(os.listdir(text_dir)):
        if f.endswith(".txt"):
            shutil.copyfile(os.path.join(text_dir, f), os.path.join(dest, "text-" + f))
            names.append("text-" + f)

    seen = set()
    lines = []
    for src in SOURCES:
        for lit in literals(os.path.join(skia, src)):
            if not keep(lit):
                continue
            for part in lit.split("\n"):
                if part and part not in seen and keep(part):
                    seen.add(part)
                    lines.append(part)
    with open(os.path.join(dest, "tests-strings.txt"), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    names.append("tests-strings.txt")

    texts = []
    seen = set()
    skottie = os.path.join(skia, "resources", "skottie")
    for root, _, files in sorted(os.walk(skottie)):
        for f in sorted(files):
            if not f.endswith(".json"):
                continue
            try:
                doc = json.load(open(os.path.join(root, f), encoding="utf-8"))
            except (ValueError, UnicodeDecodeError):
                continue
            found = []
            walk_text_docs(doc, found)
            for t in found:
                for part in t.replace("\r", "\n").split("\n"):
                    if part and part not in seen:
                        seen.add(part)
                        texts.append(part)
    with open(os.path.join(dest, "skottie-text.txt"), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(texts) + "\n")
    names.append("skottie-text.txt")

    with open(os.path.join(dest, "index.txt"), "w", newline="\n") as f:
        f.write("# Generated by gen_corpus.py; do not edit.\n")
        for n in sorted(names):
            f.write(n + "\n")
    print(len(lines), "test strings,", len(texts), "skottie texts,", len(names), "files")


main()
