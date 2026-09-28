# W0 G2' falsifier: residual inspection against committed anchors (legacy engine)

Date 2026-09-28 (revision 2, after a verification fix round). Lane `g2falsify` of the fable_5_1 next push. It
implements directive 0.1.3 of the audit addendum in `HANDOFF_opus_5_5.md`, on master plan sections 3.10/3.11 and
gate W4.2. Branch `fable_5_1-v3-g2falsify`, from `fable_5_1` 04a4aedd. The lever is throwaway and env-gated; it
is never merged into the production engine.
- Receipt: `TMP/w0/g2falsify/RESULTS.md`, with the full tables `summary-*.md`, `gate-table.md` (first revision),
  `gate-table-2.md` / `.json` (fix round) and `matched-*.md`.
- Run directories: `TMP/w0/g2falsify/runs/<label>/<family>`.

Labels: **[M]** is measured (a run directory or receipt is cited), **[E]** is an estimate or interpretation.
Nothing here is an ETA or a closure claim; `family_closure_claim` stays false. Both five-loop campaigns stayed
stopped, and nothing under `campaigns/` was written or touched. The runs used CPUs 16-31 and 272-287 only. Every
pilot finished in under 1 h; the longest traversal was 644 s.

## 0. Verdict in brief

**What the fix round changed.** A verification pass found three problems in the first revision (commit
d6f83175), and all three were confirmed against the files:
- About 31% of the union arm's anchor references pointed to earlier G2' residual records, not to full native
  inspections. The measured union arm was therefore not S7 / plan 3.11 ("each anchor is Native") read literally.
- The offline oracle matched the engine's inspection ratio only because two differences cancel.
- The gate was framed as a near pass on a wall-time metric measured under heavy foreign load.

The fix round added binary B4 (461dbd84) with a fourth mode, **n**: the union form with anchors restricted to full
native inspections. B4 re-ran off / u / n in one session, with n = 3 per arm (C-HOT-sub flag off n = 4), every run
under `perf stat`. The gate tables now carry on-CPU time, useful-work units and instructions next to the
registered wall-time metric.

1. **The brief's minimal G2' (one anchor, optionally two) is weak.** It gets 0.78-1.06x inspector
   record-seconds on the five-loop controls [M]. On every control it produces zero single-anchor full covers: the
   legacy admission already aliases any Q that one existing domain contains. The W0.7 census reports the same
   (`single_container_share` 0.0 on C-5F).
2. **The union form carries the lever, and it needs earlier G2' records as anchors.** B4 session, ratio of means
   against the flag-off arm of the same binary and session [M]:

   | Control | Arm | Record-s (registered) | run_s (on-CPU) | Useful-work units | Instructions:u | Apply native calls | Scheduled domains | Peak pending |
   |---|---|---|---|---|---|---|---|---|
   | C-5F Ordered W24 | u | 0.496 | 0.552 | 0.525 | 0.577 | 0.539 | 0.802 | 0.836 |
   | C-5F Ordered W24 | n | 0.676 | 0.699 | 0.656 | 0.741 | 0.666 | 0.877 | 0.911 |
   | C-5F Ready W24 | u | 0.503 | 0.530 | 0.517 | 0.565 | 0.532 | 0.794 | 0.816 |
   | C-5F Ready W24 | n | 0.694 | 0.698 | 0.658 | 0.731 | 0.665 | 0.872 | 0.920 |
   | C-HOT-sub r1a12 Ready W12 | u | 0.523 | 0.532 | 0.418 | 0.561 | 0.436 | 0.795 | 0.861 |
   | C-HOT-sub r1a12 Ready W12 | n | 0.770 | 0.761 | 0.544 | 0.784 | 0.556 | 0.865 | 0.935 |

   - **u:** an anchor is any Apply record committed without error before Q's dispatch. That includes G2' residual
     records: they draw 30.8-31.9% of the anchor references, and 47.6-49.1% of union records use at least one.
   - **n:** anchors are full native inspections only, which is S7 read literally. It keeps 48-64% of u's
     record-s reduction and 62-66% of its scheduled-domain reduction.
   - Every run drained with 0 frontiers, passed the audit, and passed the independent pointwise re-check
     `g2verify`. The audit also checks that every mode-n anchor is a full native inspection; a relabelled mode-u
     run fails this check.
