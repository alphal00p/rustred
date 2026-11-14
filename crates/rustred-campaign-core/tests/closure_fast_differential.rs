//! M5 (audit loop-A): randomized differential test of the closure's own dispatch index (`closure::FastDispatch`)
//! against the core's `Dispatcher::first_match` / `first_match_slow`, and of its exclusion / reuse paths against a
//! direct reference (tables in priority, records in order, excluded records skipped, the first record whose stage is
//! not `NotSelected` stops). Seeded; arities 5..=20; 1-5 tables with random (interleaved) priorities; fixed slots,
//! equations, exception branches, non-canonical target patterns, chart-less domains, elided witnesses (Blocked
//! winners). Default suite (the full large key sets: ignored release tests in closure_t178.rs / closure_t266.rs).

use rustred_campaign_core::closure::FastDispatch;
use rustred_campaign_core::cover::{Dispatch, Dispatcher, RuleRef, Table};
use rustred_campaign_core::domain::{CaseDomain, Stage, TargetMap, TargetSlot};
use rustred_campaign_core::poly::Poly;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

/// The reference first match with exclusions (positions in dispatch order).
fn reference(disp: &Dispatcher, key: &[i32], excluded: &[(usize, usize)]) -> Dispatch {
    if let Some(mask) = disp.chart()
        && let Some(slot) = key
            .iter()
            .enumerate()
            .position(|(i, &x)| (mask[i] && x < 1) || (!mask[i] && x > 0))
    {
        return Dispatch::OutsideChart { slot };
    }
    for &t in disp.priority() {
        let tab = &disp.tables()[t];
        for r in 0..tab.len() {
            if excluded.contains(&(t, r)) {
                continue;
            }
            let v = tab.domains[r].applies(key);
            let at = RuleRef {
                table: t,
                record: r,
                label: tab.labels[r],
            };
            match v.stage() {
                Stage::Applies => return Dispatch::Covered(at),
                Stage::NotSelected => {}
                Stage::Blocked => return Dispatch::Blocked { at, reason: v },
                Stage::Undecided => return Dispatch::Undecided { at, verdict: v },
            }
        }
    }
    Dispatch::Uncovered
}

#[test]
fn fast_equals_core_randomized() {
    let mut rng = Lcg(0x5eed_f457);
    let mut checked = 0u64;
    let mut kinds = [0u64; 5];
    let mut excl_changed = 0u64;
    for case in 0..240 {
        let n = 5 + rng.below(16) as usize; // 5..=20
        let nl = 2 + rng.below((n - 2) as u64) as usize;
        let chart: Vec<bool> = (0..n).map(|s| s < nl).collect();
        let n_tables = 1 + rng.below(5) as usize;
        let mut tabs = vec![];
        for t in 0..n_tables {
            let mut tab = Table::new(format!("t{t}"), n);
            let nrec = 1 + rng.below(10) as usize;
            for r in 0..nrec {
                let mut d = if rng.below(5) == 0 {
                    CaseDomain::free(n, None)
                } else {
                    CaseDomain::free(n, Some(chart.clone()))
                };
                for s in 0..n {
                    if rng.below(3) == 0 {
                        d.fixed[s] = Some(if chart[s] {
                            1 + rng.below(2) as i32
                        } else {
                            -(rng.below(2) as i32)
                        });
                    }
                }
                if rng.below(6) == 0 {
                    d.witness_elided = true;
                }
                if rng.below(5) == 0 {
                    d.equations.push(Poly::parse_d_free("n0-n1", n).unwrap());
                }
                if rng.below(5) == 0 {
                    let s = nl + rng.below((n - nl) as u64) as usize;
                    d.exceptions
                        .push(vec![Poly::parse_d_free(&format!("n{s}+1"), n).unwrap()]);
                }
                if rng.below(6) == 0 && d.fixed[0].is_none() {
                    d.target = TargetMap::Pattern(
                        (0..n)
                            .map(|s| TargetSlot::Param {
                                param: s,
                                offset: (s == 0) as i64,
                            })
                            .collect(),
                    );
                }
                tab.push((r as i64) * 7 + 3, d).unwrap();
            }
            tabs.push(tab);
        }
        let mut prio: Vec<usize> = (0..n_tables).collect();
        for i in (1..prio.len()).rev() {
            let j = rng.below(i as u64 + 1) as usize;
            prio.swap(i, j);
        }
        let disp = Dispatcher::new(tabs, prio).unwrap();
        let fast = FastDispatch::new(&disp);
        let (mut b, mut st) = (vec![], vec![]);
        for _ in 0..400 {
            let key: Vec<i32> = (0..n)
                .map(|s| {
                    if chart[s] {
                        if rng.below(40) == 0 { 0 } else { 1 + rng.below(2) as i32 } // rarely 0: outside the chart
                    } else {
                        -(rng.below(3) as i32) + (rng.below(20) == 0) as i32
                    }
                })
                .collect();
            let f = fast.first_match_with(&key, &mut b, &mut st);
            assert_eq!(f, disp.first_match(&key).unwrap(), "case {case} key {key:?}");
            assert_eq!(f, disp.first_match_slow(&key).unwrap(), "case {case} key {key:?}");
            assert_eq!(f, reference(&disp, &key, &[]), "case {case} key {key:?}");
            kinds[match f {
                Dispatch::Covered(_) => 0,
                Dispatch::Uncovered => 1,
                Dispatch::Blocked { .. } => 2,
                Dispatch::Undecided { .. } => 3,
                Dispatch::OutsideChart { .. } => 4,
            }] += 1;
            // exclusions: drop the winner (and sometimes a random record) at this key
            let mut excl: Vec<(usize, usize)> = vec![];
            if let Some(w) = f.winner().or(match &f {
                Dispatch::Blocked { at, .. } | Dispatch::Undecided { at, .. } => Some(*at),
                _ => None,
            }) {
                excl.push((w.table, w.record));
            }
            if rng.below(2) == 0 {
                let t = rng.below(n_tables as u64) as usize;
                let r = rng.below(disp.tables()[t].len() as u64) as usize;
                excl.push((t, r));
            }
            let mut pos: Vec<u32> = excl
                .iter()
                .map(|&(t, r)| {
                    fast.position(RuleRef {
                        table: t,
                        record: r,
                        label: 0,
                    })
                })
                .collect();
            pos.sort_unstable();
            pos.dedup();
            let fe = fast.first_match_ex(&key, 0, None, &pos, &mut b, &mut st).0;
            let re = reference(&disp, &key, &excl);
            assert_eq!(fe, re, "case {case} key {key:?} excluded {excl:?}");
            excl_changed += (fe != f) as u64;
            // reuse: with the true stop as `old_stop` and every position reusable, the verdict is unchanged
            let stop = match &f {
                Dispatch::Covered(r) => fast.position(*r),
                Dispatch::Blocked { at, .. } | Dispatch::Undecided { at, .. } => fast.position(*at),
                _ => u32::MAX,
            };
            let all = |_: u32| true;
            let fr = fast.first_match_ex(&key, stop, Some(&all), &[], &mut b, &mut st);
            assert_eq!(fr.0, f, "case {case} key {key:?} reuse");
            assert!(fr.1 <= 1, "reuse evaluates at most the stopping record");
            checked += 1;
        }
    }
    println!(
        "fast == core == slow == reference on {checked} keys; covered/uncovered/blocked/undecided/outside = {kinds:?}; \
         exclusions changed the verdict on {excl_changed}"
    );
    assert!(kinds[0] > 0 && kinds[1] > 0 && kinds[2] > 0 && kinds[4] > 0);
}
