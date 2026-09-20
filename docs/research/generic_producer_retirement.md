# Generic producer retirement plan

Status, 2026-09-20: source and consumer audit complete; retirement **planned,
not implemented**. This document records the dependency boundary and acceptance
gates. It changes no implementation, defaults, artifact, or dependency pin.

## Requirement

The authoritative requirement in [GOAL.md](../../GOAL.md) applies to every lane:
never implement a strategy or production algorithm specialized to a loop count
or named topology. Structural conditions must make sense at arbitrary loop
count; unsupported inputs retain a generic fallback or an explicit diagnostic.
Concrete families belong in external inputs and validation fixtures. Existing
specialized producers are cleanup debt, not exceptions to this requirement.

The frontend's compiled arity range is a separate adapter limitation. Core
solver and exact-authority services must remain generic in topology and arity.
Renaming a specialized recipe or placing a generic wrapper around it does not
meet the retirement requirement.

## Audited production boundary

The active `family_candidates` and `family_close` paths accept family data and
use the generic solver and source-port authority. The historical campaign API
instead selects the sole `FoundryCampaignPreset` variant for K6. Its wave runner
uses a compiled orbit itinerary and constructs the K6 family and terminal
authority. These remain reachable through Rust, CLI, and Python.

The relevant core boundaries are [campaign registration](../../crates/rustred-core/src/foundry/campaign/mod.rs),
[artifact registration](../../crates/rustred-core/src/foundry/artifact/mod.rs),
and [durable persistence dispatch](../../crates/rustred-core/src/foundry/artifact/persistence/mod.rs).
Decoding the old K6 producer grammar also reaches specialized terminal
construction, independently of the public campaign entry points. Removing only
those entry points is incomplete.

Existing K6 cell studies, closure sweeps, SpIReD diagnostics, and shared discovery
fixtures already have `#[cfg(test)]` boundaries. Their concrete families are
valid regression inputs. The K6 family, manifest, symmetry, terminal-authority,
factorization, and publication constructors currently also compile in production;
their remaining test consumers need explicit fixture ownership.

## First migration: obsolete compiled K6 machinery

Retire the K6 campaign, publication, and old producer-format decoding together:

1. Remove the compiled preset, K6 resource/profile and requested-domain policy,
   fixed wave itinerary, public runners, publication types, and dedicated
   application adapters. Preserve generic proposal/completion services such as
   the independent `involutive_seed` bridge and their regression coverage.
2. Remove `install_published_k6_sector_waves`, its specialized installer, and
   persistence support for `rustred.generated.three-loop-unit-mass-vacuum-k6.v1`.
   Move concrete constructors needed by tests into test-only fixtures. Keep
   production decoder behavior identical in unit and integration test builds;
   do not re-enable removed grammars under `cfg(test)`.
3. Remove CLI `campaign run`/`run-waves` and Python
   `run_foundry_campaign`/`run_foundry_wave_campaign`, with their Rust exports,
   request/report types, Python stubs, registrations, and dedicated progress
   adapters. Preserve generic planning, preflight, inspection, reduction, and
   family workflows, including shared progress rendering.
4. Migrate affected application/CLI/Python tests and the legacy campaign
   example/configs. Preserve exact replay, source-authentication, owner-route,
   factorization, and resource-limit coverage using explicit test inputs.
   Update README, interface/CLI documentation, and example instructions;
   retain historical research results with accurate retirement annotations.

The existing external [K6 input](../../examples/input/three_loop_k6.toml) and
its generic CLI/Python drivers already provide the migration destination.
Completing this slice retires the old K6 producer; it does not complete the
separate lower-loop recipe cleanup.

## Coupled K1/K3 Vakint migration

The audited local GammaLoop/Vakint checkout pins RustRed and rustred-app to
`8ad62b964de6f3a508fd165dc6ac2509f25839fe`. Its embedded assets use native
schema V6, but their producer identities differ:

| Shipped artifact | Algorithm identity | Current loading dependency |
| --- | --- | --- |
| K1 | `rustred.generated.one-loop-unit-mass-tadpole.v1` | Specialized family/rule verifier |
| K3 | `rustred.generated.two-loop-unit-mass-sunset.v1` | Regenerated specialized recipe, including K1 dependency |
| K6 | `rustred.source-port-original-domain.v1` | Generic source-port decoder and installer |

Vakint calls `ClosedArtifact::decode_durable`; K3 decoding itself calls
`derive_two_loop_unit_mass_sunset_with_limits`. These are current native assets,
not obsolete schema files. Native transport alone does not make their producer
semantics generic. The shipped K6 asset does not use the old K6 producer grammar.

Before deleting K1/K3 recipes, generate and independently certify replacements
from external family inputs through the generic engine. Cold-load them and
establish exact terminal projections for their actual master sets. Vakint's
`rustred_evaluation/artifact.rs` and `terminal.rs` bind schema, algorithm identity,
family fingerprint, and exact terminal-key coverage; migrate those manifests,
vendored assets, dependency pins, and factory-based tests together.

Then remove the lower-loop factories, specialized verifiers and codec branches,
`ClosingFamilySelector`, CLI `campaign generate`, and Python
`generate_closing_artifact`/`ClosingFamily`. Preserve the generation result class
also returned by `family_close`. Merely relabeling old bytes is not a migration.

## Preservation constraints

Keep generic solver/completion, source-port certification, terminal authority,
native Atom/State persistence, scalar numerator services, factorization,
reduction, and common-mass homogeneity. Preserve current native schemas and the
candidate/catalog/normalization services used by Vakint's four-loop programs.
Do not delete generic helpers solely because a retired recipe used them.

Sever K6-named constants from shared `ArtifactCoverReplayLimits` without changing
their values: arity 4,096; requested boxes 1,000,006; uncovered boxes 1,000,000;
requested/uncovered coordinate cells and split operations 12,000,072 each.
These caller-owned resource ceilings also serve generic source-port proofs.

## Acceptance gates

These are required future checks, not results of the audit:

1. Finish the current verification/source freeze before implementation. Audit
   production reachability after removal; check unchanged shared defaults,
   matching CLI help/parser and Python extension/stubs, and explicit rejection
   of retired APIs and the retired K6 producer identity.
2. Run workspace library and affected application/CLI/Python tests. Preserve
   exact replay, coverage, forged-source/guard rejection, typed limits, and
   generic numerator/factorization tests. Exercise production decoding through
   integration tests as well as unit tests.
3. Cold-load shipped K1/K3/K6 and run scalar, dotted, numerator/pinch, and
   homogeneity canaries. Run the existing external-input K6 generation,
   certification, cold-application, native semantic-equivalence, and
   worker-count determinism gates. No new higher-loop solve is required.
4. Against the updated pin, run Vakint's focused asset/catalog/normalization
   checks and all 83 established lower-loop numerical cases with unchanged
   tolerances and invalid FORM path on the native lane. Preserve four-loop
   assets and verify their cold native application; rerun the full applicable
   numerical matrix if shared runtime semantics, assets, or manifests change.
5. For K1/K3 replacement, additionally compare old/new mathematical reductions
   after exact terminal projection, including routing/permutations, dots,
   numerators, pinches, and scale restoration. Terminal sets and bytes may
   change; exact projected results must agree. Ship assets/manifests/pins
   together only after these checks pass. Generic K1/K3 terminal cardinalities
   and migration timings remain unmeasured by this source-only audit.
