#!/usr/bin/env python3
"""Writes the synthetic fonts that make `hb-subset` run hb-repacker's extension promotion and subtable
splitting. Real fonts hardly ever overflow in those ways, so the corpus built from Skia's resources never
reaches them. Each font is a minimal TrueType font (a few empty glyphs, one `cmap`, one layout table)
whose GSUB or GPOS has subtables that do not fit 16-bit offsets.

    gen_synthetic.py <out-dir>

writes the fonts and prints their corpus lines (`<font> 0 all <gids>`). Deterministic."""
import random
import struct
import sys

NUM_GLYPHS = 3000


def u16(*v):
    return b''.join(struct.pack('>H', x) for x in v)


def coverage(glyphs):
    return u16(1, len(glyphs), *glyphs)


def class_def(first, classes):
    return u16(1, first, len(classes), *classes)


def lay_out(parts):
    """parts: list of byte strings; returns (offsets, joined)."""
    offs, pos = [], 0
    for p in parts:
        offs.append(pos)
        pos += len(p)
    return offs, b''.join(parts)


def pair_pos1(rng, first, count, pairs_of):
    """PairPos format 1 with `count` pair sets; set k has `pairs_of(k)` records."""
    glyphs = list(range(first, first + count))
    sets = []
    for k, g in enumerate(glyphs):
        n = pairs_of(k)
        seconds = sorted(rng.sample(range(1, NUM_GLYPHS), n))
        sets.append(u16(n) + b''.join(u16(x, rng.randrange(1, 60000)) for x in seconds))
    header_len = 10 + 2 * count
    cov = coverage(glyphs)
    offs, body = lay_out([cov] + sets)
    header = u16(1, header_len + offs[0], 4, 0, count) + b''.join(u16(header_len + o) for o in offs[1:])
    return header + body


def pair_pos2(rng, c1, c2):
    cov = coverage(list(range(1, c1)))
    cd1 = class_def(1, list(range(1, c1)))
    cd2 = class_def(1, list(range(1, c2)))
    recs = b''.join(u16(rng.randrange(1, 60000)) for _ in range(c1 * c2))
    header_len = 16
    offs, body = lay_out([cov, cd1, cd2])
    # the children first (their offsets are 16 bits), then the records
    header = u16(2, header_len + offs[0], 4, 0, header_len + offs[1], header_len + offs[2], c1, c2)
    return header + body + recs


def anchor(x, y):
    return u16(1, x & 0xFFFF, y & 0xFFFF)


def mark_base(rng, marks, bases, classes):
    mark_glyphs = list(range(1, 1 + marks))
    base_glyphs = list(range(1000, 1000 + bases))
    mark_cov = coverage(mark_glyphs)
    base_cov = coverage(base_glyphs)
    # MarkArray: count, records (class, anchor offset), anchors
    mark_anchors = [anchor(i * 7, i * 3) for i in range(marks)]
    mark_hdr_len = 2 + 4 * marks
    offs, manchors = lay_out(mark_anchors)
    mark_array = u16(marks) + b''.join(u16(i % classes, mark_hdr_len + offs[i]) for i in range(marks)) + manchors
    # BaseArray: count, records of `classes` anchor offsets, distinct anchors
    base_hdr_len = 2 + 2 * classes * bases
    anchors = []
    seen = set()
    while len(anchors) < bases * classes:
        x, y = rng.randrange(0, 60000), rng.randrange(0, 60000)
        if (x, y) in seen:
            continue
        seen.add((x, y))
        anchors.append(anchor(x, y))
    offs, banchors = lay_out(anchors)
    base_array = u16(bases) + b''.join(u16(base_hdr_len + o) for o in offs) + banchors
    header_len = 12
    parts = [mark_cov, base_cov, mark_array, base_array]
    offs, body = lay_out(parts)
    header = u16(1, header_len + offs[0], header_len + offs[1], classes, header_len + offs[2], header_len + offs[3])
    return header + body


