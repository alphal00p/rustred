# Physical numerators and saved massive-family admission

15 October 2026. Read-only boundary audit; no admission policy, campaign,
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
