# Finite weighted terminal normalization

Status: delivered, 20 September 2026. Core/native milestone `8ad62b96` and
GammaLoop `fdfb0e43c297c5d78c488d73ad4be508d03de2dd` (`vakint_rustred`) are
published. Vakint vendors and loads the four verified native sidecars, reducing
its effective output convention from 179 to 74 family-local representatives
while preserving all 1,155 raw declarations/catalog entries. The numerical
acceptance and measured public benchmark gates below pass on the pinned runtime.

## API and authority

`rustred::reduction::terminal_normalization::TerminalNormalizationPlan` is a
distinct, immutable optional convention, not an IBP program or a certificate of
coverage or master independence. Its generic factory is
`vacuum_quadratic_numerators(family, raw_terminals, ordering, limits)`.
No loop count, family name, topology or catalog value selects an algorithm.

The factory admits unshifted unit-mass vacuum supports with rank `L`, `L+1`
denominators at power one, a primitive circuit with coefficients zero or ±1,
and one quadratic numerator. Existing native momentum/circuit preparation is
reused. Adjacent circuit permutations and coloop sign flips propose `L` maps;
the existing symmetry verifier establishes each exact active-denominator unit
permutation, unit Jacobian and absence of new nonconstant guards. Symbolica's
native rational matrices solve and replay the full affine numerator identity.
Inactive denominator slots need not be permuted.

The positive output union is normalized by the existing exact native
U-polynomial service and bound back to already declared positive keys.
Pinches disappear only with retained unconditional zero certificates.
Unsupported geometry, incomplete spans, missing positive bindings and valid
bindings that violate the saved strict ordering retain the original terminal
with a typed skip. Weighted structural-limit exhaustion returns a typed error;
the inherited U service retains its existing conservative preparation-limit
skips. Ordinary IBP descent checks are untouched.

The owner exposes the final rows and statistics, verified momentum generators,
complete affine columns, exact native combination vectors, positive alias
witnesses/bindings and zero certificates. Every emitted key is a declared
self-identity row: outputs are flattened and nonrecursive. The positive-only
unit-alias factory still leaves each negative-power key separate until this
weighted projection.

Install with `CandidateReducer::install_terminal_normalization(plan)` on an
empty cache. Family, ordering, exact raw terminal set, coefficient contexts
(both ordered maps, including constants) and current algebra limits are checked
before mutation. Successful installation replaces a unit-alias convention;
the reverse installation also requires an empty cache. Failure is atomic and
never silently clears cached results. Defaults remain unchanged.

The terminal hot path only copies the sealed row into the existing sparse or
factorized cache. It performs no momentum proof, matrix solve or recursive
normalization. Original source-pole validation, cache budgets and the requested
target survive unchanged. In particular, the existing mass power
`sum(output powers) - sum(original target powers)` restores the different mass
factors of scalar and pinch terms; downstream callers must not add them twice.

## Native sidecar contract

`plan.encode_native(BinaryIoLimits)` writes the uniform native envelope with
kind `TerminalNormalization` (4), using Symbolica Atom/State coefficient pooling
and standard bincode integer records. Schema 1 records arity, family fingerprint,
saved ordering, exact ordered raw keys and output/coefficient dictionary IDs.
Existing candidate/certified coefficient wire bytes are unchanged. Vendored
four-family filenames are `h.rrnorm.bin`, `fg.rrnorm.bin`, `bmw.rrnorm.bin` and
`x.rrnorm.bin`; filenames confer no authority.

`TerminalNormalizationPlan::decode_generated(bytes, family, raw, ordering,
preparation_limits, io_limits)` is explicitly for trusted generated native
data. It checks framing, bounds, kind/schema, owner, ordering, exact raw keys,
dictionary structure and native coefficient contexts, then regenerates the
finite plan **once** and compares every output and full native coefficient,
including both ordered variable maps. It returns the regenerated proof owner,
not an unchecked deserialized replacement table. This is neither a hostile
Symbolica parser guarantee nor a new source of rule/certificate authority.

`TerminalNormalizationLimits` bounds raw/union keys, supports, matrix cells and
output terms and carries the existing parametric limits. These are structural
work bounds, not peak-memory or per-CAS-operation time guarantees. Unsupported
cases are explicit; no five-loop or topology-specific cutoff is introduced.

## Validation boundaries

