# Optional modular prefilter for saved terminal equations

Status: design recommendation only, 2026-10-08. No implementation or performance
claim. This backend optimization must not replace the separate containing-sector
source and reflection experiments. Those mechanisms should be assessed first.

## Motivation and authority boundary

The ongoing five-loop saved-assistance experiment was reported at approximately
96,000 consumed saved equations, 5,653 ordinary source rows, 25,997 independent
exact rows, 103,164 columns and 634,000 retained nonzeros. These are intermediate
observations supplied by the campaign coordinator, not a completed reduction or
a controlled benchmark. They suggest substantial redundant input, roughly three
quarters of consumed rows, although rebuilds and normalization prevent treating
that ratio as an exact duplicate count. The four-loop depth-one comparison also
reported much higher assisted runtime without further compression.

A modular filter would select which saved equations deserve exact elimination.
It would not establish a new identity, certify saved candidate provenance, prove
the omitted equations redundant over the rational-function field, or establish
master independence. Every retained equation would still enter the existing
exact Symbolica row reducer with its original coefficients and conditions.

Let E be the supplied exact equations and K the retained subset. Exact row
operations on K produce only consequences of K, hence consequences of E under
the inherited conditions. A bad prime or point can make the filter omit an
equation that is independent over the exact field. That can lose discoveries;
it cannot create a false exact consequence. Accordingly, describe omissions as
"modularly dependent rows omitted by the finite search policy," not as proved
exact dependencies. Saved formulas retain their original candidate/conditional
authority throughout.

## Smallest useful integration

Start with saved equations only, in
`crates/rustred-core/src/reduction/terminal_relations/assistance.rs`, immediately
before the existing `normalized_row` / `add_row` path. Ordinary sources remain
unchanged and exact. Validate the complete provider batch as today before
filtering it.

Maintain a separate `SparseRowReducer<Zp64>` over the original saved-equation
keys, with its own stable append-only key-to-column inventory. Do not restrict
the probe to the terminal block, discard auxiliary terms, or recursively query
children. The unnormalized key space intentionally avoids coupling modular
state to the exact engine's generated-column aliases and row-space rebuilds.
It can miss additional redundancies exposed by those aliases; that is a useful
simplicity tradeoff for the first implementation.

For each equation:

1. Evaluate its rational-polynomial coefficients at one fixed finite-field
   point, using the authenticated family variable map. Remove modular zeros and
   provide sorted, unique column indices to Symbolica.
2. If any coefficient denominator vanishes, retain the original equation for
   exact processing without inserting a modular image. Likewise use exact
   fallback if an inherited nonzero condition vanishes at the sampled point,
   or its own rational denominator vanishes. Never divide by zero or treat a
   pole as a zero coefficient.
3. `add_row` returning a pivot selects the original exact equation; `None`
   permits omitting it. No modular coefficients, pivot values, or sampled
   conditions enter the exact matrix.
4. Preserve all original source, applicability and pole conditions of retained
   equations. Omitted equations need not contribute conditions to the result.

Use a fixed validated prime, for example the existing RustRed choice
2^61 - 1, and a deterministic generic point. Do not privilege physical integer
dimensions such as d = 4. Persist the actual ordered residues, not just an RNG
seed: replay should not depend on a later RNG implementation. The point order
must exactly match `CoefficientContext::parameter_names()` and both polynomial
variable maps. Zero-valued samples need not be categorically forbidden, but
nonzero generic coordinates avoid common unhelpful loci.

Do not resample just a failing row against a basis evaluated at a different
point. Per-row pole fallback is enough initially. A later multi-point filter
would need separately maintained, separately persisted bases and explicit
selection semantics. Feeding ordinary rows into the same original-key probe
after their exact acceptance is another optional later improvement, not needed
for saved-only duplicate suppression.

## Existing native APIs

The pinned and inspected vendor version is Symbolica 3.0.0; no new CAS or custom
sparse Gaussian-elimination kernel is required.

- `vendor/symbolica/src/poly/polynomial.rs:2775` provides
  `evaluate_with_coeff_map`. Map integer coefficients using `ToFiniteField`,
  evaluate numerator and denominator separately, check the denominator, then
  use native field division. The point-length assertion is not an input
  validator; validate shapes before calling it.
- `crates/rustred-core/src/solver/search.rs:669` and
  `solver/discovery/semi_numerical/frame.rs:104` already implement this rational
  evaluation pattern. The latter also filters modular zeros before insertion.
