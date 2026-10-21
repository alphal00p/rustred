use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;

fn family() -> Arc<IntegralFamily> {
    let c = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            "parallel-finite-sunset",
            vec!["k".into(), "l".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
                .into_iter()
                .map(|v| {
                    AffineDenominator::new(
                        c.integer(-1),
                        v.into_iter().map(|i| c.integer(i)).collect(),
                    )
                })
                .collect(),
            vec![],
            vec![c.zero(); 3],
        )
        .unwrap(),
    )
}
fn key(v: [i64; 3]) -> IntegralKey {
    IntegralKey::try_new(v).unwrap()
}
fn session() -> TerminalRelationSession {
    let mut result = TerminalRelationSession::new(
        family(),
        BTreeSet::from([
            key([1, 1, 1]),
            key([2, 1, 1]),
            key([1, 2, 1]),
            key([2, 0, 1]),
            key([1, -1, 2]),
        ]),
        1,
        Default::default(),
    )
    .unwrap();
    result.extend_containing_sector_seeds(1).unwrap();
    result
}
fn serial_finish(s: &mut TerminalRelationSession) {
    while !s.is_complete() {
        s.step(&AtomicBool::new(false)).unwrap();
    }
}
fn executor(workers: usize, batch_rows: usize, block_rows: usize) -> TerminalRelationExecutor {
    TerminalRelationExecutor::new(TerminalRelationExecutionOptions {
        workers,
        batch_rows,
        block_rows,
    })
    .unwrap()
}
fn parallel_finish(s: &mut TerminalRelationSession, executor: &TerminalRelationExecutor) {
    while !s.is_complete() {
        executor.step(s, &AtomicBool::new(false)).unwrap();
    }
}

#[test]
fn exact_serial_equivalence_across_workers_blocks_and_native_checkpoints() {
    let mut reference = session();
    for _ in 0..13 {
        reference.step(&AtomicBool::new(false)).unwrap();
    }
    let checkpoint = reference.to_native_bytes(Default::default()).unwrap();
    serial_finish(&mut reference);
    for (workers, batch, block) in [(1, 11, 3), (2, 23, 4), (4, 64, 8), (4, 128, 1)] {
        let mut s = TerminalRelationSession::from_native_bytes(
            &checkpoint,
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let pool = executor(workers, batch, block);
        pool.step(&mut s, &AtomicBool::new(false)).unwrap();
        let partial = s.to_native_bytes(Default::default()).unwrap();
        let mut s = TerminalRelationSession::from_native_bytes(
            &partial,
            Default::default(),
            Default::default(),
        )
        .unwrap();
        parallel_finish(&mut s, &pool);
        assert_eq!(
            s.statistics(),
            reference.statistics(),
            "workers={workers}, batch={batch}, block={block}"
        );
        assert_eq!(
            s.apply_all_terminals().unwrap(),
            reference.apply_all_terminals().unwrap()
        );
        assert_eq!(s.nonzero_conditions(), reference.nonzero_conditions());
        assert_eq!(
            s.to_native_bytes(Default::default()).unwrap(),
            reference.to_native_bytes(Default::default()).unwrap()
        );
    }
}

#[test]
fn scope_extension_rebuild_is_parallel_and_checkpointable() {
    let mut baseline = session();
    serial_finish(&mut baseline);
    baseline
        .extend(&BTreeSet::from([key([3, 1, 1]), key([1, 3, 1])]), 1)
        .unwrap();
    let encoded = baseline.to_native_bytes(Default::default()).unwrap();
    assert!(baseline.statistics().pending_rebuild_rows > 0);
    serial_finish(&mut baseline);
    let mut parallel = TerminalRelationSession::from_native_bytes(
        &encoded,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    parallel_finish(&mut parallel, &executor(4, 37, 5));
    assert_eq!(
        parallel.apply_all_terminals().unwrap(),
        baseline.apply_all_terminals().unwrap()
    );
    assert_eq!(parallel.statistics(), baseline.statistics());
}

#[test]
fn cancellation_leaves_a_durable_exact_source_prefix() {
    let mut s = session();
    let pool = executor(4, 128, 8);
    let before = s.to_native_bytes(Default::default()).unwrap();
    pool.step(&mut s, &AtomicBool::new(true)).unwrap();
    assert_eq!(s.to_native_bytes(Default::default()).unwrap(), before);
    let cancel = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(1));
            cancel.store(true, Ordering::Relaxed);
        });
        pool.step(&mut s, &cancel).unwrap();
    });
    let durable = s.to_native_bytes(Default::default()).unwrap();
    let mut restored = TerminalRelationSession::from_native_bytes(
        &durable,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut reference = session();
    serial_finish(&mut reference);
    parallel_finish(&mut restored, &pool);
    assert_eq!(
        restored.apply_all_terminals().unwrap(),
        reference.apply_all_terminals().unwrap()
    );
    assert_eq!(restored.statistics(), reference.statistics());
}

#[test]
fn cross_block_auxiliary_equations_survive_for_the_global_relation() {
    let c = CoefficientContext::new(["d"]);
    let s = session();
    let x = key([3, 0, 0]);
    let a = key([1, 1, 1]);
    let b = key([2, 1, 1]);
    let columns = vec![x.clone(), a.clone(), b.clone()];
    let offsets = columns
        .iter()
        .enumerate()
        .map(|(i, k)| (k.clone(), i as u32))
        .collect();
    let rows: Vec<_> = [a.clone(), b.clone()]
        .into_iter()
        .map(|m| PreparedRow {
            original: BTreeMap::from([(x.clone(), c.one()), (m, c.one())]),
            conditions: vec![],
            support: BTreeSet::new(),
        })
        .collect();
    let blocks: Vec<_> = rows
        .iter()
        .map(|row| {
            reduce_block(
                &s,
                std::slice::from_ref(row),
                &columns,
                &offsets,
                columns.len(),
                &AtomicBool::new(false),
            )
            .unwrap()
        })
        .collect();
    assert!(blocks.iter().all(|block| block[0].contains_key(&x)));
    let mut merged = SparseRowReducer::new(3, RationalPolynomialField::new(Z), LuLMode::None);
    for block in blocks {
        let mut terms: Vec<_> = block[0]
            .iter()
            .map(|(k, c)| (offsets[k], c.clone()))
            .collect();
        terms.sort_by_key(|(i, _)| *i);
        let (ids, values): (Vec<_>, Vec<_>) = terms.into_iter().unzip();
        merged.add_row(&values, &ids);
    }
    assert_eq!(merged.u().nrows(), 2);
    let u = merged.u();
    let range = u.row_ptrs()[1]..u.row_ptrs()[2];
    assert_eq!(&u.col_idcs()[range.clone()], &[1, 2]);
    assert_eq!(&u.values()[range], &[c.one(), c.integer(-1)]);
}

#[test]
fn batch_and_worker_sizes_are_operational_and_invalid_zero_is_rejected() {
    for options in [
        TerminalRelationExecutionOptions {
            workers: 0,
            batch_rows: 1,
            block_rows: 1,
        },
        TerminalRelationExecutionOptions {
            workers: 1,
            batch_rows: 0,
            block_rows: 1,
        },
        TerminalRelationExecutionOptions {
            workers: 1,
            batch_rows: 1,
            block_rows: 0,
        },
    ] {
        assert!(TerminalRelationExecutor::new(options).is_err());
    }
}