The focused source tests cover successful generic `L=3` and `L=5` projections,
multiple numerator terminals, a coloop with an undeclared proved-zero pinch,
unsupported masses/shifts/powers/circuits, harder-sector retention, unbound
outputs and resource limits. They also cover native owner/kind/schema/value/
context tampering, both cache representations, mass exponents, atomic convention
replacement, inherited source poles and failed weighted cache admission.
The fresh complete core release suite passes: **2,223 passed, 31 ignored,
zero failures**, including all ten new weighted tests and both independently
added failure-path tests (162.08 seconds runtime; compilation excluded).
The complete application gate also passes: **91 unit tests and 67 integration
tests across ten targets**, zero failures or ignored tests; the binary and
documentation targets contain no tests. Application runtime totals 62.54
seconds (8m59s compilation excluded). The downstream Vakint gates also pass,
as recorded below.
The initial L=5 success fixture hit the inherited U prospective-product bound
(25,200 terms versus 20,000); that test and its cold replay now explicitly use
larger finite Symanzik budgets. All exact assertions and production defaults
are unchanged; the initial failure and exact diagnostic are retained.

The artifact gate below encodes and cold-reloads the four unchanged saved
candidate programs, replays every witness, checks all final edges and all 1,155
catalog expansions exactly, and preserves the input hashes. Catalog values are
post-hoc checks only, never discovery inputs. Delivery additionally includes
newly vendored sidecars and the unchanged 83 lower-loop, 15 four-loop reference
and 16 expanded-numerator pinch numerical cases. Preparation, first application
and warm cache timings must remain separate.

Scratch protocol, exact 105 relations, the 74-output census and independent
repeat are documented in
[the independent symmetry critique](four_loop_numerator_symmetry_critique_2026-09-20.md).
Implementation evidence is retained in `TMP/terminal-weighted-core.udaCBZ/`.

## Native saved-corpus gate

The public-API exporter and a separate fresh verification process both pass.
The latter deliberately seeds unrelated Symbolica symbols and ordered variable
maps before decoding; expected plans are prepared only after import. Both runs
independently reconstruct affine columns, reverify momentum maps, replay every
native combination and check strict saved-order descent and declared fixed
points. Every raw installed-reducer result agrees with the sidecar, including
both coefficient maps, original target and per-term mass powers. This terminal
gate applies no IBP rules.

| Family | Raw keys | Previous U outputs | Numerator projections | Final outputs | Sidecar bytes |
|---|---:|---:|---:|---:|---:|
| H | 386 | 52 | 30 | 22 | 8,934 |
| FG | 145 | 26 | 10 | 16 | 5,759 |
| BMW | 179 | 37 | 20 | 17 | 6,291 |
| X | 445 | 64 | 45 | 19 | 9,811 |
| Total | 1,155 | 179 | 105 | 74 | 30,795 |

There are 21 admitted supports and 84 verified generators, with no skipped
projections, unbound/new outputs or omitted-zero terms in this corpus. All
1,155 post-hoc catalog expansions and 1,260 output coefficient-map checks pass
in both processes. All eight complete row/proof TSVs are byte-identical between
export and verification. Saved candidate inputs, libraries, executable and
all twelve existing vendored program/catalog/input assets pass hash checks.
The sidecars are frozen by `sidecar-assets.sha256` in
`TMP/gamma-terminal-normalization-rollout.GN7v1b/`, alongside the independently
reviewed `sidecar-corpus-audit.md` and raw logs.

Observed preparation and decode-with-proof-regeneration times (milliseconds):

| Family | Export preparation | Export decode/replay | Fresh verification preparation | Fresh verification decode/replay |
|---|---:|---:|---:|---:|
| H | 388.483 | 341.462 | 334.167 | 365.937 |
| FG | 139.779 | 127.222 | 125.763 | 139.310 |
| BMW | 188.896 | 177.082 | 168.501 | 186.414 |
| X | 467.145 | 438.707 | 432.186 | 462.091 |

These are one shared-host observation per mode on CPU 83 with nested pools one,
not a speedup distribution. Both processes have a 120-second external deadline
and an 8-GiB virtual-address cap (not an RSS cap). Export completes in 11.36 s
(7.10 user + 4.18 system seconds; peak RSS 1,049,972 KiB); fresh verification
completes in 11.48 s (6.92 + 4.47 CPU seconds; peak RSS 1,050,964 KiB). Both exit
zero with empty stderr. Whole-process costs include candidate loading, extra
independent proof/catalog checks, installed terminal applications and evidence
output; they are not bare sidecar-load or scalar-reduction times. Hashing and
prior reads preclude a cold-filesystem claim. Verification preparation occurs
after its decode/replay phase and is not a second independent cold-load sample.

