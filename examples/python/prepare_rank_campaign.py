#!/usr/bin/env python3
"""Stage saved rules for a fresh input-rank-scoped campaign; never run native code.

All saved owners, routes and rule overlays are reused. Required queries are
retained by exact ID; auxiliary starts are omitted unless --include-auxiliary
is explicit. Only max_numerator_rank changes, to min(original cap, requested
cap). Coordinate and positive-power bounds are untouched, including empty
intersections and unbounded auxiliary positive powers. Descendants are NOT
rank-clipped. Later rank stages must be derived from the original campaign,
not by widening a previous rank-scoped stage or reusing its checkpoint.

This writes only a fresh destination's inputs. Use the ordinary production
launcher separately to freeze a tested executable and resource policy.
"""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile


def module(name):
    spec = importlib.util.spec_from_file_location(
        name, Path(__file__).with_name(name + ".py"))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


STAGE = module("stage_saved_owner_campaign")
PRODUCTION = module("production_saved_owner_campaign")
RECEIPT_NAME = "rank-scope-receipt.json"
SOURCE_QUERIES_NAME = "rank-source-queries.json"
WORKSPACE = Path(__file__).resolve().parents[2]
WORKSPACE_TEMP = WORKSPACE / "TMP"


def plan_rank_queries(query_bytes, max_numerator_rank, *, include_auxiliary=False):
    """Intersect starting queries with an input rank cap, not an algebra rule."""
    STAGE.unsigned(max_numerator_rank, 32, "input rank")
    if type(include_auxiliary) is not bool:
        raise ValueError("include_auxiliary must be a Boolean")
    document = STAGE.ROLES.loads_document(query_bytes)
    if (not isinstance(document, dict)
            or document.get("schema") != PRODUCTION.QUERY_SCHEMA
            or not isinstance(document.get("queries"), list)
            or not document["queries"]):
        raise ValueError("rank planning requires a nonempty owner-domain query document")
    originals = document["queries"]
    for index, row in enumerate(originals):
        if not isinstance(row, dict) or set(row) != PRODUCTION.QUERY_ROW_FIELDS:
            raise ValueError(f"query row {index} has unsupported or missing fields")
        if (not isinstance(row["id"], str)
                or not 1 <= len(row["id"].encode("utf-8")) <= 128):
            raise ValueError("query IDs must contain 1..128 UTF-8 bytes")
        owner = row["owner"]
        if not isinstance(owner, str) or not owner or set(owner) - {"0", "1"}:
            raise ValueError("query owner must be a binary mask")
        if any(not isinstance(row[name], list) or len(row[name]) != len(owner)
               for name in ("lower", "upper")):
            raise ValueError("query coordinate bounds must match the owner arity")
        if not isinstance(row["power_bounds"], dict):
            raise ValueError("query power_bounds must be an object")
        if row["max_numerator_rank"] is not None:
            STAGE.unsigned(row["max_numerator_rank"], 32, "original input rank")
    roles = STAGE.ROLES.query_roles(document, require_explicit=True)
    retained = [row for row in originals
                if roles[row["id"]] == "required" or include_auxiliary]
    if not retained:
        raise ValueError("rank-scoped campaign must retain at least one starting query")
    removed = [row["id"] for row in originals if roles[row["id"]] == "auxiliary"
               and not include_auxiliary]
    changed = []
    for row in retained:
        previous = row["max_numerator_rank"]
        cap = max_numerator_rank if previous is None else min(previous, max_numerator_rank)
        if previous != cap:
            changed.append({"id": row["id"], "from": previous, "to": cap})
        row["max_numerator_rank"] = cap
    document["queries"] = retained
    document["query_roles"]["auxiliary"] = (
        document["query_roles"]["auxiliary"] if include_auxiliary else [])
    scoped = (json.dumps(document, indent=2, allow_nan=False) + "\n").encode()
    receipt = {
        "schema": "rustred.rank-scoped-starting-queries.v1",
        "max_numerator_rank": max_numerator_rank,
        "include_auxiliary": include_auxiliary,
        "source_queries_sha256": hashlib.sha256(query_bytes).hexdigest(),
        "queries_sha256": hashlib.sha256(scoped).hexdigest(),
        "source_query_count": len(originals),
        "query_count": len(retained),
        "required_query_count": len(document["query_roles"]["required"]),
        "auxiliary_query_count": len(document["query_roles"]["auxiliary"]),
        "retained_query_ids": [row["id"] for row in retained],
        "removed_auxiliary_query_ids": removed,
        "changed_rank_caps": changed,
        "query_order_preserved": True,
        "coordinate_bounds_unchanged": True,
        "positive_power_and_difference_bounds_unchanged": True,
        "empty_intersections_retained": True,
        "descendant_clipping": False,
        "rule_generation_performed": False,
        "native_coverage_checked": False,
        "family_closure_claim": False,
        "scope": "starting-query intersection only; native admission and closure remain authoritative",
    }
    return scoped, receipt


