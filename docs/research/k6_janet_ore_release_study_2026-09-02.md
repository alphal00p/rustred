# K6 Janet/Ore bounded release study — 2026-09-02

Status: active stopping-milestone run record. This note records exact release
diagnostics of RustRed's proposal-only Janet/Ore seed engine on every full-rank
sector orbit of the three-loop `K = 6` family. It does **not** claim a closing
K6 artifact, compiler closure, guard-comprehensive closure, or Vakint
three-loop parity.

The mathematical design and its authority boundary are described in
[Certificate-first parametric-IBP closure](blind_domain_janet_closure_2026.md)
and the [Janet/Ore proposal integration seam](janet_ore_integration_seam_2026.md).

## Scope and interpretation

Here `K = 6` is the six-index complete scalar-product family at three loops.
The release harness covers the six reviewed full-rank sector representatives:

| Orbit | Representative | Active lines |
|---:|---|---:|
| 0 | `[0,0,1,0,1,1]` | 3 |
| 1 | `[0,0,1,1,0,1]` | 3 |
| 2 | `[0,0,1,1,1,1]` | 4 |
| 3 | `[0,1,1,1,1,0]` | 4 |
| 4 | `[0,1,1,1,1,1]` | 5 |
| 5 | `[1,1,1,1,1,1]` | 6 |

The diagnostic performs exact chart lift, deterministic same-head
preprocessing, autoreduction, and Janet nonmultiplicative completion. Only
after the Janet queue is exhausted does it compute the standard complement,
pure-power coverage, and requested-support proposal. Therefore:

- `JanetQueueExhaustedProposalOnly` means that complement reporting was
  reached, but is still not an executable-rule or artifact certificate;
- a typed resource stop during completion means `complement=not_reached` and
  `rays=not_reached`; and
- a resource stop must never be reported as a number of rays left, a
  positive-dimensional complement, or an integrand declared to be a master.

## Reproducibility envelope

The milestone began at RustRed revision
`b519e4bb94d11a780e73e9f2532771255834736c` on `main`; the Janet/Ore changes
measured here were still an audited working-tree milestone when the runs
started. The release test executable was built once with

```console
nix develop --command cargo test --release -p rustred --no-run
```

and then invoked directly so timings exclude compilation. One representative
command is:

```console
RUSTRED_K6_JANET_DIAGNOSTIC_TIER=study \
  /run/current-system/sw/bin/time \
  -f 'MEASURE wall=%e user=%U sys=%S max_rss_kib=%M' \
  target/release/deps/rustred-8820c92ce82dd231 \
  --ignored --exact \
  foundry::campaign::involutive_seed::tests::diagnostic_release_k6_orbit_three_bounded \
  --nocapture
```

The exact executable hash suffix may change after a rebuild. The printed
`k6-involutive-seed-envelope` is the authoritative effective limit profile;
individual `RUSTRED_K6_JANET_*` overrides are parsed before algebra starts.
Every final study run is serial because the available Symbolica license admits
one process. Concurrent attempts are not timing or algebra evidence.

## Historical pre-Janet baseline

The pre-Janet autonomous release campaign is preserved here only as geometric
motivation. At the milestone's starting revision above, it ran for about 2 h
59 min and did not publish an artifact. After 190 exact owners and the 4,096
requested-task ceiling, the first two sector orbits still had respectively 59
and 58 unbounded complement boxes.

Those are **pre-Janet blind boxes**, not post-Janet rays. They establish that
the earlier translated-source walk did not close even the low-line dotted and
numerator lattices; they say nothing about how many standard pairs survive a
completed Janet basis.

## Exact ingress correction

Orbit three originally exposed two ordinary-source rows with the same leading
shift `[1,1,1,1,1,1]`. The implemented deterministic equal-head Ore
preprocessor now retains nine distinct nonzero heads from the nine inputs: one
elimination produces the new lower head `[0,2,1,1,0,2]`.

Exact lift plus preprocessing took 0.002095 s, of which preprocessing took
0.000194 s. The census was:

| Metric | Value |
|---|---:|
| Input / retained rows | 9 / 9 |
| Equal-head eliminations | 1 |
| Zero / nonzero remainders | 0 / 1 |
| Cascading collisions | 0 |
| Maximum collision chain / head class | 1 / 2 |
| Sort comparisons / payload visits | 25 / 137,050 |
| Pivot comparisons / coordinate visits / moves | 23 / 138 / 26 |
| Normal-form steps / divisor visits | 1 / 1 |
| Exact coefficient operations | 30 |

