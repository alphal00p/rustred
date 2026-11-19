gate13c audit scripts (audit/gate13c on ac/procfix7 362e932f; notes WS/notes/implementer.gate13c.md)
- rbc2_wave.sh TAG WAVES label=BIN...: rb_c2 (proc_reaudit_regressions) reproduction; each test binary is pid 1 of its own
  private user+pid+mount namespace (rule L10), private TMPDIR; started by the caller as a detached service unit. TRACE=1:
  RR_HUNT_TRACE=1 (debug builds print the end-of-job hunt's rounds and passes).
- trig.sh TAG BIN RUNS: gate13_trigger runs, each in its own private namespace.
- reh13c.sh TAG ITEMS CASE_MS: the FROZEN 20261117T1943Z host-step script in --rehearse-ns inside a private namespace.
- mutation M-a (guard.rs PinnedSession check disabled; uncommitted, restored): see the notes' records.
