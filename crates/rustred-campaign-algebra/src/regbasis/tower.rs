//! THE TOWER TEST (POLES_OBJECTIVE 2 (C); notes THE TOWER TEST): with L regular on a band and X = M_X L (X = the
//! published masters when their map exists, else the closure's terminals), every band key has a pole <= z = v(det M_X)
//! = length(N^X / O^m) on X. The cheap test composes only the DOT-FREE keys (every line at power 1) rank by rank: the
//! tower lattice is a SUBMODULE of the band lattice of the same ranks, so its z_r is a LOWER bound (exact on the ranks
//! seeded from a certified band). Per rank: z_r, the increment, the elementary divisors, the max pole on X, the first
//! dot-free key in the preference order that enlarges the lattice ("the first key showing a new pole"). Verdict "z
//! through rank r" (the last rank completed within the budget).

use std::cmp::Ordering;
use std::time::Instant;

use serde_json::{Value, json};

use rustred_campaign_core::key::Key;
use rustred_campaign_core::regbasis::record::{Binding, TOWER_SCHEMA, seal};
use rustred_campaign_core::regbasis::spec::CandidateOrder;
use rustred_campaign_core::regbasis::{
    Compose, Preference, RegBasisSpec, dot_free_keys, sort_by_preference,
};

use crate::valuation::D0;

use super::build::{Published, QMap, elementary, scaled, stream_parsed};
use super::lattice::HermiteLattice;
use super::series::{Laurent, t_vars};

/// A band lattice to seed the tower with: the lattice of the band ranks `<= rank` on the reference basis, scaled by
/// `t^q` (socle q).
pub struct TowerSeed {
    pub rank: u32,
    pub lattice: HermiteLattice,
    pub q: u32,
}

impl TowerSeed {
    /// The seed from a construction: the band lattice through its last complete rank, on P when the construction had
    /// the published map and `on_published`, else on T. `None` when no rank is complete.
    pub fn from_build(b: &super::build::BuildResult, on_published: bool) -> Option<TowerSeed> {
        let r = b.r_complete?;
        if on_published {
            let (v, q) = b.prefix_p.as_ref()?;
            Some(TowerSeed {
                rank: r,
                lattice: v.get(r as usize)?.clone(),
                q: *q,
            })
        } else {
            Some(TowerSeed {
                rank: r,
                lattice: b.prefix_t.get(r as usize)?.clone(),
                q: b.q,
            })
        }
    }
}

pub struct TowerInput<'a> {
    pub chart: &'a [bool],
    pub terminals: &'a [Key],
    pub cmp: &'a dyn Fn(&[i32], &[i32]) -> Ordering,
    /// Highest rank of the tower (the band's r_max unless the spec caps it).
    pub r_max: u32,
    pub spec: &'a RegBasisSpec,
    /// Reference basis: the published masters (when given), else the terminals.
    pub published: Option<&'a Published>,
    pub seed: Option<TowerSeed>,
    pub binding: Option<Binding>,
}

#[derive(Clone, Debug)]
pub struct TowerRank {
    pub rank: u32,
    pub keys_total: usize,
    pub keys_composed: usize,
    pub undecided: usize,
    /// z_r on the reference basis.
    pub length: u64,
    pub increment: u64,
    pub elementary: Vec<u32>,
    pub max_pole: u32,
    pub first_new_key: Option<Key>,
    pub first_new_key_pole: Option<u32>,
    /// `band` (seeded, exact) or `tower` (dot-free keys, lower bound).
    pub source: String,
    pub complete: bool,
}

#[derive(Clone, Debug)]
pub struct TowerResult {
    pub reference: String,
    pub ranks: Vec<TowerRank>,
    pub z_through: Option<(u64, u32)>,
    pub verdict: String,
    pub time_decided: bool,
    pub record: Value,
}