def prepare(source_campaign, destination, max_numerator_rank, *, include_auxiliary=False):
    """Reuse the audited saved-input stager; do not freeze or invoke a binary."""
    source_campaign = Path(source_campaign).resolve(strict=True)
    destination = Path(destination)
    inputs = source_campaign / "inputs"
    _, _, original = PRODUCTION.verify_inputs(inputs)
    if any(row["name"] in {RECEIPT_NAME, SOURCE_QUERIES_NAME}
           for row in original.get("attachments", [])):
        raise ValueError("derive each rank stage from the original campaign, not a rank-scoped stage")
    source_bytes = (inputs / "queries.json").read_bytes()
    scoped, receipt = plan_rank_queries(
        source_bytes, max_numerator_rank, include_auxiliary=include_auxiliary)
    if receipt["source_queries_sha256"] != original["queries_sha256"]:
        raise ValueError("source queries changed during rank planning")
    receipt.update({
        "source_campaign": str(source_campaign),
        "source_selection_sha256": original["selection_sha256"],
        "owner_count": original["owner_count"],
        "all_saved_owners_routes_and_rules_retained": True,
    })
    attachments = [inputs / row["path"] for row in original.get("attachments", [])]
    # Ignore process-global TMPDIR: this preparer's temporary files must remain
    # in the checkout, even when the caller's environment points at /tmp.
    if WORKSPACE_TEMP.resolve().parent != WORKSPACE:
        raise ValueError("workspace TMP must not resolve outside the checkout")
    WORKSPACE_TEMP.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="rustred-rank-scope-", dir=WORKSPACE_TEMP) as temporary:
        directory = Path(temporary)
        queries = directory / "queries.json"
        queries.write_bytes(scoped)
        original_queries = directory / SOURCE_QUERIES_NAME
        original_queries.write_bytes(source_bytes)
        scope_receipt = directory / RECEIPT_NAME
        scope_receipt.write_text(json.dumps(receipt, indent=2, allow_nan=False) + "\n")
        PRODUCTION.prepare_from(
            source_campaign, destination, "preserve", queries_override=queries,
            attachments=[*attachments, original_queries, scope_receipt])
    if PRODUCTION.verify_inputs(inputs)[2] != original:
        raise ValueError("source input identity changed during rank preparation")
    return receipt


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--source-campaign", type=Path, required=True,
                        help="original saved-owner campaign; read-only source")
    parser.add_argument("--destination", type=Path, required=True,
                        help="nonexistent, disjoint campaign directory; inputs only")
    parser.add_argument("--max-numerator-rank", required=True,
                        type=STAGE.cli_unsigned(32, "input rank"))
    parser.add_argument("--include-auxiliary", action="store_true",
                        help="also intersect and retain auxiliary starting rows; default required-only")
    args = parser.parse_args(argv)
    try:
        receipt = prepare(args.source_campaign, args.destination, args.max_numerator_rank,
                          include_auxiliary=args.include_auxiliary)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))
    print(json.dumps({"status": "inputs-prepared-no-native-work", **receipt}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
