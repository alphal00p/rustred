# Native HEPKit sessions and the four-loop notebook

Status: native RustRed API pushed and corrected embedded-host tests pass;
notebook acceptance is in progress. This is not a five-loop closure or
performance claim.

## Ownership and architecture

The implementation milestone is `f836862a`; `7f3924bc` corrects the FeynKit
momentum-head accessor against the pinned upstream API. The community host
pins the latter pushed revision. RustRed owns the actual generation job,
bounded event stream, cancellation state and binary-artifact views. HEPKit
supplies its existing native family; the notebook controls presentation and
the sequence of four examples. No standalone RustRed extension is imported
into the community host and no second Symbolica kernel is loaded.

The notebook starts from ordinary DOT inputs for H, X, BMW and FG, using
`FeynmanDiagram`, routing, `Kinematics` and `IntegralFamily`. Explicit edge
identifiers preserve physical denominator order. Auxiliary ISP coordinates
complete the family and remain nonpositive. The example normalizes the common
mass to one using Symbolica before constructing `IBPFamily`.

`IBPFamily.start_generation(...)` passes the prepared native family to
RustRed, not a printed or independently reconstructed TOML family. The
`parameter_bindings` property retains the correspondence to original native
Symbolica expressions. Example reference TOMLs are test oracles only; the
notebook does not load them to construct its inputs.

## Stream and lifecycle

- Expensive generation starts only from the explicit Generate button. Opening
  the notebook, changing a graph selection, or reevaluating a display cell does
  not start a solve. The button starts native execution immediately; generation
  is not a side effect of lazily rendering a result.
- A stable native coordinator runs the existing candidate-generation engine.
  Polling and waiting release the Python GIL; native workers do not invoke
  arbitrary Python callbacks.
- The bounded event queue records dropped intermediate events explicitly.
  Its retained aggregate snapshot supplies progress even for a slow consumer.
- Cancellation requests a native safe-point stop, not interruption of an
  arbitrary algebra operation. A request is not reported as a drained job.
- Abandoned consumers, queued cancellation, worker panic and post-fork access
  have explicit behavior and focused tests. A poisoned native coordinator is
  not silently reused. New processes should use `spawn`, not inherit sessions.

The public standalone package remains `import rustred`. HEPKit exposes the
same native types under `symbolica.community.hepkit.rustred`, with a small
Python convenience namespace that delegates the existing synchronous methods.

## Lazy exploration

`CandidateArtifact` supports binary open/open-file, cheap metadata, bounded
sector/rule/terminal pages, selected rule structure, and explicit coefficient
rendering. Structural browsing does not decode coefficient polynomials.
The first selected coefficient imports the native Symbolica state once;
selected values are decoded and cached on demand.

Rendered coefficient text is a bounded display preview, not an alternative
serialization or an input to a new CAS. Binary artifacts remain authoritative
and are intended for trusted local data. The notebook's table search is over
the fetched page, not an eager global scan of every expression in the artifact.

## Workload and interpretation

The actual notebook acceptance run must generate all four families in order
H, X, BMW, FG, with one native worker, exact sparse arithmetic, numerical
search depth two, and the complete physical positive-sector downset of each
parent. It must not substitute packaged artifacts or a small selected sector.

This notebook demonstrates **candidate generation**, not a routed reduction
campaign, full-family closure verification, master independence, or numerical
master evaluation. A completed job or finite residual list proves none of
those additional properties. Generation, UI polling, artifact writing/opening
and selected-view timings must be reported with their distinct boundaries.

## Validation record

Focused optimized RustRed gates passed before the community-host integration:

| Gate | Result |
| --- | --- |
| App session/view/lifecycle and bounded printer | 7 passed |
| Public core lazy decode and cancellation | 3 passed |
| Python coordinator and deterministic GIL heartbeat | 8 passed |
| Actual standalone extension regressions | 10 passed, no skips |

The initial optimized community wheel built successfully in 1,537.403 seconds
inclusive, with peak recorded RSS 17.829 GB. Its first installed-host test run
failed at a real API mismatch: the converter extracted a callable instead of
calling FeynKit's zero-argument momentum-head accessor. The narrow three-line
correction preserves all assertions; an independent reviewer checked it
against the exact upstream API. A temporary process-local proxy diagnostic
then passed all 109 tests, but is **not release acceptance**.

The corrected optimized host built in 1,043.870 seconds inclusive, with peak
RSS 12.358 GB. The actual installed-host suite then passed all 109 tests,
zero skips, in 1.74 seconds pytest time (3.238 seconds guarded). This run uses
the corrected compiled converter, not a proxy. The installed and packaged
native core hashes match; only one Symbolica DSO is loaded. The actual
fourteen-slot nonzero reduction and certificate test passes in this host.

The default-feature CLI typecheck also passes in 74.235 seconds inclusive.
Required remaining evidence: the real four-loop notebook and live visual
review must complete; saved outputs must reopen in a fresh process. The final
community PR and reviewer-request outcome will be linked here after those
gates, not predicted in advance.

Receipts are workspace-local under `TMP/hepkit-*`; generated artifacts, wheels,
licenses and browser evidence are not committed. The continuously updated
[progress ledger](../../CODEX_PROGRESS.md) preserves failed attempts and exact
test/build pins. This delivery is independent of the frozen
[second five-loop campaign](five_loop_banana485_launch_2026-10-04.md).
