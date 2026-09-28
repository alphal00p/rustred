#!/usr/bin/env python
"""Caller attribution of `perf record --call-graph fp|dwarf` samples.

Frames are read with `perf script -F tid,ip,sym,symoff,dso --no-demangle`.
Frames in the rustred executable are expanded into their inline chains with
one batched `addr2line -i -f -C` call on static addresses (nm symbol address +
symoff; return addresses minus 1 for caller frames), because the coordinator
loop inlines most of its duty helpers (ready_service, save, ReadyStreams::poll,
commit_prepared) into `execution::run_pool` and `Engine::commit_chunk`.
Symbols are compared after stripping generic arguments (symnorm.normalize).

Coordinator role: every sample is mapped to one coordinator-duty bucket by the
first (leaf-most) expanded frame that matches a bucket anchor, i.e. the
functions the native duty timers wrap (walking/execution.rs run loop,
execution/admission.rs).  Refinements keep it aligned with the timers:
  * dispatch below ready_service counts as ready_service (ready service
    includes its own dispatch);
  * publish_delegated / commit_delegated below dispatch count as publication
    (the dispatch timer subtracts nested publication), which first-match
    already gives.
A sample is "named" when at least one frame is a symbolized rustred frame; it
is "attributed" when a bucket matched.  The report also lists, for samples
whose leaf is in libc or the allocator, the nearest symbolized rustred caller,
and the inclusive (children) share of every normalized function.

Usage: perf_attribution.py DATA --perf PERF [--role coordinator|worker] [--tid T ...]
"""
import argparse
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from symnorm import normalize  # noqa: E402

NM = "/nix/store/j5rd8xm5zqgzcb0l19942ms7r96q0hip-binutils-wrapper-2.46/bin/nm"
def _find_symbolizer():
    """$LLVM_SYMBOLIZER, else the original llvm-22.1.8 store path, else the newest
    llvm-*/bin/llvm-symbolizer left in /nix/store (the 22.1.8 path was garbage-collected
    by 2026-09-28), else llvm-symbolizer on PATH."""
    import glob
    import os
    import shutil
    env = os.environ.get("LLVM_SYMBOLIZER")
    if env:
        return env
    original = "/nix/store/15knsirbfh6w3wfabmh1w2z063qq6gn2-llvm-22.1.8/bin/llvm-symbolizer"
    if os.path.exists(original):
        return original
    found = sorted(glob.glob("/nix/store/*-llvm-[0-9]*/bin/llvm-symbolizer"),
                   key=lambda p: [int(x) for x in re.findall(r"llvm-(\d+)\.(\d+)\.(\d+)", p)[0]])
    return found[-1] if found else (shutil.which("llvm-symbolizer") or original)


