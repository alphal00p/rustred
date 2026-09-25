#!/usr/bin/env python3
"""Thin steering for Rust-native independent starting-owner campaigns.

Python writes only the input configuration and optionally execs RustRed. The
native supervisor owns scheduling, RAM protection, checkpointing, publication,
combined output and the terminal/JSON monitor. No algebra or monitoring loop
lives in this adapter. Without --start, no native process is launched.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shlex


def positive(text):
    value = int(text)
    if value <= 0:
        raise argparse.ArgumentTypeError("must be positive")
    return value


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--executable", type=Path, default=Path("target/release/rustred"))
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--queries", type=Path)
    parser.add_argument("--owner-base", type=Path)
    parser.add_argument("--config-output", type=Path)
    parser.add_argument("--shards", type=positive,
                        help="optional root grouping; default one queued job per starting owner")
    parser.add_argument("--jobs", type=positive)
    parser.add_argument("--workers-per-job", type=positive)
    parser.add_argument("--total-workers", type=positive)
    parser.add_argument("--cpus", help="comma-separated physical CPU IDs; default permitted affinity")
    parser.add_argument("--max-memory-bytes", type=positive)
    parser.add_argument("--checkpoint-interval-seconds", type=positive)
    parser.add_argument("--publication-policy", choices=("ordered", "ready"))
    parser.add_argument("--route-joint-source-support-pruning", action="store_true")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--start", action="store_true")
    args = parser.parse_args(argv)
    executable = args.executable.resolve()
    directory = args.directory.resolve()
    command = [str(executable), "campaign", "shards", "--directory", str(directory)]
    if args.resume:
        if any((args.manifest, args.queries, args.owner_base, args.config_output)):
            parser.error("resume uses frozen native inputs; do not supply new input/config paths")
        if any(value is not None for value in (
            args.shards, args.jobs, args.workers_per_job, args.total_workers, args.cpus,
            args.max_memory_bytes, args.checkpoint_interval_seconds, args.publication_policy,
        )) or args.route_joint_source_support_pruning:
            parser.error("resume uses frozen scheduling and pruning; do not supply policy overrides")
        command.append("--resume")
    else:
        for name, default in {
            "jobs": 10, "workers_per_job": 5, "total_workers": 50,
            "max_memory_bytes": 500_000_000_000, "checkpoint_interval_seconds": 3600,
            "publication_policy": "ordered",
        }.items():
            if getattr(args, name) is None:
                setattr(args, name, default)
        if not all((args.manifest, args.queries, args.owner_base)):
            parser.error("a new campaign requires --manifest, --queries and --owner-base")
        if args.jobs * args.workers_per_job > args.total_workers:
            parser.error("jobs times workers-per-job exceeds total-workers")
        try:
            cpus = ([int(x) for x in args.cpus.split(",")] if args.cpus else
                    sorted(os.sched_getaffinity(0))[:args.total_workers])
        except (ValueError, AttributeError) as error:
            parser.error(f"invalid/unavailable CPU affinity: {error}")
        if len(set(cpus)) != len(cpus) or len(cpus) < args.total_workers or any(x < 0 for x in cpus):
            parser.error("CPU IDs must be unique, nonnegative and cover total-workers")
        native_options = ["--bounded-refinement-axes", "finite-axes",
                          "--max-guard-univariate-degree", "64",
                          "--route-domain-overcover", "--transfer-unreserved-lookahead", "256",
                          "--reuse-initial-d-bands"]
        if args.route_joint_source_support_pruning:
            native_options.append("--route-joint-source-support-pruning")
        config = {
            "schema": "rustred.independent-root-config.v1",
            "manifest": str(args.manifest.resolve()),
            "queries": str(args.queries.resolve()),
            "owner_base": str(args.owner_base.resolve()),
            "jobs": args.jobs,
            "workers_per_job": args.workers_per_job, "total_workers": args.total_workers,
            "cpus": cpus, "max_memory_bytes": args.max_memory_bytes,
            "host_reserve_bytes": 20_000_000_000,
            "checkpoint_interval_seconds": args.checkpoint_interval_seconds,
            "publication_policy": args.publication_policy,
            "native_options": native_options,
        }
        if args.shards is not None:
            config["shards"] = args.shards
        path = (args.config_output or directory.with_suffix(".config.json")).resolve()
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("x", encoding="utf-8") as stream:
            json.dump(config, stream, indent=2, allow_nan=False)
            stream.write("\n")
        command += ["--config", str(path)]
        print(f"Prepared native configuration: {path}", flush=True)
    print(shlex.join(command), flush=True)
    print("Read-only monitor: " + shlex.join(
        [str(executable), "campaign", "monitor", "--directory", str(directory)]), flush=True)
    if args.start:
        os.execv(str(executable), command)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
