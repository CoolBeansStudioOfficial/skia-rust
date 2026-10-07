#!/usr/bin/env python3
"""Generates crates/skia-rust-raster/src/draw_tests/cases.txt for oracle/draw/draw.cpp.

Deterministic: all randomness comes from the LCG below (Knuth's MMIX constants).
Usage:
  python gen_cases.py [out_path]
The case grammar is documented at the top of draw.cpp. Floats are written with '%.9g' of the
float32-rounded value, so C++ strtof and Rust's f32 parsing read identical values.
"""
import math
import os
import struct
import sys

M64 = (1 << 64) - 1


class Rng:
    def __init__(self, seed):
        self.s = seed & M64

    def u32(self):
        self.s = (self.s * 6364136223846793005 + 1442695040888963407) & M64
        return self.s >> 32

    def rint(self, lo, hi):
        return lo + self.u32() % (hi - lo + 1)

    def chance(self, num, den):
        return self.u32() % den < num

    def pick(self, seq):
        return seq[self.u32() % len(seq)]

    def fl(self, lo, hi):
        # A float in [lo, hi] with 1/64 resolution plus an odd fraction now and then.
        v = lo + (self.u32() % 1000001) / 1000000.0 * (hi - lo)
        if self.chance(1, 3):
            v = round(v * 4) / 4.0
        return v


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


def F(x):
    v = f32(float(x))
    if math.isinf(v) or math.isnan(v):
        raise ValueError("non-finite float %r" % x)
    return "%.9g" % v


def Fs(*vs):
    return " ".join(F(v) for v in vs)


def C(argb):
    return "%08X" % (argb & 0xFFFFFFFF)


# ---------------------------------------------------------------------------------------------
# Cases
# ---------------------------------------------------------------------------------------------

CASES = []
NAMES = set()


class Case:
    def __init__(self, name):
        base = name
        n = 2
        while name in NAMES:
            name = "%s_%d" % (base, n)
            n += 1
        NAMES.add(name)
        self.name = name
        self.lines = []
        self.w = self.h = 0
        self.saves = 0
        CASES.append(self)

    def add(self, line):
        self.lines.append(line)
        return self

    def device(self, w, h, ct, at, cs, pad=0):
        self.w, self.h = w, h
        self.ct = ct
        self.add("device %d %d %s %s %s%s" % (w, h, ct, at, cs, (" %d" % pad) if pad else ""))
        return self

    def save(self):
        self.saves += 1
        return self.add("save")

    def restore(self):
        assert self.saves > 0
        self.saves -= 1
        return self.add("restore")

    def text(self):
        out = ["case " + self.name] + self.lines
        if not self.lines or self.lines[-1] != "end":
            out.append("end")
        return "\n".join(out)


# ---- devices ---------------------------------------------------------------------------------

CTS = ["n32", "a8", "gray8", "rgb565", "argb4444", "rgbaf16", "rgbaf32", "rgba1010102"]
ATS = {
    "n32": ["premul", "opaque", "unpremul"],
    "a8": ["premul"],
    "gray8": ["opaque"],
    "rgb565": ["opaque"],
    "argb4444": ["premul", "opaque"],
    "rgbaf16": ["premul", "opaque", "unpremul"],
    "rgbaf32": ["premul", "opaque", "unpremul"],
    "rgba1010102": ["premul", "opaque"],
}
CSS = ["srgb", "linear", "none"]
SIZES = [(40, 32, 0), (64, 48, 0), (33, 27, 3), (40, 32, 5), (17, 13, 1), (48, 40, 0)]

DEVCONFIGS = [(ct, at, cs) for ct in CTS for at in ATS[ct] for cs in CSS]


class Cycler:
    """Round robin over a list, so every value shows up in many combinations."""

    def __init__(self, items, start=0):
        self.items = items
        self.i = start

    def next(self):
        v = self.items[self.i % len(self.items)]
        self.i += 1
        return v


DEV_CYCLE = Cycler(DEVCONFIGS)
SIZE_CYCLE = Cycler(SIZES)
# Mostly n32 with all the others mixed in.
N32_HEAVY = Cycler(
    [("n32", "premul", "srgb"), ("n32", "premul", "none"), ("n32", "opaque", "srgb"),
     ("a8", "premul", "none"), ("rgbaf16", "premul", "linear"), ("rgb565", "opaque", "srgb"),
     ("n32", "unpremul", "srgb"), ("n32", "premul", "linear"), ("argb4444", "premul", "none"),
     ("rgbaf32", "premul", "srgb"), ("gray8", "opaque", "none"), ("rgba1010102", "premul", "srgb"),
     ("n32", "premul", "srgb"), ("rgbaf16", "unpremul", "srgb")])

TRANSLUCENT = [0x80336699, 0x40FF0000, 0xC000A0FF, 0x7F7F7F7F, 0x2010E040, 0xFE102030,
               0x99FFCC00, 0x5A00FF80]
# Nearly or fully transparent colors, used sparingly.
FAINT = [0x00336699, 0x01FFFFFF, 0x03804020]
OPAQUE = [0xFF336699, 0xFF000000, 0xFFFFFFFF, 0xFF20C040, 0xFFE01060]
COLORS = TRANSLUCENT + OPAQUE + TRANSLUCENT[:4] + [FAINT[0]]
BGS = [0xFFFFFFFF, 0x80204080, 0xFF102030, 0x00000000, 0xC0F0E0D0, 0xFF808080]


def bg(c, rng):
    if rng.chance(1, 2):
        c.add("noise %d" % rng.rint(1, 100000))
    else:
        c.add("erase " + C(rng.pick(BGS)))


def new_case(name, rng, cfg=None, size=None, background=True):
    if cfg is None:
        cfg = DEV_CYCLE.next()
    if size is None:
        size = SIZE_CYCLE.next()
    ct, at, cs = cfg
    w, h, pad = size
    c = Case("%s_%s_%s_%s" % (name, ct, at[0:2], cs[0:2]))
    c.device(w, h, ct, at, cs, pad)
    if background:
        bg(c, rng)
    return c


# ---- matrices --------------------------------------------------------------------------------

def mat(*v):
    return "ctm " + Fs(*v)


def rot(deg, cx, cy):
    a = math.radians(deg)
    cs_, sn = math.cos(a), math.sin(a)
    if deg == 90:
        cs_, sn = 0.0, 1.0
    # T(cx,cy) * R * T(-cx,-cy)
    return mat(cs_, -sn, cx - cs_ * cx + sn * cy, sn, cs_, cy - sn * cx - cs_ * cy, 0, 0, 1)


def ctms(w, h):
    cx, cy = w / 2.0, h / 2.0
    return {
        "id": "ctm id",
        "tri": mat(1, 0, 3, 0, 1, 5, 0, 0, 1),
        "trf": mat(1, 0, 0.375, 0, 1, 0.6, 0, 0, 1),
        "sc2": mat(2, 0, -cx * 0.5, 0, 2, -cy * 0.5, 0, 0, 1),
        "sch": mat(0.5, 0, cx * 0.5, 0, 0.5, cy * 0.5, 0, 0, 1),
        "scxy": mat(1.5, 0, 0.25, 0, 0.75, 1.5, 0, 0, 1),
        "flip": mat(-1, 0, w, 0, 1, 0, 0, 0, 1),
        "negs": mat(-1.25, 0, w + 2.5, 0, -0.8, h - 1.5, 0, 0, 1),
        "rot30": rot(30, cx, cy),
        "rot45": rot(45, cx, cy),
        "rot90": rot(90, cx, cy),
        "skew": mat(1, 0.3, -2, 0.2, 1, -1, 0, 0, 1),
        "persp": mat(1, 0.1, 0, 0.05, 1, 0, 0.002, 0.001, 1),
    }


