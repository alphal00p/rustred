//! Finite symbolic source projection with conservative failed-descent refinement.
//! Native Symbolica owns algebra; existing source/chart proof and exporter own
//! authority. No sampled-weight authority, source-bank growth, chart change or walk.
#[path = "symbolic_projector/boundary.rs"]
mod boundary;
#[path = "symbolic_projector/boundary_correction.rs"]
mod boundary_correction;
#[path = "symbolic_projector/certificate.rs"]
mod certificate;
#[path = "symbolic_projector/cofinal.rs"]
mod cofinal;
#[path = "symbolic_projector/direct_l.rs"]
mod direct_l;
#[path = "symbolic_projector/endpoint_locality.rs"]
mod endpoint_locality;
#[path = "symbolic_projector/exact_dual.rs"]
mod exact_dual;
#[path = "symbolic_projector/inspect.rs"]
mod inspect;
#[path = "symbolic_projector/nominate.rs"]
mod nominate;
#[path = "symbolic_projector/positive_power_envelope.rs"]
mod positive_power_envelope;
#[path = "symbolic_projector/progress.rs"]
mod progress;
#[path = "symbolic_projector/project.rs"]
mod project;
#[path = "symbolic_projector/reconstructed.rs"]
mod reconstructed;
#[path = "symbolic_projector/refine.rs"]
mod refine;
#[path = "symbolic_projector/root_policy.rs"]
mod root_policy;
#[path = "symbolic_projector/selection.rs"]
mod selection;
#[path = "symbolic_projector/source.rs"]
mod source;
#[path = "symbolic_projector/source_obstruction.rs"]
mod source_obstruction;
#[path = "symbolic_projector/source_preimage.rs"]
mod source_preimage;
#[path = "symbolic_projector/support_inspect.rs"]
mod support_inspect;
#[cfg(test)]
#[path = "symbolic_projector/tests.rs"]
mod tests;
#[path = "symbolic_projector/trace.rs"]
mod trace;
use rustred::{
    algebra::{ExactAlgebraLimits, IndexedAlgebraLimits},
    foundry::{
        artifact::{
            OriginalSourceCombinationLimits, OriginalSourceCombinationRequest,
            SourcePortAuditError, check_original_source_combination,
        },
        cell::FixedIndexRestriction,
    },
    identity::{
        IndexShift, IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits,
        TranslatedSourceRequest,
    },
    sector::Mask,
    solver::RuleDispatchPolicy,
};
use rustred_app::{
    CandidateOwnerBundle, CandidateOwnerLoadLimits, encode_checked_priority_owner_with_policy,
    inspect_generated_candidate_bundle, load_generated_candidate_owners,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::Arc,
    time::Instant,
};
type Result<T> = std::result::Result<T, String>;
const SCHEMA: &str = "rustred.symbolic-source-projector.v1";
fn checked<T, E: Display>(v: std::result::Result<T, E>) -> Result<T> {
    v.map_err(|e| e.to_string())
}
fn require(b: bool, message: &str) -> Result<()> {
    if b { Ok(()) } else { Err(message.into()) }
}
fn number(v: &Value, k: &str) -> Result<usize> {
    v[k].as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("invalid {k}"))
}
fn limit(r: &Value, k: &str) -> Result<usize> {
    let n = number(&r["limits"], k)?;
    require(n > 0, "limits must be positive")?;
    Ok(n)
}
fn array<'a>(v: &'a Value, k: &str) -> Result<&'a [Value]> {
    v[k].as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{k} must be an array"))
}
fn integers(v: &Value, n: usize) -> Result<Vec<i64>> {
    let a = v.as_array().ok_or("integer vector required")?;
    require(a.len() == n, "vector arity mismatch")?;
    a.iter()
        .map(|x| x.as_i64().ok_or_else(|| "signed integer required".into()))
        .collect()
}
fn fixed(r: &Value) -> Result<Vec<(usize, i64)>> {
    array(&r["chart"], "fixed")?
        .iter()
        .map(|v| {
            let p = integers(v, 2)?;
            Ok((checked(usize::try_from(p[0]))?, p[1]))
        })
        .collect()
}

