# Algebra and Symbolica boundary

RustRed has one production computer-algebra authority: the public Rust API of
GMP-enabled Symbolica. RustRed owns mathematical meaning, variable roles,
integral ordering, guards, provenance, resource admission, deterministic
scheduling, and proof interpretation. It does not own a second integer,
rational-function, polynomial, determinant, matrix-product, row-reduction, or
graph-isomorphism engine.

This document describes the live exact-algebra boundary and the audited limits
of the pinned dependency. It is not evidence that the narrow live tensor and
anchored/parametric foundry slices provide a complete projector or closing
foundry, nor that future artifact or reduction services are implemented.

## Pinned backend

The workspace requests `symbolica = "=2.2.0"` with default features disabled
and exactly the `gmp` and `tracing_max_level_info` features enabled in the
[`workspace manifest`](../Cargo.toml). The workspace root patches Symbolica,
Graphica, and Numerica to the checked-in
[`vendor/symbolica`](../vendor/symbolica) tree. That tree is pinned at revision
`77c137481904b8a5531ede86e3ef36b82beed7fd` and declares Symbolica 2.2.0 in its
[`Cargo.toml`](../vendor/symbolica/Cargo.toml).

Consequences of this configuration are deliberate:

- GMP-backed `Integer` is the arbitrary-precision integer authority;
- the `no_gmp` feature is unsupported and must never enter a RustRed build;
- disabling Symbolica defaults also avoids its process-global
  `faster_alloc`/mimalloc choice; and
- RustRed's registry-shaped exact dependency can be resolved to one compatible
  source by a consuming workspace, while this workspace remains reproducible
  through its root patch.

Rust standard integers remain appropriate for indices, shapes, checked
counts, prospective degree arithmetic, and resource budgets. Using them for
control data is not a second CAS. Algebraic values and operations remain
Symbolica values and operations.

## Search, compose, probe

Every proposed algebraic operation follows this gate before implementation:

1. **Search.** Define the exact domain and semantic result, pin the Symbolica
   revision/features, then search public exports, Rustdoc, source, examples,
   tests, internal production call sites, and existing RustRed adapters.
   Relevant starting points are `symbolica::prelude`, `symbolica::domains`,
   `symbolica::tensors`, and the public Atom/polynomial traits.
2. **Compose.** Prefer a checked composition of public operations. RustRed may
   add map/shape authentication, physics structure, resource admission, panic
   containment, guards, stable interpretation, and exact replay, but
   Symbolica still performs the algebra.
3. **Probe.** Compile and run a small licensed-GMP probe for empty, zero,
   singular, overflow, malformed-map, one-below-limit, equivalent-spelling,
   and deterministic parallel cases. Authenticate the result with an exact
   native product, substitution, divisibility check, or regenerated residual.
4. **Stop honestly.** If public operations cannot be composed into the needed
   semantic, record the searched surface and missing upstream capability and
   return a typed unsupported or resource-limited result. Do not fill the gap
   with a RustRed-authored CAS primitive.

Performance preference, familiarity with an older implementation, or a more
convenient representation is not an upstream gap. The search must be repeated
when the Symbolica revision or feature graph changes.

## Current coefficient ownership

The public scalar facade is
[`algebra`](../crates/rustred-core/src/algebra/mod.rs). Its live types are
organized by field role rather than by historical pipeline stage.

| Owner/type | Exact domain | Responsibility |
|---|---|---|
| `CoefficientPolynomial` | `MultivariatePolynomial<IntegerRing, u16>` | The sole raw sparse integer-polynomial representation shared by core domains. |
| `Coefficient` | `RationalPolynomial<IntegerRing, u16>` | Exact base-field values `K = Q(theta)` in one authenticated ordered map. |
| `CoefficientContext` | Ordered names, `Arc<Vec<PolyVariable>>`, and a same-map template | Namespaced symbol construction, constants/parameters, map authentication, and checked base-field operations. |
| `IndexedPolynomial` | Polynomial in the ordered map `(theta, n)` | Authenticated polynomial condition over the index-extended context. |
| `IndexedCoefficient` | Rational function in `(theta, n)` | Authenticated coefficient in `K(n)` for parametric relations. |
| `IndexedCoefficientContext` | One base context plus private index symbols and a scope fingerprint | Explicit `K -> K(n)` lifting, indexed arithmetic, translation, and guarded projection back to `K`. |

