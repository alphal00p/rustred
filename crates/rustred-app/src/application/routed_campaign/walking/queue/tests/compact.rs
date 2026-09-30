//! Compact storage is an encoding, not a policy: every predicate and every
//! admission result must equal the one computed on the transport types.
use super::super::compact::{Digest, ExactIndex, Query, Stored};
use super::prepared::{parallel_prepare, same_state};
use super::*;

/// Deterministic generator; a numerical probe would hide failing seeds.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 17
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// Small random domains with frequent None bounds, equal owners and nested
/// boxes (so both containment outcomes are common), plus rare extreme power
/// and rank bounds whose native extrema do not fit the compact summary.
fn random_domain<const N: usize>(rng: &mut Lcg, owner: [bool; N], extreme: bool) -> Domain<N> {
    let mut lower = Vec::with_capacity(N);
    let mut upper = Vec::with_capacity(N);
    for _ in 0..N {
        let lo = if rng.chance(50) { 0 } else { rng.below(4) };
        lower.push(lo);
        upper.push((!rng.chance(35)).then(|| lo + rng.below(5)));
    }
    let small_i64 = |rng: &mut Lcg| rng.below(9) as i64 - 3;
    let mut powers = DomainPowerBounds {
        max_positive_power: rng.chance(60).then(|| rng.below(12)),
        min_power_difference: rng.chance(40).then(|| small_i64(rng)),
        max_power_difference: rng.chance(40).then(|| small_i64(rng) + 3),
    };
    let mut rank = rng.chance(70).then(|| rng.below(10) as u32);
    if extreme {
        match rng.below(5) {
            0 => powers.max_positive_power = Some(u64::MAX - rng.below(3)),
            1 => powers.min_power_difference = Some(i64::MIN + rng.below(3) as i64),
            2 => powers.max_power_difference = Some(i64::MAX - rng.below(3) as i64),
            3 => rank = Some(u32::MAX - rng.below(3) as u32),
            _ => {
                powers.max_positive_power = Some(u64::MAX);
                powers.min_power_difference = Some(i64::MIN);
            }
        }
    }
    Domain {
        phase: if rng.chance(50) {
            Phase::Apply
        } else {
            Phase::Route
        },
        owner,
        lower,
        upper,
        rank,
        powers,
    }
}

/// A random sub-box of `domain` (tightened coordinates, rank and powers).
fn shrink<const N: usize>(rng: &mut Lcg, domain: &Domain<N>) -> Domain<N> {
    let mut d = domain.clone();
    for axis in 0..N {
        if rng.chance(40) {
            d.lower[axis] += rng.below(2);
        }
        d.upper[axis] = match d.upper[axis] {
            None if rng.chance(50) => Some(d.lower[axis] + rng.below(4)),
            None => None,
            Some(hi) => Some(
                hi.max(d.lower[axis])
                    .saturating_sub(rng.below(2))
                    .max(d.lower[axis]),
            ),
        };
    }
    if rng.chance(30) {
        d.rank = Some(d.rank.map_or(rng.below(10) as u32, |r| r.saturating_sub(1)));
    }
    if rng.chance(30) {
        d.powers.max_positive_power = Some(
            d.powers
                .max_positive_power
                .map_or(rng.below(12), |a| a.saturating_sub(1)),
        );
    }
    d
}

fn owners<const N: usize>(rng: &mut Lcg) -> Vec<[bool; N]> {
    let mut out = vec![[true; N], [false; N]];
    for _ in 0..2 {
        out.push(std::array::from_fn(|_| rng.chance(50)));
    }
    out
}

fn summary<const N: usize>(d: &Domain<N>) -> Option<DomainPowerSummary<N>> {
    DomainPowerSummary::try_new(d.owner, &d.lower, &d.upper, d.rank, d.powers).ok()
}

/// Two stored candidates (IDs 0 and 1) and the production comparison view.
fn stored_pair<const N: usize>(
    a: &Domain<N>,
    b: &Domain<N>,
) -> (Vec<CompactDomain<N>>, Vec<CompactSummary<N>>) {
    let domains: Vec<_> = [a, b]
        .iter()
        .map(|d| CompactDomain::try_from_domain(d).unwrap())
        .collect();
    let summaries = [a, b]
        .iter()
        .map(|d| CompactSummary::from_core(&summary(d).unwrap()))
        .collect();
    (domains, summaries)
}

