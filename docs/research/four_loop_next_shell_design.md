# Four-loop genuine seed shell beyond the fixed corner certificate

Date: 2026-08-13

## Outcome

The smallest useful **nested shell found by the current deterministic search**
has 123 seeds and 1,968 native IBP origins.  It consists of:

1. the ten already certified scalar corners;
2. every 72 one-dot seed;
3. every 28 numerator-only seed; and
4. the 13 mixed one-dot/one-numerator seeds listed below.

At three independent finite-field images this shell pivots all 48 nonterminal
coordinates left free by the exact 160-row corner certificate.  The only
inherited free coordinates which remain are its ten scalar corners and six
canonical lower-loop products.  Extending the probe to the complete 296-seed
`(D,N)<=(1,1)` box leaves the same inherited set, so the result is stable under
the remaining 173 mixed seeds at those images.

This is a discovery result, not yet a RustRed reduction certificate.  In
particular:

- the 13-seed prefix has not been proved globally cardinality-minimal;
- modular ranks do not replace elimination over `Q(d,m2)` with replayable
  source-row weights; and
- the shell exposes 910 distinct higher factorized-boundary columns which the
  current production boundary service does not reduce.

The exact strict-mask redispatch, witness-complete preclosure inventory, and
factorized component-transport stage have now landed. The transport certifies
all 1,066 exact keys and 4,230 occurrence references, including complete local
power vectors and componentwise parity.  Two exact sibling certificates now
close the two plan slices separately.  `FourLoopT1S2Closure` covers the 243 plans
containing only T1 and S2 components and 1,134 occurrence references.
`FourLoopThreeLoopClosure` covers the complementary 823 plans
(`T1*B4:223`, `T1*F5:494`, `T1*M6:106`) and 3,096 occurrence references.  Its
204-target service (`T1:4`, `B4:41`, `F5:89`, `M6:70`) uses one caller-context
three-loop `(D,N)<=(2,1)` finite pipeline. `FourLoopNextClosedRows` now
integrates the two slices and canonically scales all 1,968 parent rows over
`Q(d)`, with exact grouped-versus-raw source replay. Exact elimination, rank,
source weights, and exceptional-factor recording remain pending. This is not
an unrestricted four-loop reduction or a proof that the ten corners form a
minimal master basis.

The exact origin-only portion has now landed as `FourLoopNextManifest`.  It
freezes the 123 ordered seed IDs under checksum
`fnv1a64:0bff80d5dddb4340` (FNV-1a-64 over each ordered stable key followed by
one LF), checks that both raw-row loop labels satisfy `0 <= label < 4`,
authenticates their built-in H/X reference corners and masks, and regenerates
all 1,968 native derivative origins.

`FourLoopPolynomialHaloMapper` is the landed transport layer.  It caches at
construction whether a mapper owns the exact built-in H/X family fingerprint,
authenticates the manifest corner type, topology, and reference mask, and maps
and replays every complete raw identity with its native integral keys and
coefficients intact.  It emits same-genuine-mask and strictly-lower-physical-
mask branches, but the mapper itself performs no recursive dispatch or
normalization. The manifest status therefore remains
`ExactOriginsNormalizationPending`.

`FourLoopNextInventory` is the separate landed recursive consumer. It maps all
1,968 origins, follows strict physical-mask descent to the fixed maximum depth
two, tests full-family and scalar-corner scalelessness in that order, and
replays every scalar-support factorization witness. Its status is narrowly
`ExactPreclosureInventory`. The frozen exact census is 26,078 coefficient-free
compact paths, split by dynamic-map depth as `[14,766, 10,313, 999]`, and 2,794
leaves. It retains 4,230 raw boundary occurrences and 1,066 full-identity
boundary keys; 4,214 contributor paths survive in nonzero sums across 1,289
blocked rows. There are 28 repeated row/key groups and eight exact row-local
cancellations.

The 1,066 keys retain the exact family fingerprint, all ten powers, scalar
support product, and full decomposition witness. The separate landed
`FourLoopComponentTransport` authenticates the inventory-owned family context
and maps every key into a complete factorized scalar-product basis. Its exact
production census is 577 N0 and 489 N1 plans with products `S2^2:52`,
`T1*B4:223`, `T1*F5:494`, `T1*M6:106`, `T1^2*S2:91`, and `T1^4:100`.
Construction and replay verify `T=B^-1 U_scatter`, every signed active line,
the B4 compact lift `[0,1,3,5]`, eleven affine probes, local checked shifts,
and two separately owned rank-one parity zeros per nonzero cross term.

