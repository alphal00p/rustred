//! Differential tests of the struct-of-arrays kernel against its scalar
//! references: lanes (necessary; exact when unsaturated), block masks, the
//! narrow historical envelope, and point-set inclusion by lattice enumeration.
use super::*;
use crate::application::routed_campaign::walking::queue::bits;
use crate::application::routed_campaign::walking::queue::{Domain, Phase, Queue};
use rustred::solver::DomainPowerBounds;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 11
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// A random valid summary, with small, saturating (>254), wide (>u32) and
/// infinite values; some are empty.
fn random_summary<const N: usize>(
    rng: &mut Lcg,
    owner: [bool; N],
) -> Option<DomainPowerSummary<N>> {
    let scale = match rng.below(10) {
        0 => 300,     // saturates the u8 lanes
        1 => 70_000,  // beyond the narrow u16 envelope codes
        2 => 1 << 40, // wide compact summaries
        _ => 8,
    };
    let lower: [u64; N] = std::array::from_fn(|_| rng.below(scale.min(6) + 1));
    let upper: [Option<u64>; N] =
        std::array::from_fn(|axis| (!rng.chance(25)).then(|| lower[axis] + rng.below(scale)));
    let rank = rng
        .chance(60)
        .then(|| rng.below(scale.min(u32::MAX as u64)) as u32);
    let a = rng.chance(50).then(|| rng.below(scale + 4));
    let d1 = rng
        .chance(40)
        .then(|| rng.below(2 * scale + 2) as i64 - scale as i64);
    let d2 = rng
        .chance(40)
        .then(|| rng.below(2 * scale + 2) as i64 - scale as i64);
    let (dmin, dmax) = match (d1, d2) {
        (Some(x), Some(y)) => (Some(x.min(y)), Some(x.max(y))),
        other => other,
    };
    DomainPowerSummary::try_new(
        owner,
        &lower,
        &upper,
        rank,
        DomainPowerBounds {
            max_positive_power: a,
            min_power_difference: dmin,
            max_power_difference: dmax,
        },
    )
    .ok()
}

/// A random summary likely to be contained in `outer` (a shrunk copy).
fn shrink<const N: usize>(
    rng: &mut Lcg,
    outer: &DomainPowerSummary<N>,
) -> Option<DomainPowerSummary<N>> {
    let e = outer.extrema()?;
    let lower: [u64; N] = std::array::from_fn(|axis| e.lower()[axis] + rng.below(2));
    let upper: [Option<u64>; N] = std::array::from_fn(|axis| {
        let base = e.upper()[axis].unwrap_or(lower[axis] + 5);
        Some(base.saturating_sub(rng.below(2)).max(lower[axis]))
    });
    DomainPowerSummary::try_new(
        *outer.owner(),
        &lower,
        &upper,
        None,
        DomainPowerBounds::default(),
    )
    .ok()
}

fn lanes_le<const N: usize>(a: &Lanes<N>, b: &Lanes<N>) -> bool {
    a.may_contain(b)
}

fn check_pair<const N: usize>(
    c: &DomainPowerSummary<N>,
    q: &DomainPowerSummary<N>,
    tally: &mut [usize; 4],
) {
    let native = c.contains(q);
    let (lc, lq) = (Lanes::of_core(c), Lanes::of_core(q));
    if native {
        tally[0] += 1;
        assert!(bits::may_contain(bits::word(c), bits::word(q)));
        if let (Some(lc), Some(lq)) = (&lc, &lq) {
            assert!(lanes_le(lc, lq), "necessity: {c:?} ⊇ {q:?}");
        }
    }
    if let (Some(lc), Some(lq)) = (&lc, &lq) {
        if !lc.lossy && !lq.lossy {
            tally[1] += 1;
            // Same owner and nonempty: the lane test is exact.
            assert_eq!(lanes_le(lc, lq), native, "exactness: {c:?} vs {q:?}");
        } else {
            tally[2] += 1;
        }
    } else {
        tally[3] += 1;
    }
}