No minimal-master or complete-family reduction claim follows from this finite
74-output convention.

## Cross-family evaluation census — not a new normalization rollout

A September 20 follow-up joins the **actual 74 positive output keys** to their
existing exact offline catalog projections:

| Family input | Output keys | Distinct literal catalog projections |
| --- | ---: | ---: |
| H | 22 | 22 |
| FG | 16 | 16 |
| BMW | 17 | 17 |
| X | 19 | 19 |
| Combined | **74** | **23** |

All output keys are accounted for. There is no repeated projection within one
family's final output set; 51 entries repeat an expression from another family.
The union contains 19 standalone PR symbols and four additional proportional
or linear-combination expressions. PR0–PR15 are sixteen labels, while PR4d,
PR9d and PR11d are additional dotted integrals; PR9x is eliminated. These
counts are not a proof of master independence or a new autonomously derived
23-class integral quotient.

The independent census rechecks all four current native catalog hashes and
all four shipped normalization sidecars against the frozen rollout records.
Exported and independently verified output-key rows agree. The count uses
literal expression/key joins, not numerical sampling or custom algebra. It
refines the earlier 1,155-raw-key/26-expression census; the two count different
sets. Exact oracle projections can coincide because of IBPs without admitting
a momentum or Feynman-parameter relabeling.

A useful next **offline** experiment would pool these existing output supports
across input families, reuse native power-colored graph proposals and replay
full scale-preserving U-polynomial equality or explicit unit-Jacobian momentum
witnesses. Families, dimensions, powers, coefficient maps and measure conventions
must remain bound to every key. Catalogs belong only in post-hoc validation.
The normalization plans and current U-equivalence helper are family-bound.
The generic momentum verifier already accepts distinct source and target
families, but its witness alone does not install cross-family output aliases;
moving an integer vector into another family is not such a service.
No loop count or topology name may select such an algorithm.

Vakint already combines the exact PR projections before numerical evaluation.
Consequently these repeated projections do not imply 74 separately evaluated
independent numerical masters. A global quotient may avoid duplicate terminal
studies, but it does not promise faster per-family memoized reductions. The
shipped 74-output sidecars, values, dependency pins and numerical acceptance
remain unchanged. Research and reproducible census command:
`TMP/cross-family-positive-terminal-census-2026-09-20.md`; independent replay:
`TMP/cross-family-positive-terminal-census-independent-audit-2026-09-20.md`.

## Delivered Vakint acceptance and public timings

GammaLoop revision `fdfb0e43c297c5d78c488d73ad4be508d03de2dd` pins core
`8ad62b964de6f3a508fd165dc6ac2509f25839fe` in both dependencies and lock sources.
The thin const-N adapter delegates native proof reconstruction, weighted
application and caching to core. No loop-count or topology-name algorithm
dispatch, tensor reducer, default-method change or master-value update is added.

The unchanged 83-case lower-loop selection, all 15 reference inventory entries,
all 16 expanded-numerator/pinch pairs, 11 focused loader/catalog/constructor
checks and 3 fixtures pass. Four explicitly ignored fixture tests remain visible;
the public numerical suites and benchmark were run separately. These filters
overlap and are not counts of distinct physical inputs. Original nonunit
mass/scale inputs, precision and tolerances are unchanged. Both native stages
use the existing forbidden FORM path; only the separate FMFT oracle uses FORM.

A fresh frozen U-only control and the new weighted runtime each pass the same
nine-input public benchmark: 54 numerical comparisons and 108 timed calls.
They run sequentially on CPUs 88–93, one nested worker, each with a 600-second
deadline and 8-GiB virtual-address cap. First H/X D1-cubed parent calls change
from 6.230761/30.811257 s to 5.586718/21.411233 s, including lazy program loading
and exact plan preparation. FG's first call instead changes from 1.042856 to
1.109604 s; some warm expanded-D7 and factorized-control calls also regress.
Five same-process warm-call medians do not represent five fresh runs.

Whole-harness wall time is 97.22 -> 89.82 s, user + system CPU
92.97 + 3.63 -> 85.44 + 3.75 s, and peak RSS 2,155,424 -> 2,039,496 KiB.
These totals include both backends, initialization and numerical comparisons,
not isolated RustRed reduction. FMFT's X warm median also changes from 4.780624
to 5.857101 s; this shared-host pair cannot attribute all timing movement to
normalization or establish a universal/statistical speedup. First cubed-parent
RustRed calls remain slower than FMFT; output shrinkage does not imply a
proportional runtime improvement. This D1-cubed study is distinct from D1-squared below.

