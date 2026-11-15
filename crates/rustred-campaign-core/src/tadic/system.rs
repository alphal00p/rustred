//! The IBP SYSTEM of a seed set, with EXACT coefficients (`a + b d`, integers after scaling each row), from the
//! family text by the campaign's own generator (`Family::exact_row`: the L(L+1) operators d/dk_i . v_j, v_j over the
//! loop momenta and q, power shifts included), plus the generic seed shells.
//!
//! Engine equality (lane tadic item 2, WS/notes/implementer.tadic.md): the engine's recipe solve
//! (`rustred-core/src/node/recipe.rs` `build_seeds` + `solver::with_explicit_seeds`) instantiates, at every seed, the
//! rows of its source system (the same L(L+1) IBP operators of the compiled family) and drops columns in proved-zero
//! sectors; symmetry rows only when the recipe sets `symmetry_rows`. p* depends only on the row SPACE, so the
//! relevant equality is that of the spans at each seed; it is checked in the tests against engine records.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use crate::replay_base::algebra::Q;
use crate::replay_base::family::Family;
use crate::replay_base::sha256::sha256_hex;

/// Identity of a system (what a p* record speaks about: "inherent" is relative to THIS seed set).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SystemId {
    /// sha256 of the family text bytes
    pub family_sha256: String,
    /// sha256 of the canonical JSON of the SORTED seed list
    pub seed_set_sha256: String,
    pub seeds: usize,
    pub rows: usize,
    pub columns: usize,
    /// column entries removed because the key lies in a scaleless (zero) sector
    pub zero_entries_dropped: usize,
    /// "ibp" (only IBP rows) or a caller label for other row kinds
    pub kind: String,
}

/// Which seed / operator produced a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct RowLabel {
    pub seed: u32,
    pub op: u32,
}

/// A finite linear system: row i says `sum_k (a_k + b_k d) I(columns[col_k]) = 0` (integers; a row may be scaled by
/// any nonzero constant without changing anything).
#[derive(Clone, Debug)]
pub struct ExactSystem {
    pub arity: usize,
    pub columns: Vec<Vec<i64>>,
    pub rows: Vec<Vec<(u32, i128, i128)>>,
    pub labels: Vec<RowLabel>,
    pub seeds: Vec<Vec<i64>>,
    pub id: SystemId,
}

impl ExactSystem {
    /// A system from explicit rows (tests, constructed systems, other row kinds). Rows: (key, a, b) triples.
    pub fn from_rows(
        arity: usize,
        rows: &[Vec<(Vec<i64>, i128, i128)>],
        kind: &str,
    ) -> ExactSystem {
        let mut index: HashMap<Vec<i64>, u32> = HashMap::new();
        let mut columns: Vec<Vec<i64>> = Vec::new();
        let mut out = Vec::new();
        for r in rows {
            let mut acc: BTreeMap<u32, (i128, i128)> = BTreeMap::new();
            for (k, a, b) in r {
                let c = *index.entry(k.clone()).or_insert_with(|| {
                    columns.push(k.clone());
                    (columns.len() - 1) as u32
                });
                let e = acc.entry(c).or_insert((0, 0));
                e.0 += a;
                e.1 += b;
            }
            out.push(
                acc.into_iter()
                    .filter(|(_, (a, b))| *a != 0 || *b != 0)
                    .map(|(c, (a, b))| (c, a, b))
                    .collect(),
            );
        }
        let n = out.len();
        let id = SystemId {
            family_sha256: String::new(),
            seed_set_sha256: String::new(),
            seeds: 0,
            rows: n,
            columns: columns.len(),
            zero_entries_dropped: 0,
            kind: kind.into(),
        };
        ExactSystem {
            arity,
            columns,
            rows: out,
            labels: (0..n as u32).map(|i| RowLabel { seed: i, op: 0 }).collect(),
            seeds: Vec::new(),
            id,
        }
    }

    /// The sub-system of the rows whose seed index is in `keep` (seed indices into `self.seeds`); columns are
    /// re-indexed, the identity recomputed (the minimiser's "after" system without regenerating rows).
    pub fn restrict_to_seeds(&self, keep: &[usize]) -> ExactSystem {
        let keep_set: HashSet<u32> = keep.iter().map(|&s| s as u32).collect();
        let mut map: HashMap<u32, u32> = HashMap::new();
        let mut columns = Vec::new();
        let mut rows = Vec::new();
        let mut labels = Vec::new();
        for (r, l) in self.rows.iter().zip(&self.labels) {
            if !keep_set.contains(&l.seed) {
                continue;
            }
            rows.push(
                r.iter()
                    .map(|&(c, a, b)| {
                        let nc = *map.entry(c).or_insert_with(|| {
                            columns.push(self.columns[c as usize].clone());
                            (columns.len() - 1) as u32
                        });
                        (nc, a, b)
                    })
                    .collect(),
            );
            labels.push(*l);
        }
        let seeds: Vec<Vec<i64>> = keep.iter().map(|&s| self.seeds[s].clone()).collect();
        let id = SystemId {
            family_sha256: self.id.family_sha256.clone(),
            seed_set_sha256: seed_set_sha256(&seeds),
            seeds: seeds.len(),
            rows: rows.len(),
            columns: columns.len(),
            zero_entries_dropped: self.id.zero_entries_dropped,
            kind: self.id.kind.clone(),
        };
        // labels keep the ORIGINAL seed indices (into the parent's list); `seeds` is the kept list in `keep` order
        ExactSystem {
            arity: self.arity,
            columns,
            rows,
            labels,
            seeds,
            id,
        }
    }
}

