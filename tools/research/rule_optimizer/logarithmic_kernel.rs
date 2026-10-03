//! Bounded joint logarithmic source-kernel discovery, never rule publication.
//!
//! Symbolica owns polynomial arithmetic, row reduction and matrix products.
//! This adapter only enumerates finite coefficient constraints, binds native
//! source provenance, enforces resource limits and checks exact full replay.
use rustred::{
    algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits},
    family::IntegralFamily,
    identity::{
        IntegralShift, ParametricIbpGenerator, RowId, TranslatedSourceLimits,
        TranslatedSourceRequest,
    },
    sector::Mask,
    solver::{Integral, IntegralOrder, Power},
};
use rustred_app::{
    CandidateOwnerBundle, CandidateOwnerLoadLimits, load_generated_candidate_owners,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    fs::File,
    io::Read,
    path::Path,
    sync::Arc,
    time::Instant,
};
use symbolica::{
    domains::{SelfRing, rational_polynomial::RationalPolynomialField},
    prelude::{IntegerRing, Z},
    tensors::sparse::{LuLMode, SparseMatrix, SparseRowReducer},
};

#[path = "logarithmic_kernel/linear.rs"]
mod linear;
#[path = "logarithmic_kernel/polynomial.rs"]
mod polynomial;
#[path = "logarithmic_kernel/sources.rs"]
mod sources;
#[cfg(test)]
#[path = "logarithmic_kernel/tests.rs"]
mod tests;

type Result<T> = std::result::Result<T, String>;
type Field = RationalPolynomialField<IntegerRing, u16>;
type Matrix = SparseMatrix<Field>;
const SCHEMA: &str = "rustred.finite-logarithmic-kernel.v1";
const MAX_REQUEST_BYTES: usize = 1 << 20;

fn checked<T, E: Display>(x: std::result::Result<T, E>) -> Result<T> {
    x.map_err(|e| e.to_string())
}
fn require(ok: bool, msg: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(msg.into()) }
}
fn number(v: &Value, key: &str) -> Result<usize> {
    v[key]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("invalid {key}"))
}
fn limit(r: &Value, key: &str) -> Result<usize> {
    let n = number(&r["limits"], key)?;
    require(n > 0, "limits must be positive")?;
    Ok(n)
}
fn admit(r: &Value, key: &str, n: usize) -> Result<()> {
    require(n <= limit(r, key)?, &format!("{key} exceeded: {n}"))
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or("resource addition overflow".into())
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or("resource multiplication overflow".into())
}
fn vector(v: &Value, n: usize) -> Result<Vec<i64>> {
    let a = v.as_array().ok_or("vector required")?;
    require(a.len() == n, "vector arity mismatch")?;
    a.iter()
        .map(|x| x.as_i64().ok_or("signed index required".into()))
        .collect()
}
fn coefficient(c: &Coefficient) -> Value {
    json!({"numerator":c.numerator.to_string(),"denominator":c.denominator.to_string(),"display_only":true})
}
fn matrix_rows(m: &Matrix) -> Value {
    json!((0..m.nrows() as usize).map(|i| {
    let range=m.row_ptrs()[i]..m.row_ptrs()[i+1];
    json!({"row":i,"entries":range.map(|at|json!({"column":m.col_idcs()[at],"value":coefficient(&m.values()[at])})).collect::<Vec<_>>()})
}).collect::<Vec<_>>())
}
fn key<const N: usize>(v: &Value) -> Result<Integral<N>> {
    let v = vector(v, N)?
        .into_iter()
        .map(i16::try_from)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    checked(Integral::numeric(
        <[i16; N]>::try_from(v).map_err(|_| "key arity")?,
    ))
}
fn powers<const N: usize>(k: &Integral<N>) -> Vec<i16> {
    k.powers().iter().map(|x| x.value()).collect()
}
fn field() -> Field {
    Field::new(Z)
}

