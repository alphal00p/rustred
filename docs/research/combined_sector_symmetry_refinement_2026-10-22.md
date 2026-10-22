# Combined full-integral sector-symmetry refinement

This extends the P102 numerator-symmetry work in one optional refinement provider,
not a second campaign. Production campaigns remain unchanged. All operations are
exact at symbolic dimension; remaining terminals are not claimed independent.

## What changed

The new `TerminalSectorSymmetryEquations` service discovers signed denominator
permutations from the exact line-space projector `Q (Q^T Q)^(-1) Q^T`. Symbolica's
graph canonicalization proposes automorphisms and cross-support maps; existing
native momentum verification checks the actual denominators, masses and unit
Jacobian. Dotted powers move with their lines. A graph match alone never proves an
integral identity. Cross-family witnesses retain their source/target families.

The existing native transporter acts on the entire numerator polynomial,
including affine mass constants. Each original integral is routed to a fixed
support representative. Two explicit runtime source policies are available:

- `generators` (application default): exact `I - T_r(I)` and `I - T_(r g)(I)`
  rows for the canonical route `r` and every discovered representative generator
  `g`. This avoids enumerating their products and makes no full-group span claim.
- `polynomial-orbits`: average distinct whole-polynomial images, producing
  `I - mean(orbit(I)) = 0`. This retains full orbit closure but can be expensive.

Expanded monomials are auxiliary columns, not recursively seeded new symmetry
jobs. Native v3 certificates bind the policy and replay every map, input
inventory and selected source equation. Only orbit mode claims and checks full
orbit reachability/closure. Existing Rust orbit APIs remain available; explicit
policy selection is used by the application and comparative clients.

The established P102/weak equations are also prepared once on the original keys
and embedded in the same provider. This matters because a finite saved IBP
application need not commute with averaging; retaining the old equations can
expose additional consequences. Both cohorts and exact normalization equations
are coalesced and reduced to an independent row span before saved application.
There is no duplicate full refinement run.

Existing inherited reductions are composed first. A shared guard envelope avoids
copying the large inherited condition union per equation. Unresolved offspring
use the existing routed saved-rule reducer, including its factorization/cache
services. Failure retains the complete exact source equation and an explicit
incomplete-projection receipt, never an invented master. A cancelled projection
retains only completed rows and their immutable native task binding.

The old normalization convention is not silently replaced. The new provider
adds exact `I - N(I)` equations from caller/RAM-controlled normalization while
preserving the source session and its published master identities. Redundant keys
become exact dependencies. No ordinary IBP generation, numerical evaluation,
dimensional-shift engine or independent CAS is added.

## Why whole polynomials matter

The first implementation recursively treated every expanded endpoint monomial
as another symmetry source. It was finite in principle but impractical on the
current five-loop set: an isolated preparation pilot reached 998,662 discovered
keys after processing 4,637, consuming 32.04 GB in 81.48 seconds. The pilot's
explicit diagnostic RAM grant stopped that owned process; no complete plan or
rank result was produced. This negative result is preserved.

For one finite group, averaging distinct images equals averaging all group
elements: all orbit points have equally sized stabilizer cosets. Its Reynolds
projector is idempotent. Equations on original wanted vectors recover that group's
relations among them without constructing the whole ambient monomial basis.
Full-rank support is required for this finite-orbit path; valid single maps on a
rank-deficient free-loop support do not imply a finite group.

This is not a claim of saturation under every pinch group plus all IBPs. Source
preparation, saved projection, and combined working-set completion have separate
receipts. New unresolved saved leaves remain visible rather than being declared
closed. Unequal masses/powers and nonunit Jacobians have rejection controls.

The corrected full-orbit five-loop preparation does finish in 500.562 seconds:
939 sources, 319 verified generators, 1,800 combined equations (including 961
legacy rows), and 6,776,578 output terms. Its inventory has 3,973,249 columns;
the largest orbit has 720 images and 16,570,026 transient expanded terms. The
source capsule is 120,652,209 bytes. Four early rank-at-least-eight sources
dominate the cost. Cold replay and exact joining are separate boundaries; this
preparation result alone establishes no new independent rank. It motivates the
generator policy rather than an arbitrary rank/count cutoff.

