//! Nominated coordinate-case research search with optional rules-only output.
//!
//! Usage: probe_owner_case --selection FILE --owner-base DIR --owner MASK
//!        --powers SIGNED,PHYSICAL,INDICES --numerical-depth 1
//! Use `*` for a free index and explicitly add `--search-rank none` for an
//! unbounded coordinate ray. `--replay` independently checks original sources.
//! `--inspect-installed` additionally checks a residual-free in-memory overlay;
//! it changes no saved rules, production process or campaign checkpoint.
//! `--output-overlay FILE` writes a new native partial payload only after exact
//! replay and with no finite residuals. A separate cold load is still required.
//! `--load-overlay FILE` cold-replays that payload without regenerating rules.
//! `--follow-repaired-targets CSV --follow-max-nodes COUNT` uses the full saved
//! selection and verified native routes for bounded concrete descendant checks.
//! The external caller must enforce a wall/RAM limit; native algebra is not
//! preemptible here. A stopped process has no successful diagnostic receipt.
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rustred::foundry::artifact::{SourcePortAuditError, SourcePortLimits};
use rustred::solver::{
    Case, CoordinateCase, FiniteCasePolicy, Integral, OwnerDomainAttemptLimits,
    OwnerDomainMatchDisposition, OwnerDomainMatchFailure, OwnerDomainScope, OwnerFeedbackError,
    OwnerFeedbackPolicy, SectorEvent, SectorSolveError, SolverError,
};
use rustred_app::{
    MAX_CANDIDATE_BUNDLE_BYTES, MAX_INPUT_BYTES, RoutedCampaignRequest, RoutedFeedbackOptions,
    RoutedFeedbackSession, encode_generated_domain_overlay, load_generated_domain_overlay,
};
use serde_json::{Value, json};

#[path = "probe_owner_case/follow.rs"]
mod follow;

type Result<T> = std::result::Result<T, String>;
const MAX_RULE_DETAILS: usize = 16;
const MAX_RHS_DETAILS: usize = 256;
const MAX_ERROR_CHARS: usize = 1024;

struct Args {
    selection: PathBuf,
    owner_base: PathBuf,
    owner: String,
    powers: Vec<Option<i16>>,
    numerical_depth: u32,
    replay: bool,
    inspect_installed: bool,
    output_overlay: Option<PathBuf>,
    load_overlay: Option<PathBuf>,
    follow_targets: Option<PathBuf>,
    follow_max_nodes: Option<usize>,
}

