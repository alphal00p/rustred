//! Query binding and saved-graph readiness, without an independent edge proof.
use super::super::*;
use super::*;

pub(super) struct Scope {
    pub complete: bool,
    pub report: Value,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check<const N: usize>(
    request: &OwnerDomainWalkRequest,
    options: &OwnerDomainWalkVerifyOptions,
    selection: &input::Selection,
    original: &[matching::input::Query],
    loaded: &Loaded<N>,
    reducer: &RoutedCandidateReducer<N>,
    closed: &[bool],
    violations: &mut Violations,
    cancellation: &AtomicBool,
) -> Result<Scope, AppError> {
    let amendments = request
        .amendments
        .iter()
        .map(|path| super::super::super::rescue::parse(path, selection.physical_arity()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::input)?;
    let base = checkpoint::rescue_chain_base(
        request,
        &loaded.raw.request,
        loaded.raw.g2_activation.as_ref(),
        &loaded.raw.amendments,
    )
    .map_err(AppError::input)?;
    super::super::super::rescue::check_chain(&loaded.raw.amendments, &amendments, &base, original)
        .map_err(AppError::input)?;
    if amendments.len() != loaded.raw.amendments.len() {
        violations.add("amendment_chain", || {
            "command and captured amendment inventory differ".into()
        });
    }
    let queries: Vec<_> = original
        .iter()
        .map(|query| (query, None))
        .chain(
            amendments
                .iter()
                .flat_map(|a| a.queries.iter().map(move |q| (q, Some(a.sequence)))),
        )
        .collect();
    let physics = match options.certification_scope {
        OwnerDomainWalkVerifyScope::AllRoots => false,
        OwnerDomainWalkVerifyScope::PhysicsQueries => true,
        OwnerDomainWalkVerifyScope::Auto => !amendments.is_empty(),
    };
    let cp6 = loaded
        .epoch
        .as_ref()
        .is_some_and(|e| e.manifest["format"] == "RUSTRED-WALK-CP6");
    let unadmitted = queries.len().saturating_sub(loaded.raw.inputs.len());
    if loaded.raw.inputs.len() > queries.len() || (!cp6 && unadmitted != 0) {
        violations.add("root_mapping", || {
            "captured input count differs from command queries".into()
        });
    }
    if cp6 {
        let metadata = &loaded.epoch.as_ref().expect("CP6 sections").manifest;
        if metadata["total_queries"].as_u64() != Some(original.len() as u64)
            || metadata["processed_queries"].as_u64()
                != Some(loaded.raw.inputs.len().min(original.len()) as u64)
        {
            violations.add("root_mapping", || {
                "CP6 processed query prefix differs".into()
            });
        }
    }
    let installed: BTreeSet<_> = reducer.programs().owner_sectors().copied().collect();
    // Only root admission uses exact inclusion. No per-edge covers, union
    // geometry, brute-force enumeration or forward cones are checked here.
    let containment = Containment::new(0, 0);
    let mut first = BTreeMap::new();
    let mut roots = Vec::new();
    let mut unresolved = 0usize;
    for (index, (query, amendment)) in queries.iter().enumerate() {
        if cancellation.load(Ordering::Relaxed) {
            break;
        }
        let Some(entry) = loaded.raw.inputs.get(index) else {
            roots.push(None);
            continue;
        };
        if entry["id"].as_str() != Some(query.id.as_str()) {
            violations.add("root_mapping", || {
                format!("captured input {index} differs from query {}", query.id)
            });
        }
        if cp6
            && (entry["role"].as_str()
                != Some(if query.auxiliary {
                    "auxiliary"
                } else {
                    "required"
                })
                || entry["role_declared"].as_bool() != Some(query.role_declared))
        {
            violations.add("root_mapping", || {
                format!("query {} role differs", query.id)
            });
        }
        if let Some(sequence) = amendment {
            if entry["amendment"].as_u64() != Some(*sequence) {
                violations.add("amendment_chain", || {
                    format!("query {} amendment differs", query.id)
                });
            }
        }
        let owner = rustred::storage_array::<_, N>(&query.owner, false)
            .ok_or_else(|| AppError::input("query owner exceeds native storage arity"))?;
        let phase = query_root_phase(
            installed.contains(&owner),
            request.route_domain_overcover,
            reducer.domain_routing_requires_source_conditions(),
            amendment.is_some(),
        );
        if cp6
            && phase.is_none()
            && entry["domain"].is_null()
            && entry["source_validity_unresolved"] == true
        {
            if loaded
                .raw
                .input_frontiers
                .get(unresolved)
                .is_none_or(|f| !epoch_checkpoint::source_frontier_matches::<N>(query, f))
            {
                violations.add("root_mapping", || {
                    format!("query {} source obligation differs", query.id)
                });
            }
            unresolved += 1;
            roots.push(None);
            continue;
        }
        let root = entry["domain"]
            .as_u64()
            .and_then(|id| usize::try_from(id).ok())
            .filter(|&id| {
                id < loaded.domains.len() && (amendment.is_some() || id < loaded.raw.counters[10])
            });
        let Some(root) = root else {
            violations.add("root_mapping", || {
                format!("query {} has no valid saved root", query.id)
            });
            roots.push(None);
            continue;
        };
        let first_admission = *first.entry(root).or_insert(index) == index;
        if !query_root_matches(
            &loaded.domains[root],
            phase,
            &query_cell::<N>(query),
            first_admission && amendment.is_none(),
            &containment,
        ) {
            violations.add("root_mapping", || {
                format!("query {} differs from its saved admitted scope", query.id)
            });
        }
        roots.push(Some(root));
    }
    if cp6
        && (first
            .range(..loaded.raw.counters[10])
            .map(|(&id, _)| id)
            .ne(0..loaded.raw.counters[10])
            || unresolved != loaded.raw.input_frontiers.len())
    {
        violations.add("root_mapping", || {
            "CP6 protected prefix or source-frontier inventory differs".into()
        });
    }
    for (id, &is_closed) in closed.iter().enumerate() {
        if loaded.raw.flags[id] & FLAG_CLOSED != 0 && !is_closed {
            violations.add("saved_closure", || {
                format!("record {id} is flagged closed but reaches an unfinished saved record")
            });
        }
    }
    let mut required = BTreeSet::new();
    let mut helpers = BTreeSet::new();
    let mut missing = Vec::new();
    for ((query, _), own) in queries.iter().zip(&roots) {
        if cancellation.load(Ordering::Relaxed) {
            break;
        }
        if query.auxiliary {
            if let Some(root) = own {
                helpers.insert(*root);
            }
        }
        if physics && query.auxiliary {
            continue;
        }
        let chosen = if physics {
            let cell = query_cell::<N>(query);
            let phase = own.map_or(Phase::Apply, |r| loaded.domains[r].phase());
            // A rescued required query can be discharged by a later closed
            // containing input root; helper roots are not all required closed.
            own.filter(|&id| closed[id]).or_else(|| {
                roots.iter().flatten().copied().find(|&id| {
                    closed[id]
                        && loaded.domains[id].phase() == phase
                        && containment.contains(&ccell(&loaded.domains[id]), &cell)
                })
            })
        } else {
            *own
        };
        if let Some(root) = chosen {
            required.insert(root);
        }
        if chosen.is_none_or(|root| !closed[root]) && missing.len() < 1000 {
            missing.push(query.id.clone());
        }
    }
    let complete = unadmitted == 0
        && unresolved == 0
        && roots.len() == queries.len()
        && (!options.require_closure || missing.is_empty())
        && !cancellation.load(Ordering::Relaxed);
    Ok(Scope {
        complete,
        report: json!({
            "certification_scope":if physics { "physics_queries_through_closed_containing_roots" } else { "all_roots" },
            "authority":"trusted_saved_scope","independently_verified":false,
            "query_roles":"immutable_exact_id_declaration; undeclared_queries_required",
            "queries":queries.len(),"unadmitted":unadmitted,"unresolved_input_frontiers":unresolved,
            "roots_total":required.len(),"roots_closed_in_saved_graph":required.iter().filter(|&&id| closed[id]).count(),
            "queries_without_closed_saved_root":missing,"helper_roots":helpers.len(),
            "helper_roots_closed":helpers.iter().filter(|&&id| closed[id]).count(),
            "amendments":amendments.len(),"complete":complete,
            "claim":"saved graph readiness and root bindings; geometric edge correctness and termination are not certified",
        }),
    })
}
