# W0 intel: lens tools (W0.1) and offline gen-7 index replay (W0.4), gate 0.4 verdict

Status: W0.1 done; W0.4 measured, gate 0.4 decided. Branch `fable_5_1-v3-intel` (from
`fable_5_1` @ b15316b9): 055986ba research-only `admission-trace` cargo feature (off by
default), 43fdec96 `tools/research/` lens tools and `idxreplay`, 1d5b80a3 idxreplay v4 and the
gate-0.4 projection (`project.py`). Plan: `fable51_next_push_master_plan_2026-09-27.md` §5
(W0.1, W0.4, gate 0.4), §3.2-3.3. Evidence: `/common/dev/rustred/TMP/w0/intel/` (working copy
`RESULTS.md`; `TMP/...` paths below are relative to `/common/dev/rustred` and untracked). The
small receipts (every replay JSONL, the markdown tables, the projection grid, the foreign-load
samples and the driver scripts as run) are copied to `tools/research/outputs/idxreplay/`.
Labels: **[M]** measured (file, run directory or binary sha256 cited), **[E]** estimate with
the model stated. Nothing here is an ETA or a closure claim; `family_closure_claim` stays false
and termination of the five-loop walk is not established.

## Verdict in brief

1. **Gate 0.4(a): PASS, adopt the SoA compare kernel [M].** 32-entry SoA blocks with u8 lanes
   in ID order need 5.2-10.4x less CPU per tested candidate than today's layout on every
   request set, session and thread count measured; in pattern order 4.2-10.4x on forward sets,
   3.3-5.1x on reverse checks per candidate (below 4x only in contaminated or heavily loaded
   cells), and 15.7-24x less CPU per reverse request.
2. **Layout depends on the resolution rule [M].** First-found (epoch Rolling): SoA-pattern is
   best on real gen-7 streams (580 candidates per hit, 2,626 per miss, 2,938 per reverse check;
   SoA-id 910 / 3,902 / 9,999; today 1,134 / 4,585 / 12,330). Minimum-ID (legacy engine, W1.1
   strict Ordered identity, W2 canonical Lockstep): SoA in ID order; pattern ordering loses the
   min-ID early exit (6,720 candidates per hit vs 2,096 in ID order and 2,322 today).
3. **Gate 0.4(b): the trigger fires, W3.2 (miss path) moves into W2 [E].** The projected
   admission share of worker CPU at N = 1G for the chosen design (pipeline + SoA-pattern
   first-found) is 6.3% at 74M and 16.8-35.0% at 1G with one-thread costs (alpha 0.44-0.82),
   28.3-57.0% with the measured 48/90-thread factors; it exceeds 25% in 37 of 45 grid cells.
   Misses plus reverse scans are 56% of the projected admission CPU at 74M, hits 38%.
4. **Gate 0.4(c): not triggered [M].** Cheap tiers resolve 76.6-88.9% of real gen-7 requests
   (MRU k = 1..64), 55.3-72.0% on C-5F and 73.2-98.7% on the four-loop runs.
5. **The replay is exact [M].** Rebuilding today's index in commit order reproduces every
   admission outcome, maintenance charge and retirement count of six traced runs (four-loop
   FG/BMW/H/X W6, FG W1, C-5F W18; 26,748,231 admissions, 0 mismatches), so `idxreplay` can
   serve as the differential oracle for the W1.1 kernel.

## Consequences for W1-W5 [E unless stated]

- **W1.1 kernel** (legacy lane, min-ID, strict Ordered identity): SoA blocks in ID order; the
  >= 4x per-candidate gate is met by SoA-id in every measured condition [M].
- **W2.3 inspector resolution**: layers in finite-upper-pattern order for first-found. The
  canonical Lockstep mode either keeps an ID-ordered copy or accepts about 2.5x CPU per hit
  (57.6 vs 23.3 µs at one thread [M]) on controls; a per-block minimum-ID skip in pattern
  order is untested. The W2.3 first-found vs canonical gate licenses first-found in production.
- **W2 gains W3.2**: a sublinear structure for forward misses and reverse (retirement) scans
  (per-group dominance pruning on sorted lower corners, or the 2N+6 feature trie), gated by the
  exponent measured on P-IMP (M-adm), not by the extrapolated alpha.
- **Route jobs** are 79% of jobs in the gen-7 window and 84.7% of their requests reach the
  layers (one request per job, no MRU/Local benefit) [M]: Route micro-batching (§3.4) matters
  for admission cost as well as for dispatch.
- **Snapshot refresh (A6)**: request-weighted stale share is 1.5% at a lag of 1,024 new IDs,
  2.6% at 65,536 and 3.5% at 1M [M, gen-7 streams]; the edge-based figures agree.
- **Measurement**: the 48/90-thread sweep ran once under 25-33 foreign busy CPUs; a clean
  repeat (<= 10% foreign load, >= 2 interleaved repeats, v4 LoadMon fields, `schedstat`) is
  still open and is the only way to settle the SoA-pattern reverse ratio at high thread counts.
