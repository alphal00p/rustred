import sys, struct, glob
# Sum native seconds and count inspections from admission-trace job files (headers only).
tot = {0: [0, 0.0, 0], 1: [0, 0.0, 0]}
for path in sorted(glob.glob(sys.argv[1] + '/jobs-*.bin')):
    with open(path, 'rb') as f:
        head = f.read(12)
        arity = struct.unpack('<I', head[8:12])[0]
        ib = 2 + 4 + 4 + 4 * arity + 24
        hdr = 36 + ib
        while True:
            h = f.read(hdr)
            if len(h) < hdr:
                break
            magic, jid, blen, recs = struct.unpack('<IIII', h[:16])
            assert magic == 0x4A4F4231
            total, secs = struct.unpack('<Qd', h[16:32])
            status = h[32]
            phase = h[36]
            t = tot[min(phase, 1)]
            t[0] += 1; t[1] += secs; t[2] += (status & 1)
            f.seek(blen, 1)
for ph, (n, s, e) in tot.items():
    print(('apply' if ph == 0 else 'route'), 'records', n, 'native seconds', round(s, 1), 'ms/record', round(1000 * s / max(n, 1), 3), 'errors', e)
n = tot[0][0] + tot[1][0]; s = tot[0][1] + tot[1][1]
print('all records', n, 'native seconds', round(s, 1), 'ms/record', round(1000 * s / max(n, 1), 3))