#[test]
fn lanes_are_necessary_in_both_directions_and_exact_when_unsaturated() {
    let mut rng = Lcg(7);
    let mut tally = [0; 4];
    for _ in 0..200_000 {
        let owner = [rng.chance(50), rng.chance(50), rng.chance(50)];
        let Some(c) = random_summary::<3>(&mut rng, owner) else {
            continue;
        };
        let q = if rng.chance(50) {
            shrink(&mut rng, &c)
        } else {
            random_summary::<3>(&mut rng, owner)
        };
        let Some(q) = q else { continue };
        check_pair(&c, &q, &mut tally);
        check_pair(&q, &c, &mut tally);
    }
    println!(
        "lane_pairs contained={} exact_checked={} lossy={} empty={}",
        tally[0], tally[1], tally[2], tally[3]
    );
    assert!(tally.iter().all(|&count| count > 1000), "{tally:?}");
}

#[test]
fn lane_codes_keep_infinities_apart_from_saturated_values() {
    let mut w = LaneWriter { lossy: false };
    assert_eq!(w.upper(None), 0);
    assert_eq!(w.upper(Some(254)), 1);
    assert!(!w.lossy);
    assert_eq!(w.upper(Some(u128::MAX)), 1);
    assert!(w.lossy);
    let mut w = LaneWriter { lossy: false };
    assert_eq!((w.lower(254), w.lower(0)), (254, 0));
    assert!(!w.lossy);
    assert_eq!(w.lower(255), 255);
    assert!(w.lossy);
    let mut w = LaneWriter { lossy: false };
    assert_eq!(w.difference_lower(None), 0);
    assert_eq!(w.difference_lower(Some(-127)), 1);
    assert_eq!(w.difference_lower(Some(127)), 255);
    assert_eq!(w.difference_upper(None), 0);
    assert_eq!(w.difference_upper(Some(126)), 1);
    assert_eq!(w.difference_upper(Some(-128)), 255);
    assert!(!w.lossy);
    assert_eq!(w.difference_lower(Some(-128)), 1);
    assert!(w.lossy);
    let mut w = LaneWriter { lossy: false };
    assert_eq!(w.difference_upper(Some(127)), 1);
    assert!(w.lossy);
}

/// Per-slot scalar reference of `Block::forward`/`reverse`.
fn scalar_masks<const N: usize>(
    entries: &[(u64, Option<Lanes<N>>)],
    probe: &Probe<'_, N>,
    forward: bool,
) -> (u32, u32) {
    let (mut words, mut pass) = (0, 0);
    for (slot, (word, lanes)) in entries.iter().enumerate() {
        let word_ok = if forward {
            probe.word & !word == 0
        } else {
            word & !probe.word == 0
        };
        let lanes_ok = match (lanes, &probe.lanes) {
            (Some(stored), Some(query)) => {
                if forward {
                    stored.may_contain(query)
                } else {
                    query.may_contain(stored)
                }
            }
            _ => true,
        };
        words |= u32::from(word_ok) << slot;
        pass |= u32::from(word_ok && lanes_ok) << slot;
    }
    (words, pass)
}

