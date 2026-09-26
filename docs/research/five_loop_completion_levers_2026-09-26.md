# Five-loop campaign: what limits time to completion, and the strongest levers

Date: 2026-09-26, 02:20 UTC. Read-only audit (before the `fable_5_1` implementation branch) of `campaigns/five-loop-dependency-closure` (run `20260925T161054.596593Z`, native PID 3473368), the RustRed source, all `docs/research` records, the retired campaigns, and the literature. Nothing was changed, signalled, built or launched. Method: 13 parallel auditors (scheduler, containment index, native inspection, memory/checkpoint, measured-evidence compiler, architecture-proposal reviewer, live-receipt forensics, physics envelope) and 5 literature researchers, two independent synthesizers, a merge into 12 candidate levers, and three adversarial lenses per lever (wall-clock, soundness, prior evidence). I then re-verified the load-bearing claims in the source myself.

## 1. Where the time goes (measured)

Snapshot at 10.1 h: 7.60M native inspections completed, 26.98M domains discovered, 12.52M pending, 14.46M committed; 7 of 67 initial roots and 3.02M of 26.98M domains recursively closed; RSS 121 GB; checkpoint gen 11 = 46.4 GB written in 331 s; zero frontiers.

| Quantity | Value | Meaning |
|---|---:|---|
| Busy cores (whole run) | 1.8 of 10 reserved | inspectors ~1.0, helpers ~0.6, coordinator ~0.25 (per-thread `/proc` CPU) |
| Head = owner `011101110111000` Apply | 62% of wall | one owner's heavy Apply inspections hold the ordered publication head |
| "Slow regime" (head dwell >= 10 s) | 65% of wall, 1.5% of completions | ~430 heads, mean ~41 s, ~0.8-1M successors each |
| Finished-but-held inspections | mean 154, max 255 (= H-1) | the 256-ID lookahead window is saturated; 47% of IDs are delegated aliases |
| Backpressure | 38,117 s cumulative | ~1.07 inspectors permanently blocked in `publish()` |
| Coordinator prep + ordered commit | 6,383 s + 4,783 s = 31% of wall | rising per request (2.3 to ~13 us) |
| Containment checks | 432 billion | ~250 forward checks per lookup plus ~7,700 charged reverse comparisons per admission |
| Checkpoint stall | 5.5% of wall, +28 s per generation | full single-threaded JSON rewrite; 57% of bytes are dependency edges |
| RSS growth | ~4.5 KB per discovered domain, 4-15 GB/h | 712.5 GB save-and-stop at ~160-170M discovered domains: 48-100 h at current pace, ~25 h at 2x throughput |

**Mechanism (verified in source).** Under Ordered publication the coordinator polls only the head ticket ([execution.rs:1180-1185](../../crates/rustred-app/src/application/routed_campaign/walking/execution.rs:1180)). A non-head inspector that fills its single 8 MiB / 16,384-record chunk blocks in `publish()` until it becomes the head ([parallel.rs:216-242](../../crates/rustred-app/src/application/routed_campaign/walking/parallel.rs:216)); only *finished* jobs move to escrow. Heavy Apply jobs (hundreds of thousands of successors) therefore run one at a time, as the head, on the critical path, while the four other inspectors handle only sub-millisecond jobs. This is why the retired 50-worker run (25 inspectors) was only 10-12% ahead of the current 10-worker run at equal elapsed time, and why adding cores did nothing.

**Memory (verified in source).** 60-65% of RSS is the write-only `serde_json::Value` record pushed per committed domain for the final `result.json` ([execution.rs:663](../../crates/rustred-app/src/application/routed_campaign/walking/execution.rs:663), [delegation.rs:74](../../crates/rustred-app/src/application/routed_campaign/walking/execution/delegation.rs:74)); ~5-6 KB each in RAM, never read during the walk. Queue state is ~1.2 KB per domain; dependency edges ~11 GB.

