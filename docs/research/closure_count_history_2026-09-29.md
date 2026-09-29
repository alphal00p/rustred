# Historical 12/67 versus LC2 6/67: bounded read-only audit

2026-09-29, observations around 20:43–20:55 UTC. This is an audit of saved
metadata, small record/edge prefixes, source and existing test receipts. No
campaign was written, signalled, restored or cold-verified. In particular, the
76 GB LC2 generation was not loaded. Prefix reads do not independently validate
the full checkpoint digests and are not a new closure certificate.

## Result

The remembered 12/67 is real. It does not establish a same-workload engine
regression against LC2: the interim run had 47 different helper bounds and
1,299 frontiers. A stronger, genuinely same-input comparison exists: v2 and LC1
closed roots 14 and 15, while LC2 generation 8 has not. This is **different
closure progress**, not yet evidence of an incorrect closure counter.
These are separately bootstrapped walks, not a single checkpoint whose closed
count fell from eight to six on resume.

LC1 and LC2 have identical initial-domain bytes, input order and native records
for the four relevant initial anchors. Their launch-source walking subtrees are
also identical. The leading explanation is different descendant admission /
representative selection under Ready scheduling and global queue competition.
That explanation is plausible, not established for the live graph.

## Exact historical receipts

| Campaign / frozen receipt | Workers | Local native completions | Domains | Saved/observed closed roots |
|---|---:|---:|---:|---:|
| Interim `campaigns/five-loop-qcd-feynman-d9d10/runs/20260926T122852.894005Z/status.json` | 50 | 17,405,724 | 53,228,053 | 12/67 |
| v2 `campaigns/five-loop-qcd-feynman-d9d10-v2/checkpoints/main/latest.json`, generation 7 | 100 | 27,465,422 | 74,156,033 | 8/67 |
| LC1 `campaigns/five-loop-qcd-feynman-d9d10-lc1/checkpoints/main/latest.json`, generation 3 | 100 | 6,912,414 | 21,610,862 | 8/67 |
| LC2 `campaigns/five-loop-qcd-feynman-d9d10-lc2/checkpoints/main/latest.json`, generation 8 | 100 | 46,239,150 | 116,803,323 | 6/67 |

The interim status is a paused 80,241-second session with a stale conservative
snapshot (age 5,063 seconds), 1,299 frontiers and binary SHA256
`32fdec098a57dd0c51aef71c01c260fb5cf7d0954b0992db08bb0f955aed358a`.
The later small failed resume-attempt statuses are not the historical run.

For v2, LC1 and LC2 the saved `closure.revision == closure.snapshot_revision`.
Only 99 bytes per nodes file were read: the 32-byte RRW5 header and 67 initial
flag bytes. The stored closed sets are:

- v2 / LC1: `{0,2,12,14,15,23,24,26}`.
- LC2 generation 8: `{0,2,12,23,24,26}`.
- All 67 initial nodes are sealed in all three saved generations.

LC2's paused pre-upgrade status at
`runs/20260928T234230.071605Z/status.json` reports the same fresh 6/67 and
generation-8 counts. The later live heartbeat is stale, but telemetry staleness
does **not** explain the saved six. This difference existed before the Stage A
16:56 upgrade. The count is not the 116-required-query closure count.

An existing, stronger old graph audit is preserved at
`TMP/root-blockers-gen6/receipt.json` and documented in
`docs/research/fable51_root_blockers_2026-09-27.md`. Its full production restore
and independent forward reachability reproduced v2's eight closed roots. Its
comparison interim set was `{0,2,12,14,15,23,24,25,26,35,42,43}`. The four
interim-only roots 25/35/42/43 had real pending queue blockers in v2; that result
must not be silently extended to LC2 roots 14/15, whose present cones have not
been walked.

## Input, owner and policy comparability

