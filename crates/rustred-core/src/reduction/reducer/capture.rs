//! Exact per-integral guard provenance for explicit cache preparation.
//!
//! Coefficients are computed by the existing reducer. This module records only
//! their validity dependencies, including nested product-moment reductions.
//! Ordinary application does not allocate a provenance arena.
use super::*;
use crate::reduction::GuardedCaptureStatistics;

impl<'artifact> Reducer<'artifact> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn reduce_artifact_unit_mass_guarded_capturing<E: From<ReductionError>>(
        artifact: &'artifact ClosedArtifact,
        target: &IntegralKey,
        limits: ReductionLimits,
        shared: Arc<SharedCacheBudget>,
        lookup: &mut GuardedRootLookup<'_, E>,
        capture: &mut GuardedRootCapture<'_, E>,
        metadata_limit: Option<usize>,
        statistics: &mut GuardedCaptureStatistics,
    ) -> Result<(MasterDecomposition, Vec<Coefficient>, ReductionStatistics), E> {
        let mut reducer = Self::build(artifact, limits, Arc::clone(&shared))?;
        let mut request = ReductionRequest {
            guarded_conditions: Some(Vec::new()),
            guard_provenance: Some(GuardProvenance::new(shared, limits, metadata_limit)),
            ..Default::default()
        };
        let result = (|| {
            reducer.validate_target(target)?;
            let canonical_target = reducer.canonicalize(target)?;
            let unit = reducer.reduce_canonical_unit_mass(
                &canonical_target,
                &mut request,
                Some(lookup),
                Some(capture),
            )?;
            let unit = if canonical_target == *target {
                unit
            } else {
                MasterDecomposition::new(
                    reducer.family_fingerprint.clone(),
                    target.clone(),
                    unit.into_terms(),
                )
            };
            // The union remains available if optional provenance metadata was
            // exhausted. That stops intermediate capture, never native descent
            // or retention of the requested result's conditions.
            let conditions = request
                .guard_provenance
                .as_ref()
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "capturing closed request lost its guard arena",
                })?
                .all_conditions();
            Ok((unit, conditions, reducer.statistics()))
        })();
        if let Some(graph) = &request.guard_provenance {
            *statistics = graph.statistics();
        }
        result
    }

    /// IDs are global to the request and keyed by the immutable artifact as
    /// well as powers. Equal powers in different lower families cannot alias.
    pub(super) fn prepare_guard_node(
        &self,
        target: &IntegralKey,
        request: &mut ReductionRequest,
    ) -> Result<Option<GuardNodeId>, ReductionError> {
        let Some(graph) = request.guard_provenance.as_mut() else {
            return Ok(None);
        };
        let node = graph.node_for(self.artifact as *const ClosedArtifact as usize, target)?;
        if !graph.is_active() || !graph.is_complete(node) {
            for condition in self.artifact.family().domain().conditions() {
                if condition.polynomial().is_zero() || !condition.polynomial().is_constant() {
                    graph.add_local(node, &[condition.polynomial().clone().into()])?;
                }
            }
            // Masters were seeded before the request. They have no rule
            // dependencies, but still inherit their own family's conditions.
            if self.artifact.masters().contains(target) {
                graph.finish(node)?;
            }
        }
        Ok(Some(node))
    }

    pub(super) fn finish_guard_node(
        &self,
        target: &IntegralKey,
        request: &mut ReductionRequest,
    ) -> Result<(), ReductionError> {
        if let Some(graph) = request.guard_provenance.as_mut() {
            let node = graph.node_for(self.artifact as *const ClosedArtifact as usize, target)?;
            if !graph.is_complete(node) {
                graph.finish(node)?;
            }
        }
        Ok(())
    }

    pub(super) fn retain_child_guard_nodes(
        &self,
        target: &IntegralKey,
        terms: &[(IntegralKey, Coefficient)],
        request: &mut ReductionRequest,
    ) -> Result<(), ReductionError> {
        let Some(parent) = self.prepare_guard_node(target, request)? else {
            return Ok(());
        };
        for (child, _) in terms {
            let child = self.prepare_guard_node(child, request)?.ok_or(
                ReductionError::ReducerInvariant {
                    detail: "captured child lost its guard node",
                },
            )?;
            request
                .guard_provenance
                .as_mut()
                .unwrap()
                .add_child(parent, child)?;
        }
        Ok(())
    }

    pub(super) fn capture_completed_node<E: From<ReductionError>>(
        &self,
        target: &IntegralKey,
        requested: &IntegralKey,
        request: &mut ReductionRequest,
        capture: &mut Option<&mut GuardedRootCapture<'_, E>>,
    ) -> Result<(), E> {
        // Identity/zero lookups are already cheap native leaves. The root has
        // its own publication and must never be counted as an intermediate.
        if target == requested
            || self.artifact.masters().contains(target)
            || self.artifact.is_zero_terminal(target)
        {
            return Ok(());
        }
        let (Some(capture), Some(graph)) = (capture.as_mut(), request.guard_provenance.as_mut())
        else {
            return Ok(());
        };
        if !graph.is_active() {
            graph.record_skipped();
            return Ok(());
        }
        let node = graph.node_for(self.artifact as *const ClosedArtifact as usize, target)?;
        let value = self
            .cache
            .get(target)
            .ok_or(ReductionError::ReducerInvariant {
                detail: "captured closed intermediate has no completed coefficients",
            })?;
        capture(target, value, graph, node)
    }

    pub(super) fn retain_factorization_guard(
        &self,
        ordinal: usize,
        request: &mut ReductionRequest,
    ) -> Result<(), ReductionError> {
        if request.guard_provenance.is_none() && !request.ordered_track_guards {
            return Ok(());
        }
        let rule = self.artifact.factorization_rules().get(ordinal).ok_or(
            ReductionError::ReducerInvariant {
                detail: "captured factorization has no rule",
            },
        )?;
        request.retain_nonzero_guard(
            &rule.normalization().denominator.clone().into(),
            self.limits,
        )
    }

    /// Same selected conditions as the ordinary final DAG walk, recorded at
    /// their actual node. Original denominators survive RHS cancellation.
    pub(super) fn retain_selected_rule_guards(
        &self,
        selected: &SelectedRule<'_>,
        request: &mut ReductionRequest,
    ) -> Result<(), ReductionError> {
        if request.guard_provenance.is_none() && !request.ordered_track_guards {
            return Ok(());
        }
        let guards = match selected.cell_guards {
            Some(guards) => guards
                .iter()
                .map(|guard| guard.polynomial())
                .collect::<Vec<_>>(),
            None => selected
                .rule
                .nonzero_guards()
                .iter()
                .map(|guard| guard.polynomial())
                .collect(),
        };
        for guard in guards {
            let value = self
                .artifact
                .indexed_context()
                .specialize_polynomial_sealed(
                    guard,
                    &selected.assignment,
                    self.limits.indexed_algebra,
                )?;
            request.retain_nonzero_guard(&value.into(), self.limits)?;
        }
        // Original coefficient denominators are retained by the unchanged
        // specialization in apply_selected_rule, without a second CAS pass.
        Ok(())
    }
}
