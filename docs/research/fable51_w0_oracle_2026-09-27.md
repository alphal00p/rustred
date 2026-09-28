# W0.2 closure oracles after the adversarial review (2026-09-27)

This note is the persisted record of plan item W0.2 (gate 0.2) of
`docs/research/fable51_next_push_master_plan_2026-09-27.md` after fix round 2. Round 2 answers every item in
`HANDOFF_opus_5_5.md` §7.1, "Adversarial verification of the oracles", and the orchestrator's course correction
(from `FABLE_5_1_CRITIQUE.md` §2.5 and handoff §0.1 item 6). Fix round 3 (2026-09-28, §11) answers three findings
of the independent verifiers of the round-2 report. The working record is `TMP/w0/oracle/RESULTS.md`.

Labels: **[M]** measured (run directory, report file and binary sha256 are cited); **[E]** estimate or
inference. Nothing here is a closure, termination or ETA claim: `family_closure_claim` stays false in every report.
No campaign under `campaigns/` was started, stopped, signalled or written. No algebraic code was written. The only
algebra the oracles run is the existing Symbolica-backed reducer. The new union-cover predicate is integer interval
bookkeeping, and the round-1 Symbolica search (RESULTS §2) covers it.

## 0. Verdict

**Gate 0.2 passes on the contract of §1** for the complete four-loop run C-4L and for C-5F [M]. Details are in
§4-§7.
- **Calibration.** 10 of 10 drained outputs pass the gate: verdict PASS and every root independently verified,
  with full F10 and native levers off. The paired audits pass on the same `result.json` files.
- **Partial re-inspection.** It is INCOMPLETE, never PASS.
- **FG mutation matrix.** 35 of 35 rows as predicted, including exact class sets. The consistently hidden frontier
  is caught only by the per-node F10 check.
- **C-5F mutation matrix.** 21 of 23 rows as predicted. The other 2 mutations cannot be applied to a run with a
  single initial record.
- **Gen 7, sample mode.** 16 min at 33.6 GB [M]. Full F10 at gen 7 would take about 15 h at 32 threads [E].
- **Not covered.** C-HOT stays audit-only (its state is CP3).
- **Open.** G2' must still wire the new exact union-cover predicate into its own check (§9).

**Fix round 3 (2026-09-28, §11)** answers three verifier findings, all confirmed by code reading:
- **Partial anchors were not required to be well-founded.** A partial anchored on itself, on another partial, or
  a Route partial passed. The verifier now requires an Apply partial anchored on an earlier initial Native
  record, as the Python audit already did. Six new mutations with exact class sets cover it.
- **Route natives had no negative control.** A routed-edge drop, a routed false hit and a Route event miscount
  now have rows. They run on a new in-repo routed fixture and on C-5F.
- **The union-cover validation was overstated.** The "3,580/3,580" real-data figure is the D-cut identity check,
  not a multi-target validation. Genuine multi-target covers are now validated against enumeration on real
  10- and 15-axis cells:
  - on C-4L and C-5F: 84,525 decisions against earlier native records (25,376 covers need 2-8 records) and
    341,124 synthetic decisions;
  - on gen 7: 16,381 real (5,866 union-only) and 66,916 synthetic decisions;
  - 0 disagreements and 0 undecided in total [M].

  The G2' mutations that need G2' records are W4.2 preconditions.

Round-3 calibration with binary `rustred-b0835196` still passes the gate on 10/10 outputs [M].

Commits on `fable_5_1-v3-oracle`:

| Commit | Content |
|---|---|
| `749e3447` | Rust verifier |
| `4395ae41` | Python gate helper, audit pairing and matrix v2 |
| `93094ed3` | Drivers and `--jobs` |
| `d33e2276`, `21c7cb87` | Union-test generator (test-only) |
| this note | |

Verifier binary: `rustred-46d4dd28`.

## 1. Gate contract (what a gate script asserts)

`rustred walk-verify-closure` exposes two top-level fields for gates:

| Field | Meaning |
|---|---|
| `verdict` | `PASS` (exit 0), `FAIL` (exit 1: some violation), `INCOMPLETE` (exit 9: no violation, but re-inspection was partial or none; never a certificate) |
| `roots_total` | distinct root records the queries map to (several queries can share one root) |
| `roots_independently_verified` | roots that are closed (re-derived from the saved edges and the F8 seal rule) AND whose every native was re-inspected by F10; 0 whenever any violation was found |

A gate asserts `verdict == "PASS"` and `roots_independently_verified == roots_total >= 1`. The helper is
`examples/python/assert_oracle_pass.py REPORT.json...`. It exits 0 only when every report passes, 1 when some
report fails, and 2 when a report is unreadable. It also checks the schema and `family_closure_claim == false`.
The equality already implies closure of every root, so the gate also rejects a consistent PASS run without
`--require-closure` whose roots are not all closed. The frontier fixture is such a run: PASS, 60/124.

The Python audit's `--verify-report` pairing applies the same equality when it requires closure. It also binds
the report to the audited `result.json` (canonical path, bytes, mtime, generation, record digests).

`--require-closure` semantics (fix (a)): every root must be closed (a `closure_required` violation otherwise) AND
independently verified. `--reinspect none` or `sample:N:SEED` with `--require-closure` is INCOMPLETE, never PASS,
even over a defect that only F10 sees (dropped edge, injected false hit). This is regression-tested in Rust (§5) and
in the mutation matrix (rows `partial-*`).

## 2. What F10 checks and what it does not

