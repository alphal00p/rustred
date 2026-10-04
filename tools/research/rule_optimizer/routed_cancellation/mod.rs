//! Research only: default one-parent weighted routing, or opt-in bounded Apply cut.
//! Neither mode claims snapshot coverage, rule publication or exceptional-d validity.
mod input;
mod plateau;
#[cfg(test)]
mod tests;

use crate::applied_observer::{
    input::{Queries, read_bounded},
    record::{Recorder, write_new_json},
};
use input::{Limits, Request, Route, Selection};
use rustred::{
    algebra::{ExactAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext},
    family::{IntegralFamily, IntegralKey},
    persistence::{BinaryIoLimits, CoefficientTableBuilder},
    sector::{
        Mask,
        symmetry::{self, CoefficientMatrix, MomentumMap, integral_transport},
    },
    solver::{
        CandidateDomainRouteEvent, CandidateDomainRouteLimits, OwnerAppliedEvent,
        OwnerAppliedNonzero, OwnerAppliedStats, OwnerDomainMatchDisposition, OwnerFeedbackPolicy,
        RoutedCandidateReducer,
    },
};
use rustred_app::{
    FiniteCasePolicy, RoutedCampaignRequest, RoutedFeedbackOptions, RoutedFeedbackSession,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    ops::ControlFlow,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use symbolica::prelude::{Matrix, Q, Rational};

fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn mask<const N: usize>(text: &str) -> Result<[bool; N], String> {
    if text.len() != N || text.bytes().any(|b| b != b'0' && b != b'1') {
        return Err("invalid literal route mask".into());
    }
    text.bytes()
        .map(|b| b == b'1')
        .collect::<Vec<_>>()
        .try_into()
        .map_err(err)
}
fn bits<const N: usize>(key: &IntegralKey) -> Result<[bool; N], String> {
    key.powers()
        .iter()
        .map(|n| *n > 0)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(err)
}
fn mask_text(mask: &[bool]) -> String {
    mask.iter().map(|b| if *b { '1' } else { '0' }).collect()
}
fn singleton(sector: &[bool], lower: &[u64], upper: &[Option<u64>]) -> Result<IntegralKey, String> {
    if sector.len() != lower.len()
        || lower.len() != upper.len()
        || lower.iter().zip(upper).any(|(lo, hi)| *hi != Some(*lo))
    {
        return Err("non-singleton or malformed image".into());
    }
    let powers = sector
        .iter()
        .zip(lower)
        .map(|(on, x)| {
            let x = i64::try_from(*x).map_err(|_| "physical power overflow")?;
            if *on {
                x.checked_add(1).ok_or("physical power overflow")
            } else {
                Ok(-x)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    IntegralKey::try_new(powers).map_err(err)
}

/// Native field authentication happens even for values which might later cancel.
fn checked_copy(
    context: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
    exact: ExactAlgebraLimits,
    max_terms: usize,
) -> Result<IndexedCoefficient, String> {
    context.validate_with_limits(value, exact).map_err(err)?;
    let terms = value
        .raw()
        .numerator
        .nterms()
        .checked_add(value.raw().denominator.nterms())
        .ok_or("coefficient term overflow")?;
    if terms > max_terms {
        return Err("coefficient term cap".into());
    }
    // Native operation/term admission bounds each object; outer process RSS
    // bounds allocator capacity and simultaneous copies, not this logical count.
    Ok(value.clone())
}

fn require_fixed_coefficient(
    value: &IndexedCoefficient,
    index_positions: &[usize],
) -> Result<(), String> {
    for polynomial in [&value.raw().numerator, &value.raw().denominator] {
        let width = polynomial.variables().len();
        if width == 0
            || index_positions.iter().any(|i| *i >= width)
            || polynomial
                .exponents
                .chunks_exact(width)
                .any(|powers| index_positions.iter().any(|i| powers[*i] != 0))
        {
            return Err("coefficient is not fully specialized in original source indices".into());
        }
    }
    Ok(())
}

struct SourceTerm {
    key: IntegralKey,
    coefficient: IndexedCoefficient,
}
#[derive(Default)]
struct Capture {
    terms: Vec<SourceTerm>,
    selected: Option<(usize, usize)>,
    finished: usize,
}
impl Capture {
    fn event<const N: usize>(
        &mut self,
        event: &OwnerAppliedEvent<N>,
        request: &Request,
        query: &crate::applied_observer::input::Query,
        index_positions: &[usize],
        context: &IndexedCoefficientContext,
        exact: ExactAlgebraLimits,
    ) -> Result<(), String> {
        match event {
            OwnerAppliedEvent::Classified(piece) => {
                if mask_text(piece.owner()) != query.owner
                    || piece.lower() != query.lower
                    || piece.upper() != query.upper
                    || piece.max_numerator_rank() != query.max_numerator_rank
                    || piece.power_bounds() != query.power_bounds.native()
                {
                    return Err("classified source scope changed".into());
                }
                let OwnerDomainMatchDisposition::SelectedRule { batch, rule } = piece.disposition()
                else {
                    return Err("parent is not one selected rule".into());
                };
                if self.selected.replace((batch, rule)).is_some() {
                    return Err("multiple selected source pieces".into());
                }
            }
            OwnerAppliedEvent::Successor(term) => {
                if self.selected.is_none()
                    || self.finished != 0
                    || term.coefficient_nonzero != OwnerAppliedNonzero::Uniform
                    || self.terms.len() >= request.expected_successors
                {
                    return Err("conditional, extra or out-of-order successor".into());
                }
                let key = singleton(term.target_sector, term.target_lower, term.target_upper)?;
                if term.source_lower != query.lower || term.source_upper != query.upper {
                    return Err("source application cell is not original singleton".into());
                }
                require_fixed_coefficient(term.coefficient, index_positions)?;
                let coefficient = checked_copy(
                    context,
                    term.coefficient,
                    exact,
                    request.limits.max_coefficient_terms,
                )?;
                self.terms.push(SourceTerm { key, coefficient });
            }
            OwnerAppliedEvent::RuleFinished {
                successors,
                problems,
                ..
            } => {
                if *successors != self.terms.len() || *problems != 0 || self.finished != 0 {
                    return Err("incomplete rule finish".into());
                }
                self.finished += 1;
            }
            OwnerAppliedEvent::Problem(_)
            | OwnerAppliedEvent::OptionalCoefficientRefusal { .. } => {
                return Err("native source problem or optional coefficient refusal".into());
            }
        }
        Ok(())
    }
    fn complete(&self, stats: &OwnerAppliedStats, request: &Request) -> bool {
        self.selected.is_some()
            && self.finished == 1
            && stats.selected_pieces == 1
            && self.terms.len() == request.expected_successors
            && stats.successors == request.expected_successors
            && stats.strict_subsupport_successors == request.expected_strict_subsupport_successors
            && stats.problems == 0
            && stats.conditional_successors == 0
            && stats.optional_coefficient_refusals == 0
    }
}

fn matrix(
    rows: &[Vec<String>],
) -> Result<Matrix<symbolica::domains::rational::RationalField>, String> {
    let values = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|x| x.parse::<i64>().map(Rational::from))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    Matrix::from_nested_vec(values, Q).map_err(err)
}
/// Same literal route and native verifier as application/routed_campaign/prepare/routes.rs.
/// This does not choose a new map. Full original selection admission precedes it.
fn reverify<const N: usize>(
    family: &Arc<IntegralFamily>,
    route: &Route,
) -> Result<integral_transport::Prepared, String> {
    let loops = family.loop_count();
    if !route.requires_transport
        || route.source_to_representative.len() != loops
        || route.owner_to_representative.len() != loops
    {
        return Err("invalid transport witness".into());
    }
    let source = Mask::try_new(mask::<N>(&route.source_mask)?).map_err(err)?;
    let target = Mask::try_new(mask::<N>(&route.owner_mask)?).map_err(err)?;
    let inverse = matrix(&route.owner_to_representative)?.inv().map_err(err)?;
    let composed = &matrix(&route.source_to_representative)? * &inverse;
    let mut entries = Vec::new();
    for i in 0..loops {
        for j in 0..loops {
            // This is the existing integer-matrix admission, NOT a parse of IBP or
            // coefficient displays. A nonintegral composed witness fails closed.
            let value = composed[(i as u32, j as u32)]
                .to_string()
                .parse::<i64>()
                .map_err(|_| "composed route is not an integral loop map")?;
            entries.push(family.coefficient_context().integer(value));
        }
    }
    let momentum = MomentumMap::new(
        CoefficientMatrix::try_new(loops, loops, entries).map_err(err)?,
        CoefficientMatrix::try_new(loops, 0, []).map_err(err)?,
        CoefficientMatrix::try_new(0, 0, []).map_err(err)?,
    );
    let verified = symmetry::verify(family, family, momentum, Default::default()).map_err(err)?;
    integral_transport::compile(
        family,
        family.clone(),
        Arc::new(verified),
        source,
        target,
        Default::default(),
    )
    .map_err(err)
}

fn charge(count: &mut usize, add: usize, cap: usize, name: &str) -> Result<(), String> {
    let next = count
        .checked_add(add)
        .ok_or_else(|| format!("{name} overflow"))?;
    if next > cap {
        return Err(format!("{name} limit: {next}>{cap}"));
    }
    *count = next;
    Ok(())
}

struct Sum<'a> {
    context: &'a IndexedCoefficientContext,
    exact: ExactAlgebraLimits,
    limits: &'a Limits,
    map: BTreeMap<IntegralKey, IndexedCoefficient>,
    occurrences: usize,
    additions: usize,
}
impl Sum<'_> {
    fn add(&mut self, key: IntegralKey, coefficient: &IndexedCoefficient) -> Result<(), String> {
        let coefficient = checked_copy(
            self.context,
            coefficient,
            self.exact,
            self.limits.max_coefficient_terms,
        )?;
        if coefficient.is_zero() {
            return Err("unexpected zero endpoint coefficient".into());
        }
        charge(
            &mut self.occurrences,
            1,
            self.limits.max_endpoint_occurrences,
            "endpoint occurrences",
        )?;
        if let Some(old) = self.map.get_mut(&key) {
            let sum = self
                .context
                .add_with_limits(old, &coefficient, self.exact)
                .map_err(err)?;
            *old = checked_copy(
                self.context,
                &sum,
                self.exact,
                self.limits.max_coefficient_terms,
            )?;
            self.additions += 1;
        } else {
            if self.map.len() >= self.limits.max_distinct_keys {
                return Err("distinct key cap".into());
            }
            self.map.insert(key, coefficient);
        }
        // Zero sums deliberately remain in the map: pre-distinct vs final support
        // is the cancellation signal, unlike occurrence deduplication.
        Ok(())
    }
}

/// Only use the native service's SOURCE zero decision. Its geometric target
/// covers are discarded and never mistaken for exact transported endpoints.
fn known_zero<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    key: &IntegralKey,
    cancel: &AtomicBool,
) -> Result<bool, String> {
    let sector = bits::<N>(key)?;
    let lower = key
        .powers()
        .iter()
        .map(|n| {
            if *n > 0 {
                (*n - 1) as u64
            } else {
                n.unsigned_abs()
            }
        })
        .collect::<Vec<_>>();
    let upper = lower.iter().map(|x| Some(*x)).collect::<Vec<_>>();
    let mut zero = false;
    reducer
        .visit_bounded_domain_route_overcover(
            sector,
            &lower,
            &upper,
            None,
            CandidateDomainRouteLimits::default(),
            cancel,
            |event| {
                if let CandidateDomainRouteEvent::ZeroSector {
                    sector: s,
                    source_conditions_required: false,
                    ..
                } = event
                {
                    if s == sector {
                        zero = true;
                    }
                }
                ControlFlow::Continue(())
            },
        )
        .map_err(err)?;
    Ok(zero)
}

