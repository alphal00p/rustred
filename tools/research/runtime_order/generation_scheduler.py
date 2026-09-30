"""Bounded independent native jobs with one aggregate resource authority.

Only the main thread installs signals, samples resources and publishes events.
Native jobs own disjoint fixed CPU slots; an idle slot takes the next parent.
There is no runtime deadline and no per-parent copy of the RAM allowance.
"""
from collections import deque
from contextlib import ExitStack
from dataclasses import dataclass
import json
import os
from pathlib import Path
import signal
import subprocess
import time

from generation_guard import ROOT, SUPERVISOR, group_running, native_environment
from generation_state import Progress, diagnostic_tail, native_observation


def schedule(resources):
    workers, count = resources["workers"], resources.get("generation_jobs", 1)
    cpus = sorted(SUPERVISOR.parse_cpu_set(resources["cpus"]))
    if (not isinstance(count, int) or isinstance(count, bool) or count <= 0
            or count > workers or workers % count):
        raise ValueError("generation-jobs must divide workers exactly and be in 1..workers")
    if len(cpus) != workers or not set(cpus) <= os.sched_getaffinity(0):
        raise ValueError("provide exactly workers permitted CPU IDs")
    width = workers // count
    return dict(schema="rustred.generation-schedule.v1", jobs=count, workers_per_job=width,
                slots=[cpus[index*width:(index+1)*width] for index in range(count)])


@dataclass(frozen=True)
class Job:
    id: str
    command: tuple
    metadata: dict
    native_progress: Path | None = None
    checkpoint: Path | None = None


@dataclass
class Child:
    job: Job
    process: object
    slot: int
    directory: Path
    streams: ExitStack
    started: float
    started_unix: float
    start_identity: int | None = None


def leader_status(process):
    # Do not reap the leader until its whole process group has drained. Its
    # reserved PID therefore cannot be reused while we might signal that PGID.
    status = os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
    if status is None:
        return None
    return status.si_status if status.si_code == os.CLD_EXITED else -status.si_status