#[derive(Default, Debug)]
struct Tally {
    pairs: usize,
    contained: usize,
    wide: usize,
}

fn differential<const N: usize>(pairs: usize, seed: u64) -> Tally {
    let mut rng = Lcg(seed ^ (N as u64) << 32);
    let owners = owners::<N>(&mut rng);
    let mut tally = Tally::default();
    while tally.pairs < pairs {
        let owner = owners[rng.below(owners.len() as u64) as usize];
        let extreme = rng.chance(3);
        let a = random_domain(&mut rng, owner, extreme);
        let b = match rng.below(3) {
            0 => shrink(&mut rng, &a),
            1 => {
                let extreme = rng.chance(3);
                random_domain(&mut rng, owner, extreme)
            }
            _ => {
                let other = owners[rng.below(owners.len() as u64) as usize];
                random_domain(&mut rng, other, false)
            }
        };
        let (Some(sa), Some(sb)) = (summary(&a), summary(&b)) else {
            continue;
        };
        tally.pairs += 1;
        let (ca, cb) = (
            CompactSummary::from_core(&sa),
            CompactSummary::from_core(&sb),
        );
        for (c, s) in [(&ca, &sa), (&cb, &sb)] {
            if c.is_wide() {
                tally.wide += 1;
                assert_eq!(c.signature(), None);
            } else {
                assert!(c.matches_core(s), "{s:?} -> {c:?}");
                assert_eq!(c.signature(), Some(Signature::of(s)));
            }
        }
        let native = [sa.contains(&sb), sb.contains(&sa)];
        tally.contained += usize::from(native[0]) + usize::from(native[1]);
        if !ca.is_wide() && !cb.is_wide() {
            assert_eq!(ca.contains(&cb), Some(native[0]), "{a:?} vs {b:?}");
            assert_eq!(cb.contains(&ca), Some(native[1]), "{b:?} vs {a:?}");
        } else {
            // Release check (A1): a wide side never takes the compact path.
            assert_eq!(ca.contains(&cb), None);
            assert_eq!(cb.contains(&ca), None);
        }
        // The production view, including the rebuilt-native wide fallback and
        // the explicit phase/owner check of every positive (A1): a candidate
        // of another bucket is never a positive, not even for an empty query.
        let (domains, summaries) = stored_pair(&a, &b);
        let stored = Stored {
            domains: &domains,
            summaries: &summaries,
            quarantine: &[],
        };
        let (qa, qb) = (
            Query::new(sa.clone(), a.phase),
            Query::new(sb.clone(), b.phase),
        );
        let same = a.phase == b.phase && a.owner == b.owner;
        assert_eq!(stored.contains(0, &qb), same && native[0]);
        assert_eq!(stored.contained_by(1, &qa), same && native[0]);
        assert_eq!(stored.contains(1, &qa), same && native[1]);
        assert_eq!(stored.contained_by(0, &qb), same && native[1]);
        // Kernel images agree between the admission and the restore paths.
        for (d, s, q) in [
            (&domains[0], &summaries[0], &qa),
            (&domains[1], &summaries[1], &qb),
        ] {
            assert_eq!(
                super::super::compact::stored_image(d, s).unwrap(),
                q.image()
            );
        }
        assert_eq!(stored.signature(0), Signature::of(&sa));
        assert_eq!(stored.signature(1), Signature::of(&sb));
        // Raw syntactic containment and the orthant predicate, too.
        let (da, db) = (&domains[0], &domains[1]);
        assert_eq!(da.contains(db), a.contains(&b));
        assert_eq!(db.contains(da), b.contains(&a));
        assert_eq!(da.is_full_orthant(), a.is_full_orthant());
    }
    tally
}

#[test]
fn compact_summary_contains_matches_core_on_random_boxes() {
    macro_rules! arities { ($($n:literal),*) => { $({
        let tally = differential::<$n>(100_000, 0x5eed);
        // Each arity must exercise both outcomes and the wide fallback.
        assert!(tally.contained > 1_000, "N={}: {tally:?}", $n);
        assert!(tally.contained < 2 * tally.pairs - 1_000, "N={}: {tally:?}", $n);
        assert!(tally.wide > 100, "N={}: {tally:?}", $n);
        println!("compact_differential N={} {tally:?}", $n);
    })* } }
    arities!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);
}

