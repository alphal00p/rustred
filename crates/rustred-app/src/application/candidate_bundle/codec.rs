use std::sync::Arc;

use rustred::algebra::{Coefficient, CoefficientPolynomial, IndexedCoefficientContext};
use rustred::persistence::{
    BinaryIoError, BinaryProgramKind, BinarySection, CoefficientTableBuilder,
    EncodedCoefficientTable, NativeFamilyRecord, ProgramEnvelope, SectionTag, encode_program,
};
use rustred::propagator::bundle_wire::{self, BundleWireError, BundleWireErrorKind, StoredProgram};
use rustred::solver::{Integral, SectorSolution};

pub(super) mod dispatch;
pub(super) mod rules;

use crate::application::AppError;

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
    let coefficients =
        bundle_wire::import_coefficients(&envelope, &records, limits.binary_limits())?;
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
    let envelope = bundle_wire::candidate_sections(bytes, limits.binary_limits())?;
    let program = envelope
        .section(SectionTag::PROGRAM)
        .expect("checked section");
    let allowance = limits.bundle_byte_limit();
    let records = match bundle_wire::decode_program_record(program, allowance)? {
        StoredProgram::Current(records) => records,
        StoredProgram::Legacy(old) => {
            let integral_order =
                super::load::candidate_ordering(old.root_sector.len(), old.permutation.as_deref())?
                    .stable_id()
                    .to_string();
            old.with_order(integral_order)
        }
    };
    // Shape/collection checks precede native Symbolica state import. The native
    // state and algebra records still require trusted generated provenance.
    validate(&records, limits)?;
    let encoded_family = envelope
        .section(SectionTag::FAMILY)
        .expect("checked section");
    let family = bundle_wire::decode_family_record(encoded_family, allowance)?;
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

/// One bincode record under the caller's allowance (`bundle_wire::decode_record`,
/// the candidate bundle's reader).
pub(in crate::application) fn decode_record<T: bincode::Decode<()>>(
    bytes: &[u8],
    allowance: usize,
) -> Result<(T, usize), AppError> {
    Ok(bundle_wire::decode_record(bytes, allowance)?)
}

/// The shared decoder's refusals keep their kind, message and resource cause.
impl From<BundleWireError> for AppError {
    fn from(error: BundleWireError) -> Self {
        let kind = match error.kind() {
            BundleWireErrorKind::Input => crate::AppErrorKind::Input,
            BundleWireErrorKind::Schema => crate::AppErrorKind::Schema,
            BundleWireErrorKind::Limit => crate::AppErrorKind::Limit,
        };
        AppError::from_native(error, kind)
    }
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
        BinaryIoError::Resource(error) => AppError::resource(error),
        BinaryIoError::Limit { .. } | BinaryIoError::Allocation { .. } => {
            AppError::limit(error.to_string())
        }
        _ => AppError::schema(error.to_string()),
    }
}