struct Evidence {
    table: Option<CoefficientTableBuilder>,
    ledger: Vec<Value>,
    max_records: usize,
}
impl Evidence {
    fn coefficient(&mut self, value: &IndexedCoefficient) -> Result<usize, String> {
        self.table
            .as_mut()
            .unwrap()
            .intern(value.raw())
            .map(|id| id.index())
            .map_err(err)
    }
    fn record(&mut self, event: Value) -> Result<(), String> {
        if self.ledger.len() >= self.max_records {
            return Err("routing ledger cap".into());
        }
        self.ledger.push(event);
        Ok(())
    }
}

fn route_all<const N: usize>(
    session: &RoutedFeedbackSession<N>,
    request: &Request,
    selection: &Selection,
    source: Capture,
    cancel: &AtomicBool,
    evidence: &mut Evidence,
) -> Result<Value, String> {
    let programs = session.programs();
    let family = programs.context().family_owner();
    let context = programs.context().coefficient_context();
    let exact = programs.context().limits().exact_algebra;
    if session
        .routed_reducer()
        .domain_routing_requires_source_conditions()
    {
        return Err("shared source conditions require an additional validity adapter".into());
    }
    let owners: BTreeSet<_> = programs.owner_sectors().copied().collect();
    let mut routes = BTreeMap::new();
    for route in &selection.routing {
        let key = mask::<N>(&route.source_mask)?;
        if routes.insert(key, route).is_some() {
            return Err("duplicate manifested route source".into());
        }
    }
    let mut compiled = BTreeMap::new();
    let mut zero_cache = BTreeMap::new();
    let mut total = Sum {
        context,
        exact,
        limits: &request.limits,
        map: BTreeMap::new(),
        occurrences: 0,
        additions: 0,
    };
    let mut calls = 0;
    let mut route_endpoints = 0;
    let mut zero_occurrences = 0;
    let mut expansion = request.limits.expansion()?;
    expansion.exact_algebra = exact;
    let source_count = source.terms.len();
    for (source_ordinal, term) in source.terms.into_iter().enumerate() {
        let id = evidence.coefficient(&term.coefficient)?;
        evidence.record(json!({"kind":"source","source_ordinal":source_ordinal,"key":term.key.powers(),"coefficient":id}))?;
        let mut stack = vec![(term.key, term.coefficient, N + 1)];
        while let Some((key, weight, previous_support)) = stack.pop() {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            context.validate_with_limits(&weight, exact).map_err(err)?;
            let sector = bits::<N>(&key)?;
            let support = sector.iter().filter(|b| **b).count();
            if support >= previous_support {
                return Err("nondecreasing route reentry".into());
            }
            let zero = match zero_cache.get(&sector) {
                Some(value) => *value,
                None => {
                    let value = known_zero(session.routed_reducer(), &key, cancel)?;
                    zero_cache.insert(sector, value);
                    value
                }
            };
            let weight_id = evidence.coefficient(&weight)?;
            if zero {
                zero_occurrences += 1;
                evidence.record(json!({"kind":"native_zero","source_ordinal":source_ordinal,"key":key.powers(),"coefficient":weight_id,
                    "authority":"admitted native zero-sector evidence; shared source conditions absent"}))?;
                continue;
            }
            if owners.contains(&sector) {
                evidence.record(json!({"kind":"endpoint","source_ordinal":source_ordinal,"owner":mask_text(&sector),"key":key.powers(),"coefficient":weight_id}))?;
                total.add(key, &weight)?;
                continue;
            }
            charge(&mut calls, 1, request.limits.max_route_calls, "route calls")?;
            if !compiled.contains_key(&sector) {
                let route = routes
                    .get(&sector)
                    .ok_or("missing exact manifested route")?;
                if !owners.contains(&mask::<N>(&route.owner_mask)?) {
                    return Err("route owner not installed".into());
                }
                let prepared = reverify::<N>(family, route)?;
                if prepared.source_family_fingerprint() != family.fingerprint()
                    || prepared.target_family().fingerprint() != family.fingerprint()
                    || prepared.source_root().active_bits() != sector
                    || prepared.target_root().active_bits() != mask::<N>(&route.owner_mask)?
                {
                    return Err("reverified route binding differs".into());
                }
                evidence.record(
                    json!({"kind":"route_witness","manifest":route,"family":family.fingerprint()}),
                )?;
                compiled.insert(sector, prepared);
            }
            let prepared = &compiled[&sector];
            let transported = prepared.transport(&key, expansion).map_err(err)?;
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled after native transport".into());
            }
            if transported.source() != &key
                || transported.source_family_fingerprint() != family.fingerprint()
                || transported.target_family_fingerprint() != family.fingerprint()
            {
                return Err("transport identity mismatch".into());
            }
            charge(
                &mut route_endpoints,
                transported.terms().len(),
                request.limits.max_endpoint_occurrences,
                "materialized route endpoints",
            )?;
            evidence.record(json!({"kind":"route_application","call":calls,"source_ordinal":source_ordinal,"key":key.powers(),
                "coefficient":weight_id,"source_mask":mask_text(&sector),"target_owner":mask_text(prepared.target_root().active_bits()),"terms":transported.terms().len()}))?;
            for endpoint in transported.terms() {
                let endpoint_mask = Mask::try_from_indices(endpoint.key().powers()).map_err(err)?;
                if !endpoint_mask
                    .is_subsector_of(prepared.target_root())
                    .map_err(err)?
                {
                    return Err("transport outside target root".into());
                }
                let lifted = context.lift(endpoint.coefficient()).map_err(err)?;
                let product = context
                    .mul_with_limits(&weight, &lifted, exact)
                    .map_err(err)?;
                if cancel.load(Ordering::Relaxed) {
                    return Err("cancelled after native multiplication".into());
                }
                if product.is_zero() {
                    return Err("unexpected zero product in exact nonzero field".into());
                }
                let product = checked_copy(
                    context,
                    &product,
                    exact,
                    request.limits.max_coefficient_terms,
                )?;
                let lifted_id = evidence.coefficient(&lifted)?;
                let product_id = evidence.coefficient(&product)?;
                evidence.record(json!({"kind":"transport_term","call":calls,"key":endpoint.key().powers(),"factor":lifted_id,"weighted_coefficient":product_id}))?;
                let previous_support = if endpoint_mask == *prepared.target_root() {
                    // Native full-root transport is Apply. Its installed-owner
                    // check on stack pop prevents a second same-root route.
                    support + 1
                } else {
                    if endpoint_mask.active_count() >= support {
                        return Err("strict pinch did not reduce support".into());
                    }
                    support
                };
                if stack.len() >= request.limits.max_endpoint_occurrences {
                    return Err("pending route cap".into());
                }
                stack.push((endpoint.key().clone(), product, previous_support));
            }
        }
    }
    let mut final_nonzero = 0;
    for (key, coefficient) in &total.map {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled before final sum encoding".into());
        }
        let id = evidence.coefficient(coefficient)?;
        let zero = coefficient.is_zero();
        if !zero {
            final_nonzero += 1;
        }
        evidence.record(
            json!({"kind":"final_sum","key":key.powers(),"coefficient":id,"is_zero":zero}),
        )?;
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled after complete routing".into());
    }
    Ok(
        json!({"source_rhs_count":source_count,"route_calls":calls,"materialized_route_endpoints":route_endpoints,
        "native_zero_occurrences":zero_occurrences,"routed_nonzero_endpoint_occurrences":total.occurrences,
        "pre_coalescing_distinct_keys":total.map.len(),"final_nonzero_keys":final_nonzero,
        "keys_removed_by_exact_cancellation":total.map.len()-final_nonzero,"coalescing_additions":total.additions,
        "selected":source.selected,"family_fingerprint":family.fingerprint(),
        "route_budget_charges":"per-call native expansion limits; prospective aggregate usage unavailable through public transport API",
        "interpretation":"fixed-parent Q(d) identity under all inherited generic-parameter poles/guards; not a parametric IBP, closure suppression, or campaign-cost claim"}),
    )
}

