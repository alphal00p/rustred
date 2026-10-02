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

## Five-loop transfer: finite-terminal tradeoff, not an end-to-end win

Regenerating owner17941 with the same absolute-degree prefix passes exact
original-source replay but leaves28 fixed terminal keys instead of25. The first
registered equal-terminal screen therefore stops honestly; this is not a solver
failure or a project requirement for a minimal master basis. A separate paired
test explicitly accepts the larger finite basis and retains the exact original
required query `conv-d10-a16-r6-101010000110001`, all67 owners/8,246 routes and
the unchanged repair overlay. There are no auxiliary starting queries or clipped
descendants in this small control.

| Metric | Default | One global-F owner |
| --- | ---: | ---: |
| Finite terminal keys in changed owner | 25 | 28 |
| Scheduled domains | 23,625 | 23,447 |
| Native inspections | 16,301 | 16,213 |
| Dependency edges | 252,456 | 249,488 |
| Native traversal | 6.385 s | 5.595 s |
| Owned walk | 87.101 s | 86.829 s |
| Staging + walk + cold, arm wall time | 177.022 s | 177.725 s |

Both arms pass full cold native reinspection with no uncovered obligations.
Domains decrease0.753%, inspections0.540%, but complete arm time increases0.397%.
Preparation dominates this small query; traversal alone is not the end-to-end
cost. Baseline counts also vary slightly across parallel runs, so this single
pair does not establish a robust gain. It uses a changed finite basis, not
equal-basis reduction. Cold sealing of cyclic dependency coverage is not a
proof of algebraic reduction termination.

At the nominated F10 input, successors decrease46→42 and all four F-growing
children disappear. The three new finite leaves have old reductions, each
producing six F11 children: reinserting those rows unchanged would violate the
new degree constraint. They are not proven independent masters, and none is
an immediate child of the nominated new rule. Exact whole-campaign use of the
three leaves is not exposed by this aggregate diagnostic.

The pair takes361.404s inclusive, excluding the separately retained249.213s
qualification/initial stopped screen and compilation. W16/CPUs32–47,150GB
guard, all processes drained; independent audit passes. Evidence:
`TMP/postlaunch-20261001/global-degree-order/five-loop17941-terminal-tradeoff-v1/`.
There is no campaign-switch recommendation from either loop-count control.

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

### A weaker finite bound may already exist for the old rules

Global F is sufficient for a tight degree bound, but is **not necessary** under
stronger support-transition assumptions. Root proposed, and the independent
source/math auditor checked, this conditional argument for a fresh helper-free
finite-input campaign:

1. Every applicable same-support rule has total-excess-first descent, hence
   `ΔF≤0` because `F=total_excess+support_count` on that support.
2. Every support-changing rule is a literal pinch, with no inactive-axis
   reactivation. Let `M≥0` bound the F increase of any such saved shift; the
   maximum sum of absolute components over the finite saved shifts is safe.
3. Route cannot increase positive support count, A cap or R cap. New residual
   domains retain or tighten their incoming caps.

Then the coarse domain quantity `Φ=A_cap+R_cap+M*support_count` cannot increase
when a new obligation is created. A pinch consumes at least one unit of support
count, paying for at most M degree growth. References to already-created anchors
preserve the global maximum bound even if they do not preserve the requesting
node's individual bound. For the finite required inputs a safe global bound is
therefore `F≤max_initial Φ≤37+15M`. Global-F rules could reduce this coarse
ceiling to37, potentially an important improvement, rather than creating
finiteness from nothing.

This is not yet a theorem about the actual saved campaign. The domain Apply
API permits support swaps and lower-count activations inside a wider saved
root; its support-transition tests explicitly cover these. Ordinary generation
and source replay instead require the relevant activation exclusions. The
67 actual programs **and** repair overlay must satisfy that corpus-level
contract and the E-primary condition. No complete corpus audit or measured M
was run here. An unbounded starting helper or imported unbounded anchor also
invalidates the finite-input premise.

Even an established finite key universe can be enormous; equal-degree routing
cycles and worklist behavior remain separate issues. This supplies neither a
practical ETA nor missing reductions. It does correct an overstrong inference:
seeing F grow at a pinch is not, by itself, evidence of an infinite reachable
key space. No rank clipping or extra certificate implementation follows.

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
canonical replay follows.

The first native version of this discriminator has now run after the global-F
control. At the actual owner481/rule151 point, it retains the45 non-growing
siblings and jointly targets the24 F-growing terms. Its fixed47 centres/752
ordinary source rows yield1,322 columns, of which1,240 are forbidden. Symbolica
finishes elimination in0.496s, but the target marker does not pivot: this bank
does not contain the requested eliminating combination. All source and original
parent guards/denominators are retained. Native preparation plus algebra takes
0.899s; the owned process harness takes5.969s. Compilation is separate.
This is a bounded negative result, not irreducibility or a proof that larger or
differently chosen banks cannot work. No source-bank expansion or rule
publication is implied. Evidence:
`TMP/postlaunch-20261001/global-degree-order/joint-combination-v1/probe-r3/`.

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

### Local bubble follow-up: compare structure, not only term count

The original-family obstacle is not necessarily fatal: an outer denominator
such as `p²=D4+1` can sometimes be cleared to obtain a polynomial-weighted
integral identity, then oriented as a rule. This changes the equation being
solved; it does not license treating `1/p²` as a coefficient independent of the
remaining loop momenta. A hand-derived rank-two bubble example at the actual
required FG point `[2,1,1,1,1,1,1,1,-2,0]` has 27 formal RHS keys, all F≤10,
against the parent's F11. It retains massive tadpoles and a d−1 denominator.
It is not yet a native source-proved or parametric rule.

Native inspection of the installed rule9 at that same point gives 26 distinct
uniform successors after nine exact zeros: 20 at F10, one at F9 and five at F8.
Thus neither term count nor maximum degree establishes an improvement. However,
all 26 existing endpoints retain the mixed bubble numerator, whereas the
prospective identity removes it from 26 of its 27 endpoints. Its hand-counted
F histogram is two/three/nine/seven/six terms at F10/9/8/7/6. That locality and
degree-distribution difference is a plausible mechanism worth an exact proof
test, not a measured saving. The actual baseline inspection completed and
drained in2.063s; no proposed replacement or descendant comparison ran.