**Convergence.** Pending has grown every hour in all three campaigns. Cumulative scheduled/completed rose from 3.13 to 3.55 here and was 3.63 (flat) at 17 h in the W50 run (13.05M completed, 23.3M pending). Max scheduled finite rank is 21 and positive-power caps reach 25-31, although the entry class is R<=15, A<=24. A gen-10 checkpoint census shows 79% of pending IDs are Route obligations on 6,917 uninstalled masks, and 99% of pending Apply IDs on installed owners lie *above* the R15/A24 anchors. No ETA is defensible; the closure size is unknown.

**What the walk is actually closing.** The 134 helper-first queries coalesce into 67 admitted roots: the 67 helper anchors (R<=15 orthants, no D bound; 54 with A<=24, 13 with unbounded A) absorb the 67 physical requests by containment. The campaign is therefore closing the anchor class, which is much larger than the physical envelope A<=24, R<=15, D>=9.

## 2. Strongest suggestions, ranked

Every lever below needs a fresh campaign: the checkpoint binds executable bytes and the request/policy ([checkpoint.rs:167](../../crates/rustred-app/src/application/routed_campaign/walking/checkpoint.rs:167)). Only the checkpoint interval is exempt. Deterministic replay makes a restart cheaper than it looks: the live run and the retired W50 helpers-first run reached byte-identical states (1,373,731 completed / 5,419,289 scheduled) at different wall times, and ~10 h of Ordered progress is re-walked in ~4-6 h under a 2x configuration.

### 2.1 Fresh campaign under the existing Ready publication policy, ~24 workers (12 inspectors / 11 helpers / 1 coordinator), 4 h checkpoint interval

Survived all three adversarial lenses. Zero code: same frozen binary and inputs, `--publication-policy ready --workers 24 --checkpoint-interval-seconds 14400`.

Why: Ready round-robins the poll over every active ticket ([publication.rs:20-38](../../crates/rustred-app/src/application/routed_campaign/walking/execution/publication.rs:20)), so every slot's chunks drain continuously and H=256 becomes an outstanding-credit count ([ledger.rs:412-430](../../crates/rustred-app/src/application/routed_campaign/walking/delegation/ledger.rs:412)). Heavy heads overlap instead of serializing. Measured support: the W50 Ready control was 1.31-1.35x faster than Ordered on a 1,324-tuple input that had far less head-of-line slack than the live run.

Expected: about 2x on completions per hour (range 1.5-2.5x). Ceiling ~2.4x at 5/4/1 and ~2.8-3x at 12/11/1 from the measured serial coordinator floor; beyond that the single coordinator's admission is the limit. Work volume is unchanged, so this does not change whether the walk converges; it reaches the RAM wall in ~25 h instead of ~50-75 h unless closure comes first.

Risks: Ready was 5-7% slower at W6, so W24 is an extrapolation and the pilot gate is mandatory; no five-loop Ready run has been drained and the multi-prefix resume-to-exhaustion gate is still inconclusive (`docs/research/five_loop_ready_publication_2026-09-24.md`); work inflation from concurrent same-owner heads is unmeasured (+1.5% in the control, +15% in the older concurrent-owner lane).

Gate: W24 Ready pilot on spare socket-1 CPUs with a 100 GB RAM guard, at least 6 h. Compare at matched wall: native completions, no-completion stall share (live: 62% at >=5 s, 54% at >=20 s), heavy epochs per hour (live 14-60), mean computing inspectors (live ~1.1-1.3), pending growth per completion (live 1.4-3.7). Launch it through the production launcher so a passing pilot can be promoted by raising its RAM guard at resume rather than restarted.

### 2.2 Decide the physics class, then re-stage the inputs (the only lever that can change convergence)

This is a scope decision for you, not an engineering optimization; the adversarial lenses refuted it as a *throughput* lever (per-head cost is insensitive to box size, and the hot heads already sit at A 11-13) but upheld its soundness. It matters because the queue is now dominated by escaped descendants above the R15/A24 anchors, and root rank is what drives the rank staircase (15 -> 21) and the numerator fan-out.

