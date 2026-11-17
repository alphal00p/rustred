# lane arrivals: the --edge / --child-table arguments of one parent of the chain DAG (inspection helper; data only)
import json, sys
S, A, parent = sys.argv[1], sys.argv[2], sys.argv[3]
dag = json.load(open(S + '/chain162/dag.json'))
tables = {json.loads(l)['node']: json.loads(l)['table'] for l in open(A + '/specs/nodes.jsonl')}
special = {'T162i': '/common/build-cache/vhirschi/p_integral_dev/bank-conv/20261114c/qualified/T162.origin000.table.jsonl',
           'T178f': '/common/build-cache/vhirschi/p_integral_dev/vk5l-wp1/t178/T178-box.unionbox.jsonl'}
out = []
for k, f in sorted(dag['files'].items()):
    p, c = k.split('->')
    if p != parent:
        continue
    lab = json.load(open(f))['child']['label']
    out += ['--edge', f, '--child-table', lab + '=' + special.get(lab, tables.get(c, tables.get(lab, '')))]
print(' '.join(out))
