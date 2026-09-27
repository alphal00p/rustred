//! Static k-d tree over container keys for exact dominance ("some container
//! C with key(C) <= q componentwise") queries with an ID filter. Each node
//! stores the componentwise minimum of its keys and the minimum IDs of its
//! members (all, and natives only), so a subtree is skipped when its minimum
//! corner is not dominated by the query or when no member can satisfy the
//! ID filter. Leaves are checked exactly; the answer is exact.
pub struct Tree {
    pub k: usize,
    /// Keys in tree order, k values each.
    keys: Vec<i16>,
    ids: Vec<u32>,
    native: Vec<bool>,
    nodes: Vec<Node>,
    mins: Vec<i16>,
}

#[derive(Clone, Copy)]
struct Node {
    start: u32,
    end: u32,
    left: u32,
    right: u32,
    min_id: u32,
    min_native_id: u32,
}

const LEAF: usize = 12;
const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Filter {
    /// Any member with id < bound.
    Before(u32),
    /// Any native member with id < bound.
    NativeBefore(u32),
    /// Any member other than `except`.
    Other(u32),
}

struct Build<'a> {
    k: usize,
    keys: &'a [i16],
    ids: &'a [u32],
    native: &'a [bool],
    nodes: Vec<Node>,
    mins: Vec<i16>,
}

impl Build<'_> {
    fn key(&self, i: u32, d: usize) -> i16 {
        self.keys[i as usize * self.k + d]
    }
    fn rec(&mut self, perm: &mut [u32], base: usize) -> u32 {
        let k = self.k;
        let mut mn = vec![i16::MAX; k];
        let mut mx = vec![i16::MIN; k];
        let (mut min_id, mut min_nat) = (NONE, NONE);
        for &i in perm.iter() {
            let key = &self.keys[i as usize * k..(i as usize + 1) * k];
            for d in 0..k {
                mn[d] = mn[d].min(key[d]);
                mx[d] = mx[d].max(key[d]);
            }
            let id = self.ids[i as usize];
            min_id = min_id.min(id);
            if self.native[i as usize] {
                min_nat = min_nat.min(id);
            }
        }
        let idx = self.nodes.len() as u32;
        self.nodes.push(Node {
            start: base as u32,
            end: (base + perm.len()) as u32,
            left: NONE,
            right: NONE,
            min_id,
            min_native_id: min_nat,
        });
        self.mins.extend_from_slice(&mn);
        if perm.len() <= LEAF {
            return idx;
        }
        let dim = (0..k)
            .max_by_key(|&d| (mx[d] as i32 - mn[d] as i32, std::cmp::Reverse(d)))
            .unwrap();
        if mx[dim] == mn[dim] {
            return idx;
        }
        let mid = perm.len() / 2;
        perm.select_nth_unstable_by_key(mid, |&i| self.key(i, dim));
        let v = self.key(perm[mid], dim);
        // Partition into key < v | key >= v; if the left side is empty (v is
        // the minimum), use key <= v | key > v instead.
        let mut cut = partition(perm, |i| self.key(i, dim) < v);
        if cut == 0 {
            cut = partition(perm, |i| self.key(i, dim) <= v);
        }
        if cut == 0 || cut == perm.len() {
            return idx;
        }
        let (l, r) = perm.split_at_mut(cut);
        let left = self.rec(l, base);
        let right = self.rec(r, base + cut);
        self.nodes[idx as usize].left = left;
        self.nodes[idx as usize].right = right;
        idx
    }
}

fn partition(perm: &mut [u32], pred: impl Fn(u32) -> bool) -> usize {
    let mut i = 0;
    for j in 0..perm.len() {
        if pred(perm[j]) {
            perm.swap(i, j);
            i += 1;
        }
    }
    i
}

impl Tree {
    /// `keys` holds `ids.len()` keys of length `k`, flat.
    pub fn build(k: usize, ids: Vec<u32>, native: Vec<bool>, keys: Vec<i16>) -> Tree {
        let n = ids.len();
        assert_eq!(keys.len(), n * k);
        assert_eq!(native.len(), n);
        let mut perm: Vec<u32> = (0..n as u32).collect();
        let mut b = Build {
            k,
            keys: &keys,
            ids: &ids,
            native: &native,
            nodes: Vec::new(),
            mins: Vec::new(),
        };
        if n > 0 {
            b.rec(&mut perm, 0);
        }
        let (nodes, mins) = (b.nodes, b.mins);
        let mut okeys = Vec::with_capacity(n * k);
        let mut oids = Vec::with_capacity(n);
        let mut onat = Vec::with_capacity(n);
        for &i in &perm {
            okeys.extend_from_slice(&keys[i as usize * k..(i as usize + 1) * k]);
            oids.push(ids[i as usize]);
            onat.push(native[i as usize]);
        }
        Tree {
            k,
            keys: okeys,
            ids: oids,
            native: onat,
            nodes,
            mins,
        }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Some member satisfying `filter` with key <= q. Returns its id.
    pub fn find(&self, q: &[i16], filter: Filter, stack: &mut Vec<u32>) -> Option<u32> {
        if self.nodes.is_empty() {
            return None;
        }
        let k = self.k;
        stack.clear();
        stack.push(0);
        while let Some(ni) = stack.pop() {
            let node = self.nodes[ni as usize];
            match filter {
                Filter::Before(b) if node.min_id >= b => continue,
                Filter::NativeBefore(b) if node.min_native_id >= b => continue,
                _ => {}
            }
            let m = &self.mins[ni as usize * k..(ni as usize + 1) * k];
            if !crate::tight::dominated(m, q) {
                continue;
            }
            if node.left == NONE {
                for p in node.start as usize..node.end as usize {
                    let id = self.ids[p];
                    let ok = match filter {
                        Filter::Before(b) => id < b,
                        Filter::NativeBefore(b) => id < b && self.native[p],
                        Filter::Other(e) => id != e,
                    };
                    if ok && crate::tight::dominated(&self.keys[p * k..(p + 1) * k], q) {
                        return Some(id);
                    }
                }
            } else {
                stack.push(node.right);
                stack.push(node.left);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_linear_scan() {
        let mut s = 12345u64;
        let mut rnd = |m: u64| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s % m) as i16
        };
        let k = 6;
        let n = 3000;
        let ids: Vec<u32> = (0..n).map(|i| (i * 7 % 3001) as u32).collect();
        let native: Vec<bool> = (0..n).map(|_| rnd(2) == 0).collect();
        let keys: Vec<i16> = (0..n * k).map(|_| rnd(8)).collect();
        let tree = Tree::build(k, ids.clone(), native.clone(), keys.clone());
        assert_eq!(tree.len(), n);
        let mut stack = Vec::new();
        for _ in 0..5000 {
            let q: Vec<i16> = (0..k).map(|_| rnd(9)).collect();
            let b = rnd(3001) as u32;
            for f in [Filter::Before(b), Filter::NativeBefore(b), Filter::Other(b)] {
                let sat = |i: usize| {
                    let ok = match f {
                        Filter::Before(x) => ids[i] < x,
                        Filter::NativeBefore(x) => ids[i] < x && native[i],
                        Filter::Other(x) => ids[i] != x,
                    };
                    ok && keys[i * k..(i + 1) * k].iter().zip(&q).all(|(a, b)| a <= b)
                };
                let brute = (0..n).any(sat);
                let got = tree.find(&q, f, &mut stack);
                assert_eq!(got.is_some(), brute, "{f:?}");
                if let Some(id) = got {
                    let i = ids.iter().position(|&x| x == id).unwrap();
                    assert!(sat(i));
                }
            }
        }
    }
}
