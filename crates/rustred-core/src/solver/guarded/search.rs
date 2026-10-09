use super::{
    GuardedRule, GuardedSolution, GuardedSourceSystem, GuardedUnresolved, GuardedUnresolvedReason,
    IndexBounds, IndexDomain, IndexRole,
};
use crate::algebra::Coefficient;
use crate::solver::{
    Case, CoordinateCase, ExactRow, IntegralOrder, PolynomialRow, Power, RuleCandidate,
    SearchOptions, SectorSolver, Seed, SolverError, Term, extract_exceptions,
};
use std::collections::VecDeque;

pub(in crate::solver) struct GuardedSearchScope<'a, const N: usize> {
    pub problem: &'a GuardedSourceSystem<N>,
    pub domain: IndexDomain<N>,
}

impl<'a, const N: usize> GuardedSearchScope<'a, N> {
    pub fn new(problem: &'a GuardedSourceSystem<N>, domain: IndexDomain<N>) -> Self {
        Self { problem, domain }
    }

    pub fn source_domain(&self, seed: &Seed<N>) -> Result<IndexDomain<N>, SolverError> {
        image_domain(&self.domain, &seed.integral)
    }

    pub fn instantiate(
        &self,
        ordinal: usize,
        source: &PolynomialRow<N>,
        seed: &Seed<N>,
        order: &IntegralOrder<N>,
    ) -> Result<Option<ExactRow<N>>, SolverError> {
        let info = self.problem.sources.get(ordinal).ok_or_else(|| {
            SolverError::InvalidInput("guarded source ordinal is out of range".into())
        })?;
        if !self.source_domain(seed)?.is_subset_of(&info.domain) {
            return Ok(None);
        }
        let row = crate::solver::instantiate::instantiate(
            source,
            seed,
            self.problem.system.index_variables(),
            self.problem.system.fixed(),
            order,
            &[],
            None,
        )?;
        let mut result: ExactRow<N> = Vec::with_capacity(row.len());
        for term in row {
            if let Some(last) = result
                .last_mut()
                .filter(|last| last.integral == term.integral)
            {
                last.coefficient = &last.coefficient + &term.coefficient;
            } else {
                result.push(term);
            }
        }
        result.retain(|term| !term.coefficient.is_zero());
        let valid = IndexDomain::for_roles(&self.problem.roles);
        for term in &result {
            if !image_domain(&self.domain, &term.integral)?.is_subset_of(&valid) {
                return Ok(None);
            }
        }
        Ok(Some(result))
    }
}

pub(super) fn image_domain<const N: usize>(
    domain: &IndexDomain<N>,
    integral: &crate::solver::Integral<N>,
) -> Result<IndexDomain<N>, SolverError> {
    let mut bounds = *domain.bounds();
    for (axis, bound) in bounds.iter_mut().enumerate() {
        let power = integral[axis];
        if power.is_symbolic() {
            let shift = i64::from(power.value());
            let add = |value: i64| {
                value.checked_add(shift).ok_or_else(|| {
                    SolverError::InvalidInput("source-domain translation overflow".into())
                })
            };
            *bound = IndexBounds::new(
                bound.lower().map(add).transpose()?,
                bound.upper().map(add).transpose()?,
            )?;
        } else {
            *bound = IndexBounds::fixed(i64::from(power.value()));
        }
    }
    IndexDomain::new(bounds)
}

impl<const N: usize> GuardedSourceSystem<N> {
    /// Discover reusable rules on one explicit integer box. Search and domain
    /// traversal are bounded; uncovered pieces remain in `unresolved`.
    pub fn solve_domain(
        &self,
        domain: IndexDomain<N>,
        options: SearchOptions,
    ) -> Result<GuardedSolution<N>, SolverError> {
        self.solve_domains(vec![domain], options, 256)
    }

