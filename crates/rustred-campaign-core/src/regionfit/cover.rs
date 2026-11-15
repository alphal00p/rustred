//! The cover search: candidate regions -> greedy (deterministic tie-break) and an exact branch and bound below a size
//! threshold. Objective (lexicographic, [`Cost`]): pieces (regions + residual_key_cost x residual keys), atoms, kind
//! weight, choice ranks.

use super::atoms::{Atom, Bits};

/// A candidate region: a truth set on the face, its simplest condition and the best ranked choice acceptable on it.
#[derive(Clone, Debug)]
pub struct Cand {
    pub bits: Bits,
    pub size: usize,
    pub atoms: Vec<Atom>,
    pub weight: u64,
    /// Index into the sorted choice universe.
    pub choice: usize,
    pub rank: u64,
}

impl Cand {
    pub fn simplicity(&self) -> (usize, u64, &[Atom]) {
        (self.atoms.len(), self.weight, &self.atoms)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Cost {
    pub pieces: u64,
    pub atoms: u64,
    pub weight: u64,
    pub rank: u64,
}

impl Cost {
    fn add_region(self, c: &Cand) -> Cost {
        Cost {
            pieces: self.pieces + 1,
            atoms: self.atoms + c.atoms.len() as u64,
            weight: self.weight + c.weight,
            rank: self.rank + c.rank,
        }
    }
}

/// Drop candidates another candidate dominates (a strict superset of keys whose (atoms, weight, rank) is not larger:
/// replacing the first by the second in any cover never makes the objective worse). `cands` must be sorted by size
/// descending.
pub fn prune_dominated(cands: Vec<Cand>) -> Vec<Cand> {
    let mut kept: Vec<Cand> = vec![];
    for c in cands {
        let dominated = kept.iter().any(|k| {
            k.size > c.size
                && (k.atoms.len(), k.weight, k.rank) <= (c.atoms.len(), c.weight, c.rank)
                && c.bits.subset_of(&k.bits)
        });
        if !dominated {
            kept.push(c);
        }
    }
    kept
}

pub struct Solution {
    /// Candidate indices in dispatch order.
    pub regions: Vec<usize>,
    pub cost: Cost,
}

fn residual_cost(unc: &Bits, key_rank: &[u64], w: u64) -> Cost {
    let mut c = Cost::default();
    for k in unc.ones() {
        c.pieces += w;
        c.rank += key_rank[k];
    }
    c
}

fn plus(a: Cost, b: Cost) -> Cost {
    Cost {
        pieces: a.pieces + b.pieces,
        atoms: a.atoms + b.atoms,
        weight: a.weight + b.weight,
        rank: a.rank + b.rank,
    }
}

/// Greedy: most new keys first; ties by fewer atoms, lower weight, better choice rank, then the candidate order.
/// A region is added only while it lowers the piece count (residual_key_cost x new keys > 1).
pub fn greedy(cands: &[Cand], coverable: &Bits, key_rank: &[u64], w: u64) -> Solution {
    let mut unc = coverable.clone();
    let mut regions = vec![];
    let mut cost = Cost::default();
    loop {
        let mut best: Option<(usize, usize)> = None;
        for (i, c) in cands.iter().enumerate() {
            let g = c.bits.and_count(&unc);
            if g == 0 {
                continue;
            }
            let better = match best {
                None => true,
                Some((bi, bg)) => {
                    let b = &cands[bi];
                    (std::cmp::Reverse(g), c.atoms.len(), c.weight, c.rank)
                        < (std::cmp::Reverse(bg), b.atoms.len(), b.weight, b.rank)
                }
            };
            if better {
                best = Some((i, g));
            }
        }
        match best {
            Some((i, g)) if w.saturating_mul(g as u64) > 1 => {
                cost = cost.add_region(&cands[i]);
                unc = unc.and_not(&cands[i].bits);
                regions.push(i);
            }
            _ => break,
        }
    }
    Solution {
        regions,
        cost: plus(cost, residual_cost(&unc, key_rank, w)),
    }
}

pub struct ExactOutcome {
    pub solution: Option<Solution>,
    pub nodes: u64,
    pub exhausted: bool,
}

struct Bnb<'a> {
    cands: &'a [Cand],
    by_key: Vec<Vec<usize>>,
    key_rank: &'a [u64],
    w: u64,
    budget: u64,
    nodes: u64,
    exhausted: bool,
    best: Cost,
    best_regions: Option<Vec<usize>>,
}

impl Bnb<'_> {
    fn go(&mut self, unc: &Bits, chosen: &mut Vec<usize>, cost: Cost) {
        if self.exhausted {
            return;
        }
        self.nodes += 1;
        if self.nodes > self.budget {
            self.exhausted = true;
            return;
        }
        let r = unc.count() as u64;
        if r == 0 {
            if cost < self.best {
                self.best = cost;
                self.best_regions = Some(chosen.clone());
            }
            return;
        }
        let m = self
            .cands
            .iter()
            .map(|c| c.bits.and_count(unc) as u64)
            .max()
            .unwrap_or(0)
            .max(1);
        let lb = Cost {
            pieces: cost.pieces + r.div_ceil(m),
            ..cost
        };
        if lb >= self.best {
            return;
        }
        // branch on the open key with the fewest candidates (then the lowest index)
        let k = unc
            .ones()
            .min_by_key(|&k| (self.by_key[k].len(), k))
            .expect("r > 0");
        let mut opts: Vec<usize> = self.by_key[k].clone();
        opts.sort_by_key(|&i| {
            let c = &self.cands[i];
            (
                std::cmp::Reverse(c.bits.and_count(unc)),
                c.atoms.len(),
                c.weight,
                c.rank,
                i,
            )
        });
        for i in opts {
            let c = &self.cands[i];
            chosen.push(i);
            self.go(&unc.and_not(&c.bits), chosen, cost.add_region(c));
            chosen.pop();
            if self.exhausted {
                return;
            }
        }
        // the key served per key
        let mut u2 = unc.clone();
        u2.clear(k);
        let c2 = Cost {
            pieces: cost.pieces + self.w,
            rank: cost.rank + self.key_rank[k],
            ..cost
        };
        self.go(&u2, chosen, c2);
    }
}

/// Exact branch and bound, seeded with the value `bound` (only a strictly better cover is returned).
pub fn exact(
    cands: &[Cand],
    coverable: &Bits,
    key_rank: &[u64],
    w: u64,
    bound: Cost,
    budget: u64,
) -> ExactOutcome {
    let n = key_rank.len();
    let mut by_key: Vec<Vec<usize>> = vec![vec![]; n];
    for (i, c) in cands.iter().enumerate() {
        for k in c.bits.ones() {
            if k < n {
                by_key[k].push(i);
            }
        }
    }
    let mut b = Bnb {
        cands,
        by_key,
        key_rank,
        w,
        budget,
        nodes: 0,
        exhausted: false,
        best: bound,
        best_regions: None,
    };
    let mut chosen = vec![];
    b.go(coverable, &mut chosen, Cost::default());
    let solution = b.best_regions.map(|r| Solution {
        regions: r,
        cost: b.best,
    });
    ExactOutcome {
        solution,
        nodes: b.nodes,
        exhausted: b.exhausted,
    }
}
