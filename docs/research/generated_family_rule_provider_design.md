# Generated family rule provider

`GeneratedFamilyRuleSystemProvider` is the concrete application boundary for
`GeneratedFamilyRuleSystemCertificate`. It is topology- and loop-independent:
the constructor receives no recurrence, topology label, loop count, or master
count. It replays the family transcript and installs exactly the generated
coverage and live-leaf certificates retained by that transcript.

The provider stack has the fixed order

```text
certified zero-sector policy
  -> certified compatible-symmetry canonicalization
    -> explicit caller master policy
      -> conditional live-leaf rules
        -> global parametric sector rules
```

Putting zero analysis outermost prevents an accidentally selected zero key
from shadowing an analytic zero certificate or a cut zero. The symmetry layer
uses only row-span certificates proved compatible with the exact cut and
sector-pattern policy. It rewrites a noncanonical concrete key to the unique
easiest member of its bounded verified orbit before master/rule lookup, and
retains a replayable proof for that rewrite. Master terminals are empty by
default and may only be inserted through the checked caller-policy API; their
keys are canonicalized before insertion, so equal orbit declarations
deduplicate and conflicting declarations fail transactionally. In particular,
`Uncovered` and `Unsupported` are never renamed masters, and an unsupported
global candidate remains a typed `UnsupportedLeaf` after an exhausted
conditional scan.

Construction rejects every family transcript containing `ResourceLimited` or
`Failed`, retaining the exact sector, pipeline stage, and nested interruption
in the error. A successful `Unresolved` transcript is accepted because it
means the generated stages completed, not that the sector was solved. The
provider retains and replays the original family certificate, restrictions,
formal power-shift policy, ordering, zero-analysis limits, global coverages,
complete conditional queue payloads, and explicit master policy. Aggregate
preflight limits bound sectors, candidate attempts, global leaves, queue work
items, consumed terminal declarations (including duplicates), symmetry orbit
work, and lazily retained symmetry proofs. A failed multi-generator symmetry
path commits none of its staged proof cache.

## Current oracle boundary

The family-certificate path reduces the massive one-loop tadpole through power
four to the frozen Vakint coefficients, with `I(1)` selected explicitly.  The
Symbolica-`Atom` Vakint numerator fixture also runs end to end through this
same family certificate/provider: bounded Atom parsing, authenticated tensor
projection, scalar-product lowering, generated parametric IBP application,
and comparison in the unreplaced `I(1)` master basis.  Concrete powers and
the frozen Vakint expression occur only in that oracle test.

For the connected equal-mass two-loop sunset at discovery depth zero, the
generated top-sector rule for `J(2,1,1)` has RHS support

```text
J(0,1,2), J(1,0,2), J(1,1,1), J(1,1,2).
```

The exact concrete coverage classifications are:

```text
J(0,1,2): Unsupported candidates [0, 2], no conditional pivots
J(1,0,2): Unsupported candidates [1, 3], no conditional pivots
J(1,1,2): DescendingRule candidate 2, no conditional pivots
```

Thus the depth-zero blocker is factorized-sector closure, not absence of a
top-sector first step. RustRed reports the typed unsupported sector `011`; it
does not install the Vakint recurrence or promote that boundary integral to a
master.

A separate family-wide discovery-depth-one diagnostic was stopped after more
than 4 minutes 15 seconds at roughly 332 MB resident memory without producing
a sector result. This is a performance observation only: it is not a retained
RustRed resource interruption or an algebraic failure. The durable family-wide
provider oracle therefore uses the fully replayed depth-zero certificate,
while the separately tested single-sector depth-one scaling certificate remains
distinct.
