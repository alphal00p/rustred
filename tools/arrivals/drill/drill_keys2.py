# lane arrivals: DRILL start keys from CELL_TOPS.tsv (coordinator / implementer 2026-11-17): per ancestor in the TSV, the MAIN-line
# top cells (D, top(D)) with top(D) = top_r_at_D_if_n4hint (top_r_at_D where empty), PER_MAIN keys each, plus the MARGIN-line top
# cells (D, top_r_at_D) where they differ, PER_MARGIN keys each (seeded uniform compositions; all keys when a cell is smaller).
# Ancestors absent from the TSV (no six-loop core) get no own keys. Writes drill/<X>/start.keys.jsonl + spec.json.
import csv, hashlib, json, math, os, random, sys
S = sys.argv[1]; PER_MAIN = int(sys.argv[2]); PER_MARGIN = int(sys.argv[3]); OUTD = sys.argv[4] if len(sys.argv) > 4 else 'drill2'
TSV = '/common/build-cache/vhirschi/p_integral_dev/helper-share/tadic/inputs/CELL_TOPS.tsv'
tops_main, tops_margin = {}, {}
for row in csv.DictReader(open(TSV), delimiter='\t'):
    x, D = row['node'], int(row['D'])
    hint = row.get('top_r_at_D_if_n4hint', '').strip()
    tops_margin.setdefault(x, {})[D] = int(row['top_r_at_D'])
    # exact-n4 file (2026-11-17): hint column 'superseded' -> main line = top_r_exact (= top_r_at_D there)
    if hint.lstrip('-').isdigit():
        tops_main.setdefault(x, {})[D] = int(hint)
    elif hint and (row.get('top_r_exact') or '').strip().lstrip('-').isdigit():
        tops_main.setdefault(x, {})[D] = int(row['top_r_exact'])
    else:
        tops_main.setdefault(x, {})[D] = int(row['top_r_at_D'])
plan = [json.loads(l) for l in open(S + '/physchain/run2/plan.jsonl.with-T165')]
man = json.load(open(S + '/phys/specs/manifest.json'))
def comps(total, parts, rng):
    bars = sorted(rng.sample(range(total + parts - 1), parts - 1))
    out, prev = [], -1
    for b in bars + [total + parts - 1]:
        out.append(b - prev - 1); prev = b
    return out
summary = {}
for o in plan:
    x = o['node']
    mask = json.loads(open(man[x]['table']).readline())['mask']
    lines = [i for i, c in enumerate(mask) if c == '1']; nums = [i for i, c in enumerate(mask) if c == '0']
    E, I = len(lines), len(nums)
    rng = random.Random(f'drill2-{x}')
    cells = []
    for D, r in sorted(tops_main.get(x, {}).items()):
        cells.append(('main', D, r, PER_MAIN))
        rm = tops_margin[x][D]
        if rm != r:
            cells.append(('margin', D, rm, PER_MARGIN))
    keys, info = set(), []
    for kind, D, r, per in cells:
        size = math.comb(D + E - 1, E - 1) * math.comb(r + I - 1, I - 1)
        want = min(per, size); got = set()
        while len(got) < want:
            dl = comps(D, E, rng); rn = comps(r, I, rng)
            k = [0] * len(mask)
            for i, v in zip(lines, dl): k[i] = 1 + v
            for i, v in zip(nums, rn): k[i] = -v
            got.add(tuple(k))
        keys |= got
        info.append({'line': kind, 'D': D, 'r': r, 'cell_size': size, 'sampled': len(got)})
    d = f'{S}/phys/{OUTD}/{x}'; os.makedirs(d, exist_ok=True)
    kp = d + '/start.keys.jsonl'
    open(kp, 'w').write(''.join(json.dumps(list(k), separators=(',', ':')) + '\n' for k in sorted(keys)))
    spec = json.load(open(o['spec']))
    spec['start'] = {'path': kp, 'format': 'keys_jsonl', 'sha256': hashlib.sha256(open(kp, 'rb').read()).hexdigest()}
    json.dump(spec, open(d + '/spec.json', 'w'), indent=1)
    summary[x] = {'lines': E, 'keys': len(keys), 'cells': info}
json.dump({'tsv': TSV, 'tsv_sha256': hashlib.sha256(open(TSV, 'rb').read()).hexdigest(), 'per_main': PER_MAIN, 'per_margin': PER_MARGIN, 'nodes': summary},
          open(f'{S}/phys/{OUTD}/summary.json', 'w'), indent=1)
print(sum(1 for v in summary.values() if v['keys']), 'ancestors with own keys;', sum(v['keys'] for v in summary.values()), 'start keys')
