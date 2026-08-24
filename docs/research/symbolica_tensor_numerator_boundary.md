# Symbolica tensor-numerator boundary

`src/symbolica_tensor_numerator.rs` is the topology- and loop-count-independent
Atom boundary in front of RustRed's existing covariant tensor projector.  It is
native Rust using the vendored Symbolica API; it does not invoke FORM or use
Vakint's legacy reducers.

## Accepted representation

The caller supplies an authenticated `IntegralFamily`, five distinct Symbolica
heads, and an exact map from every family loop-momentum name to a bare vector
Atom.  With the conventional Vakint syntax these are:

- `vakint::k(identity..., index)` for an indexed loop vector;
- `vakint::p(identity..., index)` for an indexed spectator vector;
- `vakint::g(index,index)` for a metric;
- `vakint::dot(bare_vector,bare_vector)` for loop--loop,
  loop--spectator, or spectator--spectator products.

Vector identities and Lorentz indices are exact Atoms.  Their arguments are
not required to be integers, so decorated indices such as
`user_space::mink4(4,33)` and multi-argument vector identities round-trip.
Loop lookup is exact and simultaneous: no ordered replacement chain is used.

The bounded normalizer distributes `Add` and `Mul` and expands a nonnegative
integer `Pow` whose base contains tensor syntax.  This includes composite
powers such as `(k(1,mu)*k(1,nu))^2`.  Scalar-only factors remain opaque Atom
weights, including a scalar-only summand whose tensor monomial is empty.

## Coefficient-field boundary

Compilation is lossless and does not infer coefficient variables.  Every
source monomial retains its scalar weight as an `Atom`.  Conversion to
`WeightedCovariantTensorMonomial` is a separate checked operation against the
family's already-declared Symbolica rational-polynomial map.  A weight outside
that map returns `DeferredWeight` and is neither dropped nor allowed to widen
the authenticated field.

## Identity allocation and replay

All explicit user indices are interned before loop--spectator dots allocate
private contraction indices.  Generated `rustred::tensor_dummy_index(n)` Atoms
are collision-checked against the complete input-index set.  The compilation
retains ordered index and spectator allocation transcripts, the original Atom,
and a work census; `verify_replay` recompiles the source and compares the full
result.

Projected covariants are rendered with the retained loop, spectator, and index
Atoms.  Exact Symbolica coefficients are emitted with `to_expression`, and
scalar products use the configured symmetric `dot` head.  A separately public,
bounded `render_covariant` operation supports the final grouping produced by a
tensor-plus-IBP reduction, where a covariant no longer carries its original
projected scalar coefficient.

## Deliberate limits

The boundary does not parse `topo(...)`, propagators, or integral powers; those
belong to the integral adapter.  It does not treat an unknown reserved-head
nesting as a scalar.  Tensor-containing powers must have bounded nonnegative
integer exponents.  Input nodes/depth, polynomial expansion, tensor factors,
distinct identities, fresh-index attempts, projector work, and rendering are
all explicitly bounded and fail with typed errors.

The input is an `AtomView`, so Symbolica's canonicalization has already taken
place.  If a reserved tensor subtree cancels exactly to zero (or to one), it is
no longer present and the mathematically tensor-free result is accepted.  Any
reserved head still present anywhere below an otherwise scalar factor is
rejected; RustRed never relies on later coefficient conversion to erase it.

`project` calls the authenticated vacuum covariant projector, which rejects a
family with denominator-basis external momenta and only constructs
`LoopLoop` scalar-product coordinates.  `render_projected` nevertheless checks
coordinates defensively: a caller-synthesized or foreign numerator containing
`LoopExternal` is returned as `UnsupportedRenderedScalarProduct` rather than
being guessed or silently emitted.

The Atom-level validation target covers Vakint's one-loop tensor fixtures A and
B, decorated indices and arbitrary declared weights, opaque deferred weights,
dummy collision/replay across normalized summands, shuffled two-loop identity
maps, composite powers, structural-before-work preflight precedence, canonical
cancellation, foreign-coordinate rejection, and reserved-syntax/resource
failures.  Concrete fixtures validate this generic boundary; no
loop-count-specific tensor formula is embedded in it.
