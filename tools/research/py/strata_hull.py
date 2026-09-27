#!/usr/bin/env python3
"""Read-only: if every Apply native box were replaced by its stratum hull, how do volumes and
(power-law-predicted) costs compare?  Estimates only; ignores cascade effects."""
import json, math, sys, collections, statistics

path = sys.argv[1]
H = {}
rows = []
with open(path, 'rb') as f:
    for line in f:
        if b'"native_inspection"' not in line or b'"phase":"Apply"' not in line:
            continue
        r = json.loads(line)
        up = r['upper']; lo = r['lower']
        if any(u is None for u in up):
            continue
        pb = r.get('power_bounds') or {}
        key = (r['owner'], pb.get('max_positive_power'), r.get('rank'), pb.get('max_power_difference'), pb.get('min_power_difference'))
        sec = r.get('seconds') or 0.0
        succ = (r.get('stats') or {}).get('successors', 0)
        lv = sum(math.log2(b - a + 1) for a, b in zip(lo, up))
        rows.append((lv, sec, succ))
        h = H.get(key)
        if h is None:
            H[key] = h = [list(up), 0, 0.0, 0, 0.0, 0.0]
        else:
            h[0] = [max(x, y) for x, y in zip(h[0], up)]
        h[1] += 1; h[2] += sec; h[3] += succ; h[4] += 2 ** lv; h[5] = max(h[5], sec)

xs = [x[0] for x in rows if x[1] > 0]
ys = [math.log2(x[1]) for x in rows if x[1] > 0]
zs = [math.log2(x[2] + 1) for x in rows if x[1] > 0]
mx, my, mz = statistics.fmean(xs), statistics.fmean(ys), statistics.fmean(zs)
vx = sum((a - mx) ** 2 for a in xs)
b = sum((a - mx) * (c - my) for a, c in zip(xs, ys)) / vx
a0 = my - b * mx
bz = sum((a - mx) * (c - mz) for a, c in zip(xs, zs)) / vx
az = mz - bz * mx
print(f'fit log2(sec) = {a0:.3f} + {b:.3f} log2(vol); log2(succ+1) = {az:.3f} + {bz:.3f} log2(vol); n={len(xs)}')

tot_sec = sum(h[2] for h in H.values())
tot_succ = sum(h[3] for h in H.values())
pred_sec = 0.0; pred_succ = 0.0
cov_ratios = []
multi = 0
for key, (hup, n, s, su, vsum, smax) in H.items():
    hv = sum(math.log2(u + 1) for u in hup)
    ps = max(2 ** (a0 + b * hv), smax)  # hull at least as expensive as its heaviest member (assumption)
    pred_sec += ps
    pred_succ += 2 ** (az + bz * hv)
    cov_ratios.append(vsum / 2 ** hv)
    if n > 1:
        multi += 1
cov_ratios.sort()
m = len(cov_ratios)
print(f'Apply strata {len(H)} ({multi} with >1 native); natives {sum(h[1] for h in H.values())}')
print(f'actual member seconds {tot_sec:.0f}; predicted hull seconds [E] {pred_sec:.0f} (ratio {pred_sec/tot_sec:.3f})')
print(f'actual member successors {tot_succ:.3g}; predicted hull successors [E] {pred_succ:.3g} (ratio {pred_succ/tot_succ:.3f})')
print('sum(member vol)/hull vol quantiles p10 p50 p90:', cov_ratios[m // 10], cov_ratios[m // 2], cov_ratios[9 * m // 10])
# hot owner
hot = '011101110111000'
hs = [(k, h) for k, h in H.items() if k[0] == hot]
print('hot owner strata', len(hs), 'natives', sum(h[1] for _, h in hs), 'sec', round(sum(h[2] for _, h in hs), 1),
      'max single native sec', round(max(h[5] for _, h in hs), 1))
for k, h in sorted(hs, key=lambda kh: -kh[1][2])[:8]:
    hv = sum(math.log2(u + 1) for u in h[0])
    print('   ', k[1:], 'n', h[1], 'sec', round(h[2], 1), 'max', round(h[5], 2), 'hull log2vol', round(hv, 1), 'hull upper', h[0])
