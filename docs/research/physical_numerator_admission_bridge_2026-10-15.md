# Physical numerators and saved massive-family admission

15 October 2026. Boundary audit and native preparation measurement; no admission policy, campaign,
artifact, rule generation or numerical master evaluation was changed.

## Current behavior is consistent with the actual saved scope

Vakint's `rustred_matched_input_key` transports the numerator with the same
verified routing witness as the denominators, calls RustRed's
`FamilyScalarNumeratorService::lower_weighted`, and checks every nonzero routed
endpoint with `PreparedSavedApplication::routed_entry_is_admitted`.
The final `reduce_routed_linear_combination` independently enforces the saved
entry union before coefficient-cache lookup. Neither path treats R/D maxima
as a rectangular coverage guarantee. Intermediate IBP descendants are not
clipped to the starting bounds.

Relevant sources:

- Vakint `crates/vakint/src/rustred_evaluation/five_loop.rs` and `numerator.rs`;
- RustRed `crates/rustred-core/src/scalar_numerator/weighted.rs` and `model.rs`;
- `crates/rustred-app/src/application/saved_application/{mod,scope,package}.rs`.

Weighted lowering already processes the complete polynomial and coalesces
equal family keys before admission. It is not naive term-by-term expansion of
the original numerator. Its public result carries the authenticated family
identity, exact coefficients, scalar spectators and original nonzero conditions.
Its constructors are internal to the native lowering service.

Importantly, the weighted result folds common-mass powers into each scalar
spectator and deliberately sets `common_mass_squared_power()` to zero. That
getter therefore cannot recover a physical mass grading afterward. The older
unweighted lowering type has different bookkeeping semantics.

## Physical dimension is not an alternative admission certificate

For a Minkowski denominator `P=k²−M²`, the exact identity is

```text
k²/P² = 1/P + M²/P².
```

The physical numerator presentation has A=2, scalar degree R=1 and D=1.
The two raw integral keys have D=1 and D=2; the second carries one explicit
M² factor, so D−j=1 for both. If a deliberately bounded saved scope admits
only the first key, recognizing the physical grading does not prove that the
second key is covered. Keeping the input numerator unexpanded does not change
this algebraic obligation. The example is a statement about a restricted scope,
not a claim that the existing full one-loop rules lack the familiar recurrence.

Likewise, the five-loop R6/D10 physical argument alone cannot authorize every
raw higher-D term produced by massive-basis lowering. A failing guard here can
be a real representation/coverage mismatch, not a bad numerical inequality.
It must not be repaired by disabling entry checks, treating an unresolved key
as a master, or silently replacing D10 with a larger starting rectangle.

## Smallest sound generic bridge

1. Introduce a prepared physical-scalar request minted only by the existing
   exact lowering service. Retain original numerator/base key/common mass,
   family identity, complete coalesced weighted terms and all pole conditions.
   A diagnostic can report original physical grading and actual raw endpoint
   A/R/D separately, but only when homogeneity is established exactly. Do not
   read a mass exponent from the weighted result's zero-valued legacy getter.
2. Reuse native verified owner transport to collect the complete request's
   weighted endpoints before admission, preserving every original condition.
   This may eliminate presentation-only rejection when transported siblings
   cancel exactly. Any such API must be distinct from the existing arbitrary
   linear-combination API, whose admission-before-cancellation contract stays
   intact. Native Symbolica collection, not approximate zero tests, supplies
   the algebra; arbitrary scalar spectators need the same exact collection
   used by weighted lowering rather than a new coefficient engine.
3. Admit every surviving endpoint against the unchanged published union.
   Report remaining unsupported keys and their source/lowering provenance.
   Whole-input collection cannot make genuine higher-D survivors disappear,
   as the one-loop example demonstrates.

If genuine survivors lie outside that union, the next step needs explicit
authority, not a frontend heuristic: for example, a compact published cover
of already closed reachable domains, or a separately requested finite exact
completion for the actual missing keys using existing rules. The runtime
package currently exports the recorded starting queries and owner programs,
not the campaign's enormous descendant ledger or a closed-descendant-cover
admission certificate. A loaded owner alone is not such a certificate.

This proposal first improves provenance and whole-input handling. It does not
by itself establish full physical R6/D10 coverage. The smallest useful tests
are an exactly cancelling transported sibling pair, a genuine surviving
mass-shift term that must still be rejected, unchanged original-pole guards,
package/scope separation, and numerical-rank-zero through rank-two physical
numerators whose *actual* lowered endpoints are admitted by a completed saved
publication. No new CAS or broad certification framework is required for that
diagnostic/collection slice.

## Physical ladder preparation reproduced on the current integration

The existing standalone `five_loop_qcd_ladder` example completed native
preparation from its DOT/model input, including symbolic-d gamma/color algebra,
quadratic Taylor expansion and the transverse q² projector. This was the full
preparation path, not the `--dress-only` shortcut, and it loaded no RustRed
application artifact or FORM executable.

| Measurement | Result |
|---|---:|
| Native preparation time | 1,254.947 s (20m55s) |
| Driver including evidence checks | 1,259.621 s |
| Workers | 1 |
| Peak process-tree RSS | 15.456 GB |
| Taylor contributions | 21 |
| Reported scalar terms | 8,306 |
| Final Vakint input | 34,795,612 bytes |

All 21 contribution indices have a DOT and a prepared numerator. The exact
physical power inventory records A=14..16 and A−R=10, with scalar ranks
R=4 (one contribution), R=5 (five), and R=6 (fifteen). Five preparation
condition records were saved; these include unity/redundancy and must not be
described as five distinct poles.