pub(super) fn validate_ids(records: &ProgramRecord, count: usize) -> Result<(), AppError> {
    for sector in &records.sectors {
        for rule in &sector.rules {
            bundle_wire::validate_rule_ids(rule, count)?;
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
    crate::application::validate_ingress(&bundle.family_source)?;
    let n = bundle.root_sector.len();
    super::preparation::validate_arity(n)?;
    super::preparation::validate_permutation(n, bundle.permutation.as_deref())?;
    super::order::saved_policy(bundle)?;
    let mut budget = CollectionBudget::new(limits, bundle.sectors.len())?;
    bundle_wire::validate_sectors(bundle, |sector| budget.admit_sector(sector))
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
        if self.limits.diagnostics_enabled && self.entries > self.limits.max_collection_entries {
            return Err(AppError::limit(format!(
                "candidate aggregate collection-entry budget exceeded: {} > {}",
                self.entries, self.limits.max_collection_entries,
            )));
        }
        Ok(())
    }
}

pub(super) fn collection_entries(record: &ProgramRecord) -> Result<usize, AppError> {
    let limits = CandidateBundleLimits::default();
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
    fn mid_decode_resource_refusal_keeps_its_cause_and_can_retry() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

        let expected = vec![1u64, 2];
        let bytes = bincode::encode_to_vec(&expected, bincode::config::standard()).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let budget = ResourceBudget::with_memory_monitor(
            ResourceLimits {
                max_process_rss_bytes: Some(1),
                ..Default::default()
            },
            move || {
                Ok(if observed.fetch_add(1, Ordering::SeqCst) < 2 {
                    0
                } else {
                    2
                })
            },
        );
        let error = budget
            .run(|| decode_record::<Vec<u64>>(&bytes, usize::MAX))
            .unwrap_err();
        assert!(calls.load(Ordering::SeqCst) >= 3);
        assert_eq!(
            error.resource_limit_error(),
            Some(&ResourceLimitError::Memory {
                rss_bytes: 2,
                additional_bytes: 0,
                limit_bytes: 1,
            })
        );
        let (restored, consumed) = decode_record::<Vec<u64>>(&bytes, usize::MAX).unwrap();
        assert_eq!(restored, expected);
        assert_eq!(consumed, bytes.len());
    }

    #[test]
    fn ordinary_candidate_census_ignores_count_templates_and_preserves_resource_failures() {
        let limits = CandidateBundleLimits {
            max_collection_entries: 0,
            ..Default::default()
        };
        let mut census = CollectionBudget::new(limits, 2).unwrap();
        census.admit_sector(&sector(false)).unwrap();
        census.admit_sector(&sector(true)).unwrap();
        assert_eq!(census.admitted_entries(), 32);
        assert!(CollectionBudget::new(limits.with_diagnostics(), 2).is_err());
        let bytes = bincode::encode_to_vec(vec![1u64, 2], bincode::config::standard()).unwrap();
        let budget = symbolica::utils::resources::ResourceBudget::with_memory_monitor(
            symbolica::utils::resources::ResourceLimits {
                max_process_rss_bytes: Some(1),
                ..Default::default()
            },
            || Ok(2),
        );
        let error = budget
            .run(|| decode_record::<Vec<u64>>(&bytes, usize::MAX))
            .unwrap_err();
        assert!(matches!(
            error.resource_limit_error(),
            Some(symbolica::utils::resources::ResourceLimitError::Memory { .. })
        ));
    }

    #[test]
    fn individually_admissible_shards_cannot_bypass_final_aggregate_entry_limit() {
        let sectors = vec![sector(false), sector(true)];
        // Each sector costs 15: rule(1), residuals(2), equations(2), RHS(3),
        // sources(2), branches(2), branch entries(3). Final count adds 2.
        let mut limits = CandidateBundleLimits::default();
        limits.diagnostics_enabled = true;
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
        limits.diagnostics_enabled = true;
        limits.max_collection_entries = 32;
        let mut exact = CollectionBudget::new(limits, 2).unwrap();
        for sector in &bundle.sectors {
            exact.admit_sector(sector).unwrap();
        }
        assert_eq!(exact.entries, 32);
        validate(&bundle, limits).unwrap();
        limits.diagnostics_enabled = true;
        limits.max_collection_entries = 1;
        assert_eq!(
            CollectionBudget::new(limits, 2).unwrap_err().kind(),
            AppErrorKind::Limit
        );
    }
}

fn integral_record<const N: usize>(value: &Integral<N>) -> IntegralRecord {
    IntegralRecord {
        symbolic: value.powers().iter().map(|p| p.is_symbolic()).collect(),
        values: value.powers().iter().map(|p| p.value()).collect(),
    }
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
pub(super) use bundle_wire::storage_variables;

/// Rebuild ordinary transport values only. This performs no source replay and
/// never constructs a ClosedArtifact, replay certificate, or trusted matrix.
pub(super) fn solutions<const N: usize>(
    bundle: &Bundle,
    context: &IndexedCoefficientContext,
    indices: &[usize; N],
    limits: CandidateBundleLimits,
) -> Result<Vec<([bool; N], SectorSolution<N>)>, AppError> {
    solutions_from_table(
        &bundle.records,
        bundle.coefficients.as_ref(),
        context,
        indices,
        limits,
    )
}

pub(super) fn solutions_from_table<const N: usize>(
    bundle: &ProgramRecord,
    coefficients: &impl rules::NativeCoefficientSource,
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
    let restore = bundle_wire::Restore {
        table: coefficients,
        variables: &storage_variables,
        indices,
    };
    (0..bundle.sectors.len())
        .map(|ordinal| {
            Ok(restore.sector(
                bundle,
                ordinal,
                &mathematical_order,
                context.index_count(),
                max_numerator_rank,
                finite_case_policy,
            )?)
        })
        .collect()
}
