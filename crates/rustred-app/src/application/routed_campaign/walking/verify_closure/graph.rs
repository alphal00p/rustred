//! Dependency closure re-derived from a saved edge set, independently of the
//! walker's `descendant_closure::Tracker`: a node is closed iff no unsealed
//! node is reachable from it (itself included). Two derivations are kept and
//! compared: reverse reachability from every unsealed node, and a forward
//! cone per root. Closure is coinductive (sealed cycles and self-edges count
//! as closed); `cyclic` reports which nodes lie on a cycle so a report can
//! say how much of a cone rests on that.

/// Out-adjacency in CSR form with u64 offsets.
pub(super) struct Graph {
    offsets: Vec<u64>,
    targets: Vec<u32>,
}

impl Graph {
    /// Edges are (source, target): the source depends on the target. Both
    /// endpoints must be below `nodes`. Duplicate edges are kept (and counted
    /// by the caller); they never change reachability.
    pub fn from_edges(nodes: usize, edges: &[(u32, u32)]) -> Result<Self, String> {
        let mut degree = vec![0u64; nodes + 1];
        for &(source, target) in edges {
            if source as usize >= nodes || target as usize >= nodes {
                return Err(format!(
                    "edge ({source}, {target}) outside the {nodes} saved domains"
                ));
            }
            degree[source as usize + 1] += 1;
        }
        for index in 1..=nodes {
            degree[index] += degree[index - 1];
        }
        let mut cursor = degree.clone();
        let mut targets = vec![0u32; edges.len()];
        for &(source, target) in edges {
            let slot = &mut cursor[source as usize];
            targets[*slot as usize] = target;
            *slot += 1;
        }
        let graph = Self {
            offsets: degree,
            targets,
        };
        Ok(graph.sorted())
    }

    fn sorted(mut self) -> Self {
        for node in 0..self.nodes() {
            let (start, end) = (self.offsets[node] as usize, self.offsets[node + 1] as usize);
            self.targets[start..end].sort_unstable();
        }
        self
    }

    pub fn nodes(&self) -> usize {
        self.offsets.len() - 1
    }

    pub fn edges(&self) -> usize {
        self.targets.len()
    }

    pub fn out(&self, node: usize) -> &[u32] {
        &self.targets[self.offsets[node] as usize..self.offsets[node + 1] as usize]
    }

    pub fn has_edge(&self, source: usize, target: usize) -> bool {
        u32::try_from(target).is_ok_and(|target| self.out(source).binary_search(&target).is_ok())
    }

    /// Adjacent equal targets after the per-source sort.
    pub fn duplicate_edges(&self) -> usize {
        (0..self.nodes())
            .map(|node| self.out(node).windows(2).filter(|w| w[0] == w[1]).count())
            .sum()
    }

    /// The reversed CSR, built in place from this one (no edge-pair copy).
    fn reversed(&self) -> Self {
        let nodes = self.nodes();
        let mut offsets = vec![0u64; nodes + 1];
        for &target in &self.targets {
            offsets[target as usize + 1] += 1;
        }
        for index in 1..=nodes {
            offsets[index] += offsets[index - 1];
        }
        let mut cursor = offsets.clone();
        let mut targets = vec![0u32; self.targets.len()];
        for source in 0..nodes {
            for &target in self.out(source) {
                let slot = &mut cursor[target as usize];
                targets[*slot as usize] = source as u32;
                *slot += 1;
            }
        }
        Self { offsets, targets }
    }

    /// closed[i] iff no unsealed node is reachable from i.
    pub fn closed(&self, sealed: &[bool]) -> Vec<bool> {
        let reverse = self.reversed();
        let mut blocked = vec![false; self.nodes()];
        let mut stack: Vec<u32> = Vec::new();
        for (node, &seal) in sealed.iter().enumerate() {
            if !seal && !blocked[node] {
                blocked[node] = true;
                stack.push(node as u32);
                while let Some(current) = stack.pop() {
                    for &parent in reverse.out(current as usize) {
                        if !blocked[parent as usize] {
                            blocked[parent as usize] = true;
                            stack.push(parent);
                        }
                    }
                }
            }
        }
        blocked.into_iter().map(|b| !b).collect()
    }

    /// Visit every node of the forward cone of `root` once; returns its size.
    /// `mark` is a per-node stamp array reused across roots with distinct
    /// `stamp`s.
    pub fn cone(
        &self,
        root: usize,
        mark: &mut [u32],
        stamp: u32,
        mut visit: impl FnMut(usize),
    ) -> usize {
        let mut size = 0usize;
        let mut stack = vec![root as u32];
        mark[root] = stamp;
        while let Some(current) = stack.pop() {
            size += 1;
            visit(current as usize);
            for &target in self.out(current as usize) {
                if mark[target as usize] != stamp {
                    mark[target as usize] = stamp;
                    stack.push(target);
                }
            }
        }
        size
    }

