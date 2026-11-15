# gate7 (g) MJ-B variants. INSIDE ns() (root map). TAG CASE
#  noreplace : renameat2(RENAME_NOREPLACE) -> EINVAL (seccomp; a file system without it): refuse or fall back unsafely?
#  race      : two DIFFERENT campaign directories, same external work_dir, started at the same moment (10 rounds)
#  killclaim : SIGKILL at random 0-400 ms of a campaign's FIRST start (20 rounds): self-lockout? a second campaign taking over?
#  killcreate: SIGKILL at random times while launching (12 rounds) then the same command: '.creating.' leftovers, result == ref
#  oldcopy   : campaign started by the PROCFIX6 binary (no owner file), copied, both resumed by procfix7: who owns the work_dir?
#  rmwork    : the external work_dir is moved away mid-run: typed end? resume == reference?
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7/scripts/lib.sh
in_ns_guard6
tag=$1; case_=$2
R=$S/runs/mjb/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
spec_w() { # OUT CDIR WORK SEED ITEMS
  CAMP=${CAMP:-mjb} P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS=${CASE_MS:-'[300,900]'} MAXJOBS=2 make_spec $1 $2 $FC $4 $5
  python3 -I - "$1" "$3" <<'P'
import json,sys
p,w=sys.argv[1],sys.argv[2]; s=json.load(open(p)); s["dirs"]["work_dir"]=w; json.dump(s,open(p,"w"))
P
}
owner() { cat $1/RR_CAMPAIGN_WORK_OWNER 2>/dev/null | head -1; }
case $case_ in
noreplace)
  spec_w $R/spec.json $R/c $R/w 3 4
  timeout -s KILL 300 $S/bin/g7seccomp $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1; echo "run under the renameat2-EINVAL filter: rc=$?"
  grep -h "creating\|Invalid argument\|renam\|refus\|OPERATOR\|FINISHED\|attention" $R/run.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-220 | sort | uniq -c | sort -rn | head -6 | sed 's/^/  log: /'
  echo "  work dir entries: $(ls -A $R/w 2>/dev/null | tr '\n' ' ' | cut -c1-300)"
  echo "  launch records: $(grep -h -c '"ev":"launch"' $R/c/ledger/*.jsonl 2>/dev/null | paste -sd+ | bc)  started: $(grep -h -c '"ev":"started"' $R/c/ledger/*.jsonl 2>/dev/null | paste -sd+ | bc)"
  timeout -s KILL 300 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; echo "same command without the filter: rc=$?"
  $RC verify $R/c > $R/v.out 2>&1; echo "verify rc=$?";;
race)
  for i in $(seq 1 10); do
    mkdir -p $R/r$i; spec_w $R/r$i/a.json $R/r$i/a $R/r$i/w 3 2; spec_w $R/r$i/b.json $R/r$i/b $R/r$i/w 4 2
    timeout -s KILL 300 $RC run --spec $R/r$i/a.json --dir $R/r$i/a --plain > $R/r$i/a.out 2>&1 & A=$!
    timeout -s KILL 300 $RC run --spec $R/r$i/b.json --dir $R/r$i/b --plain > $R/r$i/b.out 2>&1 & B=$!
    wait $A; ra=$?; wait $B; rb=$?
    ida=$(campid $R/r$i/a); idb=$(campid $R/r$i/b); o=$(owner $R/r$i/w)
    who=$([ "$o" = "$ida" ] && echo A || ([ "$o" = "$idb" ] && echo B || echo "?"))
    echo "round $i: rcA=$ra rcB=$rb owner=$who refusedA=$(grep -c 'belongs to campaign' $R/r$i/a.out) refusedB=$(grep -c 'belongs to campaign' $R/r$i/b.out) run dirs: $(ls $R/r$i/w | grep -vc RR_CAMPAIGN_WORK_OWNER)"
  done;;
killclaim)
  for i in $(seq 1 20); do
    mkdir -p $R/k$i; spec_w $R/k$i/a.json $R/k$i/a $R/k$i/w 3 2; spec_w $R/k$i/b.json $R/k$i/b $R/k$i/w 4 2
    d=$(( (RANDOM % 400) )); $RC run --spec $R/k$i/a.json --dir $R/k$i/a --plain > $R/k$i/a1.out 2>&1 & A=$!
    sleep $(printf '0.%03d' $d); kill -KILL $A; wait $A 2>/dev/null
    o1=$(owner $R/k$i/w); left=$(ls -A $R/k$i/w 2>/dev/null | grep -v '^RR_CAMPAIGN_WORK_OWNER$' | tr '\n' ' ')
    if [ $((i % 2)) = 0 ]; then
      timeout -s KILL 300 $RC run --spec $R/k$i/b.json --dir $R/k$i/b --plain > $R/k$i/b.out 2>&1; rb=$?
      timeout -s KILL 300 $RC run --spec $R/k$i/a.json --dir $R/k$i/a --plain > $R/k$i/a2.out 2>&1; ra=$?
      echo "round $i kill@${d}ms: claim after kill='${o1:-none}' left=[$left] then B rc=$rb, A rc=$ra (A refused: $(grep -c 'belongs to campaign' $R/k$i/a2.out); A touched work dir before refusal: no run dir of A expected)"
    else
      timeout -s KILL 300 $RC run --spec $R/k$i/a.json --dir $R/k$i/a --plain > $R/k$i/a2.out 2>&1; ra=$?
      echo "round $i kill@${d}ms: claim after kill='${o1:-none}' left=[$left] then same command rc=$ra (self-refused: $(grep -c 'belongs to campaign' $R/k$i/a2.out))"
    fi
  done;;
