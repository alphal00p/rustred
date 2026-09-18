# Five-loop vacuum case studies (2026-09-18)

These are bounded performance studies, not closing-artifact or certification
claims.  Both families are ordinary caller-supplied Symbolica/RustRed input;
RustRed does not dispatch on either name or graph.  They have five loop
momenta, no external momenta, unit mass, and `K = L(L+1)/2 = 15` quadratic
forms.  One family contains all fifteen pairwise scalar products; the other
uses a chain/control incidence pattern plus five independent auxiliary forms.
The exact input files are shipped as examples:

- `examples/input/five_loop_complete_scalar_product.toml`
- `examples/input/five_loop_chain_with_isps.toml`

The study submits only the all-positive parent sector.  It therefore measures
source preparation and one parent-sector solve, not a family closure.  The
resulting status is intentionally `uncertified-parent-study`; each run found
15 rules and one finite residual, with no proven-zero sectors in this generic
family.  No artifact was certified, serialized, or admitted to Vakint.

## Release measurements

The retained prepared-client program (`TMP/rustred-five-loop-study.rs`) ran
the same input families with one and six workers, using RustRed's sparse exact
backend and Symbolica's `SemiNumerical` backend
(degree 128, 200,000 probes, four attempts, eight primes).  Wall times below
include preparation and solve but exclude compilation.  The solve times are
reported separately by the client in microseconds.

| Family | Backend | Workers | Wall | Prepare | Solve | Rules | Residuals |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| complete scalar-product | sparse | 1 | 27.03 s | 15.294 s | 12.041 s | 15 | 1 |
| complete scalar-product | sparse | 6 | 27.11 s | 15.093 s | 11.999 s | 15 | 1 |
| complete scalar-product | semi-numerical | 1 | 34.30 s | 20.312 s | 13.986 s | 15 | 1 |
| complete scalar-product | semi-numerical | 6 | 27.23 s | 15.222 s | 11.986 s | 15 | 1 |
| chain/control + ISPs | sparse | 1 | 32.79 s | 14.363 s | 18.403 s | 15 | 1 |
| chain/control + ISPs | sparse | 6 | 32.67 s | 14.360 s | 18.287 s | 15 | 1 |
| chain/control + ISPs | semi-numerical | 1 | 32.86 s | 14.548 s | 18.292 s | 15 | 1 |
| chain/control + ISPs | semi-numerical | 6 | 32.68 s | 14.374 s | 18.284 s | 15 | 1 |

The client submits exactly one all-positive parent sector (`solved_sectors=1`;
`rules=15`; `finite_residuals=1`) on each run. It does not enumerate or solve
the remaining sectors. The public `family-candidates` command has a different
workload: its all-positive root admits every inconclusive subsector, and its
candidate-bundle path serializes every returned solution. Therefore the
27--34-second prepared-client rows cannot be compared with a
`family-candidates` run that continues beyond 15 minutes; they are not a
family-wide closure or scaling measurement. The near-identical one/six-worker
times are expected for this deliberately small one-sector workload: parallel
scheduling cannot amortize its fixed family compilation and parent setup. They
are useful as parent-sector regression controls, not as evidence of five-loop
scaling. The semi-numerical and sparse outputs
were both generated successfully; this is a backend equivalence smoke check,
not an exact output-equivalence or reconstruction certificate.

## Reproduction

The release client used for the measurements is retained only in `TMP/` and
is not part of the repository.  A normal RustRed caller can reproduce the
same workload through `family-candidates` or the Rust/Python family APIs.  Set
the Symbolica license in the environment, use a release build, and request
`n_cores=1` or `6`.  Keep the resulting bundle labelled uncertified until a
separate closure/certification campaign proves all reachable sectors and
guards.

## Follow-up release probe (2026-09-18)

I repeated the complete-scalar-product parent command from the workspace's
current `target/release/rustred` binary (SHA-256
`d550c94a87c60e7afa226e8f006ddf81eba8bb8787ed7c2f62758dc5a7f767a3`) with
`SYMBOLICA_LICENSE` set and `--n-cores 1`. The exact command was:

```console
target/release/rustred family-candidates \
  --input examples/input/five_loop_complete_scalar_product.toml \
  --input-format toml \
  --output TMP/five-loop-current/complete-sparse-w1.toml \
  --report-output TMP/five-loop-current/complete-sparse-w1.report.toml \
  --n-cores 1 --force
```

This probe was deliberately capped at 15 minutes so it could not consume an
unbounded worker slot. It produced no candidate bundle, report, stdout, or
stderr before termination. Shell timing recorded `real 15m3.426s`,
`user 14m50.209s`, and `sys 0m1.511s`; sampled peak resident memory was about
638,420 KiB immediately before SIGTERM (status 143). This is a
resource-censored run, not a failed mathematical solve and not a replacement
for the completed matrix above. The earlier 27--33 second measurements were
made by the retained one-sector prepared client, not by `family-candidates`,
and did not use a recorded CPU-affinity or isolated-host protocol. Until those
measurements are repeated under a documented paired host/affinity setup, they
must not be called a controlled benchmark, regression, or combined into a
speedup claim. This follow-up does not
establish closure, reconstruction success, or a new five-loop performance
baseline.

The retained prepared-client binary (`TMP/rustred-five-loop-study`, SHA-256
`9a617db58c90f966c9f8e25e984ff36bedc1566fb9b38563889fa0814e8b2d08`) was
also rerun with the `.input` (Symbolica syntax) files, using the same 0.1 s
RSS sampler. The resulting partial rerun is:

| Family | Backend | Workers | Wall | Prepare | Solve | Peak RSS | Status |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| complete scalar-product | sparse | 1 | 108.654 s | 54.736 s | 53.633 s | 166,864 KiB | completed |
| complete scalar-product | sparse | 6 | 120.015 s | 69.568 s | 50.144 s | 190,024 KiB | completed |
| complete scalar-product | semi-numerical | 1 | 127.514 s | 70.655 s | 56.807 s | 166,852 KiB | completed |
| chain/control + ISPs | sparse | 1 | 126.023 s | 56.858 s | 69.087 s | 177,328 KiB | completed |

These runs found the same `15` rules and one finite residual as the original
one-sector matrix. They were made on a heavily shared host (load average about 95--125
at the time), without CPU affinity, so they are diagnostic observations and
not a replacement for the controlled one/six-worker matrix. In particular,
they should not be interpreted as an algorithmic slowdown until a paired,
isolated rerun is made under the original host and affinity conditions. The
semi-numerical row completed successfully and used Symbolica's reconstruction
backend; no custom reconstruction code was introduced.
