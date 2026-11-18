import re, sys, glob, os, collections
rx = re.compile(r"G14L kind (\w+) jobs (\d+) conc (\d+) loops (\d+): wall ([\d.]+) s, ([\d.]+) jobs/min; proof latency p50 ([\d.]+) s p90 ([\d.]+) s max ([\d.]+) s \(n \d+\); verified (\d+), unverified (\d+), cleared later (\d+) \(max after ([\d.]+) s\), never cleared within 120 s (\d+)")
rows = collections.defaultdict(list)
for f in sorted(glob.glob(os.path.join(sys.argv[1], '*.txt'))):
    if f.endswith('summary.txt'): continue
    s = open(f, errors='replace').read()
    m = rx.search(s); ld = re.search(r'load (\S+) start', s)
    if not m: print('UNPARSED', f); continue
    b = os.path.basename(f).split('_')[0]
    kind, jobs, conc, loops, wall, jpm, p50, p90, mx, ver, unv, clr, clrmax, never = m.groups()
    rows[(f"{kind} {jobs}x{conc}x{loops}", b)].append((ld.group(1) if ld else '?', jpm, p90, mx, unv, clr, clrmax, never))
print(f"{'workload':18} {'build':5} {'load':>9} {'jobs/min':>11} {'p90 s':>11} {'max s':>11} {'unv@10s':>8} {'cleared(max after s)':>22} {'never':>5}")
for k in sorted(rows):
    r = rows[k]; j = lambda i: "/".join(x[i] for x in r)
    print(f"{k[0]:18} {k[1]:5} {j(0):>9} {j(1):>11} {j(2):>11} {j(3):>11} {j(4):>8} {j(5)+' ('+j(6)+')':>22} {j(7):>5}")
