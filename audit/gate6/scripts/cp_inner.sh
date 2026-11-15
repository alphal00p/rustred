# COPY (gate6 g, copied-directory ruling + run book A10 caveat). INSIDE ns(). TAG WORKMODE(shared|internal) SEED
# O = original (CDIR $R/c), crashed by SIGKILL while 3 jobs run; C = byte copy $R/c2 (made while O is dead) started with
# --adopt-moved-campaign; O restarted at the same time: BOTH run concurrently. WORKMODE shared: the spec's dirs.work_dir is
# ONE external directory $R/work used by both; internal: no work_dir (each CDIR holds its own runs). A wrapper logs every
# program start (marker, cwd) to $R/ran.log. Checks: results of O and C vs an uninterrupted reference; verify; run
# directories moved aside ("unrecorded"); for every ACCEPTED attempt, which campaign's RUN/sup/CAMPAIGN its run dir names.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
tag=$1; mode=$2; seed=${3:-5}
R=$S/runs/cp/$tag; [ -e "$R" ] && { echo "exists: $R (use a new tag)"; exit 3; }; mkdir -p $R
cat > $R/wrap.sh <<W
#!$BASH_BIN
[ "\$1" = job ] && echo "RAN \$RR_CAMPAIGN_JOB \$(pwd) \$(date +%s.%N)" >> $R/ran.log
exec $FC "\$@"
W
chmod +x $R/wrap.sh
wd=""; [ $mode = shared ] && wd="\"work_dir\":\"$R/work\""
mkspec() { cat > $1 <<J
{"schema":"rustred.campaign.spec.v1","campaign":"g6cp","kind":"test","dirs":{$2},
 "programs":{"rr_fake_child":{"path":"$R/wrap.sh"}},
 "resources":{"policy":"plain","cpus":"$CPUS","max_jobs":3,"mem_total":"4G","host_floor":"50G","disk_floor":"50M","grace_seconds":2,"sampler_ms":200},
 "progress":{"cadence_seconds":1},
 "synthetic":{"items":6,"nodes":2,"rungs":1,"seed":$seed,"case_ms":[3000,5000],"p_norule":0.25,"p_mem":0,"p_crash":0,"deps":0,"mem_cap_mb":48}}
J
}
mkspec $R/spec.json "$wd"
mkspec $R/spec_ref.json ""
timeout -s KILL 600 $RC run --spec $R/spec_ref.json --dir $R/ref --plain > $R/ref.out 2>&1; echo "reference rc=$?"
$RC result $R/ref > $R/ref.result 2>&1
$RC run --spec $R/spec.json --dir $R/c --plain > $R/o1.out 2>&1 & O=$!
for i in $(seq 1 300); do n=$(cat $R/c/ledger/*.jsonl 2>/dev/null | grep -c '"ev":"started"'); [ "${n:-0}" -ge 3 ] && break; sleep 0.1; done
sleep 1; kill -KILL $O; wait $O; echo "O crashed (SIGKILL) after $n started; marked of O alive: $(marked "RR_CAMPAIGN_JOB=$(campid $R/c):" | wc -w)"
cp -a $R/c $R/c2
$RC run --spec $R/spec.json --dir $R/c2 --plain > $R/c_noadopt.out 2>&1; echo "copy without --adopt-moved-campaign rc=$?"
$RC run --spec $R/spec.json --dir $R/c2 --plain --adopt-moved-campaign > $R/c.out 2>&1 & C=$!
$RC run --spec $R/spec.json --dir $R/c --plain > $R/o2.out 2>&1 & O2=$!
wait $C; crc=$?; wait $O2; orc=$?
echo "copy rc=$crc original restart rc=$orc"
IDO=$(campid $R/c); IDC=$(grep -h -o '"campaign:marker_id"[^}]*' $R/c2/ledger/*.jsonl | grep -o '"path":"[^"]*"' | tail -1 | sed 's/"path":"//;s/"//')
echo "ids: original $IDO copy $IDC"
for d in c c2; do $RC result $R/$d > $R/$d.result 2>&1; $RC verify $R/$d > $R/$d.verify 2>&1; echo "$d: verify rc=$? $(python3 -I $S/scripts/rescmp.py $R/ref.result $R/$d.result)"; done
echo "moved-aside run dirs: $(ls -d $R/work/*/a*.unrecorded-* $R/c/jobs/*/a*.unrecorded-* $R/c2/jobs/*/a*.unrecorded-* 2>/dev/null | wc -l)"
ls -d $R/work/*/a*.unrecorded-* 2>/dev/null | sed "s#$R/##" | head -6 | sed 's/^/  /'
grep -h "foreign_attempt" $R/c2/ledger/*.jsonl | grep -o '"text":"[^"]\{0,160\}' | head -4 | sed 's/^/  copy note: /'
python3 -I - $R $IDO $IDC "$mode" <<'PY'
import sys, json, glob, os
R, ido, idc, mode = sys.argv[1:5]
for d, me in (("c", ido), ("c2", idc)):
    acc=[]; started={}
    for f in sorted(glob.glob(f"{R}/{d}/ledger/*.jsonl")):
        for l in open(f):
            try: e=json.loads(l)["event"]
            except Exception: continue
            if e.get("kind")!="job": continue
            b=e["body"]; ev=b.get("ev",{})
            if ev.get("ev")=="launch": started[(b["job"],b["attempt"])]=ev.get("run_dir")
            if ev.get("ev") in ("finished","settle","accept") or "accept" in json.dumps(ev)[:80]:
                acc.append((b["job"],b["attempt"],ev.get("ev")))
    foreign=0; rows=[]
    for (j,a),rd in started.items():
        try: cid=open(os.path.join(rd,"sup","CAMPAIGN")).read().strip()
        except Exception as x: cid=f"? {x.__class__.__name__}"
        if cid!=me: foreign+=1; rows.append((j,a,cid[:24]))
    print(f"{d}: launches {len(started)}; run dirs whose RUN/sup/CAMPAIGN names ANOTHER campaign (or unreadable): {foreign} {rows[:4]}")
PY
echo "program starts: $(wc -l < $R/ran.log); by original: $(grep -c "RAN $IDO:" $R/ran.log); by copy: $(grep -c "RAN $IDC:" $R/ran.log)"
awk '{print $2}' $R/ran.log | sed 's/^[^:]*://' | sort | uniq -c | awk '$1>1' | head -5 | sed 's/^/  attempt label started by both campaigns: /'
grep -h "a resource end\|crash\|row 22\|contract\|tamper\|invalid" $R/o2.out $R/c.out | cut -c1-200 | head -8 | sed 's/^/  log: /'
echo "MARKED_LEFT original=$(marked "RR_CAMPAIGN_JOB=$IDO:" | wc -w) copy=$(marked "RR_CAMPAIGN_JOB=$IDC:" | wc -w)"
