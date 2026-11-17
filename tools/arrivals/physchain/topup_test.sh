#!/usr/bin/env bash
# lane arrivals: TOP-UP SOUNDNESS TEST on a finished node X of run R: walk X on its box + the first half of its incoming files (A),
# and on its box + the second half (B), merge (merge_topup.py), compare with R/X/out (the from-scratch walk on all inputs).
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; P=$S/physchain
BIN=$1; R=$2; X=$3; T=$P/topup_test/$X
eval "$(python3 -I -c 'import json,sys
for l in open(sys.argv[1]):
    o=json.loads(l)
    if o["node"]==sys.argv[2]: print("SPEC=%s EARGS=%s INC=%s" % (o["spec"], o["edge_args"], o["incoming"]))' "$R/plan.jsonl" "$X")"
source $WS/ops/env.sh
mapfile -t inc < <(grep . "$INC")
h=$(( (${#inc[@]} + 1) / 2 ))
for part in A B; do
  D=$T/$part; mkdir -p $D/tmp $D/out $D/index; export TMPDIR=$D/tmp
  if [ $part = A ]; then sel=("${inc[@]:0:$h}"); else sel=("${inc[@]:$h}"); fi
  printf '%s\n' "${sel[@]}" > $D/incoming.txt
  arr=(); for f in "${sel[@]}"; do arr+=(--arrivals "$f"); done
  "$BIN" closure-rows --spec "$SPEC" "${arr[@]}" --out $D/rows.jsonl --summary $D/rows.summary.json --threads 8 --index-dir $D/index > $D/rows.out 2> $D/rows.err || exit 2
  "$BIN" compute --spec "$SPEC" --rows $D/rows.jsonl --out-dir $D/out --parent "$X" --edge-producer "topup test" --exact none --depth \
    --structural-min-keys 16 "${arr[@]}" --threads 8 --cache-entries 4000000 --index-dir $D/index $(cat "$EARGS") > $D/compute.out 2> $D/compute.err || exit 2
done
python3 -I $P/merge_topup.py $T/A/out $T/B/out $T/M "$X" > $T/merge.json || exit 2
python3 -I - "$R/$X/out" "$T/M" "$X" > $T/compare.json <<'PY'
import json, os, sys, hashlib
F, M, X = sys.argv[1:4]
res = {'node': X, 'files': {}, 'identical_keys_and_depths': True}
for name in sorted(os.listdir(F)):
    if not name.endswith('.arrivals.jsonl'):
        continue
    def load(p):
        d = {}
        if os.path.exists(p):
            for l in open(p):
                if '"kind":"arrival"' in l:
                    v = json.loads(l); d[tuple(v['key'])] = (v['depth'], v['senders'], tuple(v['first']['parent_key']), v['first']['term'])
        return d
    a, b = load(f'{F}/{name}'), load(f'{M}/{name}')
    ka = hashlib.sha256(''.join(str(list(k)) + '\n' for k in sorted(a)).encode()).hexdigest()
    kb = hashlib.sha256(''.join(str(list(k)) + '\n' for k in sorted(b)).encode()).hexdigest()
    same_keys = set(a) == set(b)
    dd = sum(1 for k in a if k in b and a[k][0] != b[k][0])
    fd = sum(1 for k in a if k in b and a[k][2:] != b[k][2:])
    sl = sum(1 for k in a if k in b and b[k][1] < a[k][1])
    sm = sum(1 for k in a if k in b and b[k][1] > a[k][1])
    res['files'][name] = {'scratch_keys': len(a), 'topup_keys': len(b), 'key_sha_scratch': ka, 'key_sha_topup': kb, 'same_keys': same_keys,
                          'depth_diffs': dd, 'first_sender_diffs': fd, 'topup_senders_below_scratch': sl, 'topup_senders_above_scratch (double count)': sm}
    res['identical_keys_and_depths'] &= same_keys and dd == 0
res['first_sender_diffs_total'] = sum(v['first_sender_diffs'] for v in res['files'].values())
print(json.dumps(res, indent=1))
PY
