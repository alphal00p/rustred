# Forcer golden fixtures

Provenance: Forcer, https://github.com/benruijl/forcer at commit `68a30bf` ("Improved sign matching performance"),
B. Ruijl, T. Ueda, J. A. M. Vermaseren. The Forcer authors permit this use (Ben Ruijl collaborates on the project;
p-integral effort DECISIONS.md, 2026-11-03 09:10, answer to question 4). The files below are data extracted from
Forcer's sources, not Forcer code.

| File | Content | Source |
|---|---|---|
| `forcer_dag_68a30bf.tsv` | the 438 topologies of Forcer's 4L reduction tree: name, L, action, topology, children (triangle / diamond / custom / trivial rows only) | `forcer/pathforcer.h` |
| `forcer_tops_68a30bf.tsv` | the 79 top-level diagrams, L = 2..5 (1 + 3 + 11 + 64) | `generation/pathforcer.py` `createTopLevelDiagrams` |
| `forcer_fixed_68a30bf.tsv` | the 18 fixed topologies of a 4L run | `generation/pathforcer.py` `createFixedTopologies(4)` |
| `graph_keys_p_topology_v1.tsv` | the `GraphKey` of each of the 438 topologies in hex (`P_TOPOLOGY_VERSION` 1) | written by the test `graph_keys_match_golden` |

Topology notation (Forcer): `a-b` is one line, each `*` one eps unit on it (power `n + eps * epsilon`); the lines at
vertex 0 and at the highest vertex are the two external legs (their stars are the eps power of `q`, which is not part
of a topology). The action of a topology is the call in its Reduce procedure (`insertionS`, `carpet`, `triangleS`,
`diamondS`, `#call Reduce` = custom; no internal line = trivial). Children are the `change topology to X` targets of
the block; insertion and carpet blocks hand their rule output over generically, so their child sets are not readable
from the block and are left empty.

`extract.py` regenerates the three Forcer files from a Forcer checkout (`python3 -I extract.py FORCER_ROOT OUT_DIR`);
it uses regular expressions only, no simulator, and is not run in CI.

`graph_keys_p_topology_v1.tsv` pins the key bytes. It fails when the key encoding, `normalise`, or graphica's canonical
labelling changes (for instance a different graphica version: the graphica-skew detector that M2-V4 reuses). A
deliberate change bumps `P_TOPOLOGY_VERSION` and adds `graph_keys_p_topology_v<N>.tsv`, written with
`RUSTRED_TOPOLOGY_BLESS=1 cargo test -p rustred-topology --test forcer_dag graph_keys`.
