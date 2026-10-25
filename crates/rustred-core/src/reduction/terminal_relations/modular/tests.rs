//! Independent matrix and persistence checks for modular terminal circuits.

use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;

fn tadpole() -> Arc<IntegralFamily> {
    let c = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            "modular-circuit-audit",
            vec!["q".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            vec![AffineDenominator::new(c.integer(-1), vec![c.one()])],
            vec![],
            vec![c.zero()],
        )
        .unwrap(),
    )
}

fn key(n: i64) -> IntegralKey {
    IntegralKey::try_new([n]).unwrap()
}

fn options() -> TerminalRelationModularOptions {
    let mut options = TerminalRelationModularOptions::default();
    options.batch_rows = 1;
    // These are explicit test-call budgets, never production defaults. In
    // particular, intentionally inconsistent proposals must return finitely.
    options.discovery_samples = 4;
    options.reconstruction.arithmetic = ExactAlgebraLimits::default();
    options.reconstruction.max_rows = 1_000_000;
    options.reconstruction.max_columns = 1_000_000;
    options.reconstruction.max_input_nonzeros = 16_000_000;
    options.reconstruction.max_coefficient_terms = 16_000_000;
    options.reconstruction.max_degree = 8;
    options.reconstruction.max_probes = 512;
    options.reconstruction.max_attempts = 8;
    options.reconstruction.max_primes = 16;
    options.reconstruction.max_cached_images = 256;
    options.reconstruction.max_cached_values = 16_000_000;
    options.reconstruction.max_weight_slots = 1_000_000;
    options
}

#[test]
fn default_resource_grants_have_only_native_representation_bounds() {
    let options = TerminalRelationModularOptions::default();
    assert_eq!(options.discovery_samples, usize::MAX);
    let limits = options.reconstruction;
    assert_eq!(limits.arithmetic, ExactAlgebraLimits::unbounded_resources());
    assert_eq!(limits.max_degree, u16::MAX / 2);
    for limit in [
        limits.max_rows,
        limits.max_columns,
        limits.max_input_nonzeros,
        limits.max_coefficient_terms,
        limits.max_probes,
        limits.max_attempts,
        limits.max_primes,
        limits.max_cached_images,
        limits.max_cached_values,
        limits.max_weight_slots,
    ] {
        assert_eq!(limit, usize::MAX);
    }
}

fn new_session() -> TerminalRelationSession {
    TerminalRelationSession::new(
        tadpole(),
        BTreeSet::from([key(1), key(2)]),
        0,
        Default::default(),
    )
    .unwrap()
}

fn row(session: &TerminalRelationSession, terms: &[(i64, &str)]) -> TerminalRelationRow {
    terms
        .iter()
        .map(|(n, value)| {
            (
                key(*n),
                session
                    .family
                    .coefficient_context()
                    .coefficient_fixture(value),
            )
        })
        .collect()
}

fn add_fixture(
    modular: &mut TerminalRelationModularSession,
    terms: &[(i64, &str)],
    guards: &[&str],
) {
    let terms = row(&modular.session, terms);
    let conditions = guards
        .iter()
        .map(|guard| {
            modular
                .session
                .family
                .coefficient_context()
                .coefficient_fixture(guard)
        })
        .collect();
    modular.rows.push(SourceRow {
        // Matrix fixtures deliberately use no persistence authority. Codec
        // controls below exclusively use authentic generated ordinary rows.
        identity: TerminalRelationModularSource::Ordinary {
            seed_ordinal: modular.rows.len(),
            source_ordinal: 0,
        },
        terms,
        conditions,
    });
    assert!(modular.discover_last().unwrap().is_some());
}

fn accepted_tadpole() -> TerminalRelationModularSession {
    let mut modular = TerminalRelationModularSession::new(new_session(), options()).unwrap();
    for _ in 0..10 {
        modular.step(&AtomicBool::new(false)).unwrap();
        if !modular.circuits.is_empty() {
            return modular;
        }
    }
    panic!("authentic finite tadpole cohort produced no terminal circuit");
}

fn assert_bad_checkpoint(modular: &TerminalRelationModularSession) {
    if let Ok(bytes) = modular.to_native_bytes(Default::default()) {
        assert!(
            TerminalRelationModularSession::from_native_bytes(
                &bytes,
                Default::default(),
                Default::default(),
            )
            .is_err(),
            "a mutated circuit or discovery record acquired authority on reload"
        );
    }
}

