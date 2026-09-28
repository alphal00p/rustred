# W0 Route-side coverage census on v2 generation 7 (routecensus lane, 2026-09-28)

This note answers handoff §0.1 item 3 and critique §2.2: the W0.7 census measured Apply coverage only, while Route is
74% of the domains (54,789,967 of 74,156,033) and 72% of the native-pending (14,724,634 of 20,572,079) at v2
generation 7. It feeds W0.11 and the D2 decision. Branch `fable_5_1-v3-routecensus`, based on `fable_5_1-v3-census`.
Labels: **[M]** measured (receipt file and binary sha256 cited; sampled shares carry a stratified bootstrap standard
error), **[E]** estimate, model output or projection. Nothing here is an ETA or a closure claim
(`family_closure_claim` stays false). Receipt copy of this note: `/common/dev/rustred/TMP/w0/routecensus/RESULTS.md`.

**Revision (fix round, 2026-09-28 ~02:00 UTC).** Verifiers raised two problems; both were real and are corrected here.
- **Net RSS of an admission-time union cover.** The first version charged only the cover's own anchor edges. An
  avoided domain also loses the requests that later land on it: 3.3-10.8 per covered domain, 86M-497M over v2's
  history [M]. Each of them needs its own admission. With k' anchors per redirected hit unmeasured, the net RSS
  factor is now a range whose floor is below 1x: 0.6-1.9x (merged natives), 0.3-3.1x (earlier non-delegated) and
  0.06-3.0x (every smaller ID). "The union test runs only on the 74.2M misses" is replaced by up to 2.2-7.7x as many
  union tests. New sections: §6b; tool `census route-hits` and `route_hits.py`.
- **Creation weights of the Route pending.** They were overstated by 1.4-2.3x across the four anchor sets: for
  all_natives, 51.8% became 22.1% (22-32%). The "at resume" projection falls from
  4.2M / 6.0M to 2.0-2.9M / 3.1-3.9M creations. New section: §3b.

Every other figure is unchanged.

## 0. Verdicts

1. **Route natives are not a CPU lever. Their weight is in the admission requests they emit and, much less, in the
   domains they create [M].** The 21.52M Route natives used 1,951 s, 0.80% of all native seconds (median 40.7 us). They
   emitted 544.6M transition edges, 46.5% of the 1.172G in the graph, but 9.3% of the raw successor count. They
   created 16.31M domains, 22.0% of all domains. Every Route native has at most one Apply edge, and 9.8% of them create
   an Apply domain. **Route domains are mostly created by Apply natives:** 71.9-77.6% of every Route class has an Apply
   native as its creator.
2. **Route pending at resume [M]** (14.72M domains, 6,791 stratified draws). Share fully covered by:
   - every gen-7 Route native: **45.9% ± 0.8 pp**;
   - the well-founded union of smaller non-delegated IDs: 58.2% ± 0.8;
   - every smaller ID: 63.0% ± 0.8;
   - every other domain of the (Route, owner) bucket (upper bound): **69.4% ± 0.8**.

   Weighted by the successors [E] these domains are predicted to emit, the shares are 45.0 / 55.3 / 60.1 / 66.1%
   (calibration leaves them unchanged within 2 pp). Weighted by the domains they are predicted to create, the shares
   are **22.1 / 31.8 / 36.0 / 43.1% [E]** (± 2.7-3.8 pp), after calibration by coverage status (§3b). An out-of-sample
   check under-predicts by about 10 pp, so read them as 22-32 / 32-43 / 36-46 / 43-53%. The uncalibrated
   per-(owner, rank) predictor gave 51.8 / 63.6 / 67.6 / 73.8%, an overstatement by 1.4-2.3x: it cannot see that
   covered natives create fewer domains than others of their group (g6-g7 natives: 51.1% of the count but 25.0% of
   the creations were covered before dispatch [M]). For comparison, the Apply
   figures (W0.7, pending Apply) are 55.4% by natives vs 79.5% by all other domains. The step from natives to the
   general union is the same size on both sides: +23.5 pp for Route, +24.1 pp for Apply (PPS; +24.2 pp uniform).
3. **Residuals do not need C2 on the Route side either [M].** In the partially covered Route pending, D-only residual
   cuts have 1 piece in more than 99.9% of the weight, and never more than 3. Residual inspection (gate: at most 10%
   of |Q|) adds +8.0 pp over full cover with D-only cuts and +22.4 pp with the hull. Route natives cost about 40 us,
   so a residual saves no CPU. Only full cover (no successors emitted) or a smaller residual image (fewer successors)
   matters.
