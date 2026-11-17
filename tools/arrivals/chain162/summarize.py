# lane arrivals: per-node cost and arrival counts of a chain run (inspection of our own outputs)
import json, os, re, sys
R = sys.argv[1]
def tv(path):
    try:
        e = open(path).read()
    except OSError:
        return {}
    g = lambda pat: (re.search(pat, e) or [None, None])[1]
    return {'wall': g(r'Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): (\S+)'), 'user': float(g(r'User time \(seconds\): (\S+)') or 0),
            'sys': float(g(r'System time \(seconds\): (\S+)') or 0), 'gb': round(int(g(r'Maximum resident set size \(kbytes\): (\d+)') or 0) / 1e6, 1)}
tot = 0.0
for l in open(R + '/plan.jsonl'):
    o = json.loads(l); x = o['node']; d = f'{R}/{x}'
    if not os.path.isdir(d):
        continue
    rows = tv(d + '/rows.err'); comp = tv(d + '/compute.err')
    rs = json.load(open(d + '/rows.summary.json')) if os.path.exists(d + '/rows.summary.json') else {}
    cs = json.load(open(f'{d}/out/{x}.summary.json')) if os.path.exists(f'{d}/out/{x}.summary.json') else {}
    arr = {f['child']: f['keys'] for f in cs.get('files', [])}
    cpu = rows.get('user', 0) + rows.get('sys', 0) + comp.get('user', 0) + comp.get('sys', 0)
    tot += cpu
    print(f"{x} L{o['lines']} done={os.path.exists(d + '/done')} start={rs.get('start_keys')} closure={rs.get('closure_keys')} "
          f"rows={rows.get('wall')} compute={comp.get('wall')} cpu_h={cpu / 3600:.2f} peak={comp.get('gb')}GB "
          f"arrivals_total={sum(arr.values())} top={sorted(arr.items(), key=lambda kv: -kv[1])[:4]}")
print(f'TOTAL CPU-h so far {tot / 3600:.2f}')