impl Args {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut args = args.into_iter();
        let mut values = BTreeMap::new();
        let mut replay = false;
        let mut inspect_installed = false;
        while let Some(key) = args.next() {
            if matches!(key.as_str(), "--replay" | "--inspect-installed") {
                let flag = if key == "--replay" {
                    &mut replay
                } else {
                    &mut inspect_installed
                };
                if *flag {
                    return Err(format!("duplicate {key}"));
                }
                *flag = true;
                continue;
            }
            if !matches!(
                key.as_str(),
                "--selection"
                    | "--owner-base"
                    | "--owner"
                    | "--powers"
                    | "--numerical-depth"
                    | "--search-rank"
                    | "--output-overlay"
                    | "--load-overlay"
                    | "--follow-repaired-targets"
                    | "--follow-max-nodes"
            ) {
                return Err("unknown option; expected selection, owner-base, owner, powers and numerical-depth".into());
            }
            let value = args.next().ok_or("missing option value")?;
            if value.is_empty() || value.len() > 4096 || value.starts_with("--") {
                return Err("missing or oversized option value".into());
            }
            if values.insert(key, value).is_some() {
                return Err("duplicate option".into());
            }
        }
        let search_rank = values.remove("--search-rank");
        let output_overlay = values.remove("--output-overlay").map(PathBuf::from);
        let load_overlay = values.remove("--load-overlay").map(PathBuf::from);
        let follow_targets = values
            .remove("--follow-repaired-targets")
            .map(PathBuf::from);
        let follow_max_nodes = values
            .remove("--follow-max-nodes")
            .map(|value| {
                value
                    .parse::<usize>()
                    .ok()
                    .filter(|&n| n > 0)
                    .ok_or("follow-max-nodes must be a positive integer")
            })
            .transpose()?;
        let mut take = |name| values.remove(name).ok_or_else(|| format!("missing {name}"));
        let selection = take("--selection")?.into();
        let owner_base = take("--owner-base")?.into();
        let owner = take("--owner")?;
        if !(1..=16).contains(&owner.len()) || !owner.bytes().all(|b| b == b'0' || b == b'1') {
            return Err("owner must have 1..16 original-axis bits".into());
        }
        let powers = take("--powers")?
            .split(',')
            .map(|part| match part.trim() {
                "*" => Ok(None),
                value => value.parse::<i16>().map(Some),
            })
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| "invalid signed physical powers")?;
        if powers.len() != owner.len() {
            return Err("power/owner arity mismatch".into());
        }
        for (&power, active) in powers.iter().zip(owner.bytes()) {
            if power.is_some_and(|power| (power > 0) != (active == b'1')) {
                return Err("physical powers do not match owner signs".into());
            }
        }
        if powers.iter().any(Option::is_none) {
            if search_rank.as_deref() != Some("none") {
                return Err("symbolic cases require explicit --search-rank none".into());
            }
        } else if search_rank.is_some() {
            return Err("fixed cases use their actual rank; omit --search-rank".into());
        }
        let numerical_depth = take("--numerical-depth")?
            .parse::<u32>()
            .map_err(|_| "invalid numerical depth")?;
        if numerical_depth == 0 {
            return Err("diagnostic requires explicit positive numerical depth".into());
        }
        if inspect_installed && !replay {
            return Err("--inspect-installed requires --replay".into());
        }
        if output_overlay.is_some() && !replay {
            return Err("--output-overlay requires --replay".into());
        }
        if load_overlay.is_some() && (!replay || output_overlay.is_some()) {
            return Err("--load-overlay requires --replay and excludes --output-overlay".into());
        }
        if follow_targets.is_some() != follow_max_nodes.is_some() {
            return Err(
                "follow target CSV and explicit --follow-max-nodes must be supplied together"
                    .into(),
            );
        }
        if follow_targets.is_some() && !(replay && inspect_installed) {
            return Err(
                "--follow-repaired-targets requires --replay and --inspect-installed".into(),
            );
        }
        Ok(Self {
            selection,
            owner_base,
            owner,
            powers,
            numerical_depth,
            replay,
            inspect_installed,
            output_overlay,
            load_overlay,
            follow_targets,
            follow_max_nodes,
        })
    }

    fn fixed_rank(&self) -> Result<u32> {
        self.powers.iter().flatten().try_fold(0u32, |rank, &power| {
            rank.checked_add(if power < 0 {
                (-i32::from(power)) as u32
            } else {
                0
            })
            .ok_or_else(|| "numerator rank overflow".into())
        })
    }

    fn rank(&self) -> Result<Option<u32>> {
        if self.powers.iter().any(Option::is_none) {
            Ok(None)
        } else {
            self.fixed_rank().map(Some)
        }
    }

    /// A concrete loader witness, NOT the nominated domain or its rank scope.
    fn setup_witness(&self) -> Vec<i16> {
        self.powers
            .iter()
            .zip(self.owner.bytes())
            .map(|(power, active)| power.unwrap_or(i16::from(active == b'1')))
            .collect()
    }
}

fn integral<const N: usize>(key: &Integral<N>) -> Value {
    json!({"values":key.powers().iter().map(|p|p.value()).collect::<Vec<_>>(),
        "symbolic":key.powers().iter().map(|p|p.is_symbolic()).collect::<Vec<_>>()})
}

fn error_category<const N: usize>(error: &OwnerFeedbackError<N>) -> &'static str {
    // Never format a possibly enormous symbolic error payload merely to
    // truncate it afterwards. Native errors are not classified as residuals.
    match error {
        OwnerFeedbackError::Candidate(_) => "candidate-binding-error",
        OwnerFeedbackError::Source(_) => "source-preparation-error",
        OwnerFeedbackError::Replay(_) => "source-replay-error",
        OwnerFeedbackError::Search(_) => "native-search-error",
        OwnerFeedbackError::InvalidInput(_) => "invalid-input",
        OwnerFeedbackError::ResourceLimit { .. } => "resource-limit",
    }
}

fn bounded_message(message: &str) -> String {
    message.chars().take(MAX_ERROR_CHARS).collect()
}

fn solver_error(error: &SolverError) -> Value {
    match error {
        SolverError::InvalidInput(message) => {
            json!({"kind":"invalid-input","message":bounded_message(message)})
        }
        SolverError::ExactReplay(message) => {
            json!({"kind":"exact-replay","message":bounded_message(message)})
        }
        SolverError::Certification(message) => {
            json!({"kind":"certification","message":bounded_message(message)})
        }
        SolverError::SearchExhausted { depth, rows } => {
            json!({"kind":"search-exhausted","depth":depth,"rows":rows})
        }
        SolverError::UnluckySample => json!({"kind":"unlucky-modular-sample"}),
        SolverError::Power(_) => json!({"kind":"compact-power-error"}),
    }
}

