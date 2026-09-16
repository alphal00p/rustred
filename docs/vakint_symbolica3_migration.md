# Vakint integration with the current RustRed dependency stack

Status: independently audited API adapters and focused runtime gates pass in
an isolated worktree, 2026-09-16; **the production branch is not yet migrated**.

Vakint's current `vakint_rustred` branch ships RustRed `ce92d3a7` and the
coherent Symbolica/Numerica 2.2 stack. RustRed's current main branch uses the
vendored Symbolica 3.0 checkout at
`953e26e2754e9a4b918404fbdea590725d8e863d`. The existing through-three-loop
acceptance matrix passes on the shipped stack. Four-loop integration must
first align these dependencies and regenerate the embedded artifacts.

## Observed compatibility boundary

Vakint passes native Symbolica atoms and exact coefficient values to RustRed,
so using two Symbolica versions is not an adapter solution. The relevant crate
closure is Vakint, FeynKit tensor/graph/model/kinematics, Idenso, Spenso and its
macros, Linnet, and Symbolica-utils. The existing RustRed reduction and scalar
numerator APIs are unchanged across the two RustRed revisions.

A detached diagnostic worktree, narrowed to this crate closure and using local
dependency paths, initially reported twelve concrete Spenso errors:

- `LicenseManager` moved from the Symbolica crate root to `symbolica::license`.
- Ten matching builders use the removed `level_range((min,max))` setter. The
  native replacements are `min_level(min).max_level(max)`, with the same
  inclusive limits and existing depth interpretation.
- The native equation solver now returns `SolutionSet`, not `Vec<Solution>`.
  Its global coverage and coverage guard are mathematically significant;
  blindly collecting its branches into a vector would discard information.

Downstream inspection additionally found eight matching-depth calls in Idenso
and Vakint's simultaneous routing helper expecting `as_slice()` on the old
solver result. These demonstrated incompatibilities have now been adapted in
the isolated worktree. The complete relevant production dependency closure,
including FeynKit tensor and Vakint, passes the scoped library check. No
FeynKit production source change was needed.

## Isolated implementation and runtime evidence

The patch touches five production files: Spenso's license import, parsing
materialization and parametric-atom wrappers; Idenso's metric shorthand
matcher; and Vakint's topology routing. The existing Spenso pattern-wrapper
interface still forwards the same inclusive matching depths to Symbolica's
native setters. Its linear solver forwards the native `SolutionSet` after the
existing native linearity gate, solving the original expressions so input
denominator conditions are preserved.

Vakint accepts a routing witness only with complete coverage, an empty global
guard, exactly one unconditional point branch and every requested momentum
assigned. Native coordinates retain requested-variable order. The existing
simultaneous substitution into numerator and propagators is unchanged. No
custom algebra, topology rematching, defaults change or artifact compatibility
shim was introduced.

The focused runtime gates passed with an invalid `FORM_PATH`:

| Gate | Result | What it protects |
| --- | --- | --- |
| Vakint topology and MATAD routing | 9/9 | All five 3L matcher classes, signed momentum identities, mass/power preservation, simultaneous transport, and rejection of conditional, dependent or nonlinear routes |
| Spenso linear-system forwarding | 4/4 | Ordered exact solutions, retained global/branch conditions and free variables, and nonlinear rejection |

The Spenso regression explicitly distinguishes `a*x=0`, whose answer `x=0`
requires the global generic-coverage guard `a != 0`, from `x/a=0`, whose
answer has complete coverage only on the branch's original domain `a != 0`.
Neither obligation may disappear just because the returned coordinate has
no denominator. An independent audit verified this contract and the matching
depth and routing changes against the vendored Symbolica API.

The scoped Vakint library check and no-dependencies Clippy gate with
`-D warnings` also passed. This does not claim a warning-free upstream
dependency stack or a whole-workspace lint gate.

These are focused adapter tests, **not** the complete 83-test comparative
acceptance gate on the upgraded stack. The narrowed workspace and local Cargo
paths remain diagnostic conveniences. Idenso's additional integration-test
and benchmark matching calls still require mechanical adaptation if those
targets are selected in the broader workspace gate.

## Coherent delivery sequence

1. Adapt only demonstrated incompatibilities in the relevant local crates,
   preserving Vakint's public conventions, defaults and FORM-based backends.
   Preserve `SolutionSet` coverage/conditions rather than weakening the solver
   contract. Add exact tests for conditional and nonunique routing results.
2. Align Symbolica/Numerica/Graphica and the published RustRed Git revision in
   one dependency milestone. Local paths are allowed during development, not
   in the pushed milestone. Do not broadly update unrelated dependencies.
3. Regenerate and cold-validate K1/K3/K6 with that producer. K6's source-port
   parent-plan tag changed from `0x701` to `0x704` (including explicit root scope)
   although the outer artifact
   schema is still V5. Do not edit bytes or add a compatibility shim. Confirm
   terminal keys before reusing the exact offline catalog.
   The new external K6 input and Rust generator use the ingress-compatible
   name `rustred_three_loop_unit_mass_vacuum_k6_v1`; the currently shipped
   Vakint asset uses the earlier hyphenated name. This deliberately changes
   artifact identity, not denominators, coordinate order or mathematics.
   Update Vakint's private expected family fingerprint atomically with the
   new bytes and dependency pin. Compare all 38 typed terminal keys directly
   against `crates/vakint/src/rustred_evaluation/terminal/k6.rs::SOURCES`;
   equality of the old and new family-fingerprint strings is not expected.
4. Rerun the complete 83-test selection, routing diagnostics, FeynKit/Spenso
   regressions and the relevant compilation/lint gates. Native acceptance
   retains invalid FORM paths; separate legacy oracle lanes use FORM.
5. Check the broader GammaLoop workspace separately. Newly generated native
   evaluators now carry explicit input/output dimensions; pre-upgrade compiled
   evaluator caches must be rebuilt, not assumed ABI-compatible.
6. Only then integrate successfully published four-loop artifacts, routing and
   offline terminal values. Dependency compatibility is not four-loop closure.

The initial full-workspace offline resolution hit a local cache/index mismatch
for `typed-index-collections` 3.3.0 versus a cached 3.5.0 candidate. This does
not prove an incompatible dependency constraint: the intended locked version
satisfies both requirements. Fetch/preserve it rather than changing unrelated
constraints. The full diagnostic report and compiler logs remain outside the
repository at `/tmp/vakint-symbolica3-census.1Szmr3/`. No production GammaLoop
manifest or source was changed by this investigation.
