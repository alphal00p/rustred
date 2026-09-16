# RustRed project goal

## Current assignment — four-loop delivery (2026-09-15)

**Parallel candidate validation (user directive, 2026-09-16):** do not wait for
whole-family certification before testing the current four-loop candidate IBPs
against Vakint's existing numerical acceptance cases. Run a separate explicitly
experimental lane with candidate application implemented in RustRed and steered
by Vakint's existing topology/routing and FeynKit tensor machinery. Check actual
integer targets, exceptional conditions, denominators and strict descent; retain
typed unresolved-integral errors and explicit supplied finite residuals. Use the
existing FMFT oracle offline to prepare residual values/projections and compare
the FORM-less native tail numerically. Never present candidate data as a sealed
`ClosedArtifact`, change production defaults, invent missing master values, or
count finite test parity as whole-family closure. This lane runs in parallel with
the exact certification work; final four-loop delivery still requires both.

**Separate generation and certification (user directive, 2026-09-16):** expose
generation without the additional independent certification pass for fair
SpIReD workload comparisons. Preserve the resulting exact candidate rules in an
explicitly uncertified bundle that can be certified in a separate invocation,
without rerunning discovery. Distinguish solver-internal checks from original-
source replay, uniform descent, complete coverage, publication and cold loading.
The C++ reference does perform construction/exception/reconstruction checks; its
example timings do not include RustRed's independent artifact certification.
Report matched solver-only and solver-plus-output timings separately from
certification and cold validation. Keep family/sectors/ordering/workers/backend
and output costs explicit. Skipping certification must never produce a
`ClosedArtifact` or imply a family-closure claim. Reuse the same candidate rules
for the experimental Vakint lane; preserve Rust/CLI/public Python API parity.
See [generation versus certification](docs/generation_and_certification.md)
for the inspected C++ timer boundary and comparison protocol.

**Latest implementation checkpoint (2026-09-16):** native factor-derived affine
guard consequences and explicit caller-owned predicate-atom allowances pass
642 focused core tests (16 existing ignored tests), the earlier 128-test
application/CLI gate, and 29 fresh release-wheel Python tests. K1/K3/K6 release
artifacts and cold reductions are unchanged; K6 bytes agree at one, two and six
workers. The fresh FG campaign clears its old guard blocker but stops at a
uniform-descent check in sector 115, rule 76; H advances to an affine-chart
integer-bit preflight limit in sector 229, rule 65. Neither publishes an
artifact. BMW and X reruns remain in progress at this checkpoint. These are
not whole-family closure or four-loop numerical-acceptance results. Evidence:
`/tmp/rustred-factor-policy-release.QSclTI/`.

The active tool-managed goal is now complete four-loop single-scale vacuum
closure and FORM-less Vakint evaluation against every applicable FMFT example.
K1/K3/K6 assets and exact offline terminal projections are shipped on
GammaLoop's `vakint_rustred` branch. **Latest validation (2026-09-16):** the
complete recorded 83-test selection now passes on `433e42d3`: **83 passed,
0 failed, 0 ignored**. This follows the independently audited 34-test targeted
gate for MATAD's signed-numerator routing correction. Both previously failing
all-class tests reach all five three-loop classes with unchanged tolerances
and an invalid FORM path on the FeynKit/RustRed lanes. Separate legacy oracle
lanes use FORM. The selected matrix includes native peers, references,
matching/defaults and the offline 38-terminal catalog; it is not a count of
distinct integrals. The exact manifest, frozen binary hashes and nine batched
process results are retained in `/tmp/vakint-orientation-full-gate.Ny652I/`
and `/tmp/vakint-batched-83.baAvzc/`. Clippy also passes for the touched Vakint
library and comparison targets. Four-loop acceptance is a separate open gate.

**Dependency-migration rerun (2026-09-16):** the same frozen 83-test selection
also passes in GammaLoop milestone `a3d26dab`, pushed to `vakint_rustred`, using published RustRed
`09cef8e3`, Symbolica 3 at `953e26e2`, and the regenerated `0x704` K6 asset:
83 passed, zero failed or ignored. Native lanes retain invalid FORM paths;
legacy reference lanes alone use FORM5. Full workspace library, test and
benchmark compilation passes, as do 28 focused GammaLoop tests, four Spenso
tests, 52 FeynKit tests (one unchanged ignored test), and scoped Vakint Clippy.
Native solver guards, requested precision, and zero-loop point samples are
preserved. Symbolica 3 uses runtime signed licensing; the removed OEM activation
path is not emulated. Evidence is retained in
`/tmp/vakint-symbolica3-main.bMELju/acceptance/`.

**Four-loop routing milestone (2026-09-16):** GammaLoop `30705f12` is pushed
to `vakint_rustred`. Typed tests exercise the existing simultaneous routing
witnesses for all 19 registered four-loop classes and 123 surviving physical
slots, including six nonidentity witnesses. The shared parent validator
preserves signed momenta and checks contracted-away defining slots as well.
The focused gate passes 39 tests (23 library and 16 K6 pipeline), with one
existing offline catalog comparison ignored; the full 83-test inventory was
not rerun for this routing-only change. No dependency pin, shipped artifact,
default, or public API changed, and four-loop RustRed admission remains
unavailable pending exact artifacts and numerical validation. See the
[typed census and evidence](docs/four_loop_vakint_census.md).

The earlier physical FG full-family blocker was an exact-source certificate failure,
not failure to finish the sector search. That scoped release campaign searched all
124 nonzero sectors in 32.9 seconds, then rejects three certificates in sector
106. A selected-sector diagnostic reproduces this in 0.82 seconds and proves
that the disputed `n0=0,n8=-1` boundary is admitted. A conservative,
translation-safe source-projection correction has since passed release validation;
all 225 solver unit tests and 104 artifact source-port tests pass (five larger
source-port workloads excluded from that debug gate). A separate reconstruction
of rule 85 confirms that only the speculative parent-sign quotient finds a
certificate: the pivot is recentered by `+1` on `n0`, but the deleted column
then activates at `n0=0`. Its 29 nonzero weights have only `n8` denominators;
an unrecorded `n0` pole is not the cause of this actual failure. Neither a
valid-looking candidate formula nor zero uncovered boxes can bypass complete
original-source verification. The corrected release build now passes that
selected sector: 99/99 replayed and descending rules, one finite terminal,
zero uncovered boxes and no issues, in 1.68 seconds wall (557 ms search,
1.002 s audit). This costs more than the old 0.82-second failing diagnostic.
The matched, unchanged SpIReD C++ solver reproduces the old 96-rule candidates
and the disputed targets/RHS/exclusions exactly; matching that output does not
replace source-certificate validation.

The corrected full physical-FG CLI attempt subsequently searches all 124
nonzero sectors in 57.1 seconds (9,272 rules, 145 finite residuals), passes 32
sector audits including sector 106, and reaches its 600.13-second bound while
checking sector 214. No artifact is written. A 15-second, 724-sample CPU profile
of that long check attributes 97.90% inclusive sampled cycles to native exact
sparse elimination and 95.67% to original-source membership proposals;
polynomial GCD has 64.90% inclusive cost. These overlapping percentages are
local to that window, not whole-campaign phase fractions. The next slice is
compact original-source certificate construction using native modular
dependency traces followed by exact lifting and unchanged full replay—not
larger search budgets or weaker publication checks.

**Compact-certificate follow-up (2026-09-16):** bounded native modular support
selection, exact lifting, full unprojected replay and no-new-index-pole admission
are implemented and independently audited. All 121 source-port tests pass
(five existing larger workloads ignored). Release K1/K3/K6 generation, cold
inspection and canary application pass; K6 retains 623 rules, 5,640 cells and
38 masters, with byte-identical output at one, two and six workers. This
certificate-only slice was checked through the CLI, not a rebuilt Python
extension. The full physical FG rerun searches 124 sectors in 59.4 seconds,
passes 32 audits, then again times out in sector 214 at 600.14 seconds. No
four-loop artifact is published. The next implementation records the existing
polynomial preconditioner's exact forward derivation for certificate reuse,
with full original-source replay and guard checks retained. See
[the certificate profiling report](docs/research/original_source_certificate_performance.md).

**Preconditioner-provenance follow-up (2026-09-16):** the existing polynomial
preconditioner now optionally records its forward row derivation. Exact
selected-frame weights can be composed onto original IBPs instead of solving
that membership problem again; native Symbolica arithmetic, no-new-pole checks,
full replay, and the previous fallback remain authoritative. All 359 focused
solver/source-port tests pass (five existing larger workloads ignored).
The release selected-sector check passes FG106 in 0.70 seconds; FG214 now
finishes in 106.60 seconds rather than hitting its prior cap. It accepts 154
of 161 candidates and rejects seven affine cases on strict-descent geometry.
The cover check returns on its first counterexample valuation: one reported
bounded box and zero unbounded boxes are **not** an exhaustive inventory or
proof that no infinite gaps remain. No new terminal or four-loop artifact is
authorized by those counts. Independent audit now exhibits three genuine
infinite gaps if the rejected rules are simply omitted; it identifies exact
fixed-face/equality reasoning that can address the reported false descent
pieces without weakening the order.

The frozen release CLI rerun searches all 124 physical FG sectors by 39.9
seconds and passes 32 audits. It finishes in **138.07 seconds wall**, rejecting
sector 214 on the same seven geometry obligations rather than timing out;
no artifact is written. Current CLI K1/K3/K6 generation, fresh-process loading
and reduction pass. K6 is byte-identical at one, two and six workers, with
623 rules, 5,640 cells, 38 masters and 8,926,013 bytes; its dotted reduction
matches the previous producer exactly. Its 3.23-second serial CLI observation
is a shared-host regression, not a controlled speed comparison. The Python
extension is not rebuilt for this certificate-only slice. The next gate is
the independently audited generic geometry correction and another exact
full-family attempt. Evidence:
`/tmp/rustred-provenance-certificate.gSddBU/`.

**Exact-domain follow-up (2026-09-16):** relative child-face implication,
native joint-equation reduction after singleton specialization, and bounded
exhaustion of one short finite axis are implemented and independently audited.
The combined solver/source-port/affine gate passes 427 tests, zero failed,
five existing ignored. An intermediate selected release run admits 159/161
FG214 rules in 100.29 seconds; the final bounded-refinement snapshot admits
**161/161 replayed and uniformly descending rules in 85.10 seconds wall**
(15.086 s search, 69.748 s audit). These single shared-host observations are
not controlled speed ratios. All 11 finite terminal candidates are unchanged.

Sector publication still rejects the exact predicate cover. The new bounded
diagnostic identifies its first abstract Boolean failure at
`n=(-1,1,1,0,1,-1,1,1,-2,0)`: it assigns `1+n5-n9` nonzero although that
polynomial is zero there. Independent inspection finds stored rule 117 owns
the point. Thus this first witness is unrealizable, not a new uncovered
integral; it says nothing about later failed valuations or whole-family
closure. At that checkpoint a narrow native singleton-substitution consistency
check was the next implementation, with nonconstant restrictions, unknowns and budget failures
remaining inconclusive. No extra terminal, sampling, or weakened replay is
authorized. This release snapshot has not rerun the full family and writes
no four-loop artifact. Evidence:
`/tmp/rustred-affine-width-release.CNbu4g/`.
The same snapshot passes release K1/K3/K6 core generation followed by
fresh-process inspection and canary application with the preceding compatible
CLI. Artifact bytes and reductions are unchanged. The unnamed diagnostic K6
input takes 2.88 seconds wall; this is not the named CLI benchmark or a new
frontend build. These lower-loop regressions do not establish four-loop closure.

**Predicate-work and exclusion follow-up (2026-09-16):** a leaf-only native
consistency check passes 433 focused tests but its full FG run still rejects
coverage after 131.50 seconds; that snapshot validates all 161 FG214 rules.
The subsequent early-pruning change retains the same proof budget, prunes
contradictions before unrelated Boolean branches, and reports exhaustion
explicitly instead of displaying it as another apparent uncovered point.
Its independent audit and 436-test focused gate pass. The release full-family
run searches all 124 physical FG sectors by 39.384 seconds, passes 32 sector
audits, and exits in **125.79 seconds** on the exact-substitution work limit.
The FG214 rule traversal reaches its last entry, but no final sector report
is returned; do not infer a new aggregate coverage or certificate count from
that marker. No artifact is written. The next bounded investigation reuses
identical native polynomial restrictions within a traversal, with immutable
context binding, finite cache storage and unchanged exact proof limits.

The leaf-only snapshot also searches all 314 physical H sectors by 61.639
seconds, passes 47 audits and exits in **71.92 seconds**, retaining 53/54
H226 rules. Independent all-owner inspection proves that the rejected rule's
bad sign cell lies wholly in an existing affine exclusion, while the rule
alone covers another genuine infinite ray. The exclusion was retained but
omitted from the descent proof call. A generic shared whole-box containment
service now threads that domain through raw descent, producer lowering and
cold verification; entirely excluded incoming cells are rejected, not sealed
by vacuous proofs. This follow-up passes 442 focused tests and independent
implementation/math audits. Its selected release H226 audit passes all 54
rules, with zero uncovered regions, in 0.50 seconds. The full H rerun searches
all 314 sectors and passes 56 sector audits before exiting after 79.43 seconds
at H370. That sector retains 133/134 replayed/descending rules. Rule 66's
exception is rejected by the singular intersection used during replay; an
independent native-API probe resolves the same conjunction with the existing
exact disjunctive intersection already used by discovery. Its first abstract
cover witness is also inconsistent: after fixing n0=-1, a true equation is
the negative of an equation assigned false. These are two distinct remaining
verifier corrections, not evidence of a missing master. No artifact is written, and no master
inflation, dropped rule or relaxed guard is authorized by either failure.

The early-pruning release regenerates byte-identical K1/K3/K6 artifacts and
passes current-core fresh-process decoding and master-only canaries. Unnamed
K6 generation takes 2.34 seconds wall, retaining 623 rules, 5,640 cells and
38 masters. These are shared-host Rust API regression observations, not
controlled speedups or rebuilt frontend benchmarks. Evidence is retained in
`/tmp/rustred-early-pruning-release.72tckt/` and
`/tmp/rustred-h226-case.vE4em0/AUDIT.md`. Four-loop closure and Vakint numerical
acceptance remain open.

The H-exclusion snapshot independently regenerates the same K1/K3/K6 bytes
and passes current-core cold loading and master-only canaries; its unnamed
K6 observation is 2.51 seconds. H release evidence is at
`/tmp/rustred-exclusion-release.zcMlbk/`. The subsequent bounded restriction
cache is a separate development slice and is not included in those timings.

**Cache/replay release and next exact proof slice (2026-09-16):** bounded
reuse removes the observed FG substitution-work cap. The full release FG
attempt searches all 124 sectors in 55.181 seconds, then validates all 161
FG214 rules after 32 earlier passing sector audits; the process exits after
139.95 seconds on a contradictory abstract guard assignment. H searches all
314 sectors in 58.482 seconds, passes 56 earlier audits, and now validates
134/134 H370 rules after switching replay to the existing complete disjunctive
intersection service. It exits after 83.20 seconds on its contradictory guard
assignment. These are shared-host observations, not controlled speed ratios.
Neither process writes an artifact. K1/K3/K6 bytes and current-core cold/reducer
canaries remain unchanged.

