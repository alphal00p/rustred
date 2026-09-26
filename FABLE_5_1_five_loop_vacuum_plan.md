# FABLE_5_1 plan: a five-loop vacuum parametric-IBP closure campaign that can finish

Branch `fable_5_1`. Written 2026-09-26 (UTC) at the start of implementation and
assigned as the governing goal of this branch. `GOAL.md` remains the historical
authority for the mathematical scope; where this file narrows the required
entry class for the new campaign, it records the user's explicit decision.
Evidence discipline is unchanged: measured numbers cite a run directory and a
binary SHA; estimates say so; no ETA is ever claimed; `family_closure_claim`
stays false until the ledger drains.

## 1. Context and objective

The live campaign `campaigns/five-loop-dependency-closure` (10 workers, Ordered
publication, A<=24/R<=15/D>=9 roots absorbed by R<=15 orthant helpers) uses
~1.8 of 10 cores, serializes heavy Apply inspections at the ordered head,
grows its pending queue every hour (10.8M completed / 19.2M pending / 38.6M
discovered at 17.6 h), holds ~4.5 KB of RAM per discovered domain (168 GB, 60%
of it write-only report records) and writes 69 GB JSON checkpoints in ~500 s.
It will hit its RAM guard long before it can close the class it was given. The
audit of 2026-09-26 (report in `docs/research/five_loop_completion_levers_2026-09-26.md`,
to be added) identified the levers; the user asked to implement all of them.

Objective: launch, in the `fable_5_1` tab of Zellij session `rustred`, a fresh
five-loop campaign over the same 67 saved owners whose required roots cover
exactly the class needed for complete five-loop QCD renormalization, with a
walker that (a) keeps all reserved cores busy, (b) does not retain write-only
state in RAM, (c) checkpoints in seconds not minutes, and (d) can be upgraded
in place with performance-only binaries. The live campaign is left untouched;
the user stops it themselves when the new one shows better odds.

Non-goals for this branch: Vakint wiring, master minimization, AMFlow-grade
numerics, rule regeneration. Ordered publication and the original A24/R15/D9
inputs must remain runnable (fresh) with the new binary.

## 2. Physics decisions (user-confirmed 2026-09-26)

Definitions: A = total positive (denominator) power, R = total ISP/numerator
power (each ISP has mass dimension 2), D = A - R. Dimension of a five-loop
vacuum integral is 20 - 2A + 2R, so log-divergent coefficients have D = 10 and
the dimension-2 (p^0 gluon self-energy) term has D = 9. V4 = number of quartic
vertices of a realization; t = active line count of an owner.

- Gauge: Feynman. All Z's via gluon/quark/ghost self-energies and the
  ghost-gluon vertex with one external momentum nullified, in the
  auxiliary-mass (all lines massive) tadpole scheme.
- Connected entry owners (41): roots at exact D = 10 with A <= 16 - V4min,
  R <= 6 - V4min, and at exact D = 9 with A <= 14 - V4min, R <= 5 - V4min.
- Factorized entry owners (18, matroid direct sums): kept, with the nested
  full-jet bounds A <= 24 - V4min, R <= 15 - V4min, D >= 9 (user: keep it safe,
  small cost).
- Non-entry owners (8: `000011001001011`, `101010000110001`, `010111011000001`,
  `001000100101111`, `011001110110100`, `111000100011101`, `110010101101011`,
  `011011111101001`): kept as convenience roots for a community-usable tool,
  with the widest connected boxes (V4 = 0): D = 10: A <= 16, R <= 6; D = 9:
  A <= 14, R <= 5.
- Per-coordinate uppers: A_max - t on active axes, R_max on inactive axes.
- Helpers: one full orthant per owner (lower 0, upper null, no D bound) at
  max_numerator_rank = the owner's largest root R_max, positive power unbounded
  (keeps the initial-orthant fast path); max_positive_power = the owner's
  largest root A_max only for owners whose matching-only diagnostic shows
  unresolved guard pieces at unbounded A. Helpers first in query order.