## Matched current five-loop normalization result

Both arms use the same immutable 939 raw keys, 862 retained basis, original maps
and exact dimension conditions. No ordinary source work is consumed.

| Existing normalizer recipe | New equations | Preparation | Final retained basis |
|---|---:|---:|---:|
| Conservative source recipe | 0 | 1.615 s | 862 |
| Caller/RAM-controlled resources | 211 | 9.842 s | 693 |

The second arm finds 197 unit aliases and 14 numerator projections; its 728
normalizer outputs are not the final independent count. The exact session join
adds 169 independent dependencies. All 939 inherited maps compose exactly, all
136,725 current guards remain in the same ordered domain, and native normalizer
and final-session cold replay pass. The full cold client takes 23.016 seconds and
810.6 MB peak RSS. The join itself takes 0.00160 seconds.

This component's gain comes from **existing normalization**, not the broader new
symmetry search measured below. The historical 829-key studies
use a different capsule and are not substituted for this matched result.

Evidence (local, deliberately not committed):
`TMP/sector-symmetry-acceptance-20261022/normalization-census/summary.json`,
its `conservative`/`ram` process receipts, and `checks/normalization-*`.
Source hashes were checked unchanged. Compilation is excluded from every solver
timing. Final broader-provider integration measurements are recorded separately.

Matched lower-loop checks also completed. Both conservative and RAM-controlled
normalization already expose the same aliases there, so neither adds rank to
the saved baselines. The broader v2 pure-symmetry equations likewise add none:

| Frozen control | Raw keys | Basis before | Basis after normalization / pure symmetry |
|---|---:|---:|---:|
| K6 (three loops) | 38 | 5 | 5 / 5 |
| H | 386 | 22 | 22 / 22 |
| FG | 145 | 16 | 16 / 16 |
| BMW | 179 | 17 | 17 / 17 |
| X | 445 | 19 | 19 / 19 |

All 1,193 raw maps compose exactly, with unchanged guard domains, zero ordinary
source work, and cold normalizer/certificate/final-session checks. These are
individual fixed native baselines, not a new combined four-loop master census.
The five-loop 169-relation gain must not be transferred to them. Evidence:
`TMP/sector-symmetry-acceptance-20261022/v2-lower` and the matched lower
normalization census. Overlapping owned work is recorded in process receipts;
the lower controls are correctness checks, not a throughput speedup claim.

Applying the EXISTING saved programs to the new complete rows gives further
lower-loop relations on the same original inventories (no R1/R2 probes and no
ordinary source work):

| Control | Original basis | Generator + saved application |
|---|---:|---:|
| K6 | 5 | 5 |
| H | 22 | 18 |
| FG | 16 | 15 |
| BMW | 17 | 14 |
| X | 19 | 15 |

All 1,193 maps compose with cold exact checks and zero saved-application failures.
The four-loop family-local sum is 74 -> 62, **not** a global independent-master
count: the four native families have different exact fingerprints, and their
coordinates cannot be merged without verified routings. The separate existing
qualified full-U census gives **23 -> 22** representatives (aliases 51 -> 40,
no skips). The twelve local eliminations therefore overlap heavily between
parent presentations; only one fewer full-U representative is established.
Extra guards from saved
rules are retained (H/FG/BMW/X: 1,019/411/1,247/2,173 final conditions). H's saved
projection takes 1.908 s, program preparation 4.424 s and full cold client
23.514 s. These candidate-program consequences retain their supplied-program
authority; they are not newly regenerated original IBP certificates.

## Broader generator relations: measured additional gain

The explicit generator policy has now completed an independent exact join on
the same original 939-key inventory, starting from the normalization-only 693
basis. It adds **26 independent dependencies**, giving **667 remaining**. From
the current 862 basis, the combined gain is therefore **195**, not the sum of
changed-map counts. An old independent public-core checker verifies all 939
original reductions from both starting capsules and retains all 136,725 source
conditions; six new exact elimination guards give 136,731 final conditions.

