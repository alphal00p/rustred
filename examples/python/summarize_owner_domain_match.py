#!/usr/bin/env python3
"""Summarize a matching-only owner-domain result into matching-summary.json.

Role: offline read-only reporting; native classification remains the
authority and nothing here implies applicability, closure or reduction.

The input is the `rustred.owner-domain-match.json.v2` document written by
`rustred owner-domain-match` without `--follow-successors` (per-query records
under `queries`, each with `pieces[].disposition.kind`). The file may be large,
so the `queries` array is streamed with a bounded incremental JSON decoder and
never held in memory as a whole.

Output: per-query status, disposition counts (selected_rule, terminal,
exact_zero_sector, exact_gap, unresolved, invalid_source_condition),
unresolved piece counts, and `helper_positive_power_owners`: the owner masks of
helper queries (`owner-anchor-` prefix) that have unresolved pieces or an
incomplete classification. The planner's
`--helper-positive-power-owners-from` option reads that list.
"""
from __future__ import annotations

import argparse
import codecs
import hashlib
import json
from pathlib import Path
import sys

SUMMARY_SCHEMA = "rustred.owner-domain-match-summary.json.v1"
RESULT_SCHEMA = "rustred.owner-domain-match.json.v2"
HELPER_PREFIX = "owner-anchor-"
KINDS = ("selected_rule", "terminal", "exact_zero_sector", "exact_gap", "unresolved", "invalid_source_condition")
CHUNK = 1024 * 1024
MAX_VALUE_BYTES = 256 * 1024 * 1024


class Stream:
    """Bounded-memory reader of one JSON value at a time from a byte file."""

    def __init__(self, path):
        self.file = Path(path).open("rb")
        self.hash = hashlib.sha256()
        self.utf8 = codecs.getincrementaldecoder("utf-8")()
        self.decoder = json.JSONDecoder()
        self.buffer = ""
        self.cursor = 0
        self.eof = False

    def more(self):
        self.buffer = self.buffer[self.cursor:]
        self.cursor = 0
        block = self.file.read(CHUNK)
        self.hash.update(block)
        self.buffer += self.utf8.decode(block, final=not block)
        self.eof = not block
        if len(self.buffer) > MAX_VALUE_BYTES:
            raise ValueError("one JSON value exceeds the bounded streaming buffer")

    def peek(self):
        while True:
            while self.cursor < len(self.buffer) and self.buffer[self.cursor].isspace():
                self.cursor += 1
            if self.cursor < len(self.buffer):
                return self.buffer[self.cursor]
            if self.eof:
                return ""
            self.more()

    def token(self, expected):
        if self.peek() != expected:
            raise ValueError(f"expected {expected!r} at {self.buffer[self.cursor:self.cursor + 40]!r}")
        self.cursor += 1

    def value(self):
        if not self.peek():
            raise ValueError("unexpected end of JSON input")
        while True:
            try:
                result, end = self.decoder.raw_decode(self.buffer, self.cursor)
                if end == len(self.buffer) and not self.eof:
                    self.more()
                    continue
                self.cursor = end
                return result
            except json.JSONDecodeError:
                if self.eof:
                    raise
                self.more()

    def finish(self):
        if self.peek():
            raise ValueError("trailing JSON input")
        self.file.close()
        return self.hash.hexdigest()


def summarize_record(record):
    if not isinstance(record, dict) or not isinstance(record.get("id"), str):
        raise ValueError("each query record needs a string id")
    counts = {kind: 0 for kind in KINDS}
    pieces = record.get("pieces")
    if not isinstance(pieces, list):
        pieces = []
    for piece in pieces:
        kind = piece.get("disposition", {}).get("kind") if isinstance(piece, dict) else None
        if kind not in counts:
            raise ValueError(f"query {record['id']}: unknown disposition kind {kind!r}")
        counts[kind] += 1
    return {"id": record["id"], "owner": record.get("owner"), "helper": record["id"].startswith(HELPER_PREFIX),
            "classification_complete": record.get("classification_complete"), "error": record.get("error"),
            "error_kind": record.get("error_kind"), "summary_limit": record.get("summary_limit"),
            "piece_count": len(pieces), "counts": counts, "unresolved_pieces": counts["unresolved"],
            "gap_pieces": counts["exact_gap"], "invalid_pieces": counts["invalid_source_condition"]}