The concrete aliases, context, validation, and checked base operations live
under
[`algebra/coefficient`](../crates/rustred-core/src/algebra/coefficient/mod.rs).
The index-extended owner is split under
[`algebra/indexed`](../crates/rustred-core/src/algebra/indexed/mod.rs) into
context, value, scope, limits, translation, specialization, and errors. There
is no parallel `parametric_coefficient` stack.

Stored polynomial exponents use Symbolica's native `u16`. Prospective sums and
powers widen through checked `u32` or `u64` arithmetic before entering native
code. `u128` is not an exponent representation. Term and allocation counts use
checked `usize`; integer-magnitude bounds have their own checked width.

### Base-field contract

`CoefficientContext` constructs one exact ordered variable map. Its checked
`try_add`, `try_sub`, `try_mul`, `try_div`, and `try_neg` paths:

- require both numerator and denominator to use exactly that map;
- reject malformed sparse layouts, explicit numerical zero terms,
  noncanonical monomial order, and a zero denominator;
- preflight exponent and visible term-operation bounds before panic-prone
  native arithmetic; and
- authenticate the normalized Symbolica result again, because rational
  cancellation may produce more retained terms than a sparse input-pair
  estimate predicts.

Symbolica ordinarily unifies differing polynomial maps. RustRed forbids that
implicit extension on proof-bearing values. Context equality and map
authentication are semantic checks, not convenience wrappers around a second
coefficient implementation.

### Indexed-field contract

`IndexedCoefficientContext::try_new` appends private index symbols to the base
map and binds their order and caller-provided scope into an exact fingerprint.
Public values carry that full context identity and do not expose unchecked
constructors. Native Symbolica names come from one deterministic, versioned
private positional pool shared by every indexed context in the process.
Symbolica therefore retains at most one native symbol for each admitted index
position rather than a distinct namespace for every family. Compatibility and
authentication still compare the complete fingerprint; shared native symbols
never make different scopes interchangeable. Family construction bounds the
pool through `IntegralFamilyLimits::max_scalar_products`, and the generator's
default `IndexedContextLimits` uses the same 4096-position ceiling. Callers
that raise the family ceiling can raise `ParametricIbpConfig::context_limits`
explicitly. Direct low-level construction accepts the same configurable
policy through `IndexedCoefficientContext::try_new_with_limits`; its limits
cover index arity, exact fingerprint bytes, and aggregate native-name work.

The current indexed services are:

- exact lifting of a base coefficient or base polynomial into the extended
  map;
- checked add/subtract/multiply/divide/negate in `K(n)`, plus authenticated
  numerator and denominator extraction for guard construction;
- integer translation `n -> n + a`, implemented with Symbolica polynomial
  `replace_with_poly` after a complete support/bit/work preflight; and
- simultaneous integer specialization back to `K`, returning both the
  normalized coefficient and the mapped nonconstant denominator before
  cancellation. A caller must retain that polynomial as a nonzero guard.

The translation is restricted to commuting per-index integer shifts. It is not
a general mixed affine-substitution service. Specialization performs bounded
structural traversal and constructs Symbolica integer/polynomial values; it
does not define independent integer or polynomial arithmetic. Any expansion
of this surface, especially general affine substitution, must pass the
search-compose-probe gate rather than grow a handwritten substitution engine.

### Feynman-polynomial contract

[`family/symanzik`](../crates/rustred-core/src/family/symanzik/mod.rs) binds the
outer polynomial ring `K[x_0,...,x_{N-1}]` to an authenticated family. The live
context uses Symbolica's public `MultivariatePolynomial` operations directly:
native template `constant`/`monomial` construction, borrowed add/subtract and
multiply, `mul_coeff`, and `derivative(variable)`. RustRed does not collect
those results through a parallel map-based polynomial implementation.

Every operand first crosses the exact family fingerprint, coefficient-field,
and ordered-variable-map boundary. Sum/product term slots, dense exponent
entries, aggregate structural work, and prospective product degrees are then
checked before native entry. Native panics become typed errors; every retained
result is authenticated and rebound to the context-owned variable map,
including identically-zero results.

