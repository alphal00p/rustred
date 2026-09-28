# W0 G2' falsifier: residual inspection against committed Native anchors (legacy engine)

Date 2026-09-28. Lane `g2falsify` of the fable_5_1 next push. It implements directive 0.1.3 of the audit addendum
in `HANDOFF_opus_5_5.md`, on master plan sections 3.10/3.11 and gate W4.2. Branch `fable_5_1-v3-g2falsify`, from
`fable_5_1` 04a4aedd. The lever is throwaway and env-gated; it is never merged into the production engine.
- Receipt: `TMP/w0/g2falsify/RESULTS.md`, with the full tables `summary-*.md`, `gate-table.md` and
  `matched-*.md`.
- Run directories: `TMP/w0/g2falsify/runs/<label>/<family>`.

Labels: **[M]** is measured (a run directory or receipt is cited), **[E]** is an estimate or interpretation.
Nothing here is an ETA or a closure claim; `family_closure_claim` stays false. Both five-loop campaigns stayed
stopped, and nothing under `campaigns/` was read for writing or touched. The runs used CPUs 16-31 and 272-287
only. Every pilot finished in under 1 h; the longest traversal was 644 s.

## 0. Verdict in brief

1. **The brief's minimal G2' (one anchor, optionally two) is weak.** It gets 0.78-1.06x inspector
   record-seconds on the five-loop controls [M]. On every control it produces zero single-anchor full covers: the
   legacy admission already aliases any Q that one existing domain contains. The W0.7 census reports the same
   (`single_container_share` 0.0 on C-5F).
2. **The master plan's G2' works: the union of committed anchors per D level, with a one-piece D-only residual.**
   Measured as the mean of 2 repeats against the flag-off arm of the same binary and session [M]:

   | Control | Record-s | Apply native calls | Route natives | Scheduled domains | Peak pending |
   |---|---|---|---|---|---|
   | C-5F Ordered | 0.518x | 0.539x | 0.825x | 0.802x | 0.831x |
   | C-5F Ready | 0.537x | 0.532x | 0.818x | 0.794x | 0.819x |
   | C-HOT-sub r1a12 Ready W12 | 0.534x | 0.437x | 0.804x | 0.794x | 0.860x |

   Every union run drained with 0 frontiers. The audit passed on each (`--require-closure
   --g2-residual-anchors`), and so did the independent pointwise re-check `g2verify`.
3. **Gate W4.2 fails for every arm.** The gate was pre-registered as: drain with 0 frontiers at ≤ 0.5x inspector
   seconds with audit PASS, and scheduled domains and peak pending not worse.
   - The union arm meets every clause except 0.5x, which it misses by 0.018-0.037.
   - The cross ranges of its ratios (0.46-0.60) straddle 0.5.
   - Without the G2' plan time (a linear anchor scan here), its ratios are 0.494, 0.510 and 0.518.
4. **D2 recommendation [E]: G2' in its union form.** It is preferred over "none", and over the single-anchor form
   the brief specified. The general union (pending domains as anchors) was not tested; see section 5.

## 1. What was built

`RUSTRED_WALK_G2_DONLY` selects the arm. With it unset, the engine creates no store, no dispatch stamps and no
record or report fields, and it is byte-identical to the frozen binary 4a17f9c7 (section 2).

Scope:
- pool runs (W > 1) without Apply subdivision;
- initial-prefix domains are never planned;
- checkpoints written with the flag on are not resumable.

The three modes:
- **Mode 1** (the brief's minimal G2'). It takes the oldest anchor that contains Q. Failing that, it takes the
  anchor that contains Q restricted to D ≥ c for the smallest cut c. The job then inspects only the residual,
  Q restricted to D ≤ c-1.
- **Mode 2.** It does mode 1, then runs the same search on the residual with the first anchor excluded. A second
  anchor covers c2 ≤ D ≤ c1-1, and the job inspects D ≤ c2-1.