A generic bounded native affine-implication check is implemented and passes
473 focused tests with five existing workloads ignored, plus independent
review. It uses the existing native canonical equality service, never a new
CAS: only inconsistency of true equations or consistent equal-rank augmentation
can refute an abstract branch. Its subsequent release results are below. BMW's
separate full search finishes 133/134 sectors before rejecting the root-free
quadratic `n1^2-3*n1+4=0`; reuse of already-computed native irreducible factors
is the next separate geometry slice. No sampled coverage, extra masters or
larger proof cap is authorized. The
[guard-consistency report](docs/research/four_loop_predicate_consistency.md)
records the exact examples and evidence.

**Native implication release follow-up:** the next frozen release closes
H370's actual sector audit (134/134 replayed/descending rules, zero uncovered
regions, 11.02s selected process). Whole H now passes 91 complete sector
audits, then stops at H214 on the native affine-consistency work budget after
112.97s; whole FG stops at the same resource after 109.92s and 32 passing
audits. No final aggregate is returned for either stopping sector and no
artifact is written. K1/K3/K6 bytes, cold-load canaries, and K6 workers 1/2/6
determinism pass. Next reuse repeated native true-system/false-extension
checks with complete exact cache keys and bounded observational counters;
do not increase caps or weaken proofs. Separately, the root-free univariate
factor correction for BMW/X passes 44 focused tests and independent audit,
with its subsequent release results below.

**Root-free-factor release follow-up:** complete native Symbolica
factorization now proves univariate irreducible degree-above-one exceptions
empty over integer indices. No root solver, sampling, extra factorization,
or topology-specific rule is introduced. All 484 focused tests pass (five
existing workloads ignored), with independent proof/code audit. BMW223 now
passes 60/60 replayed and descending rules and complete coverage in 5.21s.
X214 completes discovery but reaches the native affine-consistency budget
during exact publication. Whole BMW and X now finish every sector search:
134 and 328 sectors, respectively. Publication passes 45 BMW audits and 31 X
audits before stopping at BMW158 and X460 on consistency-budget errors, after
178.17s and 257.47s wall. Neither writes an artifact or identifies an actual
uncovered domain. K1/K3/K6 bytes, current-core cold/reducer canaries and K6
workers1/2/6 determinism pass unchanged. Bounded exact-key scalar consistency
reuse and detailed observational counters are the separate next slice, not
part of these release observations. Evidence and mathematical boundary are
in the [guard-consistency report](docs/research/four_loop_predicate_consistency.md).

**Bounded consistency-reuse release follow-up:** successful native base and
false-extension outcomes now share the existing bounded cache envelope,
keyed by exact boxes and complete true-atom sets. All 495 focused tests and
independent audits pass, with unchanged proof limits and K1/K3/K6 artifact
bytes, cold canaries and workers1/2/6 determinism. Full H and FG now pass every
sector audit: 314 sectors/21,360 rules and 124 sectors/9,272 rules respectively,
with exact replay, strict descent and zero coverage gaps or issues. Durable
lowering still fails at H370 rule66 on a prospective guard-chart bit bound,
and FG114 rule66 on a guard-nonvanishing proof. Their complete attempts take
338.17s and 232.39s wall; neither writes an artifact. BMW advances to 97 passing
sector audits before another consistency-work limit; selected X460 retains
that limit. A sampled BMW audit is dominated by exact source replay rather
than affine consistency. Next diagnose the captured H/FG guard formulas and
reuse existing native services for the smallest exact correction; pursue
bounded work-policy changes separately if needed. No extra masters, sampled
closure, proof bypass or numerical Vakint parity follows from these sector
results. Evidence: `/tmp/rustred-consistency-reuse-release.kKCcQ1/`.

**Joint guard-coefficient follow-up (2026-09-16):** native substitution now
discards a candidate exceptional hyperplane if any coefficient equation
restricts to a nonzero constant there. Nonconstant restrictions remain
inconclusive, and exactness requires every equation to vanish on every
surviving hyperplane. Existing full work precharges and limits remain.
All 553 focused tests pass (five existing ignored), with independent code
and mathematical audit. The full release FG run again passes all 124 sector
audits and 9,272 replayed/descending rules with zero gaps or issues. It now
lowers FG114 completely, passing the previous failure. Publication advances
from 17 to 32 complete sectors, then FG214 rule74 hits the prospective affine
guard-chart integer-bit bound. The process exits after 246.78s wall and writes
no artifact. K1/K3/K6 bytes, current-core cold/reducer canaries and K6 workers
1/2/6 determinism remain unchanged. Next capture the actual failed lowered
guard and avoid charging chart expansion for variables not substituted,
using authenticated fixed/pivot support and Symbolica's existing APIs.
No four-loop closure or Vakint numerical acceptance is claimed. Evidence:
`/tmp/rustred-joint-guard-release.QQBORj/`.

**Support-aware guard follow-up (2026-09-16):** bounded diagnostics captured
the actual H370 and FG214 failures. Their polynomials contain none of their
prepared chart's substituted variables, so dense expansion estimates were
unnecessary. The exact support shortcut preserves original admission and
downstream guard proofs. Native specialization of cell singletons also proves
the captured H guard without inventing an exclusion. Independent source and
mathematical audits and all 603 focused tests pass (16 existing ignored in
the broader diagnostic-inclusive selection). Full release H and FG again
pass every sector audit: 314/21,360 and 124/9,272 sectors/rules respectively,
with zero gaps or issues. Both previous guard failures are cleared.

The next publication bounds are endpoint-storage counts: H370 rule90 requests
8,720 cells and FG214 rule76 requests 11,600, against the unchanged 8,192
allowance. These are conservative allocation estimates, not uncovered
integrals. H exits after 327.18s wall and FG after 240.70s; neither writes an
artifact. K1/K3/K6 bytes, current-core cold loads, exact master-only canaries
and K6 workers1/2/6 determinism remain unchanged. Next expose the existing
resource allowance consistently to generation and cold loading, retaining
typed failures and all mathematical checks; handle BMW/X's separate native
consistency allowance through the same explicit policy discipline. Do not
remove real proof buffers from the estimate or let artifact bytes authorize
their own load limits. Evidence: `/tmp/rustred-chart-support-release.FT3gS3/`
and the [guard-consistency report](docs/research/four_loop_predicate_consistency.md).
No four-loop artifact or numerical Vakint acceptance is claimed.

**Explicit resource-policy follow-up (2026-09-16):** caller-selected endpoint
and predicate-consistency allowances now propagate through producer audits,
lowering, installation and independent cold loading. Rust, CLI and Python
surfaces expose the same settings; defaults remain unchanged, zero is restrictive,
and artifact bytes cannot authorize their own proof budget. The independently
audited core gate passes 606 tests, zero failures and 16 existing ignored.
The rebuilt frontend gates pass 134 Rust tests and all 29 public Python API
tests; setup/stale-example failures and their corrected reruns are recorded
separately in the research report.
Current release CLI K1/K3/K6 generation, fresh-process loading and exact canaries
pass, with unchanged bytes and K6 worker1/2/6 determinism.

Complete public release CLI attempts selected 65,536 endpoint-storage cells
and 67,108,864 consistency-work units. H, FG and BMW pass every sector audit:
314/21,360, 124/9,272 and 134/9,024 sectors/rules respectively. They lower
66, 41 and 31 complete sectors, then encounter distinct guard proof-service
limitations. H/FG repeat a weaker coordinate guard check after stronger private
original-domain proof; BMW's captured guard zero locus lies in an already
retained exclusion, but the verifier has not proved that implication. X passes
65 sector audits, then reports an abstract complement at X394; an exact linear
combination and its box bounds prove this first branch empty. These findings
do not imply later branches or all remaining publication obligations are solved.
Whole-process times are H318.00s, FG232.45s, BMW485.26s and X340.06s on a shared
host, not controlled performance ratios. All exit without an artifact.

Next reuse the existing private full-domain guard seal for coordinate cells,
without changing public/unsealed constructors; pursue bounded existing native
consistency refinement for X and native polynomial-ideal proof for BMW. Each
requires independent code/mathematical audit and another release full-family
attempt. Never hard-code the captured identities, relax guards or create masters
from abstract complements. Evidence: `/tmp/rustred-publication-policy-release.4PE5dn/`
and the [guard-consistency report](docs/research/four_loop_predicate_consistency.md).

**Private coordinate-seal follow-up (2026-09-16):** the private original-domain
cell constructor now consumes its existing full-domain guard proof for coordinate
pieces too. Public/unsealed guards are unchanged. Independent code/math audit
and 608 focused tests pass (16 existing ignored), including real-source K3
partitioning, genuine-zero/widening rejection, cold roundtrip and exact reduction.
Release public CLI K1/K3/K6 bytes, cold loads and exact reductions are unchanged;
configured workers1/2/6 also agree with six CPUs available.

Full H/FG release attempts retain all 314/21,360 and 124/9,272 passing sector
audits respectively and clear the weaker recheck. They still reject later pieces
of the same displayed rules in the full original-domain guard proof, after
312.95s and 247.25s wall. No artifact is written. The captured H guard's joint
coefficients imply a forbidden zero of n9; FG has a factored base coefficient
nonzero by its affine box bounds. Reuse those existing native proof services
alongside the independently designed BMW affine-conjunction and X finite-axis
refinements. Do not count source prototypes as release cures or infer closure
from these analyzed guards. Evidence: `/tmp/rustred-private-seal-release.9Fm7bz/`.

**Native proof-refinement follow-up (2026-09-16):** the runtime-arity guard
verifier now retains the full coefficient conjunction, repeatedly applies
existing native affine charts to nonlinear siblings, and reuses native
affine-box proofs with bounded precharged work. Boolean coverage can exhaust
one short finite integer axis while preserving every other infinite direction;
all children share one budget and cannot recursively split. Independent
implementation/mathematical audit passes, and the combined focused gate passes
629 tests, zero failed, 16 existing ignored. All 21 new regressions pass,
including the captured X/BMW/H/FG obstructions and surviving-domain controls.
The first run's obsolete work-counter expectation was corrected without
changing any genuine-root assertion. This is a proof-service improvement,
not a new IBP, terminal or family closure claim. The frozen public release
CLI/Python build succeeds in 6m53. K1/K3/K6 bytes, fresh-process canaries and
exact reduction reports remain unchanged; K6 workers1/2/6 agree. A freshly
installed release wheel passes all 29 Python API tests with no skips.

All four new full campaigns finish without an artifact. H/FG clear the old
guards and fully lower 72/82 sectors (previously 66/41), then stop at new guards
H58/FG83 after 308.58s/283.58s. BMW clears the previous guard but rejects the
next guard of the same displayed rule after 568.98s, with 31 sectors lowered.
All H/FG/BMW sector audits still pass. X advances from 65 to 144 complete sector
audits (8,893 rules), then stops after 551.93s when preparing X369's 33rd distinct
predicate under the separate 32-atom limit. This is not an uncovered witness;
the rest of X remains unverified. Reuse native factor evidence to expose the
necessary affine consequences without dropping original coefficient siblings;
investigate explicit predicate policy/normalization separately, preserving
fail-closed work and traversal limits. Evidence:
`/tmp/rustred-native-refinement-release.YaRtcr/`.

**Numerical diagnostics directive (2026-09-16):** include bounded reproducible
integer and finite-field sampling in the investigation, not only symbolic
reasoning. Use native Symbolica evaluations/reducers to find counterexamples,
screen dependency/support candidates and prioritize exact materialization.
Retain target/exclusion conditions; finite fields do not model integer order or
box inequalities. Record sample domains, seeds, primes and unlucky-prime/pole
handling. Replay proposed counterexamples exactly. Successful samples guide
work but cannot certify infinite-domain coverage, remove guards, authorize a
terminal or replace the final exact artifact gates. The current observed
obstructions are proof-service/resource limits, not demonstrated missing rules;
do not infer that every later unchecked obligation will pass.

**Semi-numerical proof research (2026-09-16):** independent modular-algebra
and geometric-proof investigations favor bounded discovery of small witnesses,
followed by native exact verification. A diagnostic Symbolica experiment
discovers affine multiplier coefficients for the captured H58/FG83 guards from
point evaluations and verifies both complete polynomial identities exactly.
The coefficient pairs, target monomials and multiplier supports were supplied;
this is not autonomous certificate-template selection or production integration.
Integer/finite-field probes are diagnostic only. Prioritize existing native
factor/affine consequences, then sparse multiplier and local affine-conflict
certificates; retain complete domain/exclusion checks. General CAD/SOS and
randomized-only verification are not the immediate implementation direction.
See the audited [research synthesis](docs/research/seminumerical_proof_systems.md).
No new four-loop artifact or Vakint acceptance follows from these experiments.

**Vakint precision policy (user directive, 2026-09-16):** compare requested
precision with the known precision of supplied master data. If the request is
higher, emit a clear warning identifying the requested and available precision
and continue at the requested arithmetic working precision. Do not invent
digits or silently relabel source accuracy. Missing Laurent coefficients and
unknown expansion orders remain errors. Apply this consistently to relevant
native and legacy master-substitution paths while preserving defaults and
existing evaluation results.

This warning-only policy is implemented and pushed on `vakint_rustred` as
`98e0989d`: native/legacy FMFT, MATAD, AlphaLoop and RustRed numerical terminal
substitution inspect the original stored float precision before resizing.
Per-evaluation/table-pass warnings use the normal logger and do not alter
values, working precision or missing-Laurent-data errors. The focused native
and K6 pipeline gate passes 43 tests; this is not a new full-83 rerun or a
four-loop RustRed acceptance claim.

The frozen Symbolica 3 Vakint baseline independently passes all **15 existing
deterministic four-loop FMFT reference entrypoints**, including the decorated
four-loop clover whose test name says `1l`, plus all eight native PR-finalizer
unit tests with invalid FORM paths. These are not four-loop RustRed acceptance
passes. The adapter must consume four structural parent descriptors and the
existing simultaneous routing witness, append auxiliary zero powers, and use
RustRed's generic numerator lowering and rule applier. Five optional PySecDec
tests remain separate; one needs deeper master-series data than current FMFT
tables supply. Existing numerical constants are not a 20,000-digit catalog.
Evidence: `/tmp/vakint-four-loop-frozen-gate.FKioIB/REPORT.md`.

Release CLI K1/K3/K6 generation, fresh-process inspection and canary reduction
all pass with the correction. K6 remains 623 rules and 38 masters, now with
5,640 refined cells and 8,917,577 bytes. One-, two- and six-worker artifacts are
byte-identical; its dotted canary matches the previous artifact exactly.
The rebuilt public Python extension generates the same bytes, and all 27
Python API tests pass against that extension and the updated release CLI.
The serial K6 generation observation is 2.47 seconds wall; this shared-host
regression is not a controlled speed comparison. Evidence is retained at
`/tmp/rustred-safe-projection.DMp3co/`.

