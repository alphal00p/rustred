//! Opt-in measurement of cross-query sharing, never publication authority.
use super::*;
use crate::application::routed_campaign::{input::Selection, prepare};
use crate::{CandidateOwnerLoadLimits, RoutedCampaignRequest};
use rustred::solver::{
    CandidateRoutedCampaignReport, CandidateRoutedSelectionIdentity,
    CandidateRoutedTraceCensusLimits, DomainPowerSummary, FiniteRootAdmission, RootRegionInput,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const STARTS: usize = 2_557;
const MAX_SET_BYTES: usize = 512 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(28 * 60);

// Twice the complete corrected per-region sums, including reservation headroom.
// Seed/layer/storage and every native per-operation algebra limit stay unchanged.
const LIMITS: OwnerDomainWalkFiniteReplayLimits = OwnerDomainWalkFiniteReplayLimits {
    max_nodes: 2 * 1_215_215,
    max_rule_applications: 2 * 536_591,
    max_transport_calls: 2 * 383_579,
    max_transport_operations: 2 * 53_320_124,
    max_transport_endpoints: 2 * 11_576_012,
    max_coalescing_additions: 2 * 11_576_012,
    max_positive_layers: 64,
    max_seed_points: 1024,
    max_seed_bytes: 1024 * 1024,
};
const CENSUS: CandidateRoutedTraceCensusLimits = CandidateRoutedTraceCensusLimits {
    max_unique_selections: LIMITS.max_rule_applications,
    max_identity_payload_bytes: 128 * 1024 * 1024,
};

fn check(started: Instant, cancel: &AtomicBool) -> Result<()> {
    if started.elapsed() >= DEADLINE {
        cancel.store(true, Ordering::Release);
    }
    if cancel.load(Ordering::Acquire) {
        return Err("union diagnostic cancelled or deadline reached".into());
    }
    Ok(())
}

fn read(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("diagnostic input exceeds 4 MiB".into());
    }
    Ok(bytes)
}

fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn cpu_seconds() -> Result<f64> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: a valid out pointer and a constant process CPU clock identifier.
    if unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut time) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(time.tv_sec as f64 + time.tv_nsec as f64 * 1e-9)
}

fn executable_digest() -> Result<String> {
    let mut source = File::open(std::env::current_exe()?)?;
    let mut hash = blake3::Hasher::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            return Ok(hash.finalize().to_hex().to_string());
        }
        hash.update(&buffer[..count]);
    }
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    let mut file = BufWriter::new(OpenOptions::new().write(true).create_new(true).open(path)?);
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

#[derive(Default)]
struct Evidence {
    terminals: BTreeSet<IntegralKey>,
    zeros: BTreeSet<IntegralKey>,
    selections: BTreeMap<CandidateRoutedSelectionIdentity, u64>,
    totals: BTreeMap<&'static str, u64>,
    cases: Vec<Value>,
}

