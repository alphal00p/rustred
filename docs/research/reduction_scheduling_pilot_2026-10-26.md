# Experimental reduction scheduling: completed private tuning pilot

Date: 2026-10-26. This development branch preserves opt-in research code and
completed exploratory observations. Canonical root and dependency ordering
remain the defaults. There is no production-policy recommendation, Vakint pin
change, global weighted-frontier implementation, or physical-integrand result.
The [source study](reduction_scheduling_study_2026-10-26.md) explains the dispatch,
descent, guard, cache and scheduling contracts.

## Scope and exact request

Four policies were preregistered and run once each, in the table order below,
with a fresh process and empty application cache. This is a synthetic weighted
sum of authenticated parent-31740 tuning roots, not the original public
integrand. It has a known routed component shared with the frozen held-out
identity set. It is therefore **PRIVATE TUNING / SHARED-COMPONENT DIAGNOSTIC**,
not independent held-out evidence or cache-training evidence. No exported cache
or portable usage profile was produced.

The request uses schema `rustred.saved-application-scheduling-request.v1`, ID
`tuning-31740-root-sum-synthetic-v1`, arity 15 and `entry_mode = routed`. The
ordered terms are:

| Powers | Exact coefficient |
| --- | --- |
| `[1,0,1,1,2,2,1,0,1,1,0,0,0,0,0]` | `(d^2+d+1)/7` |
| `[1,1,1,1,2,2,1,0,1,0,0,0,0,0,0]` | `-1/2` |
| `[1,1,1,1,2,2,1,0,1,1,-1,0,0,0,0]` | `(d+1)/3` |
| `[1,1,1,1,2,2,1,0,1,1,0,0,-1,0,0]` | `-2/5` |

The original 1,110-byte request JSON was frozen before timing, SHA256
`372d34ed9f242ee1b7405822a84d1f6e6744500405526e08fb05c355de3f3ce2`.
The immutable external data checkout was
`d4384149b5076de84b3f3fda761585dff1776532`, using `5L/ibps`; its root manifest
SHA256 was `794c0e4b44a1c6be1d78f8ebdbca188473678823ea1f3b821f70b63b2a6dd5a7`.
The study-specific example explicitly selected the existing
`entry_coverage_policy = Advisory` because some transported entries are outside
the recorded starting domain. It retained `UnresolvedPolicy::Reject` and
required complete declared closure. Advisory is not the library default and
does not certify recorded entry coverage.

## Matched execution and observations

All four samples used one optimized example executable, one mathematical worker,
CPU 149, a 16 GiB process-group memory limit, zero swap, a 240-second diagnostic
target and a 300-second external hard wall limit. Each exact worker was
registered before Native/data activation with the task guard enforcing host
`MemAvailable >= 50,000,000,000` bytes. There were no retries, warm-ups,
persistent backing, capture, evictions or recomputed nodes.

| Root order | Dependency order | Application seconds | Package load seconds | Process VmHWM bytes |
| --- | --- | ---: | ---: | ---: |
| `canonical` | `canonical` | 3.474096323 | 104.471857423 | 3,015,516,160 |
| `numerator-rank-descending` | `canonical` | 3.194117724 | 104.944250733 | 3,017,244,672 |
| `canonical` | `positive-power-sum-descending` | 3.504069977 | 107.967676478 | 3,014,156,288 |
| `coefficient-terms-descending` | `canonical` | 3.399341941 | 104.859397148 | 3,016,265,728 |

Application wall time includes terminal substitutions. VmHWM is the process
lifetime high-water mark through application return: it includes setup and
excludes subsequent serialization/checks. Setup dominates these standalone runs.
These are four single observations, not medians, confidence intervals or an
estimate of general speedup. The approximately 8.1% lower rank-first application
observation is a lead for a future preregistered repeated comparison only.

Every sample completed with 11 declared terminal terms, zero unresolved terms
and 140,246 exact unique nonzero obligations. Outside the application timer,
the complete family-qualified Native-rendered coefficient map and every exact
guard expression matched the canonical control by identity in the same
coefficient field/cohort. This was not a count-only or sampled-numerical check.
Both root-order variants changed the raw guard vector order; the full guard sets
were identical. Dependency-only ordering also preserved the original vector.

