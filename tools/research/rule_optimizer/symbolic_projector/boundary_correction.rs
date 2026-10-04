//! Two-stage, full-original boundary correction. No saved-rule/zero quotient
//! ingress: the incumbent is the actually checked stage-one native product.
//! Pure pinched ordinary rows make the existing homogeneous projector affine:
//! only the incumbent has the normalized target coefficient, hence its weight
//! must remain one. Successful export requests remain typed and pass through
//! the existing checked exporter; this module never installs a candidate.
use super::*;
use rustred::{
    algebra::IndexedCoefficientContext, foundry::artifact::CheckedOriginalSourceCombination,
    identity::CompletedIbpSourceRows,
};
use symbolica::domains::SelfRing;

pub const SCHEMA: &str = "rustred.boundary-correction.v1";

pub struct Outcome {
    pub report: Value,
    /// Available only after the complete stage-two original-source check.
    pub export_request: Option<OriginalSourceCombinationRequest>,
}

impl From<Value> for Outcome {
    fn from(report: Value) -> Self {
        Self {
            report,
            export_request: None,
        }
    }
}

#[path = "boundary_dual.rs"]
mod dual;
#[path = "boundary_guard_diagnostic.rs"]
mod guard_diagnostic;
#[path = "source_nomination.rs"]
mod nomination;
#[path = "boundary_reproof.rs"]
mod reproof;

pub fn enabled(r: &Value) -> bool {
    r.get("boundary_correction").is_some()
}

pub fn validate(r: &Value, n: usize) -> Result<()> {
    if !enabled(r) {
        return Ok(());
    }
    let cfg = &r["boundary_correction"];
    let object = cfg
        .as_object()
        .ok_or("boundary correction must be an object")?;
    require(
        object.keys().all(|k| {
            matches!(
                k.as_str(),
                "schema"
                    | "fixed_pinch_axis"
                    | "correction_sources"
                    | "cancel_rank_positive_shifts"
                    | "forbid_new_rank_positive"
                    | "max_numerator_rank"
                    | "exact_dual_separator"
                    | "witness_preimage_nomination"
                    | "translated_pinch_sources"
                    | "export_guard_diagnostic"
                    | "fresh_original_frame_reproof"
            )
        }),
        "unknown boundary correction field",
    )?;
    require(
        cfg["schema"] == SCHEMA,
        "boundary correction schema required",
    )?;
    rank_cap(r)?;
    checked(dual::enabled(cfg))?;
    nomination::enabled(cfg)?;
    let translated = translated_pinch_sources(cfg)?;
    guard_diagnostic::enabled(cfg)?;
    reproof::enabled(cfg)?;
    require(
        number(r, "max_refinements")? == 0,
        "boundary correction does not refine or grow its bank",
    )?;
    for key in [
        "boundary_polynomial",
        "modular_nomination",
        "exact_source_ordinals",
        "endpoint_locality",
        "forbid_endpoint_changes_on_axes",
        "forbid_endpoint_increases_on_axes",
        "max_positive_power_envelope",
    ] {
        require(
            r.get(key).is_none(),
            "boundary correction refuses additional projection modes",
        )?;
    }
    require(
        !reconstructed::enabled(r) && !direct_l::enabled(r)?,
        "boundary correction uses the native sparse projector only",
    )?;
    require(
        positive_power_envelope::cap(r)?.is_none(),
        "boundary correction refuses a positive-power envelope",
    )?;
    let pin = number(cfg, "fixed_pinch_axis")?;
    let fixed = fixed(r)?;
    let value = fixed
        .iter()
        .find(|(a, _)| *a == pin)
        .map(|(_, v)| *v)
        .ok_or("pinch axis must be explicitly fixed")?;
    require(pin < n && value > 0, "pinch axis must be fixed positive")?;
    // The first slice deliberately has rank-zero parent charts. No diagonal,
    // sampled support, free inactive ray or affine replacement is inferred.
    for i in 0..n {
        if r["owner_mask"].as_str().unwrap().as_bytes()[i] == b'0' {
            require(
                fixed.contains(&(i, 0)),
                "boundary correction requires fixed-zero inactive axes",
            )?;
        }
    }
    let corrections = array(cfg, "correction_sources")?;
    require(!corrections.is_empty(), "empty correction bank")?;
    let max_rows = limit(r, "max_source_rows")?;
    require(
        array(r, "sources")?
            .len()
            .checked_add(corrections.len())
            .is_some_and(|v| v <= max_rows),
        "combined original-source bank exceeds allowance",
    )?;
    let mut seen = BTreeSet::new();
    let mut seen_pairs = BTreeSet::new();
    for source in corrections {
        let fields = source
            .as_object()
            .ok_or("correction source must be an object")?;
        require(
            fields
                .keys()
                .all(|k| matches!(k.as_str(), "source_row" | "offset")),
            "correction source accepts only RowId/offset",
        )?;
        let id = source["source_row"]
            .as_str()
            .filter(|v| !v.is_empty())
            .ok_or("correction RowId required")?;
        let offset = integers(&source["offset"], n)?;
        if translated {
            require(
                i128::from(value) + i128::from(offset[pin]) <= 0,
                "translated correction source must keep the declared pinch nonpositive",
            )?;
            require(
                seen_pairs.insert((id, offset)),
                "duplicate correction RowId/offset",
            )?;
        } else {
            require(
                offset
                    .iter()
                    .enumerate()
                    .all(|(i, &v)| v == if i == pin { -value } else { 0 }),
                "correction offset must make only the declared fixed pinch zero",
            )?;
            require(seen.insert(id), "duplicate correction RowId")?;
        }
    }
    let mut selected = BTreeSet::new();
    for shift in array(cfg, "cancel_rank_positive_shifts")? {
        let shift = integers(shift, n)?;
        require(
            shift.iter().any(|&v| v != 0) && selected.insert(shift),
            "duplicate/target boundary cancellation shift",
        )?;
    }
    Ok(())
}

