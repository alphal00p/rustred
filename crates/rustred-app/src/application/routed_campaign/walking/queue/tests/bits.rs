//! The bit tier is a necessary condition only: it never changes a result.
use super::super::bits::{may_contain, word};
use super::prepared::{parallel_prepare, same_state};
use super::*;

/// Every small-grid summary of arity N (Err projections skipped, empties kept)
/// under two owner patterns, thinned by `stride` to keep pair counts sane.
fn grid<const N: usize>(stride: usize) -> Vec<DomainPowerSummary<N>> {
    const AXES: [(u64, Option<u64>); 12] = [
        (0, None),
        (0, Some(0)),
        (0, Some(1)),
        (0, Some(3)),
        (1, None),
        (1, Some(0)),
        (1, Some(1)),
        (1, Some(3)),
        (3, None),
        (3, Some(0)),
        (3, Some(1)),
        (3, Some(3)),
    ];
    let mut owners = vec![[true; N]];
    let mut alternating = [true; N];
    for (axis, flag) in alternating.iter_mut().enumerate() {
        *flag = axis % 2 == 0;
    }
    owners.push(alternating);
    let mut out = Vec::new();
    let mut ordinal = 0_usize;
    for owner in owners {
        for combo in 0..AXES.len().pow(N as u32) {
            let mut lower = Vec::with_capacity(N);
            let mut upper = Vec::with_capacity(N);
            let mut rest = combo;
            for _ in 0..N {
                let (lo, hi) = AXES[rest % AXES.len()];
                rest /= AXES.len();
                lower.push(lo);
                upper.push(hi);
            }
            for rank in [None, Some(0), Some(2)] {
                for a in [None, Some(0), Some(4)] {
                    for dmin in [None, Some(-1), Some(1)] {
                        for dmax in [None, Some(0)] {
                            ordinal += 1;
                            if ordinal % stride != 0 {
                                continue;
                            }
                            let powers = DomainPowerBounds {
                                max_positive_power: a,
                                min_power_difference: dmin,
                                max_power_difference: dmax,
                            };
                            if let Ok(summary) =
                                DomainPowerSummary::try_new(owner, &lower, &upper, rank, powers)
                            {
                                out.push(summary);
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn necessary_condition_holds<const N: usize>(stride: usize) -> (usize, usize, usize) {
    let summaries = grid::<N>(stride);
    let words: Vec<u64> = summaries.iter().map(word).collect();
    let mut inclusions = 0;
    let mut rejections = 0;
    let mut empties = 0;
    for (container, &c) in summaries.iter().zip(&words) {
        for (candidate, &q) in summaries.iter().zip(&words) {
            let contains = container.contains(candidate);
            let passes = may_contain(c, q);
            assert!(
                !contains || passes,
                "N={N}: inclusion rejected by bits\n{container:?}\n{candidate:?}\n{c:#b}\n{q:#b}"
            );
            inclusions += usize::from(contains);
            rejections += usize::from(!passes);
        }
        if container.is_empty() {
            empties += 1;
            assert_eq!(c, 0, "empty summaries have word 0");
        }
    }
    assert!(inclusions > 0 && rejections > 0, "N={N}: degenerate grid");
    println!("bit_prefilter_grid N={N} empties={empties}");
    (summaries.len(), inclusions, rejections)
}

#[test]
fn axes_beyond_the_packed_width_are_ignored_not_misfiled() {
    // 20 axes: only the first 16 are packed; the rest never shift into the
    // aggregate flags and the word stays a necessary condition.
    let owner = [true; 20];
    let mut lower = [0_u64; 20];
    let mut upper = [Some(3_u64); 20];
    lower[18] = 2;
    upper[19] = None;
    let wide =
        DomainPowerSummary::try_new(owner, &lower, &upper, Some(40), Default::default()).unwrap();
    let mut narrow_lower = lower;
    narrow_lower[19] = 1;
    let mut narrow_upper = upper;
    narrow_upper[19] = Some(5);
    let narrow = DomainPowerSummary::try_new(
        owner,
        &narrow_lower,
        &narrow_upper,
        Some(40),
        Default::default(),
    )
    .unwrap();
    let (w, n) = (word(&wide), word(&narrow));
    assert_eq!(w & 0xFFFF, 0, "no packed axis is unbounded");
    assert_eq!(w >> 16 & 0xFFFF, 0xFFFF, "all packed lower bounds are zero");
    assert!(wide.contains(&narrow) && may_contain(w, n));
    assert!(!narrow.contains(&wide));
}

#[test]
fn bit_words_are_a_necessary_condition_for_exact_inclusion_on_small_grids() {
    let one = necessary_condition_holds::<1>(1);
    let two = necessary_condition_holds::<2>(5);
    let three = necessary_condition_holds::<3>(41);
    println!("bit_prefilter_grid N=1 {one:?} N=2 {two:?} N=3 {three:?}");
}

#[test]
fn empty_summary_rules_match_contains() {
    let mut empty = domain(None);
    empty.powers.max_positive_power = Some(0);
    empty.lower[0] = 1;
    let empty = DomainPowerSummary::try_new(
        empty.owner,
        &empty.lower,
        &empty.upper,
        empty.rank,
        empty.powers,
    )
    .unwrap();
    assert!(empty.is_empty());
    assert_eq!(word(&empty), 0);
    let full = domain(None);
    let full =
        DomainPowerSummary::try_new(full.owner, &full.lower, &full.upper, full.rank, full.powers)
            .unwrap();
    assert!(full.contains(&empty) && may_contain(word(&full), word(&empty)));
    assert!(!full.is_empty() && word(&full) != 0);
    // An empty container passes only an all-zero candidate; the exact
    // comparison then rejects it, exactly as before the tier existed.
    assert!(!may_contain(word(&empty), word(&full)));
    assert!(!empty.contains(&full));
}

fn shuffled(order: usize) -> Vec<Domain<2>> {
    let mut stream = super::aggregate::complete_proposals();
    if order == 1 {
        stream.reverse();
    } else if order == 2 {
        let mut state = 43_u64;
        for i in (1..stream.len()).rev() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            stream.swap(i, state as usize % (i + 1));
        }
    }
    stream
}

#[test]
fn prefilter_toggle_preserves_results_and_persisted_counters_on_proposal_streams() {
    let mut rejections = SessionCounters::default();
    for (order, workers, batch_size) in [(0, 1, 1), (1, 3, 17), (2, 6, 256)] {
        let stream = shuffled(order);
        let mut filtered = Queue::new(stream.len(), None);
        let mut unfiltered = Queue::new(stream.len(), None);
        unfiltered.disable_bit_prefilter();
        let mut serial = Queue::new(stream.len(), None);
        let mut serial_unfiltered = Queue::new(stream.len(), None);
        serial_unfiltered.disable_bit_prefilter();
        for batch in stream.chunks(batch_size) {
            let with = parallel_prepare(&filtered, batch, workers);
            let without = parallel_prepare(&unfiltered, batch, workers);
            for ((item, with), without) in batch.iter().zip(with).zip(without) {
                let expected = serial.admit(item.clone());
                assert_eq!(serial_unfiltered.admit(item.clone()), expected);
                assert_eq!(filtered.admit_prepared(with), expected);
                assert_eq!(unfiltered.admit_prepared(without), expected);
                same_state(&serial, &serial_unfiltered);
                same_state(&filtered, &unfiltered);
                same_state(&serial, &filtered);
                assert_eq!(
                    serial.containment_checks,
                    serial_unfiltered.containment_checks
                );
                assert_eq!(filtered.containment_checks, unfiltered.containment_checks);
            }
        }
        assert_eq!(
            serial.session.forward_callbacks,
            serial_unfiltered.session.forward_callbacks
        );
        assert_eq!(
            serial.session.reverse_callbacks,
            serial_unfiltered.session.reverse_callbacks
        );
        assert_eq!(serial_unfiltered.session.forward_bit_rejections, 0);
        assert_eq!(serial_unfiltered.session.reverse_bit_rejections, 0);
        // Whether the tier rejects anything beyond the group/block filters is
        // order dependent (wide domains first leave only immediate hits), so
        // only the totals over all orders must show both tiers firing.
        rejections.forward_bit_rejections += serial.session.forward_bit_rejections;
        rejections.reverse_bit_rejections += serial.session.reverse_bit_rejections;
        assert_eq!(
            filtered.session.forward_callbacks,
            unfiltered.session.forward_callbacks
        );
        assert_eq!(
            filtered.session.reverse_callbacks,
            unfiltered.session.reverse_callbacks
        );
        // Single-proposal batches commit entirely from their snapshots: the
        // coordinator then runs no callback at all. Larger batches compare
        // in-batch admissions above the watermark on the commit path.
        if batch_size == 1 {
            assert_eq!(filtered.session.forward_callbacks, 0);
            assert_eq!(filtered.session.reverse_callbacks, 0);
        } else {
            assert!(filtered.session.forward_callbacks + filtered.session.reverse_callbacks > 0);
        }
        println!(
            "bit_prefilter order={order} workers={workers} batch={batch_size} serial={:?} prepared={:?}",
            serial.session, filtered.session
        );
    }
    assert!(rejections.forward_bit_rejections > 0);
    assert!(rejections.reverse_bit_rejections > 0);
}

#[test]
fn restored_queue_rebuilds_bit_words_and_keeps_admitting_identically() {
    let stream = shuffled(2);
    let mut queue = Queue::new(stream.len() + 8, None);
    for item in &stream[..stream.len() / 2] {
        queue.admit(item.clone()).unwrap();
    }
    let expected_words: Vec<u64> = queue.bit_words().to_vec();
    assert_eq!(expected_words.len(), queue.domains.len());
    assert!(expected_words.iter().any(|&w| w != 0));
    let state = super::super::super::execution::State::new(queue, 0, None);
    let mut restored = super::super::super::checkpoint::round_trip_state(&state).unwrap();
    let mut original = state;
    assert_eq!(restored.queue.bit_words(), expected_words.as_slice());
    assert_eq!(restored.queue.session, SessionCounters::default());
    for item in &stream[stream.len() / 2..] {
        assert_eq!(
            restored.queue.admit(item.clone()),
            original.queue.admit(item.clone())
        );
    }
    same_state(&original.queue, &restored.queue);
    assert_eq!(
        original.queue.containment_checks,
        restored.queue.containment_checks
    );
    assert_eq!(original.queue.bit_words(), restored.queue.bit_words());
}
