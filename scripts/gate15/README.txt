gate15 audit scripts (independent audit of procfix7 round 11, a422bfa9). Every process-layer run is namespaced (each test
binary pid 1 of its own unshare -Urpf --mount-proc --kill-child; private TMPDIR after WS/ops/env.sh), started by the
caller as its own systemd --user service unit (CPUQuota, nice 10, CPU_LEDGER lines with >>).
wave15.sh       TAG WAVES TEST 'label|binary|VAR=val,...'...: parallel namespaced test waves (adversary, liveness)
tally15.py      tally of gate14_adversary runs per label (violations / verified clean / unverified then cleared)
live15.py       table of gate14_liveness runs (jobs/min, p90, max, unverified at 10 s, cleared, never)
proof_latency.py  per-job proof latency of a campaign directory (ledger 'finished' minus the job's own last event)
reh15.sh        host_step_fake1.sh --rehearse-ns in a private namespace (copy of the lane's reh_r9b.sh; HELPERS via env)
suite15.sh      the lane's 31-binary process-layer suite + lib filters (LIGHT_PIDNS=1)
item6.sh        procfix7 mj1 + procfix9 g9_b2_fork_churner_live_session with --nocapture
trig15.sh       gate13_trigger N times
