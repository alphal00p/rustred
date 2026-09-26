# Design: physics-scoped inputs, validation ladder, pilots, profiling protocol, launch and monitoring (fable_5_1, 2026-09-26)

File-level plan produced by a planning agent for sections 3.A, 3.D, 3.E and 3.F of `FABLE_5_1_five_loop_vacuum_plan.md`. Packages A, D and E are implemented and committed on `fable_5_1`; the pilots, profiling matrix with the new binary, multi-owner pilot and final launch protocol remain as described here.

I have what I need. Below is the plan.

# Plan: physics-scoped input planner, validation ladder, pilots, profiling protocol, launch and monitoring

## 0. Facts this plan rests on (verified in the tree today)

- Query schema v2 (`crates/rustred-app/src/application/routed_campaign/matching/input.rs:89-199`) rejects unknown fields at both document and row level, so all planner metadata must live in a separate receipt, never in `queries.json`.
- `Domain::is_full_orthant` (`walking/queue.rs:66-74`) requires `powers.is_unconstrained()`: a helper with `max_positive_power` set leaves the `InitialOrthants` fast path (`walking/initial_orthants.rs:24-71`). Rank-only orthants stay on it. This is why helpers default to unbounded A.
- `Queue::admit` aliases a later same-owner request to an already admitted full orthant when `rank_contains` holds (`queue.rs:299-305`); so helpers-first at rank R_max makes every physical root an alias of its owner's helper, exactly as the live run does (`initial_entry_domains_total: 67` for 134 queries).
- Joining `campaigns/five-loop-dependency-closure/inputs/selection.json` (`owners[].representative`) with `examples/input/tide_five_loop_manifest.json` (`representatives[].factorized`, 19 true) and the scratchpad `skeleton_result2.json` gives exactly: 41 non-factorized entry-capable, 18 factorized entry-capable, 7 non-factorized non-entry, 1 factorized non-entry (`101010000110001`). The 8 non-entry masks equal the list in the task. Connected V4min histogram: {0:16, 1:5, 2:13, 3:4, 4:3}; factorized V4min histogram: {1:7, 2:4, 3:4, 4:3}.
- Rust `EntryPowerBudget` supports `min_power_difference` and `max_power_difference` (exact D bands) and one budget per `entry-domain-plan` call (`entry_domain/mod.rs:19-31`, `plan.rs:47-160`); the CLI accepts `--input - --output -`. The `rustred` Python extension is not importable in `nix develop`, so the planner must call the CLI.
- Launcher caps: production `1..50` workers (`production_saved_owner_campaign.py:260`), supervisor `workers + other_workers <= 50` (`shared_owner_campaign.py:402`), CLI `<= 64` (`cli/args/owner_match.rs:405`). `--cpus` is a comma list only in both Python scripts (`production:179`, `supervisor:414`).
- `frozen_policy` (`production:146-204`) hard-codes ordered/H256 and does not freeze `--inspection-workers`; `--publication-policy` is only string-compared against the frozen argv.
- Live run now (status.json, gen 18, ~17.6 h): 10.80M completed, 19.16M pending, 38.56M discovered, 7/67 roots closed, RSS 168.8 GB, checkpoint 68.7 GB in 496 s. Heavy-head dwell (>=10 s) in the first 10 h: `011101110111000` 58.5% of head wall, then `010011111101011` 6.7%, `011001110110100` 2.3%, `111000100011101` 2.1%, `111001100111001` 2.0%, `110010101101011` 1.8% (three of these six are non-entry owners).
- Closed-form check of the physics box for the hot owner (t=9, V4min=3): D=10 box (A<=13, R<=3) has 31,464 tuples; D=9 box (A<=11, R<=2) has 1,000; the same closed form reproduces the control's 1,324 tuples (A<=11, R<=2, D>=9). So an independent Python count is available for the checker.

Generic constants (user-confirmed L=5 numbers written as functions of L, to be confirmed by the parent for other L): connected root at exact D: `R_max = D - L + 1 - V4min`, `A_max = R_max + D` (L=5: D=10 gives 16-V4/6-V4, D=9 gives 14-V4/5-V4). Nested factorized: `A <= 5L-1-V4min`, `R <= 3L-V4min`, `D >= 2L-1` (L=5: 24/15/9; L=4: 19/12/7, matching the four-loop control). Linear-xi gauge adds `+K` to every A_max and R_max (K = xi powers).

---

## 1. Planner package (effort: 2.5-3 days including tests and fixture)

### 1.1 New committed input: `examples/input/tide_five_loop_parent_vertices.json`

Transcribed from `docs/research/tide_five_loop_census.md:150-170` (the four 8-vertex witness lists). Schema `rustred.vacuum-parent-vertices.json.v1`:

