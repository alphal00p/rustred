"""Terminal and append-only dashboard consumers of campaign telemetry.

No campaign control or algebra lives here. The normalized producer is in
campaign_telemetry.py; this module can be replaced without changing that stream.
"""
from __future__ import annotations

from datetime import datetime, timezone
import importlib.util
import json
import math
import os
from pathlib import Path
import shutil
import sys
import time
import unicodedata

_SPEC = importlib.util.spec_from_file_location("campaign_telemetry", Path(__file__).with_name("campaign_telemetry.py"))
TELEMETRY = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(TELEMETRY)
number = TELEMETRY.number
descendant_closure_summary = TELEMETRY.descendant_closure_summary

def clean(value) -> str:
    return "".join(character for character in str(value) if character.isprintable())


def count(value) -> str:
    return "unknown" if number(value) is None else f"{value:,}" if type(value) is int else f"{value:,.0f}"


def duration(seconds) -> str:
    if number(seconds) is None:
        return "unknown"
    seconds = max(0, int(seconds))
    return f"{seconds // 3600:02d}:{seconds // 60 % 60:02d}:{seconds % 60:02d}"


def percent(value) -> str:
    return "unknown" if number(value) is None else f"{100 * value:.0f}%"


def rank_text(rank):
    if rank.get("status") == "unbounded":
        return "unbounded"
    maximum = number(rank.get("maximum"))
    if rank.get("status") == "finite" and maximum is not None:
        return f"≤{count(maximum)} (bound)"
    qualifier = "finite cap" if rank.get("scope", "").startswith("legacy") else "finite part"
    return "unknown" if maximum is None else f"unknown ({qualifier} {count(maximum)})"


def extended_scope_line(frame, compact=False):
    scope = frame.get("query_scope", {})
    if (number(scope.get("appended_required")) or 0) <= 0:
        return None
    if compact:
        return (f"Required {count(scope.get('required'))} · {count(scope.get('appended_required'))} appended"
                + (" · cumulative closure observed" if scope.get("available") else
                   " · cumulative closure unknown"))
    return (f"Required requests {count(scope.get('required'))} cumulative · "
            f"{count(scope.get('appended_required'))} appended · "
            + ("closed-containing-root observation; independent verification separate"
               if scope.get("available") else
               "cumulative closure unavailable; base roots are not enlarged-scope closure"))


def required_progress(frame):
    """Return the principal bar's honest numerator/denominator and label."""
    scope = frame.get("query_scope", {})
    if number(scope.get("required")) is not None:
        closed = scope.get("required_closed") if scope.get("available") is True else None
        detail = "cumulative requests · recursive"
        if closed is None:
            reason = scope.get("reason")
            if reason == "cumulative required-query closure unavailable in this binary":
                reason = "solver does not emit live query counts"
            detail = "cumulative requests · " + clean(reason or "closure observation unavailable")
        return ("REQUIRED CLOSURE", closed, scope["required"],
                "≥" if closed is not None else "", detail)
    counts = frame["counts"]
    return ("BASE ROOT CLOSURE", counts["initial_closed"], counts["initial_total"],
            "≥" if frame["closure_snapshot"].get("stale") is True else "",
            "base prefix only · required closure unknown")


def required_progress_line(frame):
    label, closed, total, prefix, detail = required_progress(frame)
    return f"{label}: {prefix}{count(closed)} / {count(total)} · {detail}"


def restore_line(frame):
    restore = frame.get("restore_progress", {})
    if not restore.get("stage"):
        return None
    stage = clean(restore["stage"]).replace("_", " ")
    completed, total = restore.get("completed"), restore.get("total")
    counts = (f"{count(completed)} / {count(total)}"
              if number(total) is not None else "in progress")
    return (f"Restore: {stage} · {counts} · worker cap {count(restore.get('worker_limit'))}"
            " · checkpoint work, not new domains")


