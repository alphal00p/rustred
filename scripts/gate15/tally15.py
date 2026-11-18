import re, sys, glob, os, collections
d = sys.argv[1]
t = collections.defaultdict(lambda: [0, 0, 0, 0, 0])  # runs, viol, verified clean, unverified cleared, panics
loads = collections.defaultdict(list)
for f in sorted(glob.glob(os.path.join(d, '*.txt'))):
    if f.endswith('summary.txt'):
        continue
    lab = os.path.basename(f).rsplit('.', 3)[0]
    s = open(f, errors='replace').read()
    m = re.search(r'load (\S+) start', s)
    if m: loads[lab].append(float(m.group(1)))
    for line in s.splitlines():
        if line.startswith('G14 run ') or 'G14 run ' in line and 'first report' in line:
            r = t[lab]; r[0] += 1
            if 'VIOLATION' in line: r[1] += 1
            elif 'Verified, relay stopped' in line: r[2] += 1
            elif 'cleared after' in line: r[3] += 1
    if 'panicked' in s: t[lab][4] += 1
tot = collections.defaultdict(lambda: [0, 0, 0, 0, 0])
for lab, r in sorted(t.items()):
    print(f"{lab:28s} runs {r[0]:4d} violations {r[1]:3d} verified-clean {r[2]:4d} unverified-cleared {r[3]:3d} panics {r[4]} load {min(loads[lab]):.0f}-{max(loads[lab]):.0f}")
    k = lab.split('_')[0]
    for i in range(5): tot[k][i] += r[i]
for k, r in tot.items():
    print(f"TOTAL {k}: runs {r[0]} violations {r[1]} verified-clean {r[2]} unverified-cleared {r[3]} panics {r[4]}")