impl Evidence {
    fn absorb<const N: usize>(
        &mut self,
        report: &CandidateRoutedCampaignReport<N>,
        expected: usize,
        family: &str,
        label: &str,
        seconds: f64,
        cpu: f64,
    ) -> Result<()> {
        let s = report.snapshot();
        if !s.finished
            || s.queued_nodes != 0
            || s.active_nodes != 0
            || s.failed_nodes != 0
            || s.reserved_coalescing_additions != 0
            || s.first_failure.is_some()
            || s.missing_owners != 0
            || s.missing_rules != 0
            || s.scheduled_nodes != s.completed_nodes
            || !enumeration::complete_cardinality(s.input_targets, s.requested_targets, expected)
            || !report.trace().frontier().is_empty()
            || report.trace().family_fingerprint() != family
        {
            return Err(
                format!("{label}: incomplete, wrong-family or frontier-bearing trace").into(),
            );
        }
        let census = report
            .complete_census()
            .ok_or("missing complete native census")?;
        let applications = census
            .selections()
            .values()
            .try_fold(0u64, |sum, count| sum.checked_add(*count))
            .ok_or("native census multiplicity overflow")?;
        if applications != u64::try_from(s.rule_applications)? {
            return Err("native census does not account for every rule application".into());
        }
        // Admit conservative combined payload before cloning. BTree metadata is
        // separately bounded by these cardinalities and the outer RSS supervisor.
        let key_count = self
            .terminals
            .len()
            .checked_add(self.zeros.len())
            .and_then(|n| n.checked_add(report.trace().declared_terminals().len()))
            .and_then(|n| n.checked_add(report.trace().visited_zeros().len()))
            .ok_or("retained exact-set count overflow")?;
        let bytes = key_count
            .checked_mul(std::mem::size_of::<IntegralKey>() + N * 8)
            .ok_or("retained exact-set bytes overflow")?;
        if bytes > MAX_SET_BYTES {
            return Err("retained exact terminal/zero sets exceed diagnostic allowance".into());
        }
        self.terminals
            .extend(report.trace().declared_terminals().iter().cloned());
        self.zeros
            .extend(report.trace().visited_zeros().iter().cloned());
        for (identity, count) in census.selections() {
            if !self.selections.contains_key(identity)
                && self.selections.len() >= CENSUS.max_unique_selections
            {
                return Err("combined rule-identity set exceeds diagnostic allowance".into());
            }
            let stored = self.selections.entry(identity.clone()).or_default();
            *stored = stored
                .checked_add(*count)
                .ok_or("combined census multiplicity overflow")?;
        }
        for (name, count) in [
            ("input_targets", s.input_targets),
            ("requested_targets", s.requested_targets),
            ("operational_nodes", s.scheduled_nodes),
            ("physical_keys", s.reachable_integrals),
            ("deduplication_hits", s.deduplication_hits),
            ("rule_attempts", s.rule_attempts),
            ("rule_applications", s.rule_applications),
            ("transport_calls", s.transport_calls),
            ("transport_operations", s.transport_operations),
            ("transport_endpoints", s.transport_endpoints),
            ("coalescing_additions", s.coalescing_additions),
        ] {
            let total = self.totals.entry(name).or_default();
            *total = total
                .checked_add(u64::try_from(count)?)
                .ok_or("counter sum overflow")?;
        }
        self.cases.push(json!({"case":label,"wall_seconds":seconds,"process_cpu_seconds":cpu,"native":work_json(s,"completed"),
            "complete_census":true,"census_identities":census.selections().len(),
            "census_payload_bytes":census.identity_payload_bytes(),"census_multiplicity":applications}));
        Ok(())
    }

    fn json(&self) -> Value {
        json!({"terminals":self.terminals.iter().map(IntegralKey::powers).collect::<Vec<_>>(),
            "visited_zeros":self.zeros.iter().map(IntegralKey::powers).collect::<Vec<_>>(),
            "selections":self.selections.iter().map(|(key,count)| json!({"owner":key.owner,
                "batch_ordinal":key.batch_ordinal,"baseline_rule_ordinal":key.baseline_rule_ordinal,
                "selected_rule_ordinal":key.selected_rule_ordinal,"multiplicity":count})).collect::<Vec<_>>(),
            "totals":self.totals,"cases":self.cases})
    }
}

fn trace<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    targets: &[IntegralKey],
    admission: &FiniteRootAdmission<N>,
    started: Instant,
    cancel: &AtomicBool,
    evidence: &mut Evidence,
    label: &str,
) -> Result<()> {
    check(started, cancel)?;
    let cpu = cpu_seconds()?;
    let call = Instant::now();
    let report = reducer.trace_targets_inline_with_entry_admission_and_census_observer(
        targets.iter().cloned(),
        CandidateEntryAdmission::ExplicitFinite(admission),
        LIMITS.budget(),
        CENSUS,
        cancel,
        |_| {
            let _ = check(started, cancel);
        },
    )?;
    let seconds = call.elapsed().as_secs_f64();
    let cpu = cpu_seconds()? - cpu;
    check(started, cancel)?;
    evidence.absorb(
        &report,
        targets.len(),
        reducer.programs().context().family().fingerprint(),
        label,
        seconds,
        cpu,
    )?;
    println!(
        "union_diagnostic case={label} seconds={seconds:.6} inputs={} nodes={}",
        targets.len(),
        report.snapshot().scheduled_nodes
    );
    Ok(())
}

