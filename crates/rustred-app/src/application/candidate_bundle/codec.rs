use std::collections::BTreeSet;
use std::sync::Arc;

use rustred::algebra::{Coefficient, CoefficientPolynomial, IndexedCoefficientContext};
use rustred::persistence::{
    BinaryIoError, BinaryProgramKind, BinarySection, CoefficientId, CoefficientTableBuilder,
    DecodedCoefficientTable, EncodedCoefficientTable, NativeFamilyRecord, ProgramEnvelope,
    SectionTag, encode_program, inspect_program,
};
use rustred::solver::{
    AffineCase, AffineIntersection, Case, CoordinateCase, ExceptionalConditions, Integral, Power,
    RuleCandidate, SearchStats, SectorRule, SectorSolution, SectorStats, Seed, SeedSource, Term,
};
use symbolica::prelude::PolyVariable;

use crate::application::{AppError, MAX_INPUT_BYTES};

use super::model::*;

pub(super) fn read(bytes: &[u8], limits: CandidateBundleLimits) -> Result<Bundle, AppError> {
    read_with_budget(bytes, limits, None)
}

/// Assembly admits the cumulative structural count before importing a shard.
pub(super) fn read_with_budget(
    bytes: &[u8],
    limits: CandidateBundleLimits,
    budget: Option<&mut CollectionBudget>,
) -> Result<Bundle, AppError> {
    let (envelope, records, family) = read_structure(bytes, limits)?;
    if let Some(budget) = budget {
        for sector in &records.sectors {
            budget.admit_sector(sector)?;
        }
    }
    let coefficients = DecodedCoefficientTable::import_generated(
        envelope
            .section(SectionTag::SYMBOLICA_STATE)
            .expect("checked section"),
        envelope
            .section(SectionTag::COEFFICIENTS)
            .expect("checked section"),
        limits.binary_limits(),
    )
    .map_err(binary_error)?;
    validate_ids(&records, coefficients.len())?;
    Ok(Bundle {
        records,
        family,
        coefficients: Arc::new(coefficients),
    })
}

pub(super) fn read_structure(
    bytes: &[u8],
    limits: CandidateBundleLimits,
) -> Result<(ProgramEnvelope<'_>, ProgramRecord, NativeFamilyRecord), AppError> {
    if bytes.len() > limits.bundle_byte_limit() {
        return Err(AppError::limit("candidate bundle exceeds its byte limit"));
    }
    let envelope = inspect_program(bytes, limits.binary_limits()).map_err(binary_error)?;
    if envelope.kind() != BinaryProgramKind::Candidates
        || envelope
            .sections()
            .iter()
            .map(|s| s.tag)
            .collect::<Vec<_>>()
            != [
                SectionTag::SYMBOLICA_STATE,
                SectionTag::COEFFICIENTS,
                SectionTag::FAMILY,
                SectionTag::PROGRAM,
            ]
    {
        return Err(AppError::schema(
            "expected a generated candidate binary program",
        ));
    }
    let program = envelope
        .section(SectionTag::PROGRAM)
        .expect("checked section");
    let (records, consumed): (ProgramRecord, usize) = bincode::decode_from_slice(
        program,
        bincode::config::standard().with_limit::<MAX_CANDIDATE_BUNDLE_BYTES>(),
    )
    .map_err(|e| AppError::schema(format!("invalid candidate structural record: {e}")))?;
    if consumed != program.len() {
        return Err(AppError::schema("trailing candidate structural bytes"));
    }
    // Shape/collection checks precede native Symbolica state import. The native
    // state and algebra records still require trusted generated provenance.
    validate(&records, limits)?;
    let encoded_family = envelope
        .section(SectionTag::FAMILY)
        .expect("checked section");
    let (family, consumed): (NativeFamilyRecord, usize) = bincode::decode_from_slice(
        encoded_family,
        bincode::config::standard().with_limit::<MAX_CANDIDATE_BUNDLE_BYTES>(),
    )
    .map_err(|e| AppError::schema(format!("invalid native family record: {e}")))?;
    if consumed != encoded_family.len() {
        return Err(AppError::schema("trailing native family record bytes"));
    }
    family
        .validate_shape(limits.family_limits(), limits.binary_limits())
        .map_err(binary_error)?;
    if family.arity() != records.root_sector.len() {
        return Err(AppError::input(
            "native family and candidate root arities differ",
        ));
    }
    Ok((envelope, records, family))
}

