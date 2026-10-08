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
                " · base bar ≠ stage closure; cold verify")
    return (f"Required requests {count(scope.get('required'))} cumulative · "
            f"{count(scope.get('appended_required'))} appended · "
            "base-root bar is not enlarged-scope closure; cold verification required")


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
    return [
        f"RustRed · {state} · {duration(status.get('elapsed_seconds'))}{stale}",
        f"CPU {bar(cpu, status.get('workers'))} {cpu_text} / {count(status.get('workers'))} total reserved",
        f"Workers {count(progress.get('active_native_slots'))} native active, {count(progress.get('backpressured_native_slots'))} blocked"
        + (f" · {count(progress['finished_native_awaiting_publication'])} finished waiting"
           if progress.get('finished_native_awaiting_publication') is not None else "")
        + f" · reserved {count(allocation.get('inspectors'))} inspect + {count(allocation.get('admission_helpers'))} admission + {count(allocation.get('coordinator'))} coordinator",
        f"Closure {closure_bar} {closed_prefix}{count(closure['initial_closed'])} / {count(closure['initial_total'])} initial roots recursively closed{closure_note}",
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
    root = f"{conservative}{count(counts['initial_closed'])} / {count(counts['initial_total'])}"
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
        (0, row("BASE ROOT CLOSURE" if extended_scope_line(frame) else "ROOT CLOSURE", root,
                bar(counts["initial_closed"], counts["initial_total"], frame["elapsed_seconds"]) + " recursive", "94")),
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


def render_master_table(frame, width=100, height=24, color=True):
    """Phase-two view of the same event stream; no inferred algebra progress."""
    width, height = max(20, min(160, width)), max(8, height)
    inner = width - 4
    master, resource, checkpoint = frame["master_reduction"], frame["resources"], frame["checkpoint"]
    publishing = master.get("operation") == "publish"
    label_width, value_width = (19, 18) if width >= 74 else (10, 8)
    detail_width = inner - label_width - value_width - 6
    border = lambda left, right: _paint(left + "─" * (width - 2) + right, "2;36", color)
    full = lambda text, tint=None: "│ " + _paint(fit(text, inner), tint, color) + " │"

    def row(label, value, detail="", tint=None):
        if detail_width < 12:
            return full(f"{label}: {value}" + (f" · {detail}" if detail else ""), tint)
        return ("│ " + fit(label, label_width) + " │ " + _paint(fit(value, value_width), tint, color)
                + " │ " + fit(detail, detail_width) + " │")

    def gb(value):
        return "unknown" if number(value) is None else f"{value / 1e9:,.2f}"

    cpu, workers = resource.get("native_busy_cores"), resource.get("workers")
    cpu_ratio = cpu / workers if number(cpu) is not None and number(workers) is not None and workers > 0 else None
    total, completed = master.get("total_work"), master.get("completed_work")
    work = f"{count(completed)} / {count(total)}" if total is not None else count(completed)
    progress = bar(completed, total, frame["elapsed_seconds"]) if total is not None else "total not yet known"
    rows = [
        (0, row("Stage", master["stage"] or "preparing", "verify and package scoped rules" if publishing else "bounded exact relation search", "36")),
        (1, row("Retained keys", count(master["raw_terminals"]), "phase-one keys, including prior stages")),
        (1, row("Normalized", count(master["normalized_terminals"]), "exact symmetry aliases; not minimal")),
        (0, row("Remaining basis", count(master["remaining_terminals"]), "finite, potentially nonminimal", "32")),
        (1, row("Eliminated", count(master["eliminated_terminals"]), "exact relations between terminal keys", "32")),
        (0, row("Relation work", work, progress, "35")),
        (1, row("Rows generated", count(master["relation_rows"]), "seed depth " + count(master["seed_depth"]))),
        (1, row("Sparse system", count(master.get("independent_rows")) + " rows", count(master.get("nonzeros")) + " nonzeros")),
        (0, row("Active CPU cores", f"{'unknown' if cpu is None else f'{cpu:.1f}'} / {count(workers)}",
                "observed / reserved", indicator_color(cpu_ratio, "cpu"))),
        (0, row("Memory (GB)", gb(resource.get("aggregate_rss_bytes")) + " / " + gb(resource.get("hard_memory_bytes")),
                "save+stop at " + gb(resource.get("soft_memory_bytes")), "33")),
        (0, row("Checkpoint", checkpoint["state"], "generation " + count(checkpoint["generation"]), "33")),
        (1, row("Checkpoint size", master_checkpoint_size(checkpoint["bytes"]), duration(checkpoint["duration_seconds"]) + " write")),
        (2, full("Checkpoint " + (checkpoint["directory"] or "not yet saved"))),
        (2, full("Scope " + master["scope_binding"], "2")),
        (2, full("Receipts " + frame["run_directory"], "2")),
    ]
    if publishing:
        rows = [entry for index, entry in enumerate(rows) if index not in (4, 5, 6, 7)]
        rows.insert(1, (0, row("Coverage", "verified" if frame["state"] == "published_unrefined" else "checking",
                              "scoped dependency coverage; not termination", "32")))
        rows.insert(2, (0, row("Refinement", "not requested", "explicit refine command only", "36")))
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
    if master["artifact"]:
        rows.insert(0, (0, full("Artifact " + master["artifact"], "32")))
    if frame["stop_reason"]:
        rows.insert(0, (-1, full(frame["stop_reason"], "1;33")))
    if frame["heartbeat_stale"]:
        rows.insert(0, (-2, full("STALE HEARTBEAT — current activity unverified", "1;31")))
    fixed = [border("╭", "╮"),
             full(f"RustRed / {'ARTIFACT PUBLICATION' if publishing else 'MASTER REFINEMENT'} / {clean(frame['state']).upper()} / {duration(frame['elapsed_seconds'])}", "1;36"),
             border("├", "┤")]
    if detail_width >= 12 and height >= 20:
        fixed += [row("METRIC", "VALUE", "DETAIL", "1"), border("├", "┤")]
    footer = [border("├", "┤"), full("Portable scoped artifact · unrefined terminals · no numerical values" if publishing else
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
        return [f"RustRed · {frame['phase']} · {frame['state']} · {duration(frame['elapsed_seconds'])}",
                f"Stage {master['stage']} · {count(master['remaining_terminals'])} remaining terminals (nonminimal)",
                f"Relations {count(master['relation_rows'])} · {count(master['eliminated_terminals'])} eliminated",
                f"Checkpoint {frame['checkpoint']['state']} · {frame['checkpoint']['directory']}",
                "Exact bounded search; no numerical master values or minimality claim"]
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