Thus duplicate-head ingress has been cured and is not the reason the bounded
full-orbit calculations below stop.

## Bounded progression before the final serial study

These measurements establish real completion progress but intentionally use
resource envelopes too small to establish queue exhaustion.

At the `x4` divisor envelope, orbit three stopped at the exact
1,048,577/1,048,576 divisor-visit boundary during autoreduction. It had reached
35 basis rows at revision 36 after 228 completion iterations, 924 normal-form
steps, 37 autoreduction passes, and 120,950 exact coefficient operations. Wall
time was 0.22 s and maximum RSS was 6,144 KiB.

At the `x16` envelope, all six orbits reached the exact
257/256 completion-iteration boundary before complement construction:

| Orbit | Basis rows | Revision | NF steps | Divisor visits | Autoreduction passes | Exact coefficient operations | Wall (s) | Max RSS (KiB) |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | 31 | 32 | 865 | 661,310 | 33 | 85,286 | 0.10 | 3,072 |
| 1 | 29 | 29 | 665 | 392,917 | 30 | 63,807 | 0.08 | 3,132 |
| 2 | 32 | 31 | 764 | 637,405 | 32 | 96,265 | 0.11 | 3,080 |
| 3 | 37 | 40 | 1,077 | 1,408,449 | 41 | 151,139 | 0.27 | 6,144 |
| 4 | 34 | 40 | 994 | 1,164,545 | 41 | 135,535 | 0.19 | 3,072 |
| 5 | 31 | 37 | 1,352 | 1,615,645 | 38 | 199,403 | 0.42 | 6,156 |

Basis growth from nine ordinary rows to 29–37 rows within 256 obligations is
evidence that Janet prolongation supplies new leading coverage. It is not yet
evidence that the queue terminates or that the eventual complement is finite.

## Preliminary `study`-tier stops

These exploratory serial observations motivated widening the coefficient and
exact-algebra limits before the final six-orbit matrix. They remain useful as
the first measured failure surface, but the final table below supersedes them.

| Orbit | Stop | Checkpoint | Work at checkpoint | Wall (s) | Max RSS (KiB) |
|---:|---|---|---|---:|---:|
| 0 | Exact-addition numerator needed 5,089,930 terms; limit 4,000,000 | Old binary did not classify this algebra resource stop into the diagnostic checkpoint path | Not retained | 27.29 | 156,684 |
| 1 | Exact-addition numerator needed 5,127,831 terms; limit 4,000,000 | Not retained; observed as the sole surviving process of a rejected concurrent batch and therefore excluded from the final serial matrix | Not retained | 9.25 | 155,596 |
| 2 | Consequence coefficient terms 307,992; limit 262,144 | Completion autoreduction, basis 80, revision 102, pass 1 | 3,068 iterations; 23,434 NF steps; 64,604,699 divisor visits; 102 passes; 5,548,794 exact operations | 23.01 | 116,748 |
| 3 | Consequence coefficient terms 388,928; limit 262,144 | Completion, basis 54, revision 73 | 672 iterations; 4,069 NF steps; 10,555,497 divisor visits; 74 passes; 1,031,813 exact operations | 8.42 | 138,240 |
| 4 | Divisor visits 67,108,865; limit 67,108,864 | Completion, basis 71, revision 87 | 2,485 iterations; 21,468 NF steps; 88 passes; 3,510,170 exact operations | 8.96 | 27,660 |
| 5 | No valid serial result retained at this stage | — | — | — | — |

The attempted concurrent `study` batch is deliberately not included as a
measurement: four processes aborted when Symbolica rejected concurrent
licensed instances. Orbit one's observation is retained only as a provisional
failure-surface clue and must not be used for a cross-orbit timing comparison.
All final runs below are serial.

## Final serial pre-monic six-orbit study

This is the authoritative pre-monic baseline from serial runs of the same
release executable, `rustred-8820c92ce82dd231`. A row is complete only when it
identifies the effective profile, elapsed resources, terminal outcome, and
whether the Janet queue and geometric complement were actually reached.

<!-- BEGIN FINAL SERIAL PRE-MONIC RESULTS -->