#[cfg(test)]
pub(super) fn write(bundle: &Bundle, limits: CandidateBundleLimits) -> Result<Vec<u8>, AppError> {
    let mut table = CoefficientTableBuilder::new(limits.binary_limits());
    for index in 0..bundle.coefficients.len() {
        let id = CoefficientId::try_from_index(index).map_err(binary_error)?;
        let restored = table
            .intern(bundle.coefficients.coefficient(id).map_err(binary_error)?)
            .map_err(binary_error)?;
        if restored != id {
            return Err(AppError::input(
                "generated coefficient table contains duplicate IDs",
            ));
        }
    }
    write_records(
        &bundle.records,
        &bundle.family,
        &table.finish().map_err(binary_error)?,
        limits,
    )
}

pub(super) fn write_records(
    records: &ProgramRecord,
    family: &NativeFamilyRecord,
    table: &EncodedCoefficientTable,
    limits: CandidateBundleLimits,
) -> Result<Vec<u8>, AppError> {
    validate(records, limits)?;
    let program = bincode::encode_to_vec(records, bincode::config::standard())
        .map_err(|e| AppError::serialization(e.to_string()))?;
    family
        .validate_shape(limits.family_limits(), limits.binary_limits())
        .map_err(binary_error)?;
    let family = bincode::encode_to_vec(family, bincode::config::standard())
        .map_err(|e| AppError::serialization(e.to_string()))?;
    encode_program(
        BinaryProgramKind::Candidates,
        &[
            BinarySection {
                tag: SectionTag::SYMBOLICA_STATE,
                bytes: &table.state,
            },
            BinarySection {
                tag: SectionTag::COEFFICIENTS,
                bytes: &table.atoms,
            },
            BinarySection {
                tag: SectionTag::FAMILY,
                bytes: &family,
            },
            BinarySection {
                tag: SectionTag::PROGRAM,
                bytes: &program,
            },
        ],
        limits.binary_limits(),
    )
    .map_err(binary_error)
}

pub(super) fn binary_error(error: BinaryIoError) -> AppError {
    match error {
        BinaryIoError::Limit { .. } | BinaryIoError::Allocation { .. } => {
            AppError::limit(error.to_string())
        }
        _ => AppError::schema(error.to_string()),
    }
}

fn validate_ids(records: &ProgramRecord, count: usize) -> Result<(), AppError> {
    for sector in &records.sectors {
        for rule in &sector.rules {
            if rule
                .case
                .equations
                .iter()
                .chain(rule.rhs.iter().map(|term| &term.coefficient))
                .chain(rule.exclusions.iter().flatten())
                .any(|&id| id as usize >= count)
            {
                return Err(AppError::input(
                    "candidate coefficient ID is outside its table",
                ));
            }
        }
    }
    Ok(())
}

fn validate(bundle: &ProgramRecord, limits: CandidateBundleLimits) -> Result<(), AppError> {
    if bundle.schema != CANDIDATE_BUNDLE_SCHEMA || bundle.status != STATUS {
        return Err(AppError::schema(
            "unsupported candidate bundle schema/status/solver policy",
        ));
    }
    super::policy::numerical_depth(&bundle.solver_policy)?;
    if bundle.family_source.len() > MAX_INPUT_BYTES {
        return Err(AppError::limit(
            "candidate family input exceeds its byte limit",
        ));
    }
    let n = bundle.root_sector.len();
    if !(1..=16).contains(&n) {
        return Err(AppError::input("candidate root arity must be 1 through 16"));
    }
    super::preparation::validate_permutation(n, bundle.permutation.as_deref())?;
    let mut budget = CollectionBudget::new(limits, bundle.sectors.len())?;
    let mut seen = BTreeSet::new();
    for sector in &bundle.sectors {
        if sector.sector.len() != n
            || !seen.insert(&sector.sector)
            || sector
                .sector
                .iter()
                .zip(&bundle.root_sector)
                .any(|(&active, &root)| active && !root)
        {
            return Err(AppError::input(
                "candidate sector is duplicate, wrong-arity, or outside root",
            ));
        }
        budget.admit_sector(sector)?;
        for integral in &sector.finite_residuals {
            validate_integral(integral, n)?;
            if integral.symbolic.iter().any(|&value| value)
                || integral
                    .values
                    .iter()
                    .zip(&sector.sector)
                    .any(|(&v, &active)| (v > 0) != active)
            {
                return Err(AppError::input(
                    "candidate residual is not a concrete key in its sector",
                ));
            }
        }
        for rule in &sector.rules {
            validate_integral(&rule.target, n)?;
            let case = &rule.case;
            if case.fixed_axes.len() != case.fixed_values.len()
                || case.fixed_axes.iter().any(|&axis| axis >= n)
                || case.fixed_axes.windows(2).any(|axes| axes[0] >= axes[1])
                || !matches!(case.kind.as_str(), "coordinate" | "affine")
                || (case.kind == "coordinate") != case.equations.is_empty()
            {
                return Err(AppError::input("invalid candidate case shape"));
            }
            for term in &rule.rhs {
                validate_integral(&term.integral, n)?;
                if term.integral.symbolic != rule.target.symbolic {
                    return Err(AppError::input(
                        "candidate RHS symbolic layout differs from target",
                    ));
                }
            }
            for source in &rule.sources {
                validate_integral(&source.integral, n)?;
                if source.shifts.len() != n {
                    return Err(AppError::input(
                        "candidate seed shift arity differs from root",
                    ));
                }
            }
        }
    }
    Ok(())
}

