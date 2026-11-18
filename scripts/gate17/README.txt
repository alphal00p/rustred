gate17 review scripts (delta review of ac/procfix7 round 15 at e07c7d88 vs 041e7fe8 + 6c9501da). Scratch root:
/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate17 (logs/, bin/, rt/, host-rehearsal/, frozen/, replay/).
Every script that spawns or signals runs each binary as pid 1 of its own `unshare -Urpf --mount-proc --kill-child`.
  suite17.sh / suite17b.sh  item 6: the lane's 32 process-layer binaries + lib (part 1 / part 2 after a wrapper time limit)
  rep17.sh / rep17_one.sh   item 3: 100 runs each: rb_reuse before (041e7fe8) / after; proc_reaudit_regressions, proc_audit_regressions
  bc4_one.sh / bc4old_one.sh item 3: b_c4_orphans_daemons_and_escapees under contention, e07c7d88 vs 041e7fe8 (test race)
  reh17.sh                  items 1 + 5: copy of gate16 reh16.sh (= procfix7 reh_r9b.sh), paths only
  reh17_pair.sh             items 1 + 5: FAKE_TRIPLE_STOP with the e07c7d88 and the 041e7fe8 release binaries
  reh17_pair2.sh            item 5: manager stall within the limit; stall beyond the limit with jobs running (drain)
  trig17.sh                 item 4: gate13_trigger N times (clean and the M-a mutant)
  freeze17.sh               item 7: scripts/procfix7/freeze_host_step.sh with paths only changed (WT, lane, tmp), FROZEN_ROOT=scratch
  units_lock17.sh           item 3: `units clear` after a supervisor crash hides the stale-lock takeover from the next start
  leader_exit_probe.py      item 3: kernel facts: a live process whose leader thread ended reads exe ENOENT, cgroup ok, no waitid
Review test: crates/rustred-campaign/tests/gate17_review.rs (namespace only; the same facts through the product's ProcRoot).
