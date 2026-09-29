#!/usr/bin/env python3
"""Plot a saved telemetry.jsonl as a portable SVG (Python stdlib only).

Local completion and recursively closed domain rates are distinct measured
trailing-hour quantities. No curve is extrapolated, no missing rate becomes
zero, and scan-batched closure is not instantaneous throughput. This utility
reads receipts only; it never connects to or controls a running campaign.
"""
from __future__ import annotations

import argparse
import html
import importlib.util
import json
import math
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location("campaign_telemetry", Path(__file__).with_name("campaign_telemetry.py"))
TELEMETRY = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(TELEMETRY)
number = TELEMETRY.number


def records(path):
    """Bound each read even for corrupt input; None is an explicit plot break."""
    with Path(path).open("rb") as stream:
        while True:
            line = stream.readline(TELEMETRY.MAX_FRAME_BYTES + 1)
            if not line:
                return
            if len(line) > TELEMETRY.MAX_FRAME_BYTES:
                while line and not line.endswith(b"\n"):
                    line = stream.readline(TELEMETRY.MAX_FRAME_BYTES + 1)
                yield None
                continue
            if not line.endswith(b"\n"):
                # A live writer may not have completed the final record yet.
                return
            try:
                frame = json.loads(line)
                if not isinstance(frame, dict) or frame.get("schema") != TELEMETRY.SCHEMA:
                    raise ValueError("unsupported frame")
                yield frame
            except (ValueError, UnicodeError, RecursionError):
                yield None


def observations(path, start=0.0, end=None):
    """Yield bounded scalar plot observations, preserving missing/reset gaps."""
    last_elapsed = None
    last_run = None
    last_endpoints = None
    for frame in records(path):
        if frame is None:
            yield None
            continue
        elapsed = number(frame.get("elapsed_seconds"))
        if elapsed is None or elapsed < start or end is not None and elapsed > end:
            continue
        run = frame.get("run_directory")
        if last_elapsed is not None and (elapsed <= last_elapsed or run != last_run):
            yield None
        last_elapsed, last_run = elapsed, run
        rates = TELEMETRY.mapping(frame.get("rates"))
        completion = TELEMETRY.mapping(rates.get("local_completion"))
        closure = TELEMETRY.mapping(rates.get("recursive_closure"))
        gap = TELEMETRY.mapping(rates.get("discovery_minus_closure"))
        snapshot = TELEMETRY.mapping(frame.get("closure_snapshot"))
        endpoints = (number(completion.get("last_elapsed_seconds")), number(closure.get("last_elapsed_seconds")))
        if (endpoints == last_endpoints and any(value is not None for value in endpoints)
                and frame.get("heartbeat_stale") is not True
                and completion.get("state") == "measured" and closure.get("state") in ("valid", "warmup")):
            # The supervisor/UI can poll more often than a native heartbeat.
            # A repeated endpoint is not a new rate observation.
            continue
        last_endpoints = endpoints
        values = {"completion": number(completion.get("per_second")),
                  "closure": number(closure.get("per_second")), "gap": number(gap.get("per_second"))}
        if completion.get("state") != "measured":
            values["completion"] = None
        if closure.get("state") not in ("valid", "warmup"):
            values["closure"] = None
        if gap.get("state") not in ("valid", "warmup"):
            values["gap"] = None
        if frame.get("heartbeat_stale") is True:
            values = dict.fromkeys(values)
        yield {"elapsed": float(elapsed), **values,
               "closure_stale": snapshot.get("stale") is not False,
               "snapshot_age_seconds": number(snapshot.get("snapshot_age_seconds")),
               "scan_advanced": snapshot.get("advanced"),
               "warmup": completion.get("warmup") is True or closure.get("warmup") is True}


