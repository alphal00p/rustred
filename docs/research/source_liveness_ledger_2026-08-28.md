# RustRed Phase-0 source liveness ledger

**Baseline:** `4d843bd`, updated after each R3 cluster milestone.
**Status:** R3 working authority, subordinate to `GOAL.md` and the clean-repository architecture plan.

This ledger classifies every one of the 170 tracked Rust source/build/test paths
remaining after the current R3 deletion milestone. It is intentionally hostile to
accidental preservation: `move` retains a cohesive responsibility, `split`
retains only symbols proved live while deleting the rest, `delete` removes the
whole path after any named sentinel is in place, and `replace` writes a new
authority rather than relocating the file. A `split` decision is not
permission to carry the whole file forward. New Rust paths must be added before
their milestone commit, and an unclassified path blocks R3.

Regenerate this inventory after every R2-R4 milestone. Repeated long prefixes
must become a cohesive parent module with short role-named children or be
deleted. Retention also requires topology-neutral production semantics:
concrete topology names are fixture/artifact metadata only, while optimized
core lanes may dispatch solely on proved generic family properties.

| Decision | Paths |
|---|---:|
| move | 23 |
| split | 138 |
| delete | 8 |
| replace | 1 |

| Current path | Decision | Final owner | Evidence/action |
|---|---|---|---|
| `build.rs` | delete | — | replace vendored-manifest scraping with the resolved Symbolica public version API in R5 |
| `crates/rustred-app/src/application/campaign/plan.rs` | split | rustred-app::campaign | retain roots-only planning needed by the live app; remove obsolete schema surface freely |
| `crates/rustred-app/src/application/campaign/preflight.rs` | split | rustred-app::campaign | retain topology-free admission preflight needed by the live app |
| `crates/rustred-app/src/application/derive.rs` | split | rustred-app | retain evidenced generic derivation; narrow the output/API during facade reset |
| `crates/rustred-app/src/application/error.rs` | split | rustred-app | retain typed cross-frontend errors only |
| `crates/rustred-app/src/application/input.rs` | split | rustred-app / core input | keep transport decoding in app and move generic normalization downward |
| `crates/rustred-app/src/application/lowering.rs` | split | rustred-app / core input | keep composition only; core owns reusable lowering semantics |
| `crates/rustred-app/src/application/memory.rs` | split | rustred-app / runtime | retain bounded ingress/output policy actually used by live operations |
| `crates/rustred-app/src/application/model.rs` | split | rustred-app | freely replace obsolete request/output schemas |
| `crates/rustred-app/src/application/mod.rs` | split | rustred-app | rewrite as the narrow composition facade over live core domains |
| `crates/rustred-app/src/application/options.rs` | split | rustred-app | retain only live invocation policy; no compatibility aliases |
| `crates/rustred-app/src/application/producer.rs` | split | rustred-app | retain truthful producer metadata using resolved Symbolica version |
| `crates/rustred-app/src/cli/args.rs` | move | rustred-app::cli | cohesive live CLI argument boundary |
| `crates/rustred-app/src/cli/error.rs` | move | rustred-app::cli | cohesive CLI diagnostics boundary |
| `crates/rustred-app/src/cli/io.rs` | move | rustred-app::cli | bounded CLI I/O authority |
| `crates/rustred-app/src/cli/mod.rs` | move | rustred-app::cli | private CLI composition owner |
| `crates/rustred-app/src/lib.rs` | move | rustred-app | thin package facade, narrowed with application reset |
| `crates/rustred-app/src/main.rs` | move | rustred-app | thin `rustred` binary entry point |
| `crates/rustred-app/tests/application_api.rs` | split | rustred-app tests | retain only fresh byte/error parity for supported operations |
| `crates/rustred-app/tests/cli_campaign_plan.rs` | split | rustred-app tests | retain a compact roots-only planning contract |
| `crates/rustred-app/tests/cli_campaign_preflight.rs` | split | rustred-app tests | retain a compact topology-free resource contract |
| `crates/rustred-app/tests/cli_derive.rs` | split | rustred-app tests | fresh inline fixtures guard supported forms; prune historical breadth later |
| `crates/rustred-python/src/coordinator.rs` | move | rustred-python | single process/fork/panic coordination boundary |
| `crates/rustred-python/src/lib.rs` | move | rustred-python | thin private `_rustred` adapter behind public `import rustred` |
| `src/adaptive_rules.rs` | split | artifact / reduction | retain only closed proof-bearing artifact and application semantics |
| `src/algebra/coefficient.rs` | split | algebra | retain checked Symbolica primitives; prune duplicate or context-free surfaces |
| `src/algebra/exact.rs` | split | algebra | retain checked Symbolica primitives; prune duplicate or context-free surfaces |
| `src/algebra/matrix.rs` | split | algebra | retain checked Symbolica primitives; prune duplicate or context-free surfaces |
| `src/algebra/mod.rs` | move | algebra | narrow public scalar-algebra facade over private implementation children |
| `src/affine_parametric_ordering.rs` | split | sector / foundry::solver::exact | merge duplicate ordering/locus pipelines |
| `src/affine_prepare_point_schedule.rs` | split | sector / foundry::solver::exact | merge duplicate ordering/locus pipelines |
| `src/affine_prepare_points.rs` | split | sector / foundry::solver::exact | merge duplicate ordering/locus pipelines |
| `src/automatic_isps.rs` | move | family | live generic family construction and normalization |
| `src/base_specialization.rs` | move | family | live generic family construction and normalization |
| `src/campaign/admission.rs` | move | campaign | live deterministic work/resource/admission infrastructure |
| `src/campaign/execution.rs` | move | campaign | bounded execution authority co-located with admission and width planning |
| `src/campaign/execution_width.rs` | move | campaign | live deterministic work/resource/admission infrastructure |
| `src/campaign/mod.rs` | move | campaign | live deterministic work/resource/admission infrastructure |
| `src/campaign/plan.rs` | move | campaign | live deterministic work/resource/admission infrastructure |
| `src/campaign/resource_profile.rs` | move | campaign | live deterministic work/resource/admission infrastructure |
| `src/campaign/resources.rs` | move | campaign | live deterministic work/resource/admission infrastructure |
| `src/campaign/work.rs` | move | campaign | live deterministic work/resource/admission infrastructure |
| `src/canonical_parametric_locus_table.rs` | split | sector / foundry::solver::exact | merge duplicate ordering/locus pipelines |
| `src/certified_rewrite.rs` | split | artifact / reduction | retain only closed proof-bearing artifact and application semantics |
| `src/certified_rule_provider.rs` | split | artifact / reduction | retain only closed proof-bearing artifact and application semantics |
| `src/certified_symmetry_provider.rs` | split | artifact / reduction | retain only closed proof-bearing artifact and application semantics |
| `src/conditional_reelimination.rs` | split | foundry::solver | retain live exact/condition semantics; delete old whole-schedule variants |
| `src/conditional_rules.rs` | split | artifact / reduction | retain only closed proof-bearing artifact and application semantics |
| `src/coordinate_equality_loci.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/coverage_decision_dag.rs` | split | artifact / reduction | retain only closed proof-bearing artifact and application semantics |
| `src/cylindrical_ordering.rs` | split | sector / foundry::solver::exact | merge duplicate ordering/locus pipelines |
| `src/cylindrical_prepare_point_schedule.rs` | split | sector / foundry::solver::exact | merge duplicate ordering/locus pipelines |
| `src/cylindrical_prepare_points.rs` | split | sector / foundry::solver::exact | merge duplicate ordering/locus pipelines |
| `src/direct_bad_formula_arbitrary.rs` | split | foundry::solver | retain live exact/condition semantics; delete old whole-schedule variants |
| `src/direct_bad_formula.rs` | split | foundry::solver | retain live exact/condition semantics; delete old whole-schedule variants |
| `src/exact_identity.rs` | split | algebra / identity | separate coefficient context from stable identity values |
| `src/exact_sparse_elimination.rs` | split | algebra / foundry::solver::exact | retain only if certified-rewrite caller survives Symbolica authority audit |
| `src/family_sector_inventory.rs` | split | sector / foundry::solver | stable sector evidence below bounded inventory orchestration |
| `src/feynman_polynomials.rs` | move | family | live generic family construction and normalization |
| `src/generated_affine_initial_global_affine_terminal.rs` | split | foundry::solver::{exact,closure} | retain live row/refinement semantics; delete superseded generations |
| `src/generated_affine_parametric_ordering.rs` | split | foundry::solver::{exact,closure} | retain live row/refinement semantics; delete superseded generations |
| `src/generated_affine_prepare_point_schedule.rs` | split | foundry::solver::{exact,closure} | retain live row/refinement semantics; delete superseded generations |
| `src/generated_affine_residual_boolean_cover.rs` | split | foundry::solver::{exact,closure} | retain live row/refinement semantics; delete superseded generations |
| `src/generated_affine_residual_case_bound_relation.rs` | split | foundry::solver::{exact,closure} | retain live row/refinement semantics; delete superseded generations |
| `src/generated_affine_residual_case_bound_unit_equality_refinement.rs` | split | foundry::solver::{exact,closure} | retain live row/refinement semantics; delete superseded generations |
| `src/generated_affine_residual_case_completed_bound_row.rs` | split | foundry::solver::{exact,closure} | retain live row/refinement semantics; delete superseded generations |
| `src/generated_affine_residual_case_mapped_nonzero.rs` | split | foundry::solver::{exact,closure} | retain live row/refinement semantics; delete superseded generations |
| `src/generated_affine_residual_case_pivot_target_matching.rs` | split | foundry::solver::{exact,closure} | retain live row/refinement semantics; delete superseded generations |
| `src/generated_affine_residual_case_premises.rs` | split | foundry::solver::{exact,closure} | retain live row/refinement semantics; delete superseded generations |
| `src/generated_affine_residual_case_reelimination.rs` | split | foundry::solver::{exact,closure} | retain live row/refinement semantics; delete superseded generations |
| `src/generated_affine_residual_case_unit_equality_refinement.rs` | split | foundry::solver::{exact,closure} | retain live row/refinement semantics; delete superseded generations |
| `src/generated_affine_residual_source_authority.rs` | split | foundry::solver::{exact,closure} | retain live row/refinement semantics; delete superseded generations |
| `src/generated_cylindrical_candidate_authority.rs` | split | foundry::solver::closure | retain live exceptional-start semantics; delete duplicate facades |
| `src/generated_cylindrical_family_source_set.rs` | split | foundry::solver::closure | retain live exceptional-start semantics; delete duplicate facades |
| `src/generated_cylindrical_persistent_elimination.rs` | split | foundry::solver::closure | retain live exceptional-start semantics; delete duplicate facades |
| `src/generated_cylindrical_residual_start.rs` | split | foundry::solver::closure | retain live exceptional-start semantics; delete duplicate facades |
| `src/generated_cylindrical_row_system.rs` | split | foundry::solver::closure | retain live exceptional-start semantics; delete duplicate facades |
| `src/generated_cylindrical_sector_coverage.rs` | split | foundry::solver::closure | retain live exceptional-start semantics; delete duplicate facades |
| `src/generated_cylindrical_sector_provider.rs` | split | foundry::solver::closure | retain live exceptional-start semantics; delete duplicate facades |
| `src/generated_cylindrical_sector_root_start.rs` | split | foundry::solver::closure | retain live exceptional-start semantics; delete duplicate facades |
| `src/generated_cylindrical_when_bad.rs` | split | foundry::solver::closure | retain live exceptional-start semantics; delete duplicate facades |
| `src/generated_family_depth_growth.rs` | split | foundry / artifact | replace public fixed-point/provider stack with narrow emission boundary |
| `src/generated_family_fixed_point_provider.rs` | split | foundry / artifact | replace public fixed-point/provider stack with narrow emission boundary |
| `src/generated_family_fixed_point.rs` | split | foundry / artifact | replace public fixed-point/provider stack with narrow emission boundary |
| `src/generated_family_rule_provider.rs` | split | foundry / artifact | replace public fixed-point/provider stack with narrow emission boundary |
| `src/generated_family_rule_system.rs` | split | foundry / artifact | replace public fixed-point/provider stack with narrow emission boundary |
| `src/generated_provider_stack.rs` | split | foundry / artifact | replace public fixed-point/provider stack with narrow emission boundary |
| `src/generated_residual_affine_condition_accumulator.rs` | split | foundry::solver::closure | retain live exceptional closure semantics; delete chronology/provider shells |
| `src/generated_residual_affine_when_bad.rs` | split | foundry::solver::closure | retain live exceptional closure semantics; delete chronology/provider shells |
| `src/generated_sector_conditional_provider.rs` | split | foundry::solver::closure | retain live discovery/queue semantics; delete provider duplication |
| `src/generated_sector_discovery.rs` | split | foundry::solver::closure | retain live discovery/queue semantics; delete provider duplication |
| `src/generated_sector_live_leaf_queue.rs` | split | foundry::solver::closure | retain live discovery/queue semantics; delete provider duplication |
| `src/generated_symbolic_row_span.rs` | split | identity / foundry::solver | topology-neutral row span below high orchestration |
| `src/generated_when_bad.rs` | split | foundry / artifact | replace public fixed-point/provider stack with narrow emission boundary |
| `src/generic_family.rs` | move | family | live generic family construction and normalization |
| `src/generic_tensor_family.rs` | split | tensor / reduction | retain low tensor semantics; high composition belongs in reduction |
| `src/generic_tensor_polynomial.rs` | split | tensor / reduction | retain low tensor semantics; high composition belongs in reduction |
| `src/generic_tensor_projector.rs` | split | tensor / reduction | retain low tensor semantics; high composition belongs in reduction |
| `src/guards.rs` | split | family / sector | separate stable family constraints from sector evidence |
| `src/lib.rs` | replace | crate facade | write from retained use cases; do not move exports |
| `src/master_policy.rs` | split | artifact / reduction | retain only closed proof-bearing artifact and application semantics |
| `src/master_product.rs` | split | artifact / reduction | retain only closed proof-bearing artifact and application semantics |
| `src/parametric_coefficient.rs` | split | algebra / foundry::solver::exact | retain checked Symbolica coefficient/sparse authority only |
| `src/parametric_coefficient/symbolica_sparse/persistent.rs` | split | algebra / foundry::solver::exact | retain checked Symbolica coefficient/sparse authority only |
| `src/parametric_coefficient/symbolica_sparse.rs` | split | algebra / foundry::solver::exact | retain checked Symbolica coefficient/sparse authority only |
| `src/parametric_elimination.rs` | split | foundry::solver | retain live exact/condition semantics; delete old whole-schedule variants |
| `src/parametric_ibp.rs` | split | identity | retain topology-neutral IBP/LI rows and stable provenance |
| `src/parametric_relation.rs` | split | identity | retain topology-neutral IBP/LI rows and stable provenance |
| `src/parametric_rules.rs` | split | artifact / reduction | retain only closed proof-bearing artifact and application semantics |
| `src/parametric_sector_coverage.rs` | split | artifact / reduction | retain only closed proof-bearing artifact and application semantics |
| `src/parametric_sector_formula_affine_terminal.rs` | delete | — | eager/legacy sector formula and synthetic test stack |
| `src/parametric_sector_formula_ir.rs` | delete | — | eager/legacy sector formula and synthetic test stack |
| `src/parametric_sector_formula_residual.rs` | delete | — | eager/legacy sector formula and synthetic test stack |
| `src/parametric_sector_k21_test_support.rs` | delete | — | eager/legacy sector formula and synthetic test stack |
| `src/parametric_sector_mtbdd_certificate.rs` | delete | — | eager/legacy sector formula and synthetic test stack |
| `src/parametric_sector_mtbdd.rs` | delete | — | eager/legacy sector formula and synthetic test stack |
| `src/parametric_sector_normalized_source.rs` | split | identity / foundry::solver | raw identity values below sector/coverage orchestration |
| `src/parametric_sector_one_pass_tests.rs` | delete | — | eager/legacy sector formula and synthetic test stack |
| `src/parametric_sector_provider.rs` | split | artifact / reduction | retain only closed proof-bearing artifact and application semantics |
| `src/product_locus_boolean_cover.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/reduction_engine.rs` | split | reduction | extract generic application services; delete compatibility engines |
| `src/residual_affine_atom_rows.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/residual_affine_branch_guard_composition.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/residual_affine_branch_system.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/residual_affine_integer_lattice_kernel.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/residual_affine_integer_system.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/residual_unit_affine_index_map.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/runtime/mod.rs` | move | runtime | single Symbolica initialization/version authority |
| `src/sectors.rs` | split | family / sector | separate stable family constraints from sector evidence |
| `src/shift_operators.rs` | move | family | live generic family construction and normalization |
| `src/solver/closure/case_inventory.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/committed_exceptional_reentry/capability.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/committed_exceptional_reentry/fresh_session.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/committed_exceptional_reentry/mod.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/committed_exceptional_source/epoch_adapter.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/committed_exceptional_source/mod.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/committed_exceptional_source/protocol.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/mod.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/post_ready/analysis.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/post_ready/condition_plan.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/post_ready/materialization.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/post_ready/mod.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/post_ready/partition.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/post_ready/publication.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/publication_handoff/mod.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/publication_handoff/publication_epoch_owner/mod.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/closure/publication_handoff/publication_epoch_owner/result_batch.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/exact_session/database.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/exact_session/mod.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/exact_session/physical_key.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/exact_session/physical_row.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/exact_session/plan.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/exact_session/recenter.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/exact_session/session.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/exact_session/targets.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/exact_session/telemetry.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/solver/mod.rs` | split | foundry::solver | retain live exact-session/closure spine; co-locate or delete historical tests |
| `src/symbolica_affine_denominator.rs` | split | algebra | retain checked Symbolica primitives; prune duplicate or context-free surfaces |
| `src/symbolica_integral_input.rs` | split | input / rustred-app | retain typed normalization; move transport policy to app |
| `src/symbolica_target_numerator.rs` | split | tensor / reduction | retain low tensor semantics; high composition belongs in reduction |
| `src/symbolica_tensor_numerator.rs` | split | tensor / reduction | retain low tensor semantics; high composition belongs in reduction |
| `src/symbolic_sector_cases.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/symbolic_symmetry_transport.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/symmetry_discovery.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/symmetry.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/tensor_reduction_engine.rs` | split | reduction | extract generic application services; delete compatibility engines |
| `src/tensor.rs` | split | tensor / reduction | retain low tensor semantics; high composition belongs in reduction |
| `src/when_bad.rs` | split | foundry::solver | retain live exact/condition semantics; delete old whole-schedule variants |
| `src/zero_sector_provider.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/zero_sectors.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
