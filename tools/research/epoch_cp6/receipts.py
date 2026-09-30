"""Small-file identity and read-only checkpoint checks; native authenticates payloads."""
import hashlib
import importlib.util
import json
from pathlib import Path
import stat

from contract import CONTRACT, FORMAT, checkpoint_schema, clean_collection, expected_schedule, require, walk_semantics

ROOT = Path(__file__).resolve().parents[3]


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


ROLES = load_module("_cp6_roles", ROOT / "examples/python/owner_query_roles.py")


def read(path, limit=4 << 20):
    path = Path(path)
    require(path.stat().st_size <= limit, f"bounded JSON exceeded: {path}")
    return ROLES.loads_document(path.read_text())


def sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_new(path, value):
    with Path(path).open("x") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")


def argv_value(argv, name):
    require(argv.count(name) == 1, f"exactly one {name} required")
    index = argv.index(name)
    require(index + 1 < len(argv), f"missing {name} value")
    return argv[index + 1]


def validate_plan(plan):
    require(plan["contract"] == CONTRACT and plan["mode"] in ("all-miss", "snapshot")
            and type(plan["b"]) is int and 1 <= plan["b"] <= 4096, "unknown plan/mode/B")
    schedule = expected_schedule(plan)
    checkpoint_schema(plan)
    g2 = plan.get("g2", "off")
    require(g2 in ("off", "union"), "unknown G2 control")
    for key in ("run", "checkpoint", "queries", "binary", "native_cwd", "verification_cwd"):
        require(Path(plan[key]).is_absolute(), f"absolute {key} required")
    for key in ("queries_sha256", "queries_blake3", "binary_sha256"):
        require(len(plan[key]) == 64 and all(c in "0123456789abcdef" for c in plan[key]), f"invalid {key}")
    require(sha(plan["queries"]) == plan["queries_sha256"], "changed frozen queries")
    document = read(plan["queries"])
    roles = ROLES.query_roles(document, require_explicit=True)
    expected = dict(plan, roles={"total": len(roles), "required": sum(v == "required" for v in roles.values()),
                                 "auxiliary": sum(v == "auxiliary" for v in roles.values())})
    require(expected["roles"]["total"] > 0 and expected["roles"]["required"] > 0, "empty scope")
    argv = plan["native_argv"]
    require(isinstance(argv, list) and argv and argv[0] == plan["binary"]
            and all(isinstance(s, str) for s in argv), "native argv")
    for option, value in (("--queries", plan["queries"]), ("--checkpoint", plan["checkpoint"]),
                          ("--output", str(Path(plan["run"]) / "result.json")),
                          ("--publication-policy", "epoch"), ("--epoch-inspector-lookup", plan["mode"])):
        require(argv_value(argv, option) == value, f"native {option} mismatch")
    require("--resume" not in argv and "--follow-successors" in argv, "fresh symbolic arms only")
    for option in ("--epoch-rolling", "--epoch-dispatch", "--g2-residual-anchors",
                   "--epoch-publication-order", "--epoch-cut-size", "--epoch-window",
                   "--epoch-result-escrow-jobs", "--epoch-result-escrow-bytes"):
        require(not any(arg.startswith(option + "=") for arg in argv), f"use exact {option} argv")
    rolling = schedule["kind"] == "rolling"
    require(argv.count("--epoch-rolling") == int(rolling), "native rolling policy mismatch")
    dispatch = schedule.get("dispatch", "fifo")
    if dispatch == "adaptive" or "--epoch-dispatch" in argv:
        require(argv_value(argv, "--epoch-dispatch") == dispatch, "native dispatch policy mismatch")
    publication = schedule.get("publication_order", "oldest_sequence_prefix")
    native_publication = {"oldest_sequence_prefix": "oldest-prefix", "oldest_ready_sequences": "oldest-ready"}[publication]
    if publication != "oldest_sequence_prefix" or "--epoch-publication-order" in argv:
        require(rolling and argv_value(argv, "--epoch-publication-order") == native_publication,
                "native publication order mismatch")
    if "--epoch-cut-size" in argv:
        text = argv_value(argv, "--epoch-cut-size")
        require(text.isascii() and text.isdecimal() and 1 <= int(text) <= 4096,
                "invalid native cut size")
        # Inline W1's automatic window can be smaller than the requested cut;
        # the native summary reports the effective clamped cut.
        require(rolling and min(int(text), schedule.get("window", plan["b"])) == schedule["cut_size"],
                "native cut size mismatch")
    if "--epoch-window" in argv:
        require(rolling and argv_value(argv, "--epoch-window") == str(schedule.get("window", plan["b"])),
                "native window mismatch")
    extra = schedule.get("result_escrow_jobs", 0)
    if extra:
        require(argv_value(argv, "--epoch-result-escrow-jobs") == str(extra)
                and argv_value(argv, "--epoch-result-escrow-bytes") == str(schedule["result_escrow_bytes"]),
                "native escrow policy mismatch")
    else:
        require("--epoch-result-escrow-bytes" not in argv, "unregistered escrow byte policy")
        if "--epoch-result-escrow-jobs" in argv:
            require(argv_value(argv, "--epoch-result-escrow-jobs") == "0",
                    "unregistered escrow logical capacity")
    if g2 == "union":
        require(argv_value(argv, "--g2-residual-anchors") == g2, "native G2 policy mismatch")
    else:
        require("--g2-residual-anchors" not in argv, "Off runner must omit native G2 flag")
    return expected


