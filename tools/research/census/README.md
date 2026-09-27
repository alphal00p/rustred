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
