# RustRed production-generality audit

Status: read-only source audit. No FORM or Mathematica process was invoked.

## Conclusion

The current loop-named `src/` modules are legacy concrete finite
certificates/oracles, not LiteRed-style parametric rule discovery. The generic
core does not depend on them, but `lib.rs` still publishes them and two public
application paths still invoke specialized reducers. This is a production
scope violation, not merely a naming issue.

The only accepted production route is:

```text
IntegralFamily
  -> ParametricIbpGenerator
  -> generated guarded rule compiler/provider
  -> generic authenticated rule application
  -> tensor/Vakint adapter
```

Concrete loop families, powers, finite shells, and explicit formulae belong in
tests or a test-only oracle crate.

## P0 application paths to replace

- `src/tensor_family.rs` exposes specialized two-loop boundary/pipeline and
  three-loop pipeline methods.
- `crates/rustred-legacy-oracles/src/vakint_adapter.rs` owns and constructs a
  `TwoLoopReductionPipeline`.

These paths must instead accept an authenticated generic family and a sealed
generated parametric-rule provider.

Production hardcoded recurrences/formula dispatch currently occur in:

- `crates/rustred-legacy-oracles/src/one_loop.rs`;
- `crates/rustred-legacy-oracles/src/two_loop.rs` and `crates/rustred-legacy-oracles/src/two_loop_top_dot.rs`;
- `crates/rustred-legacy-oracles/src/three_loop_top_dot.rs`, `crates/rustred-legacy-oracles/src/three_loop_proper_dot.rs`, and
  `crates/rustred-legacy-oracles/src/three_loop_boundary.rs`;
- `crates/rustred-legacy-oracles/src/four_loop_boundary_halo.rs`, with transitive specialized services in
  `crates/rustred-legacy-oracles/src/four_loop_t1s2_closure.rs` and
  `crates/rustred-legacy-oracles/src/four_loop_three_loop_service.rs`; and
- `crates/rustred-legacy-oracles/src/five_loop_boundary.rs` and `crates/rustred-legacy-oracles/src/five_loop_d2.rs`.

Some legacy pipelines regenerate finite rows from concrete seeds, but that is
not a parametric recurrence derivation. Representative concrete-seed paths are
`two_loop_pipeline.rs`, `three_loop_pipeline.rs`, `three_loop_b4_d2.rs`,
`four_loop_corner_shell.rs`, `four_loop_next_manifest.rs`, and
`five_loop_d3.rs`.

## Migration classes

Extract topology-independent kernels, then move their fixed wrappers to
tests:

- `three_loop_boundary.rs`;
- `four_loop_boundary.rs`, `four_loop_genuine.rs`, `four_loop_halo.rs`,
  `four_loop_component_transport.rs`, and
  `four_loop_polynomial_halo.rs`; and
- `five_loop_boundary.rs`.

Move the remaining loop-named modules wholesale to test/oracle support. No
loop-named module currently qualifies as generic production infrastructure.
The structural family constructors in `three_loop.rs`, `four_loop.rs`, and
`five_loop.rs` are useful fixtures but remain fixtures.

Two source files identified here were orphaned/incomplete rather than compiled
capabilities: `four_loop_next_conditions.rs` contained no builder, while
`five_loop_d4.rs` referenced undefined pieces and ended in a partial builder.
The Phase 0 reachability audit confirmed that neither had ever entered a module
graph or executable test, and both were deleted rather than preserved as
oracles. They are not milestones.

## Prioritized migration

1. Complete the generated guarded rule provider and application seam.
2. Parameterize tensor/Vakint lowering over that generic provider and remove
   the specialized application methods.
3. Stop exporting loop-specific reducers/certificates from `lib.rs`; move
   them to a test-only oracle crate or `tests/support`.
4. Extract reusable routing, factorization, affine transport, and tensor
   kernels with dynamic loop count/arity and authenticated generic-family
   inputs.
5. Preserve finite results as independent goldens. Tests must derive
   parametric rules first, specialize only at the validation boundary, and
   compare against the oracle or Vakint without substituting master values.
6. Add an API-boundary test which fails if a production adapter imports a
   legacy recurrence module.

This migration is deliberately sequenced after the currently active
parametric derivation slice; legacy formulae remain non-authoritative even
before their physical relocation.
