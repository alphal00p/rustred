# Vakint integration with the current RustRed dependency stack

Status: audited migration census, 2026-09-16; **not an implemented upgrade**.

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
dependency paths, successfully checked current RustRed, Symbolica,
Symbolica-utils, Linnet, and FeynKit model/kinematics/graph. It then reported
twelve concrete Spenso errors:

- `LicenseManager` moved from the Symbolica crate root to `symbolica::license`.
- Ten matching builders use the removed `level_range((min,max))` setter. The
  native replacements are `min_level(min).max_level(max)`, with the same
  inclusive limits and existing depth interpretation.
- The native equation solver now returns `SolutionSet`, not `Vec<Solution>`.
  Its global coverage and coverage guard are mathematically significant;
  blindly collecting its branches into a vector would discard information.

Static downstream inspection additionally finds eight matching-depth calls
in Idenso and Vakint's simultaneous routing helper expecting `as_slice()` on
the old solver result. That helper must retain its existing ordered witness
and require complete, guard-free coverage with a unique unconditional point
solution before accepting the route. These are native API adaptations, not
new algebra algorithms. Further compiler errors may appear after the first
layer is fixed; the initial count is not a completed migration estimate.

## Coherent delivery sequence

1. Adapt only demonstrated incompatibilities in the relevant local crates,
   preserving Vakint's public conventions, defaults and FORM-based backends.
   Preserve `SolutionSet` coverage/conditions rather than weakening the solver
   contract. Add exact tests for conditional and nonunique routing results.
2. Align Symbolica/Numerica/Graphica and the published RustRed Git revision in
   one dependency milestone. Local paths are allowed during development, not
   in the pushed milestone. Do not broadly update unrelated dependencies.
3. Regenerate and cold-validate K1/K3/K6 with that producer. K6's source-port
   parent-plan tag changed from `0x701` to `0x703` although the outer artifact
   schema is still V5. Do not edit bytes or add a compatibility shim. Confirm
   terminal keys before reusing the exact offline catalog.
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