All four campaigns contain 183 query rows: 67 owner anchors and 116 physics
rows. Historical rows have no explicit `role` field. No historical 12/67 or
6/67 number counts all 116 physics queries separately: initial admission
maps those rows to the 67 protected anchor domains.

Interim versus v2/LC1/LC2:

- All 116 physics row objects, full row-owner order, anchor-owner order and
  owner selection order match.
- Exactly 47 anchor objects change only `id` and `power_bounds`: positive-power
  A is narrowed. Unbounded-A anchors fall from 60 to 13. Rank/lower/upper are
  unchanged.
- Interim query SHA256 is
  `0f7f0a043de8a896725d3bd1fcb42a7948d2aeadc68cbd78bf2fa549d80d97b8`.
- v2, LC1 and LC2 query bytes all hash to
  `2c7148601436ebd1e50f7c13d922859cdb5d296f5584f33d266f0f866d27204f`.
- Their actual 3,704-byte initial-domain segments are byte-identical, SHA256
  `03d3a5c084653af34eda95907953214023b842f87dd44201eb2693811bd618d4`.

The interim, v2 and LC1 input receipts have all 67 identical owner-program
SHA256 values. LC2's 67 values differ, but every old/new pair and byte length
matches `TMP/inputs-v6/five-loop/inputs/conversion-manifest.json` exactly.
`examples/python/convert_native_v5_to_v6.py::rewrite` changes only the validated
Symbolica state-version and coefficient-frame format bytes; it does not
regenerate or decode/re-encode algebra. This audit checked the manifest links
and converter source, not another full 1.28 GB owner-file rehash. The prior
conversion/identity evidence is `TMP/lc2/logs/convert.log` and
`docs/research/fable51_lc2_patchfree_2026-09-29.md`.

All use Ready, lookahead 256, route overcover, initial-D-band reuse and G2 Off.
LC1 and LC2 both use W100 / CPUs128–227 / 600 GB requested cap / frontier stop;
the interim used W50 and a 700 GB cap. Ready completion/representative order is
schedule-dependent even for the same source and exact inputs.

LC1 launch source is `43269bc2ea986ddec28bceae565fb226169b0983`, LC2 launch
source `5e9f200dd543064cde52d3839ab72379070945d9`. The diff of their entire
`crates/rustred-app/src/application/routed_campaign/walking` subtree is empty.
The changed core/Symbolica ownership/format implementation can change execution
timing; unchanged walker source is not proof of identical schedules. LC1 binary
SHA begins `4606cc4b`, LC2 launch SHA begins `fd9b9ac9`; these are executable
hashes, not Git revisions.

## Two focused roots and direct saved-record evidence

Every listed row has 15 zero lower coordinates and 15 null upper coordinates.

| Protected ID | Owner | Rank bound | A bound | Old v2 cone nodes / edges | Other initial roots reached |
|---|---|---:|---:|---:|---|
| 14 | `111100000011100` | 11 | unbounded | 8,383 / 196,438 | 23,24 |
| 15 | `111010100100101` | 2 | 12 | 15,036 / 215,390 | 14,23,24 |
| 23 | `000010011001001` | 11 | unbounded | — | — |
| 24 | `001100101110000` | 11 | unbounded | — | — |

D bounds are null for all four. Cone numbers are the archived v2 generation-6
`root_blockers.roots` results, not estimates of the present LC2 cones. Root14's
cone has 8,326 Route nodes and depth11; root15's has 14,975 Route nodes and
depth10. A new subset may produce different representatives and different work.

Reading only 253,816 / 237,656 / 279,415 bytes of the first v2 / LC1 / LC2
records segment located all four initial native records. Removing only
`seconds`, each complete JSON record is identical across the three campaigns:
geometry, selected-rule refusal provenance, all native statistics, success,
frontiers and accepted events. Accepted counts for 14/15/23/24 are
24,160 / 18,489 / 18,369 / 14,615. Thus no changed *initial* algebra stream is
visible in these records (a record is not the full callback payload).