Every reported Native work counter was identical across policies: 23,813
operational nodes, 10,886 rule applications, 174,569 cache hits, 286,624
coalescing additions, 8,069 transport calls, 159,794 transport endpoints,
1,021,542 transport operations and 28 terminal-substitution additions. All
samples ended with 23,796 cached integrals, 1,514,061 coefficient terms and
88,309,722 retained coefficient bytes. No measured reduction in graph work or
cancellation work was established. Separate Route/Apply node counts, maximum
intermediate coefficient growth, actual zero-cancellation counts and narrow
IBP/materialization timing were not measured.

All actual Native workers exited zero. The unchanged outer actor subsequently
exited one because its 1 MiB metadata reader rejected each approximately 69 MB
scientific result. Original actor failures were preserved; separate scientific
postflights checked the actual worker exits, complete results and guard
registration. No worker was rerun and the metadata-reader policy was not
weakened. All per-job guards were retired after worker exit without signals or
abort/latch events.

## Reproduction identities and engineering evidence

The measured source was `8823121ba065d93ffbd2bd77c5d0c88c321f1835`, tree
`1c0db9a18a67cfc2cee7f9dd8fd50e1035b20f16`. Later documentation commits are not
new compiled cohorts. The kernel was
`9cd57678d8817ec33c05cacba4925162f1c86b0b`. The exact executable SHA256 was
`8f6a00eecbb4523cc3c161d51f800e8f3e765e28591af871615d12dd4cad05f7`.
It was built with arity registry `1,15` using:

```text
RUSTRED_RUNTIME_ARITIES=1,15 cargo build --release --locked --offline -p rustred-app --no-default-features --features native,runtime-arity-selection --example saved_application
```

The recorded driver, RustRed and kernel library profiles used optimization level
3, no debug information or debug assertions, and were not test profiles. The
example's existing `scheduling-sample` command takes the package, frozen request,
root policy, dependency policy, new output path and operational-options file.
The measured options explicitly selected Advisory entry coverage; no other
solver budget was added. Its required five-field task-admission environment and
external resource-controller configuration are environment-owned controls,
not distributed launch configuration or authorization. Reproduction requires
separate data/license access and current resource admission.

The final focused core suite passed 87 tests. The applicable saved-application
subset passed 117 tests with two pre-existing ignores. The original broader
attempt also retained one configuration failure: an existing arity-three test
could not run with the then-compiled registry `[1]`; no assertion was weakened.
This is not a full-workspace or normal full-arity suite PASS. The final example
suite passed 16 tests with one real-controller test ignored; actual admission
for the four scientific workers was checked separately. No new mathematical
execution is needed to publish these source and documentation changes.

The local evidence remains outside Git. Relative to the campaign validation
directory, the principal retained artifacts and SHA256 identities are:

| Artifact | SHA256 |
| --- | --- |
| `private-scheduling-8823-runtime/scientific-summary.json` | `d3fe0ce59b086c2ceb78ef34c4b12d1ccb68480f81850fb48f9bd11ad3a1414a` |
| `reduction-scheduling/advisory-pilot-v1/build-and-pilot.json` | `a680d2e20489ef00d14070932455d139fcdde460fa7e7f11b7bad65ebef8210a` |
| `reduction-scheduling/final-pure-tests.log` | `59a6005d48bb84576b7eaa133d51c15cb1256af30726f6596490571544bd0bd8` |
| `reduction-scheduling/app-example-applicable-tests.log` | `b625e98f04bb43f84264b1305a63802242b548b19070e13d68d706f7f17648db` |
| `reduction-scheduling/advisory-pilot-v1/pure-tests.log` | `53120ba1ea64deadb6a029f58671b591c5c5ed72a1894aa34357745d6539543c` |

The complete canonical term-map digest is
`2edb607ae8ce171a6215856c2fa187c3ddd00eeef50ca4e258d8928f1400efa1`;
the complete guard-set digest is
`705ef0f055dbd9d74d65ac6838f6adb225df1dc0ae26dc6a8bf2ee5a562299c9`.
These hashes authenticate the retained comparison artifacts; the exact full
expression comparisons, rather than hashes alone, establish observed parity.
Raw scientific outputs, executables, private environment/configuration files
and external package/cache data are not included in this development branch.

## Interpretation

Root and DFS sibling ordering can affect reuse, residency and arithmetic order.
The implemented engine still fully reduces each selected root before collecting
it, so it cannot cancel opposing nonterminal contributions across roots before
expansion. Weighted frontier fusion remains a separate design outline only.
This one shared-component request supplies no independent-case, held-out,
repeated-performance or slow physical/q0 comparison. Canonical defaults remain
unchanged until separately reviewed evidence justifies a policy change.