Symbolica 2.2.0 does not expose a fallible callback or scratch census inside
its outer-polynomial coefficient operations, nor does it expose which private
dense/heap multiplication lane was selected. Consequently the configured
exact-coefficient limits can be enforced on inputs and retained outputs, but
not on every transient rational-function coefficient created inside one
native outer-ring call. The composition makes that gap explicit: prospective
outer structure is admitted, the infallible native call is unwind-contained,
and its result is post-authenticated. This boundary is not a hard RSS limit and
must not be described as one.

## Current matrix ownership

[`algebra/matrix`](../crates/rustred-core/src/algebra/matrix/mod.rs) is private
to the core. Family, symmetry, ISP, and zero-sector services call narrow
domain-neutral functions instead of depending on raw matrix behavior.

Symbolica's generic `Matrix<F>` requires an infallible `Ring`/`Field`
implementation. RustRed's private
[`CheckedCoefficientField`](../crates/rustred-core/src/algebra/matrix/field/mod.rs)
adapts one authenticated `CoefficientContext` to those traits. It delegates
every scalar operation to Symbolica while tracking admitted operations and
carrying typed RustRed failures through a private unwind payload. Consequently
this boundary requires `panic = "unwind"`; a `panic = "abort"` build is
rejected at compile time.

The implemented native matrix operations are:

- rectangular exact rank through Symbolica's destructive
  `Matrix::partial_row_reduce`;
- determinant through `Matrix::det`;
- inverse through `Matrix::inv`, but only after an independent nonzero native
  determinant guard and followed by native products in both orders;
- two- and three-matrix products, transpose-backed congruence, and independent
  inverse replay; and
- one deterministic primitive integer right-kernel witness for the
  zero-sector rank proof.

For the last operation, Symbolica owns rational RREF, rational
`Matrix::primitive_part`, integer classification, and the final `Matrix<Z>`
product. RustRed only authenticates the dense exponent matrix, selects the
first free RREF column, fixes a deterministic sign, and interprets the result.
The implementation is in
[`algebra/matrix/right_kernel`](../crates/rustred-core/src/algebra/matrix/right_kernel/mod.rs).
It is intentionally not a full nullspace or integer-lattice engine.

## Admission and resource boundaries

RustRed admits work before native entry and authenticates returned values
afterwards. The live policies are layered:

| Boundary | Visible limits and checks |
|---|---|
| `ExactAlgebraLimits` | `u16` exponent ceiling, retained polynomial terms, and sparse term-operation envelope |
| `IndexedAlgebraLimits` | Base exact limits plus specialization/translation power work and prospective integer magnitude |
| `FeynmanPolynomialLimits` | Family-map authentication, prospective native sum/product slots and degrees, dense exponent entries, aggregate structural work, determinant structure, and retained exact-coefficient checks |
| `SymbolicaCoefficientMatrixLimits` | Shape and `u32` conversion, largest single/live matrix payload, exact-operation envelope, input clone-owned bytes, and authenticated output bytes |
| `RightKernelLimits` | Rows, columns, entries, RREF work and integer-bit bounds, witness length, and witness integer bits |
| Input/family/sector owners | Syntax depth and bytes, affine projection work, family dimensions, and owner-specific proof/result limits before they call algebra |
| Campaign/application owners | Invocation core and RAM ceilings, bounded transport payloads, and deterministic worker-width admission |

Fallible Rust allocations are reserved before caller-sized buffers are filled
where the public API permits it. Dense matrices are staged with checked
row-major `Matrix::from_linear`, not the empty/zero-column-sensitive
`from_nested_vec`. Output maps, shapes, sparse layouts, exponents, integer
sizes, and retained bytes are checked after native calls.

These limits are truthful but not omnipotent. Symbolica 2.2.0 does not expose
a complete scratch-memory census or cancellation token for exact matrix,
polynomial GCD/quotient, factorization, rational power, or graph-canonization
work. A prospective envelope plus post-authentication is therefore not a hard
RSS proof. Such an operation must retain calibrated native headroom and a typed
failure boundary; documentation and artifacts must not relabel it as fully
memory-bounded.