The next gate was full native original-source-span equality with d left generic,
including every physical column and denominator. The hand derivation uses
translation/reflection and scaleless polynomial identities; a finite ordinary
IBP bank need not contain their consequences. A failed span test therefore
rejects that bank, not the tensor identity. Any added symmetry or zero relation
would need its own existing native authority, not an unlabelled source row.
Evidence: `TMP/postlaunch-20261001/global-degree-order/LOCAL_DIMENSIONAL_RECURRENCE_NOTE.md`
and `local-fg-point-v1/execution-r1/` beneath the same evidence directory.

That exact test now has a useful positive result, but not for the shortest row.
Using252 ordinary IBPs on84 explicit source points, Symbolica proves the
**unreflected** angular identity with147 nonzero original-source weights and
an empty residual. Dimension remains symbolic. Both a full augmented product
and an independently formed ordinary-only product reproduce the complete row.
No change-of-variables or zero-sector identity is assumed. The reflected
27-RHS version leaves67 residual terms in this fixed bank; it is not accepted
as an ordinary-source consequence, nor disproved by that bounded miss.

The proved relation has37 nonzero RHS keys. At F6/7/8/9/10 it has10/10/12/3/2
terms;36 remove the mixed numerator, while the one remaining term pinches an
outer line. The target coefficient is4(d−1), retained as a normalization
condition. This is a fixed-index identity, not yet a parametric case rule.
Its descendant-union comparison against the installed26 terms is the next
work gate (completed below). The exact diagnostic costs0.121s native/1.355s inclusive; cached
adapter compilation costs69.336s separately. Evidence:
`TMP/postlaunch-20261001/global-degree-order/local-fg-source-proof-v1/probe-r1/`.

This identifies a concrete connection to local dimensional methods without
adding a shifted-dimension runtime: a local angular consequence can already
be expressed through ordinary same-dimension IBPs. Whether that consequence
is cheaper to use, and whether it lifts to a useful broad case, are separate
questions. The implementation still has no new tensor reduction service.

### Completed local-identity work test

Two counterbalanced pairs use the same frozen16-owner/508-route pool, starting
from the complete saved26 or proved37 successor sets of that exact input.
Nothing is clipped, no new rule is installed, and all starts and native domains
are independently cold-reinspected. All four arms finish without frontiers or
uncovered obligations, using16 workers on CPUs32–47 and the same150GB guard.

| Metric | Saved rule, pair1 | Proved identity, pair1 | Saved rule, pair2 | Proved identity, pair2 |
| --- | ---: | ---: | ---: | ---: |
| Domains | 13,931 | 10,068 | 13,974 | 10,004 |
| Native inspections | 10,082 | 7,869 | 10,146 | 7,905 |
| Successor admissions | 422,656 | 203,242 | 424,988 | 204,431 |
| Native traversal (s) | 1.465 | 0.803 | 1.423 | 0.849 |
| Guarded walk+cold phase sum (s) | 5.081 | 4.375 | 5.537 | 4.353 |

This is a substantial **local** work reduction: about28% fewer domains,22%
fewer inspections and52% fewer successor admissions despite the larger RHS.
All four arms together take21.259s inclusive of setup/report/drain; the earlier
proof/adapter-compilation project is separate. Sampled RSS varies and does not
establish a memory improvement. The comparison excludes constructing/applying
the parent shortcut itself and does not run the unchanged58-query scope.

It therefore justifies an exact parametric-lift experiment, not a production
restart or claimed five-loop speedup. The six outer indices may be freed only
after checking the full original-source product and retained conditions with
those variables still symbolic. A fixed-point identity is insufficient for
that claim. No reflected-row authority or new tensor engine follows.
Evidence:`TMP/postlaunch-20261001/global-degree-order/local-fg-endpoint-union-v1/`;
independent source, measurement and drain audits pass.

### Exact six-outer-index lift

The next gate has now passed. Reusing the147 recovered source weights and
regenerating the full original shifted IBPs yields the identical38-term
homogeneous relation with **all six outer powers symbolic**. The bubble powers
remain one, the mixed numerator is quadratic, and the other ISP power is zero.
Dimension stays generic. All2165 source terms are restored before partial
specialization, including1268 that vanished at the original point; conditions
are retained rather than inferred from a sampled equality.

Freeing the bubble powers as well leaves74 residual keys with those same
weights. This is a bounded negative result for that lift, not an obstruction
to all broader rules. The primary exact product costs0.234s native within a
1.378s owned probe; cached optimized-adapter compilation costs68.621s separately.
Independent source and actual-output audits pass. Evidence:
`TMP/postlaunch-20261001/global-degree-order/local-fg-partial-lift-v1/`.

The existing work reduction still measures one fixed input's successor union.
This symbolic identity has not been installed as a parametric case: native
descent, applicability, case ownership and replay/publication must still be
checked. In particular the current canonical-forward-pivot replay path does
not automatically accept every independently proved source combination.

A structurally identical bubble appears in required five-loop owner18910 at
`[0,1,2,2,1,0,1,1,1,-2,0,1,0,0,1]`, with ell=k2,p=k3,q=k4 and the six
Gram coordinates all represented directly by existing denominators. The native
five-loop transfer now passes: all147 weighted original sources are regenerated
in the15-index family and their full2694-term product equals the38-term
identity, both at this point and with all seven outer powers symbolic. All1797
point-zero terms and denominator records are retained before testing the lift.
This is new five-loop exact-source evidence, not an assumption of family
isomorphism. Probe0.981s native/2.358s inclusive; cached adapter compilation
74.912s separately. Source and actual-output audits pass.

The saved native rule35 emits64 uniform nonzero successors after31 exact zeros,
all retaining the mixed D10 numerator. The proved37 terms remove it from36;
their R0/1/2 counts are31/5/1, against saved R1/2 counts10/54. This justifies a
full descendant-union comparison with the same67-owner rule pool. It does not
yet establish five-loop work savings or publish a case rule. Evidence:
`TMP/postlaunch-20261001/global-degree-order/local-five-point-v1/` and
`local-fg-five-loop-transfer-v1/`.

### Completed five-loop counterbalanced work test

Both separately bounded pairs now finish with full cold reinspection of every
input and native domain. They use the same67-owner/8246-route pool and overlay,
16 workers on CPUs32–47, fresh graphs, no descendant clipping, and the same
frozen release executable. These are the complete successor unions of one
required physical input, not the entire116-query campaign.

