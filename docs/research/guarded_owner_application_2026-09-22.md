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

## Later-guard rejection: core release gate passed

The follow-up ordinary dispatcher now handles the concrete failure pattern
above. When an equality or excluded atom is genuinely unknown, it may probe
later necessary guards of that same candidate on the unchanged cell and actual
rank. A later uniformly nonzero equality, whole all-zero excluded conjunction,
or zero original denominator proves the candidate inapplicable. Dispatch then
resumes at the next rule's ordinary fixed-case check, preserving saved priority.

Source-condition uncertainty and native work/backend failures do not enable
this shortcut. Intersecting coordinate planes are not uniform zero, speculative
cuts are not retained, and any encountered later error keeps its own predicate
identity. Without a rejection witness, the original refinement/residual path
remains. Every probe uses the existing native Symbolica-backed guard service;
no additional CAS or integer-feasibility solver was introduced.

The independently reviewed release gate passes **2,694 tests**, zero failures
and 32 existing ignored diagnostics. All ten new rejection tests pass, as do
the 11 preceding excluded-AND tests and the 68-test matching selection; these
focused counts overlap the full suite. One existing test updates only its
predicate-work count for the newly charged inconclusive probe, retaining its
behavioral assertions. Raw receipts and independent runtime review are in
`TMP/rejection-lookahead-core-retry.rx4GGf/`.

This is tested dispatcher functionality, not a claim that all real five-loop
guard frontiers disappeared. That needs the new integrated saved-program run.

## Actual-input correction: optional rejection is not a mandatory guard check

The first actual 67-owner check stopped in owner `000011001001011` at a
separable-factor-work preflight for rule 405 / term 1. All fifteen integer
points of that R10 cell had selected rule 405 in the previous result. Retrying
the original unknown guard through exact bounded refinement fixed this case,
but a second run stopped at owner `111101111111100`, rule 252 / term 3. That
second cell has three unbounded positive axes and no varying inactive axis:
finite numerator refinement cannot help. It is exactly an earlier unresolved
excluded-conjunction region (rule 252 / branch 3 / atom 0), not a missing-rule
counterexample. These were operational regressions caused by optional lookahead.

The corrected policy treats the shared closed class of native next-operation
preflight refusals from **optional rejection probes** as inconclusive. It
retains the original predicate, bounds, rank and refinement cursor. The existing
exact bounded split is used when admitted; otherwise the original Unknown is
reported. No refusal proves rejection, applicability, nonzero or closure. All
already-spent predicate and geometry work stays charged. Mandatory denominator
and source checks, cumulative budgets, cancellation, input/output limits,
allocation and backend errors remain strict. The existing same-AND witness
path is unchanged. No CAS implementation or native allowance was added/raised.

The corrected core passes **2,700 release tests**, zero failures and 32 existing
ignored diagnostics, including six refinement regressions. A mandatory guard
test still fails at the same factor-work allowance that the optional probe can
leave inconclusive. Other tests retain the original fifteen-point geometry,
explicit-face/concrete parity, positive-only unknowns, work accounting and
cancellation. Raw core evidence: `TMP/optional-rejection-core.4h4f00/`.

The new actual check processes **all 67 owners without a native error**, finding
213,030 selected-rule pieces, 900 terminal pieces and **57 unresolved regions**.
Sixty owners are locally covered, as before. Against the earlier 81 Unknowns,
51 records are identical, 30 old predicate identities disappear and six later
predicates are exposed on exactly the same six previously unknown boxes. These
are not six newly unknown geometries. The previous fifteen points all still
select rule 405, and the second failed cell again reports its original Unknown.

This is local applicability, not recursive R10 closure, rule generation or a
performance comparison. The run overlapped release validation on disjoint CPUs.
It returns incomplete because the 57 Unknowns remain, not because of a fatal
refusal. Evidence: `TMP/optional-rejection-local.UBJDgj/`, including independent
comparison with the saved baseline. The earlier failed attempts remain saved
as `TMP/rejection-full67-local.isowEn/` and
`TMP/rejection-refinement-local.O5Mrky/`.
