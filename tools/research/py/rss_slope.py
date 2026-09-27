"""RSS vs committed/discovered domains from heartbeat events (read-only).

Usage: rss_slope.py EVENTS.jsonl [EVENTS.jsonl ...]
Prints per file: points, simple slope of process_rss_bytes vs committed
domains, and the two-variable least-squares fit
RSS = a + b * scheduled_nodes + c * committed_domains (bytes per domain).
"""
import json
import sys


def points(path):
    out = []
    with open(path) as stream:
        for line in stream:
            if '"process_rss_bytes"' not in line:
                continue
            event = json.loads(line)
            rss = event.get("process_rss_bytes")
            progress = event.get("progress") or {}
            committed = progress.get("committed_domains")
            scheduled = progress.get("scheduled_nodes")
            if rss is None or committed is None or scheduled is None:
                continue
            out.append((float(scheduled), float(committed), float(rss)))
    return out


def solve3(m, v):
    # Gaussian elimination for a 3x3 system.
    a = [row[:] + [v[i]] for i, row in enumerate(m)]
    for col in range(3):
        pivot = max(range(col, 3), key=lambda r: abs(a[r][col]))
        a[col], a[pivot] = a[pivot], a[col]
        if abs(a[col][col]) < 1e-12:
            return None
        for r in range(3):
            if r != col:
                f = a[r][col] / a[col][col]
                for k in range(col, 4):
                    a[r][k] -= f * a[col][k]
    return [a[i][3] / a[i][i] for i in range(3)]


def main():
    for path in sys.argv[1:]:
        pts = [p for p in points(path) if p[1] > 0]
        n = len(pts)
        if n < 3:
            print(path, "points", n)
            continue
        xs = [p[1] for p in pts]
        ys = [p[2] for p in pts]
        mx, my = sum(xs) / n, sum(ys) / n
        sxx = sum((x - mx) ** 2 for x in xs)
        sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
        simple = sxy / sxx if sxx else float("nan")
        m = [[0.0] * 3 for _ in range(3)]
        v = [0.0] * 3
        for s, c, r in pts:
            row = (1.0, s, c)
            for i in range(3):
                v[i] += row[i] * r
                for j in range(3):
                    m[i][j] += row[i] * row[j]
        fit = solve3(m, v)
        print(json.dumps({
            "events": path, "points": n,
            "committed_range": [min(xs), max(xs)],
            "rss_range_bytes": [min(ys), max(ys)],
            "simple_slope_bytes_per_committed": round(simple, 1),
            "fit_bytes_per_discovered": None if fit is None else round(fit[1], 1),
            "fit_bytes_per_committed": None if fit is None else round(fit[2], 1),
        }))


if __name__ == "__main__":
    main()
