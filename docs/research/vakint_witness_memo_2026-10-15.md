# Package-bound Vakint witness reuse

15 October 2026. This is an **application** optimization for an already loaded
saved artifact, not a five-loop IBP-generation or family-closure result.

Delivered on GammaLoop's `vakint_rustred` branch in commit
`7b817a99` after independent source, runtime and reversed-pair audits.
Only the seven reviewed source/test/documentation files were committed; no
application package, cache, DOT output or local benchmark evidence was pushed.

## Mechanism and safety boundary

The preceding native DOT acceptance pilot found that the same scalar input's
warm frontend call spent 44.789s selecting an admitted routing witness but only
0.000572s applying cached IBPs. The new private memo retains the exact witness
chosen by the unchanged minimum-key matcher. It does not switch to the first
admitted witness, change topology matching, add rules, or bypass input admission.

The prepared application owns the memo, binding it to one immutable package and
terminal convention. Exact integral/numerator Atoms, relevant scalar/matching
policies, and retained registry/catalogue incarnation tokens form the key.
Registry replacement misses; clones sharing the same registry/package can reuse.
Lookup follows package/path/options validation. Hits rerun native admission and
compare the accepted key; mismatches invalidate, while actual typed errors
propagate. Non-admitted fallback witnesses are never retained.

The private LRU is bounded to 32 entries and 16MiB of tracked encoded Atom
payload. This is **not total RSS**: topology graphs, patterns, containers, index
vectors and shared symbol metadata are outside that metric. Oversize entries
are skipped, older entries evicted, and a poisoned optional memo bypassed.
The native coefficient cache and its configurable LRU/LFU policies are unchanged.
The existing clear operation clears retained witnesses; a separate Rust getter
reports witness-cache statistics. No persistent cache/schema/setting was added.

## Completed gates

All tests used the same immutable completed R0/D10 package described in
[the R0 application report](r0_d10_application_acceptance_2026-10-15.md), with
an invalid FORM path and no numerical master substitution.

| Gate | Result | Native time | Supervised time | Sampled peak RSS |
|---|---|---:|---:|---:|
| Five focused metadata/policy/registry tests |5 passed|0.34s|1.183s|0.019GB|
| External exact minimum-witness and mutation gate |1 passed|270.59s|271.777s|2.743GB|
| Existing 4MiB coefficient-LRU regression |1 passed|88.40s|89.381s|2.905GB|
| Public scalar DOT example and invariance controls |passed|phase table below|250.845s|2.879GB|

The external gate compares exact witness maps and accepted keys against the
uncached matcher; tests clone-before-load, tightened matching/scalar limits,
unsupported fallback, concurrent hits, registry replacement, explicit clearing,
stale comparison-key invalidation and deliberate optional-mutex poisoning.
Its panic is intentionally caught; the final uncached fallback must succeed.
The public example checks exact guarded values, standard/detailed APIs,
simultaneous loop-basis change, relabeling/orientation and indexed notation.
These are scalar controls, not nontrivial numerator or all-domain acceptance.

The old LRU regression still records 4,842 evictions, 4,059,928 evicted
coefficient bytes and 815 recomputed nodes. The public example still applies
3,753 IBP rules and retains 7,627 coefficient-cache integrals. Its witness memo
records four hits, two misses, two retained entries and 4,767 tracked Atom bytes.

## Observed timing and unresolved cold variation

Same package, scalar DOT/powers and CPU affinity; optimized builds, compilation
excluded. These are single runs on a shared host, not a statistical comparison.

| Measurement | Before memo | With memo |
|---|---:|---:|
| Package preparation |73.366379s|73.860758s|
| First complete frontend application |53.832493s|64.196009s|
| First admitted-witness selection |41.044568s|47.981652s|
| First IBP application |12.708001s|16.150116s|
| Warm complete frontend application |44.808307s|0.032923s|
| Warm admitted-witness selection |44.788877s|0.003011s|
| Warm IBP application |0.000572s|0.001266s|
| Uncached rerouted-input witness selection |46.878812s|109.639257s|
| Complete public example, supervised |353.044770s|250.844656s|

The same-input warm benefit is directly supported by hit counters, exact tests
and phase logs. Do not extrapolate its approximately 1,361× ratio to general
throughput. The complete example was approximately 29% shorter in this pair,
but first-use costs did not improve. In particular, the rerouted miss became
slower despite identical printed original, rerouted and postcanonical inputs.
The cause of that cold variation has not been established; it is not silently
attributed to host contention. The memo still runs the unchanged matcher on
such misses. A reversed-order frozen repeat investigated this uncertainty.

### Reversed-order frozen repeat