This certificate is transport only. Its separate consumer
`FourLoopT1S2Closure` closes exactly the `T1^4:100`, `T1^2*S2:91`, and
`S2^2:52` plans. It authenticates 25 local targets, 1,442 component uses, and
1,134 occurrence references, performs ordinary convolution before
mass-normalization, proves literal removal of `m2`, and freezes checksum
`fnv1a64:a2b92a62c988d2cb`. It returns an enclosing `Partial` status with 823
plans and 3,096 occurrences outside its slice.  The landed complementary
sibling's exact input census is
443 N0 and 380 N1 plans, 1,646 components, 5,761 complete local slots, 1,884
scalar branches, and 3,768 component calls with 3,564 cache hits.  Its local
branch split is base/constant `443/323` and T1/B4/F5/M6
`186/220/656/56`; its call split is `1,884/444/1,260/180`.  These are exact
facts derived from the landed transport and are now enforced by
`FourLoopThreeLoopClosure`.  Its exact build closes the 823 plans and 3,096
occurrences, reports 243 plans and 1,134 occurrences outside its slice, and
records 969 completed-row incidences, 511 outside-row incidences, and 191
mixed rows.  It performs 7,356 convolution pairs, retains 3,598 precollection
and 2,159 collected terms, takes 4,279 mass-power steps and 17,456 coefficient
operations, and retains 256,603 output-coefficient bytes.

The shared `FourLoopThreeLoopService` validates 1,800 native target identities
and retains 502 semantic output terms in 12,555 coefficient bytes.  Its finite
pipeline records 306 input equations, 149 rules, 157 dependent equations, and
a maximum of 30 terms per equation.  The frozen target-manifest, service, and
closure checksums are respectively `fnv1a64:9bb3c1a6d4ea7bdd`,
`fnv1a64:6a1b52ddb449d5bb`, and `fnv1a64:da3c250b95b10976`.

Neither sibling alone integrates admitted and outside-slice occurrences.
Their landed `FourLoopNextClosedRows` consumer now zips both partitions with
the common transport, substitutes every occurrence, and authenticates 1,968
complete parent rows. It does not identify the modular 910 keys with the exact
manifest, compute rank or source weights, eliminate, or claim a complete
next-shell reduction.

All discovery calculations used the pure-`std` Rust probe
[`four_loop_next_shell_rank.rs`](../../tools/four_loop_next_shell_rank.rs).
No FORM, Mathematica, Cargo, or Symbolica process is used by the probe.

## 1. Coordinates and exact seed combinatorics

For a frozen genuine type `t`, let `p` be the number of active physical lines
and `q=10-p` the number of inactive entries in its completed scalar-product
basis.  For powers `a`, use

```text
D(a) = sum_(active i)   max(a_i-1,0),
N(a) = sum_(inactive i) max(-a_i,0).
```

Inactive entries include pinched physical denominators and the generated ISP.
Position 9 is the generated auxiliary for both H and X; X9 must remain in the
X basis and must not be rewritten as an H exponent vector.

The frozen types and seed counts are:

| type | topology | mask | `p` | `q` | dots | numerators | mixed |
|---|---|---:|---:|---:|---:|---:|---:|
| V5 | H | `0x06b` | 5 | 5 | 5 | 5 | 25 |
| V6a | H | `0x06f` | 6 | 4 | 6 | 4 | 24 |
| V6b | H | `0x0cf` | 6 | 4 | 6 | 4 | 24 |
| V7a | H | `0x13f` | 7 | 3 | 7 | 3 | 21 |
| V7b | H | `0x07f` | 7 | 3 | 7 | 3 | 21 |
| V7c | H | `0x0df` | 7 | 3 | 7 | 3 | 21 |
| V8a | H | `0x17f` | 8 | 2 | 8 | 2 | 16 |
| V8b | H | `0x0ff` | 8 | 2 | 8 | 2 | 16 |
| H9 | H | `0x1ff` | 9 | 1 | 9 | 1 | 9 |
| X9 | X | `0x1ff` | 9 | 1 | 9 | 1 | 9 |
| **total** | | | 72 | 28 | **72** | **28** | **186** |

Here `dots`, `numerators`, and `mixed` mean exact seed layers `(1,0)`,
`(0,1)`, and `(1,1)`.  Consequently the following counts are combinatorial
facts, independent of a coefficient field:

| shell | seed count | native rows at 16 rows/seed |
|---|---:|---:|
| corners | 10 | 160 |
| corners + all axis seeds | `10+72+28=110` | 1,760 |
| recommended axis + mixed prefix | `110+13=123` | 1,968 |
| complete `(D,N)<=(1,1)` box | `10+72+28+186=296` | 4,736 |

The one-step genuine dependency universe lies within `(D,N)<=(2,2)`.
The exact pre-quotient count from the weak-composition formula is 3,231
genuine vectors across the ten types.  Adding the six product keys gives a
3,237-column post-boundary structural bound.  It does not include unresolved
factorized-boundary columns.

## 2. Deterministic discovery ordering

The production column order is retained:

```text
product stable key
< typed factorized boundary key
< genuine(active lines, D+N, D, type stable key, powers lexicographic),
```

with the reverse order used for pivot selection.  A typed boundary is made
easier than every genuine integral so that temporarily retaining a boundary
does not turn it into a preferred four-loop pivot.