killcreate)
  spec_w $R/ref.json $R/ref $R/wref 5 6; timeout -s KILL 300 $RC run --spec $R/ref.json --dir $R/ref --plain > $R/ref.out 2>&1; echo "reference rc=$?"
  spec_w $R/spec.json $R/c $R/w 5 6
  for i in $(seq 1 12); do
    $RC run --spec $R/spec.json --dir $R/c --plain > $R/s$i.out 2>&1 & A=$!
    sleep $(printf '%d.%03d' $((RANDOM % 3)) $((RANDOM % 1000))); kill -KILL $A; wait $A 2>/dev/null; echo "session $i SIGKILLed: creating-dirs $(find $R/w -maxdepth 2 -name '.*creating*' | wc -l), unrecorded $(find $R/w -maxdepth 2 -name '*.unrecorded-*' | wc -l)"
  done
  timeout -s KILL 600 $RC run --spec $R/spec.json --dir $R/c --plain > $R/final.out 2>&1; rc=$?
  echo "final rc=$rc"; $RC result $R/ref > $R/ref.result 2>/dev/null; $RC result $R/c > $R/c.result 2>/dev/null; python3 -I $S/scripts/rescmp.py $R/ref.result $R/c.result 2>&1 | tail -2
  $RC verify $R/c > $R/v.out 2>&1; echo "verify rc=$?";;
oldcopy)
  OLD=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/bin/debug/rr_campaign
  [ -x $OLD ] || { echo "no procfix6 binary"; exit 4; }
  CASE_MS='[2000,3000]' spec_w $R/spec.json $R/c $R/w 3 6
  $OLD run --spec $R/spec.json --dir $R/c --plain > $R/old.out 2>&1 & A=$!
  for i in $(seq 1 300); do n=$(cat $R/c/ledger/*.jsonl 2>/dev/null | grep -c '"ev":"started"'); [ "${n:-0}" -ge 2 ] && break; sleep 0.1; done
  kill -KILL $A; wait $A 2>/dev/null; echo "procfix6 session SIGKILLed; owner file: '$(owner $R/w)'"
  cp -a $R/c $R/cp
  timeout -s KILL 300 $RC run --spec $R/spec.json --dir $R/cp --plain --adopt-moved-campaign > $R/cp.out 2>&1; rcp=$?
  echo "copy (procfix7, --adopt-moved-campaign) started FIRST: rc=$rcp owner now '$(owner $R/w)' (copy id $(campid $R/cp | tail -1))"
  grep -h "not this campaign\|belongs to\|claimed\|run_dir_not_ours\|foreign" $R/cp.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-200 | sort | uniq -c | head -5 | sed 's/^/  copy log: /'
  timeout -s KILL 300 $RC run --spec $R/spec.json --dir $R/c --plain > $R/orig.out 2>&1; ro=$?
  echo "original (procfix7) afterwards: rc=$ro"; grep -h "belongs to" $R/orig.out | cut -c1-200 | head -2 | sed 's/^/  orig log: /'
  echo "  renamed / removed run dirs of the original: $(find $R/w -maxdepth 2 -name '*.unrecorded-*' | wc -l)";;
rmwork)
  spec_w $R/ref.json $R/ref $R/wref 7 6; timeout -s KILL 300 $RC run --spec $R/ref.json --dir $R/ref --plain > $R/ref.out 2>&1; echo "reference rc=$?"
  CASE_MS='[1500,2500]' spec_w $R/spec.json $R/c $R/w 7 6
  $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & A=$!
  for i in $(seq 1 300); do n=$(cat $R/c/ledger/*.jsonl 2>/dev/null | grep -c '"ev":"started"'); [ "${n:-0}" -ge 2 ] && break; sleep 0.1; done
  mv $R/w $R/w.gone; echo "work dir moved away"; wait $A; rc=$?
  echo "run rc=$rc"; grep -h "ERROR\|WARN.*work\|not this campaign\|stopping\|exit" $R/run1.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-220 | sort | uniq -c | sort -rn | head -5 | sed 's/^/  log: /'
  echo "  work dir recreated: $([ -e $R/w ] && echo yes || echo no); owner file in it: '$(owner $R/w)'"
  timeout -s KILL 600 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; echo "same command: rc=$?"
  $RC result $R/ref > $R/ref.result 2>/dev/null; $RC result $R/c > $R/c.result 2>/dev/null; python3 -I $S/scripts/rescmp.py $R/ref.result $R/c.result 2>&1 | tail -2
  $RC verify $R/c > $R/v.out 2>&1; echo "verify rc=$?";;
esac
