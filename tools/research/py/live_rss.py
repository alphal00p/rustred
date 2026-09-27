"""Read-only: live campaign process_rss_bytes around a checkpoint generation's save."""
import json, sys, datetime
events, generation = sys.argv[1], int(sys.argv[2])
run = events.rstrip('/').split('/')[-2]
start = datetime.datetime.strptime(run, "%Y%m%dT%H%M%S.%fZ").replace(tzinfo=datetime.timezone.utc).timestamp()
beats = []  # (line, elapsed, rss, progress_event)
saved = started = None
with open(events, 'rb') as f:
    for n, raw in enumerate(f, 1):
        if b'"heartbeat"' not in raw and b'"checkpoint_saved"' not in raw and b'"checkpoint_started"' not in raw:
            continue
        e = json.loads(raw)
        ev = e.get('event')
        if ev == 'heartbeat' and 'process_rss_bytes' in e:
            beats.append((n, e.get('elapsed_seconds'), e['process_rss_bytes'], (e.get('progress') or {}).get('event')))
        elif ev == 'checkpoint_saved' and (e.get('checkpoint') or {}).get('generation') == generation:
            saved = (n, e['checkpoint'])
        elif ev == 'checkpoint_started' and (e.get('checkpoint_write') or {}).get('generation') == generation:
            started = (n, e['checkpoint_write'])
        if saved and beats and beats[-1][0] > saved[0] + 400:
            break
assert saved, 'no checkpoint_saved for generation'
meta = saved[1]
t_saved, t_started = meta['saved_unix_time'], meta['started_unix_time']
before_save = [b for b in beats if b[0] < (started[0] if started else saved[0])]
during = [b for b in beats if started and started[0] < b[0] < saved[0]]
after = [b for b in beats if b[0] > saved[0]]
window = [b for b in beats if abs((start + b[1]) - t_saved) <= 900]
def row(b):
    return {"events_line": b[0], "elapsed_seconds": b[1], "approx_unix_time": round(start + b[1], 1), "process_rss_bytes": b[2], "progress_event": b[3]}
out = {
  "events_file": events, "generation": generation, "run_start_unix_from_directory_name": start,
  "checkpoint_started_line": started[0] if started else None, "checkpoint_saved_line": saved[0],
  "started_unix_time": t_started, "saved_unix_time": t_saved,
  "checkpoint_metadata": {k: meta.get(k) for k in ("committed_domains","pending_domains","committed_events","completed_native_inspections","bytes","save_seconds","executable")},
  "last_heartbeat_before_save_started": row(before_save[-1]) if before_save else None,
  "heartbeats_during_save": len(during),
  "max_rss_during_save": max((b[2] for b in during), default=None),
  "first_heartbeat_after_saved": row(after[0]) if after else None,
  "window_plus_minus_900s": {"heartbeats": len(window), "min_rss": min(b[2] for b in window), "max_rss": max(b[2] for b in window),
      "first": row(window[0]), "last": row(window[-1])},
}
print(json.dumps(out, indent=1))