#[test]
fn block_masks_equal_the_per_slot_scalar_reference() {
    let mut rng = Lcg(11);
    let owner = [true, false, true];
    let pool: Vec<_> = (0..4000)
        .filter_map(|_| random_summary::<3>(&mut rng, owner))
        .collect();
    let mut checked = 0;
    for round in 0..3000 {
        let len = 1 + rng.below(32) as usize;
        let members: Vec<_> = (0..len)
            .map(|_| &pool[rng.below(pool.len() as u64) as usize])
            .collect();
        let (mut meta, mut block) =
            Meta::<3>::prepare(Coordinates::of(members[0]), &mut || Ok(())).unwrap();
        let mut entries = Vec::new();
        for (id, summary) in members.iter().enumerate() {
            let spare = meta
                .needs_wide_storage(Coordinates::of(summary))
                .then(|| Meta::<3>::wide_storage(Coordinates::of(summary).unwrap()).unwrap());
            let (word, lanes) = (bits::word(summary), Lanes::of_core(summary));
            meta.push(
                &mut block[0],
                Entry {
                    id,
                    coordinates: Coordinates::of(summary),
                    word,
                    lanes,
                },
                spare,
            );
            entries.push((word, lanes));
        }
        // Retire a random subset now and then, exercising compaction.
        if round % 3 == 0 {
            let keep = rng.next() as u32;
            meta.retain(&mut block[0], keep);
            entries = entries
                .into_iter()
                .enumerate()
                .filter(|(slot, _)| keep & (1 << slot) != 0)
                .map(|(_, entry)| entry)
                .collect();
        }
        let len = meta.len as usize;
        for (slot, entry) in entries.iter().enumerate() {
            let (_, word, lanes) = block[0].entry(slot);
            assert_eq!((word, lanes), *entry);
        }
        let or = entries.iter().fold(0, |a, e| a | e.0);
        let and = entries.iter().fold(u64::MAX, |a, e| a & e.0);
        assert_eq!(block[0].block_words(), (or, and));
        for _ in 0..8 {
            let query = &pool[rng.below(pool.len() as u64) as usize];
            let probe = Probe::new(
                Coordinates::of(query),
                bits::word(query),
                Lanes::of_core(query),
                true,
            );
            for forward in [true, false] {
                let (words, pass) = if forward {
                    block[0].forward(&probe, len)
                } else {
                    block[0].reverse(&probe, len)
                };
                let (ref_words, ref_pass) = scalar_masks(&entries, &probe, forward);
                assert_eq!(pass, ref_pass, "forward={forward}");
                // A whole-block word reject may report no word mask; then
                // every live slot failed the word anyway.
                assert!(words == ref_words || (words == 0 && pass == 0 && ref_words == 0));
                checked += 1;
            }
        }
    }
    assert!(checked > 40_000);
}

type Point = ([u64; 2], [Option<u64>; 2]);

fn coordinates(p: &Point) -> Coordinates<'_> {
    Coordinates {
        lower: &p.0[..],
        upper: &p.1[..],
    }
}

#[test]
fn narrow_envelopes_decide_exactly_like_the_persisted_form() {
    let mut rng = Lcg(5);
    let values = [
        0,
        1,
        7,
        254,
        255,
        65_533,
        65_534,
        65_535,
        65_536,
        1 << 33,
        u64::MAX - 1,
    ];
    let pick = |rng: &mut Lcg| values[rng.below(values.len() as u64) as usize];
    let mut checked = 0;
    for _ in 0..20_000 {
        let points: Vec<Point> = (0..1 + rng.below(4))
            .map(|_| {
                let lower = [pick(&mut rng), pick(&mut rng)];
                let upper = std::array::from_fn(|axis| {
                    (!rng.chance(30)).then(|| pick(&mut rng).max(lower[axis]))
                });
                (lower, upper)
            })
            .collect();
        let mut envelope = Envelope::<2>::prepare(Some(coordinates(&points[0]))).unwrap();
        let mut exact: Vec<_> = (0..2)
            .map(|axis| AxisEnvelope::point(points[0].0[axis], points[0].1[axis]))
            .collect();
        for point in &points[1..] {
            let c = coordinates(point);
            let spare = envelope
                .needs_wide_storage(Some(c))
                .then(|| Meta::<2>::wide_storage(c).unwrap());
            envelope.widen(Some(c), spare);
            for (axis, e) in exact.iter_mut().enumerate() {
                e.widen(point.0[axis], point.1[axis]);
            }
        }
        assert_eq!(envelope.axes().collect::<Vec<_>>(), exact);
        let restored = Envelope::<2>::restore(&exact).unwrap();
        assert_eq!(restored.axes().collect::<Vec<_>>(), exact);
        for _ in 0..8 {
            let lower = [pick(&mut rng), pick(&mut rng)];
            let upper: [Option<u64>; 2] = std::array::from_fn(|axis| {
                (!rng.chance(30)).then(|| pick(&mut rng).max(lower[axis]))
            });
            let probe = Probe::<2>::unfiltered(Some(Coordinates {
                lower: &lower,
                upper: &upper,
            }));
            let forward = exact
                .iter()
                .enumerate()
                .all(|(axis, e)| e.may_contain(lower[axis], upper[axis]));
            let reverse = exact
                .iter()
                .enumerate()
                .all(|(axis, e)| e.may_be_contained(lower[axis], upper[axis]));
            for env in [&envelope, &restored] {
                assert_eq!(env.may_contain(&probe), forward);
                assert_eq!(env.may_be_contained(&probe), reverse);
            }
            checked += 1;
        }
    }
    assert!(checked == 160_000);
}

