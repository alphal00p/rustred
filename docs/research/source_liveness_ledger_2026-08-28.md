# RustRed Phase-0 source liveness ledger

**Baseline parent:** `47cf825`, immediately before the native-coefficient, dead-division,
and redundant-replay cleanup.
**Status:** R3 working authority, subordinate to `GOAL.md` and the clean-repository architecture plan.

This ledger classifies every one of the 48 tracked Rust source/test paths
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
| move | 16 |
| split | 31 |
| delete | 0 |
| replace | 1 |

| Current path | Decision | Final owner | Evidence/action |
|---|---|---|---|
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
| `src/algebra/coefficient.rs` | split | algebra | retain checked Symbolica primitives; prune duplicate or context-free surfaces |
| `src/algebra/matrix.rs` | split | algebra | retain checked Symbolica primitives; prune duplicate or context-free surfaces |
| `src/algebra/mod.rs` | move | algebra | narrow public scalar-algebra facade over private implementation children |
| `src/automatic_isps.rs` | move | family | live generic family construction and normalization |
| `src/campaign/admission.rs` | split | campaign | retain generic RAM admission; delete fixed-component and resident-transform prototype branches |
| `src/campaign/execution.rs` | move | campaign | bounded execution authority co-located with admission and width planning |
| `src/campaign/execution_width.rs` | move | campaign | live deterministic work/resource/admission infrastructure |
| `src/campaign/mod.rs` | move | campaign | live deterministic work/resource/admission infrastructure |
| `src/campaign/plan.rs` | split | campaign / rustred-app | move family/sector binding upward; retain only opaque generic work planning below |
| `src/campaign/resource_profile.rs` | move | campaign | live deterministic work/resource/admission infrastructure |
| `src/campaign/resources.rs` | move | campaign | live deterministic work/resource/admission infrastructure |
| `src/campaign/work.rs` | split | campaign / rustred-app | rewrite around opaque work identities and delete publication-era variants |
| `src/feynman_polynomials.rs` | move | family | retain Symbolica-native construction; delete the uncalled face-restriction API instead of moving dead sector coupling |
| `src/generic_family.rs` | split | family | retain the live generic family model while removing aliases and constructor self-replay |
| `src/guards.rs` | split | family / sector | separate stable family constraints from sector evidence |
| `src/lib.rs` | replace | crate facade | write from retained use cases; do not move exports |
| `src/parametric_coefficient.rs` | split | algebra | retain checked Symbolica base/index coefficient and sparse-polynomial authority; Phase 0 has no foundry owner |
| `src/parametric_ibp.rs` | split | identity | retain topology-neutral IBP/LI rows, stable provenance, one prepared source-batch type, and one completed semantic-scope token shared by ordinary and LI-only layouts; application owns execution policy |
| `src/parametric_relation.rs` | split | identity | retain topology-neutral IBP/LI rows and stable provenance |
| `src/sectors.rs` | split | family / sector | separate stable family constraints from sector evidence |
| `src/symbolica_affine_denominator.rs` | split | algebra | retain checked Symbolica primitives; prune duplicate or context-free surfaces |
| `src/symbolica_integral_input.rs` | split | input / rustred-app | retain typed normalization; move transport policy to app |
| `src/symmetry_discovery.rs` | split | sector | retain verified internal-permutation compilation/replay; delete bounded integer-matrix search and move future candidate generation to admitted foundry lanes |
| `src/symmetry.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
| `src/zero_sectors.rs` | split | sector | retain stable proofs/maps; move orchestration to foundry and prune ceremonies |