fn search_error<const N: usize>(error: &OwnerFeedbackError<N>) -> Value {
    let detail = match error {
        OwnerFeedbackError::Source(source) => solver_error(source),
        OwnerFeedbackError::Replay(source) => replay_error(source),
        OwnerFeedbackError::InvalidInput(message) => json!({"message":bounded_message(message)}),
        OwnerFeedbackError::ResourceLimit {
            resource,
            requested,
            limit,
        } => json!({"resource":resource,"requested":requested,"limit":limit}),
        OwnerFeedbackError::Search(source) => match source.as_ref() {
            SectorSolveError::Search { source, .. } | SectorSolveError::Numeric(source) => {
                solver_error(source)
            }
            SectorSolveError::CaseBudget { solved, pending } => {
                json!({"kind":"symbolic-case-budget","solved":solved,"pending":pending})
            }
            SectorSolveError::NonProgress { .. } => {
                json!({"kind":"rule-excludes-entire-current-case"})
            }
            SectorSolveError::Geometry { .. } => json!({"kind":"exceptional-geometry-error"}),
            SectorSolveError::Intersection(_) => {
                json!({"kind":"exceptional-conjunction-intersection-error"})
            }
            SectorSolveError::Exceptions { .. } => json!({"kind":"guard-extraction-error"}),
            SectorSolveError::FiniteRetention { .. } => json!({"kind":"finite-retention-error"}),
        },
        OwnerFeedbackError::Candidate(_) => json!({"kind":"candidate-binding-error"}),
    };
    json!({"category":error_category(error),"detail":detail})
}

fn replay_error(error: &SourcePortAuditError) -> Value {
    match error {
        SourcePortAuditError::Message(message) => {
            json!({"kind":"source-replay","message":bounded_message(message)})
        }
        SourcePortAuditError::ResourceBudgetExhausted { resource } => {
            json!({"kind":"source-replay-budget","resource":resource})
        }
        SourcePortAuditError::UnsupportedResourcePolicy {
            resource,
            requested,
            supported_max,
        } => {
            json!({"kind":"unsupported-replay-policy","resource":resource,"requested":requested,"supported_max":supported_max})
        }
        SourcePortAuditError::UnsupportedAffineOwnership { role, .. } => {
            json!({"kind":"unsupported-affine-replay","role":format!("{role:?}")})
        }
        SourcePortAuditError::UnprovedDescentObligation {
            term_ordinal,
            shift,
            local_lower,
            local_upper,
            child_sector,
        } => json!({"kind":"unproved-descent-obligation","term_ordinal":term_ordinal,
            "shift":shift,"local_lower":local_lower,"local_upper":local_upper,
            "child_sector":child_sector,"concrete_counterexample_claim":false}),
    }
}

fn numeric_rank<const N: usize>(key: &Integral<N>) -> Option<u32> {
    key.powers().iter().try_fold(0u32, |rank, power| {
        if power.is_symbolic() {
            None
        } else {
            rank.checked_add((-i32::from(power.value())).max(0) as u32)
        }
    })
}

fn match_failure(error: &OwnerDomainMatchFailure) -> Value {
    match error {
        OwnerDomainMatchFailure::UnknownOwner => json!({"kind":"unknown-owner"}),
        OwnerDomainMatchFailure::InvalidInput(message)
        | OwnerDomainMatchFailure::Geometry(message) => {
            json!({"kind":"domain-input-or-geometry","message":bounded_message(message)})
        }
        OwnerDomainMatchFailure::Cancelled => json!({"kind":"cancelled"}),
        OwnerDomainMatchFailure::StoppedByConsumer => json!({"kind":"stopped-by-consumer"}),
        OwnerDomainMatchFailure::ResourceLimit {
            resource,
            requested,
            limit,
        } => {
            json!({"kind":"domain-budget","resource":resource,"requested":requested,"limit":limit})
        }
        OwnerDomainMatchFailure::CountOverflow { resource } => {
            json!({"kind":"count-overflow","resource":resource})
        }
        OwnerDomainMatchFailure::AllocationFailure { resource } => {
            json!({"kind":"allocation-failure","resource":resource})
        }
        OwnerDomainMatchFailure::Algebra(_) => json!({"kind":"native-guard-algebra-error"}),
        OwnerDomainMatchFailure::PowerDomain(_) => json!({"kind":"power-domain-error"}),
    }
}

fn base_owner_digest(args: &Args, selection: &str) -> Result<[u8; 32]> {
    let selection: Value = serde_json::from_str(selection).map_err(|_| "invalid selection JSON")?;
    let record = selection["owners"]
        .as_array()
        .and_then(|owners| owners.iter().find(|owner| owner["mask"] == args.owner))
        .ok_or("nominated owner is absent from selection")?;
    let path = args
        .owner_base
        .join(record["path"].as_str().ok_or("owner path missing")?);
    let bytes = record["bytes"].as_u64().ok_or("owner byte size missing")?;
    if bytes > MAX_CANDIDATE_BUNDLE_BYTES as u64 {
        return Err("owner digest input exceeds native bundle allowance".into());
    }
    let mut file = File::open(path).map_err(|_| "cannot open base owner for digest")?;
    if file.metadata().map_err(|_| "cannot stat base owner")?.len() != bytes {
        return Err("base owner size differs from selection".into());
    }
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "cannot hash base owner")?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or("base owner size overflow")?;
        if total > bytes {
            return Err("base owner grew while hashing".into());
        }
        hasher.update(&buffer[..count]);
    }
    if total != bytes {
        return Err("base owner shrank while hashing".into());
    }
    Ok(*hasher.finalize().as_bytes())
}

