# m2 (gate6 g): spawn-barrier attacks by a same-uid process. INSIDE innsu (host-like: uid 1000, no capabilities). TAG MODE
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
tag=$1; mode=$2
R=$S/runs/m2/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
echo "uid=$(id -u) $(grep CapEff /proc/self/status)"
cat > $R/wrap.sh <<W
#!$BASH_BIN
[ "\$1" = job ] && echo "RAN \$RR_CAMPAIGN_JOB \$(date +%s.%N)" >> $R/ran.log
exec $FC "\$@"
W
chmod +x $R/wrap.sh
P_CRASH=0 P_MEM=0 CASE_MS='[200,600]' MAXJOBS=2 make_spec $R/spec.json $R/c $R/wrap.sh 11 ${ITEMS:-12}
timeout -s KILL 600 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1 & SUP=$!
sleep 0.05
$S/bin/thief $mode $R/thief.log $SUP & TH=$!
wait $SUP; rc=$?
kill -KILL $TH; wait $TH 2>/dev/null
ID=$(campid $R/c)
echo "run rc=$rc; thief: opened $(grep -c OPENED $R/thief.log), open refused $(grep -c 'open .*: Permission denied' $R/thief.log), other open errors $(grep -v OPENED $R/thief.log | grep -v Permission | grep -c ' open ')"
grep '^supervisor' $R/thief.log | sed 's/^/  /'
grep -h 'stole\|wrote\|replayed' $R/thief.log | sort | uniq -c | sort -rn | head -5 | sed 's/^/  /'
# every RAN must have its job.started BEFORE (ledger) -- check existence per attempt
python3 -I - $R <<'PY'
import sys, json, glob
R=sys.argv[1]; started=set(); launched=set(); evs={}
for f in sorted(glob.glob(R+"/c/ledger/*.jsonl")):
    for l in open(f):
        try: e=json.loads(l)["event"]
        except Exception: continue
        if e.get("kind")!="job": continue
        b=e["body"]; ev=b.get("ev",{}).get("ev"); k=(b["job"],b["attempt"])
        evs.setdefault(k,[]).append(ev)
        if ev=="started": started.add(k)
ran=[]
try:
    for l in open(R+"/ran.log"):
        _,m,t=l.split(); c,job,a=m.split(":",2); ran.append((job,int(a[1:])))
except FileNotFoundError: pass
bad=[k for k in ran if k not in started]
print(f"programs RAN: {len(ran)}; RAN without job.started: {len(bad)} {bad[:5]}; attempts {len(evs)}")
from collections import Counter
print("  attempt event sequences:", Counter(tuple(v) for v in evs.values()).most_common(6))
PY
grep -h 'wrong token\|ended before it recorded' $R/c/jobs/*/*/sup/* $R/c/jobs/*/sup/* 2>/dev/null | sort | uniq -c | head -3 | sed 's/^/  shim: /'
grep -rh 'wrong token\|ended before it recorded' $R/c/jobs 2>/dev/null | sort | uniq -c | head -3 | sed 's/^/  shim msg: /'
grep -h 'row 23\|crashed\|crash\|125' $R/run.out | head -5 | cut -c1-220 | sed 's/^/  log: /'
grep -h 'FINISHED\|ATTENTION\|exit' $R/run.out | tail -3 | cut -c1-220 | sed 's/^/  log: /'
# is a stolen token persisted anywhere under the campaign dir or the logs?
for t in $(grep -o 'stole 32 bytes:[0-9a-f]*' $R/thief.log | sed 's/.*://' | head -3); do
  echo "  token $t: hex found in CDIR files: $(grep -rl "$t" $R/c 2>/dev/null | wc -l); raw bytes found: $(python3 -I -c "import sys,os,glob;b=bytes.fromhex(sys.argv[1]);print(sum(1 for p in glob.glob(sys.argv[2]+'/**',recursive=True) if os.path.isfile(p) and b in open(p,'rb').read()))" $t $R/c)"
done
echo "MARKED_LEFT=$(marked "RR_CAMPAIGN_JOB=$ID:" | wc -w)"
$RC verify $R/c > $R/verify.out 2>&1; echo "verify rc=$?"
