//! Session tests on the three-loop K6 (tetrahedron) vacuum family with line 0
//! deformed to mass^2 `1 + eta`, parameters `(d, eta)`: the eta-derivative
//! demand of the campaign's K6 DE has 12 masters (campaign engine and RustRed
//! main's LiteRed lane agree on them).

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::algebra::{CoefficientContext, ExactAlgebraLimits};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::sector::CutConstraint;

use super::super::{
    BoxOptions, CensusRouter, ColumnOrder, Insertion, SectorRouter, Seeding, SlotPriority,
};
use super::BoxLaportaSession;

const N: usize = 6;

fn k6_eta() -> Arc<IntegralFamily> {
    const ROWS: [[i64; 6]; 6] = [
        [1, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0, 1],
        [1, 0, -2, 0, 0, 1],
        [1, -2, 0, 1, 0, 0],
        [0, 0, 0, 1, -2, 1],
    ];
    let context = CoefficientContext::try_new(["d", "eta"]).unwrap();
    let denominators = ROWS
        .iter()
        .enumerate()
        .map(|(slot, row)| {
            let mut constant = context.integer(-1);
            if slot == 0 {
                constant = context
                    .try_sub(
                        &constant,
                        &context.parameter("eta").unwrap(),
                        ExactAlgebraLimits::default(),
                    )
                    .unwrap();
            }
            AffineDenominator::new(constant, row.map(|value| context.integer(value)).to_vec())
        })
        .collect();
    Arc::new(
        IntegralFamily::new(
            "k6_eta",
            ["k1", "k2", "k3"].map(String::from).to_vec(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); N],
        )
        .unwrap(),
    )
}

/// The eta-derivative demand of the campaign K6 DE: the top integral and the
/// derivative seeds `m + e_0` of its masters with a positive slot-0 power.
const TARGETS: [[i16; N]; 9] = [
    [1, 1, 1, 1, 1, 1],
    [2, 1, 1, 1, 1, 1],
    [2, 1, 1, 1, 1, 0],
    [2, 1, 1, 1, 0, 1],
    [2, 1, 0, 1, 0, 2],
    [2, 1, 1, 1, 0, 0],
    [2, 1, 1, 0, 0, 1],
    [2, 1, 0, 1, 0, 1],
    [2, 1, 1, 0, 0, 0],
];

/// The campaign engine's 12 masters for this demand.
const CAMPAIGN_MASTERS: [[i16; N]; 12] = [
    [1, 1, 1, 1, 1, 0],
    [1, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 1, 1],
    [1, 1, 0, 1, 0, 2],
    [1, 1, 1, 1, 0, 0],
    [1, 1, 1, 0, 0, 1],
    [1, 1, 0, 1, 0, 1],
    [0, 1, 1, 1, 1, 0],
    [0, 1, 1, 1, 0, 1],
    [1, 1, 1, 0, 0, 0],
    [0, 1, 1, 1, 0, 0],
    [1, 1, 1, 1, 1, 1],
];

fn options(workers: usize, insertion: Insertion) -> BoxOptions {
    BoxOptions {
        seeding: Seeding::KiraBox { r: 8, s: 1, d: 2 },
        order: ColumnOrder::LinesRankDots,
        slot_priority: SlotPriority::TopSectorFirst(63),
        workers,
        insertion,
        ..BoxOptions::default()
    }
}

fn session(workers: usize, insertion: Insertion) -> BoxLaportaSession<N> {
    let family = k6_eta();
    let router: Arc<dyn SectorRouter<N>> =
        Arc::new(CensusRouter::discover(family.clone(), &[true; N]).unwrap());
    BoxLaportaSession::new(
        family,
        &CutConstraint::none(N).unwrap(),
        router,
        options(workers, insertion),
    )
    .unwrap()
}

#[test]
fn k6_eta_has_twelve_masters_at_both_samples() {
    let mut session = session(2, Insertion::BottomUp);
    let report = session.extend(&TARGETS, &[]).unwrap();
    assert_eq!(report.masters.len(), 12, "{:?}", report.masters);
    assert_eq!(report.masters_first_sample, 12);
    assert_eq!(report.masters_second_sample, 12);
    assert_eq!(report.rounds, 1);
    assert!(report.growth.is_empty());
    assert!(report.selected_rows > 0 && report.selected_rows <= session.stats().rows);
    assert_eq!(session.masters(), &report.masters[..]);
}

#[test]
fn preferred_campaign_masters_become_the_basis() {
    let mut session = session(2, Insertion::BottomUp);
    let report = session.extend(&TARGETS, &CAMPAIGN_MASTERS).unwrap();
    let masters: BTreeSet<[i16; N]> = report.masters.iter().copied().collect();
    let expected: BTreeSet<[i16; N]> = CAMPAIGN_MASTERS.iter().copied().collect();
    assert_eq!(masters, expected);
}

#[test]
fn masters_do_not_depend_on_workers_or_insertion_schedule() {
    let one = session(1, Insertion::BottomUp)
        .extend(&TARGETS, &[])
        .unwrap();
    let three = session(3, Insertion::BottomUp)
        .extend(&TARGETS, &[])
        .unwrap();
    assert_eq!(one.masters, three.masters);
    assert_eq!(one.pivot_hash, three.pivot_hash);
    assert_eq!(one.selected_hash, three.selected_hash);
    let top_down = session(2, Insertion::TopDownLevels)
        .extend(&TARGETS, &[])
        .unwrap();
    assert_eq!(one.masters.len(), top_down.masters.len());
}

#[test]
fn extend_is_incremental() {
    let mut session = session(2, Insertion::BottomUp);
    let first = session.extend(&TARGETS[..1], &[]).unwrap();
    let rows = session.stats().rows;
    let second = session.extend(&TARGETS, &[]).unwrap();
    assert!(session.stats().rows >= rows);
    assert_eq!(second.masters.len(), 12);
    // The earlier rows are kept: the system only grows, and the top
    // integral alone needs no more masters than the whole demand.
    assert!(first.masters.len() <= second.masters.len());
}
