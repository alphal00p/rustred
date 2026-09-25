# Four-loop helper bounds: smaller input regions are not cheaper traversal

## Question and result

The user asks whether bounding auxiliary denominator powers can improve the
four-loop control, and whether removing the numerator bound as well is faster.
This experiment reuses saved IBPs. It measures successor-domain traversal and
scoped coverage, **not IBP generation or numerical integral evaluation**.

Bounding every helper by `A <= 19, R <= 12` still finishes all four controls,
but increases both traversal time and the number of discovered domains.
The existing selective helper strategy remains faster in these measurements.
Completely unrestricted helpers inspect much faster in all four diagnostics,
but leave unresolved dispatch conditions and do **not** close. Those diagnostic
times must not be presented as successful solve timings.

No Rust implementation, saved rule, master catalog, or live five-loop campaign
was changed. These are input-only experiments with the existing release binary.

## Matched setup

Every original physical query is retained verbatim, with the conservative
four-loop full-UV-jet envelope `A <= 19, R <= 12, A-R >= 7`. See
[the physical assumptions](../finite_starting_domains.md). Helpers precede
their owner's original query in every arm; owner order is unchanged.

The strategies are:

1. **Existing helpers:** FG/H/X have `R <= 12` and no denominator-power bound.
   BMW retains its previously successful selective policy: 37 helpers have
   `A <= 19`, and the other 97 have no denominator-power bound. All have `R <= 12`.
2. **All helpers A19/R12:** the only change is adding `A <= 19` to previously
   unbounded helpers. Helper coordinate uppers and difference bounds remain
   null; the aggregate positive-power bound supplies the finite restriction.
3. **Unrestricted:** remove both A and R bounds from every helper. Original
   physical queries remain in the input and are contained in the larger helpers.

The required physical scope is identical, but the additional auxiliary scope
is deliberately different. This compares strategies for covering the same
physical request, not identical sets of auxiliary obligations.

All runs use frozen release executable SHA-256
`32fdec098a57dd0c51aef71c01c260fb5cf7d0954b0992db08bb0f955aed358a`,
six total compute workers, shared Ordered/H256 scheduling, initial-band reuse,
unchanged saved owners and rules, and no joint-support pruning or subdivision.
The native shard supervisor runs one shared job, not independent owner shards.
Affinity is CPUs 64–69 for FG, 72–77 for BMW, and 80–85 for H/X. Families can
overlap on disjoint CPU sets; this is a shared host, not an exclusive-machine
benchmark. No filesystem cache flush, elapsed deadline, or cumulative work cap
is used. A 100 GB per-experiment RAM guard and host headroom protection remain
active; neither fires.

FG/BMW completed two runs per arm; existing/all-bounded comparisons use ABBA
order. H/X completed one run per existing/all-bounded arm and are supplemental
single-pair measurements, not repeated statistical estimates. The unrestricted
diagnostic was repeated twice for every family. Compilation and
post-run independent audits are outside every timed boundary.

## Successfully completed coverage

Times are medians for FG/BMW and individual observations for H/X. All eight
strategy/family combinations below finish with zero pending work, zero
frontiers/errors, and every initial root and discovered domain recursively closed.

| Family | Existing traversal (s) | All-A19 traversal (s) | Ratio | Existing domains | All-A19 domains |
| --- | ---: | ---: | ---: | ---: | ---: |
| FG | 12.039 | 63.281 | 5.26x | 98,643 | 427,851 |
| BMW | 31.399 | 151.112 | 4.81x | 157,999 | 777,087 |
| H, one pair | 8.872 | 139.596 | 15.73x | 24,346 | 894,290 |
| X, one pair | 29.358 | 115.425 | 3.93x | 46,865 | 634,524 |

The corresponding closed initial-root counts are FG 124/124, BMW 134/134,
H 314/314, and X 328/328. Counts include aliases; native inspections differ.
FG has 98,627 versus 406,724 native inspections, BMW 146,850 versus 712,321,
H 24,247 versus 838,162, and X 46,498 versus 588,556.

