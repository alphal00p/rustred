# SUPND (gate6 m2 / run book A12): a RUNNING supervisor's /proc as seen by a capability-less same-uid process, and the
# run book's A12 listing VERBATIM, with (a) the live supervisor, (b) a MARKED non-dumpable process of this campaign.
# INSIDE innsu (host-like).
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
R=$S/runs/supnd; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
P_CRASH=0 P_MEM=0 CASE_MS='[20000,20000]' MAXJOBS=2 make_spec $R/spec.json $R/c $FC 5 2
$RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1 & SUP=$!
for i in $(seq 1 200); do n=$(cat $R/c/ledger/*.jsonl 2>/dev/null | grep -c '"ev":"started"'); [ "${n:-0}" -ge 2 ] && break; sleep 0.1; done
ID=$(campid $R/c); echo "supervisor $SUP running, $n jobs started, id $ID"
for f in environ fd/0 cwd mem status cmdline; do ( exec 3< /proc/$SUP/$f ) 2>/dev/null && echo "  open /proc/SUP/$f: OK" || echo "  open /proc/SUP/$f: DENIED"; done
echo "  owner of /proc/SUP dir: $(stat -c %u /proc/$SUP), of /proc/SUP/environ: $(stat -c %u /proc/$SUP/environ); stat -c %U /proc/SUP = $(stat -c %U /proc/$SUP)"
env RR_CAMPAIGN_JOB=$ID:zz9:a1 $H nodump </dev/null >/dev/null 2>&1; sleep 0.3
ND=$(for p in /proc/[0-9]*; do tr '\0' ' ' < $p/cmdline 2>/dev/null | grep -q "hostile nodump" && echo ${p#/proc/}; done | tail -1)
echo "marked non-dumpable process $ND (env RR_CAMPAIGN_JOB=$ID:zz9:a1): dir owner $(stat -c %u /proc/$ND), environ readable: $(head -c1 /proc/$ND/environ >/dev/null 2>&1 && echo yes || echo NO)"
echo "--- RUNBOOK A12 listing, verbatim (ID=$ID) ---"
for p in /proc/[0-9]*; do e=$(tr '\0' '\n' < $p/environ 2>/dev/null) || { [ "$(stat -c %U $p 2>/dev/null)" = root ] && \
  grep -q "^Uid:[[:space:]]*$(id -u)[[:space:]]" $p/status 2>/dev/null && echo "UNREADABLE ${p#/proc/} $(cat $p/comm)"; continue; }
  echo "$e" | grep -q "^RR_CAMPAIGN_JOB=$ID:" && echo "MARKED ${p#/proc/} $(tr '\0' ' ' < $p/cmdline | cut -c1-120)"; done
echo "--- same listing with the environ FILE owner (stat -c %u \$p/environ) instead of the directory owner ---"
for p in /proc/[0-9]*; do e=$(tr '\0' '\n' < $p/environ 2>/dev/null) || { [ "$(stat -c %u $p/environ 2>/dev/null)" != "$(id -u)" ] && \
  grep -q "^Uid:[[:space:]]*$(id -u)[[:space:]]" $p/status 2>/dev/null && echo "UNREADABLE ${p#/proc/} $(cat $p/comm)"; continue; }
  echo "$e" | grep -q "^RR_CAMPAIGN_JOB=$ID:" && echo "MARKED ${p#/proc/} $(tr '\0' ' ' < $p/cmdline | cut -c1-120)"; done
echo "(supervisor is pid $SUP, marked non-dumpable is pid $ND)"
$RC control $R/c stop --reason gate6 > /dev/null 2>&1; wait $SUP; echo "supervisor after control stop: rc=$?"
grep -h "sweep\|unreadable\|survivor\|unproven" $R/run.out | sed 's/^[0-9TZ:-]* //' | cut -c1-200 | sort | uniq -c | head -6 | sed 's/^/  log: /'
echo "marked non-dumpable alive after the stop: $(alive $ND && echo yes || echo no)"
alive $ND && kill -KILL $ND
