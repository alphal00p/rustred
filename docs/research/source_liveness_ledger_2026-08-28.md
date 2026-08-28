# RustRed Phase-0 source liveness ledger

**Baseline parent:** `d7c6bbe`, immediately before pruning the relation and
parametric-condition API to the live generation/application spine.
**Status:** R3 working authority, subordinate to `GOAL.md` and the clean-repository architecture plan.

This ledger classifies every one of the 86 tracked Rust source/test paths
remaining after the current ownership milestone. It is intentionally hostile to
accidental preservation: `move` retains a cohesive responsibility, `split`
retains only symbols proved live while deleting the rest, `delete` removes the
whole path after any named sentinel is in place, and `replace` writes a new
authority rather than relocating the file. A `split` decision is not
permission to carry the whole file forward. New Rust paths must be added before
their milestone commit, and an unclassified path blocks R3.

Immediate application reachability is not the sole retention criterion. The
generic ISP, Symanzik, symmetry, zero-sector, and indexed-specialization
authorities are strategic capabilities required by `GOAL.md`; their `split`
entries retain only the topology-neutral kernel assigned to the named final
owner, even where the current thin application does not invoke it yet.

Regenerate this inventory after every R2-R4 milestone. Repeated long prefixes
must become a cohesive parent module with short role-named children or be
deleted. Retention also requires topology-neutral production semantics:
concrete topology names are fixture/artifact metadata only, while optimized
core lanes may dispatch solely on proved generic family properties.

| Decision | Paths |
|---|---:|
| move | 27 |
| split | 58 |
| delete | 0 |
| replace | 1 |

