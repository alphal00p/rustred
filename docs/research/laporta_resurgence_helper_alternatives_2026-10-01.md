# Target-directed Laporta identities and resurgence: scoped alternatives

Date: 2026-10-01. Read-only source/literature assessment at main `df059d8d`.
No solver, build, campaign, or numerical experiment was run for this note.
Only this document was written. Proposals below are not implementation grants.

## Recommendation

The useful near-term Laporta experiment is **one exact, case-bound shortcut
that eliminates a demonstrated costly intermediate**, using RustRed's existing
source rows and Symbolica elimination. It is not another whole-family seed
expansion, new linear-algebra kernel, or declaration that bounded misses are
masters. A real MissingRule and a valid-but-expensive recurrence are different
problems; the existing feedback path directly handles only the former.

Literal resurgence is a different avenue: it can determine analytic solutions,
asymptotic information or master-value boundary data under additional hypotheses.
The sources reviewed do not supply a drop-in theorem converting that information
into exact generic five-loop IBP dispatch rules or finite symbolic closure.
Difference equations and dimensional recurrences are relevant alternatives, but
must not silently be called resurgence.

## Existing machinery: reuse before adding anything

- [Shared numerical-case search](/common/dev/rustred/crates/rustred-core/src/solver/numeric.rs:1)
  already shares a modular reducer, seed deduplication and accepted source rows
  across fully fixed targets of one sector. Winning dependency traces are united
  and materialized exactly using Symbolica. Here **numerical case means fixed
  integer indices**, not approximate coefficients: dimension and physical
  parameters remain symbolic in the exact result. Bounded misses remain explicit
  residuals, not independently established masters.
- [The runtime bridge](/common/dev/rustred/crates/rustred-core/src/solver/bridge.rs:107)
  already exposes `solve_laporta`: recursively search newly encountered finite
  RHS targets, respect `max_targets`, and back-substitute solved equations.
  Its runtime dispatch supports only arities 1 through 12 and its local solver
  construction uses default configuration. Do not claim it directly serves the
  fifteen-index five-loop campaign or preserves a nondefault saved comparator.
  The existing const-generic `SectorSolver::<15>::solve_numeric_cases`, or the
  owner-bound domain-search path, avoids inventing another elimination kernel.
- [Owner-bound search](/common/dev/rustred/crates/rustred-core/src/solver/candidate_reduction/owners/feedback.rs:354)
  reconstructs the actual persisted order, existing source context, zero evidence
  and selected exact backends. [Independent overlay replay](/common/dev/rustred/crates/rustred-core/src/solver/candidate_reduction/owners/feedback/replay.rs:10)
  checks identity and declared guards, explicitly not descent, requested-case
  coverage, RHS closure or terminal independence.
- [Feedback nomination](/common/dev/rustred/crates/rustred-app/src/application/routed_campaign/feedback/nomination.rs:82)
  accepts actual `MissingRule` trace frontiers. It does not nominate a recurrence
  merely because its descendants are expensive. The geometric witness adapter
  also explicitly says a point does not discharge its surrounding symbolic region.
- Overlays append after existing batches, and [ordered matching](/common/dev/rustred/crates/rustred-core/src/solver/candidate_reduction/owners/domains/matching/engine.rs:375)
  exhausts each earlier batch first. Consequently an appended shortcut normally
  **cannot supersede an already applicable baseline rule**. Cost-directed use
  needs an explicit, separately reviewed case-bound replacement or newly built
  owner program, not an undocumented precedence change.

The existing numerical tests already exercise shared dependency lifting and the
danger of erasing a direct-hit row before subsequent elimination. Those are
regressions to retain, not functionality to implement anew.

## What primary Laporta research adds