- Descendants are never clipped. The walk still retains every escaping Apply
  and Route obligation.
- The classifier is generic in the loop count and vertex-degree set; the
  five-loop data (TIDE slot momenta, four parent vertex witnesses, owner masks)
  are inputs. No topology names enter Rust.

Generic formulas (L loops, exact D, K powers of the gauge parameter):
connected root R_max = D - L + 1 - V4min + K, A_max = R_max + D; nested
factorized A <= 5L - 1 - V4min + K, R <= 3L - V4min + K, D >= 2L - 1.
At L = 4 the nested formula gives 19/12/7, the four-loop control's envelope.

## 3. Work packages

### A. Physics-scoped input planner and validation (Python, generic)

1. `examples/input/tide_five_loop_parent_vertices.json`: the four parent
   witnesses transcribed from `docs/research/tide_five_loop_census.md:150-170`,
   validated against `examples/input/tide_five_loop_manifest.json` momenta
   (zero vertex sums, each slot twice with opposite signs, E - V + 1 = L).
2. `examples/python/plan_renormalization_entry_queries.py`: ports the audit's
   classifier (2-isomorphism closure of each owner's realization; enumeration of
   connected bridgeless L-loop vacuum skeletons with vertex degrees in
   {3,4}; series-class merging; canonical multigraph forms) and emits
   `queries.json` (v2, 183 rows: 67 helpers + 41x2 + 8x2 + 18x1),
   `entry-plan-receipt.json`, `skeleton-classification.json` and per-budget
   `entry-domain-plan` counts via the Rust CLI. Deterministic, byte-identical
   across runs. Options: `--loops`, `--gauge feynman|linear-xi`,
   `--gauge-parameter-powers`, `--difference-set`, `--factorized-roots
   nested|box|omit`, `--non-entry-roots widest|omit`, `--helper-positive-power-owners[-from]`,
   `--classification[-fixture]`, `--executable`.
3. `examples/python/check_renormalization_entry_queries.py`: independent
   re-derivation of every bound from the receipt, containment of each root in
   its helper, ID/size/order checks, closed-form tuple counts equal to Rust.
4. `examples/python/summarize_owner_domain_match.py`: streams a matching-only
   `result.json` into `matching-summary.json` with the helpers that need a
   finite A.
5. Tests (`unittest`): hand-known L = 2 and L = 3 families, bound formulas
   (L = 4, 5, xi shift, empty bands), document shape and determinism, fake
   `entry-domain-plan`, witness validation, committed L = 5 fixture
   `examples/python/fixtures/tide_five_loop_skeleton_classification.json`
   (41/18/8 split; slow full-enumeration test behind `RUSTRED_SLOW_TESTS=1`).
6. Validation ladder before any campaign, evidence under
   `TMP/qcd-feynman-d9d10-input.XXXXXX/`: (a) plan + checker; (b) matching-only
   diagnostic on all queries (`match_shared_owner_domains.py` without
   `--follow-successors`, CPUs 192-197): every query `locally_applicable`, zero
   `exact_gap`/`unresolved`/`invalid_source_condition`; re-plan helpers that
   need finite A and re-run on the final bytes.

### B. Scheduler and admission package (Rust, `crates/rustred-app/.../walking`)

Measured mechanism: Ordered polls only the head; a non-head inspector blocks
after one chunk; under Ready finished slots are not recycled during a chunk
commit; the single coordinator's admission (prep + ordered commit) is ~31% of
wall and rising, with ~250 forward and ~7,700 charged reverse containment
comparisons per admission.

1. Bit-signature pre-filter tier (`queue/bits.rs`): packed u64 per ID
   (unbounded-axis mask, zero-lower mask, A/R/D flags) tested by a subset
   check before `DomainPowerSummary::contains` in forward lookups, prepared
   lookups and reverse retirement. Results and persisted counters identical;
   session counters `forward/reverse_callbacks` and `*_bit_rejections`.
