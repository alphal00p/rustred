# Selective symmetry–IBP feedback and local dimension-pair recurrences

Status: both requested features implemented, in that order, on
`codex/selective-symmetry-dimrec`. Final native gate: **890 passed, zero failed,
five explicit ignores**. The user's production campaign and old frozen build
were not changed or signalled. Selective feedback finds seven additional
dependencies on the tested five-loop input. Local dimension-pair sources find
no further dependencies on that input and remain experimental, opt-in work.

## First: feed useful auxiliary targets back into finite refinement

The existing complete-numerator symmetry equations can leave auxiliary
integrals in the exact sparse row space. They are not new masters. Some link
two protected terminal integrals only after an additional ordinary IBP or
symmetry equation is generated at an auxiliary point.

For example, rows linking `T1` to `A` and then `A` to `B` need not eliminate a
terminal immediately. An equation generated at `B` can finally connect `T1` to
`T2`. Stopping after the first no-gain round misses that chain. Conversely,
recursively enqueuing every expanded numerator monomial proved far too costly
in the previous experiment.

The new opt-in policy selects one actual terminal-linked auxiliary key per
positive support, minimizing `(A+R, R, A, signed index tuple)`. It tries that
support once while the protected terminal rank remains fixed. New supports
can enter without a rank gain; old supports reopen only when that rank strictly
decreases. Every new integral remains auxiliary, and every accepted equation
retains its exact source and conditions.

There are finitely many supports for a fixed family and finitely many strict
rank decreases. This bounds the number of selected attempts mathematically,
without a hard-coded work or round ceiling. It does not bound coefficient
swell or provide a practical runtime estimate. Exhaustion means a fixed point
of this selected policy, not a minimal basis or exhaustive IBP saturation.

Each round must bind the complete parent native row-space snapshot, maps and
guards, selected keys, regenerated source witnesses, exact task vector and
committed progress. Finalization may retain proved maps from an interrupted
round without claiming that its remaining search completed.

## Second: compile a local dimension pair back to the original dimension

Choose a loop-integration direction and a momentum `K` normalized along it.
Let `q1,...,qE` span the spectator directions needed by all active denominator
and numerator factors. Define

```
H = Gram(q1,...,qE)
G = det H
P = det Gram(K,q1,...,qE)
U(A) = sum_i alpha_i * a_i * E_i^+
```

Here `alpha_i` is the coefficient of `K^2` in the complete affine quadratic
factor, including factors with negative indices; `a_i` denotes its effective
signed exponent `n_i + nu_i`, including any declared analytic shift.
With `B_i = E_i^-`, the local
dimension-pair identity becomes the fixed-d equation

```
[2 U(A) P(B) - (d-E) G(B)] I_d = 0.
```

The order matters: `[A_i,B_i]=1`, so the first product uses the original `a_i`,
not `a_i` after the lowering monomial has acted. Both intermediate shifts are
local to the selected subintegration; they are not a shift of every loop of
the parent family. No shifted-dimensional master is exported.

An independent ordinary-IBP certificate uses the polynomial vector

```
V = G*K - sum_ij q_i * adj(H)_ij * (K.q_j).
```

It satisfies `div V=(d-E)G` and gives the same equation. Polynomial weights
must be expanded into regenerated ordinary sources at shifted seeds. Multiplying
an already integrated equation by a loop-momentum polynomial would be wrong.
Existing native Symbolica polynomial arithmetic and RustRed source generation
are reused rather than implementing another algebra kernel.

For `D=K^2-m^2`, the tadpole control is

```
(2*a-d)*I(a) + 2*a*m^2*I(a+1) = 0.
```

Mass constants, dotted lines, signed numerator powers and factors such as
`d-4` stay exact. An omitted numerator spectator changes the identity; such a
nomination must be enlarged or rejected, never silently treated as scalar.
The final polynomial certificate need not divide by a Gram determinant or
`d-E`. Any divisions made later by elimination retain their ordinary guards.

This is a structured way to nominate useful finite IBP combinations. It does
not guarantee new independent identities or a minimal set of masters. Dense
global dimension-shift expansions are deliberately not the default approach.

## Validation and measurements

The final release-linked single-worker lower-loop probe compares the same
generator cohort and depth-zero ordinary search with/without the selected
feedback loop. These are single diagnostic runs, not paired performance claims.

