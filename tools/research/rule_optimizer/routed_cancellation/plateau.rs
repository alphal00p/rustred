//! Bounded saved-program substitution, not a reducer or original-IBP proof.
//! Every root has its own vector. No route/zero inference is made at the cut.
use super::*;
use crate::applied_observer::input::{Powers, Query};
use std::collections::VecDeque;

#[cfg(test)]
#[path = "plateau_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Degree {
    a: i128,
    r: i128,
    e: i128,
}
fn degree(key: &IntegralKey) -> Degree {
    let a = key.powers().iter().map(|&x| i128::from(x).max(0)).sum();
    let r = key.powers().iter().map(|&x| (-i128::from(x)).max(0)).sum();
    let support = key.powers().iter().filter(|&&x| x > 0).count() as i128;
    Degree {
        a,
        r,
        e: a - support + r,
    }
}
pub(super) fn check_query_point(q: &Query, key: &IntegralKey) -> Result<(), String> {
    check_point_caps(key, q.max_numerator_rank, q.power_bounds)
}
fn check_point_caps(key: &IntegralKey, rank: Option<u32>, powers: Powers) -> Result<(), String> {
    let d = degree(key);
    if rank.is_some_and(|r| d.r > i128::from(r))
        || powers
            .max_positive_power
            .is_some_and(|a| d.a > i128::from(a))
        || powers
            .min_power_difference
            .is_some_and(|x| d.a - d.r < i128::from(x))
        || powers
            .max_power_difference
            .is_some_and(|x| d.a - d.r > i128::from(x))
    {
        return Err("singleton parent is outside retained root caps".into());
    }
    Ok(())
}

/// Native correlation normalization may tighten redundant R/A/D bounds. For
/// a fixed box, exact scope equality means the same point inhabits BOTH sets.
/// Do not generalize this test to non-singleton domains.
fn classified_scope(
    q: &Query,
    owner: &[bool],
    lower: &[u64],
    upper: &[Option<u64>],
    rank: Option<u32>,
    powers: Powers,
) -> Result<(), String> {
    if mask_text(owner) != q.owner || lower != q.lower || upper != q.upper {
        return Err("native singleton classification changed input geometry".into());
    }
    let key = singleton(owner, lower, upper)?;
    check_query_point(q, &key)?;
    check_point_caps(&key, rank, powers)
}
fn query(key: &IntegralKey, id: String) -> Query {
    let lower: Vec<_> = key
        .powers()
        .iter()
        .map(|&n| {
            if n > 0 {
                (n - 1) as u64
            } else {
                n.unsigned_abs()
            }
        })
        .collect();
    Query {
        id,
        owner: key
            .powers()
            .iter()
            .map(|&n| if n > 0 { '1' } else { '0' })
            .collect(),
        upper: lower.iter().copied().map(Some).collect(),
        lower,
        // A singleton needs no inherited entry R/A/D restriction.
        max_numerator_rank: None,
        power_bounds: Powers::default(),
    }
}
fn same_owner(a: &IntegralKey, b: &IntegralKey) -> bool {
    a.powers()
        .iter()
        .zip(b.powers())
        .all(|(&x, &y)| (x > 0) == (y > 0))
}
fn stop_reason(
    root: &IntegralKey,
    key: &IntegralKey,
    depth: usize,
    max_depth: usize,
) -> Option<&'static str> {
    if !same_owner(root, key) {
        Some("support-change-unrouted")
    } else if degree(key).e < degree(root).e {
        Some("strict-E-decrease")
    } else if degree(key).e > degree(root).e {
        Some("E-increase")
    } else if depth >= max_depth {
        Some("depth-cut-plateau")
    } else {
        None
    }
}