Seeds in each layer are ordered by whether their own genuine column belongs
to the inherited corner-shell free set, then hardest genuine-column order.
The nested order is all dots, all numerator-only seeds, and then mixed seeds.
The complete axis is deliberately orbit/type complete even though a more
irregular subset may exist.

The selected mixed prefix is exactly:

```text
 1 V8a [1,1,1,1,1,1,2, 0,1,-1]
 2 V8a [1,1,1,1,1,1,2,-1,1, 0]
 3 V8a [1,1,1,1,1,1,1,-1,2, 0]
 4 V7c [1,1,1,1,2, 0,1,1, 0,-1]
 5 V7c [1,1,1,1,2,-1,1,1, 0, 0]
 6 V7c [1,1,1,1,1, 0,2,1, 0,-1]
 7 V7b [1,2,1,1,1,1,1,-1, 0, 0]
 8 V7b [1,1,1,1,1,2,1,-1, 0, 0]
 9 V7b [1,1,1,1,1,1,2, 0, 0,-1]
10 V7b [1,1,1,1,1,1,2,-1, 0, 0]
11 V7a [1,1,1,1,1,2,0,-1,1, 0]
12 V7a [1,1,1,1,1,1,0,-1,2, 0]
13 V7a [1,1,1,1,1,1,-1,0,2, 0]
```

The inherited pivot count rises from 44 after the axis to 45 at mixed seed 3,
46 at seed 11, 47 at seed 12, and 48 at seed 13.  Prefixes in this particular
ordering therefore cannot stop earlier than 13.  This is only prefix
minimality for the frozen order, not minimality among arbitrary subsets or
linear combinations of seeds.

As a negative comparator, globally sorting all 286 candidate additions by
the same immediate inherited-free priority closes the 48 coordinates only
after 261 added seeds.  At the first tested image that comparator has 271
total seeds, 4,336 rows, 4,810 columns, rank 3,585, and nullity 1,225.  It is
not the recommended shell.

## 3. What the probe implements

The probe independently reconstructs the H and X quadratic bases, their
generated auxiliaries, and all 16 native `partial_(k_i).k_j` identities per
seed.  It then:

1. classifies proper sectors with the ten signed-unimodular routing normal
   forms;
2. constructs complete-basis affine images over a prime field;
3. multiplies and collects up to degree-two numerator images;
4. drops only rank-deficient scaleless sectors;
5. closes scalar factorized corners and the already certified scalar
   `D1/N0` boundary formulae;
6. retains every higher factorized target as a typed opaque column; and
7. performs sparse deterministic forward elimination.

Before studying a larger shell it asserts that the corner image has exactly
223 columns, rank 159, and nullity 64.  This reproduces the exact production
corner certificate's dimensions and is a useful guard against a changed
family, canonicalization, or column order.  It does not turn the independent
probe into an exact replay of production RustRed.

An opaque boundary key contains `(H/X topology, canonical product, ten parent
powers)`.  Keeping these keys distinct deliberately weakens cross-row boundary
cancellation compared with a complete boundary reduction.  The probe never
silently identifies one with a product terminal.

## 4. Three-image finite-field evidence

The formatted probe was directly compiled with `rustc` and run at

```text
(p,d,m2) = (1000003,17,1),
           (1000033,29,1),
           (1000037,37,1).
```

All three moduli are prime.  Every count and pivot disposition in the table
below is identical across the three images except `pivot terms`, whose small
variation is shown as a range.

| shell | seeds | rows | nonzero rows | input terms | max input width | columns | rank | nullity | pivot terms | max pivot width | inherited pivoted/remain |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| corner | 10 | 160 | 159 | 1,334 | 19 | 223 | 159 | 64 | 1,285 | 19 | 0 / 64 |
| axis | 110 | 1,760 | 1,554 | 19,395 | 45 | 2,315 | 1,554 | 761 | 26,257--26,258 | 113 | 44 / 20 |
| axis + 13 mixed | 123 | 1,968 | 1,762 | 23,300 | 45 | 2,644 | 1,762 | 882 | 35,985--35,986 | 223 | 48 / 16 |
| full `(1,1)` | 296 | 4,736 | 3,931 | 67,001 | 76 | 5,192 | 3,931 | 1,261 | 177,511--177,520 | 383 | 48 / 16 |

`input terms` is the number of collected nonzero entries presented to
elimination, not the number of raw derivative incidences.  `pivot terms` is
the stored sparse width after forward elimination and can vary when a
coefficient happens to cancel at a specialization.

### 4.1 Column and free-column census

The corner census is:

| grading | columns | free |
|---|---:|---:|
| genuine `D0/N0` | 10 | 10 |
| genuine `D1/N0` | 72 | 22 |
| genuine `D1/N1` | 135 | 26 |
| products | 6 | 6 |
| **total** | **223** | **64** |

