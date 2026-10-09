#!/usr/bin/env python3
# Copyright (C) 2025 The skia-rust Authors.
# Use of this source code is governed by the BSD-3-Clause licence in the LICENSE file.
"""Writes the animated and extended-format fixtures of the demux differential harness.

The corpus has no animated file, so these are built from the still images of `corpus/` by
wrapping their own image chunks (`VP8 `, `VP8L`, `ALPH`) in `ANMF` frames. The bitstreams are
copied byte for byte. The files are written to `corpus-anim/`, together with truncated copies
of each (the partial-data cases of `WebPDemuxPartial`).

Usage: python3 make_anim.py <corpus-dir> <out-dir>
"""
import os
import struct
import sys


def read_chunks(data):
    """Splits a RIFF WEBP file into (fourcc, payload) pairs, as stored."""
    assert data[:4] == b"RIFF" and data[8:12] == b"WEBP", "not a WebP file"
    end = min(len(data), 8 + struct.unpack("<I", data[4:8])[0])
    return split_chunks(data, 12, end)


def split_chunks(data, pos=0, end=None):
    """Splits a run of RIFF chunks into (fourcc, payload) pairs."""
    out = []
    end = len(data) if end is None else end
    while pos + 8 <= end:
        fourcc = data[pos:pos + 4]
        size = struct.unpack("<I", data[pos + 4:pos + 8])[0]
        payload = data[pos + 8:pos + 8 + size]
        out.append((fourcc, payload))
        pos += 8 + size + (size & 1)
    return out


def chunk(fourcc, payload):
    pad = b"\0" if len(payload) & 1 else b""
    return fourcc + struct.pack("<I", len(payload)) + payload + pad


def image_of(path):
    """(width, height, [chunk bytes for the frame]) of a still image, or of the first frame of
    an animation (its ANMF chunk, whose frame header gives the size)."""
    data = open(path, "rb").read()
    chunks = read_chunks(data)
    kinds = [c[0] for c in chunks]
    if b"ANMF" in kinds:
        payload = dict(chunks)[b"ANMF"] if kinds.count(b"ANMF") == 1 else next(
            p for k, p in chunks if k == b"ANMF")
        w = 1 + int.from_bytes(payload[6:9], "little")
        h = 1 + int.from_bytes(payload[9:12], "little")
        frame = [chunk(k, p) for k, p in split_chunks(payload, 16)]
        return w, h, frame
    if b"VP8L" in kinds:
        payload = dict(chunks)[b"VP8L"]
        bits = struct.unpack("<I", payload[1:5])[0]
        w = 1 + (bits & 0x3FFF)
        h = 1 + ((bits >> 14) & 0x3FFF)
        return w, h, [chunk(b"VP8L", payload)]
    vp8 = dict(chunks)[b"VP8 "]
    frame = [chunk(b"VP8 ", vp8)]
    alph = dict(chunks).get(b"ALPH")
    if alph is not None:
        frame.insert(0, chunk(b"ALPH", alph))
    # VP8 key frame header: 3-byte frame tag, start code 9d 01 2a, then 14-bit width and height.
    w = struct.unpack("<H", vp8[6:8])[0] & 0x3FFF
    h = struct.unpack("<H", vp8[8:10])[0] & 0x3FFF
    return w, h, frame