**Historical acceptance-report correction:** revision
`98550cc0` records 76 passing invocations but does not establish complete
through-three-loop acceptance. Its three-loop selection has 24 tests while
the result table contains only 17. One omitted test's raw output records an
AlphaLoop-versus-MATAD failure. Independent frozen-binary reruns of the missing
seven now give five passes and two failures, bringing the joined selection to
83 tests: 81 passed and two failed. The legacy-setting variants and basketball
finite parts pass; both supplemental all-class pipelines still fail at
`I3L_pinch_1_6` on AlphaLoop versus MATAD before comparing RustRed. Separate
pairwise diagnostics now show RustRed agreeing with AlphaLoop through the
finite part, but disagreeing with MATAD at both unit and nonunit mass. The
leading-pole relative discrepancy is approximately `4.741253423e-4`; its
persistence at unit mass rules out mass restoration as the sole explanation.
Numerator/scalar-product isolation preserves the original assertions.
An independent frozen-binary diagnostic now passes eight of thirteen probes:
the constant numerator, all three loop squares, and `k1.k2` agree with MATAD;
`k1.k3` and `k2.k3` disagree by an exact sign, as does one isolated original
numerator term. Source inspection identifies missing parent-edge orientation
signs in MATAD's contracted-loop numerator mapping. Two independent audits
confirm the signed map `[+p4,+p5,+p6,-p1,+p2,-p3]` from its momentum identities.
The correction and targeted rerun above resolve this gap.
Native peers use FeynKit, RustRed and an invalid FORM path. The legacy
40-entry / 46-input inventory has passing mapped peers; the new rerun adds
passing supplemental all-class parity. Preserve the failed historical evidence
and the distinction between targeted and full-suite reruns.

Four-loop closure is **not yet achieved**. Its physical parent families H, X,
BMW and FG are explicit files in `examples/input`, each completed to ten
scalar-product coordinates with auxiliary ISP powers initially zero. They are
input data, not solver dispatch keys. Each has sixteen ordinary IBP sources.
The next production gates are exact all-sector artifact generation and cold
reload for these parents, existing-witness routing in Vakint, offline terminal
catalogs, pure-Rust FMFT master finalization and the full four-loop numerical
comparison inventory. Do not infer four-loop acceptance from lower-loop tests
or from a single sector's rule count.

The frozen [four-loop matcher census](docs/four_loop_vakint_census.md) contains
19 registered classes: H, X, BMW and FG with 15 inequivalent contractions.
All 123 surviving physical slots pass exact signed-momentum transport using
the existing simultaneous witnesses, and their ten-axis embeddings correctly
zero-fill pinches and auxiliary powers. The artifacts/adapter must cover all
19, independently of the fifteen deterministic numerical reference entrypoints.

The generic `family-close` Rust/CLI/Python interfaces are published at
`a435599`. A literal-unit H full-family release attempt with six workers took
162.272127 seconds and failed safely at the affine-domain publication boundary;
no four-loop artifact was written. The first rejected sector has 91/94
replayed/descending candidates and one uncovered box. These are sector-local
counts, not a percentage of whole-family completion.

The bounded natural-order parent probes now also record X timing out at
300.118 s with 1,210,032 KiB peak RSS, BMW timing out at 300.083 s with
845,304 KiB, and FG failing closed after 233.534 s with 1,087,096 KiB on an
unsupported nonlinear exceptional intersection. None wrote an artifact. The
exact affine-domain carrier and membership predicate are pushed in `ed889f8`;
authenticated RuleCell ownership, persistence, and coverage were absent at
that checkpoint. The predicate-aware implementation and sector validation
described below supersede that limitation; full-family acceptance is pending.

Vakint revision `512d3ce8` adds a pure-Symbolica FMFT master-finalization
boundary without activating four-loop RustRed reduction. Eight native tests
pass with invalid FORM paths, and all fourteen unchanged analytic FMFT
four-loop oracle tests pass after the shared-tail extraction. Those fourteen
are **not** RustRed four-loop acceptance passes. The milestone pins published
RustRed revision `ce92d3a7` for its existing coherent Symbolica 2.2 stack;
alignment with RustRed's current Symbolica 3.0 stack was subsequently completed
in `a3d26dab`, including atomic K6 regeneration and full selected acceptance; see
[the dependency migration plan](docs/vakint_symbolica3_migration.md).
No temporary local dependency is shipped.

The source-port artifact bridge now retains exact affine targets and exclusions,
proves guards on those predicates, and certifies their whole-sector cover. It
may omit unsupported candidates only if independently replayed, descending
rules and finite terminals already prove coverage. Unproved conditions still
fail closed. See
[four-loop ordering evidence](docs/four_loop_ordering_probe.md). Parallel agents
develop and independently audit the Vakint and artifact-delivery slices;
RustRed continues to own the generic rule applier and all algebra stays with
Symbolica. Four-loop numerical tables must state their actual precision:
existing FMFT constants do not supply a universal 20,000-digit catalog.

The affine-validation slice now also checks finite coefficient leaves against
the original affine domain, proves guard-zero faces empty or wholly excluded,
and persists exact exclusions in the new source-port plan format. A release
rerun of H sector `0010011001` passes all 94 rules, exact predicate coverage and
lowering to 1,202 rule cells in 44.44 seconds (including two replay passes;
generation alone 1.205 seconds). K1/K3/K6 durable regressions pass. The targeted
H sector is an implementation regression, not whole-four-loop closure. The
next gate is a full-family CLI attempt followed by cold reload and reduction.
The subsequent reverse-order full H CLI run reached its 600-second bound
(600.16 s wall, 843.97 s CPU, 849,296 KiB peak RSS), with neither an artifact
nor a diagnostic. Its phase cannot be inferred from CPU utilization; add
observational progress before the next full run. The isolated FG failure now
has an exact polynomial payload: it is a genuinely nonlinear quadratic
condition on three nonpositive powers, not an unrecognized affine face.
Investigate alternative valid pivots/orderings without discarding its
nonempty exceptional domain. Both natural and reversed orderings fail on this
isolated FG mask; however, the reverse-order quadratic has only one admissible
integer zero, suggesting a narrower exact geometric classification route.
The mask activates an auxiliary ISP as a propagator, outside the physical FG
root domain. A two-worker release diagnostic on that explicitly chosen
physical domain now finishes all 124 nonzero sector searches in 33.47 s wall
(plus 132 proved-zero sectors), yielding 9,264 proposed rules and 145 finite
residuals. This is search completion, not yet exact replay or a closed artifact.
The delivery slice now under validation adds an explicit, caller-specified
downward sector domain through Rust, CLI and Python. The unrestricted default
is unchanged; `nonpositive_indices` declares auxiliary coordinates that may not
become positive. The domain is serialized and cold-checked, global zero-source
evidence is retained, and out-of-domain reductions are rejected. Zero-only
scopes remain explicitly unsupported. Required gates for the physical parents:
prove every admitted sector and RHS transition, bind the domain into the
artifact/codec/identity, and reject out-of-scope reductions. Never silently
filter sectors while retaining the current unrestricted-family promise.
Run the full replay, descent, coverage, cold-load and reduction gates on the
resulting physical-domain artifacts before enabling their Vakint routing.
See [the exact FG diagnosis and scope analysis](docs/research/four_loop_fg_exceptional_geometry.md).
The subsequent instrumented unrestricted H run finishes all 728 searches by
164.6 s, but reaches a 1,200.24 s bound without an artifact. It completes 47
sector audits before spending over 926 s checking sector mask `0010011011`
(integer mask 868), which activates H's auxiliary coordinate. Thus serialized
exact verification, not discovery, is the observed bottleneck in this run.
Its inner replay/guard/descent/coverage stages still need separate profiling;
do not attribute the cost to a specific CAS routine from this observation.
The new scoped producer passes 120 core source-port tests (five separate
ignored workloads), 16 application tests, five CLI tests and eleven artifact
API tests. All 27 Python API tests pass against the release extension and CLI.
Generic external-input K6 generation now also passes through both interfaces:
623 rules, 5,639 cells, 38 masters and 26 zero sectors, with identical artifact
bytes at one, two and six workers. Serial Python generation takes 3.626 s in
one release observation; fresh CLI generation takes 2.75 s at two workers and
2.24 s at six. These are shared-host regression observations, not paired
scaling benchmarks. Separate cold inspection and the 30-term canary pass,
and all master keys match the existing Vakint catalog. The new identifier-safe
example label and `0x704` root-scope format were shipped together with the
coordinated private loader/asset/producer-pin migration in `a3d26dab`.
The first full physical-FG publication attempt completes all 124 searches but
fails original-source replay on three of 96 candidate rules in sector mask 106;
zero reported uncovered boxes cannot override these unproved source residuals.
No four-loop artifact is published. The next diagnostic is the exact residual
on the activating coordinate boundary, not relaxed proof acceptance.
Current measurements and rerun results belong in
[the parent-probe report](docs/four_loop_parent_closure_probe.md).

## Parallel Vakint delivery directive (2026-09-14)

Alongside the active SpIRed reference port, develop and independently audit the
K6 artifact-to-Vakint delivery lane now. The complete `vac3` source-port run
(617 conditional rules and 38 residual keys) is its input evidence, not an
already certified, shipped K6 artifact. Establish exact replay, descent,
guard/sector coverage, finite terminals, and coverage of all five registered
three-loop vacuum graph classes before treating that output as closing.

Separate subagents own artifact/closure auditing and Vakint integration, so
this lane does not interrupt the current solver-port work. On GammaLoop's
`vakint_rustred` branch, ship immutable generation-once artifacts, reuse the
existing topology match and routing witness, apply them with RustRed, and use
the FORM-less FeynKit tensor prepass. Reuse and extend the existing comparative
test harness to every applicable single-scale three-loop acceptance case,
including master substitution and numerical parity. A finite nonminimal master
basis is acceptable; existing Vakint oracle methods may evaluate required new
masters offline to 20,000 decimal digits. The ordinary RustRed evaluation path
must neither regenerate artifacts nor invoke/fall back to FORM. Preserve
Vakint's defaults, public conventions and existing FORM-backed modes.

Vakint is the steering adapter, not another reduction engine: durable artifact
validation, guarded rule selection, recursive/memoized IBP application and exact
integral-key reduction remain implemented in the RustRed crate. Vakint owns its
existing topology matching and momentum-routing integration, selects the
shipped artifact, and reuses its existing master-evaluation machinery. Do not
duplicate RustRed's reducer or introduce a Vakint-local recurrence engine.

Local-path RustRed dependencies are permitted during development; pushed
GammaLoop milestones pin the corresponding pushed RustRed Git revision.
Report actual certified-artifact and test coverage, not completion inferred
from a source-port rule count. Independent audits and coherent intermediate
commits/pushes remain required in both repositories.

The first complete cold source-port audit now passes (2026-09-15): all 617 K6
rules replay from the nine original IBPs and descend over their exact domains,
with 38 finite terminals, 26 proved-zero sectors and zero uncovered regions.
K1/K3 likewise pass, and all semantic reports agree at one/six workers.
This is rule-set validation, not yet a serialized `ClosedArtifact` or a Vakint
three-loop acceptance pass. The integration now retains original-source
requests and exact weights, with verified conversion from adapter normalization
to freshly generated ordinary IBPs. These records now have a private checked
owner. At that first audit checkpoint, serialization through the existing
artifact/reducer boundary remained; the later durable milestone below passes it.
A fresh autonomous
release run now also generates directly from the canonical unit-mass K6 family
used by Vakint's parent slots: **623 exact rules, 38 finite terminals, 38
nonzero sectors and 26 proved-zero sectors** enter the private checked-program
owner, with zero replay, descent or coverage gaps. This differs from the 617-rule
symbolic-mass reference convention. The 1.62-second serial diagnostic includes
generation and checked ownership, not serialization, cold reload or scalar
application; concurrent compilation ran on separate cores. A subsequent
fixture-only correctness test now binds every actual retained typed terminal
set to its sector's exact 0/1 corner, covering all 38 proposed catalog keys.
That debug test is not a timing result and does not change the frozen release
binary. The subsequent milestones below now supply
the existing `ClosedArtifact` lowering and durable codec. Fresh-process release
validation now also passes below; Vakint shipping and acceptance remain. The parallel Vakint
lane is updating required native solver API calls and checking contraction
routing. The user has approved migrating the Spenso solver wrapper and adding
the private helper that retains the existing routing witness; both are now
implementation tasks rather than approval blockers. This does not authorize
an unrelated dependency-stack upgrade to Symbolica 3.0. No newly
added Vakint acceptance test is reported as passed before the current build
and its actual execution succeed.
The independently reviewed 38-key candidate terminal data now exists in
Vakint's test-only terminal module, using five existing exact MATAD/product
expressions. Fresh frozen-binary runs now pass its five native tests, six
existing RustRed adapter tests (including generated-asset byte parity), four
default/backend tests and the three new Spenso solver tests. The Vakint groups
use an invalid FORM path. Isolated execution of the six new routing tests found
missing test-only symbol initialization. A broader native run passed 22 tests
and failed one component-template test, exposing a second setup issue: its
intentional wildcard template needs Symbolica's existing construction flag.
The user explicitly approved both setup fixes, with all assertions unchanged.
Routine, unambiguous changes no longer require separate approval under the
repository's contribution guidance; genuinely ambiguous or dangerous changes
still do. Neither failing run is counted as a routing acceptance pass. The separate
offline test now also passes exact symbolic MATAD comparison at the requested
Laurent order for all 38 actual terminal keys, including common-mass restoration;
only this oracle process uses FORM.
Complete K6 scalar acceptance remains a separate gate.
No shipped K6 asset, extra numerical master table or new complete three-loop end-to-end
acceptance result is claimed.
The parametric-rule evidence migration also passes 110 focused tests and the
separate 623-rule canonical K6 regression. It removes the assumption that every
proof needs a sampled anchor. At that historical checkpoint, combined-domain
installation was deliberately disabled pending its real constructor and cell
lowering. The in-memory milestone enabled that producer; the later durable
milestone enables encoding, while shipping remains a separate gate.

### In-memory artifact and reduction milestone (2026-09-15)

The autonomous canonical unit-mass K6 program now installs through the existing
`ClosedArtifact` and is applied by the existing memoizing `Reducer`: **623
generated rules become 5,639 sign-refined rule cells, with 38 actual finite
terminal corners and 26 proved-zero sectors**. Admission replays the full
weighted original-source identities, preserves denominator and domain guards,
proves strict descent, and checks an unbounded zero-complement partition over
every sector. No sampled canary or rule count substitutes for those gates.

K1 and K3 use the same producer. K1 agrees exactly with the established reducer,
including nonunit mass restoration and cache hits; K3 agrees after projecting
its four actual terminal keys to the established symmetry-canonical basis.
The focused batch passes 269 tests, including adversarial residual and
checked-payload mutations. The separate complete K6 installation test passes
all 38 master identities and a dotted canary per nonzero sector, adding a
negative numerator power where an inactive slot exists.
Independent mathematical and executed-evidence audits pass. These are
correctness tests, not debug-build performance measurements. Evidence:
`target/spired-original-domain-full.JEvHje/`.

### Durable artifact correctness milestone (2026-09-15)

The combined original-source payload now uses the existing V5 artifact codec.
The complete canonical K6 correctness test encodes **8,911,462 bytes**, decodes
them without rerunning rule discovery, and verifies deterministic re-encoding.
It retains the same 623 generated rules replayed from nine ordinary IBPs,
5,639 cells, 38 finite keys and 26 zero sectors, and exercises decoded master
identities, nonunit-mass application and memoization. Untrusted loading replays the exact original identities and
guards once; the existing reducer's hot path remains unchanged. The coordinated
core gate passes 302 focused tests plus the explicit complete K6 durable test
and both example test suites. Evidence:
`target/spired-codec-indices-corrected.FrerFY/`. These are correctness results,
not release timing measurements or a fresh-process Vakint acceptance result.

