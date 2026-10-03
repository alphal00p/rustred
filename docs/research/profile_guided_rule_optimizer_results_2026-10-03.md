# Campaign-guided rule optimization: first completed downstream results

October 3, 2026. This report concerns isolated development experiments, not a
change to either running production campaign. The active objective remains
[the profile-guided optimization plan](../../PROFILE_GUIDED_RULE_OPTIMIZATION_PLAN.md).

## Mechanism, not an extra master or a smaller request

Campaign profiling identified repeated work associated with owner
`101010000110001` in the five-loop input family. In its current routing, one loop
momentum appears in only one active quadratic denominator; numerator scalar
products can still couple that loop to the remaining loops.

The candidate combines eight translated ordinary IBP sources corresponding to
a vector field tangent to that isolated quadratic denominator. Schematically,
for a momentum `k` and a `k`-independent vector `p`,

```text
V = k² p − (k·p) k,       V·k = 0.
```

This avoids a particular class of denominator-power transfers while lowering
the numerator. It is a new choice of recurrence derived from the same ordinary
IBPs, not a topology-specific identity inserted into the engine. Full source
translations include all product-rule terms. The present chart allows arbitrary
active powers, a negative D14 power, and five independent spectator numerator
powers; the other three cross-numerator powers are fixed zero. Mixed-cross
extension is separate work, not silently covered by this result.

The generic research producer uses Symbolica and existing RustRed services to
regenerate the complete symbolic sum, derive its normalization, prove all18
sign cells, retain poles/guards and prove descent under the unchanged saved
integral order. Checked export adds one whole-piece priority rule and preserves
all233 old rules and25 terminals. Cases outside its scope use the old rules.
The broad RHS has nine terms: immediate RHS length is not the optimization
objective.

## Completed C19 comparison

C19 is one correlated training domain containing19 integer tuples. It comes
from an actual matched campaign piece. Both arms retain all67 saved owners,
8246 ordered routes, two frontier repairs and identical query bytes. Only the
candidate owner's payload changes. This is not the full116-required/67-helper
starting request and not an independent heldout.

The frozen optimized CLI is `8ef80b52…`, using16 workers on CPU IDs32–47,
outside production reservations64–127. Each arm stages fresh inputs, walks a
fresh graph, and cold-reinspects every native result with reference levers
Off. Inclusive pilot budget is30minutes; both pairs completed in about6minutes.
Compilation and one-time candidate generation are separate. No OS cache flush
is performed; the second pair reverses arm order.

| Pair / arm | Scheduled domains | Native inspections | Events | Traversal (s) | Full arm (s) | Waited CPU (s) | Peak tree RSS (GB) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 baseline first | 99,610 | 96,762 | 1,858,298 | 7.398 | 186.242 | 1,695.510 | 5.243 |
| 1 candidate second | 143 | 143 | 6,721 | 0.210 | 174.224 | 1,641.632 | 5.245 |
| 2 candidate first | 143 | 143 | 6,721 | 0.210 | 172.574 | 1,618.109 | 5.225 |
| 2 baseline second | 99,667 | 96,802 | 1,858,696 | 7.573 | 187.883 | 1,693.125 | 5.286 |

All four arms have zero pending work, frontiers and abandoned obligations and
pass independent native cold-All coverage checks. Independent audits of both
pairs also checked actual input/command bindings and unchanged pool/route
inventories; details are recorded separately in the audit log.

The reproducible **local** result is99.856–99.857% less scheduled domain work
and about97% less traversal time. Full-arm time falls only6.45–8.15%, because
loading/checking the unchanged complete owner pool dominates this tiny request.
Peak RSS is essentially unchanged for the same reason. CPU includes waited
children and inner supervisors, not the outer harness. Events are not newly
admitted nodes. Small baseline count variation also occurred in the identical
payload A/A control and must not be mistaken for optimization.

## Broader spectator and pinch-route control

A prospective follow-up uses12 whole sampled starting domains (19 tuples):
all seven geometrically eligible stable-stratum Apply representatives, plus
five minimum-hash pinch-route controls. Original coordinate and correlated
bounds are preserved, and no zero-benefit control may be dropped. This is
candidate-scope-informed training validation, not a pristine heldout.

| First pair | Domains | Native inspections | Events | Traversal (s) | Full arm (s) | Waited CPU (s) | Peak tree RSS (GB) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Baseline | 3,565 | 3,489 | 55,351 | 0.378 | 177.483 | 1,667.622 | 5.266 |
| Candidate | 1,529 | 1,504 | 25,965 | 0.271 | 171.872 | 1,649.911 | 5.299 |

Both arms cold-pass all12 required roots, with no pending work, frontier or
abandoned obligation. The shared-cohort domain reduction is57.11%; full-arm
time is3.16% lower. Reverse-order repetition remains pending. Evidence:
`candidates/prescribed-spectator-followup12-v1/`.

## What is established, and what is not

- Exact rule validity, common-order descent and fallback are separate from the
  downstream performance result; neither substitutes for the other.
- The finite regional graph is discharged by the existing cold verifier.
  Regional abstraction/reuse can have sealed cycles even when concrete
  transitions strictly descend. Cold coverage alone is not a general
  termination theorem or unrestricted family closure certificate.
- This result does not estimate the fraction of production work saved. C19 is
  a selected training face; other cross numerators and other owners remain.
- The12-root cohort broadens spectators, active powers and pinch-route controls
  but remains profile-informed training validation. A separately frozen331-tuple
  restricted owner-family heldout is running: all original bounds are preserved
  except explicitly limiting its first root to A≤7/R≤2; the other root is kept
  whole. Neither starting owner is the replaced owner. Failure will not justify
  shrinking or dropping a root after seeing results.
- No production switch is recommended yet. Source nomination, broader exact
  candidates, heldout checks and full controls remain active goal work.

## Reproduction evidence

Implementation milestone: `84554939` on main. Native producer tests3PASS;
finite projected-bank tests4PASS; profiling/evaluator Python tests21PASS.
The public Cargo examples are documented in
[the research-tool README](../../tools/research/rule_optimizer/README.md).

Ignored evidence root: `TMP/rule-optimizer-20261003/`.

- `candidates/prescribed-source-build-v2/`: optimized build/tests, symbolic
  source proofs, checked exports and receipts. Producer build times119.50 and
  121.75s; native proof about0.005s after about0.94s owner preparation. These
  are source-check costs, not a five-loop family-generation timing.
- `candidates/prescribed-source-dispatch-v2/`: actual C19 selection and
  unchanged outside-chart fallback controls.
- `candidates/prescribed-spectator-c19-v1/` and `-v2/`: plans, exact commands,
  guarded process/resource receipts, checkpoints, cold results and comparisons.
- `profiles/C19_DOWNSTREAM_PROFILE.md`: bounded, stage-biased diagnostics;
  no population-wide or causal work attribution is inferred from this sample.

The comparison command is `TMP/rule-optimizer-20261003/run_pair.py --plan PLAN`,
with plans generated by `tools/research/rule_optimizer/evaluate.py`. The runner
uses the existing owned-process guard and locks; do not run it on production's
cores. Both production inputs and checkpoints are unchanged.