def run(jobs, directory, resources, completed, *, retained=(), observer=None, poll_seconds=2, grace_seconds=10):
    """Run a finite job queue; output callbacks follow ALL owned-group drains.

    On any failure/cancel, stop dispatch and drain every already-owned group
    before returning an error. Callbacks validate/persist clean completed outputs
    in original job order, including clean exits that preceded a later failure.
    Final staging is the caller's barrier after this function succeeds.
    """
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=False)
    jobs = list(jobs)
    if len({job.id for job in jobs}) != len(jobs) or any(
            not job.id or Path(job.id).name != job.id or job.id in (".", "..") for job in jobs):
        raise ValueError("generation job IDs must be unique filename components")
    try:
        allocation = schedule(resources)
        host = SUPERVISOR.host_memory()
        hard, soft, floor = SUPERVISOR.memory_admission(resources["max_memory_bytes"], None, host,
            resources["host_memory_reserve_bytes"], resources["ram_guard_margin_percent"])
        guard = SUPERVISOR.RamGuard(hard, soft, floor)
    except (OSError, ValueError, KeyError) as error:
        (directory / "result.json").write_text(json.dumps(dict(exit_code=None, child_started=False,
            owned_groups_drained=True, stop_reason=f"resource_admission_failed:{error}"), indent=2) + "\n")
        raise
    request = dict(schedule=allocation, started_unix=time.time(), hard_timeout=None,
        memory=SUPERVISOR.memory_admission_record(resources["max_memory_bytes"], host, floor, hard, soft),
        jobs=[dict(id=job.id, command=list(job.command), metadata=job.metadata) for job in jobs],
        checkpoint="completed native sectors only; active sectors restart")
    (directory / "request.json").write_text(json.dumps(request, indent=2) + "\n")
    descriptors = [dict(id=job.id, state="pending", metadata=job.metadata, slot=None, cpus=[],
                       pid=None, pid_start=None, native_progress=None, checkpoint_files_observed=None)
                   for job in jobs] + list(retained)
    by_id = {row["id"]: row for row in descriptors}
    pending, active = deque(jobs), {}
    previous = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}
    reason, stopping, failure, progress = None, None, None, None
    started, peak, cpu_previous, sample_previous = time.monotonic(), 0, {}, None
    outcomes, all_children, signal_errors = {}, [], []
    clean_finished = set()
    failed_native = set()

    def send(child, sig):
        # All tracked live children remain unreaped until group_running=False.
        try:
            os.killpg(child.process.pid, sig)
        except ProcessLookupError:
            pass
        except OSError as error:
            # One unexpected signal failure must not prevent attempts to stop
            # the other owned groups. Drain still refuses to finish while any
            # live member remains; this is never reported as successful cleanup.
            signal_errors.append(dict(process_group=child.process.pid, signal=int(sig), detail=str(error)))

    def stop(why):
        nonlocal reason, stopping
        if reason is None:
            reason, stopping = why, time.monotonic()
            for child in active.values():
                send(child, signal.SIGTERM)

    try:
        for sig in previous:
            signal.signal(sig, lambda number, frame: stop(f"operator_signal_{number}"))
        collector = SUPERVISOR.ProcessTreeCollector()
        collector.register(os.getpid())
        progress = Progress(directory, descriptors,
            dict(**allocation, memory=request["memory"], total_workers=resources["workers"]), observer)
        # Progress keeps a private copy; this is the coordinator's mutable view.
        by_id = {row["id"]: row for row in progress.value["jobs"]}
        progress.emit("started")
        while pending or active:
            now = time.monotonic()
            table, diagnostics = collector.sample()
            rss = sum(row["rss_bytes"] for row in table.values())
            peak = max(peak, rss)
            host = SUPERVISOR.host_memory()
            decision = guard.observe(now, rss, host["available_bytes"], SUPERVISOR.tree_swap_bytes(table))
            elapsed = None if sample_previous is None else now-sample_previous
            delta, cpu_previous, processes = SUPERVISOR.process_cpu_sample(
                table, cpu_previous, elapsed, os.getpid(), None)
            rates = {row["pid"]: row["observed_busy_cores"] for row in processes}
            sample_previous = now
            if decision["hard"]:
                stop(decision["hard"])
                for child in active.values():
                    send(child, signal.SIGKILL)
            elif decision["cooperative"]:
                stop(decision["cooperative"])
            for slot, child in list(active.items()):
                row = by_id[child.job.id]
                members = {pid: value for pid, value in table.items() if value["pgrp"] == child.process.pid}
                row.update(elapsed_seconds=now-child.started,
                    sampled_rss_bytes=sum(value["rss_bytes"] for value in members.values()),
                    observed_cores=None if elapsed is None else sum(rates.get(pid) or 0 for pid in members),
                    native_progress=native_observation(child.job.native_progress, child.started_unix,
                        pid=child.process.pid, pid_start=child.start_identity),
                    diagnostic_stderr_tail=diagnostic_tail(child.directory / "stderr"))
                if child.job.checkpoint is not None:
                    row["checkpoint_files_observed"] = sum(1 for _ in child.job.checkpoint.glob("sector-*.rrbin"))
                code = leader_status(child.process)
                if code is not None and code != 0:
                    # A failed leader can leave live children behind. Detect
                    # failure before waiting for that group, then stop/drain
                    # it and its siblings instead of waiting indefinitely.
                    if reason is None:
                        failed_native.add(child.job.id)
                    stop(f"native_job_failed:{child.job.id}:exit_{code}")
                if code is not None and not group_running(child.process.pid):
                    code = child.process.wait()
                    # Remove before invoking a fallible completion callback;
                    # its group is gone, and cannot be signalled after PID reuse.
                    del active[slot]
                    child.streams.close()
                    outcome = dict(exit_code=code, owned_group_drained=True, process_group=child.process.pid,
                        elapsed_seconds=now-child.started, stop_reason=reason,
                        completed_sector_checkpoint_only=True, in_sector_resume=False)
                    outcomes[child.job.id] = outcome
                    (child.directory / "result.json").write_text(json.dumps(outcome, indent=2) + "\n")
                    if code == 0 and reason is None:
                        clean_finished.add(child.job.id)
                        row.update(state="native_finished", exit_code=code)
                    else:
                        row.update(state="failed" if child.job.id in failed_native else "interrupted", exit_code=code)
                        stop(f"native_job_failed:{child.job.id}:exit_{code}")
                    progress.emit("job_finished")
            if stopping is not None:
                if now-stopping >= grace_seconds:
                    for child in active.values():
                        send(child, signal.SIGKILL)
                for job in pending:
                    by_id[job.id]["state"] = "not_started"
                pending.clear()
            else:
                for slot, cpus in enumerate(allocation["slots"]):
                    if not pending:
                        break
                    if slot in active:
                        continue
                    job = pending.popleft()
                    child_directory = directory / job.id
                    child_directory.mkdir()
                    streams = ExitStack()
                    try:
                        stdout = streams.enter_context((child_directory / "stdout").open("xb"))
                        stderr = streams.enter_context((child_directory / "stderr").open("xb"))
                        process = subprocess.Popen(job.command, cwd=ROOT, env=native_environment(),
                            stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr, start_new_session=True,
                            preexec_fn=lambda cpus=tuple(cpus): os.sched_setaffinity(0, cpus))
                    except BaseException:
                        streams.close()
                        raise
                    child = Child(job, process, slot, child_directory, streams, time.monotonic(), time.time())
                    active[slot] = child
                    all_children.append(child)
                    # The leader is not reaped, so even a short-lived process
                    # keeps its identity available until its group is drained.
                    collector.register(process.pid)
                    child.start_identity = collector.identities[process.pid]
                    row = by_id[job.id]
                    row.update(state="running", slot=slot, cpus=cpus, workers=allocation["workers_per_job"],
                        pid=process.pid, pid_start=child.start_identity, started_unix=child.started_unix)
                    (child_directory / "child.json").write_text(json.dumps(dict(pid=process.pid,
                        process_group=process.pid, pid_start=child.start_identity, cpus=cpus)) + "\n")
                    progress.emit("job_started")
                    # Signals may arrive during Popen before active publication.
                    if reason is not None:
                        send(child, signal.SIGTERM)
                        break
            progress.emit("sample", state="draining" if reason else "running", stop_reason=reason,
                aggregate=dict(sampled_rss_bytes=rss, sampled_peak_tree_rss_bytes=peak,
                    observed_cores=None if elapsed is None else delta/elapsed,
                    host_available_bytes=host["available_bytes"], active_jobs=len(active),
                    pending_jobs=len(pending), guard=decision, sampling=diagnostics))
            if active or pending:
                time.sleep(poll_seconds)
    except BaseException as error:
        failure = error
        stop(f"supervisor_exception:{type(error).__name__}:{error}")
    finally:
        # Fatal Python/I/O errors must not orphan any independently created PG.
        # Leaders stay unreaped until every live member is gone (PID reuse safe).
        try:
            for child in active.values():
                send(child, signal.SIGKILL)
            for child in active.values():
                while group_running(child.process.pid):
                    send(child, signal.SIGKILL)
                    time.sleep(0.1)
                child.process.wait()
                child.streams.close()
                by_id[child.job.id]["state"] = "interrupted"
            active.clear()
        finally:
            for sig, handler in previous.items():
                signal.signal(sig, handler)
        for job in pending:
            by_id[job.id]["state"] = "not_started"
    # Hashing/final validation can involve large bundles. Never pause the
    # aggregate RAM sampler for that work while another native group is live.
    # Publish in input order, independent of native completion/slot order.
    finalization_error = None
    for job in jobs:
        if job.id not in clean_finished:
            continue
        try:
            completed(job)
            by_id[job.id]["state"] = "completed"
            if progress is not None:
                progress.emit("output_validated", state="finalizing" if reason is None else "draining")
        except BaseException as error:
            by_id[job.id]["state"] = "failed"
            finalization_error = f"{type(error).__name__}:{error}"
            if failure is None:
                failure = error
            # Preserve an earlier native/resource/operator failure as the
            # primary cause even when subsequent finalization also fails.
            if reason is None:
                reason = f"output_finalization_failed:{job.id}:{finalization_error}"
            break
    result = dict(exit_code=0 if reason is None else None, stop_reason=reason,
        elapsed_seconds=time.monotonic()-started, sampled_peak_tree_rss_bytes=peak,
        owned_groups_drained=True, process_groups=[child.process.pid for child in all_children],
        completed_jobs=[key for key, row in by_id.items() if row["state"] == "completed"],
        completed_sector_checkpoint_only=True, in_sector_resume=False, outcomes=outcomes,
        signal_errors=signal_errors, finalization_error=finalization_error)
    (directory / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    if progress is not None:
        try:
            final_state = ("completed" if reason is None else "failed"
                if failed_native or failure is not None else "stopped")
            progress.emit("finished", state=final_state,
                          stop_reason=reason)
        finally:
            progress.close()
    if failure is not None:
        raise RuntimeError(f"generation scheduler failed after owned-group drain: {directory / 'result.json'}") from failure
    if reason is not None:
        raise RuntimeError(f"generation stopped: {directory / 'result.json'}; completed sectors retained, active sectors restart")
    return result
