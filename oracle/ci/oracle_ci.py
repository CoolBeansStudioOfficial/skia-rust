#!/usr/bin/env python3
"""Glue for the CI-hosted CPU oracle (.github/workflows/oracle.yml, oracle/README.md).

Standard library only, so the workflow needs no Rust build. Subcommands:

  gn-args <build>            print the GN args for one CPU build of oracle/tiers.toml, adapted
                             to Linux clang (the `/clang:` prefixes of clang-cl are dropped)
  hash <dir> <tier> <cpu-tier> <toolchain.txt> <out.json> [<byte-order-free-tier>]
                             hash every DM output under <dir> into the `hashes-m156.json`
                             format; fails if a result ran at another CPU tier. With the last
                             argument, `565`/`f16` results (independent of N32 order) go to that
                             tier (the BGRA tier name) and `8888` results to <tier>
  merge <out.json> <in.json>...   merge several one-tier files into one
  compare <ours.json> <goldens.json> <tier> <ours-dir> <objects.tar> <report-prefix>
                             compare our hashes with a published golden tier and classify
                             every mismatch (pixel counts, deltas, R/B swap, ...)

The result id is `<config>/<src>/[<options>/]<name>`, the path of the output below the DM
`--oracleRawPath` directory without extension, exactly as `cargo xtask oracle` builds it.
"""
import hashlib
import json
import os
import subprocess
import sys
import tomllib

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def load_config():
    with open(os.path.join(ROOT, "oracle", "tiers.toml"), "rb") as f:
        return tomllib.load(f)


def linux_flag(flag):
    # tiers.toml spells flags for clang-cl; `/clang:X` passes X to clang.
    return flag[len("/clang:"):] if flag.startswith("/clang:") else flag


def gn_args(build_name):
    cfg = load_config()
    build = next(b for b in cfg["build"] if b["name"] == build_name and not b.get("gpu"))
    args = list(cfg["gn"]["args"]) + list(build.get("args", []))
    cflags = [linux_flag(f) for f in cfg["gn"].get("extra_cflags", []) + build.get("extra_cflags", [])]
    out = ['cc="clang"', 'cxx="clang++"']
    out += args
    # Differences from the Windows oracle, see oracle/README.md "CI-hosted oracle":
    # Skia enables FreeType and fontconfig on Linux only; the Windows oracle has neither, and
    # runs only the portable test font manager. Match Windows.
    out += ["skia_use_freetype=false", "skia_use_fontconfig=false"]
    out.append("extra_cflags=[" + ", ".join('"%s"' % f for f in cflags) + "]")
    return "\n".join(out) + "\n"


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def outputs(directory):
    found = {}
    for dirpath, _, files in os.walk(directory):
        for name in files:
            stem, ext = os.path.splitext(name)
            if ext not in (".raw", ".bin"):
                continue
            full = os.path.join(dirpath, name)
            rel = os.path.relpath(os.path.join(dirpath, stem), directory)
            found[rel.replace(os.sep, "/")] = full
    return found


def pin():
    with open(os.path.join(ROOT, "inventory", "skia-pin.toml"), "rb") as f:
        return tomllib.load(f)


def cmd_hash(directory, tier, cpu_tier, toolchain_path, out_path, free_tier=None):
    results = {}
    free_results = {}
    wrong = []
    for rid, path in sorted(outputs(directory).items()):
        h = sha256_file(path)
        if free_tier and rid.split("/", 1)[0] != "8888":
            free_results[rid] = h
        else:
            results[rid] = h
        with open(path[: path.rindex(".")] + ".json") as f:
            meta = json.load(f)
        if meta.get("cpu_tier") != cpu_tier:
            wrong.append((rid, meta.get("cpu_tier")))
    if wrong:
        print(f"{len(wrong)} results ran at another CPU tier than `{cpu_tier}`, e.g. {wrong[0]}")
        sys.exit(1)
    with open(toolchain_path) as f:
        toolchain = f.read()
    p = pin()
    doc = {
        "skia_branch": p["branch"],
        "skia_commit": p["commit"],
        "tiers": {tier: {"toolchain": toolchain, "results": results}},
    }
    if free_tier:
        doc["tiers"][free_tier] = {"toolchain": toolchain, "results": free_results}
    with open(out_path, "w") as f:
        json.dump(doc, f, sort_keys=True)
    print(f"{len(results)} results hashed for {tier}, {len(free_results)} for {free_tier}")


def cmd_merge(out_path, *inputs):
    merged = None
    for path in inputs:
        with open(path) as f:
            doc = json.load(f)
        if merged is None:
            merged = doc
        else:
            for tier, entry in doc["tiers"].items():
                if tier in merged["tiers"]:
                    merged["tiers"][tier]["results"].update(entry["results"])
                else:
                    merged["tiers"][tier] = entry
    with open(out_path, "w") as f:
        json.dump(merged, f, sort_keys=True)


