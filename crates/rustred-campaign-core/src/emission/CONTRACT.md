# Interface: the EMISSION CONTRACT (every rule the automation writes carries its derivation check)

Lane `gencheck` (implementer session), branch `ac/gencheck` (base int/8b 595ea105 + ac/wcertproof 42d71824).
Status: v2, 2026-11-18 (v1 reviewed by the implementer 09:5xZ: decisions 1,2,3,5,6,7 accepted; 4 and the body hash changed
as below; code on ac/gencheck). USER RULING 2026-11-18: no further certification campaigns; exactness by
construction, the class bound to the right record AT THE MOMENT OF EMISSION, never by a later pass.
Requirements read: TRICKS 67/68/69 (+ addendum), notes/final-cert-indep.md LIMITS, interfaces/weight-cert-check.md,
reports/WEIGHT-CERT-CHECK.md, notes/engine-line.WEIGHT-CERT.md (A1-A7, E1-E3), notes/sizegen-takeover.md.

## 0. Principle and words
- A RULE RECORD is any `{"type":"rule",...}` line of a table (parametric, point, symmetry / one-term, slot alias). A
  table may contain rule records ONLY in the form written by the EMITTER (section 3). No other code path writes one.
- The CLASS of a record is one of four fixed words, ordered: `whole domain` > `every first-match key in cone` >
  `sampled` > `none`. Each class names its evidence (section 4); `none` always carries a typed reason.