fn compact_coefficients(r: &Value) -> Result<bool> {
    r.get("compact_coefficient_variables")
        .map_or(Ok(false), |value| {
            value
                .as_bool()
                .ok_or_else(|| "compact_coefficient_variables must be boolean".into())
        })
}
fn owner_limits(r: &Value) -> Result<CandidateOwnerLoadLimits> {
    let v = &r["owner_load_limits"];
    let mut p = CandidateOwnerLoadLimits::default();
    p.bundle.max_bundle_bytes = number(v, "max_bundle_bytes")?;
    p.max_total_input_bytes = number(v, "max_total_input_bytes")?;
    p.bundle.max_total_coefficient_bytes = number(v, "max_total_coefficient_bytes")?;
    p.bundle.max_collection_entries = number(v, "max_collection_entries")?;
    p.max_total_symbolica_state_bytes = number(v, "max_total_symbolica_state_bytes")?;
    p.max_zero_sector_visits = number(v, "max_zero_sector_visits")?;
    p.bundle.max_coefficient_bytes = number(v, "max_coefficient_bytes")?;
    require(
        p.bundle.max_bundle_bytes <= rustred_app::MAX_CANDIDATE_BUNDLE_BYTES,
        "owner exceeds native transport ceiling",
    )?;
    for k in [
        "max_bundle_bytes",
        "max_total_input_bytes",
        "max_total_coefficient_bytes",
        "max_collection_entries",
        "max_total_symbolica_state_bytes",
        "max_zero_sector_visits",
        "max_coefficient_bytes",
    ] {
        require(number(v, k)? > 0, "positive owner ingress limits required")?;
    }
    Ok(p)
}
fn policy(r: &Value) -> Result<OriginalSourceCombinationLimits> {
    let exact = ExactAlgebraLimits {
        max_exponent: checked(u16::try_from(limit(r, "max_exponent")?))?,
        max_polynomial_terms: limit(r, "max_polynomial_terms")?,
        max_term_operations: limit(r, "max_term_operations")?,
    };
    let arithmetic = IndexedAlgebraLimits {
        exact_algebra: exact,
        max_specialization_power_operations: limit(r, "max_term_operations")?,
        max_specialization_integer_bits: limit(r, "max_term_operations")?,
    };
    let mut p = OriginalSourceCombinationLimits::default();
    p.source_generation.relation_limits.arithmetic = arithmetic;
    p.translated_sources = TranslatedSourceLimits {
        max_requested_source_translations: limit(r, "max_source_rows")?,
        max_requested_offsets: limit(r, "max_source_rows")?,
        max_translated_sources: limit(r, "max_source_rows")?,
        max_translated_term_entries: limit(r, "max_terms")?,
        max_translated_condition_entries: limit(r, "max_conditions")?,
        max_retained_condition_source_entries: limit(r, "max_conditions")?,
        max_retained_index_coordinate_cells: limit(r, "max_coordinate_cells")?,
        ..Default::default()
    };
    p.translated_sources.relation.arithmetic = arithmetic;
    p.rule.indexed_algebra = arithmetic;
    p.rule.max_source_rows = limit(r, "max_complete_source_rows")?;
    p.rule.max_source_combination_terms = limit(r, "max_source_rows")?;
    p.rule.max_replay_exact_operations = limit(r, "max_term_operations")?;
    p.rule.max_rule_guards = limit(r, "max_conditions")?;
    p.rule.max_shift_columns = limit(r, "max_terms")?;
    if r["limits"].get("max_domain_bound_endpoint_cells").is_some() {
        p.rule.max_domain_bound_endpoint_cells = limit(r, "max_domain_bound_endpoint_cells")?;
    }
    p.cell.indexed_algebra = arithmetic;
    p.cell.relation.arithmetic = arithmetic;
    p.cell.max_source_views = limit(r, "max_source_rows")?;
    p.cell.max_retained_terms = limit(r, "max_terms")?;
    p.cell.max_guards = limit(r, "max_conditions")?;
    p.cell.max_fixed_restrictions = r["owner_mask"].as_str().ok_or("mask required")?.len();
    p.cell.max_guard_split_coordinate_cells = limit(r, "max_coordinate_cells")?;
    p.geometry.max_requested_boxes = limit(r, "max_cells")?;
    p.geometry.max_uncovered_boxes = limit(r, "max_cells")?;
    p.geometry.max_requested_box_coordinate_cells = limit(r, "max_coordinate_cells")?;
    p.geometry.max_uncovered_box_coordinate_cells = limit(r, "max_coordinate_cells")?;
    p.geometry.max_split_operations = limit(r, "max_term_operations")?;
    Ok(p)
}
fn projection_limits(r: &Value) -> Result<project::Limits> {
    Ok(project::Limits {
        arithmetic: policy(r)?.cell.indexed_algebra.exact_algebra,
        rows: limit(r, "max_source_rows")?,
        columns: limit(r, "max_augmented_columns")?,
        nonzeros: limit(r, "max_matrix_nonzeros")?,
        coefficient_terms: limit(r, "max_coefficient_terms")?,
        operations: limit(r, "max_term_operations")?,
        guards: limit(r, "max_conditions")?,
    })
}

