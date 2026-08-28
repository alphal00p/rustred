# One- and two-loop vacuum validation catalog

## Interpretation of the "sunset closure"

The generated sunset work is a **test of the generic compiler**, not a
sunset-specific production reducer and not a graph-enumeration algorithm.
`src/generated_cylindrical_family_source_set.rs:1-14` explicitly describes a
raw-sector composition layer with no loop-count/topology recurrence, and its
input surface is an arbitrary `IntegralFamily` plus its authenticated
parametric context (`src/generated_cylindrical_family_source_set.rs:21-36`).
The equal-mass family is constructed only in
`tests/generated_cylindrical_sunset_family_oracle.rs:26-52`.

For this supplied three-denominator family, whose denominators form a complete
two-loop vacuum scalar-product basis,

```text
D1 = k1^2 - m1^2
D2 = k2^2 - m2^2
D3 = (k1+k2)^2 - m3^2,
```

the four non-scaleless raw sectors are:

| sector | graph interpretation |
|---|---|
| `111` | connected three-line sunset/theta graph |
| `110`, `101`, `011` | the three labelled two-line factorized double-tadpole pinches |

The current equal-mass acceptance test requires exactly this solve order at
`tests/generated_cylindrical_sunset_family_oracle.rs:100-106`. With a common
mass, the three pinches are one graph-isomorphism/S3 orbit; they are still all
compiled as raw sectors. Sectors with at most one positive index leave a free
scaleless loop. Vakint's checked-in oracle makes precisely the same zero and
sector-mapping split at
`vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:65-73`.

Thus this closure is complete for the nonzero sectors of that supplied IBP
family. It is not a universal enumeration of all field-theory graph
skeletons. Arbitrary interaction valence, repeated/linearly dependent
propagators, and arbitrary mass assignments are a graph/input concern, not a
production dispatch table. LiteRed itself takes a caller-provided complete
denominator basis (`vendor/LiteRed2/Source/LiteRed2026.m:688-693,763-803`) and
generates the IBPs from it (`vendor/LiteRed2/Source/LiteRed2026.m:1799-1823`).

## What Vakint actually represents at one and two loops

- One loop: one common-mass tadpole line, `I1L(msq(1),pow(1))`
  (`vendor/gammaloop/crates/vakint/src/topologies.rs:42-52`). The scalar
  AlphaLoop/MATAD comparison covers powers 1 through 6
  (`vendor/gammaloop/crates/vakint/tests/integral_alphaloop_vs_matad_tests.rs:24-48`).
- Two loops: one common-mass three-line sunset family and one chosen contraction
  `[3]` (`vendor/gammaloop/crates/vakint/src/topologies.rs:54-71`). The
  contraction is `I2L_pinch_3`, the factorized double tadpole
  (`vendor/gammaloop/crates/vakint/tests/input_matching_tests.rs:307-326`).
  The other two labelled pinches are equivalent only because the registered
  topology has one repeated mass wildcard on all three lines.
- Vakint's authored two-loop oracle is written parametrically across the
  positive/nonpositive integer-index regions through zero sectors, mappings,
  and guarded cases, ending on the `011` and `111` representatives
  (`vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:63-152`).
  This is reference evidence only; RustRed must derive its rules.
- Different symbolic masses are not part of Vakint's built-in AlphaLoop
  topology. Its `test_integrate_2l_different_masses` opts into `UNKNOWN` plus
  PySecDec, and then deliberately assigns all three symbols the same numerical
  value (`vendor/gammaloop/crates/vakint/tests/integral_evaluation_pysecdec_tests.rs:120-140`).
  It therefore is not an unequal-mass symbolic-reduction oracle.

The checked-in tensor fixtures are finite rather than exhaustive:

- one-loop rank-one/odd, rank-two, and rank-four structures at
  `vendor/gammaloop/crates/vakint/tests/tensor_reduction_tests.rs:7-74`;
- one two-loop mixed rank-four/rank-two/odd expression at powers `I2L(mUVsq,1,2,1)`
  in the same file at lines 77-109;
- a second two-loop rank-four numerical comparison at
  `vendor/gammaloop/crates/vakint/tests/integral_comparison_vs_pysecdec_tests.rs:234-264`;
- direct checks of the factorized pinch and another loop-momentum basis at
  `vendor/gammaloop/crates/vakint/tests/integral_comparison_vs_pysecdec_tests.rs:178-230`.

## Recommended finite two-loop validation matrix

All cases below must enter through the same generic `IntegralFamily` compiler;
no case may select a production recurrence by name, loop count, mass pattern,
or topology.

1. For the common-mass family, exhaust every labelled index triple in the
   finite cube `[-2,4]^3` (343 points), including all four nonzero sectors and
   all scaleless sectors. Compare every reducible result with a checked-in,
   frozen transcription of the Vakint oracle without substituting masters or
   executing FORM, and verify every generated proof by replay.
2. Repeat the family compilation for mass partitions `(m,m,m)`, `(m,m,M)`,
   and `(m1,m2,m3)`. Expected automorphism groups are S3, S2, and trivial.
   Vakint is an exact symbolic oracle only for the first case; for the latter
   two use direct IBP residual/replay checks and an independent numerical
   integral oracle where desired.
