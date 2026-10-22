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
    let column_keys = vec![x.clone(), a.clone(), b.clone()];
    let offsets: BTreeMap<_, _> = column_keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.clone(), i as u32))
        .collect();
    let columns = ColumnPlan {
        old: &[],
        old_offsets: &BTreeMap::new(),
        new: column_keys,
        new_offsets: offsets.clone(),
    };
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

fn finite_provider() -> TerminalRelationFiniteProvider {
    let context = CoefficientContext::new(["d"]);
    TerminalRelationFiniteProvider::new(Arc::new(BTreeMap::from([
        (
            key([2, 1, 1]),
            vec![TerminalEquation {
                terms: BTreeMap::from([
                    (key([2, 1, 1]), context.one()),
                    (key([1, 2, 1]), context.integer(-1)),
                ]),
                nonzero_conditions: vec![context.parameter("d").unwrap()],
            }],
        ),
        // An explicit finite empty entry is legitimate too. Absent keys are
        // also empty, but never need their own durable query-ledger entry.
        (key([3, 1, 1]), vec![]),
    ])))
}

#[test]
fn finite_provider_parallel_reuses_blocks_without_auxiliary_query_growth() {
    let provider = finite_provider();
    let cancel = AtomicBool::new(false);
    let mut reference = session();
    reference.enable_assistance("finite-map-v1".into()).unwrap();
    while !reference.is_complete() {
        reference
            .step_with_finite_provider(&cancel, &provider)
            .unwrap();
    }
    assert!(reference.statistics().auxiliary_columns > 0);
    assert!(reference.statistics().completed_assistance_keys <= provider.rows().len());
    assert_eq!(reference.statistics().completed_assistance_rows, 1);

    for (workers, batch, block) in [(2, 23, 4), (4, 64, 8), (4, 128, 1)] {
        let mut parallel = session();
        parallel.enable_assistance("finite-map-v1".into()).unwrap();
        let executor = executor(workers, batch, block);
        let mut observed_parallel = false;
        let mut filtered = 0;
        for ordinal in 0.. {
            if parallel.is_complete() {
                break;
            }
            executor
                .step_with_finite_provider_observed(&mut parallel, &cancel, &provider, |progress| {
                    observed_parallel |= progress.blocks_completed > 1;
                    filtered += progress.provider_keys_filtered;
                })
                .unwrap();
            if ordinal == 7 {
                // Operational binding is deliberately absent from bytes. Cold
                // restoration revalidates the complete map and resumes the
                // same durable ordinary/provider cursors with any worker count.
                parallel = TerminalRelationSession::from_native_bytes(
                    &parallel.to_native_bytes(Default::default()).unwrap(),
                    Default::default(),
                    Default::default(),
                )
                .unwrap();
            }
        }
        assert!(filtered > 0);
        assert!(observed_parallel, "ordinary exact blocks were bypassed");
        assert_eq!(parallel.statistics(), reference.statistics());
        assert_eq!(
            parallel.nonzero_conditions(),
            reference.nonzero_conditions()
        );
        assert_eq!(
            parallel.to_native_bytes(Default::default()).unwrap(),
            reference.to_native_bytes(Default::default()).unwrap()
        );
    }
}

#[test]
fn dynamic_provider_keeps_serial_request_transcript_and_exact_prefix() {
    let provider = finite_provider();
    let cancel = AtomicBool::new(false);
    let mut reference = session();
    reference
        .enable_assistance("dynamic-map-v1".into())
        .unwrap();
    let mut serial_requests = Vec::new();
    while !reference.is_complete() {
        reference
            .step_with_provider(&cancel, |key| {
                serial_requests.push(key.clone());
                Ok(provider.rows().get(key).cloned().unwrap_or_default())
            })
            .unwrap();
    }
    let mut parallel = session();
    parallel.enable_assistance("dynamic-map-v1".into()).unwrap();
    let executor = executor(4, 64, 4);
    let mut parallel_requests = Vec::new();
    let caller = std::thread::current().id();
    let mut observed_provider = false;
    while !parallel.is_complete() {
        executor
            .step_with_provider_observed(
                &mut parallel,
                &cancel,
                |key| {
                    assert_eq!(std::thread::current().id(), caller);
                    parallel_requests.push(key.clone());
                    Ok(provider.rows().get(key).cloned().unwrap_or_default())
                },
                |progress| observed_provider |= progress.stage == "provider request",
            )
            .unwrap();
    }
    assert!(observed_provider);
    assert_eq!(parallel_requests, serial_requests);
    assert_eq!(parallel.statistics(), reference.statistics());
    assert_eq!(
        parallel.nonzero_conditions(),
        reference.nonzero_conditions()
    );
    assert_eq!(
        parallel.to_native_bytes(Default::default()).unwrap(),
        reference.to_native_bytes(Default::default()).unwrap()
    );
}

