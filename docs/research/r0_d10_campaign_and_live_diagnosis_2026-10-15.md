# Scalar D10 campaign and live five-loop workload diagnosis

Status: independent R0 campaign prepared for manual launch; read-only diagnosis
and research completed. No production campaign was started, stopped or modified,
and no new IBP rules or solver features were generated/implemented in this study.
All workspace paths below are under `/shared/codex-2/rustred`.

## Launch and incremental continuation

THE_ONE stopped cleanly at checkpoint generation98; its result is resumable.
The new R0 campaign has separate input/program copies, no inherited traversal
ledger, and uses the now-free CPUs64--95. The running R6 campaign uses96--127.
Both retain a requested600GB ceiling, the normal150GB host-available floor,
5% save-and-stop margin, hourly checkpoints, and disabled swap-growth stopping.
The effective RAM ceiling is recalculated at launch;600GB is not a reservation.

In the former THE_ONE tab:

```bash
cd /shared/codex-2/rustred
export RUSTRED_CAMPAIGN="$PWD/campaigns/five-loop-minimal-r0-d10-incremental"
export TMPDIR="$PWD/TMP" TMP="$PWD/TMP" TEMP="$PWD/TMP"
nix develop --command python -B examples/python/saved_campaign.py run \
  --campaign "$RUSTRED_CAMPAIGN"
```

This same command resumes automatically after a clean interruption. After
completion or an orderly stop, extend without discarding completed work:

```bash
nix develop --command python -B examples/python/saved_campaign.py extend \
  --campaign "$RUSTRED_CAMPAIGN" --rank 1
# Subsequently use --rank 2, then 3, ... through 6.
```

The D<=10 cap is preserved. All67 original topology templates, owner programs
and routes were retained from the start; currently empty intersections are not
deleted. Raising rank automatically activates them:

| Rank cap | Nonempty request rows /116 | Nonempty topology owners /67 |
|---|---:|---:|
|0|81|56|
|1|101|63|
|2|112|67|
|6|116|67|

At R1 the seven11-line classes first enter D10; at R2 the four12-line classes
first enter D10. Some separate D9 rows remain empty until a higher rank. Empty
rows may be discharged immediately; that is not evidence of a reduction in
their otherwise excluded topology. No higher starting rank is necessary for
consistent later extension.

The actual template union retains D=9 and D=10, not every abstract D<9 input.
It also retains the audited original-graph topology-specific bounds. At R0,
A=D<=10; scalar numerators do not mean unit denominator powers. The campaign
still includes all admitted dotted scalar inputs.

Inspect the last publication or watch live progress from another terminal:

```bash
nix develop --command python -B examples/python/saved_campaign.py inspect \
  --campaign "$RUSTRED_CAMPAIGN"
nix develop --command python -B examples/python/campaign_monitor.py "$RUSTRED_CAMPAIGN"
```

## Exact input counts, not runtime forecasts

Independent integer counting and the existing native `entry-domain-plan`
agree on the scalar counts. Per-axis limits were checked not to further narrow
the aggregate count. These are owner-qualified concrete index tuples before
additional index symmetries, not parametric rules or masters.

| Starting scope | Concrete starting tuples |
|---|---:|
| Old R0/D9 |637|
| New R0/D9--10 |2,557 =637 atD9 +1,920 atD10|
| R6/D9--10 |339,607,800|

The scalar request has498 connected-physical,687 convenience and1,372 factorized
tuples. The R6 request has28,377,392 connected-physical,79,533,514 convenience
and231,696,894 factorized tuples. No runtime attribution follows from these
counts. In particular, one symbolic region can cover many tuples cheaply,
whereas a single hard concrete input can generate a large exact reduction.

The previous successful scope was **D<=9, specifically the retained D9 band**,
not a campaign over D<9. Adding D10 admits different dotted inputs and activates
additional topologies already at R0. It is not merely one more algebraically
equivalent label on the old problem.

## What the ongoing R6 campaign is doing

Around09:14 CEST on15October, the bounded live snapshot recorded:

| Metric | Observation |
|---|---:|
| Elapsed |about13hours|
| Discovered symbolic domains |102.38million|
| Native local inspections |62.48million|
| Pending domains |16.19million|
| Recursively closed at last scan |3.568million|
| Original required requests closed |0/116|
| Frontiers |0|
| Largest encountered scalar rank bound |12|
| Process RSS / observed peak |159.7 /165.4GB|
| Lifetime average native CPU usage |2.80 of32cores|

The process is working, but the measured trend does not establish convergence.
The last two completed closure scans recorded16,748,214 new domains against
484,575 additional closed domains over2h23m09s: **34.56 discoveries per closure**.
The most recent scan took130.36s; the1% scan-duty policy postpones the next
eligibility by about3h37m. Therefore a flat closed counter between scans does
not mean zero closures, but the actual recorded scan-to-scan balance is also
unfavorable. No completion ETA is defensible from it.

The native checkpoint metadata records extensive sharing already:1.472billion
contained lookup hits,120.5million exact hits and about1.034billion dependency
edges at the sampled generation. These are counters, not cycle attribution.
More workers alone does not solve repeated generation of expensive obligations.

An exactly8MiB diagnostic sample from older sealed generations10/11 decoded
21,972 records. This is deliberately bounded and biased, not a random census.
Route made up75.4% of sampled native inspections, but only12.2% of their recorded
inspection time. Apply work dominated. One connected owner,
`011101110111000`, accounted for about44% of sampled inspection seconds.

