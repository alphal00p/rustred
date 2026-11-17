# lane arrivals: S/deliver/MANIFEST.json from the delivered targets' stats (sha256 of every file, counts, cone, fixpoint check)
import glob, hashlib, json, os, sys
S = '/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals'
D = S + '/deliver'
def sha(f):
    h = hashlib.sha256()
    with open(f, 'rb') as fh:
        for b in iter(lambda: fh.read(1 << 22), b''):
            h.update(b)
    return h.hexdigest()
man = {'schema': 'rustred.campaign.arrival-delivery-manifest.v1', 'run': S + '/physchain/run2',
       'format': {'keys.bin': 'magic b"RRKEYS01", u32 LE header length, header JSON (target, frame, arity, record layout), then per key '
                  '(sorted ascending): arity x i8 (the target table frame), u8 chain depth, u8 number of sending parents (capped 255)',
                  'sample.jsonl': 'every k-th key (k in stats.files.sample.every): key, depth, dr, senders, parents {parent: depth}, first sender',
                  'stats.json': 'counts per depth / D+r / (depth, D+r) / parent; cone check; inputs with sha256'},
       'targets': {}}
man['closure_lists'] = {}
for st in sorted(glob.glob(D + '/*.stats.json')):
    s = json.load(open(st))
    t = s['target']
    if s.get('schema') == 'rustred.campaign.closure-delivery.v1':
        man['closure_lists'][t] = {k: s[k] for k in ('closure_keys', 'covered', 'terminal', 'uncovered', 'by_depth')} | {
            'files': {k: {'path': v['path'], 'sha256': sha(v['path'])} for k, v in s['files'].items()} | {'stats': {'path': st, 'sha256': sha(st)}}}
        continue
    fx = f"{S}/physchain/run2/fixpoint.{t}.json"
    man['targets'][t] = {'keys': s['keys'], 'by_depth': s['by_depth'], 'max_dr': s['cone']['max_dr_seen'], 'cone': s['cone']['label'],
                         'outside_cone': s['cone']['outside'], 'levels_populated': f"{s['cone']['levels_populated']}/{s['cone']['levels_in_cone']}",
                         'files': {k: {'path': v['path'], 'sha256': sha(v['path'])} for k, v in s['files'].items()} | {'stats': {'path': st, 'sha256': sha(st)}},
                         'fixpoint_check': {'path': fx, 'sha256': sha(fx), 'ok': json.load(open(fx))['ok']} if os.path.exists(fx) else None}
json.dump(man, open(D + '/MANIFEST.json', 'w'), indent=1)
print(json.dumps({t: {'keys': v['keys'], 'bin_sha256': v['files']['bin']['sha256'][:16]} for t, v in man['targets'].items()}))
