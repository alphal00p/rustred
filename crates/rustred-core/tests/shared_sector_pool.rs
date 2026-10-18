//! Public scheduling boundaries with exact native sector solves and nested work.
use rustred::algebra::CoefficientContext;
use rustred::solver::{
    Integral, SectorConfig, SectorExecutor, SectorSolveOptions, SourceSystem, Term,
};
use std::collections::HashSet;
use std::convert::Infallible;
use std::sync::{
    Arc, Barrier, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

fn sources() -> SourceSystem<1> {
    let context = CoefficientContext::try_new(["n0"]).unwrap();
    SourceSystem::new(
        vec![vec![Term {
            integral: Integral::symbolic([0]).unwrap(),
            coefficient: context.one().numerator,
        }]],
        [0],
    )
    .unwrap()
}

#[test]
fn remaining_family_uses_all_shared_workers() {
    let pool = SectorExecutor::new(4)
        .unwrap()
        .with_shared_scheduling(None)
        .unwrap();
    let small = pool.clone();
    let sources = sources();
    small
        .map(
            &sources,
            &[[true]],
            &SectorConfig::default(),
            SectorSolveOptions::default(),
            |_| Ok::<_, Infallible>(()),
        )
        .unwrap();
    let barrier = Barrier::new(4);
    let seen = Mutex::new(HashSet::new());
    let result = pool
        .map(
            &sources,
            &[[true]; 8],
            &SectorConfig::default(),
            SectorSolveOptions::default(),
            |done| {
                seen.lock().unwrap().insert(std::thread::current().id());
                barrier.wait();
                Ok::<_, Infallible>(done.ordinal)
            },
        )
        .unwrap();
    assert_eq!(result, (0..8).collect::<Vec<_>>());
    assert_eq!(seen.into_inner().unwrap().len(), 4);
    assert!(pool.shared_snapshot().unwrap().peak_running >= 4);
}

#[test]
fn global_limit_is_shared_across_families_with_nested_work() {
    let pool = SectorExecutor::new(4)
        .unwrap()
        .with_shared_scheduling(Some(2))
        .unwrap();
    let sources = sources();
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let (pool, sources, barrier) = (pool.clone(), &sources, &barrier);
            scope.spawn(move || {
                pool.map(
                    sources,
                    &[[true]; 4],
                    &SectorConfig::default(),
                    SectorSolveOptions::default(),
                    |_| {
                        assert_eq!(rayon::join(|| 20, || 22), (20, 22));
                        barrier.wait();
                        Ok::<_, Infallible>(())
                    },
                )
                .unwrap()
            });
        }
    });
    let stats = pool.shared_snapshot().unwrap();
    assert_eq!(
        (
            stats.peak_running,
            stats.running,
            stats.admitted,
            stats.completed
        ),
        (2, 0, 0, 8)
    );
}

#[test]
fn cancelled_admission_returns_before_running_sector_finishes() {
    let cancel = Arc::new(AtomicBool::new(false));
    let pool = SectorExecutor::new(2)
        .unwrap()
        .with_shared_scheduling(Some(1))
        .unwrap()
        .with_cancellation(Arc::clone(&cancel));
    let sources = sources();
    let (entered, started) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let gate = Mutex::new(gate);
    std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            pool.map(
                &sources,
                &[[true]],
                &SectorConfig::default(),
                SectorSolveOptions::default(),
                |_| {
                    entered.send(()).unwrap();
                    gate.lock().unwrap().recv().unwrap();
                    Ok::<_, Infallible>(())
                },
            )
        });
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        let second = scope.spawn(|| {
            pool.map(
                &sources,
                &[[true]],
                &SectorConfig::default(),
                SectorSolveOptions::default(),
                |_| Ok::<_, Infallible>(()),
            )
        });
        cancel.store(true, Ordering::Release);
        assert!(second.join().unwrap().is_err());
        release.send(()).unwrap();
        first.join().unwrap().unwrap();
    });
    assert_eq!(pool.shared_snapshot().unwrap().admitted, 0);
}

#[test]
fn panicking_consumer_returns_admission_and_leaves_pool_reusable() {
    let pool = SectorExecutor::new(2)
        .unwrap()
        .with_shared_scheduling(Some(1))
        .unwrap();
    let sources = sources();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pool.map(
                &sources,
                &[[true]],
                &SectorConfig::default(),
                SectorSolveOptions::default(),
                |_| -> Result<(), Infallible> { panic!("consumer failure") },
            )
            .unwrap();
        }))
        .is_err()
    );
    assert_eq!(pool.shared_snapshot().unwrap().admitted, 0);
    assert_eq!(
        pool.map(
            &sources,
            &[[true]],
            &SectorConfig::default(),
            SectorSolveOptions::default(),
            |_| Ok::<_, Infallible>(42)
        )
        .unwrap(),
        [42]
    );
}

#[test]
fn recorded_shared_programs_have_identical_native_structure_and_coefficients() {
    let Some(root) = std::env::var_os("RUSTRED_SHARED_COMPARISON_ROOT") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    for family in ["mass1", "mass4"] {
        let original =
            std::fs::read(root.join(format!("isolated/generation/{family}/candidates.rrbin")))
                .unwrap();
        for mode in ["shared", "shared-cap1", "native-resume"] {
            let candidate =
                std::fs::read(root.join(format!("{mode}/generation/{family}/candidates.rrbin")))
                    .unwrap();
            assert!(
                rustred::persistence::equivalent_generated_programs(
                    &original,
                    &candidate,
                    Default::default()
                )
                .unwrap(),
                "native program differs: {family} / {mode}"
            );
        }
    }
}
