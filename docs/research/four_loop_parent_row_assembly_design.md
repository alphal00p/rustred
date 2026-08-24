# Exact four-loop parent-row assembly design

Date: 2026-08-13

## Scope and status

This document records the landed production layer after the exact four-loop
next-shell inventory, component transport, and its two complementary
lower-loop closures. `FourLoopNextClosedRows` now combines those source
certificates into all 1,968 mass-normalized, canonically scaled parent rows
over `Q(d)`.

Parent-row assembly is complete. Exact next-shell elimination, source-row
weights, exceptional-factor recording, and generic rank remain **pending**.
The historical modular rank 1,762 belongs to a 2,644-column opaque-boundary
probe and cannot be the rank of this 1,734-column closed matrix. It is not an
acceptance constant or production claim for this layer. The frozen
deterministic assembly checksum is `fnv1a64:a55ce4ffda6f8f5c`.

The later, explicitly advisory `four_loop_next_modular_rank` layer now probes
this actual closed matrix at three denominator-screened prime/dimension
images. All three give rank 1,588, nullity 146, and the same hardest-first
`column@source-row` skeleton; its frozen report checksum is
`fnv1a64:2cca473b7966324a`. These remain finite-field discovery facts only.

## 1. Frozen structural input

The source is the fixed 123-seed, 1,968-origin manifest.  Its preclosure
inventory has this exact inventory-only census:

| resource | exact count |
|---|---:|
| compact paths | 26,078 |
| boundary / genuine leaves | 1,066 / 1,728 |
| boundary / genuine paths | 4,230 / 21,848 |
| scaleless paths in this fixed shell | 0 |
| raw row/boundary-leaf groups | 4,202 |
| nonzero / canceled boundary groups | 4,194 / 8 |
| contributors in nonzero groups | 4,214 |
| repeated groups | 28, all of size two |
| repeated surviving / canceled groups | 20 / 8 |
| canceled paths | 16 |
| genuine row/column groups | 20,111 |
| unique genuine columns | 1,728 |
| maximum paths in one row | 118 |
| maximum nonzero boundary groups in one row | 17 |

The two closures partition the same 1,066 transport plans and the same 4,230
occurrences:

| slice | plans | completed occurrences |
|---|---:|---:|
| T1/S2-only | 243 | 1,134 |
| T1 times B4/F5/M6 | 823 | 3,096 |

Each closure occurrence vector has length 4,230 because it records its
completed side and its outside side.  They must be zipped against transport
order, never concatenated.

## 2. Exact coefficient and mass semantics

For parent row `r`, boundary leaf `l`, and output product `P`, define

```text
s_r = sum(seed powers of row r)
w_l = sum(full dotted/numerator powers in boundary key l)
w(P) = sum_(master,multiplicity)
         multiplicity * master.physical_lines()
b_(r,l) = sum_(path p in the row/leaf group) final_coefficient(p)
O_(l,P) = ordinary lower-loop closure coefficient
N_(l,P) = O_(l,P) * m2^(w_l-w(P)).
```

The closures already retain and authenticate both `O` and `N`.  The normalized
parent contribution is

```text
[b_(r,l) * m2^(s_r-w_l)] * N_(l,P)
  = b_(r,l) * O_(l,P) * m2^(s_r-w(P)).              (2.1)
```

The primary assembler uses the left-hand side: it consumes one nonzero
`FourLoopNextCollectedBoundary`, mass-normalize its collected coefficient
from parent weight to boundary-key weight, then multiply the cached
`mass_normalized()` closure exactly once.

The right-hand side is the independent raw-path replay oracle: substitute
`ordinary()` separately for every raw boundary occurrence, normalize directly
from parent weight to product weight, collect, and compare with the primary
row.  It includes all 16 paths in the eight canceled groups and therefore
detects cancellation or grouping errors.

The following are correctness failures:

- multiplying a row-collected coefficient once per raw occurrence;
- multiplying a normalized closure and then applying `s_r-w(P)`, which
  double-normalizes it;
- using the scalar product's physical-line weight in place of the full
  dotted/numerator boundary weight `w_l`; or
- normalizing before all contributions to one typed column have been
  collected.

For a genuine leaf `G`, let

