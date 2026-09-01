# Symbolica public-API audit for K=6 closure

Date: 2026-09-01

Status: verified API inventory and experiment plan; no performance result

## Scope and claim discipline

This note records a read-only audit of the public Rust APIs available to RustRed for the complete
three-loop `K = 6` closure campaign and for later high-loop scaling work. Here `K` is family arity:
the number of independent scalar-product coordinates, not a graph label.

The audit covers exact sparse and dense linear algebra, polynomial and rational-function
arithmetic, finite fields and reconstruction primitives, graph canonization and automorphisms,
and streaming or parallel execution. It also checks the RustRed foundry path for local CAS
reimplementations and unnecessarily deep Symbolica wrappers.

Statements labelled **verified** are source-level observations. Statements labelled **proposed
experiment** are unmeasured hypotheses. None of the proposed changes below has demonstrated a
speedup, memory reduction, or K=6 closure improvement yet. Promotion therefore requires a
controlled benchmark followed by the existing exact replay and artifact admission checks.

## Audited version and source equivalence

**Verified.** RustRed pins `symbolica = 2.2.0` and patches it to `vendor/symbolica` in the workspace
`Cargo.toml`. Both audited source trees

- `vendor/symbolica`, and
- `FOR_REFERENCE_ONLY_DO_NOT_PUSH/symbolica`

are at commit `77c137481904b8a5531ede86e3ef36b82beed7fd`, declare version 2.2.0, and have identical
copies of the API modules cited below. The vendored tree is the implementation authority for
RustRed builds; the reference-only tree must never be committed as RustRed source.

Relevant public entry points are reexported from `vendor/symbolica/src/lib.rs`. Sparse tensors live
under `vendor/symbolica/lib/numerica/src/tensors/sparse.rs`; Graphica is reexported as
`symbolica::graph` from the main library.

## Native delegation already used correctly

The K=6 foundry does not contain a competing symbolic algebra or Gaussian-elimination engine.
Its main CAS operations already delegate to Symbolica:

| RustRed responsibility | Verified native delegation |
| --- | --- |
| Exact coefficient model | `crates/rustred-core/src/algebra/coefficient/model.rs` aliases `RationalPolynomial<IntegerRing, u16>` and native Symbolica polynomials. `u16` agrees with the positive exponent types documented in `vendor/symbolica/src/poly.rs`; RustRed does not use `u128` polynomial exponents. |
| Index translation | `crates/rustred-core/src/algebra/indexed/translation.rs` calls `MultivariatePolynomial::shift_var`. |
| Fixed-index specialization | `crates/rustred-core/src/algebra/indexed/specialization.rs` calls `MultivariatePolynomial::replace`. |
| Coefficient normalization | `crates/rustred-core/src/algebra/indexed/base_coefficients.rs` uses Symbolica GCD and factorization. |
| Modular rank discovery | `crates/rustred-core/src/foundry/completion/frame/modular/rank.rs` constructs `SparseMatrix` and uses `SparseRowReducer::new` plus incremental `add_row`. |
| Modular obstruction solve | `crates/rustred-core/src/foundry/completion/frame/modular/obstruction.rs` uses native sparse back substitution. |
| Exact lift and target reduction | `crates/rustred-core/src/foundry/completion/frame/exact/reduce.rs`, `crates/rustred-core/src/foundry/parametric/sparse.rs`, and `crates/rustred-core/src/foundry/target_rref.rs` use `RationalPolynomialField` with the native sparse reducer. |
| Small integer right kernel | `crates/rustred-core/src/algebra/matrix/right_kernel` calls native dense row reduction, then extracts and replays a deterministic null vector locally. Symbolica 2.2.0 has no public nullspace method, so the extraction is justified. |
| Symanzik construction | `crates/rustred-core/src/family/symanzik/operations.rs` calls native determinant operations. The local adjugate construction is skipped for vacuum families and is not a K=6 hot path. |

RustRed also correctly evaluates modular rational coefficients as separate numerator and
denominator polynomials in
`crates/rustred-core/src/foundry/completion/frame/modular/sample.rs`. This preserves explicit
denominator-zero rejection and guard evidence; replacing it with an opaque evaluation that loses
that distinction would be a semantic regression.

## Prioritized experiments and required measurements

### 1. Optimized Mersenne finite-field lane

**Verified API.** `vendor/symbolica/lib/numerica/src/domains/finite_field.rs` publicly defines:

- generic `Zp` and `Zp64` Montgomery fields;
- `Mersenne32`, with modulus `2^31 - 1`; and
- `Mersenne64`, with modulus `2^61 - 1`.

The Symbolica source describes the specialized Mersenne arithmetic as faster than its generic
Montgomery implementation. That is a statement about the library implementation, not a measured
RustRed result.

**Proposed experiment.** Add an internal monomorphic `Mersenne64` bulk-probe lane and benchmark it
against the current `Zp64` lane on identical K=6 frames, source order, sample points, and resource
budgets. Test `Mersenne32` separately when coefficient storage or memory bandwidth appears
dominant. Record:

