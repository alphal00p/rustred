# lane arrivals: DRILL SAMPLE start keys (coordinator 2026-11-16: a few hundred top-cell keys per ancestor, "sample, not a fixpoint"):
# per ancestor X (E lines in its table frame), the TOP CELLS of the conservative rule D <= 4, r <= E + D - 9, i.e. (D, r) =
# (D, E + D - 9) for D = 0..4; per cell PER_CELL distinct keys drawn uniformly over the compositions (seeded; every key when the
# cell is smaller): lines k >= 1 with sum (k - 1) = D, numerators k <= 0 with sum (-k) = r. Writes drill/<X>/start.keys.jsonl and a
# spec (the node's physical spec with this start set; T266 with its relation entries).
import json, os, random, sys, hashlib, math
S = sys.argv[1]; PER_CELL = int(sys.argv[2]) if len(sys.argv) > 2 else 50
plan = [json.loads(l) for l in open(S + '/physchain/run2/plan.jsonl')]
man = json.load(open(S + '/phys/specs/manifest.json'))
def comps(total, parts, rng):
    # a uniform random weak composition of total into parts (stars and bars)
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
    E = len(lines)
    rng = random.Random(f'drill-{x}')
    keys = []
    cells = {}
    for D in range(0, 5):
        r = E + D - 9
        size = math.comb(D + E - 1, E - 1) * math.comb(r + len(nums) - 1, len(nums) - 1)
        got = set()
        want = min(PER_CELL, size)
        while len(got) < want:
            dl = comps(D, E, rng); rn = comps(r, len(nums), rng)
            k = [0] * len(mask)
            for i, v in zip(lines, dl): k[i] = 1 + v
            for i, v in zip(nums, rn): k[i] = -v
            got.add(tuple(k))
        cells[f'{D}/{r}'] = {'cell_size': size, 'sampled': len(got)}
        keys += sorted(got)
    d = f'{S}/phys/drill/{x}'; os.makedirs(d, exist_ok=True)
    kp = d + '/start.keys.jsonl'
    open(kp, 'w').write(''.join(json.dumps(list(k), separators=(',', ':')) + '\n' for k in keys))
    spec = json.load(open(o['spec']))
    spec['start'] = {'path': kp, 'format': 'keys_jsonl', 'sha256': hashlib.sha256(open(kp, 'rb').read()).hexdigest()}
    json.dump(spec, open(d + '/spec.json', 'w'), indent=1)
    summary[x] = {'lines': E, 'keys': len(keys), 'cells': cells}
json.dump(summary, open(S + '/phys/drill/summary.json', 'w'), indent=1)
print(len(summary), 'ancestors;', sum(v['keys'] for v in summary.values()), 'start keys;', 'T265:', summary.get('T265', {}).get('cells'))
