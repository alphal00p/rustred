#!/usr/bin/env bash
# Lane allnodes: one table -> one JSON line {table, sha256, bytes, kinds{residual, composite_terminal_ledger, composite_assembly_complete}}
# (read-only on the table; C tools only). Usage: scan_table.sh <table path>
set -u
t=$1
sha=$(sha256sum -- "$t" | cut -d' ' -f1)
bytes=$(stat -c %s -- "$t")
kinds=$(LC_ALL=C grep -a -o -E '^\{ ?"type": ?"(residual|composite_terminal_ledger|composite_assembly_complete|end|header)"' -- "$t" \
  | sed -E 's/.*"([a-z_]+)"$/\1/' | sort | uniq -c | awk 'BEGIN{printf "{"} {printf "%s\"%s\": %d", (NR>1?", ":""), $2, $1} END{printf "}"}')
printf '{"table": "%s", "sha256": "%s", "bytes": %s, "kinds": %s}\n' "$t" "$sha" "$bytes" "$kinds"
