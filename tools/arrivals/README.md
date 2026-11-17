# tools/arrivals: lane arrivals ops scripts (physical chain arrivals at T162 / T178, cones, drill sample)

These are copies of the scripts the lane ran on host itphlies, from the scratch area
S = /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals (2026-11-15..17). They are kept for provenance and
reruns. The paths inside them point to S, to the p_integral_dev workspace WS = /shared/vhirschi/t3code/vakint_dev/p_integral_dev,
and to the binaries S/bin/rr_arrivals.aNN (built from this branch, `rr_arrivals` in crates/rustred-campaign-algebra).
Report: WS/plan/auto-campaign/reports/PHYSICAL-CHAIN-ARRIVALS.md. Notes: WS/notes/implementer.arrivals.md.

- physchain/: the chain run. Pieces, in the order they run:
  - DAG scheduler: run_dag*.sh, plus run_node*.inc.sh (start gate, units, CPU ledger).
  - Per-node walk: chain_node*.sh.
  - Memory guard: guard*.sh.
  - Top-ups and shards: t165*, merge_topup.py.
  - Fixpoint check: fixpoint_check.py.
  - Deliveries: deliver*.py, merge_bins.py, manifest.py.
  - Chains of single keys: trace_key.py, trace_run.sh.
  - T162 closure list: t162_final*.sh, t162closure_switch.sh. Stopped by decision on 2026-11-17; see the report, 6.2.
  - DRILL sample, "sample, not a fixpoint": run_dag_drill.sh, run_node_drill.inc.sh, chain_node_drill.sh, drill_sample.py,
    drill_chains.py, trace_run_drill.sh.
- physcone/: arrival cones.
  - cones_phys.py: physical box.
  - cones_variant.py: conservative / QED / CELL_TOPS main and margin lines, exact-n4 aware.
  - celltops_watch*.sh: recompute on a CELL_TOPS.tsv change.
- drill/: drill start keys from CELL_TOPS.tsv top cells (drill_keys2.py <S> <per main cell> <per margin cell> <out dir>).
- chain162/: the first T162 chain (wave driver, plan, expect.py, which run_node*.inc.sh still uses for the expected peak).

Earlier versions (chain_node4..9, guard5..8, run_chain8..10, run_dag..run_dag4) stay for the record of which run used which
driver. The newest of each family is the one in use.