The F10 reference is the walker's own native visitor (`inspection::inspect_reference`). The walk-level levers
are removed: the job-local reuse cache, pre-admitted orthant shortcuts, and initial-overlap planning. For a
partial record, the D < cut residual is inspected and the D >= cut slice is checked against the anchor. Physical
Apply subdivision runs are refused.

Native levers (fix (3) of the course correction): `--reference-levers off` is the default. It turns off every
native shortcut listed in `REFERENCE_NATIVE_LEVERS`. In this tree that is only the opt-in Route joint
source-support pruning. The planned levers N1 (modular zero certificates) and N4 (cover-first) do not exist yet.
Each must be added to that list, and forced off in `reference_request`, when it lands. The report names them
under `reference.native_levers_not_in_tree`. `--reference-levers as-run` keeps the run's native policies. When a
lever that was on in the run is off in the reference, event and successor counts may legitimately differ. Such
count differences are tallied (`count_mismatches_under_disabled_levers`) and are not violations. Error, frontier
and coverage checks stay enforced. Request fields that define the obligations are always kept as run: match
limits and refinement axes, Apply cell refinement, Route overcover, Route mask and native resource allowances.
They are bound in the request digest.

Checked independently of the engine (fix (e)):
- graph bookkeeping: recorded edges, alias and partial-anchor inclusion with their required edges, the F8 seal
  rule against the saved seal flags, closure re-derived by reverse reachability and cross-checked by a forward cone
  per root, the global counters (frontier, native, completed, event);
- coverage: every Admit effect of the reference must be contained, same phase and owner, in a recorded out-edge
  target of its parent, or along that target's alias chain. Admit effects are successor domains plus Apply
  domains routed by Route natives (successor = false);
- per-node parity: Apply natives are checked for error, frontier count, event count and successor count (events
  with successor = true). Route natives are checked for error, frontier count and event count only: Route records
  carry no successor statistic. Their admitted domains are still coverage-checked;
- query-to-root mapping: an admitting query must equal its record, and an absorbed query must be exactly
  contained in it;
- request, owner and result binding: request digest (queries text included), owner payload digests, and
  `result.json` bound record by record to the generation read (fix (d));
- inclusion: `lattice::Cell::contains`, an interval predicate independent of the walker's
  `DomainPowerSummary::contains`. It is cross-checked by lattice-point enumeration on cells with <= 4,096 points,
  within a point budget.

Reproduced, not independent: successor, frontier and problem generation by the native reducer. With no lever
differing, event and successor parity hold by construction for a deterministic visitor. A successor lost inside
the reducer, or an unsound policy kept as run, is reproduced rather than caught. Closure is coinductive: sealed
cycles count as closed, so termination and descent are not certified. The CP5 transport codecs are trusted and
guarded by file digests.

## 3. Exact multi-target cover predicate (course correction (2))

`lattice::Cell::covered_by_union(targets, max_regions)` decides `Q ⊆ T_1 ∪ ... ∪ T_k` exactly:
- It splits `Q \ T_1` into disjoint boxes `Q ∧ c_1 ∧ ... ∧ c_{j-1} ∧ ¬c_j` over T_1's constraints and recurses on
  T_2..T_k.
- Every piece carries interval bounds on the axes and on A, R and D. Its emptiness is decided exactly: A and R are
  sums of integer intervals over disjoint axis groups, so (A, R) fills an integer rectangle, and D = A - R takes
  every integer between its extremes.
- Only the region budget can make the answer undecided (`None`); the answer is never wrong.
- The predicate does not use any C2 code or the walker's summaries.

Unit tests:
- `union_cover_matches_point_enumeration`: 1-3 axes, 5,000 random draws per arity (draws whose Q is not
  enumerable in the window are skipped), 1-4 random targets and, with probability 2/3, a complementary pair split
  strictly inside Q on an axis or in D (with a one-layer gap one time in three). Each answer is compared with a
  point-window ground truth and with the brute-force union cover. A single target must reduce to exact
  inclusion. The test requires more than 200 covers that no single target provides, and more than 200 misses.
  The test passes at `21c7cb87` [M]. A Python port of the generator reproduces an earlier Rust draw exactly
  (4,609 covered, 147 union-only, 621 misses). For this draw it predicts 7,908 covered, 474 union-only and 789
  misses [E]; the test prints its counts only on failure.
- A D-cut example: residual D <= 2 plus anchor D >= 3 covers Q, and neither alone does. Shrinking the residual
  leaves the D = 2 layer open. A foreign-owner anchor covers nothing. The budget gives `None`.

Real-data exercise: every partial record's whole domain must be covered by {anchor, D < cut residual}. The exact
union answer must agree with the slice inclusion. A disagreement is a `partial_union_cover` violation, and the
report counts it under `containment.partial_union_cover`. Results are in §4. **Correction (round 3):** with the
recorded residual equal to the D < cut slice, {anchor, residual} partitions Q along D, so this check is logically
the single-target slice inclusion. The 3,580/3,580 agreement is therefore a D-cut identity check of two
implementations, not a multi-target validation. The unit test above drew only 1-3 axes with bounds below 12.
§11.3 adds genuine multi-target validation on 4-6-axis cells and on real 10- and 15-axis cells.

Scope: this is the predicate G2' needs ("anchor scopes plus residual cover Q exactly"). It is not yet used as a
verdict on anything other than partial records. The brute-force path stays single-target, capped at 4,096 points
per cell; `brute_force_covered_by_union` exists for tests only.

## 4. Calibration round 2 [M]

