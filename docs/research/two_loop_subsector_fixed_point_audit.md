# Connected two-loop subsector fixed-point audit

Status: read-only design audit and isolated generated-source probes. No FORM or
Mathematica process was run. The equal-mass sunset is only a concrete oracle;
the recommended mechanism is topology- and loop-count-independent.

## LiteRed ordering that matters

LiteRed runs `AnalyzeSectors`, constructs exact sector mappings in
`FindSymmetries`, and calls `SolvejSector` only on `UniqueSectors`. In
`LiteRed2026.m`, the basis-level `SolvejSector` iterates the unique-sector list
around lines 2304--2318, while `FindSymmetries` creates mapped-sector `jRules`,
`UniqueSectors`, `MappedSectors`, and `SectorsMappings` around lines
3320--3468. Within a sector, `SolvejSector` repeatedly:

1. chooses the current uncovered case and its start point;
2. constructs nearby `preparepoints`;
3. eliminates identities using the ordering at that case/start point;
4. patternizes a successful pivot and calls `WhenBad`;
5. adds its bad locus back to the uncovered-case queue; and
6. increases search depth or freezes more indices when necessary.

At a fully numeric point it adds `SR` and applies `ZerojRule` before
elimination (`LiteRed2026.m` around line 2475). Thus one elimination ordered at
the sector corner is not the whole `SolvejSector` algorithm, even when it
contains translations of the generated equations around nearby points.

## Exact RustRed blocker

The depth-zero family certificate derives a valid top-sector rule for
`J(2,1,1)`. Its factorized RHS contains `J(0,1,2)` and `J(1,0,2)`.

All six verified S3 images of `J(0,1,2)` are unsupported at depth zero:

| sector | images | unsupported candidates |
|---|---|---|
| `011` | `J(0,1,2)`, `J(0,2,1)` | `[0,2]` |
| `101` | `J(1,0,2)`, `J(2,0,1)` | `[1,3]` |
| `110` | `J(1,2,0)`, `J(2,1,0)` | `[0]` |

Consequently symmetry alone cannot close the depth-zero rule. It can remove
the redundant work only after a representative sector has been solved.

A depth-one corner-anchored discovery for canonical sector `011` produces 20
candidates (12 certified and 8 unsupported), 15 global leaves (9 descending
and 6 unsupported), and descending rules for the positive dots tested:

```text
J(0,1,2) -> candidate 11
J(0,2,1) -> candidate 10
J(0,2,2) -> candidate 10
J(0,3,1) -> candidate 10
J(0,1,3) -> candidate 11
```

The corner `J(0,1,1)` and numerator `J(-1,1,1)` remain unsupported. An
end-to-end composition which grew all three factorized sectors to depth one,
installed explicit masters `J(1,1,1)` and `J(0,1,1)`, and used verified S3
mappings still stopped honestly at `J(-1,1,1)` after 275.84 seconds. Positive
dot coverage therefore is not a closed solved-subsector interface.

As a scaling control, the corner-anchored depth-two search for sector `011`
was allowed to run for 1342.65 seconds without producing a result. It held one
CPU core at roughly 99.6% and reached 432768 KiB RSS before the isolated test
was interrupted cleanly. This is a resource/scaling rejection, not an
algebraic result: it neither proves nor disproves depth-two coverage. The
demand-oriented re-anchored search below found the needed generated candidate
in its depth-one stencil, so unselective corner-depth growth is not the
production path.

## Re-anchoring is the smallest missing operation

Rebuilding the generated elimination with the same four canonical IBPs but
with ordering anchor `J(-1,1,1)` finds pivot 10 already in the depth-one
stencil. `GeneratedWhenBadCompiler` authenticates and certifies the candidate;
it is not a hardcoded recurrence. At the concrete audit point its exact RHS is

```text
J(-1,1,1) = J(0,0,1)/(d-1)
            + 2*m2*J(0,0,2)/(d-1)
            - J(0,1,0)/(d-1)
            + m2*J(0,1,1).
```

The required guards include `d-1 != 0` and `m2 != 0`. `WhenBad` classifies
`n0=0` as an exceptional sector leak and covers both tested points `n0=-1`
and `n0=-2`. The one-propagator RHS sectors are certified zero by the existing
zero layer, leaving `m2*J(0,1,1)`.

This distinguishes two operations:

- increasing the translated equation stencil while retaining corner ordering;
- rerunning elimination with a new ordering anchor selected from the remaining
  sector domain.

The second is necessary here. It is precisely the fixed-point behavior that
the current one-anchor family certificate lacks.

