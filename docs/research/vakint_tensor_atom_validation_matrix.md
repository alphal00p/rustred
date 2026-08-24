# Vakint tensor-Atom validation matrix for RustRed

Date: 2026-08-13

## Purpose

This is the concrete validation checklist for RustRed's generic Symbolica
`Atom` tensor front end.  The production compiler and projector remain
family-, topology-, and loop-count independent.  The expressions below are
fixtures copied from or directly abstracted from Vakint tests; they are not
production dispatch cases and no expected IBP recurrence may be embedded in
the parser.

The comparison stops at exact coefficients multiplying unsubstituted master
integrals.  Vakint's epsilon expansions, measure normalization, and numerical
master values are outside this comparison.

## Source fixtures

Primary tensor-only fixtures:

- `vendor/gammaloop/crates/vakint/tests/tensor_reduction_tests.rs:7-39`
- `vendor/gammaloop/crates/vakint/tests/tensor_reduction_tests.rs:42-74`
- `vendor/gammaloop/crates/vakint/tests/tensor_reduction_tests.rs:77-109`

Additional numerator/index fixtures:

- `tests/integral_evaluation_analytic_tests.rs:262-290`
- `tests/integral_evaluation_analytic_tests.rs:293-320`
- `tests/integral_evaluation_freeform_tests.rs:23-56`
- `tests/integral_evaluation_freeform_tests.rs:142-177`
- `tests/integral_evaluation_analytic_tests.rs:388-463`

All paths in the second list are relative to the Vakint crate root above.

## Symbolica vocabulary and maps

The compiler receives, rather than globally assumes:

- the `k`, `p`, `g`, and `dot` head symbols;
- a map from arbitrary input loop-id atoms to the family's zero-based loop
  positions;
- a stable map from arbitrary spectator-vector id atoms to internal
  spectator ids;
- a bidirectional map from arbitrary Lorentz-index atoms to internal index
  ids; and
- checked construction/expansion/retention limits.

`g` and `dot` may be registered symmetric and `dot` linear in an adapter, but
proof-bearing parsing validates head identity and arity itself.  It never
trusts Symbolica pattern conditions for integer positivity, index ownership,
or tensor validity.

## One-loop tensor-only oracle matrix

Write the exact dimension as `d=4-2*epsilon`.  Vakint prints
`-(2*epsilon-4)^-1`, which is exactly `1/d`.

### A: rank two plus an odd contraction

Input numerator:

```text
k(1,1)*k(1,2) + k(1,3)*p(1,3)
```

Expected tensor projection before denominator lowering:

```text
g(1,2) * dot(k(1),k(1)) / d
```

The `k.p` term is odd in the vacuum loop momentum and vanishes.  The compiler
must retain its source/projection witness even though it contributes no output
term.

### B: powers, same-index precontraction, spectators, and a sum

Input numerator:

```text
(k(1,1)*k(1,2))^2*g(1,2)
+ k(1,3)*p(1,3)
+ k(1,1)*k(1,2)*p(2,1)*p(3,2)
```

Expected tensor projection:

```text
dot(k(1),k(1))^2*g(1,2)
+ dot(p(2),p(3))*dot(k(1),k(1))/d
```

This fixture fixes an important Vakint-compatible ordering: equal-index vector
pairs are precontracted before outside metrics are wired.  Thus the powered
first term becomes two loop scalar products while `g(1,2)` remains an outside
covariant.  Expanding the composite positive power must not lose or rename
its repeated tensor factors.

### Weighted scalar plus two `k.p` factors

Input numerator at propagator power two:

```text
A*k(1,11)*p(1,11)*k(1,12)*p(1,12) + B
```

Expected tensor projection:

```text
A*dot(p(1),p(1))*dot(k(1),k(1))/d + B
```

`A` and `B` are arbitrary Symbolica atoms.  A pure scalar term is represented
by an empty covariant/tensor monomial with opaque weight `B`; it must not be
dropped merely because it contains no recognized tensor head.

### Two distinct spectators

Input:

```text
k(1,1)*p(1,1)*k(1,2)*p(2,2)
```

Expected tensor projection:

```text
dot(p(1),p(2))*dot(k(1),k(1))/d
```

The spectator ids are data, not assumed consecutive integers.

## Decorated-index oracle

The following index atom is deliberately not a plain symbol or integer:

```text
user_space::mink4(4,33)
```

The compiler must accept it losslessly wherever an index occurs, allocate a
private internal index id, and render the exact original atom back.  Internal
dummy indices introduced while expanding `dot(k,p)` must use a private
namespace and a collision-checked transcript; they cannot alias any decorated
input atom.

The one-loop decorated fixture contains all of:

```text
(sigma(args)+sigma2(args)+sigma3(args))
  *p(1,mink4(4,33))*p(2,mink4(4,33))
  *p(1,mink4(4,11))*p(2,mink4(4,22))
+ k(3,mink4(4,11))*k(3,mink4(4,22))
+ k(3,mink4(4,77))*p(1,mink4(4,77))
```

The first term is a pure spectator covariant with arbitrary function-valued
weight; the second projects to a metric times `k3^2/d`; the third is odd and
vanishes.  Rendering must preserve the arbitrary functions and all decorated
indices.

## Dot notation

The front end must accept explicit indexed products and `dot` notation in the
same compiler.  Required cases are:

```text
dot(k(loop_a),k(loop_b))^r
dot(p(ext_a),p(ext_b))^r
dot(k(loop_a),p(ext_b))^r
```

for bounded nonnegative integer `r`.  Loop-loop dots become loop scalar-product
factors; spectator-spectator dots become spectator scalar factors; mixed dots
expand to paired indexed loop/spectator vectors using fresh private dummy
indices.  Negative, noninteger, or symbolic powers of tensor-valued factors
are typed unsupported inputs.  Arbitrary powers inside an opaque scalar weight
remain opaque.

## Parser and renderer acceptance requirements

For each accepted input the certificate must retain:

- the exact original normalized Atom;
- top-level sum and product expansion transcript;
- every composite-power expansion count;
- recognized tensor factors and opaque weight factors;
- loop/spectator/index maps and fresh-dummy allocations;
- one typed `WeightedCovariantTensorMonomial` per expanded source term; and
- an exact renderer round trip modulo Symbolica's canonical commutative
  normalization and documented dummy-index renaming.

The parser must reject, with typed errors and without panicking:

- wrong tensor arity;
- an unknown loop id;
- nonintegral/negative tensor powers;
- an index contracted more than twice where no Vakint-compatible
  precontraction is defined;
- products or expansions exceeding configured limits;
- dummy-id exhaustion/collision;
- tensor heads hidden in an opaque weight; and
- a representable-looking rational weight that cannot be authenticated in the
  supplied `CoefficientContext`.

An arbitrary weight that is deliberately outside the coefficient field may be
retained as an opaque Atom for parsing/rendering, but conversion to the exact
projector is a typed deferred/unsupported result.  It is never replaced by one
or silently discarded.

## Scalar reduction validation

After projection, loop scalar products are lowered through the authenticated
family inverse and reduced only by rules derived from freshly generated
parametric IBPs.  For the one-loop massive family, test propagator powers at
least 1 through 6 and numerators that induce positive, zero, and negative
denominator powers.  Compare the final Atom to the Vakint-compatible golden
form with `I1L(mass,1)` left unsubstituted.

Only after the complete one-loop Atom matrix passes should the same compiler
be used with the two-loop fixture

```text
(k(1,1)*k(2,2))^2*g(1,2)
+ k(2,3)*p(1,3)
+ k(1,1)*k(2,2)*p(2,1)*p(3,2),
```

whose tensor-only output is

```text
dot(k(1),k(1))*dot(k(2),k(2))*g(1,2)
+ dot(p(2),p(3))*dot(k(1),k(2))/d.
```

The two-loop scalar result must then use RustRed-discovered rules, never
Vakint's FORM recurrence table.