/// Deliberate create-new publication: never replace another file. An interrupted
/// write can leave an incomplete file, which must not be advertised as usable.
fn write_new_native(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create new overlay: {error}"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| {
            format!("overlay write/sync failed; incomplete file may remain: {error}")
        })?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("overlay file written but directory sync failed: {error}"))
}

fn run<const N: usize>(args: Args, selection: String) -> Result<Value> {
    let started = Instant::now();
    let output_binding = args
        .output_overlay
        .as_ref()
        .map(|path| {
            if path
                .try_exists()
                .map_err(|_| "cannot inspect overlay output path")?
            {
                return Err("overlay output exists; refusing to overwrite".into());
            }
            base_owner_digest(&args, &selection).map(|digest| (path.clone(), digest))
        })
        .transpose()?;
    let load_binding = args
        .load_overlay
        .as_ref()
        .map(|path| base_owner_digest(&args, &selection).map(|digest| (path.clone(), digest)))
        .transpose()?;
    let powers: [Option<i16>; N] = args
        .powers
        .clone()
        .try_into()
        .map_err(|_| "arity dispatch mismatch")?;
    let case: Case<N> = CoordinateCase::new(powers)
        .map_err(|_| "physical power outside native compact range")?
        .into();
    let target = case.integral();
    let rank = args.rank()?;
    let fixed_rank = args.fixed_rank()?;
    let owner = std::array::from_fn(|axis| args.owner.as_bytes()[axis] == b'1');
    let follow_targets = args
        .follow_targets
        .as_ref()
        .map(|path| follow::read_targets(path, &owner, &powers))
        .transpose()?;
    let setup_witness = args.setup_witness();
    let csv = setup_witness
        .iter()
        .map(i16::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut request = RoutedCampaignRequest::new(selection, csv);
    request.owner_base = args.owner_base;
    request.workers = 1;
    if let Some(limit) = args.follow_max_nodes {
        request.trace_limits.max_unique_nodes = limit;
    }
    let policy = OwnerFeedbackPolicy {
        numerical_depth: args.numerical_depth,
        ..Default::default()
    };
    let options = RoutedFeedbackOptions::new(policy, FiniteCasePolicy::SearchFinite);
    let cancelled = AtomicBool::new(false);
    // Normal loader rebuilds the real family, ordinary+LI source context and
    // zero census. No graph trace or saved entry-rank admission is requested.
    let session = RoutedFeedbackSession::<N>::prepare(request, options, &cancelled, |_| {})
        .map_err(|error| format!("native preparation failed: {:?}", error.kind()))?
        .ok_or("native preparation cancelled")?;
    let prepared_seconds = started.elapsed().as_secs_f64();
    let programs = session.programs();
    let bound = programs
        .bind_owner_search(owner, policy)
        .map_err(|error| error_category(&error))?;
    let search_started = Instant::now();
    let mut progress_events = 0u64;
    let mut numerical_starts = 0u64;
    let mut cold_replay = None;
    let mut cold_load_seconds = None;
    let outcome = if let Some((path, digest)) = load_binding {
        let mut bytes = Vec::new();
        File::open(&path)
            .map_err(|_| "cannot open partial overlay input")?
            .take(MAX_CANDIDATE_BUNDLE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "cannot read partial overlay input")?;
        if bytes.len() > MAX_CANDIDATE_BUNDLE_BYTES {
            return Err("partial overlay input exceeds native bundle allowance".into());
        }
        let (overlay, audit) =
            load_generated_domain_overlay(programs, &bytes, owner, digest, Default::default())
                .map_err(|error| {
                    format!(
                        "cold overlay {}: {}",
                        error.kind().as_str(),
                        bounded_message(error.message())
                    )
                })?;
        if overlay.requested_cases() != [case.clone()]
            || overlay.scope().max_numerator_rank != rank
            || overlay.policy().numerical_depth != args.numerical_depth
        {
            return Err(
                "cold overlay nominated case, rank scope or numerical depth differs from request"
                    .into(),
            );
        }
        cold_load_seconds = Some(search_started.elapsed().as_secs_f64());
        cold_replay = Some(audit);
        Ok(overlay)
    } else {
        bound.solve_domains_with_observer(
            vec![case],
            OwnerDomainScope {
                max_numerator_rank: rank,
                finite_case_policy: FiniteCasePolicy::SearchFinite,
            },
            OwnerDomainAttemptLimits {
                max_requested_cases: 1,
                max_symbolic_cases: Some(32),
                ..Default::default()
            },
            |event| {
                progress_events = progress_events.saturating_add(1);
                if matches!(event, SectorEvent::NumericalStarted { .. }) {
                    numerical_starts = numerical_starts.saturating_add(1);
                }
            },
        )
    };
    let mut report = json!({
        "schema":"rustred.nominated-owner-case-diagnostic.v1",
        "status":"native-error", "owner":args.owner, "powers":args.powers,
        "actual_numerator_rank":rank,"search_rank_scope":rank,
        "fixed_coordinate_rank_lower_bound":fixed_rank,
        "free_axes":args.powers.iter().enumerate().filter_map(|(i,p)|p.is_none().then_some(i)).collect::<Vec<_>>(),
        "setup_witness":setup_witness,"setup_witness_nominates_domain":false,
        "saved_entry_rank_scope":programs.context().scope().max_numerator_rank,
        "family_fingerprint_blake3":blake3::hash(programs.context().family().fingerprint().as_bytes()).to_hex().to_string(),
        "prospective_policy":{"source_order":"input-order/default, NOT historical A1",
            "numerical_depth":args.numerical_depth,"finite_case_policy":"search-finite",
            "symbolic_exact_backend":"sparse/default", "numerical_exact_backend":"sparse/default",
            "coefficient_variable_order":"original/default", "symbolic_case_limit":32},
        "mode":if cold_replay.is_some(){"cold-overlay-load"}else{"new-source-search"},
        "preparation_seconds":prepared_seconds,"search_seconds":if cold_replay.is_some(){0.0}else{search_started.elapsed().as_secs_f64()},
        "cold_load_seconds":cold_load_seconds,
        "elapsed_seconds":started.elapsed().as_secs_f64(),"progress_events":progress_events,
        "numerical_starts":numerical_starts,"complete_sector_claim":false,"family_closure_claim":false,
        "independent_source_replay_requested":args.replay,
        "independent_source_replay_performed":false,"independent_source_replay_passed":false,
        "independent_source_replay":null,"overlay_appended":false,"overlay_persisted":false,
        "installed_domain_inspection_requested":args.inspect_installed,
        "installed_domain_inspection":null,
        "overlay_output":null,
        "follow_repaired_targets":null,
        "checkpoint_written":false,"terminal_declaration":false,
        "raw_coefficients_omitted":true,"native_error":null
    });
    let overlay = match outcome {
        Ok(overlay) => overlay,
        Err(error) => {
            report["native_error"] = search_error(&error);
            return Ok(report);
        }
    };
    if cold_replay.is_some() {
        report["prospective_policy"] = Value::Null;
        report["loaded_policy"] = json!({"numerical_depth":overlay.policy().numerical_depth,
            "symbolic_exact_backend":format!("{:?}",overlay.policy().symbolic_exact_backend),
            "numerical_exact_backend":format!("{:?}",overlay.policy().numerical_exact_backend),
            "coefficient_variable_order":format!("{:?}",overlay.policy().coefficient_variable_order)});
    }
    let solution = overlay.partial_solution();
    let stats = overlay.stats();
    let native_order = solution
        .order
        .persisted_policy()
        .map_err(|_| "partial result has no persisted native order")?
        .stable_id()
        .to_string();
    let direct = solution
        .rules
        .iter()
        .filter(|rule| rule.candidate.target == target)
        .count();
    let residual = solution.finite_residuals.contains(&target);
    report["status"] = if residual {
        "partial-search-finished-target-retained"
    } else if direct > 0 {
        "partial-search-finished-direct-target-rule"
    } else {
        "partial-search-finished-no-direct-target-rule"
    }
    .into();
    report["partial_result"] = json!({
        "statistics_source":if cold_load_seconds.is_some(){"not-persisted/zero-default"}else{"current-search"},
        "requested_cases":overlay.requested_cases().len(),"rules":overlay.rule_count(),
        "finite_residuals":overlay.terminal_count(),"direct_target_rules":direct,
        "target_retained_as_residual":residual,
        "native_order":native_order,
        "rule_details_truncated":solution.rules.len()>MAX_RULE_DETAILS,
        "rule_details":solution.rules.iter().take(MAX_RULE_DETAILS).map(|rule|json!({
            "target":integral(&rule.candidate.target),"is_requested_target":rule.candidate.target==target,
            "rhs_terms":rule.candidate.rhs.len(),"rhs_keys_truncated":rule.candidate.rhs.len()>MAX_RHS_DETAILS,
            "rhs_keys":rule.candidate.rhs.iter().take(MAX_RHS_DETAILS).map(|term|integral(&term.integral)).collect::<Vec<_>>(),
            "numeric_rhs_terms":rule.candidate.rhs.iter().filter(|term|numeric_rank(&term.integral).is_some()).count(),
            "maximum_numeric_rhs_rank":rule.candidate.rhs.iter().filter_map(|term|numeric_rank(&term.integral)).max(),
            "exception_branches":rule.exceptions.branches.len(),"source_trace_rows":rule.candidate.sources.len()
        })).collect::<Vec<_>>(),
        "residual_keys_truncated":solution.finite_residuals.len()>MAX_RULE_DETAILS,
        "residual_keys":solution.finite_residuals.iter().take(MAX_RULE_DETAILS).map(integral).collect::<Vec<_>>(),
        "symbolic_cases":stats.symbolic_cases,"numerical_cases":stats.numerical_cases,
        "symbolic_rows":stats.symbolic_rows,"symbolic_search_seconds":stats.symbolic_search.as_secs_f64(),
        "numerical_search_seconds":stats.numerical_search.as_secs_f64(),
        "exception_extraction_seconds":stats.exception_extraction.as_secs_f64(),
        "geometry_seconds":stats.geometry.as_secs_f64()
    });
    if args.replay {
        let replay_started = Instant::now();
        let replay = match cold_replay.take() {
            Some(audit) => Ok(audit),
            None => bound.replay_overlay_rules(&overlay, SourcePortLimits::default()),
        };
        report["independent_source_replay_performed"] = true.into();
        report["independent_source_replay_seconds"] = cold_load_seconds
            .unwrap_or_else(|| replay_started.elapsed().as_secs_f64())
            .into();
        report["replay_timing_includes_cold_decode"] = cold_load_seconds.is_some().into();
        match replay {
            Ok(audit) => {
                report["independent_source_replay_passed"] = true.into();
                report["independent_source_replay"] = json!({
                    "rules":audit.rules.len(),"replay_core_seconds":audit.elapsed.as_secs_f64(),
                    "rule_details_truncated":audit.rules.len()>MAX_RULE_DETAILS,
                    "rule_details":audit.rules.iter().take(MAX_RULE_DETAILS).map(|rule|json!({
                        "ordinal":rule.ordinal,"original_source_entries":rule.original_source_entries
                    })).collect::<Vec<_>>(),
                    "scope":"returned-rule-replay-subreport","identity_and_guards_only":true,
                    "strict_descent_verified_by_cold_loader":cold_load_seconds.is_some(),
                    "recursive_successor_closure_claim":false
                });
            }
            Err(error) => {
                report["status"] = "source-replay-failed".into();
                report["native_error"] = replay_error(&error);
            }
        }
    }
    if let Some((path, base_digest)) = output_binding {
        let reason = if report["independent_source_replay_passed"] != true {
            Some("independent-source-replay-not-passed")
        } else if overlay.rule_count() == 0 {
            Some("no-new-rules")
        } else if overlay.terminal_count() != 0 {
            Some("finite-residuals-must-not-become-terminals")
        } else {
            None
        };
        if let Some(reason) = reason {
            report["overlay_output"] = json!({"status":"not-written","reason":reason});
        } else {
            let output_started = Instant::now();
            let encoded = encode_generated_domain_overlay(
                programs,
                &overlay,
                base_digest,
                Default::default(),
            );
            match encoded {
                Err(error) => {
                    report["status"] = "overlay-encoding-failed".into();
                    report["native_error"] = json!({"kind":error.kind().as_str(),"message":bounded_message(error.message())});
                }
                Ok(bytes) => match write_new_native(&path, &bytes) {
                    Err(error) => {
                        report["status"] = "overlay-output-failed".into();
                        report["native_error"] =
                            json!({"kind":"overlay-output-io","message":bounded_message(&error)});
                    }
                    Ok(()) => {
                        report["overlay_persisted"] = true.into();
                        report["overlay_output"] = json!({"status":"native-partial-written","path":path,
                            "bytes":bytes.len(),"blake3":blake3::hash(&bytes).to_hex().to_string(),
                            "base_owner_blake3":blake3::Hash::from_bytes(base_digest).to_hex().to_string(),
                            "rules":overlay.rule_count(),"finite_residuals":0,
                            "seconds":output_started.elapsed().as_secs_f64(),"cold_loaded":false,
                            "complete_sector_claim":false,"recursive_successor_closure_claim":false});
                    }
                },
            }
        }
    }
    if args.inspect_installed {
        let reason = if report["independent_source_replay_passed"] != true {
            Some("independent-source-replay-not-passed")
        } else if overlay.rule_count() == 0 {
            Some("no-new-rules")
        } else if overlay.terminal_count() != 0 {
            Some("finite-residuals-must-not-become-terminals")
        } else {
            None
        };
        if let Some(reason) = reason {
            report["installed_domain_inspection"] =
                json!({"status":"not-installed","reason":reason});
        } else {
            let before_terminals = programs.terminal_count();
            let installed =
                programs.append_residual_free_domain_overlays(vec![overlay], Default::default());
            match installed {
                Err(error) => {
                    report["status"] = "in-memory-overlay-install-failed".into();
                    report["native_error"] = search_error(&error);
                }
                Ok(installed) => {
                    report["overlay_appended"] = true.into();
                    let lower: Vec<_> = powers
                        .iter()
                        .zip(owner)
                        .map(|(power, active)| {
                            power.map_or(0, |power| {
                                if active {
                                    (i32::from(power) - 1) as u64
                                } else {
                                    (-i32::from(power)) as u64
                                }
                            })
                        })
                        .collect();
                    let upper: Vec<_> = powers
                        .iter()
                        .zip(&lower)
                        .map(|(power, lower)| power.map(|_| *lower))
                        .collect();
                    let mut counts = BTreeMap::<&str, u64>::new();
                    let mut pieces = Vec::new();
                    let inspect_started = Instant::now();
                    let inspection = installed.visit_owner_domain_matches(owner, &lower, &upper, rank,
                        Default::default(), &cancelled, |piece| {
                            let (kind, disposition) = match piece.disposition() {
                                OwnerDomainMatchDisposition::SelectedRule { batch, rule } => ("selected_rule",json!({"kind":"selected_rule","batch":batch,"rule":rule})),
                                OwnerDomainMatchDisposition::Terminal { batch } => ("terminal",json!({"kind":"terminal","batch":batch})),
                                OwnerDomainMatchDisposition::ExactGap => ("exact_gap",json!({"kind":"exact_gap"})),
                                OwnerDomainMatchDisposition::ExactZeroSector => ("exact_zero_sector",json!({"kind":"exact_zero_sector"})),
                                OwnerDomainMatchDisposition::Unresolved { predicate } => ("unresolved",json!({"kind":"unresolved","predicate":format!("{predicate:?}")})),
                                OwnerDomainMatchDisposition::InvalidSourceCondition { ordinal } => ("invalid_source_condition",json!({"kind":"invalid_source_condition","ordinal":ordinal})),
                            };
                            *counts.entry(kind).or_default() += 1;
                            if pieces.len() < MAX_RULE_DETAILS {
                                pieces.push(json!({"lower":piece.lower(),"upper":piece.upper(),"disposition":disposition}));
                            }
                            ControlFlow::Continue(())
                        });
                    let after_terminals = installed.terminal_count();
                    let complete = inspection.is_ok();
                    let (stats, failure) = match inspection {
                        Ok(stats) => (stats, Value::Null),
                        Err(error) => (error.stats, match_failure(&error.failure)),
                    };
                    report["installed_domain_inspection"] = json!({
                        "status":if complete {"classification-complete"} else {"classification-incomplete"},
                        "seconds":inspect_started.elapsed().as_secs_f64(),"lower":lower,"upper":upper,
                        "rank_scope":rank,"counts":counts,"pieces":pieces,"pieces_truncated":stats.pieces>MAX_RULE_DETAILS,
                        "terminal_count_before":before_terminals,"terminal_count_after":after_terminals,
                        "terminal_count_unchanged":before_terminals==after_terminals,
                        "stats":{"rules":stats.rules,"pieces":stats.pieces,"predicates":stats.predicates,"cells":stats.cells},
                        "failure":failure,"memory_only":true,"recursive_successor_closure_claim":false
                    });
                    if before_terminals != after_terminals {
                        report["status"] = "in-memory-overlay-terminal-count-changed".into();
                        report["native_error"] = json!({"kind":"residual-free-overlay-invariant"});
                    } else if let Some(targets) = follow_targets {
                        report["follow_repaired_targets"] = follow::inspect_and_follow(
                            session.routed_reducer(),
                            installed,
                            targets,
                            &cancelled,
                        )?;
                    }
                }
            }
        }
    }
    report["elapsed_seconds"] = started.elapsed().as_secs_f64().into();
    Ok(report)
}

fn main() {
    let result = (|| {
        let args = Args::parse(std::env::args().skip(1))?;
        let mut bytes = Vec::new();
        File::open(&args.selection)
            .map_err(|_| "cannot open selection")?
            .take(MAX_INPUT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "cannot read selection")?;
        if bytes.len() > MAX_INPUT_BYTES {
            return Err("selection exceeds input byte limit".into());
        }
        let selection = String::from_utf8(bytes).map_err(|_| "selection is not UTF-8")?;
        let scope: Value =
            serde_json::from_str(&selection).map_err(|_| "invalid selection JSON")?;
        let owners = scope["owners"]
            .as_array()
            .ok_or("selection owners missing")?;
        let routes = scope["initial_frontier_routes"]
            .as_array()
            .ok_or("selection routes missing")?;
        if owners
            .iter()
            .filter(|owner| owner["mask"] == args.owner)
            .count()
            != 1
        {
            return Err("selection must contain exactly one nominated owner".into());
        }
        if args.follow_targets.is_none() && (owners.len() != 1 || !routes.is_empty()) {
            return Err(
                "multi-owner selection/routes require explicit --follow-repaired-targets".into(),
            );
        }
        macro_rules! dispatch { ($($n:literal),+) => { match args.owner.len() {
            $($n => run::<$n>(args, selection),)+ _ => unreachable!(),
        } }; }
        dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
    })();
    let (value, failed) = match result {
        Ok(value) => {
            let failed = value["native_error"] != Value::Null;
            (value, failed)
        }
        Err(error) => (
            json!({"schema":"rustred.nominated-owner-case-diagnostic.v1",
            "status":"setup-error","message":error,"family_closure_claim":false}),
            true,
        ),
    };
    let mut output = std::io::stdout().lock();
    if serde_json::to_writer(&mut output, &value).is_err() || writeln!(output).is_err() {
        std::process::exit(3);
    }
    if failed {
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(owner: &str, powers: &str, depth: &str) -> Vec<String> {
        [
            "--selection",
            "input.json",
            "--owner-base",
            ".",
            "--owner",
            owner,
            "--powers",
            powers,
            "--numerical-depth",
            depth,
        ]
        .map(str::to_owned)
        .to_vec()
    }
    #[test]
    fn nominated_physical_scope_is_explicit_and_checked() {
        let request = Args::parse(args("010", "-10,2,-1", "1")).unwrap();
        assert_eq!(request.rank().unwrap(), Some(11));
        assert_eq!(request.numerical_depth, 1);
        for input in [
            args("010", "-10,0,-1", "1"),
            args("010", "-10,2", "1"),
            args("010", "-10,2,-1", "0"),
            args("x", "0", "1"),
        ] {
            assert!(Args::parse(input).is_err());
        }
        let mut duplicate = args("1", "1", "1");
        duplicate.extend(["--numerical-depth".into(), "2".into()]);
        assert!(Args::parse(duplicate).is_err());
        assert_eq!(
            Args::parse(args("0", "-32768", "1"))
                .unwrap()
                .rank()
                .unwrap(),
            Some(32768)
        );
        assert!(CoordinateCase::new([Some(-32768)]).is_err());
        let mut inspect_without_replay = args("1", "1", "2");
        inspect_without_replay.push("--inspect-installed".into());
        assert!(Args::parse(inspect_without_replay.clone()).is_err());
        inspect_without_replay.push("--replay".into());
        assert!(
            Args::parse(inspect_without_replay)
                .unwrap()
                .inspect_installed
        );
        let mut output_without_replay = args("1", "1", "2");
        output_without_replay.extend(["--output-overlay".into(), "new.rrbin".into()]);
        assert!(Args::parse(output_without_replay.clone()).is_err());
        output_without_replay.push("--replay".into());
        assert_eq!(
            Args::parse(output_without_replay).unwrap().output_overlay,
            Some(PathBuf::from("new.rrbin"))
        );
    }

    #[test]
    fn cold_load_and_follow_controls_are_explicit() {
        let mut load = args("1", "2", "1");
        load.extend(["--load-overlay".into(), "repair.rrbin".into()]);
        assert!(Args::parse(load.clone()).is_err());
        load.push("--replay".into());
        assert!(Args::parse(load.clone()).unwrap().load_overlay.is_some());
        load.extend(["--output-overlay".into(), "copy.rrbin".into()]);
        assert!(Args::parse(load).is_err());
        let mut follow = args("1", "2", "1");
        follow.extend([
            "--follow-repaired-targets".into(),
            "targets.csv".into(),
            "--replay".into(),
            "--inspect-installed".into(),
        ]);
        assert!(Args::parse(follow.clone()).is_err());
        follow.extend(["--follow-max-nodes".into(), "1000".into()]);
        assert_eq!(Args::parse(follow).unwrap().follow_max_nodes, Some(1000));
    }

    #[test]
    fn symbolic_coordinate_requires_explicit_unbounded_scope() {
        let mut input = args("010", "*,2,-1", "2");
        assert!(Args::parse(input.clone()).is_err());
        input.extend(["--search-rank".into(), "none".into(), "--replay".into()]);
        let request = Args::parse(input).unwrap();
        assert_eq!(request.rank().unwrap(), None);
        assert_eq!(request.fixed_rank().unwrap(), 1);
        assert_eq!(request.powers, vec![None, Some(2), Some(-1)]);
        assert_eq!(request.setup_witness(), vec![0, 2, -1]);
        assert!(request.replay);
        let case = CoordinateCase::new([None, Some(2), Some(-1)]).unwrap();
        assert!(case.integral().powers()[0].is_symbolic());
        for value in ["10", "0", "unbounded"] {
            let mut input = args("010", "*,2,-1", "2");
            input.extend(["--search-rank".into(), value.into()]);
            assert!(Args::parse(input).is_err());
        }
        let request = Args::parse({
            let mut input = args("110", "*,2,-1", "2");
            input.extend(["--search-rank".into(), "none".into()]);
            input
        })
        .unwrap();
        assert_eq!(request.setup_witness(), vec![1, 2, -1]);
        assert_eq!(request.powers[0], None);
    }
}