The production V4 discovery API now accepts independently bounded search
origins and replays their exact requests, deterministic order, per-origin
layer census, aggregate candidate budget, and one shared generated row span.
The fast sunset regression uses that machinery to regenerate and replay the
four-term candidate above.  Its claim is intentionally point-local: it proves
that the authenticated *parametric* candidate is admissible and applicable at
`J(-1,1,1)`.  It does **not** prove that the symbolic parent residual cell is
entirely covered.  The full 27-candidate global composition regression remains
ignored because its current symbolic overlay takes more than two minutes; this
is a transparent scaling/integration gap, not a correctness shortcut.

## Recommended proof-bearing API and dataflow

The minimal production extension should retain an ordered anchor transcript
inside each sector discovery certificate:

```rust
struct GeneratedSectorSearchAnchor {
    ordinal: usize,
    point: ConcreteIntegralKey,
    source: GeneratedSectorAnchorSource,
    candidate_layer_counts: Box<[usize]>,
    candidate_range: Range<usize>,
}

enum GeneratedSectorAnchorSource {
    SectorCorner,
    RemainingLeaf {
        prior_partition_fingerprint: Arc<str>,
        case: SymbolicSectorCaseId,
    },
}
```

For every anchor, RustRed must regenerate or reuse the same authenticated
IBP/LI row span, build the exact elimination ordering at that anchor, retain
all candidate derivations, and compile their `WhenBad` domains. The final
coverage certificate is rebuilt from the deterministic concatenation of all
anchor candidates. Replay regenerates every anchor's stencil and elimination,
checks its source-leaf membership when present, and compares the complete
merged coverage payload.

Most importantly, anchor membership is only a search-provenance and
usefulness witness.  If the selected source cell is `S`, and a new candidate's
certified admissible and bad domains are `G` and `B`, acceptance yields the
rule domain `S ∩ G` and residual descendants `S ∩ B`.  The compiler may
delete `S` only after exact symbolic composition proves that every residual
intersection is empty.  Observing that the anchor belongs to `G` never proves
that all of `S` does.  A minimal adversarial regression is the massive
one-loop tadpole: the generated recurrence is good at `n=2`, while the same
active source orthant still contains the terminal/bad point `n=1`.

The owning fixed-point transcript therefore also needs the parent-local case
id, exact predicates, prior disposition (including unsupported candidate
ordinals), selected anchor, requested local depth, and full candidate payload.
Case ids and candidate ordinals cannot be compared across independently
rebuilt partitions without their parent certificate: adding a canonically
earlier anchor can renumber both.  Failure to find a concrete witness within a
bounded enumeration is a typed interruption, never an emptiness proof.

A topology-independent anchor scheduler can enumerate the same in-sector
diamond used by LiteRed's `preparepoints`, ordered by the persisted integral
complexity key. It must rebuild elimination at each anchor rather than merely
translate more rows into one corner-ordered elimination. For the audited
sector the first two required anchors are exactly

```text
[0,1,1]  (corner)
[-1,1,1] (first numerator frontier)
```

The family dataflow should then be:

```text
inventory / zero proofs
  -> verified sector symmetry orbits
  -> unique sectors in subsector-first order
  -> anchor fixed point per unique sector
  -> live exceptional-locus re-elimination
  -> merged global and conditional rules
  -> zero -> mapped-sector symmetry -> explicit master -> generated rules
```

No uncovered or unsupported leaf becomes a master. Exhausting an anchor,
partition, or arithmetic limit remains a typed resource interruption.

The concrete application stack for both the base family and this fixed-point
extension must remain

```text
zero(symmetry(master(conditional(global))))
```

with explicit terminal keys canonicalized before master-policy insertion.
All global coverages, conditional queues, and symmetry proofs must retain and
replay the exact same generated-row-span allocation.  The existing shared
provider seam implements this order for the base family; the depth-growth
provider must use that seam as part of its V2 migration.

## Role of symmetry and solved-subsector feedback

Existing `VerifiedInternalFamilyPermutationSymmetry` certificates are enough
for the equal-mass sunset orbit: once `011` is solved, proof-bearing concrete
transport maps `101` and `110` requests into it. The family layer should own a
replayable orbit table and solve only representatives, matching LiteRed's
`UniqueSectors`/`MappedSectors` split. Whole-symbolic-row transport is a
different sound augmentation and does not replace mapped-sector application.

For this exact blocker, zero/subsector feedback closes the re-anchored rule's
RHS but is not what discovers the missing pivot: the generated candidate is
already strictly descending. The decisive missing step is anchor-specific
elimination ordering. More generally, solved proper-subsector rules and the
concrete zero/symmetry quotient must also be available before elimination,
because they can change rank and pivot orientation exactly as in LiteRed's
numeric branch.

The resulting mechanism remains fully generic: neither anchor generation,
row authentication, symmetry transport, zero feedback, nor rule application
contains a sunset label, loop-count branch, recurrence coefficient, or master
inference.
