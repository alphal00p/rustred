# RustRed Phase-0 source liveness ledger

**Baseline parent:** `ceaa079`, immediately before moving verified permutation
compilation under its symmetry owner.
**Status:** R3/symmetry-permutation working authority, subordinate to `GOAL.md` and the
clean-repository architecture plan.

This ledger classifies every one of the 166 Rust source/test paths remaining
after the current ownership milestone. It is intentionally hostile to
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
| move | 53 |
| split | 112 |
| delete | 0 |
| replace | 1 |

| Current path | Decision | Final owner | Evidence/action |
|---|---|---|---|
| `crates/rustred-app/src/application/campaign/plan.rs` | split | rustred-app::campaign | app-owned roots-only family/sector/job interning with deterministic ordinals and bounded truthful output, including the sole fallibly reserved sector-mask rendering boundary; no core replay/dependency plan or fictional future phases |
| `crates/rustred-app/src/application/campaign/preflight.rs` | split | rustred-app::campaign | retain topology-free resource preflight needed by the live app |
| `crates/rustred-app/src/application/derive/mod.rs` | split | rustred-app::derive | compose input lowering, admitted parallel IBP/LI batches, DTO conversion, and serialization without owning their representations |
| `crates/rustred-app/src/application/derive/model.rs` | split | rustred-app::derive | retain only the deterministic application output DTO and semantic-to-transport conversion |
| `crates/rustred-app/src/application/derive/census.rs` | split | rustred-app::derive | retain structural, payload, and render-bound preflights separately from execution and output representation |
| `crates/rustred-app/src/application/error.rs` | split | rustred-app | retain typed cross-frontend errors only |
| `crates/rustred-app/src/application/input.rs` | split | rustred-app / core input | retain TOML schema/transport decoding and metadata in the app while all compact/text/Atom normalization uses the canonical core input compiler |
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
| `src/identity/mod.rs` | move | identity | sole public facade over row identity, exceptional-domain conditions, sparse relations, and prepared parametric generation; no root aliases or internal index-space bridge |
| `src/identity/row.rs` | move | identity | one real stable row identifier shared by generated and translated identities, with a generic derived-row variant reserved for the foundry; no adapter row mirror |
| `src/identity/condition/mod.rs` | move | identity::condition | narrow private-owner facade whose construction seam is confined to the identity tree |
| `src/identity/condition/source.rs` | move | identity::condition | deterministic atomic parametric-condition provenance and stable user-facing encoding; no adapter or recursive provenance tree |
| `src/identity/condition/limits.rs` | move | identity::condition | independent source-cardinality policy |
| `src/identity/condition/error.rs` | move | identity::condition | typed source, resource, and authenticated-coefficient failures |
| `src/identity/condition/value.rs` | split | identity::condition | authenticated condition value, checked translation, deterministic source merging, and collection insertion; construction remains engine-owned and specialized base-field ceremony waits for a real reduction consumer |
| `src/identity/condition/tests.rs` | move | identity::condition tests | focused version-stable provenance encoding sentinel; operational event provenance is exercised through real relation events |
| `src/lib.rs` | replace | crate facade | write from retained use cases; concrete-relation, root relation/condition/row/symmetry/permutation exports and internal `IndexSpace` exports are gone, and remaining root reexports receive the same final audit rather than being moved wholesale |
| `src/identity/generator/mod.rs` | move | identity::generator | private-owner composition and canonical identity-facade exports for the prepared batch spine; no former flat-module alias |
| `src/identity/generator/config.rs` | move | identity::generator | minimal relation-resource policy for exact ordinary-IBP and LI construction |
| `src/identity/generator/counts.rs` | move | identity::generator | private checked topology-neutral ordinary/LI row census shared by construction and batch preparation |
| `src/identity/generator/error.rs` | move | identity::generator | typed allocation, row-layout, semantic-scope, resource, algebra, relation, condition, and family failures plus the owner-local exact-reservation helper |
| `src/identity/generator/scope.rs` | move | identity::generator | private ordinary/external layout and shared semantic family/context scope token |
| `src/identity/generator/model.rs` | split | identity::generator | retain only the non-cloneable family-bound generator, sealed source row/barrier, and immutable ordinary/external/LI prepared work; no serial aggregate wrapper |
| `src/identity/generator/construction.rs` | split | identity::generator | authenticated indexed context and reusable zero/unit-shift and power preparation with exact fallible reservation before every caller-sized collection is populated |
| `src/identity/generator/source.rs` | split | identity::generator | prepared ordinary/external source batches, stable ordinal generation, fallibly allocated ordered completion, and the sole consuming relation extraction |
| `src/identity/generator/ordinary.rs` | split | identity::generator | topology- and loop-count-neutral ordinary and external-contraction IBP row construction |
| `src/identity/generator/lorentz.rs` | split | identity::generator | fallibly allocated LI pair preparation and exact weighted translation over one authenticated completed source barrier |
| `src/identity/generator/domain.rs` | split | identity::generator | family-domain lifting into source-attributed nonzero conditions on each new relation |
| `src/identity/generator/tests/mod.rs` | move | identity::generator tests | private focused test composition only |
| `src/identity/generator/tests/support.rs` | move | identity::generator tests | compact topology-neutral family/relation fixtures shared by generator sentinels |
| `src/identity/generator/tests/batch.rs` | split | identity::generator tests | retain semantic scope/layout/ordinal sealing, ordinary/external batch equivalence, and the empty LI batch for fewer than two external momenta |
| `src/identity/generator/tests/counts.rs` | split | identity::generator tests | retain exact general and structural six-loop row censuses plus ordinal bounds |
| `src/identity/generator/tests/domain.rs` | split | identity::generator tests | retain real family-domain provenance on generated identities |
| `src/identity/generator/tests/limits.rs` | split | identity::generator tests | retain generator propagation of exact-algebra resource failures |
| `src/identity/generator/tests/lorentz.rs` | split | identity::generator tests | retain the LiteRed LI sign convention and exact weighted denominator shifts |
| `src/identity/generator/tests/ordinary.rs` | split | identity::generator tests | retain exact ordinary-IBP convention and coefficients |
| `src/identity/relation/mod.rs` | move | identity::relation | canonical narrow relation facade with its index-space seam confined to the identity tree |
| `src/identity/relation/index.rs` | split | identity::relation | checked index-space/shift construction, shared no-copy shift storage and its owner-local clone sentinel, value ordering, and checked translation arithmetic; only `IndexShift` inspection is public |
| `src/identity/relation/limits.rs` | move | identity::relation | independent exact-arithmetic and condition-source policy for relation operations |
| `src/identity/relation/error.rs` | move | identity::relation | typed lattice, scope, family, domain, resource, condition, and coefficient failures |
| `src/identity/relation/model.rs` | split | identity::relation | sparse authenticated relation storage, public read views, and owner-local compatibility validation |
| `src/identity/relation/operations.rs` | split | identity::relation | transactional condition attachment, term collection, scaled addition, and affine translation; mutation stays engine-owned |
| `src/identity/relation/tests/mod.rs` | move | identity::relation tests | private focused test composition only |
| `src/identity/relation/tests/support.rs` | move | identity::relation tests | real rational-term fixture shared by condition-event sentinels; no fabricated provenance |
| `src/identity/relation/tests/index.rs` | split | identity::relation tests | checked allocation, overflow, arity, and bounded-iterator sentinels |
| `src/identity/relation/tests/translation.rs` | split | identity::relation tests | exact simultaneous key/coefficient translation and composition sentinels |
| `src/identity/relation/tests/conditions.rs` | split | identity::relation tests | real-event provenance merging, transactional source limits, and error-ordering sentinels |
| `src/sector/mod.rs` | move | sector | sole canonical facade over masks, direct restriction evidence, ordering, errors, and exact symmetry verification; foundational children are private and no root/long-name aliases remain |
| `src/sector/error.rs` | move | sector | typed arity, position, allocation, ordering, and descent failures plus owner-local checked collection/string helpers; the unused mask/pattern parser errors are deleted |
| `src/sector/mask.rs` | split | sector | topology-neutral unshifted-index mask construction, shared admitted bit storage, activity views, an allocation-free exact-size corner iterator, subsector ordering, and streaming display; no parser or allocating render convenience |
| `src/sector/restriction.rs` | split | sector | cuts, shared admitted pattern storage, streaming displays, and structured exclusion evidence without synthetic analytic zero/nonzero states or parser conveniences |
| `src/sector/ordering.rs` | split | sector | one deterministic ordering identity/schema, exact injective complexity keys with shared `u64` coordinate storage and `u128` aggregate sums, streaming display, comparisons, and strict-descent witnesses |
| `src/sector/tests/mod.rs` | move | sector tests | private focused test composition only |
| `src/sector/tests/support.rs` | move | sector tests | compact exhaustive index enumerator shared by foundation sentinels |
| `src/sector/tests/allocation.rs` | split | sector tests | retain the owner-local impossible-size iterator and typed fallible mask/pattern/cut allocation sentinels through F3 hardening |
| `src/sector/tests/mask.rs` | split | sector tests | retain raw-membership, bit orientation, allocation-free corner, Boolean-lattice, and refcount-only clone semantics |
| `src/sector/tests/restriction.rs` | split | sector tests | retain direct cut/pattern exclusion evidence across the complete small mask domain; owner-local pattern tests prove refcount-only clones |
| `src/sector/tests/ordering.rs` | split | sector tests | retain exact ordering injectivity/manifest, extreme-`i64` aggregate width, refcount-only coordinate clones, and first-component descent witnesses |
| `src/symbolica_affine_denominator.rs` | split | input::affine / algebra | retain only Atom-based scalar-product validation, exact affine projection, live resource policy, and lowering payload; standalone raw parsing, schema/stats metadata, compiler cloning/inspection, and their tests are deleted; next split this file under input and replace handwritten CAS kernels with audited Symbolica APIs |
| `src/input/mod.rs` | move | input | sole canonical input facade over focused private owners; no former flat-module or root aliases, while the minimal live affine values/errors/limits remain temporary exports until their private subtree move |
| `src/input/model.rs` | split | input | retain syntax-authenticated normalized and lowered mathematical values plus their ordinary views; prune transport provenance, duplicate lowered fields, and compatibility conveniences in the later input tranche |
| `src/input/request.rs` | split | input | retain topology-neutral text/Atom request DTOs shared by Rust CLI and Python entrypoints; serde transport belongs to the app |
| `src/input/limits.rs` | split | input | retain checked parsing/lowering policies, live stats, and neutral checked resource arithmetic; obsolete Pattern counters are deleted |
| `src/input/error.rs` | split | input | retain typed parser and lowering failures; Pattern-only errors are deleted and the remaining temporary affine split taxonomy is merged after its private move |
| `src/input/compiler.rs` | split | input | thin guarded public compiler facade exposing exactly compact, text, and authenticated-Atom entrypoints with no former names or bypass aliases |
| `src/input/compact.rs` | split | input | compact `I(...)` orchestration with direct authenticated function-head/arity dispatch; no Symbolica wildcard Pattern machinery |
| `src/input/canonical.rs` | split | input | deterministic canonical project census and construction in the fixed semantic clause order |
| `src/input/gram.rs` | split | input | exact upper-triangular external-Gram validation and dense semantic completion |
| `src/input/normalize.rs` | split | input | topology-neutral cross-field validation, parameter inference, and normalized-project assembly |
| `src/input/lower.rs` | split | input | consuming and borrowed normalized-project lowering into one authenticated integral family pending copy pruning |
| `src/input/symbols.rs` | split | input | reserved-name, label, identifier, and bounded family-scalar discovery semantics |
| `src/input/parse/mod.rs` | move | input::parse | private authenticated expression ingress and one-way composition of lexical, numeric, grammar, conversion, and census stages |
| `src/input/parse/lexical.rs` | split | input::parse | raw byte, depth, digit, ANSI, and lexical-run preflight before Symbolica parsing |
| `src/input/parse/numeric.rs` | split | input::parse | checked numeric preconversion envelopes and exact signed-integer admission |
| `src/input/parse/grammar.rs` | split | input::parse | direct positional Token and Atom grammar/head/arity policy with fixed bounded clause dispatch |
| `src/input/parse/convert.rs` | split | input::parse | bounded authenticated Token-to-Atom conversion without owning symbolic algebra |
| `src/input/parse/census.rs` | split | input::parse | post-conversion Atom and aggregate project-field resource censuses |
| `src/input/tests/mod.rs` | move | input tests | private focused test composition and shared topology-neutral fixtures |
| `src/input/tests/frontends.rs` | split | input tests | retain parameter inference, canonical identity, and convergence of compact/text/Atom frontends |
| `src/input/tests/grammar.rs` | split | input tests | retain direct grammar/head/arity rejection and base-coefficient field semantics |
| `src/input/tests/lowering.rs` | split | input tests | retain signed target, external-Gram, numerator, and exact family-lowering semantics |
| `src/input/tests/resources.rs` | split | input tests | retain preconversion, caller-owned Atom, aggregate text, depth, integer, and unique-name resource boundaries |
| `src/sector/symmetry/mod.rs` | move | sector::symmetry | sole canonical exact-symmetry facade and owner of verified permutation compilation; candidate generation is explicitly outside this proof boundary and no root alias exists |
| `src/sector/symmetry/permutation/mod.rs` | move | sector::symmetry::permutation | narrow canonical facade for restriction-independent verified internal-permutation compilation and checked transport |
| `src/sector/symmetry/permutation/model.rs` | split | sector::symmetry::permutation | retain the exact verified map plus compact inverse permutation, without cloned verification policy or replay artifacts |
| `src/sector/symmetry/permutation/compile.rs` | split | sector::symmetry::permutation | compile and semantically validate a verified internal permutation independently of later sector restrictions, allocating retained state only after validation |
| `src/sector/symmetry/permutation/transport.rs` | split | sector::symmetry::permutation | apply caller-selected restrictions and transport indices into caller-owned storage with explicit arity checks |
| `src/sector/symmetry/permutation/error.rs` | split | sector::symmetry::permutation | concise typed compile and transport failures without schema/fingerprint ceremony |
| `src/sector/symmetry/permutation/tests.rs` | split | sector::symmetry::permutation tests | focused topology-neutral compilation, restriction, inverse, and caller-buffer transport coverage |
| `src/sector/symmetry/model.rs` | split | sector::symmetry | retain shape-checked coefficient matrices, momentum/scalar-product/denominator maps, row actions, Jacobian classification, and sealed verified values; the unused in-process schema constant is deleted |
| `src/sector/symmetry/condition.rs` | split | sector::symmetry | retain exact nonzero-condition source/value merging owned by symmetry verification |
| `src/sector/symmetry/limits.rs` | split | sector::symmetry | retain checked verifier resource policy and live operation statistics pending the later public-surface prune |
| `src/sector/symmetry/error.rs` | split | sector::symmetry | retain one typed topology-neutral verification and resource error taxonomy |
| `src/sector/symmetry/verify/mod.rs` | split | sector::symmetry::verify | guarded composition of shape, map, domain, determinant, denominator, replay, and output authentication stages |
| `src/sector/symmetry/verify/algebra.rs` | split | sector::symmetry::verify | family-semantic scalar-product lift and exact checked coefficient operations; native Symbolica remains determinant/product/congruence authority |
| `src/sector/symmetry/verify/kinematics.rs` | split | sector::symmetry::verify | external-Gram preservation and induced scalar-product map verification |
| `src/sector/symmetry/verify/denominator.rs` | split | sector::symmetry::verify | affine denominator-map derivation, candidate guards, and exact row classification |
| `src/sector/symmetry/verify/matrix.rs` | split | sector::symmetry::verify | narrow shape and checked matrix-adapter composition without a second CAS |
| `src/sector/symmetry/verify/replay.rs` | split | sector::symmetry::verify | independent exact denominator-map replay retained until the later ceremony/liveness prune |
| `src/zero_sectors.rs` | split | sector | retain zero-sector proofs with one owner-local condition-source representation; move orchestration to foundry and prune ceremonies |
