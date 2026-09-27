//! Dependency closure re-derived from a saved edge set, independently of the
//! walker's `descendant_closure::Tracker`: a node is closed iff no unsealed
//! node is reachable from it (itself included). Two derivations are kept and
//! compared: reverse reachability from every unsealed node, and a forward
//! cone per root.

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
        for node in 0..nodes {
            targets[degree[node] as usize..degree[node + 1] as usize].sort_unstable();
        }
        Ok(Self {
            offsets: degree,
            targets,
        })
    }

    pub fn nodes(&self) -> usize {
        self.offsets.len() - 1
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

    fn reversed(&self) -> Self {
        let nodes = self.nodes();
        let edges: Vec<(u32, u32)> = (0..nodes)
            .flat_map(|source| {
                self.out(source)
                    .iter()
                    .map(move |&target| (target, source as u32))
            })
            .collect();
        Self::from_edges(nodes, &edges).expect("endpoints already validated")
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

    /// (nodes, unsealed nodes) in the forward cone of `root`. `mark` is a
    /// per-node stamp array reused across roots with distinct `stamp`s.
    pub fn cone(
        &self,
        root: usize,
        sealed: &[bool],
        mark: &mut [u32],
        stamp: u32,
    ) -> (usize, usize) {
        let (mut size, mut unsealed) = (0usize, 0usize);
        let mut stack = vec![root as u32];
        mark[root] = stamp;
        while let Some(current) = stack.pop() {
            size += 1;
            unsealed += usize::from(!sealed[current as usize]);
            for &target in self.out(current as usize) {
                if mark[target as usize] != stamp {
                    mark[target as usize] = stamp;
                    stack.push(target);
                }
            }
        }
        (size, unsealed)
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
        assert!(graph.has_edge(5, 3) && !graph.has_edge(3, 5));
        let sealed = [true, true, false, true, true, true, true, true];
        let closed = graph.closed(&sealed);
        assert_eq!(closed, [false, false, false, true, true, true, true, false]);
        let mut mark = vec![0u32; 8];
        for (stamp, root) in (0..8).enumerate() {
            let (_, unsealed) = graph.cone(root, &sealed, &mut mark, stamp as u32 + 1);
            assert_eq!(unsealed == 0, closed[root], "root {root}");
        }
        assert_eq!(graph.cone(5, &sealed, &mut mark, 99), (3, 0));
        assert!(Graph::from_edges(2, &[(0, 2)]).is_err());
    }
}
