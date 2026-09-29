# W2 stage S2 lane scripts (epoch lockstep skeleton)

Copies of the scripts used for the S2 gates (`docs/research/fable51_w2_s2_2026-09-28.md` §3, §8). They were
run from `TMP/epoch-s2/` (main tree) and the lane worktree; paths inside are absolute to this host.

- `cargo.sh`: one cargo command under `TMP/locks/build-3.lock`, CPUs 0-15,256-271, `RUSTRED_TESTS_REQUIRE_LICENSE=1`.
- `final_gates.sh`: fmt, release build, lib suite, `cli_routed_campaign`, Python suite at HEAD.
- `run_control.py`: the shared `TMP/fable51-controls/run_control.py` with the output root `TMP/epoch-s2/runs`.
- `controls.sh`, `matrix.sh`: epoch controls at one width (optionally followed by the oracle).
- `oracle.sh`: walk-verify-closure (`--require-closure --reinspect all`) + `assert_oracle_pass.py` + paired audit.
- `identity.py`: byte identity of two epoch runs (export sections, records without timing, digests).
- `compare.py`: gate 3 table (natives, domains, edges vs legacy Ready and Ordered).
- `legacy.sh`: legacy reference runs (4a17f9c7 Ready).
- `ordered_identity.sh`: gate 4 (Ordered strict identity against `TMP/fable51-controls/int-ref-g3`).
- `socket1_session.sh`: C-5F W50 and four-all/p5 W96 under `socket1.lock`.
