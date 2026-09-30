use super::*;
use crate::application::routed_campaign::walking::queue::{Domain, Phase};
use rustred::solver::DomainPowerBounds;

fn domain(lo: u64, hi: Option<u64>, rank: Option<u32>) -> CompactDomain<2> {
    CompactDomain::try_from_domain(&Domain {
        phase: Phase::Apply,
        owner: [true, false],
        lower: vec![lo, 0],
        upper: vec![hi, None],
        rank,
        powers: DomainPowerBounds::default(),
    })
    .unwrap()
}

fn append(store: &mut Store<2>, domain: CompactDomain<2>) -> (u32, Vec<u32>) {
    let q = QueryImage::new(domain).unwrap();
    let query = Query::new(q.core.clone(), domain.phase());
    let retire = store
        .contained_live(&q, &query, &mut Default::default())
        .unwrap();
    store.try_reserve(1).unwrap();
    store.exact.try_reserve_one(q.digest).unwrap();
    let prepared = store.prepare_initial(&domain, &query, || Ok(())).unwrap();
    let id = store.push(domain, query.compact, q.digest).unwrap();
    assert_eq!(
        store.index_initial(id, &query, &retire, prepared).unwrap(),
        retire.len() as u64
    );
    (id, retire)
}

fn build(store: &Store<2>) -> Image<2> {
    Image::bootstrap(store, &mut Copies::default(), &mut || Ok(())).unwrap()
}

fn advance(image: &Image<2>, store: &Store<2>, updates: &[(u32, Vec<u32>)]) -> Image<2> {
    image
        .advance(
            store,
            updates.iter().map(|(id, r)| (*id, r.as_slice())),
            &mut Copies::default(),
            &mut || Ok(()),
        )
        .unwrap()
}

fn winner(image: &Image<2>, domain: CompactDomain<2>) -> Option<(u32, Hit)> {
    let q = QueryImage::new(domain).unwrap();
    let query = Query::new(q.core.clone(), domain.phase());
    image
        .lookup(
            &q,
            &query,
            image.len(),
            &mut Default::default(),
            &mut Default::default(),
        )
        .unwrap()
        .map(|(id, _, hit)| (id, hit))
}

fn differential(image: &Image<2>, store: &Store<2>) {
    for lo in 0..14 {
        for hi in [Some(lo), Some(lo + 1), Some(lo + 8), None] {
            for rank in [Some(0), Some(3), Some(10), None] {
                let domain = domain(lo, hi, rank);
                let q = QueryImage::new(domain).unwrap();
                let query = Query::new(q.core.clone(), domain.phase());
                let expected = store
                    .lookup(
                        &q,
                        &query,
                        store.len(),
                        &mut Default::default(),
                        &mut Default::default(),
                    )
                    .unwrap()
                    .map(|(id, _, hit)| (id, hit));
                assert_eq!(winner(image, domain), expected, "query {domain:?}");
            }
        }
    }
}

#[test]
fn held_roots_preserve_retired_exact_and_minimum_current_live_across_layers() {
    let mut store = Store::new();
    append(&mut store, domain(2, Some(2), Some(3)));
    append(&mut store, domain(0, Some(6), Some(3)));
    let old = build(&store);
    differential(&old, &store);
    let updates = [append(&mut store, domain(1, Some(10), Some(10)))];
    let next = advance(&old, &store, &updates);
    assert_eq!(next.layers.len(), 2);
    assert_eq!(
        winner(&next, domain(2, Some(2), Some(3))),
        Some((0, Hit::Exact))
    );
    assert_eq!(
        winner(&next, domain(3, Some(4), Some(3))),
        Some((1, Hit::Contained))
    );
    assert_eq!(
        winner(&next, domain(7, Some(8), Some(3))),
        Some((2, Hit::Contained))
    );
    assert_eq!(winner(&old, domain(7, Some(8), Some(3))), None);
    differential(&next, &store);
    let updates = [append(&mut store, domain(0, Some(12), Some(10)))];
    let compacted = advance(&next, &store, &updates);
    assert_eq!(compacted.layers.len(), 1);
    differential(&compacted, &store);
    assert_eq!(
        winner(&compacted, domain(3, Some(4), Some(3))),
        Some((3, Hit::Contained))
    );
    assert_eq!(
        winner(&next, domain(3, Some(4), Some(3))),
        Some((1, Hit::Contained))
    );
}

#[test]
fn dominant_orthant_and_quarantine_do_not_resurrect_a_retired_shadow() {
    let mut store = Store::new();
    append(&mut store, domain(0, None, Some(3)));
    append(&mut store, domain(0, None, Some(10)));
    store.rescue_duplicates = true;
    store.install_quarantine(vec![2]).unwrap();
    let old = build(&store);
    differential(&old, &store);
    assert_eq!(winner(&old, domain(1, Some(2), Some(3))), None);
    assert_eq!(
        winner(&old, domain(0, None, Some(3))),
        Some((0, Hit::Exact))
    );
    let updates = [append(&mut store, domain(0, None, Some(10)))];
    let next = advance(&old, &store, &updates);
    differential(&next, &store);
    assert_eq!(
        winner(&next, domain(0, None, Some(10))),
        Some((2, Hit::Exact))
    );
    assert_eq!(
        winner(&next, domain(1, Some(2), Some(3))),
        Some((2, Hit::Orthant))
    );
    assert_eq!(winner(&old, domain(0, None, Some(10))), None);
}