SYMBOLIZER = _find_symbolizer()
W = r"rustred_app::application::routed_campaign::walking::"
# inline frames: "short_name @ crates/.../walking/<file>"
F = r" @ crates/rustred-app/src/application/routed_campaign/walking/"
BUCKET_RULES = [
    # (bucket, regex on the normalized, file-qualified symbol); the leaf-most match wins
    ("ready_service", r"^ready_service" + F + r"execution\.rs|" + W + r"execution::ready_service\b"),
    ("closure_refresh", r"^refresh" + F + r"execution\.rs|^refresh_closure @|::State>::refresh_closure\b|"
                        r"descendant_closure::Tracker>::scan|^scan" + F + r"descendant_closure"),
    ("checkpoint", r"^save" + F + r"(execution|checkpoint)\.rs|checkpoint::Store>::save|checkpoint::Store>::publish"),
    ("progress_json", r"^observe" + F + r"execution\.rs|" + W + r"execution::observe\b|execution::State>::progress\b|"
                      r"^progress" + F + r"execution\.rs"),
    # State::set_parallel{,_lean}: charged to publication on the Finished path and
    # untimed on the Events path (execution.rs run_pool); reported separately.
    ("set_parallel_telemetry", r"^set_parallel\w* @|State>::set_parallel"),
    ("publication", r"^publish_delegated" + F + r"execution\.rs|execution::publish_delegated\b|"
                    r"^commit_physical\w* @|State>::commit_physical|^commit_delegated\w* @|State>::commit_delegated|"
                    r"^activate_stream @|State>::activate_stream|^finished" + F + r"execution/publication\.rs"),
    ("preparation", r"^prepare_with_replay @|Engine>::prepare_with_replay|^prepare" + F + r"execution/admission\.rs|"
                    r"rayon"),
    ("ordered_commit", r"^commit_prepared @|Engine>::commit_prepared|^commit" + F + r"execution/admission\.rs|"
                       r"queue::Queue>::admit_prepared|queue::Queue>::admit_with_lookup|^admit_prepared @|^admit_with_lookup @"),
    ("dispatch", r"execution::Dispatcher>::run\b|^run" + F + r"execution\.rs"),
    ("poll", r"^poll" + F + r"(execution/publication|parallel)\.rs|parallel::Pool>::poll\b|reclaim_(all_)?finished|"
             r"Pool>::reclaim"),
    ("wait", r"^wait" + F + r"(execution/publication|parallel)\.rs|parallel::Pool>::wait\b"),
    ("commit_chunk_other", r"Engine>::commit_chunk|^commit_chunk @"),
    ("coordinator_loop_other", r"execution::run_pool\b|^run_pool @|with_ticket_pool"),
]
COMPILED = [(name, re.compile(rx)) for name, rx in BUCKET_RULES]
# Attribution classes named by the coordinator relief design note
# (docs/research/fable51_coordinator_relief_design_2026-09-26.md, section 5 M1),
# as inclusive shares (a sample counts once per class if any frame matches).
RELIEF_CLASSES = [
    ("siphash_hashbrown_probe", r"hashbrown|::sip::|SipHasher|DefaultHasher|RandomState|HashMap"),
    ("is_live_partition_point", r"is_live|partition_point"),
    ("domain_summary_contains", r"compact::Stored>::contain|CompactSummary>::contains|CompactDomain>::contains|"
                                r"DomainPowerSummary|power_domain::geometry|^contain(s|ed_by) @"),
    ("bit_prefilter_may_contain", r"^may_contain @ .*queue/bits\.rs|queue::bits::"),
    ("index_blocks_scan", r"queue/index/blocks\.rs|index::blocks::"),
    ("find_from_retire", r"find_from|AggregateIndex>::retire|^retire @|retire_prepared|find_controlled|AggregateIndex>::find\b"),
    ("memcpy_memmove", r"^__mem(move|cpy)|^mem(move|cpy)"),
    ("malloc", r"^_int_malloc|^malloc|__libc_malloc|^__rdl_alloc|^alloc @ std/alloc/src/alloc\.rs|^realloc|_int_realloc"),
    ("free", r"^_int_free|^cfree|^free|__libc_free|^__rdl_dealloc|^dealloc|malloc_consolidate"),
    ("tracker_edge", r"descendant_closure::Tracker>::edge|^edge @ .*descendant_closure"),
    ("blake3_replay", r"blake3|walking::execution::replay|execution/replay\.rs"),
    ("serde_json_value", r"serde_json|BTreeMap|btree::"),
    ("futex_sync", r"futex|syscall|Condvar|Mutex|parking_lot|lock_contended"),
]
RELIEF_COMPILED = [(name, re.compile(rx)) for name, rx in RELIEF_CLASSES]
LIBC_ALLOC = re.compile(r"^(_int_|malloc|free|cfree|realloc|calloc|__mem|mem(cpy|move|set|cmp)|__libc_|"
                        r"unlink_chunk|tcache|__rdl_|__rust_(alloc|dealloc|realloc)|alloc::alloc::)")


def is_rustred(dso):
    return "rustred" in Path(dso).name


class Symbolizer:
    """mangled -> (demangled, static address) for one executable, plus inline chains."""

    def __init__(self, binary):
        self.binary = binary
        raw = subprocess.run([NM, "--no-sort", "--defined-only", binary], capture_output=True, text=True).stdout
        dem = subprocess.run([NM, "--no-sort", "--defined-only", "-C", binary], capture_output=True, text=True).stdout
        self.addr = {}
        self.demangled = {}
        for a, b in zip(raw.splitlines(), dem.splitlines()):
            pa, pb = a.split(" ", 2), b.split(" ", 2)
            if len(pa) == 3 and len(pb) == 3:
                self.addr[pa[2]] = int(pa[0], 16)
                self.demangled[pa[2]] = pb[2]
        self.inline = {}

    def resolve(self, addresses):
        """Inline chains via llvm-symbolizer (JSON), innermost first.  Inlined
        frames carry only short DWARF names under line-tables-only, so each name
        is qualified with the source file of its own location: "name @ path"."""
        todo = sorted({a for a in addresses if a not in self.inline})
        if not todo:
            return
        res = subprocess.run([SYMBOLIZER, f"--obj={self.binary}", "--inlines", "--functions=linkage",
                              "--demangle", "--output-style=JSON"],
                             input="\n".join(hex(a) for a in todo) + "\n", capture_output=True, text=True)
        for line in res.stdout.splitlines():
            try:
                doc = json.loads(line)
            except ValueError:
                continue
            chain = []
            for frame in doc.get("Symbol", []):
                name = frame.get("FunctionName") or "??"
                path = frame.get("FileName") or ""
                path = re.sub(r"^.*/crates/", "crates/", path)
                path = re.sub(r"^/rustc/[0-9a-f]+/library/", "std/", path)
                path = re.sub(r"^.*/vendor/", "vendor/", path)
                chain.append(f"{name} @ {path}")
            self.inline[int(doc["Address"], 16)] = chain


