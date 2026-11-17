# lane arrivals: the EXPECTED peak (GB) of a chain node for the L17 start gate (start iff host MemAvailable - expected
# >= 100 GB; the cap MemoryMax stays NODE_MEM). = 1.1 x the node's own measured compute peak if it ran; else 1.1 x the
# p75 peak of the finished nodes of its wave (>= 2 of them; wave 1 = the wave-1 table), else EXPECT_DEFAULT; never below
# 1.1 x its own measured rows peak; R/expect_override.json {node: GB} replaces the estimate until the node is measured.
# Prints one integer (GB, ceil).
# Usage: python3 -I expect.py <run dir R> <node> <default GB>
import json, math, os, re, sys
R, x, dflt = sys.argv[1], sys.argv[2], float(sys.argv[3])
def peak(path):
    try:
        m = re.search(r'Maximum resident set size \(kbytes\): (\d+)', open(path).read())
        return int(m.group(1)) / 1e6 if m else None
    except OSError:
        return None
try:
    ov = json.load(open(R + '/expect_override.json')).get(x)
except (OSError, ValueError):
    ov = None
comp, rows = peak(f'{R}/{x}/compute.err'), peak(f'{R}/{x}/rows.err')
if ov and not comp:
    print(math.ceil(max(float(ov), (rows or 0) * 1.1))); sys.exit()
if comp:
    print(math.ceil(max(comp, rows or 0) * 1.1)); sys.exit()
plan = {json.loads(l)['node']: json.loads(l) for l in open(R + '/plan.jsonl')}
w = plan.get(x, {}).get('wave')
same = sorted(p for n, o in plan.items() if o['wave'] == w and n != x
              for p in [peak(f'{R}/{n}/compute.err')] if p and os.path.exists(f'{R}/{n}/done'))
base = same[min(len(same) - 1, (3 * len(same)) // 4)] * 1.1 if len(same) >= 2 else dflt
print(math.ceil(max(base, (rows or 0) * 1.1)))
