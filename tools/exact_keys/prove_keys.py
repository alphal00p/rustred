#!/usr/bin/env python3
"""Per-key exactness proofs of table rules with the engine's proof kernel (`rr_proof --mode instance-span`).

Generic (no node, table or count is built in): every table, rule, key, family and binary is an argument.

Input  --jobs JOBS.jsonl, one line per (rule, key): {"rule": <table index>, "key": [<arity ints>], "sources": [<tags>]}
       (duplicates of (rule, key) are merged; their source tags are united).
Output --out DIR:
  rules/r<rule>.json            the rule's identity: line sha256 (checked against --index), target_rhs_sha256
                                (sha256 of json.dumps([target, rhs], separators=(",", ":"))), target, free slots
  results/r<rule>.jsonl         ONE line per key, appended as soon as its chunk ends (resumable: a key whose id already has
                                a line is never re-run). id = sha256("<target_rhs_sha256>:<json key>") binds the result to the
                                relation and the key, not to a table position.
  chunks/r<rule>/cNNNNN.*       the kernel's input (.keys) and its raw output (.out.json, .err), for audit
  STOP                          written (and nothing new started) as soon as a key is not EXACT and not "not decided"
Verdicts: EXACT (status EXACT_IN_SPAN_AT_KEY), NOT_DECIDED (time-out, memory death, kernel error, UNDECIDED: listed),
          NOT_EXACT_AT_SEEDS (EXACT_NOT_IN_SPAN_AT_KEY: the rule is not a combination of IBP rows at its seeds at that key;
          no wrong value is shown by this) and OTHER (any status not listed above): both STOP the run.
"""
import argparse, concurrent.futures as cf, hashlib, json, os, signal, subprocess, sys, time

EXACT = 'EXACT_IN_SPAN_AT_KEY'
NOT_DECIDED_STATUSES = ('UNDECIDED',)


def sha256_bytes(b):
    return hashlib.sha256(b).hexdigest()


def key_id(trs, key):
    return sha256_bytes(f'{trs}:{json.dumps(key, separators=(",", ":"))}'.encode())


def load_index(path):
    idx = {}
    for line in open(path):
        e = json.loads(line)
        if e.get('type') == 'rule':
            idx[e['index']] = e
    return idx


def rule_identity(table, idx, rule, out):
    p = f'{out}/rules/r{rule}.json'
    if os.path.exists(p):
        return json.load(open(p))
    e = idx[rule]
    with open(table, 'rb') as f:
        f.seek(e['offset'])
        b = f.read(e['length'])
    line_sha = sha256_bytes(b.rstrip(b'\n'))
    if line_sha != e['sha256'] and sha256_bytes(b) != e['sha256']:
        raise SystemExit(f'rule {rule}: line sha256 {line_sha} != index {e["sha256"]}')
    v = json.loads(b)
    if v.get('index') != rule:
        raise SystemExit(f'rule {rule}: the line at the offset carries index {v.get("index")}')
    trs = sha256_bytes(json.dumps([v['target'], v['rhs']], separators=(',', ':')).encode())
    free = [s for s, x in enumerate(v['fixed']) if x is None]
    ident = {'rule': rule, 'offset': e['offset'], 'length': e['length'], 'line_sha256': e['sha256'], 'target': v['target'],
             'target_rhs_sha256': trs, 'free_slots': free, 'fixed': v['fixed'], 'rhs_terms': len(v['rhs']), 'bytes': e['length']}
    del v, b
    tmp = p + '.tmp'
    json.dump(ident, open(tmp, 'w'))
    os.replace(tmp, p)
    return ident