| Orbit | Effective tier / overrides | Wall (s) | Max RSS (KiB) | Terminal outcome | Basis rows / revision | Iterations / NF steps / divisor visits / exact operations | Janet queue exhausted? | Complement / rays |
|---:|---|---|---:|---|---|---|---|---|
| 0 | `study` | 189.19 | 866,196 | Exact-add numerator terms 16,794,040 / 16,777,216; completion autoreduction pass 1 | 91 / 117 | 2,720 / 14,915 / 65,150,033 / 3,590,266 | No | Not reached |
| 1 | `study` | 20.46 | 379,856 | Exact-add numerator terms 17,712,632 / 16,777,216; completion | 79 / 102 | 2,075 / 10,055 / 31,025,874 / 1,610,316 | No | Not reached |
| 2 | `study` | 35.79 | 890,940 | Divisor visits 67,108,865 / 67,108,864; completion | 81 / 105 | 3,177 / 23,933 / 67,108,864 / 5,833,434 | No | Not reached |
| 3 | `study` | 46.97 | 265,960 | Exact-add numerator terms 18,368,520 / 16,777,216; completion | 63 / 86 | 861 / 5,902 / 18,658,150 / 2,315,852 | No | Not reached |
| 3, raised exact cap | `study`; exact polynomial terms 67,108,864 | 436.34 | 448,264 | Exact-add numerator terms 94,136,628 / 67,108,864; completion | 67 / 91 | 1,063 / 8,515 / 31,671,849 / 4,272,627 | No | Not reached |
| 4 | `study` | 8.78 | 27,664 | Divisor visits 67,108,865 / 67,108,864; completion | 71 / 87 | 2,485 / 21,468 / 67,108,864 / 3,510,170 | No | Not reached |
| 5 | `study` | 41.76 | 73,756 | Divisor visits 67,108,865 / 67,108,864; completion | 59 / 87 | 1,495 / 17,734 / 67,108,864 / 5,229,498 | No | Not reached |

<!-- END FINAL SERIAL PRE-MONIC RESULTS -->

The Janet basis grew from nine input rows to 59–91 rows and processed 861–3,177
completion iterations, so this implementation rescues substantial leading
coverage that the ordinary-source heads alone do not provide. Nevertheless,
none of the six queues exhausted. There is consequently no legitimate
post-Janet complement or residual-ray count from this matrix.

The raised-cap orbit-three run is especially diagnostic. Increasing the exact
polynomial-term allowance fourfold advanced the basis from 63 rows at revision
86 to 67 rows at revision 91, but the next exact addition requested 94,136,628
terms and wall time increased from 46.97 s to 436.34 s. A larger cap alone is
therefore not a plausible cure for exact coefficient swell.

## Post-monic serial follow-up

The controlled follow-up normalizes exact basis rows to monic form before
repeating the same serial matrix. It uses a rebuilt release executable for the
same test target and the same `study` profile; only the row-normalization
invariant changes. Cargo retained the same executable filename suffix across
the two builds, so that suffix must not be mistaken for a content digest.

<!-- BEGIN POST-MONIC SERIAL RESULTS -->

| Orbit | Effective tier / overrides | Wall (s) | Max RSS (KiB) | Terminal outcome | Basis rows / revision | Iterations / NF steps / divisor visits / exact operations | Janet queue exhausted? | Complement / rays |
|---:|---|---:|---:|---|---|---|---|---|
| 0 | `study` | 203.50 | 566,896 | Exact-add numerator terms 25,224,779 / 16,777,216; completion autoreduction pass 1 | 90 / 115 | 2,696 / 14,811 / 63,667,405 / 3,507,354 | No | Not reached |
| 1 | `study` | 20.80 | 374,812 | Exact-add numerator terms 18,041,439 / 16,777,216; completion | 79 / 102 | 2,075 / 10,055 / 31,025,874 / 1,597,606 | No | Not reached |
| 2 | `study` | 37.23 | 992,284 | Divisor visits 67,108,865 / 67,108,864; completion | 81 / 105 | 3,177 / 23,933 / 67,108,864 / 5,801,092 | No | Not reached |
| 3 | `study` | 54.05 | 295,164 | Exact-add numerator terms 17,967,396 / 16,777,216; completion | 63 / 86 | 861 / 5,902 / 18,658,150 / 2,319,683 | No | Not reached |
| 4 | `study` | 9.72 | 27,656 | Divisor visits 67,108,865 / 67,108,864; completion | 71 / 87 | 2,485 / 21,468 / 67,108,864 / 3,477,257 | No | Not reached |
| 5 | `study` | 38.32 | 70,660 | Divisor visits 67,108,865 / 67,108,864; completion | 59 / 87 | 1,495 / 17,734 / 67,108,864 / 5,215,584 | No | Not reached |