#[test]
fn compact_domain_round_trips_every_field_and_rejects_out_of_range() {
    let mut rng = Lcg(7);
    let owners = owners::<5>(&mut rng);
    let mut seen: Vec<(Domain<5>, Digest)> = Vec::new();
    for i in 0..20_000 {
        let owner = owners[i % owners.len()];
        let extreme = rng.chance(10);
        let mut d = random_domain(&mut rng, owner, extreme);
        if rng.chance(5) {
            d.lower[rng.below(5) as usize] = MAX_COMPACT_COORDINATE;
            d.upper = vec![None; 5];
        }
        let compact = CompactDomain::try_from_domain(&d).unwrap();
        assert_eq!(compact.expand(), d);
        assert_eq!(compact.phase(), d.phase);
        assert_eq!(compact.owner(), d.owner);
        assert_eq!(compact.rank(), d.rank);
        assert_eq!(compact.powers(), d.powers);
        let digest = compact.digest();
        if i % 16 == 0 {
            for (other, other_digest) in &seen {
                assert_eq!(*other == d, *other_digest == digest);
            }
            seen.push((d, digest));
        }
    }
    let mut d = random_domain(&mut rng, owners[0], false);
    for (axis, value) in [(0, MAX_COMPACT_COORDINATE + 1), (4, u64::MAX)] {
        let mut above = d.clone();
        above.lower[axis] = value;
        above.upper[axis] = None;
        assert_eq!(
            CompactDomain::try_from_domain(&above),
            Err(COMPACT_RANGE_ERROR)
        );
        let mut above = d.clone();
        above.upper[axis] = Some(value);
        assert_eq!(
            CompactDomain::try_from_domain(&above),
            Err(COMPACT_RANGE_ERROR)
        );
    }
    // The largest finite coordinate stays distinct from +infinity.
    d.upper[0] = Some(MAX_COMPACT_COORDINATE);
    let finite = CompactDomain::try_from_domain(&d).unwrap();
    d.upper[0] = None;
    let infinite = CompactDomain::try_from_domain(&d).unwrap();
    assert_ne!(finite, infinite);
    assert!(infinite.contains(&finite) && !finite.contains(&infinite));
    assert_eq!(finite.upper(0), Some(MAX_COMPACT_COORDINATE));
    assert_eq!(infinite.upper(0), None);
}

#[test]
fn borrowed_compact_coordinates_preserve_validation_and_native_summary() {
    fn check<const N: usize>() {
        let mut rng = Lcg(0xb077_0eed ^ N as u64);
        for case in 0..1_000 {
            let owner: [bool; N] = std::array::from_fn(|_| rng.chance(50));
            let mut domain = random_domain(&mut rng, owner, case % 7 == 0);
            if case % 11 == 0 {
                domain.lower[0] = 65534;
                domain.upper[0] = Some(65534);
            }
            if case % 13 == 0 {
                domain.lower[0] = 2;
                domain.upper[0] = Some(1);
            }
            let compact = CompactDomain::try_from_domain(&domain).unwrap();
            assert_eq!(
                CompactDomain::try_from_parts(
                    domain.phase,
                    domain.owner,
                    &domain.lower,
                    &domain.upper,
                    domain.rank,
                    domain.powers,
                ),
                Ok(compact)
            );
            assert_eq!(
                compact.try_native_summary(),
                DomainPowerSummary::try_new(
                    domain.owner,
                    &domain.lower,
                    &domain.upper,
                    domain.rank,
                    domain.powers,
                )
            );
            for (lower, upper) in [
                (&domain.lower[..N - 1], domain.upper.as_slice()),
                (domain.lower.as_slice(), &domain.upper[..N - 1]),
            ] {
                assert_eq!(
                    CompactDomain::try_from_parts(
                        domain.phase,
                        domain.owner,
                        lower,
                        upper,
                        domain.rank,
                        domain.powers,
                    ),
                    Err("domain coordinate arity")
                );
            }
        }
    }
    check::<1>();
    check::<2>();
    check::<6>();
    check::<10>();
    check::<15>();
    check::<16>();
    check::<32>();
}