| Generator-policy boundary | Measured cost / count |
|---|---:|
| Prepare original-source equations | 6.541 s |
| Cold replay source certificate | 13.649 s |
| Exact retained-session join | 8.865 s |
| Full cold native client | 43.446 s |
| Peak process RSS | 3.45 GB |
| Combined equations / terms | 2,585 / 450,180 |
| Independent source rows before join | 2,184 |
| Auxiliary-inclusive column inventory | 327,266 |
| Native source certificate | 12,211,683 bytes |

This client consumes **zero ordinary IBP rows and no saved-program projection**.
Its equations combine broader verified generator maps and the 961 legacy rows;
existing inherited reductions are composed. The 667 count is not a minimum or a
claimed match to TIDE's roughly 131–132 different master convention. It is also
not yet a whole-application timing: the native CLI's optional extra saved-rule
and ordinary-corner work is measured separately.

The orbit/generator preparation timings concern different exact source spans,
not identical-workload speedups. The generator pilot overlapped an isolated
full-orbit replay on a different reserved physical CPU; host contention is
recorded. Source hashes are unchanged, native final-state replay passes, and the
exact independent checker found 26/195 rank gains beyond the 693/862 baselines.
Local receipts: `v3-five-generators-after-normalization693` and
`v3-checks/{beyond-normalization693,combined-from-current862}` under the acceptance
evidence directory.

The full-orbit pure test also completed within its original caller grant:
**693 -> 660**, hence **862 -> 660** (202 new dependencies). Its measured stages
were 500.562 s preparation, 16.648 s encoding, 672.065 s source cold replay,
67.388 s source-span compaction, 78.644 s exact join and 19.876 s final cold
check. Native total was 1,368.163 s; supervisor-inclusive wall 1,384.05 s; peak
RSS 28.026 GB. It compacts 1,800 source rows to 1,596 independent rows.

Independent checks preserve all original maps and conditions. For THIS dataset,
every generator-667 reduction also composes exactly through orbit-660 and every
one of its 136,731 guards survives; eight raw maps change and seven remaining
keys disappear. This empirical containment is not a generic theorem after
non-equivariant inherited IBP substitutions. Other finite cohorts may differ.
Neither result used new ordinary sources or saved-program projection.

## Integration and limits

Native Rust and CLI expose `sector_symmetry_assistance`; short Python steering
exposes `refine --sector-symmetry-assistance`, with
`--sector-symmetry-policy generators|polynomial-orbits`. The old weak flag remains available;
the stronger flag includes its source equations automatically. It is opt-in and
does not alter flag-off provider identity. Native v3 sidecars are carried through
fresh-search, interruption, finalization and export.

Saved-program projection is independently controlled by
`--[no-]sector-symmetry-saved-projection` (long production steering adds the
`master-` prefix). It defaults to enabled, preserving the full application path.
Explicitly disabling it sends the same complete witnessed/normalizer rows into
the existing exact finite executor without loading or applying the saved
programs again. This is useful for collecting the independently measured gains
first. It does **not** disable ordinary finite IBP work: even seed depth zero
includes corner sources. Reporting distinguishes projection not requested from
projection failure; unresolved auxiliary columns remain auxiliary, never masters.
Changing mode requires a new refinement phase; same-mode resume inherits it.

The real CLI profile also exposed a separate cold-loading cost: repeated
conversion of a shared 136,725-element condition slice into native coefficient
IDs for hundreds of output maps. A lifetime-bound slice-identity memo now reuses
the exact IDs. First misses retain the original native interning order; repeated
wire integer vectors and grants remain unchanged. It does not bypass replay,
serialize pointers or change any old artifact bytes. Seven regressions compare
cached/uncached atoms, state, structural records, complete wire bytes, limit
failures and cold round trips. This is generic serialization glue, not algebra.

Application cancellation now polls between routed operational frames, releases
dependency pins, retains completed guarded cache entries and returns no partial
current-row result. One CAS call or owner preparation is still atomic. Changing
the new source recipe requires a fresh refinement phase, not reusing an old
source cursor under different equations. Existing proved maps can be imported.

Discovery and exact elimination use pinned Symbolica 3.0, Graphica and Numerica
APIs after source/example/test inspection and executable probes. No new custom
polynomial, matrix, graph or reconstruction kernel is implemented in RustRed.

