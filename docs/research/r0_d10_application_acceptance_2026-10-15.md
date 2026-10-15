# R0/D10 saved-application acceptance pilot

15 October 2026. **A small admitted subset passes**, not the full five-loop
acceptance goal. The pilot exported an existing completed publication and
performed exact symbolic applications only. No new IBPs, numerical master
evaluation, persistent-cache preparation or production mutation occurred.

## Immutable input and external package

The source is the completed depth-zero publication, not the separately active
depth-one refinement:

```text
/shared/codex-2/rustred/campaigns/five-loop-minimal-r0-d10-incremental/master-reduction/scopes/02a4d7387ca1bd822adbf66c7d05af39c19c484d76fc549f236eeeeac2bc6eea/artifact.json
```

Source SHA256:
`f01b28b4bf0b39047cea5824934084f6644b498b5785b48538bde7b746d8c154`.
Source scope binding:
`db1374561ebb0773e0ac9b071ac0e9a8d57a9cb0475e2618550fe5fe24b1cc0e`.

This generation1 publication records116 required queries,81 nonempty and35
empty domains,67 owners,9,982 installed rules and329 terminal maps. Its scope
is R0 within the retained D9/D10 physical templates, not every integral below
these numerical caps. Export checked all329 retained substitutions through the
existing native path; inherited assurance remains `trusted_saved_scope`.

The portable runtime directory is external data and must not be committed:

```text
/shared/codex-2/rustred/TMP/r0-vakint-acceptance-20261015/application-r0-d10
```

It contains77 files totaling **200,361,076 bytes**. `application.json` SHA256:
`e458b4c2d4e9c316f64732f8ddad08dbff98c58c80c00b315baf58df7b0a6a2f`.
All77 file sizes and digests are recorded in
`TMP/r0-vakint-acceptance-20261015/package-files.json` and were checked unchanged
after application. The package is separate from THE_ONE generation54 at
`TMP/five-loop-vakint-20261013/application-gen54-first`, which was untouched.

## Code, resources and timing boundary

Unchanged public examples were linked against the audited optimized native
libraries: RustRed `78cbc5c1`, Symbolica `400a53c4`, FeynKit `024f96d6`, and
Vakint feature-branch source `3cd9b1d2`. This is the established application
stack, not the concurrently developed routing/closure-monitor build.

The runtime used CPUs20--23, single-thread nested pools,32GB aggregate RSS
allowance and150GB host reserve. The inclusive1740s cooperative deadline was
not reached. All four child groups exited0 and drained. Total runtime was
**319.282s**, including preparation, export, validation and cleanup but excluding
compilation. Exporter and Vakint example links took328.618s and501.120s
respectively. These are single-run observations on a shared host, not comparative
speedup measurements.

| Phase | Native time | Supervised wall | Sampled peak RSS |
|---|---:|---:|---:|
| Prepare/export completed publication |163.254s|164.554s|7.741GB|
| Read boundary-witness inventory |0.138s|1.191s|0.022GB|
| Vakint D9 control and invariance checks |see below|77.355s|2.736GB|
| RustRed D10 boundary control |see below|75.338s|2.732GB|

## Exact tests that passed

### Vakint pipeline: R0/D9 factorized input

The common-family key is
`[0,0,0,0,1,0,0,1,1,0,0,1,0,0,5]`: five independent tadpoles,
positive-power sum9 and numerator1. The existing native example was run with
`--admitted-tadpoles`, explicit symbolic master mode and an invalid FORM path.

- Ordinary Vakint initialization:0.093778s, with no five-loop catalogue.
- Explicit package load and preparation:73.811549s. Five-loop catalogue
  construction accounts for0.153615s and occurs only after that load.
- First application after load: **0.249233s**; exact repeat: **0.182709s**.
- Exact equality passes for an alternative unimodular loop basis, simultaneous
  edge/node relabeling and orientation changes, indexed notation, and ordinary
  versus detailed evaluation APIs.
- Independent multiplication of five guarded one-loop reductions agrees.
  Nonunit common mass squared4 and external tensor/scalar spectators agree.
- Guard-domain comparisons, artifact-path disabling, changed-option refusal
  and clearing of application/factor caches pass.
- Logged cumulative cache statistics: four rule applications, five cache hits,
  six retained entries and1,348 coefficient bytes; one owner prepared lazily.
- Output is247bytes: an exact coefficient times the one-loop master to power5.

The generic example emits an `expanded_numerator` parity marker, but this case
has numerator1. That marker is **not** a nontrivial numerator test. This run
does not exercise DOT ingress or a genuinely coupled five-loop integral.

### RustRed direct application: R0/D10 boundary

