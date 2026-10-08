//! Authenticate saved scope and enumerate classifications without certifying it.
use super::super::*;
use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::application::routed_campaign::walking::verify_closure) fn run<const N: usize>(
    request: &OwnerDomainWalkRequest,
    options: &OwnerDomainWalkVerifyOptions,
    selection: &input::Selection,
    queries: &[matching::input::Query],
    mut loaded: Loaded<N>,
    reducer: &RoutedCandidateReducer<N>,
    prepared_owners: Option<Vec<String>>,
    bound: bool,
    owners_match: bool,
    mut violations: Violations,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    collector: &Collector,
    started: Instant,
    loaded_seconds: f64,
    prepare_seconds: f64,
) -> Result<Value, AppError> {
    validate_shape(
        loaded.domains.len(),
        loaded.nodes.len(),
        loaded.raw.flags.len(),
    )?;
    let checks_started = Instant::now();
    let edges = loaded.raw.edges.len();
    let graph =
        Graph::from_edges(loaded.domains.len(), &loaded.raw.edges).map_err(AppError::input)?;
    loaded.raw.edges = Vec::new();
    let sealed = sealed_with(&loaded.nodes, &loaded.g2, &graph);
    structural(
        &loaded,
        &graph,
        &sealed,
        &mut violations,
        cancellation,
        observer,
    )?;
    // Linear graph bookkeeping detects an incomplete saved scope. The saved
    // edges are trusted here; their geometric/successor correctness is exactly
    // what the optional deep verifier establishes.
    let closed = graph.closed(&sealed);
    let scope = scope::check(
        request,
        options,
        selection,
        queries,
        &loaded,
        reducer,
        &closed,
        &mut violations,
        cancellation,
    )?;
    let result_binding = options
        .result
        .as_ref()
        .map(|path| bind_result(path, &loaded, &closed, &mut violations))
        .transpose()?;
    let checks_seconds = checks_started.elapsed().as_secs_f64();
    observer(
        json!({"event":"inventory_checked", "domains":loaded.domains.len(),
        "edges":edges, "seconds":checks_seconds, "scope_complete":scope.complete,
        "independently_verified":false, "memory":memory_status()}),
    );
    let census = if violations.is_empty() && !cancellation.load(Ordering::Relaxed) {
        census::run(
            request,
            options,
            &loaded,
            reducer,
            collector,
            cancellation,
            observer,
        )?
    } else {
        census::Summary::default()
    };
    let complete = violations.is_empty()
        && scope.complete
        && census.complete
        && !cancellation.load(Ordering::Relaxed);
    let verdict = if !violations.is_empty() {
        "FAIL"
    } else if complete {
        "TRUSTED_SAVED_SCOPE"
    } else {
        "INCOMPLETE"
    };
    Ok(json!({
        "schema":"rustred.walk-inventory-scope.v1",
        "verdict":verdict,"complete":complete,"authority":"trusted_saved_scope",
        "deep_verification":false,"independently_verified":false,
        "verdict_reason":if complete {
            "authenticated saved scope is complete; encountered identities were enumerated without independently verifying saved geometric covers or successor edges"
        } else if cancellation.load(Ordering::Relaxed) { "inventory cancelled" }
        else { "saved scope or inventory is incomplete or inconsistent" },
        "closure_required":options.require_closure,
        "certification_scope":scope.report["certification_scope"],
        "roots_total":scope.report["roots_total"],"roots_independently_verified":0,
        "scope":scope.report,"family_closure_claim":false,
        "checkpoint":{"directory":options.checkpoint,"generation":loaded.raw.generation,
            "publication_policy":loaded.raw.publication_policy,
            "walk_semantics_version":loaded.raw.walk_semantics_version,
            "executable":loaded.raw.executable,"request_digest":loaded.raw.request,
            "request_binding_matches":bound,"owner_digests_match":owners_match,
            "prepared_owner_payload_blake3":prepared_owners,
            "prepared_owner_payload_order":"selection owners, then domain_rule_overlays, then preferred_owner_programs; selection order within each group",
            "file_digest_verify_seconds":loaded.raw.verify_seconds,"engine_closure":loaded.raw.closure},
        "result_binding":result_binding,
        "counts":{"domains":loaded.domains.len(),"edges":edges,
            "sealed":sealed.iter().filter(|&&v| v).count(),
            "closed_in_saved_graph":closed.iter().filter(|&&v| v).count()},
        "reinspection":{"mode":"not_requested","complete":false,"selected":0,
            "note":"native matching enumerates identities; it does not certify successor generation or edge coverage"},
        "inventory_census":census.json(),
        "checks":{"immutable_file_digests":true,"request_and_owner_bindings":true,
            "record_structure_and_saved_graph_completion":true,
            "per_edge_geometry":false,"partial_and_g2_union_covers":false,
            "route_successor_replay":false,"lattice_enumeration":false,
            "independent_closure_certificate":false},
        "violations":violations.list,"violations_by_class":violations.by_class,
        "violations_suppressed":violations.suppressed,
        "timing":{"load_seconds":loaded_seconds,"prepare_seconds":prepare_seconds,
            "checks_seconds":checks_seconds,"inventory_seconds":census.seconds,
            "reinspect_seconds":0,"total_seconds":started.elapsed().as_secs_f64()},
        "memory":memory_status(),
    }))
}

