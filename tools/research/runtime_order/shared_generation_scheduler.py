"""One native shared worker pool, supervised by the existing aggregate guard.

Projection only presents parent rows. The supervisor owns exactly one native
process group; native Rust owns family/sector scheduling and checkpoint writes.
"""
import copy
import json
import os
from pathlib import Path
import time
import sys

import generation_scheduler as supervisor
from generation_state import native_observation


def schedule(resources, *, check_affinity=True):
    workers = resources["workers"]
    cpus = sorted(supervisor.SUPERVISOR.parse_cpu_set(resources["cpus"]))
    parents = resources["generation_jobs"]
    limit = resources.get("max_active_sectors")
    if (type(workers) is not int or workers <= 0 or type(parents) is not int or parents <= 0
            or len(cpus) != workers or (check_affinity and not set(cpus) <= os.sched_getaffinity(0))
            or (limit is not None and (type(limit) is not int or limit <= 0))):
        raise ValueError("shared generation requires positive workers/parent slots, permitted CPU IDs, and an optional positive sector limit")
    return dict(schema="rustred.shared-generation-schedule.v1", jobs=parents,
        workers_per_job=0, total_workers=workers, shared_pool=True,
        max_active_sectors=limit, slots=[cpus])


class Projection:
    def __init__(self, parents, retained, resources, status_path):
        self.parents = list(parents)
        self.retained = list(retained)
        self.resources = resources
        self.status_path = Path(status_path)

    def __call__(self, snapshot):
        child = snapshot["jobs"][0]
        native = None
        try:
            value = json.loads(self.status_path.read_bytes())
            if (value.get("schema") == "rustred.shared-generation-progress.v1"
                    and value.get("pid") == child.get("pid")
                    and value.get("timestamp_unix_seconds", 0) >= child.get("started_unix", float("inf"))
                    and time.time()-value["timestamp_unix_seconds"] <= 90):
                native = value
        except (OSError, ValueError, TypeError):
            pass
        statuses = {row["id"]: row for row in native["parents"]} if native else {}
        drained = child.get("state") in ("failed", "interrupted", "completed", "native_finished")
        rows = []
        for job in self.parents:
            parent = statuses.get(job.id, {})
            state = parent.get("state", "pending")
            if drained and state not in ("completed", "failed"):
                state = "not_started" if state == "pending" else "interrupted"
            if not native and child.get("state") in ("failed", "interrupted"):
                state = "interrupted"  # process failure does not locate a mathematical parent failure
            row = dict(id=job.id, state="running" if state == "preparing" else state,
                preparation_state=state, metadata=copy.deepcopy(job.metadata), shared_process=True,
                slot=None, cpus=child.get("cpus", []), workers=None,
                pid=child.get("pid"), pid_start=child.get("pid_start"),
                elapsed_seconds=child.get("elapsed_seconds"), sampled_rss_bytes=None, observed_cores=None,
                native_progress=native_observation(job.native_progress,
                    child.get("started_unix", time.time()), pid=child.get("pid"), pid_start=child.get("pid_start")),
                checkpoint_files_observed=None, error=parent.get("error"))
            if drained and row["native_progress"].get("snapshot") is not None:
                row["native_progress"]["status"] = "historical"
            rows.append(row)
        rows += [dict(copy.deepcopy(row), shared_process=True, preparation_state="retained")
                 for row in self.retained]
        snapshot["jobs"] = rows
        snapshot["resources"].update(schedule(self.resources))
        aggregate = snapshot.get("aggregate") or {}
        aggregate.update(active_jobs=sum(row["state"] in ("running", "preparing") for row in rows),
            pending_jobs=sum(row["state"] == "pending" for row in rows),
            pool=None if native is None else native.get("pool"))
        if drained:
            aggregate.update(observed_cores=0, sampled_rss_bytes=0, active_jobs=0, pending_jobs=0)
            if aggregate.get("pool") is not None:
                aggregate["pool"] = dict(aggregate["pool"], running=0, waiting_for_worker=0, admitted=0)
        snapshot["aggregate"] = aggregate
        return snapshot


def run(jobs, directory, resources, executable, completed, *, retained=(), observer=None):
    jobs, directory = list(jobs), Path(directory)
    schedule(resources)
    directory.parent.mkdir(parents=True, exist_ok=True)
    batch = directory.parent / (directory.name+"-batch.json")
    status = directory.parent / (directory.name+"-native.json")
    value = dict(schema="rustred.family-candidates-batch.v1", workers=resources["workers"],
        max_active_sectors=resources.get("max_active_sectors"), family_slots=resources["generation_jobs"],
        jobs=[dict(id=job.id, arguments=list(job.command[2:])) for job in jobs])
    # These files belong to this unique attempt; never overwrite unknown data.
    with batch.open("x") as stream:
        json.dump(value, stream, indent=2)
    command = (str(executable), "family-candidates-batch", "--config", str(batch), "--progress-json", str(status))
    internal = supervisor.Job("shared-worker-pool", command, {}, native_failure_status=status)
    projection = Projection(jobs, retained, resources, status)
    validated = set()
    def complete(_):
        for job in jobs:
            completed(job)
            validated.add(job.id)
    try:
        return supervisor.run([internal], directory, dict(resources, generation_jobs=1), complete,
            observer=observer, projection=projection)
    finally:
        # Preserve completed parent bundles even if another parent failed or
        # an operator stopped the process. Validation occurs after group drain.
        try:
            native = json.loads(status.read_bytes())
            clean = {row["id"] for row in native["parents"] if row["state"] == "completed"}
        except (OSError, ValueError, KeyError, TypeError):
            clean = set()
        already_failing = sys.exc_info()[0] is not None
        for job in jobs:
            if job.id in clean and job.id not in validated:
                try:
                    completed(job)
                except Exception as error:
                    if not already_failing:
                        raise
                    print(f"Completed-parent receipt could not be saved for {job.id}: {error}", file=sys.stderr)
