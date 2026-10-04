#!/usr/bin/env python3
"""Bounded diagnostic sampling of CP6/schema-1 Epoch record sidecars.

This reads explicit preregistered windows, never restores a checkpoint, and
never authenticates a whole sidecar. It is a research codec/projection only;
Rust's native reader remains the authority. See records/{wire,typed}.rs.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from datetime import datetime, timezone
import hashlib
import heapq
import importlib.util
import json
import math
from pathlib import Path
import struct

_sealed_spec = importlib.util.spec_from_file_location("rule_optimizer_profile_sealed", Path(__file__).with_name("profile_sealed.py"))
profile_sealed = importlib.util.module_from_spec(_sealed_spec)
_sealed_spec.loader.exec_module(profile_sealed)

MAGIC = b"ERB1"
MAX_AUTH = 128 << 20
MAX_DIAG = 1 << 30
COUNTERS = ("emitted", "accepted", "distinct_edges", "known_reuse", "job_duplicates")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read_json(path):
    return json.loads(Path(path).read_bytes())


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x") as out:
        json.dump(value, out, indent=2, sort_keys=True)
        out.write("\n")


class Decoder:
    """Strict subset of bincode 2 standard (variable ints, little-endian)."""

    def __init__(self, data):
        self.data, self.at = data, 0

    def take(self, count):
        if count < 0 or self.at + count > len(self.data):
            raise ValueError("truncated bincode body")
        value = self.data[self.at:self.at + count]
        self.at += count
        return value

    def u8(self):
        return self.take(1)[0]

    def boolean(self):
        value = self.u8()
        if value not in (0, 1):
            raise ValueError("invalid boolean")
        return bool(value)

    def uint(self, bits=64):
        tag = self.u8()
        if tag < 251:
            value = tag
        elif tag in (251, 252, 253):
            width = {251: 2, 252: 4, 253: 8}[tag]
            if width * 8 > bits:
                raise ValueError("integer marker exceeds field width")
            value = int.from_bytes(self.take(width), "little")
        else:
            raise ValueError("unsupported integer marker")
        if value >= 1 << bits:
            raise ValueError("integer overflow")
        return value

    def sint(self):
        value = self.uint()
        return (value >> 1) ^ -(value & 1)

    def option(self, read):
        return read() if self.boolean() else None

    def vector(self, read):
        count = self.uint()
        if count > len(self.data) - self.at:
            raise ValueError("impossible vector length")
        return [read() for _ in range(count)]

    def blob(self):
        return self.take(self.uint())

    def powers(self):
        return dict(max_positive_power=self.option(self.uint),
                    min_power_difference=self.option(self.sint),
                    max_power_difference=self.option(self.sint))

    def finish(self):
        if self.at != len(self.data):
            raise ValueError("trailing bincode body bytes")


def decode_authority(data):
    d = Decoder(data)
    record = {"id": d.uint(32), "phase": "Route" if d.boolean() else "Apply"}
    owner = d.vector(d.boolean)
    record["owner"] = "".join("1" if bit else "0" for bit in owner)
    record["lower"] = d.vector(lambda: d.uint(16))
    upper = d.vector(lambda: d.uint(16))
    record["upper"] = [None if x == 65535 else x for x in upper]
    record["rank"] = d.option(lambda: d.uint(32))
    record["power_bounds"] = d.powers()
    record["merge_epoch"] = d.uint()
    if not (0 < len(owner) <= 32 and len(owner) == len(upper) == len(record["lower"])):
        raise ValueError("invalid geometry shape")
    if 65535 in record["lower"] or not record["merge_epoch"]:
        raise ValueError("invalid geometry/epoch")
    variant = d.uint(32)
    if variant == 0:
        record.update(record_kind="delegated_not_inspected", representative_id=d.uint(32),
                      exhausted=d.boolean())
    elif variant == 1:
        record["record_kind"] = "native_inspection"
        native = dict(class_code=d.u8(), kind=d.u8(), v0=d.uint(),
                      distinct_edges=d.uint(32), self_edge=d.boolean(),
                      break_reason=d.u8(), error_kind=d.u8(),
                      err_class=d.option(d.u8), panic=d.boolean(),
                      emitted=d.uint(), accepted=d.uint(), stats_events=d.uint(),
                      has_error=d.boolean(), frontier_count=d.uint(32),
                      job_duplicates=d.uint(), known_reuse=d.uint())
        resolver = dict(version=d.uint(32), successors=d.uint(), conditional=d.uint(),
                        optional=[d.uint() for _ in range(3)], route_masks=d.uint(),
                        route_joint_pruned=d.uint())
        if (native["class_code"] not in (0, 2, 4) or native["kind"] not in range(5)
                or native["break_reason"] not in range(9) or native["error_kind"] not in range(6)
                or resolver["version"] != 1
                or (native["err_class"] is not None) != (native["class_code"] == 2)
                or native["has_error"] != (native["class_code"] == 2)):
            raise ValueError("invalid native metadata")
        if native["kind"] != 4 and (native["kind"] == 2) != (record["phase"] == "Route"):
            raise ValueError("phase/native-kind mismatch")
        scope_kind = d.uint(32)
        if scope_kind == 0:
            scope = {"kind": "whole"}
            if native["kind"] in (1, 3):
                raise ValueError("partial kind without scope")
        elif scope_kind == 1:
            scope = dict(kind="initial", anchor=d.uint(32), cut=d.sint(), residual=d.powers())
            if native["kind"] != 1:
                raise ValueError("initial scope/native-kind mismatch")
            record["record_kind"] = "partial_initial_overlap_inspection"
        elif scope_kind == 2:
            scope = dict(kind="g2", anchor_kind=d.u8(), dispatch=d.uint())
            scope["anchors"] = d.vector(lambda: dict(id=d.uint(32), stamp=d.option(d.uint),
                                                       low_slice=d.boolean()))
            scope["pieces"] = d.vector(lambda: dict(d_lo=d.option(d.sint), d_hi=d.option(d.sint),
                    lower=d.vector(lambda: d.uint(16)), upper=d.vector(lambda: d.uint(16))))
            if native["kind"] != 3 or scope["anchor_kind"] not in (1, 2):
                raise ValueError("g2 scope/native-kind mismatch")
            if len(scope["anchors"]) > 1 << 18 or len(scope["pieces"]) > 1 << 18:
                raise ValueError("g2 inventory limit")
            if any(len(p["lower"]) != len(owner) or len(p["upper"]) != len(owner)
                   for p in scope["pieces"]):
                raise ValueError("g2 geometry shape")
            record["record_kind"] = "g2_residual_inspection"
        else:
            raise ValueError("unknown scope")
        record.update(native=native, resolver=resolver, scope=scope)
    else:
        raise ValueError("unknown authority body variant")
    d.finish()
    return record


def decode_diagnostics(data):
    d = Decoder(data)
    seconds = struct.unpack("<d", d.take(8))[0]
    if not math.isfinite(seconds) or seconds < 0:
        raise ValueError("invalid seconds")
    stats_bytes = d.blob()
    diagnostics = dict(seconds=seconds, stats=json.loads(stats_bytes) if stats_bytes else None,
        error=d.option(lambda: d.blob().decode()),
        frontiers=d.vector(lambda: json.loads(d.blob())),
        refusals=d.vector(lambda: json.loads(d.blob())), refusals_truncated=d.boolean())
    d.finish()
    return diagnostics


def decode_frame(data, offset):
    if len(data) - offset < 12:
        raise EOFError("incomplete frame header")
    magic, auth_size, diag_size = struct.unpack_from("<4sII", data, offset)
    if magic != MAGIC or not (0 < auth_size <= MAX_AUTH and 0 < diag_size <= MAX_DIAG):
        raise ValueError("invalid frame header")
    end = offset + 12 + auth_size + diag_size
    if end > len(data):
        raise EOFError("incomplete frame body")
    record = decode_authority(data[offset + 12:offset + 12 + auth_size])
    diagnostics = decode_diagnostics(data[offset + 12 + auth_size:end])
    if "native" in record:
        native = record["native"]
        if (len(diagnostics["frontiers"]) != native["frontier_count"]
                or (diagnostics["error"] is not None) != native["has_error"]):
            raise ValueError("diagnostic inventory mismatch")
    elif diagnostics != dict(seconds=0.0, stats=None, error=None, frontiers=[],
                              refusals=[], refusals_truncated=False):
        raise ValueError("alias has nondefault diagnostics")
    record.update(diagnostics)
    record["frame_bytes"] = end - offset
    record["frame_sha256"] = digest(data[offset:end])
    return record, end


def synchronize(data):
    """Require three fully decoded consecutive frames before trusting a boundary."""
    candidate = 0
    while True:
        candidate = data.find(MAGIC, candidate)
        if candidate < 0:
            raise ValueError("no validated three-frame chain in window")
        end = candidate
        try:
            for _ in range(3):
                _, end = decode_frame(data, end)
            return candidate
        except (EOFError, ValueError, UnicodeError, struct.error):
            candidate += len(MAGIC)


def inventory(campaign, live_sealed_only=False):
    campaign = Path(campaign).resolve()
    receipt_path = campaign / "inputs/input-receipt.json"
    snapshots = {}
    def metadata(path, role):
        if not live_sealed_only:
            return path.read_bytes()
        snap = profile_sealed.snapshot(path)
        snapshots[role] = snap
        return snap["text"].encode("utf-8")
    receipt_bytes = metadata(receipt_path, "receipt")
    receipt = json.loads(receipt_bytes)
    active = json.loads(metadata(campaign / "active-run.json", "active"))
    run = Path(active["run_directory"])
    status_bytes = metadata(run / "status.json", "status")
    status = json.loads(status_bytes)
    out = dict(campaign=str(campaign), collected_utc=datetime.now(timezone.utc).isoformat(),
               input_receipt_sha256=digest(receipt_bytes), input_receipt=receipt,
               executable_sha256=active.get("executable_sha256"),
               status_sha256=digest(status_bytes), status=status,
               latest_manifest=None, record_segments=[])
    cp = campaign / "checkpoints/main"
    latest = cp / "latest.json"
    if latest.exists():
        raw = metadata(latest, "latest")
        envelope = json.loads(raw)
        manifest = envelope["manifest"]
        if manifest["format"] != "RUSTRED-WALK-CP6" or manifest["schema"] != 3:
            raise ValueError("unsupported checkpoint schema")
        meta_entry = next(f for f in manifest["files"] if f["key"] == "meta")
        meta_bytes = metadata(cp / meta_entry["file"], "meta")
        meta = json.loads(meta_bytes)
        if len(meta_bytes) != meta_entry["bytes"] or meta["record_schema"] != 1:
            raise ValueError("unsupported record schema or meta size mismatch")
        segments_entry = next(f for f in manifest["files"] if f["key"] == "record-segments")
        segments_bytes = metadata(cp / segments_entry["file"], "registry")
        if len(segments_bytes) != segments_entry["bytes"]:
            raise ValueError("record segment inventory size mismatch")
        out.update(latest_manifest=envelope, latest_manifest_sha256=digest(raw),
                   record_schema=meta["record_schema"],
                   record_segments_inventory_sha256=digest(segments_bytes),
                   record_segments= json.loads(segments_bytes))
    result = run / "result.json"
    if result.exists() and not live_sealed_only:
        out["result"] = read_json(result)
    if live_sealed_only:
        out["sealed_snapshots"] = snapshots
        profile_sealed.validate_source(out)
    return out


def make_plan(specs, extra_inventory, live_sealed_only=False):
    if live_sealed_only and (len(specs) != 1 or extra_inventory):
        raise ValueError("live sealed mode requires exactly one source campaign")
    sources, windows = [], []
    for spec in specs:
        directory, generations = spec.rsplit(":", 1)
        source = inventory(directory, live_sealed_only)
        if not live_sealed_only and source["status"]["progress"].get("native_status") != "stopped":
            raise ValueError("sampling requires a stopped campaign")
        source_id = len(sources)
        sources.append(source)
        for gen in map(int, generations.split(",")):
            segment = next(s for s in source["record_segments"] if s["generation"] == gen)
            path = Path(directory).resolve() / "checkpoints/main" / segment["file"]
            st = path.lstat() if live_sealed_only else path.stat()
            binding = profile_sealed.identity(st) if live_sealed_only else None
            if st.st_size != segment["bytes"]:
                raise ValueError("committed segment size mismatch")
            for numerator in (1, 3, 5, 7):
                start = segment["bytes"] * numerator // 8
                count = min(1 << 20, segment["bytes"] - start)
                windows.append(dict(source_id=source_id, generation=gen, path=str(path),
                    start=start, bytes=count, stat_size=st.st_size, stat_mtime_ns=st.st_mtime_ns,
                    segment_blake3_claim=segment["blake3"], committed_records=segment["count"],
                    committed_record_first=segment["first"]))
                if live_sealed_only:
                    windows[-1]["stat_identity"] = binding
    if sum(w["bytes"] for w in windows) > 32 << 20:
        raise ValueError("sample exceeds approved 32MiB read bound")
    plan = dict(schema="rustred-profile-window-plan-v1", created_utc=datetime.now(timezone.utc).isoformat(),
        method="four fixed 1MiB windows starting at 1/8,3/8,5/8,7/8 of each selected sealed segment",
        sources=sources, metadata_only_sources=[inventory(p) for p in extra_inventory], windows=windows,
        total_requested_bytes=sum(w["bytes"] for w in windows), full_checkpoint_authenticated=False,
        sample_bias=["purposeful generation strata; no full-campaign prevalence estimate",
            "fixed byte windows; variable record size and incomplete boundary frames affect selection",
            "publication order/time/stage dependence; nearby records are correlated",
            "inspector seconds omit coordinator cost and are not process CPU or critical-path wall time"],
        panel_policy="Select semantic regions on observed work before candidate outcomes. Reserve owner families by deterministic owner hash; no adjacent-record-only holdout.",
        counter_semantics={"accepted":"accepted native events, including non-admission events; not newly admitted children",
            "distinct_edges":"distinct outgoing graph edges, including edges to existing nodes; not newly admitted children",
            "known_reuse":"earlier emission in the same job already produced its edge (Effect::KnownReuse)",
            "job_duplicates":"same canonical image previously emitted as an Admit in this job",
            "inspector_lookup_stored_hits":"stored snapshot lookup hits; separate aggregate only, not in per-record counters"},
        missing_authority=["selected rule identity", "distinct newly admitted child nodes", "required/helper root ancestry", "recursive closure"])
    if live_sealed_only:
        plan["live_sealed_only"] = True
        validate_plan(plan)
        profile_sealed.verify_source(sources[0])
    return plan


def accumulate(acc, record):
    acc["count"] += 1
    acc["seconds"] += record["seconds"]
    for key in COUNTERS:
        acc[key] += record["native"][key]
    acc["successors"] += record["resolver"]["successors"]
    acc["conditional"] += record["resolver"]["conditional"]
    for key, value in (record["stats"] or {}).items():
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            acc["stats_" + key] += value
    for key, value in (record["stats"] or {}).get("matching", {}).items():
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            acc["matching_" + key] += value


def validate_plan(plan):
    if plan.get("schema") != "rustred-profile-window-plan-v1":
        raise ValueError("unsupported sampling plan")
    if plan.get("live_sealed_only", False) is not False:
        if plan["live_sealed_only"] is not True:
            raise ValueError("invalid live sealed mode")
        profile_sealed.validate_plan(plan)
    ranges, total = defaultdict(list), 0
    for window in plan["windows"]:
        start, count = window["start"], window["bytes"]
        if not isinstance(start, int) or not isinstance(count, int) or start < 0 or not 0 < count <= 1 << 20:
            raise ValueError("invalid sample window bound")
        if start + count > window["stat_size"]:
            raise ValueError("window exceeds committed segment")
        source = plan["sources"][window["source_id"]]
        if source["record_schema"] != 1:
            raise ValueError("unsupported source record schema")
        segment = next(s for s in source["record_segments"] if s["generation"] == window["generation"])
        expected = Path(source["campaign"]) / "checkpoints/main" / segment["file"]
        if (str(expected) != window["path"] or segment["bytes"] != window["stat_size"]
                or segment["blake3"] != window["segment_blake3_claim"]):
            raise ValueError("window differs from committed segment inventory")
        ranges[window["path"]].append((start, start + count))
        total += count
    if total > 32 << 20 or total != plan["total_requested_bytes"]:
        raise ValueError("sample total exceeds bound or disagrees with plan")
    for group in ranges.values():
        ordered = sorted(group)
        if any(a[1] > b[0] for a, b in zip(ordered, ordered[1:])):
            raise ValueError("overlapping sample windows")


def run_sample(plan, plan_bytes):
    validate_plan(plan)
    live_data = profile_sealed.read_windows(plan) if plan.get("live_sealed_only") else None
    phase, owners, strata = (defaultdict(Counter) for _ in range(3))
    kinds, windows, top, representatives = Counter(), [], [], {}
    serial = 0
    for window_id, window in enumerate(plan["windows"]):
        path = Path(window["path"])
        if live_data is not None:
            data = live_data.pop(window_id)
        else:
            before = path.stat()
            if (before.st_size, before.st_mtime_ns) != (window["stat_size"], window["stat_mtime_ns"]):
                raise ValueError("planned immutable segment changed")
            with path.open("rb") as f:
                f.seek(window["start"])
                data = f.read(window["bytes"])
            after = path.stat()
            if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
                raise ValueError("segment changed during bounded read")
        if len(data) != window["bytes"]:
            raise ValueError("short bounded read")
        offset = synchronize(data)
        first = offset
        count, first_id, last_id = 0, None, None
        phase_window = defaultdict(Counter)
        while offset < len(data):
            try:
                record, end = decode_frame(data, offset)
            except EOFError:
                break
            record.update(source_id=window["source_id"], generation=window["generation"],
                          window_id=window_id, frame_offset=window["start"] + offset)
            arity = plan["sources"][window["source_id"]]["latest_manifest"]["manifest"]["arity"]
            if len(record["owner"]) != arity:
                raise ValueError("record/checkpoint arity mismatch")
            first_id = record["id"] if first_id is None else first_id
            last_id = record["id"]
            kinds[record["record_kind"]] += 1
            count += 1
            if "native" in record:
                pb = record["power_bounds"]
                key = (record["phase"], record["owner"], record["rank"],
                       pb["max_positive_power"], pb["min_power_difference"], pb["max_power_difference"])
                source = str(window["source_id"])
                accumulate(phase[(source, record["phase"])], record)
                accumulate(phase_window[record["phase"]], record)
                accumulate(owners[(source, record["phase"], record["owner"])], record)
                accumulate(strata[(source, *key)], record)
                item = (record["seconds"], serial, record)
                serial += 1
                if len(top) < 200:
                    heapq.heappush(top, item)
                elif item[:2] > top[0][:2]:
                    heapq.heapreplace(top, item)
                # One stable representative per phase/owner/bounds stratum.
                rkey = (source, *key)
                if rkey not in representatives or record["frame_sha256"] < representatives[rkey]["frame_sha256"]:
                    representatives[rkey] = record
            offset = end
        windows.append(dict(window_id=window_id, sha256=digest(data), decoded_records=count,
            first_node_id=first_id, last_node_id=last_id, skipped_prefix_bytes=first,
            incomplete_tail_bytes=len(data) - offset, bytes_read=len(data),
            phase_totals={k:dict(v) for k,v in phase_window.items()}))
    def rows(mapping):
        return [dict(key=list(k), **dict(v)) for k, v in sorted(mapping.items(), key=lambda x: -x[1]["seconds"])]
    if live_data is not None:
        profile_sealed.verify_source(plan["sources"][0])
        for window in plan["windows"]:
            if profile_sealed.identity(Path(window["path"]).lstat()) != window["stat_identity"]:
                raise ValueError("sealed segment changed before sample completion")
    return dict(schema="rustred-profile-sample-v1", collected_utc=datetime.now(timezone.utc).isoformat(),
        plan_sha256=digest(plan_bytes), script_sha256=digest(Path(__file__).read_bytes()),
        full_checkpoint_authenticated=False, sample_only=True, kinds=dict(kinds), windows=windows,
        phase=rows(phase), owners=rows(owners), strata=rows(strata),
        slowest_records=[item[2] for item in sorted(top, reverse=True)],
        stratum_representatives=list(representatives.values()))


def make_panel(plan_path, sample_path, policy_path, directory):
    """Export exact diagnostic queries from retained rows; do not rerun records."""
    plan_bytes, sample_bytes, policy_bytes = (Path(p).read_bytes() for p in (plan_path, sample_path, policy_path))
    plan, sample, policy = map(json.loads, (plan_bytes, sample_bytes, policy_bytes))
    if policy["schema"] != "rustred-profile-panel-policy-v1" or policy["sample_plan_sha256"] != digest(plan_bytes):
        raise ValueError("panel policy/plan mismatch")
    if sample["plan_sha256"] != digest(plan_bytes):
        raise ValueError("sample/plan mismatch")
    records = {(r["source_id"], r["id"]): r for r in sample["stratum_representatives"] + sample["slowest_records"]}
    directory = Path(directory)
    cases, query_sets, sources = [], defaultdict(list), {}
    for case in policy["cases"]:
        record = records[case["source_id"], case["id"]]
        source = plan["sources"][case["source_id"]]
        if case["source_id"] not in sources:
            campaign = Path(source["campaign"])
            queries_bytes = (campaign / "inputs/queries.json").read_bytes()
            selection_bytes = (campaign / "inputs/selection.json").read_bytes()
            receipt = source["input_receipt"]
            if digest(queries_bytes) != receipt["queries_sha256"] or digest(selection_bytes) != receipt["selection_sha256"]:
                raise ValueError("source query/selection identity changed")
            sources[case["source_id"]] = json.loads(queries_bytes), json.loads(selection_bytes)
        original, selection = sources[case["source_id"]]
        base = dict(id=case["name"], owner=record["owner"], lower=record["lower"], upper=record["upper"],
                    max_numerator_rank=record["rank"], power_bounds=record["power_bounds"])
        residuals = []
        if record["scope"]["kind"] == "whole":
            native_queries = [base]
        elif record["scope"]["kind"] == "g2":
            for ordinal, piece in enumerate(record["scope"]["pieces"]):
                powers = dict(record["power_bounds"])
                for field, value, combine in (("min_power_difference", piece["d_lo"], max),
                                              ("max_power_difference", piece["d_hi"], min)):
                    if value is not None:
                        powers[field] = value if powers[field] is None else combine(powers[field], value)
                residuals.append(dict(base, id=f"{base['id']}-residual-{ordinal}", lower=piece["lower"],
                    upper=[None if n == 65535 else n for n in piece["upper"]], power_bounds=powers))
            native_queries = residuals
        else:
            raise ValueError("panel exporter does not support this inspection scope")
        owner_index = next((i for i, row in enumerate(selection["owners"]) if row["mask"] == record["owner"]), None)
        context = []
        for query in original["queries"]:
            if query["owner"] == record["owner"]:
                role = next((name for name, ids in original["query_roles"].items() if query["id"] in ids), "unknown")
                context.append(dict(original_role=role, query=query))
        cases.append(dict(case, record=record, root_ancestry="unknown", selected_rule_identity="unknown",
            recursively_closed="unknown", original_record_query_role="unknown",
            source_campaign=source["campaign"], source_selection_sha256=source["input_receipt"]["selection_sha256"],
            source_executable_sha256=source["executable_sha256"], source_owner_index=owner_index,
            source_owner_metadata=None if owner_index is None else selection["owners"][owner_index],
            original_same_owner_queries=context, native_queries=native_queries,
            expected_native_phase=record["phase"], original_parent_query=base))
        query_sets[case["role"]].extend(native_queries)
    paths = {}
    for role, queries in query_sets.items():
        path = directory / f"{role}-queries.json"
        write_json(path, dict(schema="rustred.owner-domain-queries.json.v2", queries=queries))
        paths[role] = str(path.resolve())
    return dict(schema="rustred-profile-panel-v1", created_utc=datetime.now(timezone.utc).isoformat(),
        plan_sha256=digest(plan_bytes), sample_sha256=digest(sample_bytes), policy_sha256=digest(policy_bytes),
        policy=policy, cases=cases, native_query_files=paths, native_consumption_verified=False,
        native_consumption_note="JSON query schema and exact geometry exported; native reinspection still required. Phase is derived by native admission from full owner/route context. Do not treat G2 parent as the measured residual or discard its anchors in a campaign replay.")


def summarize_match(result_path, panel_path, axes):
    result_bytes, panel_bytes = (Path(p).read_bytes() for p in (result_path, panel_path))
    result, panel = map(json.loads, (result_bytes, panel_bytes))
    if result["schema"] != "rustred.owner-domain-match.json.v2":
        raise ValueError("unsupported matching result schema")
    provenance = {case["name"]: case for case in panel["cases"]}
    rows = []
    for query in result["queries"]:
        previous = provenance[query["id"]]["record"]
        if (query["owner"] != previous["owner"] or query["input_lower"] != previous["lower"]
                or query["input_upper"] != previous["upper"]
                or query["requested_max_numerator_rank"] != previous["rank"]
                or query["power_bounds"] != previous["power_bounds"]):
            raise ValueError("matching query differs from frozen panel geometry")
        if any(not 0 <= axis < len(query["owner"]) for axis in axes):
            raise ValueError("axis outside query arity")
        groups = defaultdict(list)
        for index, piece in enumerate(query["pieces"]):
            disposition = piece["disposition"]
            if disposition["kind"] == "selected_rule":
                groups[(disposition["batch"], disposition["rule"])].append((index, piece))
        histogram = [dict(batch=b, rule=r, pieces=len(pieces)) for (b, r), pieces in groups.items()]
        histogram.sort(key=lambda row: (-row["pieces"], row["batch"], row["rule"]))
        examples = []
        for row in histogram[:8]:
            choices = groups[row["batch"], row["rule"]]
            largest_rank = max(choices, key=lambda item: (item[1]["max_numerator_rank"] is None,
                                                         item[1]["max_numerator_rank"] or 0, -item[0]))
            for label, (index, piece) in (("first_in_native_partition", choices[0]),
                                           ("largest_rank_cap_not_attainment_claim", largest_rank)):
                examples.append(dict(selection=label, piece_index_zero_based=index, piece=piece,
                    nominated_axes_zero_based=[dict(axis=axis, owner_bit=query["owner"][axis],
                        lower=piece["lower"][axis], upper=piece["upper"][axis],
                        fixed=piece["upper"][axis] is not None and piece["upper"][axis] == piece["lower"][axis])
                        for axis in axes]))
        rows.append(dict(id=query["id"], owner=query["owner"],
            classification_complete=query["classification_complete"], summary_limit=query["summary_limit"],
            matching_stats=query["stats"], historical_matching_stats=previous["stats"]["matching"],
            all_matching_fields_equal=query["stats"] == previous["stats"]["matching"],
            distinct_selected_rules=len(groups), selected_rule_piece_histogram=histogram, examples=examples))
    return dict(schema="rustred-profile-match-summary-v1", result=str(Path(result_path).resolve()),
        result_sha256=digest(result_bytes), panel_sha256=digest(panel_bytes),
        prepared_seconds=result["prepared_seconds"], matching_seconds=result["matching_seconds"],
        counts=result["counts"], classification_complete=result["classification_complete"], queries=rows,
        attribution_scope="Observed selected batch/rule per exact matching piece, NOT per-rule time, successor work or closure.",
        query_geometry_preserved=True, axes_coordinate_convention="zero-based local nonnegative sector coordinates; inactive physical exponents are their negatives")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    plan = sub.add_parser("plan")
    plan.add_argument("--source", action="append", required=True, help="CAMPAIGN:GEN,GEN,...")
    plan.add_argument("--metadata-only", action="append", default=[])
    plan.add_argument("--live-sealed-only", action="store_true",
                      help="explicit live opt-in: two older committed generations, eight fixed windows, at most 8MiB; nomination only")
    plan.add_argument("--out", required=True)
    sample = sub.add_parser("sample")
    sample.add_argument("--plan", required=True)
    sample.add_argument("--out", required=True)
    panel = sub.add_parser("panel")
    panel.add_argument("--plan", required=True)
    panel.add_argument("--sample", required=True)
    panel.add_argument("--policy", required=True)
    panel.add_argument("--directory", required=True)
    panel.add_argument("--out", required=True)
    matched = sub.add_parser("summarize-match")
    matched.add_argument("--result", required=True)
    matched.add_argument("--panel", required=True)
    matched.add_argument("--axes", default="")
    matched.add_argument("--out", required=True)
    args = parser.parse_args()
    if args.command == "plan":
        result = make_plan(args.source, args.metadata_only, args.live_sealed_only)
    elif args.command == "sample":
        plan_bytes = Path(args.plan).read_bytes()
        result = run_sample(json.loads(plan_bytes), plan_bytes)
    elif args.command == "panel":
        result = make_panel(args.plan, args.sample, args.policy, args.directory)
    else:
        result = summarize_match(args.result, args.panel, [int(x) for x in args.axes.split(",") if x])
    write_json(args.out, result)
    print(json.dumps(dict(out=str(Path(args.out).resolve()), schema=result["schema"])))


if __name__ == "__main__":
    main()