```json
{"schema":"rustred.vacuum-parent-vertices.json.v1",
 "role":"offline research input; no solver dispatch",
 "source":"docs/research/tide_five_loop_census.md, section 'Explicit graph witnesses for the four roots'",
 "loop_count":5,"coordinate_count":15,
 "convention":"each vertex lists signed one-based slot indices; signed momentum sum is zero at every vertex; each slot occurs exactly twice with opposite signs",
 "parents":[{"sector_id":32745,"sector_bits":"111111111101001","vertices":[[1,-4,-7],[-1,5,8],[2,-3,-9],[-2,4,10],[3,-5,-12],[-6,7,-15],[6,-8,12],[9,-10,15]]},
            {"sector_id":31740,...},{"sector_id":30699,...},{"sector_id":30527,...}]}
```

The planner validates every parent against the momenta manifest (parse `"k1+k2-k4"` into integer vectors; check zero vertex sums, two opposite occurrences per slot, connectivity, `E - V + 1 == loop_count`, `sector_bits` consistent with `sector_id` under the manifest's `sector_convention`). A test corrupts one sign and expects rejection (mirrors the census's 13 rejected controls).

### 1.2 New script: `examples/python/plan_renormalization_entry_queries.py`

Stdlib only, loaded by path like the other steering scripts. No topology names; loop count and vertex degrees are parameters.

CLI:
```
--loops L                       required
--manifest selection.json       owner masks + representative ids (owners[].mask, owners[].representative)
--momenta MANIFEST.json         .momenta[].{index_one_based,momentum}; .representatives[].{sector_id,factorized}
--parent-witnesses FILE.json    schema above
--gauge feynman|linear-xi       (default feynman)
--gauge-parameter-powers K      (default 0; linear-xi requires K>=1) adds +K to every A_max and R_max
--difference-set 9,10           exact D values for connected/non-entry roots (default "2L-1,2L")
--factorized-roots nested|box|omit   (default nested; the task's --nested-factorized == nested)
--non-entry-roots widest|omit   (default widest: connected formulas at V4=0)
--min-vertex-degree 3 --max-vertex-degree 4
--helper-positive-power-owners MASK,...           add max_positive_power = owner's largest root A_max
--helper-positive-power-owners-from SUMMARY.json  same, read from the matching-diagnostic summary (1.5)
--classification FILE.json      reuse an earlier skeleton-classification.json (skips the ~100 s enumeration)
--classification-fixture FILE   after enumeration, require equality with a committed fixture (regression gate)
--executable rustred            optional; runs `entry-domain-plan --input - --output -` per budget group
--output-directory DIR          must not exist; writes queries.json, entry-plan-receipt.json,
                                skeleton-classification.json, entry-plans/<group>.json
```