One hot Apply record had an outer A<=16/R<=4/D12--16 envelope and an actual
inspected G2 residual at D12--13. It emitted12,647 edges (1,001 distinct), with
52,354 matcher cells. This owner's original request was only A<=13/R<=3/D10.
Such descendants may be legitimate rule/routing images, overcoverage, or both;
the sample does not distinguish them. They must not be clipped to input bounds.
Destination-owner class is not starting-root ancestry, so these records cannot
attribute the global work to the factorized requests.

## Recommended discriminating steps

### 1. Compare exact demand with symbolic overcoverage on a tiny real root

The R0 campaign is the immediate unchanged-engine experiment. Separately, the
existing experimental finite-replay v2 supports an exact complete first query
in a fresh diagnostic. The hot nine-line owner has only nine R0/D10 inputs and
one R0/D9 input: a suitable fixed pair for comparing concrete reachability with
symbolic traversal using the same rules and exact source conditions.

This existing option is **not** a production-ready switch for all116 roots,
resumable multi-stage operation, or adaptive residuals. It must not be enabled
in the prepared progressive campaign. Historical finite-replay results are
mixed: one260-seed control reduced traversal4.542s to2.711s, but whole-arm
times188.659s and188.242s were tied because preparation and cold checking
dominated. Another harder case exhausted a128million endpoint allowance.
No new finite-replay pilot was run in this study.

If small exact cohorts close cheaply while symbolic covers explode, the next
implementation is selective finite discharge with shared memoization and
checkpointed exact evidence—not replacing reusable IBPs with a giant table.
If the exact paths also explode, focus on the selected recurrences instead.

### 2. Retain routing information currently discarded

The current native routing implementation resets inactive target-coordinate
upper bounds and replaces upperD by upperA for a general affine numerator map.
Both are conservative choices, but existing verified-map incidence data permits
more precise images without introducing another CAS.

For Q_i=c_i+sum_j M_ij Q'_j and source numerator exponents r_i, existing subset
degree arithmetic bounds each destination exponent. Keep these per-coordinate
caps, including the degree cost of a newly pinched denominator. Also, D'=D+j
where j is the degree lost to constant choices. Only rows with c_i!=0 can
contribute to j. Here Cconstant is a conservative upper bound on the sum of
source exponents over every relevant row with nonzero constant. Thus
D'<=Dmax+Cconstant, intersected with the existing safe A' cap. If all relevant
constants vanish, D'=D exactly.

This is distinct from the previously tested joint-mask pruning, whose full
benefit was negligible/negative. A native-map shadow must first establish that
the tighter caps are nonredundant on actual hot routes. Exact small endpoint
tests must include affine constants, sign crossings and padding. Then measure
whole-cohort cost: extra fragmentation or lost reuse can outweigh tighter boxes.
This candidate has no new measured speedup yet.

### 3. Retain original-graph Taylor correlations; then repair real bottlenecks

The greatest scope-level opportunity is to preserve original propagator
multiplicities and derivative placements, not just A/R/D maxima. A quadratic
Taylor expansion adds at most two denominator powers to a given original
multiplicity vector, on routed external-momentum lines; numerator derivatives
compete for the same Taylor degree. The current boxes forget these correlations.
A universal profile can enumerate all allowed renormalizable graph/attachment
patterns, rather than the diagrams of one specific amplitude. It needs its own
explicit physical-coverage argument and may not silently narrow the116requests.

If actual exact paths remain costly, seek small boundary blocks or alternative
local IBPs before forcing numerator-heavy canonical routing. TIDE, Kira3 and
Blade motivate this distinction between requested targets, seed halos and
expensive auxiliary work. Their optimizations are discovery strategies, not
automatic RustRed closure proofs. Detailed primary references, prior negative
experiments and falsifiers are in
[the companion research study](r6_d10_physics_reduction_strategy_2026-10-15.md).

No global dimensional-shift expansion, indiscriminate larger Laporta halo,
blind ordering sweep, or source-rank clipping is recommended from the present
evidence. Existing finite source replay, exact Symbolica arithmetic and native
routing services should be reused.

## Delivery evidence and limits

Prepared query SHA256:
`5b3be93a905f99ddf0e7fa7b9bc6cb6433a92a5879adea66907acbcdaa96d99f`.
Original source-template SHA256:
`42a0c62771b6e7c53cc937d46ad9505e33c282db31a8d6f64846ecbca749ef64`.
Solver814745116dd52749a6eb2272774503afc762d806823924cf4c35b8e732745ce7 and
publisherfe3f1d9167e931d9179240a61202f0c6340252b93aae9a484133d0cfe1a46050
are copied locally; saved payloads match the R6 campaign. The exact Nix launch
wrapper passed `--dry-run`. Existing native CP6 rank-extension/resume tests
were already validated; this delivery changes no solver implementation.

Local evidence:

- `TMP/r0-d10-preparation-20261015/`;
- `TMP/r0-d10-count-audit-20261015/`;
- `TMP/r0-d10-independent-audit-20261015.md`;
- `TMP/r6d10-deepdive-20261015/`;
- `TMP/r6d10-deepdive-20261015-current.md`.

No new scoped closure, runtime win, production restart recommendation or
unrestricted family-completeness claim follows from this research alone.
