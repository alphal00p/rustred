# A finite five-loop region: publication and portable application

15 October 2026. This validates the existing279-point R1/D10 pilot, not the
full116-template R6/D10 goal. No new IBPs or production campaign work was run.

## What now works

The closed finite trace can now produce a normal saved publication and portable
application package. It retains exact terminal identities and actual selected
owner/batch/rule identities, rather than treating one summarized checkpoint
record as one unit of physical work. The selected-rule census is optional in
normal native tracing. Publication requests it during one mandatory exact replay;
the fast/deep publication paths do not duplicate that replay.

The unchanged input is the whole region `hot-original-d10-r1-011101110111000`
from `TMP/route-image-pilot-hot-r1-finite-resource-20261015/on/`. Its exact279
starting points all terminate under the existing saved rules. Internal
descendants are not clipped to its starting rank/D bounds. This does not assert
closure for other starting regions or unrestricted five-loop integrals.

The production implementation is committed in `c9e5c340` (core census) and
`d74cfad2` (publication/application bridge). Independent source/runtime reviews
passed. There were528 passing core candidate-reduction tests and87 disjoint
application tests, including quota/cancellation/guard checks, exact finite versus
symbolic controls, source corruption, binary round trips and portable application.
The new real-data execution is separate evidence from those small tests.

## Real-data results

| Measurement | Result |
|---|---:|
| Starting points replayed |279|
| Completed operational states |641,239|
| Distinct physical keys |639,959|
| Native rule applications |267,694|
| Native transport endpoints |10,543,465|
| Pending/active/failed/missing-rule states at completion |0|
| Raw and remaining terminal keys |280 /280|
| Installed executable rule records |9,982|
| Distinct encountered rule identities |1,774|
| Composite baseline/selected-rule identities |1,782|
| Required binary census size |52,358bytes|
| Maximum internal scalar numerator degree / dot excess |5 /11|

The35.1335s exact census replay reproduces the previous successful trace's
non-timing counters. One worker performs this finite kernel; four workers were
reserved for the overall preparation/application phases. The internal degree5
is compatible with a starting R1 cap because the cap is not a descendant cutoff.

| Phase | Native time | Guarded wall time | Sampled peak RSS |
|---|---:|---:|---:|
| Publication, including preparation and normalization |200.427s|202.473s|7.701GB|
| Of which native owner preparation |156.478s|—|—|
| Of which finite replay kernel |35.134s|—|—|
| Export, source preparation |136.214s|—|—|
| Export, serialization |5.796s|148.399s for whole export|7.781GB|
| Corrected source/runtime comparison, including both preparations |—|248.511s|13.738GB|

Compilation is excluded. These are measured stages of a validation run, not a
generation-versus-SpIReD benchmark or a fresh paired speedup estimate. CPU affinity
was16--19, with32GB RSS allowance and150GB host reserve. No resource stop occurred
and every owned process group drained. The reused supervisor's generic
sector-generation labels do not describe these operations; native phase logs and
the actual commands establish that these were publication/export/application.

## Exact application controls and retained failure

The first comparison attempt correctly rejected an erroneous second fixture.
It put a positive power on inactive coordinate12 instead of increasing active
coordinate11; it also had an extra trailing padding coordinate and A=D=11,
not the intended10. That fixture and its failed receipt remain unchanged.
The engine, admission checks, source scope and assertions were not weakened.
A separate corrected JSON was used for a **comparison-only retry**, retaining
the already successful publication/export and both frozen executables.

Both corrected inputs have exact support011101110111000 and R0/A10/D10. Their
source-publication and cold-runtime outputs agree as exact family-qualified
coefficients and guard lists. Every output key belongs to the published terminal
set. Both warm repeats have one cache hit and zero rule applications.

| Canary | Source application | First runtime application | Warm repeat | Rules / output terms |
|---|---:|---:|---:|---:|
| First positive propagator dotted |26.80ms|42.162s|1.416ms|0 /1|
| Last positive propagator dotted |24.58ms|13.99ms|8.685ms|1 /9|

The first case is already a terminal and therefore a roundtrip/cache control.
Its first runtime call includes deferred owner-program preparation: the42.162s
is not an IBP-coefficient cost or a warm-application measurement. The second
case does apply a nontrivial rule. These two controls do not benchmark coefficient
back-substitution for all279 inputs, or prove a minimal terminal basis.
Comparison preparation is reported separately:137.869s for the source object,
59.326s for loading the runtime object. Source and runtime coexist during this
test, so its peak RSS is not a one-application memory estimate.