Module functions (unit-testable, no I/O):
- `parse_momentum(text, loops) -> tuple[int,...]`, `validate_parent(parent, momenta, loops)`.
- `owner_realization(slots, parents) -> dict[slot,(u,v)]` (contraction of the missing parent edges, from `whitney.py:realization`).
- `whitney_closure(edges) -> set[canonical unlabelled forms of connected 2-isomorphic realizations]` (twists at 2-vertex cuts, cleave at cut vertices, identify vertices across components; from `whitney.py:neighbors`, `canon`).
- `canon_unlabelled(edges)` (individualization-refinement, from `skeleton2.py:5-43`), `cosimplify(n, edges)` (series-class merging), `bridgeless`, `connected`.
- `enumerate_skeletons(loops, min_degree, max_degree) -> iterator of (V4, edges)`: for `V4` in `0..=L-1`, `V3 = 2(L-1) - 2*V4`, backtracking over symmetric multiplicity matrices with loops (from `skeleton2.py:120-168`), canonical dedupe, connected and bridgeless only. Generalize the degree set to `{min..max}` by iterating degree-sequence compositions with `sum((d-2)*n_d) == 2(L-1)`.
- `classify_owners(owner_masks, class_of, skeletons) -> {mask: {t, entry_capable, V4min, skeleton_counts_by_V4}}`.
- `root_bounds(loops, D, V4, xi) -> (A_max, R_max)`; `nested_bounds(loops, V4, xi)`; `coordinate_uppers(mask, A_max, R_max)` = `A_max - t` on active axes, `R_max` on inactive axes (refuse if `A_max - t < 0` or the exact-D band is empty: `A_max < t` or `A_max - D < 0`).
- `plan_queries(classification, factorized_flags, options) -> (document, receipt_rows)`; IDs: `phys-d10-a16-r6-<mask>`, `phys-d9-a14-r5-<mask>`, `nested-d9p-a21-r12-<mask>`, `conv-d10-a16-r6-<mask>`, `conv-d9-a14-r5-<mask>`, helpers `owner-anchor-r<R>-a<none|A>-<mask>` (same prefix as `stage_saved_owner_campaign.plan_query_order`, so `--prepare-from`'s default helpers-first reorder is a stable no-op). Order: selection owner order; within an owner the helper first, then roots by descending R_max (D=10 before D=9).
- `budget_groups(rows) -> [(EntryPowerBudget dict, [masks])]` and `entry_domain_counts(executable, groups)`; spec per group: `{"schema":"rustred.entry-domain.json.v1","budget":{"max_positive_power":A,"max_numerator_rank":R,"min_power_difference":Dmin,"max_power_difference":Dmax|null},"sectors":[...],"max_positive_layers_per_sector":A+1,"max_preview_targets":8}`. Expected 14 calls for the 67-owner case (5 D=10 groups + 5 D=9 groups, non-entry owners merged into the V4=0 groups, 4 nested groups).

Determinism: sorted iteration everywhere, canonical forms are tuples of ints (hash randomization does not affect them), JSON written with `sort_keys=True, indent=2`; the receipt records `queries_sha256` and the planner's argv, git HEAD and input SHA-256s. Two runs must give byte-identical `queries.json` (test).

Runtime expectations (estimated from the scratchpad run, to be measured and recorded in the receipt's `skeleton_enumeration.elapsed_seconds`): whitney closure for 67 owners ~10-20 s; skeleton enumeration V4=4..0 ~100 s at L=5; entry-domain-plan calls ~1 s each. With `--classification` reuse the whole planning is seconds.

### 1.3 Output files and schemas

`queries.json`: schema `rustred.owner-domain-queries.json.v2`, 183 rows for the 67 owners (67 helpers + 41x2 + 8x2 + 18x1), no extra fields, estimated ~130 kB (well under the 1 MiB default; production passes exact count/bytes anyway).

`entry-plan-receipt.json`, schema `rustred.renormalization-entry-plan.json.v1`:
```
generated_unix_time, planner{script, git_head, arguments},
inputs{manifest{path,sha256}, momenta{...}, parent_witnesses{...}, classification_fixture|null, executable{path,sha256}|null},
physics{loops, gauge, gauge_parameter_powers, difference_set, difference_mode:"exact", factorized_roots, non_entry_roots,
        min_vertex_degree, max_vertex_degree, formulas{connected, nested_factorized, non_entry, coordinate_uppers},
        descendant_clipping:false, family_closure_claim:false},
skeleton_enumeration{skeletons_by_V4, labelled_leaves_by_V4, unknown_classes_by_V4_t, elapsed_seconds},
owners[ {mask, representative, t, factorized, class: connected|factorized|non_entry, V4min, skeleton_counts_by_V4,
         roots[{id, D_min, D_max, A_max, R_max, active_upper, inactive_upper, target_count}],
         helper{id, max_numerator_rank, max_positive_power}} ],
summary{owner_count, classes{connected,factorized,non_entry}, query_count, helper_count, root_count,
        queries_sha256, queries_bytes, total_target_count, budget_groups[{budget, sectors, plan_sha256, total_target_count}]},
helper_positive_power{owners[], source: null|{path,sha256}}
```
`skeleton-classification.json`, schema `rustred.vacuum-skeleton-classification.json.v1`: per-mask `{t, entry_capable, V4min, skeleton_counts_by_V4}` plus the enumeration totals; this is also the committed fixture format.

### 1.4 Independent checker: `examples/python/check_renormalization_entry_queries.py`

Does not import the planner. Inputs `--queries --receipt --manifest [--momenta]`. Re-derives from the receipt's `physics` block and each owner's `(class, V4min, t)`: A_max/R_max/D band per root, coordinate uppers, helper rank = max root R_max, helper A = largest root A_max only for listed owners; asserts ID uniqueness and <=128 bytes, byte size, helpers-first order per owner in selection order, that every owner in the manifest has exactly one helper and the right root count (2/1/2/0 by class and options), that each root is semantically contained in its helper under the `queue.rs` predicate, `queries_sha256` equals the receipt. Membership probes like `TMP/full-jet-symbolic67.CwBp4L/check-input.py`: every point of the D=9/D=10 boxes for small owners, boundary probes on both sides of A_max, R_max, D, and 1,000 seeded random points per query, comparing the box predicate with the direct physical predicate. Independent exact counts: closed form `sum_A C(A-1, t-1) * C(R+m-1, m-1)` over the D band (m inactive axes) must equal the receipt's `target_count` from Rust (validated above on the 1,324 control). Prints a JSON summary; nonzero exit on any mismatch.

### 1.5 Matching-diagnostic summarizer: `examples/python/summarize_owner_domain_match.py`

Streams a matching-only `result.json` (top-level `all_queries_locally_applicable`, `counts{selected_rule, terminal, exact_zero_sector, exact_gap, unresolved, invalid_source_condition}`, `queries[]` with `classification_complete`, status, unresolved pieces) using the bounded `Stream` decoder from `TMP/five-loop-anchor-diagnostic.2rBzX9/audit_complete_stream.py` (promote it into the script). Emits `matching-summary.json`: per-query status, unresolved piece counts, and `helper_positive_power_owners: [masks of helpers with unresolved pieces]`, which the planner accepts via `--helper-positive-power-owners-from`.

### 1.6 Tests (`examples/python/test_plan_renormalization_entry_queries.py`, unittest, no native binary)

- Hand-known L=2 family (momenta `k1, k2, k1-k2`; parent vertices `[[1,-2,-3],[-1,2,3]]`): mask `111` connected, V4min 0, `skeleton_counts_by_V4 == {0:1}`; mask `110` factorized, V4min 1 (figure-eight), non-entry with `--min-vertex-degree 3 --max-vertex-degree 3`.
- Hand-known L=3 family (6 propagators, K4 parent): `111111` connected V4min 0; the 4-edge banana sector V4min 2; a 5-line sector V4min 1; product sectors classified via the factorized flag.
- Bound formulas: L=5 tables for V4 in 0..4 equal the user-confirmed numbers; L=4 nested equals 19/12/7; xi powers shift both bounds; empty-band refusal.
- Query document: field set exactly the v2 fields; ID format and order; helper containment; `--helper-positive-power-owners` only affects the listed helpers; byte-identical output across two runs; fake `entry-domain-plan` executable (shell script echoing a canned plan) exercises grouping and receipt counts.
- Parent-witness validation rejects a flipped sign, a missing slot, wrong loop count.
- L=5 regression: `examples/python/fixtures/tide_five_loop_skeleton_classification.json` committed (regenerated by the planner from the committed witness file, checked equal to the scratchpad `skeleton_result2.json` field by field before committing). Fast test: plan with `--classification fixture` and assert the 41/18/8 split, 183 queries, the V4min histograms above. Slow test (skipped unless `RUSTRED_SLOW_TESTS=1`): full enumeration equals the fixture (~100 s).
- Checker tests: a mutated bound, a reordered helper, a wrong count, and a stale sha each fail.

Run: `nix develop --command python -m unittest discover -s examples/python -p 'test_*.py'`.

---

## 2. Validation ladder before any campaign (effort: 1 day of work, ~1-2 days of wall time)

All steps under `TMP/qcd-feynman-d9d10-input.XXXXXX/` (mkdtemp), each with `command.json`, GNU `time -v` output, and `RESULTS.md`.

(a) Plan: run the planner with `--executable target/release/rustred` (SHA `32fdec...`, identical to the frozen live binary) and `--classification-fixture`. Run the checker. Record `entry-plan-receipt.json` counts (this satisfies (b)).

(b) Matching-only diagnostic on all 183 queries: `match_shared_owner_domains.py` with the argv pattern of `TMP/five-loop-anchor-diagnostic.2rBzX9/command.json` (`--max-total-pieces 1000000 --max-guard-univariate-degree 64 --bounded-refinement-axes finite-axes --no-progress`, per-query allowances `18446744073709551615`, `--max-queries 183 --max-query-bytes <bytes>`), pinned with `taskset -c 192-197`, no `--follow-successors`. Expected ~90 s preparation + 60-110 s matching, ~8 GB RSS (measured on the 134-query diagnostic; the new roots are smaller). Accept only: `status` locally_applicable for every query, `counts.exact_gap == counts.unresolved == counts.invalid_source_condition == 0`, `all_queries_locally_applicable == true`. If helpers show unresolved pieces (the old diagnostic had 62 pieces on 7 owners at R15/no-A; at R<=6 this is unknown), re-plan with `--helper-positive-power-owners-from matching-summary.json`, re-run the diagnostic on the changed helpers only, then once more on the final document bytes. The final staged bytes must be the ones that passed.

(c) Single-owner pilot (`011101110111000`, t=9, V4min=3): queries = its helper (R<=3, A unbounded or A<=13 if (b) required it) + `phys-d10-a13-r3` + `phys-d9-a11-r2`; selection unchanged (all 67 programs stay for routing). Runs, sequentially on CPUs 192-197, W=6, frozen `32fdec` binary, supervisor RAM guard 100 GB / 20 GB host reserve, 4 h checkpoint interval, no time or work cap: Ordered then Ready. Compare against the 1,324-tuple control (`TMP/ready-five-loop-finite-w50.a6ABXd/metrics.json`: 967,621 native inspections, max scheduled finite rank 6, all obligations discharged) and the old-envelope behaviour (rank staircase 15->21, A caps 25-31, pending growth 1.4-3.7 per completion). Record: native inspections by phase, traversal/preparation seconds, max scheduled finite rank, peak RSS, pending growth per completion, whether the walk drains (zero pending, zero frontiers, ledger discharged per the audit in 3.2). Stop criteria: drains, or the RAM guard fires, or pending exceeds 20M IDs (then record "not closed under the guard"; this is a scope signal, not a scheduler signal). Third leg only if the first two drain: same input with the helper omitted (roots admitted directly, no fast path) to measure the helper's cost.

(d) Multi-owner pilot: the six hot owners above (three connected, three non-entry), their planner rows only, all 67 programs. W=24 Ready (`--inspection-workers 12`, default split), CPUs 200-223, RAM guard 100 GB, 4 h checkpoint interval, cooperative SIGINT at ~6 h, launched through `production_saved_owner_campaign.py --campaign-directory TMP/.../pilot-campaign` so it could be promoted by a resume with a larger guard. Matched metrics computed by the same script (`examples/python/heartbeat_metrics.py`, section 5) from the pilot's and the live run's `events.jsonl` at equal elapsed time: completions/h, no-completion stall share at >=5 s and >=20 s, mean computing inspectors, pending growth per completion, RSS per discovered domain, max scheduled finite rank, roots closed. Stop criteria: 6 h elapsed, drain, guard, or pending > 30M.

Decision thresholds for both pilots (relative to the live stream at matched elapsed, all measured): go if completions/h >= 1.5x, stall share (>=5 s) < 40% (live 62%), pending growth per completion below the live window's minimum (1.4), max scheduled finite rank <= helper rank + 7 (the old run's observed escape was +7 A / +6 R above its anchors), no frontiers or errors. Hold if 1.0-1.5x with no rank improvement. Stop and revisit scope if pending grows faster than the live run at matched elapsed.

---

## 3. Profiling protocol for the engine packages (harness effort: 1.5-2 days; matrix wall time ~2 h)

### 3.1 Committed harness: `examples/python/walk_control_matrix.py`

Input: a matrix JSON (`rustred.walk-control-matrix.json.v1`) listing cases `{name, executable, manifest, queries, owner_base, workers, cpus:"192-197", publication_policy, inspection_workers|null, native_options[], max_memory_bytes, checkpoint_interval_seconds}`. For each case, sequentially and never overlapping: create `<evidence>/<case>/`, write `command.json`, run `taskset -c CPUS /usr/bin/time -v -o native.time python examples/python/shared_owner_campaign.py ... --run-directory <case>/run --no-progress` with the supervisor RAM guard, then extract `summary.json`: whole-command wall/CPU/max RSS (GNU time), preparation and traversal seconds, native inspections by phase, aliases, events, max scheduled finite rank, checkpoint seconds, sampled peak RSS (from `resources.jsonl`), exit status. Then run the audit (3.2) and the records comparison (3.3). Finally write `RESULTS.md` with the table (numbers only) and a `matrix-receipt.json` listing executable SHA-256s and input SHA-256s. Refuses to run if another case's native PID is alive or the requested CPUs are not in the affinity mask.

### 3.2 Committed audit: `examples/python/audit_owner_domain_walk.py`

Promotion of `TMP/ready-five-loop-finite-w50.a6ABXd/audit.py` plus the streaming decoder: every logical record streamed; aliases resolve to a same-phase same-owner completed native representative; Apply/Route stats checked by their own semantics (`problems == 0`, `missing_routes == 0`, successor sums); queue/ledger/pool drained; zero frontiers; initial and partial-anchor obligations discharged; input queries preserved. Output `audit.json`; nonzero exit on any violation. Test with a tiny synthetic `result.json`.

### 3.3 Records comparison: `examples/python/compare_walk_records.py`

Modes: `strict` (Ordered old vs new binary: identical completed record geometry, native/guard/dependency counters and outcomes apart from `seconds`, checkpoint bookkeeping and scheduling diagnostics, as in the four-loop repeat pass) and `multiset` (Ready or cross-policy: equal multisets of (phase, owner, lower, upper, rank, power_bounds, outcome) and equal native counts within a stated tolerance, since IDs and fragmentation may differ).

### 3.4 Matrix

| Case group | Inputs | Workers/CPUs | Policies | Binaries |
|---|---|---|---|---|
| FG, BMW, H, X | `TMP/four-loop-region-control.eazKG2/<family>/selection.json` and the helper-first queries in `TMP/four-loop-helper-order.Iy9nQt/inputs/` (FG) / `TMP/four-loop-saved-descendants.VaNmUN/<family>/queries*.json` (successful covers) | W6 on 192-197 | Ordered, Ready | old `campaigns/five-loop-dependency-closure/bin/rustred-32fdec...`, new |
| five-loop finite control | `TMP/ready-five-loop-finite-w50.a6ABXd/queries.json` (1,324 tuples) with the dependency-closure inputs | W50 on 192-241 (25/24/1) | Ordered, Ready | old, new |

16 four-loop runs (each 15-40 s plus a few seconds of preparation, measured in the docs) and 4 five-loop runs (~5-7 min each plus ~90 s preparation): about 1-1.5 h. Fixed order per pair: old Ordered, new Ordered, old Ready, new Ready, and a repeat of the FG pair for reproducibility. Reference values to reproduce before trusting the harness: FG 98,869 / BMW 147,233 / H 24,680 / X 46,826 native inspections (Ordered), five-loop control 967,621 (Ordered) and 982,498-985,839 (Ready).

Evidence: `TMP/engine-profiling-2026-09-2X.XXXXXX/` with `RESULTS.md`, `INDEPENDENT_AUDIT.md` (written by a different agent than the harness author), `bin/` frozen copies of both binaries named by SHA-256, per-case directories. Document: `docs/research/five_loop_engine_profiling_2026-09-2X.md` (whole-command wall, traversal, CPU, peak RSS, native inspections, audit and comparison outcomes per case; "shared host, sequential, not a statistical benchmark"). The scheduler and checkpoint packages each cite this document; a new binary is eligible for the campaign only if all 20 audits pass, strict comparison passes for Ordered, and no case is slower than the old binary by more than the run-to-run spread observed in the FG repeat.

---

## 4. Launcher and supervisor changes (effort: 1.5 days including tests)

### 4.1 `examples/python/production_saved_owner_campaign.py`

- `frozen_policy`: bump to `rustred.production-steering.v2`; `names` gains `publication_policy` (default `ordered`), `transfer_unreserved_lookahead` (default 256), `inspection_workers` (default None = native split), `checkpoint_interval_seconds` stays. v1 policies remain readable: missing options mean the v1 defaults, and `--publication-policy` keeps its argv cross-check. The `command` list is built from `options` (no hard-coded `"ordered"`/`"256"`), adding `--inspection-workers N` when set. Resume refuses changes to any of these (RAM options remain the only per-resume overrides).
- Worker validation `1..64` (matching the CLI cap); `--inspection-workers` validated with `DOMAIN.validate_inspection_workers` semantics (leave one coordinator).
- `--cpus` accepts `128-177` and mixed lists via a shared `parse_cpu_set(text) -> set[int]` (add to `shared_owner_campaign.py`; production imports it by path as it does the stager). Frozen `options["cpus"]` stays the sorted comma list for the supervisor.
- `--prepare-from SOURCE --queries NEW.json [--attach FILE ...]`: `prepare_from` gains `queries_override` and `attachments`; `stage()` gains `attachments=()` which copies each file read-only into `inputs/` and records `{name, path, bytes, sha256}` under `receipt["attachments"]`; the identity check compares the staged source-query sha with the override's sha. Verification of the override before copying: schema v2, every `owner` is a selection mask, no unknown fields (a Python-side mirror of `input.rs` field sets, not authority).
- Plan output gains `entry_plan_receipt` (path + sha) and the frozen v2 options.

### 4.2 `examples/python/shared_owner_campaign.py`

- `--cpus` ranges through `parse_cpu_set`; aggregate cap `workers + other_workers <= 64`; `restart_command` already replays all options.
- New derived-metrics block in `publish_status` (section 5).

### 4.3 Tests

Extend `test_campaign_monitor.py::ProductionTests` and `test_shared_owner_campaign.py`: v1 steering read as v2 defaults; `--publication-policy ready --inspection-workers 25 --transfer-unreserved-lookahead 256 --checkpoint-interval-seconds 14400` frozen and refused on resume; `--cpus 128-177` produces exactly 50 CPUs and rejects `128-176` for 50 workers; 64 accepted, 65 rejected before any file access; `--prepare-from ... --queries` copies payloads from the source, uses the new document, records attachments, and refuses an override whose owner is not in the selection; fake executable never runs.

### 4.4 Campaign directory and commands

Campaign directory: `campaigns/five-loop-qcd-feynman-d9d10`.

Prepare (run from a normal shell, not the Zellij tab; no launch):
```sh
cd /common/dev/rustred && export TMPDIR="$PWD/TMP" TMP="$PWD/TMP" TEMP="$PWD/TMP"
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --prepare-from campaigns/five-loop-dependency-closure \
  --queries TMP/qcd-feynman-d9d10-input.XXXXXX/queries.json \
  --attach TMP/qcd-feynman-d9d10-input.XXXXXX/entry-plan-receipt.json \
  --attach TMP/qcd-feynman-d9d10-input.XXXXXX/skeleton-classification.json \
  --campaign-directory campaigns/five-loop-qcd-feynman-d9d10 \
  --executable <frozen new binary, or target/release/rustred if the engine packages are not accepted> \
  --workers 50 --cpus 128-177 --publication-policy ready --inspection-workers 25 \
  --transfer-unreserved-lookahead 256 --checkpoint-interval-seconds 14400 \
  --max-memory-bytes 700000000000 --ram-guard-margin-percent 5
```
Then run the checker against `campaigns/five-loop-qcd-feynman-d9d10/inputs/queries.json` and the attached receipt (staged bytes must equal the diagnosed bytes).

Launch, typed into the `fable_5_1` tab (shows the live monitor because the supervisor owns the terminal):
```sh
cd /common/dev/rustred && nix develop --command python examples/python/production_saved_owner_campaign.py --campaign-directory campaigns/five-loop-qcd-feynman-d9d10 --start
```
Injection from outside:
```sh
export XDG_RUNTIME_DIR=/run/user/1125
zellij --session rustred action go-to-tab-name fable_5_1
zellij --session rustred action write-chars 'cd /common/dev/rustred && nix develop --command python examples/python/production_saved_owner_campaign.py --campaign-directory campaigns/five-loop-qcd-feynman-d9d10 --start'
zellij --session rustred action write 13
```
Read-only monitor (any shell):
```sh
nix develop --command python examples/python/campaign_monitor.py campaigns/five-loop-qcd-feynman-d9d10/runs/<RUN>   # add --once or --json for snapshots
```
Preconditions recorded in the launch record: the live campaign on CPUs 0-9 untouched; pilots on 192-241 finished or stopped; `zpool status` checked; host available RAM > 700 GB + 20 GB reserve.

### 4.5 Interfaces I need from the other packages

- Scheduler/admission: heartbeat `progress.snapshot.parallel.computing_workers` (inspectors executing native code, distinct from `active_workers` and `backpressured_workers`); `admission_preparation.commit_wall_seconds` (policy-independent, alongside the existing `preparation_wall_seconds`/`ordered_commit_wall_seconds`); `descendant_closure.initial_roots: [{query_ids, owner, closed}]` (67 entries); any new native flags named so they can be whitelisted in `match_shared_owner_domains.py` option lists and frozen through steering v2; the CLI worker cap if W>64 is wanted.
- Checkpoint/memory: heartbeat `checkpoint.bytes` and `duration_seconds` already exist; add `records_sidecar_bytes` (or equivalent) and any new flag (e.g. a records-streaming switch) with the same whitelisting/freezing route; confirm the Ready CP4 resume-to-exhaustion gate result before launch.

---

## 5. Monitoring (effort: 1 day)

### 5.1 Supervisor-derived metrics (`shared_owner_campaign.py::publish_status`)

Keep a bounded deque (last 2 h at 2 s samples) of `(elapsed, completed_nodes, queued_nodes, currently_discovered_nodes, process_rss_bytes, max_scheduled_finite_rank, prep+commit seconds, checkpoint durations)`, read from the heartbeat that `EventTail` already parses. Publish `status["derived"]` (schema `rustred.campaign-status.v1` unchanged, new optional block):
`completions_per_hour_1h`, `stall_share_5s`, `stall_share_20s` (fraction of wall in heartbeat intervals with zero completion delta and length >= 5/20 s; same definition as the scratchpad `heads.py`), `pending_growth_per_completion_1h`, `rss_bytes_per_discovered_domain`, `coordinator_duty_1h` (delta prep+commit / delta wall), `checkpoint_duty` (sum of save durations / elapsed), `computing_inspectors_mean_1h` (from the new field; null until provided), `max_scheduled_finite_rank`, `roots_closed`, `roots_total`, `last_checkpoint{generation, bytes, duration_seconds}`. Also add a committed offline `examples/python/heartbeat_metrics.py` that computes the same numbers from any `events.jsonl` window (used for the pilot-vs-live comparison; tested on a synthetic stream).

### 5.2 Dashboard lines (`campaign_monitor.py::dashboard`)

Add three lines, all reading `derived` and falling back to "unknown": 
`Inspectors <computing> computing / <inspectors> reserved · stall >=5 s <x>% · coordinator duty <y>%`, 
`Rate <completions/h> per hour · pending +<p> per completion · max scheduled rank <r> · RSS <k> KB per domain`, 
`Checkpoint gen <g> · <GB> in <s> s · duty <d>% · roots closed <c>/<n> (helper roots; physical roots are aliases)`. Tests: fields absent -> "unknown"; no ETA; closure bar still driven only by native closure counters.

### 5.3 Checklist and thresholds

- First 10 min: preparation ~90 s (measured on this manifest); `initial_entry_domains_total == 67`; zero frontiers/errors; `native.stderr` empty; worker reservation 25/24/1.
- Hours 1-6 (compare with the live stream at equal elapsed via `heartbeat_metrics.py`): completions/h >= 1.5x live; stall share (>=5 s) < 40%; computing inspectors >= 3 (live 1.1-1.3); pending growth per completion < 1.4; max scheduled finite rank <= 13 (live reached 21); RSS per domain <= 4.5 KB (old binary) or the checkpoint package's measured figure (new binary); first checkpoint at 4 h: bytes and seconds recorded, duty < 10%.
- Day 1-3: roots closed strictly increasing (live: 7/67, flat since hour 6); cumulative pending/completed ratio decreasing over >= 3 consecutive hours (live rose 3.13 -> 3.55); RSS trajectory extrapolated to the 665 GB save-and-stop point > 7 days; coordinator duty < 50% (if higher, the coordinator is the ceiling and adding workers will not help); every checkpoint save shorter than 10% of the interval.
- "Better odds than the old run" is declared only when, at matched elapsed, closed roots exceed the old run's count, pending per completion is lower and falling, and the rank cap stays flat; never on completions/h alone (the live hourly rate swung 16-685/s under one policy).
- Escalation: if pending growth per completion exceeds the old run's at matched elapsed for 3 h, or rank climbs past helper+7, stop cooperatively (Ctrl-C in the tab; checkpoint kept) and revisit scope before any resume.

---

## 6. Documentation (effort: 0.5-1 day)

- `docs/research/five_loop_qcd_feynman_entry_class_2026-09-26.md`: physics class as user-confirmed (Feynman gauge, degree <= 4 vertices, auxiliary-mass tadpole scheme, D in {9,10}, nested factorized, non-entry convenience roots, descendants never clipped), the 67-row classification table, planner/checker/diagnostic receipts with SHA-256s, pilot results, launch record. Every number tagged measured (with run directory and binary SHA) or estimated (with basis); no ETA; `family_closure_claim: false` language retained.
- `docs/research/five_loop_engine_profiling_2026-09-2X.md` (section 3).
- Updates: `examples/python/README.md` (planner, checker, summarizer, harness sections), `docs/shared_owner_campaign_driver.md` (steering v2 options, CPU ranges, `--prepare-from --queries`, the new recipe), `docs/finite_starting_domains.md` (physics-scoped class replaces the A24/R15/D>=9 envelope for this campaign, with a pointer to the decision), `GOAL.md` (dated follow-up recording the user's envelope decision; user-owned text).
- `FABLE_5_1_five_loop_vacuum_plan.md` (repo root, new): sections Scope and physics decision; Work packages (this one, scheduler/admission, checkpoint/memory) with owners; Interfaces between packages (4.5); Evidence ledger (table: claim, measured/estimated, source path, SHA); Validation ladder status; Launch record (exact commands, binary SHA, input SHAs, run directory); Monitoring thresholds (5.3) and decision log with UTC timestamps; Open risks (termination not established, untested large restore, ZFS errors, floating gammaloop job). Conventions: UTC dates, absolute repository paths, TMP evidence directories named `TMP/<purpose>.XXXXXX` with `RESULTS.md` and `INDEPENDENT_AUDIT.md`, no numbers without a source, no ETA claims.

## Ordered sequence and effort summary

1. Witness file + planner + checker + tests + fixture (2.5-3 d).
2. Summarizer + matching diagnostic + re-plan loop (0.5 d work, ~10 min per diagnostic).
3. Launcher/supervisor/monitor changes + tests (1.5 d) — needed before pilots (d) so the pilot can be promoted.
4. Harness + audit + comparison scripts (1.5-2 d), then the profiling matrix (~1.5 h wall) once the other packages deliver binaries.
5. Pilots (c) then (d) on CPUs 192-241 (1-2 days wall, mostly waiting).
6. Prepare `campaigns/five-loop-qcd-feynman-d9d10`, checker on staged bytes, launch in `fable_5_1`, monitor per 5.3 (0.5 d).
7. Docs and plan file updated at each gate (0.5-1 d total).

### Critical Files for Implementation
- /common/dev/rustred/examples/python/plan_renormalization_entry_queries.py (new; algorithm from /tmp/claude-1125/-common-dev-rustred/70d080f7-49ac-475d-bb3b-4c9f29d8395b/scratchpad/skeleton2.py and whitney.py)
- /common/dev/rustred/examples/python/production_saved_owner_campaign.py
- /common/dev/rustred/examples/python/shared_owner_campaign.py
- /common/dev/rustred/examples/python/campaign_monitor.py
- /common/dev/rustred/examples/python/stage_saved_owner_campaign.py