def summarize(result_path):
    stream = Stream(result_path)
    top = {}
    queries = []
    try:
        stream.token("{")
        while stream.peek() != "}":
            key = stream.value()
            if not isinstance(key, str) or key in top:
                raise ValueError("invalid or duplicate top-level key")
            stream.token(":")
            if key == "queries":
                stream.token("[")
                while stream.peek() != "]":
                    queries.append(summarize_record(stream.value()))
                    if stream.peek() != "]":
                        stream.token(",")
                stream.token("]")
                top[key] = "<streamed>"
            else:
                top[key] = stream.value()
            if stream.peek() != "}":
                stream.token(",")
        stream.token("}")
        digest = stream.finish()
    finally:
        stream.file.close()
    if top.get("schema") != RESULT_SCHEMA:
        raise ValueError(f"result schema must be {RESULT_SCHEMA}")
    ids = [q["id"] for q in queries]
    if len(set(ids)) != len(ids):
        raise ValueError("query ids must be unique")
    totals = {kind: sum(q["counts"][kind] for q in queries) for kind in KINDS}
    helpers = [q for q in queries if q["helper"]]
    flagged = sorted({q["owner"] for q in helpers
                      if q["unresolved_pieces"] or q["classification_complete"] is not True})
    incomplete = sorted(q["id"] for q in queries if q["classification_complete"] is not True)
    return {
        "schema": SUMMARY_SCHEMA,
        "role": "offline read-only summary; native classification remains the authority",
        "result_path": str(result_path), "result_sha256": digest,
        "status": top.get("status"), "classification_complete": top.get("classification_complete"),
        "all_queries_locally_applicable": top.get("all_queries_locally_applicable"),
        "error": top.get("error"), "error_kind": top.get("error_kind"), "error_query_id": top.get("error_query_id"),
        "reported_counts": top.get("counts"), "query_count": top.get("query_count"),
        "completed_queries": top.get("completed_queries"), "processed_queries": top.get("processed_queries"),
        "retained_pieces": top.get("retained_pieces"),
        "streamed_query_count": len(queries), "helper_query_count": len(helpers),
        "piece_counts": totals, "counts_agree_with_report": top.get("counts") == totals,
        "queries_with_unresolved_pieces": sorted(q["id"] for q in queries if q["unresolved_pieces"]),
        "queries_with_gaps": sorted(q["id"] for q in queries if q["gap_pieces"]),
        "queries_with_invalid_conditions": sorted(q["id"] for q in queries if q["invalid_pieces"]),
        "incomplete_queries": incomplete,
        "helper_positive_power_owners": flagged,
        "queries": queries,
        "family_closure_claim": False,
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--result", type=Path, required=True, help="matching-only result.json")
    parser.add_argument("--output", type=Path, required=True, help="matching-summary.json to write")
    parser.add_argument("--force", action="store_true", help="replace an existing output")
    args = parser.parse_args(argv)
    if args.output.exists() and not args.force:
        print(f"refused: {args.output} exists (use --force)", file=sys.stderr)
        return 2
    try:
        summary = summarize(args.result)
    except (OSError, ValueError) as error:
        print(f"refused: {error}", file=sys.stderr)
        return 2
    args.output.write_text(json.dumps(summary, sort_keys=True, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "status": summary["status"],
                      "streamed_query_count": summary["streamed_query_count"],
                      "piece_counts": summary["piece_counts"],
                      "helper_positive_power_owners": summary["helper_positive_power_owners"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