def scan_observation_lines(frame):
    """Retained completed-scan evidence, separately from the trailing-hour rate."""
    ratio = frame["rates"].get("discovery_per_recursive_closure_scans", {})
    before, after = ratio.get("previous"), ratio.get("latest")
    policy = frame["closure_snapshot"].get("refresh_policy", {})
    state = policy.get("status", "unknown")
    if state == "eligible":
        next_scan = "due; awaiting coordinator safe-point"
    elif state == "throttled":
        next_scan = "eligible in " + duration(policy.get("next_refresh_seconds")) + "; start not guaranteed"
    elif state == "unchanged":
        next_scan = "not needed; graph unchanged"
    elif state == "unavailable":
        next_scan = "unavailable"
    else:
        next_scan = "unknown"
    lines = ["Next closure scan " + next_scan]
    if before is None or after is None:
        lines.insert(0, "Completed-scan D/C unknown · awaiting two completed scans")
        return lines
    def timestamp(value):
        try:
            return datetime.fromtimestamp(value, timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
        except (TypeError, ValueError, OverflowError, OSError):
            return "unknown"
    interval = duration(ratio.get("covered_seconds"))
    lines[:0] = [
        f"Completed-scan D/C {_ratio_text(ratio)} · ΔD {count(ratio.get('discovered_delta'))} / ΔC {count(ratio.get('closed_delta'))} · interval {interval}",
        f"Scans #{count(before.get('refresh_count'))}→#{count(after.get('refresh_count'))} · {timestamp(before.get('completed_unix_seconds'))} → {timestamp(after.get('completed_unix_seconds'))}",
        f"Scan counters D {count(before.get('total_domains'))}→{count(after.get('total_domains'))} · C {count(before.get('total_closed'))}→{count(after.get('total_closed'))} · last finished {duration(ratio.get('age_seconds'))} ago",
    ]
    if ratio.get("state") != "valid":
        lines[0] += " · " + clean(ratio.get("state", "unknown"))
    return lines


def derived_lines(status: dict) -> list[str]:
    """Measured-rate lines from status["derived"]; absent fields stay unknown."""
    derived = status.get("derived")
    derived = derived if isinstance(derived, dict) else {}
    progress = status.get("progress", {})
    progress = progress if isinstance(progress, dict) else {}
    reservations = progress.get("worker_reservations", {})
    reservations = reservations if isinstance(reservations, dict) else {}
    computing = number(derived.get("computing_inspectors_mean_1h"))
    computing_text = "unknown" if computing is None else f"{computing:.1f}"
    rate = number(derived.get("completions_per_hour_1h"))
    rate_text = "unknown" if rate is None else f"{rate:,.0f}"
    growth = number(derived.get("pending_growth_per_completion_1h"))
    growth_text = "unknown" if growth is None else f"{growth:+.2f}"
    rss_per_domain = number(derived.get("rss_bytes_per_discovered_domain"))
    rss_text = "unknown" if rss_per_domain is None else f"{rss_per_domain / 1000:.1f}"
    checkpoint = derived.get("last_checkpoint")
    checkpoint = checkpoint if isinstance(checkpoint, dict) else {}
    current_checkpoint = status.get("checkpoint")
    if isinstance(current_checkpoint, dict) and current_checkpoint.get("state") == "saved":
        checkpoint = {**checkpoint, **current_checkpoint}
    size = number(checkpoint.get("bytes"))
    size_text = "unknown" if size is None else f"{size / 1e9:.2f} GB"
    seconds = number(checkpoint.get("duration_seconds"))
    seconds_text = "unknown" if seconds is None else f"{seconds:.0f} s"
    net = derived.get("discovery_closure_net_1h")
    net = net if isinstance(net, dict) else {}
    frame = TELEMETRY.normalize_status(status)
    ratio = frame["rates"]["discovery_per_recursive_closure_1h"]
    rank_detail = ("encountered rank " + rank_text(frame["rates"]["encountered_numerator_rank"])
                   if isinstance(progress.get("encountered_numerator_rank"), dict) else
                   "max scheduled rank " + count(derived.get("max_scheduled_finite_rank")))
    closure = progress.get("descendant_closure")
    if isinstance(closure, dict) and closure.get("available") is False:
        ratio.update(value=None, infinite=False, state="closure_unavailable")
    closure = closure if isinstance(closure, dict) and closure.get("available") is True else None
    stale = closure.get("snapshot_stale") if closure else net.get("snapshot_stale")
    freshness = "stale" if stale is True else "fresh" if stale is False else "unknown"
    scan = "scan advanced" if net.get("snapshot_advanced") is True else "no new closure scan" if net.get("snapshot_advanced") is False else "scan update unknown"
    age = number(closure.get("snapshot_age_seconds")) if closure else number(net.get("snapshot_age_seconds"))
    # progress_summary/read_status already age the live closure report. Only
    # the fallback sampled endpoint needs the status-file heartbeat age added.
    if closure is None and age is not None:
        age += max(0, number(status.get("heartbeat_age_seconds")) or 0)
    if age is None:
        age = frame["closure_snapshot"]["snapshot_age_seconds"]
    net_line = (f"Discovery/closure {_ratio_text(ratio)} observed scan-batched · window {duration(net.get('covered_seconds'))}"
                f"/{duration(net.get('window_seconds'))}" + (" warm-up" if net.get("warmup") is True else ""))
    if ratio["state"] not in ("valid", "warmup"):
        net_line += f" · {clean(ratio['state'])}"
    return [
        f"Inspectors {computing_text} computing / {count(reservations.get('inspectors'))} reserved"
        f" · stall >=5 s {percent(derived.get('stall_share_5s'))} · coordinator duty {percent(derived.get('coordinator_duty_1h'))}",
        f"Rate {rate_text} per hour · pending {growth_text} per completion"
        f" · {rank_detail} · RSS {rss_text} KB per domain",
        net_line,
        f"Closure snapshot {freshness} · age {duration(age)} · {scan}"
        + (" · heartbeat stale" if status.get("heartbeat_stale") is True else ""),
        *scan_observation_lines(frame),
        f"Checkpoint gen {count(checkpoint.get('generation'))} · {size_text} in {seconds_text}"
        f" · duty {percent(derived.get('checkpoint_duty'))} · roots closed {count(derived.get('roots_closed'))}/{count(derived.get('roots_total'))}",
    ]


def bar(value, total, elapsed=0, width=16) -> str:
    if number(value) is None or number(total) is None or total <= 0 or not 0 <= value <= total:
        marker = int(elapsed or 0) % width
        return "[" + " " * marker + "·" + " " * (width - marker - 1) + "]"
    filled = int(width * value / total)
    return "[" + "━" * filled + "─" * (width - filled) + "]"


def dashboard(status: dict) -> list[str]:
    progress = status.get("progress", {})
    work = progress.get("work", {})
    entry = progress.get("initial_entry_progress", {})
    closure = descendant_closure_summary(progress.get("descendant_closure"))
    resources = status.get("resources", {})
    checkpoint = status.get("checkpoint") or progress.get("checkpoint") or {}
    checkpoint_write = status.get("checkpoint_write", progress.get("checkpoint_write")) or {}
    allocation = progress.get("worker_reservations", {})
    cpu = number(resources.get("native_busy_cores"))
    cpu_text = "warming sample" if cpu is None else f"{cpu:.1f} observed cores"
    rss = number(resources.get("aggregate_rss_bytes"))
    memory_text = "unknown" if rss is None else f"{rss / 1e9:.2f} GB"
    available = number(resources.get("host_available_bytes"))
    host_text = "unknown" if available is None else f"{available / 1e9:.1f} GB"
    rate = number(work.get("recent_local_completions_per_second"))
    rate_text = "unknown" if rate is None else f"{rate:,.1f}/s"
    checkpoint_text = checkpoint.get("state", "not yet reported by Rust")
    if checkpoint.get("generation") is not None:
        checkpoint_text += f" generation {checkpoint['generation']}"
    if checkpoint.get("directory"):
        checkpoint_text += " · " + clean(checkpoint["directory"])
    if number(checkpoint.get("saved_unix_time")) is not None:
        try:
            checkpoint_text += " · completed " + datetime.fromtimestamp(checkpoint["saved_unix_time"], timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
        except (OSError, OverflowError, ValueError):
            checkpoint_text += " · completion timestamp invalid"
    if number(checkpoint.get("duration_seconds")) is not None:
        checkpoint_text += f" in {checkpoint['duration_seconds']:.2f}s"
    if checkpoint.get("bootstrap"):
        checkpoint_text += " · bootstrap; preparation restarts on resume"
    if checkpoint_write.get("state") == "writing":
        started_text = ""
        if number(checkpoint_write.get("started_unix_time")) is not None:
            try:
                started_text = " · started " + datetime.fromtimestamp(checkpoint_write["started_unix_time"], timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
            except (OSError, OverflowError, ValueError):
                started_text = " · start timestamp invalid"
        checkpoint_text = f"WRITING generation {checkpoint_write.get('generation', '?')}{started_text} · {clean(checkpoint_write.get('state_path', checkpoint_write.get('directory', '')))} · last {checkpoint_text}"
    hard = number(status.get("hard_memory_bytes"))
    soft = number(status.get("soft_memory_bytes"))
    hard_text = "unknown" if hard is None else f"{hard / 1e9:.2f} GB"
    soft_text = "unknown" if soft is None else f"{soft / 1e9:.2f} GB"
    closure_bar = bar(closure["initial_closed"], closure["initial_total"], status.get("elapsed_seconds"))
    closure_note = "" if closure["available"] else " · " + clean(closure["reason"])
    closed_prefix = unresolved_prefix = ""
    if closure["available"] and closure["snapshot_stale"]:
        closure_note = f" · conservative snapshot {duration(closure['snapshot_age_seconds'])} ago"
        closed_prefix, unresolved_prefix = "≥", "≤"
    discovered = closure["total_domains"]
    if discovered is None:
        discovered = work.get("scheduled")
    stale = " · STALE HEARTBEAT; current activity unverified" if status.get("heartbeat_stale") else ""
    state = clean(status.get('state', 'starting')).upper()
    if status.get("heartbeat_stale") and state in ("STARTING", "RUNNING", "STOPPING"):
        state = "LAST REPORTED " + state
    reason = status.get("stop_reason") or status.get("native_stop_reason") or progress.get("native_stop_reason")
    if reason:
        state += " · stop " + clean(reason)
    own_swap = number(resources.get("own_swap_growth_bytes_per_second"))
    if own_swap:
        host_text += f" · own swap +{own_swap / 1e6:.1f} MB/s"
    swap_rate = number(resources.get("host_swap_in_bytes_per_second"))
    if swap_rate is not None:
        host_text += f" · host swap-in {swap_rate / 1e6:.1f} MB/s"
    frame = TELEMETRY.normalize_status(status)
    restoring = restore_line(frame)
    label, required_closed, required_total, required_prefix, required_detail = required_progress(frame)
    closure_line = (
        f"{label} {bar(required_closed, required_total, status.get('elapsed_seconds'))} "
        f"{required_prefix}{count(required_closed)} / {count(required_total)} {required_detail}"
        if frame["query_scope"].get("required") is not None else
        f"Closure {closure_bar} {closed_prefix}{count(closure['initial_closed'])} / {count(closure['initial_total'])} initial roots recursively closed{closure_note} · base prefix only; required closure unknown")
    return [
        f"RustRed · {state} · {duration(status.get('elapsed_seconds'))}{stale}",
        *([restoring] if restoring else []),
        f"CPU {bar(cpu, status.get('workers'))} {cpu_text} / {count(status.get('workers'))} total reserved",
        f"Workers {count(progress.get('active_native_slots'))} native active, {count(progress.get('backpressured_native_slots'))} blocked"
        + (f" · {count(progress['finished_native_awaiting_publication'])} finished waiting"
           if progress.get('finished_native_awaiting_publication') is not None else "")
        + f" · reserved {count(allocation.get('inspectors'))} inspect + {count(allocation.get('admission_helpers'))} admission + {count(allocation.get('coordinator'))} coordinator",
        closure_line,
        f"Domains {count(discovered)} discovered · {closed_prefix}{count(closure['total_closed'])} recursively closed · {unresolved_prefix}{count(closure['unresolved_domains'])} unresolved",
        f"Initial {count(entry.get('published'))} / {count(entry.get('total'))} published · initial native inspected {count(entry.get('locally_inspected'))} · not closure",
        f"Queue {count(work.get('pending'))} pending · {count(work.get('locally_completed'))} local completions · {rate_text} local · frontiers {count(work.get('frontiers'))}",
        f"Descendants {count(work.get('pending_descendants'))} pending · discovered dependency coverage only; not termination/family proof · closure ETA unknown",
        *derived_lines(status),
        f"Memory {memory_text} / {hard_text} ceiling · save+stop at {soft_text} · host available {host_text}",
        f"Checkpoint {clean(checkpoint_text)}",
        f"Phase {clean(progress.get('phase', 'starting'))} · update age {duration(progress.get('progress_age_seconds'))} · heartbeat age {duration(status.get('heartbeat_age_seconds'))} · closure ETA unknown",
        f"Receipts {clean(status.get('run_directory', ''))}",
    ]


def cell_width(text):
    """Terminal columns, excluding combining marks (all text is sanitized first)."""
    return sum(0 if unicodedata.combining(c) else 2 if unicodedata.east_asian_width(c) in ("W", "F") else 1
               for c in text)


def fit(text, width):
    text = clean(text)
    if cell_width(text) <= width:
        return text + " " * (width - cell_width(text))
    clipped, used = [], 0
    for character in text:
        size = cell_width(character)
        if used + size > width - 1:
            break
        clipped.append(character)
        used += size
    return "".join(clipped) + "…" + " " * max(0, width - used - 1)


def _paint(text, color, enabled):
    return f"\x1b[{color}m{text}\x1b[0m" if enabled and color else text


def _rate(value):
    return "unknown" if number(value) is None else f"{value:,.3f}/s"


def _ratio_text(ratio):
    if ratio["infinite"]:
        return "∞"
    value = number(ratio["value"])
    return "unknown" if value is None else f"{value:.3f}"


def _closure_ratio(frame):
    completed = frame["rates"].get("discovery_per_recursive_closure_scans", {})
    if completed.get("previous") is not None or completed.get("latest") is not None:
        return completed
    # Recompute from raw paired deltas, including for saved frames that still
    # contain the old (D-C)/(D+C) field. Never reinterpret that stored value.
    ratio = TELEMETRY.discovery_per_recursive_closure_1h(
        frame["rates"]["discovery_minus_closure"], frame["closure_snapshot"])
    if frame["closure_snapshot"].get("available") is not True:
        ratio.update(value=None, infinite=False, state="closure_unavailable")
    return ratio


def _closure_rate(frame):
    # Recheck saved frames too: older producers stored a zero rate while a
    # dirty closure snapshot predated the entire observation window.
    rate = dict(frame["rates"]["recursive_closure"])
    state = TELEMETRY.discovery_per_recursive_closure_1h(
        frame["rates"]["discovery_minus_closure"], frame["closure_snapshot"])["state"]
    if state in ("awaiting_closure_scan", "closure_unavailable"):
        rate.update(per_second=None, state=state)
    return rate


def indicator_color(value, kind):
    """Exact requested thresholds; unknown observations have no alarm colour."""
    value = number(value)
    if value is None:
        return None
    if kind == "cpu":
        return "31" if value < .5 else "33" if value < .75 else "32"
    return "31" if value > 2 else "33" if value > 1 else "32"


def render_table(frame, width=100, height=24, color=True):
    """Pure, size-bounded terminal view of one normalized frame.

    It never updates rates, estimates convergence, reads files, or controls a
    worker. Narrow/short terminals retain alarms and the key rates; the JSON
    stream always contains the unabridged measurements.
    """
    if frame.get("master_reduction"):
        return render_master_table(frame, width, height, color)
    width = max(20, min(160, width))
    height = max(8, height)
    counts, resource, rates = frame["counts"], frame["resources"], frame["rates"]
    closure, completion = _closure_rate(frame), rates["local_completion"]
    snap, gap = frame["closure_snapshot"], rates["discovery_minus_closure"]
    checkpoint, writing = frame["checkpoint"], frame["checkpoint_write"]
    inner = width - 4
    # Keep the two scientifically distinct rate labels whole even at 80
    # columns; numeric cells do not need 25 columns on a narrow terminal.
    label_width = 19 if width >= 74 else min(19, max(10, inner // 5))
    value_width = 20 if 74 <= width < 100 else min(25, max(8, inner // 3))
    detail_width = inner - label_width - value_width - 6
    table = detail_width >= 12
    border = lambda left, right: _paint(left + "─" * (width - 2) + right, "2;36", color)
    full = lambda text, tint=None: "│ " + _paint(fit(text, inner), tint, color) + " │"

    def row(label, value, detail="", tint=None):
        if not table:
            return full(f"{label}: {value}" + (f" · {detail}" if detail else ""), tint)
        return ("│ " + fit(label, label_width) + " │ " + _paint(fit(value, value_width), tint, color)
                + " │ " + fit(detail, detail_width) + " │")

    def gb(value):
        return "?" if number(value) is None else f"{value / 1e9:,.2f}"

    def window(value):
        return (f"window {duration(value.get('covered_seconds'))}/{duration(value.get('window_seconds'))}"
                + (" warm-up" if value.get("warmup") is True else ""))

    def compact_window(value):
        def short(seconds):
            if number(seconds) is None:
                return "?"
            seconds = max(0, int(seconds))
            if seconds >= 3600:
                return f"{seconds // 3600}h" + (f"{seconds // 60 % 60:02d}m" if seconds // 60 % 60 else "")
            if seconds >= 60:
                return f"{seconds // 60}m{seconds % 60:02d}s"
            return f"{seconds}s"
        return (f"{short(value.get('covered_seconds'))}/{short(value.get('window_seconds'))}"
                + (" warm-up" if value.get("warmup") is True else ""))

    def rate_window(value, scan_batched=False):
        if value.get("state") == "awaiting_closure_scan":
            return "awaiting closure scan"
        prefix = "scan-batched " if scan_batched else "window "
        detailed = ("observed scan-batched · " if scan_batched else "") + window(value)
        return detailed if cell_width(detailed) <= detail_width else prefix + compact_window(value)

    state = clean(frame["state"]).upper()
    stale_heartbeat = frame["heartbeat_stale"]
    alarm = "STALE HEARTBEAT — current activity unverified" if stale_heartbeat else frame["stop_reason"]
    title = f"RustRed  /  {state}  /  {duration(frame['elapsed_seconds'])}  /  {frame['phase']}"
    conservative = "≥" if snap["stale"] is True else ""
    unresolved_bound = "≤" if snap["stale"] is True else ""
    root_label, root_closed, root_total, root_prefix, root_detail = required_progress(frame)
    root = f"{root_prefix}{count(root_closed)} / {count(root_total)}"
    growth = rates["pending_growth_per_completion_1h"]
    growth_text = "unknown" if growth is None else f"{growth:+.2f}"
    ratio = _closure_ratio(frame)
    ratio_color = "31" if ratio["infinite"] else indicator_color(ratio["value"], "closure")
    freshness = "stale" if snap["stale"] is True else "fresh" if snap["stale"] is False else "unknown"
    scan = "scan advanced" if snap["advanced"] is True else "no new closure scan" if snap["advanced"] is False else "scan update unknown"
    freshness_text = f"Closure snapshot {freshness} · age {duration(snap['snapshot_age_seconds'])} · {scan}"
    if stale_heartbeat:
        freshness_text += " · heartbeat stale"
    gap_state = ratio["state"]
    hour_ratio = TELEMETRY.discovery_per_recursive_closure_1h(gap, snap)
    gap_detail = ((f"Trailing-hour D/C {_ratio_text(hour_ratio)} · " if ratio.get("latest") else "")
                  + window(gap) + (f" · {clean(hour_ratio['state'])}" if hour_ratio["state"] not in (None, "valid", "warmup") else ""))
    published = f"published {count(counts['initial_published'])}/{count(counts['initial_entries'])}; not closure"
    cpu = number(resource["native_busy_cores"])
    reserved = number(resource["workers"])
    utilization = cpu / reserved if cpu is not None and cpu >= 0 and reserved is not None and reserved > 0 else None
    if utilization is None:
        cpu = None
    cpu_text = f"{'?' if cpu is None else f'{cpu:.1f}'} / {count(resource['workers'])} cores"
    workers = (f"active {count(resource['active_native_slots'])} · blocked {count(resource['backpressured_native_slots'])}"
               f" · awaiting {count(resource['finished_native_awaiting_publication'])}")
    reservations = resource["reservations"]
    computing = resource["computing_inspectors_mean_1h"]
    checkpoint_text = f"{checkpoint['state']} generation {count(checkpoint['generation'])}"
    if number(checkpoint["duration_seconds"]) is not None:
        checkpoint_text += f" · {checkpoint['duration_seconds']:.2f}s"
    if writing["state"] == "writing":
        checkpoint_text = f"WRITING generation {count(writing['generation'])} · last {checkpoint_text}"
    # Priorities are for terminal-height adaptation, not a progress heuristic.
    rows = [
        (0, row(root_label, root,
                bar(root_closed, root_total, frame["elapsed_seconds"]) + " " + root_detail, "94")),
        (3, row("Base roots", f"{conservative}{count(counts['initial_closed'])} / {count(counts['initial_total'])}",
                "protected original prefix · not cumulative request closure")),
        (1, row("Domains", count(counts["total_domains"]), f"{conservative}{count(counts['total_closed'])} closed · {unresolved_bound}{count(counts['unresolved_domains'])} unresolved")),
        (2, row("Queue / local", count(counts["pending"]) + " pending", f"{count(counts['locally_completed'])} completions · frontiers {count(counts['frontiers'])}")),
        (2, row("Active CPU cores", cpu_text, "observed · " + workers, indicator_color(utilization, "cpu"))),
        (5, row("Inspectors (1h)", "unknown" if computing is None else f"{computing:.1f} computing",
                f"reserved {count(reservations['inspectors'])} inspect / {count(reservations['admission_helpers'])} admission / {count(reservations['coordinator'])} coordinator")),
        (5, row("Coordinator duty", percent(resource["coordinator_duty_1h"]), "stall >=5 s " + percent(resource["stall_share_5s"]))),
        (3, row("Memory (GB)", f"{gb(resource['aggregate_rss_bytes'])} / {gb(resource['hard_memory_bytes'])}", f"stop {gb(resource['soft_memory_bytes'])} · host free {gb(resource['host_available_bytes'])}", "33")),
        (2, row("Encountered rank", rank_text(rates.get("encountered_numerator_rank", {})),
                "RSS " + ("unknown" if rates["rss_bytes_per_discovered_domain"] is None else f"{rates['rss_bytes_per_discovered_domain'] / 1000:.1f} KB / domain"))),
        (-2, row("Local completions", _rate(completion["per_second"]), rate_window(completion), "36")),
        (-2, row("Recursive closure", _rate(closure["per_second"]), rate_window(closure, scan_batched=True), "35")),
        (0, full(f"pending {growth_text} per completion · local completion ≠ recursive closure", indicator_color(growth, "pending"))),
        (3 if extended_scope_line(frame) and ratio.get("latest") else 0,
         full(f"Discovery/closure {_ratio_text(ratio)} · D/C · " +
                 ("last two completed scans" if ratio.get("latest") else "observed scan-batched"), ratio_color)),
        (3 if ratio.get("latest") else 0, full(gap_detail)),
        *[(0 if index == 0 else 1, full(line, ratio_color if index == 0 and ratio.get("latest") else None))
          for index, line in enumerate(scan_observation_lines(frame))],
        (-2, full(freshness_text, "33" if snap["stale"] is not False else "2")),
        (3, full("Initial " + published)),
        (2, full("Checkpoint " + checkpoint_text, "33" if writing["state"] == "writing" else None)),
        (3, full("Checkpoint path " + (writing["state_path"] if writing["state"] == "writing" else checkpoint["state_path"] or checkpoint["directory"]))),
        (2, row("Checkpoint size", ("unavailable" if checkpoint["bytes"] is None else gb(checkpoint["bytes"]) + " GB"), "duty " + percent(rates["checkpoint_duty"]))),
        (3, full(f"Heartbeat age {duration(frame['heartbeat_age_seconds'])} · update age {duration(frame['progress_age_seconds'])} · closure ETA unknown")),
        (4, full("Receipts " + frame["run_directory"], "2")),
    ]
    if extended_scope_line(frame):
        rows.insert(1, (-1, full(extended_scope_line(frame, compact=width < 100), "33")))
    if restore_line(frame):
        rows.insert(0, (-3, full(restore_line(frame), "1;36")))
    if alarm:
        rows.insert(0, (-3, full(alarm, "1;31")))
    fixed = [border("╭", "╮"), full(title, "1;36"), border("├", "┤")]
    if table and height >= 20:
        fixed += [row("METRIC", "VALUE", "DETAIL / SAMPLED WINDOW", "1"), border("├", "┤")]
    footer = [border("├", "┤"), full("Scoped dependency progress only · not family closure · ETA unknown", "2"), border("╰", "╯")]
    if height < 14:
        footer = [border("╰", "╯")]
    budget = max(0, height - len(fixed) - len(footer))
    keep = set(sorted(range(len(rows)), key=lambda index: (rows[index][0], index))[:budget])
    return fixed + [line for index, (_, line) in enumerate(rows) if index in keep] + footer


def master_execution_phase(execution):
    """Describe a reported native phase, never infer activity from pool size."""
    stage = execution.get("stage", "")
    if not stage:
        return None
    labels = {"provider request": "Provider barrier", "assistance equation": "Exact publication",
              "filtering provider requests": "Provider filter", "mapping columns": "Column mapping",
              "ordered merge": "Ordered merge", "atomic block merge": "Atomic block merge",
              "waiting for native batch": "Execution"}
    if stage in labels:
        return labels[stage]
    parallel_strategies = ("ordered-work-stealing-blocks-v1", "assisted-ordered-work-stealing-blocks-v1",
                           "dynamic-assisted-ordered-work-stealing-barriers-v1")
    if (execution.get("mode") == "parallel"
            or (not execution.get("mode") and execution.get("strategy") in parallel_strategies
                and stage in ("preparing rows", "parallel elimination"))):
        return "Parallel batch"
    if execution.get("mode") == "barrier":
        return "Provider barrier"
    if execution.get("mode") == "serial":
        return "Serial phase"
    return "Execution"


def master_batch_execution(master):
    """Do not present a retained batch receipt as a later phase's activity."""
    if (master.get("operation") != "refine" or master.get("refinement_strategy") == "modular-circuits"
            or master.get("stage") == "terminal_collection"
            or master.get("status") in ("completed_nonminimal", "published_unrefined")):
        return {}
    execution = master.get("execution", {})
    if (master.get("stage") in ("preparing_sector_symmetry_equations", "reducing_sector_symmetry_equations")
            or str(master.get("stage", "")).startswith("selective_symmetry_")
            or master.get("selective_symmetry_feedback", {}).get("stage") in
               ("selecting_sources", "preparing_selected_sources", "prepared")):
        # A previous elimination receipt is not activity in provider preparation.
        # The configured pool remains useful, but supplies no busy-worker count.
        return {key: value for key, value in execution.items()
                if key in ("workers", "reserved_workers")} if execution else {}
    return execution


def sector_symmetry_metrics(master):
    """Bounded finite-provider counters shared by terminal and plain consumers."""
    enabled = master.get("sector_symmetry_assistance", False)
    if not enabled or master.get("operation") != "refine":
        return ([("Sector proofs", "retained", "inherited finite proofs; provider off")]
                if enabled or master.get("inherited_sector_symmetry_authority", False) else [])
    preparation = master.get("sector_symmetry_preparation", {})
    reduction = master.get("sector_symmetry_reduction", {})
    saved_projection_requested = master.get("sector_symmetry_saved_projection")
    if saved_projection_requested is None:
        saved_projection_requested = reduction.get("saved_projection_requested")
    if saved_projection_requested is None:
        saved_projection_requested = True
    policy = master.get("sector_symmetry_policy") or preparation.get("policy") or "generators"
    polynomial_orbits = policy == "polynomial-orbits"
    policy_detail = ("whole polynomial orbit / Reynolds rows" if polynomial_orbits else
                     "I - T_r and I - T_rg per verified generator" if policy == "generators" else
                     "relation recipe not reported")
    row_detail = ("whole polynomial orbit / Reynolds rows" if polynomial_orbits else
                  "generator relation rows" if policy == "generators" else "finite source relation rows")
    column_label = "Orbit columns" if polynomial_orbits else "Symmetry columns"

    def completeness(report, field="complete"):
        return ("complete" if report.get(field) is True else
                "incomplete" if report.get(field) is False else "not reported")

    transported_rows = preparation.get("transported_rows")
    if transported_rows is None:
        transported_rows = preparation.get("equations")
    transported_keys = preparation.get("transported_keys")
    if transported_keys is None:
        transported_keys = preparation.get("equation_targets")
    discovered_maps = preparation.get("discovered_maps")
    verified_maps = preparation.get("verified_generators")
    maps = (count(discovered_maps) + " discovered" if verified_maps is None and discovered_maps is not None
            else count(verified_maps) + " verified")
    supports = preparation.get("analyzed_supports")
    support_detail = (count(supports) + " supports analyzed" if supports is not None else
                      count(preparation.get("support_closure_supports")) + " supports in finite closure"
                      if preparation.get("support_closure_supports") is not None else "unknown supports analyzed")
    map_detail = count(preparation.get("proposed_generators")) + " proposed; " + support_detail
    if verified_maps is not None and discovered_maps is not None:
        map_detail = count(discovered_maps) + " discovered; " + map_detail
    skipped = preparation.get("skipped", [])
    # Reasons are scalar headers, never source equations or support inventories.
    reasons = [clean(item.get("reason", "unknown"))[:96] + " " + count(item.get("count"))
               for item in skipped[:3] if isinstance(item, dict)] if isinstance(skipped, list) else []
    omitted = max(0, number(preparation.get("skipped_reasons_omitted")) or 0)
    if isinstance(skipped, list):
        omitted += max(0, len(skipped) - 3)
    if omitted:
        reasons.append(count(omitted) + " additional reasons omitted")
    boundary = clean(preparation.get("scope") or "finite original sources; endpoint monomials are auxiliary columns")[:240]
    if reasons:
        boundary += "; skipped " + ", ".join(reasons)
    result = [
        ("Source jobs", count(preparation.get("input_keys")),
         (count(preparation["processed_keys"]) + " processed; " if preparation.get("processed_keys") is not None else "")
         + "original source keys; " + completeness(preparation, "source_jobs_complete")),
        ("Sector symmetry", count(transported_rows) + " rows",
         row_detail + "; finite original inventory; not global"),
        ("Sector maps", maps, map_detail),
        ("Transported keys", count(transported_keys),
         "original source keys with symmetry rows"),
        ("Transported terms", count(preparation.get("output_terms")), "terms in transported rows"),
        ("Symmetry boundary", completeness(preparation), boundary),
    ]
    closure_keys = preparation.get("closure_keys")
    if closure_keys is None:
        closure_keys = preparation.get("discovered_keys")
    closure_supports = preparation.get("support_closure_supports")
    if closure_keys is not None or closure_supports is not None:
        result.insert(1, (column_label, count(closure_keys),
                         "nonzero polynomial columns; auxiliary endpoints"
                         + ("; " + count(closure_supports) + " supports" if closure_supports is not None else "")))
    if preparation.get("stage"):
        result.insert(1, ("Source preparation", clean(preparation["stage"]).replace("_", " ")[:80],
                         policy_detail + " per original key"))
    if polynomial_orbits:
        for label, field, detail in (("Orbit images", "orbit_images", "whole polynomial images in the current source orbit"),
                                     ("Orbit terms", "orbit_terms", "terms across images in the current source orbit")):
            if preparation.get(field) is not None:
                result.append((label, count(preparation[field]), detail))
    elif policy == "generators":
        if preparation.get("orbit_images") is not None:
            result.append(("Candidate images", count(preparation["orbit_images"]),
                           "generator candidates for the current original source"))
        if preparation.get("orbit_terms") is not None:
            result.append(("Candidate terms", count(preparation["orbit_terms"]),
                           "terms across generator candidates for the current source"))
    if saved_projection_requested is False:
        result[1:1] = [
            ("Saved projection", "not requested", "finite source-only symmetry cohort"),
            ("Combined closure", completeness(reduction, "combined_closure_complete")
             if type(reduction.get("combined_closure_complete")) is bool else "not assessed",
             clean(reduction.get("combined_closure_scope") or
                   "source-only finite cohort; no saved-program projection or family-closure claim")[:240]),
        ]
        ready = reduction.get("exact_source_rows_ready")
        result.append(("Source cohort", "ready" if ready is True else "incomplete" if ready is False else "not reported",
                       count(reduction.get("processed_rows")) + "/" + count(reduction.get("source_rows"))
                       + " exact source rows; finite cohort"))
        if reduction.get("source_terms") is not None:
            result.append(("Source terms", count(reduction["source_terms"]), "terms in the exact source cohort"))
        if reduction.get("unresolved_output_keys") is not None:
            result.append(("Auxiliary tails", count(reduction["unresolved_output_keys"]),
                           "auxiliary columns at the source-only finite boundary"))
    elif reduction:
        result[1:1] = [
            ("Saved projection", completeness(reduction, "saved_projection_complete"),
             "saved-program projection of original-source symmetry rows"),
            ("Combined closure", completeness(reduction, "combined_closure_complete"),
             clean(reduction.get("combined_closure_scope") or
                   "original sources and saved projection; unrestricted family closure not established")[:240]),
        ]
        result.extend([
            ("Saved-row reduction", count(reduction.get("processed_rows")) + "/" + count(reduction.get("source_rows")),
             count(reduction.get("saved_reduced_rows")) + " saved-reduced rows; "
             + count(reduction.get("saved_application_failures")) + " failed saved applications"),
            ("Projected terms", count(reduction.get("projected_terms")), "terms after saved-row projection"),
            ("Projection boundary", count(reduction.get("incomplete_boundaries")) + " incomplete",
             "row pass " + completeness(reduction) + "; saved projection "
             + completeness(reduction, "saved_projection_complete")),
        ])
        for label, field, detail in (
                ("Outside original", "saved_terminal_keys_outside_source_inventory", "saved terminal columns outside original sources"),
                ("Outside orbit" if polynomial_orbits else "Outside symmetry", "saved_output_keys_outside_symmetry_inventory", "saved output columns outside the symmetry inventory"),
                ("Normalization exits", "normalization_outputs_outside_source_inventory", "normalization outputs outside original sources"),
                ("Deferred sources", "deferred_symmetry_sources", "returned program leaves deferred at the finite boundary"),
                ("Unresolved outputs", "unresolved_output_keys", "unresolved saved-program projection tails")):
            if reduction.get(field) is not None:
                result.append((label, count(reduction[field]), detail))
        active = reduction.get("active_row")
        if isinstance(active, dict):
            result.append(("Active projection", clean(active.get("phase", "not reported"))[:80],
                           "task " + count(active.get("task")) + "; elapsed " + duration(active.get("elapsed_seconds"))
                           + "; nodes " + count(active.get("unique_nodes")) + " unique / "
                           + count(active.get("active_nodes")) + " active; pending frames "
                           + count(active.get("pending_frames")) + "; cached " + count(active.get("cached_entries"))))
    else:
        result.insert(1, ("Saved projection", "requested", "awaiting saved-program projection receipt"))
    normalization = []
    if (preparation.get("normalization_input_keys") is not None
            or preparation.get("normalization_canonical_keys") is not None):
        normalization.append(("Normalization", count(preparation.get("normalization_input_keys")) + "→"
                              + count(preparation.get("normalization_canonical_keys")) + " keys",
                              "canonical preparation outputs; retained basis after joined reduction"))
    if any(preparation.get(field) is not None for field in (
            "normalization_unit_aliases", "normalization_projected_numerators", "normalization_source_equations")):
        normalization.append(("Normalization maps", count(preparation.get("normalization_unit_aliases")) + " aliases",
                              count(preparation.get("normalization_projected_numerators")) + " numerator projections; "
                              + count(preparation.get("normalization_source_equations")) + " exact source equations"))
    if (preparation.get("normalization_output_terms") is not None
            or preparation.get("normalization_safe_zero_certificates") is not None):
        normalization.append(("Normalization terms", count(preparation.get("normalization_output_terms")),
                              count(preparation.get("normalization_safe_zero_certificates"))
                              + " safe zero certificates; finite preparation rows"))
    if preparation.get("legacy_source_equations") is not None:
        normalization.append(("Weaker source rows", count(preparation["legacy_source_equations"]),
                              "finite weaker cohort before combined row deduplication"))
    result[1:1] = normalization
    result.insert(0, ("Symmetry policy", clean(policy)[:80], policy_detail))
    result[0:0] = selective_symmetry_feedback_metrics(master)
    return result


def selective_symmetry_feedback_metrics(master):
    """Selected finite work is not an algebraic family-completeness theorem."""
    if not master.get("selective_symmetry_feedback_enabled", False) or master.get("operation") != "refine":
        return []
    feedback = master.get("selective_symmetry_feedback", {})

    def state(key):
        return "complete" if feedback.get(key) is True else "incomplete" if feedback.get(key) is False else "not reported"

    queue = feedback.get("selected_queue_exhausted")
    result = [
        ("Selective feedback", "round " + count(feedback.get("round")),
         clean(feedback.get("stage") or "awaiting native selected-work receipt")[:160]),
        ("Selected targets", count(feedback.get("completed_targets")) + "/" + count(feedback.get("selected_targets")),
         count(feedback.get("pending_targets")) + " pending; "
         + count(feedback.get("new_target_proposals")) + " new proposals"),
        ("Feedback gains", count(feedback.get("new_relations")) + " rows",
         count(feedback.get("eliminated_targets")) + " eliminated; "
         + count(feedback.get("completed_rounds")) + " committed rounds; "
         + count(feedback.get("stagnant_rounds")) + " no-gain rounds"),
        ("Selected queue", "exhausted" if queue is True else "open" if queue is False else "not reported",
         count(feedback.get("frontier_keys")) + " frontier keys; selected-worklist boundary only; not family closure or minimality"),
        ("Support epoch", count(feedback.get("terminal_rank_epoch")) + " basis keys",
         count(feedback.get("visited_supports")) + " supports visited at this rank; reopen after strict rank decrease"),
        ("Source / IBP work", state("source_jobs_complete") + " / " + state("ordinary_work_complete"),
         count(feedback.get("source_rows")) + " symmetry rows; "
         + count(feedback.get("ordinary_source_rows")) + " ordinary source rows; "
         + count(feedback.get("unresolved_leaves")) + " unresolved leaves"),
    ]
    if feedback.get("budget_exhausted") is True or feedback.get("reason"):
        result.append(("Feedback boundary", "caller budget" if feedback.get("budget_exhausted") is True else "reported",
                       clean(feedback.get("reason") or "selected work incomplete; accepted proofs retained")[:240]))
    return result


def render_master_table(frame, width=100, height=24, color=True):
    """Phase-two view of the same event stream; no inferred algebra progress."""
    width, height = max(20, min(160, width)), max(8, height)
    inner = width - 4
    master, resource, checkpoint = frame["master_reduction"], frame["resources"], frame["checkpoint"]
    partial = master.get("partial_refinement", False)
    importing = master.get("proof_importing", False)
    publishing = master.get("operation") in ("publish", "finalize-refinement", "import-refinement")
    modular = master.get("refinement_strategy") == "modular-circuits" and not publishing
    fresh = master.get("fresh_search", False) and not publishing
    total, completed = master.get("total_work"), master.get("completed_work")
    work = f"{count(completed)} / {count(total)}" if total is not None else count(completed)
    discovery = master.get("modular_refinement", {}) if modular else {}
    discovery_work = count(discovery.get("source_rows_scanned")) + " / " + count(discovery.get("total_source_rows"))
    label_width, value_width = (19, 18) if width >= 74 else (10, 8)
    if not publishing:
        # Counters take space from optional detail, not from their digits.
        # Keep one shared value width so the rest of the table stays aligned.
        available = max(value_width, inner - label_width - 6 - 12)
        value_width = max(value_width, min(cell_width(discovery_work if modular else work), available))
    detail_width = inner - label_width - value_width - 6
    border = lambda left, right: _paint(left + "─" * (width - 2) + right, "2;36", color)
    full = lambda text, tint=None: "│ " + _paint(fit(text, inner), tint, color) + " │"

    def row(label, value, detail="", tint=None, preserve_value=False):
        if preserve_value and (cell_width(value) > value_width or cell_width(label) > label_width):
            # On narrower terminals span the row before abbreviating a counter.
            # If needed, omit label spacing/the label rather than its value.
            prefix = f"{label}: "
            if cell_width(prefix + value) > inner:
                prefix = f"{label}:"
            if cell_width(prefix + value) > inner:
                prefix = ""
            return full(prefix + value, tint)
        if detail_width < 12:
            return full(f"{label}: {value}" + (f" · {detail}" if detail else ""), tint)
        return ("│ " + fit(label, label_width) + " │ " + _paint(fit(value, value_width), tint, color)
                + " │ " + fit(detail, detail_width) + " │")

    def gb(value):
        return "unknown" if number(value) is None else f"{value / 1e9:,.2f}"

    cpu, workers = resource.get("native_busy_cores"), resource.get("workers")
    cpu_ratio = cpu / workers if number(cpu) is not None and number(workers) is not None and workers > 0 else None
    progress = (bar(completed, total, frame["elapsed_seconds"], width=max(1, min(16, detail_width - 2)))
                if total is not None else "total not yet known")
    stage = {"preparing_sector_symmetry_equations": "sector preparation",
             "reducing_sector_symmetry_equations": "sector saved rows"}.get(master["stage"], master["stage"])
    rows = [
        (0, row("Stage", stage or "preparing", "replay symmetry, saved rows and exact union" if importing else "package retained exact maps" if partial else
                "extract and package exact inventory" if publishing else
                "modular discovery; exact accepted maps" if modular else "bounded exact relation search", "36")),
        (1, row("Retained keys", count(master["raw_terminals"]), "phase-one keys, including prior stages")),
        (1, row("Normalized", count(master["normalized_terminals"]), "exact symmetry aliases; not minimal")),
        (0, row("Remaining basis", count(master["remaining_terminals"]), "finite, potentially nonminimal", "32")),
        (1, row("Eliminated", count(master["eliminated_terminals"]), "exact relations between terminal keys", "32")),
        (0, row("Relation work", work, progress, "35", preserve_value=True)),
        (1, row("Exact source rows" if modular else "Rows generated", count(master["relation_rows"]),
                "retained exact proof cursor" if modular else "seed depth " + count(master["seed_depth"]))),
        (1, row("Sparse system", count(master.get("independent_rows")) + " rows", count(master.get("nonzeros")) + " nonzeros")),
        (0, row("Active CPU cores", f"{'unknown' if cpu is None else f'{cpu:.1f}'} / {count(workers)}",
                "observed / reserved", indicator_color(cpu_ratio, "cpu"))),
        (0, row("Memory (GB)", gb(resource.get("aggregate_rss_bytes")) + " / " + gb(resource.get("hard_memory_bytes")),
                "save+stop at " + gb(resource.get("soft_memory_bytes")) +
                " · host available " + gb(resource.get("host_available_bytes")), "33")),
        (0, row("Checkpoint", checkpoint["state"], "generation " + count(checkpoint["generation"]), "33")),
        (1, row("Checkpoint size", master_checkpoint_size(checkpoint["bytes"]), duration(checkpoint["duration_seconds"]) + " write")),
        (2, full("Checkpoint " + (checkpoint["directory"] or "not yet saved"))),
        (2, full("Scope " + master["scope_binding"], "2")),
        (2, full("Receipts " + frame["run_directory"], "2")),
    ]
    execution = master.get("execution", {}) if modular else master_batch_execution(master)
    if execution and not publishing and not modular:
        budget = execution.get("reserved_workers")
        if budget is None:
            budget = execution.get("workers")
        phase_label = master_execution_phase(execution)
        if phase_label:
            rows.insert(6, (1, row(phase_label, execution["stage"],
                f"prepared {count(execution.get('rows_prepared'))}/{count(execution.get('rows_total'))} · "
                f"eliminated {count(execution.get('rows_eliminated'))} · merged {count(execution.get('rows_committed'))} · "
                f"{count(execution.get('blocks_completed'))} blocks", "36")))
            rows.insert(7, (2, row("Worker limit", count(budget), "configured budget; not measured busy cores", preserve_value=True)))
        else:
            rows.insert(6, (2, row("Worker limit", count(budget), "configured; native phase not reported", preserve_value=True)))
        if execution.get("provider_keys_filtered") is not None:
            rows.insert(7, (1, row("Provider filtered", count(execution["provider_keys_filtered"]),
                                  "pending keys outside finite inventory; current call", preserve_value=True)))
    if modular:
        rows[5] = (0, row("Discovery rows", discovery_work, "sampled scan; exact span incomplete", "35",
                          preserve_value=True))
        rows.insert(1, (0, row("Relation assurance", "exact W*A identity", "every physical column verified", "32")))
        rows.insert(2, (0, row("Modular candidates", count(discovery.get("candidates_found")),
                              count(discovery.get("accepted_relations")) + " accepted; "
                              + count(discovery.get("reconstruction_misses")) + " lifting misses", "36")))
        if execution:
            rows.insert(3, (1, row("Candidate workers", count(execution.get("workers")),
                                  "discovery uses " + count(execution.get("discovery_workers")) + " worker")))
    if publishing:
        rows = [entry for index, entry in enumerate(rows) if index not in (4, 5, 6, 7)]
        verified = master.get("independently_verified", False)
        mode = master.get("publication_verification_mode", "")
        rows.insert(1, (0, row("Assurance", "exact replay" if importing else "deep replay passed" if verified else
                              "trusted saved scope" if master.get("publication_assurance") == "trusted_saved_scope" else "pending",
                              "native proof replay; authenticated exact rows" if importing else "inherited saved assurance" if partial else "independent graph replay" if mode == "independent_replay" else "exact inventory; deep replay not requested", "32")))
        rows.insert(2, (0, row("Proof import" if importing else "Refinement",
                              "completed" if importing and master.get("import_complete") else
                              master.get("proof_import", {}).get("stage") or "preparing" if importing else
                              "partial retained" if partial else "not requested",
                              "no new ordinary source search" if importing else "finite search unfinished" if partial else "explicit refine command only", "36")))
        verification_progress = master.get("verification_progress", {})
        if verification_progress:
            deep = mode == "independent_replay"
            event = verification_progress.get("event", "")
            done, total = verification_progress.get("completed"), verification_progress.get("total")
            if done is None:
                done = verification_progress.get("reinspected")
            if done is None:
                done = verification_progress.get("records")
            if total is None:
                total = verification_progress.get("selected")
            label = ("Census progress" if event.startswith("inventory_census") else
                     "Replay progress" if deep else "Inventory checks")
            if done is not None or total is not None:
                rows.insert(3, (0, row(label, count(done) + " / " + count(total), "native work counters")))
    rows.insert(1, (0, row("Published rules" if master.get("artifact") else "Available rules",
                          count(master.get("published_rule_records")),
                          "installed executable rule records", "36")))
    rows.insert(2, (1, row("Observed cover", count(master.get("observed_cover_rules")),
                          "distinct rules encountered in this scope")))
    collection = master.get("collection", {})
    if collection.get("finite_feedback_stage"):
        rows.insert(1, (0, row("Finite feedback", collection["finite_feedback_stage"],
                              "retained finite rows; no new seeds", "36")))
        rows.insert(2, (1, row("Feedback equations", count(collection.get("finite_feedback_equations")),
                              count(collection.get("finite_feedback_rows")) + " retained source rows")))
        rows.insert(3, (1, row("Feedback columns", count(collection.get("finite_feedback_columns")),
                              count(collection.get("finite_feedback_auxiliary_columns")) + " auxiliary; "
                              + count(collection.get("finite_feedback_aliases")) + " full-U aliases")))
        rows.insert(4, (1, row("Feedback nonzeros", count(collection.get("finite_feedback_nonzeros")),
                              count(collection.get("finite_feedback_replay_operations")) + " replay operations")))
    if fresh:
        rows.insert(1, (0, full("Fresh relations-only round · shape ("
            + count(master.get("seed_depth")) + "," + count(master.get("containing_sector_depth"))
            + ") · retained proved maps", "36")))
    numerator = master.get("numerator_symmetry_preparation", {})
    numerator_reduction = master.get("numerator_symmetry_reduction", {})
    if (master.get("numerator_symmetry_assistance", False)
            and not master.get("sector_symmetry_assistance", False) and not publishing):
        rows.insert(2, (0, row("Numerator symmetry", count(numerator.get("equations")) + " equations",
                              "finite certified inventory; not global", "36")))
        rows.insert(3, (1, row("Verified maps", count(numerator.get("verified_generators")),
                              count(numerator.get("proposed_generators")) + " proposed; denominator-preserving")))
        if numerator_reduction:
            rows.insert(4, (1, row("Symmetry feedback", count(numerator_reduction.get("saved_reduced_rows")) + " reduced",
                count(numerator_reduction.get("saved_application_failures")) + " saved-apply failures; native fallback on failure")))
    elif (not master.get("sector_symmetry_assistance", False)
            and (master.get("numerator_symmetry_assistance", False)
                 or master.get("inherited_numerator_symmetry_authority", False))):
        rows.insert(2, (1, row("Numerator proofs", "retained", "inherited finite proofs; provider off")))
    for index, (label, value, detail) in enumerate(sector_symmetry_metrics(master)):
        rows.insert(2 + index, (1, row(label, value, detail, "36" if index == 0 else None)))
    rows.insert(0, (0, row("Elapsed time", duration(frame["elapsed_seconds"]),
                          "wall time in this phase; current invocation", "36")))
    if master["artifact"]:
        rows.insert(0, (0, full("Artifact " + master["artifact"], "32")))
    if frame["stop_reason"]:
        rows.insert(0, (-1, full(frame["stop_reason"], "1;33")))
    if frame["heartbeat_stale"]:
        rows.insert(0, (-2, full("STALE HEARTBEAT — current activity unverified", "1;31")))
    fixed = [border("╭", "╮"),
             full(f"RustRed / {'PROVED RELATION IMPORT' if importing else 'ARTIFACT PUBLICATION' if publishing else 'MASTER REFINEMENT'} / {clean(frame['state']).upper()} / {duration(frame['elapsed_seconds'])}", "1;36"),
             border("├", "┤")]
    if detail_width >= 12 and height >= 20:
        fixed += [row("METRIC", "VALUE", "DETAIL", "1"), border("├", "┤")]
    footer = [border("├", "┤"), full("Native proved-relation replay · no new ordinary search · no minimality claim" if importing else
              "Retained exact maps · finite search unfinished · no minimality claim" if partial else
              "Portable scoped artifact · unrefined terminals · no numerical values" if publishing else
              "Exact bounded search · nonminimal basis allowed · no numerical values", "2"), border("╰", "╯")]
    if height < 14:
        footer = [border("╰", "╯")]
    budget = max(0, height - len(fixed) - len(footer))
    keep = set(sorted(range(len(rows)), key=lambda index: (rows[index][0], index))[:budget])
    return fixed + [line for index, (_, line) in enumerate(rows) if index in keep] + footer


def master_checkpoint_size(value):
    if number(value) is None:
        return "unknown"
    for scale, label in ((1e9, "GB"), (1e6, "MB"), (1e3, "KB")):
        if value >= scale:
            return f"{value / scale:,.2f} {label}"
    return f"{value:,.0f} B"


def plain_summary(frame):
    """Readable append-only summary from the same public frame as every consumer."""
    if frame.get("master_reduction"):
        master = frame["master_reduction"]
        execution = master_batch_execution(master)
        budget = execution.get("reserved_workers")
        if budget is None:
            budget = execution.get("workers")
        numerator_lines = []
        if not master.get("sector_symmetry_assistance", False):
            if master.get("numerator_symmetry_assistance", False) and master.get("operation") == "refine":
                numerator_lines = ["Numerator symmetry: finite certified inventory; denominator-preserving maps; not a global symmetry claim"]
            elif master.get("numerator_symmetry_assistance", False) or master.get("inherited_numerator_symmetry_authority", False):
                numerator_lines = ["Numerator symmetry: inherited finite proofs retained; current provider off"]
        return [f"RustRed · {frame['phase']} · {frame['state']} · elapsed {duration(frame['elapsed_seconds'])}",
                f"Stage {master['stage']} · {count(master['remaining_terminals'])} remaining terminals (nonminimal)",
                f"{'Published' if master.get('artifact') else 'Available'} executable rule records {count(master.get('published_rule_records'))} · observed cover rules {count(master.get('observed_cover_rules'))}",
                f"Relations {count(master['relation_rows'])} · {count(master['eliminated_terminals'])} eliminated",
                f"Checkpoint {frame['checkpoint']['state']} · {frame['checkpoint']['directory']}",
                "Native proved-relation replay; no new ordinary source search; no minimality claim" if master.get("proof_importing") else
                "Retained exact maps; finite search unfinished; no minimality claim" if master.get("partial_refinement") else
                "Modular discovery; exact accepted identities; exact search incomplete" if master.get("refinement_strategy") == "modular-circuits" else
                "Exact bounded search; no numerical master values or minimality claim",
                *(["Fresh relations-only round · shape (" + count(master.get("seed_depth")) + ","
                    + count(master.get("containing_sector_depth")) + ") · retained proved maps; new search cursor"]
                  if master.get("fresh_search", False) and master.get("operation") == "refine" else []),
                *(["Native execution: " + (execution.get("stage") or "not reported")
                    + " · configured worker limit " + count(budget)
                    + " (not measured busy cores)"] if execution else []),
                *(["Batch counters: prepared " + count(execution.get("rows_prepared")) + "/"
                    + count(execution.get("rows_total")) + " · eliminated " + count(execution.get("rows_eliminated"))
                    + " · merged " + count(execution.get("rows_committed")) + " · "
                    + count(execution.get("blocks_completed")) + " blocks"]
                  if execution.get("stage") else []),
                *(["Provider filtered: " + count(execution["provider_keys_filtered"])
                    + " pending keys outside finite inventory; current call"]
                  if execution.get("provider_keys_filtered") is not None else []),
                *numerator_lines,
                *[label + ": " + value + " · " + detail
                  for label, value, detail in sector_symmetry_metrics(master)]]
    rates, snapshot = frame["rates"], frame["closure_snapshot"]
    completed = rates["local_completion"]["per_second"]
    rate = "unknown" if completed is None else f"{completed * 3600:,.0f}"
    growth = rates["pending_growth_per_completion_1h"]
    growth_text = "unknown" if growth is None else f"{growth:+.2f}"
    gap = rates["discovery_minus_closure"]
    ratio = TELEMETRY.discovery_per_recursive_closure_1h(gap, snapshot)
    net_line = (f"Discovery/closure {_ratio_text(ratio)} D/C observed scan-batched · window {duration(gap['covered_seconds'])}"
                f"/{duration(gap['window_seconds'])}" + (" warm-up" if gap["warmup"] else ""))
    if ratio["state"] not in ("valid", "warmup"):
        net_line += " · " + clean(ratio["state"])
    freshness = "stale" if snapshot["stale"] is True else "fresh" if snapshot["stale"] is False else "unknown"
    scan = "scan advanced" if snapshot["advanced"] is True else "no new closure scan" if snapshot["advanced"] is False else "scan update unknown"
    checkpoint = frame["checkpoint"]
    checkpoint_text = f"Checkpoint {checkpoint['state']} generation {count(checkpoint['generation'])}"
    if checkpoint["directory"]:
        checkpoint_text += " · " + checkpoint["directory"]
    if number(checkpoint["duration_seconds"]) is not None:
        checkpoint_text += f" · {checkpoint['duration_seconds']:.2f}s"
    return [f"RustRed · {clean(frame['state']).upper()} · {duration(frame['elapsed_seconds'])}",
            *([restore_line(frame)] if restore_line(frame) else []),
            required_progress_line(frame),
            *([extended_scope_line(frame)] if extended_scope_line(frame) else []),
            f"Rate {rate} per hour · pending {growth_text} per completion",
            f"Recursive closure {_rate(_closure_rate(frame)['per_second'])} "
            + ("awaiting closure scan" if ratio["state"] == "awaiting_closure_scan" else "observed scan-batched"),
            net_line,
            f"Closure snapshot {freshness} · age {duration(snapshot['snapshot_age_seconds'])} · {scan}"
            + (" · heartbeat stale" if frame["heartbeat_stale"] else ""),
            *scan_observation_lines(frame),
            "Encountered numerator rank " + rank_text(rates.get("encountered_numerator_rank", {})),
            checkpoint_text,
            f"Checkpoint size {'unavailable' if checkpoint['bytes'] is None else format(checkpoint['bytes'] / 1e9, '.2f') + ' GB'} · duty {percent(rates['checkpoint_duty'])}",
            f"Phase {frame['phase']} · closure ETA unknown"]


class Presenter:
    """Overwriting table for TTYs; bounded periodic JSON events otherwise."""
    def __init__(self, stream=None, enabled=True, plain_seconds=30.0):
        self.stream = sys.stderr if stream is None else stream
        self.enabled = enabled
        self.tty = self.stream.isatty() and os.environ.get("TERM") != "dumb"
        self.color = self.tty and "NO_COLOR" not in os.environ
        self.plain_seconds = plain_seconds
        self.last_at = None
        self.last_state = None
        self.drawn = 0
        self.last_milestone = 0

    def render(self, status: dict, now=None, force=False):
        self.render_frame(TELEMETRY.normalize_status(status), now=now, force=force)

    def render_frame(self, frame, now=None, force=False):
        if not self.enabled:
            return
        now = time.monotonic() if now is None else now
        milestones = [event for event in frame.get("checkpoint_milestones", [])
                      if event.get("sequence", 0) > self.last_milestone]
        if milestones:
            if self.tty and self.drawn:
                self.stream.write(f"\x1b[{self.drawn}A\r\x1b[J")
                self.drawn = 0
            for event in milestones:
                label = "started" if event["event"] == "checkpoint_started" else "saved"
                detail = f"RustRed checkpoint {label} · generation {event.get('generation', '?')}"
                timestamp = number(event.get("started_unix_time" if label == "started" else "saved_unix_time"))
                if timestamp is not None:
                    try:
                        detail += " · " + datetime.fromtimestamp(timestamp, timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
                    except (OSError, OverflowError, ValueError):
                        detail += " · timestamp invalid"
                if number(event.get("duration_seconds")) is not None:
                    detail += f" · {event['duration_seconds']:.2f}s"
                detail += " · " + str(event.get("state_path") or event.get("directory", ""))
                if self.tty:
                    self.stream.write(clean(detail) + "\n")
                else:
                    self.stream.write(json.dumps({"event": "checkpoint", "message": clean(detail),
                                                  "checkpoint": event}, ensure_ascii=False, allow_nan=False) + "\n")
            self.last_milestone = max(event["sequence"] for event in milestones)
            self.stream.flush()
        state = frame.get("state")
        if not self.tty and not force and self.last_at is not None and state == self.last_state and now - self.last_at < self.plain_seconds:
            return
        if self.tty:
            terminal = shutil.get_terminal_size((100, 24))
            lines = render_table(frame, width=max(20, terminal.columns - 1),
                                 height=max(8, terminal.lines - 1), color=self.color)
            if self.drawn:
                self.stream.write(f"\x1b[{self.drawn}A\r\x1b[J")
            for line in lines:
                self.stream.write("\r\x1b[2K" + line + "\n")
            self.drawn = len(lines)
        else:
            event = {"event": "campaign_status", "telemetry": frame}
            event["message"] = " | ".join(plain_summary(frame))
            self.stream.write(json.dumps(event, ensure_ascii=False, allow_nan=False, separators=(",", ":")) + "\n")
        self.stream.flush()
        self.last_at = now
        self.last_state = state