Thus the exact corner certificate's 64 unresolved coordinates split into ten
scalar corners, six product terminals, and 48 nonterminal halo coordinates.
The latter are exactly 22 one-dot and 26 mixed coordinates.

The larger finite-field censuses are:

| grading | axis columns/free | axis+13 columns/free | full `(1,1)` columns/free |
|---|---:|---:|---:|
| genuine `D0/N0` | 10 / 10 | 10 / 10 | 10 / 10 |
| genuine `D0/N1` | 28 / 5 | 28 / 5 | 28 / 2 |
| genuine `D1/N0` | 72 / 0 | 72 / 0 | 72 / 0 |
| genuine `D1/N1` | 186 / 8 | 186 / 3 | 186 / 0 |
| genuine `D1/N2` | 329 / 50 | 329 / 50 | 329 / 50 |
| genuine `D2/N0` | 303 / 20 | 303 / 20 | 303 / 14 |
| genuine `D2/N1` | 646 / 63 | 654 / 27 | 729 / 10 |
| genuine `D2/N2` | 0 / 0 | 146 / 25 | 1,365 / 98 |
| products | 6 / 6 | 6 / 6 | 6 / 6 |
| typed boundary | 735 / 599 | 910 / 736 | 2,164 / 1,071 |
| **total** | **2,315 / 761** | **2,644 / 882** | **5,192 / 1,261** |

The 123-seed shell has all ten scalar corners and all six products free, while
only 16 inherited free columns remain in total.  Therefore every one of the
22 inherited `D1/N0` and 26 inherited `D1/N1` coordinates is pivoted.  The
additional free entries in the table are newly exposed higher-shell or
boundary coordinates; their presence is why this result does not constitute
a complete reduction table.

The full 296-seed holdout still has exactly the same 16 inherited free
coordinates.  This is strong evidence that the selected prefix is useful for
the corner targets, but the full shell's many new free nonterminals also shows
that unrestricted closure has not been reached.

## 5. The typed factorized-boundary blocker

The recommended modular image exposes 910 distinct opaque boundary columns.
All are H-family in the observed image, and their observed modular key census
is:

| canonical scalar product | `D1/N1` | `D2/N0` | `D2/N1` | total |
|---|---:|---:|---:|---:|
| `S2^2` | 12 | 18 | 15 | 45 |
| `T1*B4` | 90 | 92 | 0 | 182 |
| `T1*F5` | 130 | 187 | 116 | 433 |
| `T1*M6` | 18 | 47 | 26 | 91 |
| `T1^2*S2` | 32 | 42 | 0 | 74 |
| `T1^4` | 50 | 35 | 0 | 85 |
| **total** | **332** | **421** | **157** | **910** |

Of these, 736 remain free in the modular shell.  The opaque columns must not
be copied into a production `Complete` certificate.  Production must either:

- close every occurrence through an authenticated factorization/tensor/lower-
  loop proof and admit the row; or
- retain an `UnsupportedBoundary` record, exclude that row from exact
  elimination, and return a typed partial status.

Treating a higher boundary as a newly declared terminal would be unsound.
The landed production path takes the first branch for every one of the 4,230
occurrences; no unsupported boundary is promoted or silently discarded.

The exact preclosure inventory is deliberately broader than the modular opaque
key projection: it contains 1,066 full-identity boundary keys and 4,230 raw
occurrences. The landed component-transport certificate covers all of them.
Lower-loop closure is now exact in two complementary slices: 243 T1/S2-only
plans and 823 plans containing B4, F5, or M6.  The latter slice has this exact
local target census:

| component | exact unique target degrees |
|---|---|
| T1 | powers `0..=3` (4 targets) |
| B4 | `(D,N)=(0,0):5`, `(0,1):2`, `(1,0):16`, `(1,1):8`, `(2,0):10` |
| F5 | `(0,0):6`, `(0,1):1`, `(1,0):25`, `(1,1):5`, `(2,0):43`, `(2,1):9` |
| M6 | `(0,0):6`, `(1,0):24`, `(2,0):40` |

Each key includes the exact
family fingerprint, all ten powers, scalar-support product, and complete
factorization witness. The 910 columns above remain discovery evidence and
are not promoted, identified, or forced to coincide with the 1,066 exact
keys.

The exact transport now covers the `D1/N1`, `D2/N0`, and `D2/N1` parent keys.
It replays the parent's unimodular scalar-support witness for every key,
lowers each sole parent numerator into local and cross-component coordinates,
and retains the resulting local powers.  Both landed closures consume this
data without re-inferring ownership.  The three-loop-component sibling reduces
all B4, F5, and M6 local targets and composes their products through one
authenticated three-loop pipeline with `(D,N)<=(2,1)`, built in the exact
parent `CoefficientContext`; the focused B4/F5/M6 services are independent
replay oracles rather than separate production dispatchers.
The adapter must map the pipeline's five fixed integral terminals by exact
semantic identity:

```text
P3 -> T1^3,  ST -> T1*S2,  B4 -> B4,  F5 -> F5,  M6 -> M6.
```