4. **Covered Route natives are low-yield [M].** Historically, 41.7% ± 0.6 of Route natives were fully covered by the
   Route natives merged before their dispatch (the G2' rule). Those covered natives carried 39.7% of the Route
   successors but only **20.7% ± 2.2 of the domains Route natives created**, and 15.4% of creator-forest descendants.
   A covered native mostly re-discovers existing domains. So skipping covered Route jobs saves admission requests
   (about 220M edges, 19% of all transition edges [E, first level]) and only about 3.4M domains (4.5% of all).
5. **The domain-creation view says Route skipping adds almost nothing on the Apply side [M].** Route natives created
   11.1% of pending Apply by count but 58.4% by predicted Apply seconds [E]. 16.9% of predicted pending Apply CPU was
   created by a Route native that was itself covered before dispatch. But only 0.1% of that CPU lies in Apply domains
   that the Apply-side union (smaller IDs) does not already cover; against gen-7 natives the figure is 1.9%. Route
   coverage matters for domain volume and admission load, not for Apply CPU.
6. **Redundant domain creation is a large RAM lever, reachable from two placements [M, E].**
   - **At dispatch.** A fully covered job is aliased and emits no successors; with an empty residual this is G2'.
     Historically, the natives merged before dispatch fully covered 56.8% ± 0.9 of Apply jobs (this census's new
     `route-apply`; W0.7 measured 57.1%). Those jobs created **37.3% ± 1.8 of the domains Apply natives created**,
     21.6M. With the Route jobs' 3.4M, **25.0M first-level creations (33.7% of v2's 74.2M domains) would not have been
     made [E]**.
   - **At admission.** A successor covered by the union of existing same-bucket domains is never created. The share of
     every admitted domain that was already fully covered at its admission:

     | anchors | Route | Apply |
     |---|---|---|
     | natives merged before the creator's commit (G2'-type) | 32.9% ± 1.0 | 43.1% ± 1.0 |
     | smaller non-delegated IDs (well-founded) | 50.7% ± 1.1 | 66.4% ± 1.0 |
     | every smaller ID | 58.3% ± 1.0 | 71.9% ± 0.9 |

     That is 18.0M / 27.8M / 31.9M Route domains, or 9.0-15.1 / 13.9-23.3 / 16.0-26.8 GB at 0.5-0.84 KB/domain [E],
     gross of the edges the lever adds (net: below).

   The two placements overlap: a domain covered at admission by merged natives is also covered at its dispatch.
   Neither is additive to the other. The shares rise with generation. At the gen-7 mix [E, first level, no
   counterfactual history], the domain-creation rate falls as follows:

   | lever | merged natives | earlier non-delegated | every smaller ID |
   |---|---|---|---|
   | dispatch, Apply jobs only (G2' alias) | 1.82x | 1.88x | 2.03x |
   | dispatch, Apply + Route jobs | 2.10x | 2.21x | 2.48x |
   | admission, Route-only | 1.56x | 2.04x | 2.17x |
   | admission, both phases | 2.21x | 4.35x | 5.16x |
   | admission, both phases, RSS net of the cover's anchor edges only (k' = 1) | 1.5-1.9x | 1.9-3.1x | 1.5-3.0x |
   | admission, both phases, RSS net if each redirected later hit needs the full cover (k' = a, worst case) | 0.62-1.25x | 0.34-0.95x | 0.06-0.20x |
   | **admission, both phases, net RSS: stated range** | **0.6-1.9x** | **0.3-3.1x** | **0.06-3.0x** |
   | admission, Route-only, net RSS: stated range | 0.7-1.5x | 0.45-1.8x | 0.07-1.8x |

   Among gen-7 admissions, 51.7 / 73.7 / 78.0% of Route and 54.7 / 77.0 / 80.6% of both phases were covered.

   The net rows charge 8-16 B per extra edge against 0.5-0.84 KB per avoided domain (§6b):
   - **The cover's own anchor edges.** A cover uses 12.9-19.3 anchors (greedy first-hit count).
   - **Redirected later hits [M].** An avoided domain also loses the requests that would later have landed on it
     through an exact-digest or single-container hit. Each of them needs its own admission under the lever. A covered
     domain has **3.3 / 4.8 / 10.8** later hits on average (history, both phases). Over v2's history that is
     **86M / 195M / 497M** distinct edges.
   - **Anchors per redirected hit (k') are not measured.** If each redirected hit finds a single container (k' = 1),
     it adds no edges. If each needs as many anchors as the cover (k' = a), the lever stores more edge bytes than the
     domains it avoids, and RSS grows.

   **Corrections to the framing.**
   - With merged-native anchors, G2' already reaches most of this volume, and it reaches Route volume indirectly
     through the Apply jobs that create 72-78% of Route domains. "G2' is Apply-only, Route is 74% of domains"
     understates G2'.
   - In domain count, the general (non-native) union pays off mainly at admission: 4.35x against 2.21x at dispatch.
     In RSS, the admission-time lever is only as good as its handling of redirected later hits: 0.3-3.1x.
7. **The point space is saturated; the domain count is not [M].** The Route natives' point union grew 6.20e12 ->
   6.44e12 after g3 (+3.8%; seed 2: +3.5%). Gen-7 Route natives add 7.2-8.4 new points per native, and gen-7 admitted
   Route domains add 2.7-3.2. Sum over union is 7.2-7.8x for Route natives and 15-16x for admitted Route domains. Gen-7
   admissions are fragments of explored space: their mean size is 372 points, against 4.3M for g3 admissions.
8. **Validation constraint [M].** The four-loop controls FG, BMW, H and X have **0 Route domains**. A Route lever can
   be validated only on five-loop controls. C-5F has 712.6k Route natives, and 22.3 / 28.5 / 36.5% of its Route
   domains were covered at admission. On the four-loop runs it must be an identity no-op.

## 1. Inputs, binary, method

| item | value |
|---|---|
| receipt | `/common/dev/rustred/TMP/w0/routecensus/receipt/` (`batch.log` exit 0 at 00:53 UTC 2026-09-28; the gen7 step ran at 22:39-22:45 UTC 2026-09-27, see `batch-gen7step.log`) |
| census binary | `.claude/worktrees/fable51-rc/TMP/census-rc4`, sha256 `3859b3c0d2260f5c935d26bdcfd0601ac27d9c8b99a730493826dbc5690e60bd`, built from `1b90876c` with a clean tracked tree (`census-build.txt`); standalone crate, no Symbolica |
| Apply-native binary (`census route-apply`, §4b) | `.claude/worktrees/fable51-rc/TMP/census-rc6`, sha256 `67760c310562e128085580c82cac9f43c72f1b9fb0cd8d16d860caf3a998b963`, from `2a7e97d0` (`receipt/apply/census-build.txt`); outputs in `receipt/apply/` (`batch.log` exit 0 at 01:05 UTC). Its gen-7 JSON is identical to a first run with rc5 (`../apply-rc5-superseded/`, no rows file) |
| later-hit binary (`census route-hits`, §3b and §6b; fix round) | `.claude/worktrees/fable51-rc/TMP/census-rc8`, sha256 `53694a17f5036e80c1fba81c808cc4d777d84d24c0ffd1a6da1aedf45c5158b6`, from `4a23b78c` (`receipt/hits/census-build.txt`); outputs in `receipt/hits/` (`batch.log` exit 0 at 01:41 UTC 2026-09-28), tables in `receipt/hits/route_hits.md` (`route_hits.py` at `6db303af`). It re-reads the gen7 and C-5F rows files of `receipt/`. Its edge counts and pending model are identical to `gen7-route.json`. Foreign busy CPUs on the lane CPUs: 16-18 during these steps (counts, not timings) |
| checkpoints | block clones of v2 CP5 generations 7, 6 and 3 (`.claude/worktrees/fable51-rc/TMP/gen{7,6,3}`; each `latest.json` is byte-identical to `TMP/v2-checkpoint-copy-gen{7,6,3}`); C-5F clone of `TMP/fable51-controls/wave2-profile/w50-new-ready/five-finite/checkpoint` (`latest.json` identical) |
| heartbeat series | v2 run series from the W0.7 receipt (`receipt-v4/v2-series.txt`, copied); C-5F series from `receipt-v4/c5f/series.txt` |
| sampling | route pending: 400 uniform draws per (admission generation x points decade) stratum, Horvitz-Thompson weights; historical Route natives: 400 per (record generation x decade); historical Apply natives: 200 per (record generation x decade), 5,989 draws; admission census: 600 per (phase x admission generation); creator view: 4,000 draws per sample. Seed 20260928 |
| CPUs | 40-51,296-307, nice 19, 24 rayon threads. Foreign busy CPUs on these CPUs: 7.4-11.0 per step (other users' postgres/gammaboard and unpinned jobs; `*.metrics.txt`). Not recorded for the gen7 step, where the batch aborted in its schedstat sampler after the census exited 0 (script fixed in `0febb31f`). All outputs are counts from deterministic seeded samples, so foreign load changes wall times only |
| determinism | gen7, c5f and the saturation JSON are identical to the earlier receipts (rc2 / sat-rc3) apart from the new `anchors_used` fields. gen6 and gen3 differ in one float leaf by a relative 6e-16 |

**Anchor sets.** Every test is point-wise inside the (phase, owner) bucket of the query Q. Every lattice point of Q
is tested when |Q| <= 2e6; above that, 20,000 exactly sampled points are tested (0.3-0.7% of the weight, see §10).

| set | used for | meaning | status as a lever rule |
|---|---|---|---|
| `all_natives` | pending at resume | every Route native at the checkpoint | merged Native anchors (G2' rule S7 at resume) |
| `natives_before_dispatch` | historical natives | natives committed before Q's reconstructed dispatch (commit time - seconds - 30 s wait) | G2' rule |
| `natives_before_commit` / `natives_before_creator_commit` | historical / admission | natives committed before Q's own record / before the record that admitted Q | merged natives at that event |
| `earlier_non_delegated` | all | smaller IDs that are natives or still pending at the checkpoint | well-founded by ID; pending anchors need a protected flag so they are not later transferred |
| `all_earlier_ids` | all | every smaller ID (= every domain that existed at Q's admission) | upper bound: a delegated anchor's representative may be newer than Q |
| `all_other_domains` | pending | every other domain of the bucket | upper bound (includes newer IDs) |

**Weights.** Pending Route domains carry the mean seconds, successors, out-edges, created domains and descendants of
Route natives with the same (owner, rank bound) from record generations >= 6 [E]. The creation weights are
calibrated by coverage status in §3b. Historical natives carry their measured values. `created` counts
first-incoming edges: the creator of a domain is the source of its first incoming edge from a smaller ID.

## 2. Route natives: fan-out and creation (gen 7) [M]

| natives | count | seconds | median us | successors/native | succ p50 / p99 | edges->Route | edges->Apply | created Route/native | created Apply/native | created total | descendants/native (nested) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Route natives | 21.52M | 1951 | 40.7 | 25.31 | 8 / 219 | 24.34 | 0.972 | 0.660 | 0.098 | 16.31M | 6.44 |
| Apply natives | 5.95M | 241443 | 1714.7 | 890.97 | 72 / 13834 | 79.17 | 26.308 | 6.821 | 2.902 | 57.85M | 64.00 |

- The Route->Route edge distribution per Route native is p50 7, p90 63, p99 218, max 1,233. Route->Apply edges: 0 for
  603.5k natives, 1 for 20.91M, never more. Created Apply domains: 0 for 19.41M natives, 1 for 2.10M. Created Route
  domains: p50 0, p99 15, max 722.
- The graph has 1,192,281,291 edges. Of these, 1,172,088,707 are transition edges from inspected sources and
  994,663,820 of those point into Route. There are 18,425,291 alias edges and 1,767,293 self edges. The creator
  forest has 59 roots, and its depth has p50 8 and max 11.
- For admission requests, Route natives account for 544.6M of 1.172G distinct transition edges (46.5%). Their raw
  successor count is 544.6M; Apply's is 5.30G. Admission first tries an exact-digest lookup (`admit_with_lookup`,
  `walking/queue.rs`), so exact repeats never reach the containment search, and several successors can collapse onto
  one edge target. The containment-search load attributable to Route natives therefore lies between 9.3% and 46.5%
  [E].

| Route natives by record gen | count | seconds | successors/native | edges->Apply/native | created Route/native | created Apply/native | created Route | created Apply |
|---|---|---|---|---|---|---|---|---|
| g3 | 7.81M | 882 | 31.99 | 0.964 | 0.896 | 0.094 | 7.00M | 735.5k |
| g4 | 4.86M | 436 | 25.16 | 0.974 | 0.554 | 0.086 | 2.69M | 419.0k |
| g5 | 4.17M | 286 | 18.99 | 0.980 | 0.503 | 0.107 | 2.10M | 447.8k |
| g6 | 3.33M | 274 | 22.77 | 0.972 | 0.487 | 0.090 | 1.62M | 300.4k |
| g7 | 1.34M | 73 | 12.96 | 0.986 | 0.596 | 0.149 | 801.1k | 200.1k |

Creator of every domain, by target phase and class:

| target phase / class | domains | by Route natives | share | by Apply natives | share |
|---|---|---|---|---|---|
| Apply / delegated | 7.57M | 676.2k | 8.9% | 6.89M | 91.1% |
| Apply / native | 5.95M | 763.7k | 12.8% | 5.19M | 87.2% |
| Apply / pending | 5.85M | 662.9k | 11.3% | 5.18M | 88.7% |
| Route / delegated | 18.55M | 4.86M | 26.2% | 13.69M | 73.8% |
| Route / native | 21.52M | 6.04M | 28.1% | 15.47M | 71.9% |
| Route / pending | 14.72M | 3.31M | 22.4% | 11.42M | 77.6% |

The only domains without a creator are the 59 initial domains, plus 31 (Apply) and 53 (Route) pending domains whose
creator is a non-native (an alias edge source).

## 3. Route native-pending at resume (gen 7) [M; predicted weights E]

Population 14,724,634 domains, 18 strata, 6,791 distinct draws.

Fully covered share, by weight:

| anchor set | count | pred seconds [E] | pred successors [E] | pred created [E] | pred created Apply [E] | pred created Route [E] | points |
|---|---|---|---|---|---|---|---|
| all_natives | 45.9% | 45.7% | 45.0% | 51.8% | 49.1% | 52.4% | 25.0% |
| earlier_non_delegated | 58.2% | 56.3% | 55.3% | 63.6% | 62.7% | 63.8% | 31.2% |
| all_earlier_ids | 63.0% | 61.1% | 60.1% | 67.6% | 67.2% | 67.7% | 45.1% |
| all_other_domains | 69.4% | 67.0% | 66.1% | 73.8% | 74.0% | 73.8% | 51.2% |

Bootstrap standard errors (200 stratified replicates, `gen7-boot.md`): count 0.76-0.83 pp; pred created 1.33 pp
(all_natives). The "pred created" columns are uncalibrated and overstated by 1.4-2.3x. §3b gives the calibrated shares
(22.1 / 31.8 / 36.0 / 43.1%).

Residual view, count weighted. The gate is: residual at most 10% of |Q|, in at most 8 pieces.

| anchor set | fully covered | mean uncovered fraction | gate D-only | gate D-only <= 2 pieces | gate hull | gate A/R (C2) | gate exact | unevaluated |
|---|---|---|---|---|---|---|---|---|
| all_natives | 45.9% | 10.2% | 53.9% | 53.9% | 68.3% | 62.7% | 69.6% | 0.00% |
| earlier_non_delegated | 58.2% | 7.1% | 66.3% | 66.3% | 78.0% | 73.7% | 78.6% | 0.00% |
| all_earlier_ids | 63.0% | 6.2% | 70.1% | 70.1% | 81.0% | 76.8% | 81.6% | 0.00% |
| all_other_domains | 69.4% | 5.1% | 75.5% | 75.5% | 85.2% | 81.7% | 85.4% | 0.00% |

- **Pieces in partial covers.** Of the partially covered domains (all_natives, estimated 7.96M), all but about
  2,973 need a single D-only piece; those need 2. A/R cuts need 1 to 5 pieces. The median residual fraction is 0.220 with D-only cuts,
  0.093 with the hull and 0.077 exact (`tables.md`).
- **By admission generation**, fully covered by all_natives / all_earlier_ids (count): g5 47.4 / 60.3%, g6 40.2 /
  59.3%, g7 53.6 / 75.8%.
- **Owners.** The pending is spread over 1,123 Route owners in the sample; the largest holds 2.0% (`gen7-boot.md`).
  No hot Route owner exists.
- **Projections at resume [E].** The covered pending Route (all_natives) is 6.76M domains with 125-129M predicted
  successors. Its calibrated first-level creations are **2.0-2.9M (1.0-2.4 GB)**. With all_other_domains these rise to
  10.2M domains, 183-187M successors and **3.1-3.9M creations (1.6-3.2 GB)**. The uncalibrated 4.19M / 5.98M
  (2.1-3.5 / 3.0-5.0 GB) of the first version of this note were overstated (§3b).

### 3b. Calibrated creation weights of the pending (fix round) [E; ratios M]

The pending predictor takes per-(owner, rank bound) means over the Route natives of record generations 6-7. Within a
group, covered natives create fewer domains than uncovered ones. The predictor cannot see this, so it over-weights the
covered pending. On the historical g6-g7 Route natives, with a leave-one-out predictor (`receipt/hits/route_hits.md`):

| historical rule | count share covered | measured creation share | predicted creation share | R covered | R not covered |
|---|---|---|---|---|---|
| natives before dispatch | 51.1% | 25.0% | 55.8% | 0.472 | 1.781 |
| earlier non-delegated | 54.6% | 27.0% | 58.2% | 0.489 | 1.833 |
| all earlier IDs | 60.1% | 31.3% | 62.9% | 0.524 | 1.945 |

**Calibration.**
- R is the measured / predicted creation ratio per coverage status.
- The calibrated weight of a pending domain is its predicted creations x R of its own status, under the mapped
  historical rule. The mapping: all_natives -> natives before dispatch; earlier_non_delegated -> itself;
  all_earlier_ids and all_other_domains -> all earlier IDs.
- Successor weights need no calibration: their R is 1.02-1.04 for both statuses.

**Out-of-sample check.** Calibrating on record g6 and predicting g7 gives 28.5% covered creations, against 38.7%
measured and 65.4% uncalibrated. Calibration removes most of the bias but under-predicts by about 10 pp.

| pending anchor set | uncalibrated creation share | calibrated [E] (± bootstrap SE) | with the +10 pp out-of-sample offset | covered creations (calibrated to calibrated + offset) | bytes at 0.5-0.84 KB |
|---|---|---|---|---|---|
| all_natives | 51.8% | **22.1% ± 2.7** | 32.3% | 2.0-2.9M | 1.0-2.4 GB |
| earlier_non_delegated | 63.6% | **31.8% ± 3.2** | 43.0% | 2.5-3.4M | 1.3-2.9 GB |
| all_earlier_ids | 67.6% | **36.0% ± 3.4** | 45.9% | 2.9-3.7M | 1.4-3.1 GB |
| all_other_domains | 73.8% | **43.1% ± 3.8** | 53.0% | 3.1-3.9M | 1.6-3.2 GB |

The same bias touches any pending weight that a coverage-blind group mean predicts. The descendants weight shares the
creation predictor's structure and is not used for any headline figure. The pending-by-creator view of §5 is unaffected:
it weights Apply by the W0.7 cost law.

## 4. Historical Route natives (gen 7) [M]

Population 21,516,094, 33 strata, 12,041 distinct draws.

| anchor set | count | seconds | successors | out_edges | created | created_apply | created_route | descendants | points |
|---|---|---|---|---|---|---|---|---|---|
| natives_before_dispatch | 41.7% | 41.1% | 39.7% | 39.7% | 20.7% | 26.1% | 19.8% | 15.4% | 24.0% |
| natives_before_commit | 46.7% | 47.3% | 45.9% | 45.9% | 22.1% | 28.5% | 21.2% | 16.6% | 59.8% |
| earlier_non_delegated | 46.7% | 47.3% | 45.9% | 45.9% | 22.1% | 28.5% | 21.2% | 16.6% | 62.6% |
| all_earlier_ids | 53.4% | 53.7% | 52.5% | 52.5% | 29.2% | 36.9% | 28.0% | 21.8% | 76.6% |

Standard errors: count 0.58 pp, successors 1.07 pp, created 2.2-2.6 pp. Count-weighted gates (natives before
dispatch): D-only 48.1%, hull 60.8%, A/R 56.1%. D-only partials need 1 piece (12.48M) to 3 pieces (328). Unevaluated
draws (infinite domains) are 0.22%.

By record generation (natives_before_dispatch, count / successors / created):

| gen | count | successors | created |
|---|---|---|---|
| g3 | 30.1% | 31.5% | 18.9% |
| g4 | 45.1% | 43.6% | 19.4% |
| g5 | 49.0% | 53.2% | 21.7% |
| g6 | 46.7% | 42.3% | 18.3% |
| g7 | 61.8% | 60.1% | 38.7% |

**Dispatch-wait sensitivity (`gen7-route-w300.json`).** A 300 s margin instead of 30 s gives 39.7% count, 37.2%
successors, 19.5% created, D-only gate 46.0% and hull 58.2%.

## 4b. Historical Apply natives with creation weights (gen 7, `census route-apply`) [M]

W0.7 measured Apply coverage in CPU and inspection units. This block adds the domains the natives created, so that
the indirect volume effect of G2' can be read next to the Route one. Population 5,949,328 Apply natives, 33 strata,
5,989 distinct draws. Standard errors are from `gen7-boot.md`.

| anchor set | count | successors | out_edges | created | created Apply | created Route | descendants | points |
|---|---|---|---|---|---|---|---|---|
| natives_before_dispatch | 56.8% ± 0.9 | 52.1% ± 5.0 | 57.6% | **37.3% ± 1.8** | 39.0% | 36.6% | 8.1% | 49.6% |
| natives_before_commit | 61.7% | 55.7% | 62.3% | 42.9% | 44.5% | 42.2% | 10.3% | 76.4% |
| earlier_non_delegated | 61.7% | 54.9% | 62.1% | 42.8% ± 1.8 | 44.3% | 42.1% | 10.2% | 76.3% |
| all_earlier_ids | 66.6% | 68.1% | 69.7% | 50.5% ± 2.0 | 52.2% | 49.7% | 11.8% | 84.3% |

- **Cross-check with W0.7.** W0.7's `hist_uniform` measured 57.1% fully covered, D-only gate 59.8% and hull 72.7%.
  This run gives 56.8%, 59.6% and 72.5%.
- **CPU weight.** Seconds weights from uniform strata are noisy for a heavy-tailed cost (45.5% here). Use W0.7's PPS
  figure (61.6%) for CPU.
- **By record generation** (natives before dispatch, count / created): g3 51.4 / 27.4%, g4 56.7 / 42.9%, g5 58.6 /
  44.6%, g6 55.3 / 32.7%, g7 74.3 / 55.6% (± 3.2).
- **C-5F** (`receipt/apply/c5f-route-apply.json`): 43.2% count, 16.5% created.

The table in `tables.md` ("Dispatch-time full-cover alias vs admission-time union cover") combines these creation
shares with the Route ones of §4 and with the admission census of §6.

## 5. Domain-creation view: pending domains by the coverage of their Route-native creator (gen 7) [M; PPS weights E]

| sample (4,000 draws) | population | creator Route native | creator covered: before dispatch | creator covered: all earlier IDs | self covered: all natives | self covered: all earlier IDs |
|---|---|---|---|---|---|---|
| pending Apply, PPS by predicted Apply seconds [E] | 5.85M | 58.4% | 16.9% | 21.3% | 55.3% | 74.0% |
| pending Apply, uniform | 5.85M | 11.1% | 3.2% | 4.3% | 55.3% | 73.5% |
| pending Route, uniform | 14.72M | 22.8% | 5.0% | 6.8% | 46.5% | 64.2% |

"Self covered" is the coverage of the pending domain itself in its own bucket. The two Apply samples give the same
all_natives share (55.3%) by coincidence: they draw different domains, and the all-earlier-IDs shares differ, as
do the gen-6 shares (56.1% vs 53.0%).

Joint shares for the PPS Apply sample. This is the part of pending Apply CPU [E] that Route coverage adds beyond an
Apply-side cover:

| creator covered by | self covered by | creator covered, self covered | creator covered, self **not** covered |
|---|---|---|---|
| natives_before_dispatch | all_natives | 15.0% | 1.9% |
| natives_before_dispatch | all_earlier_ids | 16.8% | 0.1% |
| all_earlier_ids | all_natives | 16.6% | 4.7% |
| all_earlier_ids | all_earlier_ids | 20.0% | 1.4% |

## 6. Every domain at its admission (gen 7) [M; cascade E]

Population 74.16M admitted (non-initial) domains, 11 strata, 6,008 distinct draws, 23,955 ancestors evaluated
(chains of up to 64 levels, none truncated).

| phase | anchor set | domains | covered at admission | ancestor covered only | self or ancestor [E upper] |
|---|---|---|---|---|---|
| Apply | natives_before_creator_commit | 19.37M | 8.36M (43.1%) | 90.3k (0.5%) | 8.45M (43.6%) |
| Apply | earlier_non_delegated | 19.37M | 12.86M (66.4%) | 710.0k (3.7%) | 13.57M (70.1%) |
| Apply | all_earlier_ids | 19.37M | 13.93M (71.9%) | 1.79M (9.3%) | 15.72M (81.2%) |
| Route | natives_before_creator_commit | 54.79M | 18.03M (32.9%) | 631.6k (1.2%) | 18.66M (34.1%) |
| Route | earlier_non_delegated | 54.79M | 27.79M (50.7%) | 2.94M (5.4%) | 30.73M (56.1%) |
| Route | all_earlier_ids | 54.79M | 31.94M (58.3%) | 6.90M (12.6%) | 38.84M (70.9%) |

By admission generation (count; 600 draws per cell, SE about 2 pp):

| phase / admission gen | domains | natives before creator commit | earlier non-delegated | all earlier IDs |
|---|---|---|---|---|
| Apply g3 | 5.70M | 35.5% | 58.0% | 64.5% |
| Apply g4 | 4.51M | 42.0% | 66.0% | 72.5% |
| Apply g5 | 4.26M | 45.5% | 68.8% | 74.7% |
| Apply g6 | 3.27M | 45.8% | 69.3% | 73.2% |
| Apply g7 | 1.63M | 61.5% | 84.5% | 86.5% |
| Route g3 | 23.10M | 24.5% | 40.7% | 51.2% |
| Route g4 | 11.13M | 34.8% | 53.8% | 59.7% |
| Route g5 | 9.48M | 43.2% | 59.0% | 63.8% |
| Route g6 | 7.42M | 33.8% | 55.5% | 61.7% |
| Route g7 | 3.66M | 51.7% | 73.7% | 78.0% |

**Consistency check.** The gen-3 checkpoint measures the same g3 admissions at 24.5 / 40.2 / 50.2% (Route) and
36.0 / 60.2 / 66.3% (Apply). This agrees within sampling error, although only the merged-native set is independent
of the checkpoint's final states.

**Cost of a union cover [M].** A fully covered domain uses, by greedy first-hit count, the following numbers of
anchors. The greedy first-hit count is an upper bound on the minimum cover.

| sample | anchor set | mean | p50 / p90 / p99 / max | covers with <= 4 anchors |
|---|---|---|---|---|
| admission | natives_before_creator_commit | 12.9 | 9 / 26 / 70 / 198 | 17.8% |
| admission | earlier_non_delegated | 13.4 | 9 / 28 / 75 / 198 | 17.7% |
| admission | all_earlier_ids | 19.3 | 11 / 38 / 123 / 437 | 14.3% |
| Route pending | all_natives | 14.6 | 15 / 54 / 165 / 420 | 13.8% |
| Route pending | all_other_domains | 21.4 | 22 / 103 / 335 / 951 | 10.2% |
| Route natives | natives_before_dispatch | 14.8 | 13 / 48 / 157 / 994 | 17.2% |

Candidate anchors are those whose box and aggregate ranges meet Q. The union test at admission of a Route domain meets:

| anchor set | candidates p50 | candidates p99 |
|---|---|---|
| merged natives | 162 | 6,064 |
| earlier non-delegated | 267 | 10,589 |
| all earlier IDs | 436 | 17,854 |

For Apply the p50 is 350 / 691 / 1,058 (`route_rows.py`, `receipt/gen7-rows-quantiles.txt`). Admitted domains
are small: median 40 points, p90 2,244 (Route), p99 234k.

### 6b. Later hits on avoided domains (fix round) [M; RSS factors E]

A domain that an admission-time union cover never creates cannot receive the requests that later land on it. Today
those requests are exact-digest or single-container hits on the domain. Under the lever each needs its own admission:
- a single-container hit on another domain, with k' = 1 anchor and no extra edge; or
- a union test and a k'-anchor alias, with k' at most the a anchors of the domain's own cover.

`census route-hits` (`receipt/hits/`) counts each drawn domain's later hits: distinct transition in-edges from
inspected sources plus alias in-edges, excluding its creator edge and self edges.

**The later hits are heavy-tailed [M].** An exact pass over every admitted domain gives:
- 17.2 later transition in-edges per Route domain and 7.6 per Apply domain. The uniform admission sample estimates 8.3
  and 5.3: it misses the hubs.
- A strong dependence on age: Route admissions of g3 average 38.3, those of g7 0.5, whose later hits are censored.
- 1,104.8M later hits in total: 953.6M on Route domains, 151.1M on Apply domains.

So the redirected total comes from a second sample. 4,000 admitted domains per phase are drawn with probability
proportional to their later hits. The share of those draws that is covered, times the exact total, estimates the hits
that land on covered domains (Hansen-Hurwitz):

| phase | anchor set at admission | covered domains | share of later hits on covered domains | redirected later hits (history) | per covered domain h | h from the uniform sample |
|---|---|---|---|---|---|---|
| both | merged natives before the creator's commit | 26.4M | 7.8% ± 0.3 | **86.3M ± 3.7M** | **3.27** | 3.08 |
| both | earlier non-delegated | 40.6M | 17.6% ± 0.5 | **194.6M ± 5.4M** | **4.79** | 3.87 |
| both | every smaller ID | 45.9M | 44.9% ± 0.7 | **496.6M ± 7.6M** | **10.83** | 4.81 |
| Route | merged natives before the creator's commit | 18.0M | 5.9% ± 0.4 | 56.7M | 3.15 | 3.01 |
| Route | earlier non-delegated | 27.8M | 14.0% ± 0.5 | 133.0M | 4.79 | 3.59 |
| Route | every smaller ID | 31.9M | 43.2% ± 0.8 | 412.4M | 12.91 | 4.86 |

Notes on the table:
- About half of the covered domains (45-51%) receive no later hit.
- The hubs that "every smaller ID" covers weigh heavily. That set is the upper bound and includes delegated anchors.
- 4.2% (Route) and 1.5% (Apply) of the hit weight lies on unevaluated (infinite) domains. It counts as not covered.
- The merged-natives set is not nested in the every-smaller-ID set for 0.01% of the hits (initial-band IDs).

**Admission requests [E, first level].** Today the 74.2M misses are the only requests that reach a union test. Under
the lever, every redirected hit first needs a single-container search elsewhere. When that fails (k' > 1), it also
needs a union test. The counts below are distinct edges, a lower bound on requests: repeats from one source collapse
onto one edge.

| anchor set | union tests: today's misses only (every redirected hit single-contained) | union tests: misses plus every redirected hit | upper bound / today's misses |
|---|---|---|---|
| merged natives | 74.2M | 160.5M | 2.16x |
| earlier non-delegated | 74.2M | 268.8M | 3.62x |
| every smaller ID | 74.2M | 570.7M | 7.70x |

**Net RSS at the late (admission g7) mix [E].** The factor is 1 / ((1 - c) + c x e / B), where:
- c is the late avoided share;
- B = 0.5-0.84 KB per domain and e = 8-16 B per edge;
- x is the extra edges per avoided domain.

For k' = 1, x = E[a - 1], the cover's own anchors, from the uniform sample. For k' = a (worst case),
x = E[a - 1] + sum over covered domains of (a - 1) h / N_cov, with the sum taken from the PPS sample.

| lever | anchor set | c | factor, no edges | x at k' = 1 | x at k' = a | net at k' = 1 | net at k' = a | edge bytes per avoided domain, k' = 1 / k' = a |
|---|---|---|---|---|---|---|---|---|
| both phases | merged natives | 54.7% | 2.21x | 11.9 | 66.8 | 1.51-1.94x | 0.62-1.25x | 95-190 / 534-1,068 B |
| both phases | earlier non-delegated | 77.0% | 4.35x | 12.4 | 111.8 | 1.86-3.11x | 0.34-0.95x | 99-199 / 894-1,789 B |
| both phases | every smaller ID | 80.6% | 5.16x | 18.3 | 629.5 | 1.50-2.99x | 0.06-0.20x | 146-293 / 5,036-10,072 B |
| Route only | merged natives | 35.8% | 1.56x | 12.1 | 63.4 | 1.28-1.46x | 0.73-1.16x | 97-193 / 507-1,015 B |
| Route only | earlier non-delegated | 51.0% | 2.04x | 12.7 | 107.3 | 1.43-1.81x | 0.45-0.99x | 102-204 / 858-1,716 B |
| Route only | every smaller ID | 54.0% | 2.17x | 19.8 | 784.9 | 1.25-1.78x | 0.07-0.22x | 159-317 / 6,279-12,559 B |

The lever saves RSS only while x e < B, i.e. while x stays below 31-105 extra edges per avoided domain.

**Reading.**
- k' is not measured: the checkpoint records edges, not the request boxes. Most redirected hits land on old, large
  hub domains.
- A sub-request of a hub may well fit inside a single one of the hub's anchors (k' = 1), so k' = a is pessimistic
  there. It is still the bound the census can defend.
- With merged-native anchors, the stated net range is 0.6-1.9x.
- Measuring k' needs the request boxes. Two ways to get them: re-run the in-edge sources through the W0.3 native
  re-inspection harness, or have the legacy falsifier's admission arm log, per redirected hit, whether a single
  container was found and how many anchors a union alias used.
- A compact alias record would bound the later-hit cost. It is keyed by Q's box, lives in the index and stores the
  anchor list once, so later hits land on it exactly as they land on Q today. But it gives back part of the per-domain
  saving (B minus the record's bytes; not measured).

## 7. Route point-space saturation (gen 7 checkpoint) [M, sampled]

4,000 PPS-by-points draws per window; two seeds, `gen7-route-saturation{,-seed2}.json`.

| window | Route natives: members | sum points | new points per native (seed 1 / seed 2) | union after (seed 1 / seed 2) |
|---|---|---|---|---|
| g3 | 7.81M | 4.62e13 | 794.0k / 863.4k | 6.20e12 / 6.74e12 |
| g4 | 4.86M | 1.66e12 | 21.0k / 19.5k | 6.30e12 / 6.84e12 |
| g5 | 4.17M | 4.14e11 | 2,183 / 2,778 | 6.31e12 / 6.85e12 |
| g6 | 3.33M | 2.10e12 | 37.1k / 39.3k | 6.44e12 / 6.98e12 |
| g7 | 1.34M | 6.95e8 | 7.24 / 8.4 | 6.44e12 / 6.98e12 |

- **Admitted Route domains by domain segment.** g7 has 3.66M domains with 1.36e9 points. Their new fraction is 0.73%
  / 0.85% of points, which is 2.7 / 3.2 new points per domain. The union after g7 is 7.33e12 / 6.80e12.
- **Uniform-domain new fraction.** 3.2% / 2.9% of gen-7 admitted Route domains contain a new point at their random
  point.
- **Overlap.** Sum over union is 7.2-7.8x for Route natives (5.04e13 points) and 15-16x for admitted Route domains
  (1.09e14 points), over the two seeds. This matches
  the Apply picture of W0.7: the union grew +3.45% after g3, and gen 7 added 7.3 new points per native.
- **g6 is not monotone.** On both sides g6 added 1.2e11 new Route points, more than g5.

## 8. Controls [M]

| checkpoint | Route pending fully covered (natives / non-delegated / earlier IDs / all other) | historical Route natives covered before dispatch (count / created) | Route covered at admission (natives / non-delegated / earlier IDs) |
|---|---|---|---|
| v2 gen 7 | 45.9 / 58.2 / 63.0 / 69.4% | 41.7 / 20.7% | 32.9 / 50.7 / 58.3% |
| v2 gen 6 | 44.5 / 57.0 / 61.1 / 66.7% | 39.6 / 20.9% | 31.3 / 51.6 / 59.7% |
| v2 gen 3 | 36.8 / 48.7 / 56.1 / 64.6% | 32.2 / 19.4% | 24.5 / 40.2 / 50.2% |
| C-5F (drained, no pending) | - | 28.8 / 9.7% | 22.3 / 28.5 / 36.5% |
| four-loop FG, BMW, H, X | 0 Route domains (`TMP/w0/census/receipt-v4/four-loop/*-stats.json`) | - | - |

C-5F details:
- 712.6k Route natives (12 s, median 10.1 us, 4.73 successors per native); Apply covered at admission 34.0 / 46.7 /
  55.2%.
- Covered-ancestor cascade on Route [E upper]: 28.5 / 36.7 / 69.0%.
- Its partial Route covers are poor: mean uncovered fraction 51% against natives before dispatch.

## 9. Conclusion for D2 and the RAM wall [E unless marked]

### 9.1 Answer: domains and bytes a Route union-cover lever could avoid

The figures are first-level estimates on v2's actual history of 74.2M domains [E]. The underlying shares are [M].

| Route lever | domains avoided (share of all domains) | bytes at 0.5-0.84 KB/domain | gen-7 mix: share of new domains avoided; creation-rate factor |
|---|---|---|---|
| at resume: alias the covered Route pending (all natives / all other domains) | 2.0-2.9M / 3.1-3.9M calibrated creations (§3b; uncalibrated 4.2M / 6.0M); the 6.8M / 10.2M covered Route jobs and their 125-129M / 183-187M successors are not run | 1.0-2.4 / 1.6-3.2 GB | - |
| at dispatch: alias fully covered Route jobs (merged natives) | 3.4M (4.5%) | 1.7-2.8 GB | 7.3%; 1.08x |
| at admission: never create a covered Route successor (merged natives / earlier non-delegated / every smaller ID) | 18.0M / 27.8M / 31.9M (24.3 / 37.5 / 43.1%) | 9.0-15.1 / 13.9-23.3 / 16.0-26.8 GB gross; net of anchor edges and redirected later hits the Route-only RSS factor is 0.7-1.5 / 0.45-1.8 / 0.07-1.8x (§6b) | 35.8 / 51.0 / 54.0%; 1.56 / 2.04 / 2.17x in domains |
| same, with the covered-ancestor cascade [E upper] | 18.7M / 30.7M / 38.8M | 9.3-15.7 / 15.4-25.8 / 19.4-32.6 GB | - |

For scale: v2 at generation 7 holds 74.2M domains, 37-62 GB at 0.5-0.84 KB. 600 GB holds about 0.75-1.2G domains
(handoff §0.1 item 4).

A creation-rate factor f means that about f times more exploration fits under the cap before the RAM wall, at the
same bytes per domain. The byte columns are gross: they do not subtract the edges a lever adds.
- At dispatch, those are the alias's anchor edges.
- At admission, they are the cover's anchor edges plus k' anchors for each of the 3.3-10.8 later hits that an avoided
  domain would have received (§6b).

With them the admission-time RSS factor is a range whose floor is below 1x (§0 item 6).

### 9.2 What this means for the three-way D2 choice (none / G2' / general union)

**none.** At gen 7, Route produces 69% of new domains (3.66M of 5.28M admissions [M]). The RAM wall is approached at
v2's domain-creation rate per unit of exploration.

**G2' as specified** (plan §3.10: Apply jobs, dispatch-time, merged-native anchors, residual jobs):
- **CPU.** It is an Apply-CPU lever: W0.7 projects 0.10-0.13x of Apply seconds [E].
- **Volume.** This effect is new here. The full-cover part (empty residual, alias) stops the successors of 56.8% of
  Apply jobs [M]. Those jobs made 37.3% of Apply-native creations, including the Route domains they would have
  created. That is 21.6M domains (29.1% of all, 10.8-18.1 GB). At the gen-7 mix it is 45.1% of new domains, a 1.82x
  creation-rate factor [E, first level]. Residual jobs add an unmeasured further cut, because a smaller residual image
  means fewer successors.
- **Why Route is not untouched.** Route is 74% of domains, but G2' reaches it: Apply jobs create 72-78% of every
  Route class [M].
- **Route extension at dispatch.** Aliasing fully covered Route jobs adds little volume: 3.4M domains, and at the late
  mix 2.10x instead of 1.82x. It removes about 40% of Route successor admissions, about 220M edges, 19% of all
  transition edges [E]. So it is an admission-load lever for the coordinator, not a RAM lever.

**General union:**
- **At dispatch** (well-founded, earlier non-delegated anchors): 28.4M domains (38.2%); late 2.21x. That is little
  beyond G2'.
- **At admission, both phases:**

  | anchors | domains avoided | bytes (gross) | late factor (domains) | late RSS factor, net of the cover's anchor edges (k' = 1) | late RSS factor, redirected hits need the full cover (k' = a) | redirected later hits (history) |
  |---|---|---|---|---|---|---|
  | merged natives | 26.4M (35.6%) | 13.2-22.2 GB | 2.21x | 1.5-1.9x | 0.62-1.25x | 86M |
  | earlier non-delegated | 40.6M (54.8%) | 20.3-34.1 GB | 4.35x | 1.9-3.1x | 0.34-0.95x | 195M |
  | every smaller ID (upper bound) | 45.9M (61.9%) | 22.9-38.5 GB | 5.16x | 1.5-3.0x | 0.06-0.20x | 497M |

  Route carries 68-70% of the domains avoided at admission. The RSS gain of the admission placement depends on how
  the lever treats the requests that would have landed on the avoided domains (§6b). The census cannot pin this: k'
  lies between 1 and a.

**Placement barely matters with merged-native anchors, and anchors matter at admission.**
- With merged-native anchors, the first-level volume effect is 2.10x at dispatch (Apply and Route jobs) and 2.21x at
  admission. The two sets overlap heavily: a domain covered at admission is also covered at its dispatch, and the
  children of covered jobs are mostly covered at their own admission.
- G2' with its alias already captures most of the merged-native volume. What admission adds is that the covered domain
  itself is never stored. That record is also the target of 3.3 later hits on average (merged natives, §6b), so not
  storing it has a price: 86M redirected requests over v2's history, and up to 534-1,068 B of extra edges per avoided
  domain if they need the full cover.
- The extra volume of the general union comes from anchors that are pending (non-native), used at admission: 4.35x
  against 2.21x at the late mix, in domains. That is the substance of D2's "general union" option, and Route is where
  most of those extra domains are: late Route coverage goes from 51.7% to 73.7% [M].
- In RSS the same option ranges over 0.34-3.11x (§6b). The general union also covers the hubs that later requests hit:
  195M redirected hits against 86M for merged natives.

**Implications for the D-session.**

1. **Present D2 as placement x anchor rule, with volume numbers.**
   - Placement: dispatch (G2', CPU plus volume) or admission (volume plus the covered record itself).
   - Anchor rule: merged natives, which S7 allows today; or well-founded earlier non-delegated IDs, which need a per-ID
     protected flag (plan §3.10 "census P-anchors") and an ID-order well-foundedness argument; or none.
   - Offer G2' with merged natives as the base. Offer admission-time union cover with earlier non-delegated anchors as
     the volume option: 2.21x -> 4.35x at the late mix in domains [E].
   - Say plainly that its RSS effect is 0.34-3.11x net of edges [E] until k' is measured. Below 1x, the lever costs
     RAM.
   - Residual vocabulary is irrelevant for Route: D-only cuts are 1 piece, and Route transport has no lower bounds.
2. **Criterion (G) and the work-factor definition** (critique TERM-2) should count avoided domains and avoided
   admission requests. Any Route lever has no inspector-seconds effect by construction.
3. **Before any code.**
   - An exact "Q subset of union of anchors" predicate (§0.1 item 6), independent of C2, that also serves admission.
     The census predicate samples above 2e6 points.
   - Its cost [M]. At admission, a union test meets a median of 162-436 candidate Route anchors (p99 6k-18k) and
     350-1,058 Apply anchors, and covers use a median of 9-22 anchors. Its CPU per request is unmeasured.
   - How often it runs [E, first level]. It runs on today's 74.2M misses, plus the later hits redirected from
     avoided domains: 86M / 195M / 497M distinct edges [M, PPS]. Each redirected hit first needs a single-container
     search elsewhere. When that fails, it needs a union test. Up to 160M / 269M / 571M union tests, 2.2x / 3.6x /
     7.7x today's misses, where today 96.15% of requests are containment hits (handoff §5.2). Admission is the legacy
     coordinator's bottleneck.
   - Anchor edges [E].
     - The cover's own edges cost 95-293 B per avoided domain, at k' = 1.
     - Redirected hits that need the full cover raise this to 534-1,068 / 894-1,789 / 5,036-10,072 B, against the
       500-840 B saved (§6b).
     - Store the minimum cover, not the greedy first-hit set.
     - Measure k' before sizing the lever (§6b "Reading").
     - Consider a compact alias record keyed by Q's box: it keeps later hits at one edge, but gives back part of the
       per-domain saving.
4. **Falsifier.** The early env-gated legacy G2' falsifier (§0.1 item 3: merged-native anchors, residual
   re-inspected, anchor edges) will measure the dispatch-time volume effect directly.
   - This census predicts 37% fewer Apply-native creations at first level on the gen-7 history, and 16.5% at C-5F's
     maturity.
   - Add an admission-time arm (both phases, merged-native anchors; then earlier non-delegated). It should log, per
     redirected request, whether a single container was found and how many anchors a union alias used (k'). It should
     also record the admission requests and union tests per completion.
   - Controls: C-5F, the only drained control with Route (712.6k Route natives), and a C-HOT-sub box, within 1 h. The
     Route content of C-HOT-sub was not checked here.
   - Measure scheduled domains, peak pending, pending growth per completion, new points per native, admission CPU per
     request and marginal RSS/domain.
   - The four-loop runs have no Route domains and can only confirm the no-op or identity.
5. **Inputs interact.** I2 cut Route->Route edges by 56% in a W6 pilot (handoff §9.2). The figures here are for v2
   inputs, so re-run `census route` and `census route-apply` on the first v4-input checkpoint before sizing the lever.

## 10. Caveats

- **Historical shares are measurements on v2's actual history.** Every avoided-domain, byte and rate-factor figure
  is [E]: a lever changes the downstream history. The ancestor cascade is an upper bound, since a descendant can be
  re-created by another source.
- **Predicted pending weights are [E].** They are per-(owner, rank bound) means over record generations 6-7:
  pooled 19.9 successors, 0.625 created and 74 us per Route native.
  - The creation weights are coverage-blind and overstate the covered share by 1.4-2.3x. §3b calibrates them by
    coverage status.
  - The calibration's out-of-sample check under-predicts by about 10 pp, so the calibrated shares are read with a
    +10 pp band.
  - Successor weights are not affected: their calibration ratios are 1.02-1.04. Seconds weights were not
    re-checked. Historically, covered Route natives carry about their count share of seconds (§4).
- **Sampled verdicts** (|Q| > 2e6 points, 20,000 sampled points) are 0.30% of the pending weight, 0.72% of the native
  weight and 0.52% of the admission weight. "Fully covered" may be overestimated for them, but they make up at most
  0.32 pp of any fully covered share (`gen7-boot.md`). Infinite domains count as unevaluated: 0.22% of the historical
  natives and 0.00% of the pending.
- **`natives_before_dispatch` reconstructs dispatch** as commit time - seconds - wait on the heartbeat series. Route
  natives take about 40 us, so the wait dominates; 300 s instead of 30 s moves the count share 41.7 -> 39.7%.
- **The greedy first-hit anchor count is an upper bound** on the minimum cover size, and for sampled queries it counts
  only the sampled points.
- **Bytes per domain.** 0.5-0.84 KB/domain is the brief's range; handoff §0.1 item 4 gives 0.5-1.1 KB marginal and
  0.64 KB restore VmHWM. Edge bytes of 8-16 B are an assumption. CP5 stores 8 B per edge; the in-memory layout was
  not measured here.
- **The gen-7 checkpoint is a pause mid-generation.** "g7" rows describe a partial generation.
- **Later hits (§6b).**
  - They are distinct edges, a lower bound on redirected requests.
  - h is measured over v2's history. A late admission's own future hits are not observed: g7 admissions have 0.5 so
    far, g3 admissions 38.
  - The PPS sample leaves 4.2% (Route) / 1.5% (Apply) of the hit weight on unevaluated (infinite) domains, counted
    as not covered. The redirected totals are lower bounds by at most that much.
  - k' (anchors per redirected hit) is not measured. The stated RSS ranges span k' = 1 to k' = a.
- **Redirected hits also touch the dispatch placement.** A covered job that emits no successors never creates its
  children. A later request that would have hit such a child needs a container elsewhere. Without an admission-time
  union test, it is admitted as a new domain when no single container holds it. The dispatch-time avoided counts
  are upper bounds for this reason too.
- **Dispatch-time avoided creations** count the first-level creations of covered jobs. Under the lever those
  successors are not emitted, and their points are covered by the anchors' own successors. Another source may still
  create an equivalent domain later, so these counts are an [E] upper bound per level, as the admission-time counts
  are.
- **Apply seconds weights in §4b** come from uniform strata and are noisy for a heavy-tailed cost. W0.7's PPS sample
  is the CPU estimate.

## 11. Reproduction

```
# build (standalone crate; flock the shared build lock, check MemAvailable >= 150 GiB first)
cd .claude/worktrees/fable51-rc/tools/research/census
flock -w 14400 /common/dev/rustred/TMP/locks/build-2.lock nice -n 5 taskset -c 40-51,296-307 \
  env TMPDIR=$PWD/../../../TMP CARGO_TARGET_DIR=$PWD/../../../TMP/census-target \
  nix develop ../../.. --command cargo build --release --locked --offline
# receipt (read-only on the clones)
nice -n 19 taskset -c 40-51,296-307 tools/research/census/routecensus_batch.sh TMP/census-rc4 \
  /common/dev/rustred/TMP/w0/routecensus/receipt gen7 gen7-sat gen7-sat-seed2 c5f gen6 gen3 gen7-w300
nice -n 19 taskset -c 40-51,296-307 tools/research/census/routecensus_batch.sh TMP/census-rc6 \
  /common/dev/rustred/TMP/w0/routecensus/receipt/apply c5f-apply gen7-apply
# tables and standard errors
nix develop . --command python tools/research/census/route_tables.py /common/dev/rustred/TMP/w0/routecensus/receipt
nix develop . --command python tools/research/census/route_boot.py \
  /common/dev/rustred/TMP/w0/routecensus/receipt/gen7-route-rows.jsonl,/common/dev/rustred/TMP/w0/routecensus/receipt/apply/gen7-route-apply-rows.jsonl 200
# fix round: later hits (PPS by later hits, 4,000 per phase) and calibrated pending creation weights (§3b, §6b)
nice -n 19 taskset -c 40-51,296-307 tools/research/census/routecensus_batch.sh TMP/census-rc8 \
  /common/dev/rustred/TMP/w0/routecensus/receipt/hits c5f-hits gen7-hits
nix develop . --command python tools/research/census/route_hits.py /common/dev/rustred/TMP/w0/routecensus/receipt 200 \
  > /common/dev/rustred/TMP/w0/routecensus/receipt/hits/route_hits.md
```

- Tables: `receipt/tables.md`; standard errors, sampled-verdict shares and the per-owner view: `receipt/gen7-boot.md`.
- The candidate and domain-size quantiles of §6 come from `route_rows.py` over `gen7-route-rows.jsonl` and
  `apply/gen7-route-apply-rows.jsonl`. They are receipted in `receipt/gen7-rows-quantiles.txt`.
- Earlier receipts: `receipt-v1` (rc1, ambiguous admission stratum labels; aggregates identical) and `receipt-rc2`
  (rc2 + sat-rc3). They are superseded; cite `receipt/`.
