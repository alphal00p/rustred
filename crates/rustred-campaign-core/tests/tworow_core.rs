//! Lane tworow (TRICKS 8): small exact combinations of symmetry rows. Constructed cases on small families (keys where
//! the one-element rows at the key do not descend and a pair does), refusal cases, the shift / policy forms, region
//! forms. Real nodes: tests/tworow_real.rs (ignored, data paths by environment).

use std::cmp::Ordering;

use rustred_campaign_core::import::symmetry::RelationCheck;
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{
    DeriveOptions, SymRowMode, VerifiedGroup, rows_at, verify_derived,
};
use rustred_campaign_core::termmin::{SymRelation, symmetry_relation_at_key};
use rustred_campaign_core::tworow::rows::rows_pruned;
use rustred_campaign_core::tworow::{
    KeyOutcome, KeyResult, PairShift, Reject, RowSpec, ShiftOp, TwoRowPolicy, apply_combination,
    import_check, region_forms, search_key,
};
use rustred_campaign_core::worklist::items::Face;
use serde_json::json;

const SUNSET: &str = "I(name(S),loops(k1,k2),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),prop(D3,(k1+k2+q)^2,1),prop(D4,(k1+q)^2,0),prop(D5,(k2+q)^2,0),gram(q,q,1))";
const BANANA3: &str = "I(name(B),loops(k1,k2,k3),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),prop(D3,(k3)^2,1),prop(D4,(k1+k2+k3+q)^2,1),prop(D5,(k1+q)^2,0),prop(D6,(k2+q)^2,0),prop(D7,(k3+q)^2,0),prop(D8,(k1-k2)^2,0),prop(D9,(k1-k3)^2,0),gram(q,q,1))";

/// A 3-loop family built to show the F8 mechanism: a sunset in (k1, k2) with a bubble (k3) on the k2 line. The
/// reflection k1 -> -k1-k2-q swaps D1 <-> D3, fixes D2, D4, D5, D7 = (k2+q)^2 and D8 = (k3+q)^2, and maps
/// D6 = (k1+q)^2 -> -D6 - D7 + lines + 1 and D9 = (k1+k3)^2 -> -D9 - D8 + lines + 1 (non-pure, the invariant slot with
/// coefficient -1, as the hand case's g2 at its key and shifted key).
const SUNBUB: &str = "I(name(SB),loops(k1,k2,k3),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),prop(D3,(k1+k2+q)^2,1),prop(D4,(k3)^2,1),prop(D5,(k2+k3)^2,1),prop(D6,(k1+q)^2,0),prop(D7,(k2+q)^2,0),prop(D8,(k3+q)^2,0),prop(D9,(k1+k3)^2,0),gram(q,q,1))";

type CmpFn = Box<dyn Fn(&[i32], &[i32]) -> Ordering + Sync>;

struct Fam {
    group: VerifiedGroup,
    chart: Vec<bool>,
    cmp: CmpFn,
}

/// A total order of the test (the comparator is a parameter of the step): more lines present, then more dots, then
/// higher rank are HIGHER; then the weighted numerator content `sum w_i (-n_i)`; then the key itself.
fn weighted_order(chart: Vec<bool>, w: Vec<i64>) -> CmpFn {
    Box::new(move |a: &[i32], b: &[i32]| {
        let m = |k: &[i32]| {
            let (mut lines, mut dots, mut rank, mut wt) = (0i64, 0i64, 0i64, 0i64);
            for (i, (&v, &l)) in k.iter().zip(&chart).enumerate() {
                if l {
                    if v >= 1 {
                        lines += 1;
                        dots += v as i64 - 1;
                    }
                } else if v < 0 {
                    rank += -v as i64;
                    wt += w[i] * -v as i64;
                }
            }
            (lines, dots, rank, wt)
        };
        m(a).cmp(&m(b)).then_with(|| a.cmp(b))
    })
}

fn fam(text: &str, lines: usize) -> Fam {
    let f = Family::parse(text).unwrap();
    let n = f.n();
    let chart: Vec<bool> = (0..n).map(|i| i < lines).collect();
    let group = verify_derived(&f, &chart, &DeriveOptions::default()).unwrap();
    let c = NodeComparator::legacy(n, None).unwrap();
    Fam {
        group,
        chart,
        cmp: Box::new(move |a: &[i32], b: &[i32]| c.cmp(a, b)),
    }
}