2. Reverse retirement off the serial path: helpers compute each admission's
   retire set read-only during preparation (`AggregateIndex::collect_contained`);
   commit applies it with the same loop as `retire` plus `contains_new` for IDs
   above the watermark (`retire_prepared`). Identical results, layout, counters
   and transfers (proof recorded in the module doc).
3. Ready hardening: close the multi-prefix resume-to-exhaustion gate with a
   `maybe_save` diagnostic trigger (`RUSTRED_WALK_DIAGNOSTIC_PAUSE=ready-multi-prefix`),
   an in-process W=4 test with scripted inspectors, and a fresh-process
   pause/resume harness; recycle finished slots under Ready (reclaim into
   escrow and dispatch reserved work between commit batches); Ready becomes the
   launcher default for new campaigns; H stays 256 (>= 4 x inspectors up to
   W = 128); stale docs updated.
4. Closure refresh duty: interval multiplier 20x -> 100x (<= ~1% duty),
   `refresh_duty_bound` and `next_refresh_seconds` in the JSON.
5. Preparation/commit pipelining (double-buffered lagging read replica) only
   if, after 1-2, prep + commit still exceed 15% of coordinator wall on the
   five-loop control.
6. Inner parallelism for heavy Apply heads: private rayon pool carved from the
   worker budget, per-cell owned transcripts replayed in canonical order with
   sequential budget accounting and lowest-ordinal failure; equivalence tests;
   shipped off by default (`--inner-apply-workers 0`), enabled only if pilots
   show coordinator duty < 60% with idle inspectors.
7. Worker scaling: CLI/API/Python caps 64 -> 256; Ready split for W > 64 caps
   helpers at 32; the campaign starts at W = 50 and grows only if measured busy
   cores follow.
8. Telemetry for the monitor: coordinator duty breakdown (dispatch, poll,
   preparation, commit, publication, wait, closure refresh, checkpoint), per-slot
   busy/backpressure/idle seconds, `computing_workers`, heaviest active stream,
   admission filter counters.

Every item in B is a non-semantic change (admission results, records and
dependency edges identical; Ready schedules were already timing-dependent).

### C. Checkpoint and memory redesign (Rust)

Measured: RSS ~= 4.5 GB + 1.16 KB per discovered + 6.0 KB per committed domain,
the latter being write-only `serde_json::Value` report records
(`execution.rs:663`, `delegation.rs:74`) that nothing reads during the walk;
checkpoints are a single-threaded JSON rewrite (edges 57% of bytes at 53.6 B
per 24 B edge); save duration grows ~28 s per generation; no restore above
4.3 GB was ever executed.

1. Records sidecar: stream each committed record at commit time to an
   append-only sidecar with per-generation segment boundaries; keep no per-ID
   record state in RAM (restore validation already cross-checks ledger and
   closure state); manifest records the sidecar length/count/digest per
   generation so an un-checkpointed tail is truncated on resume; final
   `result.json` assembly becomes a streaming post-pass joined with
   `ledger.resolve()` and `closure.closed()`; tests get an in-memory sink and
   a read-back accessor.
2. Compact in-RAM state: `Domain`/`DomainPowerSummary` with small fixed arrays
   (u8/u16 coordinates with an infinity sentinel, lazily derived u128 extrema);
   free summaries of retired candidates; target <= 0.8 KB per discovered
   domain total. Semantics-neutral.
3. Dependency edges as u32 CSR appended per generation with heads rebuilt on
   restore; RAM 11 GB -> ~3.7 GB at today's edge count; binary on disk.
4. CP5 checkpoint format: binary sections (bincode via serde, or little-endian
   arrays for the big vectors) in separate files under one manifest with
   per-section blake3 digests computed while writing (no re-read);
   append-only sections (domains, edges, records) written as per-generation
   segments; small mutable sections (node flags, ledger, index, streams,
   counters/metadata JSON) rewritten fully; segment-aware cleanup; target save
   time O(new state), seconds not minutes; the post-resume forced save skipped
   when nothing changed; validation layer with negative tests equivalent to the
   current JSON-key corruption tests.
