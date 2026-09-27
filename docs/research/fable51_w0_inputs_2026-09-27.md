# W0.6 input intel and the W1.3 input gates (lane `inputs`, branch `fable_5_1-v3-inputs`), 2026-09-27

Status: **final** for W0.6; the W1.3 witness gate and the v4-candidate diagnostic were also run.
Revised after the Fable 5.1 audit directives (`HANDOFF_opus_5_5.md` §0.1 item 5,
`FABLE_5_1_CRITIQUE.md` §2.7): I1b is recorded as dropped, domain-volume metrics are reported
at matched natives, I1 and I2 are sized by owner class and phase, and an in-flight guard for I1
is proposed (§8.2).
Labels: [M] measured (run directory + binary sha256), [M-r] measured by a lens script, [E]
estimate or interpretation. No ETA, no closure claim (`family_closure_claim` stays false);
termination of the five-loop walk is not established.

Binary for every native run below unless stated: `TMP/fable51-controls/bin/rustred-4a17f9c7`
(sha256 `4a17f9c7c0447713370a1aed2e54c9a04251106a9183fd1b8a86dc5d1abb395e`, wave-2, walk
semantics 1, CP5). Five-loop inputs: byte-identical copies under
`TMP/retired-campaigns-20260925.UtI4ay/five-loop-saved/inputs` (selection sha256 `d2667dc9…`,
all 67 owner bundles re-hashed against the v2 input receipt: 0 mismatches). Campaign directories
were only read. Paths are relative to `/common/dev/rustred/TMP/w0/inputs/` unless absolute
(this note is committed as `docs/research/fable51_w0_inputs_2026-09-27.md`; the run directories
stay in TMP).

## 0. Summary

- **I1 (hybrid helpers for the 40 L* owners): gate 0.6 PASS** [M]; **a domain-count lever
  bounded to about one sixth of the Apply CPU** [M]. L* = L_static ∩ L_obs ∩ L_clean = 40 owners
  (§1). The 54-min five-loop probe-I1 has 0 frontiers, 0 unbounded-A domains in the 7 guard
  owners, all 121,235 unbounded-A domains in L* owners, and nothing reached from L* outside L*
  (§4.2). Domain volume against the same-period plan-v3 control at matched natives (1-3M): 0.95-0.98x
  discovered domains, 0.86-0.96x pending, lower peak pending (4.13M vs 5.23M at the stop),
  discovered per native 3.53 vs 3.71 at 3M, roots closed earlier; but 1.3-1.7x dependency edges
  (§4.3). Sizing (re-derived from the record sidecars): in v2 gen 7 the L* owners carry **16.5 % of
  Apply native seconds**, 39.3 % of Apply domains and 37.3 % of Apply natives; the four hottest
  owners (78.1 % of Apply seconds) are all outside L* (§4.4). In the probes I1 removes 65 % of the
  L* Apply domains but only 12 % of the L* Apply seconds, and total Apply seconds do not fall
  (indicative timing). Four-loop BMW I1 drains with 15.7x fewer natives than the historical
  input, audit PASS (§5).
- **I1b is dropped** (directive of the audit, not an owner decision), on this lane's evidence:
  four-loop BMW I1b does not drain in either repeat (≥ 2.68M / ≥ 1.42M natives at the caps vs
  147,233 for the baseline); five-loop I1b on the same CPUs as the full plan-v3 control has
  1.17-1.18x discovered, 1.40-1.44x pending and 1.31-1.51x edges at 1.5-2.5M natives and half the
  roots closed (3 vs 6); its reinterpreted gate ("no unbounded-A domain in a guard mask") cannot
  fail, because unbounded-A domains only descend from A-null helpers, which are the same in
  plan-v3 and I1b (§4.3, §5, §8.1).
- **I2 (route-witness rewrite): the W1.3 witness gate passes as worded, but I2 increases total
  work on C-5F** [M]. Route→Route edges per Route native −61 % (2 Ready repeats and 2 Ordered
  repeats, all audits PASS); Route→Apply per Route native unchanged (0.938 → 0.951); Route is
  0.8 % of native seconds in gen 7 and 1.6 % in C-5F, so the lever is on the memory/index side, not
  CPU. On the drained C-5F closure the totals go the wrong way: 1.52x natives, 2.13x Apply natives,
  1.45x domains, 1.30x edges, peak pending 1.09x, 1.25x user instructions (Ordered, identical
  counts in two repeats; perf counters on one) (§6.1). Recommendation [E]: amend the gate to
  include totals, rebuild I2 with a consistent frame per owner before shipping (§8.3).