    pub fn solve_domains(
        &self,
        domains: Vec<IndexDomain<N>>,
        options: SearchOptions,
        max_domains: usize,
    ) -> Result<GuardedSolution<N>, SolverError> {
        if options.max_depth.is_none() || max_domains == 0 {
            return Err(SolverError::InvalidInput(
                "guarded discovery requires a finite search depth and positive domain budget"
                    .into(),
            ));
        }
        let valid = IndexDomain::for_roles(&self.roles);
        if domains.iter().any(|domain| !domain.is_subset_of(&valid)) {
            return Err(SolverError::InvalidInput(
                "requested domain includes a negative occupation index".into(),
            ));
        }
        let mut pending: VecDeque<_> = domains.into();
        let mut visited = Vec::new();
        let mut solution = GuardedSolution {
            rules: Vec::new(),
            unresolved: Vec::new(),
        };
        let mut work = 0;
        while let Some(domain) = pending.pop_front() {
            if visited.contains(&domain) {
                continue;
            }
            if work >= max_domains {
                unresolved(
                    &mut solution,
                    domain,
                    GuardedUnresolvedReason::DomainBudget,
                    "domain traversal budget exhausted",
                );
                for domain in pending {
                    unresolved(
                        &mut solution,
                        domain,
                        GuardedUnresolvedReason::DomainBudget,
                        "domain traversal budget exhausted",
                    );
                }
                break;
            }
            work += 1;
            visited.push(domain.clone());
            // Ordinary signs remain sector coordinates; occupation zero is a
            // fixed bulk face. Neither partition asserts a zero integral.
            if let Some(axis) = domain
                .bounds()
                .iter()
                .position(|b| b.lower().is_none_or(|n| n <= 0) && b.upper().is_none_or(|n| n > 0))
            {
                let (left, right) = domain.split(axis, 0)?;
                pending.extend(left);
                pending.extend(right);
                continue;
            }
            if self.roles.iter().enumerate().any(|(axis, role)| {
                *role == IndexRole::RequiredCut
                    && domain.bounds()[axis].upper().is_some_and(|n| n <= 0)
            }) || self
                .zero_domains
                .iter()
                .any(|zero| domain.is_subset_of(zero))
            {
                continue;
            }
            if domain.bounds().iter().any(|bound| {
                bound
                    .lower()
                    .zip(bound.upper())
                    .is_some_and(|(lower, upper)| {
                        lower == upper
                            && !(i64::from(Power::MIN)..=i64::from(Power::MAX)).contains(&lower)
                    })
            }) {
                unresolved(
                    &mut solution,
                    domain,
                    GuardedUnresolvedReason::UnsupportedPower,
                    "fixed target exceeds native compact power storage",
                );
                continue;
            }
            let (sector, case) = domain_case(&domain)?;
            let scope = GuardedSearchScope::new(self, domain.clone());
            let solver = SectorSolver::new_guarded(&self.system, sector, self.roles)?;
            let candidate = match solver.solve_guarded_case(case.clone().into(), options, &scope) {
                Ok(candidate) => candidate,
                Err(SolverError::SearchExhausted { .. }) => {
                    // A boundary-only identity cannot be admitted uniformly on
                    // a ray. Refine at guard endpoints reachable by this seed
                    // budget, then let the native search handle each face.
                    if let Some((axis, at)) =
                        self.source_boundary(&domain, options.max_depth.unwrap())
                    {
                        let (left, right) = domain.split(axis, at)?;
                        pending.extend(left);
                        pending.extend(right);
                        continue;
                    }
                    unresolved(
                        &mut solution,
                        domain,
                        GuardedUnresolvedReason::SearchExhausted,
                        "no rule found within the source-seed budget",
                    );
                    continue;
                }
                Err(SolverError::Power(error)) => {
                    unresolved(
                        &mut solution,
                        domain,
                        GuardedUnresolvedReason::UnsupportedPower,
                        &error.to_string(),
                    );
                    continue;
                }
                Err(error) => return Err(error),
            };
            // Make physical sign changes exact before certifying descent.
            // The refined boundary is searched again with its own fixed face.
            if let Some((axis, at)) = candidate.rhs.iter().find_map(|term| {
                (0..N).find_map(|axis| {
                    if self.roles[axis] == IndexRole::Occupation
                        || !term.integral[axis].is_symbolic()
                    {
                        return None;
                    }
                    let at = -i64::from(term.integral[axis].value());
                    let bound = domain.bounds()[axis];
                    (bound.lower().is_none_or(|n| n <= at) && bound.upper().is_none_or(|n| n > at))
                        .then_some((axis, at))
                })
            }) {
                let (left, right) = domain.split(axis, at)?;
                pending.extend(left);
                pending.extend(right);
                continue;
            }
            let rule =
                match self.seal_candidate(candidate, solver.ordering().clone(), domain.clone()) {
                    Ok(rule) => rule,
                    Err(SolverError::Certification(detail)) => {
                        unresolved(
                            &mut solution,
                            domain,
                            GuardedUnresolvedReason::UnprovedDescent,
                            &detail,
                        );
                        continue;
                    }
                    Err(error) => return Err(error),
                };
            // A box difference has at most 2N pieces. Preserve them even when
            // the traversal budget is smaller; the queue reports each unvisited
            // piece as unresolved instead of discarding the discovered rule.
            pending.extend(domain.difference(&rule.domain, N.saturating_mul(2))?);
            // Reuse native polynomial exception extraction and exact case
            // intersections for index-only poles, including source weights.
            let exceptions = condition_exceptions(&rule, self.system.index_variables(), &sector)?;
            for branch in exceptions.branches {
                match Case::from(case.clone()).intersect_many(
                    &branch,
                    self.system.index_variables(),
                    &sector,
                    Default::default(),
                ) {
                    Ok(intersection) => {
                        for child in intersection.cases {
                            if let Case::Coordinate(child) = child {
                                if let Some(child_domain) =
                                    IndexDomain::from_sector_case(&sector, &child)
                                        .and_then(|d| d.intersection(&rule.domain))
                                {
                                    if child_domain == domain {
                                        unresolved(
                                            &mut solution,
                                            child_domain,
                                            GuardedUnresolvedReason::ExceptionalCondition,
                                            "condition vanishes throughout the requested case",
                                        );
                                    } else {
                                        pending.push_back(child_domain);
                                    }
                                }
                            } else {
                                unresolved(
                                    &mut solution,
                                    rule.domain.clone(),
                                    GuardedUnresolvedReason::ExceptionalCondition,
                                    "coupled exceptional locus remains guarded; coordinate-box refinement is unavailable",
                                );
                            }
                        }
                    }
                    Err(error) => unresolved(
                        &mut solution,
                        rule.domain.clone(),
                        GuardedUnresolvedReason::ExceptionalCondition,
                        &error.to_string(),
                    ),
                }
            }
            solution.rules.push(rule);
        }
        Ok(solution)
    }