Binary `TMP/w0/oracle/bin/rustred-46d4dd28` (sha256 `46d4dd286df25fd47ef1ed701aa515ffed26c493aad45f816e947a40f0c6eca7`,
release `--locked --offline`, commit `4395ae41`, identical in non-test code to `d33e2276`). It ran over the round-1
walk outputs `TMP/w0/oracle/runs/`, produced by the control binary `rustred-4a17f9c7`, which is unchanged. Each run
used 24 threads, `--require-closure`, full F10 and native levers off. No calibration run had a native lever on, so
count parity was enforced everywhere. Reports: `TMP/w0/oracle/r2/verify/`, paired audits `TMP/w0/oracle/r2/audits/`,
full table `TMP/w0/oracle/RESULTS.md` §R2.2.

| Case | Verify | Gate | Roots verified / total | Re-inspected | Aliases | Partials | Admitted (succ. + routed) | Successor events | Union cover agree | Verify s | Peak RSS | Paired audit |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| C-4L FG Ordered | PASS | PASS | 248 / 248 | 98,869 | 40 | 160 | 2,182,549 | 2,182,549 | 160 / 160 | 9.5 | 0.23 GB | PASS |
| C-4L BMW Ordered | PASS | PASS | 268 / 268 | 147,233 | 11,718 | 114 | 4,714,148 | 4,714,148 | 114 / 114 | 16.8 | 0.37 GB | PASS |
| C-4L H Ordered | PASS | PASS | 628 / 628 | 24,680 | 249 | 222 | 2,358,487 | 2,358,487 | 222 / 222 | 10.4 | 0.48 GB | PASS |
| C-4L X Ordered | PASS | PASS | 656 / 656 | 46,826 | 367 | 1,135 | 4,117,969 | 4,117,969 | 1,135 / 1,135 | 24.4 | 0.93 GB | PASS |
| C-4L FG Ready | PASS | PASS | 248 / 248 | 98,846 | 40 | 160 | 2,182,273 | 2,182,273 | 160 / 160 | 9.3 | 0.23 GB | PASS |
| C-4L BMW Ready | PASS | PASS | 268 / 268 | 148,185 | 10,997 | 114 | 4,742,216 | 4,742,216 | 114 / 114 | 16.0 | 0.37 GB | PASS |
| C-4L H Ready | PASS | PASS | 628 / 628 | 24,777 | 179 | 272 | 2,358,941 | 2,358,941 | 272 / 272 | 8.6 | 0.48 GB | PASS |
| C-4L X Ready | PASS | PASS | 656 / 656 | 46,826 | 374 | 1,135 | 4,117,969 | 4,117,969 | 1,135 / 1,135 | 19.2 | 0.95 GB | PASS |
| C-5F Ordered W17 | PASS | PASS | 1 / 1 | 967,621 | 305,755 | 134 | 28,406,786 | 25,077,490 | 134 / 134 | 364.5 | 6.28 GB | PASS |
| C-5F Ready W17 | PASS | PASS | 1 / 1 | 981,125 | 294,496 | 134 | 28,823,703 | 25,458,946 | 134 / 134 | 302.6 | 6.30 GB | PASS |
| frontier fixture (FG, 85 frontiers) | PASS (plain) / FAIL `closure_required` 64 | refused | 60 / 124 | 124 | 0 | 0 | 359,994 | 359,994 | - | 1.7 | 0.21 GB | FAIL (frontiers, expected) |

Notes on the table:
- "Re-inspected" counts natives including partials.
- In C-4L, roots are 124/134/314/328 helper plus 124/134/314/328 physics admitting queries (FG/BMW/H/X). In C-5F,
  the root is query `five-loop-finite-publication-r2-a11`.
- Over the 10 drained outputs [M]:
  - 3.89e9 exact `contains()` checks, of which 8.46e7 were positive;
  - enumeration confirmed 7.31e7 inclusions and 3.76e9 non-inclusions, with 0 disagreements;
  - 0 uncovered admitted domains, 0 count mismatches, and 0 undecided union covers.
- C-5F verifier phases (Ordered / Ready) at 24 threads:
  - load 16.3 / 14.5 s;
  - owner preparation 96.3 / 91.8 s;
  - checks 34.9 / 28.7 s;
  - re-inspection 216.7 / 167.3 s.

C-HOT: the extended audit PASSes (audit-only, `independently_verified: false`). There is no edge-based report,
because its state is CP3.

## 5. Regression tests

In-repo, no TMP dependency (fix (c)). File: `verify_closure/e2e_tests.rs`. The fixture is the two-loop sunset:
the family text is in-repo, candidate owners come from `family_candidates` and are split into single-sector
bundles, the Ordered checkpointed walk is built by the production argv parser, and `result.json` is published as
the CLI does.
- **Drained fixture.** Five queries (one absorbed, so four roots) at dispatch lookahead 1. It has 18 natives, 19
  aliases (successors beyond the horizon) and 3 partial records. The partials are anchored on two sub-sector
  D >= 5 queries whose boxes contain the successors' high-D slices.
- **Frontier fixture.** Owner 011 is left out, giving 3 frontiers and 1 of 2 roots closed.
- `drained_sunset_walk_passes_the_gate_with_every_record_kind_and_binding` checks:
  - PASS with and without `--require-closure`, and the gate;
  - query-to-root mapping (5 queries, 1 absorbed, 4 roots; helper vs physics);
  - request and owner digest binding, and result binding (generation, 0 mismatched records);
  - per-node F10 (all natives and partials inspected, event parity enforced, levers off);
  - partial-anchor containment and the union cover (3/3, 0 disagreements), with 0 exact-vs-enumeration
    disagreements.