The second pair ran **new then old**, reversing the original order, without
rebuilding either executable. Both completed every public-example assertion,
using the identical DOT, powers, package, CPU set and memory policy. The pair
took 866.589s including validation and cleanup, with no resource stops.

| Measurement | Old frozen binary | New frozen binary |
|---|---:|---:|
| Package preparation |75.550497s|75.757893s|
| First complete application |78.374248s|78.077734s|
| First witness search |64.975067s|64.383356s|
| Warm complete application |76.409812s|0.022868s|
| Warm witness search |76.386808s|0.002133s|
| Uncached rerouted witness search |113.596605s|83.088036s|
| Complete example, supervised |625.756187s|238.773791s|
| Sampled peak RSS |2.881GB|2.886GB|
| Full witness searches |6|2|

Every full search in both arms visited the same 721 graph mappings and 23,040
orientation candidates and verified 1,440 witnesses. The new memo avoided four
whole searches rather than truncating any search. Its two retained entries
again contained 4,767 tracked Atom bytes. The printed canonical final expression
is byte-identical across arms (SHA256 including the trailing newline:
`652a39a1d75c723cd1e3d8607ec278ac88b3e04459e7c207bd5ad1721952f595`).

The first-use times now agree closely, and the old binary also shows substantial
cold variation. Thus a consistent new-only cold regression was **not reproduced**;
the underlying variability is still not causally explained. The whole example
was 61.8% shorter in this reversed pair, in addition to the first pair's 29%.
These are two observed controls, not a universal or confidence-bounded speedup.
Only repeated identical-context inputs are guaranteed to avoid full enumeration;
new inputs retain the existing search cost.

Native search wall times came from existing phase timers. Approximate CPU time
from 1s process telemetry and a 0.2s log observer was 64.26s old versus 63.63s new
for the first search, and 112.05s versus 82.15s for the rerouted search. Subsecond
hit CPU costs are below that sampling resolution and are not invented from
interpolation. The host was not exclusive: observed1-minute load ranges during
measured new searches were approximately134--145 and old searches123--134.
Snapshots across CPUs20--23 ranged1.8--3.1GHz and include idle cores; they do not
establish the cause of timing changes. Disjoint-core development jobs were
running elsewhere on the host.

Evidence: `paired-repeat/result.json`, `phase-summary.json`, old/new native
outputs and resource logs, and the timestamped `*-observation.jsonl` files.
Both binaries and all77 package files were checked before and after this pair.

## Build, resources and evidence

The actual modified Vakint library, unit binary and public example were built
against pinned RustRed `78cbc5c1`, Symbolica `400a53c4` and FeynKit `024f96d6`,
with optimized thin LTO and 16 codegen units. The direct workspace build enables
`feynkit-ingress,rustred-app/capacity-dispatch`; its RustRed `native` alias is
not explicitly selected. That alias only forwards Symbolica native features,
and an audit found no native-feature branches in pinned core/app code. The
actual Symbolica feature set remains identical GMP/MPFR/bincode/serde/tracing.

The initial dependency-selected harness invocation compiled the library but
omitted example development dependencies and failed. The corrected workspace
build passed in 761.517s, including refreshed unchanged dependencies. The first
fast gate passed four tests and exposed a fixture that accidentally tested the
epsilon-name oversize condition instead of Atom-budget eviction. Only its
input values changed, with an explicit size precondition; eviction and oversize
assertions were preserved. Test-only relinking took 301.279s. Both failed
receipts remain available and no production algorithm was changed for them.

Runtime CPUs20--23, 32GB aggregate RSS allowance, 150GB host reserve and a
1,740s cooperative deadline; total pilot 615.208s, all four child groups
exited0 and drained without a resource stop. No production campaigns, rules,
application packages, numerical masters or persistent caches were changed.

Evidence under `TMP/vakint-witness-memo-20261015/`:

- `workspace-build/retry2/build-binding-after.json`, compiler JSON logs and
  `guard-retry2/`: exact source/manifest binding and build profile;
- `run2/pilot-result.json` and phase directories: outcomes, full commands,
  stage logs and process-tree resource observations;
- `run2/witness-memo-independent-audit.json`: independent source/runtime audit;
- `memo-fast/` and the original build directories: retained negative receipts.

Example SHA256: `c4bf7537ecaf24d65a6ef0f82f03b6eb192ff17888f80474ffcbffd5c2e3d807`.
Unit SHA256: `39352ee7f2c1f35d36f0b00aa17cdd19cdc8aaea9b6639b6206f46d58a97925c`.
The auditor verified all 78 source bindings, both binaries and all 77 external
package files unchanged. Full R6 coverage, the physical numerator ladder and
numerical master evaluation remain separate outstanding goals.
