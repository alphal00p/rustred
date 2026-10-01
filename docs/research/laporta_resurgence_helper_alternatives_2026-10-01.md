# Target-directed Laporta identities and dimensional recurrences: scoped alternatives

Date: 2026-10-01. Initial source/literature assessment at main `df059d8d` was
read-only. A subsequent authorized, frozen-native coefficient-face diagnostic
is recorded below; it changed no solver or production campaign. Experimental
proposals are distinguished from measured results. The later user request
authorizes bounded investigation of radical rule improvements, not deployment
of an unqualified identity or silent relaxation of the existing proof gates.

## Recommendation

The useful near-term Laporta experiment is **one exact, case-bound shortcut
that eliminates a demonstrated costly intermediate**, using RustRed's existing
source rows and Symbolica elimination. It is not another whole-family seed
expansion, new linear-algebra kernel, or declaration that bounded misses are
masters. A real MissingRule and a valid-but-expensive recurrence are different
problems; the existing feedback path directly handles only the former.

The user subsequently clarified that “resurgence” meant **dimensional recurrence
relations**, such as shifts by `d±2` or their epsilon equivalents, not literal
resurgent analysis. That is the intended alternative to investigate. The literal
resurgence discussion below records the original terminology distinction only;
it is not a proposed research lane. Dimensional identities may add useful exact
relations, but neither their existence nor analytic master-value information
establishes finite symbolic closure for the current dispatcher.

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

There are also two existing cancellation layers that must not be proposed as
new work: [source preconditioning](/common/dev/rustred/crates/rustred-core/src/solver/precondition.rs:111)
already does polynomial GCD-scaled forward/backward row elimination, with
[optional original-source provenance](/common/dev/rustred/crates/rustred-core/src/solver/precondition/provenance.rs:48);
and [applied-rule inspection](/common/dev/rustred/crates/rustred-core/src/solver/candidate_reduction/owners/domains/applied/engine.rs:647)
already sums equal-shift coefficients and removes native exact zeros. By
contrast, the [candidate trace contract](/common/dev/rustred/crates/rustred-core/src/solver/candidate_reduction/trace.rs:30)
explicitly does not back-substitute coefficients across paths. That is a
distinct seam for a measured, guarded composition experiment, not evidence
that useful hidden cancellations must exist in this workload.

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