- `partial_or_no_reinspection_is_incomplete_even_over_a_defect`: `none`, `sample:3:1`, and `none` with a dropped
  edge are each INCOMPLETE with 0 verified roots.
- `frontier_sunset_walk_is_consistent_but_not_closed`: PASS plain, and the gate refuses (1/2). Under
  `--require-closure` the class set is exactly `{closure_required: 1}`.
- `every_mutation_gives_its_exact_violation_classes`: 18 mutation rows with exact class sets and single-node counts,
  plus the alias-chain-detour positive control, which passes through the chain.
  - Drained rows: dropped edge, false hit, retargeted alias and anchor, seal with frontier and with error, hidden
    error, miscounted events and successors, remapped query, foreign request and owners, mismatched result.
  - Frontier rows: dropped frontier record, hidden frontier, seal with frontier, hidden error, miscounted events.
- Unit tests:
  - `union_cover_matches_point_enumeration` and `union_cover_of_a_d_cut_residual_and_its_anchor_and_the_budget`
    (§3);
  - `reference_request_turns_native_levers_off_unless_as_run`;
  - `rows_stream_and_digests_ignore_publication_fields` (result binding);
  - the round-1 lattice, graph, seal and alias-chain tests.
- Python: `test_assert_oracle_pass.py` (3 tests) and the audit pairing test, which now also covers the gate
  equality.

Results [M]:
- Release lib suite at `21c7cb87`: **794 passed / 0 failed / 7 ignored** (round-1 baseline 777/0/6, plus 17 new
  tests; the 7th ignore is the exploration aid). All four e2e tests passed on their first build.
- `cli_routed_campaign`: 6/6.
- Python suite: 234 OK, 1 skipped.
- `cargo fmt --check`: clean.
- Logs: `TMP/w0/oracle/r2/lib-suite.log`, `cli-routed-campaign.log`, `python-suite.log`.

## 6. Mutation matrices [M]

`examples/python/oracle_mutation_matrix.py` v2 checks every row for:
- the exact set of violation classes, and exact counts for single-node defects;
- the exit status;
- `applied` (a mutation that cannot be injected fails its row);
- the closure effect against the plain baseline, since every mutation runs with `--require-closure`;
- the gate, which must pass exactly on the drained PASS rows.

The frontier fixture is pinned at 60/124 closed roots.

**FG** (`TMP/w0/oracle/r2/mutations/matrix-fg.json`): 35/35 rows, `all_ok: true`.

| Defect | walk-verify-closure (exact classes) | Closed roots | Python audit (new violation kinds) |
|---|---|---|---|
| none (drained, plain and required) | PASS; gate PASS | 248/248 | PASS |
| none (frontier fixture) | PASS plain; `closure_required` 64 when required; gate refuses | 60/124 | FAIL (frontiers) |
| `--reinspect none`, `sample:10:1`, none + dropped edge | INCOMPLETE (exit 9), no class, 0 roots verified | 248 | - |
| dropped edge / edge retargeted to a non-container | `successor_uncovered` 1,242 / 1,242 | 248: closure alone misses both | not observable (no edges) |
| retargeted alias / retargeted anchor | `alias_containment` 1 / `partial_anchor` 1 | 248 | alias not contained / - |
| seal with error | `seal_parity`, `error_parity`, `native_counter` 1 each; `false_closure` 325; `closure_required` 8 | 240 | error parity, closed-with-error |
| hidden error (record and counter) | `error_parity` 1 (per-node F10 only) | 248 | not observable |
| miscounted events / successors | `event_parity` 1 / `successor_parity` 1 | 248 | not observable |
| remapped query | `root_mapping` 1 | 247 | closure / query preservation |
| foreign request / foreign owners / mismatched result | `binding` 1 / `binding` 1 / `result_binding` 1 | 248 | - |
| alias-chain detour (positive control) | PASS through the chain; gate PASS | 248 | - |
| dropped frontier record | `frontier_counter` 1, `frontier_parity` 1, `closure_required` 62 | 62 | frontier parity |
| hidden frontier (record and counter) | `frontier_parity` 1 (per-node F10 only), `closure_required` 62 | **62** | nothing new (audit blind spot, asserted) |
| seal with frontier | `seal_parity` 1, `closure_required` 64 | 60 | closed-with-frontier |

The hidden-frontier row is the adversarial case of handoff §7.1. The re-derived closure gains 2 roots, and only
the per-node F10 frontier check catches it, because the global counter agrees by construction. The matrix now
fails if that check stops firing.

**C-5F** (`matrix-c5f.json`, Rust rows, 47.9 min at 4 jobs × 8 threads): **21 of 23 rows as predicted**, with the
same classes as FG and C-5F-scale counts:
- dropped edge and edge false hit: `successor_uncovered` 7;
- seal with error: `false_closure` 119,939, root 1 → 0 closed;
- the alias-chain detour applies and PASSes.

Two rows fail because their mutation cannot be applied (`applied: false`). C-5F has a single initial record, so
there is no other initial native to retarget an anchor to, and no other initial record to remap a query to. Both
kinds are applied and caught on FG and on the in-repo fixture. The frontier rows of the C-5F matrix run on the FG
fixture, because C-5F has no frontiers.

## 7. Gen-7 sample-mode measurement [M] (fix (f))

**Setup:**
- State: a block clone of `TMP/v2-checkpoint-copy-gen7` (v2 campaign CP5 gen 7), 43 GB. The v2 `request.json` was
  only read.
- Command: `walk-verify-closure --checkpoint <clone> --no-result --reinspect sample:10000:1 --require-closure
  --threads 32` with binary `rustred-46d4dd28`, on CPUs 0-15,256-271.
