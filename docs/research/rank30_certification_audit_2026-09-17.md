# Rank-30 certification checkpoint (2026-09-17)

## Follow-up: degree-scoped exact audits (2026-09-19)

The public Rust diagnostic `SourcePortAudit::audit_sector_through_total_excess`
now requests coverage through an explicit **total excess**
`sum(max(n_i-1,0) + max(-n_i,0)) <= D`. It still performs the existing full
original-source replay, original guard checks and supplied-domain strict-descent
proofs. Its report records `max_total_excess_degree: Some(D)`; the ordinary
whole-sector report records `None`. A diagnostic report cannot publish an
artifact. The current complete-program installer additionally rejects bounded
report metadata, and the CLI/Python rank-certification option remains
fail-closed as described below.

The coverage implementation reuses the exact Boolean/predicate and `BoxCover`
traversal. A coordinate hull bounds traversal but is **not** the claimed domain:
an uncovered rectangle matters only if its lower corner satisfies the exact
degree sum. This avoids enumerating simplex points. Affine exceptions remain
exact predicates, and unresolved feasibility still fails closed. Degree probes
share the existing cumulative geometry-work allowance. Numerator-only geometry
retains genuinely unbounded positive powers; it is not silently converted into
a total-power bound.

The checker remains conservative for coupled affine predicates intersected
with the sum-bound. A follow-up tightens each remaining box before the existing
native affine checks: for counted axis `i`, its upper endpoint is at most
`D - sum(lower_j, j != i)`. This is the exact coordinate hull of the
box/simplex intersection, not a replacement for that intersection as an owner.
It resolves the small diagonal example `x=y`, `x+y<=1` with an explicitly owned
origin: `x>=1` forces `y=0`, and conversely. No new polynomial arithmetic,
sampling, or general affine/simplex feasibility solver is introduced. The
same cumulative geometry allowance pays for the extra coordinate work, and
unresolved affine feasibility still fails closed. Numerator-only bounds leave
positive axes uncapped.

An internal, immutable `EntryScope` binds the family, root sector and exact
degree convention. A proposed rectangular proof envelope composes the existing
successor-cover owner, proves entry containment by the same complement-minimum
test, and checks source/RHS images using one cumulative budget. It is not an
artifact certificate or a new CAS. This contract is a reusable boundary;
full bounded installation, persistence and runtime admission are still pending.

### Complete-program successor-envelope diagnostic

`SourcePortAudit::audit_complete_through_total_excess(&family, sectors, D)`
now consumes the same solved-sector tuples as the existing installer, but
returns an immutable `SourcePortTotalExcessAudit`, not a `ClosedArtifact`.
It validates the complete nonzero sector census beneath the root, checks the
family and common ordering, then processes sectors from harder to easier using
the persisted ordering's sector-corner keys. Every entry sector initially
receives bound `D`.

Each sector undergoes unchanged full original-source replay, guard checks and
uniform strict descent, with coverage requested through its propagated degree.
Only retained checked rules contribute edges. Their physical index shifts are
partitioned at sign changes; independently proved zero sectors and exact empty
or zero-coefficient branches may be removed. Same-sector descent cannot
increase total excess. A strictly lower-sector edge propagates the bound
derived below. An unexplained nonlower or out-of-root edge fails closed.
The report exposes both `max_entry_total_excess_degree()` and the complete
`successor_degrees()` map, so descendants are never silently subjected to the
entry cap. Input mask iteration order is not used as the reduction order.

One cumulative structural allowance covers the census and sign partitions.
Existing native algebra limits remain per proof query; this is not a global
Symbolica scratch-memory bound. Affine predicates are preserved and may still
cause a conservative failure. No installer accepts this diagnostic as an
artifact, and no persistence or runtime scope check is bypassed.

### Why entry bounds and descendant bounds must differ

Independent inspection of the saved four-loop rules found actual escapes:

* H: `(-29,0,0,1,1,1,-1,1,1,0)` has negative degree and total excess 30.
  Its first applicable stored rule contains, with coefficient one,
  `(-29,0,-1,1,1,2,-1,0,1,0)`: negative degree 31, total excess 32.
* FG: `(0,0,1,1,1,1,1,1,0,-30)` similarly produces
  `(0,-1,1,1,1,2,1,0,0,-30)` with the generically nonzero coefficient
  `(d+29)/(240-120*d)`, again increasing the degrees to 31 and 32.