Physics audit (independent enumeration, needs your sign-off): after UV Taylor expansion, a subtracted child's propagators depend only on its own loop momenta, so nested-forest terms live in factorized sectors; connected five-loop sectors receive only the top-level expansion, i.e. A <= 16 - V4, D in {9, 10}, R <= 6 - V4 (V4 = quartic vertices). Enumerating all connected bridgeless five-loop vacuum skeletons with degree-3/4 vertices and classifying them into the 67 owners: 41 owners are connected entry sectors, 18 factorized entry sectors, and 8 are never entry sectors (only descendants), including two of the six hottest Apply owners. The literature agrees on scale: no five-loop tadpole reduction ever exceeded ~7 dots and 6 scalar-product powers (Maier-Schroeder 2024, 131.9M integrals, still unfinished with finite-field Laporta), and five-loop RG functions needed <= 4 dots and 4 numerator powers (~400k integrals). The current class is 315x to 12,600x more lattice points than anything reduced at five loops, and the anchors are unbounded regions on top of that.

The catch, from the repo's own four-loop evidence: bounded helpers fragment the walk (A<=19 helper caps were 3.9-15.7x slower) because bounded boxes lose the orthant fast path, and helpers must contain their roots. So the promising variant is: keep unbounded-A orthant helpers but lower their *rank* to the physical rank, tighten the required roots per owner (41 connected owners at A<=16-V4min, R<=6-V4min, D in {9,10}; nested bounds for the 18 factorized owners; drop the 8 non-entry roots but keep their programs for routing).

Expected: unknown on wall time per se; potentially the difference between a walk that closes inside RAM and one that does not (the observed +7 A / +6 R escape would then top out near A23/R12 instead of A31/R21). This overrides the envelope frozen in `GOAL.md`; only you can make that call.

Gate (no restart, spare cores): stage the hot owner `011101110111000` alone at A<=13, R<=3, D in {9,10}, with its helper at the reduced rank and without it; compare full-closure cost and whether rank-16+ heads reappear against the existing 967,621-inspection A11/R2 control. Open physics questions to settle first: gauge (covariant gauge adds numerator powers), whether the five-loop delta-M^2 (D=9) term is wanted, whether GammaLoop hands nested products to Vakint as five-loop product topologies.

### 2.3 Build one memory/checkpoint successor binary before any multi-day run

Not a throughput lever, but it decides whether a long run survives. Contents, in order of value per effort: (a) stream the per-domain report records to an append-only sidecar at commit time instead of retaining `serde_json::Value` trees (removes ~70 GB now; RAM growth 4.5 KB -> ~1.6 KB per domain; wall moves from ~160M to ~440M domains); (b) u32 CSR dependency edges written as a binary section (checkpoint -20 GB, -140 s per save); (c) binary section codec for domains/ledger/nodes/index (46 GB -> ~9 GB, saves ~30-60 s instead of 5-30 min); (d) closure-refresh back-off multiplier 20x -> 100x (~3.5% of wall). Days for (a) and (d), weeks for the full package. Bundle it with whichever restart you choose; do not restart twice. Gate: heap profile of a small run; timed restore of a copied checkpoint.

### 2.4 Operational items that need no code

- Checkpoint interval 2-4 h on any fresh launch or resume (it is transport, not policy: [checkpoint.rs:73](../../crates/rustred-app/src/application/routed_campaign/walking/checkpoint.rs:73)). Worth 10-25% over a 40-100 h run; at 2x throughput hourly saves would reach 25-50% duty within a day. Not worth a deliberate stop of the live run for this alone (net ~1.1x on a 40 h horizon after a 25-50 min stop/restore).
- Time a `--resume` of a copy of the gen-11 checkpoint on spare cores (~130 GB RAM). No restore above 4.3 GB has ever been executed; the RAM guard's save-and-stop only protects the live run if a 46 GB (later ~250 GB) JSON restore works.
- At the eventual guard-forced pause, raise `--max-memory-bytes` (host 1.13 TB; ~85 GB more is available) and set the long interval in the same resume.

### 2.5 Coordinator/admission work, only once Ready makes the coordinator the ceiling