<!-- END POST-MONIC SERIAL RESULTS -->

For orbits one through five, the leading structural trajectory is unchanged:
basis size and revision, completion iterations, normal-form steps, divisor
visits, and autoreduction passes exactly match the pre-monic run. Exact
operation counts and wall/RSS measurements move slightly, but no stop changes
class and no queue exhausts. Orbit zero encounters a different projected exact
addition slightly earlier—basis 90/revision 115 instead of 91/117—but is not
materially closer to completion.

The A/B result is therefore negative as a performance intervention. Monic row
normalization remains a sound and useful canonical invariant, but it does not
cure K6 coefficient swell or flat divisor-search amplification.

## Measured diagnosis

The evidence so far rules out the original duplicate-head ingress failure and
does not indicate an autoreduction cycle: accepted normal-form leaders descend
strictly under the frozen order. The dominant measured problems are instead:

1. **Exact coefficient swell.** Pre-monic transient numerator additions needed
   16.8–18.4 million polynomial terms at the default study cap, and 94.1
   million after raising orbit three's cap fourfold. The monic follow-up still
   needed 18.0–25.2 million terms. Raising a cap allows progress but worsens
   cost without changing the growth mechanism.
2. **Divisor-scan amplification.** Some orbits perform tens of millions of
   term-by-basis divisor tests before a few thousand completion obligations
   have been handled. The flat divisor scan magnifies every growing basis and
   row.
3. **Whole-epoch row movement compounds the scan.** Rebuilding and moving
   complete exact basis payloads around small leader changes adds avoidable
   copying and retained-memory pressure to the arithmetic bottleneck.
4. **Monic normalization alone is neutral.** Five of six structural paths are
   identical and orbit zero stops slightly earlier; canonical scaling does not
   reduce the numerator supports that dominate this implementation.
5. **Order sensitivity remains unmeasured at adequate depth.** The current
   frozen order produces useful new leaders, but no completed cross-order
   tournament yet establishes that it minimizes basis or coefficient growth.
6. **Guard-comprehensive closure remains downstream.** Even queue exhaustion
   in this generic localization must be followed by finite-complement and
   exceptional-guard-branch checks before executable artifact publication.

Nothing in the bounded stops shows that a standard ray “escaped Janet”: the
standard complement is intentionally not constructed before the queue is
empty. If the final matrix never reaches queue exhaustion, the honest result
is a resource obstruction, not a geometric ray census.

## Ranked cure programme

The measurements rank the next implementation slices as follows:

1. **Janet divisor index and incremental epochs.** Replace the full
   term-by-basis scan with an indexed Janet-divisor lookup. Store unchanged
   basis rows behind shared immutable ownership and update epochs with
   copy-on-write or equivalent structural sharing, eliminating repeated whole
   payload moves.
2. **Support-first modular scheduling with selective exact replay.** Compute
   candidate leaders, masks, obligations, and pivot schedules over several
   deterministic finite-field specializations using Symbolica's optimized
   primitives. Retain only stable modular support, reconstruct or replay exact
   source combinations only for rows that add leading coverage, and admit them
   solely through the existing exact provenance, guard, and strict-descent
   boundary. A modular zero or leader is never publication authority.
3. **Fraction-free or common-denominator primitive rows.** Avoid repeated
   rational numerator cross-products by delaying normalization and extracting
   safe common content at controlled points. This must use Symbolica's public
   exact GCD and exact-division primitives and preserve guards and provenance;
   RustRed must not grow a duplicate computer-algebra subsystem.
4. **Bounded order portfolio.** Screen admissible variable/block orders on
   shallow modular runs and select by worst-orbit complement progress,
   coefficient swell, fill, and divisor work before restarting one exact proof
   run.
5. **Safe criteria with exact witnesses.** Add involutive/signature or sugar
   criteria only where skipped obligations carry exact ancestor or syzygy
   witnesses that can be replayed. Heuristic support coincidence may prioritize
   work but cannot suppress a mandatory completion obligation.

These are implementation hypotheses, not closure claims. Their success gate is
unchanged: deterministic Janet queue exhaustion, finite pure-power coverage on
every live guard branch, exact regenerated-source replay, and eventual cold
artifact compilation with no uncovered positive-dimensional branch.

