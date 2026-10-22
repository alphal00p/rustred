use super::*;

#[test]
fn guard_batch_merge_matches_first_occurrence_order_with_large_existing_prefix() {
    let context = CoefficientContext::new(["d"]);
    let mut existing: Vec<_> = (1..=4096).map(|n| context.integer(n)).collect();
    // Preserve even a preexisting duplicate, as the previous linear merge did.
    existing.push(existing[17].clone());
    let incoming = vec![
        existing[2047].clone(),
        context.integer(5003),
        existing[0].clone(),
        context.integer(5001),
        context.integer(5003),
        existing[4095].clone(),
        context.integer(5002),
        context.integer(5001),
    ];
    let mut expected = existing.clone();
    for condition in &incoming {
        if !expected.contains(condition) {
            expected.push(condition.clone());
        }
    }
    let original_len = existing.len();

    TerminalRelationSession::merge_nonzero_conditions(&mut existing, &incoming);

    assert_eq!(existing, expected);
    assert_eq!(
        &existing[original_len..],
        &[
            context.integer(5003),
            context.integer(5001),
            context.integer(5002),
        ]
    );
    assert_eq!(incoming[1], incoming[4]);
}

#[test]
fn empty_and_repeated_guard_batches_preserve_existing_buffer_and_order() {
    let context = CoefficientContext::new(["d"]);
    let mut existing = Vec::with_capacity(64);
    existing.extend([
        context.integer(7),
        context.parameter("d").unwrap(),
        context.integer(3),
    ]);
    let expected = existing.clone();
    let capacity = existing.capacity();
    let buffer = existing.as_ptr();

    TerminalRelationSession::merge_nonzero_conditions(&mut existing, &[]);
    assert_eq!(existing, expected);
    assert_eq!(existing.capacity(), capacity);
    assert_eq!(existing.as_ptr(), buffer);

    let repeated = vec![
        expected[2].clone(),
        expected[0].clone(),
        expected[2].clone(),
    ];
    TerminalRelationSession::merge_nonzero_conditions(&mut existing, &repeated);
    assert_eq!(existing, expected);
    assert_eq!(existing.capacity(), capacity);
    assert_eq!(existing.as_ptr(), buffer);
}

#[test]
fn cancelled_queued_guard_batch_preserves_checkpoint_until_resume() {
    let family = tadpole();
    let context = family.coefficient_context();
    let existing = vec![context.parameter("d").unwrap(), context.integer(2)];
    let incoming = vec![
        existing[0].clone(),
        context.integer(3),
        context.integer(3),
        existing[1].clone(),
        context.integer(4),
    ];
    let expected = vec![
        existing[0].clone(),
        existing[1].clone(),
        context.integer(3),
        context.integer(4),
    ];
    let mut session = assisted_tadpole();
    session.conditions = existing.clone();
    session
        .step_with_provider(&AtomicBool::new(false), |_| {
            Ok(vec![TerminalEquation {
                terms: BTreeMap::new(),
                nonzero_conditions: incoming.clone(),
            }])
        })
        .unwrap();
    assert_eq!(session.statistics().pending_assistance_rows, 1);
    assert_eq!(session.nonzero_conditions(), existing);
    let before = session.statistics();
    let bytes = session.to_native_bytes(Default::default()).unwrap();

    session
        .step_with_provider(&AtomicBool::new(true), |_| {
            panic!("cancelled queued row must not query provider")
        })
        .unwrap();
    assert_eq!(session.statistics(), before);
    assert_eq!(session.nonzero_conditions(), existing);
    assert_eq!(session.to_native_bytes(Default::default()).unwrap(), bytes);

    let mut resumed =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();
    resumed
        .step_with_provider(&AtomicBool::new(false), |_| {
            panic!("saved queued row must not query provider")
        })
        .unwrap();
    assert_eq!(resumed.nonzero_conditions(), expected);
    let after = resumed.statistics();
    assert_eq!(
        after.completed_assistance_rows,
        before.completed_assistance_rows + 1
    );
    assert_eq!(after.pending_assistance_rows, 0);
    assert_eq!(after.completed_source_rows, before.completed_source_rows);
    assert_eq!(
        after.completed_assistance_keys,
        before.completed_assistance_keys
    );
}