Smith and Zeng combine syzygy-constrained operators, operator-level row
reduction, and small partly symbolic target-neighborhood systems. This is
relevant to controlling unwanted shifts before seeding. RustRed already has
the row-reduction ingredient above; reproducing it under another name adds
nothing. What remains unestablished here is a measured advantage from a
constrained operator basis or a new targeted source combination, including its
exceptional cases. No second syzygy/CAS engine is proposed.
[Smith and Zeng, section 3 and appendix A](https://arxiv.org/html/2507.11140v2)

## Measured coefficient-face falsifier and next finite composition diagnostic

The source-reviewed wrapper and independent receipt audit are under
`TMP/postlaunch-20261001/coefficient-face-split-v1/`. Its
[result](/common/dev/rustred/TMP/postlaunch-20261001/coefficient-face-split-v1/execution-r1/RESULT.json)
completed in 4.535 seconds inclusive; the sole native phase took 3.781 seconds,
with a 150,000,000,000-byte effective hard cap, 93,851,648-byte sampled tree RSS,
CPU32, and all owned groups drained. No generation, build, walk, or new rule
publication occurred.

For saved A1 owner481 rule40 / ordered-piece393, the original native query
report was reproduced exactly. Splitting only the source into `x8=0` and
`x8>=1` removed the nominated `n8`-weighted successor from the zero face and
made it uniformly nonzero on the other face, where native target lower axis8
became 1. All original guards, 44 denominator obligations and RHS terms were
retained; all three inspections finished with zero problems/refusals.

| Native work | Original | Zero face + positive face |
| --- | ---: | ---: |
| Successor events | 37 | 22 + 37 |
| Conditional successor events | 1 | 0 + 0 |
| Native operations | 173 | 112 + 159 |
| Term visits | 58 | 44 + 44 |
| Predicates | 46 | 46 + 46 |

The other 14 shift/owner groups absent on the zero face already had native
baseline source lower axis8 equal to 1; they are not newly removed obligations.
Only the nominated conditional group had the broader source. This confirms
one loss of coefficient-support information, **not** a global saving: immediate
work increases, and another RHS term may reach the excluded target points.
No target-union comparison, complete-program equivalence, closure claim or
speedup was inferred from these event counts.

The next proposed falsifier is finite and algebraic: select one actual
same-owner parent/child recurrence pair from the qualified four-loop program,
use native applicability at fixed integer points, and substitute the child's
entire RHS into the parent's entire RHS with Symbolica. Count exact zero sums
at identical final integral keys, coefficient growth, and all retained
denominator/exception conditions. A reduced local key count is only a witness
worth investigating; a useful result must later survive the unchanged full
four-loop cohort and a separately scoped five-loop test. No parametric lift
or priority change follows from fixed-point success. At this update, that
composition diagnostic is being prepared, not yet executed.

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

### Clarified intent and checked implementation boundary

The requested subject is dimensional recurrence. Do not confuse the fact that
existing coefficients depend symbolically on `d` with a relation between integrals
at different dimensions. The present
[native integral transport compiler](/common/dev/rustred/crates/rustred-core/src/sector/symmetry/integral_transport/compile.rs:44)
rejects unequal source and target dimensions; its
[explicit regression](/common/dev/rustred/crates/rustred-core/src/sector/symmetry/integral_transport/tests.rs:603)
shows that a verified denominator map still does not authorize a `d→d+2`
integral relation. A dimensional recurrence therefore needs its own exact
identity/provenance and dimension-labelled obligations; it cannot be installed
as an ordinary momentum-routing alias.

There is already a
[two-mass bubble Laporta test](/common/dev/rustred/crates/rustred-feynkit/tests/test_integral_expression.py:43),
but the tested equation lowers `[2,1]` to the bubble and two tadpoles at the same
symbolic dimension. That test is not a dimension-shift implementation. The
reference FMFT implementation **does contain a genuine bubble dimensional
recurrence**: [its function declarations](/common/dev/rustred/FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop/crates/vakint/form_src/fmft/fmft.frm:142)
explicitly make the first argument of `G(dp,...)` a dimension shift, and
[the `drrG` procedure](/common/dev/rustred/FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop/crates/vakint/form_src/fmft/fmft.frm:527)
replaces positive-`dp` terms by `G(dp-2,...)` and, in its second case, tadpole
`T1(dp-2,...)` terms. Its coefficients contain `d+dp`; case restrictions and
denominators are explicit. This supports the user's recollection, but is
reference FORM code, not an already installed RustRed dimensional dispatcher.

The separate [epsilon-power derivation](/common/dev/rustred/EPSILON.md:160)
integrates a massless bubble to a Gamma prefactor times a remaining propagator
with an epsilon-dependent exponent. That lowers loop count without itself
changing the ambient dimension of the remaining integral. It is distinct both
from the fixed-d bubble IBP test and from FMFT's `dp` recurrence; a fully massive
bubble cannot generally be replaced by that simple massless formula. For
`d=4-2*epsilon`, a shift `d→d±2` changes epsilon by `∓1`, not by an infinitesimal
expansion step. Any proposal must retain dimension labels, normalizations,
case restrictions, denominator obligations and all shifted RHS terms. No new
dimension-shift kernel or pilot is proposed here.

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
This is the dimensional-recurrence subject clarified by the user. Its analytic
solution machinery and a new exact identity source for reduction remain
separate implementation questions.
[Lee](https://arxiv.org/html/0911.0252v2)

Finally, a finite master dimension is not a termination or runtime theorem for
the present symbolic-overcover algorithm. A finite input list, finite seed
budget, finite coefficient sample, finite terminal list and proved finite
parametric closure are different claims.
[Smirnov and Petukhov](https://arxiv.org/abs/1004.4199)
