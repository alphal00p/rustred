# Equal-mass two-loop all-dot recurrence

## Scope

`crates/rustred-legacy-oracles/src/two_loop_top_dot.rs` is the unrestricted positive-scalar counterpart of
the finite `two_loop_pipeline`.  It uses Symbolica coefficients over
`Q(d,m2)`, invokes neither FORM nor Mathematica, and reduces every accepted
request to

```text
S = I(1,1,1),  P = I(0,1,1).
```

The existing analytic boundary reducer closes every sector with at most two
positive lines, including an arbitrary nonpositive power on the inactive
line.  The new recurrence supplies the missing all-positive dot descent.

`ThreeLoopBoundaryReducer` uses this service for the sunset factor produced by
every paw boundary.  Its older finite `TwoLoopReductionPipeline` is still
constructed and exposed for compatibility and finite-box audits, but
`max_two_loop_dots` no longer rejects actual paw reductions.  The boundary
configuration's existing seed-candidate, polynomial-operation, and analytic
boundary budgets become per-induced-sunset memo-state, coefficient-operation,
and boundary-iteration guards, respectively.  They are not one aggregate budget
or one shared memo table for the complete paw polynomial.

## Native identity and orientation

Use the complete `S3` action to orient a positive target as `I(a,b,c)` with
`a >= b >= c >= 1`.  Except at `S`, this gives `a>1`.  At the positive seed
`I(a-1,b,c)`, the native row difference `E00-E01` is

```text
0 = 3*(a-1)*m2 I(a,b,c)
  + (d-3*(a-1)) I(a-1,b,c)
  - 2*c I(a-2,b,c+1)
  + 2*c I(a-1,b-1,c+1)
  + (a-1) I(a,b,c-1)
  - (a-1) I(a,b-1,c).
```

`TwoLoopTopDotReducer::raw_ibp` regenerates `E00` and `E01` from
`IbpGenerator`; `expected_raw_ibp` constructs the displayed expansion
independently.  `validate_raw_ibp_provenance` is an explicit certificate and
validation API: it requires exact equality, but the production `rewrite_once`
path uses the already certified closed formula and does not regenerate native
rows on every reduction step.  The pivot is nonzero over the generic field
because `a-1` is a positive integer and `m2 != 0`.

The top-sector recurrence introduces no exceptional locus beyond `m2=0`.
Closing arbitrary inactive numerators through the analytic two-line formula can
also introduce angular-average poles `d+2*j=0` for nonnegative integers `j`
(the first is the `/d` visible at numerator degree two).  These factors remain
explicit in the returned Symbolica rational-function denominators.  This API
does not yet return a separate structured condition list, so callers needing
special-dimensional reductions must inspect or specialize those denominators
separately.

## Descent proof

Every surviving all-positive branch changes the total exponent sum by `-1`
and therefore lowers total dot degree by one.  The stronger statement that
every raw RHS pattern lowers dot degree is false at a pinch: for example a
unit line can become zero while another line gains a dot.  Such a term has
only two active propagators, however, so it is strictly lower under the exact
sector-first ordering

```text
(active line count, dot+numerator degree, dot degree, powers).
```

Thus every recurrence dependency is lower.  Induction terminates at either
`S` or a boundary closed to `P`.

## Eager normal form and bounds

The normal-form evaluator uses an explicit heap stack and a memo table, not
one Rust call-stack frame per dot.  Before coefficient work it bounds all
positive triples through degree `D` by `C(D+3,3)` and all labelled scalar
pinches by `3*C(D+2,2)`.  A generated pinch has inactive power zero and direct
boundary work at most `D+3`.  At most twenty multiply/add operations are
charged per possible positive state.  Products have coefficient degree at
most `D+1`; the preflight reserves `2*(D+1)` for conservative rational
cross-addition.  Runtime checks additionally inspect every actual product and
sum before asking Symbolica to construct it.

The independent direct-boundary preflight uses `r+q`, where `r` is inactive
numerator degree and `q` is the two active lines' dot degree.  This equals the
specialization of the analytic boundary reducer's variable-degree proof for
the authenticated built-in `d,m2` family.

Limits cover explicit formula terms, native raw terms, memoized states,
coefficient operations, per-variable Symbolica degree, and boundary-formula
iterations.  Index shifts use checked `i32` arithmetic.  Limit and overflow
failures are typed and occur before the protected work.

## Validation

`crates/rustred-legacy-oracles/tests/two_loop_top_dot.rs` freezes an asymmetric raw expansion, replays every
distinct native recurrence target in a small positive box, checks all six
permutations, verifies strict descent including an equal-dot pinch, compares
every target in the finite pipeline's default
`[-2,4]^3` cube, tests numerator/scaleless boundary dispatch, and exercises
each resource and overflow guard. `crates/rustred-legacy-oracles/tests/three_loop_boundary.rs` additionally
uses an induced sunset beyond the retained finite table, replays its native
provenance, checks all 24 parent `S4` routing images, checks a native high-dot
paw IBP subset, and retains the decorated-paw numerator regression.
