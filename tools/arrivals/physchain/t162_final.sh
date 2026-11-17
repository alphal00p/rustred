#!/usr/bin/env bash
# lane arrivals: T162 END GAME (coordinator 2026-11-16): (1) T165 top-up merge once T165.A and T165.B are done; (2) once T163, T165, T167
# are done: partial manifest lines, the full T162 ARRIVALS delivery (deliver.py) and its fixpoint check; (3) T162's own EXACT CLOSURE
# from all arrivals under its shipped service (table + relation entries): S/deliver/T162.closure.keys.bin (+ .uncovered.jsonl), with
# the closure paths of 12 sample uncovered keys. Heavy steps run as transient units (ledger lines); everything logged in chain.log.
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; P=$S/physchain; R=$P/run2; F=$P/t162final
mkdir -p $F
log() { echo "$(date -u +%FT%TZ) $*" >> $R/chain.log; }
cpus() { python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held"; }
unit() { # unit <name> <threads> <memG> <what> -- cmd...
  local n=$1 th=$2 mem=$3 what=$4; shift 5
  while :; do av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo); [ $((av - mem / 2)) -ge 75 ] && break; sleep 60; done
  printf '%s (implementer:arrivals): unpinned, CPUQuota %s00 %%, nice 10, MemoryMax %sG (unit %s: %s)\n' "$(date -u +%FT%TZ)" "$th" "$mem" "$n" "$what" >> $WS/ops/CPU_LEDGER.md
  systemd-run --user --wait --collect --unit=$n -p CPUQuota=${th}00% -p AllowedCPUs="$(cpus)" -p MemoryMax=${mem}G -p MemorySwapMax=0 -p Nice=10 -- "$@" > $F/$n.unit.txt 2>&1
}
# (1) T165 top-up merge
until [ -e $R/T165.A/done ] && [ -e $R/T165.B/done ]; do sleep 60; done
if [ ! -e $R/T165/done ]; then
  mkdir -p $R/T165/out
  unit impl-arrivals-t165merge 1 8 "T165 top-up merge (keys union, depth min)" -- python3 -I $P/merge_topup.py $R/T165.A/out $R/T165.B/out $R/T165/out T165 || { log "T165 merge FAILED"; exit 2; }
  echo '{"parts": ["A", "B"], "merge": "merge_topup.py: keys union, depth min, first = least (parent_key, term), senders summed (upper bound)", "proof": "physchain/topup_test/T203,T208 compare.json: same key sha, depths, firsts"}' > $R/T165/topup.json
  touch $R/T165/done; log "T165 done (top-up merge of T165.A + T165.B)"
  cp $R/plan.jsonl.with-T165 $R/plan.jsonl
fi
# (2) every T162 parent done: partial lines, full arrivals delivery, fixpoint check
until [ -e $R/T163/done ] && [ -e $R/T165/done ] && [ -e $R/T167/done ]; do sleep 60; done
log "T162: every parent done"
unit impl-arrivals-deliver-T162p3 1 4 "partial T162 lines of the last parents" -- python3 -I $P/deliver_partial.py $R T162 T162i $S/physcone/cones.json
unit impl-arrivals-deliver-T162 1 8 "T162 ARRIVALS fixpoint delivery (merge of the per-parent key files)" -- python3 -I $P/merge_bins.py $S/deliver/T162.partial T162 $S/physcone/cones.json $S/deliver || { log "T162 delivery FAILED"; exit 2; }
python3 -I $P/fixpoint_check.py $R T162 T162i > $F/fixpoint.out
python3 -I $P/manifest.py > $F/manifest.out
log "T162 ARRIVALS delivered (S/deliver/T162.*; fixpoint check $(cat $F/fixpoint.out))"
touch $F/arrivals.done
# (3) T162's exact closure list
ls $R/*/out/*-T162i.arrivals.jsonl | while read f; do grep -q '"kind":"arrival"' $f && echo $f; done > $F/arrivals.txt
TK=$(python3 -I -c 'import json,sys; print(" ".join("--trace-key \x27" + json.dumps(k, separators=(",", ":")) + "\x27" for k in json.load(open(sys.argv[1]))))' $P/t162_sample_uncovered.json)
cat > $F/closure.sh <<EOS
#!/usr/bin/env bash
source $WS/ops/env.sh
export TMPDIR=$F/tmp; mkdir -p \$TMPDIR $F/index $F/out
arr=(); while read -r f; do [ -n "\$f" ] && arr+=(--arrivals "\$f"); done < $F/arrivals.txt
[ -s $F/rows.done ] || { /run/current-system/sw/bin/time -v $S/bin/rr_arrivals.a31 closure-rows --spec $S/phys/specs_rel/T162/spec.json "\${arr[@]}" --out $F/rows.jsonl --summary $F/rows.summary.json --threads 32 --index-dir $F/index > $F/rows.out 2> $F/rows.err && date -u +%FT%TZ > $F/rows.done; } || exit 2
/run/current-system/sw/bin/time -v $S/bin/rr_arrivals.a31 compute --spec $S/phys/specs_rel/T162/spec.json --rows $F/rows.jsonl --out-dir $F/out --parent T162 --edge-producer "T162 closure list" \
  --exact none --depth "\${arr[@]}" --closure-only --closure-out $S/deliver/T162.closure.keys.bin $TK --threads 32 --cache-entries 4000000 --index-dir $F/index > $F/closure.json 2> $F/closure.err || exit 2
EOS
chmod +x $F/closure.sh
log "T162 closure list starts"
unit impl-arrivals-t162closure 32 40 "T162 exact closure list from all arrivals (covered / UNCOVERED, depth, sample chains)" -- unshare -Urpf --mount-proc -- bash $F/closure.sh || { log "T162 closure list FAILED"; exit 2; }
log "T162 closure list written (S/deliver/T162.closure.keys.bin)"
touch $F/closure.done