3. **Gate W4.2 fails for every arm, as registered.**
   - **Wall time (the registered metric).** The union arm's record-seconds span 0.496-0.537 over six
     control-session cells (B3 n = 2: 0.518 / 0.537 / 0.534; B4 n = 3: 0.496 / 0.503 / 0.523). Five of the six
     are above 0.5. Under the foreign load of these sessions the wall-time clause is at the threshold and cannot
     be decided at this n.
   - **Load-insensitive measures.** On C-5F they are all above 0.5: on-CPU run_s 0.53-0.57, useful-work units
     0.52-0.53 (directive 0.1.2) and instructions 0.56-0.58. On C-HOT-sub only useful-work units pass (0.42).
   - No relaxation of the 0.5 clause is proposed.
   - Mode n misses by far (0.68-0.77x).
4. **D2 recommendation [E]: G2' in its union form, with S7 and plan 3.11 amended.** The amendment lets a merged G2'
   residual record serve as an anchor. Its coverage is its residual plus its own anchors, resolved transitively in
   merge order. If S7 stays as written, G2' is the weaker mode n. The general union (pending domains as anchors)
   was not tested; see section 5.

## 1. What was built

`RUSTRED_WALK_G2_DONLY` selects the arm. With it unset, the engine creates no store, no dispatch stamps and no
record or report fields, and it is byte-identical to the frozen binary 4a17f9c7 (section 2).

Scope:
- pool runs (W > 1) without Apply subdivision;
- initial-prefix domains are never planned;
- checkpoints written with the flag on are not resumable.

The modes:
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
- **Mode n** (fix round). Mode u, but only full native inspections are appended to the anchor store. G2' residual
  records, G2' full covers and initial-overlap partials still receive a commit stamp.

Anchors and stamps:
- The coordinator stamps each eligible Apply dispatch with the count of Apply records committed without error so
  far (`dispatch_snapshot`). Every committed Apply record carries its own stamp (`g2_commit_seq`).
- A job's anchors are same-(phase, owner) records whose stamp is below its dispatch stamp.
  - In modes 1, 2 and u these are Apply records committed without error: full native inspections, G2' residual
    records, and initial-overlap partials.
  - In mode n they are full native inspections only.
  - Anchors are never pending, aliased or in flight. Anchor links are strictly ordered in commit time, so there
    is no mutual subtraction.

Authority:
- Each band slice in modes 1 and 2 is checked with the native `DomainPowerSummary::contains`.
- Each point assignment in modes u and n is checked with the native `contains` of the one-point domain.
- The compact scan record is only a necessary prefilter.

Bookkeeping:
- The partial-record semantics of `initial_overlap.rs` are reused: `partial_initial_overlap_inspection`, with
  residual power bounds D ≤ cut-1, and Q's identity and coordinates unchanged.
- The anchor edges (one, two, or the union list) are recorded in the ledger before Q publishes, and in the
  descendant closure.
- The ledger resolves anchor status as a DAG, so frontiers or errors of any anchor, or of an anchor's anchors,
  block Q.
- Each record carries a `g2_residual_anchor` block with the mode, the stamps, the cuts, the second anchor or the
  union list, and plan statistics.

Checks:
- **Audit.** The oracle-branch audit, extended with `--g2-residual-anchors`, re-derives the commit stamps from the
  record stream. For each anchor it checks:
  - same (phase, owner);
  - Apply Native or partial record, and a full native inspection for mode-n records;
  - stamp equal to the stream stamp;
  - anchor stamp < dispatch stamp ≤ own stamp.

  It also checks the exact slice containment of the bands, the residual bounds, and that `full_cover` holds
  exactly when the residual is empty.
- **`g2verify`** (`tools/research/w0_g2falsify`). It re-checks every G2' record point by point, with an
  enumerator that is independent of the engine code:
  - its point count is checked against a DP count;
  - every lattice point of Q with D ≥ cut must lie in an anchor that the record names for that point.
- **Anchor-kind census.** `g2stats` (`g2_anchor_reference_kinds`) classifies every anchor reference by the kind of
  record it points to.

No algebra was written. The lever only counts and compares integer points of boxes, which is lattice geometry,
not CAS.

## 2. Validation [M]