| Input | Basis before ordinary search | Baseline final | Feedback final | Baseline seconds | Feedback seconds |
|---|---:|---:|---:|---:|---:|
| K6 | 5 | 5 | 5 | 0.402 | 0.602 |
| H | 22 | 19 | 19 | 14.225 | 18.633 |
| FG | 16 | 15 | 15 | 6.011 | 8.216 |
| BMW | 17 | 15 | 15 | 7.013 | 8.615 |
| X | 19 | 16 | 16 | 15.829 | 18.633 |

All 1,193 raw maps compose and retain their original guards. Cold loading passes.
Selected rounds: 4/6/5/5/5; targets: 15/162/105/79/109. Each case has actual
no-gain rounds exposing new supports before exhausting the finite selected
queue. The mechanical behavior is validated; no physical master-count benefit
or speedup is established by these lower-loop controls. A real tadpole test
does establish that a selected auxiliary source can add an exact relation
without promoting that source to the protected master roster.

These are family-local basis counts, not globally independent-master counts.
Evidence: `TMP/selective-symmetry-acceptance-20261023/final-stage1/` and
`TMP/selective-symmetry-acceptance-20261023/STAGE1_RESULTS.md`.

### Five-loop selective feedback: seven verified additional dependencies

The complete original roster has 939 retained keys. Initial symmetry-generator
work produces a 667-key basis; draining 23,475 ordinary rows produces 659.
Neither decrease is attributed to the new feedback policy. Its first completed
round chooses 1,177 actual terminal-linked support targets and produces
**659 → 652**, seven genuinely additional dependencies. All 939 original maps
compose exactly, all 136,790 inherited guard records survive, final outputs
contain only declared remaining terminals, and native cold replay passes.

This is not a comparison against the user's different full-orbit-plus-ordinary
best map, a minimum-master proof, or unrestricted family closure. Existing
proved maps remain inherited when starting a fresh refinement search.

The owned 30-minute pilot stops cooperatively after 1,764.410 seconds
(1,799.109 CPU seconds, 5.005 GB peak RSS). It retains 652 basis keys and the
next round's prepared witnesses/tasks: 995 committed rows and 5,724 pending.
That round selects 3,701 supports. Its expensive preparation is predominantly
serial; a configured 16-worker budget averaged only 1.02 observed cores here.
The selected worklist has **not** reached its fixed point on this five-loop
pilot. No engine work/round cap was introduced.

An actual cold app resume, changing workers 16 → 4 and requesting an immediate
safe pause, completes in 433.347 seconds. Native bytes, generation, accepted
and pending cursors, and all 113 input files remain unchanged. Cold proof
regeneration is therefore substantial, not an instantaneous restart. A prepared
round survives interruption; a wave interrupted before preparation commits can
require its source planning again. Finalization retains committed exact maps
without pretending that pending search finished.

The immutable 652-key state used by the second-stage tests is
`TMP/selective-symmetry-acceptance-20261023/native-cli-five-loop-selected-final/round-0-complete/state.rrbin`,
BLAKE3 `03acf3779cf7cff8238ad991f55ab9429dad731e76f3b94e7d3ff7b620e6463d`.

### Local dimension-pair sources: complete negative result

This comparison imports the same existing exact maps and guards into fresh
finite sessions, optionally prepares and cold-replays all local sources, then
runs the same depth-zero ordinary work. It does not include Stage 1's generator
preparation; times across these two tables are not directly comparable.

| Input | Initial basis | After local sources alone | Ordinary final, baseline / local | Baseline / local wall seconds |
|---|---:|---:|---:|---:|
| K6 | 5 | 5 | 5 / 5 | 0.201 / 0.402 |
| H | 22 | 22 | 19 / 19 | 1.404 / 23.841 |
| FG | 16 | 16 | 15 / 15 | 0.602 / 7.615 |
| BMW | 17 | 17 | 15 / 15 | 0.802 / 18.233 |
| X | 19 | 19 | 16 / 16 | 1.604 / 48.086 |
| Five-loop retained set | 652 | 652 | 652 / 652 | 25.848 / 1,254.158 |

Every full raw cohort was processed. All 1,193 lower-loop maps and all 939
five-loop maps compose exactly; inherited guards, source regeneration and final
native cold checks pass. These are completed runs, not censored estimates.
The five-loop local branch uses 1,243.275 CPU seconds and 10.033 GB peak RSS.

