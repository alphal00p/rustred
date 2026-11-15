//! Graph layer of the planner and the gate: a compact key interner, adjacency in memory or on disk, and the
//! comparator-free acyclicity checks (Kahn, a witness cycle by a backward walk, an iterative Tarjan over the residual).
//!
//! Nothing here looks at the order of keys: these checks are the independent guard beside the descent check.
//!
//! Memory of the disk graph (per key): arena `arity` bytes (4 x `arity` once an index leaves i8), interner table
//! ~8 bytes, in- and out-degree 8 bytes, CSR offsets 8 bytes; per edge: nothing in memory (12 bytes in the spill
//! file, 4 bytes in the CSR file). The CSR is filled in passes over the spill file, each holding at most
//! `chunk_bytes` of targets.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

const EMPTY: u32 = u32::MAX;
/// Largest number of distinct keys an interner holds (ids are u32; `u32::MAX` is the empty slot).
pub const MAX_KEYS: usize = (u32::MAX - 1) as usize;

fn hash_iter(len: usize, k: impl Iterator<Item = i32>) -> u64 {
    let mut h: u64 = 0x9E37_79B9_7F4A_7C15 ^ (len as u64);
    for x in k {
        h = (h ^ (x as u32 as u64)).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        h ^= h >> 29;
    }
    h ^ (h >> 32)
}

fn hash_key(k: &[i32]) -> u64 {
    hash_iter(k.len(), k.iter().copied())
}

/// Interns keys of one arity as dense u32 ids (first come, first numbered). Keys are stored in a flat arena: one byte
/// per slot while every index seen fits in i8, four bytes per slot after the first that does not (one conversion).
#[derive(Clone, Debug)]
pub struct KeyInterner {
    arity: usize,
    narrow: Vec<i8>,
    wide: Option<Vec<i32>>,
    table: Vec<u32>,
    len: usize,
}

impl KeyInterner {
    pub fn new(arity: usize) -> KeyInterner {
        KeyInterner {
            arity,
            narrow: Vec::new(),
            wide: None,
            table: vec![EMPTY; 1024],
            len: 0,
        }
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Bytes held by the arena and the table.
    pub fn bytes(&self) -> usize {
        self.narrow.capacity()
            + self.wide.as_ref().map_or(0, |w| w.capacity() * 4)
            + self.table.capacity() * 4
    }

    fn eq_at(&self, id: u32, key: &[i32]) -> bool {
        let a = self.arity;
        let s = id as usize * a;
        match &self.wide {
            Some(w) => &w[s..s + a] == key,
            None => self.narrow[s..s + a]
                .iter()
                .zip(key)
                .all(|(&x, &y)| x as i32 == y),
        }
    }

    fn hash_of(&self, id: u32) -> u64 {
        let a = self.arity;
        let s = id as usize * a;
        match &self.wide {
            Some(w) => hash_key(&w[s..s + a]),
            None => hash_iter(a, self.narrow[s..s + a].iter().map(|&x| x as i32)),
        }
    }

    /// The id of `key` if interned. Panics on a key of another arity (callers check the arity first).
    pub fn get(&self, key: &[i32]) -> Option<u32> {
        assert_eq!(key.len(), self.arity, "KeyInterner: key of another arity");
        let mask = self.table.len() - 1;
        let mut p = hash_key(key) as usize & mask;
        loop {
            let id = self.table[p];
            if id == EMPTY {
                return None;
            }
            if self.eq_at(id, key) {
                return Some(id);
            }
            p = (p + 1) & mask;
        }
    }

    /// The id of `key`, interning it if new. Panics on a key of another arity or past [`MAX_KEYS`].
    pub fn intern(&mut self, key: &[i32]) -> u32 {
        assert_eq!(key.len(), self.arity, "KeyInterner: key of another arity");
        let mask = self.table.len() - 1;
        let mut p = hash_key(key) as usize & mask;
        loop {
            let id = self.table[p];
            if id == EMPTY {
                break;
            }
            if self.eq_at(id, key) {
                return id;
            }
            p = (p + 1) & mask;
        }
        assert!(self.len < MAX_KEYS, "KeyInterner: more than {MAX_KEYS} keys");
        let id = self.len as u32;
        if self.wide.is_none() && key.iter().any(|&x| x < i8::MIN as i32 || x > i8::MAX as i32) {
            self.wide = Some(self.narrow.iter().map(|&x| x as i32).collect());
            self.narrow = Vec::new();
        }
        match &mut self.wide {
            Some(w) => w.extend_from_slice(key),
            None => self.narrow.extend(key.iter().map(|&x| x as i8)),
        }
        self.table[p] = id;
        self.len += 1;
        if self.len * 2 > self.table.len() {
            self.grow();
        }
        id
    }

    fn grow(&mut self) {
        let n = self.table.len() * 2;
        let mut t = vec![EMPTY; n];
        let mask = n - 1;
        for id in 0..self.len as u32 {
            let mut p = self.hash_of(id) as usize & mask;
            while t[p] != EMPTY {
                p = (p + 1) & mask;
            }
            t[p] = id;
        }
        self.table = t;
    }

    /// The key of `id`.
    pub fn key(&self, id: u32) -> Vec<i32> {
        let a = self.arity;
        let s = id as usize * a;
        match &self.wide {
            Some(w) => w[s..s + a].to_vec(),
            None => self.narrow[s..s + a].iter().map(|&x| x as i32).collect(),
        }
    }
}

/// Successor lists of a graph on nodes `0..node_count()`.
pub trait Adjacency {
    fn node_count(&self) -> usize;
    /// Clears `buf` and appends the successors of `v` (with multiplicity, in insertion order).
    fn successors(&self, v: u32, buf: &mut Vec<u32>) -> io::Result<()>;
}

/// A graph in memory (CSR).
#[derive(Clone, Debug, Default)]
pub struct MemGraph {
    offsets: Vec<usize>,
    targets: Vec<u32>,
}

impl MemGraph {
    /// From an edge list on nodes `0..n` (stable: successors keep the list order).
    pub fn from_edges(n: usize, edges: &[(u32, u32)]) -> MemGraph {
        let mut offsets = vec![0usize; n + 1];
        for &(s, _) in edges {
            offsets[s as usize + 1] += 1;
        }
        for i in 0..n {
            offsets[i + 1] += offsets[i];
        }
        let mut fill = offsets.clone();
        let mut targets = vec![0u32; edges.len()];
        for &(s, t) in edges {
            targets[fill[s as usize]] = t;
            fill[s as usize] += 1;
        }
        MemGraph { offsets, targets }
    }