fn validate(r: &Value) -> Result<usize> {
    require(r["schema"] == SCHEMA, "unknown input schema")?;
    let n = r["target"].as_array().ok_or("target array required")?.len();
    require(
        (1..=16).contains(&n),
        "native owner transport arity must be1..16",
    )?;
    let degree = number(r, "max_weight_degree")?;
    // Explicit experiment policy, unrelated to family or loop count.
    require(
        degree <= 1,
        "this experiment admits degree0/1 only; no escalation",
    )?;
    require(
        r["compute_parametric_image_rank"].is_boolean(),
        "compute_parametric_image_rank must be explicit boolean",
    )?;
    let target = vector(&r["target"], n)?;
    require(
        target
            .iter()
            .all(|&x| (i64::from(Power::MIN)..=i64::from(Power::MAX)).contains(&x)),
        "target exceeds native compact powers",
    )?;
    let mask = r["owner_mask"].as_str().ok_or("owner mask required")?;
    require(
        mask.len() == n
            && mask
                .bytes()
                .zip(&target)
                .all(|(b, x)| (b == b'1' && *x > 0) || (b == b'0' && *x <= 0)),
        "target/owner mismatch",
    )?;
    let protected = r["protected_axes"]
        .as_array()
        .ok_or("protected_axes required")?
        .iter()
        .map(|x| {
            x.as_u64()
                .and_then(|x| usize::try_from(x).ok())
                .ok_or("invalid protected axis".to_string())
        })
        .collect::<Result<Vec<_>>>()?;
    require(
        !protected.is_empty()
            && protected
                .iter()
                .all(|&axis| axis < n && mask.as_bytes()[axis] == b'1')
            && protected.windows(2).all(|pair| pair[0] < pair[1]),
        "protected_axes must be a sorted unique nonempty subset of active axes",
    )?;
    checked(IntegralShift::try_new(vector(&r["recenter"], n)?))?;
    for name in ["family_fingerprint", "expected_order"] {
        require(
            r[name].as_str().is_some_and(|s| !s.is_empty()),
            "family/order binding required",
        )?;
    }
    for name in [
        "max_owner_bytes",
        "max_denominators",
        "max_ordinary_rows",
        "max_unknowns",
        "max_constraint_columns",
        "max_augmented_columns",
        "max_input_nonzeros",
        "max_reducer_nonzeros",
        "max_kernel_vectors",
        "max_polynomial_operations",
        "max_polynomial_terms",
        "max_coordinate_cells",
        "max_translated_terms",
        "max_conditions",
        "max_product_terms",
        "max_exact_operations",
        "max_physical_columns",
        "max_coefficient_terms",
        "max_report_bytes",
    ] {
        limit(r, name)?;
    }
    admit(r, "max_denominators", n)?;
    owner_load_limits(r)?;
    Ok(n)
}

/// Explicit transport policy, distinct from finite source/matrix budgets.
/// Defaults below retain only native exact-algebra policy; every public ingress
/// cardinality/byte allowance is supplied by the request, never by topology.
fn owner_load_limits(r: &Value) -> Result<CandidateOwnerLoadLimits> {
    let value = &r["owner_load_limits"];
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
        .ok_or("explicit owner_load_limits required")?;
    require(
        object.len() == fields.len() && object.keys().all(|key| fields.contains(&key.as_str())),
        "unknown or missing owner ingress field",
    )?;
    for name in fields {
        require(
            number(value, name)? > 0,
            "owner ingress limits must be positive",
        )?;
    }
    let mut policy = CandidateOwnerLoadLimits::default();
    policy.bundle.max_bundle_bytes = number(value, "max_bundle_bytes")?;
    require(
        policy.bundle.max_bundle_bytes <= rustred_app::MAX_CANDIDATE_BUNDLE_BYTES,
        "owner bundle exceeds native hard ceiling",
    )?;
    policy.max_total_input_bytes = number(value, "max_total_input_bytes")?;
    policy.bundle.max_total_coefficient_bytes = number(value, "max_total_coefficient_bytes")?;
    policy.bundle.max_collection_entries = number(value, "max_collection_entries")?;
    policy.max_total_symbolica_state_bytes = number(value, "max_total_symbolica_state_bytes")?;
    policy.max_zero_sector_visits = number(value, "max_zero_sector_visits")?;
    policy.bundle.max_coefficient_bytes = number(value, "max_coefficient_bytes")?;
    Ok(policy)
}

