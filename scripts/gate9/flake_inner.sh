# gate8 (i): N runs of ledger_lock::crashed_supervisor_with_live_child_is_taken_over_and_recorded, each in its OWN
# private pid namespace (the test binary is pid 1 there), while LOAD loops of other suite binaries run (each in its own ns).
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; T=$S/bin/tests; N=${1:-200}
echo "cpu.max: $(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max 2>/dev/null)"
nsrun() { local t=$S/tmp/fl.$$.$RANDOM; mkdir -p $t; TMPDIR=$t unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=32m tmpfs /dev/shm && exec "$0" "$@"' "$@"; local rc=$?; [ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/tmp/?*) rm -r -- "$t";; esac; return $rc; }
mkdir -p $S/tmp
STOP=$S/tmp/flake.stop.$$; [ -n "$STOP" ] && case "$STOP" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/tmp/flake.stop.*) rm -f -- "$STOP";; esac
load() { while [ ! -e $STOP ]; do nsrun $T/$1 --test-threads=4 > /dev/null 2>&1; done; }
for b in ledger_basic-d115d5a8b89a4a0c ledger_crash-0f4b9e9bd088cc29 ledger_rotation-031483a0ad49e229 sup_ops-640c5a9cbf9cc12a procfix5-b789db82a19c92b8; do load $b & done
pass=0; fail=0
for i in $(seq 1 $N); do
  out=$(nsrun $T/ledger_lock-3bcb60cfdd767ab2 --exact crashed_supervisor_with_live_child_is_taken_over_and_recorded --test-threads=1 2>&1)
  if echo "$out" | grep -q 'test result: ok. 1 passed'; then pass=$((pass+1)); else fail=$((fail+1)); echo "RUN $i FAILED:"; echo "$out" | grep -E 'panicked|left:|right:|carries|expected' | head -6; fi
done
touch $STOP; wait; [ -n "$STOP" ] && case "$STOP" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/tmp/flake.stop.*) rm -f -- "$STOP";; esac
echo "FLAKE RESULT runs=$N pass=$pass fail=$fail"