- **Mode u** (the plan's G2' restricted to a one-piece D-only residual). It applies to a finite Q with at most 2^18
  lattice points.
  - The worker enumerates Q's points from its tight extrema.
  - It walks the D levels from the top down and assigns every point to an anchor that contains it.
  - It cuts at the lowest D of the fully covered top run.
  - An empty residual means a union cover with zero native work.
  - Infinite Q, larger Q, or a search over its test budget falls back to the mode-2 plan. The fallback never
    triggered on the five-loop controls.

Anchors and stamps:
- The coordinator stamps each eligible Apply dispatch with the count of Apply records committed without error so
  far (`dispatch_snapshot`). Every committed Apply record carries its own stamp (`g2_commit_seq`).
- A job's anchors are the same-(phase, owner) records whose stamp is below its dispatch stamp. They are committed
  Native records, full or partial. They are never pending, aliased or in flight.
- Anchor links are therefore strictly ordered in commit time, so there is no mutual subtraction.

Authority:
- Each band slice in modes 1 and 2 is checked with the native `DomainPowerSummary::contains`.
- Each point assignment in mode u is checked with the native `contains` of the one-point domain.
- The compact scan record is only a necessary prefilter.

Bookkeeping:
- The partial-record semantics of `initial_overlap.rs` are reused: `partial_initial_overlap_inspection`, with
  residual power bounds D ≤ cut-1, and Q's identity and coordinates unchanged.
- The anchor edges (one, two, or the union list) are recorded in the ledger before Q publishes, and in the
  descendant closure.
- The ledger resolves anchor status as a DAG, so frontiers or errors of any anchor block Q.
- Each record carries a `g2_residual_anchor` block with the stamps, the cuts, the second anchor or the union
  list, and plan statistics.

Checks:
- **Audit.** The oracle-branch audit, extended with `--g2-residual-anchors`, re-derives the commit stamps from the
  record stream. For each anchor it checks:
  - same (phase, owner);
  - Apply Native or partial record;
  - stamp equal to the stream stamp;
  - anchor stamp < dispatch stamp ≤ own stamp.

  It also checks the exact slice containment of the bands, the residual bounds, and that `full_cover` holds
  exactly when the residual is empty.
- **`g2verify`** (`tools/research/w0_g2falsify`). It re-checks every G2' record point by point with an enumerator
  independent of the engine code. Its point count is checked against a DP count. It checks that every lattice
  point of Q with D ≥ cut lies in an anchor the record names for that point.

No algebra was written. The lever only counts and compares integer points of boxes, which is lattice geometry,
not CAS.

## 2. Validation [M]

- **Identity.** With the flag off, the binaries match 4a17f9c7 under the strict Ordered compare
  (`TMP/w0/oracle/runs/{c4l-ordered,c5f-ordered}`):
  - C-4L (FG/BMW/H/X × 2 repeats) passes 8/8 for each of B1 13a3271a, B2 8918122a and B3 b04d00b2;
  - C-5F passes 2/2 for B1 and for B3.
- **Audits.** Every run was audited with `--require-closure`, plus `--g2-residual-anchors` on the G2' arms. All
  94 passed:
  - B1: 26;
  - B2: 24;
  - B3: 44 (C-4L 24, C-5F 12, C-HOT-sub 8).
- **g2verify** passed on all 59 G2' arms (B1 13, B2 16, B3 30). It found 0 uncovered points and 0 count mismatches. Examples: 107,288
  union records on C-HOT-sub u-r1, and 91,628 on C-5F Ordered u-r1.
- **Closure.** Every run had 0 frontiers, and every root closed:
  - C-4L: 248/268/628/656;
  - C-5F: 1/1;
  - C-HOT-sub: 1/1.

  Unlike G1 widening in W0.9, G2' adds no points, so H has no guard frontier.

## 3. Results [M]

Binary B3 (sha256 b04d00b23182a017c48b47f42b968c869b4bb866aeab22260173e7c081515191) ran:
- C-5F: five-finite, W24 on 32 CPUs.
- C-HOT-sub r1a12: the hot-owner full orthant with R ≤ 1 and A ≤ 12, Ready W12 on 16 CPUs, two arms concurrently
  on disjoint physical cores.

Each cell is the ratio of means over 2 repeats against flag off, with the min-max of the cross ratios in brackets.

