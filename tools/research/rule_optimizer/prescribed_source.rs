//! Input-prescribed ordinary-source chart proof and isolated priority export.
//! This orchestrates existing native source/algebra/proof/codec services. It
//! does not lift point weights, delete zero sectors or walk. An isolated
//! geometry-nomination module feeds the same full original-source proof path.
mod geometry_tangent;
mod geometry_two_protected;
use rustred::{
    algebra::{
        ExactAlgebraLimits, IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext,
        IndexedPolynomial,
    },
    foundry::{
        artifact::{
            CheckedOriginalSourceCombination, OriginalSourceCombinationLimits,
            OriginalSourceCombinationRequest, OriginalSourceContribution,
            check_original_source_combination,
        },
        cell::FixedIndexRestriction,
    },
    identity::{
        IndexShift, IntegralShift, ParametricIbpGenerator, SelectedTranslatedSourceBatch,
        TranslatedSourceLimits, TranslatedSourceRequest,
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
const SCHEMA: &str = "rustred.prescribed-source-chart.v1";
fn checked<T, E: Display>(x: std::result::Result<T, E>) -> Result<T> {
    x.map_err(|e| e.to_string())
}
fn require(b: bool, s: &str) -> Result<()> {
    if b { Ok(()) } else { Err(s.into()) }
}
fn array<'a>(v: &'a Value, k: &str) -> Result<&'a [Value]> {
    v[k].as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{k} must be an array"))
}
fn limit(r: &Value, k: &str) -> Result<usize> {
    let n = r["limits"][k]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("invalid limit {k}"))?;
    require(n > 0, "all limits must be positive")?;
    Ok(n)
}
fn integers(v: &Value, n: usize) -> Result<Vec<i64>> {
    let a = v.as_array().ok_or("expected integer array")?;
    require(a.len() == n, "wrong vector arity")?;
    a.iter()
        .map(|v| v.as_i64().ok_or_else(|| "invalid integer".into()))
        .collect()
}
fn coefficient(c: &IndexedCoefficient) -> Value {
    json!({"native_display":c.raw().to_string(),"numerator":c.raw().numerator.to_string(),"denominator":c.raw().denominator.to_string()})
}
fn read(path: &Path, max: usize) -> Result<Vec<u8>> {
    let f = checked(File::open(path))?;
    require(
        checked(f.metadata())?.len() <= max as u64,
        "input exceeds byte allowance",
    )?;
    let mut b = Vec::new();
    checked(f.take(max as u64 + 1).read_to_end(&mut b))?;
    require(b.len() <= max, "input grew past allowance")?;
    Ok(b)
}
fn fresh_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut f = checked(OpenOptions::new().write(true).create_new(true).open(path))?;
    checked(f.write_all(bytes))?;
    checked(f.sync_all())
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

/// Optional transport policy only; absent input preserves the native default.
/// Algebra/proof limits are not changed by these seven ingress fields.
fn owner_load_limits(r: &Value) -> Result<CandidateOwnerLoadLimits> {
    let Some(value) = r.get("owner_load_limits") else {
        return Ok(CandidateOwnerLoadLimits::default());
    };
    let fields = [
        "max_bundle_bytes",
        "max_total_input_bytes",
        "max_total_coefficient_bytes",
        "max_collection_entries",
        "max_total_symbolica_state_bytes",
        "max_zero_sector_visits",
        "max_coefficient_bytes",
    ];
    let object = value
        .as_object()
        .ok_or("owner_load_limits must be an object")?;
    require(
        object.len() == fields.len() && object.keys().all(|key| fields.contains(&key.as_str())),
        "unknown or missing owner ingress field",
    )?;
    let number = |name: &str| -> Result<usize> {
        value[name]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|&n| n > 0)
            .ok_or_else(|| format!("positive owner ingress limit required: {name}"))
    };
    let mut policy = CandidateOwnerLoadLimits::default();
    policy.bundle.max_bundle_bytes = number("max_bundle_bytes")?;
    require(
        policy.bundle.max_bundle_bytes <= rustred_app::MAX_CANDIDATE_BUNDLE_BYTES,
        "owner bundle exceeds native hard ceiling",
    )?;
    policy.max_total_input_bytes = number("max_total_input_bytes")?;
    policy.bundle.max_total_coefficient_bytes = number("max_total_coefficient_bytes")?;
    policy.bundle.max_collection_entries = number("max_collection_entries")?;
    policy.max_total_symbolica_state_bytes = number("max_total_symbolica_state_bytes")?;
    policy.max_zero_sector_visits = number("max_zero_sector_visits")?;
    policy.bundle.max_coefficient_bytes = number("max_coefficient_bytes")?;
    Ok(policy)
}