fn run<const N: usize>(
    request: &OwnerDomainWalkRequest,
    selection: &Selection,
    load_limits: CandidateOwnerLoadLimits,
    expected_payloads: &[String],
    started: Instant,
    cancel: &AtomicBool,
    output: &Path,
    receipt: &mut Value,
) -> Result<()> {
    let mut bounded = request.clone();
    bounded.finite_replay = Some(LIMITS);
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    load.owner_base = request.matching.owner_base.clone();
    load.workers = 4;
    load.reduction_limits = request.matching.reduction_limits;
    let original_expansion = load.trace_limits.expansion;
    configure_load(&bounded, &mut load);
    load.trace_limits.max_input_targets = STARTS;
    assert_eq!(load.trace_limits.expansion, original_expansion);
    assert_eq!(
        format!("{:?}", load.reduction_limits),
        format!("{:?}", request.matching.reduction_limits)
    );
    let mut digests = Vec::new();
    let preparation_cpu = cpu_seconds()?;
    let preparation = Instant::now();
    let reducer = prepare::prepare_with_fingerprints::<N>(
        &load,
        selection,
        load_limits,
        cancel,
        &|_| {
            let _ = check(started, cancel);
        },
        Some(&mut |payloads| {
            if payloads.as_slice() != expected_payloads {
                return Err("immutable payload/order binding changed".into());
            }
            digests = payloads;
            Ok(())
        }),
    )?
    .ok_or("native preparation cancelled")?;
    check(started, cancel)?;
    receipt["preparation_seconds"] = json!(preparation.elapsed().as_secs_f64());
    receipt["preparation_cpu_seconds"] = json!(cpu_seconds()? - preparation_cpu);
    receipt["payload_blake3"] = json!(digests);
    receipt["family_fingerprint"] = json!(reducer.programs().context().family().fingerprint());
    receipt["policy"] = budget::summary(
        LIMITS,
        reducer.limits(),
        reducer.programs().context().limits(),
    );
    receipt["per_operation_expansion"] = json!(format!("{original_expansion:?}"));
    let arity = reducer.programs().context().family().denominator_count();
    let enumeration_started = Instant::now();
    let queries = matching::input::parse(
        &request.matching.queries_json,
        arity,
        request.matching.max_queries,
        request.matching.max_query_bytes,
    )?;
    if queries.len() != 116 {
        return Err("expected complete 116-query input".into());
    }
    let mut rows = Vec::new();
    let mut domains = Vec::new();
    let mut empty_domains = Vec::new();
    let mut regions = Vec::new();
    let mut cases = Vec::new();
    let mut union = BTreeSet::new();
    let mut starts = 0usize;
    let mut empty = 0usize;
    let mut largest = 0usize;
    for query in &queries {
        check(started, cancel)?;
        let domain =
            query_domain(&reducer, request, query)?.ok_or("unresolved source-validity domain")?;
        let summary = DomainPowerSummary::try_new(
            domain.owner,
            &domain.lower,
            &domain.upper,
            domain.rank,
            domain.powers,
        )?;
        // Native tight geometry, with source support and phase retained. Empty
        // syntactically different rows on the same support remain accounted rows.
        let identity = (domain.phase, summary.clone());
        if !domains.contains(&identity) {
            domains.push(identity.clone());
        }
        let mut row = json!({"id":query.id,"auxiliary":query.auxiliary,"role_declared":query.role_declared,
            "domain":domain,"empty":summary.is_empty()});
        if summary.is_empty() {
            empty += 1;
            if !empty_domains.contains(&identity) {
                empty_domains.push(identity);
            }
            row["proof"] = json!("native DomainPowerSummary::is_empty");
        } else {
            let prepared = enumeration::prepare_with_arity(&domain, arity, LIMITS, STARTS, cancel)
                .map_err(|failure| {
                    format!(
                        "{}: native complete enumeration failed: {failure:?}",
                        query.id
                    )
                })?;
            let work = serde_json::to_value(&prepared.work)?;
            if work["exhausted"] != true
                || work["expected_seed_points"].as_u64() != Some(prepared.targets.len() as u64)
            {
                return Err("native enumeration did not exhaust its complete count".into());
            }
            starts = starts
                .checked_add(prepared.targets.len())
                .ok_or("input sum overflow")?;
            if starts > STARTS {
                return Err("whole diagnostic input allowance exceeded".into());
            }
            largest = largest.max(prepared.targets.len());
            for key in &prepared.targets {
                prepared.admission.validate_entry(key)?;
                union.insert(key.clone());
            }
            row["enumeration"] = work;
            row["keys"] = json!(
                prepared
                    .targets
                    .iter()
                    .map(IntegralKey::powers)
                    .collect::<Vec<_>>()
            );
            regions.push(RootRegionInput {
                support: domain.owner,
                lower: domain.lower,
                upper: domain.upper,
                rank: domain.rank,
                powers: domain.powers,
            });
            cases.push((query.id.clone(), prepared));
        }
        rows.push(row);
    }
    // Epoch initial admission reuses containing domains, including a previous
    // nonempty domain for an empty query in its phase/owner bucket. Its saved
    // root IDs therefore need not count equal native geometries. Every original
    // row is still checked here, independently of that bookkeeping/reuse.
    if (empty, cases.len(), starts, largest) != (35, 81, STARTS, 196) {
        return Err(format!("complete enumeration differs: empty={empty}, nonempty={}, starts={starts}, max={largest}",cases.len()).into());
    }
    let admission = FiniteRootAdmission::try_new(regions, 81)?;
    for (_, case) in &cases {
        for target in &case.targets {
            admission.validate_entry(target)?;
        }
    }
    let targets = union.into_iter().collect::<Vec<_>>();
    let input_bytes = (starts + targets.len())
        .checked_mul(std::mem::size_of::<IntegralKey>() + N * 8)
        .ok_or("combined input payload overflow")?;
    if input_bytes > 2 * 1024 * 1024 {
        return Err("combined input buffers exceed 2 MiB".into());
    }
    receipt["enumeration_seconds"] = json!(enumeration_started.elapsed().as_secs_f64());
    receipt["input"] = json!({"query_rows":116,"empty_rows":empty,"distinct_empty_geometries":empty_domains.len(),
        "distinct_geometries":domains.len(),"geometry_equivalence":"same phase and native DomainPowerSummary equality; not saved queue containment IDs",
        "nonempty_regions":cases.len(),"summed_starts":starts,
        "distinct_union_starts":targets.len(),"largest_region":largest,"retained_input_payload_bytes":input_bytes,
        "input_payload_excludes":"Vec/BTree allocator metadata and temporary enumeration storage", "regions":admission.region_count()});
    write_json(
        &output.join("enumeration.json"),
        &json!({"rows":rows,"union_targets":targets.iter().map(IntegralKey::powers).collect::<Vec<_>>()}),
    )?;
    let mut separate = Evidence::default();
    let cpu = cpu_seconds()?;
    let side = Instant::now();
    for (id, case) in &cases {
        trace(
            &reducer,
            &case.targets,
            &case.admission,
            started,
            cancel,
            &mut separate,
            id,
        )?;
    }
    receipt["separate_seconds"] = json!(side.elapsed().as_secs_f64());
    receipt["separate_cpu_seconds"] = json!(cpu_seconds()? - cpu);
    write_json(&output.join("separate.json"), &separate.json())?;
    let mut shared = Evidence::default();
    let cpu = cpu_seconds()?;
    let side = Instant::now();
    trace(
        &reducer,
        &targets,
        &admission,
        started,
        cancel,
        &mut shared,
        "exact_union",
    )?;
    receipt["shared_seconds"] = json!(side.elapsed().as_secs_f64());
    receipt["shared_cpu_seconds"] = json!(cpu_seconds()? - cpu);
    write_json(&output.join("shared.json"), &shared.json())?;
    check(started, cancel)?;
    let same = separate.terminals == shared.terminals
        && separate.zeros == shared.zeros
        && separate.selections.keys().eq(shared.selections.keys());
    receipt["exact_parity"] = json!({"terminals":separate.terminals == shared.terminals,
        "visited_zeros":separate.zeros == shared.zeros,"composite_rule_identity_sets":separate.selections.keys().eq(shared.selections.keys()),
        "multiplicities_compared":false});
    receipt["separate_totals"] = json!(separate.totals);
    receipt["shared_totals"] = json!(shared.totals);
    for evidence in [&separate, &shared] {
        let sum = evidence
            .selections
            .values()
            .try_fold(0u64, |sum, count| sum.checked_add(*count))
            .ok_or("total census overflow")?;
        if Some(&sum) != evidence.totals.get("rule_applications") {
            return Err("whole-side census accounting mismatch".into());
        }
    }
    if !same {
        return Err("exact shared/separate terminal, zero or rule-identity sets differ".into());
    }
    Ok(())
}