- coefficient-evaluation throughput;
- sparse-reducer wall time;
- reducer fill and peak resident memory;
- rank and pivot fingerprints; and
- exact candidates admitted per unit of work.

Retain independent primes and points for confirmation. A modular candidate has no publication
authority until exact lifting and full regenerated-source replay succeed. Do not make the entire
foundry generic over field types merely to run this experiment; dedicated monomorphic lanes avoid
injecting abstraction into the hot loop.

### 2. Cached field conversion or compiled expression evaluation

**Verified API.** The current sampler repeatedly invokes `evaluate_with_coeff_map` on exact
integer polynomials. Symbolica additionally exposes:

- `RationalPolynomial::to_finite_field` in
  `vendor/symbolica/src/domains/rational_polynomial.rs`;
- coefficient mapping and polynomial evaluation in
  `vendor/symbolica/src/poly/polynomial.rs`;
- `Atom::evaluator_multiple` in `vendor/symbolica/src/atom/core.rs`; and
- reusable `ExpressionEvaluator<T>`, ring evaluation, and `map_to_ring` in
  `vendor/symbolica/src/evaluate/evaluator.rs`.

**Proposed experiment.** Compare three bounded strategies:

1. the current direct exact-polynomial evaluation;
2. converting a frame to finite-field polynomials once per modulus and reusing it across sample
   points; and
3. compiling stable chunks of numerator, denominator, and guard expressions into reusable
   evaluators, then mapping them to each field.

Measure build cost, per-point cost, peak memory, expression size, and amortization point. Keep
numerators and denominators as separate evaluator outputs so denominator-zero chronology remains
observable. Evaluator compilation, Horner rewriting, and common-subexpression elimination may
increase memory or fail to amortize, so this remains a benchmark rather than a prescribed
replacement.

### 3. Exact Groebner unit-ideal test as a cold guard refiner

**Verified API.** `vendor/symbolica/src/poly/groebner.rs` exposes the F4-style
`GroebnerBasis::new` and public normal-form reduction over a field.

**Proposed experiment.** In a strictly budgeted cold path, convert a difficult multivariate guard
system to exact rational-coefficient polynomials and ask whether its Groebner basis contains one.
When it does, this proves that the guard equations have no common solution even over the
algebraic closure and can discharge that exceptional-domain branch.

This is only a sufficient emptiness certificate. A non-unit ideal does not prove the existence of
an integer-lattice exceptional branch, and commutative F4 must not be treated as a parametric-IBP
shift-module basis. Every retained IBP relation still requires ordinary-source provenance and
exact replay.

### 4. Trim wrappers inside sealed exact hot loops

**Verified implementation finding.** `CoefficientContext` is valuable at an untrusted boundary:
it fixes the coefficient-variable identity and rejects undeclared-variable unification. However,
`crates/rustred-core/src/algebra/coefficient/operations.rs` and
`crates/rustred-core/src/algebra/indexed/context/arithmetic.rs` preflight and authenticate many
individual scalar operations. Exact replay then invokes those operations repeatedly in
`crates/rustred-core/src/foundry/completion/frame/exact/replay.rs` and
`crates/rustred-core/src/foundry/parametric/replay.rs`.

The deeper `CheckedCoefficientField` adapter in `crates/rustred-core/src/algebra/matrix/field`
forwards Symbolica Ring and Field traits through `Rc<RefCell<_>>` counters and panic-to-typed-error
translation. It is non-`Send`/`Sync` and adds wrapper work around scalar operations. The main
foundry sparse reducers already use raw native fields instead, so this adapter is not identified
as the current K=6 blocker.

**Proposed experiment.** Preserve one-time boundary validation, context fingerprints, resource
admission, exact final replay, and publication postconditions. Inside a sealed replay or exact
accumulation loop, compare the checked path with a trusted native `RationalPolynomialField`
accumulator followed by one final validation. Measure wall time and peak intermediate term counts;
do not remove semantic replay or guard checks. Restrict `CheckedCoefficientField` to cold audit or
debug paths only if measurements demonstrate material overhead and equivalent failure coverage.

The prospective term, bit, and work formulas elsewhere in the algebra layer are admission
policies rather than reimplemented CAS algorithms. Any simplification must retain coarse resource
ceilings and typed failure, even if repeated per-operation scans are removed.

## Graphica guidance

**Verified API.** `vendor/symbolica/lib/graphica/src/lib.rs` provides:

- `Graph::canonize`, returning a canonical graph and automorphism orbit generators;
- `Graph::is_isomorphic`;
- edge canonical sorting and automorphism-size support; and
- `GenerationSettings` with `Graph::generate` for bounded graph generation.

`Graph::generate` returns a materialized map rather than a streaming iterator. Canonization
exposes vertex automorphisms, not a complete denominator-edge permutation and loop-momentum
routing witness. No public stabilizer chain, Schreier--Sims engine, or canonical minimum under
RustRed's integral-key ordering was found.

