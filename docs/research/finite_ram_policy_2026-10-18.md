# Finite R5 recovery: user RAM policy instead of hidden work ceilings

## Failure and retained state

The five-loop finite R5/D<=10 invocation `20261017T230917.167071Z` ended
with native exit 4. Its first failure was a conservative transport preflight:
68,644,170 native exponent entries exceeded the inherited 64,000,000 default.
The process guard did not stop it; observed peak RSS was 160.807 GB against
the user's 600 GB ceiling. Other workers then stopped through the shared failure
path. This is not evidence of missing rules, new masters or invalid input scope.

Generation 151816 remains intact, digest
`8fd3356c992192cfe34d1e6c6f0a89382b103e67d167bee1b2f73b08ee768bf5`:

- 500,197,014 committed Work records;
- 47,581,588 input occurrences;
- 360/464 cumulative required rows;
- current row 360, window 357; existing rank-five amendment retained.

The unfinished approximately ten-hour group was not committed and must retry.
No production process was launched, checkpoint edited or source amended during
this repair. A read-only inspection with the new binary reproduces these counts
and the exact generation digest.

## Implementation

The trusted finite campaign's `operation_limits: {}` default removes implicit
local arithmetic, routing-expansion and planning-work ceilings. All fields
remain available as explicit nullable caller quotas. Top-level null deliberately
retains source defaults for diagnostics. General-purpose core API defaults are
unchanged; explicit `unbounded_resources()` constructors are available.

Only quota fields change. Source payloads, routing, rule precedence, guards,
exponent representations, input geometry and original checkpoint identity stay
bound. Resource failure aborts work; it cannot select another rule or promote
an unresolved leaf. Original seed sizes/window partitions are unchanged even
when their separate planning-work quotas are relaxed. Portable publication and
inspection receive the caller's planning policy too.

The high-memory profile has no fixed cumulative manifest/state or raw-union
merge-head byte ceiling. Its null metadata decoder budget is resolved by Python
from the user's process RAM soft threshold: 570 GB for 600 GB with a 5% margin.
Native direct callers must provide a concrete decoder budget. Bincode allocation
claims remain bounded, after subtracting actual retained encoded-buffer capacity.
Resident Work accounting charges actual old/new metadata and live buffers, not
the entire allocation permission. Page/cache chunk sizes remain buffering knobs,
not ceilings on campaign history. Host availability is telemetry in process-limit
mode; the swap-growth stop remains disabled.

Actual resource exhaustion, IO/OS failures, corrupt data, representation overflow
and mathematical failures can still stop execution. This change is not a promise
that every requested calculation fits RAM, terminates, or closes. No custom CAS
or alternative algebra algorithm was added. The symbolic/Epoch lane remains.

## Reproduction and validation

The exact failing descendant was
`[-5,1,-2,2,0,0,1,-1,1,0,1,0,-1,-1,0]`. A generic diagnostic loads its actual
204 KB owner, verifies the recorded loop map using the existing Symbolica-based
services, and executes just that transport, never the dependency graph.

The old preflight failure is reproduced exactly. The uncapped transport completes
in **1.151 s**, producing **1,535,413 exact endpoints**; complete diagnostic time
is **2.011 s** and peak RSS **862 MB**. The conservative preflight support bound
was 3,268,760. These measurements establish feasibility of this immediate operation,
not the duration or completeness of the remaining R5 campaign.

All five fixed native controls complete with byte-identical Work and raw-census
objects, and identical selection/query inputs, against the previous `8ed4…` build:

| Control | Completed Work | Input occurrences | New process wall time |
|---|---:|---:|---:|
| Four-loop FG, singly dotted controls | 6,301 | — | 3.430 s |
| Four-loop BMW, singly dotted controls | 8,913 | — | 4.444 s |
| Four-loop H, singly dotted controls | 49,423 | — | 7.461 s |
| Four-loop X, singly dotted controls | 147,428 | — | 13.514 s |
| Five-loop bounded R1 reuse control | 41,055 | 76,608 | 145.840 s |

These are validation timings on a shared host, not matched performance claims
or the full four-/five-loop physics envelope. The five-loop control is dominated
by cold owner preparation. A separate 150-worker K1 smoke completes 1,200 input
occurrences and 2,400 Work in 0.961 s; it tests the requested worker configuration,
not 150-core scaling.

An old-binary checkpoint -> new-binary resume -> appended scope -> publication ->
cold inspection test passes, including the updated null-metadata/95 GB diagnostic
allocation policy. CURRENT remains unchanged on completed resume and publication.
Public core tests verify uncapped encoded permissions do not disable bounded
decoding, frame integrity, original library defaults or native exponent bounds.
Python tests cover automatic RAM budget resolution, temporary read-only inspection
policies, explicit quotas, immutable source/window policies and command transport.

Independent implementation and mathematical/resource audits were performed by
separate agents. Final native unit-test totals and commit/release identity are
recorded in `CODEX_PROGRESS.md` after the last build gate.

Local evidence:

- `TMP/single-transport-replay-qwMuio/` — actual failing transport;
- `TMP/public-operation-resource-tests-DuvEA5/` — public core tests;
- `TMP/finite-ram-validation-20261018/` — controls, cold resume and production inspection;
- `TMP/finite-r5-resume-plan-20261018-A2nx6e/` — read-only steering audit;
- `TMP/finite-ram-fix-release-20261018/` and `TMP/finite-ram-fix-app-tests-20261018/` — builds.

## Resume, not extend

Select the new frozen executable and updated policy explicitly. The existing R5
amendment and all committed Work are reused; no conversion/deletion is required.
Do not issue another `extend --rank 5`.

```bash
cd /shared/codex-2/rustred
export RUSTRED_CAMPAIGN="$PWD/campaigns/five-loop-finite-r0-d10-resident-fixed-20261016"
nix develop --command python -B examples/python/saved_campaign.py run \
  --campaign "$RUSTRED_CAMPAIGN" --resume \
  --executable "$PWD/TMP/releases/20261018-finite-r5-resource-fix/rustred" \
  --finite-policy examples/input/finite_campaign/high_memory.json \
  --workers 150 --cpus 0-63,96-181 --windows-per-batch 64 \
  --worker-pool persistent --window-preparation parallel \
  --max-memory-bytes 600000000000 --memory-policy process-limit
```

The launch freezes these settings for subsequent resumes. The source templates,
existing rank scope and external legacy campaign are not changed. The separate
release directory is populated only after validation; use the delivery message
and its recorded executable identity, not an intermediate compiler output.