fn observe<const N: usize>(
    request: &Request,
    selection_text: &str,
    targets: &str,
    queries: &Queries,
    output: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let started = Instant::now();
    let mut native = RoutedCampaignRequest::new(selection_text.to_owned(), targets.to_owned());
    native.owner_base = request.observation.owner_base.clone();
    native.workers = request.observation.workers;
    let options = RoutedFeedbackOptions::new(
        OwnerFeedbackPolicy::default(),
        FiniteCasePolicy::SearchFinite,
    );
    let session = RoutedFeedbackSession::<N>::prepare(native, options, cancel, |_| {})
        .map_err(err)?
        .ok_or("preparation cancelled")?;
    let preparation_seconds = started.elapsed().as_secs_f64();
    if let Some(config) = &request.plateau_cut {
        return plateau::observe(
            &session,
            request,
            config,
            queries,
            output,
            cancel,
            preparation_seconds,
        );
    }
    let selection: Selection = serde_json::from_str(selection_text).map_err(err)?;
    let query = &queries.queries[0];
    let owner = mask::<N>(&query.owner)?;
    let source_key = singleton(&owner, &query.lower, &query.upper)?;
    let original = output.join("original");
    fs::create_dir(&original).map_err(err)?;
    write_new_json(
        &original.join("binding.json"),
        &json!({"selection_blake3":blake3::hash(selection_text.as_bytes()).to_hex().as_str(),
        "query":query,"source_key":source_key.powers(),"source_conditions_required":session.routed_reducer().domain_routing_requires_source_conditions(),
        "guard_authority":"native ordered matching and complete original-term visitor; original payload retained by selection binding"}),
        request.limits.max_report_bytes,
    )?;
    let mut recorder = Recorder::new(&original, request.observation.recorder_limits.clone())?;
    recorder.begin_query(0, query)?;
    let mut capture = Capture::default();
    let mut capture_error = None;
    let phase = Instant::now();
    let result = session
        .programs()
        .visit_power_bounded_owner_applied_successors(
            owner,
            &query.lower,
            &query.upper,
            query.max_numerator_rank,
            query.power_bounds.native(),
            request.observation.native_limits.build()?,
            cancel,
            |event| {
                let admitted = capture.event(
                    &event,
                    request,
                    query,
                    session.programs().context().index_variables(),
                    session.programs().context().coefficient_context(),
                    session.programs().context().limits().exact_algebra,
                );
                let recorded = recorder.event(event);
                if let Err(error) = admitted {
                    capture_error = Some(error);
                    return ControlFlow::Break(());
                }
                recorded
            },
        );
    let visitor_seconds = phase.elapsed().as_secs_f64();
    let matching_complete = result
        .as_ref()
        .is_ok_and(|stats| capture.complete(stats, request));
    let complete = recorder.end_query(result, visitor_seconds)?
        && matching_complete
        && capture_error.is_none();
    let recording_complete = recorder.finish(json!({"preparation_seconds":preparation_seconds,"visitor_seconds":visitor_seconds,"capture_error":capture_error}), complete)?;
    if !complete || !recording_complete {
        return Err(
            "complete fixed-parent observation gate failed; original evidence retained".into(),
        );
    }
    let r = &request.observation.recorder_limits;
    let binary = BinaryIoLimits {
        max_program_bytes: r
            .max_state_bytes
            .checked_add(r.max_total_atom_bytes)
            .ok_or("byte overflow")?,
        max_collection_entries: r.max_coefficients,
        max_state_bytes: r.max_state_bytes,
        max_atom_bytes: r.max_atom_bytes,
        max_total_atom_bytes: r.max_total_atom_bytes,
        ..Default::default()
    };
    let mut evidence = Evidence {
        table: Some(CoefficientTableBuilder::new(binary)),
        ledger: Vec::new(),
        max_records: request.limits.max_ledger_records,
    };
    let phase = Instant::now();
    let mut outcome = route_all(
        &session,
        request,
        &selection,
        capture,
        cancel,
        &mut evidence,
    );
    let route_seconds = phase.elapsed().as_secs_f64();
    let count = evidence.table.as_ref().unwrap().len();
    let table = evidence.table.take().unwrap().finish().map_err(err)?;
    for (name, bytes) in [
        ("coefficients.state", &table.state),
        ("coefficients.atoms", &table.atoms),
    ] {
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(output.join(name))
            .map_err(err)?
            .write_all(bytes)
            .map_err(err)?;
    }
    write_new_json(
        &output.join("ledger.json"),
        &evidence.ledger,
        request.limits.max_report_bytes,
    )?;
    if cancel.load(Ordering::Relaxed) {
        outcome = Err("cancelled before result publication".into());
    }
    let report = json!({"schema":"rustred.routed-cancellation.result.v1","complete":outcome.is_ok(),
        "outcome":outcome.as_ref().ok(),"failure":outcome.as_ref().err(),
        "source_conditions_required":session.routed_reducer().domain_routing_requires_source_conditions(),
        "preparation_seconds":preparation_seconds,"visitor_seconds":visitor_seconds,"routing_seconds":route_seconds,
        "coefficient_table":{"count":count,"state_bytes":table.state.len(),"atoms_bytes":table.atoms.len(),
            "state_blake3":blake3::hash(&table.state).to_hex().as_str(),"atoms_blake3":blake3::hash(&table.atoms).to_hex().as_str()},
        "claims":{"one_complete_parent":true,"generic_parameter_assumptions_retained":outcome.is_ok(),"parametric_rule":false,"closure":false,"performance":false},
        "scratch_scope":"native per-object/per-route algebra limits plus bounded multiplicities; outer process-tree RSS is mandatory"});
    write_new_json(
        &output.join("result.json"),
        &report,
        request.limits.max_report_bytes,
    )?;
    outcome.map(|_| ())
}