| Metric | Saved64, pair1 | Proved37, pair1 | Saved64, pair2 | Proved37, pair2 |
| --- | ---: | ---: | ---: | ---: |
| Domains | 143,218 | 105,721 | 139,739 | 106,690 |
| Native inspections | 106,640 | 76,980 | 103,380 | 77,614 |
| Successor admissions | 11,107,100 | 7,382,357 | 10,643,049 | 7,410,507 |
| Owner preparation (s) | 76.590 | 76.409 | 66.416 | 71.871 |
| Native traversal (s) | 51.884 | 32.674 | 55.415 | 38.970 |
| Guarded cold verification (s) | 128.831 | 150.773 | 136.836 | 131.829 |
| Guarded walk+cold sum (s) | 262.544 | 264.396 | 264.176 | 247.315 |

The repeated result is23.65–26.18% fewer domains,24.92–27.81% fewer native
inspections,30.37–33.53% fewer successor admissions, and29.68–37.02% less
forward traversal time. Native Ready scheduling changes exact counts slightly
between runs; the direction and size of the local work gain survive reversal.
Observed maximum descendant rank is5 rather than6, with no unbounded-rank
domains in either arm. This is an observation, not an imposed rank cutoff.

Whole walk+cold performance is mixed:0.71% slower in pair1 and6.38% faster in
pair2. No substantial complete-lifecycle speedup or consistent memory gain is
established. The cold native-visitor tally includes verifier callback/coverage
work and is not a pure CAS or CPU profile; its variable cost has not been
isolated. Both full pair entries take532.655s and517.112s respectively, including
setup/report/drain. Source-proof compilation/materialization and parent rule
application remain separately excluded from this fixed-program endpoint test.

All roots and native inspections pass the independent cold check, with zero
frontiers, uncovered obligations or reported violations, and all owned process
groups drain. Native worklist exhaustion and sealed-cycle coverage are not a
new global descent/termination theorem. No new case rule has been installed,
and other required inputs may still demand portions of the avoided graph.
The next meaningful gate is generic source construction and checked integration,
then unchanged whole-cohort tests—not a production restart based on this table.
Evidence: `TMP/postlaunch-20261001/global-degree-order/local-five-endpoint-union-v1/`.

## Revisited triangular-rule literature: what is still different

