# Inspect shared parametric successor requirements

`owner-domain-scan` scans saved owner rules once, rather than expanding a list
of concrete integrals. Positive powers remain unbounded. The purpose is to find
where a shared campaign may require additional owner domains or higher
intermediate numerator rank before scheduling new source searches.

The scan is a **conservative one-hop inventory**, not a completed campaign or
independent certificate. It includes every stored formula without subtracting
earlier rules/terminals. Native equalities, excluded AND-conjunctions, original
denominators and coefficient conditions stay attached in the Rust service;
their satisfiability is not decided. A potential edge is therefore not a proved
missing rule. No owner program is regenerated or modified.

## Python and CLI

Use the same external saved-owner selection as the
[shared campaign driver](shared_owner_campaign_driver.md). For example:

```bash
python examples/python/scan_shared_owner_domains.py \
  --executable target/release/rustred \
  --manifest selection.json --owner-base . \
  --max-numerator-rank 10 \
  --output domains.json --events domains.jsonl
```

The script inherits the license, configures one native worker and invokes:

```bash
rustred owner-domain-scan --manifest selection.json --owner-base . \
  --max-numerator-rank 10 --output domains.json --events domains.jsonl
```

To inspect all saved rule geometry without a numerator-rank prefilter, replace
`--max-numerator-rank 10` with `--unbounded-rank` in either command. Exactly one
scope selector is required. Unbounded scope is not a large finite rank and does
not imply that the saved rules cover that scope.

Direct CLI use requires the same serial inner-pool environment as
`routed-campaign`. Existing outputs are never overwritten. `--stop-file PATH`
supports cooperative cancellation. Live TTY output overwrites its dashboard;
non-TTY monitoring records structured heartbeats. There is no elapsed deadline.
Completion heartbeats contain bounded counts/status/errors, not the potentially
large successor-group array; that detailed array remains in the output file.
The Python script introduces no extra worker pool; caller affinity and memory
limits apply to the replaced native process.

The result identifies source owners, possible successor sectors, same-support
rank changes, conservative **one-step** target-rank bounds, known zero sectors
and available installed owners/verified routes. Summary groups distinguish a
nominally unbounded positive coordinate from fully bounded positive boxes and
flag additional native equalities. A box remains a prefilter: those equalities
may further bound it or make it empty. Higher-rank successors are not clipped.

Successful output has `scan_complete: true`, but always
`family_closure_claim: false`, `priority_overapproximation: true` and
`guard_satisfiability_decided: false`. Resource/cancellation errors leave an
explicit incomplete prefix. Aggregate `retained_regions` counts callbacks that
fit the summary budget; an owner's `regions` also counts the callback rejected
when a summary cap is reached. Neither is a count of solved integrals.

Both the Python script and direct CLI accept `--max-rules-per-owner`, `--max-terms-per-owner`,
`--max-regions-per-owner`, `--max-total-regions` and `--max-summary-groups` allow
bounded scans. These are work/storage allowances, not a mathematical scope
restriction. `--max-numerator-rank` or `--unbounded-rank` selects the requested
input domain.
The default summary allowance is 16,384 groups; an explicit value may be raised
up to 1,000,000. Exhausting it returns an incomplete prefix, not a coverage claim.

## Structural shift census

The report and each owner row include `structural_census`, accumulated before
successors are grouped. It records the largest observed sum of absolute RHS
index shifts, strict-pinch shifts, possible support changes, and same-support
changes in `P=A+R`. Rank increase alone need not increase P: the shift
`(-2,0,-1)` on a same-support cell of support `110` lowers A by two and raises R
by one, so ΔP=-1.
Counts are potential sign regions, not distinct integrals or proved reachable
transitions. Up to four diagnostic examples per obstruction kind are retained.

At report level, `complete_unbounded_shift_l1_bound` is a decimal string only
after every installed owner finishes an unbounded scan; it is null for a
finite-rank scan or an incomplete aggregate. A successfully completed earlier owner's own bound remains
owner-local when a later owner fails. Original identically-zero RHS terms are
excluded; no sampled cancellation, guard satisfiability or first-rule priority
is assumed. This information can guide a finite-cover experiment, but it proves
neither coverage nor validity of the saved formulas as original IBP consequences.

## Rust boundary

`owner_domain_scan_with_progress(OwnerDomainScanRequest, ...)` owns input loading,
normal route validation and the bounded report. The underlying
`CandidateOwnerPrograms::visit_owner_rule_successors` streams descriptors with
borrowed native guards, original-coordinate sign cells and exact RHS shifts.
It neither clones the formula library nor invokes a proof/replay engine.

The next campaign layer must discharge these guarded obligations against
existing domains, identify genuine uncovered source cases and reuse the
retained source-feedback service. Merely finishing this inventory does not
implement that worklist or establish recursive R=10 closure.
