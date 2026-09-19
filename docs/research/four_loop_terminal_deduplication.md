# Four-loop terminal deduplication investigation

19 September 2026. Read-only algorithm investigation with independent catalog,
solver and runtime audits. No terminal-minimization algorithm was implemented.
The user's local `EPSILON.md` informed this investigation and was left unchanged.

## Result: the terminal count is not a master count

The corrected production catalogs have **1,155 family-local terminal keys,
26 distinct exact projection vectors, and 19 FMFT PR basis symbols**. FMFT's
PR0 through PR15 are sixteen labels, but PR4d, PR9d and PR11d are retained dotted
integrals as well. PR9x is an intermediate eliminated by FMFT, not an additional
primitive required by these catalogs.

| Parent input | Terminal keys | Distinct exact PR coefficient vectors |
| --- | ---: | ---: |
| H | 386 | 23 |
| FG | 145 | 18 |
| BMW | 179 | 20 |
| X | 445 | 21 |
| Combined | 1,155 | 26 |

The last column is a union for the combined row, not a sum. FG's count is now
18 rather than the 19 recorded in `EPSILON.md`: the four corrected FG numerator
projections changed that local count, without changing the combined count.
The entries have three forms:

| Form | Entries | Example |
| --- | ---: | --- |
| One PR symbol with unit coefficient | 934 | PR1 |
| One PR symbol times a rational coefficient | 84 | `(3-4 ep) PR4 / 5` |
| Linear combination of PR symbols | 137 | `(3 PR1 + PR4) / 2` |

Symbolica exact rational-polynomial normalization confirms the 26 vectors;
allowing an overall nonzero rational-function factor gives 25 directions.
All nineteen unit PR vectors occur, so the coefficient matrix has rank 19 in
the **formal declared PR basis**. This statement neither proves that physical
masters are independent nor certifies the underlying parametric rules.

Large duplicate groups are not subtle: 241 keys project to PR1, 213 to PR3,
114 to PR6, 97 to `(3 PR1+PR4)/2`, 94 to PR2, and 84 to
`(3-4 ep) PR4/5`. Thus the adapter does not need 1,155 unrelated numerical
master evaluations. It already uses the small existing FMFT basis.

## Concrete distinctions

### Equal integrals under a change of loop variables

For the FG input, the terminal keys

```
A = [0,0,1,0,0,1,1,1,0,0]
B = [0,0,1,0,1,0,1,1,0,0]
```

both project to PR1. In A, take independent denominator momenta
`(k3, k2-k3, k1-k3+k4, k1-k2)`; in B take
`(k3, k4, k1-k3+k4, k1-k2)`. Both linear transformations have unit absolute
Jacobian and leave four independent unit-mass tadpoles. Their equality needs
neither FMFT nor an epsilon-shifted propagator. A factorized product canonical
form can recognize this directly.

### Proportional integrals are not duplicate graph labels

One H terminal is `[0,1,0,0,1,1,2,0,1,0]`; the catalog assigns it
`(3-4 ep) PR4/5`, while `[0,1,0,0,1,1,1,0,1,0]` maps to PR4.
These differ by a propagator power. A permutation cannot supply their
dimension-dependent ratio: an additional exact relation is required, such as
an IBP or, when the line symmetry is verified, common-mass differentiation
combined with dimensional homogeneity. For a symmetric five-line four-loop
integral with the current `D = q²-m²` convention,
`I(m²) = (m²)^(2d-5) I(1)` and differentiating the five equal
propagators gives `I_dotted(1) = (2d-5) I(1)/5`. Negative indices can
similarly produce sums such as `(3 PR1+PR4)/2`, not an equality with a single
representative.

The two longer catalog projections offer another exact check. Let A denote
the H terminal `[2,1,0,0,1,1,1,1,1,0]` and B denote
`[2,1,0,1,1,1,0,1,1,0]`, with their unit-mass projected values. Symbolica verifies

```
A = FMFT's eliminated PR9x expression
B + 2 A = (1-4 ep) PR9 / 2 - PR9d / 2
```