    /// The reverse graph.
    pub fn reversed(&self) -> MemGraph {
        let n = self.node_count();
        let mut edges = Vec::with_capacity(self.targets.len());
        for v in 0..n {
            for &t in &self.targets[self.offsets[v]..self.offsets[v + 1]] {
                edges.push((t, v as u32));
            }
        }
        MemGraph::from_edges(n, &edges)
    }

    pub fn succ(&self, v: u32) -> &[u32] {
        &self.targets[self.offsets[v as usize]..self.offsets[v as usize + 1]]
    }

    pub fn in_degrees(&self) -> Vec<u32> {
        let mut d = vec![0u32; self.node_count()];
        for &t in &self.targets {
            d[t as usize] += 1;
        }
        d
    }
}

impl Adjacency for MemGraph {
    fn node_count(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }
    fn successors(&self, v: u32, buf: &mut Vec<u32>) -> io::Result<()> {
        buf.clear();
        buf.extend_from_slice(self.succ(v));
        Ok(())
    }
}

const SPILL_RECORD: usize = 12;

/// Appends edges (src, dst, tag) to a spill file and counts degrees; [`DiskGraphBuilder::finish`] turns it into a
/// [`DiskGraph`] whose CSR lives in a second file.
pub struct DiskGraphBuilder {
    spill_path: PathBuf,
    csr_path: PathBuf,
    w: BufWriter<File>,
    outdeg: Vec<u32>,
    indeg: Vec<u32>,
    edges: u64,
}

fn unique_paths(dir: &Path) -> (PathBuf, PathBuf) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let stem = format!("pieces-gate-{}-{n}", std::process::id());
    (
        dir.join(format!("{stem}.edges")),
        dir.join(format!("{stem}.csr")),
    )
}

impl DiskGraphBuilder {
    /// A builder writing into `dir` (created if missing). Both files are removed when the graph is dropped.
    pub fn new(dir: &Path) -> io::Result<DiskGraphBuilder> {
        std::fs::create_dir_all(dir)?;
        let (spill_path, csr_path) = unique_paths(dir);
        let f = File::create(&spill_path)?;
        Ok(DiskGraphBuilder {
            spill_path,
            csr_path,
            w: BufWriter::with_capacity(1 << 20, f),
            outdeg: Vec::new(),
            indeg: Vec::new(),
            edges: 0,
        })
    }