## External files and next steps

No artifact is committed. Local publication:

```text
/shared/codex-2/rustred/TMP/finite-census-real279-validation-20261015/publication/artifact.json
SHA256 b2c01a1097f8b8afd8dc493233dbe737c0145958ebaf04b580f93d9b180c176c
```

Portable package:

```text
/shared/codex-2/rustred/TMP/finite-census-real279-validation-20261015/application
application.json SHA256 1f8a865478ddc509ad0970926098f4470780f17a87473502f3c107def80d9aec
```

The directory's apparent size is200,324,099bytes, including the existing owner
programs and routes, not the traversal ledger. Required census data is validated
at the source load/export boundary; the exported runtime is self-contained and
does not re-read it during hot application.

Evidence is under `TMP/finite-census-real279-validation-20261015/` (including
the original fixture failure) and
`TMP/finite-census-real279-canaries-corrected-20261015/` (successful retry and
immutability checks). Frozen executables and compiler receipts are recorded in
`TMP/finite-census-real279-plan-20261015/` and
`TMP/releases/20261015-finite-census-tests/`.

## Resumable multi-query follow-up

The v3 implementation (`afac5ecd`) passed241 distinct native checks and146
related Python tests, then a real R0→resume→R1 experiment on the same input
family. A fresh nine-point scalar region closes; resuming it unchanged performs
zero new finite attempts; appending the279-point R1 region performs exactly one
new attempt while preserving the original sealed record. Cold replay verifies
both required queries, with identical live/cold non-timing trace counters and
all69 owner-payload digests unchanged.

| Phase | Guarded whole-process time | Native finite trace |
|---|---:|---:|
| Fresh R0 |156.369s|0.005s|
| Unchanged R0 resume |144.378s|No new attempt|
| Append R1 |166.378s|29.537s|
| Cold replay of both records |158.458s|28.679s for R1|

The complete four-process functional pilot took627.519s, including repeated
owner preparation. This is not a new matched speedup measurement. The R1 trace
deliberately includes the nine R0 points again; no cross-query point memo is
claimed. One finite kernel is serial, despite the four-worker preparation budget.

A separate543.127s publication/export/application pilot then passed, with an
independent audit. Publication took175.054s native/176.396s guarded; export
took142.413s guarded. Two native census invocations contribute289 terminal
occurrences but280 unique terminal keys and267,695 total rule applications.
The required binary census is52,527bytes. Installed/encountered rules remain
9,982/1,774. Both corrected canaries have identical exact source/runtime/warm
coefficients and conditions; the second executes one IBP to nine master terms,
then zero rules on its warm repeat (12.71ms first runtime/4.93ms warm after
owner preparation). The first terminal-only runtime call includes40.435s of
deferred preparation and must not be interpreted as IBP arithmetic cost.

The v3 local package is:

```text
/shared/codex-2/rustred/TMP/finite-initial-queries-publication-20261015/application
application.json SHA256 7f2738bcf575829177ad9d69936ef2b2ee3f12483d8bf8b1a3cdc7feffe75684
```

Evidence: `TMP/finite-initial-queries-real-r0-r1-20261015/` and
`TMP/finite-initial-queries-publication-20261015/`, each with an independent
audit receipt. The fresh v3 CLI derives the inventory; the frozen A2 application
helper is transparently reused because core and application/export code are
unchanged between the two revisions. No source checkpoint or production
campaign was modified; all owned processes drained.

The mode currently requires rolling FIFO/oldest-prefix execution, rejects
unsupported rescue-policy changes, and binds the exact original/amended query
images while preserving valid containment aliases. A clean stop saves merged
finite cases; the unfinished trace restarts at its case boundary. A hard crash
retains only the latest durable checkpoint, not every subsequent merge. There
is no mid-trace queue checkpoint or forced disk save after every case. Existing
default-off/v2 execution is unchanged.

Large regions still require the symbolic path or another sound decomposition.
Potential finite roots are currently gated until ordinary fallback commits,
even when too large for enumeration; mixed eligible/ineligible performance is
therefore still a deployment gate. These successes do not justify enumerating
the fullR6 input lattice or restarting production automatically.
