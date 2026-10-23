# Refinement checkpoint resume repair — October 21, 2026

A valid generation-5 checkpoint failed with `terminal relations binary:
LimitExceeded`. Its 272,182,620 encoded bytes were below the documented native
transport allowance, but bincode's private fixed 1 GiB decoded-allocation claim
limit counted primitive widths and nested vectors, not encoded varint bytes.
The matching writer could therefore save a checkpoint its reader rejected.

The fixed decoded-claim ceiling is removed, not raised. No schema, coefficient,
cursor, source binding or historical normalization recipe is changed. Outer
caller-selected transport grants and mathematical consistency checks remain.
Generated native checkpoints are not a hostile-input allocation sandbox.

Actual validation used a separate reflinked scratch copy, never the production
phase. The repaired optimized exact CLI loaded generation 5 and cooperatively
saved generation 6 without advancing any work: both native files have BLAKE3
`ee4066c4c376abb2c6440672e53141803993bfa915a0f8912ea0397cc2e6f6a5`
and 272,182,620 bytes. Completed work stayed 901,801, with 33,633 current
independent rows and 594,636 nonzeros. Load plus save took 28.18 seconds and
8,518,576 KiB peak RSS on a shared host. Native exit 4 was the requested pause.
The production file checksum and generation 5 were independently unchanged.

Frozen local exact repair executable:
`/shared/codex-2/rustred/TMP/releases/20261021-checkpoint-resume-repair/rustred`.
SHA256: `bde684441170c2db06736ad182cc46dcab08544107f98393ce9b6fff4c327b38`.
This is an exact-only release build. The opt-in modular delivery is separate.
The local build also includes the pending app native-transport cleanup; that
cleanup and modular work are being integrated separately from this codec fix.

Resume the original campaign, not the scratch validation copy:

```bash
cd /shared/codex-2/rustred
export RUSTRED_CAMPAIGN="$PWD/campaigns/five-loop-minimal-r0-d10-incremental"
nix develop --command python -B examples/python/production_saved_owner_campaign.py \
  --campaign-directory "$RUSTRED_CAMPAIGN" --resume --refine-masters \
  --master-reduction-executable "$PWD/TMP/releases/20261021-checkpoint-resume-repair/rustred" \
  --master-refinement-workers 128 --master-refinement-cpus 64-191 \
  --master-seed-depth 1 --master-containing-sector-depth 1 \
  --master-relation-batch-rows 4096 --master-relation-block-rows 8 \
  --max-memory-bytes 600000000000 --memory-policy process-limit \
  --swap-growth-stop-bytes-per-second 0 --start
```

The resource flags explicitly retain the requested 600 GB process ceiling,
without a host-reserve clamp or swap-growth stop. The existing RAM save-and-stop
margin remains in effect. No campaign restart or deletion is needed.

Local evidence: `TMP/modular-master-refinement-20261021/`, especially
`resume-repair-smoke-v2.*` and `resume-state-before.sha256`.
