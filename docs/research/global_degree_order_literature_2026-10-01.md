# Global absolute-degree ordering: literature limits and bounded-scope use

Started 2026-10-01; native pilot results added 2026-10-02. Primary-literature,
source review and controlled measurements are distinguished below. No master
evaluation or production change. Native results, not the literature analogy,
determine performance conclusions.

## First native discriminator: useful local rule, modest whole-work reduction

An optimized current-build comparison regenerated complete owner481 with the
all-ones absolute-degree prefix. Original-source identity/guard replay passes,
and its exact eleven finite terminal keys are unchanged. At required physical
point `[0,1,1,2,2,0,0,0,0,1]`, the selected rule changes from 69 successors
(24 increasing F) to 32 successors (none increasing F). This is not a guessed
terminal replacement or omitted RHS.

Replacing only that complete owner in the unchanged 58-query, 16-owner,
508-route four-loop control gives:

| Metric | Default | One global-F owner |
| --- | ---: | ---: |
| Scheduled domains | 26,025 | 25,077 |
| Native inspections | 17,957 | 17,200 |
| Dependency edges | 495,898 | 483,637 |
| Native traversal | 4.225 s | 4.241 s |
| Cold verification, owned phase | 7.947 s | 9.056 s |
| Staging + walk + cold, arm wall time | 15.747 s | 16.870 s |

Both arms exhaust their queues without frontiers and pass fresh full native
reinspection: all 58 queries and 32 geometric roots, no uncovered obligations.
This verifies the saved dependency coverage under the existing semantics; it
does not assert unrestricted family closure or algebraic termination of sealed
cycles. The short run's stale live closure counters are not the cold result.

The domain reduction is 3.64%, inspection reduction 4.22%, but this one matched
pair shows no speedup. Only one owner changed, so this is neither a full global-F
family study nor five-loop evidence. Its setup is dearer: owner generation
31.535 versus 4.933 seconds; bytes 4,294,254 versus 2,895,567; source replay
about 155 versus 31 seconds per payload. The seven-minute qualification budget
includes duplicate bundle/shard diagnostic replay, not ordinary hot-path work.
There is no campaign-switch recommendation from this result.

Evidence (ignored local receipts):
`TMP/postlaunch-20261001/global-degree-order/{owner481-v1,whole58-v1}/receipts/`.
Frozen CLI SHA256:
`715eb1f3fef9d656d3243a176962a29d46ff6639a30965e4bb2271dc0eb39973`.
Both arms use 16 workers on CPUs32–47 and the same 150GB guard; all processes
drain. The whole pair takes33.358s,449.744s including prior qualification,
excluding compilation and review waiting. Independent source/receipt audits
pass, as do75 selected core tests,6 focused app tests and the complete app
suite (1,325 passed,14 existing ignores,0 filtered). These are not W50 tests.

## Bottom line

Putting `F(n)=sum_i |n_i|=A+R` before support is a genuinely different experiment
from sector-first E/R ordering. It supplies a useful **conditional finite-input
bound**, not a literature-backed guarantee of efficient parametric discovery,
minimal masters, or symbolic-worklist termination. Keeping inactive-reactivation
restrictions may limit its search space, but removing them is not an established
cure for parametric IBP reduction.

## What the primary sources establish