The standalone release producer now generates byte-identical K6 artifacts at
one and six workers. Separate CLI processes cold-load both artifacts and apply
five canaries, including a dotted multi-master reduction, a negative numerator
power and symbolic common-mass restoration. The exact 38 terminal keys and 26
zero masks partition all 64 sectors. Evidence:
`target/spired-k6-producer.007TpB/run/`. Observed whole-process generation times
were 3.20 s and 1.86 s respectively; these single shared-host observations are
not a controlled speedup comparison or C++ port benchmark.

Next, the delegated Vakint lane must ship generation-once K1/K3/K6 assets
with Vakint. The parallel Vakint lane must bind actual terminal keys to its
existing exact master expressions, transport contracted topologies and
numerators through the existing routing witness, and run the full comparative
three-loop acceptance inventory with FeynKit plus RustRed and an invalid FORM
path. **Fresh-process K6 generation/loading/application passes; complete new
three-loop Vakint acceptance is not yet claimed.** This delivery work continues
independently of the source-port performance lane.

## Active reference-port assignment (2026-09-14)

The current tool-managed assignment is to port the actual C++ SpIRed
`solver::solveSector` algorithm in `vendor/spired`, using existing RustRed
family preparation and native Symbolica/numerica algebra. This supersedes
earlier interpretations of the notes wherever the reference's executable
algorithm differs. The immediate acceptance set is **all supplied one- through
three-loop (2–4PM) examples**, at equal or better release performance and with
identical or algebraically smaller rules. The vacuum examples are additional
measured workloads, not substitutes for the PM examples.

The implementation and verification sequence is in
[`docs/spired_port.md`](docs/spired_port.md). Compact search state belongs in
`solver`; existing artifact ownership and certification are separate from its
per-row hot path. Janet/Ore and the longer-term Stage 1/Stage 2 integration
objectives remain available but do not redefine this reference-port assignment.
When there is an unresolved policy conflict between existing RustRed behavior
and SpIRed, ask the user before deciding it.

Efficient bounded parallelism is part of this port, not a later afterthought:
share immutable family/source data, keep mutable search state local to live
workers, and verify deterministic equations and residuals across worker counts.
Compare release serial and multicore timings against the same C++ workload.
All CAS remains native Symbolica/numerica after public API audits; rational
polynomial reconstruction remains deferred until Symbolica provides it.

The complete supplied-input census is in
[`docs/spired_pm_acceptance.md`](docs/spired_pm_acceptance.md): PM fixtures also
require linear-cut preparation and, for some inputs, final-coordinate ordering
permutations. These are topology-generic solver capabilities; benchmark family
data stay in examples. A successful reference sector run does not itself certify
the older publication pipeline or the independence of numerical residuals.

Validated reference-port checkpoints now include complete `vac3` and
`fam1_11` workloads, including serial/multicore exact comparisons. The latter
reproduces all 802 sector rules, two preliminary cut rules, and 16 residual keys;
the remaining supplied PM examples are still part of the active objective.
See [`docs/spired_fam1_11_results.md`](docs/spired_fam1_11_results.md).

Additional complete reference workloads now include `bc4PMRad1` (856 rules,
16 residuals across 38 sectors) and selected `fam_cosmo` (15 rules and four
residuals). Exact outputs agree serially and in the bounded executor. The
full `fam1_12` and `fam1_111` manifests now pass after affine search/queue
integration and native joint polynomial normalization: 40 sectors with 1,104
nonempty rules and 132 sectors with 10,333 rules, respectively. Exact guards,
required domains, coefficients, and all 32/26 residual keys match the reference;
serial and six-worker outputs agree. One C++ `fam1_12` rule is omitted because
its required domain is proved integer-empty. The next scope is `fam1_112` and
the supplied ordering studies, plus measured parallel scheduling improvements.
These remain mandatory; source-port output is not a certified closing artifact.
See [`docs/spired_affine_results.md`](docs/spired_affine_results.md).

The first complete-manifest `fam1_112` attempt now supplies a concrete failure
census: Rust writes 420 of 436 sector files but exits with an error; isolated
reruns find three rational-chart admission failures and 13 nonlinear
exceptional-conjunction failures. The unchanged C++ diagnostic reaches 288
sectors before its 600-second cap; these concurrent diagnostic times are not a
performance comparison. Native exact rational computational charts now retain
the original integer domains and pass all established regression workloads.
A fresh full run produces 422/436 sector files. A follow-up generic sign-bound
check removes two proved-empty cases from one new sector; both newly completed
sectors now pass fresh strict native rule/guard comparisons (347 and 398 rules)
with one and six workers. All 20 release regression runs pass, including the
complete 617-rule vacuum workload. Fourteen
sectors still lack completed outputs; the reported first error is nonlinear
geometry. The next
slice now provides a standalone exact intersection API that preserves
normalized coupled equations and performs factorization-driven OR branching.
It initially passed 13 dedicated tests and all 169 solver tests. The additional full
captured census now admits all 13 original nonlinear conjunctions exactly:
two are proved empty and the remaining eleven produce 15 explicit branches,
with no unsupported remainder. Exact disjunctive queue integration now passes
all 180 solver tests, including ten new branch/chronology/fast-path regressions.
All 20 fresh release regressions pass, with 939 byte-identical mathematical
file comparisons. The subsequent complete-manifest run reaches 435/436 observed
rule sections before its 600-second cap, with 72,355 rules and 390 provisional
residual records. Only `111010100001111` lacks output; it is still solving
symbolic cases when censored, with no terminal geometry error emitted.
This remains an incomplete diagnostic, not complete-family acceptance or a
paired performance comparison.
Bounded integer scans are not treated as completeness. Neither slice is a
closed-artifact claim. Measured scheduling
tests retain the current active-first default; input-order was slower on the
tested full `fam1_12` workload. See the
[acceptance census](docs/spired_pm_acceptance.md) for the full evidence boundary.

## Preamble — user directive (verbatim)

````text
Ok, but did you not read all the markdown listed in the HANDOFF (especially in `./docs/research`)?

the goal is very ambitious, and it is to get inspiration from LiteRed2 (see `./FOR_REFERENCE_ONLY_DO_NOT_PUSH/LiteRed2`) which is a poorly software-designed software in Mathematica (you cannot run it) to generate closing parametric IBPs).

you must implement a version of that, RustRed, in fully rust and maximally using Symbolica (see public API in `./FOR_REFERENCE_ONLY_DO_NOT_PUSH/symbolica`), optimally parallelized and highly efficient, to push to up to 6-loop IBPS (using algorithms fully general in topology and loop counts) suitable in practice for example to build the parametric IBPS for the 6-loop single scale vacuum integral showing up in the 6-loop QCD beta function we aim to compute as a breakthrough eventually using the R-formulat for BPHZ renormalisation as implemented in gammaloop (e.g. see `./FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop`), so such IBPs can be used in a new reduction mode of vakint (see `./FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop/crates/vakint`) which is *pure* rust and symbolica, with numerical masters that will come from AMFlow.

