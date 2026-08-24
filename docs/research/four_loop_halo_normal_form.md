# Four-loop genuine-corner halo normal form

## Scope

RustRed's 214 connected rank-four scalar corners collapse, by replayable signed
`GL(4,Z)` maps, to ten frozen H/X routing types.  The next IBP layer consists of
the 16 raw identities `d/dk_i . k_j` at each frozen corner, hence 160 rows.

The checked-in `four_loop_halo` module implements the exact transport needed to
put those rows into a shared basis.  It does **not** yet solve the rows or claim
that the ten scalar corners are a minimal master basis.

## Affine convention

The genuine-corner witness stores a loop map `U` satisfying

```text
q_source U = +/- q_reference,
k_source = U k_reference.
```

For every one of the ten source denominator-basis entries, including generated
ISPs, the halo mapper computes

```text
D_source[i] = c[i] + sum_j A[i,j] D_reference[j].
```

The flattened quadratic representation stores the coefficient multiplying
`k_a.k_b` directly.  Its off-diagonal entries therefore already contain the
usual factor of two for a squared routing.  The implementation transforms each
scalar-product basis monomial explicitly and then uses the frozen reference
family's exact inverse denominator map.  Replay independently checks both the
quadratic row and affine shift.  Every witnessed active propagator must reduce
to exactly one reference propagator with coefficient one and zero constant.

Construction is preflighted before coefficient work.  It retains exactly ten
images and charges a conservative 4,000 exact operations.  Expanding a
degree-one numerator emits at most eleven reference integrals.

## Raw halo coverage

A raw identity at a scalar corner can only emit:

- a scalar corner term;
- one total dot on a witnessed active physical line;
- one pinch paired with that dot; or
- one dot and one degree-one numerator on an inactive physical/ISP entry.

`FourLoopHaloMapper::map_raw_halo_integral` accepts this direct `(D,N) <=
(1,1)` shape and also checks the pinch adjacency which `(D,N)` does not record:
the corner may lose at most one active line, and only in a dotted term; a
numerator likewise requires a dot and cannot accompany a pinch. Positive
powers are transported by the authenticated active-line bijection. A
polynomial numerator is expanded through its complete affine image. Tests
construct all ten frozen representatives, replay all 100 basis images, and
transport every integral appearing in the 160 raw corner rows. Independent
quadratic evaluations at ten determining momentum assignments check the
flattened cross-term convention, and nonidentity BMW-to-H and FG-to-H examples
check the direction of genuine interfamily maps and their distinct ISP bases.

## Stable columns and remaining work

`FourLoopHaloColumnKey` reserves disjoint versioned namespaces for exact zero,
canonical lower-loop master products, and genuine integrals expressed in their
frozen representative family.  The next stage must actually normalize the
transported terms into those keys:

1. prove scaleless halo sectors using the family criterion;
2. apply native scalar/tensor closure to factorized dotted/numerator sectors;
3. remap every proper genuine sector to its own frozen representative;
4. assemble the 160 sparse rows, eliminate them exactly, and replay each raw
   identity against the resulting table.

Until all four steps are complete, the affine maps are a raw-corner quotient
certificate and normal-form foundation, not a four-loop IBP reduction.
