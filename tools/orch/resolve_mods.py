"""Resolve add/add conflicts in module-list files by taking the sorted union.

Usage: python resolve_mods.py <file> [--cfg-test]

An item is one `[pub[(..)]] mod name;` line plus the attribute lines just above it.
Visibility and attributes are preserved. Any other line inside a conflict hunk aborts
(exit 2) so nothing is silently dropped. With --cfg-test every item in the whole file
gets exactly one `#[cfg(test)]` (for tests/src/unit/mod.rs).
"""
import re
import sys

MOD_RE = re.compile(r'^((?:pub(?:\([^)]*\))? )?)mod (\w+);$')
HUNK_RE = re.compile(r'<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n', re.S)

path = sys.argv[1]
cfg_test = '--cfg-test' in sys.argv
text = open(path, encoding='utf-8').read()


def items(block, strict):
    out, attrs = {}, []
    for line in block.splitlines():
        s = line.strip()
        if not s:
            continue
        if s.startswith('#['):
            attrs.append(s)
            continue
        m = MOD_RE.match(s)
        if m:
            out[m.group(2)] = ([a for a in attrs if not (cfg_test and a == '#[cfg(test)]')], m.group(1))
        elif strict:
            print(f'{path}: unexpected line in conflict: {s!r}')
            sys.exit(2)
        attrs = []
    return out


def render(merged):
    lines = []
    for name in sorted(merged):
        attrs, vis = merged[name]
        lines.extend(attrs + (['#[cfg(test)]'] if cfg_test else []))
        lines.append(f'{vis}mod {name};')
    return '\n'.join(lines) + '\n'


def resolve(m):
    merged = items(m.group(1), True)
    merged.update(items(m.group(2), True))
    return render(merged)


new, n = HUNK_RE.subn(resolve, text)

if cfg_test:
    # Re-render the whole module list so cfg(test) is uniform; keep the header.
    lines = new.splitlines(keepends=True)
    first = next(i for i, l in enumerate(lines)
                 if l.strip().startswith('#[') or MOD_RE.match(l.strip()))
    header = ''.join(lines[:first]).rstrip('\n') + '\n\n'
    new = header + render(items(''.join(lines[first:]), False))

# Test modules (`tests`, `tests_*`, `*_tests`) must always be cfg(test): a shared attribute
# line outside a conflict hunk can otherwise end up on the wrong module after sorting.
TEST_MOD_RE = re.compile(r'tests(_\w+)?|\w+_tests')
fixed = []
for line in new.split('\n'):
    m = MOD_RE.match(line.strip())
    if m and TEST_MOD_RE.fullmatch(m.group(2)):
        if not fixed or fixed[-1].strip() != '#[cfg(test)]':
            fixed.append('#[cfg(test)]')
    fixed.append(line)
new = '\n'.join(fixed)

open(path, 'w', encoding='utf-8', newline='\n').write(new)
print(f'{path}: resolved {n} conflict(s)')
