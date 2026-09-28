#!/usr/bin/env python3
"""Inclusive/exclusive shares of a harness perf profile, restricted to the
samples taken inside native inspections.

  perf script -i perf.data -F tid,ip,sym --no-inline | profile_categories.py [TOP]

A sample counts as native work when its call stack contains an inspection
entry frame (inspection::inspect / routing::inspect / the applied visitor);
samples of the prepare phase or the harness loop are reported separately.
Shares are of native samples. "Inclusive" counts a sample once per symbol or
category present anywhere in its stack; exclusive counts the leaf only.
Categories are regular expressions over demangled symbol names; they overlap
by construction and must not be summed.
"""
import re
import sys
from collections import Counter

NATIVE = re.compile(r"inspection::inspect|routing::inspect|visit_power_bounded_owner_applied_successors|visit_power_bounded_domain_route_overcover")
CATEGORIES = [
    ("allocator (malloc/free/realloc and arenas)", r"^(malloc|free|realloc|calloc|cfree|_int_malloc|_int_free|_int_realloc|malloc_consolidate|unlink_chunk|tcache|__libc_malloc|__libc_free|__libc_realloc|__rdl_|__rust_alloc|__rust_dealloc|__rust_realloc|mi_|_mi_)|alloc::raw_vec|RawVec.*grow|finish_grow"),
    ("GMP/MPFR", r"^(__gmp|mpfr_|__mpfr)"),
    ("symbolica polynomial GCD", r"symbolica.*(gcd|Gcd|GCD)"),
    ("symbolica polynomial (any)", r"symbolica::poly|symbolica\[[0-9a-f]+\]::poly"),
    ("specialization (algebra::indexed::specialization)", r"indexed::specialization|specialize"),
    ("matching / guards", r"owner_domain_match|matching::|guard"),
    ("applied engine apply_group", r"apply_group"),
    ("applied engine apply_piece", r"apply_piece"),
    ("coefficient classification", r"classify_coefficient|applied::algebra"),
    ("base coefficient system", r"base_coefficient"),
    ("zero locus", r"zero_locus"),
    ("route overcover", r"domain_overcover|route_overcover"),
    ("sorting", r"sort|merge_sort|quicksort|insertion_sort|driftsort"),
    ("hashing / hash maps", r"hashbrown|HashMap|hash::|RawTable"),
    ("memcpy/memmove/memset", r"^(__memmove|__memcpy|__memset|memcpy|memmove|memset)"),
]


def demangle_hint(sym):
    return sym


def main():
    top = int(sys.argv[1]) if len(sys.argv) > 1 else 40
    cats = [(name, re.compile(rx)) for name, rx in CATEGORIES]
    incl = Counter()
    excl = Counter()
    cat_incl = Counter()
    cat_excl = Counter()
    native = other = 0
    depth_hist = Counter()
    stack = []

    def flush():
        nonlocal native, other
        if not stack:
            return
        syms = [s for s in stack]
        if any(NATIVE.search(s) for s in syms):
            native += 1
            depth_hist[min(len(syms) // 10 * 10, 200)] += 1
            for s in set(syms):
                incl[s] += 1
            excl[syms[0]] += 1
            present = set()
            for name, rx in cats:
                if any(rx.search(s) for s in syms):
                    present.add(name)
                if rx.search(syms[0]):
                    cat_excl[name] += 1
            for name in present:
                cat_incl[name] += 1
        else:
            other += 1

    for line in sys.stdin:
        line = line.rstrip("\n")
        if not line.strip():
            flush()
            stack = []
            continue
        if line.startswith("\t"):
            parts = line.strip().split(None, 1)
            if len(parts) == 2:
                sym = parts[1]
                sym = re.sub(r"\+0x[0-9a-f]+$", "", sym)
                sym = re.sub(r" \(.*\)$", "", sym)
                stack.append(sym)
        # header lines (tid) start a new sample; stack collected after
    flush()
    total = max(native, 1)
    print(f"samples: native {native}, other (prepare/harness) {other}")
    print("\n## categories (share of native samples; overlapping, do not sum)")
    for name, _ in CATEGORIES:
        print(f"{100 * cat_incl[name] / total:6.2f}% incl  {100 * cat_excl[name] / total:6.2f}% excl  {name}")
    print(f"\n## top {top} inclusive symbols")
    for sym, n in incl.most_common(top):
        print(f"{100 * n / total:6.2f}%  {sym[:200]}")
    print(f"\n## top {top} exclusive (leaf) symbols")
    for sym, n in excl.most_common(top):
        print(f"{100 * n / total:6.2f}%  {sym[:200]}")
    print("\n## stack depth histogram (frames, bucket of 10)")
    for d in sorted(depth_hist):
        print(f"{d:4d}: {depth_hist[d]}")


if __name__ == "__main__":
    main()
