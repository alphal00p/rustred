gate8 audit of ac/procfix7 a19d5d0b (notes: WS/notes/implementer.gate8.md). Copies of the scratch harness
(/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate8/{scripts,src}); helper sources reused from gate6 / gate7
are on branch audit/gate7 (audit/gate6/src/hostile.c, audit/gate7/src/g7hostile.c, g7capless.c, g7fault.c).
EVERY *_inner.sh runs ONLY inside a private user+pid+mount namespace and refuses otherwise (in_ns_guard6 + shm_guard):
  ns8r.sh  root-mapped namespace with a PRIVATE tmpfs on /dev/shm (the intent-log fallback never reaches the host);
  ns8u.sh  HOST-LIKE: the same, then a nested user namespace (uid 1000, root unmapped): every process capability-less,
           a non-dumpable process's environ unreadable to the supervisor (as on the host).
unit8.sh runs a batch as the auditor's own transient systemd user unit (unpinned, CPUQuota, MemoryMax, ledger lines with >>,
refuses below 100 GB MemAvailable); waitmem.sh waits for 100 GB first.
Key reproductions:
  G8-B1 (BLOCKER) g8_inj_inner.sh TAG u G8_MATCH=/proc/@DP@/stat G8_OPS=open G8_PERTHREAD=1 G8_FROM=2 G8_EVERY=2 G8_ERRNO=24|5
        (src/g8fault.c LD_PRELOAD: every 2nd open of the crashed session's marked non-dumpable daemon's /proc/PID/stat per
        thread fails = the lineage re-read): exit 0 with the daemon alive. Random model: batch_rand.sh (G8_PROB=500).
        Pure tests: crates/rustred-campaign/tests/gate8_lineage.rs (2 ignored tests fail on a19d5d0b).
  MJ8-1 g8_timens_inner.sh TAG t6 (session 1 in a time namespace with its own /proc/self/ns/time unreadable).
  (a) one-at-a-time sweep: batch_one.sh + one_job.sh (every candidate read n of the recovery / settle / sweep / final-sweep
      file reads, errnos 24 / 5 / 13 / 2, short reads, truncated listings; lineage /proc reads sampled).
  (i) flake_inner.sh (ledger_lock under CPUQuota 30 % + load), rbflake_inner.sh (proc_reaudit_b rb_recover),
      execwin*_inner.sh + src/g8execwin*.c, crates/rustred-campaign/tests/gate8_flake.rs (the exec window).
  (e) g8_e_inner.sh / batch_e.sh (random interruptions, external work_dir, max_jobs 1, unproven-end wait).
  (g) g8_mj1_inner.sh (MJ-1 variants), g8_timens_inner.sh (MJ-2 variants), g8_g7partial_inner.sh (gate7 B1 verbatim).
  (b)(c)(d)(F) g8_b_inner.sh, g8_reg_e7_inner.sh, batch_b2.sh, g8_c_inner.sh, g8_d_inner.sh, g8F_*.sh.
  mutation check: batch_mut.sh + mutate8.py (in the worktree, uncommitted, restored with git checkout after each).
