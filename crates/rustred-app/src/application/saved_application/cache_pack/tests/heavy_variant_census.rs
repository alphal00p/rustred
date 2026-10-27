//! Opt-in real-fixture census; never reads an implicit or live cache.
use super::*;
use serde_json::{Value, json};
use std::{
    env,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    time::Instant,
};

const MAX_SOURCE_BYTES: u64 = 10_000_000;
const PREFIX: &str = "RUSTRED_HEAVY_CENSUS_";

fn input(name: &str) -> PathBuf {
    PathBuf::from(env::var(format!("{PREFIX}{name}")).expect("explicit census path required"))
        .canonicalize()
        .expect("census input exists")
}

fn files(path: &Path, output: &mut Vec<(PathBuf, u64)>) {
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(!metadata.is_symlink(), "census rejects input symlinks");
        if metadata.is_dir() {
            files(&path, output);
        } else {
            assert!(metadata.is_file(), "only regular input files permitted");
            output.push((path, metadata.len()));
        }
    }
}

fn hashes(paths: &[(PathBuf, u64)], root: &Path) -> Value {
    let mut result = serde_json::Map::new();
    for (path, length) in paths {
        let mut digest = blake3::Hasher::new();
        let mut file = File::open(path).unwrap();
        let mut buffer = [0; 65_536];
        let mut actual = 0u64;
        loop {
            let n = file.read(&mut buffer).unwrap();
            if n == 0 {
                break;
            }
            actual += n as u64;
            assert!(actual <= *length, "input grew while being inventoried");
            digest.update(&buffer[..n]);
        }
        assert_eq!(actual, *length);
        result.insert(
            path.strip_prefix(root).unwrap().to_str().unwrap().into(),
            json!({"bytes":actual,"blake3":digest.finalize().to_hex().to_string()}),
        );
    }
    Value::Object(result)
}

fn original_process_is_live(pid: u64, start: u64) -> bool {
    let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    let mut fields = stat.rsplit_once(')').unwrap().1.split_whitespace();
    fields.nth(19).unwrap().parse::<u64>().unwrap() == start
}

