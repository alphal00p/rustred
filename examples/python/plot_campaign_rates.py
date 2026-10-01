#!/usr/bin/env python3
"""Plot a saved telemetry.jsonl as a portable SVG (Python stdlib only).

Left axis: observed unresolved-domain total (discovered minus recursively
closed). Right axis: RAW trailing-window discovery-minus-closure rate in
domains/second, not the dimensionless dashboard D/C ratio. No interpolation or
extrapolation, missing values stay missing, and scan-batched closure is not
instantaneous throughput. This utility never controls a running campaign.
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
    last_observation = None
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
            last_observation = None
        last_elapsed, last_run = elapsed, run
        rates = TELEMETRY.mapping(frame.get("rates"))
        completion = TELEMETRY.mapping(rates.get("local_completion"))
        closure = TELEMETRY.mapping(rates.get("recursive_closure"))
        gap = TELEMETRY.mapping(rates.get("discovery_minus_closure"))
        snapshot = TELEMETRY.mapping(frame.get("closure_snapshot"))
        counts = TELEMETRY.mapping(frame.get("counts"))
        total, closed = number(counts.get("total_domains")), number(counts.get("total_closed"))
        unresolved = total - closed if (snapshot.get("available") is True and total is not None
            and closed is not None and 0 <= closed <= total) else None
        paired = TELEMETRY.discovery_per_recursive_closure_1h(gap, snapshot)
        observation = (number(gap.get("last_elapsed_seconds")), total, closed,
                       gap.get("state"), number(gap.get("per_second")), snapshot.get("stale"), paired["state"])
        if (observation == last_observation and observation[0] is not None
                and frame.get("heartbeat_stale") is not True):
            # The supervisor/UI can poll more often than a native heartbeat.
            # A repeated endpoint is not a new rate observation.
            continue
        last_observation = observation
        values = {"completion": number(completion.get("per_second")),
                  "closure": number(closure.get("per_second")), "gap": number(gap.get("per_second")),
                  "unresolved": unresolved}
        if completion.get("state") != "measured":
            values["completion"] = None
        if closure.get("state") not in ("valid", "warmup"):
            values["closure"] = None
        if snapshot.get("available") is not True or paired["state"] not in ("valid", "warmup", "empty_window"):
            values["gap"] = None
            values["closure"] = None
        if frame.get("heartbeat_stale") is True:
            values = dict.fromkeys(values)
        yield {"elapsed": float(elapsed), **values,
               "closure_stale": snapshot.get("stale") is not False,
               "snapshot_age_seconds": number(snapshot.get("snapshot_age_seconds")),
               "scan_advanced": snapshot.get("advanced"),
               "warmup": gap.get("warmup") is True}


def load_points(path, start=0, end=None, max_points=20000):
    """Two-pass bounded extrema envelope, with explicit breaks in each bucket.

    Each bucket retains first/last plus per-series extrema. If a bucket contains
    missing data, that series is omitted for the bucket: never bridge an unseen
    gap just to make a smoother plot. The SVG discloses this conservative view.
    """
    options = dict(start=start, end=end)
    total = sum(1 for _ in observations(path, **options))
    bucket_size = max(1, math.ceil(total / max(1, max_points // 6)))
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
            missing.update(("unresolved", "gap"))
        else:
            pair = (index, point)
            first, last = first or pair, pair
            for key in ("unresolved", "gap"):
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


def make_svg(points, metadata, title="RustRed unresolved domains and net rate"):
    samples = [point for point in points if point is not None]
    if not samples:
        raise ValueError("no telemetry frames in the requested elapsed-time interval")
    origin = min(point["elapsed"] for point in samples)
    last = max(point["elapsed"] for point in samples)
    span = max(1.0, last - origin)
    time_scale, time_unit = (1.0, "Seconds") if span < 120 else (60.0, "Minutes") if span < 7200 else (3600.0, "Hours")
    left, right, top, bottom, width = 110, 990, 120, 475, 1120
    totals = [point["unresolved"] for point in samples if point["unresolved"] is not None]
    raw_rates = [point["gap"] for point in samples if point["gap"] is not None]
    # Scale before adding headroom: finite extreme inputs must not overflow.
    total_scale = max(totals, default=0) or 1.0
    rate_scale = max((abs(value) for value in raw_rates), default=0) or 1.0
    rmin = min([0.0, *(value / rate_scale for value in raw_rates)])
    rmax = max([0.0, *(value / rate_scale for value in raw_rates)])
    if rmin == rmax:
        rmin, rmax = -1.0, 1.0
    rpad = (rmax - rmin) * .05
    rmin, rmax = rmin - rpad, rmax + rpad
    esc = html.escape
    svg = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="690" viewBox="0 0 {width} 690">',
           '<rect width="100%" height="100%" fill="#0e1726"/>',
           '<style>text{font-family:system-ui,sans-serif;fill:#dae4f0} .small{font-size:13px} .axis{font-size:14px;fill:#a8b8cd}</style>',
           f'<text x="42" y="42" font-size="24" font-weight="600">{esc(title)}</text>',
           '<text x="42" y="68" class="small">Left: unresolved domains · Right: RAW discovery − recursive closure (domains/s), not the D/C ratio</text>',
           '<text x="42" y="91" class="small">Observed samples only; no interpolation · trailing-hour rate uses actual sampled windows during warm-up · no ETA</text>']
    x = lambda seconds: left + (seconds - origin) / span * (right - left)
    y_total = lambda value: bottom - (value / total_scale) / 1.08 * (bottom - top)
    y_rate = lambda value: bottom - ((value / rate_scale) - rmin) / (rmax - rmin) * (bottom - top)
    for index in range(6):
        fraction = index / 5
        value = total_scale * fraction  # Upper padding is not a fabricated sample.
        yy = y_total(value)
        svg += [f'<line x1="{left}" x2="{right}" y1="{yy:.2f}" y2="{yy:.2f}" stroke="#27384e"/>',
                f'<text x="{left - 12}" y="{yy + 5:.2f}" text-anchor="end" class="axis" style="fill:#59cbfa">{value:,.3g}</text>']
        rate_tick = rmin + fraction * (rmax - rmin)
        # Tick labels stay inside the measured scale to avoid extreme overflow.
        rate_tick = max(-1.0, min(1.0, rate_tick))
        yy_rate = y_rate(rate_tick * rate_scale)
        svg.append(f'<text x="{right + 12}" y="{yy_rate + 5:.2f}" class="axis" style="fill:#f2ba72">{rate_tick * rate_scale:+,.3g}</text>')
        xx = left + index / 5 * (right - left)
        elapsed = span * index / 5 / time_scale
        svg += [f'<line x1="{xx:.2f}" x2="{xx:.2f}" y1="{top}" y2="{bottom}" stroke="#1b2b40"/>',
                f'<text x="{xx:.2f}" y="{bottom + 28}" text-anchor="middle" class="axis">{elapsed:.2f}</text>']
    svg += ['<text x="22" y="295" transform="rotate(-90 22 295)" text-anchor="middle" class="axis" style="fill:#59cbfa">Unresolved domains (total)</text>',
            '<text x="1100" y="295" transform="rotate(90 1100 295)" text-anchor="middle" class="axis" style="fill:#f2ba72">Discovery − closure (domains / second)</text>',
            f'<text x="{(left + right) / 2}" y="{bottom + 55}" text-anchor="middle" class="axis">{time_unit} since plotted interval start (run elapsed {origin:.3f} s)</text>']
    zero = y_rate(0)
    svg.append(f'<line x1="{left}" x2="{right}" y1="{zero:.2f}" y2="{zero:.2f}" stroke="#f2ba72" stroke-dasharray="3 5" opacity=".35"/>')
    for key, color, label, y in (("unresolved", "#59cbfa", "LEFT: discovered − recursively closed total", y_total),
                                ("gap", "#f2ba72", "RIGHT: raw net rate, domains/s (signed)", y_rate)):
        count = 0
        for point in points:
            if point is None or point[key] is None:
                continue
            xx, yy = x(point["elapsed"]), y(point[key])
            stale = point["closure_stale"]
            fill = "#0e1726" if stale else color
            radius = 3.5 if point["scan_advanced"] is True else 2.5
            svg.append(f'<circle cx="{xx:.2f}" cy="{yy:.2f}" r="{radius}" fill="{fill}" stroke="{color}"/>')
            count += 1
        legend_y = 565 if key == "unresolved" else 590
        svg += [f'<line x1="48" x2="73" y1="{legend_y - 5}" y2="{legend_y - 5}" stroke="{color}" stroke-width="3"/>',
                f'<text x="85" y="{legend_y}" class="small">{esc(label)} ({count:,} plotted observations)</text>']
    max_age = max((point["snapshot_age_seconds"] for point in samples if point["snapshot_age_seconds"] is not None), default=None)
    age_label = "unknown" if max_age is None else f"{max_age:,.1f}s"
    policy = f"Hollow: graph-dirty closure snapshot (max age {age_label}); unresolved total is then an upper bound."
    svg += [f'<text x="42" y="625" class="small">{esc(policy)}</text>',
            f'<text x="42" y="650" class="small">{metadata["records"]:,} records · extrema-envelope bucket {metadata["bucket_size"]} · missing / invalid / stale-heartbeat samples are gaps.</text>',
            '<text x="42" y="674" class="small">Larger dots: scan advanced. A zero scan-batched rate is not proof that no new domains closed. JSON retains window metadata.</text>',
            '</svg>']
    return "\n".join(svg) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("telemetry", type=Path)
    parser.add_argument("--output", type=Path, required=True, help="SVG output (no plotting dependency required)")
    parser.add_argument("--start", type=float, default=0, help="inclusive run elapsed seconds")
    parser.add_argument("--end", type=float, help="inclusive run elapsed seconds")
    parser.add_argument("--max-points", type=int, default=20000)
    parser.add_argument("--title", default="RustRed unresolved domains and net rate")
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
