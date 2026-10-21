"""Versioned preparation telemetry, independent of terminal/web presentation.

These observations never establish native output validity or family closure.
Consumers may tail events.jsonl or read the atomically replaced snapshot.json.
"""
import copy
import json
import math
import os
from pathlib import Path
import time

SCHEMA = "rustred.preparation-progress.v1"
NATIVE_SCHEMA = "rustred.family-generation-progress.v1"
MAX_NATIVE_SNAPSHOT_BYTES = 1024 * 1024


def finite_float(text):
    value = float(text)
    if not math.isfinite(value):
        raise ValueError("non-finite native telemetry number")
    return value


def reject_constant(text):
    raise ValueError(f"invalid native telemetry constant: {text}")


def native_observation(path, attempt_started, now=None, stale_seconds=90, *, pid=None, pid_start=None,
                       completed=False):
    """Read invocation-bound telemetry; completed output is final, never live.

    Only the coordinator's completed-parent outcome permits retaining an old
    output_written snapshot. Age and the actual native counters are preserved.
    """
    now = time.time() if now is None else now
    completed = completed is True
    result = dict(status="unavailable", native_observed_at=None, age_seconds=None, snapshot=None)
    if path is None:
        return result
    try:
        path = Path(path)
        with path.open("rb") as stream:
            stat = os.fstat(stream.fileno())
            result.update(native_observed_at=stat.st_mtime, age_seconds=max(0, now-stat.st_mtime))
            if type(attempt_started) not in (int, float) or not math.isfinite(attempt_started):
                raise ValueError("native observation has no frozen attempt start")
            if stat.st_mtime < attempt_started or (now-stat.st_mtime > stale_seconds and not completed):
                result["status"] = "stale"
                return result
            raw = stream.read(MAX_NATIVE_SNAPSHOT_BYTES+1)
        if len(raw) > MAX_NATIVE_SNAPSHOT_BYTES:
            raise ValueError("native snapshot exceeds 1 MiB telemetry limit")
        value = json.loads(raw, parse_float=finite_float, parse_constant=reject_constant)
        if not isinstance(value, dict) or value.get("schema") != NATIVE_SCHEMA:
            result["status"] = "unsupported"
            return result
        if ((pid is not None and (type(value.get("pid")) is not int or value.get("pid") != pid))
                or (pid_start is not None and (type(value.get("process_start_ticks")) is not int
                                               or value.get("process_start_ticks") != pid_start))):
            result["status"] = "wrong_invocation"
            return result
        if value.get("family_closure_claim", False) is not False:
            raise ValueError("native telemetry cannot claim family closure")
        if completed:
            if type(pid) is not int or pid <= 0 or type(pid_start) is not int or pid_start < 0:
                result["status"] = "wrong_invocation"
                return result
            if value.get("state") != "output_written" or value.get("family_closure_claim") is not False:
                raise ValueError("completed parent has no terminal output_written native snapshot")
            result.update(status="final", snapshot=value)
        else:
            result.update(status="fresh", snapshot=value)
    except FileNotFoundError:
        pass
    except (OSError, ValueError, TypeError) as error:
        result.update(status="invalid", diagnostic=str(error)[:1024])
    return result


def diagnostic_tail(path):
    try:
        with Path(path).open("rb") as stream:
            stream.seek(max(0, stream.seek(0, 2)-4096))
            return stream.read().decode("utf-8", errors="replace")[-4096:]
    except OSError:
        return None


class Progress:
    """Coordinator-owned state; observers receive independent JSON values."""
    def __init__(self, directory, jobs, resources, observer=None, phase="generation", projection=None):
        self.directory = Path(directory)
        self.observer = observer
        self.projection = projection
        self.started = time.monotonic()
        self.value = dict(schema=SCHEMA, run_id=str(self.directory.parent),
            attempt_id=self.directory.name, snapshot_seq=0, phase=phase, state="running",
            timestamp_unix_seconds=time.time(), elapsed_seconds=0,
            resources=copy.deepcopy(resources), jobs=copy.deepcopy(jobs),
            aggregate=None, stop_reason=None, family_closure_claim=False,
            checkpoint=dict(resume_granularity="completed-sector", in_sector_resume=False))
        self.events = (self.directory / "events.jsonl").open("x")
        self.last_sample_event = None

    def emit(self, event, **updates):
        self.value.update(updates)
        self.value["snapshot_seq"] += 1
        self.value.update(timestamp_unix_seconds=time.time(), elapsed_seconds=time.monotonic()-self.started)
        presented = self.value
        if self.projection is not None:
            try:
                presented = self.projection(copy.deepcopy(self.value))
            except Exception as error:
                self.value["last_projection_error"] = f"{type(error).__name__}: {error}"[:1024]
        encoded = json.dumps(presented, allow_nan=False, sort_keys=True)
        temporary = self.directory / ".snapshot.json.part"
        with temporary.open("w") as stream:
            stream.write(encoded + "\n")
        os.replace(temporary, self.directory / "snapshot.json")
        # Full latest state stays available at the dashboard cadence. Historical
        # samples retain compact counters at most every30s, never every native
        # frame/active-job array or raw stderr tail (which can be large).
        now = time.monotonic()
        if event != "sample" or self.last_sample_event is None or now-self.last_sample_event >= 30:
            record = {key: presented[key] for key in ("schema", "run_id", "attempt_id", "snapshot_seq",
                "phase", "state", "timestamp_unix_seconds", "elapsed_seconds", "aggregate", "stop_reason")}
            record["jobs"] = [dict(id=row["id"], state=row["state"], slot=row.get("slot"),
                pid=row.get("pid"), pid_start=row.get("pid_start"), started_unix=row.get("started_unix"),
                native_progress_path=row.get("native_progress_path"), observed_cores=row.get("observed_cores"),
                sampled_rss_bytes=row.get("sampled_rss_bytes"),
                checkpoint_files_observed=row.get("checkpoint_files_observed"),
                native_status=(row.get("native_progress") or {}).get("status"),
                native_counts=((row.get("native_progress") or {}).get("snapshot") or {}).get("counts"))
                for row in presented["jobs"]]
            self.events.write(json.dumps(dict(event=event, snapshot=record), allow_nan=False) + "\n")
            self.events.flush()
            if event == "sample":
                self.last_sample_event = now
        if self.observer is not None:
            try:
                self.observer(dict(event=event, snapshot=json.loads(encoded)))
            except Exception as error:
                # Presentation is not resource/IBP authority. A failed optional
                # renderer must not terminate otherwise healthy solver groups.
                self.value["last_observer_error"] = f"{type(error).__name__}: {error}"[:1024]

    def close(self):
        self.events.close()
