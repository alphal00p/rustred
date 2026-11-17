#!/usr/bin/env python3
"""Assemble a per-key exactness delivery directory (generic: every path is an argument).

  deliver.py --out DIR --results-dir R1 [--results-dir R2 ...] --class-table-args '<args for class_table.py>'
             [--copy SRC=DEST ...] [--tsv-scope-only]

1. results: every results/r<rule>.jsonl of the given prover output dirs merged into DIR/results/r<rule>.jsonl, ONE line per id
   (the last verdict of an id wins: a key "not decided" and later re-run keeps its final verdict), sorted by key.
2. class table: class_table.py run on the merged results -> DIR/class_table.jsonl + DIR/class_table.summary.json, and the
   evidence form DIR/class_table.evidence.tsv (columns table_index, dispatch_position_f0bd7e76, target_rhs_sha256, class, ...) for the
   rules in scope (the class words as evidence: "every first-match key in cone", "sampled N keys", "dead in cone", "none").
3. --copy SRC=DEST copies inputs into DIR (DEST relative).
4. MANIFEST.json: sha256 and bytes of every file under DIR (MANIFEST itself excluded), and of the prover chunk dirs listed with
   --chunks-dir (as proofs.chunks.manifest.json: per chunk file sha256, not copied).
"""
import argparse, hashlib, json, os, shlex, shutil, subprocess, sys


def sha256_file(p):
    h = hashlib.sha256()
    with open(p, 'rb') as f:
        for b in iter(lambda: f.read(1 << 22), b''):
            h.update(b)
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--out', required=True)
    ap.add_argument('--results-dir', action='append', default=[])
    ap.add_argument('--class-table', required=True, help='path of class_table.py')
    ap.add_argument('--class-table-args', required=True)
    ap.add_argument('--copy', action='append', default=[])
    ap.add_argument('--chunks-dir', action='append', default=[])
    a = ap.parse_args()
    out = a.out
    os.makedirs(os.path.join(out, 'results'), exist_ok=True)
    merged = {}
    for d in a.results_dir:
        rd = os.path.join(d, 'results')
        for f in sorted(os.listdir(rd)):
            for line in open(os.path.join(rd, f)):
                if line.endswith('\n'):
                    v = json.loads(line)
                    merged.setdefault(v['rule'], {})[v['id']] = v
    for r, rows in merged.items():
        with open(os.path.join(out, 'results', f'r{r}.jsonl'), 'w') as g:
            for v in sorted(rows.values(), key=lambda v: v['key']):
                g.write(json.dumps(v) + '\n')
    ct = os.path.join(out, 'class_table.jsonl')
    subprocess.run([sys.executable, '-I', a.class_table] + shlex.split(a.class_table_args)
                   + ['--results', os.path.join(out, 'results'), '--out', ct, '--summary', os.path.join(out, 'class_table.summary.json')],
                   check=True)
    cols = ['table_index', 'dispatch_position_f0bd7e76', 'target_rhs_sha256', 'domain_sha256', 'class', 'keys_exact', 'keys_not_decided',
            'fm_keys_cone', 'live', 'liveness_source', 'evidence_class_before', 'keys_by_source', 'class_source']
    with open(os.path.join(out, 'class_table.evidence.tsv'), 'w') as g:
        g.write('\t'.join(cols) + '\n')
        for line in open(ct):
            v = json.loads(line)
            if v.get('class_source', '').startswith('per-key proofs'):
                row = dict(v, table_index=v['rule'])
                g.write('\t'.join('' if row.get(c) is None else (json.dumps(row[c]) if isinstance(row[c], (dict, list)) else str(row[c]))
                                  for c in cols) + '\n')
    for c in a.copy:
        src, dst = c.split('=', 1)
        d = os.path.join(out, dst)
        os.makedirs(os.path.dirname(d), exist_ok=True)
        shutil.copy2(src, d)
    if a.chunks_dir:
        cm = {}
        for d in a.chunks_dir:
            for root, _, files in os.walk(d):
                for f in sorted(files):
                    if f.endswith(('.keys', '.out.json', '.err', '.time')):
                        p = os.path.join(root, f)
                        cm[p] = {'sha256': sha256_file(p), 'bytes': os.path.getsize(p)}
        json.dump({'chunks': cm, 'files': len(cm)}, open(os.path.join(out, 'proofs.chunks.manifest.json'), 'w'))
    man = {}
    for root, _, files in os.walk(out):
        for f in sorted(files):
            p = os.path.join(root, f)
            rel = os.path.relpath(p, out)
            if rel == 'MANIFEST.json':
                continue
            man[rel] = {'sha256': sha256_file(p), 'bytes': os.path.getsize(p)}
    json.dump({'schema': 'exact-keys.delivery.manifest.v1', 'dir': out, 'files': man}, open(os.path.join(out, 'MANIFEST.json'), 'w'),
              indent=1, sort_keys=True)
    print(json.dumps({'files': len(man), 'manifest_sha256': sha256_file(os.path.join(out, 'MANIFEST.json'))}))


if __name__ == '__main__':
    main()