A bounded 8 MiB LC2 edge-prefix read found 3,571 edges sourced at14. Of their
targets, 334 are sealed delegated nodes lacking CLOSED; the only inspected
unclosed target is the self-edge14. The prefix is not asserted to contain the
complete outgoing inventory. One actual chain is
`14 → 168168 → 1391654`: the first edge is in the prefix; a bounded record-prefix
search found the `delegated_not_inspected` record for168168 after 94,076,989
bytes. It delegates the Route owner `101100000011100`, rank≤10, to1391654;
generation8 marks1391654 inspected/sealed but not closed. This is concrete
representative dependence, not yet a path to an unsealed leaf. No full edge
scan or claim that a sealed cycle alone blocks closure follows.

## Tracker / alias / cycle audit

No counter bug found in this pass. `Tracker::scan` seeds every unsealed node,
traverses incoming edges, and marks the complement closed. Sealed cycles with
no outgoing unresolved obligation close correctly. Edge insertion after seal
is rejected; native completion adds partial-anchor dependencies before sealing;
delegation adds the representative edge before sealing the alias. Stale
snapshots therefore remain lower bounds. Forced pre-save refresh bypasses the
periodic throttle and stop cancellation.

Existing exact tests cover independent forward reachability, sealed cycles,
alias/partial-anchor blocking, CSR folding/restoration and rejection of missing
alias/anchor edges. They passed in both `TMP/lc1-ship/lib_suite.log` and the
later `TMP/codex-parallel-validation.RPJKV5/native-app-f3f707af-opt1/stdout`.
The latter full suite had one unrelated protected-prefix verifier failure,
subsequently fixed and publicly regressed; it is not described as full green.
No new test or large checkpoint validation was executed for this audit.

## Smallest registered next test

Prepared, not executed: `TMP/codex-stage-a.2RU3AX/closure-history-controls/`.
`queries.json` consists of the exact unchanged anchor row objects14/15/23/24,
in that original order, SHA256
`63e4ced47188293624a060c0da0898e711ccfad750d725be4cc184810b11e699`.
All67 existing LC2 owner programs remain available. No program generation,
coordinate change or checkpoint modification is involved. To satisfy the
existing CP6 adapter's explicit-role contract, both arms use
`queries-required.json` (SHA256
`c3959a3e2e9380fa4f01617ee95d35c8b2b7754ae9ee08a3beecb7739e5bae1c`):
identical row objects plus a top-level `query_roles` declaration listing the4
IDs Required and no auxiliaries. This declares the native default explicitly;
it does not add per-row fields or change any historical campaign.
The prepared `root-map.json` includes the common historical native records and
maps historical IDs14/15/23/24 to fresh pilot IDs0/1/2/3. Assertions must bind
the exact query IDs, not confuse the old numeric IDs with the new prefix.

After the new optimized binary is frozen and resources explicitly granted:
same-binary Ready versus Epoch rolling Snapshot FIFO, **G2 Off in both**,
W50/CPUs32–81, existing guarded runner and cold-All verification of all four
roots and all selected native records. Use the existing 1,800-second whole-arm
budget, 900 native +300 drain, each verification150+60 and180 preparation /
admission/reporting reserve. No empty PhysicsQueries acceptance. This is a
four-anchor diagnostic subset, not the frozen183-row production scope, not a
Union performance qualification, and not a 116-query closure claim.

Falsifier: native CLOSED disagreeing with independent cold graph coverage is a
concrete correctness finding. If both subset runs close and cold-All agrees,
that supports context-dependent scheduling / representative choice and refutes
a general inability of the new code to close these obligations; it does not
prove that only starvation explains LC2. An open/censored subset instead
provides a small genuine checkpoint whose dependency blockers can be inspected
without the live 76 GB generation. Censoring is not a correctness failure or a
completed speed ratio. Do not force the four-anchor result into the full4L or
5L acceptance claims.