#[test]
fn coordinates_above_the_compact_range_are_refused_without_publishing() {
    for max_checks in [None, Some(100)] {
        let mut queue = Queue::new(8, max_checks);
        let mut ok = domain(Some(3));
        ok.upper = vec![Some(MAX_COMPACT_COORDINATE), None];
        assert_eq!(queue.admit(ok.clone()), Ok((0, true)));
        let mut above = ok.clone();
        above.upper[0] = Some(MAX_COMPACT_COORDINATE + 1);
        assert_eq!(queue.admit(above.clone()), Err(COMPACT_RANGE_ERROR));
        above.upper[0] = None;
        above.lower[1] = MAX_COMPACT_COORDINATE + 1;
        assert_eq!(queue.admit(above), Err(COMPACT_RANGE_ERROR));
        assert_eq!(queue.domains.len(), 1);
        assert_eq!(queue.exact.len(), 1);
        assert_eq!(queue.summaries.len(), usize::from(max_checks.is_none()));
        assert_eq!((queue.deduplicated, queue.containment_checks), (0, 0));
        assert_eq!(queue.admit(ok), Ok((0, false)));
    }
}

/// The campaign inputs stay far below the compact coordinate range.
#[test]
fn campaign_query_bounds_are_inside_the_compact_range() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/input/five_loop_qcd_feynman_d9d10/queries.json"
    );
    let document: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let queries = document["queries"].as_array().unwrap();
    assert!(!queries.is_empty());
    let mut largest = 0;
    for query in queries {
        for key in ["lower", "upper"] {
            for value in query[key].as_array().unwrap() {
                if let Some(value) = value.as_u64() {
                    largest = largest.max(value);
                } else {
                    assert!(value.is_null());
                }
            }
        }
    }
    assert!(largest <= 64, "largest finite query coordinate {largest}");
    assert!(largest < MAX_COMPACT_COORDINATE);
}

#[test]
fn compact_state_size_of_is_within_budget() {
    use std::mem::size_of;
    assert_eq!(size_of::<CompactDomain<15>>(), 96);
    assert_eq!(size_of::<CompactSummary<15>>(), 92);
    assert!(size_of::<CompactDomain<16>>() <= 104);
    assert!(size_of::<CompactSummary<16>>() <= 96);
    // Exact-map entry: 64-bit digest key and the ID.
    assert_eq!(size_of::<(Digest, usize)>(), 16);
    // Transport types, for the record: what one queued ID used to retain
    // (Arc block + two coordinate vectors + native summary + map entry).
    let arc_domain = 2 * size_of::<usize>() + size_of::<Domain<15>>();
    let vectors = 15 * (size_of::<u64>() + size_of::<Option<u64>>());
    let old = size_of::<std::sync::Arc<Domain<15>>>()
        + arc_domain
        + vectors
        + size_of::<DomainPowerSummary<15>>()
        + size_of::<(std::sync::Arc<Domain<15>>, usize)>()
        + size_of::<u64>();
    // Immutable per-ID summary; the filter word lives in the index block.
    let new = size_of::<CompactDomain<15>>()
        + size_of::<CompactSummary<15>>()
        + size_of::<(Digest, usize)>();
    assert!(size_of::<DomainPowerSummary<15>>() >= 500);
    assert!(2 * new < old, "old {old} B, new {new} B");
    println!(
        "per-ID payload bytes before allocator/table overhead: old {old}, new {new} (immutable per-ID summary; index blocks excluded)"
    );
}

