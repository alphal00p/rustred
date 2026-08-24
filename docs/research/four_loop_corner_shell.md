# Four-loop native corner-shell certificate

`four_loop_corner_shell` implements the finite stage specified in section 10
of [the four/five-loop plan](four_five_loop_reduction_plan.md).  It is a
certificate for exactly ten frozen scalar seeds times sixteen native
`d/dk_i . k_j` identities: 144 H-reference rows and 16 X-reference rows.  It
does not claim an unrestricted four-loop reduction.

## Stable algebraic surface

`FourLoopCornerRawRowId` fixes `(corner type, differentiated loop,
contraction loop)` in generator order.  `FourLoopCornerColumnId` has only two
disjoint variants:

- a canonical `MasterProduct<MassiveVacuumMaster>`; or
- a genuine corner type plus exactly ten powers in that type's frozen H/X
  completed basis.

There is no zero column.  Persistent column keys reuse
`FourLoopHaloColumnKey::SCHEMA`, so they are independent of Rust enum
discriminants.  The deterministic easiest-to-hardest order is product stable
key, then genuine `(active lines, D+N, D, corner stable key, powers)`.  Pivot
search uses the reverse order.  Free columns are reported as
`free_unresolved_columns`; the API deliberately does not call them masters.

## Exact normalization

Every row comes from `IbpGenerator::try_generate_raw`.  Each direct term first
passes through the authenticated source `FourLoopHaloMapper`.  A transported
term is then dispatched by its positive physical corner:

1. proved scaleless terms are omitted;
2. supported factorized scalar corners become canonical product columns;
3. a genuine proper corner is mapped to its frozen type, same-mask branches
   become genuine columns, and only strictly lower-mask branches recurse; and
4. a factorized non-corner first becomes an exact `UnsupportedBoundaryHalo`
   record; the proved H-family scalar `D1/N0` subset is then closed through an
   authenticated 28-plan witness cache and six fixed component-dot formulae.

The unsupported record retains the H/X topology, full ten-power integral,
canonical product, factorization witness, and collected coefficient.  The
default shell closes all 234 observed records before canonicalization, so all
160 rows enter elimination. `preclosure_blocked_rows()` retains the 95-row
normalized provenance and `boundary_halo_closures()` records every mapped
component/line and mass-normalized product substitution. `blocked_rows()` is
reserved for records still outside the service and is empty by default.

After closure, exact elimination of all 160 rows has rank 159 and leaves 64
free columns in this finite column universe. These are deliberately reported
as unresolved shell coordinates rather than four-loop masters.

For every supported or blocked term the module applies

```text
c_bar = c * (m2)^(p_seed - w(column))
```

with `w(genuine)=sum(powers)` and product weight equal to the multiplicity
weighted physical-line counts.  It then inspects Symbolica numerator and
denominator degrees at the actual `m2` variable and rejects any residual mass
dependence.  Complete rows are canonically divided by their hardest
coefficient, with that exact row scale retained.

## Sparse certificate and limits

Forward sparse Gaussian elimination is exact over Symbolica rational
polynomials.  Every pivot stores source-row weights expressing
`pivot - rhs` as a combination of the retained normalized rows. Construction
and public `replay()` first rebuild every factorized halo substitution from its
exact witness, then substitute all rules into all 160 normalized inputs and
reconstruct every pivot from those source weights.

The configuration preflights the section-10 structural bounds before family
or coefficient construction:

| resource | bound |
|---|---:|
| raw rows | 160 |
| global columns | 736 |
| raw term incidences | 12,712 |
| normalization contributions | 139,832 |
| collected nonzeros | 117,760 |
| elimination updates | 18,723,840 |
| source-row weights | 25,600 |

Dynamic guards additionally cover the mapper cache, recursion depth,
coefficient degrees, actual normalization contributions, nonzeros,
elimination updates, and stored provenance weights.

## Current limitation

The result has status `Complete` for the fixed native shell, but this is not an
unrestricted four-loop reduction. The retained preclosure census is grouped by
`(reference topology, physical mask, dot degree, numerator degree, canonical
product)` and is independently reconstructed by the consolidated integration
test. Numerator-bearing factorized halos and a larger genuine-sector seed box
remain separate milestones.