| Family | Whole command (s), existing / A19 | Child CPU (s), existing / A19 | Peak RSS (GB), existing / A19 |
| --- | ---: | ---: | ---: |
| FG | 17.076 / 73.767 | 43.495 / 197.137 | 1.167 / 4.392 |
| BMW | 38.210 / 166.764 | 104.266 / 425.921 | 1.860 / 7.833 |
| H, one pair | 20.345 / 161.009 | 39.039 / 411.863 | 0.681 / 9.160 |
| X, one pair | 39.715 / 134.942 | 91.257 / 343.675 | 1.319 / 7.114 |

Whole command includes supervisor startup, owner loading, traversal, checkpoint
and result output, packaging, and process shutdown. Native traversal is the
walker's reported boundary, including its own finalization. Child CPU is waited
user plus system time; RSS is the maximum single-process peak across the
repetitions, in decimal GB, not the sum of simultaneous processes. Large reports
and checkpoints contribute to the whole-command cost. H/X wall times have no
repeat-based uncertainty estimate.

## Why bounding helpers made this implementation slower

There are two relevant effects, not separately isolated by this experiment:

- **More fragmented obligations.** A broad helper can absorb many translated
  successor regions. A capped helper cannot absorb a child exceeding its cap;
  that child remains a legitimate obligation, often alongside many overlapping
  representations. A smaller starting set need not produce a smaller abstract
  worklist in this implementation.
- **Loss of the specialized rank-only reuse path.** `Domain::is_full_orthant`
  requires unconstrained power predicates. `InitialOrthants` therefore excludes
  helpers carrying A19, although ordinary containment still handles them.
  FG loses 1,470,992 pre-admitted-orthant hits, and BMW loses 2,022,660.
  Their general containment checks rise from 169,508,610 to 1,070,298,821 and
  from 651,480,397 to 3,061,473,914, respectively.

These counters establish extra work, not an exclusive causal attribution of all
wall-time differences to one lookup. An equally optimized bounded-helper lookup
was not implemented or benchmarked, and no optimal finite cap was sought.

The cap is not applied to descendants. Independent raw inspection finds a
closed FG successor at domain 124 with A20/rank11, BMW domain 134 with A20/rank11,
and an H A20 successor at domain 366. Scheduled rank also reaches 14 in FG and
13 in BMW. Discarding such children would invalidate the comparison.

## Completely unrestricted diagnostic: faster, but incomplete

The unrestricted arms reproduce the distinction in the
[earlier full-orthant falsifier](four_loop_saved_cover_control_2026-09-24.md).
They exhaust the queue, return native/supervisor exit 4, and preserve unresolved
frontiers rather than publishing successful coverage. No timer or resource
limit ends these runs. Times below are medians of two diagnostic runs for every
family; RSS is the maximum single-process peak across those two runs.

| Family | Traversal (s) | Whole command (s) | Child CPU (s) | Peak RSS (GB) | Guard frontiers | Recursively closed roots |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| FG | 1.649 | 5.118 | 8.079 | 0.204 | 85 | 60/124 |
| BMW | 2.172 | 6.550 | 10.447 | 0.327 | 138 | 46/134 |
| H | 4.260 | 10.609 | 19.920 | 0.475 | 258 | 90/314 |
| X | 6.305 | 17.656 | 30.631 | 0.925 | 556 | 10/328 |

FG's 85 frontiers occur on 13 locally blocked owners: 25 equality and
60 excluded-conjunction conditions. Although 111 owners finish local
classification, dependencies leave 64 of the 124 roots recursively unresolved.
BMW has 29 equality and 109 excluded-conjunction conditions on 32 locally
blocked owners; 88 roots remain recursively unresolved. H has 65 equality and
193 excluded-conjunction frontiers on 49 locally blocked owners, with 224 roots
recursively unresolved. X has 58 equality and 498 excluded-conjunction frontiers
on 159 locally blocked owners, with 318 roots recursively unresolved.
Only the initial 124/134/314/328
domains are inspected in these unrestricted controls. These are completed
diagnostic walks, **not completed reduction/coverage workloads**.