pub fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: routed-cancellation REQUEST.json FRESH_OUTPUT_DIR".into());
    }
    let request_text = read_bounded(Path::new(&args[1]), 1 << 20)?;
    let request: Request = serde_json::from_str(&request_text).map_err(err)?;
    let input_cap = request.observation.recorder_limits.max_input_bytes;
    let selection = read_bounded(&request.observation.selection, input_cap)?;
    let query_text = read_bounded(&request.observation.queries, input_cap)?;
    let queries: Queries = serde_json::from_str(&query_text).map_err(err)?;
    let targets = read_bounded(&request.observation.preparation_targets_csv, input_cap)?;
    let n = request.validate(&queries)?;
    let output = Path::new(&args[2]);
    fs::create_dir(output).map_err(err)?;
    write_new_json(
        &output.join("binding.json"),
        &json!({"request":request,"request_blake3":blake3::hash(request_text.as_bytes()).to_hex().as_str(),
        "selection_blake3":blake3::hash(selection.as_bytes()).to_hex().as_str(),"queries_blake3":blake3::hash(query_text.as_bytes()).to_hex().as_str(),
        "targets_blake3":blake3::hash(targets.as_bytes()).to_hex().as_str()}),
        request.limits.max_report_bytes,
    )?;
    let cancel = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, cancel.clone()).map_err(err)?;
    }
    macro_rules! dispatch { ($($n:literal),*) => {match n {
        $($n => observe::<$n>(&request, &selection, &targets, &queries, output, &cancel),)*
        _ => Err("unsupported arity".into()),
    }}; }
    dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}