/// Every key collides: lookups must still confirm on the stored domain, and
/// results, counters and layouts must equal the uncolliding queue's.
#[test]
fn exact_index_survives_injected_digest_collisions() {
    fn collide_all<const N: usize>(_: &CompactDomain<N>) -> Digest {
        Digest(7)
    }
    fn collide_in_pairs<const N: usize>(d: &CompactDomain<N>) -> Digest {
        Digest(d.lower(0) / 2)
    }
    let stream = super::aggregate::complete_proposals();
    for (name, key) in [
        ("all", collide_all::<2> as fn(&CompactDomain<2>) -> Digest),
        ("pairs", collide_in_pairs::<2>),
    ] {
        let mut reference = Queue::new(stream.len(), None);
        let mut colliding = Queue::new(stream.len(), None);
        colliding.exact.set_key_function(key);
        let mut finite = Queue::new(stream.len(), Some(usize::MAX));
        let mut finite_colliding = Queue::new(stream.len(), Some(usize::MAX));
        finite_colliding.exact.set_key_function(key);
        for request in stream.iter().take(12_000) {
            let expected = reference.admit(request.clone());
            assert_eq!(colliding.admit(request.clone()), expected, "{name}");
            let expected = finite.admit(request.clone());
            assert_eq!(finite_colliding.admit(request.clone()), expected, "{name}");
        }
        assert_eq!(reference.domains, colliding.domains);
        assert_eq!(reference.exact_hits, colliding.exact_hits);
        assert_eq!(reference.containment_checks, colliding.containment_checks);
        assert_eq!(reference.exact.len(), colliding.exact.len());
        assert_eq!(reference.exact.overflow_len(), 0);
        assert!(colliding.exact.overflow_len() > 0, "{name}");
        assert_eq!(finite.domains, finite_colliding.domains);
        assert_eq!(finite.exact_hits, finite_colliding.exact_hits);
        assert!(reference.exact_hits > 0);
        // The production restore path rebuilds the index under the same key
        // function: distinct domains sharing a digest are not duplicates, and
        // a genuinely repeated domain still is.
        let image = serde_json::to_value(&colliding).unwrap();
        let restore = |image: &serde_json::Value| {
            let mut exact = ExactIndex::new();
            exact.set_key_function(key);
            Queue::<2>::restore_image(serde_json::from_value(image.clone()).unwrap(), exact)
        };
        let mut restored = restore(&image).unwrap();
        assert_eq!(restored.domains, colliding.domains);
        assert_eq!(restored.exact, colliding.exact);
        let mut repeated = image.clone();
        let first = repeated[1][0].clone();
        repeated[1].as_array_mut().unwrap().push(first);
        let error = restore(&repeated).err().unwrap();
        assert!(
            error.contains("duplicate checkpoint exact domain"),
            "{error}"
        );
        // Restored serial and helper-prepared admissions (whose digest the
        // commit reuses) continue under collisions exactly like uncolliding
        // serial and prepared references, over seen (exact hit) and new
        // proposals. A revalidated prepared scan may do more or less work
        // than a serial one, so prepared counters match the prepared reference.
        let mut reference_prepared: Queue<2> =
            serde_json::from_value(serde_json::to_value(&reference).unwrap()).unwrap();
        for batch in stream[11_000..13_000].chunks(64) {
            let tokens = parallel_prepare(&colliding, batch, 2);
            let reference_tokens = parallel_prepare(&reference_prepared, batch, 2);
            for ((request, token), reference_token) in
                batch.iter().zip(tokens).zip(reference_tokens)
            {
                let expected = reference.admit(request.clone());
                assert_eq!(restored.admit(request.clone()), expected, "{name}");
                assert_eq!(colliding.admit_prepared(token), expected, "{name}");
                assert_eq!(
                    reference_prepared.admit_prepared(reference_token),
                    expected,
                    "{name}"
                );
            }
        }
        same_state(&restored, &colliding);
        for (queue, reference) in [(&restored, &reference), (&colliding, &reference_prepared)] {
            assert_eq!(queue.domains, reference.domains);
            assert_eq!(queue.exact_hits, reference.exact_hits);
            assert_eq!(queue.containment_checks, reference.containment_checks);
            assert_eq!(queue.exact.len(), reference.exact.len());
        }
    }
}