#[test]
fn every_refused_refresh_keeps_old_root_and_canonical_store() {
    let mut store = Store::new();
    append(&mut store, domain(2, Some(2), Some(3)));
    let old = build(&store);
    let updates = [append(&mut store, domain(0, Some(6), Some(3)))];
    let mut failed = 0;
    let mut completed = false;
    for fail_at in 0..100 {
        let mut calls = 0;
        let mut checkpoint = || {
            let at = calls;
            calls += 1;
            if at == fail_at {
                Err("injected refresh refusal")
            } else {
                Ok(())
            }
        };
        match old.advance(
            &store,
            updates.iter().map(|(id, r)| (*id, r.as_slice())),
            &mut Copies::default(),
            &mut checkpoint,
        ) {
            Err(_) => failed += 1,
            Ok(next) => {
                differential(&next, &store);
                completed = true;
                break;
            }
        }
        assert_eq!(old.len(), 1);
        assert_eq!(store.len(), 2);
        assert_eq!(winner(&old, domain(1, Some(2), Some(3))), None);
    }
    assert!(failed > 5 && completed);
}

#[test]
fn incomplete_or_double_retirement_journal_is_rejected() {
    let mut store = Store::new();
    append(&mut store, domain(2, Some(2), Some(3)));
    let old = build(&store);
    append(&mut store, domain(0, Some(6), Some(3)));
    assert!(
        old.advance(
            &store,
            std::iter::empty(),
            &mut Copies::default(),
            &mut || Ok(())
        )
        .is_err()
    );
    assert!(
        old.advance(
            &store,
            [(1, &[0, 0][..])].into_iter(),
            &mut Copies::default(),
            &mut || Ok(())
        )
        .is_err()
    );
}

#[test]
fn overlapping_survivor_retirements_are_idempotent_across_a_cut() {
    let mut store = Store::new();
    append(&mut store, domain(2, Some(2), Some(3)));
    let old = build(&store);
    append(&mut store, domain(0, Some(3), Some(3)));
    append(&mut store, domain(1, Some(4), Some(3)));
    let image = advance(&old, &store, &[(1, vec![0]), (2, vec![0])]);
    differential(&image, &store);
    assert_eq!(
        winner(&image, domain(2, Some(3), Some(3))),
        Some((1, Hit::Contained))
    );
}

#[test]
fn nonmonotone_cut_sizes_keep_logarithmic_cohort_count() {
    let mut store = Store::new();
    let mut image = build(&store);
    let mut next = 0;
    for count in (1..=24).rev() {
        let updates: Vec<_> = (0..count)
            .map(|_| {
                let at = next;
                next += 2;
                append(&mut store, domain(at, Some(at), Some(3)))
            })
            .collect();
        image = advance(&image, &store, &updates);
        assert!(image.layers.len() <= 32);
        for pair in image.layers.windows(2) {
            assert!((pair[0].end - pair[0].first).ilog2() > (pair[1].end - pair[1].first).ilog2());
            assert_eq!(pair[0].end, pair[1].first);
        }
    }
    differential(&image, &store);
}

#[test]
fn forced_digest_collisions_continue_to_a_true_match_in_a_later_cohort() {
    let mut store = Store::new();
    append(&mut store, domain(0, Some(0), Some(3)));
    append(&mut store, domain(100, Some(100), Some(3)));
    let old = build(&store);
    let target = domain(200, Some(200), Some(3));
    let updates = [append(&mut store, target)];
    let mut current = advance(&old, &store, &updates);
    drop(old);
    assert_eq!(current.layers.len(), 2);
    let digest = target.digest().0;
    let layer = Arc::get_mut(&mut current.layers[0]).unwrap();
    layer.exact = vec![(digest, 0), (digest, 1)];
    assert_eq!(winner(&current, target), Some((2, Hit::Exact)));
    let mut miss = QueryImage::new(domain(300, Some(300), Some(3))).unwrap();
    miss.digest = digest;
    let query = Query::new(miss.core.clone(), miss.image.phase());
    assert!(
        current
            .lookup(
                &miss,
                &query,
                current.len(),
                &mut Default::default(),
                &mut Default::default()
            )
            .unwrap()
            .is_none()
    );
}

#[test]
fn phase_owner_correlated_constraints_and_wide_summary_match_canonical_lookup() {
    let mut store = Store::new();
    let mut domains = Vec::new();
    for phase in [Phase::Apply, Phase::Route] {
        for owner in [[true, false], [false, true]] {
            for positive in [6, u64::MAX] {
                let mut raw = domain(0, None, Some(3)).expand();
                raw.phase = phase;
                raw.owner = owner;
                raw.powers.max_positive_power = Some(positive);
                raw.powers.min_power_difference = Some(-2);
                let image = CompactDomain::try_from_domain(&raw).unwrap();
                append(&mut store, image);
                domains.push(image);
            }
        }
    }
    assert!(store.summaries.iter().any(CompactSummary::is_wide));
    let image = build(&store);
    for original in domains {
        for lo in 0..4 {
            for hi in [Some(lo), Some(lo + 2), None] {
                let mut raw = original.expand();
                raw.lower[0] = lo;
                raw.upper[0] = hi;
                let target = CompactDomain::try_from_domain(&raw).unwrap();
                let q = QueryImage::new(target).unwrap();
                let query = Query::new(q.core.clone(), q.image.phase());
                let expected = store
                    .lookup(
                        &q,
                        &query,
                        store.len(),
                        &mut Default::default(),
                        &mut Default::default(),
                    )
                    .unwrap()
                    .map(|(id, _, hit)| (id, hit));
                assert_eq!(winner(&image, target), expected);
            }
        }
    }
}