These are checks of saved rule domains and index shifts, not new source
certificates. Same-sector numerator increases also occur while dots decrease,
so an unbounded-dot numerator scope needs more than a sector-only argument.

For an explicitly requested total-excess scope, the existing strict ordering
gives a simpler route: within one sector total excess cannot increase. Across
sectors, a conservative bound for a physical shift `s` is
`E(child) <= E(parent) + sum(abs(s_i)) + p(parent) - p(child)`, where `p` is
the number of positive indices. Propagating such bounds through the finite
strict sector-order graph gives proposed per-sector degree envelopes. Each
envelope must still pass coverage, source replay, guards and successor checks;
none of this assumes a minimal terminal basis.

The next integration should reuse whole-domain replay/descent proofs where
they succeed, attach the entry/envelope semantics at artifact level, recheck
them on cold loading, and enforce entry scope before memoization. Bound 30 is
not itself a cure for every old cover failure: the historical difficult FG214
abstract Boolean witness had total excess only four. Its feasibility obligation
must still be discharged exactly.

Independent counterexamples and integration audit are retained in
`TMP/bounded_certificate_math_audit_2026-09-19.md`. No four-loop bounded
artifact is claimed at this checkpoint.

The preceding focused release gate passed **306 tests**, with five pre-existing ignored
workloads. This includes ten entry/envelope contract tests, five degree-cover
tests (one exhaustively compares all 512 subsets of a 3x3 grid at three bounds),
and two source-port integration regressions. The first run had one overstrong
test expectation for the affine diagonal example above; it was corrected to
require the conservative rejection, with no production proof weakened. Both
logs remain in `TMP/bounded-scope-release-tests*.log`. Independent source and
mathematical reviews approve the implementation; formatting checks pass.
The subsequent full release core suite also passes: **2,187 passed, 31 ignored,
zero failed** in 56.37 s (compilation excluded). Its record is
`TMP/bounded-scope-full-core-tests.log`. This is a regression gate, not a
four-loop artifact certification run.

### Measured FG214 follow-up and regression gate

The saved four-loop FG sector `0110101100` (mask 214) now passes the actual
selected-sector exact audit through **total excess 30** with unchanged default
proof limits. All 161 rules replay from original IBP sources and uniformly
descend; stored and checked predicate covers both leave zero uncovered and
zero unbounded boxes, with no issues and 11 explicit terminal declarations.
All 43,582 saved `SeedSource` entries were retained. The resulting replay
report counts 11,217 nonzero original-source contributions; these are different
censuses, not contradictory source counts.

The release, single-worker CPU-83 run completed in **74.837 s** for the audit
call and **75.15 s** for the whole process, using 74.40 s user plus 0.11 s system
CPU and 101,376 KiB peak RSS. The audit timer includes exact replay, descent
and both coverage checks; loading, the zero-sector census and owner setup are
outside it. No candidate search, rule regeneration or proof-budget increase
occurred. The 300-second external limit and 32-GiB virtual-address-space cap
were not reached. This is one completed observation, not a timing distribution.

The earlier cheap probe independently passed stored-guard coverage in 0.709 s
(1.11 s whole process), but deliberately removed source traces and proved no
identities. It remains separate evidence, not part of the exact proof result.
Evidence and reproducible clients are in `TMP/fg214-degree-cover.52OJcp/` and
`TMP/fg214-original-audit.wUmO3W/`, with native input/library hashes and the
unchanged production codec used to read the saved candidate solution.

**This is not a complete FG-parent artifact.** It does not check the complete
parent census under propagated successor degrees, lower the rules into runtime
cells, persist the scope, cold-replay a bounded owner, or test runtime entry
admission. Those are the next gates. In particular, degree 30 here is a
selected-sector coverage bound, not a claim that all its descendants have
degree at most 30.

The new complete-program diagnostic is exercised on real K1 and K3 solver
outputs, including arbitrarily ordered input sectors, propagated bounds,
foreign/nonlower successor rejection, exact zero pruning and cumulative
geometry limits. Exhaustive small-box tests check the tightened degree hull's
coordinate extrema, and extreme endpoints and numerator-only infinite axes
are covered. Independent mathematical and implementation reviews pass.
The focused release suite now passes **319 tests, five ignored**; the full core
release suite passes **2,200 tests, 31 ignored, zero failures**, in 41.35 s
excluding compilation. Logs are `TMP/total-excess-focused-release-tests.log`
and `TMP/total-excess-full-core-release-tests.log`; formatting checks pass.