/// The unchanged candidate structural-entry policy, reusable incrementally
/// while assembling checkpoint shards. Charge the final sector count once, then
/// each sector before importing/materializing its native coefficients. This is
/// admission only; record shape checks remain in `validate`.
#[derive(Debug)]
pub(super) struct CollectionBudget {
    entries: usize,
    limits: CandidateBundleLimits,
}

impl CollectionBudget {
    pub(super) fn new(
        limits: CandidateBundleLimits,
        total_sector_count: usize,
    ) -> Result<Self, AppError> {
        let mut budget = Self { entries: 0, limits };
        budget.entries(total_sector_count)?;
        Ok(budget)
    }

    pub(super) fn admit_sector(&mut self, sector: &SectorRecord) -> Result<(), AppError> {
        self.entries(sector.rules.len())?;
        self.entries(sector.finite_residuals.len())?;
        for rule in &sector.rules {
            self.entries(rule.case.equations.len())?;
            self.entries(rule.rhs.len())?;
            self.entries(rule.sources.len())?;
            self.entries(rule.exclusions.len())?;
            for branch in &rule.exclusions {
                self.entries(branch.len())?;
            }
        }
        Ok(())
    }

    fn entries(&mut self, count: usize) -> Result<(), AppError> {
        self.entries = self
            .entries
            .checked_add(count)
            .ok_or_else(|| AppError::limit("candidate collection count overflow"))?;
        if self.entries > self.limits.max_collection_entries {
            return Err(AppError::limit(
                "candidate aggregate collection-entry budget exceeded",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod collection_budget_tests {
    use super::*;
    use crate::AppErrorKind;

    fn sector(active: bool) -> SectorRecord {
        let integral = IntegralRecord {
            symbolic: vec![false],
            values: vec![i16::from(active)],
        };
        SectorRecord {
            sector: vec![active],
            finite_residuals: vec![integral.clone(); 2],
            rules: vec![RuleRecord {
                case: CaseRecord {
                    kind: "affine".into(),
                    fixed_axes: vec![0],
                    fixed_values: integral.values.clone(),
                    equations: vec![0, 1],
                },
                target: integral.clone(),
                rhs: vec![
                    TermRecord {
                        integral: integral.clone(),
                        coefficient: 0
                    };
                    3
                ],
                sources: vec![
                    SeedRecord {
                        basis_row: 0,
                        integral,
                        shifts: vec![0]
                    };
                    2
                ],
                exclusions: vec![vec![0, 1], vec![2]],
            }],
        }
    }

    #[test]
    fn individually_admissible_shards_cannot_bypass_final_aggregate_entry_limit() {
        let sectors = vec![sector(false), sector(true)];
        // Each sector costs 15: rule(1), residuals(2), equations(2), RHS(3),
        // sources(2), branches(2), branch entries(3). Final count adds 2.
        let mut limits = CandidateBundleLimits::default();
        limits.max_collection_entries = 31;
        for sector in &sectors {
            let mut individual = CollectionBudget::new(limits, 1).unwrap();
            individual.admit_sector(sector).unwrap();
            assert_eq!(individual.entries, 16);
        }
        let mut aggregate = CollectionBudget::new(limits, 2).unwrap();
        aggregate.admit_sector(&sectors[0]).unwrap();
        assert_eq!(
            aggregate.admit_sector(&sectors[1]).unwrap_err().kind(),
            AppErrorKind::Limit
        );
        let bundle = ProgramRecord {
            schema: CANDIDATE_BUNDLE_SCHEMA.into(),
            status: STATUS.into(),
            solver_policy: SOLVER_POLICY.into(),
            family_source: String::new(),
            input_format: "toml".into(),
            family_fingerprint: "structural-budget-fixture".into(),
            root_sector: vec![true],
            permutation: None,
            sectors,
        };
        assert_eq!(
            validate(&bundle, limits).unwrap_err().kind(),
            AppErrorKind::Limit
        );
        limits.max_collection_entries = 32;
        let mut exact = CollectionBudget::new(limits, 2).unwrap();
        for sector in &bundle.sectors {
            exact.admit_sector(sector).unwrap();
        }
        assert_eq!(exact.entries, 32);
        validate(&bundle, limits).unwrap();
        limits.max_collection_entries = 1;
        assert_eq!(
            CollectionBudget::new(limits, 2).unwrap_err().kind(),
            AppErrorKind::Limit
        );
    }
}

fn validate_integral(value: &IntegralRecord, n: usize) -> Result<(), AppError> {
    if value.symbolic.len() != n || value.values.len() != n {
        return Err(AppError::input("candidate integral has the wrong arity"));
    }
    for (&symbolic, &value) in value.symbolic.iter().zip(&value.values) {
        Power::new(symbolic, value).map_err(|error| AppError::input(error.to_string()))?;
    }
    Ok(())
}

fn integral_record<const N: usize>(value: &Integral<N>) -> IntegralRecord {
    IntegralRecord {
        symbolic: value.powers().iter().map(|p| p.is_symbolic()).collect(),
        values: value.powers().iter().map(|p| p.value()).collect(),
    }
}

fn integral<const N: usize>(value: &IntegralRecord) -> Result<Integral<N>, AppError> {
    validate_integral(value, N)?;
    let mut powers = [Power::default(); N];
    for (axis, power) in powers.iter_mut().enumerate() {
        *power = Power::new(value.symbolic[axis], value.values[axis])
            .map_err(|error| AppError::input(error.to_string()))?;
    }
    Ok(Integral::new(powers))
}

pub(super) fn sector_record<const N: usize>(
    sector: [bool; N],
    solution: &SectorSolution<N>,
    table: &mut CoefficientTableBuilder,
) -> Result<SectorRecord, AppError> {
    Ok(SectorRecord {
        sector: sector.to_vec(),
        finite_residuals: solution
            .finite_residuals
            .iter()
            .map(integral_record)
            .collect(),
        rules: solution
            .rules
            .iter()
            .map(|rule| {
                let candidate = &rule.candidate;
                let fixed: Vec<_> = candidate
                    .case
                    .fixed()
                    .iter()
                    .enumerate()
                    .filter_map(|(axis, value)| value.map(|v| (axis, v)))
                    .collect();
                Ok(RuleRecord {
                    case: CaseRecord {
                        kind: if candidate.case.affine().is_some() {
                            "affine"
                        } else {
                            "coordinate"
                        }
                        .into(),
                        fixed_axes: fixed.iter().map(|(axis, _)| *axis).collect(),
                        fixed_values: fixed.iter().map(|(_, value)| *value).collect(),
                        equations: candidate
                            .case
                            .affine()
                            .map(|case| {
                                case.equations()
                                    .iter()
                                    .map(|p| intern_polynomial(table, p))
                                    .collect::<Result<Vec<_>, _>>()
                            })
                            .transpose()?
                            .unwrap_or_default(),
                    },
                    target: integral_record(&candidate.target),
                    rhs: candidate
                        .rhs
                        .iter()
                        .map(|term| {
                            Ok(TermRecord {
                                integral: integral_record(&term.integral),
                                coefficient: intern_coefficient(table, &term.coefficient)?,
                            })
                        })
                        .collect::<Result<Vec<_>, AppError>>()?,
                    sources: candidate
                        .sources
                        .iter()
                        .map(|source| SeedRecord {
                            basis_row: source.basis_row,
                            integral: integral_record(&source.seed.integral),
                            shifts: source.seed.shifts.to_vec(),
                        })
                        .collect(),
                    exclusions: rule
                        .exceptions
                        .branches
                        .iter()
                        .map(|branch| {
                            branch
                                .iter()
                                .map(|p| intern_polynomial(table, p))
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .collect::<Result<Vec<_>, AppError>>()?,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?,
    })
}

fn intern_coefficient(
    table: &mut CoefficientTableBuilder,
    value: &Coefficient,
) -> Result<u32, AppError> {
    Ok(table.intern(value).map_err(binary_error)?.index() as u32)
}

fn intern_polynomial(
    table: &mut CoefficientTableBuilder,
    value: &CoefficientPolynomial,
) -> Result<u32, AppError> {
    intern_coefficient(table, &Coefficient::from(value.clone()))
}

/// Rebuild ordinary transport values only. This performs no source replay and
/// never constructs a ClosedArtifact, replay certificate, or trusted matrix.
pub(super) fn solutions<const N: usize>(
    bundle: &Bundle,
    context: &IndexedCoefficientContext,
    indices: &[usize; N],
    limits: CandidateBundleLimits,
) -> Result<Vec<([bool; N], SectorSolution<N>)>, AppError> {
    validate(bundle, limits)?;
    let max_numerator_rank = super::policy::parse(&bundle.solver_policy)?.max_numerator_rank;
    if bundle.root_sector.len() != N {
        return Err(AppError::input("candidate reconstruction arity"));
    }
    let identity = context.one();
    let variables = identity.raw().get_variables().as_slice();
    bundle.sectors.iter().map(|record| {
        let sector: [bool; N] = record.sector.as_slice().try_into().expect("validated arity");
        let rules = record.rules.iter().map(|record| {
            let mut fixed = [None; N];
            for (&axis, &value) in record.case.fixed_axes.iter().zip(&record.case.fixed_values) {
                fixed[axis] = Some(value);
            }
            let face = CoordinateCase::new(fixed).map_err(|e| AppError::input(e.to_string()))?;
            if !face.is_in_sector(&sector) { return Err(AppError::input("candidate fixed face is outside sector")); }
            let case: Case<N> = if record.case.kind == "coordinate" { face.into() } else {
                let equations = record.case.equations.iter()
                    .map(|&id| polynomial(bundle, variables, id)).collect::<Result<Vec<_>, _>>()?;
                match AffineCase::from_coordinate(&face, &equations, indices, &sector)
                    .map_err(|e| AppError::input(e.to_string()))?
                {
                    AffineIntersection::Affine(case) if case.face().fixed() == &fixed => case.into(),
                    _ => return Err(AppError::input("saved affine case does not reconstruct its declared fixed face")),
                }
            };
            let target = integral(&record.target)?;
            if target != case.integral() { return Err(AppError::input("candidate target is not canonical for its case")); }
            let rhs = record.rhs.iter().map(|term| Ok(Term {
                integral: integral(&term.integral)?, coefficient: coefficient(bundle, variables, term.coefficient)?.clone(),
            })).collect::<Result<Vec<_>, AppError>>()?;
            let sources = record.sources.iter().map(|source| Ok(SeedSource {
                basis_row: source.basis_row,
                seed: Seed {
                    integral: integral(&source.integral)?,
                    shifts: source.shifts.as_slice().try_into().expect("validated seed arity"),
                },
            })).collect::<Result<Vec<_>, AppError>>()?;
            let branches = record.exclusions.iter().map(|branch| branch.iter()
                .map(|&id| polynomial(bundle, variables, id)).collect())
                .collect::<Result<Vec<Vec<_>>, _>>()?;
            Ok(SectorRule {
                candidate: RuleCandidate { case, target, rhs, sources, stats: SearchStats::default() },
                exceptions: ExceptionalConditions { branches },
            })
        }).collect::<Result<Vec<_>, AppError>>()?;
        let finite_residuals = record.finite_residuals.iter().map(integral).collect::<Result<Vec<_>, _>>()?;
        Ok((sector, SectorSolution { rules, finite_residuals, stats: SectorStats::default(), max_numerator_rank }))
    }).collect()
}

fn coefficient<'a>(
    bundle: &'a Bundle,
    variables: &[PolyVariable],
    id: u32,
) -> Result<&'a Coefficient, AppError> {
    let id = CoefficientId::try_from_index(id as usize).map_err(binary_error)?;
    let value = bundle.coefficients.coefficient(id).map_err(binary_error)?;
    // The table validates sparse shape once. Bind each use to the indexed
    // family map without parsing expressions, changing order, or recomputing GCDs.
    if value.get_variables().as_slice() != variables {
        return Err(AppError::input(
            "candidate coefficient has the wrong indexed variable map",
        ));
    }
    Ok(value)
}

fn polynomial(
    bundle: &Bundle,
    variables: &[PolyVariable],
    id: u32,
) -> Result<CoefficientPolynomial, AppError> {
    let value = coefficient(bundle, variables, id)?;
    if !value.denominator.is_one() {
        return Err(AppError::input("candidate equation is not a polynomial"));
    }
    Ok(value.numerator.clone())
}