#[test]
fn finite_binding_keeps_old_checkpoint_rows_and_guards_but_drops_empty_pending() {
    let provider = finite_provider();
    let cancel = AtomicBool::new(false);
    let mut original = session();
    original.enable_assistance("old-serial-map".into()).unwrap();
    // Stop immediately after querying the nonempty equation, before inserting
    // it. Resume must not filter or silently discard already-queued equations.
    while original.statistics().pending_assistance_rows == 0 {
        original
            .step_with_provider(&cancel, |key| {
                Ok(provider.rows().get(key).cloned().unwrap_or_default())
            })
            .unwrap();
    }
    let bytes = original.to_native_bytes(Default::default()).unwrap();
    let mut resumed =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();
    let executor = executor(4, 31, 4);
    while !resumed.is_complete() {
        executor
            .step_with_finite_provider(&mut resumed, &cancel, &provider)
            .unwrap();
    }
    while !original.is_complete() {
        original
            .step_with_provider(&cancel, |key| {
                Ok(provider.rows().get(key).cloned().unwrap_or_default())
            })
            .unwrap();
    }
    assert_eq!(
        resumed.apply_all_terminals().unwrap(),
        original.apply_all_terminals().unwrap()
    );
    assert_eq!(resumed.nonzero_conditions(), original.nonzero_conditions());
    assert_eq!(resumed.statistics().completed_assistance_rows, 1);
    assert!(
        resumed.statistics().completed_assistance_keys
            < original.statistics().completed_assistance_keys
    );
    assert_eq!(
        resumed.statistics().completed_source_rows,
        original.statistics().completed_source_rows
    );
}

#[test]
fn finite_provider_authority_is_immutable_and_cancelled_binding_does_not_mutate() {
    let mut session = session();
    session.enable_assistance("immutable-map".into()).unwrap();
    let provider = finite_provider();
    let executor = executor(4, 64, 8);
    let before = session.to_native_bytes(Default::default()).unwrap();
    executor
        .step_with_finite_provider(&mut session, &AtomicBool::new(true), &provider)
        .unwrap();
    assert_eq!(session.to_native_bytes(Default::default()).unwrap(), before);
    executor
        .step_with_finite_provider(&mut session, &AtomicBool::new(false), &provider)
        .unwrap();
    let changed = TerminalRelationFiniteProvider::new(Arc::new(BTreeMap::new()));
    assert!(
        executor
            .step_with_finite_provider(&mut session, &AtomicBool::new(false), &changed)
            .is_err()
    );
    assert!(
        executor
            .step_with_provider(&mut session, &AtomicBool::new(false), |_| Ok(vec![]))
            .is_err()
    );
    assert!(
        session
            .step_with_provider(&AtomicBool::new(false), |_| Ok(vec![]))
            .is_err()
    );
}

#[test]
fn column_plan_borrows_history_and_only_owns_new_prefix() {
    let session = session();
    let context = session.family.coefficient_context();
    let old = session.columns[0].clone();
    let new = key([5, 5, 5]);
    let rows = vec![PreparedRow {
        original: BTreeMap::from([(old.clone(), context.one()), (new.clone(), context.one())]),
        conditions: vec![],
        support: BTreeSet::new(),
    }];
    let plan = ColumnPlan::new(&session, &rows).unwrap();
    assert!(std::ptr::eq(plan.old, session.columns.as_slice()));
    assert!(std::ptr::eq(plan.old_offsets, &session.column_offsets));
    assert_eq!(plan.new, vec![new.clone()]);
    assert_eq!(plan.key(plan.index(&old)), &old);
    assert_eq!(plan.key(plan.index(&new)), &new);
}

