//! Assemble qualified finite sources and replay through the shared native kernel.
use super::*;

/// Source order is local snapshots, retained diagonal sums, then earlier
/// feedback equations. This order is the meaning of native provenance indices.
pub(super) fn inherited_rows(
    families: &BTreeMap<String, Arc<IntegralFamily>>,
    proofs: &[Arc<VacuumDiagonalCollectionPlan>],
    feedbacks: &[Arc<FiniteFeedback>],
    after_layer: usize,
    layer_ends: &[usize],
    limits: TerminalCollectionLimits,
) -> Result<(Vec<Row>, Vec<Coefficient>)> {
    let end = if after_layer == 0 {
        0
    } else {
        *layer_ends
            .get(after_layer - 1)
            .ok_or(VacuumCollectionError::ReplayFailed)?
    };
    let mut rows = Vec::new();
    let mut conditions = Vec::new();
    for proof in proofs
        .get(..end)
        .ok_or(VacuumCollectionError::ReplayFailed)?
    {
        for (key, alias) in proof.aliases().aliases() {
            let Some(family) = families.get(key.family_fingerprint()) else {
                continue;
            };
            rows.push(BTreeMap::from([
                (key.clone(), family.coefficient_context().one()),
                (
                    alias.representative().clone(),
                    family.coefficient_context().integer(-1),
                ),
            ]));
        }
        for source in proof.sources() {
            let Some(family) = families.get(source.family_fingerprint()) else {
                continue;
            };
            rows.push(
                source
                    .sum()
                    .terms
                    .iter()
                    .map(|(key, value)| {
                        (
                            VacuumIntegralKey::from_family(family, key.clone()),
                            value.clone(),
                        )
                    })
                    .collect(),
            );
            for value in &source.sum().nonzero_conditions {
                retain(
                    &mut conditions,
                    value.clone(),
                    limits.diagonal.max_conditions,
                )?;
            }
        }
    }
    for feedback in feedbacks {
        if feedback.after_layer > after_layer {
            return Err(VacuumCollectionError::ReplayFailed);
        }
        if !feedback
            .local
            .keys()
            .any(|family| families.contains_key(family))
        {
            continue;
        }
        if feedback
            .local
            .keys()
            .any(|family| !families.contains_key(family))
        {
            return Err(VacuumCollectionError::WrongFamily);
        }
        for equation in &feedback.result.equations {
            rows.push(equation.terms.clone());
        }
        for (key, alias) in feedback.result.aliases.aliases() {
            let family = &families[key.family_fingerprint()];
            rows.push(BTreeMap::from([
                (key.clone(), family.coefficient_context().one()),
                (
                    alias.representative().clone(),
                    family.coefficient_context().integer(-1),
                ),
            ]));
        }
        for value in feedback.result.conditions.iter() {
            retain(
                &mut conditions,
                value.clone(),
                limits.diagonal.max_conditions,
            )?;
        }
    }
    Ok((rows, conditions))
}

impl FiniteFeedback {
    pub(in crate::reduction::terminal_relations::collection::saved) fn layer(
        &self,
    ) -> BTreeMap<VacuumIntegralKey, &GuardedVacuumReduction> {
        self.result
            .reductions
            .iter()
            .flat_map(|(fingerprint, rows)| {
                let family = &self.result.families[fingerprint];
                rows.iter().map(move |(key, row)| {
                    (VacuumIntegralKey::from_family(family, key.clone()), row)
                })
            })
            .collect()
    }

