# Matcher-bound affine descent/condition validation matrix

This is the acceptance checklist for the crate-private phase between an
authenticated generated affine `WhenBad` input and affine-boundary pullback
construction. It is intentionally topology-generic. Generated sunset cases
`001`, `011`, and `101` are validation fixtures only; no recurrence
coefficient, RHS shift table, or expected rule is embedded in a test.

## Phase-order oracles

- Precharge and fallibly reserve every RHS descent witness and all
  `rhs_count * ambient_arity` components before proving any witness.
- Prove RHS shifts in the `BTreeMap` order of `ParametricRelation::terms()`,
  omitting the centered pivot without changing the relative order.
- A positive or zero first complexity delta returns a redacted typed
  `Unsupported` result before inspecting zero candidate conditions.
- Only after every RHS passes may domain conditions be collected. Pullback
  counters must remain zero at this checkpoint.

A private synthetic test combines a failing `+1` active-sector shift with a
zero candidate condition. The result must be descent `Unsupported`, never
`IdenticallyBad`. One-below aggregate component admission must fail before
that mathematical outcome is computed.

A separate completeness fixture fixes one active affine target row at `1`
and uses an RHS shift which pinches that row but is globally non-descending
because of another positive active shift.  The staged V1 proof may classify
this as `NonDescendingInGlobalOrthant`; the final target-local compiler must
prove the universal pinch and route it as lower-sector rather than publish a
terminal unsupported result.  This rescue is structural and must run without
using any coefficient-zero or candidate-condition fact.

## Condition input and canonicalization oracles

The exact encounter order is:

1. selected-target guard-composition entries;
2. recentered relation guards;
3. the centered-pivot coefficient denominator; and
4. RHS coefficient denominators in the same ordered RHS traversal.

Every input, including a discharged nonzero constant, is represented in a
bounded crate-private typed transcript. The denominator-input count is exactly
`relation.terms().len()`, so the unit pivot denominator cannot be skipped.

Canonicalization searches every retained row for exact polynomial equality
before it performs an associate search. For `[p,p]`, no associate call occurs.
For index-dependent `p`, `[p,theta*p]` merges over `K*=Q(theta)*`. For
base-only inputs, `[theta,2theta]` merges over `Q*`, while
`[theta,theta+1]` and `[theta,theta^2]` remain distinct. Dependency classes
never cross-merge. The first polynomial remains the representative. If any
merged source is inherited, inherited scope dominates while all candidate
provenance remains attached privately.

Input/source limits are charged before deduplication. A long duplicate stream
therefore cannot evade work limits by producing one canonical row. Base-only
nonzero polynomials are retained as formal coefficient-field assumptions and
never create an index split. A zero candidate polynomial is identically bad;
a zero inherited target premise is a hard authority invariant failure.

## Free-position and privacy oracles

Before condition use, scan every selected-target guard, relation guard, and
every relation-coefficient numerator and denominator. A nonzero exponent in a
private index slot not listed by the selected target's authenticated affine
map is a hard mismatch. Repeat this defense after future compositions.

Public views expose only ordinals, scope/classes, and typed source kinds.
`Debug` output must not contain a private relation manifest, raw polynomial,
coefficient, exact RHS shift, denominator-source shift, or numerator gate.
The target-local unsupported type must not reuse the global
`WhenBadUnsupportedReason` formatter because that formatter includes
`rhs_shift`.

## Resource-boundary oracles

Generated `001` supplies the ordinary nonzero counters. Synthetic duplicate,
associate, zero, and wide-sparse cases cover counters which happen to be zero
there. For every counter, an exact measured limit succeeds and a one-below
limit fails transactionally:

- RHS terms, witnesses, and witness components;
- target-guard, relation-guard, pivot/RHS-denominator, and total inputs;
- retained condition sources and source-shift components;
- dependency scans;
- equality comparisons, term units, dense exponent entries, and integer bits;
- index-associate projection/native work and base-associate scalar-call,
  input/output/comparison/temporary envelopes; and
- retained polynomial terms, exponent entries, integer bits, display bytes,
  and owned bytes.

A one-term polynomial supported only at a high index position verifies that
dense exponent-entry work is charged independently of sparse term count.

## Generated integration

For each reachable generated pending row in `001`, `011`, and `101`, derive
the expected ordered RHS/source transcript directly from the authenticated
matcher input. Independently recompute signed descent components and
free-position support, compare typed ordinals/classes/counters, and replay the
pre-pullback outcome. Concrete powers may be used only after this generic
phase is sealed, as reduction/application tests and Vakint comparisons.

Licensed validation is always run with the GMP backend and parallel nextest:

```bash
SYMBOLICA_LICENSE="$SYMBOLICA_LICENSE" SYMBOLICA_HIDE_BANNER=1 \
  cargo nextest run -j4
```

Neither FORM nor Mathematica is executed.