#[test]
fn authentic_tadpole_circuit_matches_exact_without_consuming_its_cursor() {
    let mut reference = new_session();
    while !reference.is_complete() {
        reference.step(&AtomicBool::new(false)).unwrap();
    }
    let mut modular = accepted_tadpole();
    for _ in 0..10 {
        if modular.is_search_complete() {
            break;
        }
        modular.step(&AtomicBool::new(false)).unwrap();
    }
    assert_eq!(
        modular.session.statistics().completed_source_rows,
        0,
        "sampled scans must not change exact finite-search progress"
    );
    assert_eq!(modular.statistics().accepted_relations, 1);
    assert_eq!(
        modular.session.apply_all_terminals().unwrap(),
        reference.apply_all_terminals().unwrap()
    );
    assert!(!modular.session.is_complete());
    assert!(modular.statistics().search_exhausted);
}

#[test]
fn dependent_empty_and_nonmonotone_inputs_keep_original_source_positions() {
    let mut modular = TerminalRelationModularSession::new(new_session(), options()).unwrap();
    // aux8 arrives first, then the harder aux9. A dependent row grows native L
    // without growing U; an empty row grows neither. Neither changes source IDs.
    add_fixture(&mut modular, &[(8, "d")], &["d-7"]);
    add_fixture(&mut modular, &[(8, "2*d")], &[]);
    add_fixture(&mut modular, &[], &[]);
    add_fixture(&mut modular, &[(9, "d"), (2, "1")], &[]);
    add_fixture(&mut modular, &[(9, "d"), (1, "1")], &[]);
    assert_eq!(modular.candidates.len(), 1);
    assert_eq!(modular.candidates[0].sources, vec![3, 4]);
    assert_eq!(modular.image.accepted_sources, vec![0, 3, 4]);
    assert!(
        modular
            .image
            .parents
            .iter()
            .enumerate()
            .all(|(row, parents)| { parents.iter().all(|&parent| parent < row) })
    );
    let circuit = modular.lift(0, &mut |_| {}).unwrap();
    assert_eq!(circuit.image, row(&modular.session, &[(2, "1"), (1, "-1")]));
    assert_eq!(circuit.sources.len(), 2);
    assert!(
        circuit.nonzero_conditions.contains(
            &modular
                .session
                .family
                .coefficient_context()
                .coefficient_fixture("d-7")
        )
    );
    // The auxiliary computational pivot cancels; it must not add a false
    // permanent d != 0 restriction to this particular exact source circuit.
    assert!(
        !circuit.nonzero_conditions.contains(
            &modular
                .session
                .family
                .coefficient_context()
                .parameter("d")
                .unwrap()
        )
    );
}

#[test]
fn lift_column_labels_preserve_frozen_order_without_changing_physical_source_identities() {
    let baseline = TerminalRelationSession::new(
        tadpole(),
        BTreeSet::from([key(1), key(2), key(3)]),
        0,
        Default::default(),
    )
    .unwrap();
    let mut modular = TerminalRelationModularSession::new(baseline, options()).unwrap();
    add_fixture(&mut modular, &[(9, "d"), (8, "1")], &[]);
    add_fixture(&mut modular, &[(9, "2*d"), (8, "3"), (1, "1")], &[]);
    add_fixture(
        &mut modular,
        &[(9, "3*d"), (8, "4"), (2, "1"), (1, "3")],
        &[],
    );
    assert!(key(8) < key(9));
    let frozen_position = |wanted: &IntegralKey| {
        modular
            .image
            .columns
            .iter()
            .position(|key| key == wanted)
            .unwrap()
    };
    assert!(frozen_position(&key(9)) < frozen_position(&key(8)));
    assert_eq!(modular.candidates.len(), 1);
    assert_eq!(modular.candidates[0].sources, [0, 1, 2]);

    let selected: Vec<_> = modular.candidates[0]
        .sources
        .iter()
        .map(|&ordinal| &modular.rows[ordinal])
        .collect();
    let labels = labels::ColumnLabels::new(
        &modular.session,
        &modular.image,
        &selected,
        &BTreeSet::from([key(3)]),
        &key(2),
    )
    .unwrap();
    assert!(labels.label(&key(9)).unwrap() < labels.label(&key(8)).unwrap());
    // I3 is a harder terminal absent from every selected source. Its exact
    // zero column is still a forbidden obligation, not an omitted domain.
    assert!(
        selected
            .iter()
            .all(|source| !source.terms.contains_key(&key(3)))
    );
    assert!(labels.label(&key(3)).unwrap() < labels.label(&key(2)).unwrap());
    assert_eq!(
        labels.physical(labels.label(&key(3)).unwrap()).unwrap(),
        &key(3)
    );
    for physical in selected.iter().flat_map(|source| source.terms.keys()) {
        assert_eq!(
            labels.physical(labels.label(physical).unwrap()).unwrap(),
            physical
        );
    }

    let circuit = modular.lift(0, &mut |_| {}).unwrap();
    let context = modular.session.family.coefficient_context();
    assert_eq!(
        circuit.weights,
        BTreeMap::from([
            (0, context.integer(-1)),
            (1, context.integer(-1)),
            (2, context.one()),
        ])
    );
    assert_eq!(circuit.image, row(&modular.session, &[(2, "1"), (1, "2")]));
    assert_eq!(
        circuit.sources,
        modular.candidates[0]
            .sources
            .iter()
            .map(|&ordinal| modular.rows[ordinal].identity.clone())
            .collect::<Vec<_>>()
    );
    // The public certificate remains in physical IntegralKeys, not temporary
    // ordinal labels. Its independent full-column exact product also checks
    // that the inverse transport did not exchange auxiliary/terminal columns.
    validation::validate_circuit(&modular.session, &selected, &[], &circuit).unwrap();
    assert!(modular.session.terminal_rules().is_empty());
}