fn validate(r: &Value) -> Result<usize> {
    require(
        r["schema"] == SCHEMA
            || r["schema"] == geometry_tangent::SCHEMA
            || r["schema"] == geometry_two_protected::SCHEMA,
        "unknown request schema",
    )?;
    let mask = r["owner_mask"].as_str().ok_or("owner_mask required")?;
    let n = mask.len();
    require(
        (1..=16).contains(&n) && mask.bytes().all(|c| c == b'0' || c == b'1'),
        "invalid owner arity/mask",
    )?;
    for k in ["family_fingerprint", "expected_order"] {
        require(
            r[k].as_str().is_some_and(|s| !s.is_empty()),
            "family/order binding required",
        )?;
    }
    owner_load_limits(r)?;
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
    ] {
        limit(r, k)?;
    }
    require(
        limit(r, "max_exponent")? <= usize::from(u16::MAX),
        "exponent ceiling exceeds native representation",
    )?;
    let lower = array(&r["chart"], "lower")?;
    let upper = array(&r["chart"], "upper")?;
    require(lower.len() == n && upper.len() == n, "chart arity mismatch")?;
    let restrictions = fixed(r)?;
    require(restrictions.len() <= n, "too many fixed coordinates")?;
    for (i, &(axis, value)) in restrictions.iter().enumerate() {
        require(
            axis < n && (i == 0 || restrictions[i - 1].0 < axis),
            "fixed coordinates must be sorted and unique",
        )?;
        require(
            (value > 0) == (mask.as_bytes()[axis] == b'1'),
            "fixed coordinate has wrong support",
        )?;
        let local = checked(u64::try_from(if value > 0 {
            i128::from(value) - 1
        } else {
            -i128::from(value)
        }))?;
        require(
            lower[axis].as_u64() == Some(local) && upper[axis].as_u64() == Some(local),
            "fixed coordinate does not equal chart interval",
        )?;
    }
    for axis in 0..n {
        let lo = lower[axis].as_u64().ok_or("invalid chart lower bound")?;
        require(
            upper[axis].is_null() || upper[axis].as_u64().is_some_and(|hi| hi >= lo),
            "invalid chart upper bound",
        )?;
        if !restrictions.iter().any(|&(i, _)| i == axis) {
            require(
                lo <= 1 && upper[axis].is_null(),
                "free priority axes require lower0/1 and no upper bound",
            )?;
        }
    }
    if r["schema"] == geometry_two_protected::SCHEMA {
        geometry_two_protected::parse(r, n)?;
        return Ok(n);
    }
    if r["schema"] == geometry_tangent::SCHEMA {
        nomination_mode(r)?;
        require(
            r.get("sources").is_none(),
            "geometry nomination cannot also prescribe sources",
        )?;
        let axis = nomination_index(r, "numerator_axis")?;
        let loop_index = nomination_index(r, "differentiated_loop")?;
        require(
            axis < n && loop_index < n,
            "nomination axis/loop exceeds input arity",
        )?;
        require(
            mask.as_bytes()[axis] == b'0' && lower[axis].as_u64().unwrap() >= 1,
            "nominated numerator chart must be strictly negative",
        )?;
        return Ok(n);
    }
    let sources = array(r, "sources")?;
    require(
        !sources.is_empty() && sources.len() <= limit(r, "max_source_rows")?,
        "source count outside allowance",
    )?;
    let mut seen = BTreeSet::new();
    for source in sources {
        let id = source["source_row"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("native source_row required")?;
        require(
            source.get("source_ordinal").is_none(),
            "prescribed chart uses named native RowIds only",
        )?;
        let offset = integers(&source["offset"], n)?;
        checked(IntegralShift::try_new(offset.clone()))?;
        require(seen.insert((id, offset)), "duplicate source RowId/offset")?;
        let weight = integers(&source["weight"], 2)?;
        require(
            weight[0] != 0 && weight[1] != 0,
            "nonzero rational source weights required",
        )?;
    }
    Ok(n)
}

fn nomination_index(r: &Value, key: &str) -> Result<usize> {
    r["nomination"][key]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("nomination {key} must be an index"))
}

