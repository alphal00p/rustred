# Decision table of the child-job contract (child-job.md 6.5)

GENERATED from `rustred_campaign_contract::classify::ROWS` by `decision_table_markdown()`; do not edit by hand.
Regenerate: `RR_UPDATE_GOLDEN=1 cargo test -p rustred-campaign-contract --test audit_regressions decision_table`.
Rows are evaluated in the order of child-job.md 6.5 (first match wins); the order of the lines below is the
order of `ROWS`.

| # | condition | kind | accept | ledger action |
|---|---|---|---|---|
| 1 | c=0, O, S | complete | canonical | item_done |
| 2 | c=10, O, S, stop max_cases, verified rules >= 1 | rule_found | canonical | queue_children |
| 3 | c=10, O, S, stop max_cases, no verified rule, every case result CLAIMED covered / empty / finite / declared_terminal (OUTCOME per_case, unverified: never closes the item by itself) | no_rule | nothing | confirm_coverage_else_next_rung |
| 24 | c=10, O, S, stop max_cases, no verified rule, another case result or no per-case list | no_rule | nothing | next_rung |
| 4 | c=10, O, S, stop lift_refused / guard_timeout / numeric_search_refused / before_numeric | no_rule | committed_rules | next_rung |
| 5 | c=10, O, S, stop open_regions / open_obligations, verified rules >= 1 | rule_found | canonical | queue_open_list |
| 27 | c=10, O, S, stop open_regions / open_obligations, no verified rule | no_rule | nothing | queue_open_list |
| 6 | c=10, O, S, stop not_lower | crashed | nothing | attention |
| 28 | c=10, O, S, kind solve-recipe, stop keys_unsolved, accepted relation records >= 1 | rule_found | solved_records | queue_failures |
| 29 | c=10, O, S, kind solve-recipe, stop keys_unsolved, no accepted relation record | no_rule | nothing | queue_failures |
| 30 | c=0, O, S, kind node-basis (diagnostic kind: sealed records sample / dim / end) | complete | diagnostic_records | item_done |
| 31 | c=0, O, S, kind solve-recipe run with --probe-d (probe records only), every key answered (EXISTS_AT_D0 / EXISTS_ONLY_AT_D0 / NO_RELATION_AT_D0 / D0_SINGULAR_ROWS) | complete | diagnostic_records | item_done |
| 32 | c=10, O, S, kind solve-recipe run with --probe-d, stop keys_unsolved, >= 1 unanswered key (UNRESOLVED_PRIMES: other primes, then attention; BUDGET_WALL; ERROR) | undecided | diagnostic_records | queue_failures |
| 33 | c=0, O, S, kind symmetry-composite (legacy-wrapped; the child's own seal checked by the wrapper), every key has a shipped relation (candidates) | complete | supplied_relations | item_done |
| 34 | c=10, O, S, kind symmetry-composite, stop keys_unsolved, shipped relations >= 1 and >= 1 key none / over_cap / error (typed per-key outcomes) | rule_found | supplied_relations | queue_failures |
| 35 | c=10, O, S, kind symmetry-composite, stop keys_unsolved, no shipped relation | no_rule | nothing | queue_failures |
| 37 | c=0, O, S, kind pole-regular-at (diagnostic, legacy-wrapped: one answer per request of the pinned requests input, each checked against its request; pole orders, never evidence) | complete | diagnostic_records | item_done |
| 38 | kind pole-regular-at, c in {0, 10, 20} with O but the answers not sealed / not verified against the requests (the wrapper refused them), or c in {10, 20}: nothing is read, the relations stay undecided (evaluated before 22 for this kind) | undecided | nothing | diagnostics_undecided |
| 40 | c=0, O, S, kind proof-symrows (legacy-wrapped rr_proof --symrows: one certificate per requested key, each bound to the pinned request: relation digest, record digests, key, symrows digest, group, elements, catalogue records, kernel sha256) | complete | certificates | item_done |
| 43 | kind proof-symrows, c in {0, 10, 20} with O but the certificate not sealed / not bound to the pinned request (another relation, record, key, symrows input or kernel: a forged or misattributed certificate), or c in {10, 20} (evaluated before 22 for this kind) | crashed | nothing | attention |
| 7 | c=11, O, checker kind | decided_negative | nothing | record_negative_verdict |
| 7b | c=12, O, checker kind | undecided | nothing | other_tool_or_attention |
| 8 | c=20, O, S (producing kind) | spill | committed_rules | next_rung_after_spill |
| 25 | c in {0, 10, 20}, O, S, the spec allowed elision and the verifier found elided witness loci | sealed_with_elided | nothing | rerun_without_elision |
| 26 | c in {0, 10, 20}, O, the kind has no sealing rule (generate-auto) | unverifiable_kind | nothing | unverifiable_kind |
| 9 | c=21, O | killed_time | nothing | ladder_decides |
| 10 | c=22, O | killed_memory | nothing | next_memory_class_or_rung |
| 11b | c=30, O, K=time or unresponsive (the supervisor stopped the child for time or silence) | killed_time | nothing | ladder_decides |
| 11c | c=30, O, K=memory (the supervisor stopped the child for memory) | killed_memory | nothing | next_memory_class_or_rung |
| 11 | c=30, O, K=none or stop | interrupted | nothing | resume_on_campaign_resume |
| 12 | c=40 | usage_error | nothing | stop_launching_kind |
| 13b | c in {41, 70}, O whose error names a reader error (UNREADABLE_RECORD, UNKNOWN_RECORD_KIND), kind proof-instantiate / proof-instance-span | refused | nothing | table_unreadable_by_tool |
| 36 | c=41, O whose error names PINNED_INPUTS_MISMATCH, kind symmetry-composite (the child's sealed result is bound to other inputs: request digest, input pins, tool digest) | refused | nothing | item_attention_no_retry |
| 39 | c=41, kind pole-regular-at (the pinned requests input unreadable or not the spec's: POLE_INPUT_MISMATCH): nothing is read, the relations stay undecided | refused | nothing | diagnostics_undecided |
| 41 | c=41, kind proof-symrows, O whose error names SYMROW_REFUSED (the kernel refused a symmetry row of the request: a decided answer, never evidence) | refused | nothing | certificate_refused |
| 42 | c in {41, 70}, kind proof-symrows, O whose error names a reader error (UNREADABLE_RECORD, UNKNOWN_RECORD_KIND) | refused | nothing | table_unreadable_by_tool |
| 13 | c=41 | refused | nothing | item_attention_no_retry |
| 14 | c=42 | refused | nothing | attention |
| 15 | c=50 | lost | nothing | pause_all_launches |
| 15b | c in {51, 70}, O whose error names a host resource errno (EAGAIN, EMFILE, ENFILE, ENOMEM, ENOSPC, EDQUOT) | lost | nothing | retry_after_resource_backoff |
| 16 | c=51 | crashed | nothing | retry_once_then_attention |
| 17 | c=70, O | crashed | nothing | retry_same_spec_once_then_attention |
| 18 | s=9, K=memory (or cgroup oom_kill count increased) | killed_memory | nothing | next_memory_class_or_rung |
| 19 | s in {9, 15, 2, 1}, K=time or unresponsive | killed_time | nothing | ladder_decides |
| 20 | s in {9, 15, 2, 1}, K=stop | interrupted | nothing | resume_on_campaign_resume |
| 21 | s=9, K=none, no OUTCOME | lost | nothing | resume_once_then_attention |
| 21b | W unknown (supervisor restart), no OUTCOME, R valid | lost | nothing | resume_once_then_attention |
| 21c | W unknown, no OUTCOME, R invalid | lost | nothing | restart_from_scratch |
| 22 | contract violation: OUTCOME unreadable, inconsistent (`check`), of another attempt or spec (job_id, attempt, kind, spec_blake3), exit_code != c, a path not the layout path, family_blake3 not the spec's, elided loci under "forbidden"; or c in {0, 10, 20} with O and an output not sealed / not verified; or c outside the kind's set (checker 10 / 20, producing kind 11 / 12, solve-recipe 20 or an exit-10 stop other than keys_unsolved, keys_unsolved of another kind, proof-instantiate / proof-instance-span / coefficient-facts / node-basis 10 / 20); or solve-recipe exit 0 with an unsolved record, keys_unsolved without one (probe job: an unanswered record); or symmetry-composite exit 20, an exit-10 stop other than keys_unsolved, exit 0 with a key without relation, keys_unsolved without one; or W unknown with an invalid OUTCOME | crashed | nothing | attention |
| 23 | any other c or s (an exit status 128+N is read as signal N) | crashed | nothing | resume_once_if_valid_else_attention |
