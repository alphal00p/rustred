Session scripts of the W0.6 close-out (2026-09-27), copied from `/common/dev/rustred/TMP/w0/inputs/`
for reproducibility; they use absolute paths on the host and write under `TMP/w0/inputs/`.

- `run_i2.sh`: I2 route-witness rewrite and load scan (earlier in the day).
- `run_probes_b.sh`: probe-v3b (plan-v3 same-CPU control) and probe-I1, concurrently, W24 each, 54 min.
- `c5f_ab.sh REPEAT CPUS_ORIG CPUS_I2`: C-5F witness A/B (Ready, as the control), W18 per arm, audit.
- `c5f_ab_policy.sh REPEAT CPUS_ORIG CPUS_I2 [ready|ordered] [PERF_EVENTS]`: same with a publication-policy
  override and optional perf counters (run_four.py --perf-stat).

Results: `docs/research/fable51_w0_inputs_2026-09-27.md`.
