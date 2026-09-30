# Runtime-order acceptance and input staging

This directory contains **study tooling**, not a second IBP implementation.
`stage.py` copies freshly generated candidate shards into a fixed selected-owner
manifest. It neither parses native bincode/Atoms nor proves an IBP identity.
Native candidate admission and independent cold reinspection follow staging.
Historical discovery-only stagers, payloads and benchmark receipts stay intact.

## Data-only staging

Run the lightweight tests in the repository Nix environment:

```sh
nix develop --command python -B -m unittest discover \
  -s tools/research/runtime_order -p test_stage.py -v
```

The tests use deliberately synthetic, non-native bytes, exercise positive and
negative metadata cases, and write their temporary files only under `TMP/`.
They are not native loading or closure tests.

A plan is JSON, with paths relative to the plan file. For example:

```json
{
  "schema": "rustred.runtime-order-stage-plan.v1",
  "family": {"path": "family.toml", "sha256": "FROZEN_INPUT_SHA256"},
  "selection": {"path": "selection.json", "sha256": "FROZEN_SELECTION_SHA256"},
  "queries": {"path": "queries.json", "sha256": "FROZEN_QUERY_SHA256"},
  "input_format": "toml",
  "integral_order": "EXACT_NATIVE_EMITTED_ORDER_IDENTITY",
  "discovery_strategy": null,
  "exact_backend": "sparse",
  "solver_policy": "EXACT_NATIVE_SOLVER_POLICY",
  "roots": [
    {"parent": 1, "mask": "111", "checkpoint": "generation/checkpoint.toml",
     "report": "generation/report.toml"}
  ]
}
```

`discovery_strategy` must explicitly match the native serialized recipe, including
an explicit `null` for the legacy default. Obtain `integral_order` from the native
report for the frozen requested descriptor; do not reproduce its encoder in
Python. Record the descriptor JSON and exact generation command alongside the
plan. The IDs and values above are placeholders, not a runnable physics fixture.

```sh
nix develop --command python -B tools/research/runtime_order/stage.py \
  --plan TMP/STUDY/stage-plan.json --output TMP/STUDY/inputs
```

Every input must be frozen and generation must have finished. The helper checks:

- Byte hashes for family source, selected-owner/routes manifest and query file.
- Unambiguous JSON (duplicate object keys are rejected).
- Complete explicit query roles, selected-owner references and route census.
- Native checkpoint version 3 and the order-v3 recipe; exact order, family,
  backend, discovery recipe, solver policy and root bindings.
- Every sector in each root's complete route-census downset, in native ordinal
  order, and a nonempty generated shard for every sector, not just selected ones.
- A matching normally completed generation report and all selected owners.

The staged selection retains exactly the frozen routes and selected-owner order.
Shard ordinals are looked up from sector masks rather than copied from an old
generation. Owner paths are relative; staged query bytes are unchanged. Receipts
record per-shard hashes, native ordinals and checkpoint/report provenance. Hashes
detect accidental copying/mixing errors; they are not mathematical certification.
In particular, a valid old or swapped native shard accompanied by matching new
external metadata is not detected by this Python tool. The follow-up native
runner must compare each payload's actual `integral_order` to the planned
identity using `inspect_generated_candidate_bundle`, then perform native owner
loading, including expected selected-sector/family admission. Merely loading a
valid but differently ordered payload is not a matched-order experiment.
The auto-discovered native example implements that step:

```sh
NATIVE_INSPECTOR --selection TMP/STUDY/inputs/selection.json \
  --owner-base TMP/STUDY/inputs --expected-order EXACT_NATIVE_ORDER_IDENTITY
```

`NATIVE_INSPECTOR` is the frozen `inspect_candidate_orders` executable, built
from `crates/rustred-app/examples/inspect_candidate_orders.rs`. It loads the same
immutable buffers it inspected, uses the existing shared selected-owner loader,
and checks the expected native family and exact sector set. It performs no IBP
generation, source replay or route-witness verification. A successful result is
`NATIVE_OWNER_BINDING_ADMITTED`, not a closed artifact.
The Python staging output explicitly states `STAGED_NATIVE_ADMISSION_AND_COLD_REQUIRED` and makes
no native-admission, source-replay or closure claim. Existing output directories
are never overwritten. An I/O failure can leave an incomplete new directory but
cannot produce a successful receipt; use a new output path after diagnosing it.

## Native correctness acceptance

Use one frozen build for an entire comparison. Record binary hashes, source
revision/dirty patch identity, compiler profile and exact argv. Do not rebuild
between descriptor experiments. Run these tests before considering timings:

| Test binary | Filter |
|---|---|
| Core library | `sector::ordering::programmed_tests::` |
| Core library | `source_replay_accepts_canonical_identity_priority_but_not_mismatches` |
| Core library | `multi_index_programmed_generation_replays_under_its_actual_descriptor` |
| Core library | `direct_reducer_and_owner_admission_bind_solution_order_and_sector` |
| Core library | `overlay_solution_order_is_bound_independently_of_its_outer_metadata` |
| Core library | `independently_programmed_owners_compose_after_a_strict_pinch` |
| Core library | `support_changing_same_count_edge_fails_instead_of_trying_a_later_rule` |
| Application library | `application::candidate_bundle::tests::order::` |
| CLI integration (`cli_candidates`) | `programmed_integral_order_survives_cli_reload_certification_and_application` |

Broader regression filters are `sector::ordering::`, `solver::index::`,
`solver::candidate_reduction::owners::feedback::tests::`,
`foundry::artifact::source_port::tests::` and the complete application
`application::candidate_bundle::` group. Execute native binaries directly with
the filter and initially `--test-threads=1`; obtain their paths from the Cargo
JSON `compiler-artifact` records, where `profile.test` is true and `executable`
is present. A `--lib` build does not build the CLI integration target or Python.

For correctness builds already using the shared target and application opt-level
override, the smallest additional CLI build is:

```sh
nix develop --command env CARGO_INCREMENTAL=0 \
  CARGO_TARGET_DIR=/common/dev/rustred/TMP/codex-runtime-discovery.280crc/target-native \
  cargo test --release --no-run --locked --offline --message-format=json \
  --config profile.release.package.rustred-app.opt-level=1 -j8 \
  -p rustred-app --test cli_candidates --example inspect_candidate_orders
```

That compiles the inspector's argument/decision tests. Build its runnable example
separately with `cargo build --example inspect_candidate_orders`, retaining the
same target/profile/configuration. This reuses existing engine dependencies;
the inspector introduces no library source or manifest changes.

The matching Python wheel can reuse that target/profile:

```sh
nix develop --command env CARGO_INCREMENTAL=0 maturin build \
  --release --locked --offline -j8 \
  --target-dir /common/dev/rustred/TMP/codex-runtime-discovery.280crc/target-native \
  --config profile.release.package.rustred-app.opt-level=1 \
  --out /common/dev/rustred/TMP/ORDER_ACCEPTANCE/wheels
```

These commands are **correctness-profile** acceptance, not optimized benchmark
results. Preserve flags, target directory and dependency features to avoid
redundant compilation. Do not use `maturin build --sdist`, which builds from an
extracted sdist. If distribution checking is needed, build the sdist separately
and run `crates/rustred-python/tests/assert_distribution_contents.py WHEEL SDIST`.

Extract/install the wheel into a fresh `TMP/` environment, set `RUSTRED_CLI` to
the matching frozen CLI, and put that wheel's package plus
`crates/rustred-python/tests` on `PYTHONPATH`. Check `rustred.__file__` first, then:

```sh
python -B -m unittest test_integral_order \
  test_candidate_api.CandidateApiTests.test_integral_order_native_cli_replay_and_checkpoint_identity
```

Follow with the complete `test_candidate_api` module. A pure Python builder test
does not establish native generation, persistence or application correctness.

## Admission of actual historical payloads

Before the S5 matched walking study, cold-load the **actual unchanged bytes** in
`TMP/inputs-v6/four-all/selection.json` and its `owners/` directory. Do not
regenerate/convert them to simulate backward admission. A minimal public smoke is
`routed-campaign` with a separate headerless CSV of owner corner powers (positive
mask bits become 1, the others 0), one worker and a new output directory:

```sh
NEWCLI routed-campaign --manifest TMP/inputs-v6/four-all/selection.json \
  --owner-base TMP/inputs-v6/four-all --targets TMP/ORDER_ACCEPTANCE/corners.csv \
  --workers 1 --output TMP/ORDER_ACCEPTANCE/legacy-smoke.json
```

Preparation loads all selected owner payloads. Record hashes before and after;
success proves native loading of those bytes and, if the trace completes, the
finite queried reductions. It does **not** establish the full physical query
scope. The separate full walker and independent `All` cold verification provide
that acceptance evidence.

## Matched runtime-order portfolio

Existing local four-family controls are under
`TMP/four-loop-saved-descendants.VaNmUN/{fg,bmw,h,x}/command-*.json`. The combined
common-coordinate study uses:

- `examples/input/four_loop_combined/four_loop_common_basis.toml`;
- `examples/input/four_loop_combined/selection.json` (16 selected owners and 508
  labelled nonzero-sector route records);
- `TMP/codex-rolling-measurements.eBFGjR/queries/four-all.json` (58 required queries);
- `TMP/codex-runtime-pivot-portfolio.78ueFJ/strategies/sparse-coeff.json`;
- generation roots `1111111110` and `0111111111`, respectively inactive axes 9
  and 0. These are **study inputs**, never topology dispatch in the engine.

