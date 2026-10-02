//! Uniform optional replacement of an already selected baseline piece.
//! This never calls cut, refinement, or the ordinary matcher recursively.
use super::*;

impl<'a, const N: usize, F> Matcher<'a, '_, N, F>
where
    F: FnMut(OwnerDomainMatchPiece<N>) -> ControlFlow<()>,
{
    pub(super) fn after_baseline(
        &mut self,
        cell: &LatticeBox,
        rank: Option<u32>,
        baseline: OwnerDomainMatchDisposition,
    ) -> Result<OwnerDomainMatchDisposition, (OwnerDomainPredicate, OwnerDomainMatchFailure)> {
        let OwnerDomainMatchDisposition::SelectedRule { batch, .. } = baseline else {
            return Ok(baseline);
        };
        let programs = self.programs;
        let prepared = &programs.owners[&self.owner].batches[batch];
        'alternatives: for &index in &prepared.whole_piece_alternatives {
            let rule = &prepared.rules[index];
            let identity = OwnerDomainPredicate::WholePieceAlternative {
                batch,
                rule: rule.ordinal,
            };
            self.cancelled().map_err(|failure| (identity, failure))?;
            charge(
                &mut self.budget.stats.rules,
                1,
                self.budget.limits.max_rules,
                "rules",
            )
            .map_err(|failure| (identity, failure))?;
            // A sufficient whole-hull check. Correlation predicates remain
            // attached to the piece, never replaced by these projections.
            for (axis, fixed) in rule.fixed.iter().enumerate() {
                if let Some(n) = fixed {
                    let local = if self.owner[axis] {
                        (i64::from(*n) - 1) as u64
                    } else {
                        i64::from(*n).unsigned_abs()
                    };
                    if cell.lower()[axis] != local || cell.upper()[axis] != Some(local) {
                        continue 'alternatives;
                    }
                }
            }
            for (ordinal, p) in rule.equalities.iter().enumerate() {
                if !matches!(
                    self.alternative_guard(
                        cell,
                        rank,
                        p,
                        OwnerDomainPredicate::Equality {
                            batch,
                            rule: rule.ordinal,
                            ordinal
                        }
                    )?,
                    Resolution::Zero
                ) {
                    continue 'alternatives;
                }
            }
            for (branch, conjunction) in rule.exceptions.iter().enumerate() {
                let mut excluded_is_false = false;
                for (ordinal, p) in conjunction.iter().enumerate() {
                    if matches!(
                        self.alternative_guard(
                            cell,
                            rank,
                            p,
                            OwnerDomainPredicate::ExcludedConjunction {
                                batch,
                                rule: rule.ordinal,
                                branch,
                                ordinal
                            }
                        )?,
                        Resolution::Nonzero
                    ) {
                        excluded_is_false = true;
                        break;
                    }
                    // Unknown/Planes for one atom does not prevent a later
                    // uniform nonzero atom disproving the whole excluded AND.
                }
                if !excluded_is_false {
                    continue 'alternatives;
                }
            }
            for (term, rhs) in rule.rhs.iter().enumerate() {
                if !matches!(
                    self.alternative_guard(
                        cell,
                        rank,
                        &rhs.denominator,
                        OwnerDomainPredicate::OriginalDenominator {
                            batch,
                            rule: rule.ordinal,
                            term
                        }
                    )?,
                    Resolution::Nonzero
                ) {
                    continue 'alternatives;
                }
            }
            return Ok(OwnerDomainMatchDisposition::SelectedRule {
                batch,
                rule: rule.ordinal,
            });
        }
        Ok(baseline)
    }

    fn alternative_guard(
        &mut self,
        cell: &LatticeBox,
        rank: Option<u32>,
        polynomial: &IndexedPolynomial,
        identity: OwnerDomainPredicate,
    ) -> Result<Resolution, (OwnerDomainPredicate, OwnerDomainMatchFailure)> {
        self.cancelled().map_err(|failure| (identity, failure))?;
        let result = guards::resolve(
            &self.programs.context.shared.context,
            polynomial,
            cell,
            &self.owner,
            rank,
            self.programs.context.limits.indexed_algebra,
            &mut self.budget,
        );
        self.cancelled().map_err(|failure| (identity, failure))?;
        // Only normal Unknown/Planes values are optional misses. Resource,
        // corruption/context, cancellation and other typed failures propagate.
        result.map_err(|failure| (identity, failure))
    }
}