| Control | Arm | Record-s | Apply native calls | Scheduled domains | Peak pending | Inspected Apply points |
|---|---|---|---|---|---|---|
| C-5F Ordered | ≤ 2 anchors | 0.831 (0.761-0.909) | 0.824 | 0.898 | 0.932 | 0.705 |
| C-5F Ordered | union | 0.518 (0.464-0.579) | 0.539 | 0.802 | 0.831 | 0.513 |
| C-5F Ready | ≤ 2 anchors | 0.911 (0.794-1.046) | 0.820 | 0.894 | 0.918 | 0.699 |
| C-5F Ready | union | 0.537 (0.484-0.597) | 0.532 | 0.794 | 0.819 | 0.503 |
| C-HOT-sub | 1 anchor | 0.777 (0.684-0.881) | 0.901 | 0.924 | 0.968 | 0.865 |
| C-HOT-sub | ≤ 2 anchors | 0.830 (0.658-1.023) | 0.742 | 0.923 | 0.978 | 0.848 |
| C-HOT-sub | union | 0.534 (0.490-0.583) | 0.437 | 0.794 | 0.860 | 0.599 |

B1 was the one-anchor binary with a slow linear native-summary scan (plan 178-336 s per run). On C-5F it gave
record-s 1.064x (Ordered) and 0.977x (Ready), Apply calls 0.91x, scheduled domains 0.90x and inspected points
0.72x.

**Per owner class** (Apply record-seconds, union / off, per repeat):
- **Owner 000011001001011:**
  - C-5F: 0.40-0.44x, with 36-37% of its Apply records fully union-covered.
  - C-HOT-sub: 0.48-0.54x, with 49%.
- **Owner 011101110111000:** it has only 510-1,268 Apply records (8-12 s) on these controls, so its effect is not
  measurable here.
- **All other owners:**
  - C-5F: 0.55-0.60x;
  - C-HOT-sub: 0.49-0.60x.

**Volume.**
- **Pending per completion at matched discovered domains is lower under union.** At 500k scheduled domains on
  C-5F it is 0.452-0.473 against 0.591-0.596 for flag off; at 750k it is 0.196-0.204 against 0.336-0.361. On
  C-HOT-sub at 900k scheduled it is 0.159-0.168 against 0.248-0.288 (`matched-*.md`).
- **Discovered domains per native completion are unchanged** (1.28-1.36).
- **Distinct inspected points are unchanged**, to within 798 of 2.27M. The missing points belong to successor
  boxes that the flag-off arm derives from whole domains (hull over-cover) [E]. Every G2' run is
  closure-certified.

**Union plans.**
- Union plans: 91,628 on C-5F Ordered and 107,288 on C-HOT-sub.
- Full covers: 72.6% and 80.0% of them.
- Anchors per record: mean 2.75 and 3.11, max 27 and 31.
- 81-87% of union plans need at least two anchors, which is why the single-anchor forms capture little.
- The planning cost was 21-41 s per run (4-5% of record-s), for 2.8-3.1e9 anchor visits in the linear bucket scan.

**Oracle consistency.**
- **Inspection counts match.** The offline union-only oracle of W0.9 predicted Apply inspections of 0.521x
  (C-5F) and 0.423x (C-HOT-sub); the measured Apply native calls are 0.532-0.539x and 0.437x. The oracle used any
  earlier ID as an anchor; this engine uses only anchors committed before dispatch.
- **Seconds are worse than the oracle's point projections.** Its power-law seconds projections of 0.318 and 0.153
  were optimistic by 1.6-3.5x. The measured seconds sit at the upper end of its [E] range: inside it on C-HOT-sub,
  and at or slightly above its 0.518 upper bound on C-5F.

## 4. Gate W4.2 (pre-registered) [M]

| Arm | C-5F drains, 0 frontiers, audit PASS | ≤ 0.5x record-s | Scheduled not worse | Peak pending not worse | Verdict |
|---|---|---|---|---|---|
| 1 anchor | yes | no (0.98-1.06; C-HOT-sub 0.78) | yes | yes | FAIL |
| ≤ 2 anchors | yes | no (0.83-0.91; C-HOT-sub 0.83) | yes | yes | FAIL |
| union | yes | no (0.518 / 0.537; C-HOT-sub 0.534) | yes (0.79-0.80) | yes (0.82-0.86) | FAIL (narrow) |