/// SUNBUB with the order that ranks numerator content on D6 lowest, D9 below D8, D7 highest (slots 5, 8, 7, 6).
fn sunbub() -> Fam {
    let f = Family::parse(SUNBUB).unwrap();
    let chart: Vec<bool> = (0..f.n()).map(|i| i < 5).collect();
    let group = verify_derived(&f, &chart, &DeriveOptions::default()).unwrap();
    Fam {
        group,
        cmp: weighted_order(chart.clone(), vec![0, 0, 0, 0, 0, 0, 100, 2, 1]),
        chart,
    }
}

/// Box keys: lines in 1..=max_line, numerators in -max_rank..=0 with total rank <= max_rank (deterministic order).
fn box_keys(chart: &[bool], max_line: i32, max_rank: i32) -> Vec<Vec<i32>> {
    let mut out = vec![vec![]];
    for &l in chart {
        let mut next = vec![];
        for k in &out {
            let vals: Vec<i32> = if l {
                (1..=max_line).collect()
            } else {
                (-max_rank..=0).rev().collect()
            };
            for v in vals {
                let mut k2: Vec<i32> = k.clone();
                k2.push(v);
                next.push(k2);
            }
        }
        out = next;
    }
    out.retain(|k| {
        k.iter()
            .zip(chart)
            .filter(|(_, l)| !**l)
            .map(|(v, _)| -v)
            .sum::<i32>()
            <= max_rank
    });
    out
}

/// Keys of the box where route 0b (every element's rows at the key together) gives NO relation, with the tworow result.
fn keys_needing_more(f: &Fam, keys: &[Vec<i32>], policy: &TwoRowPolicy) -> Vec<KeyResult> {
    let cmp = |a: &[i32], b: &[i32]| (f.cmp)(a, b);
    let mut out = vec![];
    for k in keys {
        match symmetry_relation_at_key(&Independent, &f.group, k, &cmp, 65_536) {
            SymRelation::Relation { .. } => continue,
            SymRelation::NoRelation { .. } => {}
            SymRelation::Unknown(e) => panic!("route 0b undecided at {k:?}: {e}"),
        }
        out.push(search_key(&Independent, k, &f.group, &cmp, policy, None));
    }
    out
}

fn check_served(f: &Fam, r: &KeyResult, policy: &TwoRowPolicy) {
    let cmp = |a: &[i32], b: &[i32]| (f.cmp)(a, b);
    let KeyOutcome::Served { relations } = &r.outcome else {
        panic!("not served: {:?}", r.outcome)
    };
    assert!(!relations.is_empty() && relations.len() <= policy.max_relations);
    for c in relations {
        // acceptance: every RHS key strictly lower, never the key itself
        for (t, q) in &c.rhs {
            assert_eq!(cmp(t, &r.key), Ordering::Less, "{t:?} vs {:?}", r.key);
            assert!(!q.is_zero());
        }
        // the direct weighted sum reproduces the relation
        let d = apply_combination(
            &Independent,
            &f.group,
            &r.key,
            &c.shape(),
            &c.weights,
            &cmp,
            policy,
            None,
        )
        .unwrap();
        assert_eq!(d.rhs, c.rhs);
        // route (A): the import's verifier at the combination's seeds
        match import_check(&Independent, &f.group, c, policy.max_terms_per_row) {
            RelationCheck::Verified { .. } => {}
            x => panic!("import verifier: {x:?} for {}", c.to_json()),
        }
        let j = c.to_json();
        assert_eq!(j["d_free"], json!(true));
        assert!(
            j["engine_certificate"]
                .as_str()
                .unwrap()
                .starts_with("none")
        );
    }
    // ranking: fewest RHS terms first
    assert!(
        relations
            .windows(2)
            .all(|w| w[0].rhs.len() <= w[1].rhs.len())
    );
}

#[test]
fn constructed_pair_serves_where_single_rows_do_not() {
    let policy = TwoRowPolicy::default();
    let mut summary = vec![];
    let mut pair_keys = 0usize;
    for (name, max_line, max_rank) in [
        ("sunset/legacy", 2i32, 3i32),
        ("banana3/legacy", 2, 2),
        ("sunbub/weighted", 1, 4),
    ] {
        let f = match name {
            "sunset/legacy" => fam(SUNSET, 3),
            "banana3/legacy" => fam(BANANA3, 4),
            _ => sunbub(),
        };
        let keys = box_keys(&f.chart, max_line, max_rank);
        let rs = keys_needing_more(&f, &keys, &policy);
        let mut by_kind: std::collections::BTreeMap<String, usize> = Default::default();
        let mut by_outcome: std::collections::BTreeMap<&str, usize> = Default::default();
        for r in &rs {
            *by_outcome.entry(r.outcome.word()).or_default() += 1;
            if let Some(b) = r.outcome.best() {
                *by_kind.entry(b.kind()).or_default() += 1;
                check_served(&f, r, &policy);
                if b.rows.len() == 2 {
                    pair_keys += 1;
                }
            } else {
                // never "none" from a cap
                if r.outcome == KeyOutcome::NoneInNeighbourhood {
                    assert!(r.stats.complete());
                }
            }
        }
        let first_pair = rs
            .iter()
            .find(|r| r.outcome.best().is_some_and(|b| b.rows.len() == 2))
            .map(|r| r.to_json());
        summary.push(
            json!({"family": name, "box_keys": keys.len(), "route0b_none": rs.len(),
            "outcomes": by_outcome, "best_kind": by_kind, "first_pair": first_pair}),
        );
    }
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    assert!(
        pair_keys > 0,
        "no key where single rows fail and a pair serves: {summary:?}"
    );
}