/// A snapshot winner retired before commit released its slot, which the next
/// admission reuses. Revalidation must fall back without reading that slot
/// (a released slot read panics even in release builds), exactly like an
/// `is_live == false` winner, with results identical to the serial queue.
#[test]
fn retired_summary_slots_are_released_and_never_read_by_revalidate() {
    let boxed = |lower: [u64; 2], upper: [u64; 2]| Domain {
        lower: lower.to_vec(),
        upper: upper.into_iter().map(Some).collect(),
        ..domain(Some(10))
    };
    let contained = boxed([1, 1], [2, 2]);
    let container = boxed([0, 0], [3, 3]);
    let unrelated = boxed([7, 0], [9, 1]);
    let request = boxed([1, 1], [1, 1]);
    for workers in [1, 2] {
        let mut serial = Queue::new(20, None);
        let mut prepared = Queue::new(20, None);
        for queue in [&mut serial, &mut prepared] {
            assert_eq!(queue.admit(contained.clone()), Ok((0, true)));
        }
        let tokens = parallel_prepare(&prepared, std::slice::from_ref(&request), workers);
        for queue in [&mut serial, &mut prepared] {
            assert_eq!(queue.admit(container.clone()), Ok((1, true)));
            assert!(!queue.is_indexed(0));
            assert_eq!(queue.containment_candidate_count(), 1);
            assert_eq!(queue.admit(unrelated.clone()), Ok((2, true)));
            // Summaries are per ID and immutable: the retired ID keeps its own.
            assert_eq!(queue.summaries.len(), 3);
            assert_eq!(queue.containment_candidate_count(), 2);
        }
        for token in tokens {
            let expected = serial.admit(request.clone());
            assert_eq!(expected, Ok((1, false)));
            assert_eq!(prepared.admit_prepared(token), expected);
        }
        same_state(&serial, &prepared);
        assert_eq!(serial.containment_checks, prepared.containment_checks);
    }
}

/// Every admitted ID keeps its immutable summary across a long overlapping
/// stream, and a restored queue rebuilds the same summaries and index kernel.
#[test]
fn immutable_summaries_and_index_kernel_survive_restore() {
    let stream = super::aggregate::complete_proposals();
    let mut queue = Queue::new(stream.len(), None);
    for request in &stream {
        let _ = queue.admit(request.clone());
    }
    let live = queue.containment_candidate_count();
    assert!(queue.containment_retired_candidates > 0);
    assert_eq!(queue.summaries.len(), queue.domains.len());
    assert_eq!(
        (0..queue.domains.len())
            .filter(|&id| queue.is_indexed(id))
            .count(),
        live
    );
    let image = serde_json::to_string(&queue).unwrap();
    let restored: Queue<2> = serde_json::from_str(&image).unwrap();
    assert_eq!(restored.domains, queue.domains);
    assert_eq!(restored.exact, queue.exact);
    assert_eq!(restored.summaries, queue.summaries);
    assert_eq!(restored.bit_words(), queue.bit_words());
    assert_eq!(restored.containment_candidate_count(), live);
    for (key, bucket) in &queue.by_owner {
        let other = &restored.by_owner[key];
        assert_eq!(bucket.indexed.layout(), other.indexed.layout());
        assert_eq!(bucket.indexed.kernel_image(), other.indexed.kernel_image());
    }
    // The image round-trips byte for byte (stale dead slots included).
    assert_eq!(serde_json::to_string(&restored).unwrap(), image);
    // The restored queue continues exactly like the original.
    let mut original = queue;
    let mut restored = restored;
    for request in stream.iter().rev().take(2_000) {
        assert_eq!(
            restored.admit(request.clone()),
            original.admit(request.clone())
        );
    }
    assert_eq!(restored.domains, original.domains);
    assert_eq!(restored.containment_checks, original.containment_checks);
}

fn owner_box(lower: [u64; 2], upper: [Option<u64>; 2], rank: Option<u32>) -> Domain<2> {
    Domain {
        phase: Phase::Apply,
        owner: [true, true],
        lower: lower.to_vec(),
        upper: upper.to_vec(),
        rank,
        powers: DomainPowerBounds::default(),
    }
}