fn closed_source_receipt(source: &Path, receipt: &Value) {
    assert_eq!(
        PathBuf::from(receipt["native_cache_directory"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        source,
        "closure receipt must identify the exact supplied source"
    );
    assert!(receipt["state"].as_str().unwrap().starts_with("PASS_"));
    let workers = receipt["actual_worker"].as_object().unwrap();
    assert!(!workers.is_empty());
    for worker in workers.values() {
        assert_eq!(worker["process_exists"], false);
        assert!(!original_process_is_live(
            worker["pid"].as_u64().unwrap(),
            worker["start_ticks"].as_u64().unwrap(),
        ));
    }
    let states = receipt["unit_states_after_worker_wait_and_guard_retirement"]
        .as_object()
        .unwrap();
    assert!(!states.is_empty());
    for state in states.values() {
        assert_eq!(state["ActiveState"], "inactive");
        assert_eq!(state["Result"], "success");
    }
}

fn parse_binding(value: &Value) -> ApplicationCacheBinding {
    let digest: Vec<u8> = value["artifact_digest"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| u8::try_from(n.as_u64().unwrap()).unwrap())
        .collect();
    ApplicationCacheBinding {
        artifact_digest: digest.try_into().unwrap(),
        family_fingerprint: value["family_fingerprint"].as_str().unwrap().into(),
        physical_arity: usize::try_from(value["physical_arity"].as_u64().unwrap()).unwrap(),
        layer: value["layer"].as_str().unwrap().into(),
    }
}

struct Variant {
    record: ApplicationCacheRecord,
    references: u64,
    origins: Vec<Value>,
}

fn record_value(variant: &Variant) -> Value {
    json!({
        "references":variant.references,
        "origins":variant.origins,
        "terms":variant.record.terms.iter().map(|(key,value)|
            json!({"powers":key.powers(),"coefficient_debug":format!("{value:?}")}))
            .collect::<Vec<_>>(),
        "ordered_guards":variant.record.nonzero_conditions.iter()
            .map(|value|format!("{value:?}")).collect::<Vec<_>>(),
    })
}

fn process_memory_snapshot() -> Value {
    let status = fs::read_to_string("/proc/self/status").unwrap();
    let field = |name: &str| {
        status.lines().find_map(|line| {
            line.strip_prefix(name)
                .and_then(|value| value.split_whitespace().next())
                .map(|value| value.parse::<u64>().unwrap())
        })
    };
    json!({"rss_kib":field("VmRSS:"),"peak_rss_kib":field("VmHWM:"),
        "scope":"Whole census test process: includes retained source census, global native state and allocator/native scratch; not writer-only RAM."})
}

fn optional_one_file_export(
    source: &Path,
    binding: &ApplicationCacheBinding,
    context: &CoefficientContext,
    grouped: &BTreeMap<ApplicationCacheKey, Vec<Variant>>,
    expected_references: u64,
    exact_duplicates: u64,
    limits: SavedApplicationCachePackLimits,
) -> Option<Value> {
    use crate::{
        SavedApplicationCacheFile, SavedApplicationCacheFileLimits,
        SavedApplicationCacheFileOptions, SavedApplicationCacheFileWriter,
    };
    let output = PathBuf::from(env::var_os("RUSTRED_HEAVY_EXPORT_OUTPUT")?);
    assert!(
        grouped.values().all(|variants| variants.len() == 1),
        "one-file export must not silently select conflicting coefficients or guards"
    );
    let output_parent = output.parent().unwrap().canonicalize().unwrap();
    let output = output_parent.join(output.file_name().unwrap());
    assert!(!output_parent.starts_with(source));
    assert!(!output.exists(), "export destination must be newly created");
    let temporary_directory =
        PathBuf::from(env::var_os("TMPDIR").expect("explicit private TMPDIR required"))
            .canonicalize()
            .unwrap();
    assert!(!temporary_directory.starts_with(source));
    assert!(temporary_directory.is_dir());
    let temporary_names = || {
        let mut names = fs::read_dir(&temporary_directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        names.sort();
        names
    };
    let temporary_before = temporary_names();
    // Explicit small-pilot grants: this test never needs GiB-scale payload I/O.
    let file_limits = SavedApplicationCacheFileLimits {
        max_compressed_bytes: 20_000_000,
        ..SavedApplicationCacheFileLimits::default()
    }
    .with_decoded_bytes(128 * 1024 * 1024)
    .unwrap();
    let started = Instant::now();
    let memory_before = process_memory_snapshot();
    let mut writer = SavedApplicationCacheFileWriter::new_unordered(
        binding,
        context,
        file_limits,
        &temporary_directory,
    )
    .unwrap();
    let bundle =
        SavedApplicationCacheBundle::open_generated(source, std::slice::from_ref(binding), limits)
            .unwrap();
    let mut visitor = bundle.open_reader(binding).unwrap();
    let visited = visitor
        .visit_all_completed_records(context, |record| {
            let original = &grouped
                .get(&record.key)
                .expect("visitor emitted an uncensused key")[0]
                .record;
            assert_eq!(
                &record, original,
                "public visitor must preserve full terms and ordered guards"
            );
            writer.push_unordered(&record)
        })
        .unwrap();
    assert_eq!(visited as u64, expected_references);
    let output_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .unwrap();
    let (file, identity, written) = writer.finish(output_file).unwrap();
    file.sync_all().unwrap();
    drop(file);
    assert_eq!(written.input_records as u64, expected_references);
    assert_eq!(written.identical_duplicates as u64, exact_duplicates);
    assert_eq!(written.records, grouped.len());
    let export_seconds = started.elapsed().as_secs_f64();
    let memory_after_export = process_memory_snapshot();
    let options = SavedApplicationCacheFileOptions {
        limits: file_limits,
        decoded_directory: Some(temporary_directory.clone()),
    };
    let import_started = Instant::now();
    let mut imported =
        SavedApplicationCacheFile::open_with_options(&output, binding, Some(&identity), &options)
            .unwrap();
    assert_eq!(imported.identity(), &identity);
    assert_eq!(imported.binding(), binding);
    assert_eq!(
        imported.keys().cloned().collect::<Vec<_>>(),
        grouped.keys().cloned().collect::<Vec<_>>()
    );
    let before_lookup = imported.statistics().unwrap();
    assert_eq!(before_lookup.state_imports, 0);
    assert_eq!(before_lookup.decoded_coefficients, 0);
    let mut fork = imported.fork_reader();
    assert_eq!(fork.statistics().unwrap().state_imports, 0);
    let probe = grouped
        .values()
        .find(|variants| {
            !variants[0].record.terms.is_empty()
                || !variants[0].record.nonzero_conditions.is_empty()
        })
        .expect("pilot must exercise coefficient import");
    assert_eq!(
        fork.lookup(&probe[0].record.key, context).unwrap().as_ref(),
        Some(&probe[0].record)
    );
    // The untouched original reader observes its fork's one shared import.
    assert_eq!(imported.statistics().unwrap().state_imports, 1);
    assert_eq!(fork.statistics().unwrap().state_imports, 1);
    for (ordinal, (key, variants)) in grouped.iter().enumerate() {
        let reader = if ordinal % 2 == 0 {
            &mut imported
        } else {
            &mut fork
        };
        let actual = reader
            .lookup(key, context)
            .unwrap()
            .expect("exported key missing");
        assert_eq!(
            actual, variants[0].record,
            "single-file reimport must preserve exact terms and ordered guards"
        );
    }
    let after_lookup = imported.statistics().unwrap();
    let fork_after_lookup = fork.statistics().unwrap();
    assert_eq!(after_lookup.state_imports, 1);
    assert_eq!(fork_after_lookup.state_imports, 1);
    assert_eq!(
        after_lookup.decoded_coefficients,
        written.unique_coefficients
    );
    assert_eq!(
        fork_after_lookup.decoded_coefficients,
        after_lookup.decoded_coefficients
    );
    assert_eq!(
        after_lookup.decoded_coefficient_terms,
        fork_after_lookup.decoded_coefficient_terms
    );
    assert_eq!(
        after_lookup.decoded_coefficient_bytes,
        fork_after_lookup.decoded_coefficient_bytes
    );
    let import_and_compare_seconds = import_started.elapsed().as_secs_f64();
    let memory_after_comparison = process_memory_snapshot();
    drop(fork);
    drop(imported);
    drop(visitor);
    drop(bundle);
    assert_eq!(
        temporary_names(),
        temporary_before,
        "writer spools and decoded backing must leave no persistent temporary paths"
    );
    let compressed_identity = hashes(
        &[(output.clone(), identity.compressed_bytes)],
        &output_parent,
    );
    assert_eq!(
        compressed_identity[output.file_name().unwrap().to_str().unwrap()]["blake3"],
        blake3::Hash::from_bytes(identity.compressed_blake3)
            .to_hex()
            .to_string()
    );
    Some(json!({
        "success":true,"output":output,"identity":identity,
        "input_record_references":visited,"unique_records":written.records,
        "exact_duplicate_references":written.identical_duplicates,
        "all_unique_terms_and_ordered_guards_reimported_equal":true,
        "shared_fork_state_imports_before":before_lookup.state_imports,
        "shared_fork_state_imports_after":after_lookup.state_imports,
        "decoded_backing":"checked file range reader with owned unlinked descriptor",
        "unique_coefficient_atoms":written.unique_coefficients,
        "unique_coefficient_payload_bytes":written.unique_coefficient_payload_bytes,
        "encoded_coefficient_table_bytes":written.encoded_coefficient_table_bytes,
        "interner_index_lower_bound_bytes":written.interner_index_lower_bound_bytes,
        "unordered_index_owned_bytes":written.unordered_index_owned_bytes,
        "unordered_spool_bytes":written.unordered_spool_bytes,
        "native_bytes":written.native_bytes,
        "reader_index_owned_bytes":after_lookup.index_owned_bytes,
        "reader_decoded_coefficient_terms":after_lookup.decoded_coefficient_terms,
        "reader_decoded_coefficient_bytes":after_lookup.decoded_coefficient_bytes,
        "export_seconds":export_seconds,"import_and_compare_seconds":import_and_compare_seconds,
        "memory_before":memory_before,"memory_after_export":memory_after_export,
        "memory_after_comparison":memory_after_comparison,
        "resource_qualifier":"Writer interner/index fields are known payload/admission lower bounds, not total RAM or allocator/native scratch bounds. Spool bytes are temporary I/O, not heap. Process RSS includes source census and native state.",
        "temporary_paths_cleaned":true,
        "compressed_file_identity_independently_rehashed":compressed_identity,
        "scope":"Actual closed small3 one-gzip-member native export/reimport only; not a HEAVY-size resource scaling or physical speedup certificate."
    }))
}

#[test]
#[ignore = "requires explicit closed <=10 MB source, request, closure receipt and fresh report path"]
fn closed_small_source_all_native_key_and_ordered_guard_variants() {
    let started = Instant::now();
    let source = input("SOURCE");
    let request_path = input("REQUEST");
    let closure_path = input("CLOSURE");
    let output = PathBuf::from(env::var(format!("{PREFIX}OUTPUT")).unwrap());
    let parent = output.parent().unwrap().canonicalize().unwrap();
    assert!(!parent.starts_with(&source));
    assert!(!output.exists(), "report must be newly created");
    let mut inventory = Vec::new();
    files(&source, &mut inventory);
    inventory.sort();
    let input_bytes = inventory
        .iter()
        .try_fold(0u64, |n, (_, bytes)| n.checked_add(*bytes))
        .unwrap();
    assert!(input_bytes <= MAX_SOURCE_BYTES);
    // Open an existing lock read-only: do not create or alter any source file.
    let lock = File::open(source.join("writer.lock")).unwrap();
    lock.try_lock()
        .expect("closed source must have no active writer");
    let closure_bytes = fs::read(&closure_path).unwrap();
    let closure: Value = serde_json::from_slice(&closure_bytes).unwrap();
    closed_source_receipt(&source, &closure);
    // No chunk payload was read before size, closure and writer checks above.
    let before = hashes(&inventory, &source);
    let request_bytes = fs::read(&request_path).unwrap();
    let request: Value = serde_json::from_slice(&request_bytes).unwrap();
    let binding = parse_binding(&request["semantic_binding"]);
    let limits = SavedApplicationCachePackLimits::default();
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let bundle = SavedApplicationCacheBundle::open_generated(
        &source,
        std::slice::from_ref(&binding),
        limits,
    )
    .unwrap();
    let pack = bundle.open_reader(&binding).unwrap();
    let child = child_path(&source, &binding);
    let manifest_bytes =
        index::read_bounded(&child.join(index::MANIFEST), limits.max_manifest_bytes).unwrap();
    let (manifest, statistics) = index::decode(&manifest_bytes, &binding, limits).unwrap();
    assert_eq!(manifest.generation, pack.generation());
    let source_requests = request["sources_newest_first"].as_array().unwrap();
    let source_censuses = closure["source_censuses"].as_array().unwrap();
    assert_eq!(source_requests.len(), source_censuses.len());
    let mut source_runs = Vec::new();
    let mut expected_references = 0u64;
    for (source_request, census) in source_requests.iter().zip(source_censuses).rev() {
        let count = usize::try_from(census["runs"].as_u64().unwrap()).unwrap();
        expected_references += census["stored_record_references"].as_u64().unwrap();
        for original_run in 0..count {
            source_runs.push(json!({
                "source_id":source_request["id"],"original_run":original_run,
                "original_directory":source_request["directory"],
                "original_native_index_identity":source_request["cache_native_index_identity"]
            }));
        }
    }
    assert_eq!(source_runs.len(), manifest.runs.len());
    let mut grouped = BTreeMap::<ApplicationCacheKey, Vec<Variant>>::new();
    let mut references = 0u64;
    let mut physical_chunks = std::collections::BTreeSet::new();
    for (run_index, run) in manifest.runs.iter().enumerate() {
        for (chunk_index, chunk) in run.iter().enumerate() {
            // The production all-version reader is deliberately private to
            // reader::visit. Reuse its native transport/codec primitives here;
            // do not expose a new API merely for this opt-in census.
            let encoded = index::read_bounded(
                &child.join("chunks").join(chunk.file_name()),
                usize::try_from(chunk.bytes).unwrap(),
            )
            .unwrap();
            assert_eq!(encoded.len() as u64, chunk.bytes);
            assert_eq!(blake3::hash(&encoded).as_bytes(), &chunk.digest);
            let payload =
                compression::decode(&encoded, chunk.decoded_bytes, &chunk.decoded_digest, limits)
                    .unwrap();
            let decoded =
                rustred::persistence::application_cache::OwnedApplicationCacheChunk::inspect(
                    std::sync::Arc::from(payload),
                    &binding,
                    limits.chunk,
                )
                .unwrap();
            assert_eq!(decoded.len(), chunk.records);
            assert_eq!(decoded.keys().next(), Some(&chunk.first));
            assert_eq!(decoded.keys().last(), Some(&chunk.last));
            if let Some(expected) = &chunk.key_fingerprints {
                assert_eq!(
                    &index::key_filter::collect(decoded.keys()).unwrap(),
                    expected
                );
            }
            physical_chunks.insert(chunk.digest);
            for key in decoded.keys() {
                let record = decoded.decode_record(key, &context).unwrap().unwrap();
                references += 1;
                let origin = json!({"run":run_index,"chunk":chunk_index,
                    "chunk_blake3":blake3::Hash::from_bytes(chunk.digest).to_hex().to_string(),
                    "source":source_runs[run_index]});
                let variants = grouped.entry(key.clone()).or_default();
                if let Some(variant) = variants.iter_mut().find(|v| v.record == record) {
                    variant.references += 1;
                    variant.origins.push(origin);
                } else {
                    variants.push(Variant {
                        record,
                        references: 1,
                        origins: vec![origin],
                    });
                }
            }
        }
    }
    let mut conflicting_keys = 0u64;
    let mut coefficient_variant_keys = 0u64;
    let mut ordered_guard_variant_keys = 0u64;
    let mut full_variants = 0u64;
    let mut exact_duplicates = 0u64;
    let mut witnesses = Vec::new();
    for (key, variants) in &grouped {
        full_variants += variants.len() as u64;
        exact_duplicates += variants.iter().map(|v| v.references - 1).sum::<u64>();
        if variants.len() == 1 {
            continue;
        }
        conflicting_keys += 1;
        let first = &variants[0].record;
        let differs_coefficients = variants.iter().any(|v| v.record.terms != first.terms);
        let differs_ordered_guards = variants
            .iter()
            .any(|v| v.record.nonzero_conditions != first.nonzero_conditions);
        coefficient_variant_keys += u64::from(differs_coefficients);
        ordered_guard_variant_keys += u64::from(differs_ordered_guards);
        witnesses.push(
            json!({"stage":format!("{:?}",key.stage()),"owner":key.owner(),
            "powers":key.powers(),"different_coefficients":differs_coefficients,
            "different_ordered_guards":differs_ordered_guards,
            "variants":variants.iter().map(record_value).collect::<Vec<_>>() }),
        );
    }
    assert_eq!(references, expected_references);
    assert_eq!(references as usize, statistics.records);
    assert_eq!(references, full_variants + exact_duplicates);
    let one_file_export = optional_one_file_export(
        &source,
        &binding,
        &context,
        &grouped,
        references,
        exact_duplicates,
        limits,
    );
    let mut after_inventory = Vec::new();
    files(&source, &mut after_inventory);
    after_inventory.sort();
    assert_eq!(inventory, after_inventory);
    let after = hashes(&after_inventory, &source);
    assert_eq!(
        before, after,
        "all source lengths and hashes must remain identical"
    );
    assert_eq!(fs::read(&request_path).unwrap(), request_bytes);
    assert_eq!(fs::read(&closure_path).unwrap(), closure_bytes);
    let mut report = json!({
        "schema":"rustred.closed-small-native-full-variant-census.v1",
        "success":true,"source":source,"source_bytes":input_bytes,
        "native_generation":manifest.generation,"native_runs":manifest.runs.len(),
        "physical_chunks":physical_chunks.len(),"stored_record_references":references,
        "distinct_keys":grouped.len(),"distinct_full_payload_variants":full_variants,
        "exact_duplicate_record_references":exact_duplicates,
        "conflicting_keys":conflicting_keys,"coefficient_variant_keys":coefficient_variant_keys,
        "ordered_guard_variant_keys":ordered_guard_variant_keys,
        "one_table_unique_key_count":if conflicting_keys==0 {Some(grouped.len())} else {None},
        "one_table_export_performed":one_file_export.is_some(),
        "one_table_note":"Existing single-chunk codec accepts one payload per exact key. Variant selection is not performed by this census.",
        "manifest_blake3":blake3::hash(&manifest_bytes).to_hex().to_string(),
        "request_blake3":blake3::hash(&request_bytes).to_hex().to_string(),
        "closure_receipt_blake3":blake3::hash(&closure_bytes).to_hex().to_string(),
        "hashes_before":before,"hashes_after":after,"all_inputs_unchanged":true,
        "conflict_witnesses":witnesses,"elapsed_seconds":started.elapsed().as_secs_f64(),
        "scope":"Every committed record version in explicit closed small source. Native exact term equality and ordered guard equality; no IBP revalidation, cache export, guard weakening, live q0 read or source writes."
    });
    if let Some(export) = one_file_export {
        report["one_file_export"] = export;
        report["one_table_note"] = "All identical versions passed through the native unordered writer; every unique reimported key, coefficient and ordered guard matched the independent source census.".into();
        report["scope"] = "Every committed record version in the explicit closed small source, plus optional actual native one-file export and checked file-backed reimport. Native exact term and ordered guard equality; no IBP revalidation, guard weakening, live q0 read or source writes.".into();
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .unwrap();
    file.write_all(&serde_json::to_vec_pretty(&report).unwrap())
        .unwrap();
    file.write_all(b"\n").unwrap();
    file.sync_all().unwrap();
    eprintln!(
        "small native variant census: {references} references, {} keys, {conflicting_keys} conflicting keys",
        grouped.len()
    );
}