- **Identity.** With the flag off, the binaries match 4a17f9c7 under the strict Ordered compare
  (`TMP/w0/oracle/runs/{c4l-ordered,c5f-ordered}`):
  - C-4L (FG/BMW/H/X × 2 repeats) passes 8/8 for each of B1 13a3271a, B2 8918122a, B3 b04d00b2 and B4 461dbd84;
  - C-5F passes 2/2 for B1 and B3, and 3/3 for B4.
- **Audits.** Every run was audited with `--require-closure`, plus `--g2-residual-anchors` on the G2' arms. All
  138 passed: B1 26, B2 24, B3 44, B4 44.
- **Negative control.** A B3 C-4L BMW union run whose records were relabelled as mode n fails the new mode-n
  anchor check, as intended.
- **g2verify** passed on all 85 G2' arms (B1 13, B2 16, B3 30, B4 26). It found 0 uncovered points and 0 count
  mismatches. Examples: 107,288 union records on C-HOT-sub u-r1 (B3), and 91,628 on C-5F Ordered u-r1 (B3).
- **Closure.** Every run had 0 frontiers, and every root closed:
  - C-4L: 248/268/628/656;
  - C-5F: 1/1;
  - C-HOT-sub: 1/1.

  Unlike G1 widening in W0.9, G2' adds no points, so H has no guard frontier.

## 3. Results [M]

**Controls.**
- C-5F: five-finite, W24 on 32 CPUs.
- C-HOT-sub r1a12: the hot-owner full orthant with R ≤ 1 and A ≤ 12, Ready W12 on 16 CPUs. Two arms ran
  concurrently on disjoint CCD halves.

**Sessions.**
- B3 (b04d00b2): arms off / 1 / ≤ 2 / u, n = 2.
- B4 (461dbd84): arms off / u / n, n = 3, with `perf stat`.

**First revision (B3), ratio of means over 2 repeats (cross min-max):**

| Control | Arm | Record-s | Apply native calls | Scheduled domains | Peak pending | Inspected Apply points |
|---|---|---|---|---|---|---|
| C-5F Ordered | ≤ 2 anchors | 0.831 (0.761-0.909) | 0.824 | 0.898 | 0.932 | 0.705 |
| C-5F Ordered | union u | 0.518 (0.464-0.579) | 0.539 | 0.802 | 0.831 | 0.513 |
| C-5F Ready | ≤ 2 anchors | 0.911 (0.794-1.046) | 0.820 | 0.894 | 0.918 | 0.699 |
| C-5F Ready | union u | 0.537 (0.484-0.597) | 0.532 | 0.794 | 0.819 | 0.503 |
| C-HOT-sub | 1 anchor | 0.777 (0.684-0.881) | 0.901 | 0.924 | 0.968 | 0.865 |
| C-HOT-sub | ≤ 2 anchors | 0.830 (0.658-1.023) | 0.742 | 0.923 | 0.978 | 0.848 |
| C-HOT-sub | union u | 0.534 (0.490-0.583) | 0.437 | 0.794 | 0.860 | 0.599 |

B1 was the one-anchor binary with a slow linear native-summary scan (plan 178-336 s per run). On C-5F it gave
record-s 1.064x (Ordered) and 0.977x (Ready), Apply calls 0.91x, scheduled domains 0.90x and inspected points
0.72x.

**Fix round (B4):** see the table in section 0 and section 2 of the receipt.
- Instructions per native fall to 0.77-0.80x under u, because residual inspections are smaller. They are
  0.89-1.01x under n.
- IPC is 2.1-2.4 flag off and 2.2-2.6 under G2'.

**Per owner class** (Apply record-seconds, arm / off, per repeat, B4):
- **Owner 000011001001011:**
  - u: 0.37-0.41x on C-5F, 0.48-0.53x on C-HOT-sub;
  - n: 0.57-0.61x and 0.73-0.79x.
- **All other owners:**
  - u: 0.51-0.55x and 0.50-0.54x;
  - n: 0.70-0.74x and 0.69-0.77x.
- **Owner 011101110111000:** it has only 510-1,268 Apply records (6-12 s) on these controls, so its effect is not
  measurable here.

