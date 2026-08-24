# Design for exact higher factorized-boundary closure of the four-loop next shell

Date: 2026-08-13

## Outcome

The modular fixed-shell discovery probe exposes 910 opaque factorized-boundary
columns. They are not masters, and that modular projection is not an exact
production key identity.

The exact production preclosure inventory and its component-transport
certificate have now landed. Across all 1,968 rows the inventory retains 4,230
raw boundary occurrences and 1,066 full-identity keys, where identity includes
family fingerprint, all ten powers, scalar-support product, and the complete
decomposition witness. `FourLoopComponentTransport` maps every exact key into
complete local component bases and authenticates every occurrence reference.
`FourLoopT1S2Closure` closes and replays the 243 plans whose components are
only T1 or S2.  Its landed sibling, `FourLoopThreeLoopClosure`, now closes and
replays the complementary 823 plans (`T1*B4:223`, `T1*F5:494`,
`T1*M6:106`), split into 443 N0 and 380 N1 plans and referenced by 3,096
occurrences.  Each is an exact slice certificate and deliberately reports the
other slice outside its parent-status partition. Their common
`FourLoopNextClosedRows` consumer has now landed: it zips the complementary
partitions against the shared transport and assembles all 1,968 canonical
parent rows over `Q(d)` with exact source-backed replay.

The required parent domain is finite:

```text
(D,N) in {(1,1), (2,0), (2,1)}
```

and all probe-observed projected columns are H-family presentations.  More
importantly, exact component transport proves that every required three-loop
local target lies in `(D,N)<=(2,1)`.  The landed production service therefore
uses one authenticated `ThreeLoopReductionPipeline` with that complete finite
box, built in the parent plan's caller-owned coefficient context.  Existing
specialized services remain independent validation oracles:

- arbitrary positive one-loop tadpole recurrences;
- two-loop scalar sunset reduction through `D=2`;
- the tested three-loop `D<=1,N<=1` and scalar `D<=3,N=0` certificates;
- the replayable scalar `B4,D=2` shell; and
- the exact `F5,D2/N1` rebuild wrapper.

`ThreeLoopF5D2N1Reducer` covers the complete 15-target labelled `F5,D2/N1`
domain by rebuilding and replaying the same authenticated 306-row finite-box
kind of pipeline.  In production it is a focused cross-check, not a reason to
construct a second coefficient domain or dispatch only F5 targets.  The one
caller-context pipeline emits five fixed integral terminals, which the
adapter maps by semantic identity as

```text
I(1,1,1,0,0,0) -> T1^3
I(1,1,1,1,0,0) -> T1*S2
I(1,1,0,1,0,1) -> B4
I(1,1,1,1,1,0) -> F5
I(1,1,1,1,1,1) -> M6.
```

A local reduction may contain several of these terminals.  For example, the
B4 D2 certificate mixes `T1^3` and `B4`, while dotted F5 reductions can mix
`T1*S2`, `B4`, and `F5`.  Treating the requested component label as the sole
output product would therefore be incorrect.

The pure-`std` audit mode is part of
[`four_loop_next_shell_rank.rs`](../../tools/four_loop_next_shell_rank.rs):

```text
rustc --edition=2024 -D warnings -O \
  tools/four_loop_next_shell_rank.rs -o /tmp/four_loop_next_shell_rank

/tmp/four_loop_next_shell_rank 1000003 17 --component-boundary-audit
```

It runs no FORM, Mathematica, Cargo, or Symbolica process.

## 1. Guarantee and proof boundary

The landed aggregate milestone is named
`ExactFixedSeedParentRowsGenericQdEliminationPending`. It means that every
higher boundary in the versioned 123-seed manifest has been reduced and
replayed and that every parent row has been assembled over `Q(d)`. It does not
mean that:

- every four-loop boundary at arbitrary integer powers is reduced;
- the 123-seed four-loop shell is an unrestricted reduction;
- the ten genuine scalar corners are a minimal master basis; or
- the finite lower-loop candidate terminals are unrestricted masters.

The following facts are exact in the current repository:

- `FourLoopBoundaryReducer` gives an exact unimodular component witness for
  every scalar factorized corner and independently replays it;
- `FourLoopBoundaryHaloReducer` closes the complete H-family `D1/N0` boundary
  of the 160-row corner shell;
- the component ranks can only be `1+1+1+1`, `1+1+2`, `2+2`, or `1+3`;
- the local T1 and S2 formulae, the three-loop `D1/N1` finite box, the scalar
  `D<=3` finite box, and the scalar `B4,D=2` shell are exact tested services;
- the algebraic decomposition and componentwise odd-parity proof in section 4
  are exact for every single parent numerator once its map is replayed;
- `FourLoopNextInventory` exactly dispatches all strict masks to depth two and
  replays 26,078 compact paths, including scaleless and scalar-support
  factorization classifications;
- the exact preclosure census is 4,230 raw occurrences and 1,066 full-identity
  boundary keys, with cancellation provenance retained;
- `FourLoopComponentTransport` maps and independently replays all exact keys
  and occurrences, with 577 N0 and 489 N1 plans and product counts
  `52/223/494/106/91/100` in `S2^2/T1*B4/T1*F5/T1*M6/T1^2*S2/T1^4`
  order;