Authentication is proportional to trust. Untrusted inputs and future durable
artifacts receive full checks. Once a private or sealed value has crossed that
boundary, callers use its type and ownership instead of repeatedly serializing,
fingerprinting, or replaying it between every internal function.

## Pinned 2.2.0 defects and mitigations

The following are revision-specific source and runtime findings. They explain
the checked compositions in RustRed; they do not authorize replacement
algebra.

| Symbolica 2.2.0 behavior | RustRed policy |
|---|---|
| Generic dense `Matrix::inv` can accept a singular coefficient block because the identity half of `[A|I]` participates in its rank test. | Compute `Matrix::det` first, reject zero, call `inv`, and verify `A A^-1` and `A^-1 A` entry by entry with native matrix products. |
| Dense and sparse matrix `is_one` are incorrect on important zero/identity cases. | Never use a matrix-wide `is_one` as proof; inspect the authenticated native product's diagonal and off-diagonal coefficients. |
| `det_in_place` loses row-swap parity. | Use `Matrix::det` at proof-bearing boundaries. |
| Dense single-row indexing uses the wrong stride for rectangular matrices. | Use tuple indexing and iterators, never `Index<u32>` row slices. |
| Dense rank panics for a valid `0 x N` matrix with `N > 0`. | Reject empty coefficient matrices before native reduction; handle structural empty cases outside that call. |
| Sparse inverse shares the augmented-matrix defect, and sparse inconsistent-row detection is inverted. | The anchored and parametric foundry paths do not use either operation. They pin the adopted `SparseRowReducer` pivot/`U`/`L` behavior and exactly replay every returned candidate from source rows. |
| Multivariate multiplication is infallible, selects private dense/heap lanes, and can panic when fixed-width exponents overflow; its coefficient-ring and scratch censuses are opaque. | Authenticate inputs, preflight the outer product count and per-variable degree sums, invoke native arithmetic, then authenticate and exactly rebind the result. The known exponent-overflow path is excluded prospectively; standalone coefficient operations do not claim a general unwind boundary. |
| Rational-polynomial power is infallible, rejects exponents above `u32::MAX` by panic, uses a linear multiplication schedule, and can overflow `u16` degrees. | Checked matrix sessions preflight exponent, degree box, terms, operations, and retained output; their native-session transport contains unwind and reauthenticates the result. Do not replace it with a RustRed power algorithm. |
| Symbolica `Integer` equality/hash and zero/one predicates can observe noncanonical backend variants. | Construct values through canonical conversions and numerically reject every representation of zero during sparse-polynomial authentication. |

The relevant upstream implementations are the pinned
[`dense matrix`](../vendor/symbolica/lib/numerica/src/tensors/matrix.rs),
[`sparse matrix`](../vendor/symbolica/lib/numerica/src/tensors/sparse.rs),
[`integer domain`](../vendor/symbolica/lib/numerica/src/domains/integer.rs), and
[`rational-polynomial domain`](../vendor/symbolica/src/domains/rational_polynomial.rs),
as well as the
[`multivariate polynomial`](../vendor/symbolica/src/poly/polynomial.rs).

## Known public-API gaps

At the pinned revision, exhaustive public-API searches found no directly
usable service for:

- Smith or Hermite normal form, a complete integer kernel basis, or a complete
  underdetermined integer-affine solution lattice;
- a full right-nullspace convenience API (the current zero-sector composition
  deliberately returns only one witness);
- target-directed sparse parametric candidate selection, reducer cancellation,
  and a hard native scratch-memory census (the live fixed-sector path supplies
  RustRed ordering, guards, chronology, and exact replay around Symbolica's
  fixed-pivot reducer);
- a complete multivariate rational-function reconstruction service;
- fallible, caller-cancellable, resource-censused polynomial
  GCD/quotient/factorization and rational-power sessions;
- fallible selective tensor-subtree expansion that simultaneously preserves
  an opaque spectator grammar and caller budgets; or
- a hard-bounded/cancellable graph-canonization operation.