Outputs may mix these products; a requested B4, F5, or M6 label is not an
output support promise.  A sole numerator is a quadratic form in the
component loop momenta.
Same-component scalar products are lowered with that component's complete
denominator basis.  Cross-component scalar products factor into two odd
rank-one vacuum tensors and vanish only after their component ownership and
parity projection are authenticated.  Scalar dots do not change that parity
argument.  This is native Rust tensor reduction; FORM is neither required nor
permitted.

The surviving local component integrals can carry two scalar dots and one
numerator before lowering. Each T1, S2, B4, F5, or M6 input reached by the
exact 1,066-key inventory must therefore be matched to an exact lower-loop
formula or finite certificate. Existing lower-loop APIs may be reused only
where their declared coverage contains the precise local power vector. A
familiar master label is not itself a boundary proof.

## 6. Degree-two affine transport and ISP treatment

The degree-one `FourLoopHaloMapper` remains the authenticated source of ten
affine basis images.  The landed `FourLoopPolynomialHaloMapper` composes those
images for every collected term in all 1,968 native manifest origins.  It
accepts the one-shift dependency halo through `N<=2` and retains the exact raw
integral key and coefficient for provenance and replay.

One raw term has at most two numerator factors and each affine factor has at
most 11 terms.  Their Cartesian product therefore has at most `11*11=121`
precollection products.  Collection in ten commuting denominator coordinates
leaves at most

```text
binomial(11+2-1,2) = binomial(12,2) = 66
```

monomials including repeated factors and lower-degree constant branches.  Each
witness records the source factors, their exact affine images, collected
monomials, output integrals, and typed active-line cancellations.  Complete
row witnesses additionally retain exact generated-row statistics under
conservative bounds of 111 collected raw terms, 13,431 aggregate convolution
products, and 7,326 output branches.  Replay regenerates the raw row and
compares its keys, coefficients, polynomial maps, branches, and statistics.

A cancellation is emitted as either `SameGenuineMask` or
`StrictlyLowerPhysicalMask`. The mapper does not redispatch a lower branch,
recurse, call boundary services, or construct a normalized row. The separate
inventory now consumes those branches and implements the well-founded strict
mask descent to depth two. It classifies terminal scaleless and factorized
scalar supports and retains their full keys. The separate component transport
then maps every factorized key into local/cross coordinates. Neither layer
constructs a mass-normalized row; the later landed
`FourLoopNextClosedRows` layer performs that composition.

An inactive physical entry and the generated ISP are treated uniformly as
numerator coordinates while the polynomial is transported.  They are not
treated uniformly as physical lines: only positive entries among positions
`0..8` define the physical sector, position 9 may never create a propagator,
and H/X topology remains part of the source witness.

## 7. Exact production API

The origin, polynomial, preclosure-inventory, component-transport, both
lower-loop closure slices, and normalized parent-row records below have
landed. The elimination certificate remains a proposed next layer:

```text
FourLoopNextSeedId {
    phase: Corner | Dot | Numerator | Mixed,
    corner_type: FourLoopGenuineCornerType,
    powers: [i32; 10],
}

FourLoopNextRawRowId {
    seed: FourLoopNextSeedId,
    differentiated_loop: u8,
    contraction_loop: u8,
}

FourLoopPolynomialMapWitness {
    source_family_fingerprint,
    reference_family_fingerprint,
    corner_type,
    manifest_raw_id,
    source_seed,
    source_term,
    source_numerator_factors,
    factor_images,
    collected_monomials,
    branches,
    stats,
}

FourLoopPolynomialRawRowMap {
    raw_id,
    terms: [(native_raw_coefficient, polynomial_map)],
    exact_row_stats,
}

FourLoopNextCompactPath {
    raw_term_index,
    root_branch_index,
    recursive_depth,
    recursive_branch_indices,
    leaf_id,
}

FourLoopNextBoundaryKey {
    topology,
    family_fingerprint,
    powers: [i32; 10],
    sector_mask,
    scalar_support_product,
    full_factorization_witness,
}

FourLoopNextInventory {
    status: ExactPreclosureInventory,
    rows,
    compact_paths,
    leaves,
    raw_boundary_occurrences,
    nonzero_row_local_blockers,
    exact_resource_stats,
}

FourLoopComponentTransport {
    status: ExactComponentTransport,
    plans: 1_066,
    occurrences: 4_230,
    n0_plans: 577,
    n1_plans: 489,
    component_loop_transforms,
    complete_local_powers,
    affine_images,
    scalar_branches,
    parity_witnesses,
}

FourLoopHigherBoundaryInput {
    topology,
    powers: [i32; 10],
    dot_degree,
    numerator_degree,
    canonical_product,
    factorization_witness,
}

FourLoopHigherBoundaryClosure {
    input,
    component_power_vectors,
    numerator_image,
    tensor_parity_witnesses,
    lower_reduction_witnesses,
    mass_normalized_products,
}

FourLoopNextClosedRow {
    raw_id,
    seed_mass_weight,
    path_dispositions,
    boundary_group_indices,
    row_scale,
    pivot_column_index,
    entries,
}

FourLoopNextClosedRows {
    status: ExactFixedSeedParentRowsGenericQdEliminationPending,
    plan_bindings: 1_066,
    occurrence_bindings: 4_230,
    boundary_groups: 4_202,
    columns: 1_734,
    rows: 1_968,
    stats,
    checksum,
}

FourLoopInheritedFreeDisposition {
    base_certificate_hash,
    inherited_columns: 64 typed keys,
    pivoted_nonterminals: 48 typed keys,
    retained_candidates: 16 typed keys,
}

FourLoopNextShellCertificate {
    schema,
    seed_manifest,
    family_fingerprints,
    column_order,
    normalized_rows,
    preclosure_boundary_inventory,
    boundary_closures,
    blocked_rows,
    rank,
    pivots,
    free_columns,
    source_row_weights,
    inherited_disposition,
    exceptional_d_factors,
    discovery_profile,
}
```