#[derive(Default)]
struct Transaction {
    terms: Vec<SourceTerm>,
    selected: Option<(usize, usize)>,
    classified: usize,
    finished: usize,
    unsafe_reason: Option<String>,
}
impl Transaction {
    fn event<const N: usize>(
        &mut self,
        event: &OwnerAppliedEvent<N>,
        q: &Query,
        context: &IndexedCoefficientContext,
        indices: &[usize],
        exact: ExactAlgebraLimits,
        limits: &Limits,
        used_terms: &mut usize,
    ) -> Result<(), String> {
        match event {
            OwnerAppliedEvent::Classified(piece) => {
                self.classified += 1;
                classified_scope(
                    q,
                    piece.owner(),
                    piece.lower(),
                    piece.upper(),
                    piece.max_numerator_rank(),
                    Powers::from_native(piece.power_bounds()),
                )?;
                if let OwnerDomainMatchDisposition::SelectedRule { batch, rule } =
                    piece.disposition()
                {
                    self.selected = Some((batch, rule));
                } else {
                    self.unsafe_reason =
                        Some(format!("native disposition {:?}", piece.disposition()));
                }
            }
            OwnerAppliedEvent::Successor(term) => {
                charge(
                    used_terms,
                    1,
                    limits.max_endpoint_occurrences,
                    "all attempted source terms",
                )?;
                if self.selected.is_none()
                    || self.finished != 0
                    || term.source_lower != q.lower
                    || term.source_upper != q.upper
                {
                    return Err("out-of-order or non-singleton native successor".into());
                }
                require_fixed_coefficient(term.coefficient, indices)?;
                let coefficient = checked_copy(
                    context,
                    term.coefficient,
                    exact,
                    limits.max_coefficient_terms,
                )?;
                let key = singleton(term.target_sector, term.target_lower, term.target_upper)?;
                let source = singleton(&mask::<N>(&q.owner)?, &q.lower, &q.upper)?;
                for ((&n, &s), &child) in source.powers().iter().zip(term.shift).zip(key.powers()) {
                    if n.checked_add(s) != Some(child) {
                        return Err("native shifted singleton differs".into());
                    }
                }
                if term.coefficient_nonzero != OwnerAppliedNonzero::Uniform {
                    self.unsafe_reason =
                        Some("conditional native successor; entire child retained".into());
                }
                self.terms.push(SourceTerm { key, coefficient });
            }
            OwnerAppliedEvent::RuleFinished {
                successors,
                problems,
                ..
            } => {
                self.finished += 1;
                if *successors != self.terms.len() || *problems != 0 {
                    self.unsafe_reason =
                        Some("incomplete or problematic native rule finish".into());
                }
            }
            OwnerAppliedEvent::Problem(_) => {
                self.unsafe_reason = Some("native source/child problem".into())
            }
            OwnerAppliedEvent::OptionalCoefficientRefusal { .. } => {
                self.unsafe_reason = Some("native optional coefficient refusal".into())
            }
        }
        Ok(())
    }
    fn accepted(&self, stats: &OwnerAppliedStats, native_complete: bool) -> bool {
        native_complete
            && self.classified == 1
            && self.selected.is_some()
            && self.finished == 1
            && self.unsafe_reason.is_none()
            && stats.selected_pieces == 1
            && stats.successors == self.terms.len()
            && stats.problems == 0
            && stats.conditional_successors == 0
            && stats.optional_coefficient_refusals == 0
            && stats.unsupported_support_successors == 0
    }
}

enum Step {
    Applied(Vec<SourceTerm>),
    Boundary(String),
}
struct Pending {
    key: IntegralKey,
    coefficient: IndexedCoefficient,
    depth: usize,
}
struct Work {
    apply_calls: usize,
    source_terms: usize,
    products: usize,
    boundary_occurrences: usize,
    fallbacks: usize,
    all_native_complete: bool,
}

#[derive(Debug)]
struct Cut {
    report: Value,
    before: BTreeSet<IntegralKey>,
    after: BTreeSet<IntegralKey>,
}

