#!/usr/bin/env bash
# gate17 item 3 (m16-units-lock fix): does `rr_campaign units clear` after a supervisor CRASH hide the stale-lock takeover
# from the next start? Runs as pid 1's child of a private user+pid+mount namespace (the caller: unshare -Urpf --mount-proc
# --kill-child). Release binaries in $1, output root $2 (fresh). Case A: crash (SIGKILL of OUR OWN supervisor, its pid
# from $!, cmdline checked), restart. Case B: crash, `units clear`, restart. Fake children only.
set -u
REL=$1; OUT=$2
mkdir "$OUT" || exit 2
CPUS=$(python3 -I -c 'import os; l=[c for c in sorted(os.sched_getaffinity(0)) if not (168<=c<=175 or 288<=c<=319)]; print(",".join(map(str,l[:4])))')
for case in A B; do
  mkdir "$OUT/$case"; D=$OUT/$case/c
  cat > "$OUT/$case/spec.json" <<J
{"schema":"rustred.campaign.spec.v1","campaign":"g17-unitslock-$case-$$","programs":{"rr_fake_child":{"path":"$REL/rr_fake_child"}},
 "resources":{"policy":"plain","cpus":"$CPUS","containment":"marker_only","max_jobs":2,"mem_total":"4G","disk_floor":"1G",
  "grace_seconds":5,"sampler_ms":300,"host_floor":"100M","heartbeat_timeout_s":60,"start_timeout_s":120},
 "progress":{"cadence_seconds":1},
 "synthetic":{"items":4,"nodes":1,"rungs":1,"seed":3,"case_ms":[15000,20000],"mem_cap_mb":40,"heartbeat_s":1,"stop_grace_s":3}}
J
  "$REL/rr_campaign" run --spec "$OUT/$case/spec.json" --dir "$D" < /dev/null > "$OUT/$case/run1.log" 2>&1 & P=$!
  for _ in $(seq 100); do grep -q '"kind":"job".*"started"' "$D"/ledger/segment-*.jsonl 2>/dev/null && break; sleep 0.2; done
  sleep 1
  if tr '\0' ' ' < "/proc/$P/cmdline" | grep -q "^$REL/rr_campaign run "; then kill -KILL "$P"; fi
  wait "$P"; echo "$case: run1 rc $? (SIGKILL = 137); LOCK.json after the crash: $(ls "$D/LOCK.json" 2>&1)"
  if [ "$case" = B ]; then
    "$REL/rr_campaign" units clear "$D" > "$OUT/$case/units.log" 2>&1; echo "$case: units clear rc $?: $(tr '\n' ' ' < "$OUT/$case/units.log")"
    echo "$case: LOCK.json after units clear: $(ls "$D/LOCK.json" 2>&1)"
  fi
  "$REL/rr_campaign" run --spec "$OUT/$case/spec.json" --dir "$D" < /dev/null > "$OUT/$case/run2.log" 2>&1; echo "$case: run2 rc $?"
  echo "$case: lock_takeover records: $(grep -c '"kind":"lock_takeover"' "$D"/ledger/segment-*.jsonl | tr '\n' ' ')"
  grep -h '"kind":"lock_takeover"' "$D"/ledger/segment-*.jsonl | cut -c1-400
  echo "$case: 'stale lock' lines in the supervisor log: $(grep -c 'stale lock' "$D/log/supervisor.log")"
  "$REL/rr_campaign" verify "$D" > "$OUT/$case/verify.txt" 2>&1; echo "$case: verify rc $?"
done