- `FourLoopT1S2Closure` closes the three T1/S2-only product classes, exactly
  243 plans (`134 N0`, `109 N1`) and 1,134 occurrence references. Its 25
  cached local targets serve 1,442 component uses; ordinary convolution and
  termwise mass normalization replay under checksum
  `fnv1a64:a2b92a62c988d2cb`. Its parent status retains 823 plans and 3,096
  occurrences as open; and
- `FourLoopThreeLoopService` and `FourLoopThreeLoopClosure` close and replay
  the complementary 823 plans and 3,096 occurrence references through one
  caller-context `D2/N1` pipeline.  This sibling's parent status retains the
  243 T1/S2 plans and 1,134 occurrence references as outside its slice; and
- `FourLoopNextClosedRows` authenticates both closures against the same
  inventory and transport, binds all 1,066 plans and 4,230 occurrences, and
  assembles and canonically scales all 1,968 parent rows over `Q(d)`.

The landed three-loop-component sibling has the following exact build census,
enumerated directly from the 1,066-plan transport and replayed by its closure:

| retained quantity | exact count |
|---|---:|
| B4/F5/M6 plans | 823 |
| N0 / N1 plans | 443 / 380 |
| components / complete local slots | 1,646 / 5,761 |
| scalar branches | 1,884 |
| base / constant branches | 443 / 323 |
| local T1 / B4 / F5 / M6 branches | 186 / 220 / 656 / 56 |
| component calls | 3,768 |
| T1 / B4 / F5 / M6 calls | 1,884 / 444 / 1,260 / 180 |
| unique targets / cache hits | 204 / 3,564 |
| unique T1 / B4 / F5 / M6 targets | 4 / 41 / 89 / 70 |
| occurrence references in this slice | 3,096 |
| outside-slice plans / occurrences | 243 / 1,134 |
| completed / outside / mixed row incidences | 969 / 511 / 191 |
| convolution pair operations | 7,356 |
| precollection / collected terms | 3,598 / 2,159 |
| mass-power steps / coefficient operations | 4,279 / 17,456 |
| retained closure output-coefficient bytes | 256,603 |

The target-degree histogram is an exact production-domain assertion, not the
modular component-service projection below:

| component | `(D,N) -> unique targets` |
|---|---|
| B4 | `(0,0):5`, `(0,1):2`, `(1,0):16`, `(1,1):8`, `(2,0):10` |
| F5 | `(0,0):6`, `(0,1):1`, `(1,0):25`, `(1,1):5`, `(2,0):43`, `(2,1):9` |
| M6 | `(0,0):6`, `(1,0):24`, `(2,0):40` |

Thus B4 needs numerator coverage only through `D1/N1`, F5 is the only
three-loop component needing `D2/N1`, and M6 is scalar through D2.  The four
T1 targets are powers `0..=3` and use the existing tadpole service.

`FourLoopThreeLoopService` builds the one caller-context finite pipeline once
for this complete manifest.  Its exact build validates 1,800 native target
identities and retains 502 semantic output terms in 12,555 coefficient bytes.
The underlying pipeline records 306 input equations, 149 rules, 157 dependent
equations, and a maximum of 30 terms per equation.  The frozen target-manifest,
service, and closure checksums are respectively
`fnv1a64:9bb3c1a6d4ea7bdd`, `fnv1a64:6a1b52ddb449d5bb`, and
`fnv1a64:da3c250b95b10976`.

The exact parent-row assembly census is:

| retained or performed quantity | exact count |
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

The grouped production route consumes each nonzero row/boundary group once.
An independent raw route replays every source path, including all canceled
groups, applies ordinary closures with direct parent-to-product mass weights,
and agrees before row-scale division. Every stored coefficient is literally
`m2`-free. The frozen checksum is `fnv1a64:a55ce4ffda6f8f5c`. This
production path is native Rust with Symbolica coefficient arithmetic and does
not execute FORM.

The following are stable three-image discovery evidence:

- the 910-column projected parent census;
- the component-dot splits in section 3;
- the induced numerator-service and parity-support counts in section 5; and
- the statement that exactly 31 projected parent columns emit a full five-line
  `F5,D2/N1` branch.

The probe constructs these supports over prime fields.  It does not serialize
exact affine coefficients, exact lower-loop source-row weights, or a selected
nonzero minor. The landed production inventory and transport certificate
independently enumerate and replay the exact paths and component maps over
`Q(d,m2)`, but do not identify the 1,066 full-identity keys with the probe's
910 projected columns. Lower-loop closure must be driven from these exact
transported plans; no modular component count is a production acceptance
condition.

## 2. Modular projected parent census

The probe's 910 projected opaque columns split as follows. These are
probe-identity keys, not the 1,066 exact witness-complete production keys:

| product | `D1/N1` | `D2/N0` | `D2/N1` | total |
|---|---:|---:|---:|---:|
| `S2^2` | 12 | 18 | 15 | 45 |
| `T1*B4` | 90 | 92 | 0 | 182 |
| `T1*F5` | 130 | 187 | 116 | 433 |
| `T1*M6` | 18 | 47 | 26 | 91 |
| `T1^2*S2` | 32 | 42 | 0 | 74 |
| `T1^4` | 50 | 35 | 0 | 85 |
| **total** | **332** | **421** | **157** | **910** |

There are 421 numerator-free projected columns and 489 one-numerator projected
columns. A production key must retain topology, all ten parent powers,
product, sector, and factorization
witness identity.  Product plus `(D,N)` is not sufficient: it loses dot
ownership, numerator routing, and the compact line maps needed by the lower
components.

