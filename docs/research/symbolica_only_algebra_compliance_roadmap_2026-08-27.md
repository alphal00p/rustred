# Symbolica-only production algebra compliance roadmap

Date: 2026-08-27; consolidated 2026-08-28

> **Current subordinate roadmap.** This document is subordinate to
> [`GOAL.md`](../../GOAL.md). It records the CAS-authority gates that remain
> valid through the repository reset. File ownership and implementation status
> come from the live tree and the source-liveness ledger, not from historical
> prototype names in Git.

## Contract

All production computer algebra is performed through public Symbolica APIs.
RustRed owns physics semantics, typed validation, integral ordering, guards,
provenance, resource admission, deterministic scheduling, and exact replay. It
does not own a second polynomial, rational-function, determinant,
matrix-product, row-reduction, graph-isomorphism, or integer-normal-form
engine.

The production configuration is pure Rust plus licensed, GMP-enabled
Symbolica. It invokes no FORM process, Mathematica kernel, SymPy, or Symbolica
`no_gmp` path. Before any algebra is implemented locally, search Symbolica's
public Rust API, rustdocs, examples, tests, and internal call sites. If an
essential operation is absent or cannot be safely composed, return a typed
unsupported or resource-limited outcome and record the precise upstream gap.

A wrapper is permitted only to add domain authentication, physics structure,
admission, cancellation boundaries, panic containment, provenance, and replay
while Symbolica still performs every algebraic operation.

## Phase-0 algebra ownership gates

| Lane | Required Symbolica-owned algebra | RustRed-owned boundary |
|---|---|---|
| Family and raw identities | exact coefficient conversion, determinant, inverse, matrix/vector products, polynomial arithmetic | momentum/scalar-product semantics, affine layout, family conditions, exact replay |
| Zero sectors | polynomial construction, differentiation, face substitution, determinant/rank | sector masks, cuts, sufficient-rank theorem, certificate dependencies |
| Symmetry | graph canonization/isomorphism, matrix products, determinants, exact solving | graph encoding, routing proposals, family verification, Jacobian and induced-index proof |
| Identity specialization | simultaneous polynomial substitution, translation, normalization | context/arity checks, condition sources, arithmetic limits, exceptional-domain classification |
| Foundry elimination | sparse row arithmetic, pivots, normalization, dependencies | physical column order, row provenance, pivot conditions, descent, publication |
| Tensor numerator path | bounded native expansion where semantically equivalent, polynomial lowering, exact projector matrices | tensor grammar, dummy-index safety, pairing/orbit structure, spectator semantics |
| Artifact application | exact specialization, multiplication, addition, collection | guarded dispatch, integral-key traversal, termination, memoization, master policy |

A lane is not Symbolica-only merely because its stored values are Symbolica
types. Its reachable production call graph must contain no handwritten CAS
schedule for the operations in the middle column.

## Required checked compositions

- Authenticate one coefficient context and ordered variable map before native
  entry; reject implicit map extension or foreign values.
- Admit visible input/output ownership and calibrated opaque native scratch
  before allocation or cloning, then authenticate returned term counts,
  exponents, integer sizes, maps, and denominators.
- For inverse-dependent work, compute native determinant first, reject zero,
  call native inverse, and replay both products entry by entry. Do not rely on
  the affected 2.2.0 `is_one` methods.
- Use native `SparseRowReducer` for forward row algebra after mapping the
  physical ordering to Symbolica's pivot columns. Preserve guards and source
  combinations by replaying its public decomposition; do not recompute them
  with a second solver.
- Use native graph canonization only behind calibrated admission. The audited
  revision has no hard cancellation or retained-memory bound.
- Keep per-lane algebra limits as typed mathematical/resumable outcomes.
  Invocation-wide core/RAM admission schedules work and atomically reserves
  owners/clones; it does not rewrite those limits or reinterpret a pause.

## Public Symbolica gaps at the pinned revision

1. There is no public Smith/Hermite normal form, integer kernel basis, or
   complete integral-affine solution-lattice API. A literal-unit-pivot affine
   subset may compose native primitives; the general case is typed
   unsupported, never a handwritten lattice CAS.
2. Exact matrix, sparse reducer, polynomial GCD/quotient/factorization,
   rational power, and graph canonization APIs lack a caller cancellation
   token and complete retained/scratch-memory census. Conservative admission,
   calibrated headroom, and panic containment are required, but cannot be
   presented as a hard native-memory proof.
3. No documented fallible selective tensor-subtree expansion simultaneously
   preserves RustRed's opaque spectator grammar and exposes caller budgets.
   Retain only syntax control proven by differential tests; algebra within the
   accepted subset remains native.
4. No complete public multivariate rational-function reconstruction service is
   exposed. RustRed may own sample scheduling, support discovery, CRT assembly,
   bad-sample rejection, and exact replay while delegating field/polynomial
   primitives to Symbolica.
5. `Graph::canonize` has an unbounded internal leaf collection and no public
   step/cancellation interface; candidate generation remains admitted and
   contained rather than declared hard-bounded.

Detailed evidence and revision-specific correctness defects are frozen in
[`symbolica_rust_api_for_litered.md`](symbolica_rust_api_for_litered.md),
[`symbolica_exact_linear_algebra_api_inventory.md`](symbolica_exact_linear_algebra_api_inventory.md),
and
[`symbolica_upstream_gap_audit_2026-08-25.md`](symbolica_upstream_gap_audit_2026-08-25.md).

## Closure evidence

For every migrated lane:

1. inspect the complete production call graph and record which public
   Symbolica APIs own each algebraic operation;
2. add exact success, malformed-context, singular/zero, and one-below-resource
   tests, including panic recovery where the native API is infallible;
3. compare algebraically equivalent input forms and independently replay every
   proof-bearing output;
4. run licensed default-GMP tests in serial and parallel configurations; and
5. remove the superseded handwritten implementation and its compatibility
   facade rather than retaining a dormant second authority.

No acceptance command may enable `no_gmp` or invoke FORM or Mathematica.
Passing one source audit, type migration, or oracle fixture is not evidence of
six-loop closure.