- Guard: the verifier is killed if MemAvailable drops below 120 GiB. The minimum seen was 589 GiB.
- Report: `TMP/w0/oracle/r2/gen7/report-sample10000.json`.

**Scale:** 74,156,033 domains and 1,192,281,291 edges. Records: 27.47M natives, 246 partials, 18.42M aliases and
28.27M unpublished. 67 roots.

**Verdict:** FAIL, only from `closure_required` 59. Oracle and engine agree: 8 of 67 roots and 4,727,095 nodes
are closed. This is a mid-run campaign state, so the FAIL is the correct answer, not a defect. Request and owner
digests match.

**Sample:** 10,000 of 27,465,422 natives (fraction 3.64e-4, which is also the detection probability for a single
defective native):
- 0 uncovered among 2,116,594 admitted domains (1,918,280 successor events);
- 0 parity mismatches;
- partial union cover 246/246;
- 7.46e8 exact checks with 0 enumeration disagreements.

**Cost:**

| Phase | Value |
|---|---|
| Wall | **16 min 19 s** |
| Load | 412.7 s, of which file-digest verification 118.4 s |
| Owner preparation | 107.7 s |
| Checks | 277.6 s |
| Re-inspection | 171.9 s |
| VmHWM | **33.57 GB** |
| RSS after load / after preparation / at end | 22.1 / 28.0 / 24.0 GB |

The peak falls in the checks phase, which builds the reverse CSR for closure [E, for this attribution]. A dry run
with the pre-round-2 binary (16 threads) took 18:02 with 32.8 GB and gave the same counts.

**What this means for W2.6:**
- **Memory is not the problem** [M]. The measured 33.6 GB replaces both the round-1 estimate (40-60 GB) and the
  review's (70-90 GB). The compact 96-byte domain images account for it, and sample mode fits a 1-hour slot at
  gen-7 scale.
- **Full F10 is.** The sample spent 63 ms per native in native-seconds, so all 27.5M natives need about 1.7e6
  thread-seconds, **about 15 h at 32 threads** [E]. A production certificate therefore needs one of:
  - re-inspection sharded across cores or hosts (the reinspect phase is parallel over natives);
  - an incremental F10 over natives new since the last verified generation.
- Per-root cones are not computed at this scale (visit budget 2e10). Per-root independence then reduces to
  "every native re-inspected".
- Sample mode is a smoke test and never a certificate. It returns INCOMPLETE, or FAIL when roots are open.

## 8. Corrections to round-1 statements

These round-1 statements are withdrawn or narrowed (course correction (4) and handoff §7.1):
- "Both oracles pass on every drained output" overstated the case. C-HOT is audit-only: its CP3 `state-*.bin` is
  refused by the CP5 reader, so it has no edge-based re-derivation. Its "certified" counts are engine-claimed
  closure plus exact alias containment.
- "3.8e9 inclusions" is wrong. About 3.8e9 is the number of `contains()` calls in the coverage scan over a
  parent's targets. Most are non-inclusions settled after one lattice point. Positive inclusions confirmed by
  enumeration are on the order of 1e8. The report now separates `exact_checks`, `exact_inclusions`,
  `brute_force_confirmed_inclusions` and `brute_force_confirmed_non_inclusions`.
- "28.4M successors covered" (C-5F) is wrong. That tally counted every Admit effect, including Apply domains routed
  out of Route natives. The successor events were 25.08M (Ordered) and 25.46M (Ready). The tally now reports
  `admitted_domains` = `admitted_successor_domains` + `admitted_routed_domains`, and `successor_events` separately.
- "Frontier, error, event and successor counts must match" held only for Apply natives. Route natives get event,
  frontier and error parity (§2).
- "The exact reducer with every lever off" meant the walk levers only. The native levers are now off by default
  (§2), and F10 does not independently check successor generation.
- The round-1 audit mutation "injected false hit" was a remapped query (a query pointed at a non-containing
  record), caught by query preservation. It is now called `remapped-query`. The audit sees no edges, so it can
  detect neither dropped edges nor successor-level false hits.
- The round-1 "C-5F matrix" ran its frontier rows on the FG frontier fixture (C-5F has no frontiers). This is still
  true and is labelled in §6.
- The round-1 gen-7 estimate of 40-60 GB [E] was low. §7 has the measurement.

## 9. Open items

1. **Union cover beyond partial records.** `covered_by_union` is exact and tested, but it is used as a verdict
   only on partial records. G2' must call it on the real "anchor scopes plus residual" sets. Round 3 added
   enumeration cross-checks on real cells (`--union-sample`, §11.3). The W4.2 preconditions are in §11.5.
2. **N1/N4 in the reference.** Neither lever exists in this tree. When one lands it must be added to
   `REFERENCE_NATIVE_LEVERS` and forced off in `reference_request`, with an e2e test run as-run vs off.
3. **F10 does not check successor generation.** The reference is the engine's own visitor, so a successor lost
   inside the reducer is reproduced. An independent generator (IBP replay on a sample) would be a separate tool.
4. **C-HOT is audit-only.** Its CP3 state cannot be read by the CP5 verifier, and a re-run does not fit the
   1-hour rule (3.7 h at W6 [M, its summary.json]).
5. **Gen-7 scale.** See §7 for what the W2.6 streaming port must do.
6. **Records carry no distinct-edge count.** The F10 "restore checks out-degrees" item needs a CP6 record-schema
   change (W2.2).
7. **Epoch policy.** The verifier and the audit accept ordered/ready. Epoch (semantics 3) support comes with W2.
8. **Coinductive closure.** Sealed cycles count as closed. Termination and descent are out of scope for both
   oracles.