```text
g_(r,G) = sum_(paths p to G) final_coefficient(p)
w(G) = sum(genuine powers).
```

Its parent-row coefficient is

```text
g_(r,G) * m2^(s_r-w(G)).                              (2.2)
```

Genuine paths are collected by authenticated genuine column before this
normalization.  A scaleless path, if a future manifest contains one, remains
in replay provenance but adds no matrix entry.

Every final coefficient is inspected in Symbolica and has literal zero
`m2` degree in both numerator and denominator.  Dimensional reasoning alone
is not a cancellation proof.

## 3. Landed production records

The implementation lives in the separate `four_loop_next_closed_rows` module
and borrows the inventory, transport, and both closure certificates. Its
principal records are:

```text
FourLoopNextClosureSlice = T1S2 | ThreeLoop

FourLoopNextPlanBinding {
    leaf_id,
    transport_plan_index,
    slice,
    closure_plan_index,
}

FourLoopNextOccurrenceBinding {
    row_index,
    path_index,
    leaf_id,
    transport_plan_index,
    plan_binding_index,
    boundary_group_index,
}

FourLoopNextBoundaryGroup {
    row_index,
    leaf_id,
    contributor_path_indices,
    collected_coefficient,
    seed_mass_weight,
    boundary_mass_weight,
    mass_bridge_exponent,
    seed_to_boundary_coefficient,
    plan_binding_index,
}

FourLoopNextClosedRow {
    raw_id,
    seed_mass_weight,
    path_dispositions,
    boundary_group_indices,
    row_scale,
    sparse_entries,
}
```

Path dispositions remain compact coordinates and references, rather
than retaining 26,078 heap-allocated coefficients.  Exact coefficients are
regenerated by inventory replay.  Boundary groups retain their collected
coefficient and mass bridge because these are the actual production
substitution units.

Final entries reuse `FourLoopCornerColumnId`: the six allowed master
products and authenticated genuine representatives share its stable ordering
and mass-weight convention.  Before discarding a next-inventory genuine
leaf's topology and family fingerprint, assembly must prove that they are the
exact frozen reference family named by its corner type.

After full collection and mass normalization, a nonzero row is divided by the
coefficient of its hardest column under `FourLoopCornerColumnId` ordering.
That coefficient is retained as `row_scale`; the stored hardest coefficient
is one.  A zero row retains scale one.

The landed narrow status is
`ExactFixedSeedParentRowsGenericQdEliminationPending`. It asserts the exact
fixed-seed parent rows and deliberately does not imply a rank, a
master-minimal basis, or unrestricted four-loop reduction.

## 4. Authentication and proof obligations

Construction and replay establish all of the following:

1. the supplied transport borrows the supplied inventory;
2. both closures borrow that same transport object, not merely certificates
   with coincident checksums;
3. both closures use the exact same ordered Symbolica variable map, checked
   with `CoefficientContext::has_same_variable_map`;
4. the 243- and 823-plan leaf sets are disjoint and their union equals all
   1,066 transport plans;
5. both 4,230-entry occurrence partitions match transport row/path/leaf/plan
   order, and exactly one sibling has a completed-plan index at each position;
6. every compact inventory path has exactly one typed disposition;
7. each retained nonzero boundary group has precisely its advertised
   contributor path indices and replayed coefficient sum;
8. all eight omitted raw groups replay to exact zero;
9. every closure output lies in the six-key product whitelist
   `T1^4`, `T1^2*S2`, `S2^2`, `T1*B4`, `T1*F5`, `T1*M6`;
10. primary collected-group assembly equals independent raw-occurrence
    assembly before row-scale division;
11. equations (2.1) and (2.2) leave no literal `m2`; and
12. stored row scales and canonical sparse rows replay exactly in manifest
    order.

Object-identity checks are important.  Accepting separately built inventory,
transport, or closure objects solely because a schema or checksum agrees can
mix incompatible private family ownership and is unsound.

The checksum covers its own schema, every upstream schema/checksum,
coefficient parameter order, all plan and occurrence bindings, every group
and contributor coordinate, path dispositions, mass weights and exponents,
canonical coefficients, row scales, ordered rows, configuration, and complete
statistics.  A checksum is deterministic regression metadata; replay is the
certificate.