#[test]
fn exact_validation_rejects_auxiliary_outside_the_discovery_support() {
    let mut modular = TerminalRelationModularSession::new(new_session(), options()).unwrap();
    add_fixture(&mut modular, &[(9, "d"), (2, "1")], &[]);
    add_fixture(&mut modular, &[(9, "d"), (1, "1")], &[]);
    assert!(modular.lift(0, &mut |_| {}).is_ok());
    // Mutate only the exact source after discovery. The new auxiliary was not
    // present in the sampled row/support at all, and cannot be omitted on lift.
    modular.rows[1].terms.insert(
        key(10),
        modular
            .session
            .family
            .coefficient_context()
            .coefficient_fixture("d-5"),
    );
    assert!(!modular.image.columns.contains(&key(10)));
    let selected: Vec<_> = modular.candidates[0]
        .sources
        .iter()
        .map(|&ordinal| &modular.rows[ordinal])
        .collect();
    let labels = labels::ColumnLabels::new(
        &modular.session,
        &modular.image,
        &selected,
        &BTreeSet::new(),
        &modular.candidates[0].target,
    )
    .unwrap();
    assert!(labels.label(&key(10)).unwrap() < labels.label(&modular.candidates[0].target).unwrap());
    assert_eq!(
        labels.physical(labels.label(&key(10)).unwrap()).unwrap(),
        &key(10)
    );
    assert!(modular.lift(0, &mut |_| {}).is_err());
    assert!(modular.session.terminal_rules().is_empty());
}

#[test]
fn a_sampled_zero_auxiliary_can_only_propose_and_never_publish_a_relation() {
    let mut modular = TerminalRelationModularSession::new(new_session(), options()).unwrap();
    let hidden = format!("d-{}", modular.image.point[0]);
    add_fixture(&mut modular, &[(9, "d"), (2, "1")], &[]);
    add_fixture(&mut modular, &[(10, &hidden), (9, "d"), (1, "1")], &[]);
    assert_eq!(modular.candidates.len(), 1);
    assert_eq!(modular.candidates[0].target, key(2));
    assert!(modular.lift(0, &mut |_| {}).is_err());
    assert!(modular.circuits.is_empty());
    assert!(modular.session.terminal_rules().is_empty());
}

#[test]
fn default_unbounded_policy_refuses_a_false_hit_by_a_generic_rank_witness() {
    let mut modular = TerminalRelationModularSession::new(
        new_session(),
        TerminalRelationModularOptions::default(),
    )
    .unwrap();
    let hidden = format!("d-{}", modular.image.point[0]);
    add_fixture(&mut modular, &[(9, "d"), (2, "1")], &[]);
    add_fixture(&mut modular, &[(10, &hidden), (9, "d"), (1, "1")], &[]);
    assert_eq!(modular.candidates.len(), 1);
    // The sampled hidden coefficient was zero, but at a generic fresh point
    // both source rows have independent forbidden columns. A nonzero minor
    // then refuses this exact frame without consuming an artificial retry cap.
    let mut events = Vec::new();
    assert!(modular.lift(0, &mut |event| events.push(event)).is_err());
    assert!(matches!(
        events.as_slice(),
        [ProjectedSourceWeightEvent::FramePrepared { .. }]
    ));
    assert!(modular.circuits.is_empty());
    assert!(modular.session.terminal_rules().is_empty());
    assert_eq!(modular.session.statistics().completed_source_rows, 0);
}

#[test]
fn default_unbounded_policy_still_accepts_an_authentic_exact_circuit() {
    let mut modular = TerminalRelationModularSession::new(
        new_session(),
        TerminalRelationModularOptions::default(),
    )
    .unwrap();
    modular.step(&AtomicBool::new(false)).unwrap();
    assert_eq!(modular.circuits.len(), 1);
    assert_eq!(modular.statistics().accepted_relations, 1);
    assert_eq!(modular.session.statistics().completed_source_rows, 0);
    let bytes = modular.to_native_bytes(Default::default()).unwrap();
    let resumed = TerminalRelationModularSession::from_native_bytes(
        &bytes,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        resumed.session.apply_all_terminals().unwrap(),
        modular.session.apply_all_terminals().unwrap()
    );
}