**Volume.**
- **Pending per completion at matched discovered domains is lower under both union arms, and lowest under u.**

  | Control | At scheduled domains | off | u | n |
  |---|---|---|---|---|
  | C-5F | 750k | 0.337-0.361 | 0.196-0.205 | 0.257-0.268 |
  | C-HOT-sub | 900k | 0.247-0.263 | 0.155-0.163 | 0.196-0.200 |

  Source: `matched-461dbd84.md`.
- **Discovered domains per native completion are unchanged** (1.28-1.36).
- **New distinct Apply points per native call rise.** For the hot class they are 4.8 flag off, 10.6-10.9 under u
  and 8.0-8.3 under n on C-5F.
- **Distinct inspected points are unchanged**, to within 1,020 of 2.27M. The missing points belong to successor
  boxes that the flag-off arm derives from whole domains (hull over-cover) [E]. Every G2' run is
  closure-certified.

**Union plans.**
- Union plans under u: 91,635 on C-5F Ordered, of which 72.6% are full covers; 107,382 on C-HOT-sub, 80.1% full
  covers.
- Under n there are more plans but fewer full covers: 101,885 plans with 51.9% full covers, and 120,277 with
  63.6%.
- Anchors per record under u: mean 2.75 and 3.11, max 27 and 31. 81-87% of union plans need at least two
  anchors.
- The planning cost was 11-41 s per run, for 1.7-3.1e9 anchor visits in the linear bucket scan.

**Oracle consistency (restated in the fix round).** The W0.9 union-only oracle predicted Apply inspections of
0.521x (C-5F) and 0.423x (C-HOT-sub). The measured values are 0.532-0.539x and 0.435-0.437x, but the composition
differs:

| | C-5F | C-HOT-sub |
|---|---|---|
| Oracle: Apply domains | 271,341, frozen at the flag-off population | 292,203, frozen |
| Oracle: fully covered | 129,840 (47.9%) | 168,560 (57.7%) |
| Engine (B3 u r1): Apply domains | 212,957 (22% fewer) | 215,147 (28% fewer) |
| Engine (B3 u r1): fully covered | 66,500 | 85,874 |

- The engine fully covers about half as many domains as the oracle. The effects cancel in the ratio, so the oracle
  is **not** validated as a predictor of this lever.
- The C-HOT-sub oracle comes from an Ordered W12 reference, while the measured arm is Ready W12.
- The census gives 44.2% of C-5F Apply fully covered by merged natives; the engine achieves 30-37% per class.
- The oracle's power-law seconds projections (0.318 and 0.153) were 1.6-3.5x optimistic.

## 4. Gate W4.2 (pre-registered) [M]

Registered as: C-5F and C-HOT-sub drain with 0 frontiers at ≤ 0.5x inspector record-seconds with audit PASS, and
scheduled domains and peak pending not worse.

| Arm | Drains, 0 frontiers, audit PASS | ≤ 0.5x record-s | Scheduled not worse | Peak pending not worse | Verdict |
|---|---|---|---|---|---|
| 1 anchor | yes | no (0.98-1.06; C-HOT-sub 0.78) | yes | yes | FAIL |
| ≤ 2 anchors | yes | no (0.83-0.91; C-HOT-sub 0.83) | yes | yes | FAIL |
| union u (B3) | yes | no (0.518 / 0.537; C-HOT-sub 0.534) | yes (0.79-0.80) | yes (0.82-0.86) | FAIL |
| union u (B4) | yes | C-5F Ordered 0.496 yes; Ready 0.503 no; C-HOT-sub 0.523 no | yes (0.79-0.80) | yes (0.82-0.86) | FAIL |
| union n (B4) | yes | no (0.676 / 0.694; C-HOT-sub 0.770) | yes (0.86-0.88) | yes (0.91-0.94) | FAIL |

**Load-insensitive restatement (informational, not a gate).** It follows directive 0.1.2.
- Useful-work units are native calls times the per-owner flag-off cost of the same session. G2' residual
  inspections are charged in full, and plan time is excluded.
- Under u they are 0.525 / 0.517 on C-5F and 0.418 on C-HOT-sub; under n, 0.656 / 0.658 / 0.544.
- On-CPU run_s under u is 0.53-0.57, and whole-process instructions are 0.56-0.58.
- In every unit the union arm misses 0.5 on C-5F. The clause stands as registered.

## 5. D2: the three-way choice for the owner [E, on the M figures above]