## Indexed natural-order raised-visit follow-up — 2026-09-02

Two further serial release runs isolate the immutable Janet-divisor index under
the natural variable order. The historical logical divisor-visit envelope was
raised so it would remain a compatibility census rather than the first stop.
Those logical visits reproduce how many basis entries the removed flat scan
would have inspected; they are not physical index probes. The index's own build
and query-operation counters record the work actually performed by lookup.

| Orbit | Wall (s) | Max RSS (KiB) | Terminal exact-add projection | Basis rows / revision | Iterations / NF steps | Historical logical visits | Index build / query operations | Exact operations | Janet queue exhausted? | Complement / rays |
|---:|---:|---:|---|---|---|---:|---:|---:|---|---|
| 4 | 206.83 | 789,516 | 17,105,235 / 16,777,216 | 84 / 110 | 3,677 / 37,310 | 165,049,549 | 268,522 / 114,892,390 | 8,221,428 | No | Not reached |
| 5 | 2,808.29 | 1,720,032 | 20,845,452 / 16,777,216 | 110 / 182 | 4,753 / 103,514 | 988,486,132 | 602,063 / 578,403,529 | 65,159,239 | No | Not reached |

Both runs stopped at the exact-addition resource preflight before exhausting
the Janet queue. Consequently neither run reached complement construction,
reported a residual-ray count, or produced closure evidence. The requested
term counts are conservative preflight projections, not measured canonical
Symbolica output sizes or peak scratch allocations: native rational-polynomial
addition can reduce denominator cross-products by GCD before materializing the
sum. The exact interpretation and the proposed GCD-aware/fraction-free remedy
are documented in the
[modular scheduling and fraction-free exact replay design](k6_janet_modular_fraction_free_design_2026-09-02.md).

The index removes the growing term-by-basis scan from the wall-time path, but
these deeper runs show that lookup alone is insufficient. Orbit five reaches
almost one billion historical scan-equivalent visits while performing about
578 million bounded index operations, yet its 46.8-minute wall time, 1.64 GiB
maximum RSS, 65.2 million exact-operation census, and 183 complete epoch
generations expose exact coefficient arithmetic and repeated epoch work as the
remaining dominant obstruction. These measurements predate the subsequent
copy-on-write autoreduction slice, so unchanged rows were still rematerialized
and successor metadata was still rebuilt wholesale. The ownership and
incremental-rebuild separation is analyzed in the
[incremental-epoch and copy-on-write audit](k6_janet_incremental_epoch_audit_2026-09-02.md).

The code milestone following these measurements implements indexed
copy-on-write autoreduction and a GCD-aware fallback for conservative exact
sum projections. The next grounded comparison is a release run of that
combined implementation under the same natural order and limits, followed by
modular-guided or fraction-free replay if exact work remains dominant. Until
one of those runs actually exhausts the queue and constructs the complement,
there is no K6 closure claim.

## Indexed copy-on-write and GCD-aware natural-order follow-up — 2026-09-02

A clean detached worktree at RustRed commit `86df172` was compiled in release
mode, and the resulting library-test executable was copied before any
concurrent source work could replace it. Its SHA-256 was
`d866f1785938691a1bce70f62787b7fa8b999b7114537305074c94bbcdc1bc4f`.
The five-line orbit 4 diagnostic then ran serially under the natural order,
the `study` profile, and a raised one-billion historical logical-visit cap.
Compilation is excluded from the measurement.

| Wall (s) | User / system (s) | Max RSS (KiB) | Typed stop | Basis rows / revision | Iterations / NF steps | Historical logical visits | Index build / query operations | Exact operations | Shared / materialized autoreduction rows |
|---:|---:|---:|---|---|---|---:|---:|---:|---:|
| 794.26 | 781.84 / 7.11 | 1,770,948 | consequence coefficient exponent cells 57,804,810 / 50,331,648 | 88 / 118 | 4,097 / 44,168 | 220,676,435 | 308,096 / 149,329,259 | 9,990,032 | 5,589 / 286 |

The denominator-GCD fallback therefore admitted the earlier rejected
17,105,235-term cross-sum projection and advanced the exact trajectory by four
basis rows and eight revisions. Copy-on-write shared 95.1% of scanned
autoreduction rows. Nevertheless, wall time increased by a factor of 3.84 and
peak RSS by 2.24 while exact-operation count increased by only 21.5%, which
confirms that the admitted coefficient arithmetic is entering a superlinear
support-growth regime.

