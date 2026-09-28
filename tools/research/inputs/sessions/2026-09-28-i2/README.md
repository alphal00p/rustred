# Session scripts, lane i2 (W1 I2 rebuild: one coordinate frame per owner), 2026-09-28

Copies of the scripts run from `/common/dev/rustred/TMP/w1/i2/` (run directories stay in TMP).
- `build.sh`: route_witness_rewrite example build (build-2.lock, MemAvailable >= 150 GiB, CPUs 88-127,344-383).
- `run_rewrite.sh`: `--frame route` reproduction of the W0.6 selection (must be sha256 d9760837...) and the
  `--frame owner` rebuild -> `selection-i2b.json`.
- `c5f_ab.sh REPEAT CPUS_ORIG CPUS_I2B POLICY [PERF]`: C-5F A/B, W18 per arm, concurrent on disjoint 18-core sets,
  record audit + cp5hop census per arm. Run as: ord1 88-105/106-123 ordered; ord2 106-123/88-105 ordered
  instructions:u,cycles:u; r1 88-105/106-123 ready; r2 106-123/88-105 ready.
- `verify.sh CPUS THREADS LABEL=DIR...`: merged oracle `walk-verify-closure --require-closure` (full re-inspection,
  binary TMP/fable51-controls/bin/rustred-8da58390), paired record audit, assert_oracle_pass.py.
- `mech_run_one.sh CPUS`: mechanism diagnostic (not a gate arm): only owner 000011001001011 rotated.
- `run_probes.sh`: the 30-min plan-v3 five-loop probe pair; NOT run (the C-5F gate failed, so the probe clause was
  not reached).
The selection for the mechanism run was made by copying the i2b `source_to_representative` of the 44 routes into
000011001001011 onto the original selection (JSON field copy, no arithmetic).