def metadata(path):
    st = path.lstat()
    require(stat.S_ISREG(st.st_mode), f"nonregular checkpoint entry: {path}")
    return [st.st_dev, st.st_ino, st.st_mode, st.st_size, st.st_mtime_ns, st.st_ctime_ns]


def checkpoint_snapshot(directory):
    directory = Path(directory)
    require(directory.is_dir() and not directory.is_symlink(), "real checkpoint directory required")
    require(not (directory / "epoch-poison").exists(), "poisoned checkpoint")
    # File metadata detects cold writes/replacements without rehashing large
    # payloads. The raw oracle authenticates every consumed payload itself.
    entries = {p.name: metadata(p) for p in sorted(directory.iterdir())}
    small = {}
    for name in ("latest.json", "previous.json", "epoch-session.bin", "checkpoint.lock"):
        path = directory / name
        if not path.exists():
            require(name == "previous.json", f"missing {name}")
            small[name] = None
        else:
            require(path.stat().st_size <= 64 << 10, f"oversized authority file {name}")
            small[name] = sha(path)
    manifest = read(directory / "latest.json", 64 << 10)
    return {"entries": entries, "authority_sha256": small, "latest": manifest}


def freeze(plan):
    expected = validate_plan(plan)
    run = Path(plan["run"])
    require(read(run / "command.json") == plan["native_argv"], "actual native argv differs")
    metrics, summary = read(run / "metrics.json"), read(run / "result.json")
    clean_collection(metrics, summary, expected)
    snapshot = checkpoint_snapshot(plan["checkpoint"])
    envelope = snapshot["latest"]
    manifest = envelope["manifest"]
    require(manifest["format"] == FORMAT and type(manifest["schema"]) is int
            and manifest["schema"] == checkpoint_schema(plan)
            and type(manifest["walk_semantics_version"]) is int
            and manifest["walk_semantics_version"] == walk_semantics(plan)
            and manifest["resumable"] is True and type(manifest["generation"]) is int
            and manifest["generation"] == summary["checkpoint"]["generation"], "manifest identity differs")
    require(summary["checkpoint"]["manifest"] == str(Path(plan["checkpoint"]) / "latest.json"),
            "summary manifest path differs")
    digest = envelope["blake3"]
    require(len(digest) == 32 and all(type(b) is int and 0 <= b <= 255 for b in digest), "manifest digest shape")
    # Compare the writer's authenticated digest; do not reimplement canonical BLAKE3.
    require(bytes(digest).hex() == summary["checkpoint"]["manifest_blake3"], "summary manifest digest differs")
    return {"contract": CONTRACT, "expected": expected, "checkpoint": snapshot,
            "run_files": {name: sha(run / name) for name in ("command.json", "metrics.json", "result.json")}}


def commands(plan):
    run = Path(plan["run"])
    cold = [plan["binary"], "walk-verify-closure", "--command", str(run / "command.json"),
            "--checkpoint", plan["checkpoint"], "--no-result", "--require-closure", "--reinspect", "all",
            "--certification-scope", "all-roots", "--reference-levers", "off", "--threads", str(plan["verify_threads"]),
            "--output", str(run / "cold-verify.json")]
    audit = [plan["python"], "-B", str(ROOT / "examples/python/audit_owner_domain_walk.py"),
             str(run), "--require-closure", "--output", str(run / "cold-audit.json")]
    # Use these exact arrays inside the existing guard_build request, with
    # explicit bounded timeout. This module never launches either command.
    prefix = ["timeout", "--signal=INT", "--kill-after=60", str(plan["verification_timeout"])]
    return {"cold-verifier": prefix + cold, "python-audit": prefix + audit}