def load_points(path, start=0, end=None, max_points=20000):
    """Two-pass bounded extrema envelope, with explicit breaks in each bucket.

    Each bucket retains first/last plus per-series extrema. If a bucket contains
    missing data, that series is omitted for the bucket: never bridge an unseen
    gap just to make a smoother plot. The SVG discloses this conservative view.
    """
    options = dict(start=start, end=end)
    total = sum(1 for _ in observations(path, **options))
    bucket_size = max(1, math.ceil(total / max(1, max_points // 8)))
    points = []
    first = last = None
    extrema, missing, count = {}, set(), 0

    def flush():
        if count and first is None:
            points.append(None)
        elif first is not None:
            chosen = {pair[0]: pair[1] for pair in (first, last, *extrema.values())}
            for index, point in sorted(chosen.items()):
                points.append({**point, **{key: None for key in missing}})

    for index, point in enumerate(observations(path, **options)):
        # A concurrently appended live file must not expand the second pass
        # beyond the measured prefix (or defeat the bounded plot size).
        if index >= total:
            break
        count += 1
        if point is None:
            missing.update(("completion", "closure", "gap"))
        else:
            pair = (index, point)
            first, last = first or pair, pair
            for key in ("completion", "closure", "gap"):
                if point[key] is None:
                    missing.add(key)
                    continue
                for direction in ("min", "max"):
                    known = extrema.get((key, direction))
                    if known is None or (point[key] < known[1][key] if direction == "min" else point[key] > known[1][key]):
                        extrema[(key, direction)] = pair
        if count == bucket_size:
            flush()
            first = last = None
            extrema, missing, count = {}, set(), 0
    flush()
    return points, {"records": total, "bucket_size": bucket_size, "plotted_samples": len(points)}


def make_svg(points, metadata, title="RustRed campaign rates"):
    samples = [point for point in points if point is not None]
    if not samples:
        raise ValueError("no telemetry frames in the requested elapsed-time interval")
    origin = min(point["elapsed"] for point in samples)
    last = max(point["elapsed"] for point in samples)
    span = max(1.0, last - origin)
    time_scale, time_unit = (1.0, "Seconds") if span < 120 else (60.0, "Minutes") if span < 7200 else (3600.0, "Hours")
    left, right, top, bottom, width = 95, 1050, 115, 475, 1120
    rate_values = [point[key] for point in samples for key in ("completion", "closure") if point[key] is not None]
    ymax = max(rate_values, default=0)
    ymax = 1.0 if ymax <= 0 else ymax * 1.08
    esc = html.escape
    svg = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="690" viewBox="0 0 {width} 690">',
           '<rect width="100%" height="100%" fill="#0e1726"/>',
           '<style>text{font-family:system-ui,sans-serif;fill:#dae4f0} .small{font-size:13px} .axis{font-size:14px;fill:#a8b8cd}</style>',
           f'<text x="42" y="42" font-size="24" font-weight="600">{esc(title)}</text>',
           '<text x="42" y="68" class="small">Measured trailing-hour rates; actual sampled windows during warm-up · no ETA</text>']
    x = lambda seconds: left + (seconds - origin) / span * (right - left)
    y = lambda value: bottom - value / ymax * (bottom - top)
    for index in range(6):
        value = ymax * index / 5
        yy = y(value)
        svg += [f'<line x1="{left}" x2="{right}" y1="{yy:.2f}" y2="{yy:.2f}" stroke="#27384e"/>',
                f'<text x="{left - 12}" y="{yy + 5:.2f}" text-anchor="end" class="axis">{value:,.3g}</text>']
        xx = left + index / 5 * (right - left)
        elapsed = span * index / 5 / time_scale
        svg += [f'<line x1="{xx:.2f}" x2="{xx:.2f}" y1="{top}" y2="{bottom}" stroke="#1b2b40"/>',
                f'<text x="{xx:.2f}" y="{bottom + 28}" text-anchor="middle" class="axis">{elapsed:.2f}</text>']
    svg += ['<text x="22" y="280" transform="rotate(-90 22 280)" text-anchor="middle" class="axis">domains / second</text>',
            f'<text x="{(left + right) / 2}" y="{bottom + 55}" text-anchor="middle" class="axis">{time_unit} since plotted interval start (run elapsed {origin:.3f} s)</text>']
    for key, color, label in (("completion", "#59cbfa", "Local completions"),
                              ("closure", "#d0a0ff", "Recursively closed — conservative scan-batched observations")):
        count = 0
        previous = None
        for point in points:
            if point is None or point[key] is None:
                previous = None
                continue
            xx, yy = x(point["elapsed"]), y(point[key])
            stale = key == "closure" and point["closure_stale"]
            if previous is not None:
                # Step plot: retain measured endpoints; do not invent smooth fits.
                dashed = stale or key == "closure" and previous["closure_stale"]
                style = ' stroke-dasharray="5 4" opacity="0.7"' if dashed else ''
                svg.append(f'<path d="M {x(previous["elapsed"]):.2f},{y(previous[key]):.2f} H {xx:.2f} V {yy:.2f}" fill="none" stroke="{color}" stroke-width="2"{style}/>')
            fill = "#0e1726" if stale else color
            radius = 3 if key == "closure" and point["scan_advanced"] is True else 1.7
            svg.append(f'<circle cx="{xx:.2f}" cy="{yy:.2f}" r="{radius}" fill="{fill}" stroke="{color}"/>')
            previous, count = point, count + 1
        legend_y = 565 if key == "completion" else 590
        svg += [f'<line x1="48" x2="73" y1="{legend_y - 5}" y2="{legend_y - 5}" stroke="{color}" stroke-width="3"/>',
                f'<text x="85" y="{legend_y}" class="small">{esc(label)} ({count:,} plotted observations)</text>']
    max_age = max((point["snapshot_age_seconds"] for point in samples if point["snapshot_age_seconds"] is not None), default=None)
    age_label = "unknown" if max_age is None else f"{max_age:,.1f}s"
    policy = f"Dashed / hollow: graph-dirty closure snapshot (max age {age_label}); larger dots: scan advanced."
    svg += [f'<text x="42" y="625" class="small">{esc(policy)}</text>',
            f'<text x="42" y="650" class="small">{metadata["records"]:,} records · extrema-envelope bucket {metadata["bucket_size"]} · missing / invalid / stale-heartbeat samples are gaps.</text>',
            '<text x="42" y="674" class="small">A zero scan-batched rate is an observed count delta, not proof that no new domains closed.</text>',
            '</svg>']
    return "\n".join(svg) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("telemetry", type=Path)
    parser.add_argument("--output", type=Path, required=True, help="SVG output (no plotting dependency required)")
    parser.add_argument("--start", type=float, default=0, help="inclusive run elapsed seconds")
    parser.add_argument("--end", type=float, help="inclusive run elapsed seconds")
    parser.add_argument("--max-points", type=int, default=20000)
    parser.add_argument("--title", default="RustRed campaign rates")
    args = parser.parse_args(argv)
    if (not math.isfinite(args.start) or args.start < 0 or
            args.end is not None and (not math.isfinite(args.end) or args.end < args.start) or
            not 16 <= args.max_points <= 100000):
        parser.error("require 0 <= start <= end and 16 <= max-points <= 100000")
    if args.output.suffix.lower() != ".svg":
        parser.error("--output must have an .svg extension")
    if args.output.resolve() == args.telemetry.resolve():
        parser.error("plot output must not overwrite its telemetry input")
    try:
        if args.output.exists() and args.output.samefile(args.telemetry):
            parser.error("plot output must not overwrite its telemetry input")
        points, metadata = load_points(args.telemetry, args.start, args.end, args.max_points)
        svg = make_svg(points, metadata, args.title)
        args.output.write_text(svg, encoding="utf-8")
        print(json.dumps({"output": str(args.output), **metadata}, sort_keys=True))
        return 0
    except (OSError, ValueError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    raise SystemExit(main())