5. Binding relaxation: `WALK_SEMANTICS_VERSION` constant (documented contract:
   bump on any change to admission ordering, containment semantics, replay
   token hashing, routing/matching results); manifest records schema, semantics
   version, request/policy binding digest, owner digests, and the executable
   digest as informational; resume refuses a different semantics version and
   accepts a different executable with the same version (tests for both).
6. Save scheduling: interval remains transport; default 4 h for the new
   campaign; size/time-adaptive interval optional; cooperative-stop save
   unchanged.
7. Restore-at-scale: timed restore of a copied large checkpoint on CPUs
   192-241; interrupted-vs-uninterrupted equivalence on the four-loop FG
   control (identical records, counters, closure); synthetic large-state
   save/restore benchmark showing per-generation cost is O(new state).

Old CP1/CP3/CP4 checkpoints become unresumable with the new binary (accepted).

CP5 specifics (from the checkpoint design, 2026-09-26):

- Directory layout, flat and generation-suffixed: `latest.json`/`previous.json`
  (manifest schema 5, `format: "RUSTRED-WALK-CP5"`), rewritten per generation:
  `meta-<G>.json` (counters, queue metadata, progress, parallel, uncommitted,
  inputs, streams, closure counters, sidecar aggregates), `nodes-<G>.bin` (u8
  flags), `ledger-<G>.bin` (16 B entries), `index-<G>.bin` (bincode with a size
  limit, buckets sorted by (phase, owner) so bytes are deterministic);
  append-only segments: `domains-<S>.bin`, `edges-<S>.bin` ((u32, u32) pairs),
  `records-<S>.jsonl`. Every binary section has a 32-byte header (magic
  `RRW5`, tag, arity, flags, semantics version, count, first) and fixed-width
  payloads are length-validated before decoding.
- Manifest: schema, format, kind, generation, `walk_semantics_version`,
  arity, publication policy, request binding digest, owner digests,
  `executable` and `executable_first` (informational), per-section
  `{file, bytes, blake3}` and segment lists tiling `[0, total)`, and the
  `metadata` block with every key the Python monitor and shard supervisor read
  today plus `new_bytes` and `executable_changed_since_bootstrap`.
- Store: sections written in a `rayon::scope` through hashing writers (no
  re-read for digests); `publish` keeps the rename+fsync and `previous.json`
  logic; segment-aware cleanup removes only unreferenced files below the
  previous generation; failed saves never delete. Restore verifies every digest
  in parallel, then validates sections (domain ranges, ledger invariants,
  index positions plus an envelope containment check, closure counters,
  streams with the accepted-events counter) and a ledger-closure cross-check
  that replaces the record scan.
- Records sidecar: `RecordSink::{Memory, Sidecar}` in `execution/records.rs`,
  `State.records` becomes a `RefCell<RecordSink>` plus `records_accepted_events`;
  segments sealed at each save; orphans from crashes are ignored and cleaned;
  `OwnerDomainWalkResult::write_json` streams `domains` into `result.json`;
  finalization annotations come from `ledger.resolve()` and `closure.closed()`.
- Compact state: `CompactDomain<N>` (u16 coordinates with an infinity
  sentinel, ~96 B), `CompactSummary<N>` (u32 coordinates, ~176 B) in a slab
  with retired-slot reuse, `HashMap<u128, u32>` exact map, optional 16 B ledger
  entries; `Domain<N>` stays the inspection transport type. The only new
  refusal is a coordinate above 65534, unreachable for these inputs.
- Edges: u32 CSR-by-target plus a bounded append log folded after each save;
  node flags as bytes; restore rebuilds heads from the segments.
- Semantics constant `WALK_SEMANTICS_VERSION` in `walking/mod.rs` with the
  documented bump contract; `Store::open_with_identity` test seam; resume
  refuses a different semantics version and accepts a different executable
  (event `checkpoint_executable_changed`).
- Scheduling: adaptive `effective_interval = max(interval, 20 x last save)`,
  unchanged-state skip via a change stamp (makes the post-resume forced save
  free), pre-save forced closure refresh.
