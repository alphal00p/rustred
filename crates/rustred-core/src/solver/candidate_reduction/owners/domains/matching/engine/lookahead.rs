//! Sufficient rejection of the CURRENT candidate on an unchanged exact cell.
//! No alternative rule is selected here, and no speculative cuts are retained.
use super::*;

impl<'a, const N: usize, F> Matcher<'a, '_, N, F>
where
    F: FnMut(OwnerDomainMatchPiece<N>) -> ControlFlow<()>,
{
    /// Only ordinary Unknown equality/exclusion predicates reach this helper.
    /// Source validity and operational refusals never enable lookahead. Any
    /// encountered failure retains its actual predicate identity. The caller
    /// treats only eligible native preflight refusals of this OPTIONAL probe as
    /// inconclusive and resumes the ORIGINAL refinement/Unknown path. A refusal
    /// is never a witness; all other failures remain strict.
    pub(super) fn rejection_lookahead(
        &mut self,
        cell: &LatticeBox,
        resume: PredicateResume,
    ) -> Result<Option<Phase<'a>>, (OwnerDomainPredicate, OwnerDomainMatchFailure)> {
        let PredicateResume::Rule {
            batch,
            index,
            stage,
        } = resume
        else {
            return Ok(None);
        };
        let (equality_start, branch_start) = match stage {
            RuleStage::Equality(ordinal) => (Some(ordinal + 1), 0),
            // The current branch contains an unknown atom, so it cannot be
            // proved all-zero. Its later nonzero witnesses were tried first.
            RuleStage::Exception(branch, _) => (None, branch + 1),
            RuleStage::Fixed | RuleStage::Denominator(_) => return Ok(None),
        };
        let programs = self.programs;
        let rule = &programs.owners[&self.owner].batches[batch].rules[index];
        let next = Some(Phase::Rule {
            batch,
            index: index + 1,
            stage: RuleStage::Fixed,
        });
        if let Some(start) = equality_start {
            for (ordinal, polynomial) in rule.equalities.iter().enumerate().skip(start) {
                if matches!(
                    self.probe_rejection_guard(
                        cell,
                        polynomial,
                        OwnerDomainPredicate::Equality {
                            batch,
                            rule: rule.ordinal,
                            ordinal,
                        },
                    )?,
                    Resolution::Nonzero
                ) {
                    return Ok(next);
                }
            }
        }
        for (branch, atoms) in rule.exceptions.iter().enumerate().skip(branch_start) {
            let mut all_zero = true;
            for (ordinal, polynomial) in atoms.iter().enumerate() {
                if !matches!(
                    self.probe_rejection_guard(
                        cell,
                        polynomial,
                        OwnerDomainPredicate::ExcludedConjunction {
                            batch,
                            rule: rule.ordinal,
                            branch,
                            ordinal,
                        },
                    )?,
                    Resolution::Zero
                ) {
                    // Nonzero, Unknown and even exact intersecting planes are
                    // not uniform zero. Never reinterpret a conservative cover
                    // as its zero set or combine witnesses from distinct cells.
                    all_zero = false;
                    break;
                }
            }
            // The empty conjunction is true, as in the concrete dispatcher.
            if all_zero {
                return Ok(next);
            }
        }
        for (term, rhs) in rule.rhs.iter().enumerate() {
            if matches!(
                self.probe_rejection_guard(
                    cell,
                    &rhs.denominator,
                    OwnerDomainPredicate::OriginalDenominator {
                        batch,
                        rule: rule.ordinal,
                        term,
                    },
                )?,
                Resolution::Zero
            ) {
                // Check the ORIGINAL denominator even for zero coefficients;
                // no normalization or RHS cancellation grants applicability.
                return Ok(next);
            }
        }
        Ok(None)
    }

    fn probe_rejection_guard(
        &mut self,
        cell: &LatticeBox,
        polynomial: &IndexedPolynomial,
        identity: OwnerDomainPredicate,
    ) -> Result<Resolution, (OwnerDomainPredicate, OwnerDomainMatchFailure)> {
        self.cancelled().map_err(|error| (identity, error))?;
        let result = guards::resolve(
            &self.programs.context.shared.context,
            polynomial,
            cell,
            &self.owner,
            self.rank,
            self.programs.context.limits.indexed_algebra,
            &mut self.budget,
        );
        self.cancelled().map_err(|error| (identity, error))?;
        result.map_err(|error| (identity, error))
    }
}
