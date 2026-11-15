//! Condition atoms: a small closed family of index conditions, enumerated from SPEC DATA ([`AtomFamily`]) and the
//! values the face's keys take (no slot, constant or form is known to the code).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

/// A bitset over the keys of one face (bit `k` = key `k` of the sorted key list).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Bits(pub Vec<u64>);

impl Bits {
    pub fn empty(n: usize) -> Bits {
        Bits(vec![0; n.div_ceil(64)])
    }
    pub fn full(n: usize) -> Bits {
        let mut b = Bits::empty(n);
        for k in 0..n {
            b.set(k);
        }
        b
    }
    pub fn set(&mut self, k: usize) {
        self.0[k / 64] |= 1u64 << (k % 64);
    }
    pub fn clear(&mut self, k: usize) {
        self.0[k / 64] &= !(1u64 << (k % 64));
    }
    pub fn get(&self, k: usize) -> bool {
        self.0[k / 64] >> (k % 64) & 1 == 1
    }
    pub fn count(&self) -> usize {
        self.0.iter().map(|w| w.count_ones() as usize).sum()
    }
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&w| w == 0)
    }
    pub fn and(&self, o: &Bits) -> Bits {
        Bits(self.0.iter().zip(&o.0).map(|(a, b)| a & b).collect())
    }
    pub fn and_not(&self, o: &Bits) -> Bits {
        Bits(self.0.iter().zip(&o.0).map(|(a, b)| a & !b).collect())
    }
    pub fn and_count(&self, o: &Bits) -> usize {
        self.0
            .iter()
            .zip(&o.0)
            .map(|(a, b)| (a & b).count_ones() as usize)
            .sum()
    }
    /// `self` is a subset of `o`.
    pub fn subset_of(&self, o: &Bits) -> bool {
        self.0.iter().zip(&o.0).all(|(a, b)| a & !b == 0)
    }
    pub fn ones(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(w, &x)| {
            (0..64usize).filter_map(move |b| (x >> b & 1 == 1).then_some(w * 64 + b))
        })
    }
}

/// One atom. The variant order is the deterministic tie-break among equally simple atoms: SCALE-FREE relations
/// between indices first (they do not depend on the size of the box), then constant bounds, parity, sums.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Atom {
    /// n_i <= n_j
    LeRel { i: usize, j: usize },
    /// n_i < n_j
    LtRel { i: usize, j: usize },
    /// n_i = n_j (i < j)
    EqRel { i: usize, j: usize },
    /// n_i = c
    EqConst { i: usize, c: i64 },
    /// n_i <= c
    LeConst { i: usize, c: i64 },
    /// n_i >= c
    GeConst { i: usize, c: i64 },
    /// n_i mod 2 = (odd as 0/1), rem_euclid
    Parity { i: usize, odd: bool },
    /// n_i + n_j <= c (i < j)
    SumLe { i: usize, j: usize, c: i64 },
}

impl Atom {
    pub fn eval(&self, k: &[i32]) -> bool {
        let v = |i: usize| i64::from(k[i]);
        match *self {
            Atom::LeRel { i, j } => v(i) <= v(j),
            Atom::LtRel { i, j } => v(i) < v(j),
            Atom::EqRel { i, j } => v(i) == v(j),
            Atom::EqConst { i, c } => v(i) == c,
            Atom::LeConst { i, c } => v(i) <= c,
            Atom::GeConst { i, c } => v(i) >= c,
            Atom::Parity { i, odd } => (v(i).rem_euclid(2) == 1) == odd,
            Atom::SumLe { i, j, c } => v(i) + v(j) <= c,
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Atom::LeRel { .. } => "le_rel",
            Atom::LtRel { .. } => "lt_rel",
            Atom::EqRel { .. } => "eq_rel",
            Atom::EqConst { .. } => "eq_const",
            Atom::LeConst { .. } => "le_const",
            Atom::GeConst { .. } => "ge_const",
            Atom::Parity { .. } => "parity",
            Atom::SumLe { .. } => "sum_le",
        }
    }
    pub fn text(&self) -> String {
        match *self {
            Atom::LeRel { i, j } => format!("n{i}<=n{j}"),
            Atom::LtRel { i, j } => format!("n{i}<n{j}"),
            Atom::EqRel { i, j } => format!("n{i}=n{j}"),
            Atom::EqConst { i, c } => format!("n{i}={c}"),
            Atom::LeConst { i, c } => format!("n{i}<={c}"),
            Atom::GeConst { i, c } => format!("n{i}>={c}"),
            Atom::Parity { i, odd } => format!("n{i} {}", if odd { "odd" } else { "even" }),
            Atom::SumLe { i, j, c } => format!("n{i}+n{j}<={c}"),
        }
    }
    pub fn json(&self) -> serde_json::Value {
        use serde_json::json;
        match *self {
            Atom::LeRel { i, j } | Atom::LtRel { i, j } | Atom::EqRel { i, j } => {
                json!({"kind": self.kind(), "i": i, "j": j})
            }
            Atom::EqConst { i, c } | Atom::LeConst { i, c } | Atom::GeConst { i, c } => {
                json!({"kind": self.kind(), "i": i, "c": c})
            }
            Atom::Parity { i, odd } => json!({"kind": self.kind(), "i": i, "odd": odd}),
            Atom::SumLe { i, j, c } => json!({"kind": self.kind(), "i": i, "j": j, "c": c}),
        }
    }
    pub fn weight(&self, w: &KindWeights) -> u64 {
        match self {
            Atom::LeRel { .. } | Atom::LtRel { .. } | Atom::EqRel { .. } => w.rel,
            Atom::EqConst { .. } | Atom::LeConst { .. } | Atom::GeConst { .. } => w.constant,
            Atom::Parity { .. } => w.parity,
            Atom::SumLe { .. } => w.sum,
        }
    }
}