    /// Makes nodes `0..n` known (nodes without edges included).
    pub fn ensure_nodes(&mut self, n: usize) {
        if self.outdeg.len() < n {
            self.outdeg.resize(n, 0);
            self.indeg.resize(n, 0);
        }
    }

    pub fn push(&mut self, src: u32, dst: u32, tag: u32) -> io::Result<()> {
        self.ensure_nodes(src.max(dst) as usize + 1);
        let mut rec = [0u8; SPILL_RECORD];
        rec[0..4].copy_from_slice(&src.to_le_bytes());
        rec[4..8].copy_from_slice(&dst.to_le_bytes());
        rec[8..12].copy_from_slice(&tag.to_le_bytes());
        self.w.write_all(&rec)?;
        self.outdeg[src as usize] = self.outdeg[src as usize]
            .checked_add(1)
            .ok_or_else(|| io::Error::other("out-degree overflow"))?;
        self.indeg[dst as usize] = self.indeg[dst as usize]
            .checked_add(1)
            .ok_or_else(|| io::Error::other("in-degree overflow"))?;
        self.edges += 1;
        Ok(())
    }

    pub fn edges(&self) -> u64 {
        self.edges
    }

    pub fn nodes(&self) -> usize {
        self.outdeg.len()
    }

    /// Bytes of the in-memory degree arrays.
    pub fn bytes(&self) -> usize {
        (self.outdeg.capacity() + self.indeg.capacity()) * 4
    }

    /// Writes the CSR file in passes of at most `chunk_bytes` of targets (at least one node per pass).
    pub fn finish(mut self, chunk_bytes: usize) -> io::Result<DiskGraph> {
        self.w.flush()?;
        let n = self.outdeg.len();
        let mut offsets = vec![0u64; n + 1];
        for i in 0..n {
            offsets[i + 1] = offsets[i] + self.outdeg[i] as u64;
        }
        drop(std::mem::take(&mut self.outdeg));
        let csr = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.csr_path)?;
        let per_pass = (chunk_bytes / 4).max(1) as u64;
        let mut lo = 0usize;
        let mut passes = 0u32;
        while lo < n {
            let mut hi = lo + 1;
            while hi < n && offsets[hi + 1] - offsets[lo] <= per_pass {
                hi += 1;
            }
            let base = offsets[lo];
            let len = (offsets[hi] - base) as usize;
            if len > 0 {
                let mut buf = vec![0u32; len];
                let mut fill: Vec<u32> = (lo..hi)
                    .map(|v| (offsets[v] - base) as u32)
                    .collect::<Vec<_>>();
                // u32 fill positions are enough: a pass holds at most chunk_bytes / 4 targets
                let f = File::open(&self.spill_path)?;
                let mut r = BufReader::with_capacity(1 << 20, f);
                let mut rec = [0u8; SPILL_RECORD];
                for _ in 0..self.edges {
                    r.read_exact(&mut rec)?;
                    let s = u32::from_le_bytes(rec[0..4].try_into().unwrap()) as usize;
                    if s >= lo && s < hi {
                        let d = u32::from_le_bytes(rec[4..8].try_into().unwrap());
                        let p = &mut fill[s - lo];
                        buf[*p as usize] = d;
                        *p += 1;
                    }
                }
                let mut bytes = Vec::with_capacity(len * 4);
                for t in &buf {
                    bytes.extend_from_slice(&t.to_le_bytes());
                }
                csr.write_all_at(&bytes, base * 4)?;
            }
            passes += 1;
            lo = hi;
        }
        csr.sync_data().ok();
        Ok(DiskGraph {
            spill_path: std::mem::take(&mut self.spill_path),
            csr_path: std::mem::take(&mut self.csr_path),
            csr,
            offsets,
            indeg: std::mem::take(&mut self.indeg),
            edges: self.edges,
            passes,
            scratch: RefCell::new(Vec::new()),
        })
    }
}

impl Drop for DiskGraphBuilder {
    fn drop(&mut self) {
        if !self.spill_path.as_os_str().is_empty() {
            let _ = std::fs::remove_file(&self.spill_path);
        }
    }
}

/// A graph whose successor lists live in a file (positioned reads), with the in-degrees in memory.
pub struct DiskGraph {
    spill_path: PathBuf,
    csr_path: PathBuf,
    csr: File,
    offsets: Vec<u64>,
    /// In-degrees (consumed by [`kahn`]).
    pub indeg: Vec<u32>,
    pub edges: u64,
    /// Passes over the spill file needed to write the CSR.
    pub passes: u32,
    scratch: RefCell<Vec<u8>>,
}