The frontend distinguishes a proved gap from an unresolved dispatch predicate.
These observations are unresolved dispatch conditions, not demonstrated missing
IBPs. The finite-axis classifier can exactly partition a bounded inactive
coordinate, deriving its upper from the rank budget and other-coordinate minima.
When both the coordinate upper and rank are unbounded, that split is unavailable;
unresolved coupled predicates remain frontiers rather than falling through to a
later rule. Thus removing R may simplify containment enormously while preventing
the current classifier from resolving exceptional geometry.

Every FG unrestricted frontier has an independently computed box maximum
`A-R <= 5`, outside the original `A-R >= 7` entry envelope. This does not license
dropping those frontiers: descendants are not clipped to the physical input.
The successfully completed rank-bounded controls provide the scoped coverage
evidence; the unrestricted diagnostic alone does not.

## Audit, evidence, and consequence for five loops

Separate implementation/input and result audits check unchanged physical query
objects and order, intended helper-only edits, fixed executable/policy, exact
initial-query containment mappings, all native/alias/partial-anchor records,
fully discharged ledgers, and final dependency-closure counters. Repeated
FG/BMW arms reproduce domain, inspection, edge, and native application counts.
The unrestricted arms are independently classified as incomplete, not timed
successful closure. No new mathematical certification is claimed.

Workspace evidence and reproduction scripts:

- `TMP/bounded-helper-control.SsjeHf/`: FG preparation, generic `run_control.py`,
  per-arm inputs/configurations, raw runs, timing receipts and audited summaries.
- `TMP/bounded-helpers-bmw.laTYbK/`: BMW inputs, runs, `comparison.json` and
  `unbounded-ar-comparison.json`.
- `TMP/bounded-helper-hx.rGD7DT/`: H/X preparation, preserved-input plans,
  configurations and raw runs.

For example, using the existing environment's Python and license, a fresh
workspace reproduction is:

```sh
python TMP/bounded-helper-control.SsjeHf/run_control.py \
  --config TMP/bounded-helper-control.SsjeHf/fg-a19.config.json \
  --directory TMP/bounded-helper-control.SsjeHf/fg-a19-new-repeat
```

The output directory must not exist. No rebuild or new IBP generation is needed.
The normal CLI beneath this measurement script is `rustred campaign shards`.
These local receipts are not pushed as solver inputs or reference material.

**Conclusion:** do not change the running five-loop campaign merely to cap all
helpers at the physical input bound. That change consistently worsens these
four-loop controls. Conversely, the very fast unrestricted diagnostics do not
justify removing all rank bounds: they fail to establish the requested coverage.
This is evidence about the current implementation and these input strategies,
not a proof that finite helpers are inherently inferior, a five-loop runtime
prediction, or a guarantee of five-loop termination. The four-loop controls have
no basis-changing Route jobs; the five-loop campaign's routing-heavy workload
remains an important difference.

## Follow-up: use the unrestricted output to select bounded repairs

A subsequent user-requested four-loop-only
[pilot-and-repair experiment](four_loop_unbounded_pilot_repair_2026-09-25.md)
retains unrestricted helpers only for recursively closed pilot owners, keeping
the baseline bounded helpers elsewhere. All original requests remain and a
fresh native pass revalidates everything. This input-only strategy improves
the directly timed FG whole workflow from 17.434 to 12.377 seconds, including
the fresh pilot and selection. It does not repay its pilot cost on BMW/H/X.
Thus the blanket helper changes above remain unsuccessful optimizations, but
data-selected mixed helpers have now demonstrated a selective benefit. No
five-loop experiment or production change is implied.