The next-shell probe keys a boundary by

```text
(H/X topology, canonical product, ten parent powers).
```

The landed production inventory additionally binds the exact key to the full
replayed decomposition witness and family fingerprint.  Two equal exponent
vectors in incompatible completed bases cannot share a cache entry.

## 3. Component dot domains

The exact matroid split assigns every positive parent line to one component.
Conditional on the observed 910-column probe census, the following dot counts
are integer combinatorics; they do not depend on `d` or `m2`. Repeated
components are shown in sorted order.

| product/domain | component dot split | projected columns |
|---|---|---:|
| `S2^2,D1/N1` | `(S2:D1,S2:D0)` | 12 |
| `S2^2,D2/N0` | `(D2,D0)` / `(D1,D1)` | 10 / 8 |
| `S2^2,D2/N1` | `(D2,D0)` / `(D1,D1)` | 8 / 7 |
| `T1*B4,D1/N1` | `(T1:D1,B4:D0)` / `(T1:D0,B4:D1)` | 29 / 61 |
| `T1*B4,D2/N0` | `(D2,D0)` / `(D1,D1)` / `(D0,D2)` | 7 / 28 / 57 |
| `T1*F5,D1/N1` | `(T1:D1,F5:D0)` / `(T1:D0,F5:D1)` | 32 / 98 |
| `T1*F5,D2/N0` | `(D2,D0)` / `(D1,D1)` / `(D0,D2)` | 10 / 50 / 127 |
| `T1*F5,D2/N1` | `(D2,D0)` / `(D1,D1)` / `(D0,D2)` | 7 / 41 / 68 |
| `T1*M6,D1/N1` | `(T1:D1,M6:D0)` / `(T1:D0,M6:D1)` | 4 / 14 |
| `T1*M6,D2/N0` | `(D2,D0)` / `(D1,D1)` / `(D0,D2)` | 2 / 12 / 33 |
| `T1*M6,D2/N1` | `(T1:D1,M6:D1)` / `(T1:D0,M6:D2)` | 6 / 20 |

For `T1^2*S2` the 32 `D1/N1` keys split evenly: 16 put the dot on a
tadpole and 16 on S2.  Its 42 `D2/N0` keys split as

| component dot multiset | keys |
|---|---:|
| `T1:D2 + T1:D0 + S2:D0` | 5 |
| `T1:D1 + T1:D1 + S2:D0` | 3 |
| `T1:D1 + T1:D0 + S2:D1` | 17 |
| `T1:D0 + T1:D0 + S2:D2` | 17 |

For `T1^4`, all 50 `D1/N1` keys have one dotted tadpole.  Among the 35
`D2/N0` keys, 10 put both dots on one tadpole and 25 put one dot on each of two
tadpoles.

These splits give the scalar component domain before the sole numerator is
lowered.  Lowering never increases dot degree: it either retains the base
through the constant branch or subtracts one local denominator power.

## 4. Exact factorized numerator map

### 4.1 Combined loop map

For a factorization witness, let the selected parent routing basis be `B`, so

```text
p = B k,                  det(B) = +/-1.
```

For component `c`, let `U_c` be the stored map from the component's selected
`p` slots to its frozen reference momenta.  Scatter the `U_c` blocks into the
recorded, not necessarily contiguous, `global_basis_slots`, obtaining
`U_scatter`.  With all reference component loop momenta concatenated as
`ell`, the direction that must be used is

```text
k = T ell,                T = B^-1 U_scatter.          (4.1)
```

Replay every active line:

```text
q_parent B^-1 U_scatter
  = sign * q_reference embedded in its component.     (4.2)
```

Equation (4.2) is essential.  A transposed map can have the right dimensions
and determinant while transporting every numerator incorrectly.

### 4.2 Complete block reference basis

Use a ten-entry completed four-loop scalar-product basis whose intra-component
entries are complete lower-family denominator bases:

| component | complete local entries | active corner entries |
|---|---:|---:|
| T1 | 1 | 1 |
| S2 | 3 | 3 |
| B4 | 6 | tetrahedron positions `0,1,3,5` |
| F5 | 6 | tetrahedron positions `0..4` |
| M6 | 6 | tetrahedron positions `0..5` |

The remaining entries are cross-component scalar products.  The resulting
counts are:

| component ranks | intra entries | cross entries | total |
|---|---:|---:|---:|
| `1+1+1+1` | 4 | 6 | 10 |
| `1+1+2` | 5 | 5 | 10 |
| `2+2` | 6 | 4 | 10 |
| `1+3` | 7 | 3 | 10 |

This convention is why B4 compact positions must lift as
`0,1,2,3 -> 0,1,3,5`.  B4 positions 2 and 4 and F5 position 5 of the complete
tetrahedron basis are inactive numerator directions, not additional active
lines.

### 4.3 Affine expansion

For the sole negative parent entry `n`, transform the exact flattened
quadratic form, including its mass shift:

```text
D_n(k) = Q_n(T ell) + s_n

       = C
         + sum_(c,j) A_(c,j) D_(c,j)
         + sum_(c<e,a,b) X_(c,a,e,b)
             ell_(c,a).ell_(e,b).                    (4.3)
```

