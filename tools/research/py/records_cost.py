#!/usr/bin/env python3
"""Read-only: per-native cost distribution, box volume vs cost, stratum compression.

Streams a records-*.jsonl sidecar segment (optionally every k-th line).
"""
import json, math, sys, collections

path = sys.argv[1]
stride = int(sys.argv[2]) if len(sys.argv) > 2 else 1

kinds = collections.Counter()
nat = []  # (phase, owner, seconds, successors, logvol, A, R, Dmax, Dmin, pieces, shift_groups, npts_width)
strata = collections.Counter()
strata_sec = collections.Counter()
owner_sec = collections.Counter()
owner_cnt = collections.Counter()
unbounded = 0
with open(path, 'rb') as f:
    for i, line in enumerate(f):
        if i % stride:
            continue
        r = json.loads(line)
        k = r.get('record_kind')
        kinds[k] += 1
        if k != 'native_inspection':
            continue
        lo = r.get('lower') or []
        up = r.get('upper') or []
        lv = 0.0
        wsum = 0
        ub = False
        for a, b in zip(lo, up):
            if b is None:
                ub = True
                continue
            lv += math.log2(b - a + 1)
            wsum += b - a
        if ub:
            unbounded += 1
        pb = r.get('power_bounds') or {}
        st = r.get('stats') or {}
        phase = r.get('phase'); owner = r.get('owner')
        sec = r.get('seconds') or 0.0
        succ = st.get('successors', st.get('events', 0))
        A = pb.get('max_positive_power'); R = r.get('rank')
        dmax = pb.get('max_power_difference'); dmin = pb.get('min_power_difference')
        nat.append((phase, owner, sec, succ, lv, A, R, dmax, dmin, st.get('selected_pieces', 0), st.get('shift_groups', 0), wsum))
        key = (phase, owner, A, R, dmax, dmin)
        strata[key] += 1
        strata_sec[key] += sec
        owner_sec[(phase, owner)] += sec
        owner_cnt[(phase, owner)] += 1

print('path', path, 'stride', stride)
print('kinds', dict(kinds))
n = len(nat)
print('natives', n, 'with unbounded coord', unbounded)

def dist(vals, label):
    vals = sorted(vals)
    tot = sum(vals)
    m = len(vals)
    if not m:
        return
    q = lambda p: vals[min(m - 1, int(p * m))]
    print(f'{label}: n={m} sum={tot:.4g} mean={tot/m:.4g} p50={q(.5):.4g} p90={q(.9):.4g} p99={q(.99):.4g} p99.9={q(.999):.4g} max={vals[-1]:.4g}')
    for frac in (0.001, 0.01, 0.1):
        top = vals[int((1 - frac) * m):]
        print(f'   top {frac*100:g}% carry {sum(top)/tot*100:.1f}% of {label}')

for ph in ('Apply', 'Route'):
    sub = [x for x in nat if x[0] == ph]
    print('==', ph, len(sub))
    dist([x[2] for x in sub], 'seconds')
    dist([x[3] for x in sub], 'successors')

apply = [x for x in nat if x[0] == 'Apply']
# volume bins vs cost
bins = collections.defaultdict(lambda: [0, 0.0, 0, 0])
for x in apply:
    b = int(x[4])  # log2 box volume (coordinate box only)
    e = bins[b]
    e[0] += 1; e[1] += x[2]; e[2] += x[3]; e[3] += x[9]
print('Apply: log2(box lattice points) bin -> count, mean sec, mean successors, mean pieces, mean sec per 2^bin points')
for b in sorted(bins):
    c, s, su, pc = bins[b]
    print(f'  {b:3d}  n={c:8d}  sec={s/c:9.4f}  succ={su/c:10.1f}  pieces={pc/c:8.1f}  sec/pt={s/c/2**b:.3e}')

# Spearman-ish: correlation of log volume and log seconds
import statistics
xs = [x[4] for x in apply if x[2] > 0]
ys = [math.log2(x[2]) for x in apply if x[2] > 0]
if len(xs) > 2:
    mx, my = statistics.fmean(xs), statistics.fmean(ys)
    cov = sum((a - mx) * (b - my) for a, b in zip(xs, ys))
    vx = sum((a - mx) ** 2 for a in xs); vy = sum((b - my) ** 2 for b in ys)
    slope = cov / vx if vx else float('nan')
    print(f'Apply log2(sec) vs log2(vol): pearson={cov/math.sqrt(vx*vy):.3f} slope={slope:.3f}')
# rank R vs cost
rb = collections.defaultdict(lambda: [0, 0.0, 0])
for x in apply:
    e = rb[x[6]]; e[0] += 1; e[1] += x[2]; e[2] += x[3]
print('Apply by rank cap R: count, total sec share, mean sec, mean succ')
tot = sum(x[2] for x in apply) or 1
for r in sorted(rb, key=lambda v: (v is None, v)):
    c, s, su = rb[r]
    print(f'  R={r}: n={c} share={s/tot*100:.1f}% mean_sec={s/c:.4f} mean_succ={su/c:.1f}')

print('distinct (phase,owner) among natives', len(owner_cnt))
print('distinct strata (phase,owner,A,R,Dmax,Dmin) among natives', len(strata), 'natives per stratum mean', n / max(1, len(strata)))
top = sorted(strata.items(), key=lambda kv: -kv[1])[:10]
for kk, c in top:
    print('   stratum', kk, 'natives', c, 'sec', round(strata_sec[kk], 1))
tot_sec = sum(owner_sec.values()) or 1
print('top (phase,owner) by native seconds:')
for kk, s in sorted(owner_sec.items(), key=lambda kv: -kv[1])[:10]:
    print('   ', kk, f'{s/tot_sec*100:.1f}% of seconds', owner_cnt[kk], 'natives')
