# gate5 harness library (adapted from gate4/procfix5 lib.sh; checked line by line). EVERYTHING that launches/signals
# processes runs inside ns(): unshare -Urpf --mount-proc (private user+pid+mount namespace), private TMPDIR, ulimit -c 0.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7
BASH_BIN=/home/vhirschi/.nix-profile/bin/bash
H=$S/bin/hostile
: "${CPUS:?CPUS must be set by unit.sh}"
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR_G7:-/nonexistent-gate7-xdg}
BUILD=${BUILD:-debug}
RC=$S/bin/$BUILD/rr_campaign
FC=$S/bin/$BUILD/rr_fake_child
ulimit -c 0 2>/dev/null

# make_wrapper OUT MODES... : contract program that, for `job`, spawns hostile descendants (MODES) then execs the fake child
make_wrapper() {
  local out=$1; shift
  cat > "$out" <<W
#!$BASH_BIN
if [ "\$1" = job ]; then
  export HOSTILE_MARK="\$RR_CAMPAIGN_JOB"
  for m in $*; do $H \$m 25 >/dev/null 2>&1 & done
  wait
fi
exec $FC "\$@"
W
  chmod 755 "$out"
}

# make_spec OUT CDIR PROGRAM SEED ITEMS [extra synthetic json] ; env CAMP, MAXJOBS, CASE_MS, P_*, GRACE, EXTRA_RES
make_spec() {
  local out=$1 cdir=$2 prog=$3 seed=$4 items=$5 extra=${6:-}
  cat > "$out" <<J
{"schema":"rustred.campaign.spec.v1","campaign":"${CAMP:-g5}","kind":"test",
 "dirs":{"campaign_dir":"$cdir"},
 "programs":{"rr_fake_child":{"path":"$prog"}},
 "resources":{"policy":"plain","cpus":"$CPUS","max_jobs":${MAXJOBS:-4},"mem_total":"${MEMTOTAL:-4G}","host_floor":"50G","disk_floor":"${DISK_FLOOR:-50M}",
              "grace_seconds":${GRACE:-2},"sampler_ms":200 ${EXTRA_RES:-}},
 "progress":{"cadence_seconds":1},
 "synthetic":{"items":$items,"nodes":2,"rungs":2,"seed":$seed,"case_ms":${CASE_MS:-[300,1500]},"p_norule":${P_NORULE:-0.25},"p_mem":${P_MEM:-0.1},
              "p_crash":${P_CRASH:-0.1},"deps":0.3,"mem_cap_mb":${MEMCAP:-48} $extra}}
J
}

# ns CMD... : private user+pid+mount namespace, pinned to CPUS, private TMPDIR
ns() {
  local t=$S/tmp/ns.$$.$RANDOM; mkdir -p "$t"
  TMPDIR=$t taskset -c "$CPUS" unshare -Urpf --mount-proc --kill-child -- "$@"
  local rc=$?; rm -rf -- "$t"; return $rc
}

# marked PATTERN : pids (inside the ns) whose environ contains PATTERN (pid 1 and self excluded)
marked() {
  local pat=$1 p
  for p in /proc/[0-9]*; do
    p=${p#/proc/}
    [ "$p" = 1 ] && continue
    [ "$p" = "$$" ] && continue
    if { tr "\0" "\n" < /proc/$p/environ; } 2>/dev/null | grep -q -- "$pat"; then echo $p; fi
  done
}
alive() { [ -d /proc/$1 ] && ! grep -q '^State:.*Z' /proc/$1/status 2>/dev/null; }
starttime() { awk '{print $22}' /proc/$1/stat 2>/dev/null; }
campid() { grep -h -o '"campaign:marker_id"[^}]*' $1/ledger/*.jsonl 2>/dev/null | grep -o '"path":"[^"]*"' | head -1 | sed 's/"path":"//;s/"//'; }
# in_ns_guard : refuse to run unless we are pid 1's descendant in a private pid namespace (pid 1 is not systemd/init)
in_ns_guard() { [ "$(cat /proc/1/comm)" != systemd ] && [ "$(cat /proc/1/comm)" != init ] || { echo "NOT IN A PRIVATE PID NAMESPACE: abort" >&2; exit 99; }; }
# gate6: in_ns_guard also refuses an identity uid map (= host userns)
in_ns_guard6() { in_ns_guard; [ "$(tr -s ' ' < /proc/self/uid_map | sed 's/^ //')" != "0 0 4294967295" ] || { echo "HOST USERNS: abort" >&2; exit 99; }; }
# hostlike: are we in the non-root mapped namespace (uid 1000, no caps)?
hostlike() { [ "$(id -u)" = 1000 ]; }
# L18: the ONLY removal helper: one FILE, path must lie under this lane's scratch (no trees, no globs).
rmfile7() { [ -n "$1" ] && [ -f "$1" ] && case "$1" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7/runs/*) rm -- "$1";; esac; }
