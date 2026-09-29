#!/usr/bin/env bash
# epoch-s2 final build + suites at HEAD (build-3.lock, CPUs 0-15,256-271, license required).
W=/common/dev/rustred/.claude/worktrees/fable51-epoch
S=$W/TMP/epoch-s2
echo "$(date -u +%FT%TZ) head=$(git -C $W rev-parse HEAD)" > $S/final.log
nix develop --command cargo fmt --all -- --check > $S/final-fmt.log 2>&1; echo "$(date -u +%FT%TZ) fmt rc=$?" >> $S/final.log
$S/cargo.sh final-bin build --release --locked --offline -p rustred-app > /dev/null 2>&1; echo "$(date -u +%FT%TZ) bin: $(tail -1 $S/final-bin.log)" >> $S/final.log
SHA=$(sha256sum $W/target/release/rustred | cut -c1-64); cp $W/target/release/rustred /common/dev/rustred/TMP/epoch-s2/bin/rustred-${SHA:0:8}; echo "$(date -u +%FT%TZ) binary rustred-${SHA:0:8} sha256=$SHA" >> $S/final.log
$S/cargo.sh final-lib test --release --locked --offline -p rustred-app --lib > /dev/null 2>&1; echo "$(date -u +%FT%TZ) lib: $(grep 'test result' $S/final-lib.log | tail -1) $(tail -1 $S/final-lib.log)" >> $S/final.log
$S/cargo.sh final-cli test --release --locked --offline -p rustred-app --test cli_routed_campaign > /dev/null 2>&1; echo "$(date -u +%FT%TZ) cli_routed_campaign: $(grep 'test result' $S/final-cli.log | tail -1) $(tail -1 $S/final-cli.log)" >> $S/final.log
cd $W && nix develop --command python -m unittest discover -s examples/python -p 'test_*.py' > $S/final-python.log 2>&1; echo "$(date -u +%FT%TZ) python rc=$? $(tail -3 $S/final-python.log | tr '\n' ' ')" >> $S/final.log
echo "$(date -u +%FT%TZ) done" >> $S/final.log
