# Rank-30 certification checkpoint (2026-09-17)

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
   through `CheckedProgram` and every retained rule cell.  Keep source replay
   on the original full rows; only executable RHS domains are clipped.
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