/// The support-only comparator uses precisely these substitutions, including
/// all off-plateau carried terms. It differs only in not testing final sums.
fn cut(
    root: &IntegralKey,
    context: &IndexedCoefficientContext,
    exact: ExactAlgebraLimits,
    limits: &Limits,
    config: &input::PlateauCut,
    cancel: &AtomicBool,
    evidence: &mut Evidence,
    work: &mut Work,
    mut apply: impl FnMut(&IntegralKey, usize, &mut Evidence, &mut Work) -> Result<Step, String>,
) -> Result<Cut, String> {
    let mut queue = VecDeque::from([Pending {
        key: root.clone(),
        coefficient: context.one(),
        depth: 0,
    }]);
    let mut total = Sum {
        context,
        exact,
        limits,
        map: BTreeMap::new(),
        occurrences: 0,
        additions: 0,
    };
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    let mut root_applied = false;
    let mut maximum_reached_depth = 0;
    while let Some(item) = queue.pop_front() {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled weighted cut".into());
        }
        maximum_reached_depth = maximum_reached_depth.max(item.depth);
        let reason = match stop_reason(root, &item.key, item.depth, config.max_depth) {
            Some(reason) => reason.to_owned(),
            None => {
                charge(
                    &mut work.apply_calls,
                    1,
                    config.max_apply_calls,
                    "whole-batch Apply calls",
                )?;
                match apply(&item.key, item.depth, evidence, work)? {
                    Step::Applied(terms) => {
                        root_applied |= item.depth == 0;
                        if queue
                            .len()
                            .checked_add(terms.len())
                            .ok_or("pending overflow")?
                            > config.max_pending_terms
                        {
                            return Err("pending cut term cap".into());
                        }
                        for term in terms {
                            charge(
                                &mut work.products,
                                1,
                                limits.max_endpoint_occurrences,
                                "whole-batch weighted products",
                            )?;
                            let factor = checked_copy(
                                context,
                                &term.coefficient,
                                exact,
                                limits.max_coefficient_terms,
                            )?;
                            let product = context
                                .mul_with_limits(&item.coefficient, &factor, exact)
                                .map_err(err)?;
                            if cancel.load(Ordering::Relaxed) {
                                return Err("cancelled after native product".into());
                            }
                            let product = checked_copy(
                                context,
                                &product,
                                exact,
                                limits.max_coefficient_terms,
                            )?;
                            if product.is_zero() {
                                return Err("zero product of native nonzero field terms".into());
                            }
                            let factor_id = evidence.coefficient(&factor)?;
                            let incoming_id = evidence.coefficient(&item.coefficient)?;
                            let product_id = evidence.coefficient(&product)?;
                            evidence.record(json!({"kind":"substitution","depth":item.depth,"from":item.key.powers(),
                                "to":term.key.powers(),"incoming":incoming_id,"factor":factor_id,"product":product_id}))?;
                            queue.push_back(Pending {
                                key: term.key,
                                coefficient: product,
                                depth: item.depth + 1,
                            });
                        }
                        continue;
                    }
                    Step::Boundary(reason) => {
                        work.fallbacks += 1;
                        reason
                    }
                }
            }
        };
        charge(
            &mut work.boundary_occurrences,
            1,
            limits.max_endpoint_occurrences,
            "whole-batch cut boundary",
        )?;
        *reasons.entry(reason.clone()).or_default() += 1;
        let id = evidence.coefficient(&item.coefficient)?;
        evidence.record(json!({"kind":"cut_boundary","key":item.key.powers(),"depth":item.depth,"reason":reason,"coefficient":id}))?;
        total.add(item.key, &item.coefficient)?;
    }
    let mut after = 0;
    let mut rows = Vec::new();
    for (key, value) in &total.map {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled before cut publication".into());
        }
        let zero = value.is_zero();
        after += usize::from(!zero);
        let id = evidence.coefficient(value)?;
        evidence
            .record(json!({"kind":"cut_sum","key":key.powers(),"coefficient":id,"zero":zero}))?;
        let d = degree(key);
        let [a, r, e, delta] = [d.a, d.r, d.e, d.e - degree(root).e].map(i64::try_from);
        let (a, r, e, delta) = (
            a.map_err(err)?,
            r.map_err(err)?,
            e.map_err(err)?,
            delta.map_err(err)?,
        );
        rows.push(json!({"key":key.powers(),"coefficient":id,"zero":zero,"same_owner":same_owner(root,key),
            "A":a,"R":r,"E":e,"E_delta":delta}));
    }
    let report = json!({"root":root.powers(),"pre_coalescing_occurrences":total.occurrences,
        "pre_coalescing_distinct_keys":total.map.len(),"final_nonzero_keys":after,
        "keys_removed_by_exact_cancellation":total.map.len()-after,"coalescing_additions":total.additions,
        "root_applied":root_applied,"maximum_reached_depth":maximum_reached_depth,"stop_reasons":reasons,"boundary":rows});
    Ok(Cut {
        report,
        before: total.map.keys().cloned().collect(),
        after: total
            .map
            .into_iter()
            .filter(|(_, v)| !v.is_zero())
            .map(|(k, _)| k)
            .collect(),
    })
}