/// A shell `S<up>m<down>`: `up` unit steps up and `down` unit steps down (multisets of slots; a slot never both up and
/// down), in the HARDER unit (+1 on a line slot, -1 on a numerator slot = one step "up" makes the integral harder).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ShellSpec {
    pub up: u32,
    pub down: u32,
}

/// Parse `S0`, `S1`, `S2`, `S3`, `Sm1`, `Sm2`, `S1m1`, `S2m1`, `S1m2`, ... (generic grammar `S<p>[m<q>]` / `Sm<q>`).
pub fn parse_shell(name: &str) -> Result<ShellSpec, String> {
    let rest = name
        .strip_prefix('S')
        .ok_or_else(|| format!("shell {name:?}: expected S<p>[m<q>]"))?;
    let (p, q) = match rest.split_once('m') {
        None => (rest, "0"),
        Some((a, b)) => (if a.is_empty() { "0" } else { a }, b),
    };
    let up = p
        .parse::<u32>()
        .map_err(|_| format!("shell {name:?}: bad up count"))?;
    let down = q
        .parse::<u32>()
        .map_err(|_| format!("shell {name:?}: bad down count"))?;
    Ok(ShellSpec { up, down })
}

fn multisets(n: usize, k: u32, avoid: &[usize], out: &mut Vec<Vec<usize>>) {
    fn rec(
        n: usize,
        k: u32,
        start: usize,
        avoid: &[usize],
        cur: &mut Vec<usize>,
        out: &mut Vec<Vec<usize>>,
    ) {
        if k == 0 {
            out.push(cur.clone());
            return;
        }
        for j in start..n {
            if avoid.contains(&j) {
                continue;
            }
            cur.push(j);
            rec(n, k - 1, j, avoid, cur, out);
            cur.pop();
        }
    }
    rec(n, k, 0, avoid, &mut Vec::new(), out);
}

/// The offsets of the shells, in order (ups lexicographic, then downs lexicographic; duplicates kept: the caller
/// deduplicates the seeds). Matches the engine's enumeration order for S0 S1 Sm1 S1m1 S2 S3 S2m1 Sm2.
pub fn shell_offsets(lines: &[bool], shells: &[ShellSpec]) -> Vec<Vec<i64>> {
    let n = lines.len();
    let unit = |j: usize| if lines[j] { 1 } else { -1 };
    let mut out = Vec::new();
    for s in shells {
        let mut ups = Vec::new();
        multisets(n, s.up, &[], &mut ups);
        for u in &ups {
            let mut downs = Vec::new();
            multisets(n, s.down, u, &mut downs);
            for dn in &downs {
                let mut off = vec![0i64; n];
                for &j in u {
                    off[j] += unit(j);
                }
                for &j in dn {
                    off[j] -= unit(j);
                }
                out.push(off);
            }
        }
    }
    out
}

/// Seeds = key + centre + shell offset, for every centre (offsets from the key; empty = the key itself), keeping
/// only valid keys of the family frame when `sector_filter` (every line >= 1, every numerator <= 0), deduplicated in
/// order of first appearance.
pub fn seeds_around(
    key: &[i64],
    centres: &[Vec<i64>],
    shells: &[ShellSpec],
    lines: &[bool],
    sector_filter: bool,
) -> Vec<Vec<i64>> {
    let offs = shell_offsets(lines, shells);
    let own = [vec![0i64; key.len()]];
    let centres: &[Vec<i64>] = if centres.is_empty() { &own } else { centres };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for c in centres {
        for o in &offs {
            let s: Vec<i64> = (0..key.len()).map(|j| key[j] + c[j] + o[j]).collect();
            if sector_filter && (0..key.len()).any(|j| if lines[j] { s[j] < 1 } else { s[j] > 0 }) {
                continue;
            }
            if seen.insert(s.clone()) {
                out.push(s);
            }
        }
    }
    out
}

/// sha256 of the canonical JSON of the sorted seed list.
pub fn seed_set_sha256(seeds: &[Vec<i64>]) -> String {
    let mut s = seeds.to_vec();
    s.sort();
    sha256_hex(serde_json::to_string(&s).unwrap_or_default().as_bytes())
}

fn lcm(a: i128, b: i128) -> Option<i128> {
    let (mut x, mut y) = (a.abs(), b.abs());
    while y != 0 {
        (x, y) = (y, x % y);
    }
    (a / x).checked_mul(b).map(i128::abs)
}