These are oracle-derived **diagnostic relations**, not autonomous RustRed
discoveries and not imported parametric rules. They illustrate exactly which
kind of relation a generic terminal-relation pass should be able to find.

Unit-mass equalities must not discard homogeneity when restoring a common mass.
For a same-loop terminal relation `I_n(1) = c(d) I_m(1)`, the corresponding
coefficient at arbitrary mass is `c(d) (m²)^(sum(m)-sum(n))`. Equal index-vector
length alone never establishes compatible family definitions or normalization.

## Why the current solver retains these keys

The SpIReD source-port solver treats each sector independently.
`SectorSolveOptions::numerical_depth` defaults to two;
`SectorSolver::solve_numeric_cases` shares its sparse reducer among fixed
corners of that sector, not among every sector and parent family. A bounded
miss becomes a finite residual, explicitly not a claim of master independence.

`CandidateReducer::try_new` deduplicates identical `IntegralKey` values in a
`BTreeSet`. At application time declared terminals return their identity
decomposition before rule lookup. It does not quotient distinct terminal keys
by momentum routing or search for extra relations between them.

Existing functionality is reusable but not connected to this path:

- `sector::symmetry::verify` checks exact momentum transformations,
  denominator actions and measure factors, including cross-family maps.
- `sector::symmetry::canonical::Canonicalizer` handles verified same-family
  permutations with ordering witnesses.
- Artifact factorization replay handles exact products and dependency masters.
- Source-port installation currently supplies no canonicalizer, dependencies
  or factorization rules.

An initial suspicion that later symbolic rules might trivially cover already
queued numerical points was withdrawn after independent review. Queue admission
already excludes points contained in pending domains, and future cases descend
from those domains. No concrete four-loop counterexample was found. Do not
present that suspicion as a bug or a measured optimization opportunity.

## Runtime and memory implications

The current reducer does combine coefficients of identical terminal keys during
each recursive accumulation, with exact zero removal. Its memoized answers can
still contain several distinct keys representing equal integrals. Vakint then
substitutes their offline PR projections and combines exact coefficients before
Laurent expansion, including contributions sharing a common mass. Consequently
these duplicates are not double-counted; their equivalence is exploited late.

All four `.rrcat` files total only **83,020 bytes**. Their 37,589 bytes of repeated
right-hand sides reduce to 1,610 bytes of unique expression text. Retaining the
keys and adding a small expression dictionary would save roughly 34 kB before
dictionary-format overhead. Catalog compression alone is not a major memory fix.

In contrast the saved programs contain **1,070,847,788 bytes of uncompressed
TOML** (29,947,045 compressed); X alone contributes 537,049,880 bytes.
Loading overlaps raw bytes, parsed string records and rebuilt exact solution
objects; application rules additionally retain denominator data. The four
lazy program owners remain resident after use. Full-harness peak RSS reached
13.89 GiB. These are identifiable structural costs, not a measured breakdown
assigning all that memory to any one allocation or cache.

Earlier terminal projection could narrow memoized decompositions; it cannot
by itself remove the large rule-loading representation. Profile and benchmark
the two opportunities separately.

## Recommended generic follow-up

1. Report raw residual keys, canonical integral representatives, projection
   vectors and numerical basis symbols separately. Never call all residuals
   independent masters.
2. Introduce exact terminal normalization using verified momentum maps and
   separable-product reductions. Reuse existing core services; Symbolica's
   graph canonicalization proposes equivalences, while exact algebra checks
   the full powers, masses, offsets, numerator and Jacobian. Full-family
   denominator permutations alone miss valid sector-local transformations.
3. Search for remaining **finite-terminal relations** globally: generate
   ordinary IBPs near residual keys, reduce the nonterminal integrals using
   existing rules, and solve the resulting sparse terminal equations with
   Symbolica. Finite-field discovery may choose rows; exact checks establish
   the retained relations. Keep the budget bounded and allow a nonminimal
   residual basis on exhaustion.