First hold discovery, selected owners, all routing witnesses, query bytes, worker
budget, CPU placement, backend and walking policy fixed. Regenerate both root
downsets for each mathematical order. Never fill a missing new owner with an old
payload. The discovery winner remains terms-first, then coefficient-monomials;
varying both discovery and mathematical order at once would obscure the cause.

The initial, small **proposed** portfolio is:

1. The previous legacy-order winner versus explicit `rustred.integral_order(10)`.
   This tests semantic equivalence and descriptor overhead, not byte identity:
   explicit programmed metadata intentionally has a distinct persisted identity.
2. **Shared-interface, E-primary.** The two roots differ on axes 0 and 9, so use
   support weights `[3,1,1,1,1,1,1,1,1,3]` and support priority
   `[0,9,3,4,1,2,5,6,7,8]`. Keep unweighted total excess as the first degree row;
   then penalize positive excess on axes 0 and 9; finally use numerator degree.
   This tests whether making shared equal-line supports and unraised private
   lines simpler reduces routing/branching work. It may instead make an
   expensive shared subsector more prominent; full-campaign measurements decide.
3. **Routing-density, E-primary.** Keep default support priorities. The explicit
   common-coordinate denominators have momentum support sizes
   `[1,1,1,1,2,2,2,2,2,3]`; use the quadratic-incidence proxy
   `[1,1,1,1,3,3,3,3,3,6]` for both signs in a second degree row, after total
   excess and before numerator degree. Coordinate ties inspect higher-density
   coordinates first. This is a representation-dependent fill heuristic, not a
   graph invariant or a promised speedup.

These proposals are data variations using the existing descriptor API, not
implemented/tested performance wins. Only if there is evidence for it should a
further arm place a weighted row before total excess. That remains a lawful order
but can increase unweighted excess and must not be mislabeled E-primary.

For each root, start with the existing generation command, changing only output
locations and `--integral-order FILE` (never also `--permutation`). Example:

```sh
NEWCLI family-candidates --input examples/input/four_loop_combined/four_loop_common_basis.toml \
  --input-format toml --nonpositive-indices 9 --n-cores 16 --exact-backend sparse \
  --numerical-depth 2 --finite-case-policy search \
  --bundle-max-bytes 1073741824 --bundle-max-entries 8000000 \
  --bundle-max-total-coefficient-bytes 536870912 \
  --checkpoint-dir TMP/STUDY/root1022/sectors --checkpoint-max-bytes 4294967296 \
  --output TMP/STUDY/root1022/candidates.rrbin --report-output TMP/STUDY/root1022/generation.toml \
  --discovery-strategy TMP/codex-runtime-pivot-portfolio.78ueFJ/strategies/sparse-coeff.json \
  --integral-order TMP/STUDY/order.json
```

After metadata staging, use the unchanged combined scope with the matched
`owner-domain-match` Ready/Union policy. Freeze complete argv in `command.json`:

```sh
NEWCLI owner-domain-match --manifest TMP/STUDY/inputs/selection.json \
  --owner-base TMP/STUDY/inputs --queries TMP/STUDY/inputs/queries.json \
  --follow-successors --workers 16 --max-queries 58 --max-query-bytes 34104 \
  --max-guard-univariate-degree 64 --bounded-refinement-axes finite-axes \
  --transfer-unreserved-lookahead 256 --publication-policy ready \
  --route-domain-overcover --reuse-initial-d-bands --g2-residual-anchors union \
  --checkpoint TMP/STUDY/walk/checkpoint --checkpoint-interval-seconds 3600 \
  --unbounded-work --no-progress --output TMP/STUDY/walk/result.json \
  --events TMP/STUDY/walk/events.jsonl --stop-file TMP/STUDY/walk/STOP
NEWCLI walk-verify-closure --command TMP/STUDY/walk/command.json \
  --result TMP/STUDY/walk/result.json --require-closure --reinspect all \
  --reference-levers off --certification-scope all-roots --threads 16 \
  --output TMP/STUDY/cold.json
```

Report generation, walking and cold-validation times separately and combined,
CPU, peak RSS, generated rules/coefficient size, scheduled domains, peak pending,
required-query closure and frontiers. Cold scoped closure does not by itself
prove every candidate identity: strong regenerated-source replay remains its
own gate and any proof limitation must be reported. Run optimized matched arms
under the existing resource controller, keeping each pilot including preparation
and safe shutdown within 30 minutes, off production cores. Require two successful
counterbalanced matched pairs for a performance recommendation.

Only after the four-loop controls pass should the finite five-loop transfer
control in `TMP/codex-runtime-pivot-portfolio.78ueFJ/five-transfer/matrix.json` be
used. Its 14 owners and one 784-point query do not represent the complete 67-root
five-loop request. No production start/stop or campaign-switch recommendation is
authorized by this tooling.