fn translated_limits(r: &Value) -> Result<TranslatedSourceLimits> {
    Ok(TranslatedSourceLimits {
        max_requested_source_translations: limit(r, "max_unknowns")?,
        max_requested_offsets: limit(r, "max_unknowns")?,
        max_translated_sources: limit(r, "max_unknowns")?,
        max_translated_term_entries: limit(r, "max_translated_terms")?,
        max_translated_condition_entries: limit(r, "max_conditions")?,
        max_retained_condition_source_entries: limit(r, "max_conditions")?,
        max_retained_index_coordinate_cells: limit(r, "max_coordinate_cells")?,
        ..Default::default()
    })
}

fn probe<const N: usize>(bytes: &[u8], r: &Value) -> Result<Value> {
    let started = Instant::now();
    let target = key::<N>(&r["target"])?;
    let sector = std::array::from_fn(|i| target[i].value() > 0);
    let mask = checked(Mask::try_new(sector))?;
    let (family, programs) = checked(load_generated_candidate_owners::<N>(
        &[CandidateOwnerBundle {
            bytes,
            owner_sector: &mask,
        }],
        owner_load_limits(r)?,
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
        "saved order mismatch",
    )?;
    let order = checked(IntegralOrder::from_persisted_policy(
        sector,
        bound.owner_ordering(),
    ))?;
    let preflight = polynomial::preflight(&family, r)?;
    let generator = checked(ParametricIbpGenerator::try_new(&family))?;
    let prepared = checked(generator.prepare_ordinary_ibp())?;
    require(
        prepared.len() == preflight.directions,
        "ordinary source count mismatch",
    )?;
    let generated = (0..prepared.len()).map(|i| prepared.generate(i)).collect();
    let completed = checked(prepared.complete(generated))?;
    let zero = checked(IntegralShift::try_new([0_i64; N]))?;
    let inventory = checked(generator.translate_selected_completed_source_rows(
        &completed,
        (0..preflight.directions).map(|i| TranslatedSourceRequest::new(i, zero.clone())),
        translated_limits(r)?,
    ))?;
    require(
        inventory.is_complete_ordinary() && inventory.len() == preflight.directions,
        "incomplete native inventory",
    )?;
    let mut identities = BTreeMap::new();
    for source in inventory.sources() {
        let p = source.provenance();
        require(
            p.offset().values() == [0_i64; N],
            "inventory offset differs",
        )?;
        require(
            identities
                .insert(p.source_row().clone(), p.source_ordinal())
                .is_none(),
            "duplicate native row ID",
        )?;
    }
    let system = polynomial::assemble(&family, &identities, &preflight, r)?;
    let kernel = linear::left_kernel(&system.matrix, &family.coefficient_context().one(), r)?;
    let recenter = vector(&r["recenter"], N)?;
    let requests = system
        .unknowns
        .iter()
        .map(|u| {
            let off = recenter
                .iter()
                .zip(&u.alpha)
                .map(|(&a, &b)| {
                    a.checked_sub(i64::from(b))
                        .ok_or("source offset overflow".to_string())
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(TranslatedSourceRequest::new(
                u.ordinal,
                checked(IntegralShift::try_new(off))?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let translated = checked(generator.translate_selected_completed_source_rows(
        &completed,
        requests.clone(),
        translated_limits(r)?,
    ))?;
    require(
        translated.is_complete_ordinary() && translated.len() == system.unknowns.len(),
        "translated source inventory changed",
    )?;
    let map = translated
        .sources()
        .iter()
        .enumerate()
        .map(|(i, s)| {
            (
                (
                    s.provenance().source_ordinal(),
                    s.provenance().offset().values().to_vec(),
                ),
                i,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut source_indices = Vec::new();
    for (u, q) in system.unknowns.iter().zip(&requests) {
        let i = *map
            .get(&(q.source_ordinal(), q.offset().values().to_vec()))
            .ok_or("source provenance pair missing")?;
        require(
            translated.sources()[i].row_id() == &u.row_id,
            "source RowId differs",
        )?;
        source_indices.push(i);
    }
    let images = sources::images(
        &family,
        &generator,
        &translated,
        &source_indices,
        &kernel.weights,
        r,
    )?;
    let point = sources::point::<N>(
        &family,
        &generator,
        &translated,
        &source_indices,
        &kernel.weights,
        &images,
        &target,
        &order,
        r,
    )?;
    let unknowns=system.unknowns.iter().zip(requests.iter()).enumerate().map(|(i,(u,q))|json!({"unknown":i,"row_id":u.row_id.stable_string(),"native_ordinal":u.ordinal,"monomial":u.alpha,"source_offset":q.offset().values(),"translated_provenance":translated.sources()[source_indices[i]].provenance().stable_string()})).collect::<Vec<_>>();
    Ok(
        json!({"schema":SCHEMA,"status":"FINITE_LOGARITHMIC_KERNEL_DIAGNOSTIC_COMPLETE","request":r,
        "family_fingerprint":family.fingerprint(),"saved_order":format!("{:?}",bound.owner_ordering()),"base_parameters":family.coefficient_context().parameter_names(),
        "protection_scope":protection_scope(r),
        "preflight":preflight.report(),"construction":system.report,"unknown_provenance":unknowns,
        "constraint_columns":system.columns,"constraint_matrix":matrix_rows(&system.matrix),
        "coefficient_kernel_over_K":kernel.report,"kernel_weights":matrix_rows(&kernel.weights),
        "full_parametric_images":images.report,"point":point,"total_seconds":started.elapsed().as_secs_f64(),
        "rule_published":false,"recursive_walk":false,"dimension_specialized":false,"zero_sector_terms_discarded":false,
        "source_authority":"Complete native original translated rows and exact products; finite point discovery, not parametric-chart certificate",
        "master_or_irreducibility_or_coverage_or_cost_claim":false,
        "resource_scope":"Structural preflight and retained native results bounded; native transient scratch requires outer owned wall/RSS guard"}),
    )
}

fn protection_scope(r: &Value) -> Value {
    let active = r["owner_mask"]
        .as_str()
        .unwrap()
        .bytes()
        .enumerate()
        .filter_map(|(i, b)| (b == b'1').then_some(i))
        .collect::<Vec<_>>();
    json!({"active_axes":active,"protected_axes":r["protected_axes"],
        "all_active_axes_protected":r["protected_axes"] == json!(active),
        "axis_convention":"zero-based physical denominator coordinates",
        "kernel_vector_subset_selection":false})
}

fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>> {
    let f = checked(File::open(path))?;
    require(
        checked(f.metadata())?.len() <= max as u64,
        "input exceeds byte bound",
    )?;
    let mut b = Vec::new();
    checked(f.take(max as u64 + 1).read_to_end(&mut b))?;
    require(b.len() <= max, "input grew past byte bound")?;
    Ok(b)
}
fn main_result() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    require(
        (args.len() == 2 && args[0] == "validate") || (args.len() == 3 && args[0] == "probe"),
        "usage: logarithmic_kernel validate REQUEST | probe OWNER REQUEST",
    )?;
    let r: Value = checked(serde_json::from_slice(&read_bounded(
        Path::new(args.last().unwrap()),
        MAX_REQUEST_BYTES,
    )?))?;
    let n = validate(&r)?;
    let report = if args[0] == "validate" {
        json!({"status":"INPUT_SHAPE_VALID","arity":n,"native_loaded":false})
    } else {
        let b = read_bounded(Path::new(&args[1]), limit(&r, "max_owner_bytes")?)?;
        macro_rules! arities {($($n:literal),*)=>{match n{$($n=>probe::<$n>(&b,&r),)*_=>Err("unsupported arity".into())}};}
        arities!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)?
    };
    let text = checked(serde_json::to_string_pretty(&report))?;
    admit(&r, "max_report_bytes", text.len())?;
    println!("{text}");
    Ok(())
}
fn main() {
    if let Err(e) = main_result() {
        eprintln!(
            "{}",
            json!({"status":"REFUSED_OR_INCOMPLETE","error":e,"rule_published":false})
        );
        std::process::exit(2)
    }
}
