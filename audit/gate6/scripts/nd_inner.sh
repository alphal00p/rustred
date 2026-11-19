# ND (gate6): an UNMARKED non-dumpable process of our uid (stand-in for the host's `tailscaled be-child ssh`: uid ours,
# /proc/<pid> dir owned by us, environ root-owned / unreadable). INSIDE innsu (host-like ownership semantics). TAG CASE
#  before : started before the supervisor (older than the sweep bound)        -> expected: no effect
#  during : started while the campaign runs, alive until after the end         -> what do the sweeps say? exit code?
#  resume : session 1 crashed (SIGKILL); non-dumpable process started; session 2 resumes -> recovery + final sweeps?
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
tag=$1; case_=$2
R=$S/runs/nd/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
echo "uid=$(id -u) $(grep CapEff /proc/self/status)"
P_CRASH=0 P_MEM=0 CASE_MS='[500,1500]' MAXJOBS=2 make_spec $R/spec.json $R/c $FC 9 8
nd() { env -i $H nodump </dev/null >/dev/null 2>&1; sleep 0.3; for p in /proc/[0-9]*; do tr "\0" " " < $p/cmdline 2>/dev/null | grep -q "hostile nodump" && [ "$(awk '{print $22}' $p/stat)" -gt "${1:-0}" ] && echo ${p#/proc/}; done | tail -1; }
ndinfo() { echo "non-dumpable pid $1: dir owner $(stat -c %u /proc/$1), environ owner $(stat -c %u /proc/$1/environ), environ readable: $(head -c1 /proc/$1/environ >/dev/null 2>&1 && echo yes || echo NO), start $(starttime $1)"; }
case $case_ in
  before) N=$(nd); ndinfo $N; sleep 1
          timeout -s KILL 600 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1; echo "run rc=$?";;
  during) $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1 & SUP=$!
          sleep 1.5; N=$(nd); ndinfo $N
          wait $SUP; echo "run rc=$?";;
  resume) $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & SUP=$!
          for i in $(seq 1 100); do n=$(cat $R/c/ledger/*.jsonl 2>/dev/null | grep -c '"ev":"started"'); [ "${n:-0}" -ge 2 ] && break; sleep 0.1; done
          kill -KILL $SUP; wait $SUP; echo "session 1 crashed (SIGKILL)"; sleep 1
          N=$(nd); ndinfo $N
          timeout -s KILL 600 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1; echo "resume rc=$?";;
esac
grep -h "sweep\|unreadable\|OPERATOR\|FINISHED\|exit 8\|hidden\|unproven\|survivor" $R/run.out | sed 's/^[0-9TZ:-]* //' | cut -c1-230 | sort | uniq -c | head -10 | sed 's/^/  log: /'
grep -h '"text":"[a-z_]*unproven\|marked_survivor\|hidden_descendants' $R/c/ledger/*.jsonl | grep -o '"text":"[^"]\{0,200\}' | head -4 | sed 's/^/  ledger: /'
ID=$(campid $R/c); echo "MARKED_LEFT=$(marked "RR_CAMPAIGN_JOB=$ID:" | wc -w); non-dumpable alive: $(alive $N && echo yes || echo no)"
$RC verify $R/c > $R/verify.out 2>&1; echo "verify rc=$?"
alive $N && kill -KILL $N