Off-diagonal flattened coefficients already multiply `k_i.k_j`; they must not
be doubled a second time.  Replay (4.3) by comparing the constant and every
one of the ten scalar-product coefficients over exact rationals.

The surviving scalar branches are

```text
C * product_c I_c(b_c)

+ A_(c,j) * I_c(b_c-e_j) * product_(h!=c) I_h(b_h).  (4.4)
```

All shifts are checked.  Lowering an active entry pinches a line; lowering an
inactive B4/F5 entry creates one local numerator; lowering the only T1 entry
produces a scaleless zero component.  A constant branch preserves the original
component dots.

### 4.4 Cross-component parity

Every cross term in (4.3) factorizes into two odd tensors:

```text
[integral d ell_c ell_(c,a)^mu F_c]
[integral d ell_e ell_(e,b)_mu F_e]
* product_(h!=c,e) [integral d ell_h F_h].             (4.5)
```

Each scalar component integrand, including arbitrary scalar dots in this
manifest, is invariant under simultaneous reversal of all loop momenta in
that component.  Each rank-one bracket is therefore zero.

The native `VacuumTensorProjector` already returns zero for odd rank, but it
must be applied separately to the two component rank-one tensors.  Applying a
single global rank-two projector to `ell_c.ell_e` does not prove factorized
parity and must be rejected by replay.  A parity witness records component
ownership, local vector coordinates, the exact cross coefficient, and two
rank-one zero projections.

At `N=1` no even-rank component tensor survives.  At `N=2`, products of two
cross terms can have even rank in both components; that larger tensor domain
is outside this milestone.

## 5. Modular projected component-service census

The three-image affine audit gives the following projected maximum local
domains. `Lr` denotes the number of positive lines remaining after a local
lowering. The service domains are checked locally; the incidence counts below
remain modular discovery evidence until matched against exact production
keys.

| parent product | induced component targets | highest required service |
|---|---|---|
| `T1^4` | T1 `L0/L1`, scalar `D<=2` | tadpole recurrence/scalelessness |
| `T1^2*S2` | T1 `L0/L1`; S2 `L2/L3`, scalar `D<=2` | two-loop scalar `D2` |
| `S2^2` | S2 `L2/L3`, scalar `D<=2` | two-loop scalar `D2` |
| `T1*B4` | B4 `L3/L4`; full B4 through `D1/N1`, scalar B4 through `D2/N0` | caller-context three-loop `D2/N1` pipeline; B4 D2 shell cross-check |
| `T1*F5` | F5 `L4/L5`; full F5 through `D2/N1` | caller-context three-loop `D2/N1` pipeline; F5 wrapper cross-check |
| `T1*M6` | M6 `L5/L6`, scalar `D<=2` | caller-context three-loop `D2/N1` pipeline; scalar D3 cross-check |

Complete component bases remove irreducible numerator degrees for T1, S2,
and M6.  Only B4 and F5 have inactive directions in the completed
tetrahedron basis.

The observed full-component numerator incidences, deduplicated within each
parent key, are:

| parent class | full component target | parent-key incidences |
|---|---|---:|
| `T1*B4,D1/N1` | `B4,D0/N1` | 18 |
| `T1*B4,D1/N1` | `B4,D1/N1` | 38 |
| `T1*F5,D1/N1` | `F5,D0/N1` | 13 |
| `T1*F5,D1/N1` | `F5,D1/N1` | 41 |
| `T1*F5,D2/N1` | `F5,D0/N1` | 3 |
| `T1*F5,D2/N1` | `F5,D1/N1` | 19 |
| `T1*F5,D2/N1` | `F5,D2/N1` | **31** |

The other local branches in those projected columns are scalar or
proper-sector targets. The 31-column line motivated the now-landed complete
local F5 service. The landed exact transport now records the induced local
branches for every production plan; service composition must consume those
exact branches rather than infer usage from the 31-column projection.

The observed cross-parity parent-key incidences are:

| parent class | component pair | keys with nonzero cross support |
|---|---|---:|
| `S2^2,D1/N1` / `D2/N1` | S2 x S2 | 12 / 15 |
| `T1*B4,D1/N1` | T1 x B4 | 46 |
| `T1*F5,D1/N1` / `D2/N1` | T1 x F5 | 88 / 89 |
| `T1*M6,D1/N1` / `D2/N1` | T1 x M6 | 18 / 26 |
| `T1^2*S2,D1/N1` | T1 x S2 / T1 x T1 | 26 / 9 |
| `T1^4,D1/N1` | T1 x T1 | 50 |

These are key incidences, not cross-term counts: one key can have several
nonzero cross-coordinate coefficients. Production transport now retains and
replays every exact cross coefficient before applying parity.

## 6. Existing API audit

### 6.1 Four-loop services

`FourLoopBoundaryReducer` is the correct source of factorization truth. It
authenticates the global unimodular basis, component blocks, compact line
matches, and lower master labels. Its reduction surface deliberately accepts
only scalar corners, so it cannot close any of the 1,066 transported powered
keys directly. The number 910 belongs only to the modular projection.

`FourLoopBoundaryHaloReducer` exactly closes H-family `D1/N0` through six fixed
component-dot formulae.  Its domain validator explicitly rejects every
`D1/N1`, `D2/N0`, and `D2/N1` input.  Its witness-plan cache and
mass-normalized product output are reusable design patterns, but its public
coverage must not be widened without the new affine and lower-loop witnesses.