def ligature_subst(rng, nsets, ligs_per_set, comps_of):
    """Layout: header, coverage, the ligature sets, then the ligatures set by set. `comps_of(k)` is the
    number of components of the ligatures of set k (so that the sizes differ between the sets)."""
    glyphs = list(range(1, 1 + nsets))
    set_len = [2 + 2 * ligs_per_set] * nsets
    ligs_all = []
    for k in range(nsets):
        comps = comps_of(k)
        ligs_all.append([u16(rng.randrange(1, NUM_GLYPHS), comps, *[rng.randrange(1, NUM_GLYPHS) for _ in range(comps - 1)])
                         for _ in range(ligs_per_set)])
    lig_pos = []
    pos = sum(set_len)
    for k in range(nsets):
        lig_pos.append(pos)
        pos += sum(len(l) for l in ligs_all[k])
    sets = []
    set_pos = 0
    for k in range(nsets):
        offs, body = lay_out(ligs_all[k])
        base = lig_pos[k] - set_pos
        sets.append(u16(ligs_per_set) + b''.join(u16(base + o) for o in offs))
        set_pos += set_len[k]
    cov = coverage(glyphs)
    header_len = 6 + 2 * nsets
    sets_bytes = b''.join(sets)
    ligs_bytes = b''.join(b''.join(l) for l in ligs_all)
    set_offs = [header_len + len(cov) + sum(set_len[:k]) for k in range(nsets)]
    header = u16(1, header_len, nsets, *set_offs)
    return header + cov + sets_bytes + ligs_bytes


def single_subst2(rng, count):
    glyphs = list(range(1, 1 + count))
    subs = [rng.randrange(1, NUM_GLYPHS) for _ in glyphs]
    cov = coverage(glyphs)
    header_len = 6 + 2 * count
    return u16(2, header_len, count, *subs) + cov


def lookup_list(lookups, extension_type):
    """lookups: list of (type, [subtable bytes], is_extension). An extension lookup reaches its subtables
    through 32-bit offsets, so its subtables may be of any size. Layout: the list, the lookups, the
    extension subtables, the plain subtables, the subtables behind the extensions."""
    ll_len = 2 + 2 * len(lookups)
    lookup_pos, pos = [], ll_len
    for _, subs, _ in lookups:
        lookup_pos.append(pos)
        pos += 6 + 2 * len(subs)
    ext_pos = {}
    for i, (_, subs, is_ext) in enumerate(lookups):
        if is_ext:
            for k in range(len(subs)):
                ext_pos[(i, k)] = pos
                pos += 8
    plain_pos = {}
    for i, (_, subs, is_ext) in enumerate(lookups):
        if not is_ext:
            for k, sub in enumerate(subs):
                plain_pos[(i, k)] = pos
                pos += len(sub)
    big_pos = {}
    for i, (_, subs, is_ext) in enumerate(lookups):
        if is_ext:
            for k, sub in enumerate(subs):
                big_pos[(i, k)] = pos
                pos += len(sub)
    out = u16(len(lookups)) + b''.join(u16(p) for p in lookup_pos)
    for i, (ty, subs, is_ext) in enumerate(lookups):
        offs = [(ext_pos if is_ext else plain_pos)[(i, k)] - lookup_pos[i] for k in range(len(subs))]
        out += u16(extension_type if is_ext else ty, 0, len(subs), *offs)
    for i, (ty, subs, is_ext) in enumerate(lookups):
        if is_ext:
            for k in range(len(subs)):
                out += u16(1, ty) + struct.pack('>I', big_pos[(i, k)] - ext_pos[(i, k)])
    for i, (ty, subs, is_ext) in enumerate(lookups):
        if not is_ext:
            out += b''.join(subs)
    for i, (ty, subs, is_ext) in enumerate(lookups):
        if is_ext:
            out += b''.join(subs)
    return out


def layout_table(feature_tag, lookups, extension_type):
    lookup_idx = list(range(len(lookups)))
    langsys = u16(0, 0xFFFF, 1, 0)
    script = u16(4, 0) + langsys
    script_list = u16(1) + b'DFLT' + u16(8) + script
    feature = u16(0, len(lookup_idx), *lookup_idx)
    feature_list = u16(1) + feature_tag + u16(8) + feature
    ll = lookup_list(lookups, extension_type)
    hdr = 10
    s_off = hdr
    f_off = s_off + len(script_list)
    l_off = f_off + len(feature_list)
    return struct.pack('>IHHH', 0x00010000, s_off, f_off, l_off) + script_list + feature_list + ll


def sfnt(tables):
    tags = sorted(tables)
    n = len(tags)
    out = struct.pack('>IHHHH', 0x00010000, n, 0, 0, 0)
    pos = 12 + 16 * n
    body = b''
    for t in tags:
        d = tables[t]
        pad = (-len(d)) % 4
        out += t + struct.pack('>III', 0, pos + len(body), len(d))
        body += d + b'\0' * pad
    return out + body