- Scale tests (`#[ignore]`, env-driven): timed restore of a copied production
  checkpoint on CPUs 192-241; FG-control interrupted-vs-uninterrupted
  equivalence (Ordered and Ready); synthetic 20M-domain save/restore benchmark
  asserting per-generation cost is proportional to new state.
- Targets: ~0.4 KB per discovered domain of walk state (vs ~4.5 KB), saves of
  10-60 s at hundreds of millions of edges, restore of a 38M-domain state in
  minutes. Order: semantics constant -> sectioned store with today's types ->
  sidecar, edges and compact state in parallel -> scheduling -> scale tests.

### D. Launcher, supervisor and monitor (Python)

1. `production_saved_owner_campaign.py`: steering schema v2 with
   `publication_policy`, `transfer_unreserved_lookahead`, `inspection_workers`,
   `checkpoint_interval_seconds` frozen at prepare time (v1 readable with
   defaults); worker cap 64 -> 256 in both scripts; `--cpus` accepts ranges
   (`128-177`); `--prepare-from SOURCE --queries NEW.json --attach FILE...`
   copies the owner payloads and stages a new query document plus receipts;
   Ready is the default for new campaigns.
2. `shared_owner_campaign.py`: derived metrics in `status.json`
   (completions/h, stall shares at >= 5 s and >= 20 s, pending growth per
   completion, RSS per discovered domain, coordinator duty, checkpoint duty,
   computing inspectors, max scheduled rank, roots closed); committed offline
   `heartbeat_metrics.py` computing the same numbers from any `events.jsonl`.
3. `campaign_monitor.py`: three new dashboard lines (inspectors computing vs
   reserved, stall share, coordinator duty; rate, pending growth, rank, RSS per
   domain; checkpoint generation/bytes/seconds/duty, roots closed), all
   "unknown" when absent; no ETA.
4. Tests extended accordingly (`unittest discover -s examples/python`).

### E. Profiling protocol and controls

Committed harness `examples/python/walk_control_matrix.py` (interim version:
`TMP/fable51-controls/run_control.py`), audit `audit_owner_domain_walk.py`,
comparison `compare_walk_records.py` (strict for Ordered, multiset for Ready).
Matrix: FG/BMW/H/X at W6 on CPUs 192-197 (Ordered and Ready) and the 1,324-tuple
five-loop control at W50 on CPUs 192-241 (Ordered and Ready), old binary
`32fdec09...` vs new, fixed order with an FG repeat. Baselines already
recorded (2026-09-26, `TMP/fable51-controls/baseline-32fdec*/`): FG 18.2 s
whole command / 13.7 s traversal / 98,869 inspections; BMW 40.2 / 33.9 /
147,233; H 17.5 / 12.6 / 24,680; X 41.0 / 34.4 / 46,826.
Acceptance of a new binary: all audits pass; strict record equality for
Ordered; no case slower than the FG repeat spread; targets: coordinator
prep + commit per request down >= 30% after B1, ordered-commit per request
halved after B2, four-loop whole-command time not worse, five-loop W50 Ready
control faster than the old binary's Ready run. Results go to
`docs/research/five_loop_engine_profiling_2026-09-2X.md`.

### F. Pilots and launch

1. Single-owner pilot (`011101110111000`, physics roots + helper, W6, CPUs
   192-197, RAM guard 100 GB, Ordered then Ready, no work cap): drains or not;
   native inspections, max scheduled rank, pending growth, peak RSS; compared
   with the 967,621-inspection A11/R2 control and the old envelope's
   rank staircase (15 -> 21).
2. Multi-owner pilot (the six hot owners, Ready W24 on CPUs 200-223, 100 GB,
   ~6 h) through the production launcher so it can be promoted; matched
   metrics against the live stream at equal elapsed time.
3. Restricted five-loop pilots with each engine package as it lands (user:
   "don't hesitate to run pilot programs first").