Its 2,658 equations expand into 2,820,878 terms, 833,979 inventory keys and
4,126,070 regenerated weighted ordinary sources. Preparation costs 485.127 s,
complete cold source replay 484.004 s, pure-source joining 24.799 s, and ordinary
work 214.995 s. The matched baseline ordinary tail costs 14.811 s. The complete
numerator spectator span often has four directions, causing expensive Gram
polynomials and many auxiliary columns. The pair is mathematically valid but
adds work without an additional physical rank gain here. Do not enable it by
default or advertise it as a master-count improvement.

Evidence: `TMP/local-dimension-acceptance-20261023/RESULTS.md`,
`lower-matrix.summary.json`, and `five-loop-matrix.summary.json`.

### Bounded parallel correction: source witnesses, not a new strategy

Independent witness jobs reuse the existing licensed `SectorExecutor` and
merge in original ordinal order. Geometry/source definitions are shared
immutably; exact algebra and guards are worker-local. One worker stays inline.
The caller retains non-`Sync` progress callbacks and cooperative cancellation.
No partial plan authorizes rows after an error; proof errors are not masked by
a simultaneous cancellation. Worker counts are operational only and excluded
from source-capsule bytes, mathematical statistics and phase identity.

Two same-binary, same-input H pairs, in forward and reversed run order, show:

| Workers | Whole client seconds, pair 1 / 2 | Prepare seconds | Cold source replay seconds | CPU seconds, pair 1 | Peak RSS, pair 1 |
|---|---:|---:|---:|---:|---:|
| 1 | 22.436 / 22.437 | 9.101 / 9.113 | 9.286 / 9.303 | 22.144 | 543 MB |
| 16 | 13.833 / 13.833 | 4.765 / 4.719 | 5.010 / 5.071 | 59.764 | 554 MB |

This reproduces a **1.62× whole-client speedup**, at higher CPU expenditure,
not 16-core saturation. All four source capsules and pure/final native states
are byte-identical; maps, guards and basis counts agree. Serial geometry
nomination and exact joining remain. Generic inspection without an explicit
worker/cancellation context stays serial; active preparation and controlled
replay use the existing refinement worker budget.

Evidence: `TMP/local-dimension-acceptance-20261023/parallel-v1/h-pairs.summary.json`.
A separate full five-loop 16-worker measurement **completed**, uncensored:

| Component | Serial fallback seconds | Parallel correction, 16 workers, seconds |
|---|---:|---:|
| Whole client | 1,254.158 | 908.901 |
| Source preparation | 485.127 | 339.186 |
| Complete cold source replay | 484.004 | 344.730 |
| Pure-source join | 24.799 | 19.801 |
| Ordinary tail | 214.995 | 172.532 |
| CPU seconds | 1,243.275 | 2,382.297 |
| Peak RSS, decimal GB | 10.033 | 9.879 |

The whole-client decrease is 27.5%, but this is a **cross-revision comparison**,
not two matched same-binary pairs or a statistically established five-loop
speedup. CPU cost increases, and average observed utilization is only about
2.62 cores of the 16-worker budget. Serial geometry nomination takes about
283 seconds before independent witness jobs start, limiting the gain. No new
performance tuning or source-selection strategy follows in this delivery.

The complete source capsule, pure-source native state and final ordinary native
state are all **byte-identical** to the serial results. All 939 maps compose,
all 136,790 inherited guard records survive, and full source/native cold checks
pass. The basis remains **652 → 652**, both pure and after ordinary work.
The final native-state BLAKE3 is
`9e5be823167103b5c3bca471a4483d381c3ceb7670ad5cdcde8d3a98df310b20`.
Evidence: `parallel-v1/five-loop-stage1-652-16w/process.receipt.json` and
`native/receipt.json`. All owned pilots, shutdown guards and test processes
have exited; production remains untouched.

## Interfaces, exact authority and lifecycle

Rust exposes a sealed `LocalDimensionRecurrencePlan` with finite input inventory,
exact equations, native encoding and complete regeneration on cold load.
Both source providers feed the existing exact terminal-relation session and
retained-map elimination. The native generated U remains a trusted exact
authority; this work does not retrospectively claim an independent full W*A
certificate for every inherited row.

CLI opt-ins:

```text
--selective-symmetry-feedback
--local-dimensional-recurrence-assistance
```

