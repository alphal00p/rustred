# W0.2 oracle calibration drivers

Drivers used for the W0.2 gate (plan `docs/research/fable51_next_push_master_plan_2026-09-27.md` §5 W0.2);
results in `TMP/w0/oracle/RESULTS.md`. Paths are this host's.

- `run_controls.py`: `TMP/fable51-controls/run_control.py` with outputs under `TMP/w0/oracle/runs`;
  `--queries` swaps the query document (the unrestricted-helper FG frontier fixture).
- `audit_all.sh`: `examples/python/audit_owner_domain_walk.py --require-closure` over every calibration output
  and the drained hot-owner pilot (C-HOT).
- `verify_all.sh BIN [THREADS] [cases...]`: `rustred walk-verify-closure` (full reference re-inspection) over
  the calibration outputs.

The mutation matrix is `examples/python/oracle_mutation_matrix.py`.

## Round 2 (`r2/`, results in `TMP/w0/oracle/RESULTS.md` §R2 and `docs/research/fable51_w0_oracle_2026-09-27.md`)

- `r2/verify_all.sh BIN [THREADS] [cases...]`: `walk-verify-closure --require-closure` (full F10, native levers
  off, published result.json bound) over the 11 calibration outputs, with `/usr/bin/time -v`.
- `r2/audit_all.sh`: extended audit `--require-closure --verify-report` (paired to the round-2 verifier report)
  over the same outputs, plus C-HOT audit-only.
- `r2/matrices.sh BIN [fg|c5f|all]`: `oracle_mutation_matrix.py` v2 on FG (both oracles) and C-5F (Rust rows;
  frontier rows on the FG fixture), frontier fixture pinned at 60/124.
- `r2/gen7_sample.sh BIN [SAMPLE] [THREADS]`: sample-mode verification of a block clone of the v2 gen-7 checkpoint,
  with a MemAvailable guard (kills the verifier below 120 GiB) and a 55-min cap.
- `r2/summarize.py`: the calibration table from the verifier and audit reports.
- Gate on any report: `examples/python/assert_oracle_pass.py REPORT.json...`.
