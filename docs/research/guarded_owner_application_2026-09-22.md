# Applying saved guarded cases without rectangularizing them

The core now exposes `CandidateOwnerPrograms::visit_owner_guarded_rule_successors`.
This is a bounded one-hop application of an explicitly selected saved rule, not
an independent closure certificate or proof that the remaining R10 campaign is
solved. It reuses existing programs; it does not regenerate their IBPs.

## What changed

Prepared rules retain their original native `Case`, sharing affine charts by
`Arc`. The visitor takes an owner, batch/rule selector and surrounding box/rank,
then constructs the rule's own guarded domain inside that immutable snapshot.
For example, a saved rule with `n0=n1` can be applied conditionally on that
unbounded diagonal without expressing the diagonal as a union of boxes.

The domain retains the original physical-index equations, whole excluded
conjunctions, source conditions and every original denominator, including those
of zero or subsequently cancelled terms. It also retains the incoming complement.
The selector is not supplied applicability authority, and no integer-feasibility
or first-priority dispatch claim follows from this operation.

The existing RHS engine accepts a borrowed native chart. Symbolica-backed joint
rational restriction preserves coefficient scale; zero-locus normalization is
used only for predicates. Original-term child/source validity and descent checks
still precede equal-shift cancellation. The ordinary unguarded path is unchanged.
No custom polynomial substitution, factorization or reconstruction was added.

For a child `m=n+s`, the returned image carries `P(m-s)` for all source guards,
the source boundary cell and the original rank constraint. An i128 inverse-shift
carrier covers every i64 shift, including its negative extreme. The target
box/rank alone is not this image. Conditional coefficients remain conditional;
problems and complements cannot be turned into masters or discarded.

Guarded images are **not** inserted into the existing box-only work queue.
Following them through saved rules requires a separate native continuation
bridge, guided first by the actual saved diagonal cases. This is the next
operational step, not a requirement to build a general polyhedral certifier.

## Excluded-conjunction lookahead

An excluded conjunction `[p,q]` means `not(p=0 and q=0)`. If classification of
`p` is genuinely unknown but `q` is uniformly nonzero on the same current cell,
the exclusion is already satisfied. The matcher now checks later atoms for
that witness using the existing native guard service.

This does not split on an unproved surface, bypass a native work refusal, skip
source/equality requirements, ignore other exclusions or change rule priority.
Without a uniform witness it retains the original refinement/unresolved path.
Actual effects on the full-census frontier count await a new release control;
the earlier 81/449 records are not asserted resolved by this test result.

## Validation

- Full core release suite: **2,684 passed, zero failed, 32 existing ignored**.
- Focused guarded filter: **27 passed**; this includes nine new guarded-domain
  semantic tests, one retained-chart admission test and existing related tests.
- New excluded-conjunction filter: **11 passed**.
- Independent implementation/mathematical review and independent raw runtime
  review pass. Source and executable bindings were checked before/after testing.

Tests cover rational chart scaling, whole exclusions, original poles, physical
inverse shifts, source validity before cancellation, exact coalescence, descent,
retained source R11 versus pinched target R12/R11, cancellation and resource
limits. A memory-accounting fixture initially compared original polynomial
buffer capacity with the cloned prepared value. It was corrected to inspect
the actual prepared payload and test exact-limit success plus typed one-below
failure; no production condition or assertion was relaxed.

The full suite took 155.67 s external wall / 167.11 s CPU and 199,028 KiB peak
RSS. These are test-suite costs, **not IBP-generation timings**. Compilation
is separate. Receipts are in `TMP/guarded-core-retry.lHXWlG/`; the preceding
failed fixture receipt remains under `TMP/guarded-core-gate.1DOmRR/`.

CLI/Python integration, the bounded reuse-stream optimization and the two real
saved-rule diagonal controls are separate gates. No full R10 closure, new
terminal basis, parallel speedup or five-loop Vakint result is claimed here.