`SparseRowReducer` is the live primitive for both the concrete-anchor and
fixed-sector parametric foundry boundaries. Columns are reordered by RustRed
complexity, chronological identity columns recover source combinations from
public `U`, and public `L` retains pre-normalization pivots. The parametric path
uses Symbolica's native rational-polynomial representation for `K(n)` directly;
RustRed supplies map authentication, guards, structural admission, uniform
descent, and exact source replay. The native reducer's fixed pivot semantics,
lack of cancellation, and opaque scratch memory remain explicit limitations,
not permission to write another sparse field reducer.

General integer-affine work that needs a missing normal form stops at a typed
unsupported boundary. The same rule applies to every other gap. A typed pause
is a truthful capability result; a private replacement CAS is not.

## Licensing and concurrency

Configure the Symbolica license through the `SYMBOLICA_LICENSE` environment
variable before the first Atom, parser, symbol, domain, or matrix call and
before constructing worker pools. Do not persist the license value in
executable source, tests, artifacts, or logs. The non-sensitive value reproduced
verbatim in the user-mandated preamble of `GOAL.md` is the sole documentation
exception. Access to the source checkout is separate from redistribution
permission; CI, wheels, and deployment must comply with the applicable
Symbolica license.

Symbolica symbols are process-global and their numeric identities depend on
registration order. Persist names and ordered variable manifests, never raw
symbol IDs. A parallel operation must finish deterministic symbol/context
construction on its coordinator before dispatching workers and must not mutate
the registry from those workers.

The live [`campaign::ParallelExecution`](../crates/rustred-core/src/campaign/execution.rs)
implements one invocation-wide policy:

- `n_cores = 1` runs inline and creates no Rayon worker;
- a larger request is rejected if it exceeds process availability or
  Symbolica is not licensed;
- an accepted request owns one private fixed-width Rayon pool; and
- derivation preflight supplies the maximum exact result count of its selected
  batches, which the executor retains as a hard ceiling; and
- each batch reserves its exact ordered result buffer fallibly before work is
  dispatched, then collects by stable ordinal so worker arrival order does not
  alter the result or first-error order.

This is foundational execution infrastructure, not yet the memory-sharing
architecture of a six-loop foundry. The future worker model must benchmark
Symbolica TLS/scratch, avoid nested pools and per-task forks, and share
immutable family/source data instead of cloning complete symbolic state for
each lane.

The Python adapter releases the GIL, but all top-level calls enter the
application through one process-wide coordinator thread. Its zero-capacity
queue provides backpressure; a caught panic poisons later work, and a
post-fork process mismatch is rejected. Inner RustRed parallelism remains
controlled by the request's one `ParallelExecution` object.

## Upgrade probe gate

Before changing the Symbolica revision, source, or features:

1. Prove with manifests, lockfile, and source-qualified `cargo tree` output
   that exactly one compatible Symbolica/Graphica/Numerica identity is
   resolved, GMP is enabled, and `no_gmp` is absent.
2. Re-read the public traits, examples, tests, and internal call sites for
   every RustRed operation; a passing old wrapper is not proof that the API or
   schedule is unchanged.
3. Re-run dense probes for singular inverse, both identity predicates,
   row-swap determinant sign, `0 x N` rank, rectangular indexing, exact solve,
   and malformed/empty construction.
4. Re-run sparse probes for singular inverse, consistent/inconsistent solve,
   reducer decomposition, back substitution, and any parallel output-order
   guarantee used by RustRed.
5. Re-run coefficient probes for strict map retention, implicit extension,
   empty coefficient lists, exact substitution arity, rational checked
   division, power limits, degree overflow, noncanonical integer zero, GMP
   payloads, and one-below resource limits.
6. Re-run the right-kernel probes for mixed denominators, primitive content,
   deterministic first-free-column choice, sign, structural empty rows, and
   exact native `Matrix<Z>` replay.
7. In disposable helper processes, verify license-first initialization,
   multi-threaded Atom/polynomial work, deterministic symbol registration,
   and the owned Rayon pool. Never test a known unlicensed cross-thread abort
   inside the main test runner.
8. Run the complete licensed default-GMP RustRed suite and exact domain-level
   replay tests before deleting or weakening any mitigation.

Existing FORM-backed Vakint implementations may supply results to a
separately pinned oracle-regeneration job. They are not algebra providers,
dependencies, or fallbacks for RustRed or Vakint's RustRed mode. Ordinary
development, tests, and production remain pure Rust plus Symbolica.
