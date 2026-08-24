# Symbolic symmetry transport and the `SolvejSector` quotient boundary

Status: source-level design plus bounded generic implementation and generated-
source/automatic-discovery integration.
The concrete equal-mass sunset is used only as a validation oracle.  Nothing
in the production API branches on a topology name, denominator count, or loop
count.

## Source constraint from LiteRed

LiteRed does **not** quotient generic symbolic-index equations by `SR`.  In
`SolvejSector`, the submitted equations are constructed as

```mathematica
If[useSR && numeric,
  Join[ids @@ point, SR[nm] @@ point] /. ZerojRule[nm],
  ids @@ point
]
```

at [`LiteRed2026.m:2471-2480`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2471).
Here `numeric` means that no independent index variable remains.  `SR` is the
difference between an integral and each verified self-symmetry image
([`LiteRed2026.m:815-820`](../../vendor/LiteRed2/Source/LiteRed2026.m#L815)).
The symmetry maps themselves are produced by `FindSymmetries` around
[`LiteRed2026.m:3445-3468`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3445).

This is a soundness boundary, not an implementation accident.  RustRed must
not globally replace a generic shifted term `I(n+s)` by `I(n+P s)`.

## Why term-wise canonicalization is wrong

Let a verified denominator permutation be represented by a bijection
`pi(source_slot) = target_slot`.  Its exact action on a power vector is

```text
target[pi(i)] = source[i].
```

For a concrete vector `a`, the symmetry proves `I(a) = I(P a)`.  For
independent symbolic variables, however,

```text
I(n+s) = I(P n + P s),
```

not `I(n+P s)`.  The latter assertion would silently assume `P n = n` and is
false away from a symmetry-fixed index locus.  Consequently the current
`IndexShift` column model, which represents only `I(n+s)`, cannot perform a
nontrivial term-wise generic symmetry quotient.

An affine-column generalization `(A,s)` representing `I(A n+s)` would make
the group action explicit.  It does not create new generic identifications
among ordinary `(identity,s)` columns: the orbit of `(identity,s)` returns to
an identity-linear representative with the same `s`.  Its main value would be
proof bookkeeping for symmetry equations and nontrivial constrained loci,
not a magic generic collection rule.

## The two sound operations

### 1. Concrete pre-elimination quotient: LiteRed-equivalent

At a fully fixed point, every key is a `ConcreteIntegralKey`.  RustRed may:

1. specialize each freshly generated IBP/LI row at the scout point;
2. erase terms with replayed analytic-zero or cut-zero certificates;
3. transport every surviving concrete key along verified symmetry paths;
4. collect equal concrete keys;
5. eliminate over the exact base field `K`; and
6. orient only a checked descending pivot.

This is implemented by
[`CertifiedFamilyRuleProvider`](../../src/certified_rule_provider.rs) and
[`CertifiedConcreteRewrite::from_concrete_quotient_elimination`](../../src/certified_rewrite.rs).
It is the direct RustRed analogue of LiteRed's numeric branch.  The proof owns
the generated-row ordinal, assignment, raw specialization, every zero or
symmetry witness, collected row, columns, elimination trace, pivot, guards,
and descent evidence.  Replay regenerates and recomputes all of them.

### 2. Whole-identity symbolic transport: globally sound augmentation

For a complete identity

```text
R(n) = sum_s c_s(n) I(n+s) = 0,
```

apply the symmetry to every integral and rename `m = P n`.  Since
`n_i = m_pi(i)`, the transported global identity is

```text
P.R(m) = sum_s c_s(m_pi(0), ..., m_pi(N-1)) I(m + P s) = 0.
```

Both transformations are mandatory:

- coefficients use the simultaneous Symbolica substitution
  `n_source[i] -> n_target[pi(i)]`;
- shifts use `target_shift[pi(i)] = source_shift[i]`.

RustRed now implements this bounded operation in
[`symbolic_symmetry_transport.rs`](../../src/symbolic_symmetry_transport.rs):

- `ParametricCoefficientContext::permute_indices` and the polynomial/guard
  variants perform exact simultaneous variable renaming on the authenticated
  Symbolica rational-polynomial map;
- `ParametricRelation::permuted_indices` moves coefficients, shifts, and all
  prior guard provenance together;
- `SymbolicSymmetryRowTransportCompiler` accepts only a
  `VerifiedInternalFamilyPermutationSymmetry`, replays its affine proof,
  retains its complete map-domain conditions, and produces a replayable
  whole-row certificate;
- new flat `GuardOrigin` atoms record coefficient/row index permutation and
  verified-map domain condition ordinals.  The certificate owns the complete
  symmetry proof, so these atoms are identifiers rather than lossy substitutes
  for the affine witness.

The row-transport certificate proves only the transformation.  A public
`ParametricRelation` can be caller-authored, so the certificate intentionally
does not claim that its source is an IBP/LI identity.  Solver integration must
nest it below fresh generated-source authentication, as described next.

This operation is **not** LiteRed's numeric `SR` quotient.  It adds another
globally valid source identity before symbolic elimination.  It may improve a
symbolic row span, but must be evaluated as a separate algorithmic extension.

## Generated-source integration now implemented

`tests/generated_two_loop_sector_discovery.rs` establishes that all raw
depth-one top-sector sunset pivots currently fail closed as `Unsupported`.
This is not evidence for a missing hardcoded recurrence.  The generic source
has the expected four IBPs, while rule orientation sees RHS shifts that are
not uniformly descending in the unquotiented symbolic column model.

The concrete certified provider can succeed because the scout assignment has
already replaced `n` by integers, making denominator permutations genuine
key identifications before elimination.  The generic initial coverage
compiler can now optionally use
[`GeneratedSymbolicRowSpanCompiler`](../../src/generated_symbolic_row_span.rs).
That compiler:

1. freshly regenerates the canonical `IBPLI` list;
2. accepts either a bounded vacuum-internal symmetry search or explicit
   `VerifiedInternalFamilyPermutationSymmetry` inputs;
3. replays every explicit certificate against its exact owned
   `SectorRestrictions` value and fingerprint;
4. skips the identity permutation, transports each complete canonical row,
   and exact-deduplicates only equal sparse algebraic rows with equal
   exceptional polynomial sets;
5. owns the canonical rows, every retained transport certificate, search
   completion, lineage, limits, and aggregate census; and
6. replays the complete payload from the family and authenticated `K(n)`
   context.

`GeneratedSourceAuthenticator` now has four exact modes:

```text
CanonicalOriginal
ExactTranslation
VerifiedWholeRowSymmetryTransport
ExactTranslationOfVerifiedWholeRowSymmetryTransport
```

The last two nest their proof under the owned row-span certificate.  Matching
still infers a unique lattice translation from complete support and replays
`ParametricRelation::translated`; it never trusts a row label or accepts a
term-wise symmetry image.  Guard provenance is compared exactly.  Automatic
`GeneratedSectorDiscoveryCompiler` feeds the same augmented basis into its
adaptive stencil/elimination and source authentication, while its default
configuration remains canonical-only and retains the v1 behavior.

This does not make raw two-loop top-sector coverage complete.  It establishes
the sound generic extension point and proves that augmented two-loop discovery
is bounded and replayable without a hardcoded recurrence.

## Remaining integration work

1. Apply the same source-transform vocabulary inside
   `GeneratedPartialReeliminationCompiler`.  On an exceptional equality locus,
   order is: regenerate, transform whole rows, translate, partially
   specialize, quotient where the locus justifies it, and re-eliminate.  A
   parent pivot is never inherited across a rank-changing locus.

2. Keep the concrete provider as the authoritative LiteRed-equivalent
   symmetry quotient.  Even after symbolic row augmentation exists, a fully
   numeric leaf should specialize and quotient **before** base-field
   elimination, matching LiteRed's source order.

3. **Implemented:** coverage and family-wide orchestration now share one
   immutable `Arc<GeneratedSymbolicRowSpanCertificate>`.  The coverage,
   discovery, live-leaf queue, and family replay paths replay the shared proof
   once at their public boundary and use internal already-replayed entry points
   thereafter.  Every candidate still owns its exact source manifest and row
   witnesses, but points to the same authenticated basis allocation.  Public
   replay also accepts an independently reconstructed, payload-equal row span
   (as required by persistence) and normalizes the replay batch onto it;
   pointer identity remains an in-memory scaling invariant, never the
   mathematical proof.  Family-wide row-span interruption is likewise
   compiled/replayed once and projected conservatively onto scheduled sectors.

## Optional general affine-key layer

If later cross-index equality loci require direct symbolic symmetry equations,
introduce

```rust
struct ParametricAffineIntegralKey {
    linear: SignedPermutation, // A
    shift: IndexShift,         // s
}
```

for `I(A n+s)`.  Equality and collection are exact structural equality after
checked group canonicalization.  A symmetry path maps `(A,s)` to `(P A,P s)`.
The proof must retain each path and its map-domain guards.  Rule orientation
may emit only a pivot reducible to `(identity,0)` on the current leaf; every
RHS key needs a uniform descent proof after specialization of the leaf's index
constraints.  This is a broad column/elimination/WhenBad redesign and is not
needed for the initial whole-row transport slice.

## Validation already attached

`tests/symbolic_symmetry_transport.rs` and
`tests/generated_symbolic_row_span.rs` use the two-loop equal-mass family only
as an oracle and check the generic invariants:

1. a discovered, replayed denominator permutation moves every coefficient and
   shift together;
2. complete-row transport commutes with concrete specialization and verified
   key transport;
3. pre-cancellation guards retain both their source history and explicit
   coefficient/row permutation provenance;
4. non-bijective index substitutions fail with a typed error;
5. canonical-only authentication rejects a transported row, while augmented
   authentication distinguishes a transport from its exact translation;
6. a one-term fragment of a transported identity and a mathematically equal
   row with forged guard origins are rejected;
7. explicit verified inputs retain and replay nontrivial cut restrictions and
   reject a foreign family or a mismatched restriction value;
8. search, transport, output, deduplication, manifest, and witness-resource
   policies fail closed; and
9. automatic two-loop discovery consumes the augmented basis and replays its
   v2 certificate.

The production compiler receives an arbitrary family, parametric relation,
and verified permutation certificate.  No recurrence, sunset formula, or
loop-count dispatch occurs in it.
