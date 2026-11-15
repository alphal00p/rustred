#!/usr/bin/env bash
# Lane allnodes: census of rule-level `domain_bounds` in one table (read-only; C tools + a short python over the targets):
# {table, rules, rules_with_domain_bounds, with_nontrivial_target, examples}. A target is TRIVIAL iff it is I(n0,n1,...,n{N-1})
# (slot i = parameter n_i); anything else (offset, fixed value, permutation) is a non-trivial target pattern.
# Usage: census_bounds.sh <table>
set -u
t=$1
rules=$(LC_ALL=C grep -a -c -E '^\{ ?"type": ?"rule"' -- "$t")
LC_ALL=C grep -a -E '^\{ ?"type": ?"rule"' -- "$t" | LC_ALL=C grep -a -F '"domain_bounds"' \
  | LC_ALL=C grep -a -o -E '"target": ?"I\([^"]*\)"' \
  | python3 -I -c '
import json,re,sys
t,rules=sys.argv[1],int(sys.argv[2])
n=nt=0; ex=[]
for l in sys.stdin:
    tg=re.search(r"I\((.*)\)",l).group(1).split(",")
    n+=1
    if any(x.strip()!=f"n{i}" for i,x in enumerate(tg)):
        nt+=1
        if len(ex)<3: ex.append("I("+",".join(tg)+")")
print(json.dumps({"table":t,"rules":rules,"rules_with_domain_bounds":n,"with_nontrivial_target":nt,"examples":ex}))' "$t" "$rules"