Liu and Mitov's triangular construction explicitly limits numerator weight in
lower sectors, rather than accepting any pinch as automatically cheap. It also
orders equal-weight terms towards a selected coordinate. Their discovery uses
shifted ordinary IBPs, separates forbidden and allowed columns, and checks
rank before recovering coefficients. Their stated triangular setup fixes
positive indices and treats abstract indices as nonpositive; propagator dots
in our frozen requests cannot simply be ignored.
[Liu and Mitov, sections II.5–III.3](https://arxiv.org/html/2512.05923v1#S3).

RustRed already has the essential sparse search/replay mechanism. The remaining
experiment is the shape of the allowed endpoint set, not another elimination
kernel. Global F permits a lost propagator power to pay for a numerator increase;
the paper's lower-sector restriction is stronger in that respect. Conversely,
such a restriction may leave no row in a bounded bank. Neither formulation
proves the current mixed-order campaign terminates. The 752-row joint probe
already tested one stricter intersection of old-order descent and non-growing
F, unsuccessfully. Before enlarging it, use an actual local mechanism and
measure complete descendant work, with every coefficient and guard retained.
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

The initial code/input review nominated a real required four-loop rank-two bubble
point, `n=[2,1,1,1,1,1,1,1,-2,0]`, in owner `1111111100`, A9/R2/D7.
At nomination it was neither a measured expensive rule nor a native-verified
shortcut. The completed unreflected proof above now supplies fixed-point
ordinary-source authority with the complete numerator/tadpole sum retained;
full-cohort work savings and parametric publication remain unestablished. An uncancelled
non-family outer denominator would still defer a shortcut rather than license
discarding terms or starting a broad new family/tensor implementation.
The detailed read-only derivation and source locations are saved in
`TMP/postlaunch-20261001/global-degree-order/LOCAL_DIMENSIONAL_RECURRENCE_NOTE.md`.

## Two concrete follow-ons, not another full elimination framework

Smith's 2026 LoopIn description separates each integral shift's constant and
index-linear coefficient components into columns before row reduction over the
kinematic/dimension field. It follows with target-local symbolic solves where
necessary. This is different from merely changing coefficient variable order.
[LoopIn, section 3.1, equations 13–17](https://arxiv.org/html/2602.19909v1#S3.SS1).

Code inspection finds RustRed's current source preconditioner instead uses
index-polynomial GCD-scaled elimination. That can increase index degree; the
component-wise alternative preserves affine index dependence in that phase.
The narrow experiment is a source-basis comparison using Symbolica's existing
sparse reducer and exact original-source weights. Measure coefficient degree,
exceptional guards and subsequent complete descendant work, not just row count.
Retain originals where elimination scales vanish. No competing CAS kernel or
new preconditioner has been implemented for this proposal.

A second code-derived opportunity makes the local angular construction
automatic. For bubble loop ell and directions ell,q,p, native denominator
derivative contractions produce, up to an overall factor,

`M = [[z,v,w], [z-w,v-r,w-s]]`,

where z=ell², v=q·ell, w=p·ell, r=q·p and s=p². The signed two-by-two minors
give the tangent vector's three polynomial coefficients, up to overall sign.
This is a tiny family-derived polynomial null vector, not a topology-name rule.
Multiplying by v stays within the degree-three source bank already tested.
Symbolica must perform polynomial/minor arithmetic and exact tangency checks.
The falsifier is failure to recover a useful exact identity or a net workload
benefit; a degenerate matrix is not evidence that no such identity exists.

The compact-Landau work provides a broader determinant-based construction, but
its stated completeness conditions and one-/two-loop scattering examples do
not establish a turnkey massive-vacuum closure method. The useful immediate
connection here is source construction, not a new completeness theorem.
[Coro et al., sections 2–3](https://arxiv.org/html/2607.06365v1#S2).
The component-wise source-basis comparison subsequently completes with no new
structural benefit on the raw16-source fixture; see the measured result below.
The cofactor diagnostic also passes below; neither is a new tensor reducer or
an installed shifted-dimension lane.

The older general foundation is also useful: logarithmic vector fields tangent
to propagator hypersurfaces constrain unwanted dot growth, and Gram-determinant
syzygies can remove dimension shifts. Böhm et al. prove completeness for their
specified Gram syzygy generators, not completeness of every resulting reduction
strategy or termination of our symbolic-domain campaign.
[Böhm et al., sections III–V](https://arxiv.org/html/1712.09737v2#S3).

For the narrow next experiment we need less machinery: find a polynomial vector
that annihilates selected denominator derivatives identically, using the existing
family derivative-contraction matrix and Symbolica arithmetic. This stronger
local tangency condition avoids differentiating those denominator powers at all.
It may survive arbitrary bubble powers even when the subsequently simplified
unit-power37-RHS identity does not. That is a new row to derive and verify, not
permission to generalize the failed frozen-weight lift. The successful native
identity check below still supplies no completeness theorem or workload result
for the new row.

For a symbolic numerator power, the direct vector (without the extra v
multiplier) gives a particularly small proposed source. Write
`B=s*v^2-2*r*w*v+(r^2-s*t)*z+t*w^2` and let b be the power of
`Dnum=z+t-2v-1`. Then the divergence numerator is
`(d-2)*(s*v-r*w)+2*b*B/Dnum`. Its highest Dnum term has coefficient
`s*(b-d+2)/2`. With target power a=b-1 and active `s=Dactive+1`, this is a
candidate target coefficient `(a-d+3)/2` plus a companion whose active power
is lowered. One must keep that companion integral: dividing by s would change
the family. At a=-1 the b=0 contributions must cancel natively, not be removed
from the general symbolic source beforehand.

This derivation received independent mathematical review before the automatic
native construction below. Index-uniform native descent and work benefit remain
pending. The generic diagnostic uses family-derived signed minors, monomial
coefficients as original-source weights, and complete translated ordinary
IBPs. It does not import the147 coefficients of the successful local example.
Other ell-dependent numerator coordinates remain fixed to zero in this first
case; arbitrary such numerators are not silently covered.

The unit-mass vacuum setting helps orient this identity: the constant term in
`p²=Dactive+1` supplies a scalar target coefficient. With a general mass it is
the corresponding mass-squared constant; in a massless specialization that
coefficient may vanish. The identity can remain true while the proposed rule
orientation fails. A generic implementation must retain the mass/normalization
guard or refuse that orientation, not introduce `1/p²` as a field coefficient.

The longer-term workload hypothesis is local structure removal, not a new
global degree bound. A tangent vector can avoid increasing protected positive
powers while reducing a troublesome numerator. Pinches may still expose other
numerators and must remain in the output. The earlier helper-free R0 control
does not prove that arbitrary numerator inputs can be brought into that same
bounded R0 scope. First measure the supported local cases, then the unchanged
combined four-loop and representative five-loop requests; do not infer whole
campaign closure from this mechanism.

### Automatic native tangent-source result

The input-driven constructor now passes on the registered five-loop owner.
From the nominated loop, two protected propagators and three contraction
directions, it forms the signed matrix minors using Symbolica, verifies exact
tangency, and expands them into ordinary-source translations. This automatically
constructs the identity from family data; it does not yet discover all eligible
local subgraphs or import the earlier147 proof weights.

The primary vector requires42 translated sources and782 original terms.
Its full symbolic source product equals an independently assembled product-rule
row with zero residual, before index specialization. Fixing only the other five
inactive indices to zero gives47 homogeneous terms and46 prospective RHS terms.
Both bubble powers, all outer powers, dimension and the selected numerator
power remain symbolic. Its target coefficient exactly equals
`-2*(n9-d+3)`, where n9 is the tenth denominator's index. All source denominators
and this normalization condition are retained.

At the registered rank-two point the46 RHS terms have rank0/1/2 counts25/20/1,
versus10/54 rank1/2 terms for the saved64-term row. All have F<=12 below parent
F13;8 remain on the same support,38 pinch, none activates an absent propagator.
At the separate rank-one boundary the primary identity yields15 RHS terms,
no activation, F<=11 below parent F12, and target coefficient2(d-2). The
potentially troublesome zero source-numerator power is handled by exact native
cancellation, not an omitted derivative term.

Multiplying the vector by the nominated scalar product also passes full
source-product equality:123 translated sources,2217 original terms,56 RHS at
rank two, target1-d. This larger secondary row is not presumed preferable.
Its rank-one output is only an algebraic diagnostic outside its declared fixed
rank-two case; support activations there are not accepted as a reduction.

The complete diagnostic costs1.021s native/2.345s inclusive, with72.425s cached
optimized-adapter compilation separately. Two earlier setup failures are
preserved: a Rust iterator borrow error, then an expected-expression parser
using a bare index name instead of the context's private index symbol. The
correction constructs only that expected comparison through native typed APIs;
neither source construction nor its exact equality checks changed.
Evidence: `TMP/postlaunch-20261001/global-degree-order/local-tangent-source-v1/`.

This is broader exact identity evidence than the successful fixed-numerator,
unit-bubble37-RHS lift. It is a different row: the measured37-RHS descendant
savings must not be assigned to it. Concrete point descent is also not the
unbounded native case proof. Existing `PreparedOriginalDomain` can recompile
explicit original-source weights, retain guards and prove full cell descent;
the remaining implementation is a small producer and its candidate-owner
integration, with ordinary rules retained outside the admitted case. The current
generated-owner overlay path expects a canonical elimination row, so merely
injecting this independently valid combination there is not legitimate.

The next performance gate is unchanged whole-cohort coverage with an installed
checked rule, not another claim from fewer immediate successors. Until that
gate, production and its owner pool remain unchanged.

### Generic implementation and exact local-domain gate

The prototype has now become `identity::TangentSourcePlan`: a family-derived,
loop-count-independent two-protected-denominator/three-contraction capability.
Its signed minors, polynomial arithmetic and exact tangency checks use native
Symbolica operations. The nominated geometry is input, not a topology-name
dispatch. It retains unprotected denominator derivatives, external contractions,
unequal masses, native power shifts and all original source conditions.

`foundry::artifact::check_original_source_combination` supplies the complementary
admission path. It regenerates the explicitly weighted ordinary rows and checks
the entire requested coordinate box, using the existing original-source and
sign-cell machinery. It does not ask canonical elimination to rediscover the
combination, import a guessed zero sector, install an owner or claim closure.
Unsupported geometry/order capabilities and any failing cell reject the request.

The public four-loop integration test passes on the common input basis. It fixes
only the other inactive index to zero, allows arbitrary positive bubble and
outer powers, and allows the selected numerator index to range over all negative
integers. Exact native admission covers486 sign cells in0.256s; the complete
two-test body takes0.31s, with82,944KiB peak single-child RSS. These are local
correctness diagnostics, not a timed generation campaign. The six construction
tests also pass, including external momenta, unequal masses, nonzero power
shifts, optional multipliers, full product-rule comparison and invalid inputs.

The separate producer suite currently records10/11 passing: the raising-rule
case correctly rejects its non-descending shift, but its error-text assertion
needed correction. That corrected assertion is reviewed; execution is pending.
Earlier private-constructor and text-label mistakes in root's public fixture
are retained in the receipts and were fixed without changing the identities.
Evidence: `TMP/postlaunch-20261002/tangent-integration/`.

The app priority bridge now passes independent source review and a five-check
public-API smoke test, including guard refusal and unchanged fallback/terminals.
It exports the same checked broad row into a fresh owner and rejects any source/
normalization guard the runtime cannot retain. Original-row proof is distinct
from canonical seeded-source replay: this is an experimental candidate owner,
not a certified closing artifact. The private app unit suite has not executed.

### Integrated four-loop result: useful capability, unsuccessful priority policy

The actual donor export adds one rule (16 to17), keeps its one finite terminal,
and grows the binary from49,027 to51,196 bytes. The complete declared box passes
486 original-source/descent cells. Export costs3.529s inclusive, after123.732s
adapter compilation. Existing app candidate transport supports1..16 indices;
this inherited codec/loader limit is not a mathematical loop-count restriction
of the new family-generic source constructor.

A fresh, matched W16/CPU32–47 comparison uses the same frozen CLI,16 owners,
508 routes and all58 historical required queries, replacing only the one owner.
Both arms exhaust their worklists with zero frontiers and pass cold full native
reinspection for32 distinct initial roots. The checker permits sealed dependency
cycles; its PASS is not a new global descent/termination theorem.

| Metric | Saved pool | Broad tangent priority |
| --- | ---: | ---: |
| Scheduled domains | 26,025 | 26,659 |
| Native inspections | 17,957 | 18,007 |
| Events | 872,486 | 874,615 |
| Preparation, seconds | 1.085758 | 1.079235 |
| Native traversal, seconds | 4.108112 | 4.130033 |
| Cold verification, seconds | 7.610162 | 7.661685 |
| Complete arm, seconds | 15.271476 | 15.315056 |

This fails the registered work/performance gate:2.44% more domains and no timing
improvement. No reverse repetition or production promotion follows. The earlier
37-RHS fixed-numerator identity's positive endpoint-union results must not be
assigned to this broader46-RHS rule. A lower numerator or fewer immediate terms
does not itself guarantee less joint routed-domain work. Evidence is in
`TMP/postlaunch-20261002/tangent-integration/whole58-v1/forward/`; all native
phases drained, total comparison31.031s, with prior compilation/export separate.

The next distinct algebraic candidate examined is a weaker constraint: require
`V(D_i)=D_i*h_i`, not `V(D_i)=0`. Differentiation still does not raise protected
propagator powers, but a loop touching three active lines may admit new vectors.
The bounded proposed test has affine vector/h coefficients (77 unknowns for the
selected four-loop input), proposed for a Symbolica solve followed by full
original-row replay and actual descendant comparison. Independent analytic
preflight subsequently rejects this particular ansatz, as shown below. The
mathematical motivation is the additional
no-doubled-propagator constraint in
[Böhm et al., section IV](https://arxiv.org/html/1712.09737v2#S4.SS1).

### Cheap analytic veto of the proposed degree-one logarithmic search

With a single differentiated momentum ell, let its four vector coefficients be
affine polynomials in all independent scalar products over Q(d). Already the
two denominators `ell²-1` and `(ell-p)²-1` force that vector to zero. Set
`z=ell²`, `x_j=ell.r_j`, where the three spectators are p,q,r. The first
divisibility condition has the complete affine solution

```text
f_j = a_j*(z-1) + b_j + sum_k C_jk*x_k,  C^T = -C
f_0 = a_0*(z-1) - sum_j b_j*x_j
V   = f_0*ell + sum_j f_j*r_j.
```

Independent spectator-Gram coefficients exclude dependence on those variables.
On the second denominator's zero set, the independent p.q and p.r coefficients
force f_q=f_r=0. With t=z-1 and w=ell.p, the remaining condition is
`(a_0*t-b*w)*(t+1-w)+(a_p*t+b)*(t-w)=0`. Its w² coefficient gives b=0;
its t² and t coefficients then give a_p=a_0=0. Adding a third protected line
cannot repair this empty ansatz. No compilation/native solve was spent on it.

This does not reject higher-degree logarithmic vectors, massless/special Gram
settings, multiple differentiated loops, or a different derivation space such
as Baikov coordinates. Treating spectator scalar products as rational field
coefficients would change the problem and introduce new denominator obligations.
There is no automatic degree escalation after this negative result.

The next practical candidate is therefore the previously proved narrow37-RHS
combination. Its intermediate combination weights have poles such as d−2 that
cancel from the final RHS. Existing generic-coefficient-field semantics treat
these as nonzero units; they are not necessarily physical/index exceptions.
The implemented correction uses existing typed guard origins to separate
weight-only poles from genuine source hypotheses, caller-retained conditions
and final coefficient poles. All remain in proof provenance. Index-dependent
guards and mixed origins are not discarded. The existing public bridge smoke
passes, including refusal of a genuine additional condition; the real147-source
export supplies the positive weight-only case. Release core/app test sources
compile; the private classifier unit tests have not separately executed.

### Narrow37 rule: full native export passes, combined-workload benefit fails

The complete original product replays147 contributions/2,165 terms, including
1,268 terms that vanished at the old fixed sample. It verifies36 sign cells
over the declared six-free-index box, with four indices fixed. Export costs
0.242s inside the adapter (2.966s owned invocation), after124.359s adapter
compilation. These are export/check timings for an already discovered identity,
not generation or closing-campaign timings. The experimental payload grows
49,027 to52,106 bytes, adds one rule and preserves the old16-rule suffix and
single terminal. It is not a new full-owner certificate.

The unchanged whole58 cohort, with the same16 owners/508 routes and W16,
passes cold full reinspection in both arms. Nevertheless:

| Metric | Saved pool | Narrow37 priority |
| --- | ---: | ---: |
| Scheduled domains | 26,025 | 27,696 |
| Native inspections | 17,957 | 18,264 |
| Events | 872,486 | 909,620 |
| Traversal, seconds | 4.167153 | 4.366214 |
| Complete arm, seconds | 16.301522 | 16.548076 |

The extra1,671 domains comprise1,364 aliases and307 native inspections.
Routed admissions rise7.24% versus1.17% for RHS admissions; major G2 counts
are unchanged. This is a negative priority-policy result, not a successful
whole-workload optimization. No reverse repetition or production rollout.
The local endpoint pilots bypassed parent matching and kept original programs;
the integrated test partitions a narrow case and changes the recursive program.
Those are different interventions. Complement splitting is a plausible cost,
but these counters alone do not prove it caused the complete regression.

Existing partial-rule overlays are not a no-split alternative: they append
after baseline fall-through and use the same partial-intersection splitting.
An opportunistic rule used only on wholly contained input domains would require
a new explicit policy and corresponding cold/checkpoint semantics. No such
policy or benefit is claimed here. Evidence:
`TMP/postlaunch-20261002/tangent-integration/replayed-147-whole58-v1/`.

### Conditional physical invariant: a separate geometry question

A dots-first argument is false for the saved pool: an actual same-support rule
maps(A,R)=(5,5) to(6,4). A possible replacement is F+M*S, where F=A+R and S
counts active denominators. Under the common uncut order, same-support terms
have nonincreasing F; a strict support drop pays for an increase at most the
finite saved shift's L1 norm. Choosing M above every such norm therefore works
IF nonzero terms never reactivate a pinched denominator and routing respects
the same potential. This premise needs a whole-pool, guard-aware check; raw
positive shifts on inactive axes are not witnesses when coefficients vanish
on that activation face or the saved rule excludes it.

Even a valid concrete invariant does not authorize clipping widened abstract
domains. Separate caps on A and R do not represent their sum correlation;
queue, reuse and cold verification would need to agree on the restricted scope.
The attempted complete check of the original four-loop rule corpus uses the
existing native guarded-application CLI, without new algebra or a solver build.
It stops at the first of523 rules: an excluded n2=1 sign face is specialized
before the lazy guard is discharged, producing `ZeroDenominator` for2(n2−1).
This fail-closed diagnostic limitation does not affect the separately verified
ordinary dispatcher, which resolves exceptions before selecting a rule. The
census completes0/523 rules and is inconclusive, not a counterexample or proof
of the proposed invariant. No invariant-aware geometry, bound for unbounded
helpers, termination theorem or performance gain has been implemented/inferred.

### Component preconditioning: exact equivalence of source spans, no structural gain

The bounded diagnostic compares the16 unshifted ordinary IBPs of the same
four-loop owner `1111111100`: raw, RustRed's existing GCD-preconditioned basis,
and a component-wise basis over the kinematic/dimension coefficient field.
It uses Symbolica's rational-polynomial sparse reducer, not a new algebra
kernel. Constant and ten index-linear components yield880 columns from80
integral shifts. Identity columns retain the complete source weights; exact
products, mutual span checks and a rank16 transformation all pass.

At the nominated physical point `[2,1,1,1,1,1,1,1,-2,0]`:

| Metric | Raw | GCD | Component |
| --- | ---: | ---: | ---: |
| Symbolic terms | 264 | 564 | 564 |
| Maximum index degree | 1 | 1 | 1 |
| Nonzero point terms | 204 | 439 | 439 |
| Terms harder than the parent | 81 | 67 | 67 |
| Terms increasing F | 56 | 21 | 21 |

The two reduced bases have the same16 distinct leading shifts, every full
shift inventory and all these point metrics, in reversed row order. We did
not prove proportionality of their coefficients and do not infer it from
printed expressions. On this bank the existing preconditioner already avoids
the index-degree growth that motivated the experiment. This is a structural
negative, not a failed span calculation or a verdict on the complete LoopIn
method: its preceding syzygy construction is not part of this test.

There is a limited structural explanation for the negative result. In the
ordinary vacuum IBPs, a nonzero shift `+e_i` or `+e_i-e_j` carries a common
index factor `n_i` down its column. Cancelling such a pivot column can therefore
use scales from the base field rather than introduce further index powers.
The zero-shift column is different. This explains how GCD reduction can already
preserve degree one on this fixture; without a recorded full pivot trace it
is not a proof that every elimination followed that mechanism. Translated or
syzygy-generated source banks need not share the premise.

The optimized adapter compiles in69.290s; the isolated probe takes1.348s
inclusive, with0.124s inside the adapter. These are diagnostic timings, not
generation, application or closure timings. Different checks inside the GCD
and component phases prevent interpreting their ratio as solver speedup.
An independent audit verifies the receipts, exact checks and drained jobs.
Evidence: `TMP/postlaunch-20261002/tangent-integration/component-precondition-v1/`.
The experiment is parked without solver integration or another workload run.
Reopen only with observed index-degree growth or a justified new source bank;
do not enlarge the bank merely to evade a negative discriminator.

## Selective application: preserve the baseline partition

The failed priority insertions motivate a different application policy, not
another source bank. A whole-input-only shortcut initially looked attractive,
but all four original queries for owner `1111111100` cross the narrow37 case.
Its R2 descendants are also contained by an already requested R12 anchor and
can be reused before native inspection. This is not a proof of zero possible
calls: G2 can inspect a restricted residual without queue re-admission. No
whole-input policy was implemented on the strength of that uncertain opportunity.

A more direct screen uses the baseline's own partitions. The unchanged four
queries produce180 selected-rule pieces and three terminals. Each selected
piece is then sent, unchanged, to the existing native matcher with each checked
donor. A hit requires one output with precisely the same box, rank and A/D
predicates, selecting the donor rule. Separately require no split, refinement
or empty-cell removal. All other results remain diagnostic; no candidate cuts
are adopted and no terminal/gap outcome is replaced.

| Original input | Baseline selected pieces | Broad46 direct hits | Narrow37 direct hits |
| --- | ---: | ---: | ---: |
| Required R5 anchor | 46 | 1 | 0 |
| Physical D8/A13/R5 | 43 | 17 | 0 |
| Physical D7/A11/R4 | 45 | 18 | 0 |
| Required R12 anchor | 46 | 1 | 0 |
| Total occurrences | 180 | 37 | 0 |

All three matcher calls complete, with no unresolved results, in8.736s inclusive
and below94MB sampled process-tree RSS. Independent rejoining of the native
outputs confirms every count and drained process. These are overlapping-query
piece occurrences, not integral volume, actual walker hits or performance gains.
The existing whole58 cold ledger further shows that both physical queries are
absorbed by the R5 anchor. Thus35 of the37 hits are not additional native root
opportunities. Only two screened anchor hits remain; descendant applications
and an advantage over the baseline RHS still need measurement.

One example explains the distinction. Baseline rule15 emits the domain with
eight active powers equal to1, numerator coordinate `x8>=1`, `x9=0`, and retained
rank at most5. Broad46 applies to the whole piece. Narrow37 instead cuts it
into `x8=1`, `x8=2` and `x8>=3`, needing an extra partition. This supports an
**after-baseline-partition** experiment for broad46: preserve every ordinary
cut and consider a proven alternative only on a whole already-selected piece.
Fallback must leave that piece unchanged; terminals, zeroes, gaps and unresolved
outcomes must never be converted into apparent rule coverage.

The implementation design uses explicit persisted dispatch semantics, shared
by warm inspection and cold reinspection. Alternatives belong to the same
prepared batch as the selected baseline rule, preserving immutable overlay
precedence. Concrete point evaluation must also select a baseline before trying
alternatives. Uniform case/equality/exclusion/denominator checks remain mandatory;
unknown/partial applicability declines the shortcut, and actual application
still checks all RHS obligations and descent. Implementation now passes nine
native core tests and119 candidate-bundle tests, including codec roundtrips,
cold reference-Off reinspection and policy-only checkpoint-change rejection.
The first cold/checkpoint attempt stopped at an omitted Epoch scheduling option;
the corrected fixture passes without weakening its assertions. Two unrelated
external-data tests remain ignored. The whole58 result below is valid but does
not pass the performance gate.
Evidence: `TMP/postlaunch-20261002/tangent-integration/after-baseline-partition-v1/`.

### Measured result: nonfragmenting broad46 is not a workload improvement

The fresh typed policy export reproduces the same42 original sources,46 RHS
terms and864 retained conditions, preserving the16 original fallback rules and
one terminal. The51200-byte candidate is new v3 output, not relabeled eager
priority bytes. Native proof/export takes0.341s; the owned probe takes3.158s,
separate from122.754s adapter compilation and755.421s shared library/CLI build.

On the two previously screened anchor faces, exact Symbolica specialization
leaves41 coalesced baseline terms versus46 alternative terms, with77 nonzero
coefficient differences. Thus the new rule is not merely the old row rewritten.
This comparison leaves the numerator index symbolic and retains its guards; it
is not a new proof of applicability on the entire ray.

Both complete four-loop arms use the same freshly optimized executable, all
16 owners/508 routes/58 required queries,32 admitted roots, W16/CPUs32–47, and
fresh graphs. Both cold-All/reference-Off checks pass with zero violations,
frontiers, uncovered cases or errors; all owned subprocesses drain.

| Metric | Baseline | After-baseline broad46 |
| --- | ---: | ---: |
| Scheduled domains | 26,025 | 26,211 |
| Native inspections | 17,957 | 17,970 |
| Events | 872,486 | 873,448 |
| Native traversal seconds | 4.261940 | 4.218705 |
| Complete arm seconds | 15.508818 | 15.795820 |

The31.763s forward pair is a negative promotion result: domains increase0.715%
and complete arm time increases1.851%. The1.015% traversal difference is not a
robust speedup. No reverse pair or five-loop transfer is warranted. The generic
opt-in policy remains tested, but this candidate is parked. Eliminating extra
partition cuts alone did not convert the local identity into a useful global
rule. Evidence: `tangent-integration/after-baseline-owner-v1/{compile-r1,probe-r1}`
and `tangent-integration/after-baseline-whole58-v1/forward/receipts/RESULT.json`
under `TMP/postlaunch-20261002/`.

### Summed recurrences: promising mechanism, incompatible shortcut as stated

A fresh primary-source check of the [diamond rule](https://arxiv.org/html/1504.08258v1)
identifies a stronger intervention than one-step rule selection: sum a whole
recursion directly onto pinch boundaries, avoiding intermediate terms. Its
published construction, however, requires massless lower and spectator lines;
massive upper lines are allowed. It is not directly an equal-mass vacuum rule.

The obstruction is visible without a large solve. In a scalar triangle let
`D0=k²+m0²`, `Di=(k+pi)²+mi²`, `Ci=pi²+mi²`, with powers `b,ai,ci`.
Our direct Euler-IBP derivation gives

```text
E I = Σ_i ai Ai⁺(B⁻ − Ci⁻) I − m0²(Σ_i ai Ai⁺ + 2b B⁺) I,
E = d − a1 − a2 − 2b.
```

Here `Ci⁻` multiplies by `Ci`, lowering its spectator-propagator power;
prefactors `ai` and `b` are evaluated at the source indices.

The final mass-dependent terms do not lower `b+c1+c2`; one raises `b`.
Setting the common mass to one does not remove them. Thus borrowing the
massless termination argument would be invalid. A future massive summed
recurrence needs an exact combination cancelling these terms, then an actual
work comparison. No diamond implementation or successful massive shortcut is
claimed. This remains a research lead, not a reason to enlarge the failed
Laporta bank blindly.

The saved broad46 row suggests one narrower test: exactly one exported term
preserves numerator rank, `-I(n-e3)`; its other45 terms shift `n8` toward zero
by one or two. Displayed coefficients depend only on `n8,d`, but independence
from `n3` has not yet been checked through native algebra. If confirmed, write
`I(a)=-I(a-1)+K(a)` and test the exact finite identity
`I(3)=-I(0)+K(1)-K(2)+K(3)` at physical input
`[1,1,1,3,1,1,1,1,-2,0]` (A10/R2/D8). This skips a defined chain, unlike the
earlier arbitrary-child composition. It may still worsen work: up to136 terms
appear before collection. All guards, source replay and endpoint costs remain
required. The screened anchor has a=1 already, offering no skip; its unbounded
helper must not be clipped to physical a<=6. A general symbolic-length sum needs
another representation, so only this small finite discriminator is registered,
not authorized as another pilot or implemented as an engine feature.

## Target-directed elimination: distinguish a new mechanism from existing GPLU

Kira3 selects equation dependencies after forward elimination to avoid retaining
dependencies that cancel during subsequent substitution. It also adjusts seed
coverage by sector, checking target sufficiency rather than treating the seed
boundary as permission to discard real reduction terms.
[Kira3, sections3.1–3.2](https://arxiv.org/html/2505.20197v1#S3.SS2).

RustRed already records direct forward-GPLU dependencies in
`solver/discovery.rs::add_row`; `trace_many` collects their required closure.
Its target-only materializer reconstructs the same canonical row from the
unchanged source prefix. Rebranding these as a new hidden-zero optimization
would not change the descendant work. This is a source-audited conclusion,
not a new timing result.

Blade constructs compact relations within selected integral sets, organizing
their solution in blocks. It also distinguishes genuine inter-sector relations
from naive cut-based independence. These mechanisms motivate changing the
relations supplied to the walk, but its published reduction/reconstruction
speedups do not predict RustRed's symbolic-domain traversal performance.
[Blade, sections2.5 and3](https://arxiv.org/html/2405.14621v2#S3).

One genuinely untested distinction remains: our752-row joint probe used literal
same-family integral columns, not exact symmetry-identified columns. Its failed
projection therefore does not exclude an improved relation after verified
symmetry transport. A bounded next discriminator must first exhibit a nontrivial
map that preserves the declared symbolic chart. Arbitrary routing is not enough:
symbolic numerator powers can require non-fixed-length expansions, and active
permutations can leave the `n+constant shift` representation. The smallest gate
admits exact maps fixing free coordinates and permuting equally fixed bases;
an empty/trivial action is a negative, not grounds to expand the source bank.

A second necessary boundary is proof provenance. A combination using symmetry
equations cannot be exported as a zero residual of ordinary IBPs alone. It
needs either an actual ordinary-source replay or separately validated symmetry
steps. No such mixed proof is claimed here. A concrete-only solve must likewise
not be reported as a parametric recurrence. The ignored source assessment is
`TMP/postlaunch-20261002/tangent-integration/TARGET_BLOCK_RELATION_NOTE.md`.

### Concrete symmetry quotient: native experiment and negative result

An existing admitted map, `k2↔k3`, supplies the denominator permutation
`[0,2,1,3,4,6,5,8,7,9]`. The old rule151 target is fully fixed, so this first
experiment is point-only; no symbolic index was freed. Native family-map
verification and permutation compilation validate the map again and retain its
conditions. Each matrix key is transported with the existing native API.

The experiment regenerates precisely the original47 centres/752 ordinary rows,
69-term parent and24-term weighted difficult block. The45 other siblings remain
unchanged. The literal solve reproduces the old negative result exactly, including
the753 recorded pivot guards, apart from timing. Quotient outputs are allowed
only if their class contains one of the ORIGINAL82 allowed columns; an absent
image cannot create a new allowed output, and the parent stays forbidden.

| Metric | Literal bank | Verified symmetry quotient |
| --- | ---: | ---: |
| Integral columns/classes | 1,322 | 1,306 |
| Allowed columns/classes | 82 | 78 |
| Forbidden columns/classes | 1,240 | 1,228 |
| Reducer nonzeros | 21,724 | 21,727 |
| Elimination seconds | 0.236555 | 0.353793 |
| Target-block pivot | Absent | Absent |

There are16 paired column orbits, with zero mixed allowed/forbidden classes.
None of the24 difficult terms has its swapped partner anywhere in the literal
bank. Only1/47 source centres maps into that centre set. These observations
explain the limited opportunity for this particular quotient; they do not prove
absence of other useful symmetries, seeds or identities.

The owned probe completes in2.343s inclusive (native adapter0.833s), separately
from123.635s adapter compilation. Independent audit confirms guards, scope,
inventory and drained process groups. No mixed identity was produced, so the
successful residual-reconstruction branch remains unexercised at runtime.
No rule or terminal is published and no larger bank is automatically attempted.
Evidence: `TMP/postlaunch-20261002/tangent-integration/symmetry-block-v1/` and
`SYMMETRY_CHART_CENSUS.md` in its parent directory.

### Follow-up candidate: symmetry-adapted numerator coordinates

A source-level assessment identifies an actual FG bubble reflection, distinct
from changing a matrix row basis. With `ell=k3`, `p=k4`, `q=k1`, define
`C=(D1+D3+D7-D4+D5+1)/2` and `X=D9-C`. Under `ell→p-ell`, D3 and D7 exchange,
C is unchanged and X changes sign. If the two bubble denominator powers are
equal and D10 has power zero, the full integral with an odd power of X vanishes.
Its expanded monomials do NOT vanish separately. The broader46 rule permits
unequal bubble powers and therefore cannot use this condition everywhere.

This is a candidate for simpler scalar relations, not another tensor reducer.
The costs of expanding `D9^r=(C+X)^r`, changing any numerator coordinates and
restoring all outputs to the original family must be included. The saved parent
root includes positive D9, so a transformed numerator cannot simply be relabeled
as the same owner. Existing affine-family construction and verified finite
numerator transport provide a possible fixed-rank diagnostic; no native check,
new mixed-proof publication or performance benefit is claimed yet. First seek
a strictly improved fully restored row on a real covered slice, before another
campaign or source-format extension. Read-only details:
`TMP/postlaunch-20261002/tangent-integration/CENTERED_ISP_NOTE.md`.

## Requested slices versus unfinished helper cones

The current five-loop declaration contains116 required inputs with finite
coordinate, numerator-rank and total-positive-power bounds. Its67 auxiliary
inputs include13 without a total-positive-power bound. Removing those auxiliary
starts alone is not new: the earlier full-physics helper-free run used precisely
the116 required rows and was censored with2.802M pending domains.

A distinct proposal is to apply the existing exact rule to the demanded slice
Q instead of inheriting the entire unfinished dependency cone of a larger A.
Old graph edges do not retain a reusable restricted source-to-child map, so the
smallest sound mechanism reruns native matching and application on Q, retaining
every condition and successor. Containment lookup and merge must not immediately
redirect it back to A. This is not a rule-generation improvement or proof of
finite symbolic traversal.

Independent critique identifies the necessary workload distinction. In the
four-loop58-query control, the wider anchors themselves are required: their
work cannot be avoided by closing Q earlier. In five loops auxiliary-only work
can remain pending, but a demanded-work stopping policy is needed to realize
the saving rather than eventually drain that queue anyway. Existing cold
`PhysicsQueries` verification can accept a cooperative checkpoint with optional
pending work, provided all183 inputs were admitted/source-valid, every required
query is covered by a closed independently reinspected input root, and every
saved native record passes reinspection. Its `complete` flag describes the
reinspection, not an empty pending queue. An internal Q node alone is currently
not a certifying input root; initial absorption must also be handled honestly.

No scheduler change or experiment is claimed for this proposal. Its local
image witness is insufficient evidence of total-work savings. The audit and
minimal falsifier are saved in
`TMP/postlaunch-20261002/tangent-integration/DEMAND_SLICE_FALSIFIER_PLAN.md`.