CTM_NAMES = ["id", "tri", "trf", "sc2", "sch", "scxy", "flip", "negs", "rot30", "rot45", "rot90",
             "skew", "persp"]


# ---- paints ----------------------------------------------------------------------------------

def paint(**kw):
    order = ["color", "aa", "dither", "style", "width", "cap", "join", "miter", "blend",
             "shader", "pe"]
    parts = ["paint"]
    for k in order:
        if k in kw and kw[k] is not None:
            v = kw[k]
            if k in ("color",):
                v = C(v)
            elif k in ("width", "miter"):
                v = F(v)
            elif k in ("aa", "dither"):
                v = "1" if v else "0"
            parts.append("%s=%s" % (k, v))
    return " ".join(parts)


def dash_spec(iv, phase):
    return "dash:" + ",".join(F(x) for x in iv) + "@" + F(phase)


STYLES = [
    ("fill", dict(style="fill")),
    ("hair", dict(style="stroke", width=0)),
    ("s0.1", dict(style="stroke", width=0.1)),
    ("s0.5", dict(style="stroke", width=0.5)),
    ("s1", dict(style="stroke", width=1)),
    ("s2.5", dict(style="stroke", width=2.5)),
    ("s6r", dict(style="stroke", width=6, join="round", cap="round")),
    ("s5b", dict(style="stroke", width=5, join="bevel", cap="square")),
    ("s4m1", dict(style="stroke", width=4, join="miter", miter=1)),
    ("s4m10", dict(style="stroke", width=4, join="miter", miter=10)),
    ("sf0", dict(style="strokefill", width=0)),
    ("sf3", dict(style="strokefill", width=3)),
]


def rand_paint(rng, allow_pe=True, style=True, blend=True):
    kw = dict(color=rng.pick(COLORS), aa=rng.chance(1, 2))
    if style:
        sname, s = rng.pick(STYLES)
        kw.update(s)
    if rng.chance(1, 8):
        kw["dither"] = True
    if blend and rng.chance(1, 4):
        kw["blend"] = rng.pick([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25, 26, 27, 28])
    if rng.chance(1, 8):
        kw["shader"] = "solid:" + C(rng.pick(COLORS))
    if allow_pe and rng.chance(1, 6):
        kw["pe"] = rand_pe(rng)
    return paint(**kw)


def rand_pe(rng):
    k = rng.rint(0, 5)
    if k <= 1:
        n = rng.pick([2, 2, 4, 6])
        iv = [rng.pick([0.5, 1, 2, 3, 4.5, 6, 0.25]) for _ in range(n)]
        return dash_spec(iv, rng.pick([0, 0.5, 1.75, 3, 10]))
    if k == 2 or k == 3:
        return "corner:" + F(rng.pick([1, 2.5, 5, 12]))
    if k == 4:
        return "sum_dash_corner:" + dash_spec([rng.pick([2, 3, 5]), rng.pick([1, 2, 4])],
                                              rng.pick([0, 1.5]))[5:] + ":" + F(rng.pick([2, 4]))
    return "compose_corner_dash:" + dash_spec([rng.pick([2, 3, 5]), rng.pick([1, 2, 4])],
                                              rng.pick([0, 1.5]))[5:] + ":" + F(rng.pick([2, 4]))


# ---- paths -----------------------------------------------------------------------------------

def path_tokens(verbs):
    out = []
    for v in verbs:
        out.append(v[0])
        out.extend(F(x) for x in v[1:])
    return " ".join(out)


def star(cx, cy, r0, r1, n, rotate=0.0):
    verbs = []
    for i in range(2 * n):
        a = math.pi * i / n + rotate
        r = r0 if i % 2 == 0 else r1
        verbs.append(("M" if i == 0 else "L", cx + r * math.cos(a), cy + r * math.sin(a)))
    verbs.append(("Z",))
    return verbs


def pentagram(cx, cy, r):
    verbs = []
    for i in range(5):
        a = -math.pi / 2 + i * 4 * math.pi / 5
        verbs.append(("M" if i == 0 else "L", cx + r * math.cos(a), cy + r * math.sin(a)))
    verbs.append(("Z",))
    return verbs


def circle_conics(cx, cy, r):
    w = math.sqrt(0.5)
    return [("M", cx + r, cy), ("K", cx + r, cy + r, cx, cy + r, w), ("K", cx - r, cy + r, cx - r, cy, w),
            ("K", cx - r, cy - r, cx, cy - r, w), ("K", cx + r, cy - r, cx + r, cy, w), ("Z",)]


def rect_verbs(l, t, r, b, ccw=False):
    if ccw:
        return [("M", l, t), ("L", l, b), ("L", r, b), ("L", r, t), ("Z",)]
    return [("M", l, t), ("L", r, t), ("L", r, b), ("L", l, b), ("Z",)]


def paths(w, h):
    """Named path verb lists, scaled to a w x h device."""
    sx, sy = w / 40.0, h / 32.0
    P = {}
    P["tri"] = [("M", 3 * sx, 2 * sy), ("L", 36 * sx, 9.5 * sy), ("L", 12.25 * sx, 29 * sy), ("Z",)]
    P["star"] = pentagram(20 * sx, 16 * sy, 14 * min(sx, sy))
    P["star7"] = star(20 * sx, 16 * sy, 14 * min(sx, sy), 6 * min(sx, sy), 7, 0.1)
    P["bowtie"] = [("M", 4 * sx, 4 * sy), ("L", 36 * sx, 28 * sy), ("L", 36 * sx, 4 * sy),
                   ("L", 4 * sx, 28 * sy), ("Z",)]
    P["cubicloop"] = [("M", 5 * sx, 25 * sy), ("C", 45 * sx, -5 * sy, -5 * sx, -5 * sy, 35 * sx, 25 * sy),
                      ("Z",)]
    P["cubics"] = [("M", 2 * sx, 16 * sy), ("C", 10 * sx, -4 * sy, 30 * sx, 36 * sy, 38 * sx, 16 * sy),
                   ("C", 30 * sx, 30 * sy, 10 * sx, 30 * sy, 2 * sx, 16 * sy), ("Z",)]
    P["quads"] = [("M", 3 * sx, 28 * sy), ("Q", 10 * sx, 1 * sy, 20 * sx, 20 * sy),
                  ("Q", 30 * sx, 38 * sy, 37 * sx, 4 * sy), ("L", 37 * sx, 28 * sy), ("Z",)]
    P["circle"] = circle_conics(20 * sx, 16 * sy, 12.5 * min(sx, sy))
    P["conics"] = [("M", 4 * sx, 28 * sy), ("K", 4 * sx, 4 * sy, 20 * sx, 4 * sy, 2.5),
                   ("K", 36 * sx, 4 * sy, 36 * sx, 28 * sy, 0.3), ("Z",)]
    P["holes"] = (rect_verbs(2 * sx, 2 * sy, 38 * sx, 30 * sy)
                  + rect_verbs(8 * sx, 8 * sy, 18.5 * sx, 24 * sy, ccw=True)
                  + rect_verbs(22 * sx, 6 * sy, 33 * sx, 20.5 * sy)
                  + circle_conics(27 * sx, 13 * sy, 3 * min(sx, sy)))
    P["open"] = [("M", 3 * sx, 3 * sy), ("L", 30 * sx, 6 * sy), ("Q", 38 * sx, 18 * sy, 20 * sx, 28 * sy),
                 ("L", 5 * sx, 20 * sy)]
    P["open2"] = [("M", 3 * sx, 28 * sy), ("L", 20 * sx, 4 * sy), ("L", 36 * sx, 28 * sy),
                  ("M", 6 * sx, 14 * sy), ("C", 14 * sx, 2 * sy, 26 * sx, 30 * sy, 34 * sx, 14 * sy)]
    P["zerolen"] = [("M", 10 * sx, 10 * sy), ("L", 10 * sx, 10 * sy), ("M", 20 * sx, 16 * sy),
                    ("L", 20 * sx, 16 * sy), ("Z",), ("M", 30 * sx, 22 * sy),
                    ("C", 30 * sx, 22 * sy, 30 * sx, 22 * sy, 30 * sx, 22 * sy)]
    P["zeroseg"] = [("M", 4 * sx, 4 * sy), ("L", 20 * sx, 4 * sy), ("L", 20 * sx, 4 * sy),
                    ("L", 36 * sx, 28 * sy), ("Q", 36 * sx, 28 * sy, 36 * sx, 28 * sy), ("L", 4 * sx, 28 * sy),
                    ("Z",)]
    P["thin"] = [("M", 2 * sx, 10 * sy), ("L", 38 * sx, 10.3 * sy), ("L", 38 * sx, 10.6 * sy),
                 ("L", 2 * sx, 10.4 * sy), ("Z",)]
    P["big1e6"] = [("M", -1e6, -1e6), ("L", 1e6, 5 * sy), ("L", 20 * sx, 1e6), ("Z",)]
    P["bigline"] = [("M", -1e6, 3 * sy), ("L", 1e6, 29 * sy), ("L", 1e6, 30 * sy), ("Z",)]
    P["big3e7"] = [("M", -3e7, 2 * sy), ("L", 3e7, 20 * sy), ("L", 20 * sx, 3e7), ("Z",)]
    P["offdev"] = [("M", 100 * sx, 100 * sy), ("L", 140 * sx, 110 * sy), ("L", 120 * sx, 150 * sy), ("Z",)]
    P["offneg"] = [("M", -50, -40), ("L", -10, -45), ("L", -20, -5), ("Z",)]
    P["huge"] = [("M", -4e4, -4e4), ("L", 4e4, -3e4), ("L", 4e4, 4e4), ("L", -3e4, 4e4), ("Z",)]
    P["frac"] = [("M", 3.3 * sx, 2.7 * sy), ("L", 33.6 * sx, 5.1 * sy), ("L", 30.2 * sx, 27.9 * sy),
                 ("L", 6.45 * sx, 24.05 * sy), ("Z",)]
    P["spiral"] = [("M", 20 * sx, 16 * sy)] + [
        ("L", 20 * sx + (1 + i) * 0.8 * math.cos(i * 0.7) * sx, 16 * sy + (1 + i) * 0.7 * math.sin(i * 0.7) * sy)
        for i in range(22)]
    return P