impl DiskGraph {
    /// Bytes held in memory (offsets, in-degrees).
    pub fn bytes(&self) -> usize {
        self.offsets.capacity() * 8 + self.indeg.capacity() * 4
    }

    /// Streams every spilled edge (src, dst, tag) in insertion order.
    pub fn for_each_edge(&self, mut f: impl FnMut(u32, u32, u32)) -> io::Result<()> {
        let fh = File::open(&self.spill_path)?;
        let mut r = BufReader::with_capacity(1 << 20, fh);
        let mut rec = [0u8; SPILL_RECORD];
        for _ in 0..self.edges {
            r.read_exact(&mut rec)?;
            f(
                u32::from_le_bytes(rec[0..4].try_into().unwrap()),
                u32::from_le_bytes(rec[4..8].try_into().unwrap()),
                u32::from_le_bytes(rec[8..12].try_into().unwrap()),
            );
        }
        Ok(())
    }
}

impl Adjacency for DiskGraph {
    fn node_count(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }
    fn successors(&self, v: u32, buf: &mut Vec<u32>) -> io::Result<()> {
        buf.clear();
        let (a, b) = (self.offsets[v as usize], self.offsets[v as usize + 1]);
        if a == b {
            return Ok(());
        }
        let mut s = self.scratch.borrow_mut();
        s.resize(((b - a) * 4) as usize, 0);
        self.csr.read_exact_at(&mut s, a * 4)?;
        buf.extend(
            s.chunks_exact(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap())),
        );
        Ok(())
    }
}

impl Drop for DiskGraph {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.spill_path);
        let _ = std::fs::remove_file(&self.csr_path);
    }
}

/// Kahn's algorithm on `g` with in-degrees `indeg` (consumed: afterwards `indeg[v] > 0` iff `v` is RESIDUAL, i.e.
/// on a cycle or reachable from one). Nodes of in-degree 0 are taken in increasing id order, then FIFO. Returns the
/// number of processed nodes; `order` receives them in processing order when given.
pub fn kahn<A: Adjacency>(
    g: &A,
    indeg: &mut [u32],
    mut order: Option<&mut Vec<u32>>,
) -> io::Result<u64> {
    let n = g.node_count();
    assert_eq!(indeg.len(), n, "kahn: in-degree array of another size");
    let mut q: VecDeque<u32> = (0..n as u32).filter(|&v| indeg[v as usize] == 0).collect();
    let mut buf = Vec::new();
    let mut done = 0u64;
    while let Some(v) = q.pop_front() {
        done += 1;
        if let Some(o) = order.as_deref_mut() {
            o.push(v);
        }
        g.successors(v, &mut buf)?;
        for &t in &buf {
            let d = &mut indeg[t as usize];
            *d -= 1;
            if *d == 0 {
                q.push_back(t);
            }
        }
    }
    Ok(done)
}

/// A cycle among the residual nodes of [`kahn`] (`residual[v]` = `indeg[v] > 0` after it), found by walking
/// predecessors from the smallest residual node: every residual node has a residual predecessor, so the walk must
/// repeat. Returned in edge direction (`c[i] -> c[i+1]`, last -> first), rotated to start at its smallest id. Empty
/// iff no node is residual.
pub fn witness_cycle<A: Adjacency>(g: &A, residual: &[bool]) -> io::Result<Vec<u32>> {
    let n = g.node_count();
    let Some(start) = (0..n).find(|&v| residual[v]) else {
        return Ok(vec![]);
    };
    let mut pred = vec![EMPTY; n];
    let mut buf = Vec::new();
    for v in 0..n {
        if !residual[v] {
            continue;
        }
        g.successors(v as u32, &mut buf)?;
        for &t in &buf {
            if residual[t as usize] && pred[t as usize] == EMPTY {
                pred[t as usize] = v as u32;
            }
        }
    }
    let mut pos: HashMap<u32, usize> = HashMap::new();
    let mut path: Vec<u32> = Vec::new();
    let mut x = start as u32;
    loop {
        if let Some(&p) = pos.get(&x) {
            let mut c: Vec<u32> = path[p..].to_vec();
            c.reverse();
            let m = c
                .iter()
                .enumerate()
                .min_by_key(|(_, v)| **v)
                .map(|(i, _)| i)
                .unwrap_or(0);
            c.rotate_left(m);
            return Ok(c);
        }
        pos.insert(x, path.len());
        path.push(x);
        let p = pred[x as usize];
        if p == EMPTY {
            return Err(io::Error::other(
                "witness_cycle: a residual node without a residual predecessor (in-degrees not from kahn)",
            ));
        }
        x = p;
    }
}