    fn source_boundary(&self, domain: &IndexDomain<N>, depth: u32) -> Option<(usize, i64)> {
        // Native compact symbolic displacements cannot exceed this range.
        let depth = i64::from(depth.min(127));
        for source in &self.sources {
            for axis in 0..N {
                let guard = source.domain.bounds()[axis];
                for shift in -depth..=depth {
                    let endpoints = [
                        guard
                            .lower()
                            .and_then(|n| n.checked_sub(shift)?.checked_sub(1)),
                        guard.upper().and_then(|n| n.checked_sub(shift)),
                    ];
                    for at in endpoints.into_iter().flatten() {
                        let bound = domain.bounds()[axis];
                        if bound.lower().is_none_or(|n| n <= at)
                            && bound.upper().is_none_or(|n| n > at)
                        {
                            return Some((axis, at));
                        }
                    }
                }
            }
        }
        None
    }
}

fn domain_case<const N: usize>(
    domain: &IndexDomain<N>,
) -> Result<([bool; N], CoordinateCase<N>), SolverError> {
    let sector = std::array::from_fn(|axis| domain.bounds()[axis].lower().is_some_and(|n| n >= 1));
    let mut fixed = [None; N];
    for (axis, bound) in domain.bounds().iter().enumerate() {
        if let (Some(lower), Some(upper)) = (bound.lower(), bound.upper()) {
            if lower == upper {
                let value = i16::try_from(lower).map_err(|_| {
                    SolverError::InvalidInput("fixed target exceeds compact index storage".into())
                })?;
                Power::new(false, value)?;
                fixed[axis] = Some(value);
            }
        }
    }
    Ok((sector, CoordinateCase::new(fixed)?))
}

fn unresolved<const N: usize>(
    solution: &mut GuardedSolution<N>,
    domain: IndexDomain<N>,
    reason: GuardedUnresolvedReason,
    detail: &str,
) {
    solution.unresolved.push(GuardedUnresolved {
        domain,
        reason,
        detail: detail.into(),
    });
}

fn condition_exceptions<const N: usize>(
    rule: &GuardedRule<N>,
    indices: &[usize; N],
    sector: &[bool; N],
) -> Result<crate::solver::ExceptionalConditions, SolverError> {
    use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
    let rhs = rule
        .nonzero_conditions
        .iter()
        .map(|p| Term {
            integral: rule.candidate.target,
            coefficient: Coefficient::from_num_den(
                p.one(),
                p.clone(),
                &symbolica::prelude::Z,
                false,
            ),
        })
        .collect();
    let candidate = RuleCandidate {
        case: rule.candidate.case.clone(),
        target: rule.candidate.target,
        rhs,
        sources: Vec::new(),
        stats: Default::default(),
    };
    extract_exceptions(&candidate, indices, sector)
        .map_err(|e| SolverError::ExactReplay(e.to_string()))
}
