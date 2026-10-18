//! Same family rules through private versus cross-family shared execution.
use super::*;
use rustred::solver::SectorExecutor;
use std::sync::{Arc, atomic::AtomicBool};

#[test]
fn shared_families_preserve_exact_programs_and_parallel_zero_census() {
    if symbolica::license::LicenseManager::max_threads(4) < 4 {
        return;
    }
    let executor = SectorExecutor::new(4)
        .unwrap()
        .with_shared_scheduling(Some(2))
        .unwrap();
    let expected: Vec<_> = [K1, K3]
        .into_iter()
        .map(|source| {
            let mut request = FamilyCandidatesRequest::new(source);
            request.root_zero_census = true;
            family_candidates(request).unwrap()
        })
        .collect();
    let generated = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for source in [K1, K3] {
            let pool = &executor;
            handles.push(scope.spawn(move || {
                let mut request = FamilyCandidatesRequest::new(source);
                request.root_zero_census = true;
                request.n_cores = 4;
                crate::family_candidates_on_executor(
                    request,
                    pool,
                    Arc::new(AtomicBool::new(false)),
                    |_| {},
                )
                .unwrap()
            }));
        }
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    for (expected, generated) in expected.iter().zip(&generated) {
        assert_same_program(expected.bundle(), generated.bundle());
    }
    assert!(executor.shared_snapshot().unwrap().peak_running <= 2);
}

#[test]
fn shared_completed_checkpoint_reuses_all_sectors_and_preserves_recipe() {
    if symbolica::license::LicenseManager::max_threads(2) < 2 {
        return;
    }
    let directory = super::checkpoint::Directory::new();
    let pool = SectorExecutor::new(2)
        .unwrap()
        .with_shared_scheduling(Some(1))
        .unwrap();
    let mut request = FamilyCandidatesRequest::new(K1);
    request.n_cores = 2;
    request.root_zero_census = true;
    request.checkpoint = Some(directory.options());
    let first = crate::family_candidates_on_executor(
        request.clone(),
        &pool,
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .unwrap();
    let count = pool.shared_snapshot().unwrap().completed;
    request.checkpoint.as_mut().unwrap().resume = true;
    let second = crate::family_candidates_on_executor(
        request,
        &pool,
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .unwrap();
    assert_same_program(first.bundle(), second.bundle());
    assert_eq!(pool.shared_snapshot().unwrap().completed, count);
}
