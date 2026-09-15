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