#[test]
fn a_genuine_source_weight_pole_is_retained() {
    let mut modular = TerminalRelationModularSession::new(new_session(), options()).unwrap();
    add_fixture(&mut modular, &[(9, "d")], &[]);
    add_fixture(&mut modular, &[(9, "1"), (2, "1")], &[]);
    let circuit = modular.lift(0, &mut |_| {}).unwrap();
    let c = modular.session.family.coefficient_context();
    assert_eq!(circuit.image, row(&modular.session, &[(2, "1")]));
    assert_eq!(circuit.weights[&0], c.coefficient_fixture("-1/d"));
    assert!(
        circuit
            .nonzero_conditions
            .contains(&c.parameter("d").unwrap())
    );
}

#[test]
fn a_reconstructed_zero_weight_slot_keeps_its_source_guard() {
    let mut modular = TerminalRelationModularSession::new(new_session(), options()).unwrap();
    // Register both auxiliary columns in the first source. Introducing aux8
    // later would prepend it before aux9, so source1 would become a direct
    // pivot and the real ancestry would correctly exclude source0 entirely.
    // Here source0 has the unique harder aux9 pivot; both later sources use
    // it before the aux8 pivot. Their exact difference cancels its weight.
    add_fixture(&mut modular, &[(9, "d"), (8, "d")], &["d-7"]);
    add_fixture(&mut modular, &[(9, "1"), (8, "2")], &[]);
    add_fixture(&mut modular, &[(9, "1"), (8, "2"), (2, "1")], &[]);
    assert_eq!(modular.candidates[0].sources, vec![0, 1, 2]);
    let circuit = modular.lift(0, &mut |_| {}).unwrap();
    assert_eq!(circuit.sources.len(), 3);
    assert!(!circuit.weights.contains_key(&0));
    let c = modular.session.family.coefficient_context();
    assert_eq!(circuit.weights[&1], c.integer(-1));
    assert_eq!(circuit.weights[&2], c.one());
    assert_eq!(circuit.image, row(&modular.session, &[(2, "1")]));
    assert!(
        circuit.nonzero_conditions.contains(
            &modular
                .session
                .family
                .coefficient_context()
                .coefficient_fixture("d-7")
        )
    );
}

