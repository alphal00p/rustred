gate14 (independent audit of procfix7 round 10, ac/procfix7 639c26ce). Notes: WS/notes/implementer.gate14.md.
- tests (crates/rustred-campaign/tests): gate14_adversary.rs (G14-M1: a marked relay below a marker-less child subreaper;
  RR_G14T_XEXIT=s is the F2-isolation scenario), gate14_liveness.rs (honest fork-heavy jobs: jobs/min, proof latency,
  unproven ends). Both #[ignore]; run each binary as pid 1 of its own `unshare -Urpf --mount-proc --kill-child` with
  --ignored --exact <name> --nocapture --test-threads=1 (rule L10); variables in each file's header.
- wave14.sh: namespaced waves of one test (rb_c2 in item 4); suite14.sh: the lane's 30-binary process-layer suite + lib;
  mutate14.py: single-part reverts F1 / F2 of the round-10 fix (uncommitted; restore with git checkout -- hunt.rs).
