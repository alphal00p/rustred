# `SolvejSector` one-loop validation audit

Date: 2026-08-13

## Scope

This audit compares RustRed's generated-source `WhenBad` boundary with:

- LiteRed's `SolvejSector`, `WhenBad`, and `SmartReduce` implementation in
  `vendor/LiteRed2/Source/LiteRed2026.m:2323-2578`;
- the generated massive one-loop IBP identity produced by RustRed; and
- Vakint/alphaLoop's frozen one-loop behavior in
  `vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:17-29`.

The production fixture contains no recurrence coefficient, elimination
anchor, or selected pivot.  It constructs an `IntegralFamily` and asks
`GeneratedSectorDiscoveryCompiler` to regenerate `IBPLI`, grow the bounded
corner stencil, perform exact `K(n)` elimination, authenticate every pivot
through `GeneratedWhenBadCompiler`, and freeze complete finite coverage.

## Exact one-loop result

For `D_1=k^2-m^2`, RustRed freshly derives

```text
0 = (d - 2 n) I(n) - 2 n m^2 I(n+1).
```

Centering the generated pivot at the harder integral gives the decreasing
rule whose inhabited applicability locus in the active orthant is exactly
`n>=2`.  Concrete classification gives:

| integer input | disposition |
|---|---|
| `n<=0` | outside the certified active orthant |
| `n=1` | exceptional pivot/denominator locus |
| `n>=2` | covered by the descending generated rule |

This is the same range split used by frozen Vakint alphaLoop:

```text
uvprop(n<1) = 0
uvprop(n>1) -> uvprop(n-1) * (d+2-2n)/(2(n-1)m^2)
uvprop(1) = master
```

The master selection and the zero-sector proof are deliberately separate from
`WhenBad`.  A `WhenBad` exceptional leaf is not itself a master proof.

The same generated pivot compiled for the inactive orthant `n<=0` is an
authenticated `Unsupported`, because its RHS translates indefinitely toward
smaller `i64` indices.  RustRed correctly refuses to turn this into either a
rule or a master.

The executable audit is
`tests/generated_when_bad_one_loop_orthant.rs`.

## Exact associate-aware routing

The one-loop certificate currently retains two index-domain conditions that
are opposite polynomial associates:

```text
p  =  2 m^2 (1-n)
-p = -2 m^2 (1-n).
```

One comes from the pre-cancellation pivot guard and the other from a solved
coefficient denominator.  Both provenance-bearing domain conditions remain
in the certificate, but the case router proves that their quotient lies in
the nonzero formal coefficient field `K*=Q(theta)*`.  It consequently reuses
one predicate and produces exactly two structural leaves:

1. `p=0`, exceptional; and
2. `p!=0`, covered.

The associate proof authenticates both exact variable maps, performs bounded
panic-contained rational-polynomial division, and accepts only a quotient
whose numerator and denominator have no index-variable dependence.  It does
not claim radical-ideal equivalence: for example `p` and `p^2`, or two
polynomials with the same integer roots but a nonunit quotient, are not merged
by this layer.  General contradictory-looking leaves must still be retained
unless an independently replayable emptiness proof exists.

## Multi-candidate coverage acceptance conditions

The finite global compiler is accepted for its deliberately narrow role only
because black-box tests establish all of the following without hardcoded
recurrence data:

1. Every candidate is authenticated through `GeneratedWhenBadCompiler`;
   algebraic-only `WhenBad` output cannot enter the table.
2. Candidate order is persisted.  On a leaf covered by more than one
   candidate, the first input candidate wins deterministically.
3. Global predicates are replayed in deterministic candidate/local-split
   order, and exact or proved `K*`-associate polynomials are not split twice.
4. An unsupported candidate remains an explicit attempt.  If no certified
   candidate covers a leaf, that leaf is `Unsupported` or `Uncovered`, never
   a master.
5. Concrete lookup checks the sector orthant, returns exactly one structural
   leaf, and agrees with direct local `WhenBad` classification.
