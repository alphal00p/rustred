# wv: W0.11 work-volume tools (D6 symmetry merges, D1(b) piece falsifier)

Plan: `docs/research/fable51_next_push_master_plan_2026-09-27.md` §4 (D1, D6) and W0.11.
Results note: `docs/research/fable51_w0_wv_2026-09-27.md`. Everything here is read-only
analysis or input-only walks; no engine change, nothing under `campaigns/` is touched.

## D6: verified literal automorphism group

`family_automorphisms.rs` is a rustred-core example (`[[example]] wv-family-automorphisms`
in `crates/rustred-core/Cargo.toml`). It builds the family from the explicit project TOML
through `rustred::input::Compiler` (the application's path), checks its fingerprint against
the saved-owner selection, enumerates every loop map whose rows are signed line momenta
and that maps all 15 lines to signed lines (integer bookkeeping), and authenticates each
candidate with `sector::symmetry::verify` + `permutation::compile`; `Canonicalizer::try_new`
derives the complete group. Output: the group as `source_for_target` permutations, the
stabilizer of every owner mask, and route-mask orbits.

```
tools/research/wv/build_example.sh      # flock build-4, MemAvailable >= 150 GiB, lane CPUs
target/release/examples/wv-family-automorphisms examples/input/tide_five_loop.toml \
    OWNERS.txt ROUTE_MASKS.txt FINGERPRINT.txt group.json
```

## wv crate (standalone, not a workspace member)

`ckpt.rs geom.rs recs.rs util.rs` are the W0.7 census CP5 decoders
(`tools/research/census`, commit 7d6dd2f1), copied unchanged. New:

- `tight.rs`: exact extrema of a domain's lattice point set (per-axis min/max, max A,
  max R, D range) by integer interval arithmetic, and exact inclusion
  Q <= C  <=>  every defining inequality of C holds at the matching extremum of Q.
  Unit tests compare extrema and inclusion with brute-force enumeration.
- `kd.rs`: static k-d tree over container keys (34 i16 per domain at arity 15) with
  min-corner and min-ID pruning; exact dominance queries with an ID filter
  (earlier ID / earlier native / any other). Unit test against a linear scan.
- `sym.rs`: `wv sym CKPT --group group.json --out OUT.json [--route-sample N]`.
  For every Apply domain Q and every non-identity stabilizer element s: is sQ contained in
  an admitted Apply domain of the same owner with a smaller ID (realized merge), in an
  earlier native, or in any other admitted domain (upper bound)? The identity element gives
  the plain-containment baseline. Counts by domain and weighted by native seconds, per
  owner, by record kind and by domain generation. Route: uniform sample, all family elements
  (Route buckets are masks, so s may move a domain to another mask).

```
tools/research/wv/build_wv.sh test|build      # CARGO_TARGET_DIR=$WT/TMP/wv-target
taskset -c 72-87,328-343 nice -n 19 $WT/TMP/wv-target/release/wv sym TMP/w0/wv/gen7 \
    --group TMP/w0/wv/sym/group.json --out TMP/w0/wv/sym/gen7-sym.json --route-sample 2000000
```

Options: `--route-all 1` evaluates every Route domain (no sampling); `--audit N` checks N sampled
(domain, element) pairs pointwise with the census predicate `geom::contains_point` over
`geom::enumerate` (independent of `tight.rs`) and the k-d tree against a linear scan;
`--flags-out F` writes one byte per ID (bit0 sym_before, bit1 sym_other, bit2 identity_other,
bit3 sym_native_before, bit4 first hit in another Route mask, bit7 evaluated); `--skip-apply 1`.

- `volume.rs`: `wv volume CKPT --flags F --out OUT.json`. Domain-volume view of D6: the
  flagged domains (direct: no ID, no ledger obligation, no inspection, no out-edges) and the
  least-fixpoint cascade over the saved edges (a node is avoided once every in-edge source is
  avoided; protected initial prefix excluded), by class (native, committed delegated,
  native-pending, delegate-pending), native seconds [M] and predicted pending seconds [E]
  (per (phase, owner, half-decade of points) binned mean). TMP/w0/wv/sym/run_full.sh runs both.

## D1(b): helper-piece falsifier (four-loop)

- `make_piece_queries.py BASE VARIANT OUT`: helper-first query variants (pieces replace each
  helper; physics queries verbatim). Variants: base, unbounded (= the frontier fixture),
  shells, shellsk, aslabs, r12aslabs, rank14/16/20 (+k: keep BMW's A<=19 helper bounds).
- `run_variant.py`: runs one historical four-loop command with a substituted queries file
  (binary rustred-4a17f9c7, W6 Ordered), records wall, child CPU, peak RSS and the foreign
  load on the CPU set.
- `pieces_batch.sh LABEL [TIMEOUT]`: all runs on four lane CPU slots, then the Python audit
  (oracle branch), `walk-verify-closure` (rustred-f4d1870f, full F10 re-inspection) and
  `piece_cones.py` per run.
- `piece_cones.py RUN`: per piece level, in-edges, frontier-bearing pieces and physics roots
  reaching them; physics roots certified from the saved edges; roots reaching rank > 12.
- `piece_graph.py --level 12=RUN --level 14=RUN ... --top RUN`: the idealized piece-aware rule
  composed offline from single-helper runs at each rank level and the unbounded (top) run.
- `verify_runs.sh BATCH_DIR CPUS [names]`: re-runs `walk-verify-closure` over a batch (the
  in-batch verify step returned within a second for every b1 run, so b1 was re-verified with it;
  the script sets TMPDIR=/common/dev/rustred/TMP).
- `summarize_pieces.py BATCH_DIR [--json OUT]`: one row per run (natives, frontiers, drain,
  wall, foreign load, physics certification by engine, audit, verifier and cones).

Batch b1 (TMP/w0/wv/pieces/runs/b1) and the compositions (TMP/w0/wv/pieces/compose):
```
tools/research/wv/pieces_batch.sh b1 900
tools/research/wv/verify_runs.sh TMP/w0/wv/pieces/runs/b1 72-75,328-331 ...   # 4 slots
for f in fg h x; do piece_graph.py --level 12=b1/$f-base --level 14=b1/$f-rank14 \
    --level 16=b1/$f-rank16 --level 20=b1/$f-rank20 --top b1/$f-unbounded; done   # bmw: rank*k
```