pub(super) fn observe<const N: usize>(
    session: &RoutedFeedbackSession<N>,
    request: &Request,
    config: &input::PlateauCut,
    queries: &Queries,
    output: &Path,
    cancel: &AtomicBool,
    preparation_seconds: f64,
) -> Result<(), String> {
    let programs = session.programs();
    let context = programs.context().coefficient_context();
    let exact = programs.context().limits().exact_algebra;
    let native = output.join("native");
    fs::create_dir(&native).map_err(err)?;
    let r = &request.observation.recorder_limits;
    let mut recorder = Recorder::new(&native, r.clone())?;
    let mut evidence = Evidence {
        table: Some(CoefficientTableBuilder::new(BinaryIoLimits {
            max_program_bytes: r
                .max_state_bytes
                .checked_add(r.max_total_atom_bytes)
                .ok_or("byte overflow")?,
            max_collection_entries: r.max_coefficients,
            max_state_bytes: r.max_state_bytes,
            max_atom_bytes: r.max_atom_bytes,
            max_total_atom_bytes: r.max_total_atom_bytes,
            ..Default::default()
        })),
        ledger: Vec::new(),
        max_records: request.limits.max_ledger_records,
    };
    let mut work = Work {
        apply_calls: 0,
        source_terms: 0,
        products: 0,
        boundary_occurrences: 0,
        fallbacks: 0,
        all_native_complete: true,
    };
    let started = Instant::now();
    let mut parents = Vec::new();
    let mut union_before = BTreeSet::new();
    let mut union_after = BTreeSet::new();
    let outcome = (|| -> Result<(), String> {
        for (parent, original_query) in queries.queries.iter().enumerate() {
            let owner = mask::<N>(&original_query.owner)?;
            let root = singleton(&owner, &original_query.lower, &original_query.upper)?;
            evidence.record(json!({"kind":"parent_begin","parent":parent,"id":original_query.id,"key":root.powers()}))?;
            let result = cut(
                &root,
                context,
                exact,
                &request.limits,
                config,
                cancel,
                &mut evidence,
                &mut work,
                |key, depth, evidence, work| {
                    let child_query;
                    let q = if depth == 0 {
                        original_query
                    } else {
                        child_query =
                            query(key, format!("parent-{parent}-call-{}", work.apply_calls));
                        &child_query
                    };
                    let call = work.apply_calls - 1;
                    recorder.begin_query(call, q)?;
                    let mut transaction = Transaction::default();
                    let mut capture_error = None;
                    let phase = Instant::now();
                    let result = programs.visit_power_bounded_owner_applied_successors(
                        mask::<N>(&q.owner)?,
                        &q.lower,
                        &q.upper,
                        q.max_numerator_rank,
                        q.power_bounds.native(),
                        request.observation.native_limits.build()?,
                        cancel,
                        |event| {
                            let captured = transaction.event(
                                &event,
                                q,
                                context,
                                programs.context().index_variables(),
                                exact,
                                &request.limits,
                                &mut work.source_terms,
                            );
                            let recorded = recorder.event(event);
                            if let Err(error) = captured {
                                capture_error = Some(error);
                                return ControlFlow::Break(());
                            }
                            recorded
                        },
                    );
                    let stats = match &result {
                        Ok(s) => *s,
                        Err(e) => e.stats,
                    };
                    let native_ok = result.is_ok();
                    let complete = recorder.end_query(result, phase.elapsed().as_secs_f64())?;
                    work.all_native_complete &= complete;
                    if recorder.stopped() {
                        return Err("native evidence recorder exhausted".into());
                    }
                    if cancel.load(Ordering::Relaxed) {
                        return Err("cancelled native Apply".into());
                    }
                    if let Some(error) = capture_error {
                        return Err(error);
                    }
                    let accepted = transaction.accepted(&stats, complete && native_ok);
                    evidence.record(json!({"kind":"native_apply","parent":parent,"call":call,"depth":depth,
                        "key":key.powers(),"selected":transaction.selected,"accepted":accepted,
                        "native_complete":complete,"source_terms":transaction.terms.len(),"finished":transaction.finished,
                        "fallback_reason":transaction.unsafe_reason,"original_native_evidence":format!("native/query-{call:06}.json")}))?;
                    if accepted {
                        Ok(Step::Applied(transaction.terms))
                    } else {
                        Ok(Step::Boundary(transaction.unsafe_reason.unwrap_or_else(
                            || "unmatched/refused/incomplete native transaction".into(),
                        )))
                    }
                },
            )?;
            for key in result.before {
                if !union_before.contains(&key)
                    && union_before.len() >= request.limits.max_distinct_keys
                {
                    return Err("whole-batch uncollected union key cap".into());
                }
                union_before.insert(key);
            }
            union_after.extend(result.after);
            let mut result = result.report;
            result["id"] = json!(original_query.id);
            result["parent"] = json!(parent);
            parents.push(result);
        }
        Ok(())
    })();
    // A failed native transaction may be a valid retained boundary. Recording
    // failure is different: it invalidates the observation even after rollback.
    let recorded = recorder.finish(
        json!({"plateau_cut":true,"all_native_transactions_complete":work.all_native_complete}),
        work.all_native_complete,
    )?;
    if !recorded {
        let report: Value = serde_json::from_str(&read_bounded(
            &native.join("result.json"),
            r.max_record_bytes,
        )?)
        .map_err(err)?;
        if !report["recorder_failure"].is_null() || !report["coefficient_encoding_error"].is_null()
        {
            return Err("native observation evidence encoding incomplete".into());
        }
    }
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
    let outcome = if cancel.load(Ordering::Relaxed) {
        Err("cancelled before cut report".into())
    } else {
        outcome
    };
    write_new_json(
        &output.join("result.json"),
        &json!({
            "schema":"rustred.plateau-cut.result.v1","complete":outcome.is_ok(),"failure":outcome.as_ref().err(),
            "dispatch":"pointwise-native; not whole-piece campaign dispatch","max_depth":config.max_depth,
        "parents_requested":queries.queries.len(),"parents_completed":parents.len(),"parents":parents,
        "shared_raw_key_support":{"uncollected_union":union_before.len(),"surviving_union":union_after.len(),
            "keys_removed_from_union":union_before.difference(&union_after).count(),
            "scope":"set union of separate parent supports; no cross-parent coefficient addition; no canonical routing"},
            "native_apply_calls":work.apply_calls,"source_term_visits":work.source_terms,"weighted_products":work.products,
            "boundary_occurrences":work.boundary_occurrences,"atomic_fallbacks":work.fallbacks,
            "all_native_transactions_complete":work.all_native_complete,
            "family_fingerprint":programs.context().family_owner().fingerprint(),
            "source_conditions_required":session.routed_reducer().domain_routing_requires_source_conditions(),
            "coefficient_table":{"state_blake3":blake3::hash(&table.state).to_hex().as_str(),"atoms_blake3":blake3::hash(&table.atoms).to_hex().as_str()},
            "preparation_seconds":preparation_seconds,"cut_seconds":started.elapsed().as_secs_f64(),
            "authority":"native saved-program applications, with all inherited original source/RHS poles and complete per-query records; generic Q(base), not exceptional parameter values",
            "claims":{"same_cut_exact_collection":outcome.is_ok(),"fresh_original_source_proof":false,"canonical_routing":false,"closure":false,"campaign_cost":false},
            "boundary_policy":"same original owner and E plateau only; all other keys retained with weights; no pinches erased or declared closed"
        }),
        request.limits.max_report_bytes,
    )?;
    outcome
}