3. Repeat under all denominator permutations and representative unimodular
   loop-basis changes, including `k1+k2` versus `k1-k2`. This distinguishes a
   generic family compiler from a hidden routing-specific implementation.
4. Tensor acceptance through rank four should include all loop-label words:
   rank one `k_i^mu`, rank two `k_i^mu k_j^nu`, rank three (zero), and all
   sixteen rank-four assignments
   `k_i^mu k_j^nu k_k^rho k_l^sigma`, `i,j,k,l in {1,2}`. Exercise top-sector
   powers `111`, one-dot images such as `211`/`121`/`112`, and every labelled
   two-line pinch. The Vakint `121` fixture is one member, not the matrix.
5. Keep graph catalog assertions in tests. Production acceptance is instead:
   arbitrary family input -> generated IBP/LI rows -> sector inventory ->
   generated parametric rules -> concrete specialization and tensor lowering.

Massless degenerations should be a separate zero-sector suite, not silently
folded into the first massive acceptance matrix.

## Physical legacy-oracle package extraction

The earlier feature quarantine described below has now been replaced by a
physical package boundary. With default workspace members, the root `rustred`
crate exposes only the generic family/IBP/elimination/provider/tensor and
symmetry layers. All 35 compiled authored loop/topology modules and their
re-exports live in the publish-disabled `rustred-legacy-oracles` crate, along
with all 34 wholly legacy integration binaries and four diagnostic examples.
That crate depends one-way on `rustred` through the narrow hidden
`legacy-oracle-support` facade; the core has no reverse dependency.

Legacy/oracle groups:

- fixture constructors: `families`, `three_loop`, `four_loop`, `five_loop`;
- authored analytic/finite reducers: `one_loop`, `two_loop`,
  `two_loop_pipeline`, `two_loop_top_dot`, `three_loop_boundary`,
  `three_loop_pipeline`, `three_loop_top_dot`, `three_loop_proper_dot`,
  `three_loop_f5_d2n1`, `four_loop_boundary`, `four_loop_boundary_halo`,
  `four_loop_t1s2_closure`, `four_loop_three_loop_service`,
  `four_loop_three_loop_closure`, `five_loop_boundary`, `five_loop_d2`;
- frozen topology/seed/shell certificates: `three_loop_b4_d2`,
  `four_loop_genuine`, `four_loop_halo`, `four_loop_component_transport`,
  `four_loop_corner_shell`, every compiled `four_loop_next_*` module,
  `four_loop_polynomial_halo`, and `five_loop_d3`.

Some of these generate native rows and use exact generic elimination, but
their family, orbit, seed manifest, boundary formula, or terminal set is
frozen. That makes them valuable regression oracles, not generic RustRed
production.

Genuinely reusable pieces should stay default-public: `IntegralFamily` and
family algebra, parametric IBP/LI generation, symmetry/sector analysis,
generated row-span/elimination/coverage/provider layers, exact sparse
elimination, generic tensor projectors/lowering, and the generic
`MasterProduct<Id>` algebra. The older concrete `tensor_family` lowering now
lives with `VacuumFamily`, the concrete IBP generator, and the eager finite
reducer inside the publish-disabled oracle crate. The authenticated
`generic_tensor_family` lane remains default production. `product_boundary`
is loop-count-parametric, but it embeds an authored equal-mass factorization
formula; under the strict "derive, do not hardcode" rule it is quarantined as
an optional oracle/optimization until the generic proof path
derives/authenticates the same result.

### Applied extraction and remaining cleanup

1. Every module in the legacy groups and every corresponding re-export has
   moved to `crates/rustred-legacy-oracles`; the former
   `legacy-authored-oracles` core feature has been deleted without a
   compatibility alias.
2. The wholly legacy integration binaries (`one_loop`, `two_loop_boundary`,
   `two_loop_pipeline`, `two_loop_top_dot`, `sign_convention`, all
   `three_loop_*`, all authored/frozen `four_loop_*`, and `five_loop_*`) are
   owned by that crate. `vakint_adapter` and its test moved with them.
3. Generic test coverage remains default-enabled:
   `symmetry_discovery` already constructs local generic families, and
   `generic_tensor_family` owns authenticated lowering coverage. The concrete
   `tensor_family` fixture, `product_boundary`, and `two_loop_vacuum` exercise
   authored/older finite paths and are in the legacy suite, while the
   `certified_*`, `generated_*`, and `vakint_two_loop_tensor_ibp_oracle` tests
   remain default generic acceptance.
4. The package is a workspace member but not a default member, has
   `publish = false`, and depends directly only on `rustred` with default
   features disabled plus `legacy-oracle-support`. Reusable exact-matrix and
   coefficient-degree helpers cross through that deliberately narrow facade;
   Symbolica is not a direct dependency of the oracle crate.
5. Default and legacy-package checks remain separate. The next cleanup is to
   replace retained authored code with smaller fixtures or generic end-to-end
   coverage where practical. Never import frozen recurrences back into the
   generated compiler.