## 11. Fix round 3 (2026-09-28): three verifier findings

The independent verifiers of the round-2 report raised three problems. Each one was confirmed by reading the code
at `63771a50` and fixed on `fable_5_1-v3-oracle`:
- `6e1b76fc`: the Rust verifier, its unit tests and the e2e tests;
- `182211f6`: the matrix and the round-3 drivers `tools/research/oracle/r3/`.

The binary is `TMP/w0/oracle/bin/rustred-b0835196`, sha256
`b08351966802b94163476de2ac134d272e6012e012c8a21e56b625ae54c7c26e`. It was built with release `--locked
--offline` from the tree committed as `6e1b76fc`, plus the uncommitted vendor/symbolica heap-pow patch.

### 11.1 Partial anchors must be well-founded (finding 1: real)

**The hole.** The partial arm of `verify()` accepted three things the Python audit rejects:
- an anchor that is itself a Partial record, because `Node::native()` includes `Kind::Partial`;
- a partial anchored on itself, since nothing required `link < id`;
- a partial whose own phase is not Apply.

Closure is coinductive: a self-edge or a sealed cycle counts as closed. F10 re-inspects only the D < cut residual.
A self-anchored partial, or two partials anchored on each other, therefore left their D >= cut slices inspected by
nobody, and the verifier still gave PASS. The engine never writes such records, because
`walking/initial_overlap.rs` plans partials only for non-initial Apply domains against initial anchors. So no
calibrated PASS was affected [M: the 10 outputs PASS again under the new rules]. The exposure was the W2 epoch
re-implementation and the G2' residual anchors of W4.2.

**The fix** is the new function `check_partial`, with one violation class per rule:

| Class | Rule |
|---|---|
| `partial_phase` | the partial is an Apply record |
| `missing_edge` | the dependency edge to the anchor exists |
| `partial_anchor_order` | the anchor is an EARLIER INITIAL record (`link < id` and `link < initial_count`) |
| `partial_anchor_kind` | the anchor is itself a Native inspection |
| `partial_anchor` | the anchor is an Apply record containing the D >= cut slice |
| `partial_union_cover` | Q ⊆ anchor ∪ recorded residual, decided exactly |
| `partial_residual` | the recorded residual equals the D < cut slice |

`partial_anchor` and `partial_union_cover` are judged only against a well-founded anchor. Against an ill-founded
anchor they would be meaningless, and skipping them keeps the class sets fixture-independent. Together, the order
and kind rules exclude self-anchors, partial-on-partial chains and anchor cycles. The residual identity used to be
checked at load time; it is now checked in `verify()`, so that a mutation can reach it.

**Tests.**
- Unit test `partial_records_need_an_apply_phase_a_well_founded_anchor_and_an_exact_cover` isolates every rule on a
  synthetic record table, including anchor kind alone and order alone.
- New mutations. Each moves the anchor edge with the link and picks partials whose residual successors do not rest
  on the anchor edge alone, so that only the rule under test can fire:

| Mutation | Exact classes |
|---|---|
| `self-anchored-partial` | `partial_anchor_kind` 1, `partial_anchor_order` 1 |
| `partial-as-anchor` (anchor not Native) | `partial_anchor_kind` 1, `partial_anchor_order` 1 |
| `partial-anchor-cycle` | `partial_anchor_kind` 2, `partial_anchor_order` 2 |
| `non-initial-anchor` (a later native when one exists: the ID analogue of "anchor stamp >= dispatch epoch") | `partial_anchor_order` 1 |
| `shrunk-residual` (recorded residual one D layer short) | `partial_residual` 1, `partial_union_cover` 1 |
| `route-partial` | `--reinspect none`: `partial_phase` 1 (the rule alone); full F10 on the sunset: also `event_parity` 1 and `successor_uncovered` 4 |

The pre-existing `retargeted-anchor` now gives `partial_anchor` 1 and `partial_union_cover` 1. All of these are
in `e2e_tests.rs` and in `oracle_mutation_matrix.py`. The Python audit also has rows for self-anchor,
partial-as-anchor, anchor cycle and Route partial, calibrated on FG. It rejects all four.

### 11.2 Route-native negative controls (finding 2: real)

No fixture had a Route native that routes an Apply domain. The drained sunset has only Apply records, C-4L has no
Route records, and the frontier fixture's 3 Route natives route nothing. Every node mutation also picked an Apply
native.

**New in-repo fixture (ROUTED).** It is the frontier walk plus a verified loop-momentum route 011 -> 110, with
source momenta q = M k and M = [[1, -1], [0, 1]], which sends P2 and P3 onto P2 and P1. The walk has 14 Apply and
3 Route natives. Each Route native routes one Apply domain into owner 110. There are 0 frontiers and both roots
close.

New mutations:

| Mutation | Target | Exact classes (routed fixture) |
|---|---|---|
| `dropped-routed-edge` | the only covering edge of a Route native's routed Apply domain | `successor_uncovered` 1 |
| `routed-false-hit` | that edge, retargeted to a same-owner native non-container | `successor_uncovered` 1 |
| `miscounted-route-events` | a sealed Route native's event count, saved counter adjusted | `event_parity` 1 |

On the routed fixture they hit Route native 2 (edge 2 -> 12, retargeted to 3) [M]. The matrix runs them on C-5F
(§11.4). On FG they are "not applicable", decided from the baseline: FG has 0 routed admits.

### 11.3 Union-cover validation, restated and extended (finding 3: real)