#[test]
fn dynamic_provider_error_and_callback_cancellation_leave_durable_barriers() {
    let mut session = session();
    session
        .enable_assistance("fallible-dynamic-map".into())
        .unwrap();
    let executor = executor(4, 64, 4);
    let before = session.to_native_bytes(Default::default()).unwrap();
    let mut attempts = 0;
    assert!(
        executor
            .step_with_provider(&mut session, &AtomicBool::new(false), |_| {
                attempts += 1;
                Err(invalid("deliberate provider failure"))
            })
            .is_err()
    );
    assert_eq!(attempts, 1);
    assert_eq!(session.to_native_bytes(Default::default()).unwrap(), before);

    let cancel = AtomicBool::new(false);
    let context = session.family.coefficient_context();
    let equation = TerminalEquation {
        terms: BTreeMap::from([
            (key([2, 1, 1]), context.one()),
            (key([1, 2, 1]), context.integer(-1)),
        ]),
        nonzero_conditions: vec![context.parameter("d").unwrap()],
    };
    executor
        .step_with_provider(&mut session, &cancel, |_| {
            cancel.store(true, Ordering::Relaxed);
            Ok(vec![equation.clone()])
        })
        .unwrap();
    assert_eq!(session.statistics().pending_assistance_rows, 1);
    assert_eq!(session.statistics().completed_assistance_rows, 0);
    let queued = session.to_native_bytes(Default::default()).unwrap();
    executor
        .step_with_provider(&mut session, &cancel, |_| {
            panic!("cancelled execution must not query the provider")
        })
        .unwrap();
    assert_eq!(session.to_native_bytes(Default::default()).unwrap(), queued);
    let mut resumed =
        TerminalRelationSession::from_native_bytes(&queued, Default::default(), Default::default())
            .unwrap();
    executor
        .step_with_provider(&mut resumed, &AtomicBool::new(false), |_| {
            panic!("the queued equation must drain before another request")
        })
        .unwrap();
    assert_eq!(resumed.statistics().completed_assistance_rows, 1);
    assert!(
        resumed
            .nonzero_conditions()
            .contains(&equation.nonzero_conditions[0])
    );
}

fn first_prepared_rows(session: &TerminalRelationSession, count: usize) -> Vec<PreparedRow> {
    (0..count)
        .map(|ordinal| {
            let (row, conditions) = session
                .sources
                .row(
                    &session.seeds[ordinal / session.sources.len()],
                    ordinal % session.sources.len(),
                    session.limits.algebra,
                )
                .unwrap();
            let original = session.normalized_row(&row).unwrap();
            let support = session
                .assistance_support(row.keys().chain(original.keys()))
                .unwrap();
            PreparedRow {
                original,
                conditions,
                support,
            }
        })
        .collect()
}

fn old_ordered_commit(session: &mut TerminalRelationSession, rows: &[PreparedRow]) {
    for row in rows {
        session.ensure_columns(&row.original).unwrap();
        session.add_row(row.original.clone()).unwrap();
        if let Some(assistance) = &mut session.assistance {
            assistance.pending.extend(row.support.iter().cloned());
        }
        for condition in &row.conditions {
            if !session.conditions.contains(condition) {
                session.conditions.push(condition.clone());
            }
        }
        session.source_cursor += 1;
        if session.source_cursor == session.sources.len() {
            session.source_cursor = 0;
            session.seed_cursor += 1;
        }
    }
}

#[test]
fn bulk_column_commit_matches_serial_bytes_maps_and_ordered_conditions() {
    let mut legacy = session();
    let mut bulk = session();
    let mut rows = first_prepared_rows(&bulk, 8);
    let context = bulk.family.coefficient_context();
    rows[0].conditions.push(context.parameter("d").unwrap());
    rows[3].conditions.push(context.integer(2));
    rows[7].conditions.push(context.parameter("d").unwrap());
    let residuals: Vec<_> = rows.iter().map(|row| row.original.clone()).collect();
    let plan = BulkBlock::prepare(&bulk, &rows, &residuals, false).expect("nonempty new prefix");
    let matrix = plan.matrix(&bulk, residuals);
    old_ordered_commit(&mut legacy, &rows);
    plan.commit(&mut bulk, &rows, matrix);
    assert_eq!(bulk.statistics(), legacy.statistics());
    assert_eq!(bulk.nonzero_conditions(), legacy.nonzero_conditions());
    assert_eq!(
        bulk.apply_all_terminals().unwrap(),
        legacy.apply_all_terminals().unwrap()
    );
    assert_eq!(
        bulk.to_native_bytes(Default::default()).unwrap(),
        legacy.to_native_bytes(Default::default()).unwrap()
    );
}

