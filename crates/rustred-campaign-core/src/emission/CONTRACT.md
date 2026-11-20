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
  `<stem>.emission/{cert,check,prov,policy,keys}/<sha256>.json(l)`: the record names sha256s only (member < 1.25 kB;
  1 kB before the descent member of section 14, about 190 bytes, was added on 2026-11-19).
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


## 13. d = 4 REGULARITY: REPORTED, TIE-BREAK ONLY (ROUND 7, 2026-11-19; USER RULING, WS/ops/LANE_RULES.md rule 21)
SUPERSEDES the round-4 text below (kept, marked SUPERSEDED). The user withdrew the 2026-11-15 rule ("a relation regular at
d = 4 wins whatever its size"). Pole structure at d = 4 is a property of the final basis; the basis rotation is done on an
exact-d result. Therefore:
- ORDER when a record or candidate is chosen or accepted:
  validity (derivation exact, DESCENT, domain / partition) -> downstream SINGLE-CORE COST (the cone price where a policy asks
  for it, section 17; measured ops / record seconds; else the modelled cost, section 19) -> size -> d = 4 regularity LAST,
  tie-break only.
- NOTHING is refused, dropped or deprioritised because it is singular at d = 4.
- FOLLOW-UP ac/rc-followup (2026-11-20; N1-N3 of the gate R-C read; not part of the gate object):
  - N1: d = 4 regularity comes after EVERY other tie-break: cost -> size in monomials -> bytes -> regularity. This holds in
    genchoice (`PoleCand.size_ties`; genchoice's own winner ties monomials then bytes), improve's rule verdict (bytes), the
    piece planner (kind, edge terms, then regularity) and the seedmin default objective (terms, seeds, pole order).
  - N2: a measurement that FAILS is reported "not measured": the verdict has `not_measured: true`, the `error` text and
    `not_worse: null`. The record stands on its validity alone. table-check and entry-check note it, never as regular.
  - N3: seedmin's explicit `system_regularity: "on"` refuses a smaller drop that worsens the system's p*. That is NOT
    rule-21 behaviour; it is off by default and used only when a policy names it.
  - emit-replace, the ladder, the improve writer and emit-entries ACCEPT a valid replacement whatever its regularity.
  - table-check and entry-check never refuse for a missing, "worse" or v1 verdict.
- The MEASURE STAYS, as a REPORTED NUMBER: the same exact measure, keys and verdict file (schema
  rustred.emission.regularity-d4.v2 with `role`); the derivation still names it in `replaces {old_body_sha256,
  regularity_sha256}`; table-check lists what it says in `reported.notes` (counts regular -> singular, singular -> regular,
  ...). `regularity-compare`, `regularity-at-key` and `regularity-pairs` stay as measuring tools.
- RECOGNISABLE OLD RESULTS:
  - verdicts of schema regularity-d4.v1 were written under the old rule (where `not_worse: false` meant REFUSED);
  - the emission policy carries `version` 2 since this ruling, and stored policies without it are version 1;
  - genchoice / improve pole policies carry `version` 2;
  - seedmin / repairflow policy schemas are v2.
- TESTS: tests/regularity_t180p2.rs. 670 -> 672 is ACCEPTED, with the regression reported exactly: 495 of 495 served keys
  regular -> singular, and the review's 315 simple-pole daughters at its witness key. The descent negative test (T266 K2,
  section 14) stays a refusal: descent is validity.

## 13-OLD. d = 4 REGULARITY ACCEPTANCE of a replacement (ROUND 4, 2026-11-19; SUPERSEDED 2026-11-19T17:46Z by rule 21: kept for the record)
RULE: a relation regular at d = 4 wins whatever its size; a replacement may never trade d = 4 regularity for speed. Part of
the ACCEPTANCE of every replacement (emit-replace, the replacement ladder, the improve writer); no policy switch turns it off
for a live replacement; a legacy import records "not compared".
- MEASURE (`rustred-campaign-core::emission::regularity`, exact): at an integer key, every rhs coefficient (num/den,
  polynomials in the indices and d) is evaluated in exact integers, expanded around d = 4 (Taylor shift), terms with the
  same DAUGHTER KEY are merged (the engine adds them) and the pole order of each merged coefficient is the order of its
  numerator over the common denominator (a 0/0 is resolved; a merged coefficient that is identically zero is no daughter).
  The key's regularity = the LARGEST pole order among the merged daughters (0 = regular). A term whose denominator is
  identically zero at the key makes the relation UNDEFINED there.
- KEYS: every key the OLD record serves as FIRST MATCH in the cone D + r <= R (default R = 10): the old record's face keys
  in the cone, dispatched by the core dispatcher (`cover::Dispatcher`, the engine's find_rule order; witness loci off: the
  engine writes their factors into `exceptions`) over the OLD table. More than `max_keys` (default 20,000) face keys: a
  STATED SAMPLE (the first `max_keys` face keys in shell order; the sampling text is in the verdict). Each served key is
  measured with the old record and with the FIRST replacement piece whose domain applies (the replacement's own order).
- VERDICT per replacement: worse at a key = the new pole order is larger than the old one (regular -> singular included)
  or the new relation is undefined where the old one is not; ACCEPTED iff NOT WORSE at every compared key AND at least one
  key was compared (zero compared keys = refused, `regularity_d4_not_compared`: never a vacuous pass). A served key no
  piece serves is a structural finding (refused). An identical relation (a single piece, same target + rhs: an in-place
  re-certification) is NOT WORSE by construction (verdict `same_relation: true`, nothing compared).
- REFUSED: the old record stays (emit-replace: outcome `regularity_refused`, the pieces kept aside in pieces.refused.jsonl;
  improve: the old rule kept, reason in the record's untried notes). A SPLIT (the new rule only where it is not worse) is
  not built automatically: it is a new set of pieces (e.g. case branches cutting out the worse keys) that must pass this same
  acceptance (each piece whole domain, partition of the old domain proven).
- EVIDENCE: the verdict (schema `rustred.emission.regularity-d4.v1`: tool, old body sha256, new body sha256 per piece, the key
  set (kind, cone, served, compared, sampling, keys sha256), counts (keys, regular_to_singular, singular_to_regular,
  unchanged, worse, better, new_undefined), the same counts PER BRANCH (piece), the first regressions (key, piece, old / new
  pole order, worst new daughter), structural findings, not_worse) is a side file `<stem>.emission/regularity/<sha256>.json`;
  every accepted piece's derivation carries `replaces: {old_body_sha256, regularity_sha256}`.
- table-check REFUSES a replacement record (a `ladder` member, or `replaces`) without the verdict, with a verdict that is not
  NOT WORSE, that compares another old record, or that does not name this body.
- CALL (lane rulecost, v6): `rr_campaign emit-replace ... [--regularity-table OLD_FULL_TABLE] [--regularity-cone R]
  [--regularity-max-keys K]` (the old table defaults to --table: give the FULL old table when --table is a mini / job-header
  table, else the first-match set is the old record's whole face); the improve config's `emission.regularity: {cone,
  max_keys}`; standalone (already emitted pieces): `rr_campaign regularity-compare --old-table T --rule N --pieces P.jsonl
  [--cone R] [--max-keys K] --out DIR` (exit 0 not worse, 3 worse) or `--batch SPEC.jsonl` (lines {label, rule, pieces}: the
  table read once).
- TEST: the outside review's witness (astra_5L/review-20261119/T180p2-v5/regularity/WITNESS.json) is
  tests/regularity_t180p2.rs: T180p2 670 -> 672 is REFUSED (495 / 495 served keys regular -> singular in D + r <= 10; the
  review's 315 simple-pole daughters at its witness key reproduced exactly).

## 14. DESCENT of every emitted record (ROUND 5 H0 GAP-4, 2026-11-19; lane t266's correction)
RULE: every right-hand-side term of an emitted relation must be LOWER than its target in the node's order, by the ENGINE's
own descent test. Exact + d = 4 regular + NOT LOWER = REFUSED (lane t266's candidate (i): four exact, regular symmetry
relations of T266 that ascend in phys_rev and close a cycle with the partner's rule). Part of the ACCEPTANCE of every emitted
record (emit-table / the in-flow gate / emit-replace / the improve writer / the closeloop repair gate); no policy switch turns
it off.
- WHY EVERY RECORD: the lifted path only ASSUMES descent, it does not guarantee it by construction. The node driver records
  a failure as data (`native_descent_not_lower` in its end record) and goes on; the T162 final certification needed a
  separate strict-descent step (b1). So the member applies to every emitted rule, lifted, point, symmetry and replacement.
- TEST: `rr_node descent --table <the table header + this one record>` (the order is the header's; the engine prints
  `{failing_rules, first, not_lower, order{name, ...}, rules, terms}`, exit 0 pass / 1 fail). The emitter runs it FIRST, on
  exactly the record bytes, before policy admission. Bound to the record: the output must be of 1 rule with as many terms
  as the record's rhs. No engine binary, no JSON output or a binding mismatch = refused (`descent_not_checked`); NOT LOWER =
  refused (`not_lower (<first>)`).
- EVIDENCE: the engine's output line is a side file `<stem>.emission/descent/<sha256>.json`; the derivation carries
  `descent: {tool_sha256 (the rr_node binary), output_sha256, verdict LOWER | NOT_LOWER, first}`. An IMPORT states
  `descent: {verdict: NOT_CHECKED}` (exempt, and says so).
- table-check REFUSES an emitted record (origin lift) without the member, with an output missing beside the table, of
  another record (rule / term count), with NOT LOWER, or with a stated verdict that differs from the stored output.
- BINDING LIMIT AND THE RE-RUN: a PASSING engine output names no record (only counts and the order), so the record-level
  binding is 1 rule + the rhs term count. `rr_campaign table-check --descent-node BIN` RE-RUNS the engine's test on header +
  each emitted record's own bytes and refuses a record whose stored output differs (test: K2 emitted with a wrong descent
  binary carries a forged LOWER of the right shape; the record-level check passes it, the re-run refuses it).
- CALL: `rr_campaign emit-replace ... [--descent-node BIN]` (default: the producer `--rr-node`); `rr_campaign emit-table ...
  --descent-node BIN`; the node / improve / closeloop config `emission.descent_node: BIN`. The binary's sha256 is recorded
  per record. Reference binary (round 5): int/9-land2 rr_node built on the gate-PASS head (origin 4186f4472), sha256
  2bac8f270e176d1d347f77ee730fc84ab6ae04e1450ba9c2e7c5f3b2127ad637 (no frozen engine-line5 rr_node with the `descent` verb
  was found among the gated directories).
- TEST (permanent negative): tests/descent_t266.rs. Candidate (i) K2 (frozen, implementer-scratch/t266study/candidates/
  T266-cand-i-symrel-20261119T1401Z) with a stand-in EXACT checker is REFUSED `not_lower`, verdict NOT_LOWER (first: the
  term I(..,0,0,0,0,-2,0) above the target I(..,0,0,0,-2,0,0) in phys_rev); the one-dot terminal image
  I(1,2,1^12;0^6) -> I(2,1^13;0^6) descends and is accepted (LOWER). Fake-checker coverage in emission_gate_fake.rs (the
  gate refuses an ascending record and writes nothing; no descent binary = refused; table-check refuses a removed or
  restated member).

## 15. SYMMETRY RELATIONS: the checker's symmetry-row mode (ROUND 5 H0 GAP-1, 2026-11-19; lane t266's request)
A record that states it is a symmetry relation, `origin: {kind: "symmetry_relation", element: e}` (a point record: its own key),
is proven by the independent checker on the EXPANDED SYMMETRY ROW of element e at its key, alone:
`rr_proof --mode instance-span --table <header + record> --rule N --offset B --keys <the key> --symmetry-group G
 --symrows R --symrows-only --emit-certs DIR`, R = one line `{element: e, id, offset: 0^arity}` (the kernel derives the line
permutation and the numerator decomposition from the element's matrix and checks unimodularity). Without the rows the kernel
refuses the seedless record (`MISSING_SEEDS`: lane t266's emitter attempt, and the red control of the test).
- G is the checker's `--symmetry-group` (emit-table `--symmetry-group`, flow config `emission.symmetry_group`); none given =
  refused (`a symmetry relation needs the symmetry group`).
- The kernel writes the exact certificate `DIR/cert_<key>.json` (schema rustred.proof-kernel.span-cert.v1: rule,
  record_sha256 of the record line, key, L, weights; a symmetry row is m < 0). BINDING (emitter and table-check,
  `symrows::verify`): the output says `symrows: {given >= 1, only: true}` and was made on these record bytes; the certificate
  was made on these record bytes, at this rule index and key, with weights on symmetry rows only (no IBP row, m >= 0); the
  request names the record's own element at offset 0. Anything else = refused (`symrows_binding (...)`).
- EVIDENCE: the request and the group are side files `<stem>.emission/symrows/<sha256>.json`, the certificate is
  `<stem>.emission/cert/<sha256>.json`; the derivation's check carries `symrows: {element, request_sha256, group_sha256, only:
  true, rows}` and `certificate: {sha256, bytes, form: "span-cert"}`. Class: whole domain (a point record proven at its key).
- table-check REFUSES a record naming a symmetry element without the `symrows` member and certificate, or whose output,
  certificate or request does not bind to its bytes; an edited record (other bytes) is refused (body sha256).
- Records without `origin.element` (plain point records, the closeloop symmetry tables) keep the plain instance-span check.
- Descent (section 14) applies as to every record: lane t266's candidate (i) K2..K5 are EXACT on their symmetry row and
  REFUSED for not lower.
- TEST: tests/symrows_t266.rs (REAL rr_proof 03f436f7 + REAL rr_node descent 2bac8f27, one licence slot): K2 without its
  element -> MISSING_SEEDS (red control); K2 with element 0 -> EXACT, certificate weights and key equal to the lane's frozen
  K2.certificate.json, refused for descent only; the X15 form (element 1: I(1^14; -1,0^5) = I(1^13,0; 0^6) + I(1^14; 0^6))
  -> exact, LOWER, EMITTED, table-check ACCEPTED (whole domain), also with the descent re-run; edited after emission ->
  table-check refuses; coefficient 2 -> NOT_ALL_KEYS_IN_SPAN, refused. Fake-checker coverage in emission_gate_fake.rs.

## 16. RELATION ENTRIES from emitted records (ROUND 5 H0 GAP-2 + GAP-3, 2026-11-19; lane t266's request)
AMENDED 2026-11-19 (rule 21):
- The d = 4 comparison at a key against its old object is REPORTED (verdict side file, `replaces`, the summary's
  old / new pole); an entry WORSE at d = 4 is written.
- A key WITHOUT an old object is written too ("not compared").
- entry-check no longer requires the regularity verdict; it still checks the stated pole order against the relation (the
  engine reads that number).
- The text below that refuses for "worse" or "no old object" is superseded.
- ENGINE NOTE: at 4186f4472 the relation store ranks shipped entries (evidence, -pole, -terms); the engine lane flips it
  (D4-REVISIT M9).
GAP-2, THE d = 4 ACCEPTANCE AT ONE KEY (section 13's measure and rule, another old object):
- (a) the old object of a key K is the stored `rustred.relation-entry.v1` the engine would serve at K: among the given entry
  files' entries of the node identity at K with a valid digest, the engine's rank (evidence level, then the smaller pole order
  at d = 4, then fewer rhs terms, then the smaller digest). Its id = its entry_digest; its relation = its body (target, else
  I(K); rhs).
- (b) else the table rule that serves K as FIRST MATCH in the old table (the core dispatcher, witness loci off), restricted to
  K: an entry placed over the table shadows that rule at K only. Its id = the rule's body sha256.
- No old object = refused (nothing to compare). The verdict (schema rustred.emission.regularity-d4.v1, key set kind "one key
  of a stored relation entry" / "one key of table rule N (first match)", plus `old: {kind, entry_digest | table + rule, ...}`)
  is a side file; the record's derivation gets `replaces {old_body_sha256: the old id, regularity_sha256}`. Worse at K = refused.
- Standalone: `rr_campaign regularity-at-key --key JSON --pieces P.jsonl --out DIR (--old-entries F... --node-identity ID |
  --old-table T) [--header-table T]` (exit 0 not worse, 3 worse).
GAP-3, THE WRITER `rr_campaign emit-entries --emitted T --node N --node-identity ID --out ROOT [--revision R]
[--old-entries F]... [--old-table T] [--work DIR] [--descent-node BIN]`:
- T must pass table-check (with the descent re-run when --descent-node is given); every record must be a point record at its own
  key (one entry serves one key), with a LOWER descent verdict, and NOT WORSE than its old object at the key (GAP-2).
- One entry per record: body {type relation, key, target, verdict SOLVED, all_lower true, proof {ok, kind emitted/<check mode>,
  tool / output / certificate / descent-output sha256}, relation_digest null, determinism_id
  `rustred.relation-import.v1/kind=emitted-<origin kind>/statement=<record body sha256>`, rhs [[term, coefficient, top|sub]],
  rhs_count, emitted_record: THE EMITTED RECORD LINE (derivation with check, class, descent, replaces)}; entry_digest =
  sha256 of the canonical body (reproduces the engine's digest of a shipped T266 entry: tests/entries_t266.rs); evidence
  per-key (the independent checker), diagnostics.pole_order_at.4 = the measured pole order at the key.
- The root: `<ROOT>/<node>-<id16>/entries-emitted.jsonl`, `index.jsonl`, `MANIFEST.json` (rustred.node-service-manifest.v1,
  entries.precedence `over_table`, over_table_keys, kinds import), NODE.json, SHA256SUMS and the side directory
  `entries-emitted.emission/`. FAIL CLOSED: one refusal = nothing written; never over a directory that holds entries. The
  writer runs the entry-check on what it wrote.
ENTRY-CHECK `rr_campaign entry-check --dir NODE_DIR` (exit 0 / 2) REFUSES an entry: digest not of its body; body not a proved,
lower relation of its key; WITHOUT its emitted record; a record whose derivation is not of an emitted record (origin lift);
WITHOUT its regularity verdict (`replaces`); WITHOUT a LOWER descent verdict; a record table-check's record check refuses
(check, class, descent, the regularity verdict naming this body); a key / target / rhs that is not the record's; a stated
pole order that is not the relation's at the key.
TESTS: tests/entries_fake.rs (4): (a) over a stored entry (the engine's choice of two at the key), written over_table,
entry-check ACCEPTED; worse = refused and nothing written; no old object = refused; (b) one key of the first-match table rule;
entry-check refuses an entry without its emitted record, its regularity verdict, its descent verdict, with a wrong pole order or
another rhs. tests/entries_t266.rs: the digest of a shipped T266 entry reproduced.

## 17. CONE-PRICING ACCEPTANCE of a replacement (ROUND 6 J4, 2026-11-19; design of lane rulecost 13:25Z, agreed by the coordinator)
AMENDED 2026-11-19 (rule 21):
- The order is validity -> cone price (this section) -> size -> regularity.
- The line "the d = 4 guard first; a candidate worse at d = 4 is refused before it is priced" is SUPERSEDED: the d = 4 measure is
  reported only.
BINDING ORDER: the d = 4 guard (section 13) first; a candidate worse at d = 4 is refused before it is priced; a relation regular
at d = 4 wins whatever its size. OPT-IN by policy (`cone_pricing: true`; it needs engine runs). A policy without it serialises
as before (same sha256).
- MEASURE: an A/B PARTIAL RECORD PASS of the real request. The old arm is the old table / input; the new arm is the candidate
  (a package view with only the owner payload replaced, or the candidate's inputs).
  - Engine: the existing stop-after-phase (RUSTRED_RESUME_DIR + RUSTRED_RESUME_STOP_AFTER_PHASE = entrance + 3, or + 5 for a
    third level) and owner statistics (RUSTRED_P_OWNER_STATS).
  - ONE thread (`reduce-file <lib> <inputs> modular 1`), each arm one licence slot. The run ends with "stopped after a saved
    phase" (non-zero rc, expected).
- READ two full levels from the ENTRANCE (the Route phase where the replaced relation's children first arrive; Route = even
  phase 2 (14 - lines)): Route, Apply, Route, Apply. The price at the SECOND Apply:
  - children handed down;
  - keys, seconds and children per RECEIVING owner;
  - the high-shell population (keys with D + r above the request's);
  - THE (lines, dots, rank) SHELL HISTOGRAM of the keys stepped, old vs new (by rank, by dots, mean dots / rank, keys weighted
    x4 per rank step as information). This is the coordinator's addition after LIT-AUDIT-SZ-CL: a no-raise relation trading
    dots for rank downstream must show.
- VERDICT (refusal rules):
  - `cone_worse`: more children at the second Apply than today;
  - `cone_owner_grows`: a receiving owner grows by more than 1.25x in keys received or children handed down (owners under 100
    on both sides ignored);
  - `cone_absorbed`: an ENTRANCE-ONLY WIN, i.e. an entrance gain above the tolerance (4 %) and a second-Apply gain below it.
  - Otherwise ACCEPTED.
  - RANK among accepted candidates: children at the second Apply, then high shell, then seconds.
  - A third level is reported when the arms carry it and the second-level difference is still moving.
- WORK, NOT VALUES: the partial pass stops before the terminals. Exactness is the derivation's check; a full A/B compares
  tagged terminal VALUES, not expansions (CL Theorem 2.4).
- EVIDENCE: the verdict (schema rustred.emission.cone-price.v1: old / new body sha256, the request with binary, inputs and
  per-arm runs, the phases read, levels, ratios, the shell histogram, per-owner table, owners grown, rules, accepted, reason,
  rank key) is a side file `<stem>.emission/cone/<sha256>.json`; the replacement's derivation names it in
  `replaces.cone_sha256` (`rr_campaign cone-stamp`).
- table-check (policy cone_pricing) REFUSES a replacement without the member, or with a verdict that refuses, prices another
  old record, or does not name this body.
- CALL: `rr_campaign cone-price --old-stats A --new-stats B --entrance N --out V.json`, or `cone-price --run --binary
  p5l_library-diag4186 --old-lib L --old-inputs I1 [--new-lib L2] --new-inputs I2 --entrance N --out DIR [--third-level]
  [--env K=V,...]` (exit 0 accepted, 3 refused); `rr_campaign cone-stamp --table T --index I --verdict V --out T2`.
- IN emit-replace (added 2026-11-19, consolidation): `rr_campaign emit-replace ... --cone-verdict V.json`.
  - After the derivation, descent and the regularity report, EVERY piece of the replacement must be allowed to name the verdict.
    The checks are those of cone-stamp: an accepted verdict, of this old record, naming each piece's body when it names bodies.
  - Then each piece carries `replaces.cone_sha256` and the verdict is written once as `cand.emission/cone/<sha256>.json`;
    report.json has `cone {verdict, sha256, accepted, stamped_pieces}`.
  - Otherwise nothing is stamped or written: outcome `cone_refused` (reason, first piece that may not name it), the pieces
    kept aside in pieces.refused.jsonl, exit 3.
  - Without the flag under a `cone_pricing` policy the report says that table-check will refuse the pieces until cone-stamp
    names an accepted verdict.
  - The view builder (the old package with only the candidate owner's payload replaced) is NOT part of it: the arms are
    built by the caller (DEFERRED: reports/DEFERRED/gencheck-cone-verdict-emit-replace.md).
- TEST (permanent negative): tests/cone_t269.rs on lane t162opt's ab2 statistics, plus the same A/B re-run through `--run`
  (round 6: diag4186, 176 / 167 s, 12 GB per arm). The review's T269 glued triangle is REFUSED `cone_worse`:
  - entrance (twelve-line Route): 327 -> 12 keys;
  - twelve-line Apply: 23,780 vs 23,886 children;
  - second Apply (eleven lines): 905,807 vs 922,379 children (+1.83 %), with several receiving owners above 1.25x in keys;
  - shells: dots-5 keys 2,997 -> 5,133, mean dots 3.47 -> 3.61 at equal rank.

## 18. THE (key, image) d = 4 PAIR MEASURE (ROUND 6 J3; lane rulecost's O4 item 6)
AMENDED 2026-11-19 (rule 21): a MEASURING TOOL. The T164 numbers below are reported facts: 1,337 of 28,681 moved keys get a
higher pole order. Under rule 21 they are no reason to scope or switch off orbit canonicalisation; only a measurement showing
it SLOWER is.
- For an owner whose TABLE IS ON DISK: `rr_campaign regularity-pairs --table T --pairs P.jsonl --out DIR [--threads N]
  [--descent-out F]`.
  - Each pair (key K, exact orbit image K' = sigma(K), sign; I(K) = sign I(K')) compares the section-13 pole order of the
    key's first-match rule at K (today) with the image's first-match rule at K' (after canonicalisation). A declared-terminal
    image is regular.
  - WORSE = the image's pole order is higher, or it is undefined only there.
  - The command writes the one-term descent table K -> sign K' for the engine's order. `--descent-out` restricts the verdict to
    LOWER images.
  - The engine's choice (the least orbit member) is found with gencheck S/bin/orbit_least.py: all ordered member pairs through
    `rr_node descent`.
- Pairs: gencheck S/bin/key_images.py (the exact one-key images of lane rulecost's cone_images.py, plus the sign of ISPs mapped
  onto negated forms) from reached keys and the saved verified groups.
- TEST: tests/pairs_toy.rs. RESULT on T164 (lane rulecost's 53,518 reached keys of T278's record, phase 9; the minimised T164
  table = the original minus the minimiser's 680-record removal set, 357 rules):
  - 43,585 keys have an exact image; 28,281 orbits, each with a unique least member in the table's order;
  - 28,681 keys move to their orbit's least member: 1,337 WORSE (1,325 regular -> singular), 688 better (677 singular ->
    regular), 26,656 equal.
  - So unscoped canonicalisation at T164 makes 1,337 reached keys less regular.
- NEEDED FROM THE ENGINE where no table is on disk (template lanes Insertion / SeriesParallel; T278, whose payload is only in the
  package):
  - (1) a ONE-KEY STEP DUMP: for an owner class and a key, the relation the engine applies there, exactly in d, as a rule line
    (target, rhs [[daughter, coefficient]]) in the owner's frame. For a template lane: the lane's generated expansion. For a
    Custom owner: its first-match payload rule at the key. A verb (`p5l_library step-key <lib> <class> <key>`) or
    RUSTRED_P_STEP_DUMP=<file> during a reduce.
  - (2) the MOVED-KEY DUMP that lane rulecost requested (RUSTRED_P_CANON_DUMP: owner class, key, image, sign), i.e. the
    engine's own choice under its order key.
  - Alternatively for Custom owners: a payload export of the owner's rules as table_dump.v1 records, after which
    regularity-pairs runs unchanged.