/// The IBP system at `seeds`: every operator at every seed (repeated seeds once), coefficients exact, columns in a
/// zero (scaleless) sector dropped when `drop_zero_sectors` (engine semantics), empty rows skipped.
pub fn build_ibp_system(
    family: &Family,
    seeds: &[Vec<i64>],
    drop_zero_sectors: bool,
) -> Result<ExactSystem, String> {
    let n = family.n();
    // operator rows: offset -> linear form [const, y_0.., y_{n-1}, d]
    let ops: Vec<BTreeMap<Vec<i64>, Vec<Q>>> = (0..family.n_ops())
        .map(|o| family.exact_row(o))
        .collect::<Result<_, _>>()?;
    let mut index: HashMap<Vec<i64>, u32> = HashMap::new();
    let mut columns: Vec<Vec<i64>> = Vec::new();
    let mut zero_cache: HashMap<Vec<bool>, bool> = HashMap::new();
    let mut rows = Vec::new();
    let mut labels = Vec::new();
    let mut dropped = 0usize;
    let mut seen = HashSet::new();
    let mut kept_seeds = Vec::new();
    for y in seeds {
        if y.len() != n {
            return Err(format!(
                "seed of arity {} for a family of {n} slots",
                y.len()
            ));
        }
        if !seen.insert(y.clone()) {
            continue;
        }
        let si = kept_seeds.len() as u32;
        kept_seeds.push(y.clone());
        for (op, tab) in ops.iter().enumerate() {
            // coefficient at this seed: (num_a + num_b d) / den per column, exact rationals
            let mut terms: Vec<(Vec<i64>, Q, Q)> = Vec::new();
            for (delta, form) in tab {
                let key: Vec<i64> = y.iter().zip(delta).map(|(a, b)| a + b).collect();
                if drop_zero_sectors {
                    let mask: Vec<bool> = key.iter().map(|&x| x > 0).collect();
                    let z = *zero_cache
                        .entry(mask.clone())
                        .or_insert_with(|| family.zero_mask(&mask));
                    if z {
                        dropped += 1;
                        continue;
                    }
                }
                let mut a = form[0];
                for i in 0..n {
                    if !form[i + 1].is_zero() && y[i] != 0 {
                        a = qadd(a, qmul(form[i + 1], Q::int(y[i] as i128))?)?;
                    }
                }
                let b = form[n + 1];
                if a.is_zero() && b.is_zero() {
                    continue;
                }
                terms.push((key, a, b));
            }
            if terms.is_empty() {
                continue;
            }
            let mut den = 1i128;
            for (_, a, b) in &terms {
                den = lcm(den, a.d).ok_or("row denominator overflow")?;
                den = lcm(den, b.d).ok_or("row denominator overflow")?;
            }
            let mut acc: BTreeMap<u32, (i128, i128)> = BTreeMap::new();
            for (key, a, b) in terms {
                let c = *index.entry(key.clone()).or_insert_with(|| {
                    columns.push(key);
                    (columns.len() - 1) as u32
                });
                let ia =
                    a.n.checked_mul(den / a.d)
                        .ok_or("row coefficient overflow")?;
                let ib =
                    b.n.checked_mul(den / b.d)
                        .ok_or("row coefficient overflow")?;
                let e = acc.entry(c).or_insert((0, 0));
                e.0 += ia;
                e.1 += ib;
            }
            let row: Vec<(u32, i128, i128)> = acc
                .into_iter()
                .filter(|(_, (a, b))| *a != 0 || *b != 0)
                .map(|(c, (a, b))| (c, a, b))
                .collect();
            if !row.is_empty() {
                rows.push(row);
                labels.push(RowLabel {
                    seed: si,
                    op: op as u32,
                });
            }
        }
    }
    let id = SystemId {
        family_sha256: family.source_digest().to_string(),
        seed_set_sha256: seed_set_sha256(&kept_seeds),
        seeds: kept_seeds.len(),
        rows: rows.len(),
        columns: columns.len(),
        zero_entries_dropped: dropped,
        kind: "ibp".into(),
    };
    Ok(ExactSystem {
        arity: n,
        columns,
        rows,
        labels,
        seeds: kept_seeds,
        id,
    })
}

fn qadd(a: Q, b: Q) -> Result<Q, String> {
    let n =
        a.n.checked_mul(b.d)
            .and_then(|x| b.n.checked_mul(a.d).and_then(|y| x.checked_add(y)));
    let d = a.d.checked_mul(b.d);
    match (n, d) {
        (Some(n), Some(d)) => qnorm(n, d),
        _ => Err("exact rational overflow".into()),
    }
}

fn qmul(a: Q, b: Q) -> Result<Q, String> {
    match (a.n.checked_mul(b.n), a.d.checked_mul(b.d)) {
        (Some(n), Some(d)) => qnorm(n, d),
        _ => Err("exact rational overflow".into()),
    }
}

fn qnorm(n: i128, d: i128) -> Result<Q, String> {
    if d == 0 {
        return Err("division by zero".into());
    }
    let (mut x, mut y) = (n.abs(), d.abs());
    while y != 0 {
        (x, y) = (y, x % y);
    }
    let g = x.max(1);
    let s = if d < 0 { -1 } else { 1 };
    Ok(Q {
        n: s * n / g,
        d: s * d / g,
    })
}