4. Campaign `campaigns/five-loop-qcd-feynman-d9d10`: prepared with the new
   binary, `--workers 50 --cpus 128-177 --publication-policy ready
   --transfer-unreserved-lookahead 256 --checkpoint-interval-seconds 14400
   --max-memory-bytes 700000000000 --ram-guard-margin-percent 5`; checker run on
   the staged bytes; launched by typing the `--start` command into the
   `fable_5_1` tab (`XDG_RUNTIME_DIR=/run/user/1125 zellij --session rustred
   action go-to-tab-name fable_5_1`, `write-chars`, `write 13`); monitored
   read-only with `campaign_monitor.py`. Worker count may grow beyond 50 (up
   to the physical cores 128-255) only if measured busy cores follow.

Go/no-go and monitoring thresholds: at matched elapsed time versus the live
run, completions/h >= 1.5x, stall share (>= 5 s) < 40%, computing inspectors
>= 3, pending growth per completion < 1.4 and falling, max scheduled rank
<= helper rank + 7, roots closed strictly increasing, RSS trajectory to the
665 GB stop point > 7 days, checkpoint duty < 10%, coordinator duty < 50%.
"Better odds than the old run" is declared only on closed roots, falling
pending-per-completion and a flat rank cap, never on completions/h alone.
Escalation: pending growth above the old run's for 3 h or rank past
helper + 7 -> cooperative stop and scope review.

### G. Documentation and repository hygiene

`docs/research/five_loop_qcd_feynman_entry_class_2026-09-26.md` (physics
class, classification table, receipts, pilots, launch record),
`docs/research/five_loop_engine_profiling_2026-09-2X.md`,
`docs/research/five_loop_checkpoint_cp5_2026-09-2X.md`, updates to
`docs/shared_owner_campaign_driver.md`, `docs/finite_starting_domains.md`,
`examples/python/README.md`, `docs/CLI.md`, and a dated follow-up in
`GOAL.md` recording the envelope decision. Commits are small and reviewable;
`cargo fmt --all -- --check`, the release unit suite for `rustred-app`
(baseline 687 passed / 0 failed / 4 ignored) and the Python suite (110) must
pass at every commit; new tests accompany each package. Push to
`origin/fable_5_1` after each validated milestone.

## 4. Sequencing

1. A1-A5 planner and tests; A6 validation ladder (a)-(b). Pilot F1 with the
   frozen binary starts as soon as the inputs pass the diagnostic.
2. B1, B2, B4, B8, B7 (caps) with the profiling matrix E on FG/BMW/H/X and the
   five-loop control; B3 Ready hardening with its gate.
3. C1 records sidecar and C3 edges (largest RAM wins), then C4 CP5 format and
   C5 binding, C2 compaction, C6-C7; profiling matrix repeated; restore-at-scale.
4. D launcher/monitor changes (needed before F2); F2 multi-owner pilot; B5/B6
   decided from the pilots' coordinator duty and implemented if their gates fire.
5. Full release gate, docs, push; F4 launch in `fable_5_1`; monitoring per the
   thresholds; decision log entries at each gate.

## 5. Open risks

- Termination of the symbolic walk is not established even for the smaller
  class; the single-owner pilot is the first evidence.
- Restore of a large checkpoint has never been executed; C7 is mandatory
  before relying on the RAM guard.
- Ready has never been drained at five loops; the B3 gate must pass before
  launch.
- The host is shared (another user's job floats over all cores; `zpool status`
  reports 2 data errors on the single NVMe pool; 40 GB swap in use).
- The live campaign and the new one cannot both reach their RAM ceilings on
  this host; the user stops the old run when the new one shows better odds.

## 6. Decision log

- 2026-09-26: physics class fixed (Feynman gauge, D in {9,10}, nested
  factorized, all 67 roots kept); semantics-version binding accepted;
  everything before launch with pilots; CPUs 128-177 (+ more if saturated);
  700 GB guard; user stops the old run themselves. Branch `fable_5_1` created
  from `main` at `100e990f`; baseline tests 687/0/4; four-family baselines
  recorded with the frozen binary.