## Decision

RustRed now exposes an explicit, fail-closed request for a bounded entry
degree.  `max_negative_index_degree` is accepted only up to 30.  A request is
rejected before candidate decoding with a diagnostic that names the missing
successor-closed artifact contract; RustRed never silently runs the existing
whole-family certification and relabels it as a rank-bounded proof.  Values
above 30 are input errors.

This is deliberately a capability boundary, not a claim that a rank-30
four-loop artifact is complete.  It prevents expensive unrestricted retries
while the bounded proof path is being implemented.  The ordinary
`certify-candidates` behavior and artifact schema remain unchanged when the
option is omitted.

## Why the existing primitives cannot yet publish a bounded artifact

The repository already contains useful exact geometry:

* `source_port/scope/rank.rs::RankScopeBudget` enumerates the requested entry
  set
  \[
    E_D = \{n : \sum_i \max(-n_i,0) \le D\}
  \]
  as deterministic negative-index slices with genuine infinite positive-power
  rays.  Checked arithmetic and cumulative geometry budgets are in place.
* `predicate_cover::certify_predicate_cover_within` proves ownership of a
  required union, but only proves coverage.  It does not prove source replay,
  guards, strict descent, or closure of rule RHS images.
* `scope/successor.rs::SuccessorClosedScope` admits an immutable destination
  box union, checks entry containment, and checks translated RHS images with
  the existing exact sign-partition geometry.  It is an internal building
  block and intentionally does not own affine predicates, source traces, or a
  durable artifact.

The publication path still has the following whole-family assumptions:

1. `SourcePortAudit::check_sector_with_observer` runs predicate coverage over
   the complete sector orthant.  The scoped coverage helper is not wired into
   replay or descent.
2. `CheckedProgram` lowers retained applications without carrying a semantic
   proof domain.  Source rows and all original guards are checked, but no
   finite rank envelope is attached to a rule cell.
3. `ClosedArtifact` stores only the rectangular root bounds.  Its durable
   codec has no sum-bound, proof-domain union, or successor-closure witness.
4. `Reducer::validate_target` checks the rectangular root prefilter only.  It
   cannot enforce a caller's exact entry degree after cold loading.

Consequently, using the old installer after parsing a rank option would either
still require the expensive unbounded certificate or publish an artifact that
could accept an out-of-scope target.  Both outcomes would misrepresent the
mathematical guarantee, so the new option fails closed.

## Required next implementation (in order)

The smallest sound rank-30 implementation must compose all of the following,
without introducing a new CAS:

1. Carry an immutable `EntryScope` and independently proposed proof envelope
   through the checked artifact owner. Keep existing whole-domain cell proofs
   where they succeed; attach narrower domains to cells only when their proofs
   actually use scope restrictions. Source replay retains the original full
   rows, never a sampled or uncertified projection.
2. Prove coverage of the envelope, exact source applicability and guards, and
   strict descent on every retained cell.  A rank slice is not evidence by
   itself.
3. Run `SuccessorClosedScope` after exact RHS pruning for every rule that can
   be selected by runtime precedence, including affine exceptional branches.
   Any escape is an inconclusive failure, not a sampled success.
4. Version and persist the entry predicate and proof envelope.  Cold decoding
   must repeat the same containment and replay checks from untrusted bytes.
5. Make the reducer enforce the exact entry predicate before cache lookup and
   scalar lowering.  Descendants are governed by the persisted inductive
   envelope, not by the entry bound.

Only after these gates are connected should the CLI/Python option become a
working bounded certificate.  A larger envelope or full-sector widening may
be added later, but must report both the requested entry set and the proved
successor domain.

## Tests for this checkpoint

The option is covered at the application and CLI boundaries:

* `CandidateCertificationRequest::with_max_negative_index_degree(30)` returns
  an execution error naming the absent successor-closed scope and does not
  decode or certify the bundle.
* Degree 31 returns an input error before any native work.
* `rustred certify-candidates --max-negative-index-degree 30` exits nonzero,
  emits no artifact bytes, and reports that an unbounded fallback was refused.
* The help text and Python stub document the same bound and fail-closed
  behavior.

These tests intentionally do not claim closure.  Existing unrestricted K1/K3
candidate certification and all existing source-port geometry tests remain
the authority for the current artifact path.