/// A1 as a restore invariant: a CP5 whose bucket names a domain that is not
/// a full orthant as its orthant is refused. And should a queue ever hold one,
/// the helper takes the commit's own shortcut predicate, so a prepared
/// admission still finds the older container that the serial lookup finds.
#[test]
fn non_full_orthant_is_refused_at_restore_and_never_short_cuts_a_helper() {
    // One inactive axis, so the rank bounds the numerator: the rank-3 orthant
    // does not contain the rank-unbounded box (numerator up to 10).
    let owned = |mut domain: Domain<2>| {
        domain.owner = [true, false];
        domain
    };
    let orthant = owned(owner_box([0, 0], [None, None], Some(3)));
    let container = owned(owner_box([0, 0], [Some(10), Some(10)], None));
    let query = owned(owner_box([1, 1], [Some(2), Some(2)], Some(2)));
    let mut queue = Queue::<2>::new(8, None);
    assert_eq!(queue.admit(orthant), Ok((0, true)));
    assert_eq!(queue.admit(container), Ok((1, true)));
    let key = (Phase::Apply, [true, false]);
    assert_eq!(queue.by_owner[&key].orthant, Some(0));

    let image = serde_json::to_value(&queue).unwrap();
    let restored: Queue<2> = serde_json::from_value(image.clone()).unwrap();
    assert_eq!(restored.by_owner[&key].orthant, Some(0));
    let mut tampered = image.clone();
    let bucket = &mut tampered[2][0][2];
    assert_eq!(bucket["orthant"], 0);
    bucket["orthant"] = serde_json::json!(1);
    let error = serde_json::from_value::<Queue<2>>(tampered)
        .err()
        .expect("a non-full-orthant orthant ID is refused")
        .to_string();
    assert!(
        error.contains("invalid checkpoint full-orthant ID"),
        "{error}"
    );

    // In memory only (restore refuses it): the orthant names the finite box,
    // whose rank still contains the query's. Serial and prepared agree.
    let mut serial: Queue<2> = serde_json::from_value(image.clone()).unwrap();
    let mut prepared: Queue<2> = serde_json::from_value(image).unwrap();
    for queue in [&mut serial, &mut prepared] {
        queue.by_owner.get_mut(&key).unwrap().orthant = Some(1);
    }
    let token =
        prepared.prepare_admission(query.clone(), &std::sync::atomic::AtomicBool::new(false));
    let expected = serial.admit(query);
    assert_eq!(expected, Ok((0, false)));
    assert_eq!(serial.orthant_hits, 0);
    assert_eq!(prepared.admit_prepared(token), expected);
    same_state(&serial, &prepared);
    assert_eq!(serial.containment_checks, prepared.containment_checks);
}

/// Dead block slots keep their stale IDs byte for byte; one that is not a
/// former admission ID (out of range, or not a u32) is refused, never mapped.
#[test]
fn stale_block_slots_restore_exactly_or_are_refused() {
    // Points of one antidiagonal share a signature, hence one group.
    let point = |x: u64| owner_box([x, 40 - x], [Some(x), Some(40 - x)], None);
    let mut queue = Queue::<2>::new(64, None);
    for x in 0..40 {
        assert_eq!(queue.admit(point(x)), Ok((x as usize, true)));
    }
    // A segment containing x = 0..=19 retires them: the first block keeps
    // x = 20..=31 and, in its dead slots, the stale IDs of its old tail.
    let segment = owner_box([0, 21], [Some(19), Some(40)], None);
    assert_eq!(queue.admit(segment), Ok((40, true)));
    assert_eq!(queue.containment_retired_candidates, 20);
    let image = serde_json::to_value(&queue).unwrap();
    let text = serde_json::to_string(&queue).unwrap();
    let restored: Queue<2> = serde_json::from_value(image.clone()).unwrap();
    assert_eq!(serde_json::to_string(&restored).unwrap(), text);
    let block = &image[2][0][2]["indexed"]["groups"][0]["blocks"][0];
    assert_eq!(block["len"], 12);
    assert_eq!(block["ids"][31], 31, "a former live ID in a dead slot");
    let domains = queue.domains.len() as u64;
    for (stale, accepted) in [
        (domains - 1, true),
        (0, true),
        (domains, false),
        (u64::from(u32::MAX), false),
        (u64::from(u32::MAX) + 7, false),
    ] {
        let mut edited = image.clone();
        edited[2][0][2]["indexed"]["groups"][0]["blocks"][0]["ids"][31] = serde_json::json!(stale);
        match serde_json::from_value::<Queue<2>>(edited.clone()) {
            Ok(restored) => {
                assert!(accepted, "stale ID {stale} accepted");
                // Re-encoded byte for byte, the stale value included.
                assert_eq!(serde_json::to_value(&restored).unwrap(), edited);
            }
            Err(error) => {
                assert!(!accepted, "stale ID {stale} refused: {error}");
                assert!(
                    error
                        .to_string()
                        .contains("invalid checkpoint stale block slot"),
                    "{error}"
                );
            }
        }
    }
}