/// Strongly connected components of the subgraph induced by `member` (iterative Tarjan).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SccSummary {
    /// Components with more than one node or a self loop.
    pub cyclic: u64,
    /// Nodes in cyclic components.
    pub cyclic_nodes: u64,
    /// Size of the largest cyclic component.
    pub largest: u64,
    /// Nodes with a self loop.
    pub self_loops: u64,
}

/// Tarjan over the nodes with `member[v]`; `on_scc(component, cyclic)` receives EVERY component (sorted ids) in the
/// order Tarjan completes them: a component is completed only after every component reachable from it (with edges
/// read as "depends on", dependencies first).
pub fn sccs<A: Adjacency>(
    g: &A,
    member: &[bool],
    mut on_scc: impl FnMut(&[u32], bool),
) -> io::Result<SccSummary> {
    let n = g.node_count();
    let mut index = vec![EMPTY; n];
    let mut low = vec![0u32; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<u32> = Vec::new();
    let mut next = 0u32;
    let mut sum = SccSummary::default();
    // call frames: (node, successors, position)
    let mut frames: Vec<(u32, Vec<u32>, usize)> = Vec::new();
    for root in 0..n as u32 {
        if !member[root as usize] || index[root as usize] != EMPTY {
            continue;
        }
        let mut succ = Vec::new();
        g.successors(root, &mut succ)?;
        index[root as usize] = next;
        low[root as usize] = next;
        next += 1;
        stack.push(root);
        on_stack[root as usize] = true;
        frames.push((root, succ, 0));
        while let Some(top) = frames.last_mut() {
            let v = top.0;
            if top.2 < top.1.len() {
                let w = top.1[top.2];
                top.2 += 1;
                if !member[w as usize] {
                    continue;
                }
                if index[w as usize] == EMPTY {
                    let mut s = Vec::new();
                    g.successors(w, &mut s)?;
                    index[w as usize] = next;
                    low[w as usize] = next;
                    next += 1;
                    stack.push(w);
                    on_stack[w as usize] = true;
                    frames.push((w, s, 0));
                } else if on_stack[w as usize] {
                    low[v as usize] = low[v as usize].min(index[w as usize]);
                }
            } else {
                let self_loop = top.1.contains(&v);
                frames.pop();
                if let Some(parent) = frames.last() {
                    let p = parent.0 as usize;
                    low[p] = low[p].min(low[v as usize]);
                }
                if low[v as usize] == index[v as usize] {
                    let mut comp = Vec::new();
                    loop {
                        let w = stack.pop().expect("tarjan stack");
                        on_stack[w as usize] = false;
                        comp.push(w);
                        if w == v {
                            break;
                        }
                    }
                    if self_loop {
                        sum.self_loops += 1;
                    }
                    comp.sort_unstable();
                    let cyclic = comp.len() > 1 || self_loop;
                    if cyclic {
                        sum.cyclic += 1;
                        sum.cyclic_nodes += comp.len() as u64;
                        sum.largest = sum.largest.max(comp.len() as u64);
                    }
                    on_scc(&comp, cyclic);
                }
            }
        }
    }
    Ok(sum)
}

/// A shortest cycle through `start` inside the node set `member` (BFS), in edge direction starting at `start`;
/// empty if none.
pub fn cycle_through<A: Adjacency>(g: &A, member: &[bool], start: u32) -> io::Result<Vec<u32>> {
    let mut parent: HashMap<u32, u32> = HashMap::new();
    let mut q = VecDeque::new();
    let mut buf = Vec::new();
    q.push_back(start);
    while let Some(v) = q.pop_front() {
        g.successors(v, &mut buf)?;
        for &w in &buf {
            if !member[w as usize] {
                continue;
            }
            if w == start {
                let mut path = vec![v];
                let mut x = v;
                while x != start {
                    x = parent[&x];
                    path.push(x);
                }
                path.reverse();
                return Ok(path);
            }
            if let std::collections::hash_map::Entry::Vacant(e) = parent.entry(w) {
                e.insert(v);
                q.push_back(w);
            }
        }
    }
    Ok(vec![])
}
