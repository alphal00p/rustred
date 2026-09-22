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

## Application and real saved-case controls

The Rust service, `owner-guarded-apply` CLI and Python steering are now available;
see [the interface](../guarded_owner_rule_diagnostics.md). The separate release
application gate passes **362 tests** (280 library and 82 integration), with
zero failures. Its focused guarded and walking selections pass 9 and 36 tests
respectively; these overlap the full suite. All **26 Python owner-steering
tests** also pass. The application suite takes 63.80 s wall / 62.80 s CPU with
218,720 KiB peak RSS; compilation is separate. These are test costs, not solver
performance. Receipts: `TMP/guarded-reuse-app-retry.Sas0k6/`.

Two explicitly selected actual five-loop candidates were then inspected using
the same full saved 67-owner context. Both inspections finish, but **neither
candidate applies**:

- Owner `111010100100101`, rule 239 retains the equality `n0=n1`, but also
  requires `n13 != 0`; the supplied cell fixes `n13=0`.
- Owner `011011000111111`, rule 183 includes three excluded conjunctions.
  Although the first contains the apparently difficult diagonal condition,
  the second requires `n4 != 1` while this cell fixes `n4=1`. A further
  exclusion requires `n0 != 0` while this cell fixes `n0=0`.

Each query therefore reports its incoming complement and an excluded-conjunction
residual. There are zero admitted pieces, RHS term visits or successors. All
65/104 original term denominators remain visible. Exit zero means diagnostic
inspection/rendering completed, not a positive one-hop application, integer
feasibility, recursive closure or a missing-rule finding.

The observed command takes 112.61 s wall / 111.81 s CPU, with 5,810,876 KiB peak
RSS. Shared preparation accounts for 109.48255 s; both native inspections
together take 0.001485 s. **The latter is not an IBP solve time.** Inputs,
raw output and independent review are in `TMP/guarded-diagonal-controls.C43MmJ/`.

This evidence changes the immediate next step: use later uniformly false
necessary guards to reject impossible candidates before an earlier unknown
predicate obstructs dispatch. Keep first-applicable priority and all native
source/error behavior. Genuine admitted guarded successors still require the
continuation work described above; these negative controls do not validate it.

The same application gate also covers optional bounded job-local duplicate
suppression and counted callback streaming. Native algebra still runs, and
ordered publication still admits the first occurrence before any reuse marker.
This has not yet been shown to improve full-census scheduling performance.
No full R10 closure, new terminal basis, parallel speedup or five-loop Vakint
result is claimed here.
