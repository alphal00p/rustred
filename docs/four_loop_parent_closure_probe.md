# Four-loop external-parent closure probe

This is a bounded runtime probe of the three external four-loop unit-mass
parents that are not the H input. It deliberately uses the unchanged generic
`family-close` CLI, with no topology dispatch, FORM rules, ordering hints, or
sector selection.

## Protocol

- RustRed release executable frozen from revision `a435599`:
  `fba965172949c7c94caa6f68954310b7ed2d3ccae279cdbc2d73c85797c59260`.
- Inputs: `examples/input/four_loop_x.toml`,
  `examples/input/four_loop_bmw.toml`, and `examples/input/four_loop_fg.toml`.
- Natural coordinate ordering; no `--permutation`.
- Two RustRed workers pinned to physical CPUs 32 and 33.
- Fresh process and fresh output path per parent.
- Five-minute wall-time censor per process; `RAYON_NUM_THREADS=2` and all
  nested BLAS/OpenMP pools capped at one.
- A durable artifact had to be written before any cold inspection or physical /
  dotted reduction canary could run. None of these probes reached that stage.

The Python driver and complete stdout/stderr/measurement records are retained
outside the repository at:
`/tmp/rustred-four-loop-parent-probes.u9dfmK/`.

## Results

| parent | wall time | CPU time | peak RSS | exit/status | artifact | diagnostic |
|---|---:|---:|---:|---|---|---|
| X | 300.117876 s | 596.531212 s | 1,210,032 KiB | `-9`, timeout-censored | none | no sector diagnostic before censor |
| BMW | 300.082759 s | 596.357321 s | 845,304 KiB | `-9`, timeout-censored | none | no sector diagnostic before censor |
| FG | 233.533606 s | 462.959997 s | 1,087,096 KiB | `8`, typed exact-case failure | none | sector 397 retains unsupported nonlinear equalities |

The X and BMW processes were killed only after the five-minute bound. Their
absence of an artifact is therefore an incomplete run, not a mathematical
claim that their families cannot close. FG completed its bounded search far
enough to return a typed unsupported-case error, but also did not produce a
publishable artifact.

## Current-head FG rerun

After the exact disjunctive nonlinear-case engine landed, FG was rerun from the
current release binary rather than the historical `a435599` executable. The
protocol was otherwise unchanged: fresh process, two pinned workers on CPUs 32
and 33, nested pools capped at one, and a 300-second wall bound. It ran for
`235.31 s` wall (`465.39 s` user CPU, `1,092,112 KiB` peak RSS), then returned
status `8` at sector `397` with:
`incomplete exact case intersection: a branch retains unsupported nonlinear
equalities`. No artifact was written. Thus the newer engine removes neither
the need for an authenticated affine/nonlinear coverage partition nor the
fail-closed publication gate; it only advances the point at which this
particular parent is diagnosed.

## Current-head H rerun

The current release binary was also run on the generic external H family with
the same two-worker, CPU-pinned, fresh-process protocol. It reached the source
port publication gate in `101.66 s` wall (`199.58 s` user CPU, `660,572 KiB`
peak RSS), with `60/63` rules replayed and descending. The run was rejected
fail-closed because seven coordinate boxes remained uncovered and three affine
candidate rules could not be omitted by the rectangular cover. The repeated
coupled equality on the affected face is
`-1 - n4 + 2*n2 = 0` (with the corresponding fixed coordinates and sector
signs included in the diagnostic). No artifact was written. This is a useful
four-loop milestone: H now reaches the exact affine-ownership boundary rather
than failing in source search, but persistence and authenticated coverage of
that affine locus are still required before publication.

## Interpretation

None of the three external parents currently has a cold-loadable RustRed
artifact. In particular, no physical or dotted reduction canary is reported:
the CLI publishes atomically only after complete sector coverage and exact
installation, and all three runs stopped before publication. The observations
do show that the current generic lane encounters materially different resource
profiles: X and BMW consume the full five-minute budget, while FG reaches an
unsupported nonlinear exceptional branch in about 234 seconds.

These runs do not authorize hard-coded relations, topology-specific dispatch,
sampled coverage, or a claim of four-loop closure. The next implementation
requirement is the exact affine/nonlinear exceptional-domain carrier and its
artifact ownership proof; until that exists, publication must remain
fail-closed.