fn validate(r: &Value) -> Result<usize> {
    require(r["schema"] == SCHEMA, "unknown symbolic projection schema")?;
    source_obstruction::validate(r)?;
    trace::detail(r)?;
    root_policy::enabled(r)?;
    cofinal::enabled(r)?;
    positive_power_envelope::cap(r)?;
    if !reconstructed::enabled(r) {
        direct_l::enabled(r)?;
    }
    reconstructed::validate(r)?;
    let mask = r["owner_mask"].as_str().ok_or("owner_mask required")?;
    let n = mask.len();
    require(
        (1..=16).contains(&n) && mask.bytes().all(|b| b == b'0' || b == b'1'),
        "invalid owner mask/arity",
    )?;
    endpoint_locality::Policy::parse(r, n)?;
    for k in ["family_fingerprint", "expected_order"] {
        require(
            r[k].as_str().is_some_and(|s| !s.is_empty()),
            "family/order binding required",
        )?;
    }
    for k in [
        "max_owner_bytes",
        "max_output_bytes",
        "max_source_rows",
        "max_complete_source_rows",
        "max_terms",
        "max_conditions",
        "max_coordinate_cells",
        "max_cells",
        "max_polynomial_terms",
        "max_term_operations",
        "max_exponent",
        "max_report_bytes",
        "max_augmented_columns",
        "max_matrix_nonzeros",
        "max_coefficient_terms",
    ] {
        limit(r, k)?;
    }
    policy(r)?;
    owner_limits(r)?;
    let sources = array(r, "sources")?;
    require(
        !sources.is_empty() && sources.len() <= limit(r, "max_source_rows")?,
        "source bank outside allowance",
    )?;
    let mut seen = BTreeSet::new();
    for source in sources {
        let id = source["source_row"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("named RowId required")?;
        require(
            source.get("source_ordinal").is_none() && source.get("weight").is_none(),
            "raw source bank takes RowId/offset only",
        )?;
        let offset = integers(&source["offset"], n)?;
        checked(IntegralShift::try_new(offset.clone()))?;
        require(seen.insert((id, offset)), "duplicate source RowId/offset")?;
    }
    selection::ordinals(r, sources.len())?;
    compact_coefficients(r)?;
    certificate::enabled(r)?;
    let lower = array(&r["chart"], "lower")?;
    let upper = array(&r["chart"], "upper")?;
    require(lower.len() == n && upper.len() == n, "chart arity differs")?;
    for i in 0..n {
        let lo = lower[i].as_u64().ok_or("invalid chart lower")?;
        require(
            upper[i].is_null() || upper[i].as_u64().is_some_and(|hi| hi >= lo),
            "invalid chart interval",
        )?;
    }
    let restrictions = fixed(r)?;
    for (i, &(axis, value)) in restrictions.iter().enumerate() {
        require(
            axis < n && (i == 0 || restrictions[i - 1].0 < axis),
            "fixed axes must be sorted and unique",
        )?;
        require(
            (value > 0) == (mask.as_bytes()[axis] == b'1'),
            "fixed support differs",
        )?;
        let local = checked(u64::try_from(if value > 0 {
            i128::from(value) - 1
        } else {
            -i128::from(value)
        }))?;
        require(
            lower[axis].as_u64() == Some(local) && upper[axis].as_u64() == Some(local),
            "fixed axis does not hold on full chart",
        )?;
    }
    let mut f = BTreeSet::new();
    for value in array(r, "forbidden_shifts")? {
        let shift = integers(value, n)?;
        require(shift.iter().any(|&x| x != 0), "target cannot be forbidden")?;
        checked(IntegralShift::try_new(shift.clone()))?;
        require(f.insert(shift), "duplicate forbidden shift")?;
    }
    number(r, "max_refinements")?;
    boundary::config(r)?;
    boundary_correction::validate(r, n)?;
    Ok(n)
}
fn guards_json(guards: &[project::Guard]) -> Value {
    json!(guards.iter().map(|g|json!({"polynomial":g.polynomial.raw().to_string(),"origin":g.origin,"display_only":true})).collect::<Vec<_>>())
}
fn row_json(row: &project::Row) -> Value {
    json!(row.iter().map(|(s,c)|json!({"shift":s.values(),"coefficient":c.raw().to_string(),"display_only":true})).collect::<Vec<_>>())
}
fn error_json(error: &SourcePortAuditError) -> Value {
    match error {
        SourcePortAuditError::UnprovedDescentObligation {
            term_ordinal,
            shift,
            local_lower,
            local_upper,
            child_sector,
        } => {
            json!({"kind":"UNPROVED_DESCENT_OBLIGATION","term_ordinal":term_ordinal,"shift":shift,"local_lower":local_lower,
                "local_upper":local_upper,"child_sector":child_sector,"concrete_counterexample_claim":false})
        }
        SourcePortAuditError::ResourceBudgetExhausted { resource } => {
            json!({"kind":"PROOF_BUDGET_EXHAUSTED","resource":resource})
        }
        SourcePortAuditError::UnsupportedResourcePolicy { .. } => {
            json!({"kind":"UNSUPPORTED_RESOURCE_POLICY","detail":error.to_string()})
        }
        SourcePortAuditError::UnsupportedAffineOwnership { .. } => {
            json!({"kind":"UNSUPPORTED_AFFINE_OWNERSHIP","detail":error.to_string()})
        }
        SourcePortAuditError::Message(message) => {
            json!({"kind":"OTHER_PROOF_REFUSAL","detail":message,"refinement_permitted":false})
        }
    }
}

fn run<const N: usize>(bytes: &[u8], r: &Value, export: bool) -> Result<(Value, Option<Vec<u8>>)> {
    require(
        r.get("modular_nomination").is_none(),
        "modular samples are nomination-only; use nominate mode",
    )?;
    run_mode::<N>(bytes, r, export, false)
}
fn run_mode<const N: usize>(
    bytes: &[u8],
    r: &Value,
    export: bool,
    nomination_only: bool,
) -> Result<(Value, Option<Vec<u8>>)> {
    require(!(export && nomination_only), "nomination cannot export")?;
    source_obstruction::validate(r)?;
    require(
        !source_obstruction::enabled(r)? || !(export || nomination_only),
        "source obstruction is a prove-only diagnostic",
    )?;
    require(
        !boundary_correction::enabled(r) || !nomination_only,
        "boundary correction cannot run as nomination-only",
    )?;
    require(
        !nomination_only || r.get("exact_source_ordinals").is_none(),
        "exact source selection cannot alter the modular nomination bank",
    )?;
    let exact_selection = selection::ordinals(r, array(r, "sources")?.len())?;
    let compact = compact_coefficients(r)?;
    let fresh_certificate = certificate::enabled(r)?;
    let boundary_config = boundary::config(r)?;
    let endpoint_locality = endpoint_locality::Policy::parse(r, N)?;
    require(
        !(nomination_only && boundary_config.is_some()),
        "boundary polynomial constraints require exact mode",
    )?;
    let started = Instant::now();
    progress::reset();
    progress::event(
        "owner_load_start",
        || json!({"owner_bytes":bytes.len(),"export_requested":export}),
    );
    let trace_detail = trace::detail(r)?;
    let p = policy(r)?;
    let limits = projection_limits(r)?;
    let sector: [bool; N] =
        std::array::from_fn(|i| r["owner_mask"].as_str().unwrap().as_bytes()[i] == b'1');
    let mask = checked(Mask::try_new(sector))?;
    let ingress = owner_limits(r)?;
    let (family, programs) = checked(load_generated_candidate_owners::<N>(
        &[CandidateOwnerBundle {
            bytes,
            owner_sector: &mask,
        }],
        ingress,
        Default::default(),
    ))?;
    require(
        family.fingerprint() == r["family_fingerprint"].as_str().unwrap(),
        "family fingerprint differs",
    )?;
    if boundary_config.is_some() {
        require(
            family.power_shifts().iter().all(|shift| shift.is_zero()),
            "boundary activation faces require unshifted integral powers",
        )?;
    }
    let programs = Arc::new(programs);
    let owner = checked(programs.bind_owner_search(sector, Default::default()))?;
    require(
        owner.owner_ordering().stable_id() == r["expected_order"].as_str().unwrap(),
        "saved order differs",
    )?;
    let generator = checked(ParametricIbpGenerator::try_new_with_config(
        &family,
        p.source_generation,
    ))?;
    progress::event(
        "source_generation_start",
        || json!({"requested_rows":r["sources"].as_array().map(Vec::len)}),
    );
    let c = generator.context();
    let prepared = checked(generator.prepare_ordinary_ibp())?;
    let count = prepared.len();
    require(
        count <= limit(r, "max_complete_source_rows")?,
        "ordinary inventory exceeds allowance",
    )?;
    let rows = (0..count).map(|i| prepared.generate(i)).collect();
    let completed = checked(prepared.complete(rows))?;
    let zero = checked(IntegralShift::try_new([0_i64; N]))?;
    let mut inventory_limits = p.translated_sources;
    inventory_limits.max_requested_source_translations = limit(r, "max_complete_source_rows")?;
    inventory_limits.max_translated_sources = limit(r, "max_complete_source_rows")?;
    let inventory = checked(generator.translate_selected_completed_source_rows(
        &completed,
        (0..count).map(|i| TranslatedSourceRequest::new(i, zero.clone())),
        inventory_limits,
    ))?;
    let ids = inventory
        .sources()
        .iter()
        .map(|s| {
            (
                s.provenance().source_row().stable_string(),
                s.provenance().source_ordinal(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let requested = array(r, "sources")?
        .iter()
        .map(|s| {
            Ok(TranslatedSourceRequest::new(
                *ids.get(s["source_row"].as_str().unwrap())
                    .ok_or("RowId absent from complete native inventory")?,
                checked(IntegralShift::try_new(integers(&s["offset"], N)?))?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let batch = checked(generator.translate_selected_completed_source_rows(
        &completed,
        requested.clone(),
        p.translated_sources,
    ))?;
    let restrictions = fixed(r)?;
    let mut span = checked(source::Span::ordinary(
        c,
        &batch,
        &restrictions,
        p.cell.indexed_algebra,
        limits,
    ))?;
    // Input order steers visitation; original provenance remains canonical.
    let map = batch
        .requests()
        .iter()
        .enumerate()
        .map(|(i, q)| (q.clone(), i))
        .collect::<BTreeMap<_, _>>();
    let order = requested
        .iter()
        .map(|q| {
            map.get(q)
                .copied()
                .ok_or_else(|| "source request disappeared".into())
        })
        .collect::<Result<Vec<_>>>()?;
    require(
        order.len() == span.images.len(),
        "canonical source inventory changed",
    )?;
    span.weights = order.iter().map(|&i| span.weights[i].clone()).collect();
    span.images = order.iter().map(|&i| span.images[i].clone()).collect();
    let universe = span
        .images
        .iter()
        .flat_map(|row| row.keys().cloned())
        .collect::<BTreeSet<_>>();
    let mut forbidden = array(r, "forbidden_shifts")?
        .iter()
        .map(|v| {
            let values = integers(v, N)?;
            universe
                .iter()
                .find(|s| s.values() == values.as_slice())
                .cloned()
                .ok_or_else(|| "forbidden shift outside frozen image universe".into())
        })
        .collect::<Result<BTreeSet<_>>>()?;
    let derived_root_columns = if root_policy::enabled(r)? {
        root_policy::columns(owner.owner_root(), &restrictions, &universe)?
    } else {
        BTreeSet::new()
    };
    let derived_root_new_count = derived_root_columns.difference(&forbidden).count();
    forbidden.extend(derived_root_columns.iter().cloned());
    let cofinal_enabled = cofinal::enabled(r)?;
    if boundary_correction::enabled(r) {
        require(
            family.power_shifts().iter().all(|shift| shift.is_zero()),
            "boundary correction refuses shifted powers",
        )?;
    }
    progress::event(
        "cofinal_start",
        || json!({"enabled":cofinal_enabled,"image_columns":universe.len(),"f_size":forbidden.len()}),
    );
    let (cofinal_columns, cofinal_witnesses) = if cofinal_enabled {
        require(
            family.power_shifts().iter().all(|shift| shift.is_zero()),
            "cofinal nomination refuses shifted powers",
        )?;
        cofinal::columns(r, owner.owner_ordering(), &universe)?
    } else {
        (BTreeSet::new(), Vec::new())
    };
    let cofinal_new_count = cofinal_columns.difference(&forbidden).count();
    forbidden.extend(cofinal_columns);
    if positive_power_envelope::cap(r)?.is_some() {
        require(
            family.power_shifts().iter().all(|shift| shift.is_zero()),
            "positive-power envelope refuses shifted powers",
        )?;
    }
    let envelope = positive_power_envelope::columns(r, owner.owner_ordering(), &universe)?;
    let envelope_new_count = envelope
        .as_ref()
        .map_or(0, |e| e.columns.difference(&forbidden).count());
    if let Some(e) = &envelope {
        forbidden.extend(e.columns.iter().cloned());
    }
    // This uses every actual post-fixed source column, before exact frame
    // selection. Nonlocal rows remain available for coefficient cancellation.
    let locality_counts = endpoint_locality.extend_forbidden(r, &universe, &mut forbidden)?;
    let annotate = |report| {
        endpoint_locality.annotate(
            positive_power_envelope::annotate(
                cofinal::annotate_for_request(
                    report,
                    r,
                    cofinal_enabled,
                    &cofinal_witnesses,
                    cofinal_new_count,
                ),
                envelope.as_ref(),
                envelope_new_count,
            ),
            locality_counts,
        )
    };
    progress::event("source_ready", || {
        json!({"source_rows":span.originals.len(),"image_columns":universe.len(),
        "f_size":forbidden.len(),"derived_root_columns":derived_root_columns.len(),"cofinal_new_columns":cofinal_new_count,"retained_guards":span.guards.len()})
    });
    let Some(target) = universe
        .iter()
        .find(|s| s.values().iter().all(|&v| v == 0))
        .cloned()
    else {
        return Ok((
            annotate(
                json!({"schema":SCHEMA,"status":"NO_TARGET_COLUMN_IN_FROZEN_SPAN","request":r,
            "finite_bank_rows":span.originals.len(),"frozen_image_columns":universe.len(),"refinements":0,
            "derived_fixed_outside_root_forbidden_shifts":derived_root_columns.iter().map(|s|s.values()).collect::<Vec<_>>(),
            "derived_fixed_outside_root_new_count":derived_root_new_count,"bound_owner_root":owner.owner_root().as_slice(),
            "source_proof_passed":false,"nonexistence_claim":false,"production_modified":false}),
            ),
            None,
        ));
    };
    let mut attempts = Vec::new();
    if nomination_only {
        let report = nominate::run(r, c, &completed, &requested, &forbidden, &target)?;
        return Ok((annotate(report), None));
    }
    // Derive the entire source universe, F, assumptions and provenance before
    // selecting an exact frame. A sampled shortlist changes neither authority
    // nor the set of mandatory cancelled columns, including absent/zero ones.
    if let Some(ordinals) = &exact_selection {
        selection::apply(&mut span, ordinals)?;
        progress::event("exact_frame_selected", || {
            json!({"original_rows":span.originals.len(),"selected_rows":span.images.len(),
                "full_f_columns":forbidden.len(),"retained_guards":span.guards.len()})
        });
    }
    let mut added = 0;
    let mut candidate = None;
    let mut status = "REFUSED_OR_INCOMPLETE";
    loop {
        progress::event(
            "projection_requested",
            || json!({"attempt":attempts.len(),"refinements":added,"f_size":forbidden.len()}),
        );
        let projected = if let Some(config) = &boundary_config {
            boundary::project_original(
                c,
                &span,
                &target,
                &forbidden,
                p.cell.indexed_algebra,
                limits,
                config,
            )
        } else if reconstructed::enabled(r) {
            reconstructed::project(
                r,
                c,
                &span.images,
                &target,
                &forbidden,
                &span.guards,
                limits,
            )
        } else {
            let projection_function = if direct_l::enabled(r)? {
                direct_l::project_with_compaction
            } else {
                project::project_with_compaction
            };
            projection_function(
                c,
                &span.images,
                &target,
                &forbidden,
                &span.guards,
                limits,
                compact,
            )
        };
        let projection = match projected {
            Ok(v) => v,
            Err(e) => {
                progress::event(
                    "projection_refused",
                    || json!({"detail":e.to_string(),"f_size":forbidden.len()}),
                );
                trace::append(
                    &mut attempts,
                    json!({"status":"SYMBOLIC_PROJECTION_REFUSED","detail":e.to_string(),"refinement_permitted":false}),
                    trace_detail,
                );
                break;
            }
        };
        let proposal = match projection {
            project::Projection::NoTarget { guards, rows } => {
                status = if boundary_config.is_some() {
                    "NO_TARGET_IN_BOUNDARY_POLYNOMIAL_ANSATZ"
                } else if exact_selection.is_some() {
                    "NO_TARGET_IN_SELECTED_EXACT_FRAME_WITH_CURRENT_F"
                } else {
                    "NO_TARGET_IN_FROZEN_SPAN_WITH_CURRENT_F"
                };
                let mut attempt = json!({"status":status,"visited_rows":rows,"conditions":guards_json(&guards),"nonexistence_claim":false});
                source_obstruction::append_report(
                    r,
                    &generator,
                    &completed,
                    &inventory,
                    &family,
                    &span,
                    rows,
                    &target,
                    &forbidden,
                    &universe,
                    &guards,
                    p,
                    limits,
                    &mut attempt,
                );
                trace::append(&mut attempts, attempt, trace_detail);
                break;
            }
            project::Projection::Target(proposal) => proposal,
        };
        let (contributions, guards) = checked(span.compose(c, &proposal, limits))?;
        endpoint_locality.verify_image(r, &proposal.image)?;
        let rhs = proposal
            .image
            .iter()
            .filter(|(s, _)| *s != &target)
            .map(|(s, v)| Ok((s.clone(), checked(c.neg_with_limits(v, limits.arithmetic))?)))
            .collect::<Result<Vec<_>>>()?;
        let request = OriginalSourceCombinationRequest {
            root_sector: checked(Mask::try_new(*owner.owner_root()))?,
            sector: mask.clone(),
            ordering: owner.owner_ordering().clone(),
            lower: checked(serde_json::from_value(r["chart"]["lower"].clone()))?,
            upper: checked(serde_json::from_value(r["chart"]["upper"].clone()))?,
            fixed: restrictions
                .iter()
                .map(|&(a, v)| FixedIndexRestriction::new(a, v))
                .collect(),
            contributions,
            rhs,
            retained_conditions: guards.iter().map(|g| g.polynomial.clone()).collect(),
        };
        let request = if fresh_certificate {
            checked(certificate::fresh(c, &span, request, limits))?
        } else {
            request
        };
        let mut attempt = json!({"forbidden_shifts":forbidden.iter().map(|s|s.values()).collect::<Vec<_>>(),"first_target_prefix":proposal.prefix_rows,
            "normalized_full_product":row_json(&proposal.image),"conditions":guards_json(&guards),
            "ordinary_contributions":request.contributions.iter().map(|s|json!({"source_row":s.source_row.stable_string(),"offset":s.offset.values(),"weight":s.weight.raw().to_string(),"display_only":true})).collect::<Vec<_>>(),
            "full_original_product_replayed":true});
        if fresh_certificate {
            attempt["fresh_original_source_certificate"] = json!({
                "constructor":"ordinary", "identity_selection_validated":true,
                "all_original_conditions_retained":span.guards.len(),
                "native_final_condition_origin_policy":"regenerate from final original contributions",
                "retained_input_conditions":guards_json(&span.guards),
                "sealed_proof_modified":false,
            });
        }
        progress::event("proof_start", || {
            json!({"source_contributions":request.contributions.len(),
            "rhs_terms":request.rhs.len(),"retained_guards":request.retained_conditions.len(),"f_size":forbidden.len()})
        });
        match check_original_source_combination(&family, request.clone(), p) {
            Err(error) => {
                progress::event("proof_refused", || error_json(&error));
                attempt["proof_error"] = error_json(&error);
                let action = refine::refine(
                    &error,
                    &target,
                    &universe,
                    &mut forbidden,
                    added,
                    number(r, "max_refinements")?,
                );
                match action {
                    Ok(refine::Action::Added(shift)) => {
                        added += 1;
                        progress::event(
                            "refinement_added",
                            || json!({"refinements":added,"f_size":forbidden.len(),"shift":shift.values()}),
                        );
                        attempt["next_forbidden_shift"] = json!(shift.values());
                        trace::append(&mut attempts, attempt, trace_detail);
                        continue;
                    }
                    Ok(refine::Action::Stop) => {
                        attempt["status"] = json!("PROOF_REFUSED_NO_REFINEMENT");
                    }
                    Err(e) => {
                        attempt["status"] = json!("REFINEMENT_STOPPED");
                        attempt["stop_detail"] = json!(e.to_string());
                    }
                }
                trace::append(&mut attempts, attempt, trace_detail);
                break;
            }
            Ok(proof) => {
                let mut request = request;
                progress::event(
                    "proof_passed",
                    || json!({"cells":proof.cells().count(),"f_size":forbidden.len()}),
                );
                status = "EXACT_ORIGINAL_SOURCE_CHART_PROVED";
                attempt["status"] = json!(status);
                attempt["proof_cells"]=json!(proof.cells().enumerate().map(|(i,cell)|json!({"lower":proof.cell_bounds(i).unwrap().0,"upper":proof.cell_bounds(i).unwrap().1,
                    "rhs_terms":cell.rule().right_hand_side().len(),"guards":cell.rule().nonzero_guards().iter().map(|g|g.polynomial().raw().to_string()).collect::<Vec<_>>()})).collect::<Vec<_>>());
                if boundary_correction::enabled(r) {
                    let correction = boundary_correction::run(
                        r,
                        &generator,
                        &completed,
                        &inventory,
                        &ids,
                        &span,
                        &request,
                        &proposal.image,
                        &proof,
                        &target,
                        &forbidden,
                        &family,
                        p,
                        limits,
                        export,
                    );
                    let boundary_correction::Outcome {
                        report: correction,
                        export_request,
                    } = match correction {
                        Ok(outcome) => outcome,
                        Err(detail) => json!({"schema":boundary_correction::SCHEMA,
                            "status":"BOUNDARY_CORRECTION_REFUSED_OR_INCOMPLETE", "detail":detail,
                            "affine_infeasibility_claim":false,
                            "original_source_replay_verified":false})
                        .into(),
                    };
                    // The outer circuit remains the proved stage-one control;
                    // the nested report owns every stage-two outcome/circuit.
                    status = match correction["status"].as_str() {
                        Some("EXACT_BOUNDARY_CORRECTION_CHART_PROVED") => {
                            "EXACT_BOUNDARY_CORRECTION_CHART_PROVED"
                        }
                        Some("EXACT_BOUNDARY_ZERO_OBJECTIVE_CHART_PROVED") => {
                            "EXACT_BOUNDARY_ZERO_OBJECTIVE_CHART_PROVED"
                        }
                        Some("BOUNDARY_CORRECTION_SUPPORT_OBSTRUCTION") => {
                            "BOUNDARY_CORRECTION_SUPPORT_OBSTRUCTION"
                        }
                        Some("NO_BOUNDARY_CORRECTION_IN_FROZEN_WEIGHTED_SPAN") => {
                            "NO_BOUNDARY_CORRECTION_IN_FROZEN_WEIGHTED_SPAN"
                        }
                        Some("BOUNDARY_ZERO_CONTROL_REFUSED") => "BOUNDARY_ZERO_CONTROL_REFUSED",
                        Some("BOUNDARY_CORRECTION_PROOF_REFUSED") => {
                            "BOUNDARY_CORRECTION_PROOF_REFUSED"
                        }
                        _ => "BOUNDARY_CORRECTION_REFUSED_OR_INCOMPLETE",
                    };
                    attempt["boundary_correction"] = correction;
                    match (export, export_request) {
                        (true, Some(corrected)) => {
                            request = corrected;
                            attempt["export_source"] =
                                json!("checked stage-two boundary correction");
                        }
                        _ => {
                            // A miss/refusal never falls back to exporting B0.
                            trace::append(&mut attempts, attempt, trace_detail);
                            break;
                        }
                    }
                }
                if export {
                    progress::event("export_start", || json!({"base_bytes":bytes.len()}));
                    let before =
                        checked(inspect_generated_candidate_bundle(bytes, ingress.bundle))?;
                    match encode_checked_priority_owner_with_policy::<N>(
                        bytes,
                        request,
                        p,
                        ingress.bundle,
                        RuleDispatchPolicy::AfterBaselinePartitionWholePiece,
                    ) {
                        Err(error) => {
                            progress::event(
                                "export_refused",
                                || json!({"detail":error.to_string()}),
                            );
                            status = "EXACT_CHART_PROVED_EXPORT_REFUSED";
                            attempt["export_error"] = json!(error.to_string());
                        }
                        Ok(owner) => {
                            progress::event(
                                "export_encoded",
                                || json!({"candidate_bytes":owner.bytes().len()}),
                            );
                            let after = checked(inspect_generated_candidate_bundle(
                                owner.bytes(),
                                ingress.bundle,
                            ))?;
                            require(
                                after.generated_rules == before.generated_rules + 1
                                    && after.finite_residuals == before.finite_residuals
                                    && after.root_sector == before.root_sector
                                    && after.sectors == before.sectors
                                    && after.integral_order == before.integral_order
                                    && after.family_fingerprint == before.family_fingerprint,
                                "export changed base scope/counts",
                            )?;
                            require(
                                owner.bytes().len() <= limit(r, "max_output_bytes")?,
                                "export exceeds output allowance",
                            )?;
                            attempt["export"] = json!({"base_rules":before.generated_rules,"candidate_rules":after.generated_rules,"terminal_count":after.finite_residuals,
                                "source_rechecked_by_exporter":true,"bytes_alone_replay_source_proof":false,"dispatch_policy":format!("{:?}",owner.dispatch_policy())});
                            candidate = Some(owner.bytes().to_vec());
                            if boundary_correction::enabled(r) {
                                attempt["boundary_correction"]["exported"] = json!(true);
                            }
                            status = "CHECKED_PRIORITY_OWNER_EXPORTED";
                        }
                    }
                }
                trace::append(&mut attempts, attempt, trace_detail);
                break;
            }
        }
    }
    progress::event(
        "run_finished",
        || json!({"status":status,"attempts":attempts.len(),"refinements":added}),
    );
    Ok((
        annotate(
            json!({"schema":SCHEMA,"status":status,"request":r,"ordinary_sources_generated":count,
        "finite_bank_rows":span.originals.len(),"exact_projection_rows":span.images.len(),"nonzero_symbolic_images":span.images.iter().filter(|x|!x.is_empty()).count(),"frozen_image_columns":universe.len(),
        "derived_fixed_outside_root_forbidden_shifts":derived_root_columns.iter().map(|s|s.values()).collect::<Vec<_>>(),
        "derived_fixed_outside_root_new_count":derived_root_new_count,"bound_owner_root":owner.owner_root().as_slice(),
        "guard_tautology_policy":"native nonzero constants omitted after context/zero checks; all nonconstant assumptions retained",
        "refinements":added,"attempts":attempts,"seconds":started.elapsed().as_secs_f64(),"production_modified":false,"recursive_walk":false,
        "point_weight_lift":false,"coverage_or_cost_claim":false,"finite_span_miss_is_not_nonexistence":true}),
        ),
        candidate,
    ))
}
fn read(path: &Path, max: usize) -> Result<Vec<u8>> {
    let f = checked(File::open(path))?;
    require(
        checked(f.metadata())?.len() <= max as u64,
        "input byte allowance exceeded",
    )?;
    let mut b = Vec::new();
    checked(f.take(max as u64 + 1).read_to_end(&mut b))?;
    require(b.len() <= max, "input grew past allowance")?;
    Ok(b)
}
fn fresh(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut f = checked(OpenOptions::new().write(true).create_new(true).open(path))?;
    checked(f.write_all(bytes))?;
    checked(f.sync_all())
}
fn main_result() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    require(
        (args.len() == 2 && args[0] == "validate")
            || (args.len() == 3 && args[0] == "inspect")
            || (args.len() == 3 && args[0] == "support-inspect")
            || (args.len() == 3 && args[0] == "nominate")
            || (args.len() == 3 && args[0] == "prove")
            || (args.len() == 4 && args[0] == "export"),
        "usage: symbolic_projector validate REQUEST | inspect OWNER REQUEST | support-inspect OWNER REQUEST | nominate OWNER REQUEST | prove OWNER REQUEST | export OWNER REQUEST FRESH_DIRECTORY",
    )?;
    let request_bytes = read(
        Path::new(&args[if args[0] == "validate" { 1 } else { 2 }]),
        1 << 20,
    )?;
    let r: Value = checked(serde_json::from_slice(&request_bytes))?;
    let n = if r["schema"] == support_inspect::SCHEMA {
        support_inspect::validate(&r)?;
        require(
            args[0] == "validate" || args[0] == "support-inspect",
            "support-inspection schema cannot prove or export",
        )?;
        r["owner_mask"].as_str().unwrap().len()
    } else if r["schema"] == inspect::SCHEMA {
        inspect::validate(&r)?;
        require(
            args[0] == "validate" || args[0] == "inspect",
            "inspection schema cannot prove or export",
        )?;
        r["owner_mask"].as_str().unwrap().len()
    } else {
        require(
            args[0] != "inspect" && args[0] != "support-inspect",
            "inspection modes need their separate read-only schemas",
        )?;
        let n = validate(&r)?;
        if args[0] == "nominate" || (args[0] == "validate" && r.get("modular_nomination").is_some())
        {
            nominate::validate(&r, n)?;
        } else {
            require(
                r.get("modular_nomination").is_none(),
                "modular samples are nomination-only",
            )?;
        }
        n
    };
    let (report, candidate) = if args[0] == "validate" {
        (json!({"status":"INPUT_SHAPE_VALID","arity":n}), None)
    } else if args[0] == "inspect" {
        let bytes = read(Path::new(&args[1]), limit(&r, "max_owner_bytes")?)?;
        (inspect::run(&bytes, &r)?, None)
    } else if args[0] == "support-inspect" {
        let bytes = read(Path::new(&args[1]), limit(&r, "max_owner_bytes")?)?;
        (support_inspect::run(&bytes, &r)?, None)
    } else {
        let bytes = read(Path::new(&args[1]), limit(&r, "max_owner_bytes")?)?;
        macro_rules! dispatch{($($n:literal),*)=>{match n{$($n=>run_mode::<$n>(&bytes,&r,args[0]=="export",args[0]=="nominate"),)*_=>Err("unsupported arity".into())}};}
        dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)?
    };
    let text = checked(serde_json::to_string_pretty(&report))?;
    progress::event("report_serialized", || json!({"bytes":text.len()}));
    require(
        text.len() <= limit(&r, "max_report_bytes")?,
        "report byte allowance exceeded",
    )?;
    if let Some(bytes) = candidate {
        let dest = Path::new(&args[3]);
        checked(fs::create_dir(dest))?;
        fresh(&dest.join("request.json"), &request_bytes)?;
        fresh(&dest.join("candidate.rrbin"), &bytes)?;
        fresh(&dest.join("proof-export.json"), text.as_bytes())?;
        progress::event(
            "artifact_written",
            || json!({"candidate_bytes":bytes.len()}),
        );
    }
    println!("{text}");
    Ok(())
}
fn main() {
    if let Err(error) = main_result() {
        eprintln!(
            "{}",
            json!({"status":"REFUSED_OR_INCOMPLETE","error":error,"production_modified":false})
        );
        std::process::exit(2);
    }
}