/// Run the tower test.
pub fn tower(compose: &mut dyn Compose, inp: &TowerInput<'_>) -> Result<TowerResult, String> {
    let n = inp.terminals.len();
    let d0 = D0::parse(&inp.spec.d0).map_err(|e| e.to_string())?;
    let tv = t_vars();
    let mut qmap = match inp.published {
        Some(p) => Some(QMap::new(&p.q, n, d0.value(), &tv)?),
        None => None,
    };
    let m = qmap.as_ref().map_or(n, |q| q.m);
    let abs_prec: i64 = 1.max(qmap.as_ref().map_or(0, |q| i64::from(q.pole)));
    let reference = if inp.published.is_some() { "P" } else { "T" }.to_string();
    let r_top = inp
        .spec
        .tower_max_rank
        .map_or(inp.r_max, |c| c.min(inp.r_max));
    let t0 = Instant::now();
    let pref = Preference {
        chart: inp.chart,
        order: CandidateOrder::Node,
        terminals: inp.terminals,
        terminals_first: false,
        cmp: inp.cmp,
    };
    let (mut lat, mut q, seed_rank) = match &inp.seed {
        Some(s) if inp.spec.tower_seed_with_band => (s.lattice.clone(), s.q, Some(s.rank)),
        _ => (HermiteLattice::new(m, 0, tv.clone()), 0, None),
    };
    if lat.n() != m {
        return Err(format!(
            "tower seed of rank {} on a reference basis of size {m}",
            lat.n()
        ));
    }
    let mut out: Vec<TowerRank> = vec![];
    let mut prev = lat.length();
    let mut time_decided = false;
    let mut z_through = None;
    for r in 0..=r_top {
        if seed_rank.is_some_and(|s| r <= s) {
            if seed_rank == Some(r) {
                let len = lat.length();
                out.push(TowerRank {
                    rank: r,
                    keys_total: 0,
                    keys_composed: 0,
                    undecided: 0,
                    length: len,
                    increment: len,
                    elementary: elementary(&lat, q),
                    max_pole: elementary(&lat, q).first().copied().unwrap_or(0),
                    first_new_key: None,
                    first_new_key_pole: None,
                    source: format!("band ranks <= {r} (seed, exact)"),
                    complete: true,
                });
                z_through = Some((len, r));
                prev = len;
            }
            continue;
        }
        if inp
            .spec
            .tower_max_seconds
            .is_some_and(|s| t0.elapsed().as_secs_f64() > s)
        {
            time_decided = true;
            break;
        }
        let mut keys = dot_free_keys(inp.chart, r);
        sort_by_preference(&mut keys, &pref);
        let total = keys.len();
        if let Some(c) = inp.spec.tower_max_keys_per_rank {
            keys.truncate(c);
        }
        let mut vecs: Vec<Option<Vec<Laurent>>> = vec![None; keys.len()];
        stream_parsed(
            compose,
            &keys,
            inp.spec.chunk_keys,
            abs_prec,
            &tv,
            |i, res| {
                if let Ok(c) = res {
                    vecs[i] = Some(c);
                }
                Ok(())
            },
        )?;
        let mut undecided = 0usize;
        let mut max_pole = 0u32;
        let mut first: Option<(Key, u32)> = None;
        for (k, v) in keys.iter().zip(vecs) {
            let Some(c) = v else {
                undecided += 1;
                continue;
            };
            let x: Vec<Laurent> = match qmap.as_mut() {
                Some(qm) => qm.apply(&c, 0),
                None => c,
            };
            if !x.iter().all(Laurent::decides_pole) {
                undecided += 1;
                continue;
            }
            let p = x.iter().map(Laurent::pole_order).max().unwrap_or(0);
            max_pole = max_pole.max(p);
            if p > q {
                lat.rescale(p - q);
                q = p;
            }
            if lat.insert(&scaled(&x, q, q)?)? > 0 && first.is_none() {
                first = Some((k.clone(), p));
            }
        }
        let len = lat.length();
        let complete = undecided == 0 && keys.len() == total;
        out.push(TowerRank {
            rank: r,
            keys_total: total,
            keys_composed: keys.len(),
            undecided,
            length: len,
            increment: len.saturating_sub(prev),
            elementary: elementary(&lat, q),
            max_pole,
            first_new_key: first.as_ref().map(|x| x.0.clone()),
            first_new_key_pole: first.map(|x| x.1),
            source: "tower (dot-free keys; lower bound of the band value)".into(),
            complete,
        });
        prev = len;
        if complete && z_through.is_none_or(|(_, zr)| zr + 1 == r) {
            z_through = Some((len, r));
        }
    }
    let verdict = match z_through {
        Some((z, r)) => format!(
            "z = {z} through rank {r} on {reference} (lower bound beyond the seeded band ranks)"
        ),
        None => format!("no rank completed on {reference}"),
    };
    let increments_gt1: Vec<u32> = out
        .iter()
        .filter(|x| x.source.starts_with("tower") && x.increment > 1)
        .map(|x| x.rank)
        .collect();
    let mut rec = json!({
        "schema": TOWER_SCHEMA,
        "binding": inp.binding.as_ref().map(|b| b.to_value()),
        "reference": reference,
        "r_max": inp.r_max,
        "r_top": r_top,
        "verdict": verdict,
        "time_decided": time_decided,
        "increments_above_one_at_ranks": increments_gt1,
        "ranks": out.iter().map(|x| json!({
            "rank": x.rank, "keys_total": x.keys_total, "keys_composed": x.keys_composed, "undecided": x.undecided,
            "z": x.length, "increment": x.increment, "elementary": x.elementary, "max_pole": x.max_pole,
            "first_new_key": x.first_new_key, "first_new_key_pole": x.first_new_key_pole, "source": x.source,
            "complete": x.complete,
        })).collect::<Vec<_>>(),
    });
    if let Some(b) = inp.binding.as_ref() {
        b.validate()?;
        rec = seal(rec)?.0;
    }
    Ok(TowerResult {
        reference,
        ranks: out,
        z_through,
        verdict,
        time_decided,
        record: rec,
    })
}