- The class is a FUNCTION of (the record's own bytes, the check outputs bound to those bytes, and, for the two
  position-dependent classes, the table prefix before the record). It is never an input: no class table, hash lookup
  or producer claim can set it (TRICKS 67: a class bound by relation hash from another position travelled to a live
  twin through a dead record's vacuous verdict).
- Nothing in the code is specific to a node, family, arity or cone (L8): the cone, the box, the policy, the budgets
  and the ladder depth are inputs (spec / config), with defaults stated below.

## 1. What a record carries: the `derivation` member
Every rule record gets ONE extra top-level member, `"derivation"`, written LAST in the line (so the body is recovered
by removing a suffix). Importers ignore it [read: rustred-core propagator/table_import.rs reads records as JSON values
and takes named fields only]. Schema `rustred.campaign.emission.v1`:

| field | content |
|---|---|
| `schema` | `rustred.campaign.emission.v1` |
| `body_sha256` | sha256 of `chart \n case \n relation_sha256 \n domain_sha256` (REVIEW CHANGE): relation = sha256 of `[target, rhs]`, domain = sha256 of `[fixed, equations, exceptions, domain_bounds]`, both in Python-compact JSON with members in file order (= the checker's `target_rhs_sha256` / `domain_sha256`). Stable under index renumbering, whitespace, member order and every provenance member; changed by any coefficient TEXT, target, domain member, chart or case. The coefficient text is the engine's canonical Symbolica printing (the checker confirms it byte for byte); a payload that re-encodes coefficients must decode to the identical text or the record is re-emitted. |
| `kind` | `lifted` / `point` / `symmetry` / `alias` / `recipe` / `legacy` (section 6) |
| `producer` | `tool`, `binary_sha256`, `verb`, `args` (exact argv), `env` (only the `RUSTRED_*` switches that change the output, e.g. `RUSTRED_WEIGHT_CERT_ROWS`), `family_sha256`, `table_header_sha256` (the header the lift ran under: options, order), `seeds` (setting + sha256 of the seed list), `sources` (what was re-lifted: table sha256 + position + record sha256), `job` (id, attempt), `lift_seconds` |
| `certificate` | `path` (RELATIVE to the table's directory: `<stem>.certs/<body_sha256>.<form>.json`), `sha256`, `bytes`, `schema`, `form` (`v0` / `v0+rows` / `v1-ordinary` / `v1-composed`); null for kinds without one. Never in the shipped payload: beside the table |
| `check` | the CHECK RESULT of the independent checker run AT EMISSION: `tool`, `binary_sha256`, `mode`, `args`, `output_sha256` + `output_path` (one JSON line, `<stem>.checks/<body_sha256>.json`), `input_sha256` (= `body_sha256`: what the checker read), `stages` {`rows`, `identity`, `binding`, `factors`, `loci` (counts per verdict), `open_loci` (factor texts)} copied verbatim from the checker's record, `checker_class`, `seconds`, `peak_mb`; or `{"ran": false, "reason": <typed>}` |
| `class` | `word` (one of the four), `evidence` (which fields above it rests on), `cone` (predicate text, for the cone class), `keys` (count + sha256 of the sorted key list, for cone / sampled), `position` + `prefix_sha256` (for the two position-dependent classes), `reason` (typed, for `none`) |
| `ladder` | for pieces of a replacement: `replaces` {`body_sha256`, `record_sha256`, `index`}, `piece`, `of`, `depth`, `branch_added` (the locus that opened this piece), `partition` {`verdict`, `output_sha256`}; else null |
| `policy` | `name` + sha256 of the policy text that admitted the record |

No clock time inside the record (bytes are reproducible; times go to the emission log).

## 2. Who checks: the checker as a CHILD through the launcher (decision, justified)
The checker is `rr_proof --mode weight-cert` (and `--mode instance-span` for point / symmetry kinds), run as a child
job with its own unit, cap and namespace, not linked into the campaign crates. Reasons: (i) independence: the record
names the checker's binary sha256, built separately from the producer; (ii) the v1 / row-recovery checks need the
`engine` feature (RustRed's ordinary row generator), which the light campaign crates must not link; (iii) Symbolica's
licence counts PROCESSES (2 concurrent on this host): the scheduler admits checker children as a counted resource
(`emission.max_symbolica`, default 2), which an in-process call could not; (iv) a check that runs out of memory or
budget kills a child, not the supervisor. Tests that spawn it run under `LIGHT_PIDNS=1` (L10).

## 3. The emission step: binding by construction
`emit(candidate, ctx) -> Emitted | Refused`, the ONLY writer of rule lines (pure decisions in
`rustred-campaign-core::emission`, the child runs and file writes in `rustred-campaign::emission`):
1. BODY. Take the candidate line, drop any `derivation` it carries (a candidate never brings a class), set `index`
   to the target position (the positioned writer's `Positions::peek`), compute `body_sha256` (section 1).
2. CHECK ON THE BYTES. Write `[table header, body]` to the run's check directory and run the checker on exactly these
   bytes (`--offset` of the body), with the certificate the producer wrote for THIS lift (selected by the engine's
   `rule_digest` = the final rule, E1; a certificate of another lift of the same case binds `BINDING_TERMS_DIFFER`).
   The domain is read by the checker from the body itself: `fixed`, `equations`, `domain_bounds`, chart orthant,
   minus every exception branch. Accept the output only if its `rule.record_sha256` equals sha256 of the bytes written
   and its `cert.sha256` equals the certificate file that will be stored: otherwise the check is `ran: false`,
   reason `binding_mismatch`.
3. CLASS = `class_of(check, kind, evidence)` (section 4), never a producer field.
4. OPEN LOCI -> the ladder (section 5) before any write.
5. POLICY (section 7) admits or refuses. A refused record is NOT written to the table: it goes to
   `<stem>.refused.jsonl` with the full derivation and the reason, and the caller keeps the record it wanted to replace.
6. WRITE: body (index = position, checked by the positioned writer) + `,"derivation":{...}}`; certificate and checker
   output copied to `<stem>.certs/` / `<stem>.checks/` under their sha256 names (written before the table line).
A record whose check did not run or failed is written only with class `none` and a typed reason (`no_certificate`,
`checker_error`, `budget_exceeded`, `binding_mismatch`, `rows_not_verified`, `identity_fail`, `open_loci_unclosed`,
`ladder_depth`, `curve_locus`, `legacy_unchecked`), and only if the policy lets `none` in at that liveness.
RENUMBERING (a replacement earlier in the table moves later records by k): only `index` changes, so `body_sha256`
and the `whole domain` class stay bound; the two position-dependent classes stay only if the move comes with a
partition proof of the replacement (section 5: every other record's first-match set unchanged); else they drop to
`none` (`prefix_changed`) until re-emitted.

## 4. Classes and their evidence (derived, in this order; the first that holds)
| class | holds when (all on THIS record's body) |
|---|---|
| `whole domain` | kind `lifted`: checker class `WHOLE_DOMAIN`: rows VERIFIED (v1, or v0+rows with every carried row recovered as a combination of the checker's own ordinary IBP rows at its seed, section 8), identity EXACT in Q(n, d), binding EQUAL (canonical(row) = target + every rhs term), every index factor closed by (c1)-(c3) on the record's domain. Kind `point`: exact per-key proof at its one key (instance-span, d symbolic). Kind `symmetry`: exact symmetry proof of the relation over the whole case domain. Kind `alias`: section 6 |
| `every first-match key in cone` | the record's first-match keys in cone C at its position (exact enumeration under the table prefix: count + key-list sha256) ALL carry an exact per-key proof; or `whole domain` minus OPEN loci whose keys in C are all proven (TRICKS 69 (d): a locus with no integer point in C is a statement about C only) |
| `sampled` | N >= `policy.min_sample` exact per-key proofs at keys of the record's domain (sampling seed and shells stated) |
| `none` | otherwise, with the typed reason |
`whole domain` needs no position. The two others name `position`, `prefix_sha256` (sha256 of the bodies of all earlier
rule records of the chart) and `cone`. The class word is never stronger than what the check output in the record
supports: the validator re-derives it (section 9).

## 5. Open loci: the replacement ladder (certify by replacement, TRICKS 68)
For a lifted candidate whose check is `OPEN_LOCI` (or whose lift added exception branches the replaced record lacks):
- each OPEN factor whose locus is LINEAR with a unit coefficient on some free slot s (`n_s = c` or `n_s = affine(other
  slots)`) becomes a NEW EXCEPTION BRANCH of the piece; the CHILD CASE (fixed slot / case equation on that locus,
  inheriting the replaced record's exception branches and every branch added above it) is lifted next and emitted by
  the same step 1-4 (its own certificate, its own check, its domain read from its own body);
- the ladder ends when a piece has no OPEN factor and adds no branch; depth bound `ladder.max_depth` (default 6),
  pieces bound `ladder.max_pieces` (default 16); a nonlinear (curve) locus is never branched;
- a declared terminal is never a target: a piece whose domain contains a declared terminal is refused
  (`improve::serves_key`), and the replaced record cannot contain one either (terminal rule);
- PARTITION: the pieces carry the replaced record's exception branches, so their domains partition its domain by
  construction; this is NOT trusted: `parametric::prune::record_liveness` proves (1) cover (the old record dead after
  the pieces), (2) containment (each piece dead after the old record), (3) disjointness (every pairwise conjunction
  record dead), exactly on the chart orthant; when a cone is given, first-match counts per shell of the pieces sum
  to the old record's and every later record's counts are unchanged (`record_first_match_counts`);
- the pieces are written CONTIGUOUSLY at the old position (ladder order, consecutive indices), each with its own
  `derivation` (`ladder` filled in);
- typed outcomes: `closed{pieces}`, `depth_exceeded{open}`, `curve_locus{factor}`, `lift_failed{piece, reason}`,
  `check_failed{piece, stage}`, `partition_failed{which}`, `budget_exceeded`. Only `closed` with every piece
  `whole domain` writes pieces; anything else writes nothing and the old record stays as it was.

## 6. Records that are not lifted: their own check kinds
- `point` (every slot numeric; the importer's point rule): `rr_proof --mode instance-span` at its key; whole domain.
  (ROUND 2: the emitter runs it on the record's own bytes at its key; the OUTPUT is stored in `check/` and the record
  states `check: {tool_sha256, output_sha256, mode: instance-span, verdict}`. table-check derives `exact_at_key` ONLY
  from that stored output, bound by its `record_sha256` = sha256 of the body line; a stated `exact_at_key` is never
  read.)
- `symmetry` / one-term (target = coefficient x image of another key under a family automorphism): instance-span with
  the symmetry rows (`--symrows`) or the symcert proof of the action; the symmetry group record is a stated input
  (sha256 in `check.args`).
- `alias` (slot-alias record: another record under a slot permutation): check = the permutation is an exact symmetry
  (symmetry group record) AND the aliased record is named by position + body sha256; the alias's class is DERIVED
  as min(alias check, the aliased record's class read at validation): bound by position, never by hash.
- `recipe` (solve-at-run-time entry, not a stored relation): check = the seal (every key of its face up to the cone
  solved and descended); class at most `every first-match key in cone`.
- `legacy` (a record imported unchanged from a table written before this contract): `none`, reason
  `legacy_unchecked`, until re-emitted through steps 1-6 (e.g. by `rr_node witness` with its recorded options, or by
  the ladder), OR brought in by the LEGACY IMPORT WITH EVIDENCE BINDING (section 12). Old class tables are never a
  source of a class.

## 7. Policy (data; REVIEW CHANGE 2026-11-18; `class::Policy` defaults)
- A rule the automation LIFTS (origin `lift`) and that is live: `whole domain`, or `every first-match key in cone C` only
  when every open locus is a curve the ladder cannot branch and the curve's keys in C are enumerated and proven per key
  (C is then part of the class and of the table's stated reach). `sampled` is NEVER produced for a lift; `none` refused.
- A LEGACY record brought in unchanged (origin `import`): `sampled` allowed (default yes for import, `min_sample` 200),
  bound by body sha + domain at its own position; `none` refused when live.
- Dead records may enter with `none`. LIVE = a first-match key in the policy's cone at the record's position; when not
  computed (`Liveness::Unknown`) the record counts as live. A refused record never enters the table (`<stem>.emission/
  refused.jsonl` keeps its derivation and reason).

## 8. Cost budget
The check must be cheap relative to the lift: budget = max(`check.floor_s` (default 120 s), `check.ratio` (default
1.0) x the piece's lift seconds), memory cap 6 GB + 16 x input GB (lane wcertproof's measured formula; checker
observed 0.07-52 s, <= 3.6 GB on the 54 T162 certificates). Over budget: the child is stopped, the record gets
`none` / `budget_exceeded`, and a DEFERRED CHECK item (same bytes, same certificate) is queued; when it finishes the
emitter RE-EMITS the record at its position (new derivation, new table version). That is still emission, not a later
classing pass: the class is again computed on the bytes being written. Row recovery (v0+rows) is part of the check
and counted in its budget.

## 9. The table summary and the validator `rr_campaign table-check`
- SUMMARY = per rule record: its class word read from its `derivation` AFTER re-derivation, and its liveness from the
  exact dispatch; counts per (class x live / dead). It is an OUTPUT only; nothing reads it to write a class.
- `rr_campaign table-check --table T [--level bind|recheck] [--cone C] [--checker rr_proof] [--sample K]`
  REFUSES (exit 2, one line per offending record) a table in which a rule record: lacks `derivation`; has a
  `body_sha256` that differs from its own bytes; has index != position; names a certificate or check output that is
  missing or has another sha256; states a class that `class_of` does not give from its stored check output and
  evidence; (recheck) has a stored stage verdict that differs from a RE-RUN of the checker on its body bytes (all
  records, or a deterministic sample of K when given); names an alias target or replaced record that is not at the
  stated position with the stated body sha256.

## 10. Today (v0 / v0+rows) and with v1
- v0 (no rows): rows not verifiable, identity not checkable -> class `none` (`rows_not_verified`), whatever the
  producer says.
- v0+rows: identity exact over CARRIED rows; whole domain only if the checker itself verifies every carried row as an
  IBP identity instance. The engine's preconditioned basis rows are POLYNOMIAL combinations, at the same seed, of the
  ordinary source rows [read: weightcert solver/precondition/provenance.rs, `Node::Subtract` with polynomial scales].
  So the checker can RECOVER the combination itself: solve A_j = sum_m c_jm O_m(seed_j) over Q(n, d) with its own
  generated rows O_m (Symbolica `Matrix::solve_any`), compose W_(s,m) = sum_j w_j c_jm and check the v1 identity
  sum W O(s) == row exactly; the denominators of W join the factor list. Verdict `ROWS_VERIFIED_BY_RECOVERY`. Its
  feasibility on 987's pieces is the key question of item 4.
- v1 (`ordinary` / `composed`, engine branch weightcert 6031bd9a): rows verified directly by the generator; no recovery.
- RESULT (2026-11-18, item 4): recovery is EXACT and complete on 987's three pieces (103 / 154 / 140 weighted rows, every
  combination polynomial: 0 new denominators, ~30 s, < 40 MB each): class WHOLE_DOMAIN with no producer trust; a doubled
  weight -> IDENTITY_FAIL. The run on the other binding certificates of WEIGHT-CERT-CHECK.md: reports/EMISSION-CHECK.md.

## 11. Implementation (ac/gencheck) and routing
- Pure: `rustred-campaign-core::emission` (body, class, ladder incl. partition proof and curve-key enumeration, validate).
- Process: `rustred-campaign::emission` (child, emitter, replace, tablecheck). Side files are CONTENT-ADDRESSED in
  `<stem>.emission/{cert,check,prov,policy,keys}/<sha256>.json(l)`: the record names sha256s only (member < 1 kB).
- Checker: `rr_proof --mode weight-cert ... --recover-rows` (row recovery, section 10) and `rr_proof --mode source-rows`.
- CLI: `rr_campaign table-check --table T [--recheck --checker BIN [--every K]] [--manifest OUT]` (exit 2 = REFUSED;
  the manifest is one line per rule: chart, position, index, body_sha256, kind, class, certificate sha256, check output
  sha256); `rr_campaign emit-replace --table T --rule N [--offset B] --rr-node BIN --checker BIN --out DIR [--cone S]
  [--max-depth 6] [--max-pieces 16] [--case-budget S] [--policy FILE] [--write-table] [--witness-off]`.
- QUESTION (a), TESTED 2026-11-18 (T178 core, 1,515 rules, strict native import --stream-payload + --check, 12b and 12a,
  plain and shared): the member is ACCEPTED, IGNORED and DROPPED (payload byte-identical with and without it). Hence the
  per-rule CLASS MANIFEST written by table-check is the package metadata; table-check runs on the delivery's table + side
  directory, not on the payload.
ROUTING (every write of a rule line in the campaign crates):
| site | status |
|---|---|
| improve writer (`supervisor/improve.rs` write_outputs: improved / replacements tables) | ROUTED when the improve config has `emission` (checker, cert_dir in the job's run dir, policy): every replacement line is emitted before writing, refused -> old rule kept + reason. Without `emission`: the old writer (not routed). Needs the producer to write the job's certificates into its run directory (engine list, item 5) |
| certify by replacement (`emission::replace`, `rr_campaign emit-replace`) | ROUTED (new path) |
| `economy::improve::{replacement_lines, apply, apply_subtrees, subtree_lines}` | helpers of the improve writer: their output goes through the emitter when configured (body hash unaffected by their origin / index edits) |
| THE GATE (`emission::gate`, `rr_campaign emit-table --table T --out O --checker BIN [--certs DIR ...] [--kind lifted/point/symmetry/auto]`) (ROUND 2) | every rule line of a written table through the emitter: lifted records with the certificate of THEIR lift (found by the record's `case` = an engine certificate's first member), point / symmetry records with the instance-span check at their key; FAIL CLOSED (one refusal, nothing written) |
| node assembly (`assemble::assemble`: `<node>.assembled.table.jsonl`) | ROUTED THROUGH THE GATE: the library has no production caller in the campaign crates; its output is emitted by `emit-table` with the producing jobs' certificate directories (and the point kind for point records) |
| closeloop repair, symbolic table (`closeloop/repair.rs`) | ROUTED IN THE FLOW (ROUND 2) when the repair policy has `emission` (same `Settings` as improve): the kept records go through the gate with the certificates of THEIR face-lift jobs (`<run_dir>/<cert_dir>`); a refusal stops the node (nothing written). Default off (digest unchanged) |
| closeloop repair symrel table, closeloop source relations tables, closeloop derived (`symmetry-at-key`), termmin (`symmetry-relations`) | ROUTED IN THE FLOWS (ROUND 3, `gate::gate_beside`) when the node config (closeloop `NodeCfg.emission`; repair policy `emission` for symrel) is set: every record checked on its own bytes (point / symmetry: instance-span at its key, `symmetry_group` / `zero_sectors` records from the settings); the EMITTED COPY `<table>.emitted.table.jsonl` (+ side dir) is written ONCE beside the flow's own table, whose bytes stay those the flow re-derives and verifies on a restart; a later pass reuses the copy when its bodies equal the table and table-check accepts it; a verifying restart never re-gates (missing / different copy = refused); a refusal stops the node. Default off (serialisation unchanged) |
| legacy tables (genchoice choice, shipped finals) | `rr_campaign legacy-import` (section 12) |
| genchoice (choice of a whole engine table), contract seal (engine `F.table.jsonl`) | not writers of rule lines (they select / seal what the engine wrote); the chosen table is a LEGACY import under the contract until its records are emitted |
| shipecon probe table, wire_composite / symcomp / symrows inputs | not delivered tables (probe / job inputs) |

## 12. LEGACY IMPORT WITH EVIDENCE BINDING (ROUND 2, `emission::legacy`, `rr_campaign legacy-import`)
The class of a record brought in UNCHANGED, decided from the record's bytes and the evidence FILES alone (config
`rustred.legacy-import.v1`: verdict TSVs of reviewing lanes + their proof-object tables, per-key proof-line files,
first-match count / liveness files, the reference table whose positions reference-keyed rows use). NO class table is
read.
- BINDING (strict, TRICKS 67): a verdict row binds at the record's OWN position (reference rows: its
  `origin.dispatch_position`; table rows: its position) iff relation sha256 AND domain sha256 are the record's (the
  row's own values, else the reference record's computed from the reference table's bytes). A row of ANOTHER position
  binds only as a whole-domain proof object of the IDENTICAL statement (equal relation and domain sha256).
- KINDS of a row: vacuous / dead verdicts = no evidence (a LIVE record with one at its own position is refused);
  `whole domain` only from a whole-domain class word or a FACE-VERDICT whose proof object states `PROVED whole domain`
  and nothing about first-match keys; per-key verdict TEXT gives no class (its keys are not listed).
- PER-KEY: proof lines bind by `line_sha256` = sha256 of the record's line at its position; `every first-match key in
  cone` iff, shell by shell D + r <= bound, the distinct proven keys equal the COMPLETE first-match count; else
  `sampled N` (N distinct proven keys). `dead in cone` (delivery word) from the counts when no whole-domain proof object.
- EMITTED form: kind `legacy`, origin `import`, no certificate; `class.legacy` = {proof_object {row_sha256, binding},
  proofs_line_sha256, liveness}; side files `evid/<sha>.json` (the bound row), `keys/<sha>.jsonl` (the proven keys).
  table-check re-reads the row and re-verifies relation, domain and kind against THIS body, the proof-line binding and
  the key-list counts.
- T162 (shipped table a9c34170, 2026-11-18): 1,930 / 1,930 classes equal to the delivery's words (N included): whole
  domain 1,552 (1,526 helper A own + 24 helper B own + 2 identical statement: 1292 = reference 2863, 1299 = reference
  2469), every 115, sampled 48, dead 215, none 0; table-check ACCEPTED (1,930 rules, 0 refused).