Laporta's original work constructs IBP reductions and one-index difference
equations, then uses factorial series or Laplace transforms to calculate master
integrals. Algebraic equation construction and analytic solution are separate
stages. A finite target reduction does not establish a rule on an entire index
domain. [Laporta, 2000/2001](https://arxiv.org/abs/hep-ph/0102033)

Kira 3 gives a particularly relevant mechanism: dependency selection performed
after forward elimination can avoid equations pulled in by cancellations hidden
in later substitution. It checks sufficiency for the requested targets and adds
unreduced targets when needed. The transferable lesson is to select the actual
source dependency slice and account for cancellations before committing to a
large solve—not to copy its numerical checks as exact authority. RustRed already
has dependency-trace selection, so usefulness must come from a better nominated
target/composition rather than merely naming that mechanism.
[Kira 3, section 3.2](https://arxiv.org/html/2505.20197v1#S3.SS2)

Target-aware seed priorities are another possible search improvement, not a
different identity class. A 2025 study develops interpretable priorities that
reduce required seed sets; its benchmark gains are not a prediction for this
five-loop symbolic workload.
[Song et al.](https://arxiv.org/abs/2502.09544)

The 2026 tube-seeding preprint is more narrowly relevant: translate a small seed
set along a path between a concrete target and lower integrals. It demonstrates
linear-in-rank seed scaling in specified finite-field examples, including full
spanning-cut reconstruction of target reductions. It explicitly leaves a general
proof that a closing base set produces a closing tube to future work; some cuts
require extra diagonal paths. Numerical kinematics and dimension keep field
operations cheap; analytic coefficient reconstruction is a separate cost.
Thus a sparse target corridor is a candidate search schedule, never permission
to discard integrals outside it or infer a general symbolic recurrence.
[Berman et al., especially sections 4.3 and 5](https://arxiv.org/html/2606.10698v1)

NeatIBP controls propagator-degree growth using syzygies/module intersections.
That is relevant to the mathematical objective, but its Mathematica/Singular/
SpaSM implementation is not a small Symbolica-only addition to this codebase.
Do not start a second CAS stack to test the immediate shortcut hypothesis.
[Wu et al.](https://arxiv.org/abs/2305.08783)

## Concrete proposed experiment and falsifiers

### 1. Compose existing identities first; search only when a relation is missing

Freeze one existing owner, saved order and an actual guarded source case whose
native successors repeatedly produce costly routing/partition work. Select a
small explicit set of integral points, including an exceptional boundary and an
adjacent negative control. If existing rules already reduce the costly
intermediate, try their exact composition on their common applicability domain
first: this requires no new IBP search. Preserve the original source provenance
and verify the composed identity independently; an expression simplification
alone is not a new dispatch certificate.

If that relation is missing, reuse shared finite-case search with one declared
seed-depth and target budget. Do not regenerate every owner or sweep bounds.

These are three separate gates: existing-rule composition, optional fixed-target
Laporta discovery, and (when needed) parametric lifting. At the finite stage,
ask only whether exact source combinations eliminate a nominated intermediate
or expose an exact cancellation. A solution at one point is support discovery,
not a parametric rule. If a point-discovered case has free indices, regenerate
and replay the proposed combination with those indices symbolic, using the
existing source/guard machinery. No interpolation from a few index values is
proof. An already-symbolic composition still needs its complete common domain
and translated child guards checked, but need not repeat finite discovery.

For example, exact relations `T = a B + C` and `B = D` permit the composed
identity `T = a D + C`. If `D` contains an opposite copy of a term in `C`, exact
coalescing can avoid visiting that intermediate at all. But a longer `D` can
instead increase expression size and descendant work. This is the experiment,
not an assumed gain.

Acceptance requires all of:

1. Original-source identity replay with native coefficient operations, correct
   original coordinates and the unchanged mathematical comparator.
2. The complete common applicability domain. Child guards must be pulled back
   through the substitution to the source coordinates; source conditions,
   original denominators, exceptions and new pivot poles cannot be dropped.
   A cancelled denominator is not automatically a removable specialization.
3. Native descent and complete dispatch/exception coverage, or a separately
   established replacement proof if the ordinary descent contract cannot apply.
   A pointwise identity alone authorizes neither route closure nor publication.
4. Unsolved finite targets remain obligations. No new terminal is accepted just
   because the local seed budget exhausted. Residual-free overlays leave new RHS
   coverage to the walker; productive new RHS domains are allowed.
5. An explicit new program/precedence proposal, followed by the unchanged full
   scoped cold check. The failed pre-cut cold control demonstrates why fewer
   local pieces cannot substitute for this gate.

Falsify early if the same baseline identity returns, the nominated intermediate
survives, symbolic lifting fails, a new guard cannot be resolved, the bounded
solve exhausts, or the replacement creates more total descendant work. If local
proof succeeds, evaluate the full fixed cohort, not only the witness panels.
Report one-off generation/replay separately from sustained traversal; neither
omit setup nor reject an amortizable gain solely because setup costs more.

### 2. Sparse seed corridor only if the existing local search is the blocker

If trace evidence shows most seed work is irrelevant, a single predeclared
target corridor could replace the shell visitation schedule while retaining the
same source generator, exact reducer and explicit unresolved-target handling.
This is not presently a public seed-iterator option in the audited numerical
path. Its implementation would need separate authorization and native tests.
Reject it if it merely shifts work into denser elimination or exact replay.
Do not add tube widths, target orders and source portfolios as a simultaneous
grid; first demonstrate the missing useful source combination.

### Scaling to expect, not benchmark claims

For `m` unrestricted varying seed coordinates, a signed-L1 ball of radius `r`
has `sum_j 2^j binom(m,j) binom(r,j)` points. At `m=15`, radii 2 and 3 give
481 and 4,991 points before sector restrictions, deduplication and early exit.
Each admitted seed may instantiate multiple source rows. Therefore even one
extra search shell can be expensive. The [existing seed iterator](/common/dev/rustred/crates/rustred-core/src/solver/seed.rs:11)
streams rather than storing the shell, but that does not eliminate row work.

A fixed-width corridor has seed count proportional to path length times its
base-set size; this does not bound elimination fill-in, coefficient degrees,
exceptional branches or the number of targets subsequently discovered. Exact
symbolic arithmetic and a reusable parametric macro-rule can cost much more
than a finite-field solve. Independent target batches can be parallelized, but
duplicated source/context work must be charged and no scaling claim follows.

## Resurgence, index recurrences and dimension shifts are distinct

**Literal resurgence.** The rigorous output can concern Borel summability,
analytic continuation, Stokes data and selection of a physical solution among
formal solutions. Clavier proves Borel-Ecalle summability for a specific
Wess-Zumino two-point function using its renormalization-group and
Schwinger-Dyson equations; this is not an arbitrary fixed-loop IBP closure
construction. [Clavier](https://arxiv.org/abs/1912.03237)

A particularly instructive limitation comes from a zero-dimensional quartic
model: a Borel-summable expansion around one saddle can still miss information
from other saddles. Improved truncation uses additional analytic data; it does
not license setting an unresolved tail to zero. The paper explicitly leaves
higher-dimensional applications as a difficult extension.
[Peng and Shu](https://arxiv.org/html/2410.13364v2)

For RustRed, a potential resurgence contribution would therefore be a separately
proved master-value or asymptotic-boundary service for a specified recurrence,
contour, parameter region and continuation prescription. It cannot establish
that a rational index identity holds generically, declare a numerator tail zero,
or turn observational dependency closure into termination. I found no reviewed
primary result that makes this the next generic five-loop rule-search method.

**Index difference equations.** Keeping one exponent symbolic can yield useful
reusable lowering relations and is compatible in principle with the existing
partially fixed case search. Derive the equation exactly first; analytic
solution is a different task. Extra initial values and singular recurrence
coefficients must remain explicit. This is not evidence that a one-variable
recurrence covers every multivariate case or saves work in this campaign.

**Dimensional recurrence and analyticity.** Tarasov derives relations connecting
different dimensions, complementary to ordinary IBP. Such a relation can trade
numerators for dimension-shifted objects, but those objects require their own
typed semantics and reduction obligations; they are not free terminals in the
current fixed-dimension owner path.
[Tarasov](https://arxiv.org/abs/hep-th/9606018)

Lee's dimensional-recurrence-and-analyticity method uses analytic information to
fix periodic-function ambiguities of the difference equation, sometimes leaving
constants to determine separately. It is a strong master-evaluation route, not
a proof that local recurrence identities alone uniquely determine the integral.
Treat this as a separate longer-term proposal if the intended request was
recurrences rather than literal resurgence.
[Lee](https://arxiv.org/html/0911.0252v2)

Finally, a finite master dimension is not a termination or runtime theorem for
the present symbolic-overcover algorithm. A finite input list, finite seed
budget, finite coefficient sample, finite terminal list and proved finite
parametric closure are different claims.
[Smirnov and Petukhov](https://arxiv.org/abs/1004.4199)