fn nomination_mode(r: &Value) -> Result<&str> {
    match r["nomination"].get("mode") {
        None => Ok("radial"),
        Some(value) => match value.as_str() {
            Some("radial") => Ok("radial"),
            Some("protected-gradient") => Ok("protected-gradient"),
            _ => Err("unknown geometry nomination mode".into()),
        },
    }
}

fn policies(r: &Value) -> Result<OriginalSourceCombinationLimits> {
    let exact = ExactAlgebraLimits {
        max_exponent: checked(u16::try_from(limit(r, "max_exponent")?))?,
        max_polynomial_terms: limit(r, "max_polynomial_terms")?,
        max_term_operations: limit(r, "max_term_operations")?,
    };
    let indexed = IndexedAlgebraLimits {
        exact_algebra: exact,
        max_specialization_power_operations: limit(r, "max_term_operations")?,
        max_specialization_integer_bits: limit(r, "max_term_operations")?,
    };
    let mut p = OriginalSourceCombinationLimits::default();
    p.source_generation.relation_limits.arithmetic = indexed;
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
    p.translated_sources.relation.arithmetic = indexed;
    p.rule.indexed_algebra = indexed;
    p.rule.max_source_rows = limit(r, "max_complete_source_rows")?;
    p.rule.max_source_combination_terms = limit(r, "max_source_rows")?;
    p.rule.max_replay_exact_operations = limit(r, "max_term_operations")?;
    p.rule.max_rule_guards = limit(r, "max_conditions")?;
    p.rule.max_shift_columns = limit(r, "max_terms")?;
    p.cell.indexed_algebra = indexed;
    p.cell.relation.arithmetic = indexed;
    p.cell.max_source_views = limit(r, "max_source_rows")?;
    p.cell.max_retained_terms = limit(r, "max_terms")?;
    p.cell.max_guards = limit(r, "max_conditions")?;
    p.cell.max_fixed_restrictions = array(&r["chart"], "lower")?.len();
    p.cell.max_guard_split_coordinate_cells = limit(r, "max_coordinate_cells")?;
    p.geometry.max_requested_boxes = limit(r, "max_cells")?;
    p.geometry.max_uncovered_boxes = limit(r, "max_cells")?;
    p.geometry.max_requested_box_coordinate_cells = limit(r, "max_coordinate_cells")?;
    p.geometry.max_uncovered_box_coordinate_cells = limit(r, "max_coordinate_cells")?;
    p.geometry.max_split_operations = limit(r, "max_term_operations")?;
    Ok(p)
}

fn proof_summary(proof: &CheckedOriginalSourceCombination) -> Value {
    let cells=proof.cells().enumerate().map(|(i,cell)|{
        let (lower,upper)=proof.cell_bounds(i).unwrap();
        json!({"ordinal":i,"lower":lower,"upper":upper,"rhs_terms":cell.rule().right_hand_side().len(),
            "guards":cell.rule().nonzero_guards().iter().map(|g|json!({"polynomial":g.polynomial().raw().to_string(),"origins":format!("{:?}",g.origins())})).collect::<Vec<_>>()})
    }).collect::<Vec<_>>();
    json!({"family_fingerprint":proof.family_fingerprint(),"context_fingerprint":proof.context_fingerprint(),
        "requested_lower":proof.requested_bounds().0,"requested_upper":proof.requested_bounds().1,"checked_sign_cells":cells})
}

// Shared by prescribed and nominated sources: no incidence-dependent clipping.
fn source_product(
    c: &IndexedCoefficientContext,
    sources: &SelectedTranslatedSourceBatch,
    weights: &BTreeMap<TranslatedSourceRequest, IndexedCoefficient>,
    mut conditions: Vec<IndexedPolynomial>,
    r: &Value,
) -> Result<(
    BTreeMap<IndexShift, IndexedCoefficient>,
    Vec<IndexedPolynomial>,
)> {
    require(
        sources.requests().iter().eq(weights.keys()),
        "canonical source order changed",
    )?;
    let exact = policies(r)?.cell.indexed_algebra.exact_algebra;
    let mut product: BTreeMap<IndexShift, IndexedCoefficient> = BTreeMap::new();
    let mut operations = 0usize;
    for (source, weight) in sources.sources().iter().zip(weights.values()) {
        conditions.push(checked(c.denominator_condition_with_limits(weight, exact))?);
        for guard in source.nonzero_conditions() {
            conditions.push(guard.polynomial().clone());
        }
        for (shift, term) in source.terms() {
            operations = operations
                .checked_add(2)
                .ok_or("operation count overflow")?;
            require(
                operations <= limit(r, "max_term_operations")?,
                "source product work exceeds allowance",
            )?;
            conditions.push(checked(c.denominator_condition_with_limits(term, exact))?);
            let value = checked(c.mul_with_limits(weight, term, exact))?;
            let value = match product.remove(shift) {
                Some(old) => checked(c.add_with_limits(&old, &value, exact))?,
                None => value,
            };
            if !value.is_zero() {
                product.insert(shift.clone(), value);
            }
        }
        require(
            product.len() <= limit(r, "max_terms")?
                && conditions.len() <= limit(r, "max_conditions")?,
            "product/conditions exceed allowance",
        )?;
    }
    Ok((product, conditions))
}

