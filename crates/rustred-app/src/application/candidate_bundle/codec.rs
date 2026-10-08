use std::collections::BTreeSet;
use std::sync::Arc;

use rustred::algebra::{Coefficient, CoefficientPolynomial, IndexedCoefficientContext};
use rustred::persistence::{
    BinaryIoError, BinaryProgramKind, BinarySection, CoefficientTableBuilder,
    DecodedCoefficientTable, EncodedCoefficientTable, NativeFamilyRecord, ProgramEnvelope,
    SectionTag, encode_program, inspect_program,
};
use rustred::solver::{Integral, Power, SectorSolution, SectorStats};

pub(super) mod dispatch;
pub(super) mod rules;

use crate::application::{AppError, MAX_INPUT_BYTES};

use super::model::*;

#[cfg(test)]
use rustred::persistence::CoefficientId;

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
    let config = bincode::config::standard().with_limit::<MAX_CANDIDATE_BUNDLE_BYTES>();
    let (schema, _): (String, usize) = bincode::decode_from_slice(program, config)
        .map_err(|e| AppError::schema(format!("invalid candidate structural schema: {e}")))?;
    let (records, consumed): (ProgramRecord, usize) = match schema.as_str() {
        DISPATCH_CANDIDATE_BUNDLE_SCHEMA => bincode::decode_from_slice(program, config)
            .map_err(|e| AppError::schema(format!("invalid candidate structural record: {e}")))?,
        CANDIDATE_BUNDLE_SCHEMA => dispatch::decode_v2(program)?,
        LEGACY_CANDIDATE_BUNDLE_SCHEMA => {
            let (old, consumed): (LegacyProgramRecord, usize) =
                bincode::decode_from_slice(program, config).map_err(|e| {
                    AppError::schema(format!("invalid legacy candidate record: {e}"))
                })?;
            let integral_order =
                super::load::candidate_ordering(old.root_sector.len(), old.permutation.as_deref())?
                    .stable_id()
                    .to_string();
            (
                ProgramRecord {
                    schema: old.schema,
                    status: old.status,
                    solver_policy: old.solver_policy,
                    family_source: old.family_source,
                    input_format: old.input_format,
                    family_fingerprint: old.family_fingerprint,
                    root_sector: old.root_sector,
                    permutation: old.permutation,
                    integral_order,
                    sectors: old.sectors,
                    rule_dispatch: Vec::new(),
                },
                consumed,
            )
        }
        _ => return Err(AppError::schema("unsupported candidate structural schema")),
    };
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
    if !matches!(
        records.schema.as_str(),
        CANDIDATE_BUNDLE_SCHEMA | DISPATCH_CANDIDATE_BUNDLE_SCHEMA
    ) {
        return Err(AppError::schema(
            "legacy candidate structural records are read-only; new output requires explicit order metadata",
        ));
    }
    let program = dispatch::encode(records)?;
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

pub(super) fn validate_ids(records: &ProgramRecord, count: usize) -> Result<(), AppError> {
    for sector in &records.sectors {
        for rule in &sector.rules {
            rules::validate_rule_ids(rule, count)?;
        }
    }
    Ok(())
}