Anyway this is a highly ambitious project you have to delegate multiple agents to (you're mostly just the orchestrator) with the main goal for now to be able to produce closing  parameteric IBPs for 6-loop single scale (actually no scale effectively, m\_uv can be set to 1) vacuum integrals, which will be used for the rest of the program culminating in the 6-loop QCD beta function (though all rustred algos must remain fully generic, but with dedicated lanes highly optimized for that use-cased dynamically automatically opted into).

Re-read thoroughly the HANDOFF, the existing code, and the research markdown files, and layout the goal clearly in GOAL.md (incl. this message verbatim as a preamble) and assign it to yourself.

Remember you must design a very clear and well structured professional code, highly optimized and using Symbolica for all CAS taskes (triple-check any CAS feature really is not available in Symbolica before implementing your own solution for such CAS task).

To unlock multicore use of Symbolica, you can and should use:

`export SYMBOLICA_LICENSE=`dcec4a5e#6a95649c#7dca8216-8afe-57c8-975e-03eb5e68e4ee
(not sensitive info)

Remember, before implementing in vakint the application of the parametric IBPs for performing the reduction (NEVER USE FORM) you must first focus on being able to find them for 6-loops with RustRed (first goal). Howver you can use vakint as an oracle to compare reductions (up to four-loops which is the max currently supported in Vakint). RustRed only generates the parametric IBPs, then a new tensor reduction and IBP application mode `RustRed` in vakint will use rustred to actually perform the reduction But that new mode and anything you do SHOULD NEVER USE FORM, MATHEMATICA, Sympy or anything like this. Pure RUST + Symbolica implementation here.
Also never escalate commands but find workaround if sanbox is being hit.
````

> **Runtime license note (2026-08-31):** the Symbolica license literal in the
> historical verbatim preamble above has expired. Commands requiring licensed
> Symbolica features must use the current operator-provided
> `SYMBOLICA_LICENSE` environment value; that runtime value is deliberately not
> recorded in the repository.

## Authority and staged assignment

### Gregor/SpIReD implementation directive (2026-09-08)

The authoritative implementation program is recorded in
[`GREGOR_INPUT_PLAN.md`](GREGOR_INPUT_PLAN.md). It incorporates Gregor Kälin's
case-directed SpIReD strategy into both stages below and supersedes the current
obstruction-guided search front end wherever the two conflict. Gregor is a
RustRed author.

The immediate priority is a dedicated, automatically selected single-scale
vacuum lane which closes K6 without FORM-derived rules or hidden hints, while
retaining a topology-generic architecture and the existing exact replay,
strict-descent, guard, owner-cover, and immutable-artifact authority. The first
materializer uses pruned exact Symbolica algebra. RustRed must not implement its
own rational-polynomial reconstruction; a reconstruction materializer is added
only when Symbolica exposes the required public API. Janet/Ore remains an
explicit optional or hybrid backend and must impose no work or memory overhead
on a pure SpIReD run.

The production convergence driver must follow Gregor's equality-case chronology:
its outer queue contains only canonical affine integer equalities, beginning
with `C=true`; exact rule guards enqueue the overlapping cases `C & g_i=0`;
and generic-first subsumption happens before nonzero predicates are introduced.
Disjoint first-zero predicates belong to the separate owner-cover compiler, not
to discovery identities. Within one case, ordinary sources are translated in
fair signed-L1 shells. A modular GPLU hit retains both its canonical support
identity and its dependency-topological row schedule, and the compact exact
Symbolica rerun must preserve the latter. Equality rank bounds exceptional
branch depth but not source-search depth; a bounded miss is never terminal or
closure authority.

For each coordinate case the normal logical target is the canonical zero-shift
`I(n)`. A pivot shifted tangent to the case is recovered at that target by an
opposite source translation, so fair source-shell enumeration—not a finite
walk over concrete targets—is the systematic discovery mechanism. Any
depth-wide inactive-axis tightening is provisional search geometry; published
bulk and boundary obligations are recomputed from the winning exact circuit.

The case driver must also turn coefficient-checked inactive-line activation
faces into equality obligations: a term that is descending in a vacuum
sector's symbolic interior is not forbidden everywhere merely because it can
enter a supersector at finitely many boundary values.  If the first exact
pivot is unusable, the all-row stream continues and a later dependency is
materialized with an exact root-constrained Symbolica solve; replaying the
ordinary first pivot is not an adequate post-hit search.

The equality-only worklist, exact guard-to-case bridge, and ordered modular
trace now exist as isolated checked primitives. Wiring them into the
autonomous production driver, replacing the older bounded leader-target loop,
is the immediate implementation milestone; this paragraph states required
behavior, not a completed K6 claim.

Stage 1 still ends only after K1, K3, and K6 are autonomously closed and the
FeynKit-plus-RustRed Vakint acceptance lane passes through three loops. Stage 2
then applies the same design to the complete K10, K15, and K21 vacuum manifests,
adds exact affine-integer cases if K6 did not already require them, and adopts
the collaborator's tensor technology and Symbolica reconstruction when those
become available. Reproducible Section 4.5 workloads are a release-performance
target: the initial exact-materialization implementation aims for at most three
times Gregor's matched SpIReD wall time, and the later Symbolica-reconstruction
lane aims to beat it. Opaque or domain-specific rows are not replaced by
lookalike RustRed fixtures.

The confidential working notes are available locally as `./notes-spired.pdf`.
That file is ignored by Git and must never be committed or pushed.

The preamble above records the long-term scientific motivation verbatim. The
latest approved plan in this section supersedes its sequencing and defines the
only active assignment for the primary Codex agent (`/root`). The primary
agent acts mainly as architect, orchestrator, integrator, and final verifier;
independent research, implementation slices, and adversarial audits are
delegated whenever useful.

Development is divided into two hard-gated stages:

- **Stage 1 is active:** close and publish single-scale vacuum parametric-IBP
  artifacts through three loops, implement their FORM-free scalar application
  in Vakint, and reproduce Vakint's end-to-end expectations through three
  loops. GammaLoop branch `vakint_rustred` is based on its `feynkit` branch;
  tensor numerators use the collaborator-supplied, FORM-less FeynKit tensor
  reducer before the RustRed scalar backend. The accepted Stage 1 stack must
  therefore remain FORM-less end to end.
- **Stage 2 is now explicitly authorized after Stage 1 is complete (2026-09-02):**
  finish the Stage 1 K6 artifact and Vakint through-three-loop acceptance gate
  first, then continue directly into advanced tensor integration, four- through
  six-loop closure production, and the required high-loop execution
  infrastructure. Deep algorithm research and bounded, falsifiable studies
  over the complete four-, five-, and six-loop single-scale vacuum family
  manifests remain authorized during Stage 1. These
  studies must freeze and authenticate their complete family census, report
  every censored or unresolved member, and keep modular discovery evidence
  distinct from exact closure authority. LiteRed2 is a
  correctness baseline rather than an architecture target: candidate methods
  must be judged creatively against the eventual six-loop scaling problem,
  with independent research and adversarial viability audits. This permission
  does not authorize claiming a four- through six-loop artifact before its
  exact publication gates pass; it no longer requires another permission step
  once Stage 1 is complete.

The existing experimental RustRed tensor service is frozen. The obsolete
GammaLoop RustRed-tensor commits are excluded from the rebased
`vakint_rustred` branch and are not part of the active acceptance path. Vakint's
`TensorReductionMethod::FeynKit` is the Stage 1 tensor prepass. Existing
FORM-backed scalar and tensor modes remain available for backward
compatibility and as offline oracles, but the FeynKit-plus-RustRed acceptance
lane must succeed with an invalid FORM executable path.

## Current evidence boundary

The clean workspace refactor is complete. The root is a virtual Cargo
workspace containing package/library `rustred`, `rustred-app`, and
`rustred-python`; obsolete prototype solvers, authored recurrences, and legacy
compatibility layers have been removed.

The live core compiles topology-neutral families, generates generic ordinary
IBP and LI source rows, authenticates physical and auxiliary family
presentations, verifies supplied affine symmetries, analyzes requested zero
sectors, and provides deterministic campaign primitives. Its foundry can
derive concrete-anchor and guarded fixed-sector recurrences over Symbolica
`K(n)`, select a requested physical pivot with deterministic Symbolica RREF
while keeping provenance columns free, prove uniform descent, exactly replay
source combinations, partition target-sector domains, and stream proper-
subsector obligations.

The core now freshly generates and seals the canonical unit-mass one-loop
partition and equal-mass two-loop sunset as mathematical and durable closing
artifacts. Their tagged binary representations authenticate complete source,
rule-cell, projection, symmetry, factorization, terminal, and homogeneity
semantics once at the untrusted load boundary. The sunset owner derives all
four ordinary sources, closes its generic and exceptional cells, routes exact
`S3` symmetries, and feeds its pinched face into the immutable one-loop
dependency. A topology-independent deterministic memoizing reducer applies
both sealed owners without repeating whole-artifact authentication. The Rust
application API, `campaign` CLI, and public `import rustred` Python package
generate, inspect, load, and apply those actual artifact bytes with typed
errors and deterministic output.

RustRed still does **not** close or publish the three-loop `K = 6` family and
deliberately does not own evaluated master values. Vakint now ships and lazily
loads the `K = 1` and `K = 3` artifacts on branch `vakint_rustred`. Its opt-in
FORM-free RustRed scalar backend reduces the registered one-loop tadpole,
two-loop sunset, and pinch, maps typed terminals to the existing MATAD basis,
restores a general common mass, and optionally applies Vakint's pure-Rust master
values. Before the FeynKit rebase, nontrivial scalar tests passed with an
invalid FORM path and raw and substituted results agreed with MATAD through two
loops. The branch is now rebased onto `origin/feynkit` and compiles against the
local RustRed crate; repairing and extending the post-rebase acceptance matrix
is active work, so no post-rebase parity claim is made yet. Three-loop closure
and adapter coverage remain open.

The generic closing boundary has advanced beyond finite reachability probes.
Frame-local modular supports are now admitted only after exact
regenerated-source replay, compiled into deterministic semantic guard DAGs,
and extended beyond their finite discovery envelope only by an opaque proof of
same-sector strict descent. A separately audited owner-cover compiler combines
those exact rays, proves their leading-ideal complement finite or reports the
typed obstruction `NonFinite`, `GuardIncomplete`, or
`FiniteTerminalOwnership`, and never promotes a sampled miss to a master. A
test-only fraction-free reconstruction also demonstrates on the canonical K6
S4a circuit that ten elimination-induced guards can be replaced by its one
mandatory target-coefficient guard with exact source cofactors and full column
replay; it has no production or closure authority until its measured promotion
gates pass. The first all-orbit degree-one semantic-owner-input sweep is now
measured and regression-pinned across the six full-rank K6 sector
representatives. All 17
modular nominations lift and replay exactly, but they provide only
`0, 3, 2, 2, 3, 7` semantic owner inputs (of which `0, 1, 0, 0, 1, 3`
compile as guard-total owners), and
every compiled cover remains `NonFinite`; the empty first sector is reported as
`NoSemanticOwnerInputs`, never as closure. The audited combined
degree-one/degree-two S4a sweep adds 22 exactly replayed degree-two semantic
inputs to the two degree-one inputs, with zero lift failures or exact-content
duplicates. Its one
guard-total owner reduces the maximal uncovered dimension from six to five,
but the cover remains `NonFinite` with three unbounded five-dimensional boxes
and no terminals. Blind prolongation therefore helps without closing the
sector. A second degree-one sweep now uses the exact installed K6 root
authority rather than an empty owner snapshot and declares only its retained
scalar product terminals. The rank-three path sector then yields 9 replayed
semantic owners, 4 guard-total owners, and 10 still-unbounded complement boxes;
the star yields 22, 12, and 4 respectively. Both remain exactly `NonFinite`,
with no missing terminal or guard-incomplete branch. These figures are pinned
as the starting point for the next search algorithm, not reported as closure.
The active bounded lane is a native-target-zero obstruction-guided
source-discovery oracle which nominates translated rows capable of cutting the
current modular no-hit; every nomination remains discovery-only until the existing
exact replay, semantic, descent, and owner-cover gates pass. Its first two
generic prerequisites are now implemented and independently audited: a bounded
API translates only canonical requested `(ordinary source, signed offset)`
pairs, and every modular no-hit retains a plan/sample-bound sparse right
obstruction with logical target coefficient one and exact finite-field
`A q = 0` replay. Multi-probe obstruction payloads have their own aggregate
budget. The construction-neutral seam and the first structural oracle layer
are now also implemented and independently audited. `PhysicalFramePlan` owns
only sealed exact rows, raw integral columns, CSR, scope, sector, and
provenance; `OneSidedChartFrame` retains rectangular degree metadata, while
`SelectedSourceFrame` accepts only explicitly requested signed translations
without Cartesian completion. Both paths use one validated assembler. Every
semantic DAG inherits an unforgeable identity from its exact plan, and an
outer-extension witness rejoins both that identity and the plan pointer before
it can contribute owner authority. Complete-ordinary versus external-only source-layout provenance is
sealed through both translated-batch forms and is checked before any
completeness census. A reusable Symbolica-backed coefficient evaluator is
sealed behind an admitted modular frame, so its finite-field point and domain
cannot be mismatched. Finally, the bounded inverse-incidence index now
enumerates canonical `alpha = u - s` requests from either the target-unit
bootstrap or a checked obstruction, with the canonical K6 census pinned at
nine sources, 90 term incidences, 31 distinct relative shifts, and 90 unique
bootstrap requests. These values remain nomination-only. The implementation
now evaluates every nominated complete row against the current obstruction,
retains all nonzero-residual requests, and rebuilds a fresh selected plan and
exhaustive target partition after each stable augmentation. This single-epoch
boundary is regression-pinned: obstruction nominations are sealed to the
exact ordinary-source incidence index and checked query, every complete row is
evaluated before sparse projection, and every augmentation creates a new
immutable plan, partition, and modular sample from raw identities. Candidate
batch exhaustion remains telemetry only. The same checked Symbolica RREF now
also supplies a bounded proposal block consisting of the primary `q0` and at
most three deterministic target-normalized `q0 + z_i` directions, each with
independent finite-field replay. Complete translated-row evaluations are
reused by a scope-bound probe-local cache; marginal signature-rank selection
retains a q0 cutter and one epoch-rotated breadth slot. Union materialization,
logical canonicalization, row-cache storage, lookup, insertion movement,
physical evaluation, and selection are all preflighted under per-call and
aggregate budgets. None of these proposal values can mint the residual seal,
sampled-dual evidence, a rule, or an owner. The first one-epoch width-one /
width-four K6 path-and-star comparison preserves the same authoritative q0
censuses and grows each 90-request frame to 122 requests, but makes zero exact
lift attempts and leaves the exact closure baselines at path `9/4/10` and star
`22/12/4`. This is measured discovery evidence, not a K6 closure claim.
`SampledDeclaredModuleDual` now seals
one complete guard-free fixed-sample empty census after independently replaying
the inverse-incidence and exact translated-term/support counts. It is joined to
the fresh plan, sample, exhaustive partition, stratum, ordering, immutable
owner snapshot, and incidence index; guarded strata fail closed until an exact
sample witness exists. This value has no conversion to a rule, owner, terminal,
exact no-relation result, artifact, or closure claim. Production discovery
currently constructs each task natively with target shift zero. No general
arbitrary-target coordinate transport is implemented; transport of source
offsets, columns, domains, charts, coefficients, and guards is deferred until a
typed production caller exists. A bounded outer scheduler now executes a
prevalidated probe schedule in declared order. Every probe owns its bootstrap,
request accumulator, fresh epochs, modular state, obstructions, and outcome;
only bounded scalar work counters cross probe boundaries. A live modular hit
is lifted and replayed synchronously before its query and plan can escape.
Exhaustive misses can return only sealed fixed-sample dual evidence, typed
rejection or stall, or a budget stop; aggregate exhaustion explicitly marks
the unexecuted probe suffix. No scheduler result converts into a rule, owner,
terminal, artifact, or exact no-relation claim. Replayed probe-local outcomes
can now enter one bounded canonical-replay transaction. It harvests only raw
probes and complete request identities, rebuilds bootstrap then the exact
request union as one freshly tokened epoch, verifies that its maximal domain
is contained in every contributing final domain, constructs the expensive
target/lower-owner partition once, and re-samples every probe independently on
that common plan. Old circuits never cross the boundary. Fresh exact lifts
are hard-fail on every structural/replay error, sorted by exact content rather
than modular telemetry, and deduplicated with bounded support, diagnostic,
exact-payload, and integer-bit retention. The result is still a
non-authoritative proposal batch, but a genuine distinct-request epoch-one
union and its subsequent guarded `RuleCell` promotion are regression-pinned.
The replay-to-publication boundary now pairs only globally admitted executable
cells with their exact semantic circuits, preserves epoch/circuit/cell identity
through canonical candidate and owner ordering, retains normal guard
obstructions without granting them coverage, and transactionally replaces the
whole exact owner cover. Retry probes influence promotion only through their
deduplicated exact anchors in canonical coordinate order. Finite undeclared
terminals and guard/complement gaps remain explicit `Incomplete` outcomes.
A consuming seal now accepts only an exactly `Closed` executable cover,
retains its predecessor snapshot by strong ownership, and rechecks the common
family, context, sector, ordering, and exact snapshot across every owner
without recompiling or cloning its circuits and `RuleCell`s. That sealed cover
can now be published as one immutable solved-sector layer. Publication streams
the complete executable/proof payload into one bounded BLAKE3 content identity
exactly once; the strongly retained `Arc` chain, not the digest, remains proof
authority, and subsequent owner lookup does not rehash the payload. Snapshot
extension is transactional: all sectors in one worker frontier share the
exact predecessor, are canonically ordered, and later frontiers must have
strictly greater active-line rank. Immutable snapshots now retain their exact
canonicalizer authority and publish bounded append-only raw-to-owner symmetry
routes for zero, factorization, master, and solved-sector owners. Snapshot V4
commits the complete route records and derived sector buckets; cold replay
reauthenticates every group element, while lookup preserves root-owner
precedence without allocation, CAS work, or content rehashing. This closes the
noncanonical product-sector dependency gap. A topology-neutral one-shot wave
coordinator now admits only canonical-replay-produced pointer-paired executable
owners, resolves proof-equivalent worker arrivals by an exact published-content
canonical minimum, authenticates raw terminal keys against retained root
authority, and charges one aggregate resource envelope across the whole wave.
It reports every exact incomplete-cover class without publishing and seals all
members only before one atomic same-rank snapshot extension. The K6 frontier
fixture proves that the rank-three zero orbit is already owned and stages the
path (orbit 12) and star (orbit 4) against the same 32-owner root snapshot; it
honestly stops both as nonfinite and publishes neither. The remaining
dependency boundary is still the first complete bottom-up K6 rank-three wave.

The family-level campaign boundary is now explicit and independently audited.
One authenticated three-loop six-coordinate family—not five unrelated family
definitions—owns the maximal K4/Mercedes sector and its complete physical
contraction downset. Deterministic enumeration produces all 64 raw masks and
the exact 11 `S4` sector orbits. Vakint's five registered three-loop matcher
roots are retained as mandatory coverage/replay fixtures, but they omit the
four-member full-rank star orbit represented by `[0,0,1,1,0,1]`. The installed
unimodular authority proves that omitted orbit is a `K1^3` product: the five
matcher charts cover 34 raw masks, zero authority covers 26, and the star
product covers the remaining four. Thus no sixth nonfactorized graph chart is
missing, but this combined sector accounting still cannot replace saturation
of the maximal root or prove dotted/numerator recurrence closure. This complete
manifest is an obligation census, not closure evidence. An extended compact walk reached
10,926 exact reports before a typed 64 MiB retained-content limit stopped the
next transaction; it did not close, exhaust the scheduler, or hit the report
cap. Both persistent dimension-five residual representatives already lie in
the complete path sector and see all nine ordinary IBP sources. Offline
AlphaLoop/MATAD diagnosis shows that their missing first operation is a
guard-free affine loop-routing expansion of an inactive numerator, followed
by ordinary IBPs, rather than another graph class or another ordinary source.
The topology-neutral, Symbolica-backed cold compiler for that
factorized-numerator lift is now independently audited. It binds installed
factorization rules to their exact family, derives pure routed-key or
constant-width auxiliary actions for all three installed K6 factorization
classes, and explicitly proves only auxiliary descent. A separate cold,
non-owning expansion service now delegates finite affine powers to Symbolica's
native sparse-polynomial implementation and exactly coalesces the resulting
ordinary keys. It materializes the two persistent path representatives into
28 and 210 deterministic endpoints and replays the one-factor recurrence
exactly for both the path and `K3 x K1` charts. Endpoint payload,
canonicalization-orbit work, exact-coefficient policy, full `i64` shifts, and
compiled-routing provenance are all admitted explicitly. This service still
creates no `RuleCell`, owner, terminal, artifact, persistence payload, or
reducer dispatch. Its current auditable native-power boundary accepts only
parameter-independent affine coefficients; parameter-dependent coefficients
remain a typed unsupported case until their native coefficient-term work and
output can be pre-admitted exactly.

Independent follow-up audits identified and have now implemented a stronger
cycle-free consumer than installing those endpoints as raw parent-family
actions. The path and star factorizations are both `K1^3`; their authenticated
radial-preserving product-angular programs reduce admitted dot/numerator
domains directly to the installed product master. The `K3 x K1` program peels
its independent one-loop block, retains the correlated two-loop scalar
polynomial, reduces it through the closed `K3` and `K1` dependencies, and
terminates in its two installed parent-master embeddings. The installer derives
the exact sparse dependency-root preimage from the authenticated block basis
and lower artifacts, retains it in immutable symmetry-routed owner snapshots,
and cold-regenerates the executable program at load. Runtime execution uses the
parent reducer's dependency instances, memo table, request-wide application and
coalescing budgets, and exact coefficient accounting. It preserves routed row
signs, radial powers, and the indexed `d+R-2 != 0` guard family, and rejects
products with multiple correlated multi-loop blocks. This is scalar
factorization logic over typed integral keys, not the frozen general
tensor-reduction project.

The current implementation is the audited successor to the earlier bounded
prototype, which lacked owner authority, persistence, shared memoization, and
shared budgets. Exact preimages correctly reveal coupled endpoint fringes that
the former rectangular hull had overclaimed; those valid arbitrary-rank
obligations remain for ordinary-source discovery rather than being removed by
narrowing the certified root. Until all ordinary sector waves close those
fringes and pass cold artifact replay, this product lane grants neither a
closing K6 artifact nor Vakint three-loop parity by itself. Historical
feasibility numbers remain research evidence only.

Vakint's matcher roots have now been tested in their strongest useful role as
natural sector-local coordinate charts, not concatenated into a new
denominator family and not treated as closure authority. A deterministic cold
fixture derives all five roots from the one complete K6 contraction plan,
applies each shape-checked unimodular witness as `q = T k`, and builds
six-coordinate ISP completions in those routed coordinates with foreign family
fingerprints. Symbolica inverts and congruence-transforms the maps; every
completed local row is then routed back, converted to the parent denominator
basis, and replayed exactly, with each physical row required to recover its
stable parent slot. The roots cover five of the six full-rank `S4` orbits, or
34 raw masks; the four-member
star orbit `[0,0,1,1,0,1]` remains explicit and is owned structurally as a
`K1^3` product. Together with the 26 raw scaleless masks, the portfolio plus
terminal authority accounts for all 64 physical masks. The S5 chart independently
replays `2 s23 = 1 + D2 + D3 - D6`. These charts can precondition the bounded
source search in the S4 and S5 waves by making correlated parent translations
sparse locally, but the current fixture proves exact routed construction,
feasibility, and authority separation only. Its caller-supplied fixed-degree
admission limit is a resource policy, not a rank cap. The charts add no IBP
identities, cannot transport positive auxiliary poles or arbitrary symbolic
numerator powers as finite key sums,
and may only nominate parent K6 source requests. Every candidate must be
regenerated, exact-lifted, and replayed in the parent family before promotion.
The complete 64-mask/11-orbit contraction plan remains authoritative.

On the Vakint side, the shared multimethod harness now exercises 21 applicable
historical tests comprising 27 concrete inputs through two loops, including
nine tensor-bearing inputs. After the `feynkit` rebase those inputs must
explicitly select the FeynKit prepass; the post-rebase matrix is currently
being repaired and revalidated. Eleven genuine three-loop oracle fixtures and
five matcher-class fixtures remain executable but honestly ignored until the
certified K6 artifact and terminal catalog exist.

The parallel high-loop research backlog is frozen in the
[high-loop proposal experiments](docs/research/high_loop_proposal_experiments_2026.md): five
K6-controlled, staged `K = 10`, `K = 15`, and `K = 21` measurements of syzygy/minor reuse,
certificate-driven row selection, probe-local modular throughput, symbolic lowering proposals,
and guarded finite quotients. They cannot authorize a high-loop artifact, replace the sampled-dual,
exact replay, guard, descent, or immutable-owner gates, introduce Stage 2 execution infrastructure,
or require a minimal master basis.

An independent exact K6 dependency census now fixes the bottom-up pressure
order without introducing topology names into the generic core. None of the
six full-rank sector orbits is zero-owned. The three-line path and star are
first; the factorized triangle-plus-pendant four-line orbit depends on both,
while the inequivalent four-cycle depends on the path; the five-line orbit
depends on both four-line orbits; and the top orbit depends on the five-line
orbit. Existing factorization ownership covers active dots only when every
inactive power is exactly zero, so all six decorated sectors still require
rewrite layers for inactive numerators. The circular-authority blocker is now
removed without fabricating K6 closure: a crate-private terminal authority
replays the generic zero and factorization validators once, requires its
terminal manifest to equal the canonical compiled factorization images, binds
its symmetry action to the exact family, and strongly owns the closed K1/K3
dependencies. `ImmutableOwnerSnapshot` retains that exact authority while
flattening only its proved terminal cover for cheap lookup and cold
verification. The K6 fixture caches the seal once and no longer constructs a
synthetic `ClosedArtifact` or fake validation witness. An independently audited
bootstrap census now runs every one of the six full-rank orbit representatives
against three declared primes, for 18 probe-local tasks. Each first epoch has
90 selected rows, 253 physical columns, and 918 structural entries. At the
declared sample every forbidden rank and target-augmented rank is 90, so no
bootstrap support is falsely lifted; the checked modular obstruction instead
nominates 3,586--3,822 nonzero residual requests depending on the sector. The
census measurement stops at its declared one-epoch research limit and claims
neither a rule nor closure. The generic scheduler can now continue beyond
that measured first epoch without reusing stale geometry: every successful
augmentation must be a strict
canonical request superset, rebuilds the physical plan and exact maximal
stratum, authenticates the first frame against its declared anchor and every
later frame against its immediately preceding domain, and commits state only
after the entire fresh epoch succeeds. A separately sealed boundary now lowers
one live-plan-bound, exactly replayed circuit into the existing
`ParametricRule` plus direct `SourceViewBatch` representation. It remaps compact
source rows and shifts, preserves every guard origin, replays every physical
column, proves fixed-sector and sector-monotone descent, and re-admits the
payload under the caller's resource policy. It deliberately creates no
`RuleCell`, owner, terminal, artifact, or closure authority. The next K6 step
is therefore to derive bounded symmetry routes from the strongly retained
exact family action, including the noncanonical images of the three canonical
factorization domains, then replace the broad residual frontier with compact
certificate-/syzygy-/generating-function-guided batches. Successful exact hits
pass through the implemented guarded executable-owner compiler and one-time
content publication; complete same-rank waves are installed as immutable
lower-sector feedback before advancing bottom-up.

The three-loop search nevertheless now starts from an exact, test-only
pressure manifest rather than an informal topology list: it authenticates the
six-denominator unit-mass family, all nine ordinary sources, and the complete
order-24 `S4` edge action and eleven sector orbits. It freezes an internally
validated five-class Vakint routing snapshot with its exact upstream revision
and source-blob provenance; live cross-repository matching remains an
integration gate. Generic factorization tests separately certify both the
`K3 x K1` sector and both inequivalent `K1 x K1 x K1` spanning-tree sectors.
The first exact top-sector recurrence is derived from the complete nine-source
span as a test-only rule cell with retained provenance, guards, application
bounds, and strict-descent proofs. Exact residual projection on the canonical
five-line face additionally derives the two inequivalent positive dotted-edge
cells from all nine sources, retaining the complete 26-sector zero routing and
strict-descent evidence. Six exact generated-source cells now partition the
negative inactive-power direction into disjoint endpoint and bulk owners on
the all-unit active-power, adjacent active-dot, and opposite active-dot
domains. Their exact endpoint pruning, held-out replay, machine bounds, guard
domains, and `S4` routing are pinned. This advances but does not close the
five-line sector: its scalar corner and the remaining fixed-point branches stay
open. On the canonical irreducible
four-line face, the same exact projection boundary now derives a guarded
canonical-dot multi-excess cell from one target-aligned translated source span
and a canonical mixed numerator/dot cell from the untranslated span. Exact
`S4` tests route all four dotted and all eight mixed placements; the pure-dot
cell's `n1 - 1` guard excludes its isolated corner. A complete depth-one search
also derives endpoint and bulk cells for `J(0,1,1,1,1,n)` over every
representable `n<0`. Independent five-row reprojection proves the bulk through
the `i64::MIN` target; its pinched numerator children remain open instead of
being mislabeled as factorization terminals. A fixed-corner residual
projection now lowers that isolated dot to the scalar corner with the exact
rows `ordinary-ibp:0:0` and `ordinary-ibp:1:0`. A one-dot translated
projection of the complete nine-row ordinary-source layer supplies a
recurrence for the opposite two-dot orbit; exact RREF selects five rows and
produces three strictly descending right-hand-side terms. Those terms remain
subject to the surrounding fixed-point obligations. Exact global
canonicalization covers all four isolated-dot placements and both raw
opposite-pair placements. A generic bounded same-sector search planner now
constructs complete deterministic L1 translation diamonds with exact resource
preflight. A separate topology-neutral reachability planner now applies
ordered rule cells exactly over finite concrete root graphs: terminals precede
rules, guards and coefficients are specialized exactly, zero branches are
dropped, raw descent is proved before optional symmetry routing, and the
deterministic uncovered frontier carries typed work/storage counts. It is a
discovery census, never an infinite-domain closure witness. At the four-line
corner, depths zero and one retain typed target
misses, while depth two contains 28 offsets and the complete 252-row translated
ordinary-source span. Exact targeted RREF selects 16 rows and yields the
adjacent-pair recurrence

\[
J(0,1,1,2,2,0)=
-\frac{J(0,0,2,2,2,0)}{4(d-4)}
+\frac{(d-3)(3d-8)(3d-10)}{64(d-4)}J(0,1,1,1,1,0),
\]

with nine retained exact guards, complete projection replay, strict descent,
and exact `S4` routing of all four raw adjacent placements. The first RHS term
is the already certified spanning-tree `K1 x K1 x K1` dependency. The same
complete diamond separately yields the first deeper endpoint required by the
opposite-pair recurrence,

\[
J(0,1,1,1,3,0)=
\frac{3}{8(d-4)}J(0,0,2,2,2,0)
+\frac{(d-7)(3d-8)(3d-10)}{128(d-4)}J(0,1,1,1,1,0).
\]

Its exact targeted RREF again selects 16 rows, retains nine guards and the
complete 252-row projection replay, and descends only to the certified path
factorization and the prospective scalar four-line terminal. A third target
selection on a separately retained copy of that complete span exhausts the
remaining unit-dot decoration orbit,

\[
J(0,1,2,2,2,0)=
\frac{38-11d}{32(d-4)}J(0,0,2,2,2,0)
+\frac{3(d-3)(d-2)(3d-8)(3d-10)}{512(d-4)}J(0,1,1,1,1,0).
\]

This exact cell selects 17 RREF rows, retains nine guards and the complete
252-row replay, and routes all four raw three-distinct-dot placements under
`S4`. A fourth complete depth-two projection now derives a parametric rule on
the selected repeated-edge ray `J(0,1,1,1,N,0)` for every structural target
`N >= 3`. The pivot shift `[0,0,0,0,2,0]` is obtained from 50 selected source
contributions (358 source terms) and has eight RHS terms, 32 exact guards, and
367 replay keys. Schema-V4 replay uses 1078 exact operations at free index one
and 1080 at held-out indices two and eight. After fixed-coordinate
specialization, a uniform-sign leading-in-`d` coefficient proof shows that no
guard is identically zero in `d` at any positive free index; individual
exceptional dimensions remain guarded. Exact `S4` canonicalization routes all
four choices of repeated active edge.

Two further independently retained copies of the complete depth-two
four-line-corner span derive exact singleton recurrences for the two
inequivalent placements of powers two and three. The adjacent target
`J(0,1,1,2,3,0)` selects 17 generated source contributions containing 105
terms; the opposite target `J(0,1,2,1,3,0)` selects 18 contributions containing
113 terms. Each has two strictly descending children, nine guards, complete
252-row residual-projection replay, and exact schema-V4 concrete replay. All
twelve ordered placements route into the two cells under the authenticated
`S4` action. The remaining deeper mixed-dot points and all numerator faces
remain open.

A complete depth-three search then spans 84 translations and all 756 generated
ordinary rows for the exact corner target `J(0,1,2,2,3,0)`. Its exact
elimination selects 46 rows. Those generated rows are independently
retranslated and reprojected on a one-free-index face, producing a guarded
recurrence for one `S4` orbit of `J(0,1,2,2,N,0)`, structurally `N >= 3`.
The parametric rule uses 13 source contributions containing 90 source terms, has five RHS
terms, seven guards, 96 replay keys, and 275 exact schema-V4 replay operations.
The anchor free index one and held-out indices two and eight reproduce the same
exact metrics; a uniform-sign leading-in-`d` proof establishes that no guard is identically zero
for any positive free index. Its concrete i64 application box owns
`3 <= N <= i64::MAX - 1` and rejects the overflowing final endpoint. The
complete 756-row free-index projection itself retains the typed
exceptional-anchor diagnosis instead of silently selecting a
different rule. A separate complete depth-three projection derives the first
complementary-orbit singleton `J(0,1,2,3,2,0)` from 46 selected contributions
containing 310 source terms, with four RHS terms, 22 guards, 315 replay keys,
939 exact concrete replay operations, and typed target absence through depth
two. Its fixed application box and exact 16/8 `S4` orbit split prevent a ray
overclaim. The rest of that complementary ray and the first exposed descendant
ray `J(0,1,1,2,N,0)` remain explicit obligations. The fixture exposes no
installable artifact until the complete rule fixed point is closed.

Two generated three-line path recurrences now continue the scalar four-line
inactive-numerator child. Disjoint endpoint/bulk cells first lower one exact
`S4` orbit of `J(0,0,2,n,1,1)` and then the undotted
`J(0,0,1,n,1,1)` lane for every representable `n<0`. Their complete depth-one
source spans, algorithmically selected machine-safe rows, full-i64 replay,
guards, descent, terminal routing, and symmetry boundaries are pinned. The
decorated path has five inequivalent `S4` orbits; only the certified one is
owned and the other four remain explicit closure obligations.

The complete untranslated nine-row span also derives disjoint endpoint/bulk
cells for the factorized bridge-dot numerator orbit
`J(0,n,2,1,1,1)`, `n<0`. Independent compact reprojection retains five and six
production sources, respectively; both final cells are guard-free, replay
through target power `i64::MIN`, and descend strictly. The endpoint terminates
in two authenticated factorization sectors. The bulk replaces its mixed
dot/numerator frontier with one decorated-path and one undotted
factorized-face obligation, without promoting either to a terminal.

The decorated bridge descendant `J(-1,0,1,0,2,1)` now has its own exact
singleton cell. The complete untranslated nine-row span selects rows 0 and 3,
and production independently reprojects only those rows into a guard-free,
strictly descending recurrence. Its 24-image `S4` orbit is disjoint from the
other four decorated-path placements. Both children already route to the
installed decorated-path endpoint or factorization owner 2, so this cell
reduces the finite frontier by one without creating another obligation. A
candidate bulk mixed-numerator lane that would increase that frontier remains
deliberately uninstalled.

The remaining direct bridge-bulk child `J(0,-1,1,1,1,1)` is likewise owned
only at its exact endpoint. A complete depth-one search retains all 63 rows,
selects eight, and independently reprojects those eight for production. The
sole guard is `d-1`; exhaustive decoration of every inactive edge over the
12-image scalar-sector orbit reproduces exactly the endpoint's 24-image orbit.
All three children route to factorization owners 2, 0, and 2, so this second
singleton also removes one frontier node without creating another. No
negative-power bulk is inferred.

A third exact endpoint owns only the `S4` orbit of
`J(0,-1,2,2,1,1)`. Its complete depth-one search again retains all 63 rows and
selects nine; production independently reprojects those rows and eliminates
the complete-system's spurious `d-1` guard. All four children are immutable
factorization terminals with owner ordinals 2, 0, 1, and 2. Exhaustive
classification of the six inequivalent two-dot/numerator placements proves
the singleton boundary and keeps the neighboring bulk and higher-dot lanes
open.

The irreducible four-line numerator cells now live under the semantic
`four_line::numerator` module. A complete depth-one search derives an exact
singleton for `J(0,1,2,2,1,-1)`, the placement where the inactive numerator
is incident to both active dots. It selects complete ordinals 18, 21, 27, 28,
and 30; production independently reprojects those five rows and removes the
complete elimination's spurious `6-3d` guard. Exact `S4` enumeration proves
three inequivalent placement classes and owns only this one. Its four children
route to the installed adjacent-pair and triple-dot cells, factorization owner
2, and the unresolved scalar four-line corner, so the finite frontier shrinks
without acquiring a new node.

The same semantic numerator module derives exact endpoints for the opposite
inactive-numerator pair `J(-1,1,1,1,1,-1)` and its one-dot child
`J(-1,1,1,1,2,-1)`. The first independently reprojects four selected rows
from a complete 63-row depth-one search and retains the single effective
`3d-4` guard; the second reprojects two selected untranslated rows and is
guard-free. Both expose `J(0,0,1,-1,2,1)`. A separate guard-free depth-zero
three-line endpoint selects ordinary rows 0 and 3 for that shared child and
routes only to the installed undotted-path cell and factorization owner 2.
Exact `S4` partitions keep all three singleton domains disjoint from the
remaining numerator placements, and the coordinated cluster adds no uncovered
descendant.

The irreducible scalar four-line face now also has a guard-free bulk owner for
the complete machine-wide ray `J(0,1,1,1,2,N)`, `N<=-2`. Its depth-zero
search starts from all nine ordinary rows and independently reprojects selected
ordinals 0, 3, and 4 over `i64::MIN+1<=n5<=-1`, so the target reaches
`i64::MIN`. The exact pivot, source, and right-hand-side coefficients are
replay-pinned; strict descent routes every child to the existing scalar
numerator or decorated-path recurrences, except for the already-open scalar
corner at the endpoint. Exact `S4` ownership covers only the one-dot/inactive-
numerator orbit and rejects its endpoint, higher-dot, two-dot, and two-negative
neighbors.

The opposite inactive-pair endpoint now continues through the exact bulk ray
`J(-1,1,1,1,1,N)`, `N<=-2`. A complete depth-one span selects five generated
rows and independently reprojects them over the machine-safe domain. The new
child is handled by a coordinated three-line path cluster: exact depth-one
rules own the two inequivalent inactive-pair placements
`J(0,-1,1,N,2,1)` and `J(-1,0,2,N,1,1)`, while an untranslated six-row rule
owns `J(0,0,1,N,1,2)`, `N<=-2`. Complete-versus-compact selection witnesses,
guards, full-i64 endpoints, descent, and exact `S4` nonownership are pinned.
These are regression fixtures for the systematic completion work, not a
sample-driven closure representation.

The first K6-specific consumer of that generic planner now runs a deterministic
test-only fixed census. Its 115 submitted probes reduce under exact `S4` to 44
roots and discover 89 nodes: 46 rule cells are registered and produce 53
applications, 27 nodes terminate through independently checked zero or
factorization proofs, and nine remain explicitly uncovered. Three are scalar
corner certification obligations and six are genuine recurrence witnesses;
their exact inventory is frozen in the breakthrough research note. The census
checks first-applicable overlap ownership and never labels the
scalar top, five-line, or four-line corners as masters. It measures the present
frontier; it does not weaken the required zero-uncovered fixed-point
publication gate.

The first exact completion-geometry prototype maps every one of those 46
cells into the corresponding sector-local nonnegative lattice. Exhaustive
`3^6` membership comparisons per cell (33,534 comparisons in total) agree
with `RuleCell::assignment_for_target`. Exact Symbolica expansion in the base
parameters turns every retained guard into simultaneous integer-polynomial
coefficient equations. Of 205 guard occurrences, 119 have an immediate
nonzero constant equation and the remaining 86 depend on exactly one index;
exact GCD, factorization, and replay find every common integer root. None lies
inside its owning application box, so the current 46 cells have no unowned
guard-zero branch. The mapping still keeps guards separate from structural
coverage and leaves all 276 outer coordinate endpoints as explicit extension
obligations. Of these, 61 reach a maximal rule-safe application endpoint but
only 35 actually touch the `i64` chart carrier; neither condition is treated
as a proof of mathematical infinity. On the two sectors containing the six recurrence
witnesses, the exact guard-blind structural complements contain respectively
20 and 32 disjoint boxes after subtracting 7 and 19 rule boxes. Both retain
six-dimensional varying boxes and more than one million carrier points. This
is a lower bound on the true uncovered set and a precise diagnosis of missing
all-rank coverage, not a terminal count or closure claim.

The physical-frame and modular-discovery halves of the next bounded completion
experiment are now executable test-only prototypes. The frame planner
deterministically regenerates the complete one-sided degree-one through
degree-three plans for `S6`, `S5`, and both `S4` sector representatives from
the nine ordinary sources. In particular, the `S4a` degree-one plan has 63
translated rows, 157 raw physical columns, and 630 structural entries. Raw
shifts alone enter the checked CSR pattern; exact source/translation provenance
remains a row sidecar, and no `S4` quotient is taken.

The A0 modular kernel validates an odd prime before constructing Symbolica's
`Zp64`, maps sector-chart coordinates to the actual signed indices, evaluates
coefficient numerators and denominators separately, rejects vanishing source
conditions, and drops only sampled numerator zeros. Every target receives its
own `[F_b | b]` rank query. Pattern-only `L` plus coefficient-valued `U` fill
is measured and subject to the registered 20-times-input gate. Provenance
columns never enter the physical rank, and a modular miss remains explicitly
inconclusive.

The exact decorated-stratum and lift boundary is now executable. Every raw
physical column is classified exactly once as target, allowed strict descent,
or forbidden. An allowed column may cross into a proper subsector only when
every exact child cell is covered by a proof-backed zero, factorization, or
master terminal frozen from an immutable sealed artifact; ordinary RuleCells
are deliberately not treated as closure owners. A positive modular support is
bound to its physical frame and sample, lifted through Symbolica's exact
`[F_b | b | identity]` reducer, and independently replayed over all raw
columns. The retained circuit includes its translated-source combination,
pivot and denominator guards, stratum/snapshot identities, strict-descent
witnesses, and lower-owner dependencies. Synthetic controls and a genuine
nonempty degree-one `S4a` circuit pass; support that does not lift remains a
typed inconclusive result.

The bounded multi-prime evidence scheduler is also executable. It admits one
finite declared probe plan only after odd-prime, arity, canonical finite-field
sample-identity, and aggregate retained-diagnostics checks. Discovery and
HeldOut roles cannot alias the same modular point under different integer
representatives. Every task retains exactly one of RejectedSample,
RejectedQuery, ModularNoHit, or Hit. Discovery hits are grouped only as
source/pivot-trace telemetry, the largest group selects one deterministic
original hit for exact lift, and HeldOut disagreement can only mark that trace
unstable. It cannot invalidate a replayed exact identity or turn agreement
into closure evidence. Cross-prime coefficients are never combined. Synthetic
controls and a genuine K6 `S4a` target pass this complete schedule.

The first exact guard-refinement gate is now executable but remains test-only.
It rebinds every circuit to the exact frame, target, parent stratum, forbidden
columns, and immutable lower-owner snapshot; canonicalizes guards to
authority-tagged Symbolica primitive integer associates; reuses known nonzero
branches; blocks known-zero proposals; and partitions unknown atoms into one
all-nonzero child plus a deterministic disjoint first-zero chain. Only the
all-nonzero child retains the circuit. Exceptional children carry neither
partition nor owner, so they restart discovery. Aggregate count/reference/
identity limits and conservative arbitrary-integer/sparse-payload envelopes
fire before allocation. Independent audit found no false-closure or
non-disjointness path.

This still does not complete A0 or install a new RuleCell. The eager syntactic
partition is a sound fallback, not the intended scaling representation. A
first test-only semantic compiler is now executable: it performs the exact Ore
target pullback `n -> n - target_shift` before asking Symbolica to split a guard
over the declared algebraically independent base parameters, retains the
simultaneous primitive coefficient-generator set without claiming radical
canonicality, removes literal-unit ideals, and compiles priority-ordered
candidate conjunctions into a bounded reduced decision DAG. Full structural
equality, rather than a hash value, controls sharing. Its exhaustive small
truth table, forced resource caps, and the 14-atom shared-wall K15 proxy pass.
Stable candidate IDs define priority, must be strictly increasing, and are to
be assigned only after the deferred deterministic content sort; branch
predicates are queried lazily along the selected path. Each atom now retains
the least exact primitive full-guard representative seen for its coefficient-
ideal identity, and exact routing specializes those predicates itself at one
context-bound index assignment under cumulative predicate, input-term, and
specialization power-call caps. Per-predicate integer-bit limits remain in the
indexed algebra; a cumulative path bit-volume cap is still required before
untrusted production use. Independent re-audit found this generic same-context
branch semantics sound; it does not bind a physical parameter fibre.
Every residual leaf is typed `Incomplete`, and candidate leaves are discovery
routing results rather than RuleCells, terminals, or closure owners. Physical
parameter relations must be specialized or reduced before this split; a
generic-field nonzero result is not authority after an arbitrary later
specialization.

The caller-supplied Boolean oracle remains only for exhaustive compiler tests;
it has no admission authority. Production promotion must additionally persist
the physical-fibre signature, reject every reachable `Incomplete` branch
outside the separately proved finite terminal tail, and bind the selected
circuit/rule payload—not merely its candidate label—to that same point.
Logical-object caps are not yet a complete peak-RSS envelope, and no
algebraic-implication or radical-equivalence pruning is claimed.

The next production step is full rule-construction replay on admitted semantic
strata. Completion state remains separate by
sector, fixed/free coordinates, application box, and guard branch. Exact
all-rank coverage must then be proved by a finite owner cover rather than by
the present `i64` carrier endpoints. A finite strictly descending rewrite
partition may close on an affordable nonminimal typed terminal set without
constructing a minimal quotient or all shift-action matrices.

The MATAD oracle fixes the eventual in-family basis boundary without being used
as a rule generator. The scalar six-line and four-line corners map directly to
`miD6` and `m_uv^4 miBN`, respectively. The scalar five-line corner is a third
independent RustRed terminal but is not identical to MATAD's `miD5`: MATAD's
definition includes a massless `1/p^2` auxiliary denominator outside the fixed
all-massive K6 lattice. Exact raw MATAD oracles for three symmetry-equivalent
missing-edge representatives fix the unit-mass basis-change row

\[
T_5(d)=
\frac{4\,\mathrm{miT111}\,\mathrm{Gam}(1,1)}{(d-3)(d-4)}
+\frac{3(d-4)}{2(d-3)}\,\mathrm{miD5}
+\frac{8-3d}{8(d-3)}\,\mathrm{miBN}
+\frac{16\,\mathrm{Gam}(1,1)^3}{(d-2)(d-3)^2(d-4)^3}.
\]

Vakint will own this exact row and restore the common physical factor `m_uv^2`;
the degree-six denominator factorization above is independently checked with
Symbolica. These three typed candidates are not installed as artifact masters
until the remaining numerator fixed point and publication checks are complete.

The bounded scalar/odd/rank-two RustRed vacuum projector and its optional
Vakint adapter are existing experimental capability. They do not establish
rank-generic tensor reduction and are frozen for Stage 1.

## Stage 1 objective

Build a production-grade, topology- and loop-count-independent offline rule
foundry, written entirely in Rust and using GMP-enabled Symbolica as its sole
CAS. For the active pressure domain it must derive guarded, strictly
descending, exactly replay-certified, coverage-closed replacement systems and
persist them as deterministic reusable artifacts.

Stage 1 freezes a matcher-derived manifest with all eight Vakint graph classes
through three loops:

| Family artifact | Coordinates `K` | Ordinary sources | Required Vakint coverage |
|---|---:|---:|---|
| one-loop tadpole | 1 | 1 | the one-loop class |
| two-loop sunset | 3 | 4 | the sunset and its pinch |
| three-loop K4/Mercedes | 6 | 9 | the parent and four inequivalent contractions |

One sector-complete unit-mass artifact is produced for each row of this table.
The `K = 6` artifact must prove coverage of all five registered three-loop
classes; topology names are manifest labels and fixtures, never algorithmic
dispatch keys. Pinches, symmetries, factorization, and routing are handled
through generic proved transformations.

All production algorithms remain generic in topology and loop count, including
the Rust library, `campaign` CLI, and public Python package. Python users write
`import rustred`; `rustred._rustred` is private and top-level `import _rustred`
is unsupported. Generic non-vacuum family construction and identity generation
remain first-class even though Stage 1 closure pressure is the vacuum manifest.

### Unit-scale contract

The closing search and shipped tables use a proved common squared mass
`s = m^2 = 1`. For `L` loops and powers `a`,

```text
I(a; s) = s^(L*d/2 - sum(a)) I(a; 1).
```

Consequently the coefficient restoring a unit-scale reduction from target
`a` to master `b` is

```text
c[a -> b](s) = s^(sum(b) - sum(a)) c[a -> b](1).
```

This includes negative auxiliary powers. Specialization requires authenticated
single-scale homogeneity and nonzero scale evidence; dimensional analysis does
not excuse a convention or routing mismatch.

## Definition of closure

RustRed distinguishes mathematical in-process closure from durable production
publication. A root may become the immutable in-process `ClosedArtifact` only
when it establishes items 1–6 below. It may be called a published or production
artifact only after it also establishes item 7:

1. The exact family, coefficient/index contexts, kinematics, metric and
   propagator conventions, routing, cuts, power shifts, ordering, and freshly
   generated source set are bound to one canonical identity.
2. Every required ordinary IBP and LI identity is generated generically. For
   `L` loops and `E` external momenta,
   `K = L(L+1)/2 + LE`, with `L(L+E)` ordinary sources and `E(E-1)/2` LI
   sources.
3. Every rule carries exact integer-domain and nonzero-polynomial guards,
   strict well-founded descent, source provenance, and a zero residual against
   freshly regenerated identities.
4. Zero, symmetry, cross-family maps, factorization, product structure, and
   proper-subsector dependencies are proof-bearing.
5. Every generic and exceptional branch reaches a descending rule, an already
   closed dependency, an explicitly enumerated master, or an independently
   certified zero/product/factorized terminal. A residual or failed search is
   never a terminal.
6. Solved dependencies feed back immutably until the reachable dependency
   graph reaches a deterministic fixed point with no uncovered, unsupported,
   resource-limited, interrupted, or unresolved leaf.
7. The deterministic durable representation is bounded and validated once at
   the untrusted load boundary before conversion to the sealed owner; the
   reduction hot path does not repeat whole-artifact authentication.

Finite-field or numerical samples may propose candidates, but exact
regenerated-source replay is mandatory.

Closure does not require a *minimal* master basis. If exact all-rank coverage
proves that the residual complement is finite, RustRed may publish those
finitely many keys as an explicit, versioned set of evaluation terminals.
Merely observing finitely many misses in a bounded census is not such a proof:
the misses may lie on an uncovered infinite ray or algebraic guard locus. For
Stage 1, every accepted nonminimal terminal must additionally carry either an
exact basis-change row to Vakint's existing MATAD masters or a separately
validated, shipped high-precision Laurent evaluation. At higher loops, a
finite nonminimal set may instead receive precomputed AMFlow values. Minimality
is an efficiency and canonicalization objective, not a closure requirement.

For the eventual six-loop programme, favorable closure scaling takes priority
over reproducing a historically minimal basis. Every campaign therefore
records a terminal budget: terminal count, simultaneous evaluator feasibility,
required precision and storage, and numerical conditioning. The finite
universal terminal set must remain small enough to evaluate once at very high
precision and ship, but it need not be minimal. At three loops, Vakint's MATAD
mode is an authorized offline oracle for discovering exact relations missing
from RustRed's current rule set and for producing high-precision reference
values. Oracle output may guide RustRed seed points, sector waves, coordinate
orderings, resource schedules, and validation targets. The production
`K = 6` artifact, its CLI/Python example, and its claimed generation time must
nevertheless come from identities independently generated and exactly replayed
by RustRed: they must not import, translate, serialize, or package existing
FORM recurrence right-hand sides. No FORM-recurrence importer currently
remains: the only retained oracle fixture contains LHS/domain/order metadata,
is test-only, and has no publication authority. FORM/MATAD never enters the
production RustRed scalar path.

K6 discovery is pursued through two independently reported campaigns. The
`hinted` campaign may consume reviewed external metadata such as candidate
integer seed points, coordinate orderings, domain itineraries, and resource
wave schedules inferred from FORM/MATAD behavior. It may not consume a
recurrence right-hand side, coefficient, reduction result, or pre-solved rule:
RustRed regenerates and exactly replays every ordinary source identity. The
`autonomous` campaign starts only from the family, graph/routing/symmetry data,
zero and factorization authorities, and generic campaign limits; it must
discover its own seeds, ordering, and itinerary. Both campaigns emit the same
standalone artifact schema, and after generation neither artifact records or
requires FORM hints. Both must independently pass closure, strict descent,
serialization/reload, and representative reduction checks. The hinted lane is
an earlier existence/debugging milestone, never a substitute for autonomous
closure.

After every coherent foundry implementation slice passes its focused safety
gates, rerun both campaigns from a release build and record their exact
uncovered geometry, resource stop, and execution time. These periodic full-K6
grounding runs are required even when a slice is primarily architectural: the
next slice must be selected from measured surviving rays, and a zero-uncovered
result is accepted only after deterministic serialization and cold reload of
the resulting artifact.

The provisional direct-numerical-basis budget is green only for at most 100
terminals whose largest measured auxiliary AMFlow system has dimension at
most 100. Counts through 1,000 terminals and system dimension 300 are
conditional on a successful simultaneous-evaluation pilot; larger proposals
return to completion or compression unless measurements justify a reviewed
exception. Exact finite closure alone does not prove AMFlow computability.
AMFlow's recursive construction is valid in principle but enters auxiliary
lower-loop propagator or multiscale families and assumes their linear/IBP
reductions are available. Every proposed AMFlow campaign must therefore name
the reducer at every recursion node; a vacuum-only RustRed artifact is not by
itself that reducer. A RustRed-derived difference-equation/factorial-series
evaluator or another audited high-precision method may be used instead after
K6/K10 oracle validation.
Every pilot must also bound accumulated `(d-4)` pole depth with index rank,
because an unbounded spurious-pole order would require an unbounded Laurent
table even for a finite terminal set. These are falsifiable engineering gates,
not claims about AMFlow's theoretical limits.

## Stage 1 implementation tracks

### Closing foundry and artifacts

1. Refine exact coefficient and guard applicability on target cells, including
   exceptional equality and nonzero branches.
2. Add translate-before-substitute residual search, immutable lower-sector
   feedback, and a deterministic fixed point with proof-bearing symmetry,
   zero, factorization, product, mapping, and terminal providers.
3. Introduce versioned immutable artifact ownership only with its first closed
   family. Keep incomplete resumable workspaces structurally distinct from
   installable artifacts.
4. Close, replay, publish, and independently audit the `K = 1`, `K = 3`, and
   `K = 6` families in that order.

### Rule application and public APIs

Add a deterministic, memoized, strictly descending artifact applier. It
selects applicable guarded rules, detects malformed artifacts or cycles at the
trusted boundary, returns exact coefficients of typed master keys, and restores
the common scale by homogeneity. It never regenerates the artifact during an
ordinary reduction.

Expose closing-artifact generation, inspection/replay, and reduction through
the Rust library, `campaign` CLI, and `import rustred` Python API. All three
frontends call the same application services and produce the same deterministic
semantics. Three-loop Python examples must steer both the standalone hinted and
fully autonomous K6 campaigns, label their provenance accurately, write durable
bytes only after genuine closure, reload them, and demonstrate reduction.
Release-build Python measurements cover each entire generation boundary and
never report diagnostic FORM-rule import time as artifact-generation time.

### Vakint scalar backend

On GammaLoop branch `vakint_rustred`, add the opt-in scalar evaluation backend
`EvaluationMethod::RustRed(RustRedEvaluationOptions)` and
`EvaluationOrder::rustred_only()` without changing existing defaults or
behavior. The adapter:

- consumes Vakint's existing topology match and simultaneous routing witness;
- never rematches a graph, dispatches on a topology name, or duplicates the
  topology registry;
- applies shipped immutable RustRed artifacts and never regenerates them at
  evaluation time;
- returns exact coefficients of typed RustRed evaluation terminals; Vakint
  uses an exact MATAD-basis map when one exists, otherwise it substitutes a
  separately validated shipped high-precision Laurent table generated once
  with MATAD; future four-loop data may use exact FMFT reductions, but the
  currently shipped FMFT numerical tables are mostly only 26--50 digits, so
  generic 20,000-digit terminal data would require regeneration or AMFlow;
- exposes master substitution control, enabled by default; and
- reports no FORM dependency and never invokes or falls back to FORM for
  scalar IBP reduction or master substitution.

Production artifacts are generated once, checked into and shipped with Vakint,
and loaded once. A local path dependency may be used while co-developing; every
pushed GammaLoop milestone pins the exact validated RustRed Git revision.

GammaLoop branch `vakint_rustred` is rebased onto `feynkit`, and tensor-bearing
inputs in the active lane run the native FeynKit tensor prepass before the
RustRed scalar IBP and master-substitution tail. The complete lane is therefore
FORM-less. Tests explicitly select FeynKit and use an invalid FORM path so a
default change cannot silently weaken this guarantee. AlphaLoop, MATAD, and
the historical FORM tensor method remain unchanged backward-compatible oracle
lanes, never fallbacks from RustRed. Vakint continues to accept every legacy
public integral notation and API spelling supported before this branch; any
newer notation used by FeynKit or RustRed is additive and must have regression
coverage proving that the historical spelling still evaluates identically.

## Stage 1 milestones and acceptance

1. Commit and push the authoritative staged goal and documentation.
2. Close and publish the one-loop artifact with the artifact/reducer spine.
3. Close the two-loop family, cover its pinch, and expose Rust, CLI, and Python
   artifact/application APIs.
4. In parallel with three-loop closure, rebase `vakint_rustred` onto `feynkit`
   and validate the FeynKit-tensor-plus-RustRed-scalar stack through two loops.
5. Close the `K = 6` family and prove coverage of all five registered
   three-loop graph classes.
6. Pass all applicable single-scale Vakint acceptance tests through three
   loops, then benchmark and profile optimized warm RustRed scalar reduction
   against equivalent AlphaLoop/MATAD workloads. Address material regressions,
   update documentation, commit and push both repositories, then begin the
   already-authorized Stage 2 work.

Acceptance requires:

- exact regenerated-source replay, strict descent, explicit terminals, and no
  uncovered branch in each installed artifact;
- exact finiteness of any nonminimal terminal complement, plus exact MATAD
  basis-change rows or independently validated high-precision Laurent values
  for every such Stage 1 terminal;
- deterministic artifacts and reductions across supported worker counts;
- guard selection, termination, memoization, symmetry routing, terminal-only
  output, and non-unit-mass restoration tests;
- exact raw terminal-coefficient tests inside RustRed and matching numerical
  Laurent-series expectations against AlphaLoop/MATAD across the applicable
  Vakint harness; exact cross-backend raw-master comparison is required only
  where an explicit common-basis map exists;
- an explicit policy on every Vakint comparison lane: `ExactMatadBasis`
  requires raw coefficient equality after a certified basis map, whereas
  `NumericalOnly` accepts a different finite RustRed terminal basis and
  requires equality only after independently validated terminal substitution;
  a valid nonminimal terminal set is never rejected merely for differing from
  MATAD's preferred symbolic masters;
- scalar RustRed-backend tests with an invalid FORM path;
- tensor-bearing tests explicitly using the FORM-less FeynKit tensor prepass
  followed by the FORM-less RustRed scalar tail, also with an invalid FORM
  path;
- release-build benchmarks that separate cold artifact load/validation from
  warm memoized scalar reduction, compare identical input batches and output
  precision against AlphaLoop/MATAD, and retain profiles identifying dominant
  RustRed costs; and
- unchanged Vakint public API conventions, defaults, and existing FORM-backed
  behavior, including paired legacy/new-notation regression inputs, together
  with a negative test that obsolete RustRed artifact
  schemas are rejected rather than migrated, dual-decoded, or used through a
  fallback.

PySecDec comparisons are optional, non-gating corroboration.

## Stage 2 production — authorized after the Stage 1 gate

Stage 2 preserves the long-term ambition from the historical preamble and is
now explicitly authorized to begin immediately after the complete Stage 1
acceptance gate. No further permission pause is required. The already
available FeynKit tensor reducer remains the Stage 1 tensor path; Stage 2 may
integrate the collaborator's advanced rank-generic tensor technology once its
concrete implementation is available, or continue with independently audited
Rust-plus-Symbolica work that does not duplicate an unavailable CAS primitive.
During Stage 1,
the winning IBP-foundry candidate may already be studied on the **complete
authenticated** four-, five-, and six-loop single-scale vacuum manifests. Each
study is bounded,
pre-registers its resource and promotion/kill gates, includes all hard and
censored families in aggregate results, and reports only the strongest proved
state (`Manifested`, `Probed`, `ModularCandidate`, `ExactReplayed`,
`GuardOwned`, `BoundaryDischarged`, `ChartClosed`, or `FamilyClosed`). A
bounded or sampled census is never described as closure. Stage 2 production
includes:

- integrating or replacing tensor reduction beyond the current FeynKit
  capability and proving the eventual implementation generic in rank;
- closing four-, five-, and six-loop vacuum manifests;
- high-loop-specific distributed-memory, reconstruction, and extreme
  efficiency work; and
- the eventual six-loop QCD beta-function evaluation chain.

The Stage 2 manifest is necessarily multi-parent. `K=L(L+1)/2` counts scalar
products but does not make the `q_i`, `q_i-q_j` root-coordinate family
universal. Already at four loops the nonplanar cubic `K_{3,3}` vacuum graph has
a non-graphic cographic line matroid and cannot be embedded as a restriction of
the graphic `K_5` root family by a unimodular routing. Complete-graph mask
counts remain proxy/cache experiments only. Each loop order must instead use a
matcher-derived census of physical parent families with exact simultaneous
routing witnesses on denominators, ISPs, masses, guards, cuts, and ordering.

Stage 1 code must not preclude Stage 2. Research prototypes become durable
infrastructure only after measured K6 evidence; attractive but unvalidated
architecture is documented and killed or retained explicitly.

## Engineering and repository invariants

- Production RustRed and the Vakint RustRed scalar backend use only Rust plus
  GMP-enabled Symbolica. They never execute FORM, Mathematica, SymPy, or
  another CAS. The active tensor-bearing Vakint acceptance lane uses FeynKit;
  FORM-backed modes remain independent backward-compatible oracle lanes, not
  RustRed algebra providers or fallbacks.
- Search Symbolica's public API, Rustdoc, source, examples, and tests before
  implementing any algebraic primitive. RustRed owns physics meaning,
  authentication, guards, ordering, provenance, resource admission, and exact
  replay; it does not grow a second CAS or graph-isomorphism engine.
- Symbolica's intrinsic graph generation/canonization/isomorphism facilities
  are the symmetry-candidate authority. RustRed owns physics-colored encoding
  and exact momentum/routing replay.
- Use semantic module ownership; do not revive chronological, `generated`,
  `residual`, `runtime`, `legacy`, or `misc` buckets. RustRed has no pre-release
  compatibility promise. Vakint preserves its public API conventions, defaults,
  and existing FORM-backed reduction methods, but it deliberately provides no
  compatibility layer for obsolete RustRed parametric-IBP artifact schemas:
  shipped and user-supplied artifacts must use the single current schema.
- Validate untrusted inputs and durable artifacts at their boundary. Do not
  accumulate repeated internal authentication ceremonies in the hot path.
- Deterministic parallel work uses one bounded coordinator/pool, shared
  immutable state, RAM-aware admission, stable ordinals, and sorted merges.
  Stage 1 implements only the parallelism justified by three-loop workloads;
  high-loop artifact production and extreme execution infrastructure belong to
  Stage 2. Stage 1 nevertheless authorizes bounded, census-complete
  K10/K15/K21 scaling studies, with K6 controls, under the evidence gates above.
- `FOR_REFERENCE_ONLY_DO_NOT_PUSH` is ignored and never enters RustRed history.
  GammaLoop inside it is a separate repository and branch.
- Never escalate commands. Use rollback-sized commits, push passing milestones
  frequently, and configure every Git operation with:

  ```text
  user.name=ValentinHirschi
  user.email=valentin.hirschi@gmail.com
  ```

Do not claim Stage 1 complete until all three artifacts cover the frozen
through-three-loop manifest and the Vakint RustRed scalar backend reproduces
the applicable acceptance suite. Once that gate passes, record and push the
milestone, then proceed directly into Stage 2 without pausing for another
authorization.

## Stable project documentation

- [Architecture](docs/architecture.md)
- [Algebra and Symbolica boundary](docs/algebra.md)
- [Frozen tensor boundary and Vakint sequencing](docs/tensor.md)
- [Closing-rule foundry design](docs/foundry.md)
- [Application, Python, and Vakint interfaces](docs/interfaces.md)
- [Validation and oracle ladder](docs/validation.md)
- [LiteRed2 semantic reference](docs/references/litered2.md)
- [Parametric-IBP breakthrough research](docs/research/parametric_ibp_breakthrough.md)
- [Independent breakthrough viability audit](docs/research/parametric_ibp_breakthrough_audit.md)
- [Primary-literature synthesis through 2026](docs/research/parametric_ibp_literature_2026.md)
- [Finite-frame breakthrough candidates](docs/research/finite_frame_breakthrough_2026.md)
- [High-loop proposal experiments and falsification gates](docs/research/high_loop_proposal_experiments_2026.md)
- [Symbolica finite-frame feasibility audit](docs/research/symbolica_finite_frame_feasibility.md)
- [Nonminimal-terminal viability audit](docs/research/nonminimal_terminal_viability_audit_2026.md)
- [Independent six-loop candidate shootout](docs/research/six_loop_candidate_shootout_2026.md)
- [Universal nonminimal closure evidence update](docs/research/universal_nonminimal_closure_review_2026.md)
- [Graph-orbit and Baikov source-compression audit](docs/research/graph_orbit_baikov_source_compression_2026.md)
- [Executable K6 breakthrough prototype specification](docs/research/k6_breakthrough_prototype_spec_2026.md)
- [Six-loop algorithm and implementation update](docs/research/six_loop_algorithm_update_2026.md)
- [Six-loop execution runbook](docs/research/six_loop_execution_runbook_2026.md)
- [Dual-obstruction source-discovery proposal](docs/research/dual_obstruction_source_discovery_2026.md)
- [Vakint K6 oracle and terminal-budget audit](docs/research/vakint_k6_oracle.md)
- [Blind-domain-guided Janet closure synthesis](docs/research/blind_domain_janet_closure_2026.md)
- [Janet/Ore proposal integration seam](docs/research/janet_ore_integration_seam_2026.md)
- [K6 Janet/Ore bounded release study](docs/research/k6_janet_ore_release_study_2026-09-02.md)
- [K6 Janet incremental-epoch and copy-on-write audit](docs/research/k6_janet_incremental_epoch_audit_2026-09-02.md)
- [K6 Janet modular scheduling and fraction-free replay design](docs/research/k6_janet_modular_fraction_free_design_2026-09-02.md)
- [Exact-lazy persistent Janet completion design](docs/research/k6_exact_lazy_persistent_completion_design_2026-09-03.md)
- [Audited factorized product-angular owner design](docs/research/factorized_product_angular_owner_2026.md)
- [Sector-local coordinate charts as a K6 search preconditioner](docs/research/sector_local_coordinate_chart_2026.md)
- [Audited K6 boundary-walk observations](docs/research/k6_boundary_walk_2026.md)
- [Current CLI contract](docs/CLI.md)
