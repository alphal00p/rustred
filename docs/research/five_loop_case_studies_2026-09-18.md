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

The same prepared inputs and source policy were run with one and six workers,
using RustRed's sparse exact backend and Symbolica's `SemiNumerical` backend
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

The near-identical one/six-worker times are expected for this deliberately
small one-sector workload: parallel scheduling cannot amortize its fixed
family compilation and parent setup.  They are useful as regression controls,
not as evidence of five-loop scaling.  The semi-numerical and sparse outputs
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