/// Explicit admission of a frozen translated-source list, not permission to
/// search offsets or grow a bank. The native full-image/proof gates below are
/// identical in both modes; source rank and other-axis activations are not
/// substitutes for those gates.
fn translated_pinch_sources(cfg: &Value) -> Result<bool> {
    match cfg.get("translated_pinch_sources") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err("translated_pinch_sources must be boolean".into()),
    }
}

fn pin_value(r: &Value) -> Result<(usize, i64)> {
    let pin = number(&r["boundary_correction"], "fixed_pinch_axis")?;
    Ok((
        pin,
        fixed(r)?
            .into_iter()
            .find(|(a, _)| *a == pin)
            .ok_or("fixed pinch disappeared")?
            .1,
    ))
}

/// Conservative exact whole-box test. Inactive parent axes are fixed zero;
/// active powers increase with the nonnegative local coordinate. In particular
/// a NEW negative power on the pinched formerly-active axis is not overlooked.
pub(super) fn may_have_rank(r: &Value, shift: &IndexShift) -> Result<bool> {
    may_have_rank_values(r, shift.values())
}

fn may_have_rank_values(r: &Value, values: &[i64]) -> Result<bool> {
    Ok(maximum_rank(r, values)? > 0)
}

/// None is the historical exact-support lock; Some(K) permits new columns
/// only within a declared whole-chart total numerator-rank window.
fn rank_cap(r: &Value) -> Result<Option<u64>> {
    let cfg = &r["boundary_correction"];
    match cfg["forbid_new_rank_positive"].as_bool() {
        Some(true) => {
            require(
                cfg.get("max_numerator_rank").is_none(),
                "strict support lock cannot also declare a rank cap",
            )?;
            Ok(None)
        }
        Some(false) => Ok(Some(
            cfg.get("max_numerator_rank")
                .and_then(Value::as_u64)
                .ok_or(
                    "relaxed support requires an explicit nonnegative integer max_numerator_rank",
                )?,
        )),
        None => Err("forbid_new_rank_positive must be boolean".into()),
    }
}

// Exact maximum on the admitted rectangular scalar-parent chart: every
// summand is nonincreasing in each free positive physical power, so all
// maxima occur simultaneously at the coordinatewise lower corner.
fn maximum_rank(r: &Value, values: &[i64]) -> Result<i128> {
    let mask = r["owner_mask"].as_str().ok_or("mask required")?;
    require(values.len() == mask.len(), "rank shift arity differs")?;
    let fixed = fixed(r)?;
    let mut rank = 0i128;
    for (i, &s) in values.iter().enumerate() {
        let minimum = if mask.as_bytes()[i] == b'1' {
            1 + i128::from(
                r["chart"]["lower"][i]
                    .as_u64()
                    .ok_or("chart lower required")?,
            )
        } else {
            require(
                fixed.contains(&(i, 0)),
                "rank classifier requires fixed-zero inactive axis",
            )?;
            0
        };
        rank = rank
            .checked_add((-(minimum + i128::from(s))).max(0))
            .ok_or("total numerator rank overflow")?;
    }
    Ok(rank)
}