`FourLoopNextRawRowId::new` is checked and returns an error when either loop
label is at least four.  Internal generation uses unchecked construction only
after validating the labels obtained from the native generator.

The landed row assembler returns the narrow status
`ExactFixedSeedParentRowsGenericQdEliminationPending`. A future elimination
certificate may return a status such as

```text
CompleteFixedSeedShell
Partial { blocked_rows, unsupported_boundaries }
```

`CompleteFixedSeedShell` means only that every one of the 1,968 rows in the
versioned manifest was admitted and exactly replayed.  It must not be named
`CompleteFourLoopReduction`.  The expected inherited disposition is an exact
acceptance condition. The rank 1,762 in section 4 belongs to the historical
2,644-column opaque-boundary probe; because the landed closed matrix has only
1,734 columns, 1,762 cannot be its rank or an acceptance target. The closed-
matrix rank becomes an acceptance constant only after exact elimination
independently establishes and freezes it.

## 8. Replay obligations

The landed manifest and polynomial mapper satisfy obligations 1--4 below for
all 1,968 origins. The preclosure inventory satisfies obligation 5, and the
component transport satisfies obligation 6 for every exact key and occurrence.
The complementary closure pair and `FourLoopNextClosedRows` satisfy
obligations 7--9 over the complete domain. A future elimination certificate
must preserve those checks and satisfy obligations 10--12:

1. verify the schema, stable seed checksum, 123 typed seeds, phase order, and
   1,968 checked raw row IDs;
2. rebuild the exact built-in H/X families, generated auxiliaries,
   fingerprints, corner types, and reference masks;
3. regenerate every native raw IBP from `(seed,i,j)` and compare its collected
   integral keys, coefficients, and deterministic order;
4. replay every signed-`GL(4,Z)` routing witness and every degree-zero,
   degree-one, or degree-two polynomial basis map, including typed branches
   and exact row statistics;
5. replay every full-family/scalar-corner scaleless classification and every
   scalar-support factorization witness, while retaining dotted/numerator
   powers as unresolved coordinates;
6. replay component loop maps, complete local power vectors, numerator
   lowering, and two separately owned tensor-parity zeros per nonzero cross
   term;
7. replay lower-loop reductions, product convolutions, and final substitutions
   for every transported higher boundary;
8. apply mass homogeneity term by term and prove literal absence of residual
   `m2` from the normalized matrix over `Q(d)`;
9. reproduce canonical row scales and hashes;
10. reconstruct each pivot from exact source-row weights, verify strict
   hardest-column triangularity, and reduce all admitted rows to zero;
11. recover the exact 64-column inherited set from the corner certificate and
    verify the declared 48/16 disposition; and
12. retain every nonzero denominator factor in `d` as an exceptional-locus
    condition rather than evaluating it away.

Finite-field images may choose a pivot skeleton and reject exceptional
samples.  They are not serialized in place of exact coefficients or replay
weights.

## 9. Resource guards

The 123-seed manifest gives exact static origin counts:

```text
seeds                         123
raw rows                    1,968
sum of nonzero seed entries   927
```

The last number is

```text
72 corners
+ 534 across the 72 dotted seeds
+ 214 across the 28 numerator-only seeds
+ 107 across the selected 13 mixed seeds.
```

Charging four divergence terms per seed and at most 11 contraction terms in
each of 16 rows for every nonzero seed entry gives the exact structural upper
bound

```text
4*123 + 16*11*927 = 163,644 raw term incidences.
```

The landed mapper applies these conservative limits before coefficient-heavy
work:

| transport resource | per term | per complete raw row |
|---|---:|---:|
| numerator factors | 2 | checked term by term |
| terms per affine factor | 11 | checked term by term |
| uncollected convolution products | 121 | 13,431 |
| collected/output monomials | 66 | 7,326 |
| collected native raw terms | -- | 111 |