## 5. D2: the three-way choice for the owner [E, on the M figures above]

- **None.** This forgoes about 46-48% of inspector seconds and about 20% of scheduled domains on both five-loop
  controls, and the lever has no soundness finding against it. The data does not support this option.
- **G2' in its union form (recommended).**
  - Anchors are Native records merged before dispatch (invariant S7).
  - The residual is one D-only piece, with an edge per anchor.
  - It reuses the partial-record semantics that the audit already knows.
  - It measured 0.52-0.54x record-s, 0.44-0.54x Apply native calls, 0.79-0.80x scheduled domains and 0.82-0.86x
    peak pending, with lower pending at matched volume.
  - It misses the 0.5x clause by 0.02-0.04, which is within the session noise (flag-off repeats differ by up to
    16%). The owner decides whether that clause stands as written or is restated in the useful-work units of
    directive 0.1.2.
  - The single- and two-anchor forms are not worth building.
  - At scale, the W0.7 census puts 55.4% of gen-7 pending Apply fully covered by gen-7 natives (PPS). The five-loop
    controls here had 30-37% full covers per class, so the effect may be larger at gen 7 [E, untested].
- **General union** (pending domains as anchors). The gen-7 census gives 79.5% of pending Apply fully covered,
  against 55.4% for G2', i.e. up to about 1.4x G2''s coverage. It was not tested. Anchors that are not yet Native
  violate S7 as written, so this option needs deferred resolution and a new invariant.
- **Carried into W4.2 [E].** For any W4.2 build of the union form:
  - an indexed anchor search (the linear scan made 2.8-3.1e9 visits per control, which will not scale to gen-7
    buckets);
  - the one-point native authority, or an equivalent exact union predicate in `lattice.rs`, which directive 0.1.6
    asks for before W4.2 code;
  - multi-anchor CP6 bookkeeping (plan 3.11);
  - G2' mutations in the soundness battery.

## 6. Caveats and open issues

- **Noise.**
  - Other users' processes (postgres/gammaboard) ran on this lane's CPUs, averaging 0.3-16 foreign busy CPUs per
    run; the recorder is in every `metrics.json`.
  - n = 2 per arm. perf stat was not used, so instructions per native were not recorded.
  - The timing ratios on C-4L are noise-dominated (0.87-1.44x at identical counts); C-4L serves identity and
    soundness only.
- **Owner 011101110111000 is untested.** It carries 64% of gen-7 Apply CPU, but these controls do not exercise it.
  An aged (gen-7 resume) pilot would need the anchor store seeded from restored records, which is not implemented.
- **Knobs box s2/r2a12 not run.** It drains only at W48 on socket 1, which is out of reach for this lane.
- **No independent re-inspection.** `walk-verify-closure` does not know G2' anchors, so these runs are certified
  by the audit and `g2verify` only.
- **Only the D-only one-piece form was tested.** Middle-band covers (2-piece residuals) and A/R residual shapes
  (C2) were not.

## 7. Artifacts

- **Commits on `fable_5_1-v3-g2falsify`:**
  - 84885234: B1 engine;
  - 17c12a72: audit and tools;
  - 28526666: B2 engine;
  - b567c2c2: tools;
  - 8fddff9f: B3 engine (mode u) and g2verify;
  - 9abb2554: g2verify reporting;
  - de6f362d: gate.py and matched.py;
  - this note.
- **Binaries** (`TMP/w0/g2falsify/bin/`, sha256 in `SHA256SUMS`):
  - rustred-13a3271a (B1);
  - rustred-8918122a (B2);
  - rustred-b04d00b2 (B3).
- **Tools** (`tools/research/w0_g2falsify/`):
  - `session.sh` (phases c4l/c5f/hotsub for B1, c4l3 for B2, c4lx/hotsubx/c5fx for B3);
  - `run_arm.py`;
  - `g2stats`;
  - `g2verify`;
  - `pending.py`;
  - `summarize.py`;
  - `gate.py`;
  - `matched.py`.