fn produce<const N: usize>(
    bytes: &[u8],
    r: &Value,
    export: bool,
) -> Result<(Value, Option<Vec<u8>>)> {
    let started = Instant::now();
    let policy = policies(r)?;
    let ingress = owner_load_limits(r)?;
    let arithmetic = policy.cell.indexed_algebra;
    let exact = arithmetic.exact_algebra;
    let sector: [bool; N] =
        std::array::from_fn(|i| r["owner_mask"].as_str().unwrap().as_bytes()[i] == b'1');
    let mask = checked(Mask::try_new(sector))?;
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
        "family fingerprint mismatch",
    )?;
    let programs = Arc::new(programs);
    let bound = checked(programs.bind_owner_search(sector, Default::default()))?;
    require(
        bound.owner_ordering().stable_id() == r["expected_order"].as_str().unwrap(),
        "persisted order mismatch",
    )?;
    let generator = checked(ParametricIbpGenerator::try_new_with_config(
        &family,
        policy.source_generation,
    ))?;
    let c = generator.context();
    let prepared = checked(generator.prepare_ordinary_ibp())?;
    let ordinary_count = prepared.len();
    require(
        ordinary_count <= limit(r, "max_complete_source_rows")?,
        "complete source count exceeds allowance",
    )?;
    let rows = (0..ordinary_count).map(|i| prepared.generate(i)).collect();
    let completed = checked(prepared.complete(rows))?;
    let mut inventory_limits = policy.translated_sources;
    inventory_limits.max_requested_source_translations = limit(r, "max_complete_source_rows")?;
    inventory_limits.max_translated_sources = limit(r, "max_complete_source_rows")?;
    let zero = checked(IntegralShift::try_new([0_i64; N]))?;
    let inventory = checked(generator.translate_selected_completed_source_rows(
        &completed,
        (0..ordinary_count).map(|i| TranslatedSourceRequest::new(i, zero.clone())),
        inventory_limits,
    ))?;
    let ids: BTreeMap<_, _> = inventory
        .sources()
        .iter()
        .map(|s| {
            (
                s.provenance().source_row().stable_string(),
                s.provenance().source_ordinal(),
            )
        })
        .collect();
    let mut weights = BTreeMap::new();
    let mut nominated_conditions = Vec::new();
    let mut nomination_report = None;
    let mut materialized_product = None;
    if r["schema"] == geometry_two_protected::SCHEMA {
        let native = geometry_two_protected::nominate(&family, &generator, &completed, r)?;
        for ((row, offset), weight) in native.nomination.weights {
            let ordinal = *ids
                .get(&row.stable_string())
                .ok_or("nominated RowId absent from native inventory")?;
            require(
                weights
                    .insert(TranslatedSourceRequest::new(ordinal, offset), weight)
                    .is_none(),
                "duplicate nominated source pair",
            )?;
        }
        nominated_conditions = native.nomination.conditions;
        nomination_report = Some(native.nomination.report);
        materialized_product = Some(native.full_product);
    } else if r["schema"] == geometry_tangent::SCHEMA {
        let nominate = match nomination_mode(r)? {
            "radial" => geometry_tangent::nominate,
            "protected-gradient" => geometry_tangent::gradient::nominate,
            _ => unreachable!("mode was validated"),
        };
        let nomination = nominate(
            &family,
            c,
            &sector,
            nomination_index(r, "differentiated_loop")?,
            nomination_index(r, "numerator_axis")?,
            geometry_tangent::Limits {
                arithmetic: exact,
                max_sources: limit(r, "max_source_rows")?,
                max_conditions: limit(r, "max_conditions")?,
                max_operations: limit(r, "max_term_operations")?,
            },
        )?;
        for ((row, offset), weight) in nomination.weights {
            let ordinal = *ids
                .get(&row.stable_string())
                .ok_or("nominated RowId absent from native inventory")?;
            require(
                weights
                    .insert(TranslatedSourceRequest::new(ordinal, offset), weight)
                    .is_none(),
                "duplicate nominated source pair",
            )?;
        }
        nominated_conditions = nomination.conditions;
        let mut report = nomination.report;
        let restrictions = fixed(r)?;
        report["other_dependent_inactive_chart"] = json!(report["other_dependent_inactive_axes"].as_array().unwrap().iter().map(|axis| {
            let axis = axis.as_u64().unwrap() as usize;
            json!({"axis":axis,"fixed_physical_value":restrictions.iter().find_map(|&(i,v)| (i==axis).then_some(v)),
                "local_lower":r["chart"]["lower"][axis],"local_upper":r["chart"]["upper"][axis]})
        }).collect::<Vec<_>>());
        nomination_report = Some(report);
    } else {
        for source in array(r, "sources")? {
            let ordinal = *ids
                .get(source["source_row"].as_str().unwrap())
                .ok_or("native source RowId absent")?;
            let request = TranslatedSourceRequest::new(
                ordinal,
                checked(IntegralShift::try_new(integers(&source["offset"], N)?))?,
            );
            let q = integers(&source["weight"], 2)?;
            let weight = checked(c.div_with_limits(&c.integer(q[0]), &c.integer(q[1]), exact))?;
            require(
                weights.insert(request, weight).is_none(),
                "duplicate resolved source pair",
            )?;
        }
    }
    let sources = checked(generator.translate_selected_completed_source_rows(
        &completed,
        weights.keys().cloned(),
        policy.translated_sources,
    ))?;
    let (product, mut conditions) = source_product(c, &sources, &weights, nominated_conditions, r)?;
    if let Some(expected) = materialized_product {
        require(
            product == expected,
            "common full source product differs from native tangent materialization",
        )?;
        nomination_report.as_mut().unwrap()["full_product_equality_checked"] = json!(true);
    }
    let unrestricted_terms = product.len();
    let restrictions = fixed(r)?;
    let target = product
        .keys()
        .find(|shift| shift.values().iter().all(|&n| n == 0))
        .cloned()
        .ok_or("prescribed full source sum has no target")?;
    let raw_pivot = product
        .get(&target)
        .ok_or("prescribed full source sum has no target")?;
    let (pivot, pivot_guard) =
        checked(c.specialize_fixed_indices(raw_pivot, &restrictions, arithmetic))?;
    conditions.push(pivot_guard);
    require(!pivot.is_zero(), "target vanishes on requested chart")?;
    let inverse = checked(c.div_with_limits(&c.one(), &pivot, exact))?;
    conditions.push(checked(
        c.denominator_condition_with_limits(&inverse, exact),
    )?);
    let mut contributions = Vec::new();
    for (source, weight) in sources.sources().iter().zip(weights.values()) {
        contributions.push(OriginalSourceContribution {
            source_row: source.provenance().source_row().clone(),
            offset: source.provenance().offset().clone(),
            weight: checked(c.div_with_limits(weight, &pivot, exact))?,
        });
    }
    let mut rhs = Vec::new();
    for (shift, raw) in &product {
        // Preserve the specialization witness BEFORE zero removal or target omission.
        let (value, guard) = checked(c.specialize_fixed_indices(raw, &restrictions, arithmetic))?;
        conditions.push(guard);
        if shift == &target || value.is_zero() {
            continue;
        }
        let value =
            checked(c.div_with_limits(&checked(c.neg_with_limits(&value, exact))?, &pivot, exact))?;
        conditions.push(checked(c.denominator_condition_with_limits(&value, exact))?);
        rhs.push((shift.clone(), value));
    }
    require(!rhs.is_empty(), "empty RHS is not a rule candidate")?;
    require(
        conditions.len() <= limit(r, "max_conditions")?,
        "normalized conditions exceed allowance",
    )?;
    let source_view=contributions.iter().map(|s|json!({"source_row":s.source_row.stable_string(),"offset":s.offset.values(),"normalized_weight":coefficient(&s.weight)})).collect::<Vec<_>>();
    let rhs_view = rhs
        .iter()
        .map(|(s, c)| json!({"shift":s.values(),"coefficient":coefficient(c)}))
        .collect::<Vec<_>>();
    let conditions_view = conditions
        .iter()
        .map(|p| p.raw().to_string())
        .collect::<Vec<_>>();
    let proposal = OriginalSourceCombinationRequest {
        root_sector: checked(Mask::try_new(*bound.owner_root()))?,
        sector: mask,
        ordering: bound.owner_ordering().clone(),
        lower: checked(serde_json::from_value(r["chart"]["lower"].clone()))?,
        upper: checked(serde_json::from_value(r["chart"]["upper"].clone()))?,
        fixed: restrictions
            .iter()
            .map(|&(a, v)| FixedIndexRestriction::new(a, v))
            .collect(),
        contributions,
        rhs,
        retained_conditions: conditions,
    };
    let preparation_seconds = started.elapsed().as_secs_f64();
    let proof_start = Instant::now();
    let proof = checked(check_original_source_combination(
        &family,
        proposal.clone(),
        policy,
    ))?;
    let isolated_proof_seconds = proof_start.elapsed().as_secs_f64();
    let mut report = json!({"schema":r["schema"],"status":"EXACT_ORIGINAL_SOURCE_CHART_PROVED","request":r,
        "ordinary_sources_generated":ordinary_count,"selected_sources":sources.len(),"unspecialized_product_terms":unrestricted_terms,
        "target_coefficient_derived":coefficient(&pivot),"normalized_sources":source_view,"normalized_rhs":rhs_view,
        "retained_pre_cancellation_conditions":conditions_view,"proof":proof_summary(&proof),
        "preparation_seconds":preparation_seconds,"isolated_proof_seconds":isolated_proof_seconds,
        "zero_sector_terms_discarded":false,"point_weight_lift":false,"recursive_walk":false,
        "production_modified":false,"coverage_or_cost_claim":false,"export_proof_attached_to_native_object_not_json":true});
    if let Some(nomination) = nomination_report {
        report["nomination"] = nomination;
    }
    let mut candidate = None;
    if export {
        let export_start = Instant::now();
        let before = checked(inspect_generated_candidate_bundle(bytes, ingress.bundle))?;
        let owner = checked(encode_checked_priority_owner_with_policy::<N>(
            bytes,
            proposal,
            policy,
            ingress.bundle,
            RuleDispatchPolicy::AfterBaselinePartitionWholePiece,
        ))?;
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
            "priority export changed base scope/counts",
        )?;
        require(
            owner.bytes().len() <= limit(r, "max_output_bytes")?,
            "owner output exceeds allowance",
        )?;
        report["status"] = json!("CHECKED_PRIORITY_OWNER_EXPORTED");
        report["export"] = json!({"base_owner_blake3":owner.base_owner_blake3(),"owner_blake3":owner.owner_blake3(),
            "dispatch_policy":format!("{:?}",owner.dispatch_policy()),"base_rules":before.generated_rules,"candidate_rules":after.generated_rules,
            "unchanged_terminal_count":after.finite_residuals,"proof":proof_summary(owner.proof()),
            "source_rechecked_by_exporter":true,"native_codec_roundtrip_checked_by_exporter":true,
            "bytes_alone_replay_source_proof":false,"seconds":export_start.elapsed().as_secs_f64()});
        candidate = Some(owner.bytes().to_vec());
    }
    report["total_seconds"] = json!(started.elapsed().as_secs_f64());
    Ok((report, candidate))
}

