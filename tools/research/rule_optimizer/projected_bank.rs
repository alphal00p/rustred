//! Finite, input-directed source projection. No rule publication or traversal.
//!
//! Symbolica owns all elimination and exact matrix products. RustRed generates
//! and translates the named ordinary source rows, and supplies the saved global
//! ordering. H changes the discovery projection only, never descent authority.
//! Usage: projected_bank validate REQUEST.json
//!        projected_bank probe OWNER.rrbin REQUEST.json
//! The outer reviewed runner binds file digests and enforces wall/RSS limits.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    fs::File,
    io::Read,
    path::Path,
    sync::Arc,
    time::Instant,
};

use rustred::{
    algebra::{Coefficient, CoefficientContext},
    identity::{
        IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits, TranslatedSourceRequest,
    },
    sector::Mask,
    solver::{Integral, IntegralOrder, Power},
};
use rustred_app::{CandidateOwnerBundle, load_generated_candidate_owners};
use serde_json::{Value, json};
use symbolica::{
    domains::{SelfRing, rational_polynomial::RationalPolynomialField},
    prelude::{IntegerRing, Z},
    tensors::sparse::{LuLMode, SparseMatrix, SparseRowReducer},
};

type Result<T> = std::result::Result<T, String>;
type Field = RationalPolynomialField<IntegerRing, u16>;
type Row<const N: usize> = BTreeMap<Integral<N>, Coefficient>;
const SCHEMA: &str = "rustred.projected-source-bank.v1";
const MAX_REQUEST_BYTES: usize = 1024 * 1024;

fn checked<T, E: Display>(value: std::result::Result<T, E>) -> Result<T> {
    value.map_err(|error| error.to_string())
}
fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn array<'a>(value: &'a Value, name: &str) -> Result<&'a [Value]> {
    value[name]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{name} must be an array"))
}
fn number(value: &Value, name: &str) -> Result<usize> {
    value[name]
        .as_u64()
        .and_then(|x| usize::try_from(x).ok())
        .ok_or_else(|| format!("{name} must be a nonnegative integer"))
}
fn limit(request: &Value, name: &str) -> Result<usize> {
    let value = number(&request["limits"], name)?;
    require(value > 0, "all resource limits must be positive")?;
    Ok(value)
}
fn vector(value: &Value, arity: usize) -> Result<Vec<i64>> {
    let values = value
        .as_array()
        .ok_or("index/offset vector must be an array")?;
    require(
        values.len() == arity,
        "index/offset vector has the wrong arity",
    )?;
    values
        .iter()
        .map(|value| {
            value
                .as_i64()
                .ok_or_else(|| "index/offset must fit i64".into())
        })
        .collect()
}
fn key<const N: usize>(value: &Value) -> Result<Integral<N>> {
    let values = vector(value, N)?
        .into_iter()
        .map(i16::try_from)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let powers: [i16; N] = values.try_into().map_err(|_| "key arity mismatch")?;
    checked(Integral::numeric(powers))
}
fn powers<const N: usize>(key: &Integral<N>) -> Vec<i16> {
    key.powers().iter().map(|power| power.value()).collect()
}
fn coefficient(value: &Coefficient) -> Value {
    json!({"native_display": value.to_string(), "numerator": value.numerator.to_string(),
           "denominator": value.denominator.to_string(),
           "numerator_terms": value.numerator.nterms(), "denominator_terms": value.denominator.nterms()})
}
fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let file = checked(File::open(path))?;
    require(
        checked(file.metadata())?.len() <= maximum as u64,
        "input exceeds byte limit",
    )?;
    let mut bytes = Vec::new();
    checked(file.take(maximum as u64 + 1).read_to_end(&mut bytes))?;
    require(bytes.len() <= maximum, "input grew past byte limit")?;
    Ok(bytes)
}

