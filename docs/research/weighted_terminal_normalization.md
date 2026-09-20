# Finite weighted terminal normalization

Status: implementation and release validation in progress, 20 September 2026.
The shipped Vakint output convention still has 179 family-local representatives.
The core implementation and native sidecars now independently reproduce 74;
downstream numerical delivery remains a separate pending gate.

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
Existing candidate/certified coefficient wire bytes are unchanged. Proposed
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
seconds (8m59s compilation excluded). Downstream Vakint gates remain pending
at this checkpoint.
The initial L=5 success fixture hit the inherited U prospective-product bound
(25,200 terms versus 20,000); that test and its cold replay now explicitly use
larger finite Symanzik budgets. All exact assertions and production defaults
are unchanged; the initial failure and exact diagnostic are retained.

The artifact gate below encodes and cold-reloads the four unchanged saved
candidate programs, replays every witness, checks all final edges and all 1,155
catalog expansions exactly, and preserves the input hashes. Catalog values are
post-hoc checks only, never discovery inputs. Delivery additionally requires
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

The separate U-only versus weighted application diagnostic and the Gamma
numerical/public benchmark rollout are still pending. No minimal-master,
complete-family reduction or application-performance claim follows from this
finite 74-output convention alone.