`ProductBoundaryReducer` closes positive-power unimodular products of
tadpoles, so it can serve scalar `T1^4,D2/N0` after an adapter maps its stable
parent-family representative to `MasterProduct(T1^4)`.  It rejects every
negative power and therefore cannot close `T1^4,D1/N1`.

`FourLoopHaloMapper` transports degree-one numerator terms between genuine
four-loop presentations and remains intentionally distinct from factorized
transport. Its landed sibling `FourLoopComponentTransport` implements the
block-diagonal map (4.1) against authenticated inventory contexts. It retains
witness-indexed component ownership, complete T1/S2/B4/F5/M6 local bases, and
local/cross branches for every N1 numerator. Construction and independent
replay check `T=B^-1 U_scatter`, all active lines, eleven exact affine probes,
and two rank-one parity projections for every nonzero cross coefficient. It
does not call lower-loop reduction services or mass-normalize products.

`FourLoopT1S2Closure` is the separate exact consumer for transported plans
containing only T1 and S2. It retains repeated-factor witness identity until
the final commutative product, authenticates every local direct/IBP proof at
construction, convolves in the caller's exact coefficient context, and
applies (10.1) only after ordinary collection. It is intentionally not an
adapter for B4, F5, or M6.

### 6.2 Tensor and product algebra

`VacuumTensorProjector` provides exact odd-rank zeros and even-rank projectors.
It has no intrinsic notion of factorization components.
`FourLoopComponentTransport` supplies that ownership, binds two separate
rank-one calls to witness component IDs, and stores and replays the resulting
parity witnesses.

`TensorFamilyReducer` can lower scalar products in one complete family and can
compose with two- and three-loop pipelines.  It does not split a parent
quadratic form into components or convolve component products.  Its checked
affine-expansion machinery is reusable after (4.3) has selected a local
component.

`ProductLinearCombination::checked_convolve_with_limits` already supplies the
correct checked commutative convolution.  Use it component by component and
retain both distinct-term and Cartesian-pair budgets.

### 6.3 Lower-loop scalar services

`TwoLoopBoundaryReducer` closes arbitrary numerator powers only in proper
two-line sectors.  `TwoLoopReductionPipeline` closes the genuine three-line
sunset inside an advertised finite dot box.  A `max_dots=2` pipeline built in
the parent coefficient context covers every S2 target above.

`ThreeLoopBoundaryReducer` exactly closes tree and paw sectors with bounded
arbitrary numerator powers; its induced positive sunsets now use the complete
all-dot service rather than the retained finite compatibility table. It
deliberately returns `None` for B4, F5, and M6.

`ThreeLoopReductionPipeline` certifies every target in its configured finite
box at construction and rejects an unresolved non-whitelisted terminal.
`FourLoopThreeLoopService` now builds it once with `max_dots=2` and
`max_numerator_degree=1` in the parent `CoefficientContext`, then shares that
one table across all 200 distinct B4/F5/M6 targets.  The caller-context family
constructor is essential: matching parameter spellings do not by themselves
make independently constructed Symbolica polynomial maps composable.

The tested scalar `max_dots=3,max_numerator_degree=0` configuration covers all
scalar B4, F5, M6, and proper targets required here.  The specialized
`ThreeLoopB4D2Shell` independently replays all three scalar B4 D2 orbits and
records the exceptional factor `d-4`.

The generic `D=2,N=1` configuration is exercised exactly by the focused F5
service and is broad enough for the exact target histogram above.  It is a
deterministic rebuild certificate rather than a compact persisted
source-weight certificate; compact source weights remain an optional
performance optimization.  The landed service authenticates the caller's
coefficient map, invokes and replays the shared pipeline, translates each of
its five terminals semantically, and retains the mixed output combination.
The sibling closure then preserves parent component-map provenance while it
convolves those outputs and mass-normalizes each completed plan.

`ThreeLoopProperDotReducer` and `ThreeLoopTopDotReducer` give useful scalar
descent recurrences for F5 and M6, respectively.  Neither accepts numerators,
so both remain independent checks rather than production dispatch branches.

## 7. Exact component formula layer

All formula coefficients must be constructed in the parent shell's one
`CoefficientContext`.  Same-named variables from an independently created
context are not an acceptable substitute.

### 7.1 Tadpoles

For every positive integer `a`,

```text
T(a)/T1 = product_(r=1)^(a-1) (2*r-d)/(2*r*m2).       (7.1)
```

The new manifest needs only `a<=3`.  In particular,

```text
T(2) = (2-d)/(2*m2) T1,
T(3) = (2-d)*(4-d)/(8*m2^2) T1.                       (7.2)
```

A nonpositive sole T1 power is scaleless.

### 7.2 Sunset dots

Line symmetry gives

```text
S(2,1,1) = (3-d)/(3*m2) S2.                           (7.3)
```

There are two D2 scalar orbits.  Native two-loop IBPs give

```text
S(3,1,1)
  = (8-d)*(3-d)/(18*m2^2) S2
    - (d-2)^2/(12*m2^3) T1^2,

S(2,2,1)
  = (2-d)*(3-d)/(9*m2^2) S2
    + (d-2)^2/(12*m2^3) T1^2.                         (7.4)
```

The mass derivative check is

```text
2*S(3,1,1) + 2*S(2,2,1)
  = (4-d)/m2 * S(2,1,1).                              (7.5)
```

