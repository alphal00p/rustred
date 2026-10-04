use super::*;

pub(super) struct RetentionBudget {
    limits: ReplayCircuitLimits,
    sources: usize,
    rhs: usize,
    conditions: usize,
    terms: usize,
    bytes: usize,
    coordinates: usize,
}

fn charge(
    total: &mut usize,
    added: usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), SourcePortAuditError> {
    *total = total
        .checked_add(added)
        .filter(|&n| n <= maximum)
        .ok_or(SourcePortAuditError::ResourceBudgetExhausted { resource })?;
    Ok(())
}

impl RetentionBudget {
    #[cfg(test)]
    pub(super) fn retained_bytes(&self) -> usize {
        self.bytes
    }

    pub(super) fn new(limits: ReplayCircuitLimits) -> Self {
        Self {
            limits,
            sources: 0,
            rhs: 0,
            conditions: 0,
            terms: 0,
            bytes: 0,
            coordinates: 0,
        }
    }
    pub(super) fn charge_rules(&mut self, count: usize) -> Result<(), SourcePortAuditError> {
        charge(
            &mut 0,
            count,
            self.limits.max_rules,
            "retained replay rules",
        )
    }
    fn polynomial(&mut self, p: &CoefficientPolynomial) -> Result<(), SourcePortAuditError> {
        charge(
            &mut self.terms,
            p.nterms(),
            self.limits.max_coefficient_terms,
            "retained replay coefficient terms",
        )?;
        let bytes = polynomial_clone_owned_heap_byte_bound(p)
            .and_then(|n| n.checked_add(std::mem::size_of::<CoefficientPolynomial>()))
            .ok_or(SourcePortAuditError::ResourceBudgetExhausted {
                resource: "retained replay coefficient bytes",
            })?;
        charge(
            &mut self.bytes,
            bytes,
            self.limits.max_coefficient_clone_owned_bytes,
            "retained replay coefficient bytes",
        )
    }
    fn coefficient(&mut self, c: &Coefficient) -> Result<(), SourcePortAuditError> {
        let terms = c
            .numerator
            .nterms()
            .checked_add(c.denominator.nterms())
            .ok_or(SourcePortAuditError::ResourceBudgetExhausted {
                resource: "retained replay coefficient terms",
            })?;
        charge(
            &mut self.terms,
            terms,
            self.limits.max_coefficient_terms,
            "retained replay coefficient terms",
        )?;
        let bytes = coefficient_clone_owned_retained_byte_bound(c).ok_or(
            SourcePortAuditError::ResourceBudgetExhausted {
                resource: "retained replay coefficient bytes",
            },
        )?;
        charge(
            &mut self.bytes,
            bytes,
            self.limits.max_coefficient_clone_owned_bytes,
            "retained replay coefficient bytes",
        )
    }
    pub(super) fn charge<const N: usize>(
        &mut self,
        rule: &SectorRule<N>,
        partition: &geometry::ApplicationPartition,
        checked: &replay::Replay<N>,
        inherited: &[CoefficientPolynomial],
    ) -> Result<(), SourcePortAuditError> {
        charge(
            &mut self.sources,
            checked.ordinary.contributions.len(),
            self.limits.max_source_entries,
            "retained replay sources",
        )?;
        charge(
            &mut self.rhs,
            rule.candidate.rhs.len(),
            self.limits.max_rhs_terms,
            "retained replay RHS",
        )?;
        let coordinate_vectors = partition
            .boxes
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(checked.ordinary.contributions.len()))
            .and_then(|n| n.checked_add(rule.candidate.rhs.len()))
            .and_then(|n| n.checked_add(3))
            .and_then(|n| n.checked_mul(N))
            .ok_or(SourcePortAuditError::ResourceBudgetExhausted {
                resource: "retained replay coordinates",
            })?;
        charge(
            &mut self.coordinates,
            coordinate_vectors,
            self.limits.max_coordinate_cells,
            "retained replay coordinates",
        )?;
        charge(
            &mut self.conditions,
            rule.exceptions.branches.len(),
            self.limits.max_conditions,
            "retained replay exception branches",
        )?;
        for p in inherited
            .iter()
            .chain(&checked.ordinary.source_conditions)
            .chain(
                checked
                    .ordinary
                    .contributions
                    .iter()
                    .map(|c| &c.weight.denominator),
            )
            .chain(
                rule.candidate
                    .rhs
                    .iter()
                    .map(|t| &t.coefficient.denominator),
            )
            .chain(rule.exceptions.branches.iter().flatten())
        {
            charge(
                &mut self.conditions,
                1,
                self.limits.max_conditions,
                "retained replay conditions",
            )?;
            self.polynomial(p)?;
        }
        for c in checked
            .ordinary
            .contributions
            .iter()
            .map(|c| &c.weight)
            .chain(rule.candidate.rhs.iter().map(|t| &t.coefficient))
            .chain(checked.raw_pivot.iter().map(|p| &p.coefficient))
        {
            self.coefficient(c)?;
        }
        Ok(())
    }
}