def golden_bytes(objects_tar, sha):
    member = f"objects/{sha[:2]}/{sha}.zst"
    tar = subprocess.run(["tar", "-xOf", objects_tar, member], capture_output=True)
    if tar.returncode != 0:
        return None
    z = subprocess.run(["zstd", "-dc"], input=tar.stdout, capture_output=True)
    return z.stdout if z.returncode == 0 else None


def pixel_stats(ours, gold, bpp):
    """(differing pixels, max channel delta, whether R/B swapped ours == gold)."""
    n = min(len(ours), len(gold)) // bpp * bpp
    diff = 0
    maxd = 0
    for i in range(0, n, bpp):
        a, b = ours[i:i + bpp], gold[i:i + bpp]
        if a != b:
            diff += 1
            for x, y in zip(a, b):
                d = abs(x - y)
                if d > maxd:
                    maxd = d
    swapped = None
    if bpp == 4 and len(ours) == len(gold):
        sw = bytearray(ours)
        sw[0::4], sw[2::4] = ours[2::4], ours[0::4]
        swapped = bytes(sw) == gold
    return diff, maxd, swapped


BPP = {"8888": 4, "f16": 8, "565": 2}


def classify(rid, ours, gold):
    config = rid.split("/", 1)[0]
    if gold is None:
        return {"class": "golden-object-unavailable"}
    if len(ours) != len(gold):
        return {"class": "size", "ours_len": len(ours), "golden_len": len(gold)}
    bpp = BPP.get(config, 4)
    diff, maxd, swapped = pixel_stats(ours, gold, bpp)
    out = {"pixels": diff, "max_delta": maxd, "total_pixels": len(ours) // bpp, "rb_swapped_equal": swapped}
    if swapped:
        out["class"] = "n32-byte-order"
    elif maxd <= 1:
        out["class"] = "off-by-one"
    else:
        out["class"] = "pixels"
    return out


def cmd_compare(ours_json, goldens_json, tier, ours_dir, objects_tar, report_prefix):
    with open(ours_json) as f:
        ours_doc = json.load(f)
    with open(goldens_json) as f:
        gold_doc = json.load(f)
    ours = next(iter(ours_doc["tiers"].values()))["results"]
    if tier not in gold_doc["tiers"]:
        print(f"no golden tier `{tier}`")
        sys.exit(2)
    gold = gold_doc["tiers"][tier]["results"]
    files = outputs(ours_dir)
    # Only gm results have goldens.
    compared = {k: v for k, v in ours.items() if k.split("/")[1] == "gm"}
    matched = []
    mismatched = {}
    missing = []
    for rid, h in sorted(compared.items()):
        if rid not in gold:
            missing.append(rid)
        elif gold[rid] == h:
            matched.append(rid)
        else:
            mismatched[rid] = h
    configs = {k.split("/")[0] for k in compared}
    not_rendered = sorted(k for k in gold if k not in compared and k.split("/")[0] in configs)
    details = {}
    have_objects = bool(objects_tar) and os.path.exists(objects_tar)
    for rid, h in mismatched.items():
        with open(files[rid], "rb") as f:
            ours_bytes = f.read()
        gold_bytes = golden_bytes(objects_tar, gold[rid]) if have_objects else None
        details[rid] = classify(rid, ours_bytes, gold_bytes)
        details[rid]["ours"] = h
        details[rid]["golden"] = gold[rid]
    classes = {}
    for rid, d in details.items():
        classes.setdefault(d["class"], []).append(rid)
    report = {
        "tier": tier,
        "golden_results": len(gold),
        "rendered_gm_results": len(compared),
        "matched": len(matched),
        "mismatched": len(mismatched),
        "no_golden": missing,
        "golden_not_rendered": len(not_rendered),
        "not_rendered_ids": not_rendered,
        "classes": {k: len(v) for k, v in sorted(classes.items())},
        "mismatches": details,
    }
    with open(report_prefix + ".json", "w") as f:
        json.dump(report, f, indent=1, sort_keys=True)
    lines = [
        f"### Equivalence with `{tier}`",
        "",
        f"- rendered gm results: **{len(compared)}**; byte-identical to the golden: **{len(matched)}**; "
        f"different: **{len(mismatched)}**; without golden: {len(missing)}; "
        f"golden results not rendered here: {len(not_rendered)}",
    ]
    for k, v in sorted(classes.items()):
        lines.append(f"- class `{k}`: {len(v)}")
    lines += ["", "| result | class | differing pixels | max delta |", "|---|---|---|---|"]
    for rid in sorted(details):
        d = details[rid]
        lines.append(f"| `{rid}` | {d['class']} | {d.get('pixels', '')}/{d.get('total_pixels', '')} | {d.get('max_delta', '')} |")
    with open(report_prefix + ".md", "w") as f:
        f.write("\n".join(lines) + "\n")
    print("\n".join(lines[:12]))
    print(f"{len(matched)}/{len(compared)} outputs match {tier}")


def main(argv):
    cmd, args = argv[1], argv[2:]
    if cmd == "gn-args":
        sys.stdout.write(gn_args(*args))
    elif cmd == "hash":
        cmd_hash(*args)
    elif cmd == "merge":
        cmd_merge(*args)
    elif cmd == "compare":
        cmd_compare(*args)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