PATH_NAMES = ["tri", "star", "star7", "bowtie", "cubicloop", "cubics", "quads", "circle", "conics",
              "holes", "open", "open2", "zerolen", "zeroseg", "thin", "big1e6", "bigline",
              "big3e7", "offdev", "offneg", "huge", "frac", "spiral"]
COMMON_PATHS = ["tri", "star", "star7", "bowtie", "cubicloop", "cubics", "quads", "circle",
                "conics", "holes", "open", "open2", "zeroseg", "frac", "spiral"]
FILLS = ["w", "e", "iw", "ie"]


def path_cmd(cmd, fill, verbs):
    return "%s %s %s" % (cmd, fill, path_tokens(verbs))


# ---- shapes ----------------------------------------------------------------------------------

def rand_rect(rng, w, h):
    l = rng.fl(-4, w * 0.6)
    t = rng.fl(-4, h * 0.6)
    return (l, t, l + rng.fl(1, w * 0.7), t + rng.fl(1, h * 0.7))


def rand_shape(rng, w, h):
    k = rng.rint(0, 9)
    l, t, r, b = rand_rect(rng, w, h)
    if k == 0:
        return "rect " + Fs(l, t, r, b)
    if k == 1:
        return "oval " + Fs(l, t, r, b)
    if k == 2:
        return "rrect " + Fs(l, t, r, b, rng.fl(0, 8), rng.fl(0, 8))
    if k == 3:
        return "rrect4 " + Fs(l, t, r, b, *[rng.fl(0, 6) for _ in range(8)])
    if k == 4:
        return "drrect " + Fs(l - 3, t - 3, r + 3, b + 3, 4, 4, l + 2, t + 2, r - 2, b - 2, 2, 1)
    if k == 5:
        n = rng.rint(1, 3)
        rs = []
        for _ in range(n):
            x0, y0 = rng.rint(-3, w - 2), rng.rint(-3, h - 2)
            rs += [x0, y0, x0 + rng.rint(1, w // 2), y0 + rng.rint(1, h // 2)]
        return "region %d %s" % (n, " ".join(str(v) for v in rs))
    if k == 6:
        n = rng.rint(1, 12)
        mode = rng.pick(["points", "lines", "polygon"])
        pts = []
        for _ in range(n):
            pts += [rng.fl(-2, w + 2), rng.fl(-2, h + 2)]
        return "points %s %d %s" % (mode, n, Fs(*pts))
    if k == 7:
        return "drawpaint"
    P = paths(w, h)
    return path_cmd("path", rng.pick(FILLS), P[rng.pick(COMMON_PATHS)])


# ---------------------------------------------------------------------------------------------
# Sections
# ---------------------------------------------------------------------------------------------

def sec_drawpaint(rng):
    for i, cfg in enumerate(DEVCONFIGS):
        for j in range(2):
            col = rng.pick(COLORS)
            bl = rng.pick([3, 3, 1, 0, 12, 13, 24, 14])
            c = new_case("drawpaint_b%d" % bl, rng, cfg=cfg)
            c.add(paint(color=col, blend=bl, aa=rng.chance(1, 2), dither=(j == 1)))
            c.add("drawpaint")


def rect_geoms(w, h):
    return {
        "int": (4, 3, w - 6, h - 5),
        "frac": (3.3, 2.75, w - 7.6, h - 4.2),
        "tiny": (10.2, 11.4, 10.7, 11.6),
        "sub1": (5.25, 7.5, 6.0, 8.25),
        "huge": (-1e5, -2e5, 1e5, 3e5),
        "vhuge": (-3e7, -3e7, 3e7, 3e7),
        "empty": (10, 10, 10, 20),
        "flip": (w - 5.5, h - 4.5, 3.5, 2.25),
        "edge": (-3.5, -2.5, w + 2.5, h + 1.5),
        "line": (2, 10, w - 3, 10.5),
        "off": (w + 5, 2, w + 20, 10),
    }


def sec_rect(rng):
    geoms = list(rect_geoms(40, 32).keys())
    for gi, g in enumerate(geoms):
        for ci, cn in enumerate(CTM_NAMES):
            for k in range(3):
                c = new_case("rect_%s_%s" % (g, cn), rng)
                w, h = c.w, c.h
                # every style shows up for every geometry (over the matrices)
                sname, st = STYLES[(gi * 5 + ci * 3 + k * 4) % len(STYLES)]
                aa = (k % 2 == 1)
                c.add(ctms(w, h)[cn])
                c.add(paint(color=rng.pick(COLORS), aa=aa, **st))
                c.add("rect " + Fs(*rect_geoms(w, h)[g]))
                if rng.chance(1, 3):
                    c.add("hash r1")
                    c.add(paint(color=rng.pick(COLORS), aa=not aa, style="stroke", width=0))
                    c.add("rect " + Fs(*rect_geoms(w, h)[g]))
    # Every style, both aa, on rect-stays-rect matrices (kStroke, kHair, kFill fast paths).
    for sname, st in STYLES:
        for aa in (0, 1):
            for cn in ["id", "trf", "sc2", "flip", "negs", "rot90"]:
                c = new_case("rectstyle_%s_aa%d_%s" % (sname, aa, cn), rng, cfg=N32_HEAVY.next())
                c.add(ctms(c.w, c.h)[cn])
                c.add(paint(color=rng.pick(TRANSLUCENT), aa=aa, **st))
                c.add("rect " + Fs(*rect_geoms(c.w, c.h)[rng.pick(["int", "frac", "line", "edge"])]))


def sec_oval_rrect(rng):
    for cn in CTM_NAMES:
        for k in range(6):
            c = new_case("oval_%s" % cn, rng)
            w, h = c.w, c.h
            sname, st = rng.pick(STYLES)
            c.add(ctms(w, h)[cn])
            c.add(paint(color=rng.pick(COLORS), aa=k % 2, **st))
            l, t, r, b = rand_rect(rng, w, h)
            c.add("oval " + Fs(l, t, r, b))
            if k == 5:
                c.add("oval " + Fs(5, 5, 5.5, 5.75))
                c.add("oval " + Fs(-1e5, -1e5, 1e5, 1e5))
    rr_kinds = ["uni", "complex", "asrect", "asoval", "empty", "ninepatch", "bigrad", "flip"]
    for kind in rr_kinds:
        for cn in CTM_NAMES:
            for k in range(2):
                c = new_case("rrect_%s_%s" % (kind, cn), rng)
                w, h = c.w, c.h
                sname, st = rng.pick(STYLES)
                c.add(ctms(w, h)[cn])
                c.add(paint(color=rng.pick(COLORS), aa=(k == 0), **st))
                l, t, r, b = 3.5, 2.25, w - 4.75, h - 3.5
                if kind == "uni":
                    c.add("rrect " + Fs(l, t, r, b, 6, 4.5))
                elif kind == "complex":
                    c.add("rrect4 " + Fs(l, t, r, b, 2, 7, 10, 3.5, 0, 0, 5.5, 9))
                elif kind == "asrect":
                    c.add("rrect " + Fs(l, t, r, b, 0, 0))
                elif kind == "asoval":
                    c.add("rrect " + Fs(l, t, r, b, (r - l) / 2, (b - t) / 2))
                elif kind == "empty":
                    c.add("rrect " + Fs(l, t, l, b, 3, 3))
                    c.add("rrect4 " + Fs(l, t, r, t, 1, 1, 1, 1, 1, 1, 1, 1))
                elif kind == "ninepatch":
                    c.add("rrect4 " + Fs(l, t, r, b, 3, 5, 6, 5, 6, 2, 3, 2))
                elif kind == "bigrad":
                    c.add("rrect " + Fs(l, t, r, b, 100, 3))
                else:
                    c.add("rrect " + Fs(r, b, l, t, 4, 4))
    for k in range(60):
        c = new_case("drrect", rng)
        w, h = c.w, c.h
        sname, st = rng.pick(STYLES[:6])
        c.add(ctms(w, h)[rng.pick(CTM_NAMES)])
        c.add(paint(color=rng.pick(COLORS), aa=k % 2, **st))
        ol, ot, orr, ob = 2.5, 1.75, w - 3.25, h - 2.5
        inset = rng.pick([3, 5.5, 8, 0])
        c.add("drrect " + Fs(ol, ot, orr, ob, rng.fl(0, 8), rng.fl(0, 8), ol + inset, ot + inset,
                             orr - inset * 1.5, ob - inset, rng.fl(0, 5), rng.fl(0, 5)))


def sec_region(rng):
    for k in range(60):
        c = new_case("region", rng)
        w, h = c.w, c.h
        cn = rng.pick(["id", "id", "tri", "trf", "sc2", "rot30", "skew"])
        c.add(ctms(w, h)[cn])
        st = rng.pick(STYLES[:6])[1] if k % 3 == 0 else dict(style="fill")
        pe = rand_pe(rng) if k % 7 == 0 else None
        c.add(paint(color=rng.pick(COLORS), aa=k % 2, pe=pe, **st))
        n = rng.rint(1, 4)
        rs = []
        for _ in range(n):
            x0, y0 = rng.rint(-3, w - 4), rng.rint(-3, h - 4)
            rs += [x0, y0, x0 + rng.rint(1, w // 2), y0 + rng.rint(1, h // 2)]
        c.add("region %d %s" % (n, " ".join(str(v) for v in rs)))


def sec_paths(rng):
    big = {"big1e6", "bigline", "big3e7", "huge"}
    for pn in PATH_NAMES:
        reps = 3 if pn in ("big3e7",) else 16
        for k in range(reps):
            c = new_case("path_%s" % pn, rng)
            w, h = c.w, c.h
            P = paths(w, h)
            fill = FILLS[k % 4]
            sname, st = STYLES[(k * 5 + rng.rint(0, 11)) % len(STYLES)]
            if pn in big and sname not in ("fill", "hair"):
                st = dict(style="stroke", width=rng.pick([0, 1.5]))
            cn = "id" if k < 6 else rng.pick(CTM_NAMES)
            if pn in big and cn == "persp":
                cn = "rot30"
            c.add(ctms(w, h)[cn])
            if pn in big or k % 5 == 0:
                c.add("cliprect " + Fs(2.5, 3, w - 4, h - 2.5) + " intersect %d" % (k % 2))
            c.add(paint(color=rng.pick(COLORS), aa=(k // 2) % 2, **st))
            c.add(path_cmd("path", fill, P[pn]))
            c.add("hash p")


def sec_points(rng):
    counts = [1, 2, 3, 5, 8, 31, 32, 33, 34, 63, 64, 65, 70]
    variants = [
        ("w0", dict(width=0)),
        ("w1", dict(width=1)),
        ("w1sq", dict(width=1, cap="square")),
        ("w3butt", dict(width=3, cap="butt")),
        ("w3sq", dict(width=3, cap="square")),
        ("w3round", dict(width=3, cap="round")),
        ("w0.5", dict(width=0.5)),
        ("w6round", dict(width=6, cap="round")),
    ]
    modes = ["points", "lines", "polygon"]
    i = 0
    for mode in modes:
        for vn, v in variants:
            for aa in (0, 1):
                for cn in ["id", "trf", "sc2", "scxy", "rot30", "persp"]:
                    n = counts[i % len(counts)]
                    i += 1
                    c = new_case("points_%s_%s_aa%d_%s_n%d" % (mode, vn, aa, cn, n), rng,
                                 cfg=N32_HEAVY.next() if i % 2 else None)
                    w, h = c.w, c.h
                    c.add(ctms(w, h)[cn])
                    style = rng.pick(["stroke", "fill", "strokefill"])
                    c.add(paint(color=rng.pick(COLORS), aa=aa, style=style, **v))
                    pts = []
                    for j in range(n):
                        if rng.chance(1, 12):
                            pts += [rng.fl(-10, w + 10), rng.fl(-10, h + 10)]
                        else:
                            pts += [rng.fl(1, w - 1), rng.fl(1, h - 1)]
                    c.add("points %s %d %s" % (mode, n, Fs(*pts)))
    # Dashed 2-point lines: SkDashPathEffect::asPoints fast paths.
    dashes = [([2, 2], 0), ([3, 1], 0.5), ([1, 1], 0), ([4, 2], 5), ([0.5, 1.5], 0.25), ([5, 3, 1, 3], 2),
              ([2, 6], 1), ([6, 2], 0)]
    lines = [("h", (2, 10, 36, 10)), ("hf", (2.5, 10.5, 35.25, 10.5)), ("v", (12, 2, 12, 30)),
             ("d", (3, 4, 35, 27)), ("hr", (36, 20, 3, 20)), ("long", (-100, 15, 300, 15))]
    for dn, (iv, ph) in enumerate(dashes):
        for ln, pts in lines:
            for k in range(2):
                cap = rng.pick(["butt", "round", "square"])
                width = rng.pick([0, 1, 2, 3, 4])
                aa = rng.chance(1, 2)
                c = new_case("dashline_%d_%s_%s_w%d" % (dn, ln, cap, width), rng, cfg=N32_HEAVY.next())
                cn = rng.pick(["id", "id", "trf", "sc2", "rot90", "rot30", "sch"])
                c.add(ctms(c.w, c.h)[cn])
                c.add(paint(color=rng.pick(COLORS), aa=aa, style="stroke", width=width, cap=cap,
                            pe=dash_spec(iv, ph)))
                c.add("points lines 2 " + Fs(*pts))
                if k == 1:
                    c.add("points lines 4 " + Fs(*pts) + " " + Fs(pts[0], pts[1] + 3, pts[2], pts[3] + 3))


def sec_hairline_coverage(rng):
    # DrawTreatAAStrokeAsHairline: device stroke width <= 1 -> modulated hairline.
    combos = [(1, 0.5), (2, 0.5), (2.2, 0.5), (0.8, 1), (1, 1), (1.01, 1), (0.3, 1), (3, 0.25),
              (4, 0.3), (0.5, 2), (0.6, 2), (1, -0.5)]
    blends = [3, 1, 0, 12, 24, 13, 14, 5, 6, 2]
    for wd, s in combos:
        for bl in blends:
            c = new_case("hairmod_w%s_s%s_b%d" % (F(wd), F(s), bl), rng, cfg=N32_HEAVY.next())
            w, h = c.w, c.h
            c.add(mat(s, 0, w / 2.0 * (1 - s), 0, s, h / 2.0 * (1 - s), 0, 0, 1))
            c.add(paint(color=rng.pick(TRANSLUCENT + FAINT + [0xFF336699]), aa=1, style="stroke",
                        width=wd, blend=bl, cap=rng.pick(["butt", "round", "square"])))
            P = paths(w, h)
            k = rng.rint(0, 3)
            if k == 0:
                c.add(path_cmd("path", "w", P[rng.pick(["star", "open", "cubics", "quads"])]))
            elif k == 1:
                c.add("rrect " + Fs(4, 4, w - 4, h - 4, 5, 5))
            elif k == 2:
                c.add("oval " + Fs(3, 3, w - 3, h - 3))
            else:
                c.add("rect " + Fs(4.5, 4.5, w - 4, h - 4))
    # Skew/rotation where only one axis is thin.
    for k in range(20):
        c = new_case("hairmod_skew", rng, cfg=N32_HEAVY.next())
        c.add(mat(0.4, rng.fl(-0.6, 0.6), 10, rng.fl(-0.3, 0.3), 0.45, 8, 0, 0, 1))
        c.add(paint(color=rng.pick(TRANSLUCENT), aa=1, style="stroke", width=rng.pick([1.5, 2, 2.5, 3])))
        c.add(path_cmd("path", "w", paths(c.w, c.h)[rng.pick(COMMON_PATHS)]))


def sec_path_effects(rng):
    dashes = [([2, 2], 0), ([3, 1], 0.5), ([1, 1], 0), ([4, 2], 5), ([5, 3, 1, 3], 2), ([2, 6], 1),
              ([0.5, 0.5], 0)]
    shapes = ["rect", "oval", "rrect", "path_open", "path_closed", "path_star", "region", "drrect"]
    k = 0
    for iv, ph in dashes:
        for sh in shapes:
            for rep in range(2):
                k += 1
                c = new_case("dash_%s_%d" % (sh, k), rng)
                w, h = c.w, c.h
                cn = rng.pick(["id", "id", "trf", "sc2", "rot30", "skew", "sch"])
                c.add(ctms(w, h)[cn])
                style = rng.pick([("stroke", 0), ("stroke", 1), ("stroke", 2.5), ("fill", 0),
                                  ("strokefill", 2)])
                c.add(paint(color=rng.pick(COLORS), aa=rep, style=style[0], width=style[1],
                            cap=rng.pick(["butt", "round", "square"]), pe=dash_spec(iv, ph)))
                emit_shape(c, sh, rng)
    for kind in ["corner", "sum", "compose"]:
        for sh in shapes:
            for rep in range(3):
                c = new_case("pe_%s_%s" % (kind, sh), rng)
                if kind == "corner":
                    pe = "corner:" + F(rng.pick([1, 3, 6, 15]))
                elif kind == "sum":
                    pe = "sum_dash_corner:" + dash_spec([rng.pick([2, 4]), rng.pick([1, 3])],
                                                        rng.pick([0, 1]))[5:] + ":" + F(rng.pick([2, 5]))
                else:
                    pe = "compose_corner_dash:" + dash_spec([rng.pick([2, 4]), rng.pick([1, 3])],
                                                            rng.pick([0, 1]))[5:] + ":" + F(rng.pick([2, 5]))
                style = rng.pick([("stroke", 0), ("stroke", 1.5), ("fill", 0), ("strokefill", 3)])
                c.add(ctms(c.w, c.h)[rng.pick(["id", "trf", "rot45", "scxy"])])
                c.add(paint(color=rng.pick(COLORS), aa=rep % 2, style=style[0], width=style[1], pe=pe))
                emit_shape(c, sh, rng)


def emit_shape(c, sh, rng):
    w, h = c.w, c.h
    P = paths(w, h)
    if sh == "rect":
        c.add("rect " + Fs(4.5, 3.25, w - 5, h - 4))
    elif sh == "oval":
        c.add("oval " + Fs(3, 3.5, w - 3.5, h - 2))
    elif sh == "rrect":
        c.add("rrect " + Fs(3, 3, w - 3, h - 3, 6, 4))
    elif sh == "path_open":
        c.add(path_cmd("path", "w", P["open"]))
    elif sh == "path_closed":
        c.add(path_cmd("path", "w", P["quads"]))
    elif sh == "path_star":
        c.add(path_cmd("path", "e", P["star"]))
    elif sh == "region":
        c.add("region 2 3 3 20 15 15 10 %d %d" % (w - 3, h - 3))
    elif sh == "drrect":
        c.add("drrect " + Fs(2, 2, w - 2, h - 2, 5, 5, 8, 7, w - 8, h - 7, 3, 3))
    else:
        raise ValueError(sh)


def sec_blend(rng):
    devs = [("n32", "premul", "srgb"), ("n32", "premul", "none"), ("a8", "premul", "none"),
            ("rgbaf16", "premul", "linear"), ("rgb565", "opaque", "srgb"), ("argb4444", "premul", "none"),
            ("rgbaf32", "unpremul", "srgb"), ("gray8", "opaque", "none"), ("rgba1010102", "premul", "srgb")]
    for bl in range(29):
        for aa in (0, 1):
            for di, dev in enumerate(devs):
                if (bl + di + aa) % 2 == 1 and dev[0] not in ("n32", "a8"):
                    continue  # keep the size down: every blend on n32/a8, half the others
                c = new_case("blend%d_aa%d" % (bl, aa), rng, cfg=dev)
                w, h = c.w, c.h
                col = rng.pick(COLORS)
                c.add(paint(color=col, aa=aa, blend=bl))
                k = (bl + di) % 3
                if k == 0:
                    c.add(path_cmd("path", "w", paths(w, h)["tri"]))
                elif k == 1:
                    c.add("rect " + Fs(3.5, 2.25, w - 6.75, h - 3.5))
                else:
                    c.add("oval " + Fs(2, 2.5, w - 2.5, h - 2))
                c.add("hash a")
                c.add(paint(color=rng.pick(COLORS), aa=1 - aa, blend=bl, style="stroke", width=0))
                c.add(path_cmd("path", "w", paths(w, h)["star"]))


def sec_shaders(rng):
    shapes = ["rect", "oval", "rrect", "path_star", "region", "drrect", "drawpaint", "points"]
    for sh in shapes:
        for k in range(8):
            c = new_case("shader_%s" % sh, rng)
            col = rng.pick(COLORS)
            c.add(ctms(c.w, c.h)[rng.pick(["id", "rot30", "trf", "sc2"])])
            c.add(paint(color=rng.pick([0xFF000000, 0x80FFFFFF, 0x40000000]), aa=k % 2,
                        shader="solid:" + C(col), blend=rng.pick([3, 3, 1, 13, 24]),
                        dither=(k % 3 == 0)))
            if sh == "drawpaint":
                c.add("drawpaint")
            elif sh == "points":
                c.add("points polygon 5 " + Fs(2, 2, 30, 5, 10, 25, 35, 28, 2, 2))
            else:
                emit_shape(c, sh, rng)
    for ct, at, cs in [("rgbaf32", "premul", "srgb"), ("rgbaf32", "unpremul", "linear"),
                       ("rgb565", "opaque", "srgb"), ("rgb565", "opaque", "none"),
                       ("argb4444", "premul", "srgb"), ("argb4444", "opaque", "none"),
                       ("n32", "premul", "srgb"), ("n32", "opaque", "none"), ("n32", "premul", "linear"),
                       ("rgbaf16", "premul", "srgb"), ("rgba1010102", "premul", "linear"),
                       ("gray8", "opaque", "srgb")]:
        for k in range(5):
            c = new_case("dither", rng, cfg=(ct, at, cs))
            sh = k % 3
            kw = dict(color=rng.pick(COLORS), aa=k % 2, dither=True)
            if k >= 3:
                kw["shader"] = "solid:" + C(rng.pick(COLORS))
            c.add(paint(**kw))
            if sh == 0:
                c.add("drawpaint")
            elif sh == 1:
                c.add("oval " + Fs(2, 3, c.w - 3, c.h - 2))
            else:
                c.add(path_cmd("path", "e", paths(c.w, c.h)["star7"]))


def sec_clips(rng):
    def clip_op(c, k):
        w, h = c.w, c.h
        P = paths(w, h)
        op = rng.pick(["intersect", "intersect", "diff"])
        aa = rng.rint(0, 1)
        if k == 0:
            c.add("cliprect %d %d %d %d %s %d" % (rng.rint(-2, 10), rng.rint(-2, 8), rng.rint(w // 2, w + 2),
                                                  rng.rint(h // 2, h + 2), op, aa))
        elif k == 1:
            c.add("cliprect " + Fs(rng.fl(0, 10), rng.fl(0, 8), rng.fl(w / 2, w), rng.fl(h / 2, h)) +
                  " %s %d" % (op, aa))
        elif k == 2:
            c.add("cliprrect " + Fs(rng.fl(0, 8), rng.fl(0, 6), rng.fl(w / 2, w), rng.fl(h / 2, h),
                                    rng.fl(1, 8), rng.fl(1, 8)) + " %s %d" % (op, aa))
        elif k == 3:
            c.add("cliprrect4 " + Fs(2, 2.5, w - 3, h - 2, 1, 6, 8, 2, 0, 0, 5, 5) + " %s %d" % (op, aa))
        elif k == 4:
            c.add("clippath %s %d %s %s" % (op, aa, rng.pick(FILLS),
                                            path_tokens(P[rng.pick(["star", "circle", "holes", "cubics", "bowtie"])])))
        elif k == 5:
            n = rng.rint(1, 3)
            rs = []
            for _ in range(n):
                x0, y0 = rng.rint(-2, w - 4), rng.rint(-2, h - 4)
                rs += [x0, y0, x0 + rng.rint(2, w // 2), y0 + rng.rint(2, h // 2)]
            c.add("clipregion %s %d %s" % (op, n, " ".join(str(v) for v in rs)))
        elif k == 6:
            c.add("replaceclip %d %d %d %d" % (rng.rint(-3, 8), rng.rint(-3, 8), rng.rint(10, w + 5),
                                               rng.rint(10, h + 5)))
        elif k == 7:
            # empty clip
            c.add("cliprect " + Fs(10, 10, 10, 20) + " intersect %d" % aa)
        elif k == 8:
            # rotated clip -> AA clip
            c.add(ctms(w, h)[rng.pick(["rot30", "rot45", "skew", "persp"])])
            c.add("cliprect " + Fs(6, 5, w - 6, h - 5) + " %s %d" % (op, aa))
            c.add("ctm id")
        else:
            # clip entirely outside
            c.add("cliprect " + Fs(w + 5, 0, w + 20, h) + " %s %d" % (op, aa))

    for k in range(10):
        for rep in range(14):
            c = new_case("clip%d" % k, rng)
            clip_op(c, k)
            c.add("query q0")
            if rep % 3 == 0:
                clip_op(c, rng.rint(0, 9))
                c.add("query q1")
            for d in range(rng.rint(1, 3)):
                c.add(rand_paint(rng, allow_pe=(d == 0)))
                c.add(rand_shape(rng, c.w, c.h))
    # Nested save/restore.
    for rep in range(70):
        c = new_case("clipnest", rng)
        for depth in range(rng.rint(1, 3)):
            c.save()
            clip_op(c, rng.rint(0, 9))
            c.add("query s%d" % depth)
            c.add(rand_paint(rng, allow_pe=False))
            c.add(rand_shape(rng, c.w, c.h))
        c.add("hash inner")
        while c.saves:
            c.restore()
            c.add("query r%d" % c.saves)
        clip_op(c, rng.rint(0, 9))
        c.add("query after")
        c.add(rand_paint(rng))
        c.add(rand_shape(rng, c.w, c.h))
    # AA clip blitter: AA clips with every shape kind.
    for rep in range(80):
        c = new_case("aaclip", rng)
        w, h = c.w, c.h
        c.add("cliprrect " + Fs(2.5, 2.25, w - 3.5, h - 2.75, 7, 5) + " intersect 1")
        if rep % 2:
            c.add("clippath diff 1 w " + path_tokens(paths(w, h)["circle"]))
        c.add("query q")
        for d in range(rng.rint(1, 3)):
            c.add(rand_paint(rng))
            c.add(rand_shape(rng, w, h))


def sec_noninvertible(rng):
    for k in range(24):
        c = new_case("noninv", rng, cfg=N32_HEAVY.next())
        w, h = c.w, c.h
        zm = rng.pick([mat(0, 0, 5, 0, 0, 5, 0, 0, 1), mat(1, 2, 3, 2, 4, 6, 0, 0, 1),
                       mat(0, 0, 10.5, 0, 1, 0, 0, 0, 1)])
        c.add(zm)
        st = rng.pick([("stroke", 2), ("stroke", 0), ("fill", 0), ("strokefill", 1)])
        c.add(paint(color=rng.pick(COLORS), aa=k % 2, style=st[0], width=st[1],
                    pe=rand_pe(rng) if k % 3 else None))
        sh = rng.pick(["rect", "oval", "rrect", "path_star", "path_open", "drrect"])
        emit_shape(c, sh, rng)
        c.add("points lines 2 " + Fs(1, 1, 30, 20))


def sec_tiling(rng):
    devs = []
    for i in range(70):
        if i % 5 == 4:
            devs.append((rng.rint(8, 20), rng.rint(8195, 8400), rng.pick(["n32", "a8"])))
        else:
            devs.append((rng.rint(8195, 9000), rng.rint(12, 40), rng.pick(["n32", "a8"])))
    for i, (w, h, ct) in enumerate(devs):
        c = Case("tile_%dx%d_%s" % (w, h, ct))
        c.device(w, h, ct, "premul", rng.pick(["srgb", "none"]))
        if i % 7 == 0:
            c.add("noise %d" % rng.rint(1, 1000))
        else:
            c.add("erase " + C(rng.pick(BGS)))
        wide = w > h
        # coordinates around the 8191 seam
        S = 8191
        if i % 3 == 1:
            c.add(mat(1, 0, rng.pick([0.375, -0.5, 0.25]), 0, 1, rng.pick([0.6, 0, -0.25]), 0, 0, 1))
        if i % 4 == 2:
            if wide:
                c.add("cliprect " + Fs(100.5, 1.5, w - 50.25, h - 2) + " intersect %d" % (i % 2))
            else:
                c.add("cliprect " + Fs(1.5, 100.5, w - 2, h - 50.25) + " intersect %d" % (i % 2))
        if i % 6 == 5:
            if wide:
                c.add("cliprrect " + Fs(S - 300, 1, S + 400, h - 1, 6, 4) + " intersect 1")
            else:
                c.add("cliprrect " + Fs(1, S - 300, w - 1, S + 400, 4, 6) + " intersect 1")
        if i % 10 == 9:
            # A rotated (AA) clip straddling the seam; the ctm is restored afterwards.
            saved = [l for l in c.lines if l.startswith("ctm ")]
            c.add(rot(30, S, 10) if wide else rot(30, 8, S))
            if wide:
                c.add("cliprect " + Fs(S - 200, -20, S + 200, 60) + " intersect 1")
            else:
                c.add("cliprect " + Fs(-20, S - 200, 40, S + 200) + " intersect 1")
            c.add(saved[-1] if saved else "ctm id")
        c.add("query q")

        def tr(x0, y0, x1, y1):
            # (x along the long axis, y across)
            return (x0, y0, x1, y1) if wide else (y0, x0, y1, x1)

        across = h if wide else w
        for d in range(rng.rint(1, 3)):
            sname, st = rng.pick(STYLES[:8] + STYLES[10:])
            c.add(paint(color=rng.pick(COLORS), aa=rng.chance(1, 2), **st))
            k = rng.rint(0, 8)
            a0 = rng.fl(S - 60, S - 2)
            a1 = rng.fl(S + 2, S + 80)
            if k == 0:
                c.add("rect " + Fs(*tr(a0, 2.5, a1, across - 3.25)))
            elif k == 1:
                c.add("oval " + Fs(*tr(a0, 1.5, a1, across - 2)))
            elif k == 2:
                c.add("rrect " + Fs(*tr(a0, 1.25, a1, across - 1.5)) + " " + Fs(5, 3))
            elif k == 3:
                x0, y0, x1, y1 = tr(a0, 2, a1, across - 2)
                xm, ym = tr((a0 + a1) / 2, across / 3.0, 0, 0)[0:2]
                c.add("path %s M %s L %s L %s Q %s %s Z" % (rng.pick(FILLS), Fs(x0, y0), Fs(x1, y0),
                                                             Fs(x1, y1), Fs(xm, ym), Fs(x0, y1)))
            elif k == 4:
                pts = []
                for j in range(rng.rint(3, 40)):
                    pts += list(tr(rng.fl(S - 100, S + 100), rng.fl(0, across), 0, 0)[0:2])
                c.add("points %s %d %s" % (rng.pick(["polygon", "lines", "points"]), len(pts) // 2, Fs(*pts)))
            elif k == 5:
                c.add("drawpaint")
            elif k == 6:
                # entirely beyond the seam / beyond the device
                c.add("rect " + Fs(*tr(S + 300.5, 1, S + 600, across - 1)))
                far = (w if wide else h) + 100
                c.add("oval " + Fs(*tr(far, 0, far + 50, across)))
            elif k == 7:
                # many tiles: the whole long axis
                c.add("rect " + Fs(*tr(0.5, 0.75, (w if wide else h) - 0.5, across - 1.25)))
            else:
                c.add("drrect " + Fs(*tr(a0 - 30, 0.5, a1 + 30, across - 0.5)) + " " + Fs(4, 4) + " " +
                      Fs(*tr(a0, 3, a1, across - 3)) + " " + Fs(2, 2))
        c.add("hash t")


def sec_devmask(rng):
    devs = [("n32", "premul", "srgb"), ("a8", "premul", "none"), ("rgbaf16", "premul", "linear"),
            ("n32", "opaque", "none"), ("rgb565", "opaque", "srgb")]
    blends = [3, 1, 0, 13, 12, 5, 24, 9]
    k = 0
    for fmt in ["a8", "bw", "lcd16"]:
        for clip in ["rect", "oval"]:
            for bl in blends:
                for di, dev in enumerate(devs[:3] if bl not in (3, 1) else devs):
                    k += 1
                    c = new_case("devmask_%s_%s_b%d" % (fmt, clip, bl), rng, cfg=dev)
                    w, h = c.w, c.h
                    if clip == "rect":
                        cl = "rect:%d,%d,%d,%d" % (rng.rint(0, 6), rng.rint(0, 5), rng.rint(w - 8, w),
                                                   rng.rint(h - 8, h))
                    else:
                        cl = "oval:%s,%s,%s,%s" % (F(rng.fl(0, 5)), F(rng.fl(0, 5)), F(rng.fl(w - 6, w)),
                                                   F(rng.fl(h - 6, h)))
                    ml, mt = rng.rint(-6, 10), rng.rint(-6, 8)
                    mr, mb = ml + rng.rint(1, w + 4), mt + rng.rint(1, h + 4)
                    if k % 9 == 0:
                        c.add(ctms(w, h)[rng.pick(["rot30", "sc2"])])
                    kw = dict(color=rng.pick(COLORS), blend=bl, aa=rng.chance(1, 2))
                    if k % 5 == 0:
                        kw["shader"] = "solid:" + C(rng.pick(COLORS))
                    if k % 7 == 0:
                        kw["dither"] = True
                    c.add(paint(**kw))
                    c.add("devmask %s %s %d %d %d %d %d" % (cl, fmt, ml, mt, mr, mb, rng.rint(1, 99999)))
    # Empty mask bounds and masks entirely outside the clip.
    for fmt in ["a8", "bw", "lcd16"]:
        c = new_case("devmask_empty_%s" % fmt, rng, cfg=devs[0])
        c.add(paint(color=0x80FF0000))
        c.add("devmask rect:0,0,10,10 %s 5 5 5 9 3" % fmt)
        c.add("devmask rect:0,0,10,10 %s 20 20 30 30 4" % fmt)


def sec_pathcov(rng):
    styles = [("fill", dict(style="fill")), ("hair", dict(style="stroke", width=0)),
              ("s2", dict(style="stroke", width=2)), ("s0.5", dict(style="stroke", width=0.5)),
              ("sf2", dict(style="strokefill", width=2, join="round"))]
    for sname, st in styles:
        for k in range(14):
            c = new_case("pathcov_%s" % sname, rng, cfg=("a8", "premul", "none"))
            w, h = c.w, c.h
            if k % 2:
                cl = "rect:%d,%d,%d,%d" % (rng.rint(0, 5), rng.rint(0, 5), rng.rint(w - 6, w), rng.rint(h - 6, h))
            else:
                cl = "oval:%s,%s,%s,%s" % (F(rng.fl(0, 4)), F(rng.fl(0, 4)), F(rng.fl(w - 5, w)), F(rng.fl(h - 5, h)))
            if k % 4 == 3:
                c.add(ctms(w, h)[rng.pick(["rot30", "sc2", "trf", "skew"])])
            c.add(paint(color=rng.pick(COLORS), aa=(k // 2) % 2, cap=rng.pick(["butt", "round", "square"]), **st))
            c.add("pathcov %s %s %s" % (cl, rng.pick(FILLS), path_tokens(paths(w, h)[rng.pick(COMMON_PATHS)])))


def sec_pixels(rng):
    dsts = [("n32", "premul", "srgb"), ("n32", "unpremul", "srgb"), ("n32", "opaque", "none"),
            ("a8", "premul", "none"), ("gray8", "opaque", "srgb"), ("rgb565", "opaque", "none"),
            ("argb4444", "premul", "srgb"), ("rgbaf16", "premul", "linear"), ("rgbaf32", "unpremul", "srgb"),
            ("rgba1010102", "premul", "srgb"), ("rgbaf16", "unpremul", "none")]
    k = 0
    for rep in range(14):
        for dst in dsts:
            k += 1
            c = new_case("pixels", rng, cfg=N32_HEAVY.next() if rep % 2 == 0 else None)
            w, h = c.w, c.h
            if rep % 3 == 0:
                c.add(rand_paint(rng, allow_pe=False))
                c.add(rand_shape(rng, w, h))
            pw, ph = rng.rint(2, min(20, w)), rng.rint(2, min(16, h))
            where = rng.rint(0, 4)
            if where == 0:
                x, y = rng.rint(0, w - pw), rng.rint(0, h - ph)
            elif where == 1:
                x, y = rng.rint(-pw + 1, -1), rng.rint(-3, h - 2)
            elif where == 2:
                x, y = rng.rint(w - pw + 1, w - 1), rng.rint(h - ph + 1, h - 1)
            elif where == 3:
                x, y = rng.pick([w + 3, -pw - 2]), rng.rint(0, h)
            else:
                x, y = rng.rint(-2, 3), rng.rint(-2, 3)
            if rep % 2 == 0:
                c.add("readpixels rp %d %d %d %d %s %s %s" % ((x, y, pw, ph) + dst))
            else:
                c.add("writepixels wp %d %d %d %d %s %s %s %d" % ((x, y, pw, ph) + dst + (rng.rint(1, 9999),)))
                c.add("readpixels rp %d %d %d %d %s %s %s" % ((x, y, pw, ph) + rng.pick(dsts)))


def sec_toobig(rng):
    # Coordinates near SK_ScalarMax: SkPathPriv::TooBigForMath (> SK_ScalarMax / 4), saturating
    # float -> int conversions (SkScalarFloorToInt) and matrices that map to infinity.
    probes = [
        ("hair_5e37", [paint(color=0x80FF0000, style="stroke", width=0), "rect " + Fs(-5e37, -5e37, 5e37, 8)]),
        ("hairaa_5e37", [paint(color=0x80FF0000, style="stroke", width=0, aa=1), "rect " + Fs(-5e37, 3, 5e37, 8)]),
        ("hair_1e38", [paint(color=0x80FF0000, style="stroke", width=0), "rect " + Fs(-1e38, 3, 1e38, 8)]),
        ("fill_1e38", [paint(color=0x80FF0000), "rect " + Fs(-1e38, -1e38, 1e38, 1e38)]),
        ("fillaa_8e37", [paint(color=0x80FF0000, aa=1), "rect " + Fs(-8e37, -8e37, 8e37, 8e37)]),
        ("path_1e38", [paint(color=0x80FF0000, aa=1), "path w M " + Fs(-1e38, 2) + " L " + Fs(10, 1e38) + " L 12 3 Z"]),
        ("ctm_to_inf", [mat(4, 0, 0, 0, 4, 0, 0, 0, 1), paint(color=0x80FF0000, aa=1),
                        "path w M " + Fs(-3e38, 2) + " L " + Fs(10, 3e38) + " L 12 3 Z",
                        "rect " + Fs(-3e38, 2, 3e38, 3), "oval " + Fs(-3e38, 2, 3e38, 3)]),
        ("stroke_5e37", [paint(color=0x80FF0000, aa=1, style="stroke", width=3), "rect " + Fs(-5e37, 3, 5e37, 8),
                         "path w M " + Fs(-5e37, 2) + " L " + Fs(10, 5e37) + " L 12 3 Z"]),
        ("points_big", [paint(color=0x80FF0000, aa=1, style="stroke", width=0),
                        "points lines 4 " + Fs(-5e37, 3, 5e37, 8, 1, 1, 1e38, 1e38),
                        "points points 2 " + Fs(-5e37, 3, 3e38, 8),
                        paint(color=0x80FF0000, aa=0, style="stroke", width=0),
                        "points polygon 3 " + Fs(-5e37, 3, 5e37, 8, 1, 1),
                        paint(color=0x80FF0000, aa=0, style="stroke", width=3),
                        "points points 2 " + Fs(-5e37, 3, 3e38, 8)]),
        ("hair_3e9", [paint(color=0xC0204080, style="stroke", width=0), "rect " + Fs(-3e9, 4, 3e9, 9),
                      paint(color=0xC0204080, style="stroke", width=0, aa=1), "rect " + Fs(5, -3e9, 9, 3e9)]),
    ]
    for cfg in [("n32", "premul", "srgb"), ("a8", "premul", "none"), ("rgbaf16", "premul", "linear")]:
        for name, lines in probes:
            c = new_case("toobig_%s" % name, rng, cfg=cfg, size=(16, 16, 0))
            for l in lines:
                c.add(l)


def sec_random(rng):
    for k in range(1200):
        c = new_case("rnd%d" % k, rng)
        w, h = c.w, c.h
        if rng.chance(1, 2):
            c.add(ctms(w, h)[rng.pick(CTM_NAMES)])
        if rng.chance(1, 3):
            if rng.chance(1, 2):
                c.add("cliprect " + Fs(rng.fl(0, 8), rng.fl(0, 8), rng.fl(w / 2, w), rng.fl(h / 2, h)) +
                      " %s %d" % (rng.pick(["intersect", "diff"]), rng.rint(0, 1)))
            else:
                c.add("cliprrect " + Fs(rng.fl(0, 6), rng.fl(0, 6), rng.fl(w / 2, w), rng.fl(h / 2, h), 5, 4) +
                      " %s %d" % (rng.pick(["intersect", "diff"]), rng.rint(0, 1)))
        n = rng.rint(1, 4)
        for d in range(n):
            if rng.chance(1, 6):
                c.add(ctms(w, h)[rng.pick(CTM_NAMES)])
            c.add(rand_paint(rng))
            c.add(rand_shape(rng, w, h))
            if d == 0 and n > 1 and rng.chance(1, 3):
                c.add("hash d0")


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        os.path.dirname(os.path.abspath(__file__)), "..", "..", "crates", "skia-rust-raster", "src",
        "draw_tests", "cases.txt")
    sections = [
        (1, sec_drawpaint), (2, sec_rect), (3, sec_oval_rrect), (4, sec_region), (5, sec_paths),
        (6, sec_points), (7, sec_hairline_coverage), (8, sec_path_effects), (9, sec_blend),
        (10, sec_shaders), (11, sec_clips), (12, sec_noninvertible), (13, sec_tiling),
        (14, sec_devmask), (15, sec_pathcov), (16, sec_pixels), (17, sec_random), (18, sec_toobig),
    ]
    for seed, fn in sections:
        fn(Rng(0x5D15EED + seed * 7919))
    body = ["# Generated by oracle/draw/gen_cases.py; grammar in oracle/draw/draw.cpp. Do not edit.",
            "# %d cases" % len(CASES)]
    for c in CASES:
        body.append(c.text())
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    with open(out, "w", newline="\n") as fh:
        fh.write("\n".join(body) + "\n")
    print("%d cases -> %s" % (len(CASES), out))


if __name__ == "__main__":
    main()
