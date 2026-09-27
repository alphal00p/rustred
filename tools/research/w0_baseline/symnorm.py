"""Strip generic arguments from demangled Rust symbols, keeping qualified-path
brackets (`<T as Trait>::f`, `<Type>::method`), and drop `::h<hash>` suffixes."""
import functools
import re


@functools.lru_cache(maxsize=None)
def normalize(sym):
    sym = re.sub(r"::h[0-9a-f]{16}$", "", sym)
    sym = re.sub(r"\+0x[0-9a-f]+$", "", sym)
    out = []
    i, n = 0, len(sym)
    while i < n:
        c = sym[i]
        if c == "<":
            prev = sym[i - 1] if i else ""
            if prev and (prev.isalnum() or prev in "_:}"):
                depth, j = 1, i + 1
                while j < n and depth:
                    if sym[j] == "-" and j + 1 < n and sym[j + 1] == ">":
                        j += 2
                        continue
                    if sym[j] == "<":
                        depth += 1
                    elif sym[j] == ">":
                        depth -= 1
                    j += 1
                i = j
                if out and out[-1] == ":" and len(out) >= 2 and out[-2] == ":":
                    out.pop(); out.pop()
                continue
        out.append(c)
        i += 1
    return "".join(out)