def read_samples(perf, data, tids=None):
    base = [perf, "script", "-i", str(data), "-F", "tid,ip,sym,symoff,dso", "--no-demangle"]
    if tids:
        base += ["--tid", ",".join(map(str, tids))]
    res = subprocess.run(base + ["--no-inline"], capture_output=True, text=True, errors="replace")
    if res.returncode != 0 and not res.stdout:
        res = subprocess.run(base, capture_output=True, text=True, errors="replace")
    samples = []
    current = None
    frame_rx = re.compile(r"\s*([0-9a-f]+)\s+(.*?)(\+0x([0-9a-f]+))?\s+\((.*)\)\s*$")
    for line in res.stdout.splitlines():
        if not line.strip():
            if current is not None:
                samples.append(current)
            current = None
            continue
        if not line[:1].isspace() or current is None:
            if current is not None:
                samples.append(current)
            head = line.split()
            current = {"tid": head[0] if head else "?", "frames": []}
            continue
        m = frame_rx.match(line)
        if m:
            off = int(m.group(4), 16) if m.group(4) else None
            current["frames"].append((m.group(2), off, m.group(5)))
    if current is not None:
        samples.append(current)
    return samples, res.stderr[-2000:]


def expand(samples, symbolizer_cache):
    """Replace (mangled, off, dso) frames by lists of normalized names (inline chain, innermost first)."""
    wanted = {}
    for s in samples:
        for depth, (sym, off, dso) in enumerate(s["frames"]):
            if is_rustred(dso) and sym != "[unknown]" and off is not None:
                if dso not in symbolizer_cache:
                    symbolizer_cache[dso] = Symbolizer(dso) if Path(dso).exists() else None
                sz = symbolizer_cache[dso]
                if sz and sym in sz.addr:
                    wanted.setdefault(dso, set()).add(sz.addr[sym] + off - (1 if depth else 0))
    for dso, addrs in wanted.items():
        symbolizer_cache[dso].resolve(addrs)
    memo = {}
    for s in samples:
        out = []
        for depth, (sym, off, dso) in enumerate(s["frames"]):
            key = (sym, off, dso, depth > 0)
            names = memo.get(key)
            if names is None:
                sz = symbolizer_cache.get(dso) if is_rustred(dso) else None
                if sz and sym in sz.addr and off is not None:
                    chain = sz.inline.get(sz.addr[sym] + off - (1 if depth else 0))
                    if chain and not chain[0].startswith("??"):
                        names = [normalize(c) for c in chain]
                    if not names:
                        names = [normalize(sz.demangled.get(sym, sym))]
                if names is None:
                    names = [normalize(sym)]
                memo[key] = names
            out.append((names, dso))
        s["expanded"] = out
    return samples


SNAPSHOT = re.compile(r"parallel::Pool>::snapshot|^snapshot\w*" + F + r"parallel\.rs")


def bucket_of(expanded):
    flat = [n for names, _ in expanded for n in names]
    for i, name in enumerate(flat):
        for bucket, rx in COMPILED:
            if rx.search(name):
                if bucket == "dispatch" and any(COMPILED[0][1].search(x) for x in flat[i + 1:]):
                    return "ready_service", name
                if bucket in ("coordinator_loop_other", "commit_chunk_other"):
                    # Pool::snapshot_lean() is evaluated in the loop as the
                    # argument of State::set_parallel_lean, so that frame is absent.
                    snap = next((x for x in flat[:i] if SNAPSHOT.search(x)), None)
                    if snap:
                        return "set_parallel_telemetry", snap
                return bucket, name
    return None, None


def short(sym, width=150):
    return sym.replace("rustred_app::application::routed_campaign::walking::", "W::")[:width]


_CACHE = {}