#[test]
fn cancellation_and_native_roundtrip_preserve_the_accepted_exact_maps() {
    let modular = accepted_tadpole();
    let maps = modular.session.apply_all_terminals().unwrap();
    let bytes = modular.to_native_bytes(Default::default()).unwrap();
    let mut resumed = TerminalRelationModularSession::from_native_bytes(
        &bytes,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let before = resumed.statistics();
    resumed.step(&AtomicBool::new(true)).unwrap();
    assert_eq!(resumed.statistics(), before);
    assert_eq!(resumed.session.apply_all_terminals().unwrap(), maps);
    assert_eq!(resumed.session.statistics().completed_source_rows, 0);
    assert_eq!(resumed.circuits.len(), 1);
    assert_eq!(resumed.circuits[0].weights, modular.circuits[0].weights);
    for _ in 0..10 {
        if resumed.is_search_complete() {
            break;
        }
        resumed.step(&AtomicBool::new(false)).unwrap();
    }
    assert_eq!(resumed.session.apply_all_terminals().unwrap(), maps);
}

#[test]
fn reloading_revalidates_weights_image_and_source_identity() {
    let mut bad_weight = accepted_tadpole();
    let one = bad_weight.session.family.coefficient_context().one();
    bad_weight.circuits[0].weights.insert(0, one.clone());
    assert_bad_checkpoint(&bad_weight);

    let mut bad_image = accepted_tadpole();
    bad_image.circuits[0].image.insert(key(1), one.clone());
    assert_bad_checkpoint(&bad_image);

    let mut foreign_terminal = accepted_tadpole();
    foreign_terminal.circuits[0].image.insert(key(99), one);
    assert_bad_checkpoint(&foreign_terminal);

    let mut bad_source = accepted_tadpole();
    bad_source.circuits[0].sources[0] = TerminalRelationModularSource::Ordinary {
        seed_ordinal: usize::MAX,
        source_ordinal: 0,
    };
    assert_bad_checkpoint(&bad_source);
}

#[test]
fn an_accepted_proposal_cannot_rebind_its_proved_circuit_to_other_scanned_sources() {
    let mut modular = accepted_tadpole();
    while modular.rows.len() < 2 {
        modular.step(&AtomicBool::new(false)).unwrap();
    }
    let accepted = modular
        .candidates
        .iter()
        .position(|candidate| {
            matches!(
                candidate.status,
                TerminalRelationModularProposalStatus::Accepted { circuit: 0 }
            )
        })
        .unwrap();
    assert_eq!(modular.candidates[accepted].sources, [0]);
    assert_eq!(modular.circuits[0].scanned_sources, 1);
    assert!(modular.rows.len() >= 2);
    // Source 1 is a valid global scanned ID, but it was not the frame used by
    // this accepted circuit. Its exact certificate must not authenticate a
    // different proposal's dependency metadata merely because targets agree.
    modular.candidates[accepted].sources[0] = 1;
    assert_bad_checkpoint(&modular);
}

#[test]
fn inherited_maps_survive_modular_construction_and_resume() {
    let mut exact = new_session();
    exact.step(&AtomicBool::new(false)).unwrap();
    let expected = exact.apply_all_terminals().unwrap();
    let completed = exact.statistics().completed_source_rows;
    assert_eq!(exact.statistics().terminal_relations, 1);
    let modular = TerminalRelationModularSession::new(exact, options()).unwrap();
    assert!(modular.circuits.is_empty());
    assert_eq!(modular.session.apply_all_terminals().unwrap(), expected);
    let bytes = modular.to_native_bytes(Default::default()).unwrap();
    let resumed = TerminalRelationModularSession::from_native_bytes(
        &bytes,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(resumed.session.apply_all_terminals().unwrap(), expected);
    assert_eq!(
        resumed.session.statistics().completed_source_rows,
        completed
    );
}

#[test]
fn a_refused_source_can_be_saved_and_retried_without_a_duplicate_identity() {
    let mut modular = accepted_tadpole();
    modular.set_max_columns(Some(2)).unwrap();
    let before = modular.statistics();
    let sources = modular.rows.len();
    let maps = modular.session.apply_all_terminals().unwrap();
    for _ in 0..2 {
        assert!(modular.step(&AtomicBool::new(false)).is_err());
        assert_eq!(modular.statistics(), before);
        assert_eq!(modular.rows.len(), sources);
        assert_eq!(modular.session.apply_all_terminals().unwrap(), maps);
    }
    let bytes = modular.to_native_bytes(Default::default()).unwrap();
    let mut resumed = TerminalRelationModularSession::from_native_bytes(
        &bytes,
        TerminalRelationLimits {
            diagnostics_enabled: true,
            max_columns: 2,
            ..Default::default()
        },
        Default::default(),
    )
    .unwrap();
    resumed.set_max_columns(Some(3)).unwrap();
    resumed.step(&AtomicBool::new(false)).unwrap();
    assert_eq!(resumed.rows.len(), sources + 1);
    assert_eq!(
        resumed.statistics().source_rows_scanned,
        before.source_rows_scanned + 1
    );
    assert_eq!(resumed.session.apply_all_terminals().unwrap(), maps);
    assert_eq!(resumed.session.statistics().completed_source_rows, 0);
}

#[test]
fn discovery_graph_cycles_and_out_of_range_sources_are_rejected_on_reload() {
    let mut cycle = accepted_tadpole();
    let row = cycle.image.parents.len() - 1;
    cycle.image.parents[row].push(row);
    assert_bad_checkpoint(&cycle);

    let mut absent_source = accepted_tadpole();
    absent_source.image.accepted_sources[0] = absent_source.rows.len();
    assert_bad_checkpoint(&absent_source);
}

#[test]
fn acyclic_ancestry_with_an_easier_parent_pivot_is_rejected_on_reload() {
    let mut baseline = new_session();
    let one = baseline.family.coefficient_context().one();
    baseline
        .add_row(BTreeMap::from([(key(1), one.clone())]))
        .unwrap();
    baseline.add_row(BTreeMap::from([(key(9), one)])).unwrap();
    let mut modular = TerminalRelationModularSession::new(baseline, options()).unwrap();
    assert_eq!(modular.image.parents.len(), 2);
    assert!(modular.image.pivot_columns[0] > modular.image.pivot_columns[1]);
    modular.image.parents[1].push(0);
    assert_bad_checkpoint(&modular);
}

#[test]
fn authentic_multiple_circuits_have_identical_one_and_six_worker_checkpoints() {
    fn run(workers: usize) -> TerminalRelationModularSession {
        let baseline = TerminalRelationSession::new(
            tadpole(),
            BTreeSet::from([key(1), key(2), key(3)]),
            0,
            Default::default(),
        )
        .unwrap();
        let mut policy = options();
        policy.batch_rows = 2;
        let mut modular = TerminalRelationModularSession::new(baseline, policy).unwrap();
        modular.set_workers(workers).unwrap();
        for _ in 0..10 {
            if modular.is_search_complete() {
                break;
            }
            modular.step(&AtomicBool::new(false)).unwrap();
        }
        assert_eq!(modular.circuits.len(), 2);
        assert_eq!(modular.session.statistics().completed_source_rows, 0);
        modular
    }

    let serial = run(1);
    let parallel = run(6);
    assert_eq!(
        serial.session.apply_all_terminals().unwrap(),
        parallel.session.apply_all_terminals().unwrap()
    );
    let serial_bytes = serial.to_native_bytes(Default::default()).unwrap();
    let parallel_bytes = parallel.to_native_bytes(Default::default()).unwrap();
    assert_eq!(serial_bytes, parallel_bytes);
    let resumed = TerminalRelationModularSession::from_native_bytes(
        &parallel_bytes,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(resumed.circuits.len(), 2);
    assert_eq!(
        serial.session.apply_all_terminals().unwrap(),
        resumed.session.apply_all_terminals().unwrap()
    );
}

#[test]
fn reloading_rejects_a_changed_stable_ordinary_row_id() {
    let modular = accepted_tadpole();
    let bytes = modular.to_native_bytes(Default::default()).unwrap();
    let changed = codec::with_changed_ordinary_row_id(&bytes).unwrap();
    let error = TerminalRelationModularSession::from_native_bytes(
        &changed,
        Default::default(),
        Default::default(),
    )
    .err()
    .expect("a changed ordinary RowId must not inherit circuit source authority");
    assert!(error.to_string().contains("RowId"), "{error}");
}

/// Explicit, read-only workload experiment. Root coordinates affinity and
/// external CPU/RSS collection; no campaign path is an output destination.
/// Required environment: RUSTRED_MODULAR_AUDIT_STATE, ..._SOURCE_ROWS.
/// Optional: ..._START_ROW, ..._TERMINAL_ORDINAL, ..._BATCH_ROWS,
/// ..._EXACT_WORKERS, ..._MODULAR_WORKERS, ..._COMPARE_EXACT (0 or 1).
/// ..._EXPORT_SNAPSHOT explicitly writes a fresh file under repository TMP/;
/// ..._EXPORT_ONLY=1 stops after that export, before either solving backend.
#[test]
#[ignore = "explicit immutable-state positive replay benchmark; coordinate resources before running"]
fn immutable_known_terminal_relation_full_pipeline_benchmark() {
    use std::time::Instant;
    use symbolica::tensors::sparse::SparseMatrix;

    fn setting(name: &str, default: Option<usize>) -> usize {
        match std::env::var(name) {
            Ok(value) => value.parse().unwrap_or_else(|_| panic!("invalid {name}")),
            Err(_) => default.unwrap_or_else(|| panic!("set {name} explicitly")),
        }
    }

    let path = std::env::var("RUSTRED_MODULAR_AUDIT_STATE")
        .expect("set RUSTRED_MODULAR_AUDIT_STATE to the immutable native input");
    let maximum = setting("RUSTRED_MODULAR_AUDIT_SOURCE_ROWS", None);
    let start = setting("RUSTRED_MODULAR_AUDIT_START_ROW", Some(0));
    let terminal_ordinal = setting("RUSTRED_MODULAR_AUDIT_TERMINAL_ORDINAL", Some(0));
    let batch = setting("RUSTRED_MODULAR_AUDIT_BATCH_ROWS", Some(64));
    let workers = setting("RUSTRED_MODULAR_AUDIT_EXACT_WORKERS", Some(32));
    let modular_workers = setting("RUSTRED_MODULAR_AUDIT_MODULAR_WORKERS", Some(workers));
    let compare_exact = setting("RUSTRED_MODULAR_AUDIT_COMPARE_EXACT", Some(1));
    assert!(maximum > 0 && batch > 0 && workers > 0 && modular_workers > 0);
    assert!(compare_exact <= 1);
    let bytes = std::fs::read(&path).unwrap();
    let limits = TerminalRelationLimits {
        diagnostics_enabled: true,
        max_seeds: usize::MAX,
        max_columns: usize::MAX,
        max_rows: usize::MAX,
        max_nonzeros: usize::MAX,
        ..Default::default()
    };
    let load = Instant::now();
    let mut baseline =
        TerminalRelationSession::from_native_bytes(&bytes, limits, Default::default()).unwrap();
    let load_seconds = load.elapsed().as_secs_f64();
    assert!(baseline.assistance.is_none());
    assert_eq!(baseline.rebuild_cursor, baseline.rebuild.len());
    let recorded = usize::try_from(baseline.statistics().completed_source_rows).unwrap();
    assert!(
        start
            .checked_add(maximum)
            .is_some_and(|end| end <= recorded),
        "positive replay must stay within the authenticated already-processed ordinary prefix"
    );
    let original_maps = baseline.apply_all_terminals().unwrap();
    let original_rules = baseline.terminal_relations;
    let basis = baseline.basis_rows();
    let removed = basis
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            !row.is_empty() && row.keys().all(|key| baseline.terminals.contains(key))
        })
        .nth(terminal_ordinal)
        .map(|(ordinal, _)| ordinal)
        .expect("requested known terminal row is absent from the immutable state");
    let mut removed_columns: Vec<_> = basis[removed].keys().cloned().collect();
    elimination::sort_keys(&mut removed_columns);
    let omitted_pivot = removed_columns[0].clone();
    eprintln!(
        "positive replay: omitted basis row {removed}, terminal ordinal {terminal_ordinal}, pivot {:?}; source interval {start}..{}; compare_exact={compare_exact}",
        omitted_pivot.powers(),
        start + maximum,
    );

    // Preserve every other trusted U row verbatim in chronological order. This
    // is an experimental snapshot, not a change to the original source file.
    let field = RationalPolynomialField::new(Z);
    let mut upper = SparseMatrix::new(0, baseline.columns.len() as u32, field);
    let mut pivots = vec![None; baseline.columns.len()];
    for (ordinal, row) in basis.into_iter().enumerate() {
        if ordinal == removed {
            continue;
        }
        let mut terms: Vec<_> = row
            .into_iter()
            .map(|(key, value)| {
                (
                    baseline.columns.len() as u32 - 1 - baseline.column_offsets[&key],
                    value,
                )
            })
            .collect();
        terms.sort_by_key(|(column, _)| *column);
        pivots[terms[0].0 as usize] = Some(upper.nrows());
        let (ids, values) = terms.into_iter().unzip();
        upper.add_row(values, ids);
    }
    baseline.reducer = SparseRowReducer::from_upper_triangular_matrix(upper, pivots);
    baseline.terminal_relations = baseline.terminal_relation_count();
    assert_eq!(baseline.terminal_relations + 1, original_rules);
    baseline.seed_cursor = start / baseline.sources.len();
    baseline.source_cursor = start % baseline.sources.len();
    let inherited_guards = baseline.conditions.clone();
    let snapshot = baseline.to_native_bytes(Default::default()).unwrap();
    let export_only = setting("RUSTRED_MODULAR_AUDIT_EXPORT_ONLY", Some(0));
    assert!(export_only <= 1);
    if let Some(destination) = std::env::var_os("RUSTRED_MODULAR_AUDIT_EXPORT_SNAPSHOT") {
        use std::io::Write;
        use std::path::Path;

        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let temporary = repository.join("TMP").canonicalize().unwrap();
        let destination = Path::new(&destination);
        let destination = if destination.is_absolute() {
            destination.to_path_buf()
        } else {
            repository.join(destination)
        };
        let parent = destination
            .parent()
            .expect("export requires a parent")
            .canonicalize()
            .expect("create the TMP export directory explicitly");
        assert!(
            parent.starts_with(&temporary),
            "benchmark snapshot exports must stay under repository TMP/"
        );
        let destination = parent.join(destination.file_name().expect("export requires a filename"));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .expect("export must name a fresh TMP file; overwrites are forbidden");
        file.write_all(&snapshot).unwrap();
        file.sync_all().unwrap();
        eprintln!(
            "positive replay: exported immutable experimental baseline to {}",
            destination.display()
        );
        if export_only == 1 {
            println!(
                "{{\"mode\":\"export-only\",\"bytes\":{},\"blake3\":\"{}\",\"omitted_terminal_ordinal\":{terminal_ordinal},\"source_start\":{start},\"requested_source_rows\":{maximum},\"source_rows_advanced\":0,\"initial_known_relations\":{},\"remaining_terminals\":{}}}",
                snapshot.len(),
                blake3::hash(&snapshot).to_hex(),
                baseline.terminal_relations,
                baseline.statistics().remaining_terminals
            );
            return;
        }
    } else {
        assert_eq!(
            export_only, 0,
            "EXPORT_ONLY requires an explicit fresh TMP EXPORT_SNAPSHOT path"
        );
    }
    let mut exact = (compare_exact == 1).then(|| {
        TerminalRelationSession::from_native_bytes(&snapshot, limits, Default::default()).unwrap()
    });
    let modular_input =
        TerminalRelationSession::from_native_bytes(&snapshot, limits, Default::default()).unwrap();
    let mut policy = TerminalRelationModularOptions::default();
    policy.batch_rows = batch;
    let modular_timer = Instant::now();
    eprintln!("positive replay: constructing inherited finite-field image");
    let mut modular = TerminalRelationModularSession::new(modular_input, policy).unwrap();
    modular.set_workers(modular_workers).unwrap();
    let prepare_seconds = modular_timer.elapsed().as_secs_f64();
    eprintln!("positive replay: image prepared in {prepare_seconds:.3}s; scanning saved sources");
    let mut last_progress = Instant::now();
    while modular.statistics().source_rows_scanned < maximum as u64 && modular.circuits.is_empty() {
        modular.options.batch_rows =
            batch.min(maximum - usize::try_from(modular.statistics().source_rows_scanned).unwrap());
        modular
            .step_observed(&AtomicBool::new(false), |progress| {
                if last_progress.elapsed() >= std::time::Duration::from_secs(5) {
                    let s = progress.statistics;
                    eprintln!(
                        "positive replay: stage={} elapsed={:.3}s scanned={} candidates={} pending={} accepted={} misses={}",
                        progress.stage,
                        modular_timer.elapsed().as_secs_f64(),
                        s.source_rows_scanned,
                        s.candidates_found,
                        s.pending_candidates,
                        s.accepted_relations,
                        s.reconstruction_misses,
                    );
                    last_progress = Instant::now();
                }
            })
            .unwrap();
    }
    let modular_seconds = modular_timer.elapsed().as_secs_f64();
    let cohort = usize::try_from(modular.statistics().source_rows_scanned).unwrap();
    eprintln!(
        "positive replay: bounded scan finished after {modular_seconds:.3}s: {:?}",
        modular.statistics()
    );
    if modular.circuits.is_empty() {
        for (target, status) in modular.proposals().take(3) {
            eprintln!(
                "positive replay: unaccepted target {:?}: {status:?}",
                target.powers()
            );
        }
    }
    assert!(
        !modular.circuits.is_empty(),
        "bounded authentic cohort found no positive relation; this is an inconclusive performance control"
    );
    assert!(
        modular
            .session
            .terminal_rules()
            .contains_key(&omitted_pivot)
    );

    let exact_seconds = if let Some(exact) = &mut exact {
        let mut execution = TerminalRelationExecutionOptions::new(workers);
        execution.batch_rows = cohort;
        execution.block_rows = 8;
        let executor = TerminalRelationExecutor::new(execution).unwrap();
        let before = exact.statistics().completed_source_rows;
        let exact_timer = Instant::now();
        eprintln!("positive replay: matched exact control over {cohort} sources");
        while exact.statistics().completed_source_rows - before < cohort as u64 {
            executor.step(&mut *exact, &AtomicBool::new(false)).unwrap();
        }
        let seconds = exact_timer.elapsed().as_secs_f64();
        assert_eq!(exact.apply_all_terminals().unwrap(), original_maps);
        seconds.to_string()
    } else {
        "null".to_owned()
    };
    let modular_maps = modular.session.apply_all_terminals().unwrap();
    assert_eq!(modular_maps, original_maps);
    assert_eq!(
        modular.session.statistics().completed_source_rows,
        start as u64
    );
    assert!(
        inherited_guards
            .iter()
            .all(|guard| modular.session.conditions.contains(guard))
    );
    assert!(modular.circuits.iter().all(|circuit| {
        circuit
            .nonzero_conditions
            .iter()
            .all(|guard| modular.session.conditions.contains(guard))
    }));
    let max_frame_rows = modular
        .circuits
        .iter()
        .map(|circuit| circuit.sources.len())
        .max()
        .unwrap();
    let weight_terms: usize = modular
        .circuits
        .iter()
        .map(|circuit| circuit.weights.len())
        .sum();
    eprintln!(
        "{{\"experiment\":\"known-terminal-positive-replay\",\"input_blake3\":\"{}\",\"input_bytes\":{},\"load_seconds\":{load_seconds},\"omitted_terminal_ordinal\":{terminal_ordinal},\"start_row\":{start},\"cohort_rows\":{cohort},\"modular_prepare_seconds\":{prepare_seconds},\"modular_full_pipeline_seconds\":{modular_seconds},\"modular_workers\":{modular_workers},\"exact_seconds\":{exact_seconds},\"exact_workers\":{workers},\"exact_block_rows\":8,\"accepted_relations\":{},\"maximum_source_frame_rows\":{max_frame_rows},\"nonzero_weight_slots\":{weight_terms},\"reconstruction_misses\":{},\"guard_count\":{},\"probe_count\":null,\"all_terminal_coefficients_equal\":true}}",
        blake3::hash(&bytes).to_hex(),
        bytes.len(),
        modular.circuits.len(),
        modular.reconstruction_misses,
        modular.session.conditions.len(),
    );
}