#[test]
fn single_rows_alone_do_not_serve_the_pair_keys() {
    // the pair keys of the constructed families: with max_rows = 1 (single rows at K and at every shifted seed) the
    // search must NOT find a relation as small as the pair's (else the pair would not be needed)
    let f = sunbub();
    let cmp = |a: &[i32], b: &[i32]| (f.cmp)(a, b);
    let two = TwoRowPolicy::default();
    let one = TwoRowPolicy {
        max_rows: 1,
        ..TwoRowPolicy::default()
    };
    let keys = box_keys(&f.chart, 1, 4);
    let mut checked = 0;
    for r in keys_needing_more(&f, &keys, &two) {
        let Some(b) = r.outcome.best() else { continue };
        if b.rows.len() != 2 {
            continue;
        }
        let s = search_key(&Independent, &r.key, &f.group, &cmp, &one, None);
        if let Some(sb) = s.outcome.best() {
            assert_eq!(sb.rows.len(), 1);
            assert_eq!(sb.kind(), "shifted_row");
            assert!(
                sb.rhs.len() > b.rhs.len(),
                "a single row as small as the pair at {:?}",
                r.key
            );
        }
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn refusals_and_typed_outcomes() {
    let f = fam(SUNSET, 3);
    let cmp = |a: &[i32], b: &[i32]| (f.cmp)(a, b);
    let p = TwoRowPolicy::default();
    // arity
    let r = search_key(&Independent, &[1, 1, 1], &f.group, &cmp, &p, None);
    assert_eq!(r.outcome.word(), "refused");
    // invalid policy
    let bad = TwoRowPolicy {
        max_rows: 0,
        ..TwoRowPolicy::default()
    };
    let r = search_key(&Independent, &[1, 1, 1, 0, 0], &f.group, &cmp, &bad, None);
    assert_eq!(r.outcome.word(), "refused");
    assert!(TwoRowPolicy::from_spec(&json!({"tworow": {"max_rows": 0}})).is_err());
    assert!(TwoRowPolicy::from_spec(&json!({"tworow": {"rows": 2}})).is_err());
    assert!(TwoRowPolicy::from_spec(&json!({"tworow": {"shifts": {"explicit": ["x"]}}})).is_err());
    // the corner (a master): no combination, complete search -> none_in_neighbourhood
    let r = search_key(&Independent, &[1, 1, 1, 0, 0], &f.group, &cmp, &p, None);
    assert_eq!(
        r.outcome,
        KeyOutcome::NoneInNeighbourhood,
        "{}",
        r.to_json()
    );
    assert!(r.stats.complete() && r.stats.seeds > 1);
    // a cap is never "no relation": every seed's expansion over a cap of 1 term
    let capped = TwoRowPolicy {
        max_terms_per_row: 1,
        ..TwoRowPolicy::default()
    };
    let r = search_key(
        &Independent,
        &[1, 1, 1, -2, -1],
        &f.group,
        &cmp,
        &capped,
        None,
    );
    assert!(!r.stats.complete());
    assert_ne!(
        r.outcome,
        KeyOutcome::NoneInNeighbourhood,
        "{}",
        r.to_json()
    );
    if r.outcome.best().is_none() {
        assert_eq!(r.outcome.word(), "over_cap", "{}", r.to_json());
    }
    // the coefficient of the key identically 0: the same row twice with weights (1, -1)
    let k = [1, 1, 1, -1, 0];
    let g = (0..f.group.group().elements.len())
        .find(|&e| {
            apply_combination(
                &Independent,
                &f.group,
                &k,
                &[RowSpec {
                    element: e,
                    shift: None,
                }],
                &[rustred_campaign_core::replay_base::algebra::Q::ONE],
                &cmp,
                &p,
                None,
            ) != Err(Reject::NoRow {
                element: e,
                seed: k.to_vec(),
            })
        })
        .unwrap();
    let row = RowSpec {
        element: g,
        shift: None,
    };
    let one = rustred_campaign_core::replay_base::algebra::Q::ONE;
    let mone = rustred_campaign_core::replay_base::algebra::Q { n: -1, d: 1 };
    assert_eq!(
        apply_combination(
            &Independent,
            &f.group,
            &k,
            &[row.clone(), row.clone()],
            &[one, mone],
            &cmp,
            &p,
            None
        ),
        Err(Reject::KeyCancels)
    );
    // malformed shapes
    assert!(matches!(
        apply_combination(&Independent, &f.group, &k, &[row], &[], &cmp, &p, None),
        Err(Reject::Shape(_))
    ));
}

#[test]
fn not_lower_single_row_is_refused_at_a_pair_key() {
    let f = sunbub();
    let cmp = |a: &[i32], b: &[i32]| (f.cmp)(a, b);
    let p = TwoRowPolicy::default();
    let keys = box_keys(&f.chart, 1, 4);
    let r = keys_needing_more(&f, &keys, &p)
        .into_iter()
        .find(|r| r.outcome.best().is_some_and(|b| b.kind() == "pair"))
        .expect("a pair key");
    let b = r.outcome.best().unwrap();
    let at_k = b
        .rows
        .iter()
        .find(|x| x.spec.shift.is_none())
        .map(|x| x.spec.clone());
    if let Some(spec) = at_k {
        let one = rustred_campaign_core::replay_base::algebra::Q::ONE;
        let res = apply_combination(
            &Independent,
            &f.group,
            &r.key,
            &[spec],
            &[one],
            &cmp,
            &p,
            None,
        );
        assert!(
            matches!(res, Err(Reject::NotLower { .. }) | Err(Reject::KeyCancels)),
            "{res:?}"
        );
    }
}

#[test]
fn shift_text_and_neighbourhood() {
    let s = PairShift::parse("11=-1,12+1").unwrap();
    assert_eq!(
        s.ops,
        vec![
            ShiftOp::Set {
                slot: 11,
                value: -1
            },
            ShiftOp::Add { slot: 12, delta: 1 }
        ]
    );
    assert_eq!(s.text(), "11=-1,12+1");
    assert_eq!(PairShift::parse("3-2").unwrap().text(), "3-2");
    assert_eq!(
        PairShift::parse("3-2").unwrap().apply(&[0, 0, 0, 5]),
        Some(vec![0, 0, 0, 3])
    );
    assert!(PairShift::parse("x+1").is_err());
    assert!(PairShift::parse("").is_err());
    assert_eq!(PairShift::parse("7+1").unwrap().apply(&[0; 3]), None);
    let p = TwoRowPolicy::default();
    let sh = p.shifts(5).unwrap();
    assert_eq!(sh.len(), 2 * 5 + 4 * 10);
    assert_eq!(sh[0].text(), "0-1");
    assert_eq!(sh[1].text(), "0+1");
    assert_eq!(sh[10].text(), "0-1,1-1");
    let q = TwoRowPolicy::from_spec(
        &json!({"tworow": {"shifts": {"unit_slots": 1, "slots": [2, 4],
        "explicit": ["2+1", "1=-1,3+1"], "max_shifts": 4}}}),
    )
    .unwrap();
    let t: Vec<String> = q.shifts(5).unwrap().iter().map(PairShift::text).collect();
    assert_eq!(t, vec!["2-1", "2+1", "4-1", "4+1"]);
    let q =
        TwoRowPolicy::from_spec(&json!({"shifts": {"unit_slots": 0, "explicit": ["1=-1,3+1"]}}))
            .unwrap();
    let t: Vec<String> = q.shifts(5).unwrap().iter().map(PairShift::text).collect();
    assert_eq!(t, vec!["1=-1,3+1"]);
    assert_eq!(
        TwoRowPolicy::from_spec(&json!(null)).unwrap(),
        TwoRowPolicy::default()
    );
}

#[test]
fn region_forms_evaluate_every_key_of_the_region() {
    let f = sunbub();
    let cmp = |a: &[i32], b: &[i32]| (f.cmp)(a, b);
    let p = TwoRowPolicy::default();
    let keys = box_keys(&f.chart, 1, 6);
    let rs = keys_needing_more(&f, &keys, &p);
    let r0 = rs
        .iter()
        .find(|r| r.outcome.best().is_some_and(|b| b.rows.len() == 2))
        .expect("a pair key");
    let face = Face::of_key(&r0.key, &f.chart).unwrap();
    let region: Vec<Vec<i32>> = keys
        .iter()
        .filter(|k| Face::of_key(k, &f.chart).as_ref() == Some(&face))
        .cloned()
        .collect();
    let results: Vec<KeyResult> = region
        .iter()
        .map(|k| search_key(&Independent, k, &f.group, &cmp, &p, None))
        .collect();
    let rep = region_forms(&Independent, &f.group, &region, &results, &cmp, &p, None);
    println!("{}", serde_json::to_string_pretty(&rep.to_json()).unwrap());
    assert!(!rep.shapes.is_empty());
    for v in &rep.shapes {
        assert_eq!(v.accepted.len() + v.rejected.len(), region.len());
    }
    // the pair key's own best shape is accepted there
    let b = r0.outcome.best().unwrap();
    let i0 = region.iter().position(|k| *k == r0.key).unwrap();
    assert!(
        rep.acceptable()[i0]
            .1
            .iter()
            .any(|l| l.contains(&b.rows[0].spec.text()))
    );
    if let Some(w) = rep.whole_region {
        assert_eq!(rep.shapes[w].accepted.len(), region.len());
    }
}

#[test]
fn pruned_rows_equal_the_symrows_rows() {
    // without a predicate: exactly rows_at (Expanded); with the family's zero-sector predicate: rows_at minus the
    // zero-sector terms (pruning during the expansion changes nothing but the cost)
    for (text, lines) in [(SUNSET, 3usize), (BANANA3, 4)] {
        let f = Family::parse(text).unwrap();
        let chart: Vec<bool> = (0..f.n()).map(|i| i < lines).collect();
        let g = verify_derived(&f, &chart, &DeriveOptions::default()).unwrap();
        let zero = |k: &[i32]| f.zero_key(&k.iter().map(|&x| x as i64).collect::<Vec<_>>());
        let mut seeds = box_keys(&chart, 2, 3);
        seeds.push((0..f.n()).map(|i| if i < lines { 1 } else { -1 }).collect());
        let (mut rows_seen, mut dropped) = (0usize, 0usize);
        for s in &seeds {
            let s64: Vec<i64> = s.iter().map(|&x| x as i64).collect();
            let a = rows_at(&g, &(0, s64), SymRowMode::Expanded, 1 << 16).unwrap();
            let plain = rows_pruned(&Independent, &g, s, 1 << 16, None).unwrap();
            let pruned = rows_pruned(&Independent, &g, s, 1 << 16, Some(&zero)).unwrap();
            let want: rustred_campaign_core::tworow::rows::SeedRowList = a
                .iter()
                .map(|r| {
                    (
                        r.element,
                        r.terms
                            .iter()
                            .map(|((_, k), c)| (k.iter().map(|&x| x as i32).collect(), *c))
                            .collect(),
                    )
                })
                .collect();
            assert_eq!(plain, want, "seed {s:?}");
            let want_z: Vec<_> = want
                .iter()
                .map(|(e, t)| {
                    (
                        *e,
                        t.iter()
                            .filter(|(k, _)| !zero(k))
                            .cloned()
                            .collect::<Vec<_>>(),
                    )
                })
                .filter(|(_, t)| !t.is_empty())
                .collect();
            assert_eq!(pruned, want_z, "seed {s:?}");
            rows_seen += want.len();
            dropped += want.iter().map(|(_, t)| t.len()).sum::<usize>()
                - want_z.iter().map(|(_, t)| t.len()).sum::<usize>();
        }
        assert!(rows_seen > 0);
        println!("{text:.12}: {rows_seen} rows, {dropped} zero-sector terms pruned");
    }
}

/// A test catalogue: a term is served by the one-element rows at the term (route 0b), else by the tworow search at
/// the term; the seeds used are remembered so that the composed relation can be re-derived by the import's verifier.
struct SymCat<'a> {
    f: &'a Fam,
    policy: TwoRowPolicy,
    seeds: std::cell::RefCell<std::collections::BTreeMap<Vec<i32>, Vec<Vec<i32>>>>,
}

impl<'a>
    rustred_campaign_core::repairflow::composite::Catalogue<
        rustred_campaign_core::repairflow::composite::QField<'a, Independent>,
    > for SymCat<'a>
{
    fn serve(
        &self,
        _alg: &rustred_campaign_core::repairflow::composite::QField<'a, Independent>,
        term: &[i32],
    ) -> Result<
        rustred_campaign_core::repairflow::composite::Service<
            rustred_campaign_core::replay_base::algebra::Q,
        >,
        String,
    > {
        use rustred_campaign_core::repairflow::composite::{RuleUse, Service};
        let cmp = |a: &[i32], b: &[i32]| (self.f.cmp)(a, b);
        let rule = |src: &str| RuleUse {
            table: 0,
            record: 0,
            source: src.into(),
            label: 0,
            record_sha256: String::new(),
        };
        if let SymRelation::Relation { rhs, .. } =
            symmetry_relation_at_key(&Independent, &self.f.group, term, &cmp, 65_536)
        {
            self.seeds
                .borrow_mut()
                .insert(term.to_vec(), vec![term.to_vec()]);
            return Ok(Service::Rule {
                rule: rule("route0b"),
                rhs,
            });
        }
        let r = search_key(&Independent, term, &self.f.group, &cmp, &self.policy, None);
        match r.outcome.best() {
            Some(c) => {
                self.seeds.borrow_mut().insert(term.to_vec(), c.seeds());
                Ok(Service::Rule {
                    rule: rule("tworow"),
                    rhs: c.rhs.clone(),
                })
            }
            None => Ok(Service::Unserved("no symmetry relation at the term".into())),
        }
    }
}

#[test]
fn composite_with_one_catalogue_rule_on_near_misses() {
    use rustred_campaign_core::import::symmetry::verify_relation_with;
    use rustred_campaign_core::repairflow::composite::{ComposeOptions, QField};
    use rustred_campaign_core::tworow::{CompositeStep, with_catalogue_rule};
    let f = sunbub();
    let cmp = |a: &[i32], b: &[i32]| (f.cmp)(a, b);
    let lower = |a: &[i32], b: &[i32]| (f.cmp)(a, b) == Ordering::Less;
    let p = TwoRowPolicy::default();
    let cat = SymCat {
        f: &f,
        policy: p.clone(),
        seeds: Default::default(),
    };
    let qf = QField(&Independent);
    let keys = box_keys(&f.chart, 1, 4);
    let (mut none, mut with_near, mut accepted, mut verified) = (0, 0, 0, 0);
    let mut words: std::collections::BTreeMap<&str, usize> = Default::default();
    for r in keys_needing_more(&f, &keys, &p) {
        if r.outcome.best().is_some() {
            assert!(r.near_misses.is_empty());
            continue;
        }
        none += 1;
        if r.near_misses.is_empty() {
            continue;
        }
        with_near += 1;
        for n in &r.near_misses {
            assert!(!n.non_lower.is_empty());
            assert!(n.non_lower.len() <= 1, "{:?}", n.non_lower);
            assert!(
                n.comb
                    .rhs
                    .iter()
                    .all(|(k, _)| n.non_lower.contains(k) || lower(k, &r.key))
            );
        }
        let step = with_catalogue_rule(
            &qf,
            &cat,
            &r.near_misses,
            &lower,
            &cmp,
            &ComposeOptions::default(),
        )
        .unwrap();
        *words.entry(step.word()).or_default() += 1;
        if let CompositeStep::Accepted { near, composite } = step {
            accepted += 1;
            assert!(composite.rhs.iter().all(|(k, _)| lower(k, &r.key)));
            assert_eq!(composite.replacements.len(), 1);
            // the composed relation lies in the span of the symmetry rows at the near miss's seeds and the rule's seeds
            let mut seeds = near.comb.seeds();
            for rep in &composite.replacements {
                seeds.extend(cat.seeds.borrow()[&rep.term].iter().cloned());
            }
            seeds.sort();
            seeds.dedup();
            let rhs: Vec<(Vec<i32>, String)> = composite
                .rhs
                .iter()
                .map(|(k, c)| (k.clone(), rustred_campaign_core::termmin::symrel::q_text(c)))
                .collect();
            let chk = verify_relation_with(&Independent, &f.group, &r.key, &rhs, &seeds, 1 << 16);
            assert!(chk.verified(), "{chk:?}");
            verified += 1;
        }
    }
    println!(
        "{}",
        json!({"route0b_and_tworow_none": none, "with_near_misses": with_near, "composite": words,
            "accepted": accepted, "verified_in_row_span": verified})
    );
    assert!(none > 0);
}

#[test]
fn structural_filter_changes_cost_not_results() {
    // the frontier-driven scan with the structural filter must give exactly the results of expanding every row
    let f = sunbub();
    let cmp = |a: &[i32], b: &[i32]| (f.cmp)(a, b);
    let on = TwoRowPolicy::default();
    let off = TwoRowPolicy {
        structural_filter: false,
        ..TwoRowPolicy::default()
    };
    let three = TwoRowPolicy {
        max_rows: 3,
        ..TwoRowPolicy::default()
    };
    let three_off = TwoRowPolicy {
        structural_filter: false,
        ..three.clone()
    };
    let (mut rows_on, mut rows_off, mut n) = (0usize, 0usize, 0usize);
    for k in box_keys(&f.chart, 1, 4) {
        let a = search_key(&Independent, &k, &f.group, &cmp, &on, None);
        let b = search_key(&Independent, &k, &f.group, &cmp, &off, None);
        assert_eq!(a.outcome, b.outcome, "{k:?}");
        assert_eq!(a.near_misses, b.near_misses, "{k:?}");
        assert_eq!(a.stats.pool_rows, b.stats.pool_rows, "{k:?}");
        rows_on += a.stats.rows;
        rows_off += b.stats.rows;
        let c = search_key(&Independent, &k, &f.group, &cmp, &three, None);
        let d = search_key(&Independent, &k, &f.group, &cmp, &three_off, None);
        assert_eq!(c.outcome, d.outcome, "max_rows 3 at {k:?}");
        n += 1;
    }
    println!("{n} keys: rows expanded {rows_on} with the filter, {rows_off} without");
    assert!(rows_on <= rows_off);
}

/// planfix (audit r2-planners T1): `apply_combination`'s OWN strictly-lower check (search.rs, the re-derivation used for
/// every kept relation and for every region-form verdict) is exercised directly: over the one-element rows at the keys
/// where route 0b gives no relation, every `Ok` relation has every right-hand-side key strictly lower (checked here with
/// the order itself), and some row is refused `NotLower` naming a term that is indeed not lower. With that check turned
/// off (the auditor's surviving mutation M-tworow) the refusals become `Ok` relations with a non-lower term: both
/// assertions fail. The same for the region forms: every accepted (shape, key) re-derives to a strictly lower relation.
#[test]
fn apply_combination_refuses_a_term_that_is_not_strictly_lower() {
    let f = sunbub();
    let cmp = |a: &[i32], b: &[i32]| (f.cmp)(a, b);
    let p = TwoRowPolicy::default();
    let keys = box_keys(&f.chart, 1, 4);
    let rs = keys_needing_more(&f, &keys, &p);
    assert!(!rs.is_empty());
    let one = rustred_campaign_core::replay_base::algebra::Q::ONE;
    let n_el = f.group.group().elements.len();
    let (mut ok, mut not_lower) = (0usize, 0usize);
    for r in &rs {
        for e in 0..n_el {
            let spec = RowSpec {
                element: e,
                shift: None,
            };
            match apply_combination(
                &Independent,
                &f.group,
                &r.key,
                &[spec],
                &[one],
                &cmp,
                &p,
                None,
            ) {
                Ok(c) => {
                    ok += 1;
                    for (t, _) in &c.rhs {
                        assert_eq!(
                            cmp(t, &r.key),
                            Ordering::Less,
                            "{t:?} not lower than {:?}",
                            r.key
                        );
                    }
                }
                Err(Reject::NotLower { term }) => {
                    not_lower += 1;
                    assert_ne!(
                        cmp(&term, &r.key),
                        Ordering::Less,
                        "refused a lower term {term:?}"
                    );
                }
                Err(_) => {}
            }
        }
    }
    assert!(
        not_lower > 0,
        "no one-element row refused NotLower at {} keys ({ok} ok)",
        rs.len()
    );
    // region forms: every accepted shape re-derives at its keys to a strictly lower relation
    let r0 = rs
        .iter()
        .find(|r| r.outcome.best().is_some_and(|b| b.rows.len() == 2))
        .expect("a pair key");
    let face = Face::of_key(&r0.key, &f.chart).unwrap();
    let region: Vec<Vec<i32>> = keys
        .iter()
        .filter(|k| Face::of_key(k, &f.chart).as_ref() == Some(&face))
        .cloned()
        .collect();
    let results: Vec<KeyResult> = region
        .iter()
        .map(|k| search_key(&Independent, k, &f.group, &cmp, &p, None))
        .collect();
    let rep = region_forms(&Independent, &f.group, &region, &results, &cmp, &p, None);
    let mut checked = 0usize;
    for v in &rep.shapes {
        for &i in &v.accepted {
            let k = &region[i];
            let w: Vec<_> = v
                .shape
                .weights
                .iter()
                .map(|wf| wf.eval(&Independent, k).unwrap())
                .collect();
            let c = apply_combination(&Independent, &f.group, k, &v.shape.rows, &w, &cmp, &p, None)
                .unwrap();
            for (t, _) in &c.rhs {
                assert_eq!(
                    cmp(t, k),
                    Ordering::Less,
                    "region form {:?}: {t:?} vs {k:?}",
                    v.shape.rows
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 0);
}

/// planfix ("tworow certificates"): a tworow relation carries a KERNEL CERTIFICATE reference (lane symcert's sealed
/// `symrows-certificate.v1` line) bound by the relation digest of its own exact right-hand side; the import-side check
/// lifts only a bound `certified` reference, refuses one of another relation / key / a forged sealed line, and keeps a
/// kernel refusal as a refusal.
#[test]
fn a_kernel_certificate_reference_binds_to_its_relation_and_is_checked() {
    use rustred_campaign_core::tworow::cert::{
        SYMROWS_CERT_SCHEMA, relation_canonical, relation_digest,
    };
    use rustred_campaign_core::tworow::{
        CertificateCheck, KernelCertificateRef, check_certificate,
    };
    let f = sunbub();
    let p = TwoRowPolicy::default();
    let keys = box_keys(&f.chart, 1, 4);
    let r = keys_needing_more(&f, &keys, &p)
        .into_iter()
        .find(|r| r.outcome.best().is_some_and(|b| b.kind() == "pair"))
        .expect("a pair key");
    let c = r.outcome.best().unwrap().clone();
    let d = c.relation_digest().unwrap();
    assert_eq!(d.len(), 64);
    // the digest is lane symcert's: canonical {"key","rhs" sorted by key,"schema"}, order of the input irrelevant
    let mut rev = c.rhs_text();
    rev.reverse();
    assert_eq!(relation_digest(&c.key, &rev).unwrap(), d);
    let canon = relation_canonical(&c.key, &rev).unwrap();
    assert!(canon.starts_with("{\"key\":["), "{canon}");
    assert!(
        canon.ends_with("\"schema\":\"rustred.campaign.symrows-relation.v1\"}"),
        "{canon}"
    );
    // a sealed certificate line and its reference
    let line = json!({"schema": SYMROWS_CERT_SCHEMA, "relation_digest": d, "record_sha256": "a".repeat(64),
        "rule": 0, "key": c.key, "verdict": "EXACT_IN_SPAN_AT_KEY", "status": "EXACT_IN_SPAN_AT_KEY",
        "class": "certified", "kernel_sha256": "b".repeat(64)})
    .to_string();
    let sha = {
        let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
        h.update(line.as_bytes());
        h.finish_hex()
    };
    let cert = KernelCertificateRef {
        schema: SYMROWS_CERT_SCHEMA.into(),
        relation_digest: d.clone(),
        key: c.key.clone(),
        class: "certified".into(),
        record_sha256: "a".repeat(64),
        kernel_sha256: "b".repeat(64),
        certificate_sha256: sha,
    };
    assert_eq!(
        check_certificate(&c, &cert, Some(&line)),
        CertificateCheck::Certified
    );
    assert!(check_certificate(&c, &cert, None).certified());
    let j = c.to_json_with(Some(&cert));
    assert_eq!(j["engine_certificate"]["class"], "certified");
    assert_eq!(j["relation_digest"], json!(d));
    assert!(
        c.to_json_with(None)["engine_certificate"]
            .as_str()
            .unwrap()
            .starts_with("none")
    );
    // another relation (one coefficient changed) / another key / a forged line / a refusal
    let mut other = c.clone();
    other.rhs[0].1 = rustred_campaign_core::replay_base::algebra::Q::int(7);
    assert!(matches!(
        check_certificate(&other, &cert, None),
        CertificateCheck::NotBound(_)
    ));
    let mut k2 = cert.clone();
    k2.key[0] += 1;
    assert!(matches!(
        check_certificate(&c, &k2, None),
        CertificateCheck::NotBound(_)
    ));
    let forged = line.replace("\"certified\"", "\"certified\" ");
    assert!(matches!(
        check_certificate(&c, &cert, Some(&forged)),
        CertificateCheck::NotBound(_)
    ));
    let mut refused = cert.clone();
    refused.class = "kernel_refused".into();
    assert_eq!(
        check_certificate(&c, &refused, None),
        CertificateCheck::KernelRefused
    );
    let mut bogus = cert.clone();
    bogus.class = "fine".into();
    assert!(matches!(
        check_certificate(&c, &bogus, None),
        CertificateCheck::NotBound(_)
    ));
}