The separate native control uses
`[0,0,0,0,1,0,0,1,1,0,0,1,0,0,6]`, with A10/R0/D10, and is checked against
the actual saved admission union before application.

- Fresh-process package preparation:72.973771s. Route preparation dominates
  at66.328570s; owner-payload handling costs3.090816s.
- First exact application: **0.020654s**, including first-owner preparation
  of0.014893s; five rules, seven operational nodes.
- Warm repeat: **0.000015510s**, one cache hit, zero rule applications.
- Exact coefficient and guard equality; one output terminal and zero conditions.
  Seven cache entries retain1,668 coefficient bytes.

This output uses the raw saved terminal convention. Vakint additionally
normalizes established products into lower-loop factors. The two timings
therefore represent different inputs and application boundaries; they are not
a Vakint-versus-RustRed speed comparison. Package loading is reported separately
and should be amortized in realistic repeated-application workloads.

## Evidence and independent audit

Local evidence under `TMP/r0-vakint-acceptance-20261015/`:

- `pilot-result.json`: all four process outcomes, resources and deadline state;
- `export/`, `inventory/`, `vakint-tadpoles/`, `rustred-d10-tadpoles/`: native
  stdout/stderr, supervision and process-tree resource observations;
- `saved_application-binding.json`, `five_loop_saved_application-binding.json`
  and `additional-source-binding.json`: sources, libraries, executable hashes,
  included topology data and independent one-loop artifact;
- `package-files.json`: complete77-file runtime-package digest inventory;
- `independent-audit.json`: independent PASS, with source/binary bindings and
  all77 package hashes unchanged, and explicit workload/coverage qualifications.

The generated inventory contains135 distinct boundary witnesses. The pilot did
not reduce all135 witnesses or all2,557 concrete starting inputs in the R0
profile. No source artifact, native library or Vakint checkout was modified.

The frozen binaries can be rerun with the existing local toolchain wrapper;
the complete commands and supervision script are recorded in the local README.
Do not rerun the export pipeline into its already existing destination.

## Follow-up: connected topology with actual cache eviction

A separately authorized unchanged native test,
`external_package_bounded_lru_cache_preserves_admitted_repeats`, then ran against
this R0 package. Its second input is
`[0,0,0,0,1,1,0,0,1,0,0,1,0,1,4]`: a connected six-line five-loop banana,
owner `000011001001011`, TIDE representative28686, A9/R0/D9. Its first input
is the five-tadpole control above. Both keys were checked against the published
coordinate/A/R/D predicates before execution and admitted again by the native
test. The historical generation54 annotation is not a scope override; no
generation-specific count is asserted by this test.

The frozen current-stack unit binary SHA256 is
`d3e54edf8bd1a8f513bf1adbfda688a526c48b00f1cfc0b584ebe66e8c4c44f3`,
identical to the original Cargo test output. Exactly **one test passed**, zero
failed or ignored;143 unrelated tests were filtered out. Native test90.59s,
supervised wall91.393s, sampled aggregate RSS2.911GB. CPUs20--23,32GB allowance
and150GB host reserve were retained; the840s deadline was not reached and all
owned processes drained.

The **4MiB** cache limit caused4,842 evictions (4,059,928 coefficient bytes)
and815 recomputed nodes. Both inputs' exact repeated expressions, guards and
qualified-family maps nevertheless agree. A tightened scalar-numerator policy
correctly refuses before warm reuse; clearing removes every retained application
entry. FORM is invalid and numerical substitution disabled throughout.

Debug timings separate native package load74.361958s from the connected input's
saved-rule application12.313639s on first use and0.000742s on repeat. These are
**scalar-phase** times, not complete frontend timings: warm canonicalization,
witness search, scalar lowering and terminal handling add approximately0.772s,
including0.754302s for witness search alone. No no-eviction baseline comparison
was performed in this invocation.

Evidence: `TMP/r0-vakint-acceptance-20261015/connected-result.json`,
`run_connected.py` and `vakint-connected-lru/`. All77 package hashes remained
unchanged. The independent auditor reviewed this follow-up and confirmed PASS
with the stated boundaries. It adds one real connected admitted reduction,
not connected-class exhaustiveness or DOT coverage.

## Remaining acceptance obligations

This is a useful R0 application milestone, not completion of the goal. Still
required are DOT reductions, broader connected and per-domain
coverage, nontrivial numerator and cancellation invariance, cross-factor
contractions, the fully prepared physical ladder, appropriate high-rank/boundary
cases, full R6 publication and its acceptance/performance delivery. Five-loop
cache preparation remains subsequent to the relevant validated publication.

The generation54 R11/D9 tests have a different admission scope; this R0/D10
success neither replaces nor discharges them. Keep the baseline and outstanding
test dispositions separate. No numerical master values or minimality proof
are claimed.
