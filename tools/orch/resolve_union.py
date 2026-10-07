"""Resolve conflicts by keeping both sides (ours then theirs), e.g. for docs tables."""
import re, sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
s, n = re.subn(r'<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n',
               lambda m: m.group(1) + m.group(2), s, flags=re.S)
open(p, 'w', encoding='utf-8', newline='\n').write(s)
print(f'{p}: kept both sides of {n} conflict(s)')
