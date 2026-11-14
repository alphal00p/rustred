//! Winner diff and stream usage on synthetic tables (no external data).

mod box_common;

use box_common::lines_first;
use rustred_campaign_core::boxdef::BoxDef;
use rustred_campaign_core::boxdef::winner::{
    StreamOptions, WinnerState, for_each_chunk, rule_id, usage, winner_diff,
};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::key::Key;

fn dom(chart: &[bool], fixed: &[(usize, i32)]) -> CaseDomain {
    let mut d = CaseDomain::free(chart.len(), Some(chart.to_vec()));
    for &(s, v) in fixed {
        d.fixed[s] = Some(v);
    }
    d
}

fn setup() -> (BoxDef, Table, Table) {
    let chart = lines_first(6, 3);
    let b = BoxDef::from_json(
        r#"{"schema":"rustred.campaign.box.v1","id":"w","name":"w","measure":"dots-rank-v1",
            "chart":"111000","layers":[[0,2],[1,2],[2,2],[3,1]]}"#,
    )
    .unwrap();
    let mut a = Table::new("A", 6);
    a.push(0, dom(&chart, &[(0, 1)])).unwrap(); // line 0 undotted
    a.push(1, dom(&chart, &[(3, 0), (4, 0)])).unwrap();
    let mut c = Table::new("C", 6);
    c.push(0, dom(&chart, &[(5, 0)])).unwrap();
    c.push(1, dom(&chart, &[(1, 2)])).unwrap();
    (b, a, c)
}

#[test]
fn chunks_are_merged_in_stream_order_for_any_thread_count() {
    let keys: Vec<Key> = (0..1000).map(|i| vec![i]).collect();
    for threads in [1, 2, 3, 4, 7] {
        for chunk in [1, 3, 64, 1000, 5000] {
            let mut seen = vec![];
            for_each_chunk(
                keys.clone(),
                &StreamOptions { threads, chunk },
                |c| c.iter().map(|k| k[0] * 2).collect::<Vec<_>>(),
                |c, r| {
                    assert_eq!(c.len(), r.len());
                    seen.extend(r)
                },
            );
            assert_eq!(seen, (0..1000).map(|i| i * 2).collect::<Vec<_>>());
        }
    }
}

#[test]
fn winner_diff_groups_and_determinism() {
    let (b, a, c) = setup();
    let old = Dispatcher::new(vec![a.clone(), c.clone()], vec![0, 1]).unwrap();
    let new = Dispatcher::new(vec![a, c], vec![1, 0]).unwrap();
    let mut prev = None;
    for threads in [1, 2, 4] {
        for chunk in [1, 5, 1000] {
            let o = StreamOptions { threads, chunk };
            let d = winner_diff(&old, &new, b.iter(), rule_id, &o, true).unwrap();
            assert_eq!(d.keys as u128, b.count());
            let total: u64 = d.groups.values().map(|g| g.count).sum();
            assert_eq!(total, d.changed);
            // brute force
            let mut n = 0;
            for k in b.iter() {
                let x = rule_id_state(&old, &k);
                let y = rule_id_state(&new, &k);
                if x != y {
                    n += 1;
                    let g = &d.groups[&(x, y)];
                    assert!(g.keys.contains(&k));
                }
            }
            assert_eq!(n, d.changed);
            if let Some(p) = &prev {
                assert_eq!(p, &d, "output depends on threads/chunk");
            }
            prev = Some(d);
        }
    }
    let d = prev.unwrap();
    assert!(d.changed > 0);
    // coverage is order-independent: no Covered <-> Uncovered transition
    assert!(d.groups.keys().all(|(x, y)| matches!(
        (x, y),
        (WinnerState::Covered(_), WinnerState::Covered(_))
    )));
    // piece-level identity: the table name only
    let dp = winner_diff(
        &old,
        &new,
        b.iter(),
        |d: &Dispatcher, r| d.tables()[r.table].name.clone(),
        &StreamOptions::default(),
        false,
    )
    .unwrap();
    assert!(dp.changed <= d.changed);
    assert!(dp.groups.values().all(|g| g.keys.is_empty()));
    // identical dispatchers: empty diff
    let z = winner_diff(&old, &old, b.iter(), rule_id, &StreamOptions::default(), true).unwrap();
    assert_eq!(z.changed, 0);
}

fn rule_id_state(
    d: &Dispatcher,
    k: &[i32],
) -> WinnerState<rustred_campaign_core::boxdef::winner::RuleId> {
    rustred_campaign_core::boxdef::winner::winner_state(d, &d.first_match(k).unwrap(), &rule_id)
}

#[test]
fn usage_counts_and_wrong_arity() {
    let (b, a, c) = setup();
    let d = Dispatcher::new(vec![a, c], vec![0, 1]).unwrap();
    let rep = usage(&d, b.iter(), &StreamOptions { threads: 3, chunk: 7 }, true).unwrap();
    assert_eq!(rep.keys as u128, b.count());
    assert_eq!(rep.covered + rep.open(), rep.keys);
    assert_eq!(rep.open() as usize, rep.open_keys.len());
    assert_eq!(rep.served.values().sum::<u64>(), rep.covered);
    assert_eq!(rep.wins_per_table.iter().sum::<u64>(), rep.covered);
    let bad = usage(&d, vec![vec![1, 1]], &StreamOptions::default(), false);
    assert!(bad.is_err());
}