All observations, exact hashes, unchanged-asset checks and independent audits
are retained in `TMP/gamma-terminal-normalization-rollout.GN7v1b/`, including
`orchestrator-final-audit.md` and `independent-benchmark-audit.md`. GammaLoop's
vendored asset README records every first/warm benchmark row and its boundaries.

## Bounded saved-program application comparison

Eight fresh serial processes compare U-only and weighted normalization using
the same frozen release core/application/Symbolica libraries and unchanged H/X
programs. Both lanes explicitly use the ordinary Sparse coefficient cache.
The recurrence target is **D1 squared**, `[2,1,1,1,1,1,1,1,1,0]`, not the public
Vakint cubed-parent benchmark. The control pinch
`[0,1,1,1,1,1,1,1,1,0]` is already a raw terminal. Each process runs the first
application and five same-point cache hits on CPU 83, nested pools one, with a
120-second external deadline and 8-GiB virtual-address cap.

All eight processes exit zero with empty stderr. All four full postprojection
comparisons and 40 warm-result checks pass, including both ordered native
coefficient maps and mass-exponent telescoping from the original target. The
control output is transported through the shared native Atom/State coefficient
pool, never printed/reparsed algebra. Exact independent accumulation checks run
after all timed calls; no catalog or new IBP generation participates. Every
source, executable, library, program and sidecar hash remains unchanged.

Each table cell is **U-only → weighted**. Preparation is the existing U factory
versus actual sidecar decoding plus exact regeneration, excluding separate
file-read/installation phases. Warm values are medians of five cache hits in
one process; first calls are single paired observations, not statistical medians.

| Input | Plan preparation (ms) | First application (ms) | Warm median (µs) | Output terms |
|---|---:|---:|---:|---:|
| H D1² | 152.364 → 438.096 | 4,366.809 → 3,454.494 | 10.330 → 3.390 | 46 → 15 |
| H terminal pinch | 144.216 → 368.493 | 0.011700 → 0.009950 | 0.300 → 0.500 | 1 → 1 |
| X D1² | 175.612 → 472.471 | 20,655.892 → 17,782.452 | 11.720 → 4.060 | 59 → 14 |
| X terminal pinch | 195.299 → 483.190 | 0.014771 → 0.010660 | 0.351 → 0.651 | 1 → 1 |

H applies exactly 26,956 rules and retains 27,306 cached integrals in both
lanes; X applies 82,637 and retains 83,082. Coalescing additions fall from
418,514 to 312,147 (H) and 2,139,884 to 1,535,210 (X). Retained coefficient
terms fall from 583,892 to 438,759 and 2,391,387 to 1,705,348 respectively;
the corresponding byte counters fall from 29,950,048 to 22,715,736 and
123,085,704 to 88,775,062. These are coefficient-payload counters, not full
allocator memory. Each pinch applies zero rules and retains one integral,
two coefficient terms and 164 coefficient bytes in either lane.

| Input | Whole wall (s) | Whole user + system CPU (s) | Whole peak RSS (KiB) |
|---|---:|---:|---:|
| H D1² | 6.63 → 5.30 | 6.06 + 0.50 → 4.89 + 0.37 | 488,448 → 491,556 |
| H terminal pinch | 1.40 → 1.60 | 1.07 + 0.31 → 1.30 + 0.28 | 491,528 → 491,548 |
| X D1² | 23.38 → 20.84 | 22.33 + 0.83 → 19.80 + 0.86 | 1,070,432 → 1,038,392 |
| X terminal pinch | 2.55 → 2.86 | 1.89 + 0.63 → 2.18 + 0.65 | 1,038,396 → 1,038,348 |

Whole-process resources also include loading, validation, comparison and output.
The higher preparation cost dominates the trivial controls: both pinch processes
become slower overall. H's peak RSS slightly increases despite its smaller
coefficient payload, so no universal memory improvement is claimed. This is a
single-pair shared-host experiment, not an all-target or end-to-end Vakint speedup
claim. Pre-run hashes read the inputs, so it is not cold-filesystem timing.
Nanosecond timer units do not imply that accuracy; especially the tiny pinch
and warm measurements include ordinary timer/call overhead. CPU tick rounding
can report zero on those short phases.

Evidence and complete per-phase snapshots are in
`TMP/terminal-weighted-application.fs03hf/`, including `results.json`, the
reproducible client/driver, raw logs and `independent-audit.md`. Independent
review recomputed all warm medians/counter invariants and checked statuses,
native comparison markers, output snapshots and hashes. No production source
was modified by this diagnostic.