fn must_preserve(r: &Value, values: &[i64]) -> Result<bool> {
    let (pin, value) = pin_value(r)?;
    Ok(
        i128::from(value) + i128::from(*values.get(pin).ok_or("preservation shift arity differs")?)
            > 0,
    )
}

pub(super) fn verify_corrections(
    r: &Value,
    rows: &[project::Row],
    target: &IndexShift,
) -> Result<()> {
    let (pin, value) = pin_value(r)?;
    for row in rows {
        require(
            !row.contains_key(target),
            "correction row leaks into target",
        )?;
        for shift in row.keys() {
            require(
                shift.values().len() == target.values().len(),
                "correction arity differs",
            )?;
            require(
                i128::from(value) + i128::from(shift.values()[pin]) <= 0,
                "correction row reactivates the fixed pinch/same-support sector",
            )?;
        }
    }
    Ok(())
}

fn verify_leading(r: &Value, baseline: &project::Row, proposal: &project::Proposal) -> Result<()> {
    require(
        proposal.weights.get(&0).is_some_and(|v| v.raw().is_one()),
        "normalized projection changed incumbent coefficient",
    )?;
    let union = baseline
        .keys()
        .chain(proposal.image.keys())
        .collect::<BTreeSet<_>>();
    for shift in union {
        if must_preserve(r, shift.values())? {
            require(
                baseline.get(shift) == proposal.image.get(shift),
                "boundary correction changed unpinched/same-support coefficient",
            )?;
        }
    }
    Ok(())
}

pub(super) fn verify_preserved(
    r: &Value,
    baseline: &project::Row,
    proposal: &project::Proposal,
) -> Result<()> {
    verify_leading(r, baseline, proposal)?;
    let cap = rank_cap(r)?;
    for shift in proposal.image.keys() {
        if let Some(cap) = cap {
            require(
                maximum_rank(r, shift.values())? <= i128::from(cap),
                "candidate column exceeds whole-chart total numerator rank cap",
            )?;
        } else if !baseline.contains_key(shift) && may_have_rank(r, shift)? {
            require(
                !proposal.image.contains_key(shift),
                "new rank-positive column survived",
            )?;
        }
    }
    Ok(())
}

pub(super) fn forbidden(
    r: &Value,
    baseline: &project::Row,
    universe: &BTreeSet<IndexShift>,
    original: &BTreeSet<IndexShift>,
    request: &OriginalSourceCombinationRequest,
) -> Result<BTreeSet<IndexShift>> {
    let (pin, value) = pin_value(r)?;
    let mut f = original.clone();
    f.extend(root_policy::columns(
        request.root_sector.active_bits(),
        &fixed(r)?,
        universe,
    )?);
    if cofinal::enabled(r)? {
        f.extend(cofinal::columns(r, &request.ordering, universe)?.0);
    }
    for raw in array(&r["boundary_correction"], "cancel_rank_positive_shifts")? {
        let values = integers(raw, request.sector.arity())?;
        let shift = baseline
            .keys()
            .find(|s| s.values() == values)
            .ok_or("selected pinch absent from actual stage-one product")?;
        let uniform_rank = fixed(r)?
            .iter()
            .any(|&(axis, v)| i128::from(v) + i128::from(shift.values()[axis]) < 0);
        require(
            i128::from(value) + i128::from(shift.values()[pin]) <= 0 && uniform_rank,
            "selected cancellation column is not a whole-chart pinched rank-positive candidate",
        )?;
        f.insert(shift.clone());
    }
    for shift in universe {
        let prohibited = match rank_cap(r)? {
            Some(cap) => maximum_rank(r, shift.values())? > i128::from(cap),
            None => !baseline.contains_key(shift) && may_have_rank(r, shift)?,
        };
        if prohibited {
            f.insert(shift.clone());
        }
    }
    Ok(f)
}