    /// Nodes on a directed cycle: members of a strongly connected component
    /// with more than one node, or with a self-edge (iterative Tarjan).
    pub fn cyclic(&self) -> Vec<bool> {
        const UNSEEN: u32 = u32::MAX;
        let nodes = self.nodes();
        let mut index = vec![UNSEEN; nodes];
        let mut low = vec![0u32; nodes];
        let mut on_stack = vec![false; nodes];
        let mut cyclic = vec![false; nodes];
        let mut stack: Vec<u32> = Vec::new();
        // (node, next out-edge position)
        let mut calls: Vec<(u32, usize)> = Vec::new();
        let mut next = 0u32;
        for start in 0..nodes {
            if index[start] != UNSEEN {
                continue;
            }
            calls.push((start as u32, 0));
            index[start] = next;
            low[start] = next;
            next += 1;
            stack.push(start as u32);
            on_stack[start] = true;
            while let Some(&mut (node, ref mut position)) = calls.last_mut() {
                let node = node as usize;
                let out = self.out(node);
                if *position < out.len() {
                    let target = out[*position] as usize;
                    *position += 1;
                    if target == node {
                        cyclic[node] = true;
                    }
                    if index[target] == UNSEEN {
                        index[target] = next;
                        low[target] = next;
                        next += 1;
                        stack.push(target as u32);
                        on_stack[target] = true;
                        calls.push((target as u32, 0));
                    } else if on_stack[target] {
                        low[node] = low[node].min(index[target]);
                    }
                    continue;
                }
                calls.pop();
                if let Some(&(parent, _)) = calls.last() {
                    let parent = parent as usize;
                    low[parent] = low[parent].min(low[node]);
                }
                if low[node] == index[node] {
                    if stack.last().is_some_and(|&top| top as usize == node) {
                        // Singleton component: cyclic only through a self-edge.
                        stack.pop();
                        on_stack[node] = false;
                    } else {
                        loop {
                            let member = stack.pop().expect("Tarjan stack") as usize;
                            on_stack[member] = false;
                            cyclic[member] = true;
                            if member == node {
                                break;
                            }
                        }
                    }
                }
            }
        }
        cyclic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closure_blocks_every_ancestor_of_an_unsealed_node_and_keeps_sealed_cycles() {
        // 0 -> 1 -> 2 (unsealed); 3 <-> 4 sealed cycle; 5 -> 3; 6 -> 6; 7 -> 2.
        let edges = [
            (0, 1),
            (1, 2),
            (3, 4),
            (4, 3),
            (5, 3),
            (6, 6),
            (7, 2),
            (0, 1),
        ];
        let graph = Graph::from_edges(8, &edges).unwrap();
        assert_eq!(graph.duplicate_edges(), 1);
        assert_eq!(graph.edges(), 8);
        assert!(graph.has_edge(5, 3) && !graph.has_edge(3, 5));
        let sealed = [true, true, false, true, true, true, true, true];
        let closed = graph.closed(&sealed);
        assert_eq!(closed, [false, false, false, true, true, true, true, false]);
        let mut mark = vec![0u32; 8];
        for (stamp, root) in (0..8).enumerate() {
            let mut unsealed = 0;
            graph.cone(root, &mut mark, stamp as u32 + 1, |node| {
                unsealed += usize::from(!sealed[node])
            });
            assert_eq!(unsealed == 0, closed[root], "root {root}");
        }
        let mut seen = Vec::new();
        assert_eq!(graph.cone(5, &mut mark, 99, |node| seen.push(node)), 3);
        seen.sort_unstable();
        assert_eq!(seen, [3, 4, 5]);
        assert_eq!(
            graph.cyclic(),
            [false, false, false, true, true, false, true, false]
        );
        assert!(Graph::from_edges(2, &[(0, 2)]).is_err());
        // The reversed CSR holds every edge once, reversed.
        let reverse = graph.reversed();
        assert_eq!(reverse.edges(), graph.edges());
        assert_eq!(reverse.out(1), [0, 0]);
        assert_eq!(reverse.out(3), [4, 5]);
    }

    #[test]
    fn cycle_detection_matches_pairwise_reachability() {
        // Deterministic pseudo-random sparse graphs; a node is cyclic iff it
        // reaches itself through at least one edge.
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..200 {
            let nodes = 1 + (next() % 12) as usize;
            let edges: Vec<(u32, u32)> = (0..next() % 20)
                .map(|_| {
                    (
                        (next() % nodes as u64) as u32,
                        (next() % nodes as u64) as u32,
                    )
                })
                .collect();
            let graph = Graph::from_edges(nodes, &edges).unwrap();
            let cyclic = graph.cyclic();
            for node in 0..nodes {
                let mut mark = vec![0u32; nodes];
                let mut reaches_self = false;
                for &first in graph.out(node) {
                    graph.cone(first as usize, &mut mark, 1, |seen| {
                        reaches_self |= seen == node
                    });
                }
                assert_eq!(cyclic[node], reaches_self, "{edges:?} node {node}");
            }
        }
    }
}