/// Shape/size validation is CAS-free and can be run before native loading.
fn validate(request: &Value) -> Result<usize> {
    require(request["schema"] == SCHEMA, "unknown request schema")?;
    let arity = array(request, "target")?.len();
    require(
        (1..=16).contains(&arity),
        "owner-loader arity must be 1..16",
    )?;
    let target = vector(&request["target"], arity)?;
    require(
        target
            .iter()
            .all(|&n| (i64::from(Power::MIN)..=i64::from(Power::MAX)).contains(&n)),
        "target exceeds compact integral range",
    )?;
    let mask = request["owner_mask"]
        .as_str()
        .ok_or("owner_mask must be a string")?;
    require(
        mask.len() == arity && mask.bytes().all(|c| c == b'0' || c == b'1'),
        "invalid owner mask",
    )?;
    require(
        mask.bytes()
            .zip(&target)
            .all(|(b, n)| (b == b'1') == (*n > 0)),
        "target support differs from owner",
    )?;
    require(
        request["expected_order"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "expected_order is required",
    )?;
    require(
        request["family_fingerprint"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "family_fingerprint is required",
    )?;
    for name in [
        "max_owner_bytes",
        "max_source_rows",
        "max_complete_source_rows",
        "max_translated_terms",
        "max_source_conditions",
        "max_coordinate_cells",
        "max_physical_columns",
        "max_matrix_nonzeros",
        "max_augmented_columns",
        "max_reducer_nonzeros",
        "max_coefficient_terms",
        "max_report_bytes",
    ] {
        limit(request, name)?;
    }
    let expensive = array(request, "expensive_keys")?;
    require(
        !expensive.is_empty(),
        "H must name at least one explicitly selected lower key",
    )?;
    require(
        expensive.len() <= limit(request, "max_physical_columns")?,
        "H exceeds column allowance",
    )?;
    let mut seen = BTreeSet::new();
    for value in expensive {
        let k = vector(value, arity)?;
        require(
            k.iter()
                .all(|&n| (i64::from(Power::MIN)..=i64::from(Power::MAX)).contains(&n)),
            "H key exceeds compact integral range",
        )?;
        require(k != target, "H must not contain the target")?;
        require(seen.insert(k), "duplicate H key")?;
    }
    let sources = array(request, "sources")?;
    require(
        !sources.is_empty() && sources.len() <= limit(request, "max_source_rows")?,
        "source-bank size outside allowance",
    )?;
    let mut pairs = BTreeSet::new();
    for source in sources {
        require(
            source.get("source_ordinal").is_some() != source.get("source_row").is_some(),
            "each source needs exactly one ordinal or native RowId string",
        )?;
        let identity = if source.get("source_ordinal").is_some() {
            format!("ordinal:{}", number(source, "source_ordinal")?)
        } else {
            format!(
                "row:{}",
                source["source_row"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or("source_row must be a native RowId string")?
            )
        };
        let offset = vector(&source["offset"], arity)?;
        checked(IntegralShift::try_new(offset.clone()))?;
        require(
            pairs.insert((identity, offset)),
            "duplicate selected source pair",
        )?;
    }
    if let Some(order) = request.get("source_visitation") {
        let order = order
            .as_array()
            .ok_or("source_visitation must be an array")?;
        let positions = order
            .iter()
            .map(|x| x.as_u64().and_then(|n| usize::try_from(n).ok()))
            .collect::<Option<Vec<_>>>()
            .ok_or("invalid source visitation ordinal")?;
        require(
            positions.len() == sources.len()
                && positions.into_iter().collect::<BTreeSet<_>>() == (0..sources.len()).collect(),
            "source_visitation must permute canonical selected source ordinals",
        )?;
    }
    Ok(arity)
}

fn add<const N: usize>(
    row: &mut Row<N>,
    key: Integral<N>,
    value: Coefficient,
    context: &CoefficientContext,
) -> Result<()> {
    let value = match row.remove(&key) {
        Some(old) => checked(context.try_add(&old, &value, Default::default()))?,
        None => value,
    };
    if !value.is_zero() {
        row.insert(key, value);
    }
    Ok(())
}

/// F consists only of non-descending non-target columns under the immutable
/// saved order. Optional H is an additional discovery constraint, not an order.
fn forbidden<const N: usize>(
    k: &Integral<N>,
    target: &Integral<N>,
    order: &IntegralOrder<N>,
    expensive: &BTreeSet<Integral<N>>,
) -> bool {
    k != target && (order.compare(k, target) != Ordering::Greater || expensive.contains(k))
}

fn solve<const N: usize>(
    frame: &SparseMatrix<Field>,
    columns: &[Integral<N>],
    target: &Integral<N>,
    order: &IntegralOrder<N>,
    expensive: &BTreeSet<Integral<N>>,
    base: &CoefficientContext,
    request: &Value,
) -> Result<Value> {
    let started = Instant::now();
    let bad: Vec<_> = columns
        .iter()
        .enumerate()
        .filter_map(|(i, k)| forbidden(k, target, order, expensive).then_some(i))
        .collect();
    let target_column = columns
        .iter()
        .position(|k| k == target)
        .ok_or("target column absent")?;
    let projected_target = checked(u32::try_from(bad.len()))?;
    let identity_start = projected_target
        .checked_add(1)
        .ok_or("projected width overflow")?;
    let width = identity_start
        .checked_add(frame.nrows())
        .ok_or("augmented width overflow")?;
    require(
        width as usize <= limit(request, "max_augmented_columns")?,
        "augmented columns exceed allowance",
    )?;
    let mut reducer = SparseRowReducer::new(width, Field::new(Z), LuLMode::Full);
    let mut pivot_guards = Vec::new();
    for row in 0..frame.nrows() as usize {
        let mut projected = BTreeMap::new();
        for at in frame.row_ptrs()[row]..frame.row_ptrs()[row + 1] {
            let col = frame.col_idcs()[at] as usize;
            let projected_column = if col == target_column {
                Some(projected_target)
            } else {
                bad.binary_search(&col).ok().map(|i| i as u32)
            };
            if let Some(col) = projected_column {
                projected.insert(col, frame.values()[at].clone());
            }
        }
        projected.insert(identity_start + row as u32, base.one());
        let ids: Vec<_> = projected.keys().copied().collect();
        let values: Vec<_> = projected.into_values().collect();
        let pivot = reducer
            .add_row(&values, &ids)
            .ok_or("native identity column unexpectedly dependent")?;
        require(
            reducer.u().nvalues() + reducer.l().nvalues()
                <= limit(request, "max_reducer_nonzeros")?,
            "native reducer fill exceeds allowance",
        )?;
        let (_, _, scales) = reducer.l().last_row().ok_or("native L row missing")?;
        let scale = scales.last().ok_or("native pivot scale missing")?;
        require(
            !scale.numerator.is_zero() && !scale.denominator.is_zero(),
            "zero native pivot scale",
        )?;
        pivot_guards.push(
            json!({"source_row": row, "numerator_nonzero": scale.numerator.to_string(),
                                "denominator_nonzero": scale.denominator.to_string()}),
        );
        if pivot != projected_target {
            continue;
        }
        let (_, uc, uv) = reducer.u().last_row().ok_or("native U row missing")?;
        require(
            uc.first() == Some(&projected_target) && uv.first().is_some_and(|v| v.is_one()),
            "target pivot is not normalized",
        )?;
        let mut weight_ids = Vec::new();
        let mut weight_values = Vec::new();
        for (&col, value) in uc.iter().zip(uv).skip(1) {
            require(
                col >= identity_start && col - identity_start <= row as u32,
                "uneliminated projected column",
            )?;
            weight_ids.push(col - identity_start);
            weight_values.push(value.clone());
        }
        let source_weights: Vec<_> = weight_ids
            .iter()
            .zip(&weight_values)
            .map(|(&id, value)| json!({"source_row": id, "weight": coefficient(value)}))
            .collect();
        let mut weights = SparseMatrix::new(0, frame.nrows(), Field::new(Z));
        weights.add_row(weight_values, weight_ids);
        // Existing native product over ALL original columns. No unprojected
        // tail coefficient is discarded or treated as free downstream work.
        let product = &weights * frame;
        require(product.nrows() == 1, "wrong full-product row count")?;
        let mut target_seen = false;
        let mut rhs = Vec::new();
        for (&col, value) in product.col_idcs().iter().zip(product.values()) {
            if value.is_zero() {
                continue;
            }
            let k = &columns[col as usize];
            if k == target {
                require(value.is_one(), "full product target coefficient is not one")?;
                target_seen = true;
            } else {
                require(
                    !forbidden(k, target, order, expensive),
                    "full product retains forbidden coefficient",
                )?;
                require(
                    order.compare(k, target) == Ordering::Greater,
                    "RHS does not strictly descend under saved order",
                )?;
                rhs.push(json!({"key": powers(k), "coefficient": coefficient(&-value.clone())}));
            }
        }
        require(target_seen, "full product lost target")?;
        return Ok(
            json!({"status": "EXACT_FINITE_SOURCE_COMBINATION", "first_target_prefix": row + 1,
            "forbidden_columns": bad.len(), "rhs": rhs, "source_weights": source_weights,
            "source_weight_support": weights.nvalues(), "pivot_guards_conservative": pivot_guards,
            "native_reducer_nonzeros": reducer.u().nvalues() + reducer.l().nvalues(),
            "full_original_source_product_checked": true, "target_coefficient_one": true,
            "all_forbidden_coefficients_zero": true, "same_saved_order_strict_descent": true,
            "seconds": started.elapsed().as_secs_f64()}),
        );
    }
    Ok(
        json!({"status": "NO_TARGET_PIVOT_IN_FIXED_BANK", "forbidden_columns": bad.len(),
        "pivot_guards_conservative": pivot_guards, "native_reducer_nonzeros": reducer.u().nvalues() + reducer.l().nvalues(),
        "seconds": started.elapsed().as_secs_f64(), "master_or_irreducibility_claim": false}),
    )
}

fn probe<const N: usize>(bytes: &[u8], request: &Value) -> Result<Value> {
    let started = Instant::now();
    let target = key::<N>(&request["target"])?;
    let sector: [bool; N] = std::array::from_fn(|i| target[i].value() > 0);
    let mask = checked(Mask::try_new(sector))?;
    let (family, programs) = checked(load_generated_candidate_owners::<N>(
        &[CandidateOwnerBundle {
            bytes,
            owner_sector: &mask,
        }],
        Default::default(),
        Default::default(),
    ))?;
    require(
        family.fingerprint() == request["family_fingerprint"].as_str().unwrap(),
        "family fingerprint differs",
    )?;
    let programs = Arc::new(programs);
    let bound = checked(programs.bind_owner_search(sector, Default::default()))?;
    require(
        bound.owner_ordering().stable_id() == request["expected_order"].as_str().unwrap(),
        "saved order differs",
    )?;
    let order = checked(IntegralOrder::from_persisted_policy(
        sector,
        bound.owner_ordering(),
    ))?;
    let expensive: BTreeSet<_> = array(request, "expensive_keys")?
        .iter()
        .map(key::<N>)
        .collect::<Result<_>>()?;
    require(
        expensive
            .iter()
            .all(|k| order.compare(k, &target) == Ordering::Greater),
        "H contains a non-descending key",
    )?;
    let generator = checked(ParametricIbpGenerator::try_new(&family))?;
    let prepared = checked(generator.prepare_ordinary_ibp())?;
    require(
        prepared.len() <= limit(request, "max_complete_source_rows")?,
        "complete ordinary source batch exceeds allowance",
    )?;
    let complete_source_rows = prepared.len();
    let generated = (0..complete_source_rows)
        .map(|i| prepared.generate(i))
        .collect();
    let completed = checked(prepared.complete(generated))?;
    // Resolve named source IDs by asking the existing zero-translation service
    // for the complete source inventory. No loop-index-to-ordinal formula is
    // copied into this adapter. This bounded preparation is charged below.
    let native_source_ids = if array(request, "sources")?
        .iter()
        .any(|s| s.get("source_row").is_some())
    {
        let zero = checked(IntegralShift::try_new([0_i64; N]))?;
        let inventory = checked(
            generator.translate_selected_completed_source_rows(
                &completed,
                (0..complete_source_rows)
                    .map(|ordinal| TranslatedSourceRequest::new(ordinal, zero.clone())),
                TranslatedSourceLimits {
                    max_requested_source_translations: limit(request, "max_complete_source_rows")?,
                    max_requested_offsets: 1,
                    max_translated_sources: limit(request, "max_complete_source_rows")?,
                    max_translated_term_entries: limit(request, "max_translated_terms")?,
                    max_translated_condition_entries: limit(request, "max_source_conditions")?,
                    max_retained_condition_source_entries: limit(request, "max_source_conditions")?,
                    max_retained_index_coordinate_cells: limit(request, "max_coordinate_cells")?,
                    ..Default::default()
                },
            ),
        )?;
        inventory
            .sources()
            .iter()
            .map(|source| {
                (
                    source.provenance().source_row().stable_string(),
                    source.provenance().source_ordinal(),
                )
            })
            .collect::<BTreeMap<_, _>>()
    } else {
        BTreeMap::new()
    };
    let requests = array(request, "sources")?
        .iter()
        .map(|source| {
            let ordinal = match source.get("source_row") {
                Some(value) => *native_source_ids
                    .get(value.as_str().unwrap())
                    .ok_or("requested native RowId is absent")?,
                None => number(source, "source_ordinal")?,
            };
            Ok(TranslatedSourceRequest::new(
                ordinal,
                checked(IntegralShift::try_new(vector(&source["offset"], N)?))?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let translated = checked(generator.translate_selected_completed_source_rows(
        &completed,
        requests,
        TranslatedSourceLimits {
            max_requested_source_translations: limit(request, "max_source_rows")?,
            max_requested_offsets: limit(request, "max_source_rows")?,
            max_translated_sources: limit(request, "max_source_rows")?,
            max_translated_term_entries: limit(request, "max_translated_terms")?,
            max_translated_condition_entries: limit(request, "max_source_conditions")?,
            max_retained_condition_source_entries: limit(request, "max_source_conditions")?,
            max_retained_index_coordinate_cells: limit(request, "max_coordinate_cells")?,
            ..Default::default()
        },
    ))?;
    require(
        translated.len() == array(request, "sources")?.len(),
        "two request identities resolve to the same source pair",
    )?;
    let visitation: Vec<usize> = match request.get("source_visitation") {
        Some(value) => value
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_u64().unwrap() as usize)
            .collect(),
        None => (0..translated.len()).collect(),
    };
    let assignment: [i64; N] = std::array::from_fn(|i| i64::from(target[i].value()));
    let context = generator.context();
    let base = family.coefficient_context();
    let mut columns = BTreeSet::from([target]);
    let mut rows = Vec::new();
    let mut provenance = Vec::new();
    let mut source_guards = Vec::new();
    let mut coefficient_terms = 0usize;
    for (row_index, &source_index) in visitation.iter().enumerate() {
        let source = &translated.sources()[source_index];
        provenance.push(json!({"row": row_index, "canonical_selected_ordinal": source_index,
            "source_ordinal": source.provenance().source_ordinal(), "offset": source.provenance().offset().values(),
            "stable": source.provenance().stable_string()}));
        for condition in source.nonzero_conditions() {
            let guard = checked(context.specialize_polynomial(
                condition.polynomial(),
                &assignment,
                Default::default(),
            ))?;
            require(
                !guard.is_zero(),
                "original source condition vanishes at target",
            )?;
            source_guards.push(json!({"row": row_index, "kind": "original_source_nonzero",
                "polynomial": guard.to_string(), "origin": format!("{:?}", condition.sources())}));
        }
        let mut row = Row::new();
        for (shift, coefficient) in source.terms() {
            let mut power = [0_i16; N];
            for axis in 0..N {
                power[axis] = checked(i16::try_from(
                    assignment[axis]
                        .checked_add(shift.values()[axis])
                        .ok_or("translated index overflow")?,
                ))?;
            }
            let k = checked(Integral::numeric(power))?;
            let (value, guard) =
                checked(context.specialize(coefficient, &assignment, Default::default()))?;
            require(
                base.contains(&value),
                "source coefficient changed native variable map",
            )?;
            if let Some(guard) = guard {
                require(
                    !guard.is_zero(),
                    "source specialization denominator vanishes",
                )?;
                source_guards.push(json!({"row": row_index, "kind": "pre_normalization_denominator_nonzero", "polynomial": guard.to_string()}));
            }
            coefficient_terms = coefficient_terms
                .checked_add(value.numerator.nterms())
                .and_then(|n| n.checked_add(value.denominator.nterms()))
                .ok_or("coefficient term count overflow")?;
            require(
                coefficient_terms <= limit(request, "max_coefficient_terms")?,
                "source coefficient terms exceed allowance",
            )?;
            if !value.is_zero() {
                columns.insert(k);
                add(&mut row, k, value, base)?;
            }
        }
        require(
            columns.len() <= limit(request, "max_physical_columns")?,
            "physical columns exceed allowance",
        )?;
        require(
            source_guards.len() <= limit(request, "max_source_conditions")?,
            "retained source guards exceed allowance",
        )?;
        rows.push(row);
    }
    let mut columns: Vec<_> = columns.into_iter().collect();
    columns.sort_by(|a, b| order.compare(a, b));
    let registry: BTreeMap<_, _> = columns
        .iter()
        .enumerate()
        .map(|(i, &key)| (key, i as u32))
        .collect();
    let mut frame = SparseMatrix::new(0, checked(u32::try_from(columns.len()))?, Field::new(Z));
    let max_matrix_nonzeros = limit(request, "max_matrix_nonzeros")?;
    for row in rows {
        let sorted: BTreeMap<_, _> = row
            .into_iter()
            .map(|(key, value)| (registry[&key], value))
            .collect();
        require(
            frame
                .nvalues()
                .checked_add(sorted.len())
                .is_some_and(|n| n <= max_matrix_nonzeros),
            "full source matrix exceeds allowance",
        )?;
        frame.add_row(
            sorted.values().cloned().collect(),
            sorted.keys().copied().collect(),
        );
    }
    let preparation_seconds = started.elapsed().as_secs_f64();
    let baseline = solve(
        &frame,
        &columns,
        &target,
        &order,
        &BTreeSet::new(),
        base,
        request,
    )?;
    let candidate = solve(&frame, &columns, &target, &order, &expensive, base, request)?;
    let exercised: Vec<_> = baseline["rhs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|term| expensive.contains(&key::<N>(&term["key"]).unwrap()))
        .map(|term| term["key"].clone())
        .collect();
    Ok(
        json!({"schema": SCHEMA, "status": "FINITE_SOURCE_BANK_DIAGNOSTIC_COMPLETE", "request": request,
        "family_fingerprint": family.fingerprint(), "saved_order": format!("{:?}", bound.owner_ordering()),
        "exact_base_parameters": base.parameter_names(), "source_rows": frame.nrows(), "matrix_nonzeros": frame.nvalues(),
        "physical_columns": columns.len(), "source_provenance": provenance, "source_conditions": source_guards,
        "H_keys_absent_from_bank": expensive.iter().filter(|k| !registry.contains_key(k)).map(powers).collect::<Vec<_>>(),
        "baseline_H_keys": exercised, "added_constraint_exercised_by_baseline": !exercised.is_empty(),
        "baseline": baseline, "candidate": candidate, "same_source_bank": true,
        "preparation_seconds": preparation_seconds, "total_seconds": started.elapsed().as_secs_f64(),
        "dimension_specialized": false, "zero_sector_terms_discarded": false,
        "coefficient_limit_scope": "max_coefficient_terms bounds source input only; native reducer/product intermediate expressions require the outer RSS/deadline guard",
        "source_authority": "finite exact product of regenerated selected ordinary source rows; no parametric-chart certificate",
        "rule_published": false, "recursive_walk": false, "master_or_coverage_or_performance_claim": false}),
    )
}

fn dispatch(bytes: &[u8], request: &Value, arity: usize) -> Result<Value> {
    // Native owner loading has a const-generic arity API. This is the existing
    // 1..16 transport range, with no topology or loop-count discrimination.
    macro_rules! arities { ($($n:literal),*) => { match arity { $($n => probe::<$n>(bytes, request),)* _ => Err("unsupported arity".into()) } }; }
    arities!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}

fn main_result() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    require(
        (args.len() == 2 && args[0] == "validate") || (args.len() == 3 && args[0] == "probe"),
        "usage: projected_bank validate REQUEST.json | probe OWNER.rrbin REQUEST.json",
    )?;
    let path = Path::new(args.last().unwrap());
    let request: Value = checked(serde_json::from_slice(&read_bounded(
        path,
        MAX_REQUEST_BYTES,
    )?))?;
    let arity = validate(&request)?;
    let report = if args[0] == "validate" {
        json!({"status": "INPUT_SHAPE_VALID", "arity": arity, "native_loaded": false})
    } else {
        dispatch(
            &read_bounded(Path::new(&args[1]), limit(&request, "max_owner_bytes")?)?,
            &request,
            arity,
        )?
    };
    let text = checked(serde_json::to_string_pretty(&report))?;
    require(
        text.len() <= limit(&request, "max_report_bytes")?,
        "report exceeds allowance; no truncated authority",
    )?;
    println!("{text}");
    Ok(())
}

fn main() {
    if let Err(error) = main_result() {
        eprintln!(
            "{}",
            json!({"status": "REFUSED_OR_INCOMPLETE", "error": error, "rule_published": false})
        );
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> Value {
        json!({"schema": SCHEMA, "target": [2], "owner_mask": "1", "expected_order": "rustred.spired-uncut-sector-order.v1",
            "family_fingerprint": "fixture", "expensive_keys": [[1]], "sources": [{"source_ordinal": 0, "offset": [-1]}],
            "limits": {"max_owner_bytes": 1048576, "max_source_rows": 8, "max_complete_source_rows": 8,
                "max_translated_terms": 100, "max_source_conditions": 100, "max_coordinate_cells": 100,
                "max_physical_columns": 100, "max_matrix_nonzeros": 100, "max_augmented_columns": 100,
                "max_reducer_nonzeros": 1000, "max_coefficient_terms": 1000, "max_report_bytes": 1048576}})
    }

    #[test]
    fn input_rejects_bad_arity_source_duplicates_target_in_h_and_nonpermutations() {
        assert_eq!(validate(&request()).unwrap(), 1);
        for field in [
            "arity",
            "duplicate",
            "target",
            "visitation",
            "limit",
            "power",
            "identity",
        ] {
            let mut r = request();
            match field {
                "arity" => r["sources"][0]["offset"] = json!([0, 1]),
                "duplicate" => r["sources"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"source_ordinal":0,"offset":[-1]})),
                "target" => r["expensive_keys"] = json!([[2]]),
                "visitation" => r["source_visitation"] = json!([1]),
                "limit" => r["limits"]["max_source_rows"] = json!(0),
                "power" => r["target"] = json!([64]),
                "identity" => r["sources"][0]["source_row"] = json!("ibp:0:0"),
                _ => unreachable!(),
            }
            assert!(validate(&r).is_err(), "{field}");
        }
    }

    #[test]
    fn native_projection_cancels_h_and_keeps_the_entire_allowed_tail() {
        let base = CoefficientContext::try_new(["d"]).unwrap();
        let target = Integral::numeric([4]).unwrap();
        let h = Integral::numeric([3]).unwrap();
        let columns = [
            target,
            h,
            Integral::numeric([2]).unwrap(),
            Integral::numeric([1]).unwrap(),
        ];
        let order = IntegralOrder::new([true], [false]);
        let mut frame = SparseMatrix::new(0, 4, Field::new(Z));
        frame.add_row(
            vec![base.one(), base.integer(-1), base.integer(-1)],
            vec![0, 1, 2],
        );
        frame.add_row(vec![base.one(), base.integer(-1)], vec![1, 3]);
        let first = solve(
            &frame,
            &columns,
            &target,
            &order,
            &BTreeSet::new(),
            &base,
            &request(),
        )
        .unwrap();
        let second = solve(
            &frame,
            &columns,
            &target,
            &order,
            &BTreeSet::from([h]),
            &base,
            &request(),
        )
        .unwrap();
        assert_eq!(
            first["rhs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x["key"].clone())
                .collect::<Vec<_>>(),
            vec![json!([3]), json!([2])]
        );
        assert_eq!(
            second["rhs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x["key"].clone())
                .collect::<Vec<_>>(),
            vec![json!([2]), json!([1])]
        );
        assert_eq!(second["source_weight_support"], 2);
        assert_eq!(second["full_original_source_product_checked"], true);
    }

    #[test]
    fn non_descending_f_column_must_cancel_before_target_admission() {
        let base = CoefficientContext::try_new(["d"]).unwrap();
        let target = Integral::numeric([4]).unwrap();
        let columns = [
            Integral::numeric([5]).unwrap(),
            target,
            Integral::numeric([2]).unwrap(),
            Integral::numeric([1]).unwrap(),
        ];
        let order = IntegralOrder::new([true], [false]);
        let mut frame = SparseMatrix::new(0, 4, Field::new(Z));
        frame.add_row(
            vec![base.one(), base.one(), base.integer(-1)],
            vec![0, 1, 2],
        );
        let blocked = solve(
            &frame,
            &columns,
            &target,
            &order,
            &BTreeSet::new(),
            &base,
            &request(),
        )
        .unwrap();
        assert_eq!(blocked["status"], "NO_TARGET_PIVOT_IN_FIXED_BANK");
        frame.add_row(vec![base.one(), base.integer(-1)], vec![0, 3]);
        let canceled = solve(
            &frame,
            &columns,
            &target,
            &order,
            &BTreeSet::new(),
            &base,
            &request(),
        )
        .unwrap();
        assert_eq!(canceled["status"], "EXACT_FINITE_SOURCE_COMBINATION");
        assert_eq!(canceled["forbidden_columns"], 1);
        assert_eq!(canceled["source_weight_support"], 2);
        assert_eq!(
            canceled["rhs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x["key"].clone())
                .collect::<Vec<_>>(),
            vec![json!([2]), json!([1])]
        );
        assert_eq!(canceled["all_forbidden_coefficients_zero"], true);
    }

    #[test]
    fn known_massive_tadpole_bank_reports_a_bound_miss_without_terminal_inflation() {
        // Same known family as rustred-app candidate_bundle/tests.rs::K1.
        let source = r#"schema = "rustred.project.toml.v1"
[family]
name = "projected_bank_k1"
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
        let mask = Mask::try_new([true]).unwrap();
        let (family, programs) = load_generated_candidate_owners::<1>(
            &[CandidateOwnerBundle {
                bytes: bundle.bundle(),
                owner_sector: &mask,
            }],
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let mut r = request();
        r["family_fingerprint"] = json!(family.fingerprint());
        let programs = Arc::new(programs);
        let bound = programs
            .bind_owner_search([true], Default::default())
            .unwrap();
        r["expected_order"] = json!(bound.owner_ordering().stable_id().to_string());
        let result = probe::<1>(bundle.bundle(), &r).unwrap();
        assert_eq!(
            result["baseline"]["status"],
            "EXACT_FINITE_SOURCE_COMBINATION"
        );
        assert_eq!(result["baseline_H_keys"], json!([[1]]));
        assert_eq!(
            result["candidate"]["status"],
            "NO_TARGET_PIVOT_IN_FIXED_BANK"
        );
        assert_eq!(result["rule_published"], false);
        assert_eq!(result["zero_sector_terms_discarded"], false);
    }
}
