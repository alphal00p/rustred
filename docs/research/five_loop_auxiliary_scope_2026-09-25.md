# Five-loop auxiliary scope: finite roots versus broad reusable obligations

This source and retained-evidence assessment changes no live process, input,
checkpoint, binary or solver policy. No new native run was performed. The
five-loop campaign remains incomplete; neither its termination nor
nontermination is established here.

## The added obligations are materially stronger

The [coarse-cover campaign](five_loop_coarse_cover_restart_2026-09-24.md) retains
all 67 required finite A<=24, R<=15, D=A-R>=9 entry queries. It also adds 67
ordinary auxiliary obligations with R<=15 and no D bound. Of these, 54 have
A<=24; the remaining 13 have unbounded A. The
[input planner](../../examples/python/stage_saved_owner_campaign.py) appends
queries, not certified coverage or cost-free cache templates. Every escaping
Apply and Route dependency remains required.

The guard-cone rationale proves a narrower property than termination. Those
13 owners have at most seven active indices. Accepted Apply transitions do not
increase support cardinality, and admitted Route covers preserve or reduce it.
They therefore cannot return to the seven guard-problem owners, whose supports
have at least eight active indices. This does **not** prove rank nonincrease,
strict-subset transitions, or absence of repeated visits within lower-support
strata. The retained input diagnostics established local matching/guard
coverage, not recursive RHS closure.

## Why local descent does not bound rank over an infinite input family

Let A be total positive power, R total numerator rank, and P=A+R. A generic
same-sector physical-index shift illustrates the issue:

`(a,-r) -> (a-2,-r-1)`, for `a>=3` and `r>=0`.

Here A decreases by two, R increases by one, and P decreases by one. If its
source guards and coefficient permit it, this term strictly descends the saved
same-sector order: [corner distance](../../crates/rustred-core/src/sector/ordering.rs)
is P minus support cardinality, and the
[shift key](../../crates/rustred-core/src/sector/shift_ordering.rs) compares it
before degree and coordinate ties. Native
[successor geometry](../../crates/rustred-core/src/solver/candidate_reduction/owners/domains/applied/geometry.rs)
translates rank by the signed shift; it does not clip it to the entry cap.

Arbitrarily large initial a permits arbitrarily many such steps across the
family, even if every individual finite point terminates. This is a generic
allowed mechanism, **not an observed repeatable rule chain in these 13
programs**. Rank growth itself is already observed in the completed
[four-loop control](four_loop_saved_cover_control_2026-09-24.md): R12 anchors
generated retained R13/R14 work. That does not determine five-loop termination.

## What finite A replacements would and would not establish

The smallest input-only counterfactual retains the original 67 queries and the
54 existing finite auxiliaries, replacing only the 13 unbounded-A auxiliaries
with A<=24, R<=15, no-D anchors. Each replacement contains its corresponding
required root, not the old infinite auxiliary. All descendants remain required;
A24/R15 would be input geometry, never a descendant cutoff.

This gives finite initial domains with P<=39. Each successfully propagated
domain also retains finite A/R bounds:

- Apply translates A, R and D exactly on sign-refined cells, with explicit
  failure on unrepresentable bounds.
- [Power projection](../../crates/rustred-core/src/solver/candidate_reduction/power_domain/geometry.rs)
  retains the aggregate predicates. Its projected rectangle alone may be a
  strict overcover, but the actual domain still includes the A/R/D constraints.
- [Route overcover](../../crates/rustred-core/src/solver/candidate_reduction/routed/domain_overcover/visit.rs)
  preserves or lowers A and R. It can widen inactive coordinates and discard
  an unsafe D upper bound without discarding the retained finite A/R caps.

Individual-domain finiteness is not a uniform global bound. If every
changed-support Apply strictly lowered cardinality, the saved shift bound L=8
would give `P <= 39 + (m0-m)*L` along a path from initial support cardinality
m0 to current cardinality m. Same-sector Apply
and the actual retained-predicate Route overcover preserve this bound. However,
the premise is not proved globally. The
[existing architecture review](finite_closure_architecture_review_2026-09-23.md)
records the structural census and this unresolved gap.

In particular, `unsupported_support_successors` is observational, not a queue
rejection. The [native support-transition tests](../../crates/rustred-core/src/solver/candidate_reduction/owners/domains/applied/tests/support_transitions.rs)
explicitly admit same-cardinality mask exchanges, and the
[walker](../../crates/rustred-app/src/application/routed_campaign/walking/inspection.rs)
can send them to Apply or Route. Changed-sector descent compares cardinality
then mask, potentially ignoring an increase in P; routing can reset that mask.
Guarded per-rule descent therefore does not by itself establish a globally
consistent routed order. Zero unsupported successors in the completed
diagnostics is not proof for future work or a full audit of the live records.

A later cheap owner/phase edge diagnostic might seek a compatible global
potential from maximum P shifts. A positive-weight cycle in a conservative
graph would be unresolved, not proof of nontermination without feasible guards,
rule selection and nonzero coefficients. No such algorithm or certification
detour is introduced here.

## Reuse cost and the observed bottleneck

Finite replacements also remove the rank-only
[initial-orthant shortcut](../../crates/rustred-app/src/application/routed_campaign/walking/initial_orthants.rs)
for those 13 owners: it requires unconstrained power predicates. Generic
semantic containment and initial D-band reuse remain available. Smaller added
obligations may therefore trade against cheaper containment; total improvement
requires measurement, not assumption.

The [four extracted expensive heads](five_loop_slow_inspection_parallelism_2026-09-24.md#exact-current-head-geometry)
belong to owner `011101110111000`, with nine active indices and an already
finite A24 auxiliary. Their bounds are A26/R13 or A25/R14. The later profiled
owner `010011111101011` has ten active indices and likewise an A24 auxiliary.
The 13 lower-support anchors cannot generate these heads through accepted
cardinality-nonincreasing transitions. Their possible added work is therefore
not a demonstrated cause of the current hot application/admission bottleneck.

Broad regions could in principle be cached as *transformations* applied only
to requested subdomains. That differs from claiming coverage: the current
[initial-overlap mechanism](../../crates/rustred-app/src/application/routed_campaign/walking/initial_overlap.rs)
requires an actual pinned obligation. Removing it while retaining skip-work
reuse would be unsound. A transformation cache would need preserved native
guards, source binding and all instantiated conditional dependencies; it is
not the minimal input-only experiment proposed here.

Any future counterfactual must use fresh inputs and checkpoints, retain all
original queries and escapes, and report full ledger exhaustion or honest
incompletion. Existing W50 live progress is not a matched W6 timing baseline.
No replacement campaign or pilot is launched by this note. Detailed local
source references and the proposed 13-only input delta are retained under
`TMP/finite-auxiliary-scope.ulTVUl/`.
