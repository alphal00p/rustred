# Natural generated fixtures for affine residual starts

Date: 2026-08-13

Status: read-only audit completed against the current licensed, GMP-enabled
RustRed build.  No FORM or Mathematica process was executed.  The purpose of
this note is to keep synthetic algebra fixtures distinct from cases actually
discovered by the generic generated queue.

## Result

No currently probed natural generated queue leaf is a successful input to the
V1 single-equality `ResidualUnitAffineIndexMapCertificate`.

This is not a failure of the affine compositor.  It is a precise completeness
boundary of the current map language:

- an ordinary one-loop massive tadpole produces a literal-coordinate
  cylinder;
- a power-shifted tadpole produces one equality with a formal-base-dependent
  affine offset, which V1 deliberately does not claim; and
- the connected two-loop equal-mass sunset naturally produces two or three
  product-locus equality predicates, while V1 deliberately accepts exactly
  one uncompressed affine equality.

Consequently, the existing synthetic sunset case
`(d+1)*(n0+n1-3)=0` remains a valid, generated-row algebra oracle for
translation followed by Symbolica composition, but it must not be described
as a locus naturally emitted by the current queue.

## One-loop controls

### Ordinary massive tadpole

For the active sector `1`, discovery depth zero, and queue translation radius
zero, the generic pipeline returns:

- one canonical generated row;
- two global partition leaves;
- one queue work item, ordinal zero and case id one;
- the literal assignment `n0=1`;
- one recognized equality; and
- no unresolved predicate.

This is the natural positive control for the existing integer-cylinder start,
not a dependent affine start.

### Power-shifted tadpole

With the propagator power shifted by a formal base parameter, the same bounded
pipeline returns one unresolved `EqualZero` predicate and no literal
assignment.  Compiling that predicate as a V1 unit-affine index map returns the
typed unsupported outcome
`NotAssociateToSingleIntegerAffineRow`: its affine offset depends on a formal
base variable.  This is a useful negative/completeness regression.

## Connected two-loop sunset

The probed family used three equal-mass denominators and both signs of the
mixed loop-momentum cross term were checked.  At discovery depth zero and
queue radius zero the result is routing-sign independent:

| Sector | Generated rows | Global leaves | Queue items | Literal assignments | Unresolved equalities |
|---|---:|---:|---:|---:|---:|
| `011` | 4 | 3 | 1 | 0 | 2 |
| `101` | 4 | 3 | 1 | 0 | 2 |
| `110` | 4 | 4 | 1 | 0 | 3 |
| `111` | 4 | 4 | 1 | 0 | 3 |

Every attempted V1 map correctly returns
`UnconsumedEqualityPredicates`.  A depth-one probe increased rather than
removed this need: completed sectors retained between two and six equalities
per work item, and no successful V1 map was found.

Crucially, these predicates are not yet rows of one affine system.  Coverage
has compressed an exact disjunction of zero loci into a product polynomial.
For example, a predicate of the form `n0*(M-n2)*(1-n1)=0` means
`n0=0 OR n2=M OR n1=1`; treating the product as a linear row is invalid.
The observed depth-zero sunset factors are all coordinate-affine with unit
integer coefficients after nonzero base-field factors are removed, but that
fact is only a fixture observation, not a production assumption.

This establishes a two-stage generality requirement.  The production path
first needs bounded exact factor-branch decomposition of the conjunction of
product-locus equalities, with replayable Boolean provenance, contradiction
and orthant pruning, branch deduplication/subsumption, and honest unsupported
outcomes for unfactored components.  Only each surviving conjunction of
affine factors may then enter a simultaneous affine-system compiler with exact
rank, consistency, pivot, idempotence, replay, and resource proofs.
Repeatedly applying the current one-equality map is not an acceptable shortcut
because it would make substitution order part of the semantics and could
discard dependent or inconsistent rows.

## Test matrix

The next generated affine-start tests should therefore use:

1. the ordinary tadpole as a successful natural literal-cylinder control;
2. the power-shifted tadpole as a typed unsupported base-dependent-offset
   case;
3. sunset sectors `011` and `111` as natural typed product-locus/multiple-
   equality V1 rejections and future factor-branch fixtures; and
4. the existing synthetic unit-affine sunset locus only for exact
   compositor/ordering/schedule validation until the multi-equality map is
   implemented.

Concrete powers and loop counts appear only in these tests.  All production
APIs remain functions of an authenticated family, coefficient context, sector
case, equality system, and resource limits.

## Source anchors

- Natural queue construction and tadpole assertions:
  `tests/generated_sector_live_leaf_queue.rs`.
- Power-shifted dependent-start control:
  `tests/generated_cylindrical_row_system.rs`.
- Connected sunset construction and solve sectors:
  `tests/generated_family_rule_system.rs` and
  `tests/generated_two_loop_sector_discovery.rs`.
- V1 map contract:
  `src/residual_unit_affine_index_map.rs`.
- Existing synthetic sunset algebra oracle:
  `tests/affine_locus_bound_relation_sunset.rs`.

The probes were compiled against the current RustRed library with Symbolica's
default GMP feature set and run with the supplied license.  Temporary probe
executables were kept outside the workspace under `/tmp/rustred_*probe`; they
are not production artifacts or tests.