    pub(in crate::reduction::terminal_relations::collection::saved) fn replay(
        after_layer: usize,
        local: BTreeMap<String, LocalRows>,
        raw: BTreeSet<VacuumIntegralKey>,
        all_families: &BTreeMap<String, Arc<IntegralFamily>>,
        proofs: &[Arc<VacuumDiagonalCollectionPlan>],
        layer_ends: &[usize],
        feedbacks: &[Arc<FiniteFeedback>],
        limits: TerminalCollectionLimits,
        cancel: Option<&AtomicBool>,
    ) -> Result<Self> {
        // Reuse the exact same coefficient/replay kernel with its independent
        // finite-row bounds; diagonal discovery keeps its smaller work recipe.
        let limits = TerminalCollectionLimits {
            diagonal: limits.finite_feedback,
            ..limits
        };
        let families = local
            .keys()
            .map(|id| {
                all_families
                    .get(id)
                    .map(|f| (id.clone(), Arc::clone(f)))
                    .ok_or(VacuumCollectionError::WrongFamily)
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let owner = families
            .values()
            .next()
            .ok_or(VacuumCollectionError::WrongFamily)?;
        let context = owner.coefficient_context();
        let (inherited, mut conditions) = inherited_rows(
            &families,
            proofs,
            feedbacks,
            after_layer,
            layer_ends,
            limits,
        )?;
        let mut rows = Vec::new();
        for (id, source) in &local {
            for value in &source.conditions {
                retain(
                    &mut conditions,
                    value.clone(),
                    limits.diagonal.max_conditions,
                )?;
            }
            for row in &source.rows {
                rows.push(
                    row.iter()
                        .map(|(key, value)| {
                            (
                                VacuumIntegralKey::from_family(&families[id], key.clone()),
                                value.clone(),
                            )
                        })
                        .collect::<Row>(),
                );
            }
        }
        rows.extend(inherited);
        check(
            "finite feedback source rows",
            rows.len(),
            limits.diagonal.max_source_rows,
        )?;
        check(
            "finite feedback source terms",
            rows.iter().map(BTreeMap::len).sum(),
            limits.diagonal.max_source_terms,
        )?;
        let mut support = raw.clone();
        support.extend(rows.iter().flat_map(|r| r.keys()).cloned());
        check(
            "finite feedback support",
            support.len(),
            limits.diagonal.max_columns,
        )?;
        let inventories: Vec<_> = families
            .iter()
            .map(|(id, family)| {
                (
                    Arc::clone(family),
                    support
                        .iter()
                        .filter(|k| k.family_fingerprint() == id)
                        .map(|k| k.integral().clone())
                        .collect(),
                )
            })
            .collect();
        if support
            .iter()
            .any(|k| !families.contains_key(k.family_fingerprint()))
        {
            return Err(VacuumCollectionError::WrongFamily);
        }
        // The existing exact full-U plan admits only supported scalar unit-mass
        // vacuum keys. Every other family/key stays a distinct column.
        let aliases = VacuumFamilyAliasPlan::prepare(&inventories, limits.diagonal.aliases)?;
        if let Some(cancel) = cancel {
            cancelled(cancel)?;
        }
        let canonical = |key: &VacuumIntegralKey| {
            aliases
                .representative(&families[key.family_fingerprint()], key.integral())
                .cloned()
                .map_err(|e| VacuumCollectionError::Alias(e.into()))
        };
        let mut targets = BTreeMap::new();
        for key in &raw {
            targets
                .entry(canonical(key)?)
                .or_insert_with(|| key.clone());
        }
        let mut auxiliaries = BTreeSet::new();
        let mut aliased_rows = Vec::new();
        for row in rows {
            let mut aliased = Row::new();
            for (key, value) in row {
                retain(
                    &mut conditions,
                    value.denominator.clone().into(),
                    limits.diagonal.max_conditions,
                )?;
                accumulate(
                    &mut aliased,
                    canonical(&key)?,
                    &value,
                    context,
                    limits.diagonal,
                )?;
            }
            auxiliaries.extend(
                aliased
                    .keys()
                    .filter(|k| !targets.contains_key(*k))
                    .cloned(),
            );
            aliased_rows.push(aliased);
        }
        let mut ordered: Vec<_> = targets.keys().cloned().collect();
        ordered.sort_by_key(|key| {
            (
                std::cmp::Reverse(
                    key.integral()
                        .powers()
                        .iter()
                        .map(|&n| i128::from(n.max(0)))
                        .sum::<i128>(),
                ),
                key.clone(),
            )
        });
        let auxiliary_count = auxiliaries.len();
        // Preserve each local native U's established pivot order whenever
        // aliases permit it. Lexicographic integral order can turn an already
        // triangular 5-loop rowspace into an unnecessarily dense new problem.
        let mut auxiliary_order = Vec::new();
        for (id, source) in &local {
            for key in &source.columns {
                let tagged = VacuumIntegralKey::from_family(&families[id], key.clone());
                if support.contains(&tagged) {
                    let representative = canonical(&tagged)?;
                    if auxiliaries.remove(&representative) {
                        auxiliary_order.push(representative);
                    }
                }
            }
        }
        auxiliary_order.extend(auxiliaries);
        let columns: Vec<_> = auxiliary_order.into_iter().chain(ordered).collect();
        let native_rows = aliased_rows.len();
        let result = finish_rows(
            Prepared {
                families,
                aliases,
                raw,
                targets,
                sources: Vec::new(),
                rows: aliased_rows,
                columns,
                auxiliary_count,
                conditions,
                native_rows,
            },
            limits.diagonal,
            true,
            cancel,
        )?;
        Ok(Self {
            after_layer,
            local,
            result,
        })
    }
}