fn validate(bundle: &ProgramRecord, limits: CandidateBundleLimits) -> Result<(), AppError> {
    if !matches!(
        bundle.schema.as_str(),
        CANDIDATE_BUNDLE_SCHEMA | LEGACY_CANDIDATE_BUNDLE_SCHEMA | DISPATCH_CANDIDATE_BUNDLE_SCHEMA
    ) || bundle.status != STATUS
    {
        return Err(AppError::schema(
            "unsupported candidate bundle schema/status/solver policy",
        ));
    }
    super::policy::numerical_depth(&bundle.solver_policy)?;
    dispatch::validate(bundle, limits)?;
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
    super::order::saved_policy(bundle)?;
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
            rules::validate_rule(rule, n)?;
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
    pub(super) fn admitted_entries(&self) -> usize {
        self.entries
    }
    pub(super) fn new(
        limits: CandidateBundleLimits,
        total_sector_count: usize,
    ) -> Result<Self, AppError> {
        let mut budget = Self { entries: 0, limits };
        budget.entries(total_sector_count)?;
        Ok(budget)
    }

    pub(super) fn admit_sector(&mut self, sector: &SectorRecord) -> Result<(), AppError> {
        self.admit_rules(&sector.rules, sector.finite_residuals.len())
    }

    pub(super) fn admit_cases(&mut self, cases: &[CaseRecord]) -> Result<(), AppError> {
        self.entries(cases.len())?;
        for case in cases {
            self.entries(case.equations.len())?;
        }
        Ok(())
    }

    pub(super) fn admit_rules(
        &mut self,
        rules: &[RuleRecord],
        residuals: usize,
    ) -> Result<(), AppError> {
        self.entries(rules.len())?;
        self.entries(residuals)?;
        for rule in rules {
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
            return Err(AppError::limit(format!(
                "candidate aggregate collection-entry budget exceeded: {} > {}",
                self.entries, self.limits.max_collection_entries,
            )));
        }
        Ok(())
    }
}

pub(super) fn collection_entries(record: &ProgramRecord) -> Result<usize, AppError> {
    let limits = CandidateBundleLimits {
        max_collection_entries: usize::MAX,
        ..Default::default()
    };
    let mut budget = CollectionBudget::new(limits, record.sectors.len())?;
    for sector in &record.sectors {
        budget.admit_sector(sector)?;
    }
    Ok(budget.admitted_entries())
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
            rule_dispatch: Vec::new(),
            schema: CANDIDATE_BUNDLE_SCHEMA.into(),
            status: STATUS.into(),
            solver_policy: SOLVER_POLICY.into(),
            family_source: String::new(),
            input_format: "toml".into(),
            family_fingerprint: "structural-budget-fixture".into(),
            root_sector: vec![true],
            permutation: None,
            integral_order: rustred::sector::OrderingPolicy::SpiredUncutV1
                .stable_id()
                .to_string(),
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
    validate_integral(value, value.values.len())?;
    if !rustred::fits_storage(value.values.len(), N) {
        return Err(AppError::input(
            "candidate integral exceeds storage capacity",
        ));
    }
    let mut powers = [Power::default(); N];
    for (axis, power) in powers.iter_mut().take(value.values.len()).enumerate() {
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
            .map(|rule| rules::rule_record(rule, table))
            .collect::<Result<Vec<_>, AppError>>()?,
    })
}

pub(super) fn physical_sector_record<const N: usize>(
    arity: usize,
    sector: [bool; N],
    solution: &SectorSolution<N>,
    table: &mut CoefficientTableBuilder,
) -> Result<SectorRecord, AppError> {
    if !rustred::fits_storage(arity, N) || sector[arity..].iter().any(|&v| v) {
        return Err(AppError::input("candidate sector uses a padding axis"));
    }
    let mut record = sector_record(sector, solution, table)?;
    record.sector.truncate(arity);
    let project = |integral: &mut IntegralRecord| -> Result<(), AppError> {
        if integral.symbolic[arity..].iter().any(|&v| v)
            || integral.values[arity..].iter().any(|&v| v != 0)
        {
            return Err(AppError::input("candidate integral uses a padding axis"));
        }
        integral.symbolic.truncate(arity);
        integral.values.truncate(arity);
        Ok(())
    };
    for integral in &mut record.finite_residuals {
        project(integral)?;
    }
    for rule in &mut record.rules {
        project(&mut rule.target)?;
        for term in &mut rule.rhs {
            project(&mut term.integral)?;
        }
        for source in &mut rule.sources {
            project(&mut source.integral)?;
            if source.shifts[arity..].iter().any(|&v| v != 0) {
                return Err(AppError::input("candidate seed shifts a padding axis"));
            }
            source.shifts.truncate(arity);
        }
        let mut fixed = Vec::new();
        for (&axis, &value) in rule.case.fixed_axes.iter().zip(&rule.case.fixed_values) {
            if axis < arity {
                fixed.push((axis, value));
            } else if value != 0 {
                return Err(AppError::input("candidate case uses a padding axis"));
            }
        }
        rule.case.fixed_axes = fixed.iter().map(|&(axis, _)| axis).collect();
        rule.case.fixed_values = fixed.into_iter().map(|(_, value)| value).collect();
    }
    Ok(record)
}