/// Integer points of a two-axis domain with every coordinate at most 3.
fn points(domain: &Domain<2>) -> u32 {
    let mut set = 0;
    for x in 0..=3_u64 {
        for y in 0..=3_u64 {
            let local = [x, y];
            let inside = (0..2).all(|axis| {
                domain.lower[axis] <= local[axis]
                    && domain.upper[axis].is_none_or(|upper| local[axis] <= upper)
            });
            let (mut a, mut r) = (0_i64, 0_i64);
            for axis in 0..2 {
                if domain.owner[axis] {
                    a += local[axis] as i64 + 1;
                } else {
                    r += local[axis] as i64;
                }
            }
            let d = a - r;
            let p = domain.powers;
            if inside
                && domain.rank.is_none_or(|cap| r <= i64::from(cap))
                && p.max_positive_power.is_none_or(|cap| a <= cap as i64)
                && p.min_power_difference.is_none_or(|cap| d >= cap)
                && p.max_power_difference.is_none_or(|cap| d <= cap)
            {
                set |= 1 << (4 * x + y);
            }
        }
    }
    set
}

/// Every pair of small bounded domains, admitted through the queue in both
/// orders: reuse and retirement happen exactly on point-set inclusion.
#[test]
fn queue_reuse_and_retirement_match_lattice_enumeration() {
    let intervals = [(0, 0), (0, 1), (1, 2), (0, 3), (2, 3)];
    let mut domains = Vec::new();
    for owner in [[true, false], [true, true], [false, false]] {
        for &(l0, u0) in &intervals {
            for &(l1, u1) in &intervals {
                for rank in [None, Some(1), Some(3)] {
                    for (a, dmin, dmax) in [
                        (None, None, None),
                        (Some(3), None, None),
                        (None, Some(0), None),
                        (None, None, Some(1)),
                        (Some(4), Some(-2), Some(2)),
                        (Some(0), None, None),
                    ] {
                        domains.push(Domain {
                            phase: Phase::Apply,
                            owner,
                            lower: vec![l0, l1],
                            upper: vec![Some(u0), Some(u1)],
                            rank,
                            powers: DomainPowerBounds {
                                max_positive_power: a,
                                min_power_difference: dmin,
                                max_power_difference: dmax,
                            },
                        });
                    }
                }
            }
        }
    }
    let sets: Vec<u32> = domains.iter().map(points).collect();
    let mut pairs = 0;
    let mut reused = 0;
    for (i, container) in domains.iter().enumerate() {
        for (j, query) in domains.iter().enumerate() {
            if i == j {
                continue;
            }
            let same_bucket = container.owner == query.owner;
            let included = same_bucket && sets[j] & !sets[i] == 0;
            let mut queue = Queue::<2>::new(4, None);
            assert_eq!(queue.admit(container.clone()), Ok((0, true)));
            let (id, new) = queue.admit(query.clone()).unwrap();
            // An exact duplicate image is not a pair of distinct domains here.
            assert_eq!(!new, included, "{container:?} ⊇? {query:?}");
            assert_eq!(id, if included { 0 } else { 1 });
            reused += usize::from(!new);
            // Reverse: the query admitted first is retired by the container.
            let mut queue = Queue::<2>::new(4, None);
            assert_eq!(queue.admit(query.clone()), Ok((0, true)));
            let (_, new) = queue.admit(container.clone()).unwrap();
            if new {
                let retired = !queue.is_indexed(0);
                assert_eq!(retired, included, "reverse {container:?} ⊇? {query:?}");
            } else {
                // The query already contains the container.
                assert!(same_bucket && sets[i] & !sets[j] == 0);
            }
            pairs += 1;
        }
    }
    println!("lattice_pairs={pairs} reused={reused}");
    assert!(pairs > 100_000 && reused > 1_000);
}
