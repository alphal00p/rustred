#!/usr/bin/env bash
# Print a one-line summary of the last heartbeat of each given run directory.
for d in "$@"; do
  f="$d/events.jsonl"
  [ -f "$f" ] || { echo "$d: no events"; continue; }
  tail -c 20000 "$f" | grep '"event":"heartbeat"' | tail -1 | \
    sed -E 's/.*"currently_discovered_nodes":([0-9]+).*"elapsed_seconds":([0-9.]+).*"expanded_nodes":([0-9]+).*"process_rss_bytes":([0-9]+).*/elapsed=\2 expanded=\3 discovered=\1 rss=\4/' | \
    sed "s#^#$(basename $(dirname $d))/$(basename $d): #"
done
