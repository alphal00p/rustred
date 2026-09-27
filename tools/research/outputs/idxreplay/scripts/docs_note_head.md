# W0 intel: lens tools (W0.1) and offline gen-7 index replay (W0.4), gate 0.4 verdict

Status: W0.1 done; W0.4 measured. Gate 0.4: (a) PASS provisional, (b) borderline and
conditional with W3.2 moved into W2 as a conservative choice, (c) not triggered.

Branch `fable_5_1-v3-intel` (from `fable_5_1` @ b15316b9): 055986ba research-only
`admission-trace` cargo feature (off by default), 43fdec96 `tools/research/` lens tools and
`idxreplay`, 1d5b80a3 idxreplay v4 and the gate-0.4 projection (`project.py`), 92a5eed2 the
first version of this note, 82470689 `project.py --class-alpha` (fix round after verifier
round 1). Plan: `fable51_next_push_master_plan_2026-09-27.md` §5 (W0.1, W0.4, gate 0.4),
§3.2-3.3. Evidence: `/common/dev/rustred/TMP/w0/intel/` (working copy `RESULTS.md`;
`TMP/...` paths below are relative to `/common/dev/rustred` and untracked). The small receipts
(every replay JSONL, the markdown tables, the projection grids, the foreign-load and lock-state
samples and the driver scripts as run) are copied to `tools/research/outputs/idxreplay/`.
Labels: **[M]** measured (file, run directory or binary sha256 cited), **[E]** estimate with
the model stated. Nothing here is an ETA or a closure claim; `family_closure_claim` stays false
and termination of the five-loop walk is not established.

Corrections to the first version of this note (92a5eed2) and to the lane report, after
verifier round 1:
- Gate 0.4(a) is a provisional PASS, not a clean [M] PASS. Every timing is void as an A/B under
  plan §7, and the three sub-4x SoA-pattern reverse cells are no more contaminated than the
  others.
- Gate 0.4(b) is borderline and conditional. "Exceeds 25% in 37 of 45 cells" overstated it:
  the 9 cost settings are 4 distinct assumptions, and at SoA-pattern's own thinning exponents
  the trigger does not fire.
- The multi-thread factors are not an "upper-side" estimate; the direction of their bias is
  unknown.
- The socket-1 pass windows and their foreign load are recomputed.
- The lane report's "socket1.lock held by the knobs lane at 20:30Z" was written before 20:10Z
  and had no receipt. The lock state is now recorded with real times
  (`socket1-lockstate-2026-09-27T2023Z.txt`).

## Verdict in brief

1. **Gate 0.4(a): PASS, provisional; adopt the SoA compare kernel.** The candidate counts are
   [M]. Every timing is void as an A/B under plan §7 (foreign load above 10% or unrecorded, no
   interleaved repeats), so the PASS rests on the margin. In ID order, 32-entry SoA blocks with
   u8 lanes need 5.2-10.4x less CPU per tested candidate than today's layout in all 11 session
   x thread-count cells and on every request set, at least 1.3x above the 4x line. In pattern
   order they need 4.2-10.4x less on forward sets and 3.3-5.1x less on reverse checks per
   candidate. SoA-pattern's reverse ratio is undecided against 4x until a clean repeat. Per
   reverse request it needs 15.7-24x less CPU than today.
2. **Layout depends on the resolution rule [M].** First-found (epoch Rolling): SoA-pattern is
   best on real gen-7 streams (580 candidates per hit, 2,626 per miss, 2,938 per reverse check;
   SoA-id 910 / 3,902 / 9,999; today 1,134 / 4,585 / 12,330). Minimum-ID (legacy engine, W1.1
   strict Ordered identity, W2 canonical Lockstep): SoA in ID order; pattern ordering loses the
   min-ID early exit (6,720 candidates per hit vs 2,096 in ID order and 2,322 today).
3. **Gate 0.4(b): borderline and conditional [E]; W3.2 (miss path) moves into W2 as a
   conservative scope choice.** For the chosen design (pipeline + SoA-pattern first-found) the
   projected admission share of worker CPU is 6.3% at 74M. At N = 1G:
   - with one-thread costs and a uniform alpha 0.44-0.82 it is 16.8-35.0%. It crosses 25%
     only at alpha >= 0.65, and at 0.65 it is 24.8-27.4% across cost samples and MRU k, on the
     line;
   - with the layout's own miss and reverse thinning exponents it is 13.9-22.5% for a hit
     exponent of 0-0.69;
   - with the contaminated 48/90-thread factors on admission alone it is 23.9-58.2%;
   - with 90-thread factors on both admission and native work (x3.75) it is 9.6-26.2%.

   The trigger depends on two inputs that are not measured yet: the growth exponent by class
   at scale (no hit exponent exists for this layout) and clean thread factors. Misses plus
   reverse scans are 56% of the projected admission CPU at 74M, hits 38%.
4. **Gate 0.4(c): not triggered [M].** Cheap tiers resolve 76.6-88.9% of real gen-7 requests
   (MRU k = 1..64), 55.3-72.0% on C-5F and 73.2-98.7% on the four-loop runs.
5. **The replay is exact [M].** Rebuilding today's index in commit order reproduces every
   admission outcome, maintenance charge and retirement count of six traced runs (four-loop
   FG/BMW/H/X W6, FG W1, C-5F W18; 26,748,231 admissions, 0 mismatches), so `idxreplay` can
   serve as the differential oracle for the W1.1 kernel.

## Consequences for W1-W5 [E unless stated]

- **W1.1 kernel** (legacy lane, min-ID, strict Ordered identity): SoA blocks in ID order; the
  >= 4x per-candidate gate is met by SoA-id in every measured condition (provisional: the
  timings are void under §7, and the margin is at least 1.3x).
- **W2.3 inspector resolution**: layers in finite-upper-pattern order for first-found. The
  canonical Lockstep mode either keeps an ID-ordered copy or accepts about 2.5x CPU per hit on
  controls (57.6 vs 23.3 µs at one thread [M, timing void under §7]); a per-block minimum-ID
  skip in pattern order is untested. The W2.3 first-found vs canonical gate licenses
  first-found in production.
- **W2 gains W3.2** as a conservative scope choice under the borderline 0.4(b) projection: a
  sublinear structure for forward misses and reverse (retirement) scans (per-group dominance
  pruning on sorted lower corners, or the 2N+6 feature trie). Re-decide with the M-adm
  exponents by class on P-IMP and a clean thread sweep. If they project <= 25%, W3.2 can return
  to W3. Launch criterion (B) needs the same projection.
- **Route jobs** are 79% of jobs in the gen-7 window and 84.7% of their requests reach the
  layers (one request per job, no MRU/Local benefit) [M]: Route micro-batching (§3.4) matters
  for admission cost as well as for dispatch.
- **Snapshot refresh (A6)**: request-weighted stale share is 1.5% at a lag of 1,024 new IDs,
  2.6% at 65,536 and 3.5% at 1M [M, gen-7 streams]; the edge-based figures agree.
- **Measurement**: no timing here meets §7. The 48/90-thread sweep ran once, with 48 foreign
  busy CPUs during its 1-thread passes and 26-32 during the 48/90-thread passes. A clean repeat
  is still open: <= 10% foreign load, >= 2 interleaved repeats, v4 LoadMon fields and
  `schedstat`. It is the only way to settle the SoA-pattern reverse ratio against 4x and the
  thread factors that 0.4(b) depends on.