After native generation, every row stores its exact collected width and exact
degree-reserved aggregate work, then replay recomputes those statistics along
with all raw keys and coefficients.  These are transport guards only.

The landed inventory redispatches a lower-mask degree-one branch through at
most 11 further terms. Charging both the 66 intermediate branches and all
`66*11=726` redispatch outputs gives the conservative static cap
`163,644*(66+726) = 129,606,048` path contributions. The exact retained set is
far smaller:

| exact preclosure resource | count |
|---|---:|
| compact paths | 26,078 |
| depth 0 / 1 / 2 paths | 14,766 / 10,313 / 999 |
| leaves | 2,794 |
| raw boundary occurrences | 4,230 |
| full-identity boundary keys | 1,066 |
| contributors in nonzero row blockers | 4,214 |
| blocked rows | 1,289 |
| unit-cache entries / cached paths | 2,945 / 4,019 |

All compact paths are eight bytes. Raw occurrences survive row-local
cancellation; the nonzero projection contains 28 repeated row/key groups and
omits eight exact zero sums.

After complete boundary closure the genuine/product universe has the proved
bound 3,237.  Therefore conservative dense guards are:

| resource | bound |
|---|---:|
| final collected row entries | `1,968*3,237 = 6,370,416` |
| dense elimination updates | `1,968*1,967*3,237 = 12,530,608,272` |
| source-row provenance weights | `1,968^2 = 3,873,024` |
| sector/factorization cache keys | `2*2^9 = 1,024` |
| preclosure normalization recursion depth | 2 |

The exact landed parent-row assembly is much smaller than those guards:

| exact assembly resource | count |
|---|---:|
| rows / zero rows | 1,968 / 0 |
| paths, boundary / genuine | 26,078, split 4,230 / 21,848 |
| plans | 1,066 |
| raw / nonzero / canceled boundary groups | 4,202 / 4,194 / 8 |
| genuine row groups | 20,111 |
| columns | 1,734 = 1,728 genuine + 6 products |
| grouped / raw-route contributions | 28,096 / 30,353 |
| collected entries / maximum row width | 22,424 / 45 |
| mass-power steps | 26,850 |
| coefficient multiply / add / divide operations | 32,647 / 13,502 / 33,574 |
| retained coefficient terms / bytes | 71,270 / 107,123 |

For a preclosure inventory, a topology-independent upper bound on typed
`D<=2,N<=1` physical-mask/power keys is

```text
2 * sum_(r=0)^9 binomial(9,r) binomial(r+2,2) (11-r)
= 112,640.
```

This deliberately includes rank-deficient and genuine masks and is therefore
loose. The modular discovery result of 910 opaque columns is neither a
proof-valid substitute for this guard nor the identity used by the exact
1,066-key inventory.

The inventory has typed limits for paths, leaves, caches, occurrences,
contributors, coefficient degree, dense operand/result universes, retained
coefficient terms, and bounded coefficient serialization. Its metered payload
counter is not allocator RSS and does not claim to bound Symbolica's opaque
GCD workspace or integer bit growth. Component transport separately preflights
plans, occurrences, components, map entries, signed lines, local slots,
quadratic coefficients, parity projections, scalar branches, and a fixed-size
exact-rational work envelope; its retained census is frozen in tests. The
landed lower-loop closures and parent-row assembler add independent limits for
reduction outputs, path/group replay, normalized entries, coefficient
operations, degree, terms, and serialized bytes. Future elimination must add
limits for updates, provenance weights, rejected modular samples, and
exceptional factors. Resource exhaustion cannot truncate a row or promote its
hardest surviving coordinate to a terminal.

For comparison, the stable modular 123-seed image observed 23,300 collected
input entries, maximum input-row width 45, 2,644 preclosure columns, and
maximum pivot width 223.  These are planning measurements, not exact guards.

## 10. Proof boundary

The following statements are exact today:

- the type masks, `p/q` values, seed counts, ordered manifest checksum
  `fnv1a64:0bff80d5dddb4340`, checked raw-row labels, and 1,968 raw origins;
- constructor-cached exact built-in H/X family eligibility and per-origin
  authentication of topology, corner type, and reference mask;
- regeneration, mapping, and replay of all 1,968 native identities with their
  collected integral keys and coefficients retained as provenance;
- degree-two polynomial transport with the exact per-term limits 2, 11, 121,
  and 66, exact per-row statistics under the 111/13,431/7,326 conservative
  bounds, and typed same-mask/strictly-lower-mask branches;
- exact depth-two strict-mask dispatch of all 1,968 rows into 26,078 compact
  replay paths and 2,794 leaves;
- the full-identity preclosure census of 4,230 raw boundary occurrences and
  1,066 boundary keys, including row-local duplicate and cancellation
  provenance;
- replay of scaleless classifications and scalar-support factorization
  witnesses, with all dotted/numerator powers retained rather than silently
  reduced;
