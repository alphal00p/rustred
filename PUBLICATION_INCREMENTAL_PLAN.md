# Faster repeated saved-campaign publication

## Objective

Make publication after a rank extension substantially cheaper without changing
the resulting rule or terminal inventory, weakening the declared assurance, or
touching the currently running THE_ONE publication. Deep verification remains
optional and disabled by default.

## Work lanes

1. `restore_parallel_fix`: reusable native inventory/projection state for
   authenticated append-only CP6 history. Reuse only with explicit compatible
   owner, request, record-history and rescue-state bindings; otherwise use the
   existing full path. Preserve exact event multiplicities and terminal/rule
   identities, not just aggregate counts.
2. `restore_capacity_fix`: accelerate unavoidable bootstrap/fallback record
   decoding, preserving validation, cancellation, bounded memory and worker
   budgets. Coordinate shared loading interfaces with lane 1.
3. `rank6_restore_audit`: independent adversarial design and implementation
   review, including mutation, abandoned records, amended requests, partial/G2
   residuals, error-prefix replay and checkpoint portability.
4. Root: integration with previous-artifact publication, release builds,
   end-to-end full-versus-incremental controls, measurements, documentation and
   commit/push. No production process or campaign data is changed.

## Acceptance

- Exact full-versus-incremental inventory and final native payload comparison
  on real four-loop and finite five-loop saved-scope controls; native chained
  rank-extension controls, plus the public four-loop extension lifecycle.
- Safe fallback when reuse prerequisites are absent or changed; corrupt cache
  data must never authorize a successful publication.
- Cancellation cannot publish a partial cache as complete.
- Deep verification still performs its independent checks.
- Report bootstrap and warm-extension timings separately, including work
  actually avoided; do not infer whole-campaign speedups from microbenchmarks.
- Preserve unrelated work. Commit and push the audited, tested change using
  the requested Git identity and provide the optimized executable to use for
  the next publication/extension.

## Starting evidence (October 11, Europe/Zurich)

THE_ONE rank 6 has completed: generation 35, 469 required queries resolved,
140,465,585 closed saved domains, zero pending domains/frontiers. Publication
uses `--no-deep-verification`; nevertheless it currently decodes 140,465,585
records (73.06 GB), then prepares owners, checks saved graph readiness and
reclassifies encountered Apply scopes. The actual previous artifact required
about 52 minutes; its 139,772,626-domain history differs by only about 0.5%.
Previous-artifact reuse currently occurs after this census, so does not avoid
the dominant repeated work.

## Chosen bounded implementation

Reuse a small raw-census aggregate for an exactly authenticated immutable CP6
record prefix. Current file decoding, record/domain/G2 parity, graph readiness
and required-query checks still run. This avoids duplicating the large execution
checkpoint or weakening the normal publication checks. The cache is neither a
list of normalized masters nor a certificate; it preserves actual raw rule IDs,
per-rule multiplicities and terminal keys. It is absent for unsupported rescue,
failed/interrupted records, finite replay or deep verification.

The second lane parallelizes the existing binary-record diagnostic projection
and decoding in bounded ordered batches and removes per-record temporary domain
allocations. The authenticated reader and ordered state updates remain serial.
Batch size and actual speedups must be selected from release measurements.

Release probes on 628,236 actual THE_ONE records support a 4,096-record batch
with the existing 16 MiB byte limit. In paired forward/reverse probes it cut
projection time about 14% at 32 workers and 5% at 16 versus 256-record batches.
Four-worker differences were noisy. This is a component result, not evidence
that increasing worker count always improves whole-publication time.

Previously published artifacts have no cache. Their next publication therefore
uses the faster full loader and bootstraps cache state; subsequent compatible
extensions can avoid rematching the previous prefix. No change to the running
production process can retroactively add this state to its executable.
