#!/usr/bin/env bash
# Function ranges for malloc_census.so. Every function of BIN that reaches a C
# heap entry point directly (objdump call or tail-jump sites) is listed in
# callers.tsv with its class: "gmp" (GMP/MPFR memory functions), "other_c"
# (any other unmangled C function) or "rust" (Rust-mangled). The shim needs
# ranges only for gmp and other_c: a caller address outside every range but
# inside the executable is Rust code (Rust's System allocator may tail-jump to
# malloc, so its return address is arbitrary Rust code), and an address
# outside the executable is a shared library.
#   census_ranges.sh BIN OUTDIR  -> OUTDIR/{callers.tsv,gmp.ranges,other_c.ranges}
# Needs binutils (nix develop) and c++filt.
set -euo pipefail
BIN=$1; OUT=$2; mkdir -p "$OUT"
objdump -d --no-show-raw-insn "$BIN" \
  | awk '/^[0-9a-f]+ <.*>:$/ {fn=$2; next}
         /(call|jmp).*<(malloc|calloc|realloc|free|posix_memalign|aligned_alloc|memalign)@(plt|GLIBC[^>]*)>/ {
           kind = ($0 ~ /jmp/) ? "jmp" : "call";
           match($0, /<[a-z_]+@(plt|GLIBC[^>]*)>/); t=substr($0, RSTART+1, RLENGTH-2); sub(/@.*/, "", t);
           print fn "\t" kind "\t" t}' \
  | sort | uniq -c | sort -rn > "$OUT/callers.raw"
nm -S --defined-only "$BIN" > "$OUT/nm.txt"
: > "$OUT/callers.tsv"; : > "$OUT/gmp.ranges"; : > "$OUT/other_c.ranges"
while read -r count fn kind target; do
  sym=${fn#<}; sym=${sym%>:}
  line=$(awk -v s="$sym" '$NF == s && NF == 4 {print $1, $2; exit}' "$OUT/nm.txt")
  [ -z "$line" ] && { printf 'unsized\t%s\t%s\t%s\t-\t-\t%s\n' "$count" "$kind" "$target" "$sym" >> "$OUT/callers.tsv"; continue; }
  read -r addr size <<< "$line"
  case "$sym" in
    __gmp*|mpfr_*|__mpfr*) class=gmp ;;
    _R*|_ZN*) class=rust ;;
    *) class=other_c ;;
  esac
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$class" "$count" "$kind" "$target" "$addr" "$size" "$(echo "$sym" | c++filt)" >> "$OUT/callers.tsv"
  [ "$class" = rust ] || printf '%x:%x\n' "0x$addr" "0x$size" >> "$OUT/$class.ranges"
done < "$OUT/callers.raw"
sort -u -o "$OUT/gmp.ranges" "$OUT/gmp.ranges"
sort -u -o "$OUT/other_c.ranges" "$OUT/other_c.ranges"
echo "gmp=$(paste -sd, "$OUT/gmp.ranges")"
echo "other_c=$(paste -sd, "$OUT/other_c.ranges")"