#[test]
#[ignore = "manual exact-union diagnostic; frozen full-R0 command and external resource supervisor required"]
fn complete_r0_union_matches_all_separate_native_traces() -> Result<()> {
    if std::env::var("RUSTRED_RUN_UNION_DIAGNOSTIC").as_deref() != Ok("1") {
        return Err("set RUSTRED_RUN_UNION_DIAGNOSTIC=1 under the external supervisor".into());
    }
    let started = Instant::now();
    let command_path = PathBuf::from(
        std::env::var_os("RUSTRED_UNION_DIAGNOSTIC_COMMAND")
            .ok_or("explicit frozen command required")?,
    );
    let output = PathBuf::from(
        std::env::var_os("RUSTRED_UNION_DIAGNOSTIC_OUTPUT")
            .ok_or("fresh diagnostic output directory required")?,
    );
    std::fs::create_dir(&output)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let term = signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&cancel))?;
    let interrupt = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&cancel))?;
    let mut receipt = json!({"schema":"rustred.exact-union-diagnostic.v1","status":"incomplete",
        "authority":"diagnostic only; no closure/publication/checkpoint or persistent reuse authority",
        "order":["separate","shared"],"trace_workers":1,"reserved_cpus":4,"deadline_seconds":DEADLINE.as_secs(),
        "rayon_threads":rayon::current_num_threads(),"rayon_num_threads_env":std::env::var("RAYON_NUM_THREADS").ok(),
        "test_source_blake3":digest(include_bytes!("union_diagnostic.rs")),
        "lockfile_blake3":digest(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../Cargo.lock")))});
    let outcome = (|| -> Result<()> {
        let directory = command_path.parent().ok_or("command has no directory")?;
        let command_bytes = read(&command_path)?;
        let plan_bytes = read(&directory.join("plan.json"))?;
        let cold_bytes = read(&directory.join("cold.json"))?;
        let command: Vec<String> = serde_json::from_slice(&command_bytes)?;
        let plan: Value = serde_json::from_slice(&plan_bytes)?;
        let cold: Value = serde_json::from_slice(&cold_bytes)?;
        if plan["walk"] != json!(command) || cold["verdict"] != "PASS" {
            return Err("frozen command/plan/reference mismatch".into());
        }
        let request =
            crate::cli::walk_request_from_argv(command.iter().map(OsString::from).collect())?;
        let binding = super::super::super::checkpoint::epoch_request_binding(&request);
        if cold["checkpoint"]["request_digest"] != binding
            || cold["queries"]["blake3"] != digest(request.matching.queries_json.as_bytes())
        {
            return Err("native request/query binding differs from full-R0 reference".into());
        }
        let seeds = request.finite_replay.ok_or("finite replay policy absent")?;
        if (
            seeds.max_seed_points,
            seeds.max_positive_layers,
            seeds.max_seed_bytes,
        ) != (1024, 64, 1024 * 1024)
        {
            return Err("reference per-region enumeration policy changed".into());
        }
        let payloads: Vec<String> =
            serde_json::from_value(cold["checkpoint"]["prepared_owner_payload_blake3"].clone())?;
        if payloads.len() != 69 {
            return Err("expected 69 immutable source payloads".into());
        }
        let (selection, arity, load_limits) = Selection::parse(&request.matching.selection_json)?;
        // This ignored diagnostic has one frozen fifteen-coordinate fixture.
        // Instantiate only its actual capacity, not every generic app capacity;
        // the family, original rows, and all native input policies are unchanged.
        if arity != 15 || rustred::campaign_storage_arity(arity) != 16 {
            return Err(
                "this frozen fixture requires physical arity 15 with selected storage 16".into(),
            );
        }
        receipt["bindings"] = json!({"command_path":command_path,"command_blake3":digest(&command_bytes),
            "plan_blake3":digest(&plan_bytes),"reference_blake3":digest(&cold_bytes),"native_request":binding,
            "selection_blake3":digest(request.matching.selection_json.as_bytes()),"queries_blake3":digest(request.matching.queries_json.as_bytes()),
            "executable_blake3":executable_digest()?});
        receipt["reference_queue_root_ids"] = cold["roots_total"].clone();
        receipt["reference_finite_root_records"] =
            cold["reinspection"]["tally"]["finite_replay"]["records"].clone();
        run::<16>(
            &request,
            &selection,
            load_limits,
            &payloads,
            started,
            &cancel,
            &output,
            &mut receipt,
        )
    })();
    signal_hook::low_level::unregister(term);
    signal_hook::low_level::unregister(interrupt);
    let outcome = outcome.and_then(|()| check(started, &cancel));
    receipt["total_seconds"] = json!(started.elapsed().as_secs_f64());
    receipt["cancelled"] = json!(cancel.load(Ordering::Acquire));
    receipt["status"] = json!(if outcome.is_ok() {
        "passed"
    } else {
        "incomplete"
    });
    receipt["error"] = json!(outcome.as_ref().err().map(ToString::to_string));
    write_json(&output.join("receipt.json"), &receipt)?;
    outcome
}
