//! Opt-in generation-free measurement against an immutable saved publication.
//! No production lock, ordinary IBP source, campaign amendment or numerical
//! master value is used. Run explicitly with the two environment paths below.
use super::*;
use crate::application::routed_campaign::{RoutedCampaignRequest, prepare_terminal_equations};
use rustred::reduction::terminal_normalization::{
    TerminalNumeratorSymmetryEquations, TerminalNumeratorSymmetryLimits,
};
use rustred::reduction::terminal_relations::TerminalRelationRow;
use std::collections::BTreeMap;

fn tree_hashes(directory: &Path) -> BTreeMap<PathBuf, String> {
    fn walk(root: &Path, at: &Path, output: &mut BTreeMap<PathBuf, String>) {
        for entry in std::fs::read_dir(at).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let kind = entry.file_type().unwrap();
            assert!(
                !kind.is_symlink(),
                "source evidence must not follow symlinks"
            );
            if kind.is_dir() {
                walk(root, &path, output);
            } else if entry.file_name() != "writer.lock" {
                output.insert(
                    path.strip_prefix(root).unwrap().into(),
                    storage::digest_file(&path).unwrap(),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    walk(directory, directory, &mut result);
    result
}

fn row_view(row: &TerminalRelationRow) -> Value {
    json!(
        row.iter()
            .map(|(key, value)| json!({
                "key":key.powers(), "coefficient_display":value.to_string(),
            }))
            .collect::<Vec<_>>()
    )
}

fn maps_view(maps: &BTreeMap<IntegralKey, TerminalRelationRow>) -> Value {
    json!(
        maps.iter()
            .map(|(key, row)| json!({
                "target":key.powers(), "terms":row_view(row),
            }))
            .collect::<Vec<_>>()
    )
}

#[test]
#[ignore = "explicit immutable publication and new workspace evidence directory required"]
fn measured_five_loop_numerator_symmetry_only_against_retained_maps() {
    let source = PathBuf::from(
        std::env::var_os("RUSTRED_NUMERATOR_SYMMETRY_SOURCE")
            .expect("set immutable RUSTRED_NUMERATOR_SYMMETRY_SOURCE"),
    );
    let output = PathBuf::from(
        std::env::var_os("RUSTRED_NUMERATOR_SYMMETRY_EVIDENCE")
            .expect("set new RUSTRED_NUMERATOR_SYMMETRY_EVIDENCE"),
    );
    assert!(!output.exists(), "evidence directory must be new");
    storage::reject_overlapping_directories(&source.canonicalize().unwrap(), &output).unwrap();
    std::fs::create_dir_all(&output).unwrap();
    let started = Instant::now();
    eprintln!("five-loop numerator measurement: hashing immutable source");
    let hashes_before = tree_hashes(&source);
    let source_hash_seconds = started.elapsed().as_secs_f64();
    write_json(
        &output.join("source-hashes-before.json"),
        &json!(hashes_before),
    )
    .unwrap();
    let source_report = master_reduction_inspect(&source).unwrap();
    assert!(matches!(
        source_report["status"].as_str(),
        Some("published_unrefined" | "completed_nonminimal")
    ));
    eprintln!("five-loop numerator measurement: loading complete source maps");
    let phase_started = Instant::now();
    let artifact = load_master_reduction(&source).unwrap();
    let source_load_seconds = phase_started.elapsed().as_secs_f64();
    eprintln!(
        "five-loop numerator measurement: source loaded in {source_load_seconds:.3}s; importing retained maps"
    );
    let phase_started = Instant::now();
    let old_stats = artifact.primary_session().statistics();
    let mut report = source_report.clone();
    let mut options = MasterReductionOptions::new("unused", &output);
    options.fresh_search = true;
    options.numerator_symmetry_assistance = true;
    // Saved candidate programs reduce complete witnessed symmetry identities;
    // their authority and content binding remain explicit even though their
    // separate per-key equation provider is deliberately excluded below.
    options.saved_rule_assistance = true;
    options.finite_feedback = Some(false);
    options.threads = 8;
    let mut native = fresh::begin(
        &source_report,
        &artifact,
        &options,
        profile::from_report(&source_report).unwrap(),
        &mut report,
    )
    .unwrap();
    let before_maps = native.apply_all_terminals().unwrap();
    for key in native.raw_terminals() {
        let expected = artifact.apply_terminal(native.family_owner(), key).unwrap();
        let local: TerminalRelationRow = expected
            .terms()
            .iter()
            .map(|(key, value)| {
                assert_eq!(
                    key.family_fingerprint(),
                    native.family_owner().fingerprint()
                );
                (key.integral().clone(), value.clone())
            })
            .collect();
        assert_eq!(
            before_maps[key], local,
            "relations-only baseline changed source output"
        );
    }
    drop(artifact);
    let baseline_import_seconds = phase_started.elapsed().as_secs_f64();
    let before_basis = native.remaining_terminals();
    let before_stats = native.statistics();
    assert_eq!(before_stats.completed_source_rows, 0);
    assert_eq!(before_stats.pending_rebuild_rows, 0);
    let baseline_bytes = native
        .to_native_bytes(profile::native_io_limits(None))
        .unwrap();
    write_file_atomically(&output.join("baseline.rrbin"), &baseline_bytes, true).unwrap();
    write_json(
        &output.join("baseline-maps-display.json"),
        &maps_view(&before_maps),
    )
    .unwrap();
    let phase_started = Instant::now();
    storage::clone_inputs_with_policy(&source, &output, &source_report, None).unwrap();
    let clone_inputs_seconds = phase_started.elapsed().as_secs_f64();
    // Bind both exact numerator witnesses and the saved candidate programs.
    // This measurement supplies only the former's reduced rows: the saved
    // provider is not queried for standalone key identities. No production
    // checkpoint or ordinary refinement resume is published by this harness.
    // Like the production fresh-round entry, leave the imported publication
    // status before preparing a new provider certificate for this new round.
    report["status"] = json!("running");
    assistance::configure(&options, &mut native, &mut report).unwrap();
    let cancel = AtomicBool::new(false);
    let observer = |event: Value| {
        eprintln!(
            "{}",
            json!({
                "elapsed_seconds": started.elapsed().as_secs_f64(),
                "stage":event["stage"],"preparation":event["preparation_event"],
                "numerator":event["numerator_symmetry_reduction"],
            })
        )
    };
    let phase_started = Instant::now();
    let plan = super::super::numerator_symmetry::prepare(
        &options,
        &native,
        &mut report,
        &cancel,
        &observer,
        started,
    )
    .unwrap();
    let symmetry_discovery_seconds = phase_started.elapsed().as_secs_f64();
    eprintln!(
        "five-loop numerator measurement: symmetry discovery {symmetry_discovery_seconds:.3}s, {:?}",
        plan.statistics()
    );
    let phase_started = Instant::now();
    let keys = plan.input_keys().clone();
    let plan_bytes = plan.encode_native(profile::native_io_limits(None)).unwrap();
    let cold = TerminalNumeratorSymmetryEquations::from_native_bytes(
        &plan_bytes,
        Arc::clone(native.family_owner()),
        TerminalNumeratorSymmetryLimits::default(),
        profile::native_io_limits(None),
    )
    .unwrap();
    assert_eq!(cold.statistics(), plan.statistics());
    let certificate_hash = blake3::hash(&plan_bytes).to_hex().to_string();
    let mut certificate_index = Vec::new();
    for key in &keys {
        assert_eq!(cold.equations(key), plan.equations(key));
        for (ordinal, witness) in cold.witnesses(key).iter().enumerate() {
            witness
                .replay(Arc::clone(native.family_owner()), Default::default())
                .unwrap();
            let mut item = json!({"provider_blake3":certificate_hash,
                "source":key.powers(),"witness_ordinal":ordinal,
                "equation_display":row_view(&witness.equation().terms),
                "guards_display":witness.equation().nonzero_conditions.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "authority":"native provider certificate, not display strings"});
            item["index_blake3"] = json!(
                blake3::hash(&serde_json::to_vec(&item).unwrap())
                    .to_hex()
                    .to_string()
            );
            certificate_index.push(item);
        }
    }
    write_json(
        &output.join("certificate-index.json"),
        &json!(certificate_index),
    )
    .unwrap();
    let witness_replay_seconds = phase_started.elapsed().as_secs_f64();
    let selection_path = storage::safe_child(
        &output,
        source_report["inputs"]["selection"].as_str().unwrap(),
    )
    .unwrap();
    let selection = std::fs::read(&selection_path).unwrap();
    assert_eq!(
        blake3::hash(&selection).to_hex().as_str(),
        source_report["inputs"]["program_binding"].as_str().unwrap()
    );
    let mut request =
        RoutedCampaignRequest::new(String::from_utf8(selection).unwrap(), String::new());
    request.owner_base = selection_path.parent().unwrap().to_path_buf();
    request.workers = options.threads;
    super::super::numerator_symmetry::configure_saved_resources(&mut request);
    let digests = source_report["inputs"]["payloads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["blake3"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    eprintln!("five-loop numerator measurement: preparing existing saved-rule coefficient reducer");
    let phase_started = Instant::now();
    let mut saved = prepare_terminal_equations(
        &request,
        native.family_owner().fingerprint(),
        &digests,
        &cancel,
        &observer,
    )
    .unwrap();
    let saved_rule_load_seconds = phase_started.elapsed().as_secs_f64();
    eprintln!(
        "five-loop numerator measurement: saved rules loaded in {saved_rule_load_seconds:.3}s; reducing complete symmetry rows"
    );
    let phase_started = Instant::now();
    let rows = super::super::numerator_symmetry::prepare_rows(
        &cold,
        Some(&mut saved),
        &mut report,
        &cancel,
        &observer,
        started,
    )
    .unwrap();
    let saved_row_reduction_seconds = phase_started.elapsed().as_secs_f64();
    drop(saved);
    let mut provider = |key: &IntegralKey| Ok(rows.get(key).cloned().unwrap_or_default());
    let mut last_event = Instant::now();
    let phase_started = Instant::now();
    while native.statistics().pending_assistance_keys != 0
        || native.statistics().pending_assistance_rows != 0
    {
        native.step_with_provider(&cancel, &mut provider).unwrap();
        assert_eq!(native.statistics().completed_source_rows, 0);
        if last_event.elapsed() >= Duration::from_secs(10) {
            eprintln!(
                "five-loop numerator measurement: assistance {:?}",
                native.statistics()
            );
            last_event = Instant::now();
        }
    }
    let assistance_elimination_seconds = phase_started.elapsed().as_secs_f64();
    let after_maps = native.apply_all_terminals().unwrap();
    let after_basis = native.remaining_terminals();
    assert_eq!(native.raw_terminals().len(), before_maps.len());
    assert_eq!(
        native.statistics().completed_source_rows,
        before_stats.completed_source_rows
    );
    assert_eq!(
        native.statistics().completed_seeds,
        before_stats.completed_seeds
    );
    assert!(after_basis.is_subset(&before_basis));
    for row in after_maps.values() {
        assert!(
            row.keys().all(|key| after_basis.contains(key)),
            "unresolved auxiliary escaped into output basis"
        );
    }
    let changed = after_maps.iter().filter(|(key,row)| before_maps.get(*key) != Some(*row))
        .map(|(key,row)| json!({"target":key.powers(),"before":row_view(&before_maps[key]),"after":row_view(row)}))
        .collect::<Vec<_>>();
    let bytes = native
        .to_native_bytes(profile::native_io_limits(None))
        .unwrap();
    let restored = TerminalRelationSession::from_native_bytes(
        &bytes,
        profile::from_report(&report).unwrap().relation_limits(),
        profile::native_io_limits(None),
    )
    .unwrap();
    assert_eq!(restored.apply_all_terminals().unwrap(), after_maps);
    assert_eq!(restored.nonzero_conditions(), native.nonzero_conditions());
    write_file_atomically(
        &output.join("numerator-symmetry-result.rrbin"),
        &bytes,
        true,
    )
    .unwrap();
    write_json(&output.join("changed-maps-display.json"), &json!(changed)).unwrap();
    write_json(
        &output.join("final-maps-display.json"),
        &maps_view(&after_maps),
    )
    .unwrap();
    let hashes_after = tree_hashes(&source);
    assert_eq!(hashes_after, hashes_before, "immutable source changed");
    write_json(
        &output.join("source-hashes-after.json"),
        &json!(hashes_after),
    )
    .unwrap();
    report["measurement"] = json!({
        "source":source,"source_source_rows":old_stats.completed_source_rows,
        "source_independent_rows":old_stats.independent_rows,"raw_keys":before_maps.len(),
        "basis_before":before_basis.len(),"basis_after":after_basis.len(),
        "additional_dependencies":before_basis.len()-after_basis.len(),
        "changed_raw_maps":changed.len(),"ordinary_source_rows_before":before_stats.completed_source_rows,
        "ordinary_source_rows_after":native.statistics().completed_source_rows,
        "assistance_rows":native.statistics().completed_assistance_rows,
        "auxiliary_columns":native.statistics().auxiliary_columns,
        "all_original_keys_defined":true,"all_outputs_remaining_terminals":true,
        "source_hashes_unchanged":true,"independence_claim":false,
        "scope":"only new numerator-symmetry equations reduced by existing saved rules; no standalone saved-key equations or new ordinary IBPs",
        "provider_certificate_blake3":certificate_hash,
        "result_blake3":blake3::hash(&bytes).to_hex().to_string(),
        "elapsed_seconds":started.elapsed().as_secs_f64(),
        "timings_seconds":{
            "source_initial_hash":source_hash_seconds,
            "source_full_artifact_load":source_load_seconds,
            "relations_only_import_and_exact_baseline_parity":baseline_import_seconds,
            "immutable_input_clone":clone_inputs_seconds,
            "numerator_symmetry_discovery":symmetry_discovery_seconds,
            "native_certificate_encode_decode_and_replay":witness_replay_seconds,
            "saved_rule_program_load":saved_rule_load_seconds,
            "complete_symmetry_row_saved_reduction":saved_row_reduction_seconds,
            "symmetry_only_assistance_elimination":assistance_elimination_seconds,
        },
        "guard_display":native.nonzero_conditions().iter().map(ToString::to_string).collect::<Vec<_>>(),
    });
    write_json(&output.join("measurement.json"), &report).unwrap();
    eprintln!("{}", report["measurement"]);
}
