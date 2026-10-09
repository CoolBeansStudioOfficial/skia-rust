#!/usr/bin/env python3
# Copyright (C) 2025 The skia-rust Authors.
# Use of this source code is governed by the BSD-3-Clause licence in the LICENSE file.
"""Writes the truncated and corrupted variants of the corpus used by the differential harness.

Usage: python3 mutate.py <corpus-dir> <out-dir>

Writes the variants and prints their names in generation order (the order of expected_mutated.txt).

expected_mutated.txt is the C output on these files, in generation order:

    python3 mutate.py corpus <out> > <names>
    webpdump <out>/<names, in order>  > expected_mutated.txt

The variants are a pure function of the corpus bytes, so the Rust replay test
(crates/skia-rust-libwebp/tests/mutated.rs) regenerates the same bytes in-process with the same
rules (`variants` below). Files are named `<base>.t<len>.webp` (truncated to `len` bytes) and
`<base>.c<k>.webp` (corrupted: `CORRUPTIONS` bytes flipped, positions from a 64-bit xorshift
seeded by the file's index in the corpus).
"""

import os
import sys

MASK = (1 << 64) - 1
CORRUPTIONS = 2


def xorshift(state):
    state ^= (state << 13) & MASK
    state ^= state >> 7
    state ^= (state << 17) & MASK
    return state


def variants(name, data, index):
    """Yields (variant name, bytes) for one corpus file, in a fixed order."""
    n = len(data)
    lengths = []
    for cut in [0, 1, 8, 12, 20, 30, n // 4, n // 2, (3 * n) // 4, n - 1]:
        if 0 <= cut < n and cut not in lengths:
            lengths.append(cut)
    for cut in lengths:
        yield f"{name}.t{cut}.webp", data[:cut]
    state = (index + 1) * 0x9E3779B97F4A7C15 & MASK
    for k in range(CORRUPTIONS):
        buf = bytearray(data)
        for _ in range(4):
            state = xorshift(state)
            pos = state % n
            buf[pos] ^= ((state >> 8) & 0xFF) | 1
        yield f"{name}.c{k}.webp", bytes(buf)


def main():
    corpus, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    names = sorted(f for f in os.listdir(corpus) if f.endswith(".webp"))
    for index, name in enumerate(names):
        with open(os.path.join(corpus, name), "rb") as f:
            data = f.read()
        base = name[: -len(".webp")]
        for vname, vdata in variants(base, data, index):
            with open(os.path.join(out, vname), "wb") as f:
                f.write(vdata)
            print(vname)  # the generation order, which the expected output follows


if __name__ == "__main__":
    main()
