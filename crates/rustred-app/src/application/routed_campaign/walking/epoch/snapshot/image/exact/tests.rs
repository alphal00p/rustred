use super::super::super::shared::{Copies, Pages};
use super::*;
use crate::application::routed_campaign::walking::queue::{CompactDomain, Domain, Phase};
use rustred::solver::DomainPowerBounds;
use std::collections::HashMap;

// Only geometry is read by the exact-index service; no Store admission or
// aggregate-index setup is needed to exercise large deterministic key merges.
fn geometry(count: usize) -> Image<2> {
    let mut domains = Pages::default();
    for id in 0..count {
        let lower = (id % 13) as u64;
        let image = CompactDomain::try_from_domain(&Domain {
            phase: Phase::Apply,
            owner: [true, false],
            lower: vec![lower, 0],
            upper: vec![Some(lower + 1), None],
            rank: Some(3),
            powers: DomainPowerBounds::default(),
        })
        .unwrap();
        domains
            .try_push(image, &mut Copies::default(), &mut || Ok(()))
            .unwrap();
    }
    Image {
        domains,
        summaries: Pages::default(),
        live: Pages::default(),
        quarantine: Pages::default(),
        rescue_duplicates: false,
        layers: Vec::new(),
    }
}

fn expected(image: &Image<2>, first: u32, end: u32) -> Vec<(u64, u32)> {
    let mut result: Vec<_> = (first..end)
        .map(|id| (image.domains[id as usize].digest().0, id))
        .collect();
    result.sort_unstable();
    result
}

fn prior(image: &Image<2>, first: u32, end: u32) -> Arc<Layer<2>> {
    Arc::new(Layer {
        first,
        end,
        exact: expected(image, first, end),
        buckets: HashMap::new(),
        orthants: HashMap::new(),
    })
}

#[test]
fn chunk_boundaries_and_duplicate_digest_keys_match_standard_sort() {
    for count in [4095, 4096, 4097, 8193] {
        let image = geometry(count);
        let result = merge(&image, 0, count as u32, &[], &mut || Ok(())).unwrap();
        assert_eq!(result, expected(&image, 0, count as u32));
        assert!(result.windows(2).any(|p| p[0].0 == p[1].0));
    }
}

#[test]
fn prior_cohorts_and_multichunk_tail_merge_without_mutating_inputs() {
    let image = geometry(15000);
    let existing = [prior(&image, 0, 2048), prior(&image, 2048, 5000)];
    let originals: Vec<_> = existing.iter().map(|layer| layer.exact.clone()).collect();
    let mut failures = 0;
    let mut success = false;
    for stop_at in 0..64 {
        let mut calls = 0;
        let result = merge(&image, 0, 15000, &existing, &mut || {
            let at = calls;
            calls += 1;
            if at == stop_at {
                Err("injected merge interruption")
            } else {
                Ok(())
            }
        });
        assert!(
            existing
                .iter()
                .zip(&originals)
                .all(|(layer, original)| layer.exact == *original)
        );
        match result {
            Err(_) => failures += 1,
            Ok(result) => {
                assert_eq!(result, expected(&image, 0, 15000));
                success = true;
                break;
            }
        }
    }
    assert!(failures >= 10 && success);
    assert!(merge(&image, 1, 15000, &existing, &mut || Ok(())).is_err());
}
