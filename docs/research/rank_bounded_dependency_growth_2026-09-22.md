# Input rank versus intermediate dependency work

This is a mathematical and source-code audit for the shared five-loop campaign,
not a completed R10 result. Independent certification remains deferred. The
purpose is to avoid confusing an expanding work representation with missing
IBP identities, while still delivering an operationally complete reduction.

## Descent does not imply a uniform intermediate-rank bound

Consider a toy integral indexed by positive-power excess `d >= 0` and numerator
rank `r >= 0`, with these three rules:

```text
d >= 2:       (d, r) -> (d - 2, r + 1)
d = 1:        (1, r) -> (0, r)
d = 0, r > 0: (0, r) -> (0, r - 1)
terminal:     (0, 0)
```

Every step strictly decreases the nonnegative integer `d + r`. Every concrete
input therefore terminates at the same single terminal. Nevertheless, the
input family `d >= 0, r <= 10` has no uniform finite bound on intermediate
rank: starting at `(2t, 10)` reaches `(0, 10 + t)` for every nonnegative `t`.
A worklist enumerating successive finite-rank layers need not exhaust, even
though three generic rules already solve the entire family.

This is **not an identified RustRed IBP or a diagnosed infinite chain in the
live campaign**. It establishes only that strict descent, finite individual
paths and a finite terminal set do not by themselves justify waiting for a
finite-rank-layer worklist to exhaust. Clipping intermediates to the input
rank is incorrect. Replacing the rank by an unbounded domain silently would
instead ask a stronger question and is not evidence that it has been solved.

## What the current implementation preserves

The selected-rule application visitor splits sign-crossing coordinates and
keeps their fixed values. Its translated box and rank simplex are the exact
image of each resulting source cell. Same-support inactive coordinates shift
linearly; fixed crossing coordinates contribute a constant to the rank bound.
There is no generic loss of rank correlations in this image calculation.

Each original potentially nonzero term still passes source-validity and
owner-order descent checks, or authenticated zero handling, before equal-shift
cancellation. Queue containment
reuses a previously admitted domain; it does not itself construct a larger
hull or raise a rank. Pending containment is scheduling reuse, not completion.

## Where extra work can enter

- A coefficient whose nonzero support is unresolved produces a conditional
  successor over its whole image. Its nonzero predicate is not carried into
  the current box-only worklist.
- For an uninstalled child owner, conservative routing discards the child's
  lower and upper bounds and requests a whole support/rank domain. Routed
  application likewise receives a full orthant. Numerator distributions,
  fixed positive powers and the actual support/cancellations of the routed
  numerator substitution can therefore be lost. Equal-shift RHS cancellation
  remains native and exact before routing.
- Routing does not itself raise the rank bound, and the implemented strict-
  pinch bound `R-k` remains sound. Broadening the domain can nevertheless allow
  later applications that the tighter predecessor would not have required.
- Per-owner descent and an authenticated momentum map are not, by themselves,
  a global ordering-compatibility argument for all routed transitions.

Coupled applicability conditions are retained as unresolved work, not erased.
Saved native affine cases already describe some of those conditions; the
box-only matcher currently cannot consume every diagonal case. This is a
separate limitation from any demonstrated rank-growth mechanism.

## How this changes the next experiment

After inspecting the control's saved records, choose a repeated owner/rank
pattern and recover one bounded causal witness using the selected-rule
visitor: parent cell, rule and shift, conditional coefficient status, exact
pre-route image, route, and resulting cover. Current domain summaries do not
retain that entire linkage, so a rising maximum alone cannot supply it.

If routing or dropped predicates cause excess work, preserve the relevant
native case, guards and tight images in the application representation. Guarded
regions must not enter box-only containment with their predicates discarded.
If a real recurrence trades unbounded positive powers for numerator rank,
first investigate an already-valid alternative rule or a parametric recurrence
representation, rather than enumerating ever-higher rank layers. A targeted
native source search is appropriate only for an actual gap or an explicitly
labelled search overcover, not merely an unknown classifier result.

This remains a path to the requested full R10 solve, not a weaker success
criterion. It does not authorize terminal minimization, numerical-master
collection, five-loop Vakint integration, blanket owner regeneration, positive-
power truncation, or a new general certification project before that solve.

Local audit evidence: `TMP/symbolic-rank-growth-audit.cFcY7F/AUDIT.md`.
Relevant implementation: `owners/domains/applied/geometry.rs`,
`owners/domains/applied/engine.rs`, and the application's
`routed_campaign/walking/{inspection,routing,queue}.rs` modules.