fn clone_admission(span: &source::Span, limits: project::Limits) -> Result<(usize, usize)> {
    checked(project::bound(
        span.originals.len(),
        limits.rows,
        "merged original rows",
    ))?;
    checked(project::bound(
        span.guards.len(),
        limits.guards,
        "merged original conditions",
    ))?;
    let mut terms = 0usize;
    let mut entries = 0usize;
    for row in &span.originals {
        entries = entries
            .checked_add(row.len())
            .ok_or("original entry count overflow")?;
        for value in row.values() {
            terms = terms
                .checked_add(value.raw().numerator.nterms())
                .and_then(|n| n.checked_add(value.raw().denominator.nterms()))
                .ok_or("original coefficient term count overflow")?;
        }
    }
    checked(project::bound(
        entries,
        limits.nonzeros,
        "merged original nonzeros",
    ))?;
    checked(project::bound(
        terms,
        limits.coefficient_terms,
        "merged original coefficient terms",
    ))?;
    Ok((entries, terms))
}

pub(super) fn weighted(
    c: &IndexedCoefficientContext,
    baseline: &source::Span,
    corrections: &source::Span,
    checked_request: &OriginalSourceCombinationRequest,
    image: &project::Row,
    limits: project::Limits,
) -> Result<source::Span> {
    let (base_entries, base_terms) = clone_admission(baseline, limits)?;
    let (extra_entries, extra_terms) = clone_admission(corrections, limits)?;
    checked(project::bound(
        baseline
            .originals
            .len()
            .checked_add(corrections.originals.len())
            .ok_or("merged source count overflow")?,
        limits.rows,
        "combined original rows",
    ))?;
    checked(project::bound(
        base_entries
            .checked_add(extra_entries)
            .ok_or("combined entries overflow")?,
        limits.nonzeros,
        "combined original nonzeros",
    ))?;
    let guard_count = checked_request
        .retained_conditions
        .len()
        .checked_add(corrections.guards.len())
        .ok_or("combined condition count overflow")?;
    checked(project::bound(
        guard_count,
        limits.guards,
        "combined conditions before cloning",
    ))?;
    let mut cloned_terms = base_terms
        .checked_add(extra_terms)
        .ok_or("combined terms overflow")?;
    for value in checked_request
        .contributions
        .iter()
        .map(|s| &s.weight)
        .chain(image.values())
    {
        cloned_terms = cloned_terms
            .checked_add(value.raw().numerator.nterms())
            .and_then(|n| n.checked_add(value.raw().denominator.nterms()))
            .ok_or("copied terms overflow")?;
    }
    for polynomial in checked_request
        .retained_conditions
        .iter()
        .chain(corrections.guards.iter().map(|g| &g.polynomial))
    {
        cloned_terms = cloned_terms
            .checked_add(polynomial.raw().nterms())
            .ok_or("copied guard terms overflow")?;
    }
    checked(project::bound(
        cloned_terms,
        limits.coefficient_terms,
        "combined coefficient copies before cloning",
    ))?;
    let mut inputs = BTreeMap::new();
    for span in [baseline, corrections] {
        require(
            span.bindings.len() == span.originals.len(),
            "original bindings/image mismatch",
        )?;
        for (binding, row) in span.bindings.iter().zip(&span.originals) {
            for coefficient in row.values() {
                checked(c.validate_with_limits(coefficient, limits.arithmetic))?;
            }
            let key = (binding.row.clone(), binding.offset.clone());
            if let Some(old) = inputs.insert(key, row) {
                require(
                    old == row,
                    "same original binding has different native image",
                )?;
            }
        }
    }
    let indices = inputs
        .keys()
        .enumerate()
        .map(|(i, k)| (k.clone(), i))
        .collect::<BTreeMap<_, _>>();
    let mut incumbent = project::Weights::new();
    for contribution in &checked_request.contributions {
        let i = *indices
            .get(&(contribution.source_row.clone(), contribution.offset.clone()))
            .ok_or("checked incumbent contribution outside original union")?;
        require(
            incumbent.insert(i, contribution.weight.clone()).is_none(),
            "duplicate checked contribution",
        )?;
    }
    let mut weights = vec![incumbent];
    for binding in &corrections.bindings {
        let i = indices[&(binding.row.clone(), binding.offset.clone())];
        weights.push(project::Weights::from([(i, c.one())]));
    }
    // Use the assumptions of the ACTUALLY checked stage-one request, not its
    // possibly discarded computational pivot diagnostics. Native weighted
    // replay regenerates every incoming source/weight/product denominator.
    let mut guards = checked_request
        .retained_conditions
        .iter()
        .map(|p| project::Guard {
            polynomial: p.clone(),
            origin: "checked stage-one caller/source assumption".into(),
        })
        .collect::<Vec<_>>();
    guards.extend(corrections.guards.iter().cloned());
    let bindings = inputs
        .keys()
        .map(|(row, offset)| source::SourceBinding {
            row: row.clone(),
            offset: offset.clone(),
        })
        .collect();
    let originals = inputs.values().map(|row| (*row).clone()).collect();
    let span = checked(source::Span::weighted(
        c, bindings, originals, weights, guards, limits,
    ))?;
    require(
        span.images.first() == Some(image),
        "weighted incumbent differs from checked full original product",
    )?;
    Ok(span)
}