- **In-flight guard for I1 (proposal, §8.2):** stop the walk on any dependency edge from a domain
  in the L* region (L* owner Apply masks and the route masks statically reachable from them) to a
  mask outside it; checked where every successor edge is recorded (`fn dependency` in
  `crates/rustred-app/src/application/routed_campaign/walking/execution.rs`), stopping through
  a cooperative stop (the action I4's frontier policy `stop` takes); in the epoch engine at the
  epoch merge.
- The v4 candidate (I1 queries + I2 selection) passes the matching-only diagnostic (183/183 clean,
  §7). The same-CPU plan-v3 control reproduces the killed first attempt within 5 % (§4.1).
- Open for the owner / orchestrator: the I2 gate amendment (D-I2, §8.3). Nothing about I1b.

## 1. Owner sets for I1 (L_static, L_obs, L_clean, L*)

Guard-sensitive owners G (7): the owners with unresolved guard pieces at an unbounded-A helper
in the 2026-09-26 matching diagnostic `TMP/qcd-feynman-d9d10-input.dcgP73/match-v1`:
011011000111111, 011101110111000, 111000100111001, 111001100111001, 111001100111111,
111010100100101, 111101111111100.

### 1.1 L_static

- **Structural scan** [M: `scan5/` (run.sh, result.json sha256 84842e9f…, stderr GNU time)]:
  `owner-domain-scan --unbounded-rank` (rank None, A unbounded, every installed rule, priority
  not resolved) on the 67 owners: complete, 4,758,436 sign regions in 87,179 groups, 103.6 s,
  6.0 GB. Same totals as the audited 2026-09-23 census.
- **Geometric closure is vacuous** [M: `static-reach.json`; tool `tools/research/inputs/static_reach.py`]:
  owner → scan target sectors; sector → owner / route root + every proper sub-mask of the
  root (as `route_cover` emits them); exact-zero targets are sinks. With every scan region
  kept, all 67 owners reach all 67 owners, so **L_static(geometric) = ∅**. The cause is the
  1,958,316 "unsupported support-change" regions (terms whose sign cell activates an inactive
  line); the scan does not test coefficients. With those regions removed the closure gives
  exactly the 40 observed owners (below).
- **Native refinement** [M: `static-probe/`]: the 40 candidate owners' full orthants
  (lower 0, upper ∞, A unbounded) at rank ≤ 24 and ≤ 32:
  - matching-only diagnostic: clean at both ranks (r24: 133,131 selected-rule / 413 terminal
    pieces, 0 unresolved / exact-gap / invalid, 125 s; r32: 158,157 / 413 / 0, 136 s);
  - one native Apply hop per orthant with the production walk (`one_hop.py`: Ordered W18
    resp. W6, stop once the 40 initial records are published; edges read from the CP5
    checkpoint by `cp5hop`): all 40 inspections finished with 0 frontiers and 0 errors
    (longest 85 s at r24, 115 s at r32); 506 distinct (owner, target phase/mask) pairs at
    both ranks, identical sets; **0 targets outside the source owner's mask**;
  - closure with these native one-hop sets replacing the scan targets of the 40 owners (other
    owners keep the geometric targets): **L_static(R ≤ 32) = the same 40 owners**
    [M: `static-reach-r24.json`, `static-reach-r32.json`].
  - Soundness argument: the first applicable rule of a point does not depend on the box and a
    coefficient that vanishes identically on a sign cell vanishes on its sub-cells, so the
    one-hop targets of the full orthant contain those of every domain of that owner with
    rank ≤ R. Successor ranks can exceed R (rank grows by ≤ 8 per step), so the statement
    covers domains of rank ≤ 32 only; observed maxima are 18 (v2 gen 7) and 18 (interim
    gen 8). Rank ∞ is **not** covered: at rank None the first owner (000011001001011) already
    has 31 unresolved guard pieces and the guard algebra limit refuses
    (`guard separable factor work` 339,738,624 > 64,000,000) [M: `static-probe/match-rnull`].
  - Validation: every observed owner-level transition of v2 gen 3, v2 gen 7 and interim
    gen 8 lies inside the refined static closure (0 edge and 0 closure violations).

### 1.2 L_obs

- v2 gen 3 and gen 7: 40 owners (`$S/inputs_lens/an5.py`, identical at both) [M-r].
- **Interim gen 8** (the only campaign where these owners had A-unbounded helpers, 22.3 h):
  read with `cp4scan` from the 276 GB CP4 state (read-only; 4 min 42 s, 51 GB RSS)
  [M: `interim-gen8/`, `interim-gen8.log`]: 53,228,053 domains, 4,205,183,439 edges (both
  equal to the campaign's result.json). Owner-level closure: **the same 40 owners** reach
  no guard owner.
- Support increases are never realised: 0 of 633.9M Apply-sourced edges in v2 gen 7 and 0 of
  1.03G in interim gen 8 go to a mask that is not a subset of the source owner's mask [M].

### 1.3 L_clean and the interim frontiers

- Interim frontiers [M: `interim-gen8/frontiers.tsv`]: 1,299 `local_dispatch_frontier`
  records on 397 Apply nodes, all with **unbounded A**, ranks 4-7, in 6 guard owners
  (111001100111001: 247 nodes, 011101110111000: 98, 011011000111111: 20,
  111000100111001: 17, 111010100100101: 14, 111001100111111: 1).
- Their ancestors (reverse reachability over all 4.2G dependency edges): 7,762,250 nodes, of
  which 3,048,864 Apply nodes in 27 owners = the 7 guard owners + 20 others; **no L owner is
  an ancestor of any interim frontier node** [M: `interim-gen8/ancestors.tsv`,
  `seed_ancestors.tsv`].
- Matching-only diagnostic of the I1 candidate (below): 183/183 queries complete, 0
  unresolved / exact-gap / invalid pieces → every L owner is clean [M].
- **L_clean ⊇ the 40 owners.**

### 1.4 L* = L_static ∩ L_obs ∩ L_clean = 40 owners [M]

`lstar-candidate.json` (sha256 f8418350…): all 13 owners with t ≤ 7 plus 27 owners with
t = 8-10 (indices 0 2 3 4 5 9 12 14 16 17 23-29 31-33 35 37 38 40 42-47 49-55 58-60). The
27 owners outside L* (all t ≥ 8) are the 7 guard owners and 20 owners that reach them.
Caveat: the static part holds for domains of rank ≤ 32; a rank escape above 32 in an L*
owner is outside every check here and must stay a monitored stop condition.

## 2. Candidate helper sets (planner option, checker, matching-only diagnostic)

Planner extension (commit fe0879b8): `plan_renormalization_entry_queries.py
--helper-bounds-from FILE` (`rustred.helper-bounds.json.v1`: owner → {max_numerator_rank,
max_positive_power | null}); overrides take precedence over the positive-power owner list,
must contain every root of the owner, are recorded in the receipt (and the file digest), and
are re-derived by `check_renormalization_entry_queries.py`. Tests: 4 new (planner override,
6 refusal cases, checker pass + tampered receipt, override not containing roots); planner and
checker suites 48 tests OK (1 opt-in skip), re-run in the close-out session with the same result [M]. Without overrides the planner reproduces plan-v3
byte-for-byte (queries sha256 2c714860…).

Candidates [M: `candidates/` via `tools/research/inputs/make_candidates.py`; physics roots
identical in all four: 116 roots, 271,990,954,170 finite starting targets; checker `pass` for
each]:

| Set | Helpers | queries.json sha256 | Diagnostic (`candidates/match-*`) |
|---|---|---|---|
| v3 (regression) | plan-v3: A_max on all 54 owners with t ≥ 8 | 2c714860… | (= match-v3, clean) |
| I1 | L* (40): interim shape (largest root rank, A ∞); others plan-v3 | e6cbc0c1… | clean: 183/183, 398,403 selected / 1,065 terminal / 0 unresolved, gap, invalid; 143 s |
| I1b (dropped, §8.1) | 27 owners outside L*: rank = v2 gen-7 max rank + 2, A = max finite A + 2; L* keep plan-v3 | 80f3e54d… | clean: 183/183, 431,853 / 1,096 / 0; 184 s |
| I1+I1b (dropped) | both | 8aa85f23… | clean: 183/183, 432,703 / 1,096 / 0; 177 s |

Envelope source [M: `v2g7-envelope/envelope.tsv`, `cp5hop` on a block clone of
`TMP/v2-checkpoint-copy-gen7`]: per owner the largest finite rank and A upper bound over all
Apply domains of v2 generation 7 (74,156,033 domains). Margin +2 on both is a choice, not a
derived bound. Helpers that change (R, A):

| owner | t | group | plan-v3 | I1 | I1b |
|---|---:|---|---|---|---|
| 011011000111111 | 10 | G | (4, 14) | (4, 14) | (10, 24) |
| 111001100111111 | 11 | G | (5, 15) | (5, 15) | (9, 25) |
| 111000100111001 | 8 | G | (2, 12) | (2, 12) | (12, 22) |
| 111010100100101 | 8 | G | (2, 12) | (2, 12) | (11, 21) |
| 111001100111001 | 9 | G | (3, 13) | (3, 13) | (11, 23) |
| 111101111111100 | 12 | G | (6, 16) | (6, 16) | (8, 24) |
| 011101110111000 | 9 | G | (3, 13) | (3, 13) | (12, 22) |
| 010011111101011 | 10 | U | (4, 14) | (4, 14) | (11, 23) |
| 111011100111111 | 12 | U | (6, 16) | (6, 16) | (8, 24) |
| 111001111101011 | 11 | U | (6, 16) | (6, 16) | (8, 24) |
| 111011111101011 | 12 | U | (6, 16) | (6, 16) | (8, 24) |
| 111101010011100 | 9 | U | (3, 13) | (3, 13) | (10, 23) |
| 011101111101100 | 10 | U | (4, 14) | (4, 14) | (10, 25) |
| 011101111111100 | 11 | U | (5, 15) | (5, 15) | (9, 25) |
| 011110111001001 | 9 | U | (4, 14) | (4, 14) | (10, 23) |
| 110110111101001 | 10 | U | (4, 14) | (4, 14) | (10, 25) |
| 111111001001001 | 9 | U | (6, 16) | (6, 16) | (8, 18) |
| 111100011111100 | 10 | U | (4, 14) | (4, 14) | (10, 24) |
| 111100111101001 | 10 | U | (4, 14) | (4, 14) | (10, 23) |
| 111101110101001 | 10 | U | (6, 16) | (6, 16) | (9, 18) |
| 111111001101001 | 10 | U | (6, 16) | (6, 16) | (8, 18) |
| 111111011101001 | 11 | U | (6, 16) | (6, 16) | (8, 18) |
| 111111101101001 | 11 | U | (6, 16) | (6, 16) | (8, 18) |
| 111110111001001 | 10 | U | (5, 15) | (5, 15) | (9, 22) |
| 111101111101001 | 11 | U | (6, 16) | (6, 16) | (8, 18) |
| 111011111101010 | 11 | U | (5, 15) | (5, 15) | (9, 24) |
| 111111111101001 | 12 | U | (6, 16) | (6, 16) | (8, 18) |

In I1 the 27 L* owners with t ≥ 8 switch from (R, A_max) to (R, ∞); the 13 owners with
t ≤ 7 are unbounded in plan-v3 already. Owners whose v2 envelope is the helper itself (one
Apply domain in v2) get (8, 18).

## 3. Four-loop validation of the L*/I1/I1b pipeline (static part)

Inputs: `TMP/four-loop-region-control.eazKG2/<fam>` (FG 124, BMW 134, H 314, X 328 owners, no
route maps); controls as in `TMP/fable51-controls/run_control.py` (FG/H/X: rank-12 anchors
with A ∞; BMW: `upstream-a19`, i.e. A19 on the 37 anchors with ≥ 6 active lines, A ∞ on 97).

- Guard owners (matching-only diagnostic of the rank-12, A ∞ anchors of every owner)
  [M: `four/match-r12orthant-*`]: FG, H, X none (0 unresolved in 248 / 628 / 656 queries);
  BMW exactly one, **0101111100** (14 unresolved pieces on its anchor).
- BMW geometric static reach: ∅ again (47,330 unsupported-support-change groups).
  Native one-hop of the 130 candidate full orthants (rank ≤ 20, A ∞): diagnostic clean
  (32,588 selected / 172 terminal / 0 unresolved, 11 s); 130 inspections, 0 frontiers, 0
  errors, 535 (owner, target) pairs, 0 support increases [M: `four/static-probe/`].
  **L_static(BMW, R ≤ 20) = 130 owners = L_obs(BMW)** (owner-level closure of the complete
  baseline run's 1,915,548 edges; no support increase in any family's baseline graph)
  [M: `four/static-reach-bmw-r20.json`, `four/base-*/transitions.tsv`].
- Only the guard owner and its three supersets (0111111100, 1101111100, 1111111100) can reach
  the guard. The historical BMW input bounds 37 owners; bounding only the guard owner was
  refuted earlier (220 frontiers, `TMP/four-loop-saved-descendants.VaNmUN/RESULTS.md`),
  consistent with the supersets being guard ancestors.
- Four-loop candidate inputs: FG, H, X have no owner outside L* (no guard), so I1 and I1b
  leave them unchanged. BMW: I1 = A ∞ on the 130 L* anchors, A19 on the 4 others (rank 12);
  I1b = the 4 non-L* anchors at the baseline envelope + 2 (guard (14, 33), supersets
  (14, 21)); I1+I1b = both. Diagnostics clean for all three (52,101-52,690 selected / 197
  terminal / 0 unresolved, 7 s each) [M: `four/match-bmw-*`].

## 4. Five-loop closure probes (≤ 60 min each, legacy engine)

Common command (`tools/research/inputs/probe.py`): the command shape of the v2 campaign
(Ready publication, lookahead 256, route over-cover, initial D-band reuse, unbounded work,
checkpointed) on all 183 queries of a candidate document; the 116 roots alias into their
helpers, so each probe equals an anchors-only probe up to 116 alias records. Binary 4a17f9c7,
W24, fresh start, cooperative stop requested at 3,240 s, then `cp5hop` reads the final CP5
checkpoint (per-owner Apply envelope, escapes from the helper box, owner-level transitions),
`analyze_probe.py` writes `analysis.json` and `lstar_reach.py` checks observed reachability
from L*. Foreign load (other users' or lanes' CPU time on the run's logical CPUs, from
`/proc/stat` minus the walk's own utime+stime; `tools/research/inputs/loadmon.py`) was
recorded for probe-v3b and probe-I1, not for probe-I1b. The host was shared (user `nfink`'s
unpinned gammaboard/postgres jobs floated over CPUs 88-127 and 344-383 throughout).

| probe | queries (sha256) | CPUs | foreign load | stop / wall | exit | own CPU s (time -v) | peak RSS |
|---|---|---|---|---|---|---|---|
| probe-I1b | plan-I1b 80f3e54d… | 100-117 + 362-367 | not recorded | 3,240 / 3,266 s | 4 | 32,057 | 11.8 GB |
| probe-v3b | plan-v3 2c714860… | 100-117 + 362-367 (same as I1b) | **16.6 %** | 3,240 / 3,261 s | 4 | 28,384 | 11.8 GB |
| probe-I1 | plan-I1 e6cbc0c1… | 88-95, 120-127 + 344-351 | **11.6 %** | 3,240 / 3,268 s | 4 | 28,644 | 11.0 GB |

probe-v3b and probe-I1 ran concurrently (2026-09-27 19:54-20:49 UTC) on disjoint CPU sets that
share no L3 (CCDs 88-95 and 120-127 vs 96-119); probe-I1b ran earlier (13:19-14:14 UTC).
CPU 100-117 + 362-367 is 18 physical cores, 6 of them with both SMT threads; probe-I1's set is
16 physical cores, 8 with both threads. Exit 4 = cooperative stop, incomplete by design.
**Every timing number in this section is void as an A/B** (foreign load > 10 % where recorded,
unknown for I1b, different topologies); counts at matched natives are the comparison.

### 4.0 probe-I1b (I1b inputs, dropped; kept as evidence for the drop) [M: `probe-I1b/`]

- **Frontiers: 0** at every 30 s heartbeat and in result.json. Errors: none.
- At the stop: 2,593,248 natives (2,408,460 Route, 184,788 Apply), 11,390,857 discovered
  domains, 5,374,792 pending, 180,945,561 dependency edges, max rank 16, 3 of 67 roots closed.
- **Guard masks**: 258,624 Apply domains were admitted in the 7 guard owners, 35,100 of them
  inspected; **0 with unbounded A**; max finite A 25-29 and max rank 8-14 per guard owner
  (v2 gen 7: 19-23 and 6-10). (The draft's "258,617 of 258,624 outside the helper box" is
  withdrawn: 258,624 − 258,617 = 7 = the helpers themselves; a descendant inside its helper box
  is never admitted as a separate domain, so every other domain is outside by construction.)
- Trajectory vs v2 (plan-v3 inputs, W100, binary 102adcc3; lens series `$S/inputs_lens/v2_ts.json`,
  interpolated at matched natives) [M-r, different worker count and binary; superseded as the
  comparison by the same-CPU control of §4.1]:

  | natives | I1b discovered | v2 discovered | ratio | I1b pending | v2 pending | roots closed I1b / v2 |
  |---:|---:|---:|---:|---:|---:|---|
  | 0.5M | 3.51M | 2.61M | 1.35 | 2.48M | 1.21M | 1 / 1 |
  | 1.0M | 4.75M | 4.73M | 1.00 | 2.38M | 1.95M | 1 / 6 |
  | 1.5M | 7.46M | 6.12M | 1.22 | 3.61M | 2.25M | 3 / 6 |
  | 2.0M | 9.01M | 7.62M | 1.18 | 4.07M | 2.82M | 3 / 6 |
  | 2.5M | 10.97M | 9.41M | 1.17 | 5.10M | 3.61M | 3 / 6 |

### 4.1 Same-CPU plan-v3 control (probe-v3b) [M: `probe-v3b/`, `probe-v3b.load.jsonl`]

Identical to probe-I1b except the query document (plan-v3) and the date: same binary, same
CPU set, same worker count, same stop time.

- **Frontiers: 0** throughout. At the stop: 3,369,229 natives (2,982,426 Route, 386,803 Apply),
  12,353,063 discovered, 5,225,779 pending, 171,165,326 edges, max rank 17, 6 of 67 roots closed.
- Guard masks: 500,944 Apply domains in the 7 guard owners (93,165 inspected), 0 with unbounded
  A, max finite A 16-21, max rank 6-8. The only unbounded-A Apply domains (568) are in the 13
  owners with t ≤ 7, all in L*.
- **Reproducibility of the control**: the killed first attempt `probe-v3/` (same command and
  CPUs, killed at 2,071 s by the session cut-off, heartbeats only) agrees with probe-v3b at
  matched natives to within 5 %: discovered 0.97-1.01x, pending 0.95-1.04x, edges 0.99x at
  1.0-2.5M natives [M: `compare-edges-v3-v3b-I1b-I1.txt`]. Count trajectories of this probe are
  therefore reproducible at the few-percent level despite the Ready schedule.

### 4.2 probe-I1 (I1 inputs) [M: `probe-I1/`, `probe-I1.load.jsonl`]

- **Frontiers: 0** at every heartbeat and in result.json (first frontier heartbeat: none).
- At the stop: 3,145,574 natives (2,799,982 Route, 345,592 Apply), 10,859,635 discovered,
  4,129,579 pending, 222,972,620 edges, max rank 17, 6 of 67 roots closed, RSS 10.4 GB.
- **Guard masks**: 560,485 Apply domains in the 7 guard owners (106,551 inspected), **0 with
  unbounded A**, max finite A 16-21, max rank 6-8 (the guard owners keep their plan-v3 helpers
  under I1).
- **Unbounded A only where intended**: 121,235 unbounded-A Apply domains, all in the 40 L*
  owners; 0 in any of the 27 owners outside L*.
- **L* is closed in the observed graph**: in the mask-level transition graph of the census
  (7,996 nodes, 222,972,620 domain edges) the Apply masks of the 40 L* owners reach 3,736 nodes,
  among them exactly the 40 L* owner Apply masks and no other owner: no guard owner, no U owner
  [M: `probe-I1/lstar_reach.json`]. Every domain edge maps to a mask edge, so no domain descended
  from an L* domain lies in a guard (or U) owner. The same holds in probe-v3b, probe-I1b and both
  I2 pilots [M: `*/lstar_reach.json`].

### 4.3 Domain volume at matched natives (v3b vs I1 vs I1b) [M: `volume-v3b-I1-I1b.json`, `compare-v3b-I1b-I1.txt`, `compare-edges-v3-v3b-I1b-I1.txt`]

`tools/research/inputs/volume.py` on the 30 s heartbeat series (linear interpolation at matched
natives = completed native inspections; roots closed as a step function). D = discovered
(scheduled) domains, P = pending (queued) domains, peak P = the largest pending seen up to that
point, D/n = discovered per native, P/n = pending growth per completion averaged from the start,
E = dependency edges, roots = initial roots closed (of 67):

| natives | plan-v3 (v3b): D / P / peak P / D/n / P/n / E / roots | I1 | I1b (dropped) |
|---:|---|---|---|
| 0.5M | 2.56M / 1.50M / 1.50M / 5.13 / 3.00 / 19.5M / 1 | 2.65M / 1.45M / 1.45M / 5.30 / 2.91 / 41.3M / 1 | 3.51M / 2.48M / 2.48M / 7.02 / 4.95 / 23.5M / 1 |
| 1.0M | 4.92M / 2.15M / 2.15M / 4.92 / 2.15 / 49.2M / 1 | 4.71M / 1.89M / 1.89M / 4.71 / 1.89 / 83.6M / 3 | 4.75M / 2.38M / 2.53M / 4.75 / 2.38 / 57.1M / 1 |
| 1.5M | 6.32M / 2.50M / 2.50M / 4.21 / 1.67 / 76.6M / 3 | 6.16M / 2.35M / 2.35M / 4.11 / 1.57 / 109.1M / 3 | 7.46M / 3.61M / 3.66M / 4.97 / 2.40 / 100.5M / 3 |
| 2.0M | 7.66M / 2.90M / 2.91M / 3.83 / 1.45 / 95.6M / 3 | 7.30M / 2.50M / 2.50M / 3.65 / 1.25 / 149.1M / 6 | 9.01M / 4.07M / 4.07M / 4.50 / 2.04 / 143.9M / 3 |
| 2.5M | 9.36M / 3.62M / 3.62M / 3.74 / 1.45 / 134.2M / 6 | 9.09M / 3.46M / 3.46M / 3.64 / 1.38 / 182.2M / 6 | 10.97M / 5.10M / 5.10M / 4.39 / 2.04 / 176.1M / 3 |
| 3.0M | 11.13M / 4.61M / 4.61M / 3.71 / 1.54 / 158.9M / 6 | 10.58M / 4.11M / 4.11M / 3.53 / 1.37 / 209.8M / 6 | (stopped at 2.59M) |
| at the stop | 3.37M natives: 12.35M / 5.23M / 5.23M / 3.67 / 1.55 / 171.2M / 6 | 3.15M: 10.86M / 4.13M / 4.13M / 3.45 / 1.31 / 223.0M / 6 | 2.59M: 11.39M / 5.37M / 5.37M / 4.39 / 2.07 / 180.9M / 3 |

Ratios to plan-v3 at 1.0-3.0M natives (discovered, pending, edges): **I1 0.95-0.98x,
0.86-0.96x, 1.32-1.70x**; roots close earlier (3 vs 1 at 1.0M, 6 vs 3 at 2.0M). **I1b (1.5-2.5M)
1.17-1.18x, 1.40-1.44x, 1.31-1.51x**, 3 vs 6 roots at 2.5M. Pending growth per completion over
the interval 1.0-3.0M: v3b 1.23, I1 1.11 (over 1.0-2.5M: v3b 0.98, I1 1.05, I1b 1.82; the
interval values are noisy under Ready, the from-start averages P/n above are the steadier
comparison).

Run-level side data [M: `probe-*.load.jsonl`]: foreign busy CPUs on the 24-CPU sets 3.94 (v3b)
and 2.77 (I1), not recorded for I1b; own CPU 28,382 s (v3b) and 28,643 s (I1) over 3,259-3,266 s.
Instructions per native were not recorded for the probes (no counters attached; the recorder was
added to `run_four.py` afterwards, §6.1). Own CPU per native (void as a timing A/B): v3b 118.7
natives per CPU-second, I1 109.8, I1b 80.9.

Census at the stop (phase split and Route→Route edges per Route native):

| probe | Apply natives | Route natives | Apply domains | Route→Route edges | Route→Route per Route native | edges per native |
|---|---:|---:|---:|---:|---:|---:|
| v3b | 386,803 | 2,982,426 | 1,727,639 | 103.8M | 34.8 | 50.8 |
| I1b | 184,788 | 2,408,460 | 1,381,365 | 135.5M | 56.3 | 69.8 |
| I1 | 345,592 | 2,799,982 | 1,480,255 | 150.5M | 53.8 | 70.9 |

Reading [E]:
- I1 is safe on this horizon (0 frontiers, L* closed under observed reachability, no
  unbounded-A domain in a guard mask) and lighter in domain volume (−2..−5 % discovered,
  −4..−14 % pending, −21 % peak pending at the stop at 7 % fewer natives, lower discovered per
  native), with earlier root closure. It records 1.3-1.7x more dependency edges per native, almost
  all Route→Route (150.5M vs 103.8M at the stop): the A-unbounded L* domains re-enter more route
  sub-masks. Edges are checkpoint and RAM volume; the long-run balance (fewer domains vs more
  edges) is not measured here.
- I1b does more domain work per native than plan-v3 on the same CPUs and closes half the roots;
  this is part of the evidence for the drop (§8.1).

### 4.4 Size of the I1 lever: native seconds by owner class [M: `owner-cpu/*.json`]

`tools/research/inputs/owner_cpu.py` sums the per-inspection `seconds` and counts of every
`native_inspection` record in the CP5 record sidecars, by owner and phase, and classes the Apply
owners as L (the 40 L* owners), G (the 7 guard owners) and U (the other 20); Apply domains come
from the `cp5hop` envelope. `seconds` is the inspector wall time the engine records per
inspection (includes contention; the class shares within one run are the robust part).

| run | Apply s (Route s, Route share) | L*: Apply domains / natives / seconds (share) | G: Apply seconds share | U: Apply seconds share | top-4 owners by Apply seconds |
|---|---|---|---|---|---|
| v2 gen 7 (block clone, 30 GB of records) | 241,437 (1,951; **0.8 %**) | 7,611,947 (**39.3 %**) / 2,218,742 (37.3 %) / 39,726 s (**16.5 %**) | 71.7 % | 11.9 % | 011101110111000 G 64.1 %, 010011111101011 U 5.2 %, 111001100111001 G 4.8 %, 110110111101001 U 3.9 %: **78.1 %, none in L*** |
| probe-v3b (plan-v3, 54 min) | 16,766 (385; 2.2 %) | 518,245 (30.0 %) / 124,283 / 2,923 s (17.4 %) | 47.0 % | 35.6 % | G 38.3 %, U 20.6 %, U 8.2 %, G 4.1 % |
| probe-I1 (I1, 54 min) | 17,370 (460; 2.6 %) | **180,132** (12.2 %) / 49,579 / 2,569 s (14.8 %) | 48.7 % | 36.5 % | G 37.5 %, U 17.2 %, U 8.9 %, L 6.0 % |
| C-5F orig-ord1 | 1,262 (19; 1.6 %) | 372,114 (99.6 %) / 270,209 / 1,248 s (98.9 %) | 1.1 % | 0 | all four in L* |

This re-derives the audit's figures: L* owners carry 16.5 % of the gen-7 Apply seconds and 39.3 %
of its Apply domains; the four hottest owners (78.1 %) are outside L*. Per Apply native a guard
owner costs 98 ms in gen 7 against 18 ms for L* and 15 ms for U.

Reading [E]:
- **I1 is a domain-count lever on the L* owners, bounded by their CPU share.** In the 54-min
  probes it removes 65 % of the L* Apply domains (518,245 → 180,132) and 60 % of the L* Apply
  natives, while the G and U owners, whose helpers it does not change, keep (slightly more) Apply
  domains (+12 % G, +4 % U, at 7 % fewer total natives). The L* Apply seconds fall only 12 % (2,923
  → 2,569 s; per L* Apply native 23.5 → 51.8 ms: the unbounded-A domains are fewer but larger), and
  total Apply seconds do not fall (16,766 → 17,370 s; timing indicative only). Even a complete
  removal of the L* Apply cost would bound the gain at about 16.5 % of the gen-7 Apply CPU; I1 is
  not a ≥ 2x work lever.
- C-5F, the one drained five-loop control, spends 99 % of its Apply seconds in L* owners and 1 %
  in the hot owner 011101110111000 (64 % in gen 7): it inverts the gen-7 cost mix, so lever gates
  on C-5F must be reported per owner class (HANDOFF §0.1 item 7).

## 5. Four-loop complete-run validation (dynamic) [M: `four/runs/ab-r{1,2}-*/<fam>/`, table `four/ab_table.txt`]

`run_four.py`: the control command lines of `run_control.py` (W6, Ordered, route over-cover,
D-band reuse, unbounded work) with the query document replaced; CPUs 100-105, nice 5; two
repeats, order reversed in repeat 2; foreign load on the pinned CPUs recorded (9-35 %;
higher in repeat 2, when the I2 pilots ran on CPUs 106-117); non-draining arms stopped
cooperatively at a time cap (600 s repeat 1, 300 s repeat 2; the repeat-1 I1+I1b arm was
stopped by hand at 734 s). "Drained" = exit 0, 0 pending, all initial domains closed.

| family / inputs | natives | discovered | frontiers | drained | traversal s (r1 / r2) | own CPU s (r1 / r2) | roots closed |
|---|---:|---:|---:|---|---|---|---|
| FG baseline = candidate | 98,869 | 98,909 | 0 | yes | 16.1, 15.2 / 16.2, 17.5 | 55, 51 / 54, 56 | 248/248 |
| H baseline = candidate | 24,680 | 24,929 | 0 | yes | 13.9, 14.6 / 14.9, 15.1 | 44, 48 / 49, 50 | 628/628 |
| X baseline = candidate | 46,826 | 47,193 | 0 | yes | 37.5, 37.3 / 47.3, 45.6 | 107, 108 / 131, 127 | 656/656 |
| BMW baseline (upstream A19: 97 A ∞, 37 A19) | 147,233 | 158,951 | 0 | yes | 37.4 / 44.5 | 111 / 131 | 268/268 |
| **BMW I1** (130 L* A ∞, 4 A19) | **9,389** | 9,823 | 0 | **yes** | **8.6 / 10.0** | **27 / 30** | 268/268 |
| BMW I1b (4 non-L* at envelope + 2) | ≥ 2,684,469 / ≥ 1,417,002 | ≥ 3.2M / ≥ 2.0M | 0 | **no** (cap) | 600 / 300 (cap) | 1,672 / 938 | 238 / 218 |
| BMW I1+I1b | ≥ 3,288,383 / ≥ 1,617,250 | ≥ 3.7M / ≥ 2.2M | 0 | **no** (cap) | 732 / 300 (cap) | 1,891 / 928 | 248 / 218 |

- FG, H and X have no guard owner, so every owner is in L* and neither I1 nor I1b changes
  their inputs: identical natives and discovered counts in both arms and both repeats.
- **BMW I1**: the static/observed L* (130) is exactly the set that can be unbounded; the run
  drains with 0 frontiers and all 268 roots closed using **15.7x fewer natives** and
  4.3-4.5x less traversal time and 4.2-4.4x less CPU than the historical input, whose 33 extra A19 anchors
  were never needed for guard safety. (Earlier refuted variant: A19 only on the guard owner
  gave 220 frontiers, because its 3 supersets reach it; they are outside L* here.)
- **BMW I1b** does not drain: raising the finite A cap of the guard owner and its three
  supersets from 19 to 33 / 21 (rank 12 → 14) gives > 18x the baseline natives within 600 s and
  still grows (193k pending at the cap), with 0 frontiers. Finite-A helpers fragment their
  descendants in A; a larger finite cap means more fragments, not absorption.
- The complete four-loop run on the full candidate construction (I1+I1b) therefore **fails**
  on BMW (does not drain in 300-734 s vs 37-45 s), while I1 alone passes on all four families.

Audits (`examples/python/audit_owner_domain_walk.py`, walk semantics of the result schema v3)
[M: `four/runs/*/audit.json`]: **PASS** for BMW baseline (147,233 natives, 11,718 aliases),
**BMW I1 in both repeats (9,389 natives, 434 aliases, all 268 initial obligations discharged)**,
and the FG, H, X baselines (98,869 / 24,680 / 46,826 natives).

## 6. I2 route-witness rewrite [M: `i2/`]

- Tool: `crates/rustred-app/examples/route_witness_rewrite.rs` (commits daba23d6, b949e064;
  binary sha256 fa623da4…). Symbolica check before writing algebraic code: `Matrix` over Q
  with `from_nested_vec`, `inv`, `det`, `rank`, `solve` and `&a * &b` exists in the vendored
  copy (953e26e2, `lib/numerica/src/tensors/matrix.rs`) and in upstream dev 445b882d (same
  file; fetched into `vendor/symbolica` FETCH_HEAD, checkout unchanged). `graphica`
  canonizes graphs and returns vertex/edge automorphism generators, but a sector's
  loop-momentum automorphisms (signed line permutations realised by unimodular loop maps,
  2-isomorphisms included) are not available there, so the search is combinatorial over
  signed images of a line basis, with every inverse, product and determinant done by
  Symbolica. The
  cancellation support of a witness is read off the expansion of each source irreducible
  numerator in the 15 line squares (one Symbolica inverse of the 15x15 basis matrix).
- Result (v2 gen-7 route traffic = outgoing dependency edges of Route nodes per source mask,
  557,718,765 edges; `route-traffic-v2g7.tsv`): 8,179 transported routes evaluated,
  **7,467 rewritten**; traffic-weighted cancellation support **7.559 → 5.562**
  (unweighted mean 6.39 → 4.81); support-free (literal-like) routes 41 → 134; 36 s, 77 MB.
  This reproduces the lens estimate (7.56 → 5.56, `witness6.tsv`) with exact arithmetic.
- **Load-time re-verification**: `selection-i2.json` (sha256 d9760837…) loads in `prepare`
  (every transported route through `symmetry::verify` and `integral_transport::compile`;
  98 s) and the plan-v3 matching diagnostic on it is clean and identical to match-v3
  (397,553 selected / 1,065 terminal / 0 unresolved, exit 0) [M: `i2/match-v3-i2`]. A first
  load check through `owner-domain-scan --max-numerator-rank 0` also passed preparation but
  stopped at the scan's default 16,384 summary groups (exit 4 by that cap, not by the load).
- **Pilot A/B** (not the W1.3 gate, which is on C-5F with audit): fresh walks on plan-v3
  queries, original vs rewritten selection, W6 each, 15 min, concurrently on CPUs 106-111 /
  112-117 [M: `i2/pilot-orig`, `i2/pilot-i2`]:

  | | original witnesses | I2 witnesses |
  |---|---:|---:|
  | natives in 15 min | 833,541 | 1,162,863 |
  | Route→Route edges per Route native (census at stop) | 35.78 | **15.61 (−56 %)** |
  | discovered at 800k natives | 3.86M | 2.92M (−24 %) |
  | pending at 800k natives | 2.01M | 1.39M (−31 %) |
  | dependency edges at 800k natives | 35.8M | 17.8M (−50 %) |
  | frontiers | 0 | 0 |
  | Apply natives in 15 min (census) | 21,223 | 53,836 (2.5x) |

  Early-phase and route-heavy (95-97 % of natives are Route); the long-horizon
  effect is not measured [E: the −20..−30 % route-domain model is consistent with −24 %
  discovered here]. The 2.5x Apply natives were the first sign of the Apply-side cost that the
  drained C-5F A/B (§6.1) makes explicit.
- **What kind of lever I2 is** [M for the rates, E for the reading]: it acts on Route→Route
  edges (−56 % per Route native in the pilot, −61 % on C-5F), i.e. on the dependency graph and
  the containment index, not on inspection cost: Route→Apply edges per Route native are unchanged
  (C-5F 0.938 → 0.951), and Route inspections are 0.8 % of the gen-7 native seconds (1,951 of
  243,388 s) and 1.3-2.6 % in C-5F and the probes (§4.4, §6.1). So I2 is at best a memory/index
  lever with ~0 direct CPU effect; its net effect on a drained closure is measured in §6.1 and is
  negative on C-5F.

### 6.1 W1.3 witness gate: C-5F A/B with audit [M: `i2/c5f/`, `i2/c5f/gate-ready.json`, `i2/c5f/gate-ordered.json`]

C-5F is the five-loop 1,324-tuple finite control: one query on owner 011101110111000 (rank ≤ 2,
A ≤ 11, D ≥ 9, sectors ≤ 15), command `TMP/ready-five-loop-finite-w50.a6ABXd/ready-first/command.json`
(Ready, lookahead 256, route over-cover, D-band reuse, unbounded work) with binary 4a17f9c7, W18,
owner base = the retired `five-loop-saved-coarse-cover` inputs (owner files byte-identical to
`five-loop-saved`, selection sha256 d2667dc9… in both). Arms: `orig` = that selection, `i2` =
`selection-i2.json` (d9760837…). The two arms of a pair ran concurrently on disjoint 18-core
sets (no SMT siblings used), cooperative stop at 1,500 s (never reached), then
`examples/python/audit_owner_domain_walk.py` and the `cp5hop` census. Repeat 2 swaps the CPU
sets. The Ordered pair (`--policy ordered`, tool option added in c498d0d6) removes schedule
dependence from the counts. Tool: `tools/research/inputs/c5f_gate.py`.

| | orig-1 | i2-1 | orig-2 | i2-2 |
|---|---:|---:|---:|---:|
| CPUs (foreign load) | 88-105 (16.1 %) | 106-123 (25.5 %) | 106-123 (28.6 %) | 88-105 (19.4 %) |
| drained (exit 0, 0 pending, 1/1 initial closed), frontiers | yes, 0 | yes, 0 | yes, 0 | yes, 0 |
| audit | **PASS** | **PASS** | **PASS** | **PASS** |
| **Route→Route edges per Route native** | 4.073 | **1.585 (−61.1 %)** | 4.098 | **1.593 (−61.1 %)** |
| natives | 980,360 | **1,492,647 (1.52x)** | 978,668 | **1,490,768 (1.52x)** |
| Apply natives | 273,717 | **583,564 (2.13x)** | 273,245 | **583,525 (2.14x)** |
| Route natives | 706,643 | 909,083 (1.29x) | 705,423 | 907,243 (1.29x) |
| discovered domains | 1,278,872 | 1,843,942 (1.44x) | 1,273,205 | 1,841,673 (1.45x) |
| dependency edges | 12,612,518 | 16,348,720 (1.30x) | 12,620,632 | 16,352,507 (1.30x) |
| own CPU s (timing void) | 1,420 | 2,118 | 1,605 | 1,992 |
| traversal s (timing void) | 200 | 412 | 243 | 409 |

The historical W50 run of the same control (binary 8f725507…, 2026-09-25) had 982,498 natives
(274,275 Apply, 708,223 Route): the `orig` arms reproduce it within 0.4 %.

**Ordered pairs** (`--policy ordered`; counts independent of the schedule) [M: `i2/c5f/gate-ordered.json`,
`i2/c5f/volume-c5f.json`]. Repeat ord1: orig on 88-105, i2 on 106-123; repeat ord2 swaps the sets
and runs each walk under `perf stat -e instructions:u,cycles:u` (`run_four.py --perf-stat`, whole
walk process including its ~140 s preparation) with the summed thread run delay recorded.

| | orig-ord1 | i2-ord1 | orig-ord2 | i2-ord2 |
|---|---:|---:|---:|---:|
| CPUs; foreign busy CPUs of 18 (share) | 88-105; 8.8 (49.5 %) | 106-123; 8.7 (48.7 %) | 106-123; 8.0 (45.1 %) | 88-105; 7.4 (41.5 %) |
| drained, frontiers, audit | yes, 0, **PASS** | yes, 0, **PASS** | yes, 0, **PASS** | yes, 0, **PASS** |
| **Route→Route edges per Route native** | 4.137 | **1.622 (−60.8 %)** | 4.137 | **1.622 (−60.8 %)** |
| Route→Apply edges per Route native | 0.938 | 0.951 | 0.938 | 0.951 |
| natives (Apply / Route) | 967,621 (271,475 / 696,146) | **1,472,821 (577,937 / 894,884)** | identical to ord1 | identical to ord1 |
| discovered domains (Apply / Route) | 1,273,376 (373,457 / 899,919) | **1,842,191 (745,755 / 1,096,436)** | identical | identical |
| dependency edges | 12,518,693 | 16,222,943 (1.30x) | identical | identical |
| peak pending (heartbeats) | 155,510 | 168,903 (1.09x) | 155,113 | 167,975 |
| discovered per native at the end | 1.316 | 1.251 | 1.316 | 1.251 |
| containment checks | 5.31e9 | 13.23e9 (2.49x) | identical | identical |
| peak RSS | 6.36 GB | 6.52 GB | 6.35 GB | 6.53 GB |
| **user instructions** (per native) | – | – | **9.690e12 (10.01M)** | **12.074e12 (8.20M): 1.246x total** |
| user cycles, IPC | – | – | 4.597e12, 2.11 | 5.586e12, 2.16 |
| thread run delay s (20 threads) | – | – | 627 | 1,306 |
| own CPU s (timing void) | 2,020 | 3,014 | 1,846 | 2,256 |

- The two Ordered repeats agree exactly in every count (natives, domains, edges, Route→Route
  edges, containment checks) across swapped CPU sets: the Ordered pair is an identity oracle and
  the I2 effect is not schedule noise. The Ready repeats (above) agree with Ordered within 1.4 %.
- Domain volume at matched natives [M: `volume-c5f.json`]: the two arms are nearly identical up
  to ~0.6M natives (discovered 0.73M vs 0.73M at 0.4M, 0.96M vs 0.98M at 0.6M; peak pending
  0.155M vs 0.169M), after which orig drains (pending 2,670 at 0.96M) while i2 still has 84,156
  pending and runs on to 1.47M natives. Per-native ratios (discovered per native 1.32 vs 1.25 at
  the end) therefore favour i2 and hide the 1.52x total; on a drained control the totals are the
  measure.
- CPU effect [M, n = 1, counters not affected by foreign load except through spin-waiting]: the
  i2 closure executes **1.25x the user instructions** (1.22x the cycles) of the orig closure:
  fewer instructions per native (8.2M vs 10.0M; the added Apply natives are cheap) but 1.52x the
  natives. The inspector seconds by owner class (`owner-cpu/{orig,i2}-ord1.json`) say the same:
  Apply seconds 1,262 → 1,589 (1.26x; 4.6 → 2.7 ms per Apply native), Route seconds 19 → 20, all of
  the added Apply work in L* owners (the hot guard owner 011101110111000 is 1 % in both).

**Verdict.** The W1.3 gate **as worded** ("≥ 20 % fewer Route→Route edges per route native, with
audit PASS") **passes**: −61.1 % in both Ready repeats and −60.8 % in both Ordered repeats, 8/8
audits PASS. **But I2 increases the total work of the drained C-5F closure**: 1.52x natives, 2.13x
Apply natives, 1.45x domains (2.0x Apply domains, 1.22x Route domains), 1.30x dependency edges,
2.49x containment checks, 1.09x peak pending, 1.25x user instructions, with the same Apply envelope
(identical max A and max rank per owner). Own CPU is 1.22-1.49x higher (void as a timing A/B:
foreign load 16-50 %; indicative only). As a memory lever it does not pay on C-5F either: peak RSS
6.35 → 6.53 GB and 1.45x domains / 1.30x edges at the end. The gate metric
is a proxy that does not capture the Apply side, and on the one drained control where the total is
measurable, the total goes the wrong way. The W6 pilot on plan-v3 (§6) had the same warning in
its census (Apply natives 21,223 → 53,836, 2.5x, in the same 15 min) that the matched-native
discovered/pending comparison hid.

Where the extra Apply work lands [M: census envelopes of orig-1 / i2-1]: of the 13 owners with
Apply domains in C-5F, 11 grow by 347 to 124,315 inspected Apply domains, led by
000011001001011 (59,218 → 183,533), 101010000110001 (27,990 → 106,379) and 001100101110000
(17,534 → 66,433); the query owner is unchanged (1,267 → 1,268) and 111000010101001 shrinks
(43,544 → 34,364). Max A and max rank per owner are unchanged, so the growth is fragmentation of
the same region, not a larger region.

Mechanism, a hypothesis [E]: the rewrite picks, route by route, the owner automorphism that
minimises that route's cancellation support. Routes into one owner therefore land in many
different frames of the owner's coordinates (e.g. 34 distinct automorphisms for the 44 routes
into 000011001001011, 136 for the 633 routes into 001100101110000; `i2/report.tsv`), so routed
images of related sub-sector domains stop being contained in each other and in the owner's own
Apply domains; 1,329 of the 7,467 changed routes changed only on the term-count tie-break, with
no support gain. Candidate follow-ups for W1.3 (not done here): one frame per owner (a single
automorphism per target owner minimising the owner's traffic-weighted support), or change a
route only when it strictly lowers support and keeps the owner frame; and a gate that also
requires total natives and Apply natives on the drained C-5F not to increase.

## 7. Gate verdicts

| gate (master plan) | criterion | evidence | verdict |
|---|---|---|---|
| 0.6 I1 | I1 ships only for L* owners | L* = 40 owners (§1); I1 changes only L* helpers (§2); diagnostic clean 183/183 (§2); four-loop BMW I1 drains, audit PASS, 2/2 (§5); five-loop probe-I1: 0 frontiers, 0 unbounded-A guard domains, unbounded A only in L* owners, L* closed under observed reachability (§4.2) | **PASS** [M] (static part for rank ≤ 32 only; dynamic part a 54-min horizon); lever bounded to ~16.5 % of gen-7 Apply CPU (§4.4); in-flight guard proposed (§8.2) |
| 0.6 I1b | (gate removed with I1b) | four-loop BMW I1b does not drain; five-loop I1b 1.17-1.18x discovered, 1.40-1.44x pending, half the roots (§4.3, §5); the reinterpreted gate cannot fail (§8.1) | **DROPPED** (audit directive; not evaluated as a gate) |
| 0.6 I2 | witness rewrite and load verification | 8,179 routes evaluated, 7,467 rewritten; `selection-i2.json` loads through `symmetry::verify` and `integral_transport::compile`; plan-v3 diagnostic on it identical to match-v3 (§6); I1 diagnostic on it identical to match-I1 (398,403 / 1,065 / 0 unresolved, gap, invalid; `i2/match-I1-i2`, 182 s) | **PASS** [M] |
| W1.3 inputs: matching diagnostic clean | for the v4 candidate (I1 queries + I2 selection) | `i2/match-I1-i2`: clean, 183/183 | **PASS** [M] |
| W1.3 inputs: 1-h L*-only walk | 0 frontiers and no domain in a guard mask | not run as an L*-only walk. probe-I1 (all 183 queries) shows that nothing reached from the L* owners enters a guard or U owner (mask-level closure, §4.2), which is the same observable [M]; the formal L*-only run remains to be done with the frozen v4 inputs, ideally with the in-flight guard of §8.2 | **OPEN** (evidence favourable) |
| W1.3 inputs: witness A/B on C-5F | ≥ 20 % fewer Route→Route edges per route native, audit PASS | −61 % in 2 Ready and 2 Ordered repeats, all audits PASS (§6.1) | **PASS as worded** [M]; **but total C-5F work rises 1.52x natives / 2.13x Apply natives / 1.45x domains** [M]: I2 as built should not ship on this gate alone (§6.1, §8.3) |

`family_closure_claim` stays false: nothing here is a closure, termination or drain claim for
the five-loop walk.

## 8. I1b drop, the in-flight guard for I1, and what remains open

### 8.1 I1b is dropped (recorded, not an owner decision)

Per the audit directive (HANDOFF §0.1 item 5, CRITIQUE §2.7 INPUTS-1) I1b is removed from the
inputs and its gate wording is not brought to the owner. The lane's evidence for the drop:
- four-loop BMW I1b does not drain in either repeat: ≥ 2,684,469 natives at the 600 s cap and
  ≥ 1,417,002 at the 300 s cap, still growing (193k pending at the first cap), against 147,233
  natives drained in 37-45 s for the baseline input [M, §5];
- five-loop, same binary, same CPU set as the full 54-min plan-v3 control: at 1.5-2.5M natives
  I1b has 1.17-1.18x the discovered domains, 1.40-1.44x the pending, 1.31-1.51x the edges,
  discovered per native 4.39 vs 3.74 at 2.5M, and closes half the roots (3 vs 6) [M, §4.3]. The
  audit's 1.19-1.20x / 1.45-1.49x come from the killed partial control; the full control gives the
  slightly lower values above. The wall per native ratios (1.4-1.9x at 1.5-2.5M) are void as a
  timing A/B (probe-I1b's foreign load was not recorded; probe-v3b's was 16.6 %);
- the reinterpreted gate ("0 unbounded-A domains in a guard mask") cannot fail: unbounded-A
  Apply domains only descend from A-null helpers, and the set of A-null helpers is the same in
  plan-v3 and I1b (I1b changes only finite caps), so the measured 0 carries no information about
  I1b; read literally ("no domain in a guard mask") the gate fails for any input (258,624 in
  probe-I1b), because the guard owners' own helper queries are domains in guard masks.

Mechanism [E]: finite-A helpers fragment their descendants in A; a larger finite cap gives more
fragments, not absorption. I1b artifacts stay in TMP as evidence (`candidates/plan-I1b*`,
`probe-I1b/`, `four/runs/*/bmw` I1b arms); nothing ships.

### 8.2 In-flight guard for I1 (proposal, not implemented)

What it protects: I1 makes the 40 L* owners A-unbounded on the strength of (a) the static
closure (rank ≤ 32 only, §1.1) and (b) observation (no L* descendant in a non-L* owner in v2
gen 3/7, interim gen 8 and every probe here). Rank > 32, or a region not yet visited, is outside
both. An unbounded-A domain that reached a guard owner is exactly what produced the 1,299
interim frontiers, so the walk should stop at the first edge that leaves the region instead of
at the frontier it would cause later.

Rule: let R* be the (phase, mask) closure of the 40 L* owner Apply masks in the static model
(the owner → target-sector → owner/route-mask graph of `static_reach.py` with the one-hop
refinement, `static-reach-r32.json`), a set of at most 2 x 2^15 nodes. **Stop on any dependency
edge whose source domain's (phase, mask) is in R* and whose target's is not**; in particular on
any Apply edge from an L* owner into a non-L* owner mask or into a route mask outside R*. The
coordinator's minimal form (source Apply in an L* owner, target in a non-L* owner mask) is the
first-hop special case; the region form also catches a descent through route masks, which is how
L* domains reach other owners at all (Apply → Route → Apply).

Where it is checked (legacy engine, `crates/rustred-app/src/application/routed_campaign/walking/`):
- `execution.rs`, `fn dependency(&self, target)` (line 307, called at lines 506 and 602): the
  single place where every admitted or deduplicated successor edge of a native inspection is
  recorded (`closure.edge(self.dependency_source(), target)`), on the coordinator in commit
  order; source and target domains are `self.queue.domains[..]`, whose `owner()` is the mask and
  `phase()` the phase (the same fields the progress events print). This is the check that
  matters;
- `execution/delegation.rs` line 69 (`closure.edge(id, to)`: a delegated domain to its
  representative, same phase and owner by construction) and `execution.rs` line 689
  (`closure.edge(id, scope.anchor_id)`: partial initial scopes to their anchor): cannot leave
  R* by construction; an assertion suffices.
Action on a hit: record a frontier-class stop record (source/target id, masks, phases, power
bounds, rank) and request the cooperative stop (the stop-file path of the legacy walk; the same action I4's
frontier policy `stop` is to take, which the legacy engine does not implement yet), so the checkpoint
is saved and the offending edge is inspectable. Cost: two table lookups and a bitset test per
edge (12-223M edges per run here), no extra memory beyond a 2^16-bit table. In the epoch engine
the same test belongs in the epoch merge, where successor edges enter the shared graph.
Input: an `R*` file (`rustred.region-guard.v1`: masks per phase + digest of the static-reach
receipt), passed by flag and recorded in the result receipt. Validation before use: replay the
edge sections of probe-I1, probe-v3b and v2 gen 7 through the rule offline (expected 0 hits, as
`lstar_reach.py` found at mask level for the observed reach); a mutation test that adds one
non-R* route must stop the four-loop BMW I1 run at its first crossing edge.

False positives: a stop is conservative (it costs a restart with a corrected L* or R*, never
soundness). Domains of a route mask in R* that were reached from U owners share the mask and
are checked too; if the static model is complete this cannot fire for them either.

### 8.3 Open: the I2 witness gate (D-I2)

The W1.3 witness gate as worded passes (−61 % Route→Route edges per route native, all audits
PASS), yet on the same drained control I2 needs 1.52x the natives, 2.13x the Apply natives,
1.45x the domains and 1.30x the edges (§6.1). Proposed, for the owner or orchestrator to accept
or reject:
- (a) amend the W1.3 witness gate to: "on C-5F, with audit PASS, ≥ 20 % fewer Route→Route edges per
  route native **and** total natives, Apply natives and discovered domains not higher than with the
  original witnesses (Ordered counts), reported per owner class";
- (b) do not ship `selection-i2.json` as built (it fails the amended gate); rebuild I2 with one frame
  per owner (or strict-improvement-only changes) and re-run the C-5F A/B before freezing v4.
Default if unanswered: (a) and (b), i.e. v4 = I1 + I4 frontier stop + the L* in-flight guard, with
the original witnesses until a rebuilt I2 passes the amended gate. The physics scope and soundness
are untouched either way (both witness sets are verified at load, and all audits pass).

### 8.4 Confirmation (default, no decision needed unless the owner objects)

I1 ships for the 40 L* owners (gate 0.6 PASS, §7) as a domain-count lever bounded to about 16.5 %
of the gen-7 Apply CPU (§4.4). It trades −2..−5 % discovered domains, −4..−14 % pending and earlier
root closure for 1.3-1.7x more dependency edges per native in the first 54 min (§4.3); the RAM
budget should be re-projected with the I1 edge rate once the W1.3 L*-only walk has run.

## 9. Corrections to earlier summaries

- **Tautology withdrawn.** "19,365,999 of 19,366,066 v2 gen-7 Apply domains lie outside their
  plan-v3 helpers" (draft `RESULTS.md` §4) says nothing: 19,366,066 − 19,365,999 = 67 = the helpers themselves, and a
  descendant inside its helper box is never admitted as a separate domain, so every other domain
  is outside by construction. The same holds for the draft's "258,617 of 258,624 guard-mask Apply
  domains outside the I1b helper box" (the difference, 7, is the 7 guard helpers). Neither is
  used any more. The master plan's "82 % are A-escapes" is a different statistic; it supports
  unbounded-A absorption (unsafe for U owners), not finite envelopes, and is not cited for I1b
  (CRITIQUE §2.7 INPUTS-3).
- **I1b framing.** The draft presented I1b's gate wording and go/no-go as owner decisions
  (D-I1b-a/b). Replaced by the recorded drop (§8.1).
- **I1 framing.** The draft's summary read I1 as a domain saving without its size. Re-derived
  here: L* = 16.5 % of gen-7 Apply seconds, 39.3 % of Apply domains, top-4 owners (78.1 %) outside
  L* (§4.4), matching the audit.
- **Audit figures for I1b vs plan-v3** (1.19-1.20x scheduled, 1.45-1.49x pending, 1.54-1.73x wall
  per native): the scheduled and pending ranges come from the killed partial control and the
  first full one; with the full same-CPU control the values are 1.17-1.18x and 1.40-1.44x at
  1.5-2.5M natives (§4.3). Wall per native is void as an A/B (foreign load unrecorded for I1b).
- HANDOFF_opus_5_5.md §7.9 and the draft of this file: "258,624 Apply domains in guard owners,
  35,093 of them inspected". 35,100 were inspected; 35,093 is the number inspected **outside the
  helper box** (`probe-I1b/analysis.json`, `guard_totals.outside_inspected`). Minor.
- HANDOFF §7.9, same-CPU I1b vs plan-v3 at 2.5M natives (1.20x discovered, 1.49x pending, 1.32x
  edges, 3,131 s vs 2,039 s) came from the killed partial probe-v3. They are close to the full
  control (probe-v3b: 1.17x, 1.41x, 1.31x); the wall-clock ratio is void as a measurement
  (foreign load 16.6 % on probe-v3b, unrecorded on probe-I1b and probe-v3).
- The unverified numbers flagged in HANDOFF §7.9 were re-checked from the files and hold:
  1,958,316 unsupported support-change regions (`scan5/result.json`, per-owner sum of
  `potential_unsupported_support_change_regions`), 506 one-hop (source, target phase, target mask)
  pairs identical at r24 and r32 over 40 sources (`static-probe/hop-r{24,32}/hop/hop.tsv`),
  7,762,250 ancestor nodes of which 3,048,864 are Apply nodes in 27 owners, no L* owner among
  them (`interim-gen8/ancestors.tsv`).
- Planner and checker suites re-run in this session: 48 tests OK, 1 opt-in skip [M].

## 10. Reproduction and artifacts

Tools (branch `fable_5_1-v3-inputs`, `tools/research/inputs/`): `cp4scan.rs`, `cp5hop.rs`,
`static_reach.py`, `one_hop.py`, `make_candidates.py`, `probe.py`, `run_match.py`,
`rewrite_anchors.py`, `analyze_probe.py`, `run_four.py` (`--family five-finite`, `--manifest`,
`--workers`, `--policy`, `--perf-stat`; thread run delay recorded), `compare_probes.py`,
`loadmon.py`, `lstar_reach.py`, `c5f_gate.py`, `volume.py` (domain volume at matched natives),
`owner_cpu.py` (native seconds by owner class);
`crates/rustred-app/examples/route_witness_rewrite.rs`; planner option `--helper-bounds-from`
(`examples/python/plan_renormalization_entry_queries.py`, checker, tests). The session scripts
are copied under `tools/research/inputs/sessions/2026-09-27/`.

Binaries: walk `TMP/fable51-controls/bin/rustred-4a17f9c7` (sha256 4a17f9c7c044…395e);
`cp5hop` b215cf03a40b…; `cp4scan` e44815d02d56…; `route_witness_rewrite` fa623da4be6f….

Commands of this close-out (from `/common/dev/rustred`, `TMPDIR=/common/dev/rustred/TMP`):

```sh
# probes (both at once, 54 min): probe-v3b on 100-117,362-367 and probe-I1 on 88-95,120-127,344-351
TMP/w0/inputs/run_probes_b.sh
# analysis
G=011011000111111,111001100111111,111000100111001,111010100100101,111001100111001,111101111111100,011101110111000
python tools/research/inputs/analyze_probe.py TMP/w0/inputs/probe-v3b --guards $G --output TMP/w0/inputs/probe-v3b/analysis.json
python tools/research/inputs/lstar_reach.py TMP/w0/inputs/probe-I1 --lstar TMP/w0/inputs/candidates/lstar.json \
  --guards $G --owners TMP/w0/inputs/owners.txt --output TMP/w0/inputs/probe-I1/lstar_reach.json
python tools/research/inputs/compare_probes.py v3b=TMP/w0/inputs/probe-v3b I1b=TMP/w0/inputs/probe-I1b \
  I1=TMP/w0/inputs/probe-I1 --at 500000,1000000,1500000,2000000,2500000,3000000
# v4-candidate diagnostic (I1 queries + I2 selection), 3 min on one CPU
python tools/research/inputs/run_match.py --queries TMP/w0/inputs/candidates/plan-I1/queries.json \
  --selection TMP/w0/inputs/i2/selection-i2.json --out TMP/w0/inputs/i2/match-I1-i2 --cpu 124
# C-5F witness A/B, W18 per arm, ~5-10 min per pair incl. audits
TMP/w0/inputs/i2/c5f_ab.sh 1 88-105 106-123
TMP/w0/inputs/i2/c5f_ab.sh 2 106-123 88-105
TMP/w0/inputs/i2/c5f_ab_policy.sh ord1 88-105 106-123 ordered
python tools/research/inputs/c5f_gate.py TMP/w0/inputs/i2/c5f orig-1:i2-1 orig-2:i2-2 --output TMP/w0/inputs/i2/c5f/gate-ready.json
TMP/w0/inputs/i2/c5f_ab_policy.sh ord2 106-123 88-105 ordered instructions:u,cycles:u
python tools/research/inputs/c5f_gate.py TMP/w0/inputs/i2/c5f orig-ord1:i2-ord1 orig-ord2:i2-ord2 --output TMP/w0/inputs/i2/c5f/gate-ordered.json
# domain volume at matched natives
python tools/research/inputs/volume.py v3b=TMP/w0/inputs/probe-v3b I1=TMP/w0/inputs/probe-I1 I1b=TMP/w0/inputs/probe-I1b \
  --output TMP/w0/inputs/volume-v3b-I1-I1b.json
C=TMP/w0/inputs/i2/c5f; python tools/research/inputs/volume.py orig-ord1=$C/orig-ord1/five-finite i2-ord1=$C/i2-ord1/five-finite \
  orig-ord2=$C/orig-ord2/five-finite i2-ord2=$C/i2-ord2/five-finite orig-1=$C/orig-1/five-finite i2-1=$C/i2-1/five-finite \
  --at 200000,400000,600000,800000,960000,1200000,1400000 --output $C/volume-c5f.json
# native seconds by owner class (gen 7 from the lane's block clone of TMP/v2-checkpoint-copy-gen7; 75 s on 8 CPUs)
python tools/research/inputs/owner_cpu.py .claude/worktrees/fable51-inputs/TMP/v2-gen7 --lstar TMP/w0/inputs/candidates/lstar.json \
  --guards $G --owners TMP/w0/inputs/owners.txt --envelope TMP/w0/inputs/v2g7-envelope/envelope.tsv --jobs 8 \
  --output TMP/w0/inputs/owner-cpu/v2-gen7.json
# likewise probe-v3b, probe-I1 (checkpoint/ + census/envelope.tsv) and the C-5F arms -> owner-cpu/<name>.json
```

Run directories (all under `/common/dev/rustred/TMP/w0/inputs/`): `probe-I1b/`, `probe-v3b/`,
`probe-I1/` (receipt.json, result.json, analysis.json, lstar_reach.json, census/, checkpoint/,
timeseries.jsonl), `probe-v3b.load.jsonl`, `probe-I1.load.jsonl`, `compare-v3b-I1b-I1.txt`,
`compare-edges-v3-v3b-I1b-I1.txt`, `probe-v3/` (killed first attempt, heartbeats only, kept for
the reproducibility check), `i2/match-I1-i2/`, `i2/c5f/{orig,i2}-{1,2,ord1,ord2}/five-finite/`
(metrics.json, audit.json, census/, result.json, checkpoint/), `i2/c5f/*.load.jsonl`,
`i2/c5f/gate-*.json`, `i2/c5f/volume-c5f.json`, `i2/c5f/ab-*.log`, `volume-v3b-I1-I1b.json`,
`owner-cpu/*.json`; earlier: `candidates/`, `four/`, `static-probe/`,
`scan5/`, `interim-gen8/`, `v2g7-*`, `i2/` (rewrite, pilots).

## 11. Caveats

- **Foreign load.** The host was shared throughout (user `nfink`'s unpinned gammaboard/postgres
  jobs and other users' builds floated over CPUs 88-127 and 344-383). Recorded foreign load:
  probe-v3b 16.6 % (3.9 foreign busy CPUs of 24), probe-I1 11.6 % (2.8 of 24), C-5F arms 16-29 %
  (Ready) and 49-50 % (about 8.7-8.8 foreign busy CPUs of 18) in the first Ordered pair. Every wall-clock and CPU-time comparison in this note is void as
  an A/B (threshold 10 %) and is quoted as indicative only; counts are the evidence. probe-I1b's
  foreign load was not recorded.
- **Ready schedule sensitivity.** Ready counts depend on the schedule. Where tested they
  reproduce well: probe-v3 vs probe-v3b within 5 % at matched natives; C-5F Ready repeats within
  0.4 % in every count; C-5F `orig` vs the historical W50 run within 0.4 %.
- **Horizon.** The five-loop probes cover the first 54 min of a fresh walk (≤ 3.4M natives,
  6/67 roots closed). Nothing here measures the long-run balance of I1 (fewer domains, more
  edges) or of I1b, and nothing is a drain or termination statement for the five-loop walk.
- **Topology.** probe-I1 ran on 16 physical cores (8 with both SMT threads), probe-v3b and
  probe-I1b on 18 (6 with both). This affects timing only.
- **Static part of L*.** L_static is proved for domains of rank ≤ 32 only (observed maximum
  rank 17-18); rank ∞ is refused by the guard algebra limit. A rank escape above 32 in an L* owner
  must remain a monitored stop condition. The soundness argument for the one-hop refinement
  (§1.1) is the lane's own and was not independently reviewed.
- **Mask-level reachability** (`lstar_reach.py`) over-approximates domain-level reachability;
  "no guard mask reached from L*" is therefore a sufficient condition, and it held in all runs.
- **I2 mechanism.** The frame-inconsistency explanation of the C-5F Apply growth (§6.1) is a
  hypothesis consistent with the automorphism counts; it was not isolated by an experiment.
- **C-5F is a single hot-owner control.** Its route traffic is not the five-loop campaign's; the
  sign of the I2 total-work effect elsewhere is not measured, but C-5F is the only drained
  five-loop control available and the plan names it as the gate.
