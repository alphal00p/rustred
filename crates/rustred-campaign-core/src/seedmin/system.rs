//! Regularity check (ii): the SYSTEM's best route-pole order p* (lane tadic, interfaces/tadic.md 4) behind the
//! minimiser's [`SystemPoles`] trait. Per key the system of the FIRST list it is asked about (the start list of that
//! key's minimisation) is built ONCE with the campaign's own IBP generator, its exact p* is the bound, and every later
//! list is a `restrict_to_seeds` sub-system judged by `tadic::keeps_regularity` against that bound. Only EXACT p*
//! values decide (anything else = undecided -> the policy's `on_undecided_regularity`). Generic over the oracle: the
//! Symbolica-backed `rustred_campaign_algebra::tadic::SymbolicaTadic` (this crate never links Symbolica).
//! TRICKS 17 / POLES_OBJECTIVE: the BINDING test is (i), the delivered relation's pole order; (ii) only says whether a
//! regular variant still exists in the reduced system. Optional: policy `system_regularity` (default off: the cost of
//! one exact p* grows with the system: measured on lane tadic's systems, see the notes).

use std::collections::BTreeMap;
use std::time::Instant;

use super::{Key, Offset, SystemPoles, add};
use crate::replay_base::family::Family;
use crate::tadic::{
    Arith, ExactSystem, Interest, PStar, PoleOrderOracle, PoleQuery, Rat, TadicBudget, Verdict,
    build_ibp_system, keeps_regularity,
};

/// One key's cached full system and bound.
struct Full {
    seeds: Vec<Offset>,
    system: ExactSystem,
    bound: Option<PStar>,
}

/// The adapter.
pub struct TadicSystem<'a> {
    oracle: &'a dyn PoleOrderOracle,
    family: &'a Family,
    /// The chart mask (true = line): seeds outside the top sector at the key are dropped, as the engine's recipe
    /// sector filter does (the same system as the solve's).
    lines: Vec<bool>,
    /// `lower(key, target)`: may `key` appear on the right-hand side for `target` (the node comparator).
    lower: &'a dyn Fn(&[i64], &[i64]) -> bool,
    budget: TadicBudget,
    drop_zero_sectors: bool,
    cache: BTreeMap<Key, Full>,
    /// Seconds per oracle call (cost record: the default of `system_regularity` is set from it).
    pub seconds: Vec<f64>,
}

impl<'a> TadicSystem<'a> {
    pub fn new(
        oracle: &'a dyn PoleOrderOracle,
        family: &'a Family,
        lines: Vec<bool>,
        lower: &'a dyn Fn(&[i64], &[i64]) -> bool,
        budget: TadicBudget,
        drop_zero_sectors: bool,
    ) -> TadicSystem<'a> {
        TadicSystem {
            oracle,
            family,
            lines,
            lower,
            budget,
            drop_zero_sectors,
            cache: BTreeMap::new(),
            seconds: vec![],
        }
    }

    /// The offsets of `list` whose seed lies in the top sector at `key` (list order kept).
    fn in_sector(&self, key: &[i32], list: &[Offset]) -> Vec<Offset> {
        list.iter()
            .filter(|o| {
                add(key, o)
                    .iter()
                    .zip(&self.lines)
                    .all(|(&v, &l)| if l { v >= 1 } else { v <= 0 })
            })
            .cloned()
            .collect()
    }

    fn build(&self, key: &[i32], seeds_rel: &[Offset]) -> Option<ExactSystem> {
        let seeds: Vec<Vec<i64>> = seeds_rel
            .iter()
            .map(|o| add(key, o).into_iter().map(i64::from).collect())
            .collect();
        build_ibp_system(self.family, &seeds, self.drop_zero_sectors).ok()
    }
}

impl SystemPoles for TadicSystem<'_> {
    fn available(&self) -> bool {
        true
    }

    fn keeps_regularity(
        &mut self,
        before: &[Offset],
        after: &[Offset],
        key: &[i32],
        d0: &str,
    ) -> Option<bool> {
        let d0 = Rat::parse(d0)?;
        let target: Vec<i64> = key.iter().map(|&x| i64::from(x)).collect();
        let lower = self.lower;
        let pred = |k: &[i64]| lower(k, &target);
        let q = PoleQuery {
            target: &target,
            lower: &pred,
            d0,
            interest: Interest::AllLower,
            arith: Arith::Exact,
            budget: self.budget.clone(),
            want_relation: false,
        };
        // the full system: built from the first list asked about at this key (every later list is a subset of it)
        let before = self.in_sector(key, before);
        let after = self.in_sector(key, after);
        let rebuild = match self.cache.get(&key.to_vec()) {
            None => true,
            Some(f) => after.iter().any(|o| !f.seeds.contains(o)),
        };
        if rebuild {
            let t0 = Instant::now();
            let system = self.build(key, &before)?;
            let r = self.oracle.best_pole_order(&system, &q);
            self.seconds.push(t0.elapsed().as_secs_f64());
            let bound = match (&r.verdict, r.p_star) {
                (Verdict::Exact, Some(p)) => Some(p),
                _ => None,
            };
            self.cache.insert(
                key.to_vec(),
                Full {
                    seeds: before.clone(),
                    system,
                    bound,
                },
            );
        }
        let f = self.cache.get(&key.to_vec())?;
        let bound = f.bound?;
        let keep: Vec<usize> = after
            .iter()
            .filter_map(|o| f.seeds.iter().position(|s| s == o))
            .collect();
        let sub = f.system.restrict_to_seeds(&keep);
        let t0 = Instant::now();
        let v = keeps_regularity(self.oracle, &f.system, &sub, &q, Some(bound));
        self.seconds.push(t0.elapsed().as_secs_f64());
        v.kept
    }

    fn name(&self) -> String {
        "tadic::keeps_regularity (system p*, lane tadic)".into()
    }
}