fn validate_shape(domains: usize, nodes: usize, flags: usize) -> Result<(), AppError> {
    if nodes != domains || flags != domains {
        return Err(AppError::input(
            "inventory checkpoint domain, record and flag counts differ",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_record_or_flag_inventory_is_a_typed_error() {
        assert!(validate_shape(2, 2, 1).is_err());
        assert!(validate_shape(2, 1, 2).is_err());
        assert!(validate_shape(2, 2, 3).is_err());
        assert!(validate_shape(2, 2, 2).is_ok());
    }
}

fn structural<const N: usize>(
    loaded: &Loaded<N>,
    graph: &Graph,
    sealed: &[bool],
    violations: &mut Violations,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<(), AppError> {
    let mut last = Instant::now();
    for (id, node) in loaded.nodes.iter().enumerate() {
        if id % 4096 == 0 {
            if cancellation.load(Ordering::Relaxed) {
                return Ok(());
            }
            if last.elapsed() >= Duration::from_secs(10) {
                observer(
                    json!({"event":"inventory_scope_progress","records":id,"total":loaded.nodes.len()}),
                );
                last = Instant::now();
            }
        }
        if (loaded.raw.flags[id] & FLAG_SEALED != 0) != sealed[id] {
            violations.add("seal_parity", || {
                format!("node {id} seal flag differs from committed record")
            });
        }
        if node.abandoned && (loaded.raw.amendments.is_empty() || !graph.out(id).is_empty()) {
            violations.add("rescue_abandoned", || {
                format!("record {id} has no rescue amendment or has dependency edges")
            });
        }
        match node.kind {
            Kind::Alias => {
                if node.link >= loaded.nodes.len()
                    || !graph.has_edge(id, node.link)
                    || loaded.domains[id].phase() != loaded.domains[node.link].phase()
                    || loaded.domains[id].owner() != loaded.domains[node.link].owner()
                {
                    violations.add("alias_structure", || {
                        format!("alias {id} has invalid representative or bucket")
                    });
                }
            }
            Kind::Partial => {
                let valid = node.cut != i64::MIN
                    && loaded.domains[id].phase() == Phase::Apply
                    && node.link < id
                    && node.link < loaded.raw.counters[10]
                    && loaded
                        .nodes
                        .get(node.link)
                        .is_some_and(|anchor| anchor.kind == Kind::Native)
                    && graph.has_edge(id, node.link)
                    && loaded.residuals.get(&id)
                        == Some(&residual(loaded.domains[id].powers(), node.cut));
                if !valid {
                    violations.add("partial_structure", || {
                        format!("partial {id} has invalid anchor or recorded residual")
                    });
                }
            }
            Kind::G2 => {
                let Some(info) = loaded.g2.get(&id) else {
                    continue;
                };
                if loaded.domains[id].phase() != Phase::Apply
                    || id < loaded.raw.counters[10]
                    || info.merge_stamp != Some(loaded.positions[id])
                    || info.snapshot > loaded.positions[id]
                    || info.anchors.is_empty()
                {
                    violations.add("g2_structure", || {
                        format!("G2 record {id} has invalid phase, stamps or anchors")
                    });
                }
                if let Some(bounds) = info.residual {
                    let whole = loaded.domains[id].powers();
                    if bounds.max_positive_power != whole.max_positive_power
                        || bounds.min_power_difference.is_none()
                        || bounds.max_power_difference.is_none()
                        || bounds.min_power_difference > bounds.max_power_difference
                        || !whole.contains(&bounds)
                    {
                        violations.add("g2_residual", || {
                            format!("G2 record {id} has invalid residual bounds")
                        });
                    }
                } else if node.events.unwrap_or(0) != 0 || node.error || node.frontiers != 0 {
                    violations.add("g2_full_cover", || {
                        format!("G2 full-cover record {id} carries native events")
                    });
                }
                for anchor in &info.anchors {
                    let Some(other) = loaded.nodes.get(anchor.id) else {
                        violations.add("g2_anchor", || {
                            format!("G2 record {id} anchor outside checkpoint")
                        });
                        continue;
                    };
                    let kind = match (anchor.kind.as_str(), other.kind) {
                        ("native", Kind::Native) | ("initial_d_band", Kind::Partial) => true,
                        ("g2_residual", Kind::G2) => loaded
                            .g2
                            .get(&anchor.id)
                            .is_some_and(|g| g.residual.is_some()),
                        _ => false,
                    };
                    if !kind
                        || !sealed[anchor.id]
                        || anchor.stamp != loaded.positions[anchor.id]
                        || anchor.stamp >= info.snapshot
                        || !graph.has_edge(id, anchor.id)
                        || loaded.domains[id].phase() != loaded.domains[anchor.id].phase()
                        || loaded.domains[id].owner() != loaded.domains[anchor.id].owner()
                    {
                        violations.add("g2_anchor", || {
                            format!("G2 record {id} has invalid anchor {}", anchor.id)
                        });
                    }
                }
            }
            _ => {}
        }
    }
    let epoch = loaded.raw.publication_policy == "epoch";
    let nodes = &loaded.nodes;
    let natives = nodes
        .iter()
        .filter(|node| node.native() && (!epoch || !node.abandoned))
        .count();
    let completed = nodes
        .iter()
        .filter(|node| node.native() && !node.error && (!epoch || !node.abandoned))
        .count();
    if natives != loaded.raw.counters[7] || completed != loaded.raw.counters[6] {
        violations.add("native_counter", || {
            "saved native/completed counters differ from committed records".into()
        });
    }
    let frontiers: u64 = nodes
        .iter()
        .filter(|node| !epoch || !node.abandoned)
        .map(|node| u64::from(node.frontiers))
        .sum();
    let carried: u64 = loaded
        .raw
        .uncommitted
        .iter()
        .map(|record| {
            record["frontiers"]
                .as_array()
                .map_or(0, |rows| rows.len() as u64)
        })
        .sum();
    if frontiers + carried + loaded.raw.input_frontiers.len() as u64
        != loaded.raw.counters[5] as u64
    {
        violations.add("frontier_counter", || {
            "saved frontier count differs from recorded obligations".into()
        });
    }
    if loaded.raw.uncommitted.is_empty() {
        let events: u64 = nodes
            .iter()
            .filter(|node| node.native())
            .map(|node| node.events.unwrap_or(0))
            .sum();
        let successors: u64 = nodes
            .iter()
            .filter(|node| node.native())
            .map(|node| node.successors.unwrap_or(0))
            .sum();
        if events != loaded.raw.counters[0] as u64 || successors != loaded.raw.counters[1] as u64 {
            violations.add("event_counter", || {
                "saved event counts differ from committed records".into()
            });
        }
    }
    Ok(())
}