**Restated.** The round-2 real-data figure (3,580/3,580 partial records, 246/246 at gen 7) is the D-cut identity
check (§3 correction). The report now labels it so in `containment.partial_union_cover.scope`. The counters report
covered, not covered, undecided, and how many were compared with the slice inclusion.

**Unit test** `union_cover_matches_point_enumeration_on_four_to_six_axes`:
- 4-6 axes with at least two axes in each owner group, 1,500 draws per arity;
- covers made of 2-4 pieces of Q (splits strictly inside an axis range, or in D; a one-layer gap in half of the
  draws), plus 0-2 random distractors, shuffled;
- ground truth: direct evaluation at every window point, which bounds Q; `Cell::meets` is checked the same way;
- required: more than 800 covers that no single target provides, more than 250 covers that no pair provides, and
  more than 400 misses. A Python port of the generator predicts 1,357 / 508 / 766 [E]. The test passes [M].

**Real data** (`--union-sample COUNT[:SEED]`, module `verify_closure/union_sample.rs`). For each sampled saved
cell Q whose box holds at most 65,536 lattice points:
- real covers: earlier Native records of the same (phase, owner) bucket, taken greedily while each covers a point
  of Q the previous ones leave uncovered, up to 8 records. These are genuine multi-record covers and near misses:
  the G2' question;
- synthetic covers: Q split strictly inside its actual point ranges into 2 or 3 pieces along an axis or D, with
  the real records appended as distractors. The same splits are also run with a one-layer gap.

Every decided answer of `covered_by_union` is compared with `brute_force_covered_by_union`. Any disagreement, a
partition that enumeration says is not covered, or a mismatch in the greedy bookkeeping is a `union_cross_check`
violation.

Round 3, 20,000 samples per output [M, `TMP/w0/oracle/r3/calibration-table.md`]:

| Output | Axes | Real decisions | Covered only by a union (max records) | Not covered | Synthetic decisions | Disagreements / undecided |
|---|---:|---:|---:|---:|---:|---|
| C-4L FG Ordered / Ready | 10 | 1,625 / 1,649 | 5 / 10 (2) | 1,620 / 1,639 | 6,500 / 6,596 | 0 / 0 |
| C-4L BMW Ordered / Ready | 10 | 19,463 / 19,483 | 5,744 / 5,639 (8) | 13,719 / 13,844 | 77,852 / 77,932 | 0 / 0 |
| C-4L H Ordered / Ready | 10 | 4,060 / 4,071 | 148 / 156 (8) | 3,912 / 3,915 | 16,240 / 16,284 | 0 / 0 |
| C-4L X Ordered / Ready | 10 | 3,418 / 3,441 | 359 / 355 (3) | 3,059 / 3,086 | 13,672 / 13,764 | 0 / 0 |
| C-5F Ordered / Ready | 15 | 13,631 / 13,684 | 6,552 / 6,408 (8) | 7,079 / 7,276 | 56,116 / 56,168 | 0 / 0 |
| **Total** | | **84,525** | **25,376** | **59,149** | **341,124** | **0 / 0** |

No sampled cell was contained in a single earlier native. This agrees with the G2' falsifier's finding that
admission aliasing already takes every single-anchor cover. The real cells are small: on C-5F, 19,378 of 20,000
sampled cells are enumerable, averaging about 6 points. Enumeration therefore validates the predicate's logic on
real 15-axis shapes, not its behaviour on very large cells. Those rely on the exactness argument of §3 and on the
unit tests. Cost: 0.2-2.6 s per output.

**Gen-7** (`r3/gen7/report-union20000.json`, script `r3/gen7_union.sh`): clone of v2 gen 7 (74,156,033 domains),
`--reinspect none --union-sample 20000:1`, 32 threads [M]:
- Verdict INCOMPLETE by design (no re-inspection, no closure requirement). 0 violations, including the new
  partial rules on all 246 partial records (D-cut checks 246 covered).
- 20,000 sampled; 17,024 enumerable, averaging about 150 points.
- **16,381 real decisions**: 5,866 covered only by a union of earlier natives (at most 8 records), 10,515 not
  covered, 0 covered by one record.
- 66,916 synthetic decisions.
- **0 disagreements, 0 undecided.**
- Union phase 10.0 s. Wall 717 s (load 364 s, owner preparation 90 s, checks 256 s). VmHWM 33.57 GB.
- Grand total over 10 outputs plus gen 7: **100,906 real decisions (31,242 union-only covers) and 408,040
  synthetic decisions, 0 disagreements** [M].

### 11.4 Calibration round 3 and matrices [M]

Calibration: `tools/research/oracle/r3/verify_all.sh rustred-b0835196 24 20000`, run with full F10,
`--require-closure`, native levers off, and the published result.json bound. Reports are in
`TMP/w0/oracle/r3/verify/`; paired audits are in `TMP/w0/oracle/r3/audits/`.
- **10/10 drained outputs pass the gate** (`assert_oracle_pass.py`, exit 0). Roots verified per output:
  248/268/628/656 (C-4L FG/BMW/H/X, Ordered and Ready) and 1/1 (C-5F Ordered and Ready).
- 0 violations of any class, including the new partial rules on all 3,580 partial records.
- 0 uncovered admitted domains. C-5F routed admits: 3,329,296 (Ordered) and 3,364,757 (Ready).
- 0 disagreements between exact inclusion and enumeration.
- C-5F: 344.1 / 343.3 s at 24 threads, 6.28 / 6.29 GB peak.
- Frontier fixture: PASS plain, gate refused (60/124).
- Paired audits: 10/10 PASS and bound to the verifier reports. C-HOT is audit-only (PASS) and the fixture FAILs
  as expected.