fn request_for(
    c: &IndexedCoefficientContext,
    span: &source::Span,
    proposal: &project::Proposal,
    template: &OriginalSourceCombinationRequest,
    target: &IndexShift,
    limits: project::Limits,
) -> Result<OriginalSourceCombinationRequest> {
    let (contributions, guards) = checked(span.compose(c, proposal, limits))?;
    Ok(OriginalSourceCombinationRequest {
        contributions,
        rhs: proposal
            .image
            .iter()
            .filter(|(s, _)| *s != target)
            .map(|(s, v)| Ok((s.clone(), checked(c.neg_with_limits(v, limits.arithmetic))?)))
            .collect::<Result<_>>()?,
        retained_conditions: guards.into_iter().map(|g| g.polynomial).collect(),
        ..template.clone()
    })
}

fn proof_json(proof: &CheckedOriginalSourceCombination) -> Value {
    json!(proof.cells().enumerate().map(|(i,cell)|json!({
        "lower":proof.cell_bounds(i).unwrap().0,"upper":proof.cell_bounds(i).unwrap().1,
        "rhs_terms":cell.rule().right_hand_side().len(),
        "guards":cell.rule().nonzero_guards().iter().map(|g|g.polynomial().raw().to_string()).collect::<Vec<_>>()
    })).collect::<Vec<_>>())
}

pub(super) fn missing_correction_columns(
    r: &Value,
    corrections: &[project::Row],
) -> Result<Vec<Vec<i64>>> {
    let actual = corrections
        .iter()
        .flat_map(|row| row.keys())
        .map(|shift| shift.values())
        .collect::<BTreeSet<_>>();
    array(&r["boundary_correction"], "cancel_rank_positive_shifts")?
        .iter()
        .map(|raw| integers(raw, r["owner_mask"].as_str().ok_or("mask required")?.len()))
        .filter_map(|values| match values {
            Ok(values) if actual.contains(values.as_slice()) => None,
            other => Some(other),
        })
        .collect()
}

// The native checker consumes its request. Charge the one export-only copy
// before cloning it; the ordinary prove path creates no additional copy.
// These are retained payload charges, not a bound on native scratch or RSS.
fn clone_for_export(
    c: &IndexedCoefficientContext,
    request: &OriginalSourceCombinationRequest,
    coordinate_limit: usize,
    limits: project::Limits,
) -> Result<OriginalSourceCombinationRequest> {
    checked(project::bound(
        request.contributions.len(),
        limits.rows,
        "export source copies",
    ))?;
    checked(project::bound(
        request.rhs.len(),
        limits.columns,
        "export RHS copies",
    ))?;
    checked(project::bound(
        request.retained_conditions.len(),
        limits.guards,
        "export guard copies",
    ))?;
    let mut coordinates = 0usize;
    for count in [
        request.root_sector.active_bits().len(),
        request.sector.active_bits().len(),
        request.lower.len(),
        request.upper.len(),
        request.fixed.len(),
        request.fixed.len(),
    ]
    .into_iter()
    .chain(
        request
            .contributions
            .iter()
            .map(|s| s.offset.values().len()),
    )
    .chain(request.rhs.iter().map(|(s, _)| s.values().len()))
    {
        coordinates = coordinates
            .checked_add(count)
            .ok_or("export coordinate count overflow")?;
    }
    checked(project::bound(
        coordinates,
        coordinate_limit,
        "export coordinate copies",
    ))?;
    let mut terms = 0usize;
    for value in request
        .contributions
        .iter()
        .map(|s| &s.weight)
        .chain(request.rhs.iter().map(|(_, v)| v))
    {
        checked(c.validate_with_limits(value, limits.arithmetic))?;
        terms = terms
            .checked_add(value.raw().numerator.nterms())
            .and_then(|n| n.checked_add(value.raw().denominator.nterms()))
            .ok_or("export coefficient count overflow")?;
    }
    for guard in &request.retained_conditions {
        checked(c.validate_polynomial_with_limits(guard, limits.arithmetic))?;
        terms = terms
            .checked_add(guard.raw().nterms())
            .ok_or("export guard count overflow")?;
    }
    checked(project::bound(
        terms,
        limits.coefficient_terms,
        "export coefficient copies",
    ))?;
    Ok(request.clone())
}

