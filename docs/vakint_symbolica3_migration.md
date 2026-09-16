# Vakint integration with the current RustRed dependency stack

Status: **completed and pushed** as GammaLoop `a3d26dab` on `vakint_rustred`,
2026-09-16. The complete recorded 83-test selection passes on the new dependency
stack, together with full-workspace compilation and the focused runtime gates
below. Four-loop artifacts and acceptance remain separate open goals.

The preceding checkpoint shipped RustRed `ce92d3a7` and the coherent
Symbolica/Numerica 2.2 stack. RustRed's current main branch uses the
vendored Symbolica 3.0 checkout at
`953e26e2754e9a4b918404fbdea590725d8e863d`. The existing through-three-loop
acceptance matrix passes on both stacks. The dependency and embedded-artifact
alignment required before four-loop integration is now shipped.

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

These initial isolated results were focused adapter tests, **not** the complete
comparative acceptance gate. The subsequent main-tree gate below supplies
that distinct result. The narrowed workspace and local Cargo paths were only
diagnostic conveniences.

## Main-tree acceptance gate

The main GammaLoop workspace now resolves with published RustRed
`09cef8e3cf7487dded7803edc26df5d51bb9f500` and published
Symbolica/Graphica/Numerica `953e26e2754e9a4b918404fbdea590725d8e863d`.
The workspace-hack tables move with these pins. Additional Idenso test and
benchmark matching setters are mechanically adapted; no local-path dependency
is proposed for shipping.

K1 and K3 regenerate byte-identically. K6 is replaced by the generic CLI/Python
producer's identical, cold-checked `0x704` bytes, with the identifier-safe family
name updated in the private Vakint loader. All 38 terminal keys are unchanged;
no new master catalog or compatibility decoder is needed.

The exact frozen selection passes in nine batches: **83 passed, zero failed,
zero ignored**. Native FeynKit/RustRed lanes retain invalid FORM paths, while
the separate AlphaLoop/MATAD and terminal-oracle lanes use FORM5. No assertion,
tolerance or default is changed. The nine binary hashes, runner, selection and
results are retained under `/tmp/vakint-symbolica3-main.bMELju/`; the
`acceptance/` subdirectory contains per-binary logs and the batch result table.
The scoped Vakint library/test compilation also passes. These are correctness
gates; debug test runtime is not a performance benchmark.

The broader workspace library, test and benchmark compilation gates also pass.
Native matrix solving preserves unique/generic field-valued contracts where
appropriate; UV partial momentum routing retains free coordinates and rejects
conditional solutions. Focused runtime results are 28 GammaLoop passes, four
Spenso passes, and 52 FeynKit passes with one unchanged expensive ignored test.
These include exact high-precision conversion, guarded routing, and native
zero-dimensional integration samples. Scoped Vakint Clippy passes with
`-D warnings`; this is not a whole-workspace lint or runtime claim.

Symbolica 3's removed OEM macro is not emulated. Native runtime signed-license
activation is used, without committing any key. Renewed OEM distribution is a
separate upstream requirement. K6's shipped size is 8,916,759 bytes and SHA256
`bcf45f876695c99516a7cefec2ae3cd3c5886a89d3da156391a6ea75237ff2ff`;
its 38 terminal keys are unchanged. The validated producer remains pinned at
`09cef8e3`; a later RustRed producer must be validated separately before repinning.

## Completed migration sequence and remaining integration

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
   name `rustred_three_loop_unit_mass_vacuum_k6_v1`; the previous
   Vakint asset used the earlier hyphenated name. This deliberately changes
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
satisfies both requirements. The main-tree check subsequently fetched that
locked version without changing its constraints. The initial diagnostic
report and compiler logs remain outside the repository at
`/tmp/vakint-symbolica3-census.1Szmr3/`; main-tree migration evidence is at
`/tmp/vakint-symbolica3-main.bMELju/`. Production manifests, adapters, artifact
and acceptance report are committed together in `a3d26dab`.
