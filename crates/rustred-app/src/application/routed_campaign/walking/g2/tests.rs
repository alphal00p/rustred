//! Planner exactness on small random domains: every covered point lies in one
//! listed anchor (brute force), the residual band plus the anchors cover Q
//! (independent lattice predicate), anchors respect the snapshot, and plans
//! are deterministic.
use super::*;
use crate::application::routed_campaign::walking::verify_closure::lattice::Cell;

const OWNER: [bool; 3] = [true, false, true];

fn domain(lower: [u64; 3], upper: [u64; 3], rank: u32, powers: DomainPowerBounds) -> Domain<3> {
    Domain {
        phase: Phase::Apply,
        owner: OWNER,
        lower: lower.to_vec(),
        upper: upper.iter().map(|&u| Some(u)).collect(),
        rank: Some(rank),
        powers,
    }
}

fn cell(d: &Domain<3>) -> Cell {
    Cell {
        owner: d.owner.to_vec(),
        lower: d.lower.clone(),
        upper: d.upper.clone(),
        rank: d.rank,
        powers: d.powers,
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn domain(&mut self) -> Domain<3> {
        let lower = [self.below(3), self.below(3), self.below(3)];
        let upper = [
            lower[0] + self.below(4),
            lower[1] + self.below(3),
            lower[2] + self.below(4),
        ];
        let dmin = (self.below(3) == 0).then(|| self.below(6) as i64);
        let powers = DomainPowerBounds {
            max_positive_power: Some(4 + self.below(8)),
            min_power_difference: dmin,
            max_power_difference: (self.below(4) == 0).then(|| 3 + self.below(6) as i64),
        };
        domain(lower, upper, self.below(5) as u32, powers)
    }
}

fn points(d: &Domain<3>) -> Vec<[u64; 3]> {
    let c = cell(d);
    let mut out = Vec::new();
    c.for_each_point(1 << 20, |p| {
        out.push([p[0], p[1], p[2]]);
        true
    });
    out
}

#[test]
fn enumeration_equals_brute_force() {
    let mut rng = Rng(3);
    for _ in 0..400 {
        let q = rng.domain();
        let compact = CompactDomain::try_from_domain(&q).unwrap();
        let (lower, upper) = compact.raw_bounds();
        let mut got: Vec<[u64; 3]> = match Shape::of(&q.owner, lower, upper, q.rank, q.powers) {
            None => Vec::new(),
            Some(shape) => {
                let query = Query {
                    lower: *lower,
                    upper: *upper,
                    shape,
                };
                let mut out = Vec::new();
                enumerate(&q.owner, &query, POINT_CAP, &mut out).unwrap();
                out.iter()
                    .map(|p| [0, 1, 2].map(|i| u64::from(p.x[i])))
                    .collect()
            }
        };
        got.sort_unstable();
        let mut want = points(&q);
        want.sort_unstable();
        assert_eq!(got, want, "{q:?}");
    }
}

fn store_with(anchors: &[(Domain<3>, u8)], lag: Option<u64>) -> Store<3> {
    let store = Store::new(lag, 0, anchors.len() as u64);
    for (stamp, (scope, kind)) in anchors.iter().enumerate() {
        store.append(
            &scope.owner,
            Store::entry(scope, stamp, stamp as u64, *kind),
        );
    }
    store
}

#[test]
fn plans_are_exact_covers_below_the_snapshot() {
    let mut rng = Rng(11);
    let (mut planned, mut full, mut residual) = (0, 0, 0);
    for trial in 0..300 {
        let anchors: Vec<(Domain<3>, u8)> = (0..1 + rng.below(12))
            .map(|_| (rng.domain(), kind::NATIVE))
            .collect();
        // Ready (even trials): every anchor is visible. Ordered (odd trials,
        // lag 3): job `id` sees stamps below `id + 1 - 3`.
        let ordered = trial % 2 == 1;
        let store = store_with(&anchors, ordered.then_some(3));
        let q = rng.domain();
        let id = if ordered {
            rng.below(anchors.len() as u64 + 4) as usize
        } else {
            100
        };
        let snapshot = if ordered {
            (id as u64 + 1).saturating_sub(3)
        } else {
            anchors.len() as u64
        };
        let outcome = store.plan(id, &q, &AtomicBool::new(false));
        let Outcome::Planned(plan) = outcome else {
            continue;
        };
        planned += 1;
        assert_eq!(plan.snapshot, snapshot);
        assert!(!plan.anchors.is_empty());
        assert!(plan.anchors.windows(2).all(|w| w[0].stamp < w[1].stamp));
        assert!(plan.anchors.iter().all(|a| a.stamp < snapshot));
        let targets: Vec<Cell> = plan
            .anchors
            .iter()
            .map(|a| cell(&anchors[a.id as usize].0))
            .collect();
        // Brute force: every point outside the residual band is in an anchor.
        for p in points(&q) {
            let a: i64 = p[0] as i64 + 1 + p[2] as i64 + 1;
            let d = a - p[1] as i64;
            let inside = plan.residual.is_some_and(|(lo, hi)| lo <= d && d <= hi);
            if !inside {
                assert!(
                    targets.iter().any(|t| t.member(&p)),
                    "trial {trial}: point {p:?} not covered"
                );
            }
        }
        // Independent exact predicate on the union.
        let mut cells = targets.clone();
        match plan.residual_domain(&q) {
            Some(r) => {
                residual += 1;
                cells.push(cell(&r));
            }
            None => full += 1,
        }
        let refs: Vec<&Cell> = cells.iter().collect();
        assert_eq!(cell(&q).covered_by_union(&refs, 1 << 20), Some(true));
        // Deterministic: the same query plans the same way again.
        assert_eq!(
            store.plan(id, &q, &AtomicBool::new(false)),
            Outcome::Planned(plan.clone())
        );
        // Epoch's extra proof gate preserves the exact successful decision;
        // it changes only proposals whose persisted union proof is undecided.
        assert_eq!(
            store.plan_at(snapshot, &q, &AtomicBool::new(false)),
            Outcome::Planned(plan)
        );
    }
    assert!(
        planned > 50 && full > 5 && residual > 5,
        "{planned} {full} {residual}"
    );
}

#[test]
fn a_query_with_no_visible_anchor_is_inspected_whole() {
    let anchor = domain([0, 0, 0], [3, 3, 3], 3, DomainPowerBounds::default());
    let store = store_with(&[(anchor.clone(), kind::NATIVE)], Some(10));
    // Ordered, lag 10: job 5 sees stamps < 0, nothing.
    assert_eq!(
        store.plan(5, &anchor, &AtomicBool::new(false)),
        Outcome::Whole
    );
    // Job 10 sees stamp 0 and is fully covered by it.
    let Outcome::Planned(plan) = store.plan(10, &anchor, &AtomicBool::new(false)) else {
        panic!("covered");
    };
    assert_eq!(plan.residual, None);
    assert_eq!(plan.anchors.len(), 1);
}

#[test]
fn initial_d_band_anchors_lend_only_their_low_slice() {
    let whole = domain([0, 0, 0], [3, 0, 3], 0, DomainPowerBounds::default());
    // D ranges 2..8; the initial-D-band anchor lends only D <= 4.
    let mut slice = whole.clone();
    slice.powers.max_power_difference = Some(4);
    let store = store_with(&[(slice, kind::INITIAL_D_BAND)], None);
    let Outcome::Planned(plan) = store.plan(9, &whole, &AtomicBool::new(false)) else {
        panic!("bottom levels covered");
    };
    assert_eq!(plan.residual, Some((5, 8)));
    assert_eq!(plan.anchors[0].kind, kind::INITIAL_D_BAND);
}

#[test]
fn epoch_union_budget_exhaustion_falls_back_before_residual_inspection() {
    let whole = domain([0, 0, 0], [3, 0, 3], 0, DomainPowerBounds::default());
    let mut slice = whole.clone();
    slice.powers.max_power_difference = Some(4);
    let store = store_with(&[(slice, kind::INITIAL_D_BAND)], None);
    let cancel = AtomicBool::new(false);
    let legacy = store.plan(9, &whole, &cancel);
    let Outcome::Planned(plan) = &legacy else {
        panic!("pointwise discovery proves a valid low-slice cover");
    };
    assert_eq!(plan.residual, Some((5, 8)));
    // One region cannot prove the union, although every covered point has
    // already passed the native exact inclusion check. This is a normal
    // optimization miss, not a malformed rule or internal invariant failure.
    assert_eq!(
        store.plan_bound(Some(1), 0, &whole, &cancel, Some(1)),
        Outcome::Whole
    );
    assert_eq!(
        store.report()["whole_inspections"]["union_cover_undecided"],
        1
    );
    assert!(store.peek(0).is_none());
    assert!(store.pending_pins().is_empty());
    assert_eq!(store.plan_at(1, &whole, &cancel), legacy);
    assert_eq!(store.plan(9, &whole, &cancel), legacy);
}

#[test]
fn epoch_union_preflight_rejects_a_real_gap_not_just_resource_exhaustion() {
    let whole = domain([0, 0, 0], [3, 0, 3], 0, DomainPowerBounds::default());
    let mut slice = whole.clone();
    slice.powers.max_power_difference = Some(4);
    let scope = CompactDomain::try_from_domain(&slice).unwrap();
    assert_eq!(
        replayable_union_cover(
            &whole,
            Some((5, 8)),
            [&scope].into_iter(),
            EPOCH_COVER_REGIONS
        ),
        Some(true)
    );
    assert_eq!(
        replayable_union_cover(
            &whole,
            Some((6, 8)),
            [&scope].into_iter(),
            EPOCH_COVER_REGIONS
        ),
        Some(false),
        "D=5 must not be silently discarded"
    );
    assert_eq!(
        replayable_union_cover(&whole, None, [&scope].into_iter(), EPOCH_COVER_REGIONS),
        Some(false)
    );
}

#[test]
fn epoch_large_lender_union_uses_whole_inspection_without_recursive_preflight() {
    let count = EPOCH_COVER_ANCHORS + 1;
    let whole = domain(
        [0, 0, 0],
        [count as u64 - 1, 0, 0],
        0,
        DomainPowerBounds::default(),
    );
    let anchors: Vec<_> = (0..count)
        .map(|i| {
            let point = [i as u64, 0, 0];
            (
                domain(point, point, 0, DomainPowerBounds::default()),
                kind::NATIVE,
            )
        })
        .collect();
    let store = store_with(&anchors, None);
    let cancel = AtomicBool::new(false);
    let Outcome::Planned(plan) = store.plan(count + 1, &whole, &cancel) else {
        panic!("pointwise proof covers every point");
    };
    assert_eq!(plan.anchors.len(), count);
    assert_eq!(plan.residual, None);
    assert_eq!(store.plan_at(count as u64, &whole, &cancel), Outcome::Whole);
    assert_eq!(
        store.report()["whole_inspections"]["union_cover_lender_budget"],
        1
    );
}
