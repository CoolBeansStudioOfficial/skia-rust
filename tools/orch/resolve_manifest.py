"""Resolve manifest.toml conflicts: per conflicting hunk, prefer the side whose
status is 'passing' (both sides only ever flip todo->passing/failing), else theirs."""
import re, sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
def pick(m):
    ours, theirs = m.group(1), m.group(2)
    if 'status = "passing"' in ours and 'status = "passing"' not in theirs:
        return ours
    return theirs
s, n = re.subn(r'<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n', pick, s, flags=re.S)
open(p, 'w', encoding='utf-8', newline='\n').write(s)
print(f'{p}: resolved {n} conflict(s)')