- `vendor/symbolica/lib/numerica/src/tensors/sparse.rs` exposes
  `SparseRowReducer::new` (1588ff), `add_row` (1791), `add_cols` (1853), `u`, and
  `pivots`. `LuLMode::None` suffices: no modular dependency certificate is used.
  Register new columns before testing a row; a full-rank old matrix otherwise
  correctly short-circuits only within its old column space.
- `SparseMatrix::try_from_csr` (492) and
  `SparseRowReducer::from_upper_triangular_matrix` (1755) support checkpoint
  restoration. Existing terminal `codec.rs` demonstrates structural and pivot
  validation for the exact counterpart.
- `FiniteFieldCore::from_element` / `to_element` support canonical residue
  storage. In the u64 implementation (finite_field.rs:992ff), elements use
  Montgomery representation internally; do not serialize private raw limbs.
- `algebra/matrix/right_kernel/modular.rs` validates the fixed prime before
  constructing `Zp64`. Its one-sided rank-screen interpretation differs from
  this optional row-omission policy and must not be copied as an authority
  claim.

## Checkpoint and retry policy

Prefer storing the modular basis directly. Persist a separately versioned
optional filter record containing:

- Policy version, prime, explicit ordered point, family identity and bound
  provider identity.
- Ordered modular column keys, CSR row pointers/indices/canonical residues,
  and pivots.
- Candidate-row ordinal, retained/fallback row ordinals or an equivalently
  bounded selection ledger, counters, and any pending exact-admission decision.

The exact checkpoint already contains the exact row space and the current
provider batch/cursor. Keep those mechanisms. Legacy unfiltered v1/v2 snapshots
must retain their old interpretation; filtered snapshots require a new schema
or explicit optional versioned section. Include the filter policy in application
resume bindings, and reject an in-place policy change. A source-mode change can
start a new finite session from the immutable source package, as for the current
assistance policy.

There is an important transaction boundary: once a row has been inserted into
the modular basis, retrying its probe would see that same row as dependent.
Therefore persist a `keep pending exact admission` decision tied to the current
batch cursor before an exact operation that can fail. Retry must replay the
original exact row without probing it again. Do not advance the equation cursor
or count exact completion until exact admission succeeds. Alternatively stage
the modular mutation until exact success, but cloning a growing basis per row
would undercut the performance goal.

Bound the selection ledger, column inventory, dense scratch and worst-case
modular fill before mutation, independently of exact retained bytes. Native
`add_row` itself does not provide an allocation-budget contract. On load, check
prime validity, residue ranges, family/variable/point agreement, unique key
arity, CSR shape, sorted nonzero row entries, normalized leading pivots, cursor
consistency and ledger counts. A typed resource-limit error should preserve a
retryable checkpoint; an exact-fallback-on-limit policy would need its own
persisted transition to avoid changing selection after resume.

Do not reconstruct this original-key modular basis from the current exact U
matrix: exact U lives in a normalized/aliased column space and does not retain
the same selection history. Replaying all historical provider rows is possible
only with an explicitly retained deterministic chronological source plan; the
current queried-key set alone is not that plan. Direct modular CSR storage is
the narrower and cheaper solution. Source-seed append can retain the modular
basis unchanged. Exact alias rebuilds can also retain it, precisely because the
probe is in the original-key space.

## Required tests and reporting

Test exact duplicates and scalar multiples; a genuinely independent saved row;
an intentionally unlucky point where an exact-independent row is omitted;
denominator and inherited-condition fallback; unresolved auxiliary children;
new columns after modular full rank; and exact-admission failure after modular
selection. Checkpoint at every decision/cursor boundary and compare both the
selection ledger and final exact substitutions with uninterrupted execution.
Test source extension and an exact alias rebuild without resetting the probe,
legacy decoding, policy mismatch rejection, malformed residues/pivots and
atomic resource refusal.

Report candidate equations consumed, modular independent selections, modular
dependent omissions, pole/condition fallbacks, pending exact selections,
completed exact saved-row insertions, exact independent rows, and modular
columns/nonzeros separately. Existing "completed assistance rows" must not
silently become a count of exact insertions when some rows are skipped.

Measure this as an optional backend policy against the same frozen source,
seed inventory, saved provider and algebraic mechanisms. Final exact rules
remain the evidence. Modular rank, lower runtime, or a completed filtered search
does not establish a minimal-master count.
