gate11 audit of ac/procfix7 fdec15da (notes: WS/notes/implementer.gate11.md). Copies of the scratch harness
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11/{scripts,src}. gate10's harness (audit/gate10
scripts/gate10, and its scratch scripts) is reused with paths gate11; lib.sh's make_spec now writes
resources.containment = "marker_only" (required since round 6 inside a private namespace; CONTAIN="" omits it).
EVERY *_inner.sh runs ONLY inside a private user+pid+mount namespace (ns8r.sh root-mapped / ns8u.sh host-like) and refuses
otherwise; unit11.sh runs a batch as the auditor's own transient systemd user unit (refuses unless MemAvailable - MemoryMax
>= 75 GB and fewer than 2 gate11 units run; EXTRA_PROPS adds unit properties).

  src/g11filter.c   LD_PRELOAD LISTING FILTER: rewrites the /proc root listing (G11_TASKS=1: task listings too) of BOTH
                    enumerations of the supervisor (glibc readdir64 after opendir; raw syscall(SYS_getdents64)) in agreement,
                    without an error: G11_MODE drop (pids from G11_DROP_FILE / G11_DROP) | droprange | dup | reorder | dropk
                    (drop-one at every listing position k) | keepk (gate10's truncation). Only in the process whose argv[1]
                    is G11_ARGV1 (default "run").
  src/g11thr.c      an UNMARKED setsid'd parent whose NON-LEADER thread forks a MARKED sleep (mode thread), or an unmarked
                    SUBREAPER whose leader exits so the orphaned marked sleep lands on its live non-leader thread (zleader):
                    a process the completeness walk (leader children lists only) cannot reach.
  src/g11zchain.c   an unmarked fast fork chain without setsid (zombie storm for the reaper; supervisor as pid 1).
  g11_drop_inner.sh / g11_droplive_inner.sh + cells_drop.txt (+ cells_drop_c.txt, the re-run with unproven_end_limit_s 60)
                    (a)(1): 186 cells; MAJOR G11-M1 = every crashed-session cell that drops the walk-blind marked process
                    ends exit 0 with it alive (10/10). Test: crates/rustred-campaign/tests/gate11_walk.rs (2 ignored tests
                    FAIL on fdec15da; the control passes).
  cells_CH.txt      new fault class for gate10's matrix10.sh / g10_inj_inner.sh: the walk's children-list reads, the
                    boottime clock (batch_M.sh runs it, then gate10's 716 cells).
  g11_helpers_inner.sh  (a)(2): healthy campaigns with short-lived marked helpers (run / term / kill modes) vs reference.
  g11_pid1.sh       (h): the supervisor as pid 1 of its namespace vs as a child, zombie storm; zombies sampled from outside.
  g11_host17_inner.sh  (H): RUNBOOK 1.7's spec at 1/10 scale with PRIVATE copies of lock_dir and ledger_file.
  g11_H_inner.sh    (H): the start refusal without a scope; host-tests/letter_a_scope_containment.sh's namespace self-test as
                    shipped and with letter_a_selftest_containment.patch (its self-test spec lacks "containment").
  g11_deleg_inner.sh   (a)(4) emulation in a Delegate=yes unit (per-job tracking leaves: cgroup.kill + populated).
  mutate11.py / mutrun11.sh  ten mutations of round 6's checks in a scratch source copy (never the worktree).
  crates/rustred-campaign/tests/gate11_cost.rs  (i): cost of pids / pids_proven / strict scan / hidden_descendants at N
                    processes (the tree re-parented to pid 1's leader so that the walk reaches it).
