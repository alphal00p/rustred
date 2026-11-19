gate7 audit of ac/procfix7 452d0207 (notes: WS/notes/implementer.gate7.md). Copies of the scratch scripts / helper sources
(/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7/{scripts,src}); gate6's scripts they build on are in
audit/gate6/. Every *_inner.sh runs ONLY inside a private user+pid(+mount) namespace (inns.sh: unshare -Urpf --mount-proc;
innsu.sh: host-like map uid 1000, no capabilities) and refuses otherwise (in_ns_guard6). unit.sh runs a batch as the auditor's
own transient systemd user unit (unpinned, CPUQuota, MemoryMax, ledger lines with >>).
Key reproductions:
  B1  g7_partial_inner.sh TAG 2 (+ src/g7fault.c, LD_PRELOAD EMFILE on the 2nd open of ledger/MANIFEST.json = recover()'s
      ignored for_each_record): exit 0 with the crashed session's marked daemon alive; g7_partial2_inner.sh dump 2 (readable marker).
  m-fb g7_fallback_inner.sh (stale tmpfs fallback intent log of another campaign merged); tests/gate7_repro.rs (ignored test).
  m-br g7_barrier_inner.sh (the job program fakes RUN/sup/BARRIER_REFUSED); tests/gate7_repro.rs (ignored test).
  MJ-B g7_mjb_inner.sh noreplace (src/g7seccomp.c) | race | killclaim | killcreate | oldcopy | rmwork.
  F    g7_lindecoy_inner.sh live | resume | pidreuse | nested | otherns (src/g7capless.c: capability-less supervisor).
  (a)  g7_crash_inner.sh SEED MODES (src/g7hostile.c: non-dumpable daemons / chains).  (e) g7_e_inner.sh, batch_e_cl.sh.
  mutation check: mutate.py + mutrun.sh (in a source copy, never the worktree).