## Final validation and use

Final optimized native executable (local, not committed):
`TMP/releases/20261022-combined-master-refinement/rustred`, SHA256
`0aba2d21431f05015b7077e1ad4d5ee1470bf6798134ed2cdba6cd3da9efeb7a`.
All **827 native tests pass** (five explicit manual ignores), including seven
wire-byte memo regressions and twelve strong-provider lifecycle/policy tests.
All **298 Python checks pass**. Independent audits cover exact discovery,
Jacobian/mass/power transport, source-cohort replay, guard authority, saved/source
mode binding, immutable tasks, slice-cache lifetime safety and flag-off behavior.

An actual final-binary source-only command on a copy of the current publication
reaches 667, ingests all 2,134 assistance rows, and cooperatively pauses in
225.57 seconds / 218.94 CPU seconds / 2.34 GB peak. Three ordinary corner rows
ran before the polling stop; these are counted, not hidden as zero. Cold resume
with four instead of one worker takes 67.29 seconds and saves byte-identical
native state (generation 8 -> 9, same source cursor 3). Independent cold proof
replay still occurs; it does not rediscover or re-ingest the source rows.

Finalization succeeds in 46.95 seconds / 2.99 GB, preserving every source
checkpoint byte. The finalized artifact has 939 original keys and 667 collected
outputs. Normal public application cold-loads in 34.53 seconds and verifies all
939 mappings / 1,052 output terms, with one shared guard owner and no foreign or
unresolved output. All 136,725 inherited conditions survive; this derivation
records 136,790 final conditions. Exact quotient/product witnesses establish
that these imply the separately generated pure client's exclusions. A strict
literal guard-set comparison's negative receipt is retained as a representation
difference; no guards were injected and no statistical zero test was used.

The artifact is `published_unrefined`, correctly indicating that the interrupted
ordinary refinement search is not exhausted. Its accepted maps are nonetheless
exact and usable. It is not a new family-closure or master-independence claim.
Local finalized source artifact:
`TMP/sector-symmetry-acceptance-20261022/native-cli-final-source-only/finalized-application`.
This source folder and an exported runtime application package are different
objects; only a successful separately checked export establishes the latter.

The old full-projection CLI control cooperatively pauses after 932.26 seconds,
24/2,134 projected tasks, 11 saved-reduced rows, zero application failures and
zero ordinary rows. No assistance rows had yet entered its native session, so
its durable basis remains 862. Its exact preparation and completed projection
prefix are checkpointed; this is an incomplete/censored performance control,
not a claimed projection gain. The optional no-projection switch avoids this
delay; it does not discard mathematical obligations or reinterpret auxiliaries.

Portable focused Python validation:

```bash
PYTHONPATH=examples/python python3 -m unittest \
  test_campaign_phases test_saved_campaign test_finite_saved_campaign \
  test_campaign_dashboard test_campaign_monitor test_campaign_observation_details
```

Native validation used the pinned production capacity profile and exact captured
optimized dependencies. Standard Cargo entry point for the same package graph:

```bash
cargo test --release --locked -p rustred -p rustred-app \
  --features rustred-app/capacity-dispatch --lib
```

The focused native receipts list actual compiler commands and filters. Functional
test frontends are not solver benchmarks. A proposed `lto=off` test-link shortcut
fails on bitcode-only native dependency objects; its two negative receipts are
preserved and the single ThinLTO fallback passes. Compilation is excluded from
every application timing above.

After the existing user run has been interrupted cleanly and finalized, a new
round can be selected explicitly (not a same-phase policy change):

```bash
python3 examples/python/saved_campaign.py refine --campaign "$RUSTRED_CAMPAIGN" \
  --fresh-search --seed-depth 2 --containing-sector-depth 0 \
  --sector-symmetry-assistance --sector-symmetry-policy generators \
  --no-sector-symmetry-saved-projection --executable "$RUSTRED_BIN"
```

Same-round continuation after a clean pause omits `--fresh-search`; saved depths,
policy and projection mode are inherited. To change shape/policy, finalize the
paused round, then explicitly start another fresh round. No production process
was stopped, started, signalled or modified during development.