def attribute(perf, data, role="coordinator", top=30, tids=None, cache=None):
    samples, stderr = read_samples(perf, data, tids)
    n = len(samples)
    out = {"samples": n, "perf_script_stderr_tail": stderr[-400:] if n == 0 else ""}
    if n == 0:
        return out
    samples = expand(samples, cache if cache is not None else _CACHE)
    named = sum(1 for s in samples if any(is_rustred(d) and names[0] != "[unknown]" for names, d in s["expanded"]))
    out["named_share"] = named / n
    out["mean_frames"] = sum(len(s["frames"]) for s in samples) / n
    out["single_frame_share"] = sum(1 for s in samples if len(s["frames"]) <= 1) / n
    out["unknown_leaf_share"] = sum(1 for s in samples if not s["frames"] or s["frames"][0][0] == "[unknown]") / n
    dso = Counter(Path(s["frames"][0][2]).name if s["frames"] else "?" for s in samples)
    out["leaf_dso_shares"] = {k: round(v / n, 4) for k, v in dso.most_common(8)}
    inclusive, leaf, nearest_rust_for_libc = Counter(), Counter(), Counter()
    leaf_libc = 0
    for s in samples:
        seen = set()
        for names, _ in s["expanded"]:
            for name in names:
                key = short(name)
                if key not in seen:
                    seen.add(key)
                    inclusive[key] += 1
        if s["expanded"]:
            leaf[short(s["expanded"][0][0][0])] += 1
            first = s["expanded"][0][0][-1]
            if LIBC_ALLOC.search(first) or "libc" in s["expanded"][0][1]:
                leaf_libc += 1
                caller = next((short(names[0]) for names, d in s["expanded"][1:] if is_rustred(d)
                               and names[0] != "[unknown]" and not LIBC_ALLOC.search(names[-1])),
                              "[no named rustred caller]")
                nearest_rust_for_libc[caller] += 1
    out["leaf_libc_or_alloc_share"] = leaf_libc / n
    out["libc_alloc_nearest_rust_caller"] = [[round(100 * c / n, 2), k] for k, c in nearest_rust_for_libc.most_common(top)]
    out["self_top"] = [[round(100 * c / n, 2), k] for k, c in leaf.most_common(top)]
    out["inclusive_top"] = [[round(100 * c / n, 2), k] for k, c in inclusive.most_common(top + 20)]
    if role == "coordinator":
        buckets, anchors, unattributed = Counter(), Counter(), Counter()
        for s in samples:
            b, sym = bucket_of(s["expanded"])
            buckets[b or "unattributed"] += 1
            if b:
                anchors[(b, short(sym, 110))] += 1
            else:
                chain = " <- ".join(short(names[0], 70) for names, _ in s["expanded"][:4])
                unattributed[chain] += 1
        relief_incl, relief_self = Counter(), Counter()
        for smp in samples:
            flat = [x for names, _ in smp["expanded"] for x in names]
            for cls, rx in RELIEF_COMPILED:
                if any(rx.search(x) for x in flat):
                    relief_incl[cls] += 1
                if flat and rx.search(flat[0]):
                    relief_self[cls] += 1
        out["relief_classes_inclusive"] = {k: round(relief_incl[k] / n, 4) for k, _ in RELIEF_CLASSES}
        out["relief_classes_self"] = {k: round(relief_self[k] / n, 4) for k, _ in RELIEF_CLASSES}
        out["bucket_shares"] = {k: round(v / n, 4) for k, v in buckets.most_common()}
        # Traversal-loop view: only samples below execution::run_pool (excludes owner
        # loading, restore, final report serialization in whole-run profiles).
        loop_rx = re.compile(W + r"execution::run_pool\b|^run_pool @")
        loop_buckets = Counter()
        for smp in samples:
            if any(loop_rx.search(x) for names, _ in smp["expanded"] for x in names):
                loop_buckets[bucket_of(smp["expanded"])[0] or "unattributed"] += 1
        m = sum(loop_buckets.values())
        out["loop_samples_share"] = m / n
        if m:
            out["loop_bucket_shares"] = {k: round(v / m, 4) for k, v in loop_buckets.most_common()}
            out["loop_duty_bucket_share"] = sum(v for k, v in loop_buckets.items()
                                                if k not in ("unattributed", "coordinator_loop_other",
                                                             "commit_chunk_other")) / m
        out["attributed_share"] = 1 - buckets.get("unattributed", 0) / n
        # Gate 0.5 "duty bucket" share: samples in a named duty bucket, i.e. excluding
        # the loop and commit_chunk bodies outside the timers and unattributed samples.
        out["duty_bucket_share"] = sum(v for k, v in buckets.items()
                                       if k not in ("unattributed", "coordinator_loop_other",
                                                    "commit_chunk_other")) / n
        out["top_anchors"] = [[round(100 * c / n, 2), b, s] for (b, s), c in anchors.most_common(top)]
        out["top_unattributed_chains"] = [[round(100 * c / n, 2), k] for k, c in unattributed.most_common(15)]
    else:
        first_rust = Counter()
        for s in samples:
            name = next((short(names[0], 110) for names, d in s["expanded"] if is_rustred(d)
                         and names[0] != "[unknown]" and not LIBC_ALLOC.search(names[-1])), "[none]")
            first_rust[name] += 1
        out["nearest_named_rust_frame"] = [[round(100 * c / n, 2), k] for k, c in first_rust.most_common(top)]
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument("data")
    p.add_argument("--perf", required=True)
    p.add_argument("--role", default="coordinator")
    p.add_argument("--tid", action="append", default=[])
    args = p.parse_args()
    print(json.dumps(attribute(args.perf, args.data, args.role, tids=args.tid or None), indent=1))


if __name__ == "__main__":
    main()
