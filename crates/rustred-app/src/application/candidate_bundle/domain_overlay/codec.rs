use std::sync::Arc;

use rustred::foundry::artifact::SourcePortRuleReplayAudit;
use rustred::persistence::{
    BinaryProgramKind, BinarySection, CoefficientTableBuilder, DecodedCoefficientTable,
    EncodedCoefficientTable, NativeFamilyRecord, ProgramEnvelope, SectionTag, encode_program,
    inspect_program,
};
use rustred::solver::{
    BoundOwnerOverlay, CandidateOwnerPrograms, FiniteCasePolicy, IntegralOrder, OwnerFeedbackError,
    SectorDomainSolution, SectorStats,
};

use super::super::{
    CandidateBundleLimits, MAX_CANDIDATE_BUNDLE_BYTES,
    codec::{CollectionBudget, binary_error, rules},
    order,
};
use super::CandidateDomainOverlayLoadLimits;
use super::model::{DomainRecord, SCHEMA};
use crate::AppError;

const SECTIONS: [SectionTag; 4] = [
    SectionTag::SYMBOLICA_STATE,
    SectionTag::COEFFICIENTS,
    SectionTag::FAMILY,
    SectionTag::PROGRAM,
];

pub(super) fn encode<const N: usize>(
    programs: &Arc<CandidateOwnerPrograms<N>>,
    overlay: &BoundOwnerOverlay<N>,
    base_owner_blake3: [u8; 32],
    limits: CandidateBundleLimits,
) -> Result<Vec<u8>, AppError> {
    if !programs.owns_domain_overlay(overlay) {
        return Err(AppError::input(
            "partial overlay belongs to another immutable owner lineage",
        ));
    }
    if overlay.terminal_count() != 0 {
        return Err(AppError::input(
            "partial rules-only output must not retain finite residual terminals",
        ));
    }
    let family = programs.context().family();
    let solution = overlay.partial_solution();
    let mut table = CoefficientTableBuilder::new(limits.binary_limits());
    let record = DomainRecord {
        schema: SCHEMA.into(),
        base_owner_blake3,
        family_fingerprint: family.fingerprint().to_owned(),
        owner_sector: overlay.owner_sector().to_vec(),
        root_sector: overlay.owner_root().to_vec(),
        integral_order: solution
            .order
            .persisted_policy()
            .map_err(|error| AppError::input(error.to_string()))?
            .stable_id()
            .to_string(),
        permutation: solution.order.permutation().map(|p| p.to_vec()),
        max_numerator_rank: solution.max_numerator_rank,
        finite_case_policy: solution.finite_case_policy.as_str().into(),
        policy: overlay.policy().into(),
        attempt_limits: overlay.attempt_limits().into(),
        requested_cases: solution
            .requested_cases
            .iter()
            .map(|case| rules::case_record(case, &mut table))
            .collect::<Result<_, _>>()?,
        rules: solution
            .rules
            .iter()
            .map(|rule| rules::rule_record(rule, &mut table))
            .collect::<Result<_, _>>()?,
        finite_residuals: Vec::new(),
    };
    let family = NativeFamilyRecord::from_family(family, &mut table).map_err(binary_error)?;
    write(
        &record,
        &family,
        &table.finish().map_err(binary_error)?,
        limits,
    )
}