fn intern_coefficient(
    table: &mut CoefficientTableBuilder,
    value: &Coefficient,
) -> Result<u32, AppError> {
    if cfg!(feature = "capacity-dispatch")
        && value
            .get_variables()
            .iter()
            .any(|v| matches!(v, symbolica::poly::PolyVariable::Temporary(_)))
    {
        let variables = value
            .get_variables()
            .iter()
            .filter(|v| !matches!(v, symbolica::poly::PolyVariable::Temporary(_)))
            .cloned()
            .collect::<Vec<_>>();
        use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
        let value = Coefficient::from_num_den(
            value
                .numerator
                .rearrange_with_growth(&variables)
                .map_err(AppError::input)?,
            value
                .denominator
                .rearrange_with_growth(&variables)
                .map_err(AppError::input)?,
            &symbolica::domains::integer::Z,
            true,
        );
        return Ok(table.intern(&value).map_err(binary_error)?.index() as u32);
    }
    Ok(table.intern(value).map_err(binary_error)?.index() as u32)
}

fn intern_polynomial(
    table: &mut CoefficientTableBuilder,
    value: &CoefficientPolynomial,
) -> Result<u32, AppError> {
    intern_coefficient(table, &Coefficient::from(value.clone()))
}

/// Capacity variables extend the authenticated physical context only with unused temporary axes.
pub(super) fn storage_variables<const N: usize>(
    context: &IndexedCoefficientContext,
) -> Vec<symbolica::poly::PolyVariable> {
    let identity = context.one();
    let mut storage_variables = identity.raw().get_variables().as_ref().clone();
    let mut temporary = 0;
    while storage_variables.len() < identity.raw().get_variables().len() + N - context.index_count()
    {
        let variable = symbolica::poly::PolyVariable::Temporary(temporary);
        temporary += 1;
        if !storage_variables.contains(&variable) {
            storage_variables.push(variable);
        }
    }
    storage_variables
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
    let generation_policy = super::policy::parse(&bundle.solver_policy)?;
    let mathematical_order = super::order::saved_policy(bundle)?;
    let max_numerator_rank = generation_policy.max_numerator_rank;
    let finite_case_policy = generation_policy.finite_case_policy;
    if !rustred::fits_storage(bundle.root_sector.len(), N) {
        return Err(AppError::input("candidate reconstruction arity"));
    }
    let storage_variables = storage_variables::<N>(context);
    let variables = storage_variables.as_slice();
    bundle
        .sectors
        .iter()
        .enumerate()
        .map(|(sector_ordinal, record)| {
            let sector: [bool; N] =
                rustred::storage_array(&record.sector, false).expect("validated arity");
            let rules = record
                .rules
                .iter()
                .enumerate()
                .map(|(rule_ordinal, record)| {
                    let mut rule = rules::restore_rule(
                        record,
                        &bundle.coefficients,
                        variables,
                        indices,
                        &sector,
                    )?;
                    rule.dispatch_policy = dispatch::policy(bundle, sector_ordinal, rule_ordinal);
                    Ok(rule)
                })
                .collect::<Result<Vec<_>, AppError>>()?;
            let finite_residuals = record
                .finite_residuals
                .iter()
                .map(integral)
                .collect::<Result<Vec<_>, _>>()?;
            let order =
                rustred::solver::IntegralOrder::from_persisted_policy(sector, &mathematical_order)
                    .and_then(|order| order.with_physical_arity(context.index_count()))
                    .map_err(|error| AppError::input(error.to_string()))?;
            Ok((
                sector,
                SectorSolution {
                    order,
                    rules,
                    finite_residuals,
                    stats: SectorStats::default(),
                    max_numerator_rank,
                    finite_case_policy,
                },
            ))
        })
        .collect()
}