RustRed currently uses Graphica principally in K4 ordering telemetry and tests under
`crates/rustred-core/src/foundry/artifact/three_loop/ordering_portfolio.rs`. The production K4 lane
uses explicit momentum maps in
`crates/rustred-core/src/foundry/artifact/three_loop/symmetry.rs`, followed by exact verification
and compilation.

**Proposed use.** For generic future families, use Graphica to canonicalize topology graphs and
discover candidate automorphism generators. Derive the induced denominator permutation and
loop-momentum map in RustRed, then pass both through the existing exact routing, Jacobian, and
kinematic verifier. Graph equality alone must never authenticate a routing, especially for
multiedges.

The current full-group closure in
`crates/rustred-core/src/sector/symmetry/canonical/action.rs` is reasonable for the small K4 group.
At higher loop order, profile the number of stored elements and key transports before replacing
it. Graphica does not currently provide the action-specific canonicalizer needed as a drop-in
replacement.

## Parallel and streaming guidance

**Verified API.** Symbolica provides `SparseRowReducer::back_substitute_parallel` in
`vendor/symbolica/lib/numerica/src/tensors/sparse.rs`. Its own documentation states that this path
performs more total work than serial back substitution and may permute output rows. Sparse forward
elimination remains serial. The implementation uses Rayon without accepting a RustRed-owned pool.

`TermStreamer` in `vendor/symbolica/src/streaming.rs` supports memory- or disk-backed Atom term
sorting and multicore term maps. It is specialized for symbolic Atom streams; it is not a generic
transport for RustRed source requests, CSR rows, proof objects, or worker communication.

RustRed already owns a bounded ordered Rayon pool in
`crates/rustred-core/src/campaign/execution.rs`. The probe campaign separates immutable evaluation
from the serial mutation boundary in
`crates/rustred-core/src/foundry/completion/source_discovery/probe_campaign/run.rs`.

**Proposed use.** Prefer outer parallelism over independent primes, points, or immutable source
evaluations while preserving a serial deterministic ledger commit. Benchmark native parallel
back substitution only for sufficiently large modular systems, normalize any row ordering through
the pivot map, and prevent nested Rayon oversubscription. Do not adopt `TermStreamer` for the
foundry data plane without evidence that an Atom conversion is both semantically natural and
resource-positive.

The reducer cloning in
`crates/rustred-core/src/foundry/completion/source_discovery/probe_campaign/obstruction_block/select.rs`
is capped by an obstruction-block width of four. It is therefore not presently identified as a
credible K=6 scaling bottleneck.

## Public APIs not found

A repository-wide inspection of the audited Symbolica 2.2.0 source did not find public APIs for:

- dense or sparse nullspace/kernel extraction;
- a non-mutating sparse rank-update preview, snapshot, or rollback;
- sparse column views or projected matrix views;
- parallel sparse forward elimination;
- sparse fraction-free elimination;
- Wiedemann or another black-box sparse linear solver;
- a complete sparse multivariate rational-function reconstruction controller;
- a graph stabilizer chain or an edge-action canonicalizer respecting RustRed's complexity order;
- free-module or syzygy Groebner bases; or
- Ore/difference-algebra parametric-IBP completion.

“Not found” refers only to the public Rust API of the audited commit. It is not a claim about a
newer, private, or planned Symbolica API.

Symbolica does expose useful lower-level reconstruction primitives:

- `Integer::chinese_remainder` in
  `vendor/symbolica/lib/numerica/src/domains/integer.rs`;
- rational and maximal-quotient reconstruction in
  `vendor/symbolica/lib/numerica/src/domains/rational.rs`;
- polynomial CRT in `vendor/symbolica/src/poly/polynomial.rs`; and
- univariate Newton interpolation in `vendor/symbolica/src/poly/gcd.rs`.

A future modular-first coefficient reconstruction lane should reuse these primitives. RustRed
would still need to own support discovery, bad-prime policy, multivariate rational interpolation,
stopping criteria, and exact ordinary-source replay. Their availability is not evidence that such
a production controller already exists.

## Recommended experiment order

1. Benchmark `Mersenne64` against the current `Zp64` modular lane under identical K=6 work.
2. Benchmark cached finite-field polynomials and chunked compiled evaluators independently.
3. Add the exact Groebner unit-ideal check only as a resource-bounded exceptional-domain refiner.
4. Profile checked coefficient operations before trimming wrappers in sealed exact loops.
5. Use Graphica for generic automorphism discovery while retaining exact momentum-map
   authentication.
6. Consider native parallel back substitution only after outer deterministic parallelism is
   measured and nested-pool behavior is controlled.

No experiment changes closure authority: modular evidence proposes; exact regenerated-source
replay, guard coverage, strict descent, terminal ownership, and deterministic artifact admission
prove.