pub(super) fn write(
    record: &DomainRecord,
    family: &NativeFamilyRecord,
    table: &EncodedCoefficientTable,
    limits: CandidateBundleLimits,
) -> Result<Vec<u8>, AppError> {
    validate(record, limits)?;
    family
        .validate_shape(limits.family_limits(), limits.binary_limits())
        .map_err(binary_error)?;
    if family.arity() != record.owner_sector.len() {
        return Err(AppError::input(
            "partial native family and owner arities differ",
        ));
    }
    let program = bincode::encode_to_vec(record, bincode::config::standard())
        .map_err(|e| AppError::serialization(e.to_string()))?;
    let family = bincode::encode_to_vec(family, bincode::config::standard())
        .map_err(|e| AppError::serialization(e.to_string()))?;
    encode_program(
        BinaryProgramKind::DomainRules,
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

pub(super) fn read_structure(
    bytes: &[u8],
    limits: CandidateBundleLimits,
) -> Result<(ProgramEnvelope<'_>, DomainRecord, NativeFamilyRecord), AppError> {
    if bytes.len() > limits.bundle_byte_limit() {
        return Err(AppError::limit(
            "partial rules program exceeds its byte limit",
        ));
    }
    let envelope = inspect_program(bytes, limits.binary_limits()).map_err(binary_error)?;
    if envelope.kind() != BinaryProgramKind::DomainRules
        || envelope
            .sections()
            .iter()
            .map(|s| s.tag)
            .collect::<Vec<_>>()
            != SECTIONS
    {
        return Err(AppError::schema(
            "expected a partial rules-only binary program",
        ));
    }
    let program = envelope
        .section(SectionTag::PROGRAM)
        .expect("checked section");
    let config = bincode::config::standard().with_limit::<MAX_CANDIDATE_BUNDLE_BYTES>();
    let (record, consumed): (DomainRecord, usize) = bincode::decode_from_slice(program, config)
        .map_err(|e| AppError::schema(format!("invalid partial rules record: {e}")))?;
    if consumed != program.len() {
        return Err(AppError::schema("trailing partial rules structural bytes"));
    }
    validate(&record, limits)?;
    let bytes = envelope
        .section(SectionTag::FAMILY)
        .expect("checked section");
    let (family, consumed): (NativeFamilyRecord, usize) = bincode::decode_from_slice(bytes, config)
        .map_err(|e| AppError::schema(format!("invalid partial native family record: {e}")))?;
    if consumed != bytes.len() {
        return Err(AppError::schema("trailing partial native family bytes"));
    }
    family
        .validate_shape(limits.family_limits(), limits.binary_limits())
        .map_err(binary_error)?;
    if family.arity() != record.owner_sector.len() {
        return Err(AppError::input(
            "partial native family and owner arities differ",
        ));
    }
    Ok((envelope, record, family))
}

fn validate(record: &DomainRecord, limits: CandidateBundleLimits) -> Result<(), AppError> {
    let n = record.owner_sector.len();
    if record.schema != SCHEMA || !(1..=16).contains(&n) || record.root_sector.len() != n {
        return Err(AppError::schema("invalid partial rules schema or arity"));
    }
    if !record.finite_residuals.is_empty() {
        return Err(AppError::input(
            "partial rules-only input must not retain finite residual terminals",
        ));
    }
    if record
        .owner_sector
        .iter()
        .zip(&record.root_sector)
        .any(|(&owner, &root)| owner && !root)
    {
        return Err(AppError::input(
            "partial owner sector lies outside saved root",
        ));
    }
    order::bound_policy(&record.integral_order, n, record.permutation.as_deref())?;
    let finite_policy: FiniteCasePolicy =
        record.finite_case_policy.parse().map_err(AppError::input)?;
    if finite_policy == FiniteCasePolicy::RetainRankFinite && record.max_numerator_rank.is_none() {
        return Err(AppError::input(
            "partial finite retention requires explicit rank",
        ));
    }
    if record.requested_cases.is_empty() {
        return Err(AppError::input(
            "partial rules need nonempty nominated cases",
        ));
    }
    if record.family_fingerprint.len() > crate::MAX_INPUT_BYTES {
        return Err(AppError::limit(
            "partial family fingerprint exceeds its byte limit",
        ));
    }
    let mut budget = CollectionBudget::new(limits, 1)?;
    budget.admit_rules(&record.rules, 0)?;
    budget.admit_cases(&record.requested_cases)?;
    for case in &record.requested_cases {
        rules::validate_case(case, n)?;
    }
    for rule in &record.rules {
        rules::validate_rule(rule, n)?;
    }
    Ok(())
}

pub(super) fn load<const N: usize>(
    programs: &Arc<CandidateOwnerPrograms<N>>,
    bytes: &[u8],
    expected_owner: [bool; N],
    expected_base_owner_blake3: [u8; 32],
    limits: CandidateDomainOverlayLoadLimits,
) -> Result<(BoundOwnerOverlay<N>, SourcePortRuleReplayAudit<N>), AppError> {
    let (envelope, record, family) = read_structure(bytes, limits.bundle)?;
    if record.owner_sector != expected_owner
        || record.base_owner_blake3 != expected_base_owner_blake3
        || record.family_fingerprint != programs.context().family().fingerprint()
    {
        return Err(AppError::input(
            "partial rules family, owner or immutable base digest differs",
        ));
    }
    // Root/order are checked against the actually loaded immutable owner before
    // importing a Symbolica frame; no metadata-only substitute owner is built.
    let search = programs
        .bind_owner_search(expected_owner, record.policy.native()?)
        .map_err(|e| AppError::input(e.to_string()))?;
    if search.owner_root().as_slice() != record.root_sector
        || search.owner_ordering().stable_id().to_string() != record.integral_order
    {
        return Err(AppError::input(
            "partial rules saved root or mathematical order differs from owner",
        ));
    }
    let table = DecodedCoefficientTable::import_generated(
        envelope
            .section(SectionTag::SYMBOLICA_STATE)
            .expect("checked section"),
        envelope
            .section(SectionTag::COEFFICIENTS)
            .expect("checked section"),
        limits.bundle.binary_limits(),
    )
    .map_err(binary_error)?;
    let restored_family = family
        .to_family(
            &table,
            limits.bundle.family_limits(),
            limits.bundle.binary_limits(),
        )
        .map_err(binary_error)?;
    if restored_family.fingerprint() != record.family_fingerprint {
        return Err(AppError::input(
            "partial native family differs from its declared owner family",
        ));
    }
    let identity = programs.context().coefficient_context().one();
    let variables = identity.raw().get_variables().as_slice();
    let indices = programs.context().index_variables();
    let requested_cases = record
        .requested_cases
        .iter()
        .map(|case| rules::restore_case(case, &table, variables, indices, &expected_owner))
        .collect::<Result<_, _>>()?;
    let restored_rules = record
        .rules
        .iter()
        .map(|rule| rules::restore_rule(rule, &table, variables, indices, &expected_owner))
        .collect::<Result<_, _>>()?;
    let order = order::bound_policy(&record.integral_order, N, record.permutation.as_deref())?;
    let solution = SectorDomainSolution {
        order: IntegralOrder::from_persisted_policy(expected_owner, &order)
            .map_err(|e| AppError::input(e.to_string()))?,
        requested_cases,
        max_numerator_rank: record.max_numerator_rank,
        finite_case_policy: record.finite_case_policy.parse().map_err(AppError::input)?,
        rules: restored_rules,
        finite_residuals: Vec::new(),
        stats: SectorStats::default(),
    };
    search
        .restore_replayed_residual_free_overlay(
            solution,
            record.attempt_limits.native(),
            limits.overlay,
            limits.replay,
        )
        .map_err(|e| {
            let message = format!("partial rules cold validation: {e}");
            match e {
                OwnerFeedbackError::ResourceLimit { .. }
                | OwnerFeedbackError::Replay(
                    rustred::foundry::artifact::SourcePortAuditError::ResourceBudgetExhausted {
                        ..
                    }
                    | rustred::foundry::artifact::SourcePortAuditError::UnsupportedResourcePolicy {
                        ..
                    },
                ) => AppError::limit(message),
                _ => AppError::input(message),
            }
        })
}
