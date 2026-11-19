# gate11 (H) DRESS REHEARSAL of RUNBOOK 1.7 WITH respect_foreign_pinning FALSE (true never launches: no caller of set_foreign_pinned) inside a private namespace: the host step's site.json / spec.json with the SITE
# policy, but lock_dir and ledger_file are PRIVATE COPIES under this run (the real WS/ops/locks and CPU_LEDGER.md are never
# opened); held files and pause files are the real ones (read only). Scale 1/10: 24 items x one rung of 6-12 s on <= 8 CPUs.
# resources.containment = "marker_only" (no systemd scope in a namespace: the one difference to the host step).
# Then the post-run checks of 1.7 (1-5, 7): exit code, A12 listing, verify, signal records, RELEASED lines, STOPPED.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11/scripts/lib.sh
in_ns_guard6; shm_guard
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
tag=$1; R=$S/runs/host17b/$tag; [ -e $R ] && { echo "exists $R"; exit 3; }; mkdir -p $R/locks
for n in $(seq 0 383); do : > $R/locks/cpu.$n; done
: > $R/CPU_LEDGER.md
cat > $R/site.json <<J
{"schema":"rustred.campaign.site-policy.v1","tag":"rr-campaign",
 "lock_dir":"$R/locks","lock_name":"cpu.{n}",
 "ledger_file":"$R/CPU_LEDGER.md","ledger_scan_lines":1500,"candidates":null,
 "exclude":["168-175","288-319","128-162,164-166","192-215,258-260,262,271,335","342,362,363,378"],
 "held_files":[{"path":"$WS/ops/FOREIGN_5L_LIFT.held","key":"held"},
               {"path":"$WS/ops/FOREIGN_5L_LIFT.json","key":"all_cpus_listed"},
               {"path":"$WS/ops/OUR_PINS.json","key":"entries[].cpus"}],
 "pause_files":["$WS/ops/PAUSE","$WS/plan/auto-campaign/implementer_ops/PAUSE"],
 "idle_min":0.85,"max_cores":8,"respect_foreign_pinning":false,"ignore_own_tag_claims":true}
J
cat > $R/spec.json <<J
{"schema":"rustred.campaign.spec.v1","campaign":"host-fake-1","kind":"test",
 "dirs":{"campaign_dir":"$R/c"},
 "programs":{"rr_fake_child":{"path":"$FC"}},
 "resources":{"policy":"site","site_policy":"$R/site.json","max_jobs":8,"mem_total":"8G","host_floor":"50G",
              "disk_floor":"5G","grace_seconds":20,"sampler_ms":500,"heartbeat_timeout_s":60,"start_timeout_s":120,
              "containment":"marker_only",
              "pause_files":["$WS/ops/PAUSE","$WS/plan/auto-campaign/implementer_ops/PAUSE"]},
 "progress":{"cadence_seconds":2},
 "synthetic":{"items":24,"nodes":3,"rungs":1,"seed":1,"case_ms":[6000,12000],"p_norule":0.25,"p_mem":0.05,
              "p_crash":0.0,"deps":0.3,"mem_cap_mb":48}}
J
env | grep -q '^RR_CAMPAIGN_JOB=' && { echo "RR_CAMPAIGN_JOB in env: abort"; exit 2; }
t0=$(date +%s); $RC run --spec $R/spec.json --plain 2>> $R/run.log; rc=$?
ID=$(campid $R/c)
echo "1. exit $rc ($(( $(date +%s)-t0 )) s); campaign id $ID; $(grep -m1 'containment:' $R/c/log/supervisor.log | cut -c1-160)"
left=$(marked "RR_CAMPAIGN_JOB=$ID:" | wc -l); echo "2. A12 MARKED lines: $left"
$RC verify $R/c > $R/verify.txt 2>&1; echo "3. verify: $?"
echo "4. signal_sent $(cat $R/c/ledger/*.jsonl | grep -c '"signal_sent"'); POSSIBLY SENT $(cat $R/c/ledger/*.jsonl | grep -c 'POSSIBLY SENT'); job_containment notes $(cat $R/c/ledger/*.jsonl | grep -c 'job_containment'); started $(cat $R/c/ledger/*.jsonl | grep -c '"ev":"started"')"
cl=$(grep -c 'rr-campaign.*CLAIM\|claim' $R/CPU_LEDGER.md); rl=$(grep -c 'RELEASED' $R/CPU_LEDGER.md)
echo "5. private CPU ledger: $(wc -l < $R/CPU_LEDGER.md) lines, claims $cl, RELEASED $rl; CPUs used: $(grep -o 'cpu[ =]*[0-9]*' $R/CPU_LEDGER.md | sort -u | wc -l); excluded ever used: $(grep -oE '\b(16[89]|17[0-5]|29[0-9]|3[01][0-9]|28[89]|19[2-9]|20[0-9]|21[0-5]|25[89]|260|262|271|335)\b' $R/CPU_LEDGER.md | sort -u | tr '\n' ' ')"
echo "7. STOPPED processes in the namespace: $(for p in /proc/[0-9]*; do [ "$(cut -d' ' -f3 $p/stat 2>/dev/null)" = T ] && echo ${p#/proc/}; done | wc -l)"
$RC result $R/c > $R/result.json 2>/dev/null; echo "RESULT17 tag=$tag rc=$rc marked_left=$left verify=$(head -c 0 $R/verify.txt; $RC verify $R/c >/dev/null 2>&1; echo $?)"