Laporta's original extraction priority starts with denominator count, then dot
and numerator powers. He explicitly separates equation-generation order from
elimination order and explains that ordering affects substitution cost. This is
an algorithmic choice, not a requirement of exact finite-dimensional Gaussian
elimination. His paper does not establish global `sum |n|` as an efficient
replacement. [Laporta, sections 2.3–2.5 and Algorithm 1](https://arxiv.org/html/hep-ph/0102033v1)

Laporta also gives a more directly relevant combination mechanism: a
five-denominator source subsystem produces 20 additional relations between
four-denominator integrals. These reduce the whole higher-dot combination W2
although the smaller subsystem cannot reduce its terms individually. This is
not merely substitution of available single-integral rules followed by exact
zero coalescing, as in our seven-child and endpoint-union tests. It motivates
testing a coefficient-weighted RHS block against additional source relations.
Laporta presents the broader seed-sufficiency pattern empirically, not as a
universal bound. [Laporta, sections 2.6–2.7, equation 17](https://arxiv.org/html/hep-ph/0102033v1)

Smirnov and Smirnov's degree-lex orders apply to shift-operator monomials in
sector-dependent cones. Their s-reduction passes remaining work to lower
sectors. Their earlier straightforward Gröbner construction can leave redundant
irreducibles and fail at zeros of polynomial leading coefficients; they also
describe severe practical construction costs. Thus “degree-compatible” there
does not mean absolute degree ahead of every support change, and their account
does not diagnose inactive-reactivation restrictions as the sole cause of
failure. [Smirnov and Smirnov, sections 2–3, especially equations 19–36](https://arxiv.org/html/hep-lat/0509187v4)

A recent successful counterpoint deliberately constructs syzygy-constrained
identities staying within a sector or its subsectors. Smith and Zeng supplement
operator-level rules by small neighbouring-seed solves where those rules are
insufficient. Their sector definition fixes positive powers, not just signs;
their neighbourhood choice remains heuristic. This supports a hybrid local
completion strategy, not the claim that allowing support reactivation is
necessary for useful symbolic reduction. [Smith and Zeng, sections 2.2 and 3.1](https://arxiv.org/html/2507.11140v2)

Zeng's optimized one-loop bubble example finds descending total-positive-degree
seed and elimination schedules **excluding tadpoles from that observation**.
It optimizes a finite, already sufficient equation system. It is useful evidence
that different schedules can reduce arithmetic work, but not evidence for a
global absolute-degree rule across all supports or five-loop symbolic domains.
[Zeng, discussion of Figures 5–6](https://arxiv.org/html/2504.16045v2)

Smirnov and Petukhov prove finite dimension of the relevant integral quotient
space for a fixed graph. That theorem does not identify bounded-search residuals
as an independent master basis, bound the degrees needed to discover relations,
or prove termination of RustRed's selected recurrence and domain abstraction.
[Smirnov and Petukhov, Theorems 1–2](https://arxiv.org/html/1004.4199v3)

## Consequences for this implementation

The following are code-derived observations or mathematical inferences, not
additional claims of those papers.

Global absolute degree is not translation invariant across zero: `|-2|>|1|`,
but after adding 2, `|0|<|3|`. A monomial-order argument valid within one sign
cone cannot authenticate a support-crossing template. The new pre-support gate
therefore needs its actual sign-cell proof. Its conservative refusal is preferable
to a sampled or assumed-sector comparison.

There remains a separate compatibility restriction:
[exception extraction](../../crates/rustred-core/src/solver/exception.rs) rejects
fixed inactive-to-active RHS indices and excludes nonvanishing symbolic
activation faces. Finishing and original-source replay share that machinery.
The comparator may permit a lower-F activation which this policy still refuses.
This is safe incompleteness, not evidence that the identity is false. The saved
root is another deliberate scope boundary. Neither should be silently widened
as part of an order change. Details are retained in
`TMP/postlaunch-20261001/global-degree-order/GENERATION_FOLLOWTHROUGH.txt`.

Within those boundaries, the domain walker preserves an admitted changed-support
successor as Apply or Route work; absent routing remains a typed frontier. The
separate concrete `--targets` evaluator still imposes stricter literal-subsector
transitions. A domain-walk result must not imply that the latter supports every
new orientation.

## The finite-input argument and its exact limits

The frozen 116 required query rows have finite positive-power and numerator
bounds. Reading their unchanged metadata gives `max(A_cap+R_cap)=37`, attained
by `nested-d9p-a23-r14-111101101101001`. This is a conservative entry bound, not
a new geometry classification. Evidence: query SHA
`1fa81b631d35248edeb6a12c7254caf5a73a712ffa4c8e9971306a0355b9510c`,
`TMP/postlaunch-20261001/five-loop-full-physics-nohelpers-v1/queries-physics.json`.

If **every used rule and transport endpoint** satisfies `F(child)<=F(parent)`,
these entries can reach only finitely many integer keys in a fixed-arity family.
This conclusion needs all participating owners and repair layers, not merely
one regenerated owner. Existing sector-first rules fail that contract, so this
does not authorize clipping the current campaign. Equal-F routing may reset
secondary priorities; a finite key universe alone does not prove an acyclic
reduction, successful back-substitution, or efficient symbolic closure. Domain
images must preserve the inherited bound rather than independently widening A
and R. Whole-query cold coverage and work measurement remain necessary.

A narrower current-code check is encouraging. An emitted native Apply edge
fixes every sign-crossing coordinate before image construction, so its changes
ΔR and ΔD=sum(shift) are constant; ΔA=ΔR+ΔD. Explicit incoming caps A≤a and
R≤r are translated by exactly these changes, followed only by tightening.
Consequently their coarse sum is nonincreasing on an Apply edge whose all-ones
pre-support comparison proves ΔF≤0. Root derived this local property from
`owners/domains/applied/{geometry,engine}.rs` and the existing power-bound
translation; a separate agent checked the source and arithmetic. This is not
an executed whole-walker bound or a reason to discard successors. Routing,
anchor reuse, mixed old/new orders, repair layers and equal-F cycles remain
outside this local argument; the13 unbounded auxiliary starts remain unbounded.

A subsequent source audit distinguishes two issues which must not be conflated.
Admitted Route envelopes preserve finite A and R caps separately (pinches can
only tighten them), although D and coordinate envelopes can loosen. G2 residual
creation also retains the requesting A/R caps. However, containment reuse may
add an edge from a finite query Q to a larger, merely locally inspected anchor
A. Recursive closure then waits for **all descendants of A**, including regions
outside Q. Thus no newly widened geometry does not imply a finite reachable
dependency scope. `walking/epoch/g2.rs::eligible` does not require recursive
closure; both the live tracker and cold verifier follow the full anchor edge.
Evidence: `TMP/postlaunch-20261001/global-degree-order/ROUTE_CAP_NOTE.md`.
Closed-only reuse would avoid a new wait for unfinished descendants, but is not
by itself a termination argument or a demonstrated improvement. Demand-scoped
reuse is a separate candidate, not a change to the frozen production scope.

The production helper scope is stronger: among 67 auxiliary starts, 13 have
unbounded A. No finite required-root bound closes those infinite inputs. Using
their owner programs only for demand-derived descendants is a possible separate
finite-scope experiment, not equivalent coverage of the original 183 starts.
See the existing [auxiliary-scope assessment](five_loop_auxiliary_scope_2026-09-25.md)
and [finite-domain investigation](finite_physics_domain_growth_2026-10-01.md).

## One fallback if the new order is impractical

Keep ordinary source generation broad enough to cancel higher-F intermediate
columns, but seek a target-local final identity whose old-order non-descending
columns **and** F-growing columns vanish. This retains the old sector structure
and tests a stricter final-rule class; it is not equivalent to global-F-first,
which can orient lower-F terms in formerly harder supports. Reuse the existing
exact source-weight product and guard checks; an unsuccessful bounded search
remains an explicit residual. Falling back to an F-growing rule forfeits the
global-F bound and must be reported as such.

This alternative is only useful after an actual binding harmful transition is
nominated. The completed 336-row finite control produced the same seven-term
identity with and without the extra F constraint, so it did not demonstrate
that mechanism. No new sweep, unbounded helper closure claim, or implementation
is proposed by this note.

### Distinct untested combination discriminator

Normal [numeric seeding](../../crates/rustred-core/src/solver/seed.rs:107)
rejects sign crossings, and
[shared numeric search](../../crates/rustred-core/src/solver/numeric.rs:79)
requires target cases in one declared sector. It does not automatically seed
supra-sector numeric centres. However, our manual 336-row diagnostic translated
all `0,±e_i` offsets directly, including some activations at zero coordinates;
it was not wholly restricted to same-sector sources. Its objective was still
one unit target, not a weighted RHS combination.

A genuinely untested, narrow follow-up would freeze one harmful weighted RHS
block and use native Symbolica GPLU to cancel that block's difficult columns
jointly against a bounded, explicit source bank. Individual block terms need
not acquire separate reduction rules. Authenticate the complete resulting
identity with recovered original-source weights and all guards; count every
remaining endpoint. Compare against the same block reduced by installed rules,
not an unweighted endpoint union. This is not a new identity class: with the
parent relation and identical complete source bank and objective, a unit-target
solve may be algebraically equivalent. Those conditions were not established
by the earlier controls. Supra-sector source rows and final RHS
reactivation are distinct choices. No inferred master independence or automatic
canonical replay follows. This remains a proposal after the global-F pilot,
not an authorized implementation or run.

## Historical bubble dimensional recurrence: precise implementation boundary

The remembered bubble example is present in the reference FMFT implementation:
`FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop/crates/vakint/form_src/fmft/fmft.frm`,
procedure `drrG`, beginning at line527. Its formulas95/96 lower a positive
dimension offset `dp` by two. They express a bubble through three lower-dimension
bubble/tadpole contributions, with explicit index, external-invariant and
dimension-dependent denominators. This is not a hidden-zero identity or an
ordinary same-dimension routing symmetry.

This source inspection does **not** establish that RustRed's campaign already
uses dimensional recurrences. Its native integral-transport test explicitly
rejects changing dimension using a denominator-routing witness. The completed
whole-graph U/P experiments were separate diagnostics, not an installed campaign
rule source. A localized bubble/subgraph recurrence is therefore a separate
candidate: retain the dimension and denominator conditions, then eliminate any
shifted-dimension intermediate exactly before proposing a same-dimension rule.
No FORM dependency, copied topology-specific dispatcher, or new runtime rule
has been introduced by this investigation.

### Locality avoids one swell but can change the family

A focused follow-up identifies why FMFT's local strategy cannot simply be
installed as a few ordinary current-family rules. Its FG convolution uses
one- and two-loop subintegrals, tensor reduction, return from shifted dimensions,
and additional outer mass assignments generated by the rational coefficients.
These are connected parts of that method, not merely a sparse replacement for
the already tested whole-graph U/P product.
[Pikelner, FMFT, sections 2.2–2.4](https://arxiv.org/html/1707.01710v2#S2.SS2)

The connecting momentum is integrated over in the outer graph. Consequently
an uncancelled factor such as `1/p²` or `1/(p²-4m²)` is an additional propagator,
not a rational coefficient in the current d-only coefficient field. Returning
the subgraph to dimension d does not by itself return the result to the
original denominator family. Tarasov's explicit one-loop relations retain
these external-invariant and mass coefficients.
[Tarasov, section 4.5](https://arxiv.org/html/hep-ph/9703319v1#S4.SS5)

The code/input review nominated a real required four-loop rank-two bubble
point, `n=[2,1,1,1,1,1,1,1,-2,0]`, in owner `1111111100`, A9/R2/D7.
It is not yet a measured expensive rule or a native-verified shortcut. A first
probe must retain the complete numerator/tadpole sum and test exact cancellation
of all non-family outer denominators. Failure defers the shortcut rather than
discarding those terms or starting a broad new family/tensor implementation.
The detailed read-only derivation and source locations are saved in
`TMP/postlaunch-20261001/global-degree-order/LOCAL_DIMENSIONAL_RECURRENCE_NOTE.md`.