6. Reversing two overlapping valid candidates reverses the selected ordinal
   but does not change the covered integer locus.
7. Empty-looking non-associate contradictory leaves are retained unless an
   independently replayable emptiness proof is added.
8. Aggregate candidate, predicate, split, leaf-classification, retained-term,
   retained-byte, and replay budgets fail closed.

The executable coverage audits are
`tests/parametric_sector_coverage.rs` and
`tests/parametric_sector_coverage_one_loop.rs`.  The latter independently
constructs canonical and exactly translated candidates from the freshly
generated one-loop row.  It checks deterministic first-match priority in both
orders, direct local-versus-global classification for every integer
`1<=n<=64`, explicit inactive-sector `Unsupported`, and empty-search
`Uncovered` behavior.

RustRed intentionally supports caller-owned private Symbolica index namespaces
through `ParametricIbpGenerator::try_with_context`, for example when several
generation stages must share one exact `K(n)` map.  Coverage therefore accepts
any exact same-base, same-arity context.  For every nonempty attempt it still
requires the candidate's exact context fingerprint and freshly regenerates
its IBP/LI source rows in that same context.  A regression proves that a
custom-scope generated candidate compiles and replays, while cross-context,
cross-family, cross-sector, and wrong-arity candidates fail typed.  Empty
coverage is also valid in a caller-owned scope because it authenticates no
identity and claims only `Uncovered` for the supplied empty search set.

A non-unit one-loop denominator basis supplies a formal base-field guard.  Its
test proves that the guard remains in the authenticated candidate certificate
but does not manufacture a lattice predicate in the global partition.  This
is essential: base-only conditions are assumptions in `K=Q(theta)`, while
only index-dependent conditions divide a sector into parametric cases.

## LiteRed parity still missing after this milestone

Automatic initial candidate discovery and deterministic finite composition are
useful, but they are not full `SolvejSector`.  LiteRed repeatedly removes
covered cases and re-solves exceptional partially symbolic cases.  RustRed now
has exact coordinate-equality extraction, a generated-row conditional
re-elimination work queue, and a proof-bearing provider that preserves root
global routes while applying compatible conditional pivots only on their own
centered equality loci.  The remaining generic work is to:

- iterate exact stencil translations beyond the first conditional pass;
- reapplies zero-sector and verified-symmetry quotients before elimination;
- re-eliminates rather than inheriting a pivot whose guard vanished; and
- distinguishes selected/certified masters from bounded-search `Uncovered`.

General polynomial exceptional loci also remain unsupported until bounded,
panic-contained ideal/saturation and integer-inhabitation proof machinery is
available.

An authenticated `Unsupported` candidate currently conservatively marks every
otherwise-uncovered global leaf unsupported because its compilation does not
carry a finer admissibility partition.  This is sound, but deliberately
coarse.  Search exhaustion is likewise only `Uncovered`; neither status may
be promoted to a master integral without a separate replayable master-policy
certificate.

## Symbolica API observations relevant to this layer

- `RationalPolynomial` exposes exact numerator/denominator polynomials and
  performs GCD normalization through `FromNumeratorAndDenominator`; automatic
  `unify_variables` uses assertions and is therefore inappropriate at proof
  boundaries.  RustRed's authenticated context checks must remain in front.
- `MultivariatePolynomial::replace`/`replace_all` provide exact substitution,
  but raw methods allocate before output inspection.  RustRed's bounded
  wrappers and panic containment remain necessary.
- `Pattern`, `ReplaceBuilder::match_iter`, `with_map`, visitors, and
  `FunctionMap` cover expression-boundary matching and rewrites.  They do not
  replace exact rational-polynomial equality and specialization in the proof
  core.
- `make_primitive` removes integer content but does not by itself certify
  equality of loci over `Q(theta)`.  RustRed instead checks the exact quotient
  and proves that it belongs to `K*`, preserving coefficient-field semantics.