Each is 5-15% and Amdahl-bounded by the 31% coordinator share today: inline bit-signature tier per candidate (unbounded-axis mask superset / nonzero-lower subset tests before the full check), batched helper-side reverse retirement instead of ~7,700 charged reverse comparisons per admission on the serial commit path, prep/commit pipelining. Do not build kd-/R-trees: the antichain literature (Cadilhac et al. 2025) and the repo's own two lost index tiers say filtered bucketed lists are the right shape. Measure selectivity offline on the checkpoint first.

## 3. Ideas examined and set aside

| Idea | Why not now |
|---|---|
| Keep Ordered but buffer multiple chunks per non-head slot + H 4096-16384 | Same mechanism as Ready but ceiling 1.6-1.7x, days of code; dominated by 2.1. |
| Wide same-owner Apply helpers for the hot owners; ~1,000 Route hull anchors | Four-loop bounded/wide helpers were 3.9-15.7x slower; the hot owner already has unresolved guard pieces at R15/noA; hull anchors cover 150-2000x more points and their images exceed the worst current heads. |
| More cores or larger H under Ordered | W50 = W10 (+10-12%); the window is saturated by held results, not by dispatchable IDs. |
| Inspector inner parallelism, application-cell refinement, physical subdivision | Subsumed by Ready; refinement gave no gain on the new head; subdivision 1.13x native-only. |
| Successor coalescing inside the inspector | 0.97 successors per shift group; nothing to merge. |
| Independent owner shards | Reached 50 busy cores but 2x slower (lost cross-owner dedup, duplicated preparation). |
| Weighted critical-path (T_inf) measurement | No per-inspection spans exist in the receipts; not computable. |
| Rule-generation methods (Janet/Ore, syzygies, generating functions, tube seeding) | Zero frontiers after 7.6M inspections; the cost is applying saved rules, not discovering them. |
| Global R* / IRR to four-loop massless propagators | Would replace, not accelerate, this campaign; incompatible with the all-lines-massive local scheme. |

## 4. Recommended sequence

1. Today, live run untouched: Ready W24 pilot (2.1), Ready forced-stop/resume gate on the four-loop control, timed restore of a checkpoint copy (2.4). If you sign off on the physics class: the single-owner envelope pilot (2.2).
2. Decision at pilot hour ~6 on the matched metrics above. Pilot >= 1.5x with lower stall share and no worse pending growth, gates passed: cooperatively stop the live run (it keeps its checkpoint) and promote the pilot with a full RAM guard. Pilot 1.0-1.5x: keep the live run; then only the envelope decision or a successor binary changes the outcome.
3. In parallel, build the successor binary (2.3), and bundle it plus any envelope change into one restart, not several.
4. Keep the closure counters honest: 7/67 roots closed at 10 h, unchanged since hour 6, is the deliverable's real progress signal.

## 5. Open risks

- Termination is not established: pending grows every hour, rank and power caps keep climbing, ~3.5 discoveries per completion. No scheduling lever changes this.
- RAM wall and an untested large restore; post-restore RSS was 3.8x checkpoint bytes at small scale.
- Checkpoint growth (+28 s per generation) makes hourly saves self-defeating at higher throughput unless lengthened.
- Ready-specific: resume-to-exhaustion gate open; same-owner work inflation unmeasured; coordinator becomes the ceiling.
- Host: another user's gammaloop job has affinity over all CPUs and floats onto CPUs 0-9 and any pilot cores; 40 GB of swap is in use. Verified: `zpool status` reports 2 data errors ("data corruption, applications may be affected") on the single non-redundant NVMe pool `zroot`; the last clean scrub was 2026-09-01, so the errors are newer. The affected file list needs root (`sudo zpool status -v zroot`). Check it before relying on the checkpoint directory; a corrupt state file refuses resume and `previous.json` is the fallback.


## Follow-up

The user subsequently decided to implement the levers on branch `fable_5_1`; the governing plan is [`FABLE_5_1_five_loop_vacuum_plan.md`](../../FABLE_5_1_five_loop_vacuum_plan.md), which records the physics-class decision (Feynman gauge, D in {9,10}, factorized owners kept with nested bounds, all 67 roots kept), the relaxed semantics-version checkpoint binding and the launch target.
