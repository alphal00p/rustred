# Finite-demand campaign delivery — 16 October 2026

This checkpoint delivers an opt-in exact finite-demand backend using the existing
saved IBP rules. It is ready for a fresh incremental campaign, not a claim of
completed five-loop R6/D10 closure or full physical Vakint acceptance. The user
requested a pause after this tested build, push and command handoff.

## Release and validation

Tested source: `138718efb68ff21262f2120ab4dcbab1adbe1289`, integrated into
RustRed main with identical core/application/Python/runtime-guard source.
Only documentation and the separately tested high-memory input policy are
additional delivery files. Unrelated FeynKit and untracked collaborator work
are preserved and excluded from this commit.

Frozen optimized CLI on this machine:

```text
/shared/codex-2/rustred/TMP/releases/20261016-finite-demand/rustred
SHA256 f23fd31a1e6674ee53601476b9494da60578c9bea5665dcdc462628b80e938ea
```

The release is a copy of the tested CLI, not a later rebuild. Its build receipt
and original frozen binaries are in
`TMP/public-finite-final-build-20261016/`. The final optimized build took
1563.937s; compilation is excluded from the campaign timings below.

| Gate | Result |
|---|---|
| Resident-index core | 556 tests passed |
| Final optimized application/CLI | 354 executions /351 distinct tests passed |
| Python steering | 117 tests passed |
| TTY adapter/guard | 27 focused tests passed, including a real PTY |
| Actual five-loop R0 public workflow | All nine interruption/repair/publication/inspection/relocation steps passed at32 workers |
| Full recorded R0+R1 finite traversal | All232 declarations complete; exact terminal/zero/rule census and cold-reopen parity |
| Four-loop H/FG/BMW/X controls | Exact paged/resident Work, census, cursor, generation and cold parity; all24 processes passed |

Test groups overlap; do not add these counts into a distinct grand total.
Audits were performed independently of the implementation. Earlier failed
builds/tests are retained in `CODEX_PROGRESS.md` and local evidence directories.

The five-loop R0 workflow exercised the exact delivered high-memory policy at
32 workers under a smaller32GB process guard. It completed116 required rows,
2,557 input occurrences and300,673 Work records, then published9,982 installed
rule records,1,956 encountered rules and312 unrefined terminal keys. The
deliberate preparation interruption, successful repair, no-op resume, rank-one
preview and relocated cold resume all passed. Total521.492s, sampled peak7.775GB;
repair plus solve/publication was293.421s. This was a functional test, not a
W4-versus-W32 timing comparison or a600GB/R6 capacity test.

The separate four-worker rank-one continuation took148.104s in window steps
after119.147s preparation. Including exact census comparison and an additional
cold process, its full driver took430.878s and peaked at10.752GB. All6,047,386
completed Work records and60,807 starting occurrences were retained;683 terminal
keys,1,110,963 zeros and5,023 selected-rule identities matched the prior exact
reference. The earlier paged/indexed controls did not complete their30-minute
pilots. See [the detailed timing boundaries](shared_finite_r1_pilots_2026-10-15.md).

The K10 controls took151.731s in total and peaked at2.000GB. They are the small
existing literal-parent controls (H1, FG17, BMW17, X1 starting points), not
unrestricted four-loop coverage or new numerical Vakint tests.

Independent receipts:

- `TMP/public-finite-final-native-tests-20261016/independent-audit.json`
- `TMP/public-finite-final-workflow-20261016/independent-audit.json`
- `TMP/finite-demand-resident-r1-validation-20261016/phase-b/independent-audit.json`
- `TMP/finite-demand-resident-k10-validation-20261016/independent-audit.json`
- `TMP/fresh-finite-r0-staged-inputs-independent-audit-20261016.json`

## Fresh launch on this machine

Inputs are already prepared at the following path; **nothing has been launched
there**. The folder contains only staged inputs. Its selection and R0 queries
are byte-identical to the accepted original R0 campaign. All116 required
templates and67 owners are retained, with the original183-row source attachment
for future rank intersections; auxiliary starting requests are omitted while
their saved owner rules remain usable. No old CP6 checkpoint is imported.

```bash
cd /shared/codex-2/rustred
export RUSTRED_CAMPAIGN=/shared/codex-2/rustred/campaigns/five-loop-finite-r0-d10-20261016

nix develop --command python -B examples/python/saved_campaign.py run \
  --campaign "$RUSTRED_CAMPAIGN" --engine finite-demand \
  --executable /shared/codex-2/rustred/TMP/releases/20261016-finite-demand/rustred \
  --finite-policy examples/input/finite_campaign/high_memory.json \
  --cpus 0-31 --max-memory-bytes 600000000000 \
  --host-memory-reserve-bytes 150000000000
```

Use the tracked Git flake through ordinary `nix develop`; do not use `path:`,
which can copy ignored campaign data into the store. No compilation is required
for this frozen release. The Symbolica license must already be in the environment;
it is not included in the release or repository changes.

CPUs0–31 are32 distinct physical cores and do not overlap the existing campaigns'
64–95 and96–127 affinities. Other users' resource use can change. The600GB value
is a requested process-tree ceiling, not a reservation; the host-memory reserve
can lower the effective allowance or trigger a clean stop. Swap-growth stopping
is disabled. No existing campaign is stopped automatically.

The policy has a300GB optional resident-Work ceiling, which does not allocate
300GB upfront. Fixed metadata/decoder and portable-reader limits remain; a
higher-rank run can reach a typed storage refusal before exhausting RAM. Existing
committed work is preserved. Neither this profile nor the rank-one measurements
prove that rank six will fit or finish.

## Continue, inspect and refine

Resume after a clean interruption:

```bash
nix develop --command python -B examples/python/saved_campaign.py run \
  --campaign "$RUSTRED_CAMPAIGN" --resume
```

After each completed stage, extend by one rank (change1 to2,3,…,6 as desired):

```bash
nix develop --command python -B examples/python/saved_campaign.py extend \
  --campaign "$RUSTRED_CAMPAIGN" --rank 1
```

Omitting the D argument preserves D≤10. Do not change it to `10+rank`.
Original coordinate and positive-power restrictions remain part of the request;
these are not universal rectangular R/D guarantees. Previously empty topologies
activate automatically through the retained original templates. Work is shared
across rank stages. An interrupted window is recomputed, not falsely committed.
If stopped during the initial preparation before a store exists, use the printed
`run --repair-preparation` recovery instruction instead of ordinary resume.

Inspect the latest publication and distinguish it from any pending new request:

```bash
nix develop --command python -B examples/python/saved_campaign.py inspect \
  --campaign "$RUSTRED_CAMPAIGN"
```

Manual master refinement, when wanted after a completed publication:

```bash
nix develop --command python -B examples/python/saved_campaign.py refine \
  --campaign "$RUSTRED_CAMPAIGN" --seed-depth 1 --containing-sector-depth 1
```

The option is `--containing-sector-depth`, not `--external-sector-depth`.
Refinement does not run automatically. If it is interrupted, either resume the
same refine command or retain its proved maps before extending:

```bash
nix develop --command python -B examples/python/saved_campaign.py finalize-refinement \
  --campaign "$RUSTRED_CAMPAIGN"
```

See [the generic finite-campaign guide](../finite_saved_campaign.md) for policy,
checkpoint, display and publication semantics. No five-loop artifact, cache,
license or reference-only material is committed. No additional Vakint work or
numerical master evaluation is claimed in this delivery; those parts of the
full assigned goal remain open after the user-requested pause.