fn dispatch(bytes: &[u8], r: &Value, n: usize, export: bool) -> Result<(Value, Option<Vec<u8>>)> {
    macro_rules! arities{($($n:literal),*)=>{match n{$($n=>produce::<$n>(bytes,r,export),)* _=>Err("unsupported arity".into())}};}
    arities!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}
fn main_result() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    require(
        (args.len() == 2 && args[0] == "validate")
            || (args.len() == 3 && args[0] == "prove")
            || (args.len() == 4 && args[0] == "export"),
        "usage: prescribed_source validate REQUEST | prove OWNER REQUEST | export OWNER REQUEST FRESH_DIRECTORY",
    )?;
    let request_index = if args[0] == "validate" { 1 } else { 2 };
    let request_bytes = read(Path::new(&args[request_index]), 1 << 20)?;
    let request: Value = checked(serde_json::from_slice(&request_bytes))?;
    let n = validate(&request)?;
    let (report, candidate) = if args[0] == "validate" {
        (
            json!({"status":"INPUT_SHAPE_VALID","arity":n,"native_loaded":false}),
            None,
        )
    } else {
        dispatch(
            &read(Path::new(&args[1]), limit(&request, "max_owner_bytes")?)?,
            &request,
            n,
            args[0] == "export",
        )?
    };
    let text = checked(serde_json::to_string_pretty(&report))?;
    require(
        text.len() <= limit(&request, "max_report_bytes")?,
        "report exceeds allowance; no partial authority",
    )?;
    if let Some(bytes) = candidate {
        let dest = Path::new(&args[3]);
        checked(fs::create_dir(dest))?;
        fresh_file(&dest.join("request.json"), &request_bytes)?;
        fresh_file(&dest.join("candidate.rrbin"), &bytes)?;
        fresh_file(&dest.join("proof-export.json"), text.as_bytes())?;
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

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Value {
        json!({"schema":SCHEMA,"owner_mask":"1","family_fingerprint":"fixture","expected_order":"rustred.spired-uncut-sector-order.v1",
        "chart":{"lower":[1],"upper":[null],"fixed":[]},"sources":[{"source_row":"ordinary-ibp:0:0","offset":[-1],"weight":[1,1]}],
        "limits":{"max_owner_bytes":1048576,"max_output_bytes":1048576,"max_source_rows":8,"max_complete_source_rows":8,"max_terms":1000,"max_conditions":1000,"max_coordinate_cells":10000,"max_cells":64,"max_polynomial_terms":1000,"max_term_operations":100000,"max_exponent":64,"max_report_bytes":1048576}})
    }
    #[test]
    fn optional_owner_ingress_preserves_defaults_and_maps_only_transport_fields() {
        let mut r = request();
        let defaults = CandidateOwnerLoadLimits::default();
        let absent = owner_load_limits(&r).unwrap();
        let snapshot = |p: &CandidateOwnerLoadLimits| {
            [
                p.bundle.max_bundle_bytes,
                p.max_total_input_bytes,
                p.bundle.max_total_coefficient_bytes,
                p.bundle.max_collection_entries,
                p.max_total_symbolica_state_bytes,
                p.max_zero_sector_visits,
                p.bundle.max_coefficient_bytes,
            ]
        };
        assert_eq!(snapshot(&absent), snapshot(&defaults));
        let fields = [
            "max_bundle_bytes",
            "max_total_input_bytes",
            "max_total_coefficient_bytes",
            "max_collection_entries",
            "max_total_symbolica_state_bytes",
            "max_zero_sector_visits",
            "max_coefficient_bytes",
        ];
        r["owner_load_limits"] = json!({});
        for (i, name) in fields.iter().enumerate() {
            r["owner_load_limits"][name] = json!(1000 + i);
        }
        let explicit = owner_load_limits(&r).unwrap();
        assert_eq!(
            snapshot(&explicit),
            [1000, 1001, 1002, 1003, 1004, 1005, 1006]
        );
        assert_eq!(explicit.bundle.exact_algebra, defaults.bundle.exact_algebra);
        for bad in [json!(null), json!({}), json!({"invented":1})] {
            let mut wrong = r.clone();
            wrong["owner_load_limits"] = bad;
            assert!(validate(&wrong).is_err());
        }
        for bad in [
            json!(0),
            json!(-1),
            json!(rustred_app::MAX_CANDIDATE_BUNDLE_BYTES + 1),
        ] {
            let mut wrong = r.clone();
            wrong["owner_load_limits"]["max_bundle_bytes"] = bad;
            assert!(validate(&wrong).is_err());
        }
        r["owner_load_limits"]["invented"] = json!(1);
        assert!(validate(&r).is_err());
    }
    #[test]
    fn malformed_sources_and_unsupported_chart_refuse() {
        assert_eq!(validate(&request()).unwrap(), 1);
        for case in 0..5 {
            let mut r = request();
            match case {
                0 => r["sources"][0]["weight"] = json!([1, 0]),
                1 => r["chart"]["upper"] = json!([3]),
                2 => r["chart"]["fixed"] = json!([[0, 1]]),
                3 => {
                    let duplicate = r["sources"][0].clone();
                    r["sources"].as_array_mut().unwrap().push(duplicate);
                }
                _ => r["sources"][0]["offset"] = json!([]),
            }
            assert!(validate(&r).is_err());
        }
    }
    #[test]
    fn geometry_schema_keeps_prescribed_sources_separate() {
        let mut r = request();
        r["schema"] = json!(geometry_tangent::SCHEMA);
        r["owner_mask"] = json!("0");
        r["nomination"] = json!({"differentiated_loop":0,"numerator_axis":0});
        assert!(validate(&r).is_err());
        r.as_object_mut().unwrap().remove("sources");
        assert_eq!(validate(&r).unwrap(), 1); // Shape only; no family has been loaded.
        for mode in ["radial", "protected-gradient"] {
            let mut explicit = r.clone();
            explicit["nomination"]["mode"] = json!(mode);
            assert_eq!(validate(&explicit).unwrap(), 1);
        }
        let mut unknown = r.clone();
        unknown["nomination"]["mode"] = json!("guess-a-direction");
        assert!(validate(&unknown).is_err());
        for case in 0..3 {
            let mut wrong = r.clone();
            match case {
                0 => wrong["chart"]["lower"] = json!([0]),
                1 => wrong["nomination"]["differentiated_loop"] = json!(-1),
                _ => wrong["owner_mask"] = json!("1"),
            }
            assert!(validate(&wrong).is_err());
        }
    }
    fn fixture() -> (Vec<u8>, Value) {
        let source = r#"schema = "rustred.project.toml.v1"
[family]
name = "prescribed_chart_k1"
loop_momenta = ["q"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P"
expression = "q^2-1"
[target]
powers = [1]
"#;
        let bundle =
            rustred_app::family_candidates(rustred_app::FamilyCandidatesRequest::new(source))
                .unwrap();
        let inspection =
            inspect_generated_candidate_bundle(bundle.bundle(), Default::default()).unwrap();
        let mut r = request();
        r["family_fingerprint"] = json!(inspection.family_fingerprint);
        r["expected_order"] = json!(inspection.integral_order);
        (bundle.bundle().to_vec(), r)
    }
    #[test]
    fn native_bindings_refuse_wrong_family_order_or_source() {
        let (bytes, request) = fixture();
        for field in ["family", "order", "source"] {
            let mut r = request.clone();
            match field {
                "family" => r["family_fingerprint"] = json!("wrong-family"),
                "order" => r["expected_order"] = json!("wrong-order"),
                _ => r["sources"][0]["source_row"] = json!("ordinary-ibp:99:99"),
            }
            assert!(produce::<1>(&bytes, &r, false).is_err(), "{field}");
        }
    }
    #[test]
    fn explicit_ingress_reaches_native_loader_and_default_export_is_unchanged() {
        let (bytes, mut r) = fixture();
        let (_, original) = produce::<1>(&bytes, &r, true).unwrap();
        let p = CandidateOwnerLoadLimits::default();
        r["owner_load_limits"] = json!({
            "max_bundle_bytes":p.bundle.max_bundle_bytes,
            "max_total_input_bytes":p.max_total_input_bytes,
            "max_total_coefficient_bytes":p.bundle.max_total_coefficient_bytes,
            "max_collection_entries":p.bundle.max_collection_entries,
            "max_total_symbolica_state_bytes":p.max_total_symbolica_state_bytes,
            "max_zero_sector_visits":p.max_zero_sector_visits,
            "max_coefficient_bytes":p.bundle.max_coefficient_bytes});
        let (_, explicit) = produce::<1>(&bytes, &r, true).unwrap();
        assert_eq!(original, explicit);
        assert!(explicit.as_ref().unwrap().len() > bytes.len());
        let mut export_capped = r.clone();
        export_capped["owner_load_limits"]["max_bundle_bytes"] = json!(bytes.len());
        assert!(produce::<1>(&bytes, &export_capped, false).is_ok());
        assert!(produce::<1>(&bytes, &export_capped, true).is_err());
        r["owner_load_limits"]["max_total_input_bytes"] = json!(1);
        assert!(produce::<1>(&bytes, &r, false).is_err());
    }
    #[test]
    fn known_tadpole_symbolic_chart_uses_original_proof_and_preserves_suffix() {
        let (base, mut r) = fixture();
        let inspection = inspect_generated_candidate_bundle(&base, Default::default()).unwrap();
        let (report, bytes) = produce::<1>(&base, &r, true).unwrap();
        assert_eq!(report["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
        assert!(bytes.is_some());
        assert_eq!(report["normalized_rhs"].as_array().unwrap().len(), 1);
        assert_eq!(report["export"]["source_rechecked_by_exporter"], true);
        assert_eq!(report["export"]["bytes_alone_replay_source_proof"], false);
        assert_eq!(
            report["export"]["unchanged_terminal_count"],
            inspection.finite_residuals
        );
        r["chart"]["lower"] = json!([0]);
        assert!(produce::<1>(&base, &r, false).is_err());
    }
}