The Python production driver prefixes these with `--master-`; the short saved
campaign facade also accepts the unprefixed options and their `--no-` variants.
Both are exact-strategy options. Selective feedback requires sector-symmetry
assistance; unsupported dynamic saved/circuit-provider combinations fail
explicitly rather than inventing counters or silently dropping obligations.
Flag-off state and old schemas remain unchanged. New round sources bind parent
U/maps/guards, selected keys, native witness capsules, tasks and committed
prefixes. Local sources bind one complete original raw/canonical input cohort.
They are **not automatically regenerated at each later selective round**;
separate ordered phases are the recommended workflow.

The mathematical checks include complete signed/analytic powers, spectator
span, embedded momentum invariants, mass constants, orientation, operator order,
`d-4` factors and coefficient coalescing. Parameter-valued momentum geometry and
zero-Gram nominations are explicitly recorded as unsupported/skipped; algebra,
resource and cancellation failures are not swallowed as skips. No unknown leaf
is promoted to a master. Stable terminal identities and previous proved aliases
are retained. No new production IBP campaign, numerical master values or
Vakint mutation is part of this task.

Final gates: core 394 + app 496 = **890 passing native tests**, five explicit
ignores; Python 314 tests, one explicit skip. Source and mathematical audits were
independent of implementation. Early test-oracle failures and their exact
corrections remain in the evidence rather than being discarded.

## Commands and frozen build

Implementation remains isolated in
`/shared/codex-2/rustred/TMP/selective-symmetry-dimrec-20261023` on
`codex/selective-symmetry-dimrec`; unrelated main work is preserved.
Use Nix from the tracked workspace root, not from the untracked release folder.

Frozen parallel executable:

```text
/shared/codex-2/rustred/TMP/releases/20261023-local-dimension-parallel/rustred
SHA256 ca0f3fb1205f9d479551e4444f865b00cd08ffb605785de66ad0c512b5c55fdd
```

After stopping any refinement of the **same** campaign with Ctrl+C and waiting
for its checkpoint and process exit:

```bash
cd /shared/codex-2/rustred
source TMP/releases/20261023-local-dimension-parallel/workflow.sh
# If a previous refinement is paused/unfinished, first retain its proved maps:
rr --finalize-refinement
# Start stage 1 with a fresh search matrix, retaining all proved maps:
rrfeedback
```

If the preceding phase already completed normally, skip `--finalize-refinement`:
it is for unfinished stopped searches, not a mandatory post-completion step.
`rrfeedback` uses exact generators, no saved projection, depth (0,0), 16 workers
on CPUs 64–79, batch/block 256/8 and the existing 600 GB process RAM ceiling.
There are no default search/work/round ceilings. These are resource choices,
not a claim of 16-core utilization.

After stage 1 completes normally:

```bash
rrdimension
```

To switch after an early cooperative pause instead, wait for process exit,
run `rr --finalize-refinement`, then `rrdimension`. This retains accepted maps
but explicitly leaves the remaining selected search incomplete. The second
command creates a fresh local-source phase and regenerates finite ordinary-IBP
combinations, not a new campaign, rank extension or parametric-rule generation.
Its absence of incremental gain on tested inputs is documented above.

For either phase:

```bash
# Resume exactly the current phase, without resetting its search:
rr --refine-masters
# Inspect the latest published artifact:
rrinspect
# After a cooperative pause, publish the accepted exact partial maps:
rr --finalize-refinement
```

`rrinspect` reports the latest publication, not uncommitted current search.
Finalization exports proved maps without declaring optional refinement complete.
The workflow defaults to
`campaigns/five-loop-minimal-r0-d10-incremental`; override `RUSTRED_CAMPAIGN`
after sourcing if a different campaign is intended. No command here was run
against the user's production campaign during development.

The validated serial fallback is
`TMP/releases/20261023-local-dimension-refinement/rustred`, source commit
`0c881ccb`. Old delivered builds remain unchanged. Fallback must still follow
the same phase/identity rules; changing a source policy requires a fresh search
after retaining previous proved maps, not an in-place incompatible resume.

## References

- [Tarasov, hep-th/9606018](https://arxiv.org/abs/hep-th/9606018): dimension-changing
  relations and tensor integrals.
- [Bitoun et al., 1712.09215, section 2.5](https://arxiv.org/html/1712.09215): paired
  dimension shifts and their connection to momentum-space IBP operators.
- [Pikelner, FMFT, 1707.01710](https://arxiv.org/abs/1707.01710): localized
  propagator-subgraph/convolution methods motivating the structural selection.
