# Reproduce endpoint-local source-weight discovery

These two **example inputs**, not topology-specific engine code, reproduce the
successful all25-row search described in
[the nomination note](../../../../../docs/research/banana_moment_nomination_2026-10-04.md#source-nomination-after-the-raw25-locality-miss).
They contain no source weights. The family, source order, translations, charts,
protected axes and finite resource limits are explicit inputs; only the source
combination is discovered. Every operative field equals the audited request;
only historical machine-local `provenance` metadata was removed.

Both tracked files were subsequently proved natively in one4.448-second guarded
pair. Their complete reports matched the earlier successful reports after
removing only elapsed seconds and historical request provenance. This parity
check reused the audited optimized research binary; it was not a fresh Cargo
rebuild or an export test. Raw receipts remain under ignored
`TMP/rule-optimizer-20261003/candidates/banana-owner0-endpoint-locality-portable-v1/`.

## Required owner

Supply your own native generated owner bundle, `owner.rrbin`; no owner bank,
campaign output or machine-specific binary is distributed here. These are not
universal requests: the native loader must admit a15-index owner with mask
`000011001001011`, the exact `tide_five_loop_common_basis` family fingerprint
embedded in the requests, and persisted order
`rustred.spired-uncut-sector-order.v1`. The published reference used the original
483-rule owner with root bits `111011111101011`. A different root, saved rule
bank, terminal inventory or compatible executable can change proof/export
behavior. Do not edit the fingerprint or strip guards to force admission.
The owner mask selects this lower sector; the larger persisted root is its
parent program's source/root-admissibility scope, not another target sector.
The reference native projector report exposes that value as `bound_owner_root`.

In this family, `a=n11` is the D12 power, `b=n14` the D15 power, and
`r=-n12` the D13 numerator rank (indices here are zero-based). Both inputs
retain arbitrary positive spectator powers on axes `[4,5,8,13]` and require all
other inactive indices to be zero. Their lower/upper coordinates are native
excess coordinates, not physical integral powers.

| Request | Full physical chart | Common source translation |
| --- | --- | --- |
| `banana-b-ge2.json` | `a>=1, b>=2, r>=1` | `+e12-e14` (zero-based) |
| `banana-b-eq1.json` | `a>=2, b=1, r>=1` | `+e12-e11` (zero-based) |

Each declares all25 ordinary rows in contraction-major/differentiated-loop-minor
order, `forbid_endpoint_changes_on_axes: [4,5,8,13]`, fresh original-source
certification and zero refinements. No row shortlist, eight known weights,
numeric sampling or adaptive growth is supplied.

## Build and prove one chart

From the repository root, build the existing example using the repository's
normal Rust/Symbolica environment and license configuration:

```sh
cargo build --locked --release -p rustred-app --example rule_optimizer_symbolic_projector
```

With a populated offline Nix environment, the same command can be run as
`nix develop --offline --command cargo build --locked --offline --release -p rustred-app --example rule_optimizer_symbolic_projector`.

Then set the paths explicitly. If `CARGO_TARGET_DIR` is configured, adjust the
binary path. Choose a fresh output directory under the workspace; its parent
must already exist. Repeat with the second request and another fresh directory
to prove the other chart.

```sh
PROJECTOR_BIN="$PWD/target/release/examples/rule_optimizer_symbolic_projector"
OWNER_FILE="/absolute/path/to/owner.rrbin"
REQUEST_FILE="$PWD/tools/research/rule_optimizer/examples/endpoint_locality/banana-b-ge2.json"
RUN_DIR="$PWD/TMP/endpoint-locality-b-ge2"
(
  set -eu
  set -C
  mkdir "$RUN_DIR"
  "$PROJECTOR_BIN" validate "$REQUEST_FILE" >"$RUN_DIR/validate.json" 2>"$RUN_DIR/validate.stderr"
  "$PROJECTOR_BIN" prove "$OWNER_FILE" "$REQUEST_FILE" >"$RUN_DIR/prove.json" 2>"$RUN_DIR/prove.stderr"
  python3 -c 'import json,sys; r=json.load(open(sys.argv[1])); assert r["status"] == "EXACT_ORIGINAL_SOURCE_CHART_PROVED", r["status"]' "$RUN_DIR/prove.json"
)
```

These are direct native commands, **not a resource supervisor**. Run them within
your existing approved CPU/RSS/deadline guard; do not compete with an active
campaign. The reference pair used one native CPU,16GiB process-tree memory,
150GB host reserve and300 seconds inclusive, with no retry. Native input limits
do not bound every internal allocation. Compilation was measured separately.
Keep both raw JSON and stderr even on refusal or cancellation; never infer
success from process exit0 alone. A completed no-target result is a fixed-bank
miss; rejection of the first proposed rule does not exclude other combinations.

Expected reference traits: both exact chart proofs,25 admitted source rows,
eight discovered contributions, seven generic RHS tails, no spectator shifts,
four tails at rank one, and all pinches retained. The broad chart has eight
native cells, the mirror four. These are observed traits, not an extra proof
axiom or permission to drop any unmatched term. Algebraic validity remains the
native original-source, guard and descent check. Rendered coefficients are
display-only; do not parse them into a new algebra implementation.

## Optional checked export

Only after inspecting the successful proof, request a fresh native export:

```sh
(
  set -eu
  set -C
  test ! -e "$RUN_DIR/export"
  "$PROJECTOR_BIN" export "$OWNER_FILE" "$REQUEST_FILE" "$RUN_DIR/export" >"$RUN_DIR/export.json" 2>"$RUN_DIR/export.stderr"
  python3 -c 'import json,sys; from pathlib import Path; r=json.load(open(sys.argv[1])); assert r["status"] == "CHECKED_PRIORITY_OWNER_EXPORTED", r["status"]; assert (Path(sys.argv[2])/"candidate.rrbin").is_file()' "$RUN_DIR/export.json" "$RUN_DIR/export"
)
```

Export performs native search/proof again and the existing checked exporter
rechecks the source request. It may legitimately report
`EXACT_CHART_PROVED_EXPORT_REFUSED`; retain that negative rather than removing
conditions or changing the chart. A successful export writes `candidate.rrbin`,
`request.json` and `proof-export.json`. The exporter preserves the original
rules/terminal boundary and uses `AfterBaselinePartitionWholePiece`; it does
not install the candidate, update a campaign, or certify closure/performance.
The JSON report is evidence, not a portable independent source certificate.

Separate exports from the same owner are alternatives, not automatically one
combined bank. Combining the two requires explicitly supplying the first
exported candidate as the second command's owner and proving/exporting again.
Any repair overlays bound to the original owner must be officially re-exported
against the final owner before campaign use; this runbook does not rebind or
copy them. The previously measured combined bank is not distributed here.
Neither chart covers `a=b=1` or mixed numerators. A new shared-workload/cold
comparison is required for any performance or campaign recommendation.