The new stop also exposed a diagnostic-envelope error rather than an
independent exponent pathology. A K6 coefficient uses seven polynomial
variables—`d` and six indices—and Symbolica stores one dense exponent vector
per monomial. The request is therefore exactly 8,257,830 polynomial terms,
still below the intended 8,388,608-term limit. The cell cap had incorrectly
been set to six rather than seven times that term limit. Consistent K6 study
caps are 58,720,256 consequence cells, 234,881,024 basis cells, and 29,360,128
guard cells. Correcting those values is justified for the next diagnostic but
does not cure the measured coefficient swell. The queue again did not exhaust;
the complement and residual-ray census were not reached.

The same immutable executable then ran orbit 4 with the autonomously selected
coordinate priority `5,3,4,2,0,1` and otherwise identical limits. It followed
a markedly different trajectory:

| Wall (s) | User / system (s) | Max RSS (KiB) | Typed stop | Basis rows / revision | Iterations / NF steps | Historical logical visits | Index build / query operations | Exact operations | Shared / materialized autoreduction rows |
|---:|---:|---:|---|---|---|---:|---:|---:|---:|
| 2,489.84 | 2,464.23 / 6.99 | 909,372 | logical divisor visits 1,000,000,067 / 1,000,000,000 | 100 / 141 | 5,232 / 139,945 | 999,999,967 at the last checkpoint | 412,382 / 678,519,600 | 57,112,322 | 7,239 / 494 |

This order avoided the natural trajectory's coefficient-payload stop, reached
12 more basis rows and 23 more revisions, and used about half its peak memory.
It also needed 3.13 times the wall time, 5.72 times the exact-operation census,
and 4.53 times the historical logical divisor work before reaching the raised
visit cap. The result proves that ordering is a first-class performance input;
it does not identify a closing order. Raising the visit cap again would make a
multi-hour exact run plausible without changing the growth mechanism. The
next scalable comparison therefore needs bounded whole-trace modular
scheduling and exact replay, not another blind cap escalation. This run also
did not exhaust the Janet queue, so complement and residual rays remain not
reached.

## Attributed payload follow-up — 2026-09-03

A later frozen release test executable, SHA-256
`8700d10972a82bec98812d3ccf326879c1c0b5992405f5fee2154bbc9bf1be7d`,
repeated natural-order orbit 4 with the corrected seven-variable cell ratio,
a raised one-billion historical logical-visit cap, and a deliberate
1,048,576-term per-consequence stop. The test-only diagnostic observer was
independently audited before the run: ordinary attempts add no heap allocation
or second polynomial traversal, and exact denominator-reuse inspection is
claimed only once for the first failing payload.

| Wall (s) | User / system (s) | Max RSS (KiB) | Basis / revision | Attempts / NF steps | Logical visits | Index queries | Exact operations |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 101.33 | 100.21 / 0.40 | 433,156 | 83 / 108 | 3,579 / 35,720 | 152,382,806 | 106,777,948 | 6,687,228 |

The first rejected normal-form cancellation retained 802 rational
coefficients and 1,826,367 numerator-plus-denominator terms:

| Component | Coefficients | Numerator terms | Denominator terms | Approximate retained payload |
|---|---:|---:|---:|---:|
| physical row | 187 | 649,437 | 43,335 | 31.9 MiB |
| source provenance | 615 | 1,007,770 | 125,825 | 49.7 MiB |

The largest single coefficient contained 21,569 terms. In the bounded exact
denominator sample, 131 of 256 tracked nonunit instances matched an earlier
representative and 125 were distinct; 545 further instances were deliberately
outside the exact-tracking budget.

This separates the earlier aggregate stop: expanded provenance is the largest
component, physical-row numerators are also substantial, and denominators are
secondary. Fraction-free denominator control remains a useful comparison but
cannot be the primary cure. The next primary falsifier is therefore a
persistent exact-expression DAG whose support is certified by rigorous
one-sided finite-field nonzero witnesses and exact Symbolica fallback for
sampled zeros. Its independent requirements and go/no-go gates are recorded
in the
[exact-lazy audit](k6_exact_lazy_support_certificate_audit_2026-09-03.md).

This bounded run did not exhaust the queue and did not construct a complement
or K6 artifact.
