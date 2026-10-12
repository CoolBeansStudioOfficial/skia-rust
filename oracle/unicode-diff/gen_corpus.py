#!/usr/bin/env python3
# Copyright 2026 The skia-rust Authors
# Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
r"""Generates oracle/unicode-diff/corpus/ from the pinned Skia tree (docs/design/modules.md §3.3).

    python3 -I oracle/unicode-diff/gen_corpus.py <skia-checkout> <corpus-dir>

Unlike shaper-diff's corpus, a "string" here is what SkUnicode would be handed as one paragraph, so
the 34 `resources/text/*.txt` files stay whole (their newlines matter to line breaking and to the
rule-status quirk of §3.2). Files:
  text-<name>.txt     resources/text/<name>.txt, copied unchanged: one string per file.
  tests-strings.txt   every string literal of the shaper, paragraph, skottie and unicode tests and the
                      paragraph GM (deduplicated, in file order, no length filter): one string per line.
  skottie-text.txt    the text of every text layer of resources/skottie/*.json, one string per line.
  synthetic.txt       synthetic.txt of this directory, copied unchanged: hand-written cases for the
                      rules and patches in play.
  index.txt           `name kind` per file, sorted by name. kind `whole`: the file is one string.
                      kind `lines`: one string per line, with the escapes \\ \n \r \t \xHH (the code point U+00HH) \uXXXX \UXXXXXXXX.
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

HERE = os.path.dirname(os.path.abspath(__file__))
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


def escape(s):
    out = []
    for ch in s:
        if ch == "\\":
            out.append("\\\\")
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\r":
            out.append("\\r")
        elif ch == "\t":
            out.append("\\t")
        elif ord(ch) < 0x20 or ord(ch) == 0x7F:
            out.append("\\x%02X" % ord(ch))
        else:
            out.append(ch)
    return "".join(out)


def valid(s):
    if not s or "\0" in s:
        return False
    try:
        s.encode("utf-8")
    except UnicodeEncodeError:
        return False
    return "�" not in s


def main():
    skia, dest = sys.argv[1], sys.argv[2]
    shutil.rmtree(dest, ignore_errors=True)
    os.makedirs(dest)
    entries = []
    text_dir = os.path.join(skia, "resources", "text")
    for f in sorted(os.listdir(text_dir)):
        if f.endswith(".txt"):
            shutil.copyfile(os.path.join(text_dir, f), os.path.join(dest, "text-" + f))
            entries.append(("text-" + f, "whole"))

    seen = set()
    lines = []
    for src in SOURCES:
        for lit in literals(os.path.join(skia, src)):
            if valid(lit) and lit not in seen:
                seen.add(lit)
                lines.append(lit)
    with open(os.path.join(dest, "tests-strings.txt"), "w", encoding="utf-8", newline="\n") as f:
        f.write("".join(escape(s) + "\n" for s in lines))
    entries.append(("tests-strings.txt", "lines"))

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
                if valid(t) and t not in seen:
                    seen.add(t)
                    texts.append(t)
    with open(os.path.join(dest, "skottie-text.txt"), "w", encoding="utf-8", newline="\n") as f:
        f.write("".join(escape(s) + "\n" for s in texts))
    entries.append(("skottie-text.txt", "lines"))

    shutil.copyfile(os.path.join(HERE, "synthetic.txt"), os.path.join(dest, "synthetic.txt"))
    entries.append(("synthetic.txt", "lines"))

    with open(os.path.join(dest, "index.txt"), "w", newline="\n") as f:
        f.write("# Generated by gen_corpus.py; do not edit.\n")
        for n, kind in sorted(entries):
            f.write(f"{n} {kind}\n")
    print(len(lines), "test strings,", len(texts), "skottie texts,", len(entries), "files")


main()