The frozen example uses the unchanged GammaLoop/Vakint `c4e1185a` integration,
RustRed Git `74a83e94`, Symbolica `400a53c` and FeynKit `024f96d`. Binary SHA256:
`b1f28ea2b5864f2b44c13a050a797cc37afb6c7f592392cb5aa279c8c95597f4`.

The preparation run completed under the explicit 32 GB RAM/30-minute pilot
guard, with no resource stop and all processes drained. Independent audit
checked the source bindings, raw inventory and all 48 evidence-file hashes.
Evidence lives at `TMP/ladder-preparation-current-runtime-20261015/evidence/`;
its parent contains `summary.json` and `independent-audit.json`. The full slow
acceptance test still regenerates its numerator as required; manual diagnostics
can reuse this saved input without repeating the 21-minute preparation.

## Complete lowering/admission diagnostic on the published R4 scope

The existing prepared-input diagnostic completed all 21 Taylor contributions
against the R4/D10 package, without applying IBPs. It preserves the physical
numerator and checks the original poles and exact mass homogeneity using
Symbolica. One geometric matching witness is used per contribution; this is
not a search over every alternative routing.

The initial default-limit run stopped at 16,000,001 weighted term operations
against its 16,000,000 allowance. Its seven completed contributions and failure
evidence are retained. One explicitly bounded diagnostic rerun used the existing
`VAKINT_QCD_DIAGNOSTIC_MAX_WORK=128000000` override, leaving production defaults,
all other algebra/expansion limits and the saved input scope unchanged. It
completed in 548.889 seconds under the native supervisor, 552.065 seconds
including driver checks, with 3.252 GB peak process-tree RSS. Both runs used the
same frozen binary and prepared input. Contribution indices are run-local;
the auditor matched the old prefix by exact recorded input signatures, not by
assuming the indices stayed fixed.

The complete result contains 75,643 lowered occurrences and 27,812 distinct
integer tuples in this common family. Of these, 6,547 occurrences (1,240 distinct
keys) pass the native owner-transport and recorded-domain admission check.

| Raw stored D | Explicit power j of mUV² | Occurrences |
|---|---:|---:|
| 10 | 0 | 20,842 |
| 11 | 1 | 27,852 |
| 12 | 2 | 18,140 |
| 13 | 3 | 6,998 |
| 14 | 4 | 1,595 |
| 15 | 5 | 202 |
| 16 | 6 | 14 |

Every mass-graded term satisfies **D−j=10** exactly. This confirms the physical
logarithmic grading while showing why that grading alone is not admission of
the separate massive-family keys. All raw D>10 occurrences are unadmitted by
this package. Even at D=10, 14,295 occurrences are unadmitted; their stored
scalar ranks are already at most four (counts for R=0..4 are
1, 7,559, 5,698, 1,014 and 23). Raising the scalar-rank cap alone therefore does
not explain these refusals. Exact support, routing and the recorded query
geometry must be examined, rather than replacing admission with R/D maxima.

This diagnostic coalesces each contribution's polynomial, but does not collect
coefficients across all 21 differently presented contributions. It neither
proves that every rejected key survives the full sum nor that an alternative
exact witness also fails. It is not a completed ladder reduction, a missing-IBP
diagnosis, or authority to widen the campaign to a raw D=16 rectangle.

Evidence: `TMP/r4-vakint-acceptance-20261015/ladder-preflight-work128m-result.json`,
`ladder-full-census.json` and `ladder-preflight-work128m-independent-audit.json`.
The independent audit checked all 254 bound evidence files, the complete key
and mass histograms, and the correspondence with the preserved failed prefix.

## Narrow source-zero routing correction

One refusal revealed an application-path inconsistency, not a missing IBP:

```text
[3,3,1,0,1,1,0,0,0,0,0,1,0,0,0]   (stored R=0, D=10)
```

Its positive support is `111011000001000`. In this family its active
denominators do not depend on the fourth loop momentum. More importantly than
that geometric observation, the accepted full-R1 native trace already recorded
the same support as a zero, at `[1,2,1,0,1,1,0,0,0,0,0,2,0,0,0]`. The trace and
R4 application package use identical ordered owner payloads and routes. Native
traversal classifies these zeros by exact installed support certificates.

Entry transport previously checked those certificates only on transported
endpoints. The original input could therefore fail for lack of an owner before
the same certificate was consulted. Main commit `67dec9dd` checks the original
source's installed certificate after arity, source conditions and resource
charges, before owner lookup. It retains the nonzero conditions on an empty
result. It computes no new zero test, drops no uncertified term, and leaves
every surviving endpoint's published-domain admission unchanged.

The optimized core gate passed 553 distinct tests (551 candidate and two
persistence), including three new source-zero tests, in 5.573 seconds supervised;
compilation took 566.452 seconds separately. Tests include cleared-certificate
missing-owner refusal, vanishing source conditions, malformed arity, exact
resource refusals, and guard retention through actual routed reduction without
rule, transport or cache work. Independent audit passed; evidence is
`TMP/entry-source-zero-validation-20261015/runtime/independent-audit.json`.
The actual current-package K15 probe subsequently passed independent review:
the displayed key and a D11 key with the same installed zero support both
return exact zero. A D10 control not classified as zero routes to its unchanged key
with coefficient one, whereas its D11 counterpart still fails admission. This
is entry routing, not reduction of that key to masters or proof that the final
integral is nonzero. Malformed arity is
rejected before owner preparation. Rule-application
counters remain zero throughout. Package preparation took 40.245 s; the whole
guarded driver took 47.829 s and peaked at 2.551 GB. Evidence:
`TMP/r4-saved-entry-probe-20261015/runtime/`.

This confirms that one concrete application defect is fixed without broadening
nonzero admission. It is not a rerun of the full 75,643-occurrence census and
does not solve the other rejected supports, mass-shifted images, or full
ladder application.
