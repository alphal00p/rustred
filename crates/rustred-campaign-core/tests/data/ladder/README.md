# Ladder replay fixtures (test data)

Converted from read-only reference records with Python used ONLY to inspect them (golden rule 11); the converters are
`conv/k1conv.py` and `conv/h2conv.py` as recorded in WS/notes/implementer.ladder.md (2026-11-14).

- `t178_k1_attempts.jsonl` (552 records) and `t178_k1_items.jsonl` (62 items): every readable
  `/common/ben/sym-investigation/k1/results/<item>/*.json` (mode case | point, shells S0..S2 or S0..S3; skipped: locus B
  item, two S2m1/Sm2 variants, two replay-only files): case (sd, sn, fixed, eqs), seed offsets (the record's `recenter`
  vectors), floor (`seed_generator.floor == "lines"`), depth (2 | 3), measure (M1 | C), relation found, RHS terms,
  closure rows, tie-break flag (`!rhs.all_top_Dr_lower`), wall seconds, census keys and verdicts (regular | exception |
  fail | not_in_case). Items: k1/faces.jsonl census and closure keys; worker and STAGES line from k1/w*/STAGES.txt.
- `t162_h2_attempts.jsonl` (91 records) and `t162_h2_items.jsonl` (44 faces): `/common/ben/sym-investigation/h2/`
  sweep2_orbits.jsonl, sweep2_others.jsonl, sweep2.jsonl, mc.jsonl (the `_rev` union record skipped: same seed set, the
  helper's eliminator is order dependent, H s.1); census = the 35 points with D <= 7 per face (h2/STAGES.txt:22, :50);
  faces from h2/group.json orbits.

Reading in the tests: a recorded relation stands for the engine's rule; its exception set is the fit of the points it does
not cover (the reference tools have no coefficients). Unrecorded (case, setting) pairs answer "no relation" (can only
add cases). Census "exception" verdicts count as covered for the worker that read them so (w1; item 14 of w2).