| Current path | Decision | Final owner | Evidence/action |
|---|---|---|---|
| `crates/rustred-app/src/application/campaign/plan.rs` | split | rustred-app::campaign | app-owned roots-only family/sector/job interning with deterministic ordinals and bounded truthful output; no core replay/dependency plan or fictional future phases |
| `crates/rustred-app/src/application/campaign/preflight.rs` | split | rustred-app::campaign | retain topology-free resource preflight needed by the live app |
| `crates/rustred-app/src/application/derive/mod.rs` | split | rustred-app::derive | compose input lowering, admitted parallel IBP/LI batches, DTO conversion, and serialization without owning their representations |
| `crates/rustred-app/src/application/derive/model.rs` | split | rustred-app::derive | retain only the deterministic application output DTO and semantic-to-transport conversion |
| `crates/rustred-app/src/application/derive/census.rs` | split | rustred-app::derive | retain structural, payload, and render-bound preflights separately from execution and output representation |
| `crates/rustred-app/src/application/error.rs` | split | rustred-app | retain typed cross-frontend errors only |
| `crates/rustred-app/src/application/input.rs` | split | rustred-app / core input | keep transport decoding in app and move generic normalization downward |
| `crates/rustred-app/src/application/lowering.rs` | split | rustred-app / core input | keep composition only; core owns reusable lowering semantics |
| `crates/rustred-app/src/application/memory.rs` | split | rustred-app | retain bounded ingress/output policy actually used by live operations; it is not a core runtime owner |
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
| `src/algebra/base.rs` | split | algebra | retain the checked Symbolica base-field/context boundary and the sole raw `CoefficientPolynomial` alias; the indexed and affine-input domains now reuse this owner |
| `src/algebra/indexed/mod.rs` | move | algebra::indexed | narrow authenticated `K(n)` facade; the former flat parametric-coefficient module and compatibility names are deleted |
| `src/algebra/indexed/context.rs` | split | algebra::indexed | retain authenticated base/index map construction and checked exact field arithmetic; caller-sized Rust-owned names, fingerprints, counts, and vector allocations are preflighted before Symbolica registration, whose public parser/interner/template allocation boundary remains explicit; redundant default conveniences and separately stored base fingerprint are deleted |
| `src/algebra/indexed/error.rs` | move | algebra::indexed | one topology- and provenance-neutral indexed-algebra error taxonomy, including distinct count-overflow, allocation-failure, and generated-symbol registration boundaries without caller-sized diagnostic copies |
| `src/algebra/indexed/limits.rs` | split | algebra::indexed | retain prospective native-operation envelopes and checked resource arithmetic; integer-magnitude arithmetic uses sufficient `u64` rather than a fictitious `u128` exponent domain |
| `src/algebra/indexed/scope.rs` | split | algebra::indexed | private lossless Symbolica namespace and context-identity construction, with exact checked byte censuses, fallible reservations, aggregate generated-name preflight, and allocation-free decimal emission separated to keep value/context dependencies acyclic |
| `src/algebra/indexed/specialization.rs` | split | algebra::indexed | retain the standalone checked projection from `K(n)` to `K`, publicly returning the normalized value plus its mapped pre-normalization denominator so a future real consumer must retain the guard; bit-growth arithmetic is checked `u64`, while relation-specific wrappers are deleted |
| `src/algebra/indexed/translation.rs` | split | algebra::indexed | retain checked affine index translation with `u64` bit-growth bounds until execution is delegated to the public Symbolica polynomial substitution API |
| `src/algebra/indexed/value.rs` | split | algebra::indexed | retain only authenticated indexed coefficient/polynomial values with shared no-copy context identity and live queries; the redundant authenticated base-polynomial wrapper and caller-free convenience queries are deleted |
| `src/algebra/indexed/tests/mod.rs` | split | algebra::indexed tests | private test composition only |
| `src/algebra/indexed/tests/context.rs` | split | algebra::indexed tests | retain map identity, fallible construction/error-ordering, checked name/fingerprint lengths, and field-operation sentinels |
| `src/algebra/indexed/tests/specialization.rs` | split | algebra::indexed tests | retain specialization, normalization, exact/one-below integer/operation bounds through `u16::MAX`, GMP, and resource-order sentinels through native substitution migration |
| `src/algebra/indexed/tests/translation.rs` | split | algebra::indexed tests | retain translation composition, exact/one-below term/operation/integer bounds including `i64::MIN`, overflow, GMP, and resource-order sentinels through native substitution migration |
| `src/algebra/matrix/mod.rs` | move | algebra::matrix | private facade exposing only checked matrix operations and their admitted metadata to core callers |
| `src/algebra/matrix/admission.rs` | split | algebra::matrix | retain shapes, operation envelopes, payload census, and authenticated conversion; keep native scratch limitations explicit |
| `src/algebra/matrix/error.rs` | move | algebra::matrix | private typed matrix/native failure vocabulary |
| `src/algebra/matrix/field.rs` | split | algebra::matrix | retain checked field traits, bounded coefficient powers, and typed unwind transport required by Symbolica's infallible ring interfaces |
| `src/algebra/matrix/operations.rs` | split | algebra::matrix | retain the narrow native rank, determinant, inverse, product, and congruence entry points with authenticated outputs |
| `src/algebra/matrix/tests.rs` | split | algebra::matrix tests | retain focused Symbolica authority, admission-boundary, and typed-failure sentinels; reduce breadth only with equivalent black-box evidence |
| `src/algebra/mod.rs` | move | algebra | narrow public scalar-algebra facade over private implementation children |
| `src/campaign/execution.rs` | move | campaign | bounded ordered execution authority; the dead move-owned reservation mapper was deleted with admission |
| `src/campaign/execution_width.rs` | move | campaign | live deterministic resource-preflight and width-planning infrastructure; it no longer constructs execution through admission |
| `src/campaign/mod.rs` | move | campaign | narrow facade over resource preflight, width planning, and bounded execution; roots-only planning is application-owned |
| `src/campaign/resource_profile.rs` | move | campaign | calibrated execution resource profile feeding live width preflight |
| `src/campaign/resources.rs` | move | campaign | retain checked bytes, estimates, task envelopes, estimator revisions, baselines, and their minimal construction errors; work-key wave policy/planning was deleted |
| `src/family/mod.rs` | move | family | narrow family facade over authenticated model, construction, kinematics, exact adaptation, fingerprint, and replay owners |
| `src/family/model.rs` | split | family | retain family value types and sealed authenticated state; the caller-free `GenericFamily` alias is deleted |
| `src/family/error.rs` | move | family | one family-owned error taxonomy, renamed `IntegralFamilyError`, plus shared resource-limit admission |
| `src/family/exact.rs` | split | family | thin family-semantic adaptation of checked Symbolica matrix operations and exact coefficient comparison; prevents build/replay ownership cycles |
| `src/family/build.rs` | split | family | authenticated construction, labels, condition merging, and composition of the exact/fingerprint/kinematics services |
| `src/family/kinematics.rs` | split | family | scalar-product coordinates, affine expansions, derivative contractions, and their bounded construction |
| `src/family/fingerprint.rs` | split | family | typed V2 family-identity preflight, census, encoding, and writer |
| `src/family/integral.rs` | move | family | exact integral-power key and its independent raw construction errors; the self-only assignment-plus-shift constructor and its orphan error variants are deleted, while symmetry transports the family-owned value |
| `src/family/replay.rs` | split | family | exact determinant, inverse, scalar-coordinate, and derivative-contraction replay |
| `src/family/tests.rs` | split | family tests | retain the focused family construction, kinematics, fingerprint, matrix-boundary, and replay sentinels; subdivide only when it materially aids the next algorithm change |
| `src/family/isp/mod.rs` | move | family::isp | narrow ISP-completion facade; no former flat-module alias |
| `src/family/isp/model.rs` | split | family::isp | V2 schema, independent rank/family resource policy, and native-work census; V1 semantics are deleted |
| `src/family/isp/error.rs` | move | family::isp | typed independent-basis completion failures |
| `src/family/isp/rank.rs` | split | family::isp | checked Symbolica rank boundary, input authentication, work accounting, and error adaptation |
| `src/family/isp/completion.rs` | split | family::isp | topology-neutral deterministic unit-row completion and its concise retained witness; caller-free self-replay ceremony is deleted |
| `src/family/isp/tests.rs` | split | family::isp tests | retain authored-prefix, coordinate-order, native-rank, GMP, and exact resource-bound sentinels |
| `src/family/symanzik/mod.rs` | move | family::symanzik | narrow generic Symanzik facade over model, context, construction, operations, and work owners |
| `src/family/symanzik/model.rs` | split | family::symanzik | authenticated Feynman-polynomial values and representation policy |
| `src/family/symanzik/error.rs` | move | family::symanzik | typed checked polynomial/construction failures |
| `src/family/symanzik/context.rs` | split | family::symanzik | authenticated family coefficient/parameter context and checked public polynomial operations |
| `src/family/symanzik/construction.rs` | split | family::symanzik | topology-neutral `U`, `F`, and `G` assembly |
| `src/family/symanzik/operations.rs` | split | family::symanzik | determinant, adjugate, homogeneity, and currently handwritten polynomial kernels pending the separate Symbolica-native commit |
| `src/family/symanzik/work.rs` | split | family::symanzik | shared checked resource arithmetic and aggregate operation budgets; prevents context/operations ownership cycles |
| `src/family/symanzik/tests.rs` | split | family::symanzik tests | retain determinant/adjugate orientation, symbolic-term, variable-map rebinding, and exact resource-bound sentinels |
| `src/identity/mod.rs` | move | identity | narrow identity facade over row identity and exceptional-domain conditions |
| `src/identity/row.rs` | move | identity | one real stable row identifier shared by generated, translated, and specialized identities; no adapter row mirror |
| `src/identity/condition.rs` | split | identity | deterministic parametric identity-condition source sets, independent source-cardinality limits, checked affine translation, and merge semantics; construction is crate-owned, the explicit/test convenience source is deleted, and specialized base-field ceremony waits for a real reduction consumer |
| `src/lib.rs` | replace | crate facade | write from retained use cases and remove self-only concrete-relation/condition/`IndexSpace` exports; do not move exports wholesale |
| `src/parametric_ibp.rs` | split | identity | retain topology-neutral IBP/LI rows, stable provenance, one prepared source-batch type, shared fallibly constructed zero/unit shift storage with refcount-only clones, and one completed semantic-scope token shared by ordinary and LI-only layouts; compact coefficient checks call the standalone indexed specialization primitive |
| `src/parametric_relation.rs` | split | identity | retain topology-neutral sparse parametric relation arithmetic with one typed parametric-condition vector and independent `RelationLimits`; public consumers receive only row/term/condition views, construction and limit-explicit mutation are crate-owned, shift vectors use checked Rust-owned allocation followed by shared no-copy storage, and default/explicit-condition conveniences are deleted |
| `src/sectors.rs` | split | sector | separate masks, restrictions, ordering, and sector errors without importing higher layers |
| `src/symbolica_affine_denominator.rs` | split | input::affine / algebra | keep parsing and family-coordinate lowering under input; reuse algebra's sole raw polynomial alias, extract only genuinely family-neutral checked coefficient primitives into algebra, and audit handwritten exponent projection against Symbolica |
| `src/symbolica_integral_input.rs` | split | input / rustred-app | retain typed normalization; move transport policy to app |
| `src/symmetry_discovery.rs` | split | sector | retain verified internal-permutation compilation/replay; delete bounded integer-matrix search and move future candidate generation to admitted foundry lanes |
| `src/symmetry.rs` | split | sector | retain stable proofs/maps with symmetry-owned source/target condition evidence; move orchestration to foundry and prune ceremonies |
| `src/zero_sectors.rs` | split | sector | retain zero-sector proofs with one owner-local condition-source representation; move orchestration to foundry and prune ceremonies |
