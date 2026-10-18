# Validation and delivery record

This directory is a prepared input/generation study. The actual 14-parent
generation campaign has **not** been started by the assistant.

## Topology checks

Native FeynKit enumeration retained all connected cubic six-loop multigraphs,
without tadpole, self-energy, zero-flow, color or factorization filters:
388 graphs. Classification found 322 factorized and 66 nonfactorized cases.
Equal-momentum propagators were combined with explicit power maps; all 66
nonfactorized quotients embed into 14 inequivalent fifteen-line parents with
exact signed integer loop maps of determinant ±1.

The 458,752 parent-subset census contains 435 full-loop matroid classes:
351 nonfactorized and 84 factorized. The latter remain generation jobs because
they occur as descendants. Every class is assigned once in the manifest.
The 435 representative forest contractions were independently checked. Full
runtime GL maps for every duplicate labelled subset are not installed or claimed.

Independent checking enumerates simple cycles from graph adjacency, unlike the
producer's binary cycle-space method. Both use native Symbolica graph
canonicalization and matrix algebra. Four corrupted-witness controls fail.
The same pipeline reproduces four-loop 16 = 10 + 6 and five-loop 67 = 48 + 19 classes;
all five-loop propagator-count subtotals match the TIDE reference census.

Recorded offline timings on this host: cubic enumeration 0.41 s, classification
0.37 s, exact parent-cover search 4.60 s, full contraction census 85.93 s. These are
graph/input preparation timings, not IBP generation or closure timings.

## Native generation plumbing

The initial generation delivery's optimized executable had SHA256:

`baafaba31980b6a9e2e9fff3f799f4ed26369d3e6f7b135539325df73bb575fc`

The dashboard-only refresh below supersedes it at `bin/rustred`; existing runs
retain their own frozen executable.

It compiles generation arities 1 and 21, independently of the existing five-loop
campaign binaries. The generic candidate codec, explicit input selection and
completed-sector checkpoint format now support those 21 coordinates. Ordinary
campaign application/closure dispatch has not been promoted to six loops.

The exact 21-coordinate smoke input is `tests/six_tadpoles.toml`: a factorized
control, not a coupled study parent. At R0 it produced 6,551 parametric rules and
one finite terminal in 121.715 s process wall, including preparation and encoding.
Cold checkpoint resume reused the completed sector, generated no new rules,
and wrote byte-identical native output in 38.961 s. After removing the last
frontend arity ceiling, the final binary repeated cold inspection and checkpoint
resume in 39.003 s with the same output hash. The complete bundle is 7,483,333
bytes; cold native inspection checked its family, 21-coordinate shape and counts.
The diagnostic selects one rule rather than rendering thousands of rules at once.
No timing extrapolation to the coupled six-loop families is justified.

The final launcher also passed actual two-parent K1 generation, retained-output
resume with no native jobs, and checkpoint resume after withholding completed
receipts: each native job reported reused_sectors=1/newly_solved_sectors=0 and
identical bundles. The native dashboard accepted and rendered the resulting
structured snapshots. Actual six-loop metadata-only preparation verifies all
14 inputs and 435 selected sectors under 32 workers / 4 parent slots / 600 GB.

## Resource policy

The study uses a single aggregate process RSS guard. Fixed 16-coordinate frontend
and 1 GiB candidate-bundle ceilings were removed from this generation path;
no replacement frontend coordinate ceiling was introduced.
Counts, case work, export polynomial terms/operations and checkpoint bytes are
caller-controlled; the launcher requests no artificial count/runtime ceiling.
Payload permissions come from the configured RAM budget. Chunk sizes are not
limits on the amount of campaign history.

Exact source/case conditions, native exponent representations, checked overflow,
decoder allocation accounting and file integrity remain enforced. Unsupported
mathematical geometry and real memory/IO/OS failures are still possible. The
optional semi-numerical backend has separate reconstruction policies and is not
enabled in this delivered sparse-factorized build.

## Regressions and evidence

44 pure steering/scheduler/dashboard/pipeline tests pass. Native candidate
regressions pass: **191 passed, zero failed, three explicit ignored fixtures**.
The existing finite-campaign regression filter also passes: 271 passed, zero
failed, five ignored, on 32 permitted CPUs. Its separately affinity-gated
150-worker fixtures are not claimed as newly executed by this 32-CPU run.
The test harness uses the current source with unoptimized application code to
avoid another full optimized link; optimized real CLI generation, inspection,
and resume are tested separately as described above. An unrelated large epoch
fixture overflowed the default test-thread stack in this harness; rerunning with
`RUST_MIN_STACK=67108864` passes without any code or assertion change.
An initial regression run found two assertions enforcing the deliberately removed
old ceilings and a test assuming the optional reconstruction feature was enabled;
those tests were updated to validate the requested wider capability and feature
selection, without changing physics assertions.

Local detailed evidence, not committed output artifacts:

- `../TMP/six-loop-uncapped-generation-build-20261018/`
- `../TMP/six-loop-generation-test-results-20261018/`
- `../TMP/six-loop-fast-native-tests-build-20261018/`
- `../TMP/six-loop-21-native-smoke-20261018/`
- `../TMP/six-loop-launcher-native-smoke-20261018/`
- `../TMP/six-loop-generation-final-arity-dry-20261018/`

Neither five-loop production campaign was stopped, restarted or modified.
Unrelated FeynKit edits were preserved and excluded from this delivery.

## October 19 — native dashboard refresh

The initial viewer silently fell back to plain text when its inline viewport
could not query the cursor position from a detached process with null stdin.
A write-only stderr PTY reproduces that failure with the initial executable.

The replacement uses a full-screen Ratatui viewport, stderr-descriptor size
queries, resize-aware drawing and alternate-screen restoration. It never reads
stdin or requests a cursor reply. The versioned snapshot producer, solver,
scheduling, resource policy and checkpoint semantics are unchanged.

Current optimized `bin/rustred` SHA256:
`b8b93b976fa1e9e301fb2bf1bfb78ee5a97256558fcafd943fc2d3135be59260`.

Five real native PTY tests cover detached/write-only stderr, colours, NO_COLOR,
resize, signal cleanup without snapshot mutation, final scrollback, plain pipes
and JSON preservation. A synthetic 14-parent snapshot was visually inspected as
a rendered terminal image; it is explicitly not a campaign measurement. The
launcher also passes a two-parent K1 generation smoke through the new dashboard
(2 rules / 2 terminals). All 12 native preparation model/render/viewer tests and
all 44 Python steering/scheduler/dashboard tests pass.
Evidence: `../TMP/six-loop-dashboard-build-20261019/`; native layout regression
results: `../TMP/six-loop-dashboard-tests-build-20261019/`.

The user's interrupted six-loop run is left intact. No six-loop restart was
performed by the assistant. Use a new run directory to get the new frozen build.