- **None.** This forgoes about half of the inspector record-seconds and about 20% of scheduled domains on both
  five-loop controls. The lever has no soundness finding against it. The data does not support this option.
- **G2' in its union form, recommended, with S7 and plan 3.11 amended.**
  - Anchors: an anchor may be any record merged before the job's dispatch snapshot whose domain is fully
    discharged:
    - a Native record; or
    - a merged G2' residual record, whose coverage is its residual plus its own anchors, resolved transitively in
      merge order.

    The chain is acyclic by construction, and frontiers and errors propagate along anchor edges.
  - Residual: one D-only piece, with an edge per anchor.
  - CP6 restore check: each anchor's merge epoch is below the node's dispatch epoch, and each anchor is Native or
    a validated G2' record.
  - Measured (u), ratios to flag off:
    - record-s 0.50-0.54x;
    - useful-work 0.42-0.53x;
    - scheduled domains 0.79-0.80x;
    - peak pending 0.82-0.86x;
    - lower pending at matched volume.
  - It fails the registered 0.5x clause.
- **G2' with S7 as written (mode n).** It measured record-s 0.68-0.77x, useful-work 0.54-0.66x and scheduled
  domains 0.86-0.88x. It is the fallback if the owner rejects the amendment.
- **The single- and two-anchor forms** (0.78-1.06x) are not worth building.
- **At scale.** The W0.7 census puts 55.4% of gen-7 pending Apply fully covered by gen-7 natives (PPS). The
  five-loop controls here had 30-37% full covers per class, so the effect may be larger at gen 7 [E, untested].
- **General union** (pending domains as anchors). The gen-7 census gives 79.5% of pending Apply fully covered,
  against 55.4% for G2', i.e. up to about 1.4x G2''s coverage. It was not tested: anchors that are not yet merged
  need deferred resolution and a new invariant.
- **Carried into W4.2 [E].** For any W4.2 build of the union form:
  - an indexed anchor search (the linear scan made 1.7-3.1e9 visits per control, which will not scale to gen-7
    buckets);
  - the exact "Q ⊆ residual ∪ anchors" predicate of directive 0.1.6, with `g2verify` as its pointwise reference;
  - multi-anchor CP6 bookkeeping, including G2'-record anchors (plan 3.11);
  - G2' mutations in the soundness battery (residual shrunk by one point, anchor stamp ≥ dispatch, anchor neither
    Native nor a validated G2' record).

## 6. Caveats and open issues

- **Noise.**
  - Other users' processes ran on this lane's CPUs: postgres, gammaboard, and a nextest job. In B3 they averaged
    0.3-16 foreign busy CPUs per run; the C-HOT-sub halves were 37-57% busy. In B4 they averaged 4.8-13.1 of 32
    CPUs on C-5F and 5.0-6.6 of 16 on C-HOT-sub. The recorder is in every `metrics.json`.
  - The coefficient of variation of flag-off record-s was 0.07-0.10 in B3 (n = 2) and 0.01-0.09 in B4 (n = 3-4).
  - The timing ratios on C-4L are noise-dominated; C-4L serves identity and soundness only.
- **Coarse instructions.** Instructions are for the whole process. Instructions in native frames (per-frame
  attribution) were not measured. The useful-work weights are the flag-off per-owner seconds per native of the
  same session, not the harness K=1 costs.
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
  - d6f83175: first revision of this note;
  - fd1e1955: B4 engine (mode n);
  - d1f9b830: tools for the mode-n audit check, the anchor-kind census, perf stat and gate2.py;
  - this revision.
- **Binaries** (`TMP/w0/g2falsify/bin/`, sha256 in `SHA256SUMS`):
  - rustred-13a3271a (B1);
  - rustred-8918122a (B2);
  - rustred-b04d00b2 (B3);
  - rustred-461dbd84 (B4; sha256 461dbd84eb8b097cf75f9e2667a334adb68a0c412e1cf40c7777deb010e46730).
- **Tools** (`tools/research/w0_g2falsify/`):
  - `session.sh`, with phases per binary, and for B4 the phases c4ln, hotsubn and c5fn;
  - `run_arm.py`, with `--perf`;
  - `g2stats`;
  - `g2verify`;
  - `pending.py`;
  - `summarize.py`;
  - `gate.py`;
  - `gate2.py`;
  - `matched.py`.