Matrices (`TMP/w0/oracle/r3/mutations/`, 4 jobs × 8 threads) [M]:

**FG** (`matrix-fg.json`, 193 s): **48/48 rows as predicted**, `all_ok: true`.
- 16 Python-audit rows. The 4 new partial rows show exactly the calibrated new violation kinds: "partial anchor
  must be an earlier initial record" and "partial anchor N has no initial row" for self-anchor, partial-as-anchor
  and anchor cycle, and 8 kinds for the Route partial.
- 32 Rust rows. Every new partial row has its exact classes, and closed roots stay at 248/248. On FG,
  `partial-as-anchor` anchors 248 on 249, `partial-anchor-cycle` links 248 and 249, and `non-initial-anchor`
  anchors 248 on the later native 367: the genuine "later" case.
- The 3 routed rows are not applicable: the baseline has 0 routed admits, so they were not run.

**C-5F** has 34 rows, all as predicted, split in two runs of under one hour each:
- New rows (`matrix-c5f-new.json`, 26.1 min): **11/11**. Self-anchor, partial-as-anchor, cycle,
  `non-initial-anchor` (partial 7 anchored on native 9), `shrunk-residual` and `route-partial` give the same exact
  classes as on FG. `dropped-routed-edge` and `routed-false-hit` hit Route native 1: its routed Apply domain
  (owner 111000100011101) loses its only covering edge (1 -> 7409), or has it retargeted to native 8103. Each gives
  exactly `successor_uncovered` 1. `miscounted-route-events` on Route native 1 gives exactly `event_parity` 1.
  Closed roots: 1/1 on every row.
- Round-2 rows (`matrix-c5f-old.json`, 31.6 min): **23/23**, with the same classes and counts as round 2:
  `successor_uncovered` 7 for dropped edge and false hit, `false_closure` 119,939 for seal-with-error.
  `retargeted-anchor` and `remapped-query` are now reported as not applicable, decided from the baseline (C-5F has
  1 initial record). They are no longer counted as failed rows.

Routed coverage now has negative controls on real five-loop data, not only on correct data.

### 11.5 W4.2 preconditions (G2' records in the verifier)

The verifier does not read G2' records yet (`g2falsify.report-rev1.json`, open issue 2). Before any W4.2 gate
cites it, it must:
1. Read multi-anchor residual records: the anchor list, each anchor's merge stamp, the dispatch epoch, and the
   residual pieces.
2. Check each anchor edge.
3. Check each anchor kind. Anchors are Native, or, under orchestrator decision 7, merged and validated G2' residual
   records.
4. Check well-foundedness explicitly: every anchor stamp is strictly earlier than the record's dispatch epoch, and
   the anchor graph is acyclic (a strictly earlier id or stamp along every anchor edge).
5. Decide Q ⊆ anchors ∪ residual with `covered_by_union`, as the primary predicate. There is no D-cut identity to
   fall back on.
6. Re-inspect the recorded residual pieces in F10.

The G2' mutation set must then exist with exact class sets:
- residual shrunk by one point;
- anchor stamp >= dispatch epoch;
- anchor not Native (and not a validated residual record);
- anchor cycle among residual records;
- a missing anchor edge.

Their round-3 analogues on today's D-cut partials are `shrunk-residual` (one D layer), `non-initial-anchor`,
`partial-as-anchor` and `partial-anchor-cycle`. Until the list above exists, no G2' run can be certified by
`walk-verify-closure`.

### 11.6 Tests [M]

- `verify_closure` tests: 21 passed, 0 failed, 1 ignored (the exploration aid), on the first build
  (`TMP/w0/oracle/r3/verify-closure-tests.log`). The 4 new tests are the partial-rule unit test, the 4-6-axis
  union test, the routed fixture and the Route partial.
- Release lib suite: **798 passed / 0 failed / 7 ignored** (round 2: 794/0/7) (`r3/lib-suite.log`).
- `cli_routed_campaign`: 6/6.
- Python suite: 234 OK, 1 skipped.
- `cargo fmt`: applied.

## 10. Reproduction

```sh
B=TMP/w0/oracle/bin/rustred-46d4dd28
tools/research/oracle/r2/verify_all.sh $B 24          # -> TMP/w0/oracle/r2/verify/
tools/research/oracle/r2/audit_all.sh                  # -> TMP/w0/oracle/r2/audits/ (paired)
tools/research/oracle/r2/matrices.sh $B all            # -> TMP/w0/oracle/r2/mutations/
tools/research/oracle/r2/gen7_sample.sh $B 10000 32    # clone: .claude/worktrees/fable51-oracle/TMP/gen7
examples/python/assert_oracle_pass.py TMP/w0/oracle/r2/verify/c4l-*.json TMP/w0/oracle/r2/verify/c5f-*.json
cargo test --release --locked --offline -p rustred-app --lib verify_closure   # e2e + unit tests
# round 3 (fix round, binary rustred-b0835196)
B=TMP/w0/oracle/bin/rustred-b0835196
tools/research/oracle/r3/verify_all.sh $B 24 20000   # -> TMP/w0/oracle/r3/verify/ (with --union-sample)
tools/research/oracle/r3/audit_all.sh                # -> TMP/w0/oracle/r3/audits/ (paired)
tools/research/oracle/r3/matrices.sh $B fg           # then c5f-new, c5f-old (each run < 1 h)
tools/research/oracle/r3/gen7_union.sh $B 20000 32   # union sample on the gen-7 clone, no re-inspection
tools/research/oracle/r3/summarize.py                # calibration and union tables
```
