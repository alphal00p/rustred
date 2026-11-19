# m1 (gate6 g): pid reuse between a sweep's STOP and its stop-hold KILL. INSIDE ns() (root map: ns_last_pid). TAG DECOYKIND
#  DECOYKIND: unmarked | othercamp | ourmark_ghost (a decoy at the reused pid carrying THIS campaign's marker: ours by design)
# run under gdb: break final_sweep -> plant P (UNMARKED reaper) whose child X carries THIS campaign's id (ghost label zz1);
# break write_intents (the sweep's STOP batch, after X's environ was read) -> X execs into an UNMARKED sleep; continue
# (SIGSTOP delivered with a hold); break signal_held (the kill round refused X: marker gone) -> SIGKILL X from the harness,
# P reaps it, a decoy is forked AT X's pid (ns_last_pid); continue. Expected: the held kill sees the pidfd's process gone,
# sends nothing; the decoy lives (same start); no intent / signal_sent names the decoy; the run ends.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
tag=$1; kind=$2
GDB=/nix/store/2xrxnqpkwlr4smxwhg5x34s2nbs2rpi6-gdb-17.2/bin/gdb
R=$S/runs/m1/$tag; rm -rf -- "$R"; mkdir -p $R; mkfifo $R/go.fifo
CASE_MS='[200,500]' P_CRASH=0 P_MEM=0 make_spec $R/spec.json $R/c $FC 3 4
cat > $R/plant.sh <<G
#!$BASH_BIN
ID=\$(grep -h -o '"campaign:marker_id"[^}]*' $R/c/ledger/*.jsonl | grep -o '"path":"[^"]*"' | head -1 | sed 's/"path":"//;s/"//')
echo "\$ID" > $R/id.txt
env -i PATH=\$PATH $BASH_BIN -c 'env RR_CAMPAIGN_JOB='\$ID':zz1:a1 sh -c "read x < $R/go.fifo; exec env -u RR_CAMPAIGN_JOB sleep 3600" & X=\$!; echo \$X > $R/x.pid; wait \$X; echo "reaped rc=\$?" > $R/x.reaped; exec sleep 3600' </dev/null >/dev/null 2>&1 &
echo \$! > $R/p.pid
for i in \$(seq 1 200); do [ -s $R/x.pid ] && tr '\0' '\n' < /proc/\$(cat $R/x.pid)/environ 2>/dev/null | grep -q "^RR_CAMPAIGN_JOB=\$ID:zz1" && break; sleep 0.01; done
X=\$(cat $R/x.pid); echo "X \$X \$(awk '{print \$22}' /proc/\$X/stat) P \$(cat $R/p.pid)" > $R/x.txt
echo "planted: \$(cat $R/x.txt) marker: \$(tr '\0' '\n' < /proc/\$X/environ | grep RR_CAMPAIGN_JOB)" >&2
G
cat > $R/drop.sh <<G
#!$BASH_BIN
X=\$(cat $R/x.pid); echo go > $R/go.fifo
for i in \$(seq 1 300); do c=\$(cat /proc/\$X/comm 2>/dev/null); [ "\$c" = sleep ] && ! tr '\0' '\n' < /proc/\$X/environ | grep -q RR_CAMPAIGN_JOB && break; sleep 0.01; done
echo "X \$X dropped its marker: comm=\$(cat /proc/\$X/comm) state=\$(awk '/^State:/{print \$2}' /proc/\$X/status) marked=\$(tr '\0' '\n' < /proc/\$X/environ | grep -c RR_CAMPAIGN_JOB)" >&2
G
cat > $R/reuse.sh <<G
#!$BASH_BIN
X=\$(cat $R/x.pid); ID=\$(cat $R/id.txt)
echo "at signal_held: X \$X state=\$(awk '/^State:/{print \$2}' /proc/\$X/status) (stopped by the sweep)" >&2
kill -KILL \$X
for i in \$(seq 1 300); do [ -s $R/x.reaped ] && break; sleep 0.01; done
echo "X killed by the harness and reaped by P: \$(cat $R/x.reaped 2>/dev/null); /proc/\$X exists: \$([ -d /proc/\$X ] && echo yes || echo no)" >&2
case $kind in
  unmarked) D=\$($H pidfill \$X $H sleep) ;;
  othercamp) D=\$(env RR_CAMPAIGN_JOB=other.0123456789abcdef:zz1:a1 $H pidfill \$X $H sleep) ;;
  ourmark_ghost) D=\$(env RR_CAMPAIGN_JOB=\$ID:zz2:a1 $H pidfill \$X $H sleep) ;;
esac
echo "decoy at pid \$D start \$(awk '{print \$22}' /proc/\$D/stat 2>/dev/null) (X was \$X)" >&2
echo "\$D \$(awk '{print \$22}' /proc/\$D/stat)" > $R/decoy.txt
G
chmod +x $R/*.sh
$GDB -q -batch -nx -ex 'set pagination off' -ex 'set confirm off' -ex 'set startup-with-shell off' -ex 'set follow-fork-mode parent' \
  -ex 'set detach-on-fork on' -ex 'handle SIGPIPE nostop noprint pass' -ex 'handle SIGCHLD nostop noprint pass' \
  -ex 'break rustred_campaign::supervisor::Supervisor::final_sweep' -ex run -ex "shell $R/plant.sh" \
  -ex 'break rustred_campaign::launch::guard::write_intents' -ex continue -ex "shell $R/drop.sh" -ex 'delete 2' \
  -ex 'break rustred_campaign::launch::guard::signal_held' -ex continue -ex "shell $R/reuse.sh" -ex 'delete 3' \
  -ex continue -ex 'info breakpoints' \
  --args $RC run --spec $R/spec.json --dir $R/c --plain > $R/gdb.out 2>&1
echo "gdb: final_sweep hits $(grep -c 'Breakpoint 1,' $R/gdb.out), write_intents hits $(grep -c 'Breakpoint 2,' $R/gdb.out), signal_held hits $(grep -c 'Breakpoint 3,' $R/gdb.out); exit: $(grep -o 'exited with code [0-9]*\|exited normally' $R/gdb.out | tail -1)"
grep -h 'planted\|dropped its marker\|at signal_held\|killed by the harness\|decoy at pid' $R/gdb.out | sed 's/^/  /'
read D DST < $R/decoy.txt; read _ X XST _ P < $R/x.txt
echo "decoy $D alive with the same start: $(alive $D && [ "$(starttime $D)" = "$DST" ] && echo YES || echo NO)"
echo "intents naming pid $D: $(grep -c "\"pid\":$D," $R/c/log/signal-intents.jsonl); signal_sent naming pid $D: $(grep -h '"signal_sent"' $R/c/ledger/*.jsonl | grep -c "\"pid\":$D,")"
echo "intents for X ($X $XST): $(grep "\"pid\":$X," $R/c/log/signal-intents.jsonl | grep -o '"signal":[0-9]*,\|"proof":"[^"]*"' | tr '\n' ' ')"
grep -h "killed_by_stop_hold\|continued_after_stop\|STOPPED by the sweep\|marked_survivor\|final_sweep_unproven" $R/c/ledger/*.jsonl | cut -c1-300 | sed 's/^/  ledger: /'
grep -h "FINISHED\|exit\|ERROR" $R/gdb.out | grep -v "^\[" | tail -4 | cut -c1-250 | sed 's/^/  out: /'
$RC verify $R/c > $R/verify.out 2>&1; echo "verify rc=$?"