Production may freeze (7.3)--(7.4) as replayed direct rows or invoke the
finite two-loop pipeline.  Tests must compare both paths.

### 7.3 B4 dots

The D1 formula is

```text
B4(D1) = (8-3*d)/(8*m2) B4.                           (7.6)
```

At D2 the actual D8 stabilizer has three scalar orbits: a triple dot `A`, an
adjacent double dot `C_adj`, and an opposite double dot `C_opp`.  The replayed
shell gives

```text
A = -3*(d-2)^3/(64*(d-4)*m2^3) T1^3
    + (9*d^3-117*d^2+458*d-560)
        /(128*(d-4)*m2^2) B4,

C_adj = C_opp
      = (d-2)^3/(32*(d-4)*m2^3) T1^3
        + (9*d^3-81*d^2+242*d-240)
            /(64*(d-4)*m2^2) B4.                     (7.7)
```

Every use records `d-4 != 0` as an exceptional-locus condition.

### 7.4 Scalar F5 and M6

The existing one-dot rows remain valid.  At D2, apply the exact scalar
three-loop finite certificate, or recursively compose the F5/top-dot
recurrences with the B4 D2 shell and factorized boundaries.  The production
result must retain a replayable derivation; it may not infer individual D2
orbits from mass homogeneity alone.

For any L-loop equal-mass scalar integral with total positive denominator
power `A`, mass homogeneity supplies the independent check

```text
sum_i a_i I(a+e_i) = (A - L*d/2)/m2 * I(a).           (7.8)
```

Equation (7.8) checks sums of dotted outputs but does not separate dot
partitions or replace native IBP provenance.

## 8. Landed caller-context three-loop finite pipeline

The concrete discovery manifest is recorded in
[`f5_d2n1_induced_manifest.md`](f5_d2n1_induced_manifest.md). Across three
prime images the 31 projected parent-column incidences give nine labelled
targets and all six orbits of the complete local F5 D2/N1 domain. These are
still modular support facts. `ThreeLoopF5D2N1Reducer` supplies an exact
deterministic rebuild oracle for all 15 public labelled targets and validates
their 135 native target IBPs. Its underlying authenticated `(D,N)=(2,1)`
pipeline contains 95 advertised dependency targets, 34 genuine seeds, and 306
native solver rows; public replay rebuilds that pipeline. The service does not
persist source-row weights or a separate exceptional-factor list. A compact
persisted certificate would be an optional performance improvement, not a
prerequisite for using the accepted exact rebuild service.

`FourLoopThreeLoopService` and `FourLoopThreeLoopClosure` now consume the 823
three-loop-component transport plans.  Together they:

1. enumerate and deduplicate all 204 exact local targets, including the four
   T1 powers and the 41/89/70 B4/F5/M6 split;
2. preserve all six local powers, parent key, component ID, and witness
   provenance while canonicalizing through the authenticated tetrahedron;
3. build one `(D,N)<=(2,1)` pipeline in the exact parent coefficient context
   and reduce every three-loop target through it;
4. translate the five integral terminals to `T1^3`, `T1*S2`, B4, F5, and M6,
   retaining mixtures and rejecting any sixth terminal;
5. leave the B4 D2 shell, F5 D2/N1 wrapper, and scalar dot recurrences as
   selected independent checks rather than rebuilding production coefficients
   in another variable map;
6. cache the 204 exact results and replay all 3,768 component uses; and
7. expose only this exact target manifest, not a parametric or unrestricted
   three-loop domain.

The F5 wrapper's complete build-and-replay test takes about 196 seconds.  This
is evidence that repeatedly rebuilding subclass services would dominate the
closure.  The landed service builds the generic caller-context pipeline once,
replays it at the certificate boundary, and performs every target, branch,
occurrence, and tamper check in the same coefficient map.

A previously reported three-image six-seed/54-row compact shell is retracted.
Its standalone finite-field prototype encoded
`partial_(k_a) q_i^2 . k_b` with the factor two only on the diagonal scalar
product instead of on every `q_i` component.  For example, it produced half
of the native identity for `partial_(k1) k1^2 . k2`.  After correction, the
54-row system has rank 35 in 65 columns and all six targets retain genuine
F5/B4 coordinates at each of the three images.  RustRed's native generator and
an independently expanded exact derivative agree and rejected the false
closure.  No compact production certificate is claimed.

The sibling composes all 823 plans and reports 3,096 completed occurrence
references, while retaining the 243 T1/S2 plans and 1,134 occurrence
references outside its slice.  Its 969 completed-row incidences, 511
outside-row incidences, and 191 mixed rows are a partition census rather than
assembled parent equations. `FourLoopNextClosedRows` now performs that
separate aggregate step: it combines both partitions, substitutes all 4,230
occurrences, and returns
`ExactFixedSeedParentRowsGenericQdEliminationPending` only after all 1,968
rows have been assembled and replayed.

The modular probe's 110 free opaque columns remain discovery evidence only;
neither they nor any resulting four-loop coordinate may be relabelled as a
master.

## 9. Production types and stages

Suggested records are:

```text
FourLoopHigherBoundaryKey {
    family_fingerprint,
    topology,
    parent_powers: [i32; 10],
    sector_mask,
    advertised_product,
    factorization_witness_hash,
}

FactorizedComponentId {
    component_index,
    global_basis_slots,
    master,
    compact_to_local_positions,
}

FactorizedQuadraticMapWitness {
    key,
    B,
    component_maps,
    scattered_map,
    T,
    active_line_replays,
    reference_basis_columns,
    numerator_source_position,
    constant,
    local_coefficients,
    cross_coefficients,
}

ComponentParityWitness {
    map_hash,
    left_component,
    right_component,
    left_vector,
    right_vector,
    coefficient,
    left_rank_one_zero,
    right_rank_one_zero,
}

ComponentReductionWitness {
    component,
    local_powers,
    guarantee: DirectFormula | ExactFiniteCertificate,
    certificate_hash,
    exceptional_factors,
    product_terms,
}

FourLoopHigherBoundaryClosure {
    key,
    quadratic_map,
    parity_witnesses,
    scalar_branches,
    component_reductions,
    convolution_witnesses,
    ordinary,
    mass_normalized,
}

FourLoopNextClosedRowsStatus {
    ExactFixedSeedParentRowsGenericQdEliminationPending,
}
```

Repeated factors require stable component IDs until all local shifts and
parity terms are processed.  Only final master products are commutative.
Merging two T1 or S2 components before numerator transport loses ownership
and invalidates (4.5).

Implementation stages are:

1. **Landed exact preclosure inventory.** Generate all 1,968 parent raw rows,
   dispatch strict masks, and freeze the 1,066 full-identity keys and 4,230 raw
   occurrences with exact replay.
2. **Landed factorized mapper.** Build and replay (4.1)--(4.3), with exact
   component IDs, checked local shifts, and componentwise parity for all 577
   N0 and 489 N1 plans.
3. **Landed T1/S2 product closure.** Compose caller-context T1 recurrences and
   S2 top-dot/boundary services for all `T1^4`, `T1^2*S2`, and `S2^2` plans.
   This closes 243 exact plans across both N0 and N1, rather than claiming all
   scalar N0 plans.
4. **Landed three-loop-component sibling closure.** One caller-context
   three-loop `(D,N)<=(2,1)` pipeline supplies semantically mapped outputs for
   all 823 B4/F5/M6 plans.  The certificate replays 204 cached targets and
   3,768 component uses and mass-normalizes each plan only after ordinary
   convolution.  It reports 3,096 completed occurrences and the 1,134 T1/S2
   occurrences outside its slice; it is not by itself a complete parent
   certificate.
5. **Landed four-loop parent-row integration.** Substitute all 4,230
   occurrences, collect 20,111 genuine row groups, compare grouped and raw
   source-backed routes, mass-normalize and canonically scale all 1,968 rows.
6. **Pending exact elimination.** Plan pivots from the closed rows, retain
   exact source-row weights and exceptional factors, and replay the resulting
   fixed next-shell elimination certificate.

## 10. Mass normalization and product closure

For a parent integral `a` and output product `P`, define

```text
w(P) = sum_(master,multiplicity)
         multiplicity * master.physical_lines().
```

Every ordinary coefficient `r_P` must satisfy

```text
r_P * m2^(sum_i a_i - w(P)) in Q(d).                  (10.1)
```

Inspect actual Symbolica numerator and denominator degrees at `m2`; do not
infer cancellation from dimensions.  Retain both ordinary and normalized
forms in the closure witness.

All branches must collect into the same six product keys:

```text
T1^4, T1^2*S2, S2^2, T1*B4, T1*F5, T1*M6.
```

An unexpected seventh product is an error.  It is not a reason to extend the
terminal whitelist during construction.

## 11. Replay obligations

Construction and public replay must independently:

1. verify the versioned exact key and occurrence manifest;
2. rebuild the H family and its fingerprint in the stored coefficient context;
3. replay each scalar factorization witness and advertised product;
4. reconstruct `B^-1 U_scatter`, verify its determinant, and replay every
   active signed line;
5. reconstruct the complete block reference basis and replay the constant and
   ten quadratic coefficients in (4.3);
6. project every nonzero cross coefficient as two separately owned rank-one
   zeros;
7. regenerate every local power vector with checked shifts and replay every
   lower-loop direct formula or finite certificate;
8. replay every checked product convolution and the six-key whitelist;
9. verify (10.1) term by term and literal absence of residual `m2`;
10. substitute every cached closure and reproduce all 1,968 normalized,
    canonically scaled four-loop rows; and
11. in the future elimination layer, reconstruct each four-loop pivot from
    exact source-row weights, reduce all 1,968 rows to zero, and retain every
    exceptional factor in `d`.

Finite-field images may plan sparse pivots and act as holdouts.  They may not
replace exact coefficients, exact source weights, or exact numerator maps.

## 12. Resource bounds

The discovery projection has 421 numerator-free and 489 one-numerator opaque
columns. The exact production inventory instead has 1,066 witness-complete
keys, now frozen as 577 N0 and 489 N1 transport plans. The exact retained
transport census is 2,423 components, 9,592 component-map entries, 5,988
signed-line replays, 6,928 complete local slots, 4,890 transformed
coefficients, 3,182 local coefficient inspections, 1,708 cross inspections,
1,116 rank-one projections, and 2,338 scalar branches. For one N1 key the
component rank partitions prove the following gross caps:

| resource per key | cap |
|---|---:|
| transformed scalar-product coefficients | 10 |
| cross coefficients | 6 |
| component rank-one projector calls | 12 |
| surviving scalar branches | 8 |
| component target calls before caching | 32 |
| precollection product contributions | 40 |
| collected allowed products | 6 |