def anmf(x, y, w, h, duration, dispose, no_blend, frame_chunks):
    bits = (1 if dispose else 0) | (2 if no_blend else 0)
    header = (
        struct.pack("<I", x // 2)[:3]
        + struct.pack("<I", y // 2)[:3]
        + struct.pack("<I", w - 1)[:3]
        + struct.pack("<I", h - 1)[:3]
        + struct.pack("<I", duration)[:3]
        + bytes([bits])
    )
    return chunk(b"ANMF", header + b"".join(frame_chunks))


def vp8x(flags, w, h):
    payload = bytes([flags, 0, 0, 0]) + struct.pack("<I", w - 1)[:3] + struct.pack("<I", h - 1)[:3]
    return chunk(b"VP8X", payload)


def riff(chunks):
    body = b"WEBP" + b"".join(chunks)
    return b"RIFF" + struct.pack("<I", len(body)) + body


ANIM_FLAG = 0x02
XMP_FLAG = 0x04
EXIF_FLAG = 0x08
ALPHA_FLAG = 0x10
ICCP_FLAG = 0x20


def write(out_dir, name, data):
    with open(os.path.join(out_dir, name), "wb") as f:
        f.write(data)


def main():
    corpus, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    stills = [
        "stoplight.webp", "stoplight_h.webp", "required.webp", "randPixels.webp",
        "color_wheel.webp", "webp-color-profile-lossy-alpha.webp", "blendBG.webp",
    ]
    frames = [image_of(os.path.join(corpus, s)) for s in stills]
    # Frame i sits at offset (2i, 2i), so the canvas must cover offset + size for every frame.
    canvas_w = max(2 * i + w for i, (w, _, _) in enumerate(frames))
    canvas_h = max(2 * i + h for i, (_, h, _) in enumerate(frames))
    icc = chunk(b"ICCP", b"\x00" * 3 + b"acsp" + b"\x11" * 60)
    exif = chunk(b"EXIF", b"II*\x00" + b"\x08\x00\x00\x00" + b"\x00" * 10)
    xmp = chunk(b"XMP ", b"<x:xmpmeta/>")

    anim = []
    for i, (w, h, fr) in enumerate(frames):
        anim.append(anmf(2 * i, 2 * i, w, h, 40 + 7 * i, i % 2 == 1, i % 3 == 2, fr))
    anim_chunk = chunk(b"ANIM", struct.pack("<I", 0x80402010) + struct.pack("<H", 3))
    basic = riff(
        [vp8x(ANIM_FLAG | ALPHA_FLAG | ICCP_FLAG | EXIF_FLAG | XMP_FLAG, canvas_w, canvas_h), icc, anim_chunk]
        + anim + [exif, xmp]
    )
    write(out, "anim_basic.webp", basic)

    # The same frames without the feature flags: ICCP, EXIF and XMP are parsed but not stored.
    write(out, "anim_noflags.webp", riff(
        [vp8x(ANIM_FLAG, canvas_w, canvas_h), icc, anim_chunk] + anim + [exif, xmp]))

    # Frames before ANIM: an error.
    write(out, "anim_no_anim.webp", riff([vp8x(ANIM_FLAG, canvas_w, canvas_h), anim[0], anim_chunk]))

    # A last frame with an ALPH chunk and no image: an incomplete frame.
    # The lossy-alpha still has an ALPH chunk: its frame without the VP8 chunk is incomplete.
    w, h, fr = frames[5]
    alph_only = [c for c in fr if c[:4] == b"ALPH"]
    assert alph_only, "the lossy-alpha frame has no ALPH chunk"
    write(out, "anim_incomplete.webp", riff(
        [vp8x(ANIM_FLAG, canvas_w, canvas_h), anim_chunk] + anim[:2]
        + [anmf(0, 0, w, h, 10, False, False, alph_only)]))

    # A still image in the extended format with the metadata flags (not animated).
    still_w, still_h, still_fr = frames[0]
    write(out, "still_extended.webp", riff(
        [vp8x(ICCP_FLAG | EXIF_FLAG | XMP_FLAG | ALPHA_FLAG, still_w, still_h), icc] + still_fr + [exif, xmp]))

    # A frame that does not fit the canvas: an error in the animated format.
    write(out, "anim_out_of_bounds.webp", riff(
        [vp8x(ANIM_FLAG, 16, 16), anim_chunk, anmf(0, 0, frames[1][0], frames[1][1], 10, False, False, frames[1][2])]))

    # Truncated copies of the animation, for the partial demuxer.
    n = len(basic)
    for cut in [12, 20, 30, 38, 60, 90, 200, n // 3, n // 2, 2 * n // 3, n - 40, n - 1]:
        write(out, "anim_basic.t%d.webp" % cut, basic[:cut])


if __name__ == "__main__":
    main()