pub fn run(
    r: &Value,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    inventory: &rustred::identity::SelectedTranslatedSourceBatch,
    ids: &BTreeMap<String, usize>,
    baseline: &source::Span,
    checked_request: &OriginalSourceCombinationRequest,
    image: &project::Row,
    baseline_proof: &CheckedOriginalSourceCombination,
    target: &IndexShift,
    original_f: &BTreeSet<IndexShift>,
    family: &rustred::family::IntegralFamily,
    p: OriginalSourceCombinationLimits,
    limits: project::Limits,
    retain_export_request: bool,
) -> Result<Outcome> {
    let c = generator.context();
    let cfg = &r["boundary_correction"];
    let sources = array(cfg, "correction_sources")?;
    let translated = translated_pinch_sources(cfg)?;
    if !translated {
        require(
            sources.len() == ids.len(),
            "correction bank must be one complete ordinary inventory",
        )?;
    }
    let requested = sources
        .iter()
        .map(|source| {
            Ok(TranslatedSourceRequest::new(
                *ids.get(
                    source["source_row"]
                        .as_str()
                        .ok_or("correction RowId required")?,
                )
                .ok_or("correction RowId absent from native inventory")?,
                checked(IntegralShift::try_new(integers(
                    &source["offset"],
                    c.index_count(),
                )?))?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let batch = checked(generator.translate_selected_completed_source_rows(
        completed,
        requested,
        p.translated_sources,
    ))?;
    require(
        batch.len() == sources.len(),
        "native correction list lost a duplicate binding",
    )?;
    let corrections = checked(source::Span::ordinary(
        c,
        &batch,
        &fixed(r)?,
        p.cell.indexed_algebra,
        limits,
    ))?;
    verify_corrections(r, &corrections.images, target)?;
    let span = weighted(c, baseline, &corrections, checked_request, image, limits)?;
    let zero = project::Proposal {
        weights: project::Weights::from([(0, c.one())]),
        image: image.clone(),
        guards: span.guards.clone(),
        prefix_rows: 1,
    };
    // The control proves B0 unchanged, even if the candidate objective would
    // eliminate some of B0's columns (for example a requested rank-zero cap).
    verify_leading(r, image, &zero)?;
    let zero_request = request_for(c, &span, &zero, checked_request, target, limits)?;
    require(
        zero_request.rhs == checked_request.rhs,
        "zero correction changed typed RHS",
    )?;
    let contributions = |request: &OriginalSourceCombinationRequest| {
        request
            .contributions
            .iter()
            .map(|s| ((s.source_row.clone(), s.offset.clone()), s.weight.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    require(
        contributions(&zero_request) == contributions(checked_request),
        "zero correction changed original contribution map",
    )?;
    require(
        checked_request
            .retained_conditions
            .iter()
            .all(|g| zero_request.retained_conditions.contains(g)),
        "zero correction dropped an incoming condition",
    )?;
    let zero_proof = match check_original_source_combination(family, zero_request, p) {
        Ok(proof) => proof,
        Err(error) => {
            return Ok(
                json!({"schema":SCHEMA,"status":"BOUNDARY_ZERO_CONTROL_REFUSED",
            "proof_error":error_json(&error),"affine_infeasibility_claim":false,
            "original_source_replay_verified":false})
                .into(),
            );
        }
    };
    require(
        zero_proof.cells().count() == baseline_proof.cells().count()
            && (0..zero_proof.cells().count())
                .all(|i| zero_proof.cell_bounds(i) == baseline_proof.cell_bounds(i)),
        "zero correction changed native cell coverage",
    )?;
    require(
        zero_proof
            .cells()
            .zip(baseline_proof.cells())
            .all(|(a, b)| a.rule().right_hand_side() == b.rule().right_hand_side()),
        "zero correction changed native cell RHS",
    )?;
    let universe = span
        .images
        .iter()
        .flat_map(|r| r.keys().cloned())
        .collect::<BTreeSet<_>>();
    let f = forbidden(r, image, &universe, original_f, checked_request)?;
    require(!f.contains(target), "boundary objective forbids target")?;
    let mut report = json!({"schema":SCHEMA,"baseline_kind":"new stage-one full-original proposal; not the saved incumbent",
        "baseline_rhs_terms":checked_request.rhs.len(),"baseline_full_product":row_json(image),
        "zero_control":{"status":"EXACT_ORIGINAL_SOURCE_CHART_PROVED","typed_full_image_equal":true,
            "typed_rhs_equal":true,"original_contribution_map_equal":true,"incoming_conditions_retained":true,
            "cell_coverage_equal":true,"proof_cells":proof_json(&zero_proof)},
        "correction_rows":corrections.images.len(),"original_union_rows":span.originals.len(),
        "full_image_columns":universe.len(),"forbidden_shifts":f.iter().map(|s|s.values()).collect::<Vec<_>>(),
        "fresh_original_source_certificate":false,"exported":false,"recursive_walk":false,
        "original_source_replay_verified":false,
        "same_support_preserved":false,"new_rank_positive_columns":null,"conditions_discarded":false});
    if translated {
        report["correction_source_policy"] = json!({"translated_pinch_sources":true,
            "source_pinch":"nonpositive","source_rank_filtered":false,
            "bindings":corrections.bindings.iter().map(|b|json!({
                "source_row":b.row.stable_string(),"offset":b.offset.values()})).collect::<Vec<_>>(),
            "complete_native_images_target_and_unpinched_zero":true});
    }
    if let Some(cap) = rank_cap(r)? {
        report["rank_policy"] = json!({"forbid_new_rank_positive":false,
            "max_numerator_rank":cap,"scope":"whole chart, all axes, every candidate column"});
    }
    let missing = missing_correction_columns(r, &corrections.images)?;
    report["correction_only_actual_columns"] = json!(
        corrections
            .images
            .iter()
            .flat_map(|row| row.keys())
            .collect::<BTreeSet<_>>()
            .len()
    );
    report["missing_registered_correction_columns"] = json!(missing);
    let requested_p = array(cfg, "cancel_rank_positive_shifts")?
        .iter()
        .map(|v| integers(v, c.index_count()))
        .collect::<Result<Vec<_>>>()?;
    report["registered_cancellation_columns"] = json!(requested_p.len());
    report["present_registered_correction_columns"] = json!(
        requested_p
            .iter()
            .filter(|p| !missing.contains(p))
            .collect::<Vec<_>>()
    );
    if !missing.is_empty() {
        report["status"] = json!("BOUNDARY_CORRECTION_SUPPORT_OBSTRUCTION");
        report["objective_filtered"] = json!(false);
        return Ok(report.into());
    }
    let proposal = match checked(project::project_with_compaction(
        c,
        &span.images,
        target,
        &f,
        &span.guards,
        limits,
        compact_coefficients(r)?,
    ))? {
        project::Projection::NoTarget { guards, rows } => {
            report["status"] = json!("NO_BOUNDARY_CORRECTION_IN_FROZEN_WEIGHTED_SPAN");
            report["visited_rows"] = json!(rows);
            report["conditions"] = guards_json(&guards);
            let separator = dual::append_report(
                c,
                cfg,
                &corrections.images,
                image,
                &f,
                &guards,
                limits,
                &mut report,
            );
            if nomination::enabled(cfg)? {
                report["witness_preimage_nomination"] = match separator {
                    Some((columns, certificate)) => nomination::run(
                        r,
                        generator,
                        completed,
                        inventory,
                        family,
                        target,
                        &columns,
                        &certificate,
                        p,
                        limits,
                    ),
                    None => json!({"schema":"rustred.witness-preimage-nomination.v1",
                        "status":"WITNESS_PREIMAGE_REFUSED_OR_INCOMPLETE",
                        "error":"exact separator prerequisite incomplete","pairing_complete":false}),
                };
            }
            return Ok(report.into());
        }
        project::Projection::Target(proposal) => proposal,
    };
    verify_preserved(r, image, &proposal)?;
    let request = request_for(c, &span, &proposal, checked_request, target, limits)?;
    report["normalized_full_product"] = row_json(&proposal.image);
    report["conditions"] = guards_json(&proposal.guards);
    report["checked_request_retained_conditions"] = json!(
        request
            .retained_conditions
            .iter()
            .map(|p| p.raw().to_string())
            .collect::<Vec<_>>()
    );
    report["ordinary_contributions"] = json!(
        request
            .contributions
            .iter()
            .map(|s| json!({
        "source_row":s.source_row.stable_string(),"offset":s.offset.values(),
        "weight":s.weight.raw().to_string(),"display_only":true}))
            .collect::<Vec<_>>()
    );
    report["incumbent_coefficient_one"] = json!(true);
    report["same_support_preserved"] = json!(true);
    let new_rank = proposal
        .image
        .keys()
        .filter(|s| !image.contains_key(*s))
        .map(|s| may_have_rank(r, s))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|v| *v)
        .count();
    report["new_rank_positive_columns"] = json!(new_rank);
    if rank_cap(r)?.is_some() {
        report["maximum_candidate_numerator_rank"] = json!(
            proposal
                .image
                .keys()
                .map(|s| maximum_rank(r, s.values()))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .max()
                .unwrap_or(0)
        );
    }
    let fresh_reproof = reproof::enabled(cfg)?;
    // Reproof needs the exact typed final request after its independent
    // weighted check consumes the original. Charge the same bounded copy.
    let export_request = if retain_export_request || fresh_reproof {
        Some(clone_for_export(
            c,
            &request,
            limit(r, "max_coordinate_cells")?,
            limits,
        )?)
    } else {
        None
    };
    let diagnostic = if guard_diagnostic::enabled(cfg)? {
        Some(guard_diagnostic::prepare(
            c,
            family.fingerprint(),
            &request,
            p.cell.indexed_algebra,
            limit(r, "max_coordinate_cells")?,
            limits,
        ))
    } else {
        None
    };
    let (mut outcome, proof) = finish_proof(
        c,
        family,
        request,
        p,
        export_request,
        array(cfg, "cancel_rank_positive_shifts")?.is_empty(),
        report,
        diagnostic,
        limits,
    );
    if fresh_reproof {
        let fresh = match (proof, outcome.export_request.take()) {
            (Some(proof), Some(request)) => reproof::run(
                r,
                generator,
                completed,
                ids,
                baseline,
                checked_request,
                &span,
                target,
                &proposal.image,
                request,
                &proof,
                family,
                p,
                limits,
                retain_export_request,
            ),
            _ => json!({"schema":reproof::SCHEMA,
                "status":"FRESH_ORIGINAL_FRAME_REPROOF_SKIPPED_NO_WEIGHTED_PROOF",
                "original_source_replay_verified":false,
                "fresh_original_source_certificate":false})
            .into(),
        };
        outcome.report["fresh_original_frame_reproof"] = fresh.report;
        outcome.export_request = fresh.export_request;
    }
    Ok(outcome)
}

fn finish_proof(
    c: &IndexedCoefficientContext,
    family: &rustred::family::IntegralFamily,
    request: OriginalSourceCombinationRequest,
    p: OriginalSourceCombinationLimits,
    mut export_request: Option<OriginalSourceCombinationRequest>,
    control: bool,
    mut report: Value,
    diagnostic: Option<Result<guard_diagnostic::Prepared>>,
    limits: project::Limits,
) -> (Outcome, Option<CheckedOriginalSourceCombination>) {
    report["original_source_replay_verified"] = json!(false);
    let proof = match check_original_source_combination(family, request, p) {
        Err(error) => {
            export_request = None;
            report["status"] = json!("BOUNDARY_CORRECTION_PROOF_REFUSED");
            report["proof_error"] = error_json(&error);
            report["affine_infeasibility_claim"] = json!(false);
            if diagnostic.is_some() {
                report["export_guard_diagnostic"] = json!({"complete":false,
                    "status":"GUARD_DIAGNOSTIC_SKIPPED_NO_CHECKED_PROOF"});
            }
            None
        }
        Ok(proof) => {
            if let Some(input) = diagnostic {
                report["export_guard_diagnostic"] =
                    guard_diagnostic::report(c, &proof, input, p.cell.indexed_algebra, limits);
            }
            report["original_source_replay_verified"] = json!(true);
            report["status"] = json!(if control {
                "EXACT_BOUNDARY_ZERO_OBJECTIVE_CHART_PROVED"
            } else {
                "EXACT_BOUNDARY_CORRECTION_CHART_PROVED"
            });
            report["zero_objective_control_only"] = json!(control);
            report["proof_cells"] = proof_json(&proof);
            Some(proof)
        }
    };
    (
        Outcome {
            report,
            export_request,
        },
        proof,
    )
}

#[cfg(test)]
#[path = "boundary_correction_tests.rs"]
mod tests;
