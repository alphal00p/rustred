"""result.json (basis_opt) -> RUSTRED_P_ISP_PREFERENCE json (replaces the embedded table: the 3 embedded entries are kept
unless the optimiser chose those nodes). usage: write_pref.py RESULT.json SLOTS.json OUT.json [only_changed=1] [pids]"""
import sys, json
res, slots_p, out = sys.argv[1:4]
only = sys.argv[4] if len(sys.argv) > 4 else '1'
pick = set(int(x) for x in sys.argv[5].split(',')) if len(sys.argv) > 5 else None
r = json.load(open(res)); slots = {p['id']: p for p in json.load(open(slots_p))}
EMB = {"p_node_01040000010100010100000201000003010001020100010302000002030100": [13, 2, 1, 11, 3, 12, 10],
       "p_node_01040000010100010100000201000003020000010201000103020000": [1, 13, 3, 5, 6, 10, 12],
       "p_node_01040000010100010200000002010000030100010201000103020000": [1, 13, 3, 5, 6, 10, 8]}
table = dict(EMB)
def neg(v): return [-x for x in v]
for pid_s, o in r['owners'].items():
    pid = int(pid_s)
    if pick is not None and pid not in pick: continue
    if only == '1' and o['isps_new'] == o['isps_old']: continue
    p = slots[pid]; L = p['loops']; w = L + 1
    rows = [s['line'] for s in p['slots'] if 'line' in s]
    q = [int(k == L) for k in range(w)]
    def idx(v):
        for i, m in enumerate(rows):
            if v == m or v == neg(m): return i
        if v == q or v == neg(q): return -1
        return None
    def unit_of(v):
        nz = [i for i, x in enumerate(v) if x != 0]
        return nz[0] if len(nz) == 1 and abs(v[nz[0]]) == 1 else None
    pairs, units = [], []
    for a, b in o['isps_new']:
        ia, ib = idx(a), idx(b)
        if ia is not None and ib is not None and not (ia == -1 and ib == -1):
            pairs.append([ia, ib]); continue
        ua, ub = unit_of(a), unit_of(b)
        assert ua is not None and ub is not None, (pid, a, b)
        i, j = sorted((ua, ub))
        if j == L: units.append(L * (L + 1) // 2 + i)
        else: units.append(i * L - i * (i - 1) // 2 + (j - i))
    table[o['name']] = {'pairs': pairs, 'units': units}
json.dump(table, open(out, 'w'), indent=1)
print(len(table), 'entries')
