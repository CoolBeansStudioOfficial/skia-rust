"""Resolve additive conflicts in a Rust source file (e.g. a shared test module).

Usage: python resolve_rust_union.py <file>

For each conflict hunk:
- if both sides consist only of `use ...;` statements (plus blank lines), the result is the
  union of imported items, grouped per path prefix (`use a::b::{x, y};`), sorted;
- otherwise both sides are kept (ours, then theirs), as for appended test blocks.
Exits 2 if a `use` hunk contains something it can't parse, so nothing is silently dropped.
"""
import re
import sys

HUNK_RE = re.compile(r'<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n', re.S)
USE_RE = re.compile(r'use\s+([^;]+);', re.S)


def use_items(block):
    """Returns {prefix: set(items)} for a block made only of use statements, else None."""
    stripped = re.sub(r'\s+', ' ', block).strip()
    if not stripped:
        return {}
    rest = USE_RE.sub('', block)
    if rest.strip():
        return None
    out = {}
    for m in USE_RE.finditer(block):
        body = re.sub(r'\s+', '', m.group(1))
        if '{' in body:
            prefix, inner = body.split('{', 1)
            inner = inner.rstrip('}')
            if '{' in inner:  # nested groups: give up rather than guess
                return None
            prefix = prefix.rstrip(':')
            items = [i for i in inner.split(',') if i]
        else:
            prefix, _, last = body.rpartition('::')
            items = [last]
        out.setdefault(prefix, set()).update(items)
    return out


def render(uses):
    std = sorted(p for p in uses if p.split('::')[0] in ('std', 'core', 'alloc'))
    crate = sorted(p for p in uses if p.split('::')[0] in ('crate', 'super', 'self'))
    other = sorted(p for p in uses if p not in std and p not in crate)
    groups = []
    for group in (std, other, crate):
        lines = []
        for p in group:
            items = sorted(uses[p], key=lambda s: (s != 'self', s.lower()))
            if len(items) == 1:
                lines.append(f'use {p}::{items[0]};')
            else:
                lines.append(f'use {p}::{{{", ".join(items)}}};')
        if lines:
            groups.append('\n'.join(lines))
    return '\n\n'.join(groups) + '\n'


NL = chr(10)


def split_leading_uses(block):
    """(leading use-statements text, rest) — the leading part is only use stmts/blank lines."""
    lines = block.splitlines(keepends=True)
    i, depth, cut = 0, 0, 0
    buf = ''
    while i < len(lines):
        s = lines[i].strip()
        if depth == 0 and not s:
            i += 1
            cut = i
            continue
        if depth == 0 and not s.startswith('use '):
            break
        buf += lines[i]
        depth = 0 if ';' in lines[i] else 1
        i += 1
        if depth == 0:
            cut = i
    return ''.join(lines[:cut]), ''.join(lines[cut:])


def resolve(m):
    ours, theirs = m.group(1), m.group(2)
    ou, orest = split_leading_uses(ours)
    tu, trest = split_leading_uses(theirs)
    a, b = use_items(ou), use_items(tu)
    if a is None or b is None:
        print(f'{path}: unparseable use block, resolve by hand')
        sys.exit(2)
    out = ''
    if a or b:
        merged = {k: set(v) for k, v in a.items()}
        for k, v in b.items():
            merged.setdefault(k, set()).update(v)
        out = render(merged)
    rest = orest
    if trest:
        sep = '' if not rest or rest.endswith(NL + NL) else NL
        rest = rest + sep + trest
    if out and rest:
        out += NL
    return out + rest


path = sys.argv[1]
text = open(path, encoding='utf-8').read()
new, n = HUNK_RE.subn(resolve, text)
open(path, 'w', encoding='utf-8', newline='\n').write(new)
print(f'{path}: resolved {n} conflict(s) (use-union / keep-both)')