def base_tables():
    n = NUM_GLYPHS
    head = struct.pack('>IIIIHHQQhhhhHHhhh', 0x00010000, 0x00010000, 0, 0x5F0F3CF5, 0, 1000, 0, 0,
                       0, 0, 1000, 1000, 0, 8, 2, 0, 0)
    hhea = struct.pack('>IhhhHhhhhhhhhhhhH', 0x00010000, 800, -200, 0, 1000, 0, 0, 1000, 1, 0, 0, 0, 0, 0, 0, 0, 1)
    maxp = struct.pack('>IH', 0x00010000, n) + u16(3, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0)
    hmtx = u16(500, 0) + u16(*([0] * (n - 1)))
    # glyph 1: a triangle (flags 1: on curve, 16-bit coordinates); all others are empty
    glyph = struct.pack('>hhhhh', 1, 0, 0, 100, 100) + u16(2) + u16(0) + bytes([1, 1, 1]) \
        + struct.pack('>hhh', 0, 100, -100) + struct.pack('>hhh', 0, 0, 100)
    glyph += b'\0' * ((-len(glyph)) % 4)
    offsets = [0, 0] + [len(glyph)] * (n - 1)
    loca = b''.join(struct.pack('>H', o // 2) for o in offsets)
    cmap_sub = u16(4, 32, 0, 4, 4, 1, 0, 0x42, 0xFFFF, 0, 0x41, 0xFFFF, 0xFFC0, 1, 0, 0)
    cmap = u16(0, 1) + struct.pack('>HHI', 3, 1, 12) + cmap_sub
    return {b'head': head, b'hhea': hhea, b'maxp': maxp, b'hmtx': hmtx, b'glyf': glyph, b'loca': loca, b'cmap': cmap}


def main():
    out = sys.argv[1]
    rng = random.Random(20250601)
    fonts = {}

    def ext(ty, subs):
        return (ty, subs, True)

    def plain(ty, subs):
        return (ty, subs, False)

    # A PairPos2 whose records alone exceed 64 KiB cannot reach its children: it has to be split. The
    # plain lookups give the promotion to extensions something to decide on.
    for c1, c2 in ((220, 200),):
        fonts['synth-gpos-pairpos2-%dx%d.ttf' % (c1, c2)] = (b'GPOS', b'kern', [ext(2, [pair_pos2(rng, c1, c2)])])
    fonts['synth-gpos-pairpos2-wide.ttf'] = (b'GPOS', b'kern', [
        ext(2, [pair_pos2(rng, 100, 330), pair_pos2(rng, 300, 150)]),
    ])
    fonts['synth-gpos-pairpos1-ext.ttf'] = (b'GPOS', b'kern', [
        ext(2, [pair_pos1(rng, 1, 1000, lambda k: 2900 if k >= 997 else 8), pair_pos1(rng, 1001, 1000, lambda k: 2900 if k >= 997 else 8)]),
    ])
    fonts['synth-gpos-lookups.ttf'] = (b'GPOS', b'kern',
                                      [plain(2, [pair_pos1(rng, 1 + 30 * (2 * j + k), 30, lambda k: 12) for k in range(2)])
                                       for j in range(20)])
    fonts['synth-gsub-liga.ttf'] = (b'GSUB', b'liga', [
        ext(4, [ligature_subst(rng, 350, 19, lambda k: 5 - (4 * k) // 350)]),
    ])
    fonts['synth-gsub-liga-plain.ttf'] = (b'GSUB', b'liga', [
        ext(4, [ligature_subst(rng, 350, 19, lambda k: 5 - (4 * k) // 350)]),
    ] + [plain(4, [ligature_subst(rng, 100, 10, lambda k: 3)]) for i in range(6)]
        + [plain(4, [ligature_subst(rng, 400, 12, lambda k: 4)])])
    fonts['synth-gpos-markbase-plain.ttf'] = (b'GPOS', b'mark', [
        ext(4, [mark_base(rng, 2500, 1300, 6)]),
    ] + [plain(2, [pair_pos1(rng, 1 + 40 * i, 40, lambda k: 30)]) for i in range(8)]
        + [plain(2, [pair_pos1(rng, 1, 1000, lambda k: 2900 if k >= 997 else 8)])])
    fonts['synth-gpos-markbase.ttf'] = (b'GPOS', b'mark', [ext(4, [mark_base(rng, 2500, 1300, 6)])])
    for name, (table, feature, lookups) in sorted(fonts.items()):
        tables = base_tables()
        tables[table] = layout_table(feature, lookups, 9 if table == b'GPOS' else 7)
        with open(out + '/' + name, 'wb') as f:
            f.write(sfnt(tables))
        print('%s 0 all %s' % (name, ','.join(str(g) for g in range(NUM_GLYPHS))))


main()
