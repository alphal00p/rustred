# Native HEPKit sessions and the four-loop notebook

Status: native RustRed API pushed; updated shared-host acceptance passes 129/129
tests. The actual four-family notebook, visual checks, explicit normalization,
FORM-less numerical example and fresh-process readers all pass.
[Upstream PR #16](https://github.com/symbolica-dev/symbolica-community/pull/16)
is open from `c115b9d`. BenRuijl is tagged in the review-request comment;
formal assignment was denied by GitHub's upstream permission policy. This is
not a five-loop closure or performance claim.

## Ownership and architecture

The initial milestone is `f836862a`; `7f3924bc` corrects the FeynKit
momentum-head accessor against the pinned upstream API. The follow-up
`986c427f` publishes the transport budgets and explicit terminal normalization.
The community host now pins that revision and GammaLoop `f8ff93b0` from
`vakint_rustred`. The previous installed host passed128/129 checks and exposed
old Symbolica-v5 packaged assets when the H example reached the scalar reducer.
The new revision migrates all15 assets offline: eleven Num-coefficient payloads
use the existing strict converter, while four catalogues use existing native
exact-text import/encoding. Old and current readers verify all74 catalogue
values/keys/metadata exactly; current cold checks pass K1/K3/K6 replay and all
four normalization/catalogue pairs. Nothing was regenerated or approximated.
The rebuilt host passes129/129 tests, including the unchanged32-digit H
numerator reference with an invalid FORM path. The complete fresh notebook
and cold-reopen acceptance subsequently pass on that same host.
RustRed owns the actual generation job,
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

The October5 follow-up bounds nested RHS/guard previews and raw JSON too;
collapsing a panel is not treated as a lazy evaluation boundary. An actual
H artifact has21,318 rules and87,323 coefficient entries. Its largest saved
rule has499 RHS terms: the browser displays ten at a time. At a64KiB rule
detail budget it reports a controlled refusal; an explicit256KiB request
opens the structure without decoding coefficients. A selected coefficient
starts with an8KiB print allowance. Larger requests remain explicit. Native
polynomial printing now uses Symbolica's streaming formatter
directly, avoiding a complete expanded Atom solely for text display.

Generation and cold opening must use consistent caller-owned transport limits.
The transport correction retains those limits on generated results and exposes matching
optional limits on `CandidateArtifact.open`/`open_file`; the payload cannot
authorize its own resource allowance. This changes neither binary schema nor
default limits. The notebook explicitly supplies a larger aggregate-entry
allowance for source-heavy programs; this is not a topology-specific rule.

## Workload and interpretation

The actual notebook acceptance run must generate all four families in order
H, X, BMW, FG, with one native worker, exact sparse arithmetic, numerical
search depth two, and the complete physical positive-sector downset of each
parent. It must not substitute packaged artifacts or a small selected sector.

The generation portion demonstrates **candidate generation**, not a routed reduction
campaign, full-family closure verification, master independence, or numerical
master evaluation. A completed job or finite residual list proves none of
those additional properties. Generation, UI polling, artifact writing/opening
and selected-view timings must be reported with their distinct boundaries.

The notebook includes explicit native terminal normalization, comparison with
FMFT, and a separate FORM-less Vakint numerator evaluation using shipped rules
and master values. Fresh generated candidates
must not be described as byte-identical to the shipped processed inputs.
The four public reference TOMLs are byte-identical to RustRed's producer inputs
and Vakint's shipped family inputs. Native DOT tests check routing, denominator
order and auxiliary-slot parity against those references. This establishes the
input lineage, not equality of two generated programs: the fresh H binary
already has a different length and HEPKit's parameter names differ. Vakint
loads its separately packaged program, normalization map and numerical catalogue;
the notebook does not silently replace them with a new candidate file.
FMFT's basis has19 PR representatives: PR0–PR15 and the three dotted cases
PR4d, PR9d and PR11d. These differ from a count of numerical Laurent constants
or a nonminimal family-local terminal list. See [Czakon, Fig.1](https://arxiv.org/pdf/hep-ph/0411261)
and [FMFT, §2.4](https://arxiv.org/pdf/1707.01710).

## Validation record

### Final fresh browser run — October5

One explicit Generate action ran all four families, with no generation on
opening the notebook, changing a graph selection or refreshing an idle view.
The optimized installed host used one native worker on an AMD EPYC9754 machine,
with the test process group restricted to CPUs0–15; unrelated production ran
on disjoint cores. This is one observed run, not a repeated performance study.

| Family | Sectors | Rules | Raw residual keys | Native session, s | Native normalization outputs | File, decimal MB | Fresh-process open, s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H |314|21,318|386|91.753|22|58.03|0.263|
| X |328|19,907|445|281.629|19|149.41|0.511|
| BMW |134|9,018|179|110.898|17|50.07|0.191|
| FG |124|9,266|145|57.284|16|22.00|0.108|

All900 sectors finish with zero reported failures and no reused rule cache.
The summed native session time is541.563s, about9.03minutes, including native
preparation, solving and bundle assembly. This does not meet a five-minute
expectation; it is the actual measured workload. The browser workflow, including
exploration, normalization and numerical evaluation, takes569.053s. The guarded
whole test, including startup and all four fresh readers, takes578.597s with
3.246GB sampled peak process-tree RSS. Compilation is separate (625.190s).

Exact native normalization transforms the family-local1155 raw keys through
179 unit-alias representatives into74 weighted outputs (22/19/17/16), in
0.796s summed observed calls. A separate fresh-process comparison proves exact
equality of all four normalized integer-key sets with Vakint's shipped
22/19/17/16 catalogue sets, with no differences and no decoded recurrence
coefficients. This is key-set coverage, not equality of rule programs, numerical
catalogue values or arbitrary-index closure. The supplemental check takes4.155s,
peak478MB; evidence is`TMP/hepkit-normalized-key-comparison-20261005/`.
These are not74 independent masters. No recurrence
coefficient is decoded by normalization or structural browsing. Fresh readers
in separate Symbolica processes each verify metadata/pages, decode one selected
coefficient and confirm that its second display reuses the cached value.
"Fresh" refers to process/state initialization, not flushed OS filesystem caches;
the open timings exclude host import and are not source-replay certification.

The actual H numerator button reproduces all five original Laurent coefficients
at32-digit arithmetic, relative threshold1e-30, with an invalid FORM path.
Its native symbolic phase takes8.941s and numerical substitution0.00245s; the
whole observed action takes8.973s. It uses FeynKit tensors and Vakint's shipped
processed RustRed program/catalogue, not the newly generated candidate files.

Root independently reviewed the live dashboard, bounded structural/coefficient
views, normalization table and computed/reference numerical display. All native
and browser descendants drained cleanly. Evidence is retained locally under
`TMP/hepkit-gallery-e2e-20261004/actual-v5/`, including original inputs, native
timings, screenshots, artifact hashes, the four independent reader receipts and
the process guard. Generated binaries and screenshots are not added to either
repository.

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

The first actual browser run subsequently completed H (314 sectors,21,318
rules,386 residual records;91.991s native-session wall) and searched all X
sectors (328 sectors,19,907 rules,445 residual records). X then failed final
packaging at the default aggregate-entry allowance after282.548s. Its sector
checkpoints survive; BMW and FG did not start. This is not a completed
four-family benchmark. The save/reopen correction passes the optimized app
regressions, including assembly-only retry after all sectors were checkpointed.
Separately, a viewer-only recovery on H passed real page search/navigation,
the499-term rule budget/paging check, and the transition from zero decoded
coefficients to one only after explicit selection. No generation was performed
by that recovery. Final-host four-family, normalization and numerical acceptance
remain required.

The expanded notebook has24 lightweight lifecycle/display tests passing, and
all cells are at most29 lines. The final browser launcher has seven passing
stop/drain tests, including the distinction between completed generation and
still-running postprocessing. Independent source reviews of the native Vakint
compatibility port, checked graph ingress and browser lifecycle have passed.
Those source and mock checks do not replace the pending rebuilt-host run.

The subsequent native app gate passes12 tests: six session/view cases, the
unit-alias intermediate-count regression, aggregate-entry and byte-budget
checkpoint retries, two session-lifecycle cases, and the streaming print limit.
The Python and embedded-FeynKit typechecks pass after adding the two missing
crate-root type re-exports. All10 optimized Python runtime tests then pass
(seven coordinator, three streaming; zero failures/ignored tests). Their build
took785.337s with14.039GB peak RSS. The consolidated community-host acceptance
remains pending; no algebra assertions were weakened.

The first consolidated host built in1086.016s, peak13.372GB RSS. Actual
import exposed an old Vakint initializer that parsed the imaginary-unit
placeholder through Symbolica's numeric-literal lexer. The upstream direct
qualified-symbol construction is now ported in`fcb0de28`, independently
reviewed and typechecked. Before that correction, all118 non-Vakint installed
host tests passed; that partial result is not full-host acceptance. The failed
import and the separate70-literal Symbolica diagnostic are retained. Actual
corrected-host import, complex-value and32-digit H checks remain required.

Receipts are workspace-local under `TMP/hepkit-*`; generated artifacts, wheels,
licenses and browser evidence are not committed. The continuously updated
[progress ledger](../../CODEX_PROGRESS.md) preserves failed attempts and exact
test/build pins. This delivery is independent of the frozen
[second five-loop campaign](five_loop_banana485_launch_2026-10-04.md).
