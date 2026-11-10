"""base vs variant per cell: keys, edges, reduce s, step s, peak RSS (guard), T-VAL vs wp0 goldens, T-BFS.
usage: compare.py VARIANT_TAG [BASE_TAG]"""
import sys, json, os, subprocess
M = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/m5-maps'
G = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/m5-r0/exec/wp0/goldens/preduction'
T = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/m5-r0/exec/a2/tools/tval_a2.py'
var, base = sys.argv[1], (sys.argv[2] if len(sys.argv) > 2 else 'base')
cells = [('NO1_d0_r6', 'P4L_NO1_L4_d0_r6'), ('NO1_d2_r6', 'P4L_NO1_L4_d2_r6'), ('NO2_d0_r6', 'P4L_NO2_L4_d0_r6'),
         ('NO2_d2_r6', 'P4L_NO2_L4_d2_r6'), ('NO3_d0_r6', 'P4L_NO3_L4_d0_r6'), ('NO3_d2_r6', 'P4L_NO3_L4_d2_r6'),
         ('L3NO_d0_r8', 'P3L_NO_L3_d0_r8'), ('L3BE_d2_r8', 'P3L_BE_L3_d2_r8')]
def get(tag, c):
    d = f'{M}/runs/{tag}_{c}'
    s = json.load(open(f'{d}/summary.json'))
    g = json.load(open(f'{d}/guard/completion.json'))
    return s, g.get('peak_group_rss_bytes', 0) / 1e9, d
print(f'| cell | keys {base} -> {var} | edges | reduce s | step s | peak GB | values = goldens | T-BFS |')
print('|---|---|---|---|---|---|---|---|')
for c, gname in cells:
    try:
        b, bg, _ = get(base, c); v, vg, vd = get(var, c)
    except FileNotFoundError:
        continue
    tv = json.loads(subprocess.run(['python3', T, f'{vd}/preduction.txt', f'{G}/{gname}/preduction.txt'],
                                   capture_output=True, text=True).stdout)
    pct = lambda x, y: f'{100*(y-x)/x:+.1f} %'
    print(f"| {c} | {b['popped']:,} -> {v['popped']:,} ({pct(b['popped'], v['popped'])}) | {b['edges']:,} -> {v['edges']:,} "
          f"({pct(b['edges'], v['edges'])}) | {b['reduce_seconds']:.1f} -> {v['reduce_seconds']:.1f} ({pct(b['reduce_seconds'], v['reduce_seconds'])}) | "
          f"{b['step_seconds']:.1f} -> {v['step_seconds']:.1f} | {bg:.2f} -> {vg:.2f} | {tv['values_identical']} | {v['t_bfs']} |")
