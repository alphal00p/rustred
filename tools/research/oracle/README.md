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