/// Text of a conjunction (`true` for no atom).
pub fn condition_text(atoms: &[Atom]) -> String {
    if atoms.is_empty() {
        "true".into()
    } else {
        atoms.iter().map(Atom::text).collect::<Vec<_>>().join(" & ")
    }
}

/// Simplicity weights per atom kind (spec data). planfix (audit r2-planners ITEM 6): default weights rel 1, constant 1,
/// parity 2, sum 2 (simplicity order among equivalent conditions). Default not yet backed by an experiment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KindWeights {
    pub rel: u64,
    pub constant: u64,
    pub parity: u64,
    pub sum: u64,
}

impl Default for KindWeights {
    fn default() -> Self {
        KindWeights {
            rel: 1,
            constant: 1,
            parity: 2,
            sum: 2,
        }
    }
}

/// The closed atom family (spec data with a documented default: every kind on, constants = the values observed on
/// the face, which include each slot's floor value, plus `extra_constants`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AtomFamily {
    pub le_const: bool,
    pub ge_const: bool,
    pub eq_const: bool,
    pub le_rel: bool,
    pub lt_rel: bool,
    pub eq_rel: bool,
    pub parity: bool,
    pub sum_le: bool,
    /// Constants for `n_i <= c`, `n_i >= c`, `n_i = c`: the values slot i takes on the face (and the observed sums
    /// for `n_i + n_j <= c`).
    pub observed_constants: bool,
    /// Further constants (added to every slot's and every sum's list).
    pub extra_constants: Vec<i64>,
    /// Restrict atoms to these slots (default: every slot whose value varies on the face).
    pub slots: Option<Vec<usize>>,
    pub kind_weight: KindWeights,
}

impl Default for AtomFamily {
    fn default() -> Self {
        AtomFamily {
            le_const: true,
            ge_const: true,
            eq_const: true,
            le_rel: true,
            lt_rel: true,
            eq_rel: true,
            parity: true,
            sum_le: true,
            observed_constants: true,
            extra_constants: vec![],
            slots: None,
            kind_weight: KindWeights::default(),
        }
    }
}

/// Every atom of the family on the face, with its truth set; atoms true on every key or on none are dropped.
pub fn enumerate(fam: &AtomFamily, keys: &[Vec<i32>]) -> Result<Vec<(Atom, Bits)>, String> {
    let n = keys.len();
    let arity = keys.first().map_or(0, Vec::len);
    let slots: Vec<usize> = match &fam.slots {
        Some(s) => {
            let set: BTreeSet<usize> = s.iter().copied().collect();
            if let Some(&bad) = set.iter().find(|&&i| i >= arity && n > 0) {
                return Err(format!("atom slot {bad} outside the key arity {arity}"));
            }
            set.into_iter().collect()
        }
        None => (0..arity)
            .filter(|&i| keys.iter().any(|k| k[i] != keys[0][i]))
            .collect(),
    };
    let mut cands: Vec<Atom> = vec![];
    for &i in &slots {
        let mut vals: BTreeSet<i64> = fam.extra_constants.iter().copied().collect();
        if fam.observed_constants {
            vals.extend(keys.iter().map(|k| i64::from(k[i])));
        }
        for &c in &vals {
            if fam.eq_const {
                cands.push(Atom::EqConst { i, c });
            }
            if fam.le_const {
                cands.push(Atom::LeConst { i, c });
            }
            if fam.ge_const {
                cands.push(Atom::GeConst { i, c });
            }
        }
        if fam.parity {
            cands.push(Atom::Parity { i, odd: false });
            cands.push(Atom::Parity { i, odd: true });
        }
    }
    for (a, &i) in slots.iter().enumerate() {
        for &j in &slots[a + 1..] {
            if fam.le_rel {
                cands.push(Atom::LeRel { i, j });
                cands.push(Atom::LeRel { i: j, j: i });
            }
            if fam.lt_rel {
                cands.push(Atom::LtRel { i, j });
                cands.push(Atom::LtRel { i: j, j: i });
            }
            if fam.eq_rel {
                cands.push(Atom::EqRel { i, j });
            }
            if fam.sum_le {
                let mut vals: BTreeSet<i64> = fam.extra_constants.iter().copied().collect();
                if fam.observed_constants {
                    vals.extend(keys.iter().map(|k| i64::from(k[i]) + i64::from(k[j])));
                }
                for &c in &vals {
                    cands.push(Atom::SumLe { i, j, c });
                }
            }
        }
    }
    cands.sort();
    cands.dedup();
    let mut out = vec![];
    for a in cands {
        let mut b = Bits::empty(n);
        for (k, key) in keys.iter().enumerate() {
            if a.eval(key) {
                b.set(k);
            }
        }
        let c = b.count();
        if c > 0 && c < n {
            out.push((a, b));
        }
    }
    Ok(out)
}

/// One representative per distinct truth set: the simplest (weight, then the atom order).
pub fn representatives(all: &[(Atom, Bits)], w: &KindWeights) -> Vec<(Atom, Bits)> {
    let mut best: HashMap<&Bits, Atom> = HashMap::new();
    for (a, b) in all {
        best.entry(b)
            .and_modify(|cur| {
                if (a.weight(w), *a) < (cur.weight(w), *cur) {
                    *cur = *a;
                }
            })
            .or_insert(*a);
    }
    let mut out: Vec<(Atom, Bits)> = best.into_iter().map(|(b, a)| (a, b.clone())).collect();
    out.sort_by_key(|x| (x.0.weight(w), x.0));
    out
}