## 5. Structural prescan

A cheap prescan derives all counts in section 1 without rebuilding the
three-loop pipeline or performing coefficient arithmetic.  It traverses only
retained `rows().paths()`, `leaves()`, and `collected_boundaries()`:

1. count leaf and path variants;
2. group boundary paths by `(row_index, leaf_id)`;
3. compare those groups with the retained nonzero collected-boundary set;
4. count canceled, repeated, and surviving groups and contributors;
5. group genuine paths by authenticated typed column; and
6. compute maximum row paths and nonzero boundary groups.

This prescan runs before coefficient multiplication, reserves the exact
binding/group/disposition shapes, and rejects a structural mismatch early. It
does not establish coefficient cancellation; the later independent replay
does that.

## 6. Resource envelopes

Static conservative envelopes used to guard construction are:

| resource | bound |
|---|---:|
| rows | 1,968 |
| path dispositions | 26,078 |
| plan bindings | 1,066 |
| occurrence bindings | 4,230 |
| raw boundary groups | 4,202 |
| nonzero boundary groups | 4,194 |
| genuine row groups | 20,111 |
| primary grouped boundary-product contributions | `4,194*6 = 25,164` |
| raw-audit boundary-product contributions | `4,230*6 = 25,380` |
| primary contributions including genuine groups | 45,275 |
| raw-audit contributions including genuine paths | 47,228 |
| global final columns | 3,237 |
| retained collected row entries | `1,968*3,237 = 6,370,416` |
| maximum row width | 3,237 |

Configuration has independent caps for allocations, path/group replay,
mass-power steps, coefficient multiplications/additions/divisions, Symbolica
degree and dense operand/result universes, final entries, maximum row width,
and bounded coefficient term/serialization retention. No resource failure
may truncate a row or promote a remainder to a terminal.

The landed exact assembly/work census is:

| retained or performed quantity | exact count |
|---|---:|
| assembled / zero rows | 1,968 / 0 |
| paths, boundary / genuine | 26,078, split 4,230 / 21,848 |
| boundary plans | 1,066 |
| raw / nonzero / canceled boundary groups | 4,202 / 4,194 / 8 |
| genuine row groups | 20,111 |
| columns | 1,734 = 1,728 genuine + 6 products |
| primary grouped contributions | 28,096 |
| independent raw-route contributions | 30,353 |
| collected row entries | 22,424 |
| maximum row width | 45 |
| mass-power steps | 26,850 |
| coefficient multiplications / additions / divisions | 32,647 / 13,502 / 33,574 |
| retained coefficient terms / bytes | 71,270 / 107,123 |

Every row is collected completely before it is divided by its hardest-column
coefficient; all 1,968 rows are nonzero and therefore retain an authenticated
nontrivial or unit `row_scale`. All final coefficients have literal zero
`m2` degree. Construction compares the grouped production route with an
independent source-backed replay of every raw path before canonical scaling.
This implementation is pure Rust with Symbolica coefficient arithmetic and
does not execute FORM.

Elimination has separate future envelopes: up to 12,530,608,272 dense update
opportunities and 3,873,024 source-row weights.  Those costs and their own
guards do not belong to the assembly certificate.

## 7. Acceptance and negative tests

One serial integration test builds the expensive closure dependencies once
and verifies the complete census, complementary partitions, allowed columns,
mass cancellation, canonical rows, and deterministic checksum. Public replay
is invoked at most once; bounded candidate helpers avoid regenerating all
26,078 path coefficients for each negative case.

Required negative samples include:

- slice or closure-plan index swaps;
- occurrence row/path/leaf/plan corruption;
- deletion or duplication of a group contributor;
- a canceled group changed to nonzero;
- use of a collected coefficient once per occurrence;
- the double-normalization error described after (2.1);
- a wrong boundary or product mass weight;
- incompatible coefficient maps or different borrowed source objects;
- an unauthenticated genuine fingerprint;
- a seventh product key or residual `m2`;
- a changed row coefficient or row scale; and
- every resource cap set one below its exact request.

These rows now exist and replay. The next layer may use them to plan pivots,
but it must still perform exact elimination, retain source-row weights and
exceptional `d` factors, and establish an exact next-shell rank before making
any reduction or rank claim.
