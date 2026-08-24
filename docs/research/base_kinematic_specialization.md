# Checked base-kinematic specialization

RustRed's generic family coefficients live in an authenticated exact field
`K_source = Q(x_1,...,x_r)`.  `BaseKinematicSpecialization` implements a named,
ordered homomorphism into another authenticated coefficient context
`K_target = Q(y_1,...,y_s)`.  The target manifest may be empty, so exact
rational points with `K_target = Q` use the same path.

The constructor requires exactly one `BaseParameterImage` for each source
parameter, in source-manifest order, and checks both the supplied name and the
complete Symbolica variable map of its target rational function.  It retains
every nonconstant parameter-image denominator as a nonzero guard.

For a source coefficient `p/q`, evaluation is deliberately:

1. authenticate `p` and `q` on the source map;
2. evaluate each polynomial independently using bounded checked additions,
   multiplications, and binary powers in `K_target`;
3. reject an identically zero mapped `q`;
4. retain the numerator of mapped `q` before the final division; and
5. perform checked fraction-field division and re-authenticate the result.

Step 4 preserves a pole even if it cancels from the normalized result.  Equal
target guard polynomials are stored once, with a deterministic ordered union
of origins.  Family origins use the shared flat `GuardOrigin` model; parameter
image and independently evaluated coefficient denominators have their own
typed base-specialization origins.  A guard also retains the target parameter
manifest, and `authenticate_guard` checks that manifest and its Symbolica map
before cross-result composition.  The raw `SpecializedBasePolynomial` alias
must not be mixed across target contexts without that authentication.

`evaluate_family_domain` authenticates the complete source family and maps the
deduplicated `FamilyDomain::conditions()` inventory.  Its status is:

- `Applicable`: all conditions map to provably nonzero constants;
- `Conditional`: no condition maps to zero, but target-polynomial guards
  remain; or
- `Inapplicable`: one or more required loci map identically to zero.

`require_family_domain` turns the last status into an explicit typed rejection.
Mapped-zero conditions and equal mapped guards merge while retaining every
source origin, including a coincident input-denominator and basis-determinant
locus.  No external-Gram determinant is introduced: a singular declared
external Gram matrix remains allowed when the affine denominator basis is
complete.

This API does **not** return a specialized `IntegralFamily`.  Domain validity
alone does not prove that all denominators, inverse-basis entries, derivative
contractions, and cached replay data were reconstructed in the target context.
A future family-producing API must rebuild those objects with
`IntegralFamily::new_with_limits` and replay the resulting complete family.

Regression coverage is in `tests/base_specialization.rs`: rational
`det(A)=a` points at `a=0` and `a=2`, an input-denominator pole, a denominator
that cancels only after mapping, rational image guards, coincident/merged
origins, strict named manifests, and a singular external Gram matrix.
