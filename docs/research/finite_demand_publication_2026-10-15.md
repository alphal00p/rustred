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

The next implementation slice generalizes finite replay beyond a single initial
root and supports completed-case checkpoint resume and append-only rank extension.
In-flight finite traces will initially restart at their atomic case boundary;
there is no mid-trace queue checkpoint yet. Large regions still require the
symbolic path or another sound decomposition: this result does not justify
enumerating the fullR6 input lattice, nor recommend restarting production now.
