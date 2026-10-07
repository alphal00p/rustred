//! Share the expensive campaign case queue while keeping every caller's
//! physical coordinates, prepared basis ordinals and persistence unchanged.
use symbolica::domains::integer::Z;
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::poly::PolyVariable;

use super::*;
use crate::algebra::{Coefficient, CoefficientPolynomial};

pub(super) fn integral<const A: usize, const B: usize>(
    value: &Integral<A>,
) -> Result<Integral<B>, SolverError> {
    if value
        .powers()
        .iter()
        .skip(B)
        .any(|p| p.is_symbolic() || p.value() != 0)
    {
        return Err(SolverError::InvalidInput(
            "solver result depends on a padding coordinate".into(),
        ));
    }
    Ok(Integral::new(std::array::from_fn(|i| {
        value
            .powers()
            .get(i)
            .copied()
            .unwrap_or_else(|| Power::new(false, 0).unwrap())
    })))
}

struct Mapping<'a, const A: usize, const B: usize> {
    variables: &'a [PolyVariable],
    indices: &'a [usize; B],
    sector: &'a [bool; B],
}

impl<const A: usize, const B: usize> Mapping<'_, A, B> {
    fn polynomial(
        &self,
        value: &CoefficientPolynomial,
    ) -> Result<CoefficientPolynomial, SolverError> {
        value
            .rearrange_with_growth(self.variables)
            .map_err(SolverError::InvalidInput)
    }
    fn coefficient(&self, value: &Coefficient) -> Result<Coefficient, SolverError> {
        Ok(Coefficient::from_num_den(
            self.polynomial(&value.numerator)?,
            self.polynomial(&value.denominator)?,
            &Z,
            true,
        ))
    }
    fn case(&self, value: &Case<A>) -> Result<Case<B>, SolverError> {
        if value.fixed().iter().skip(B).any(|v| *v != Some(0)) {
            return Err(SolverError::InvalidInput(
                "solver case has an open padding coordinate".into(),
            ));
        }
        let face = CoordinateCase::new(std::array::from_fn(|i| {
            value.fixed().get(i).copied().unwrap_or(Some(0))
        }))?;
        let Some(affine) = value.affine() else {
            return Ok(face.into());
        };
        let equations = affine
            .equations()
            .iter()
            .map(|p| self.polynomial(p))
            .collect::<Result<Vec<_>, _>>()?;
        match AffineCase::from_coordinate(&face, &equations, self.indices, self.sector)
            .map_err(|e| SolverError::InvalidInput(e.to_string()))?
        {
            AffineIntersection::Affine(case) => Ok(case.into()),
            AffineIntersection::Coordinate(case) => Ok(case.into()),
            AffineIntersection::Empty => Err(SolverError::InvalidInput(
                "capacity conversion changed a case into an empty domain".into(),
            )),
        }
    }
    fn rule(&self, value: &SectorRule<A>) -> Result<SectorRule<B>, SolverError> {
        let source = &value.candidate;
        Ok(SectorRule {
            dispatch_policy: value.dispatch_policy,
            candidate: RuleCandidate {
                case: self.case(&source.case)?,
                target: integral(&source.target)?,
                rhs: source
                    .rhs
                    .iter()
                    .map(|t| {
                        Ok(Term {
                            integral: integral(&t.integral)?,
                            coefficient: self.coefficient(&t.coefficient)?,
                        })
                    })
                    .collect::<Result<_, SolverError>>()?,
                sources: source
                    .sources
                    .iter()
                    .map(|s| {
                        if s.seed.shifts.iter().skip(B).any(|&v| v != 0) {
                            return Err(SolverError::InvalidInput(
                                "solver seed shifts a padding coordinate".into(),
                            ));
                        }
                        Ok(SeedSource {
                            basis_row: s.basis_row,
                            seed: Seed {
                                integral: integral(&s.seed.integral)?,
                                shifts: std::array::from_fn(|i| {
                                    s.seed.shifts.get(i).copied().unwrap_or(0)
                                }),
                            },
                        })
                    })
                    .collect::<Result<_, _>>()?,
                stats: source.stats,
            },
            exceptions: ExceptionalConditions {
                branches: value
                    .exceptions
                    .branches
                    .iter()
                    .map(|branch| branch.iter().map(|p| self.polynomial(p)).collect())
                    .collect::<Result<_, _>>()?,
            },
        })
    }
    fn solution(&self, value: SectorSolution<A>) -> Result<SectorSolution<B>, SolverError> {
        Ok(SectorSolution {
            order: value.order.resize()?,
            max_numerator_rank: value.max_numerator_rank,
            finite_case_policy: value.finite_case_policy,
            rules: value
                .rules
                .iter()
                .map(|r| self.rule(r))
                .collect::<Result<_, _>>()?,
            finite_residuals: value
                .finite_residuals
                .iter()
                .map(integral)
                .collect::<Result<_, _>>()?,
            stats: value.stats,
        })
    }
    fn geometry(&self, error: AffineGeometryError) -> Result<AffineGeometryError, SolverError> {
        Ok(match error {
            AffineGeometryError::UnsupportedNonlinear { equations } => {
                AffineGeometryError::UnsupportedNonlinear {
                    equations: equations
                        .iter()
                        .map(|p| self.polynomial(p))
                        .collect::<Result<_, _>>()?,
                }
            }
            AffineGeometryError::Coordinate(GeometryError::UnsupportedGeometry { equations }) => {
                AffineGeometryError::Coordinate(GeometryError::UnsupportedGeometry {
                    equations: equations
                        .iter()
                        .map(|p| self.polynomial(p))
                        .collect::<Result<_, _>>()?,
                })
            }
            other => other,
        })
    }
    fn error(&self, error: SectorSolveError<A>) -> Result<SectorSolveError<B>, SolverError> {
        Ok(match error {
            SectorSolveError::Search { case, source } => SectorSolveError::Search {
                case: self.case(&case)?,
                source,
            },
            SectorSolveError::Geometry { case, source } => SectorSolveError::Geometry {
                case: self.case(&case)?,
                source: self.geometry(source)?,
            },
            SectorSolveError::Exceptions { case, source } => SectorSolveError::Exceptions {
                case: self.case(&case)?,
                source,
            },
            SectorSolveError::NonProgress { case } => SectorSolveError::NonProgress {
                case: self.case(&case)?,
            },
            SectorSolveError::CaseBudget { solved, pending } => {
                SectorSolveError::CaseBudget { solved, pending }
            }
            SectorSolveError::Numeric(error) => SectorSolveError::Numeric(error),
            SectorSolveError::FiniteRetention {
                case: _,
                source: FiniteRetentionError::MissingNumeratorRank,
            } => SectorSolveError::FiniteRetention {
                case: Case::generic(),
                source: FiniteRetentionError::MissingNumeratorRank,
            },
            SectorSolveError::FiniteRetention { case, source } => {
                SectorSolveError::FiniteRetention {
                    case: self.case(&case)?,
                    source: match source {
                        FiniteRetentionError::Geometry(error) => {
                            FiniteRetentionError::Geometry(self.geometry(error)?)
                        }
                        other => other,
                    },
                }
            }
            SectorSolveError::Intersection(error) => {
                let CaseIntersectionError {
                    max_numerator_rank,
                    limits,
                    original_parent,
                    original_conjunction,
                    unresolved_parent,
                    unresolved_conjunction,
                    failure,
                    stats,
                } = *error;
                SectorSolveError::Intersection(Box::new(CaseIntersectionError {
                    max_numerator_rank,
                    limits,
                    original_parent: self.case(&original_parent)?,
                    original_conjunction: original_conjunction
                        .iter()
                        .map(|p| self.polynomial(p))
                        .collect::<Result<Vec<_>, _>>()?
                        .into(),
                    unresolved_parent: self.case(&unresolved_parent)?,
                    unresolved_conjunction: unresolved_conjunction
                        .iter()
                        .map(|p| self.polynomial(p))
                        .collect::<Result<Vec<_>, _>>()?
                        .into(),
                    failure: match failure {
                        CaseIntersectionFailure::Admission(e) => {
                            CaseIntersectionFailure::Admission(self.geometry(e)?)
                        }
                        other => other,
                    },
                    stats,
                }))
            }
        })
    }
    fn search_event(&self, event: SearchEvent<A>) -> Result<SearchEvent<B>, SolverError> {
        Ok(match event {
            SearchEvent::DiscoveryProgress {
                depth,
                seeds,
                rows,
                discovery,
            } => SearchEvent::DiscoveryProgress {
                depth,
                seeds,
                rows,
                discovery,
            },
            SearchEvent::ExactStarted {
                pivot,
                trace_rows,
                discovery,
            } => SearchEvent::ExactStarted {
                pivot: integral(&pivot)?,
                trace_rows,
                discovery,
            },
            SearchEvent::CanonicalizationStarted { terms, direct_hit } => {
                SearchEvent::CanonicalizationStarted { terms, direct_hit }
            }
            SearchEvent::ExactProgress(event) => SearchEvent::ExactProgress(match event {
                MaterializationEvent::FramePrepared {
                    source_rows,
                    integral_columns,
                    target_column,
                    input_terms,
                    coefficient_variables,
                    active_variables,
                } => MaterializationEvent::FramePrepared {
                    source_rows,
                    integral_columns,
                    target_column,
                    input_terms,
                    coefficient_variables,
                    active_variables,
                },
                MaterializationEvent::DenseFractionFreeStarted {
                    rows,
                    columns,
                    reduction_columns,
                    rational_coefficients,
                } => MaterializationEvent::DenseFractionFreeStarted {
                    rows,
                    columns,
                    reduction_columns,
                    rational_coefficients,
                },
                MaterializationEvent::DenseFractionFreeFinished { rank } => {
                    MaterializationEvent::DenseFractionFreeFinished { rank }
                }
                MaterializationEvent::TargetBlockStarted { columns } => {
                    MaterializationEvent::TargetBlockStarted { columns }
                }
                MaterializationEvent::TargetWeightsStarted {
                    rows,
                    lower_nonzeros,
                } => MaterializationEvent::TargetWeightsStarted {
                    rows,
                    lower_nonzeros,
                },
                MaterializationEvent::TargetWeightsFinished { nonzero_weights } => {
                    MaterializationEvent::TargetWeightsFinished { nonzero_weights }
                }
                MaterializationEvent::TargetReconstructionStarted { rows, columns } => {
                    MaterializationEvent::TargetReconstructionStarted { rows, columns }
                }
                MaterializationEvent::TargetReconstructionFinished { output_terms } => {
                    MaterializationEvent::TargetReconstructionFinished { output_terms }
                }
                MaterializationEvent::SemiNumericalStarted {
                    rows,
                    columns,
                    variables,
                } => MaterializationEvent::SemiNumericalStarted {
                    rows,
                    columns,
                    variables,
                },
                MaterializationEvent::SemiNumericalCoefficient {
                    column,
                    probes,
                    primes,
                } => MaterializationEvent::SemiNumericalCoefficient {
                    column,
                    probes,
                    primes,
                },
                MaterializationEvent::SemiNumericalExactReplayStarted { support_recovery } => {
                    MaterializationEvent::SemiNumericalExactReplayStarted { support_recovery }
                }
                MaterializationEvent::SemiNumericalExactReplayFinished { output_terms } => {
                    MaterializationEvent::SemiNumericalExactReplayFinished { output_terms }
                }
                MaterializationEvent::SemiNumericalFinished { output_terms } => {
                    MaterializationEvent::SemiNumericalFinished { output_terms }
                }
                MaterializationEvent::RowStarted {
                    row,
                    input_nonzeros,
                    reducer_rows,
                    reducer_nonzeros,
                } => MaterializationEvent::RowStarted {
                    row,
                    input_nonzeros,
                    reducer_rows,
                    reducer_nonzeros,
                },
                MaterializationEvent::RowFinished {
                    row,
                    pivot,
                    reducer_rows,
                    reducer_nonzeros,
                } => MaterializationEvent::RowFinished {
                    row,
                    pivot: pivot.map(|p| integral(&p)).transpose()?,
                    reducer_rows,
                    reducer_nonzeros,
                },
            }),
        })
    }
    fn event(
        &self,
        event: SectorEvent<'_, A>,
        observe: &mut dyn FnMut(SectorEvent<'_, B>),
    ) -> Result<(), SolverError> {
        match event {
            SectorEvent::CaseStarted { case, pending } => observe(SectorEvent::CaseStarted {
                case: self.case(&case)?,
                pending,
            }),
            SectorEvent::RuleTrialFinished { case, summary } => {
                observe(SectorEvent::RuleTrialFinished {
                    case: &self.case(case)?,
                    summary,
                })
            }
            SectorEvent::Search { case, event } => observe(SectorEvent::Search {
                case: &self.case(case)?,
                event: self.search_event(event)?,
            }),
            SectorEvent::PhaseStarted { case, phase } => observe(SectorEvent::PhaseStarted {
                case: &self.case(case)?,
                phase,
            }),
            SectorEvent::RuleFound { rule, pending } => observe(SectorEvent::RuleFound {
                rule: &self.rule(rule)?,
                pending,
            }),
            SectorEvent::NumericalStarted { cases } => {
                let cases = cases
                    .iter()
                    .map(|c| {
                        self.case(&Case::from(*c))
                            .map(|c| *c.coordinate().expect("coordinate conversion"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                observe(SectorEvent::NumericalStarted { cases: &cases });
            }
        }
        Ok(())
    }
}

/// Non-generic observer boundary: caller conversions never specialize the
/// expensive case queue for an additional physical arity or callback type.
fn run<const K: usize>(
    solver: &SectorSolver<'_, K>,
    cases: Vec<Case<K>>,
    options: SectorSolveOptions,
    observe: &mut dyn FnMut(SectorEvent<'_, K>),
) -> Result<SectorSolution<K>, SectorSolveError<K>> {
    solver.solve_case_queue_inline(cases, options, observe)
}

fn resized<const N: usize, const K: usize>(
    solver: &SectorSolver<'_, N>,
    initial: Vec<Case<N>>,
    options: SectorSolveOptions,
    observe: &mut dyn FnMut(SectorEvent<'_, N>),
) -> Result<SectorSolution<N>, SectorSolveError<N>> {
    let result = (|| {
        // Matching physical/storage widths need no copies or chart rebuilds.
        if N == K {
            return Ok(run(solver, initial, options, observe));
        }
        let system = solver.system.resize::<K>()?;
        let order = solver.order.resize::<K>()?;
        let to = Mapping::<N, K> {
            variables: system.coefficient_variables(),
            indices: system.index_variables(),
            sector: order.sector(),
        };
        let initial = initial
            .iter()
            .map(|case| to.case(case))
            .collect::<Result<_, _>>()?;
        let basis = solver
            .basis
            .iter()
            .map(|row| {
                row.iter()
                    .map(|t| {
                        Ok(Term {
                            integral: integral(&t.integral)?,
                            coefficient: to.polynomial(&t.coefficient)?,
                        })
                    })
                    .collect()
            })
            .collect::<Result<_, SolverError>>()?;
        let config = SectorConfig {
            deltas: *order.deltas(),
            removed_deltas: std::array::from_fn(|i| {
                solver
                    .config
                    .removed_deltas
                    .get(i)
                    .copied()
                    .unwrap_or(false)
            }),
            permutation: order.permutation().copied(),
            integral_order: order.program().cloned(),
            zero_sectors: solver
                .config
                .zero_sectors
                .iter()
                .map(|mask| std::array::from_fn(|i| mask.get(i).copied().unwrap_or(false)))
                .collect::<Vec<_>>()
                .into(),
            symbolic_exact_backend: solver.config.symbolic_exact_backend,
            numerical_exact_backend: solver.config.numerical_exact_backend,
            coefficient_variable_order: solver.config.coefficient_variable_order,
            source_discovery: solver.config.source_discovery.clone(),
            rule_selection: solver.config.rule_selection.clone(),
        };
        // Already materialized against the unchanged basis; do not rebuild or
        // interpret physical-axis priorities as storage-axis priorities.
        let resized = SectorSolver {
            system: &system,
            basis,
            order,
            config,
        };
        let from = Mapping::<K, N> {
            variables: solver.system.coefficient_variables(),
            indices: solver.system.index_variables(),
            sector: solver.order.sector(),
        };
        let mut event_error = None;
        let result = run(&resized, initial, options, &mut |event| {
            if event_error.is_none() {
                event_error = from.event(event, observe).err();
            }
        });
        if let Some(error) = event_error {
            return Err(error);
        }
        Ok(match result {
            Ok(solution) => Ok(from.solution(solution)?),
            Err(error) => Err(from.error(error)?),
        })
    })();
    result.unwrap_or_else(|error| Err(SectorSolveError::Numeric(error)))
}

pub(super) fn solve_queue<const N: usize>(
    solver: &SectorSolver<'_, N>,
    initial: Vec<Case<N>>,
    options: SectorSolveOptions,
    observe: &mut dyn FnMut(SectorEvent<'_, N>),
) -> Result<SectorSolution<N>, SectorSolveError<N>> {
    match N {
        1..=4 => resized::<N, 4>(solver, initial, options, observe),
        5..=8 => resized::<N, 8>(solver, initial, options, observe),
        9..=16 => resized::<N, 16>(solver, initial, options, observe),
        // Explicit const-generic callers outside the application registry keep
        // their original exact-width API, independently of build-time filters.
        _ => solver.solve_case_queue_inline(initial, options, observe),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;

    fn compare<const N: usize>(program: bool) {
        let names = std::iter::once("d".to_owned())
            .chain((0..N).map(|i| format!("a{i}")))
            .collect::<Vec<_>>();
        let context = CoefficientContext::try_new(names.iter().map(String::as_str)).unwrap();
        let a = context.parameter("a0").unwrap();
        let d = context.parameter("d").unwrap();
        let row = vec![
            Term {
                integral: Integral::symbolic([0; N]).unwrap(),
                coefficient: (&d - &(&context.integer(2) * &a)).numerator,
            },
            Term {
                integral: Integral::symbolic(std::array::from_fn(|i| if i == 0 { 1 } else { 0 }))
                    .unwrap(),
                coefficient: (-(&context.integer(2) * &a)).numerator,
            },
        ];
        let system = SourceSystem::new(vec![row], std::array::from_fn(|i| i + 1)).unwrap();
        let sector = std::array::from_fn(|i| i == 0);
        let order = if program {
            Some(
                rustred_order::CompiledOrder::compile(
                    rustred_order::OrderDescriptor {
                        pre_support_degree_rows: vec![],
                        support_weights: vec![1; N],
                        support_priority: (0..N).collect(),
                        degree_rows: vec![rustred_order::DegreeRow {
                            active: vec![1; N],
                            inactive: vec![1; N],
                        }],
                        coordinate_priority: (0..N).rev().collect(),
                        coordinate_groups: rustred_order::CoordinateGroups::ActiveFirst,
                        active_direction: rustred_order::Direction::Ascending,
                        inactive_direction: rustred_order::Direction::Descending,
                    },
                    Default::default(),
                )
                .unwrap(),
            )
        } else {
            None
        };
        let solver = SectorSolver::new(
            &system,
            sector,
            SectorConfig {
                integral_order: order,
                ..Default::default()
            },
        )
        .unwrap();
        let case = Case::from(
            CoordinateCase::new(std::array::from_fn(|i| if i == 0 { None } else { Some(0) }))
                .unwrap(),
        );
        let options = SectorSolveOptions {
            symbolic: SearchOptions {
                max_depth: Some(2),
                ..Default::default()
            },
            ..Default::default()
        };
        let exact = solver
            .solve_case_queue_inline(vec![case.clone()], options, &mut |_| {})
            .unwrap();
        let shared = solver
            .solve_domains_with_observer(vec![case], options, |event| match event {
                SectorEvent::CaseStarted { case, .. } => assert_eq!(case.fixed().len(), N),
                SectorEvent::RuleFound { rule, .. } => {
                    assert_eq!(rule.candidate.target.powers().len(), N);
                    for term in &rule.candidate.rhs {
                        assert_eq!(
                            term.coefficient.get_variables().as_slice(),
                            system.coefficient_variables()
                        );
                    }
                }
                _ => {}
            })
            .unwrap();
        assert_eq!(shared.order, exact.order);
        assert_eq!(shared.finite_residuals, exact.finite_residuals);
        assert_eq!(shared.rules.len(), exact.rules.len());
        assert!(!shared.rules.is_empty());
        for (shared, exact) in shared.rules.iter().zip(&exact.rules) {
            assert_eq!(shared.candidate.case, exact.candidate.case);
            assert_eq!(shared.candidate.target, exact.candidate.target);
            assert_eq!(shared.candidate.rhs, exact.candidate.rhs);
            assert_eq!(shared.candidate.sources, exact.candidate.sources);
            assert_eq!(shared.exceptions, exact.exceptions);
        }
    }

    #[test]
    fn shared_campaign_queue_preserves_rules_sources_guards_and_physical_widths() {
        compare::<1>(false);
        compare::<6>(false);
        compare::<13>(false);
    }

    #[test]
    fn shared_queue_preserves_physical_programmable_order() {
        compare::<3>(true);
    }

    #[test]
    fn shrinking_storage_rejects_nonzero_or_symbolic_padding() {
        assert!(integral::<2, 1>(&Integral::numeric([1, 2]).unwrap()).is_err());
        assert!(integral::<2, 1>(&Integral::symbolic([1, 0]).unwrap()).is_err());
    }
}
