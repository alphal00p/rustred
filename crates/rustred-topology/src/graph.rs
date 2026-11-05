//! Small-graph helpers on vertex bitmasks (at most 64 vertices and 64 independent cycles; propagator topologies up
//! to seven loops have at most 16 vertices and 8 cycles in the glued graph).

/// Neighbour bitmasks of an undirected multigraph without self-loops (multiplicity is irrelevant here).
pub(crate) fn adjacency(n: usize, edges: &[(usize, usize)]) -> Vec<u64> {
    assert!(n <= 64, "rustred-topology: more than 64 vertices");
    let mut adj = vec![0u64; n];
    for &(u, v) in edges {
        if u != v {
            adj[u] |= 1 << v;
            adj[v] |= 1 << u;
        }
    }
    adj
}

/// Vertices reachable from `start` without entering the vertices of `blocked`.
fn reach(adj: &[u64], start: usize, blocked: u64) -> u64 {
    let mut seen = 1u64 << start;
    let mut frontier = seen;
    while frontier != 0 {
        let mut next = 0u64;
        let mut f = frontier;
        while f != 0 {
            let x = f.trailing_zeros() as usize;
            f &= f - 1;
            next |= adj[x];
        }
        next &= !seen & !blocked;
        seen |= next;
        frontier = next;
    }
    seen
}

fn all_vertices(n: usize) -> u64 {
    if n == 64 { u64::MAX } else { (1u64 << n) - 1 }
}

/// Is the graph on vertices `0..n` connected and free of cut vertices (2-connected; graphs with at most two
/// vertices only need to be connected)? Self-loops must have been excluded by the caller.
pub(crate) fn biconnected(n: usize, edges: &[(usize, usize)]) -> bool {
    if n == 0 {
        return false;
    }
    let adj = adjacency(n, edges);
    let all = all_vertices(n);
    if reach(&adj, 0, 0) != all {
        return false;
    }
    if n <= 2 {
        return true;
    }
    (0..n).all(|v| {
        let start = if v == 0 { 1 } else { 0 };
        reach(&adj, start, 1 << v) == all & !(1u64 << v)
    })
}

/// Are `a` and `b` connected after deleting the edges of `skip`?
pub(crate) fn connected_without_edges(
    n: usize,
    edges: &[(usize, usize)],
    skip: &[usize],
    a: usize,
    b: usize,
) -> bool {
    let kept: Vec<(usize, usize)> = edges
        .iter()
        .enumerate()
        .filter(|(i, _)| !skip.contains(i))
        .map(|(_, &e)| e)
        .collect();
    let adj = adjacency(n, &kept);
    reach(&adj, a, 0) & (1 << b) != 0
}

/// Cycle-space incidence of every edge: bit `k` is set iff the edge lies on the fundamental cycle of the `k`-th
/// non-tree edge of a spanning forest. In a 2-edge-connected graph, edges `i != j` form a 2-edge cut iff their masks
/// are equal (the indicator of `{i, j}` is orthogonal to every cycle); a bridge has mask 0.
pub(crate) fn cycle_masks(n: usize, edges: &[(usize, usize)]) -> Vec<u64> {
    let mut incident: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    for (i, &(u, v)) in edges.iter().enumerate() {
        incident[u].push((v, i));
        if u != v {
            incident[v].push((u, i));
        }
    }
    let mut parent: Vec<Option<(usize, usize)>> = vec![None; n]; // (parent vertex, tree edge)
    let mut depth = vec![usize::MAX; n];
    let mut tree = vec![false; edges.len()];
    for root in 0..n {
        if depth[root] != usize::MAX {
            continue;
        }
        depth[root] = 0;
        let mut stack = vec![root];
        while let Some(x) = stack.pop() {
            for &(y, i) in &incident[x] {
                if depth[y] == usize::MAX {
                    depth[y] = depth[x] + 1;
                    parent[y] = Some((x, i));
                    tree[i] = true;
                    stack.push(y);
                }
            }
        }
    }
    let mut masks = vec![0u64; edges.len()];
    let mut bit = 0usize;
    for (i, &(u, v)) in edges.iter().enumerate() {
        if tree[i] {
            continue;
        }
        assert!(
            bit < 64,
            "rustred-topology: more than 64 independent cycles"
        );
        let b = 1u64 << bit;
        bit += 1;
        masks[i] |= b;
        let (mut x, mut y) = (u, v);
        while x != y {
            if depth[x] < depth[y] {
                std::mem::swap(&mut x, &mut y);
            }
            let (p, e) = parent[x].expect("cycle_masks: endpoints in one tree");
            masks[e] |= b;
            x = p;
        }
    }
    masks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biconnectivity_of_small_graphs() {
        // triangle: 2-connected; path: cut vertex 1; two triangles sharing vertex 2: cut vertex
        assert!(biconnected(3, &[(0, 1), (1, 2), (2, 0)]));
        assert!(!biconnected(3, &[(0, 1), (1, 2)]));
        assert!(!biconnected(
            5,
            &[(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 2)]
        ));
        assert!(biconnected(2, &[(0, 1)]));
        assert!(!biconnected(2, &[]));
    }

    #[test]
    fn cycle_masks_find_two_edge_cuts() {
        // a 4-cycle: all four edges pairwise form 2-edge cuts; a chord splits them into two pairs
        let square = [(0, 1), (1, 2), (2, 3), (3, 0)];
        let m = cycle_masks(4, &square);
        assert!(m.iter().all(|&x| x == m[0] && x != 0));
        let chorded = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)];
        let m = cycle_masks(4, &chorded);
        assert_eq!(m[0], m[1]);
        assert_eq!(m[2], m[3]);
        assert_ne!(m[0], m[2]);
        assert_ne!(m[4], m[0]);
        assert_ne!(m[4], m[2]);
        // a pendant edge is a bridge
        let m = cycle_masks(4, &[(0, 1), (1, 2), (2, 0), (2, 3)]);
        assert_eq!(m[3], 0);
    }
}