- exact component transport for all 1,066 keys and all 4,230 occurrence
  references, split into 577 N0 and 489 N1 plans, with complete local bases,
  signed-line replay, eleven-probe affine replay, checked local shifts, and
  separately owned cross-component parity zeros;
- exact local reduction, product convolution, and mass normalization for the
  243 T1/S2-only plans: 25 cached targets, 1,442 component uses, 1,134 closed
  occurrences, 3,096 outside-slice occurrences, and checksum
  `fnv1a64:a2b92a62c988d2cb`;
- exact local reduction, product convolution, and mass normalization for the
  complementary 823 B4/F5/M6 plans: 204 cached targets, 1,884 branches, 3,768
  component calls, and 3,096 closed occurrence references, all through one
  caller-context three-loop `D2/N1` finite box;
- exact source-backed assembly and canonical scaling of all 1,968 parent rows
  over `Q(d)`: 26,078 path dispositions, 4,202 raw boundary groups, 20,111
  genuine row groups, 1,734 columns, 22,424 entries, no zero rows, and maximum
  width 45. The grouped 28,096-contribution route agrees before scaling with
  an independent 30,353-contribution raw-path route, every final coefficient
  is literally `m2`-free, and the frozen checksum is
  `fnv1a64:a55ce4ffda6f8f5c`;
- the current production corner certificate has 223 columns, exact rank 159,
  and 64 unresolved coordinates;
- the combinatorial `(2,2)` genuine universe and static resource bounds above.

The following are stable discovery evidence from three prime-field images:

- the 64 corner-free coordinates have the reported grading split;
- the axis pivots 44 inherited coordinates;
- the 13 mixed seeds pivot the remaining four nonterminal coordinates;
- the resulting rank/column/nullity and boundary censuses;
- the retained inherited set is exactly ten corners plus six products; and
- that inherited set is unchanged by the full 296-seed holdout.

The two landed occurrence partitions are now integrated and all parent rows
are mass-normalized and canonicalized over `Q(d)`. There is still no exact
next-shell rank, source-row weights, exceptional-factor inventory, or
elimination certificate. The earlier 969/511/191 completed/outside/mixed
row-incidence census describes only the three-loop-component sibling before
the landed integration.
The exact 1,066-key identity does not promote or prove the modular 910-column
projection.

The probe does not emit and replay a selected nonzero minor, and it does not
construct exact `Q(d,m2)` source-row weights.  Its rank figures are therefore
not production proofs.  Repeated primes and dimensions make accidental
specialization substantially less plausible, but they do not prove generic
rank, characterize exceptional dimensions, prove the 13-seed set minimal, or
establish an unrestricted master basis.

## 11. Next implementation order

1. **Landed:** freeze the 123-seed manifest, checksum, checked raw IDs, and
   typed stable keys, with the 296-seed box retained as a modular holdout.
2. **Landed:** generalize the genuine halo mapper to degree two, map and replay
   all 1,968 raw origins with native provenance, and emit typed transport
   branches.
3. **Landed:** recursively dispatch strict masks, enumerate all exact
   full-identity boundary keys and raw occurrences, retain cancellation
   provenance, and replay the complete bounded preclosure inventory.
4. **Landed:** map and replay all 1,066 keys and 4,230 occurrence references
   into complete component-local bases, including numerator affine images and
   cross-component parity witnesses.
5. **Landed partial closure:** compose and replay every T1/S2-only plan,
   preserving component identity through ordinary convolution and proving
   mass normalization.  Its slice status closes 243 plans and records the 823
   complementary plans outside.
6. **Landed three-loop-component sibling:** build one authenticated
   caller-context `ThreeLoopReductionPipeline` for `(D,N)<=(2,1)`, reduce the
   exact 41/89/70 B4/F5/M6 targets, semantically map all five possible
   terminals, compose the four T1 targets, and replay all 823 plans and 3,096
   occurrence references.  The dedicated B4/F5/M6 reducers remain independent
   cross-checks.
7. **Landed parent-row assembly:** combine both exact occurrence partitions,
   bind all 4,230 boundary occurrences and all 26,078 path dispositions, and
   build 1,968 mass-normalized canonical rows over `Q(d)`. Compare the grouped
   production route with independent raw-path reconstruction before scaling.
8. **Next:** reuse the three-image skeleton for sparse planning, then perform exact
   elimination with source-row provenance and exceptional-factor recording.
9. Replay all 1,968 fully normalized rows through exact source weights and
   assert the inherited 48/16 disposition.  Freeze the exact rank and resource
   census only after this succeeds.
10. Run the remaining 173 mixed seeds as independent modular holdout evidence.
   Do not expand the advertised exact domain unless that larger manifest also
   receives its own exact replay certificate.
11. After the fixed shell is exact, grow target-driven shells for the newly
   free `D0/N1`, `D1/N2`, and `D2/*` coordinates.  Only stability under those
   further shells can support a broader four-loop reduction claim.
