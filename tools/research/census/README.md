# census: work-volume census of a walk checkpoint (plan W0.7 / W0.11)

Read-only analysis of a RUSTRED-WALK-CP5 checkpoint directory (or of a
drained run's pretty `result.json`). Nothing is written into the inputs.
Plan: `docs/research/fable51_next_push_master_plan_2026-09-27.md` §5 W0.7.

Standalone crate (not a workspace member). Build:

```
cd tools/research/census
flock -w 14400 /common/dev/rustred/TMP/locks/build-1.lock \
  env CARGO_TARGET_DIR=$PWD/../../../TMP/census-target \
  nix develop ../../.. --command cargo build --release --locked --offline
cargo test --release --locked --offline   # brute-force checks of the geometry
```

## Model

A domain of arity n is a box in local coordinates x_i >= 0 with aggregate
predicates (`DomainPowerBounds`): active coordinates carry a_i = x_i + 1,
inactive ones r_i = x_i; A = t + sum_active x, R = sum_inactive x, D = A - R,
P = A + R; A <= max_positive_power, R <= rank, D in [min, max] difference.
Points are counted exactly by a level convolution (`geom.rs`).

Coverage of a query domain Q by a set of anchors of the same (phase, owner)
is decided point by point on Q: every lattice point of Q is tested (or, when
|Q| exceeds `--cap`, `--samples` uniform points drawn by an exact sampler)
against the anchors whose box and aggregate ranges meet Q. Anchors are never
enumerated. Brute-force unit tests check counting, enumeration, sampling and
coverage.

Residual vocabularies (a residual must contain every uncovered point):

| name | pieces | needs |
|---|---|---|
| `d_only` | Q cut to maximal D bands holding uncovered points | today's vocabulary (the `initial_overlap` D-band mechanism) |
| `ar_c2` | Q cut to (A, R) rectangles holding uncovered levels | lower bounds on A and R (plan C2) |
| `hull` | Q intersected with the box hull of the uncovered points, A <= Amax_unc, R <= Rmax_unc, D band | today's vocabulary, 1 piece |
| `hull_c2` | `hull` plus A >= Amin_unc, R >= Rmin_unc | C2, 1 piece |
| `hull_per_d_run` | one hull per D band | today's vocabulary |
| `exact_pointwise` | the uncovered points themselves (lower bound, not a vocabulary) | - |

Anchor sets (`cover`):

- `natives_before_dispatch`: Apply natives whose record was committed before
  the query job's dispatch, taken as (commit time - native seconds - `--wait`)
  on the heartbeat series `--series` ("elapsed committed_records" lines from
  the run's `events.jsonl`); without a series, seq - margin-base - margin-rate *
  seconds. This is the G2' anchor rule (merged Native anchors before dispatch).
- `natives_before_commit`: natives committed before the query's own record.
- `all_earlier_ids`: every Apply domain with a smaller ID (natives, aliases,
  pending): the general union-cover bound (decision D2), comparable to the
  lens tool `rtool cover`.
- pending queries: `all_natives` (every native at the checkpoint: G2' at
  resume) and `all_other_domains`.

Samples: `hist_pps_seconds` draws Apply natives with probability proportional
to their measured seconds, so a share of draws estimates a share of Apply
CPU; `hist_uniform` draws natives uniformly; `pending_pps_predicted` draws
native-pending Apply domains proportional to a predicted cost [E] from the
per-owner binned cost law (half-decade means of native seconds against
points); `pending_uniform` draws them uniformly.

Gate 0.7: share of Apply CPU whose residual is <= 10% of |Q| in <= 8 pieces.
Projected relative cost [E]: sum over residual pieces of the predicted cost of
a piece over the predicted cost of Q (per-owner OLS power law, and the binned
law), capped at 1; zero for a fully covered Q.

## Subcommands

```
census stats      CKPT                       # sections, ledger states, record kinds, points by decade
census cost       CKPT                       # per-owner cost exponents (OLS, by generation, by decade)
census compose    CKPT                       # native-pending composition; pending vs committed envelope (A,R,P,D)
census potential  CKPT                       # global-potential check on (phase, owner) edge graphs
census cover      CKPT --k N --rows R.jsonl [--series S --wait W] [--hot MASK] [--cap C --samples M]
census saturation CKPT [--draws N]           # union of natives / admitted domains per owner and generation
census pilot      CKPT --pilot RESULT.json --k N [--hot MASK]   # overlap of a drained pilot's closure
```

`CKPT` may also be a drained run's `result.json` (arity via `--arity`).
`run_all.sh CKPT OUT_PREFIX [cover options]` runs every subcommand.

## W0.7 receipt

```
w0_batch.sh CENSUS_BIN OUT [RUSTRED_BIN]      # every census step + the Q3 factor census (owner-domain-scan)
W0_ONLY=q3 w0_batch.sh - OUT RUSTRED_BIN      # only Q3 (rustred needs every inner pool = 1; the script sets them)
w0_tables.py OUT                              # the tables of docs/research/fable51_w0_census_2026-09-27.md
summarize.py cover|owners|cost|pilot|potential|saturation ...   # single-file tables
q3_summary.py OUT/q3/v2-factor-census-numerators.json OUT/gen7/cost.json
```

`census cost` reports two per-owner exponents: the per-native OLS of ln(seconds)
on ln(points) (geometric-mean law) and the plan's estimator (n-weighted LS of
ln(mean seconds) on ln(mean points) over decade bins with >= 10 natives,
`cost_exponent_binmean_wls`). Unevaluated queries (infinite, or not
enumerable) count as uncovered, gate failed, full residual and full cost.

## Route-side census (routecensus lane, 2026-09-28)

```
census route            CKPT [--series S --wait W] [--per N] [--kc N] [--per-adm N] [--max-up N] [--rows R.jsonl]
census route-saturation CKPT [--draws N] [--seed S]
census route-hits       CKPT --rows-in ROUTE_ROWS.jsonl [--rows OUT.jsonl] [--pps-adm K --pps-rows P.jsonl]   # later hits + predictor per row; K admitted domains per phase drawn PPS by later hits
routecensus_batch.sh CENSUS_BIN OUT [gen7 gen7-sat gen7-sat-seed2 gen7-w300 gen6 gen3 c5f]
route_tables.py OUT                 # tables of docs/research/fable51_w0_routecensus_2026-09-28.md
route_boot.py OUT/gen7-route-rows.jsonl [REPS]   # stratified bootstrap SEs, sampled-verdict shares, per-owner view
route_hits.py OUT [OUT/hits] [REPS]              # later hits of avoided domains, net RSS range, calibrated pending creation weights
```

`census route` (see the header of `src/route.rs`) measures on a CP5 checkpoint:
exact edge fan-out of Route and Apply natives and the creator forest (first
incoming edge of every domain); the coverage of Route native-pending
(stratified by admission generation x points decade) by `all_natives`,
`earlier_non_delegated` (smaller IDs that are natives or still pending: a
well-founded union), `all_earlier_ids` and `all_other_domains`, weighted by
count and by per-(owner, rank) predicted seconds, successors, out-edges and
created domains [E]; the coverage of historical Route natives (stratified by
record generation x decade) by `natives_before_dispatch` (G2' rule),
`natives_before_commit`, `earlier_non_delegated` and `all_earlier_ids`,
weighted by measured seconds, successors, edges, created domains and
creator-forest descendants; the creator of sampled pending Apply/Route
domains and whether that Route-native creator was itself covered; and every
admitted domain at its admission (covered by the union of the domains that
existed then, by the non-delegated ones, or by the natives committed before
its creator's record), with an ancestor cascade [E upper]. Fully covered
draws also report the anchors a union cover used (greedy first-hit count).

`census route-hits` (fix round) adds, per row of a `route` rows file, the
later hits of the drawn domain (distinct transition in-edges from inspected
sources and alias in-edges, creator edge and self edges excluded: the
requests that an admission-time union cover would redirect, since the domain
would not exist) and the pending-weight predictor at its (owner, rank bound),
in-sample and leave-one-out; it also prints exact later-hit totals per
(phase, admission generation, class). `route_hits.py` joins both files: net
RSS factor of an admission-time union cover for k' = 1 and k' = a anchors per
redirected hit, the admission-request count, and the pending creation
weights calibrated by coverage status on the historical g6-g7 natives.