#[test]
fn bulk_commits_only_its_own_prefix_and_cold_resumes_after_atomic_boundary() {
    let mut reference = session();
    let mut bulk = session();
    let rows = first_prepared_rows(&bulk, 8);
    let residuals = rows[..4]
        .iter()
        .map(|row| row.original.clone())
        .collect::<Vec<_>>();
    let plan = BulkBlock::prepare(&bulk, &rows[..4], &residuals, false).unwrap();
    let matrix = plan.matrix(&bulk, residuals);
    // A cooperative stop arriving after this indivisible operation begins
    // cannot expose future registered columns with an older source cursor.
    let cancel = AtomicBool::new(true);
    plan.commit(&mut bulk, &rows[..4], matrix);
    old_ordered_commit(&mut reference, &rows[..4]);
    let checkpoint = bulk.to_native_bytes(Default::default()).unwrap();
    assert_eq!(
        checkpoint,
        reference.to_native_bytes(Default::default()).unwrap()
    );
    executor(4, 64, 4).step(&mut bulk, &cancel).unwrap();
    assert_eq!(
        bulk.to_native_bytes(Default::default()).unwrap(),
        checkpoint
    );
    let mut restored = TerminalRelationSession::from_native_bytes(
        &checkpoint,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    parallel_finish(&mut restored, &executor(4, 64, 4));
    serial_finish(&mut reference);
    assert_eq!(
        restored.apply_all_terminals().unwrap(),
        reference.apply_all_terminals().unwrap()
    );
    assert_eq!(
        restored.nonzero_conditions(),
        reference.nonzero_conditions()
    );
}

#[test]
fn bulk_conservative_allowance_miss_falls_back_without_rejecting_valid_rows() {
    let mut constrained = session();
    let mut reference = session();
    let rows = first_prepared_rows(&constrained, 8);
    old_ordered_commit(&mut reference, &rows);
    let residuals = rows
        .iter()
        .map(|row| row.original.clone())
        .collect::<Vec<_>>();
    let width = ColumnPlan::new(&constrained, &rows).unwrap().new.len() + constrained.columns.len();
    constrained.limits.max_nonzeros = reference.reducer.u().nvalues() + width;
    assert!(constrained.limits.max_nonzeros < rows.len() * width);
    assert!(BulkBlock::prepare(&constrained, &rows, &residuals, false).is_none());
    // The operational bulk bound is deliberately conservative; normal exact
    // row-by-row admission remains available under the user's actual budget.
    old_ordered_commit(&mut constrained, &rows);
    assert_eq!(constrained.statistics(), reference.statistics());
    assert_eq!(
        constrained.apply_all_terminals().unwrap(),
        reference.apply_all_terminals().unwrap()
    );
}

#[test]
fn bulk_never_crosses_provider_barrier_and_reuse_only_needs_no_atomic_commit() {
    let mut source = session();
    source.enable_assistance("barrier-boundary".into()).unwrap();
    source.assistance.as_mut().unwrap().pending.clear();
    // These constructed provider supports exercise the commit contract rather
    // than asking any callback speculatively.
    let mut rows = first_prepared_rows(&source, 4);
    rows.iter_mut().for_each(|row| row.support.clear());
    rows[0].support.insert(key([7, 1, 1]));
    let residuals = rows
        .iter()
        .map(|row| row.original.clone())
        .collect::<Vec<_>>();
    assert!(BulkBlock::prepare(&source, &rows, &residuals, false).is_none());
    rows[0].support.clear();
    rows[3].support.insert(key([7, 1, 1]));
    assert!(BulkBlock::prepare(&source, &rows, &residuals, false).is_some());
    rows[3].support.clear();
    old_ordered_commit(&mut source, &rows);
    // Existing physical columns require no expensive U reindex, so this path
    // retains the old row-level cooperative cancellation responsiveness.
    assert!(BulkBlock::prepare(&source, &rows, &residuals, false).is_none());
}
