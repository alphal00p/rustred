#!/usr/bin/env python3
"""gate10: print one line per ledger record (seq, sha256 of the line) of CDIR/ledger: every segment-*.jsonl in name order
(sealed and active), then exit. Used before / after a run: the BEFORE list must be a prefix of the AFTER list (no earlier
record altered, removed or replaced; a new ledger over an old one changes record 1)."""
import hashlib, os, sys, json
d = os.path.join(sys.argv[1], 'ledger')
try:
    names = sorted(n for n in os.listdir(d) if n.startswith('segment-') and n.endswith('.jsonl'))
except OSError as e:
    print('ERROR', e); sys.exit(0)
for n in names:
    with open(os.path.join(d, n), 'rb') as f:
        for line in f:
            if not line.endswith(b'\n'):
                print('TORN', n, hashlib.sha256(line).hexdigest()[:16]); continue
            try:
                seq = json.loads(line.split(b'\n')[0].rsplit(b' ', 1)[0] if False else line).get('seq')
            except Exception:
                seq = '?'
            print(seq, hashlib.sha256(line).hexdigest()[:16])