The mapper retains the following conservative all-1,066-as-N1 batch preflight
for construction and replay even though the exact N1 split is smaller:

```text
quadratic coefficient inspections      10,660
cross coefficients                      6,396
component rank-one projector calls     12,792
scalar branches                         8,528
component target calls                 34,112
precollection product terms            42,640
cached collected product terms          6,396
```

These bounds apply to unique closure construction.  The number of occurrences
in the 1,968 raw rows is a separate exact manifest field.  Cache by the full
key and substitute the cached result into every occurrence; do not repeat
lower-loop elimination per coefficient occurrence. The exact inventory has
4,230 raw boundary occurrences. The upstream 163,644 raw-term-incidence bound
guards transport work; it is not a boundary-occurrence ceiling.

Additional dynamic limits must cover:

- unique witness plans and signed-line matches;
- exact matrix and quadratic-map operations;
- local affine expansion terms;
- lower-loop cache keys and rebuild work;
- component-reduction output terms;
- product-convolution pair operations;
- coefficient numerator/denominator degree and serialized bytes;
- exceptional factors;
- four-loop normalized entries, elimination updates, and source weights.

Every aggregate request is preflighted before the first Symbolica coefficient
or lower-loop certificate is built.  Resource exhaustion returns a typed error
and cannot truncate a closure or promote a remainder.

## 13. Acceptance tests

1. **Exact inventory.** Replay the landed 1,066-key/4,230-occurrence
   full-identity preclosure census from all 1,968 rows. The modular 910-column
   projection and component-dot table remain discovery evidence unless a
   separate exact projection proves their relationship to these keys.
2. **Three-image holdout.** Run the pure-std audit at
   `(1000003,17)`, `(1000033,29)`, and `(1000037,37)` and require identical
   component, numerator-service, and parity supports.
3. **Map replay (landed).** Exhaust every unique witness, nonidentity component map,
   generated auxiliary numerator, and inactive physical numerator.  Corrupt
   `B`, one scattered block, the map direction, a mass constant, and each
   quadratic coefficient in turn; replay must fail.
4. **Compact positions (landed).** Exhaust T1, S2, all four B4 compact positions with
   lift `0,1,3,5`, all five F5 positions, and all six M6 positions.
5. **Parity transport (landed).** Exhaust every nonzero cross coefficient and every component
   pair in section 5.  Require two separate rank-one zeros.  Include a negative
   test showing that a global rank-two projection is not accepted as the
   stored parity witness.
6. **Local lowering transport (landed).** Exhaust every exact scalar branch. Check active-line
   pinches, B4/F5 inactive numerators, T1 scaleless zeros, repeated components,
   and checked-shift failures.
7. **Local-service provenance (landed).** Replay the T1/S2 direct proofs and
   the one authenticated caller-context `D2/N1` pipeline.  Check all five
   semantic terminal mappings and mixed outputs, and compare selected B4 D2,
   F5 D2/N1, F5-dot, and M6-dot targets with their independent focused
   services.  Do not rebuild one pipeline per target or component class.
8. **Exact sibling census (landed).** Freeze all 823 plans, the 223/494/106
   class split, 204-target degree histogram, 1,884 branches, 3,768 component
   calls, and 3,096 completed occurrence references.  Independently
   reconstruct every branch from target outputs and reject an altered
   terminal coefficient.
9. **Product convolution and parent assembly (landed).** Independently
   reconstruct every completed T1/S2 and T1-times-three-loop branch, retaining
   witness identity until final commutative collection, then zip both
   occurrence partitions and reproduce every grouped parent-row contribution.
10. **Mass homogeneity and raw-route replay (landed).** Independently verify
    ordinary and normalized forms for each slice, reconstruct every raw path
    directly, compare it with grouped assembly before canonical scaling, and
    reject altered coefficients or residual `m2`.
11. **Slice semantics (landed).** The three-loop-component sibling reports 823
   completed plans and 3,096 completed occurrences, with the 243 T1/S2 plans
   and 1,134 occurrences explicitly outside its slice.  Its row-incidence
   split is 969 completed, 511 outside, and 191 mixed. This remains a valid
   slice-local census even though `FourLoopNextClosedRows` now performs the
   separate complete integration.
12. **Resource failures.** Set every aggregate and per-request cap one below
    its exact request and require failure before guarded work.
13. **Four-loop row replay (landed).** Regenerate all 1,968 rows, substitute
    every occurrence, verify 22,424 canonical entries and maximum width 45,
    and reproduce the grouped-versus-raw route equality.
14. **Elimination replay (pending).** Reproduce exact rank/free metadata and
    replay all four-loop source-row combinations.

No acceptance test may execute FORM or Mathematica.

## 14. Recommended next action

Use the landed 1,968 canonical parent rows as the sole input to exact sparse
elimination. Preserve source-row weights and every exceptional factor in `d`,
then independently replay the eliminated rows. The shared caller-context
`D2/N1` service remains the production three-loop path; focused B4/F5/M6
services remain independent replay checks. The historical modular rank 1,762
belongs to the 2,644-column opaque-boundary probe and cannot be the rank of
the 1,734-column closed matrix; no closed-matrix rank, source-weight,
elimination, or master claim follows from assembly alone.