def done_ids(path):
    s = set()
    if os.path.exists(path):
        with open(path) as f:
            for line in f:
                if line.endswith('\n'):
                    try:
                        v = json.loads(line)
                    except ValueError:
                        continue
                    # a key "not decided" before (time-out, memory, kernel error) is run again on resume; its last line counts
                    if v.get('verdict') != 'NOT_DECIDED':
                        s.add(v['id'])
    return s


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--table', required=True)
    ap.add_argument('--index', required=True, help='per-line index JSONL (type, index, offset, length, sha256)')
    ap.add_argument('--rr-proof', required=True)
    ap.add_argument('--family', required=True)
    ap.add_argument('--jobs', required=True)
    ap.add_argument('--out', required=True)
    ap.add_argument('--parallel', type=int, default=4)
    ap.add_argument('--chunk', type=int, default=20, help='keys per kernel run')
    ap.add_argument('--chunk-bytes', type=int, default=0,
                    help='if > 0: chunk size scaled down for big records: max(1, min(chunk, chunk-bytes / record bytes))')
    ap.add_argument('--timeout', type=int, default=7200, help='seconds per kernel run')
    ap.add_argument('--rules', default='', help='comma list: only these rules')
    ap.add_argument('--max-keys-per-rule', type=int, default=0, help='> 0: only the first N jobs of each rule (measurement)')
    ap.add_argument('--stop-file', default='', help='extra stop flag to honour (e.g. a quota PAUSE)')
    ap.add_argument('--kernel-arg', action='append', default=[],
                    help='extra kernel argument, repeatable (e.g. --kernel-arg=--symmetry-group --kernel-arg=FILE)')
    a = ap.parse_args()
    out = a.out
    for d in ('rules', 'results', 'chunks'):
        os.makedirs(f'{out}/{d}', exist_ok=True)
    stop = f'{out}/STOP'
    idx = load_index(a.index)
    only = set(int(x) for x in a.rules.split(',') if x.strip())
    jobs = {}
    for line in open(a.jobs):
        if not line.strip():
            continue
        j = json.loads(line)
        r = int(j['rule'])
        if only and r not in only:
            continue
        k = tuple(int(x) for x in j['key'])
        jobs.setdefault(r, {}).setdefault(k, set()).update(j.get('sources', []))
    log = open(f'{out}/prove.log', 'a')

    def say(msg):
        log.write(f'{time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())} {msg}\n')
        log.flush()

    plan = []
    for r in sorted(jobs):
        ident = rule_identity(a.table, idx, r, out)
        res = f'{out}/results/r{r}.jsonl'
        have = done_ids(res)
        keys = list(jobs[r].items())
        if a.max_keys_per_rule:
            keys = keys[:a.max_keys_per_rule]
        todo = [(k, src) for k, src in keys if key_id(ident['target_rhs_sha256'], list(k)) not in have]
        K = a.chunk
        if a.chunk_bytes > 0:
            K = max(1, min(K, a.chunk_bytes // max(1, ident['bytes'])))
        os.makedirs(f'{out}/chunks/r{r}', exist_ok=True)
        base = len([x for x in os.listdir(f'{out}/chunks/r{r}') if x.endswith('.keys')])
        for c in range(0, len(todo), K):
            plan.append((r, ident, base + c // K, todo[c:c + K]))
        say(f'rule {r}: {len(keys)} keys, {len(have)} already done, {len(todo)} to run in chunks of {K}')
    say(f'plan: {len(plan)} kernel runs, parallel {a.parallel}')

    def run(job):
        r, ident, c, ks = job
        if os.path.exists(stop) or (a.stop_file and os.path.exists(a.stop_file)):
            return r, c, 'skipped'
        free = ident['free_slots']
        stem = f'{out}/chunks/r{r}/c{c:05d}'
        with open(stem + '.keys', 'w') as g:
            for k, _ in ks:
                for s, x in enumerate(ident['fixed']):
                    if x is not None and k[s] != x:
                        raise SystemExit(f'rule {r}: key {k} contradicts fixed slot {s}')
                # a point rule (no free slot) gets its full key: the kernel refuses an empty key object
                g.write(json.dumps({'key': {f'n{s}': k[s] for s in (free or range(len(k)))}}) + '\n')
        wd = stem + '.wd'
        os.makedirs(wd, exist_ok=True)
        env = dict(os.environ, TMPDIR=wd)
        cmd = [a.rr_proof, '--mode', 'instance-span', '--table', a.table, '--rule', str(r), '--offset', str(ident['offset']),
               '--keys', stem + '.keys', '--family', a.family] + a.kernel_arg
        t0 = time.time()
        p = subprocess.Popen(['/run/current-system/sw/bin/time', '-f', '%e %M', '-o', stem + '.time'] + cmd,
                             stdout=subprocess.PIPE, stderr=open(stem + '.err', 'w'), cwd=wd, env=env, text=True,
                             start_new_session=True)
        try:
            so, _ = p.communicate(timeout=a.timeout)
            why = None
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)  # our own child's own session (start_new_session): nothing else is in it
            so, _ = p.communicate()
            why = f'time-out {a.timeout} s'
        secs = time.time() - t0
        rc = p.returncode
        lines = [x for x in (so or '').splitlines() if x.startswith('{')]
        rep = None
        if lines:
            try:
                rep = json.loads(lines[-1])
            except ValueError:
                rep = None
        with open(stem + '.out.json', 'w') as g:
            g.write(lines[-1] if lines else '')
        try:
            el, rss = open(stem + '.time').read().split()[-2:]
            rss_gb = int(rss) / 1048576
        except (OSError, ValueError):
            rss_gb = None
        bykey = {}
        if rep and isinstance(rep.get('keys'), list):
            for kr in rep['keys']:
                kk = kr.get('key') or {}
                full = list(ks[0][0])
                for s in (free or range(len(full))):
                    full[s] = kk.get(f'n{s}', full[s])
                bykey[tuple(full)] = kr
        rows = []
        for k, src in ks:
            kr = bykey.get(tuple(k))
            if kr is None:
                status = None
                note = why or (f'kernel rc {rc}' if rc else 'no verdict line for this key')
                if rc and rc < 0 or rc in (137, 9) or (rss_gb is None and rc):
                    note += ' (killed: memory or signal)'
            else:
                status = kr.get('status')
                note = None
            if status == EXACT:
                verdict = 'EXACT'
            elif status is None or status in NOT_DECIDED_STATUSES:
                verdict = 'NOT_DECIDED'
            elif status.startswith('EXACT_NOT_IN_SPAN_AT_KEY'):
                verdict = 'NOT_EXACT_AT_SEEDS'
            else:
                verdict = 'OTHER'
            row = {'id': key_id(ident['target_rhs_sha256'], list(k)), 'rule': r, 'target_rhs_sha256': ident['target_rhs_sha256'],
                   'line_sha256': ident['line_sha256'], 'key': list(k), 'sources': sorted(src), 'verdict': verdict,
                   'status': status, 'note': note, 'chunk': f'r{r}/c{c:05d}', 'chunk_keys': len(ks),
                   'chunk_seconds': round(secs, 3), 'chunk_max_rss_gb': rss_gb,
                   'record_sha256_kernel': (rep or {}).get('record_sha256'),
                   'key_seconds': (kr or {}).get('seconds'), 'deg_L': (kr or {}).get('deg_L'),
                   'nonzero_weights': (kr or {}).get('nonzero_weights')}
            rows.append(row)
        with open(f'{out}/results/r{r}.jsonl', 'a') as g:
            for row in rows:
                g.write(json.dumps(row) + '\n')
        bad = [x for x in rows if x['verdict'] in ('NOT_EXACT_AT_SEEDS', 'OTHER')]
        if bad:
            with open(stop, 'a') as g:
                for x in bad:
                    g.write(json.dumps(x) + '\n')
        return r, c, f'{sum(x["verdict"] == "EXACT" for x in rows)}/{len(rows)} exact, {len(bad)} stop, {secs:.1f} s, {rss_gb} GB'

    n = 0
    with cf.ThreadPoolExecutor(a.parallel) as ex:
        for r, c, msg in ex.map(run, plan):
            n += 1
            say(f'chunk r{r}/c{c:05d}: {msg} ({n}/{len(plan)})')
    say('STOPPED: see STOP' if os.path.exists(stop) else 'all chunks done')


if __name__ == '__main__':
    main()