4. Store an acyclic exact terminal-to-basis map and apply it before caching
   final decompositions. The generic core must not know FMFT PR names.
   Today's offline oracle map can be a caller-supplied optimization, but it
   does not replace autonomous five-loop terminal discovery.
5. Feed verified equivalences into sector selection/search only after measuring
   the standalone normalization pass. Separately improve saved-program loading
   to avoid unnecessary text/solution overlap without losing guards/provenance.

Symbolica already provides graph canonicalization/automorphism generators,
exact matrix operations, `SparseRowReducer`, rational-polynomial arithmetic,
and reconstruction. No private CAS or graph-isomorphism kernel is warranted.

The epsilon-power route is complementary. A massless bubble can induce analytic
power offsets; a fully massive bubble generally retains nontrivial dependence
on its internal external-momentum invariant and cannot be replaced by one
epsilon-powered massive line. FMFT instead uses massive subgraph convolutions
and dimension recurrences for FG; see its
[primary paper, sections 2.1–2.4](https://arxiv.org/pdf/1707.01710).
The PR basis including dotted representatives is also shown in
[Czakon's four-loop calculation](https://arxiv.org/pdf/hep-ph/0411261).
Neither transformation strategy is needed merely to explain the present count.

## Evidence and limits

### Smallest safe integration identified by the follow-up audit

The first implementation should be an immutable, verified **same-family
terminal-alias plan**, prepared before `CandidateReducer` has cached results.
Its existing terminal branch can return a representative instead of the raw
key; ordinary coefficient accumulation then coalesces equivalent terms before
memoizing every ancestor. Keep the original declared keys for catalog/provenance
checks, and expose canonical output representatives separately. Initially map
only to existing declared representatives, leaving unsupported numerator-bearing
cases unchanged. An oracle projection dictionary is not a generic equality proof.

Reuse the existing momentum-map verifier and factorization kinematic checks:
exact denominator/mass/offset transport, integer loop transformation with
determinant ±1, and disjoint loop blocks for products. Unit determinant alone
does not establish integer entries. Requiring a full-family permutation of all
unused ISP coordinates would miss valid sector-local changes of variables such
as the tadpole-product example above. Extract the reusable kinematic checker
from artifact installation instead of manufacturing closed-artifact authority.

Product signatures must include typed lower-family identities, master powers
and factor multiplicities. Embedding back into the same parent family preserves
the existing coefficient mass factor `(m²)^(sum(rep)-sum(target))`; avoid applying
it twice. Tests should include the A/B example, invalid Jacobians and offsets,
dotted-power distinctions, repeated factors, alias cycles, worker determinism
and cold loading. This post-generation pass may shrink cached decompositions;
it cannot remove existing source-search or TOML-loading costs. Measure it before
attempting the larger pre-generation sector-quotienting change.

### Retained measurements

The input is GammaLoop's corrected
`crates/vakint/data/rustred/four_loop/{h,fg,bmw,x}.rrcat`.
All 1,155 projections were independently re-evaluated through the corrected
FMFT oracle, with four FG fixes, before the successful public 15-reference
and 16-pinch numerical matrices recorded in the
[September 19 checkpoint](../checkpoints/2026-09-19.md).

The standalone exact Symbolica census completed in 0.33 s; this is a catalog
analysis time, not IBP generation or reduction performance. Source and complete
output remain in ignored `TMP/terminal-dedup-20260919/`. A literal census can be
reproduced without a CAS from the RustRed workspace:

```sh
awk -F '\t' '/^terminal=/{rhs[$2]++; n++}
  END {for (s in rhs) k++; print "entries",n,"distinct_RHS",k}' \
  FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop/crates/vakint/data/rustred/four_loop/{h,fg,bmw,x}.rrcat
```

It reports 1,155 and 26. The exact vector comparison additionally uses Symbolica
over rational functions of epsilon; it does not infer equality from sampled
floating-point values. None of the counts proves whole-family closure or a
minimal physical master basis. This investigation proposes no extra numerical
masters and changes no solver, reducer, catalog or user's `EPSILON.md`.
