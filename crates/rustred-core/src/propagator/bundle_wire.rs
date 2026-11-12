//! The generated candidate bundle on the wire: its records, their reader and
//! their restore into solver rules (owner: M3-T7d rr-decoder, D-T7c-1). This
//! is the one decoder of the bundle: rustred-app's candidate-bundle codec
//! writes and reads it, and the p library's Custom owners
//! ([`super::custom`]) read their payloads with it.
//!
//! A bundle is a binary program of kind [`BinaryProgramKind::Candidates`] with
//! four sections: the Symbolica state, the coefficient table, the native
//! family record and the program record. The program record has three
//! schemas: v1 ([`LEGACY_CANDIDATE_BUNDLE_SCHEMA`], read-only: a slot
//! permutation, no order), v2 ([`CANDIDATE_BUNDLE_SCHEMA`]: an order, no
//! dispatch table) and v3 ([`DISPATCH_CANDIDATE_BUNDLE_SCHEMA`]: a sparse
//! rule-dispatch table). Every record uses bincode's standard configuration,
//! so its field order is its byte layout; a v3 record without dispatch entries
//! is written in the v2 layout ([`encode_program_record`]).
//!
//! Readers add their own admission on top: the application its limits,
//! policies and ingress checks, a Custom owner its all-rank and chart rules.
//! Errors carry the application's kinds and messages ([`BundleWireError`]); a
//! resource refusal keeps its [`ResourceLimitError`] as the source.
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use symbolica::poly::PolyVariable;
use symbolica::utils::resources::{self, ResourceLimitError};

use crate::algebra::{Coefficient, CoefficientPolynomial, IndexedCoefficientContext};
use crate::persistence::{
    BinaryIoError, BinaryIoLimits, BinaryProgramKind, CoefficientId, DecodedCoefficientTable,
    LazyDecodedCoefficientTable, NativeFamilyRecord, ProgramEnvelope, SectionTag, inspect_program,
};
use crate::sector::OrderingPolicy;
use crate::solver::{
    AffineCase, AffineIntersection, Case, CoordinateCase, ExceptionalConditions, FiniteCasePolicy,
    Integral, IntegralOrder, Power, RuleCandidate, RuleDispatchPolicy, SearchStats, SectorRule,
    SectorSolution, SectorStats, Seed, SeedSource, Term,
};

/// Read-only legacy program records: a slot permutation instead of an order.
pub const LEGACY_CANDIDATE_BUNDLE_SCHEMA: &str = "rustred.generated-candidates.binary.v1";
/// Program records with an order and without a dispatch table.
pub const CANDIDATE_BUNDLE_SCHEMA: &str = "rustred.generated-candidates.binary.v2";
/// Program records with a sparse rule-dispatch table.
pub const DISPATCH_CANDIDATE_BUNDLE_SCHEMA: &str = "rustred.generated-candidates.binary.v3";
/// The status of every generated bundle: its rules are not certified.
pub const CANDIDATE_BUNDLE_STATUS: &str = "uncertified-candidates";

// ----- errors -----

/// The class of a [`BundleWireError`], as the application reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleWireErrorKind {
    /// A record or value of invalid shape.
    Input,
    /// Not a bundle of a known schema, or a malformed record.
    Schema,
    /// Bytes, an allocation or a resource budget beyond its allowance.
    Limit,
}

/// A refused bundle record: its kind, a message and, for a resource refusal,
/// the original cause (also the error's source).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleWireError {
    kind: BundleWireErrorKind,
    message: String,
    resource: Option<ResourceLimitError>,
}

impl BundleWireError {
    fn new(kind: BundleWireErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            resource: None,
        }
    }

    fn input(message: impl Into<String>) -> Self {
        Self::new(BundleWireErrorKind::Input, message)
    }

    fn schema(message: impl Into<String>) -> Self {
        Self::new(BundleWireErrorKind::Schema, message)
    }

    fn limit(message: impl Into<String>) -> Self {
        Self::new(BundleWireErrorKind::Limit, message)
    }

    fn resource(error: ResourceLimitError) -> Self {
        Self {
            kind: BundleWireErrorKind::Limit,
            message: error.to_string(),
            resource: Some(error),
        }
    }

    /// A native error of class `fallback`, unless a resource refusal sits in
    /// its cause chain (then a limit that keeps it).
    fn native(error: &(dyn Error + 'static), fallback: BundleWireErrorKind) -> Self {
        let mut source = Some(error);
        while let Some(cause) = source {
            if let Some(resource) = cause.downcast_ref::<ResourceLimitError>() {
                return Self {
                    kind: BundleWireErrorKind::Limit,
                    message: error.to_string(),
                    resource: Some(resource.clone()),
                };
            }
            source = cause
                .downcast_ref::<std::io::Error>()
                .and_then(std::io::Error::get_ref)
                .map(|inner| inner as &(dyn Error + 'static))
                .or_else(|| cause.source());
        }
        Self::new(fallback, error.to_string())
    }

    fn binary(error: BinaryIoError) -> Self {
        match error {
            BinaryIoError::Resource(error) => Self::resource(error),
            BinaryIoError::Limit { .. } | BinaryIoError::Allocation { .. } => {
                Self::limit(error.to_string())
            }
            _ => Self::schema(error.to_string()),
        }
    }

    pub fn kind(&self) -> BundleWireErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn resource_limit_error(&self) -> Option<&ResourceLimitError> {
        self.resource.as_ref()
    }
}

impl fmt::Display for BundleWireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for BundleWireError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.resource.as_ref().map(|error| error as _)
    }
}

// ----- records -----

/// The program record of schemas v2 and v3.
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct ProgramRecord {
    pub schema: String,
    pub status: String,
    pub solver_policy: String,
    pub family_source: String,
    pub input_format: String,
    pub family_fingerprint: String,
    pub root_sector: Vec<bool>,
    pub permutation: Option<Vec<usize>>,
    pub integral_order: String,
    pub sectors: Vec<SectorRecord>,
    /// Sparse execution policy, independent of equation/source wire records.
    pub rule_dispatch: Vec<RuleDispatchRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct RuleDispatchRecord {
    pub sector: usize,
    pub rule: usize,
    /// 1 = AfterBaselinePartitionWholePiece; default entries are forbidden.
    pub policy: u8,
}

/// Read-only adapter for existing control-owner inputs. No missing custom
/// order is inferred: v1 had exactly the named Spired/permutation semantics.
#[derive(bincode::Decode)]
pub struct LegacyProgramRecord {
    pub schema: String,
    pub status: String,
    pub solver_policy: String,
    pub family_source: String,
    pub input_format: String,
    pub family_fingerprint: String,
    pub root_sector: Vec<bool>,
    pub permutation: Option<Vec<usize>>,
    pub sectors: Vec<SectorRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct SectorRecord {
    pub sector: Vec<bool>,
    pub rules: Vec<RuleRecord>,
    pub finite_residuals: Vec<IntegralRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct RuleRecord {
    pub case: CaseRecord,
    pub target: IntegralRecord,
    pub rhs: Vec<TermRecord>,
    pub sources: Vec<SeedRecord>,
    pub exclusions: Vec<Vec<u32>>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct CaseRecord {
    pub kind: String,
    pub fixed_axes: Vec<usize>,
    pub fixed_values: Vec<i16>,
    pub equations: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct IntegralRecord {
    pub symbolic: Vec<bool>,
    pub values: Vec<i16>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct TermRecord {
    pub integral: IntegralRecord,
    pub coefficient: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct SeedRecord {
    pub basis_row: usize,
    pub integral: IntegralRecord,
    pub shifts: Vec<i16>,
}

impl ProgramRecord {
    /// The dispatch policy of rule `rule` of sector `sector`: after the
    /// baseline partition if the dispatch table lists it, else the partition.
    pub fn dispatch_policy(&self, sector: usize, rule: usize) -> RuleDispatchPolicy {
        match self
            .rule_dispatch
            .binary_search_by_key(&(sector, rule), |entry| (entry.sector, entry.rule))
        {
            Ok(_) => RuleDispatchPolicy::AfterBaselinePartitionWholePiece,
            Err(_) => RuleDispatchPolicy::Partition,
        }
    }
}

impl LegacyProgramRecord {
    /// The current record of a legacy one, with the order `integral_order`
    /// the caller derived from its permutation.
    pub fn with_order(self, integral_order: String) -> ProgramRecord {
        ProgramRecord {
            schema: self.schema,
            status: self.status,
            solver_policy: self.solver_policy,
            family_source: self.family_source,
            input_format: self.input_format,
            family_fingerprint: self.family_fingerprint,
            root_sector: self.root_sector,
            permutation: self.permutation,
            integral_order,
            sectors: self.sectors,
            rule_dispatch: Vec::new(),
        }
    }
}

// Exact pre-policy v2 wire layout; default writers keep these bytes.
#[derive(bincode::Decode)]
struct V2Record {
    schema: String,
    status: String,
    solver_policy: String,
    family_source: String,
    input_format: String,
    family_fingerprint: String,
    root_sector: Vec<bool>,
    permutation: Option<Vec<usize>>,
    integral_order: String,
    sectors: Vec<SectorRecord>,
}

#[derive(bincode::Encode)]
struct V2Ref<'a> {
    schema: &'a str,
    status: &'a str,
    solver_policy: &'a str,
    family_source: &'a str,
    input_format: &'a str,
    family_fingerprint: &'a str,
    root_sector: &'a [bool],
    permutation: &'a Option<Vec<usize>>,
    integral_order: &'a str,
    sectors: &'a [SectorRecord],
}

/// The bytes of a program record: the v2 layout (schema v2) without dispatch
/// entries, the record itself otherwise.
pub fn encode_program_record(
    records: &ProgramRecord,
) -> Result<Vec<u8>, bincode::error::EncodeError> {
    if records.rule_dispatch.is_empty() {
        let wire = V2Ref {
            schema: CANDIDATE_BUNDLE_SCHEMA,
            status: &records.status,
            solver_policy: &records.solver_policy,
            family_source: &records.family_source,
            input_format: &records.input_format,
            family_fingerprint: &records.family_fingerprint,
            root_sector: &records.root_sector,
            permutation: &records.permutation,
            integral_order: &records.integral_order,
            sectors: &records.sectors,
        };
        bincode::encode_to_vec(wire, bincode::config::standard())
    } else {
        bincode::encode_to_vec(records, bincode::config::standard())
    }
}

// ----- reading -----

/// One bincode record from the front of `bytes`: `(value, bytes consumed)`.
///
/// Bincode's public allocation-claim API enforces the caller's runtime grant.
/// Framing and nested allocation checks remain active above the former 1 GiB
/// default; native Symbolica payloads still require trusted provenance. A
/// Symbolica resource budget is checked before, during and after the read.
pub fn decode_record<T: bincode::Decode<()>>(
    bytes: &[u8],
    allowance: usize,
) -> Result<(T, usize), BundleWireError> {
    use bincode::de::{
        Decoder,
        read::{Reader, SliceReader},
    };
    struct Counted<'a> {
        reader: SliceReader<'a>,
        position: usize,
    }
    impl Reader for Counted<'_> {
        fn read(&mut self, bytes: &mut [u8]) -> Result<(), bincode::error::DecodeError> {
            resources::checkpoint().map_err(|error| bincode::error::DecodeError::Io {
                inner: std::io::Error::other(error),
                additional: bytes.len(),
            })?;
            self.reader.read(bytes)?;
            self.position += bytes.len();
            Ok(())
        }
        fn consume(&mut self, n: usize) {
            self.reader.consume(n);
            self.position += n;
        }
    }
    resources::checkpoint().map_err(BundleWireError::resource)?;
    if bytes.len() > allowance {
        return Err(BundleWireError::limit(
            "candidate structural bytes exceed caller allowance",
        ));
    }
    let mut decoder = bincode::de::DecoderImpl::new(
        Counted {
            reader: SliceReader::new(bytes),
            position: 0,
        },
        bincode::config::standard().with_limit::<{ usize::MAX }>(),
        (),
    );
    let error = |e: bincode::error::DecodeError| match e {
        bincode::error::DecodeError::LimitExceeded => {
            BundleWireError::limit("candidate structural allocation exceeds caller allowance")
        }
        bincode::error::DecodeError::Io { inner, .. } => {
            BundleWireError::native(&inner, BundleWireErrorKind::Schema)
        }
        _ => BundleWireError::schema(format!("invalid candidate structural record: {e}")),
    };
    decoder
        .claim_bytes_read(usize::MAX - allowance)
        .map_err(error)?;
    let value = T::decode(&mut decoder).map_err(error)?;
    let consumed = decoder.reader().position;
    resources::checkpoint().map_err(BundleWireError::resource)?;
    Ok((value, consumed))
}

/// The sections of a generated candidate bundle, in their fixed order.
pub fn candidate_sections(
    bytes: &[u8],
    limits: BinaryIoLimits,
) -> Result<ProgramEnvelope<'_>, BundleWireError> {
    let envelope = inspect_program(bytes, limits).map_err(BundleWireError::binary)?;
    let tags: Vec<_> = envelope
        .sections()
        .iter()
        .map(|section| section.tag)
        .collect();
    if envelope.kind() != BinaryProgramKind::Candidates
        || tags
            != [
                SectionTag::SYMBOLICA_STATE,
                SectionTag::COEFFICIENTS,
                SectionTag::FAMILY,
                SectionTag::PROGRAM,
            ]
    {
        return Err(BundleWireError::schema(
            "expected a generated candidate binary program",
        ));
    }
    Ok(envelope)
}

/// A program record as stored.
pub enum StoredProgram {
    /// Schema v2 or v3.
    Current(ProgramRecord),
    /// Schema v1: its order is the caller's to derive ([`LegacyProgramRecord::with_order`]).
    Legacy(LegacyProgramRecord),
}

/// The program record of `bytes` (the program section), by its schema; the
/// record must span the whole section.
pub fn decode_program_record(
    bytes: &[u8],
    allowance: usize,
) -> Result<StoredProgram, BundleWireError> {
    let (schema, _): (String, usize) = decode_record(bytes, allowance)?;
    let (program, consumed) = match schema.as_str() {
        DISPATCH_CANDIDATE_BUNDLE_SCHEMA => {
            let (records, consumed) = decode_record(bytes, allowance)?;
            (StoredProgram::Current(records), consumed)
        }
        CANDIDATE_BUNDLE_SCHEMA => {
            let (records, consumed) = decode_v2_program_record(bytes, allowance)?;
            (StoredProgram::Current(records), consumed)
        }
        LEGACY_CANDIDATE_BUNDLE_SCHEMA => {
            let (records, consumed) = decode_record(bytes, allowance)?;
            (StoredProgram::Legacy(records), consumed)
        }
        _ => {
            return Err(BundleWireError::schema(
                "unsupported candidate structural schema",
            ));
        }
    };
    if consumed != bytes.len() {
        return Err(BundleWireError::schema(
            "trailing candidate structural bytes",
        ));
    }
    Ok(program)
}

/// A record in the v2 layout, read as a current record without dispatch
/// entries: `(record, bytes consumed)`.
pub fn decode_v2_program_record(
    bytes: &[u8],
    allowance: usize,
) -> Result<(ProgramRecord, usize), BundleWireError> {
    let (old, consumed): (V2Record, usize) = decode_record(bytes, allowance)?;
    let record = ProgramRecord {
        schema: old.schema,
        status: old.status,
        solver_policy: old.solver_policy,
        family_source: old.family_source,
        input_format: old.input_format,
        family_fingerprint: old.family_fingerprint,
        root_sector: old.root_sector,
        permutation: old.permutation,
        integral_order: old.integral_order,
        sectors: old.sectors,
        rule_dispatch: Vec::new(),
    };
    Ok((record, consumed))
}

/// The native family record of `bytes` (the family section), spanning it.
pub fn decode_family_record(
    bytes: &[u8],
    allowance: usize,
) -> Result<NativeFamilyRecord, BundleWireError> {
    let (family, consumed) = decode_record(bytes, allowance)?;
    if consumed != bytes.len() {
        return Err(BundleWireError::schema(
            "trailing native family record bytes",
        ));
    }
    Ok(family)
}

/// The coefficient table of a bundle (`envelope`, [`candidate_sections`]),
/// imported into the process's Symbolica state; every coefficient id of
/// `records` must lie in it. The native state requires trusted provenance.
pub fn import_coefficients(
    envelope: &ProgramEnvelope<'_>,
    records: &ProgramRecord,
    limits: BinaryIoLimits,
) -> Result<DecodedCoefficientTable, BundleWireError> {
    let section = |tag| {
        envelope
            .section(tag)
            .ok_or_else(|| BundleWireError::schema("expected a generated candidate binary program"))
    };
    let table = DecodedCoefficientTable::import_generated(
        section(SectionTag::SYMBOLICA_STATE)?,
        section(SectionTag::COEFFICIENTS)?,
        limits,
    )
    .map_err(BundleWireError::binary)?;
    for rule in records.sectors.iter().flat_map(|sector| &sector.rules) {
        validate_rule_ids(rule, table.len())?;
    }
    Ok(table)
}

// ----- shape checks -----

/// A key of `arity` slots whose powers are valid.
pub fn validate_integral(value: &IntegralRecord, arity: usize) -> Result<(), BundleWireError> {
    if value.symbolic.len() != arity || value.values.len() != arity {
        return Err(BundleWireError::input(
            "candidate integral has the wrong arity",
        ));
    }
    for (&symbolic, &value) in value.symbolic.iter().zip(&value.values) {
        Power::new(symbolic, value)
            .map_err(|error| BundleWireError::native(&error, BundleWireErrorKind::Input))?;
    }
    Ok(())
}

pub fn validate_case(case: &CaseRecord, arity: usize) -> Result<(), BundleWireError> {
    if case.fixed_axes.len() != case.fixed_values.len()
        || case.fixed_axes.iter().any(|&axis| axis >= arity)
        || case.fixed_axes.windows(2).any(|axes| axes[0] >= axes[1])
        || !matches!(case.kind.as_str(), "coordinate" | "affine")
        || (case.kind == "coordinate") != case.equations.is_empty()
    {
        return Err(BundleWireError::input("invalid candidate case shape"));
    }
    Ok(())
}

pub fn validate_rule(rule: &RuleRecord, arity: usize) -> Result<(), BundleWireError> {
    validate_integral(&rule.target, arity)?;
    validate_case(&rule.case, arity)?;
    for term in &rule.rhs {
        validate_integral(&term.integral, arity)?;
        if term.integral.symbolic != rule.target.symbolic {
            return Err(BundleWireError::input(
                "candidate RHS symbolic layout differs from target",
            ));
        }
    }
    for source in &rule.sources {
        validate_integral(&source.integral, arity)?;
        if source.shifts.len() != arity {
            return Err(BundleWireError::input(
                "candidate seed shift arity differs from root",
            ));
        }
    }
    Ok(())
}

/// Every coefficient id of `rule` lies in a table of `count` entries.
pub fn validate_rule_ids(rule: &RuleRecord, count: usize) -> Result<(), BundleWireError> {
    if rule
        .case
        .equations
        .iter()
        .chain(rule.rhs.iter().map(|term| &term.coefficient))
        .chain(rule.exclusions.iter().flatten())
        .any(|&id| id as usize >= count)
    {
        return Err(BundleWireError::input(
            "candidate coefficient ID is outside its table",
        ));
    }
    Ok(())
}

/// The dispatch table matches the schema and lists valid entries, strictly
/// ascending, each on an existing rule.
pub fn validate_dispatch(records: &ProgramRecord) -> Result<(), BundleWireError> {
    if records.rule_dispatch.is_empty() == (records.schema == DISPATCH_CANDIDATE_BUNDLE_SCHEMA) {
        return Err(BundleWireError::schema(
            "candidate dispatch table/schema mismatch",
        ));
    }
    let mut previous = None;
    for entry in &records.rule_dispatch {
        let key = (entry.sector, entry.rule);
        if entry.policy != 1
            || previous.is_some_and(|old| old >= key)
            || records
                .sectors
                .get(entry.sector)
                .is_none_or(|sector| entry.rule >= sector.rules.len())
        {
            return Err(BundleWireError::input(
                "invalid, duplicate or unordered candidate dispatch entry",
            ));
        }
        previous = Some(key);
    }
    Ok(())
}

/// Every sector is a new chart of the root's arity inside the root, its
/// residuals are concrete keys of the chart and its rules are of valid shape.
/// `admit` sees each sector after its placement and before its records (the
/// application's collection budget).
pub fn validate_sectors<E: From<BundleWireError>>(
    records: &ProgramRecord,
    mut admit: impl FnMut(&SectorRecord) -> Result<(), E>,
) -> Result<(), E> {
    let arity = records.root_sector.len();
    let mut seen = BTreeSet::new();
    for sector in &records.sectors {
        if sector.sector.len() != arity
            || !seen.insert(&sector.sector)
            || sector
                .sector
                .iter()
                .zip(&records.root_sector)
                .any(|(&active, &root)| active && !root)
        {
            return Err(BundleWireError::input(
                "candidate sector is duplicate, wrong-arity, or outside root",
            )
            .into());
        }
        admit(sector)?;
        for integral in &sector.finite_residuals {
            validate_integral(integral, arity)?;
            if integral.symbolic.iter().any(|&value| value)
                || integral
                    .values
                    .iter()
                    .zip(&sector.sector)
                    .any(|(&v, &active)| (v > 0) != active)
            {
                return Err(BundleWireError::input(
                    "candidate residual is not a concrete key in its sector",
                )
                .into());
            }
        }
        for rule in &sector.rules {
            validate_rule(rule, arity)?;
        }
    }
    Ok(())
}

// ----- restore -----

/// A coefficient table the restore reads: both stores use the native
/// Symbolica codec. Restored rules own their values; no lazy reference
/// escapes.
pub trait CoefficientSource {
    fn coefficient_count(&self) -> usize;
    fn coefficient_owned(&self, id: CoefficientId) -> Result<Coefficient, BinaryIoError>;
}

impl CoefficientSource for DecodedCoefficientTable {
    fn coefficient_count(&self) -> usize {
        self.len()
    }
    fn coefficient_owned(&self, id: CoefficientId) -> Result<Coefficient, BinaryIoError> {
        self.coefficient(id).cloned()
    }
}

impl CoefficientSource for LazyDecodedCoefficientTable {
    fn coefficient_count(&self) -> usize {
        self.len()
    }
    fn coefficient_owned(&self, id: CoefficientId) -> Result<Coefficient, BinaryIoError> {
        Ok(self.coefficient(id)?.as_ref().clone())
    }
}

impl<T: CoefficientSource> CoefficientSource for Arc<T> {
    fn coefficient_count(&self) -> usize {
        self.as_ref().coefficient_count()
    }
    fn coefficient_owned(&self, id: CoefficientId) -> Result<Coefficient, BinaryIoError> {
        self.as_ref().coefficient_owned(id)
    }
}

/// The coefficient variables of an `N`-slot storage: the context's, then
/// unused temporary variables (`capacity-dispatch` only).
pub fn storage_variables<const N: usize>(context: &IndexedCoefficientContext) -> Vec<PolyVariable> {
    let identity = context.one();
    let declared = identity.raw().get_variables();
    let mut variables = declared.as_ref().clone();
    let mut temporary = 0;
    while variables.len() < declared.len() + N - context.index_count() {
        let variable = PolyVariable::Temporary(temporary);
        temporary += 1;
        if !variables.contains(&variable) {
            variables.push(variable);
        }
    }
    variables
}

/// The key of a record in an `N`-slot storage (padding slots at zero).
pub fn integral<const N: usize>(value: &IntegralRecord) -> Result<Integral<N>, BundleWireError> {
    validate_integral(value, value.values.len())?;
    if !crate::fits_storage(value.values.len(), N) {
        return Err(BundleWireError::input(
            "candidate integral exceeds storage capacity",
        ));
    }
    let mut powers = [Power::default(); N];
    for (axis, power) in powers.iter_mut().take(value.values.len()).enumerate() {
        *power = Power::new(value.symbolic[axis], value.values[axis])
            .map_err(|error| BundleWireError::native(&error, BundleWireErrorKind::Input))?;
    }
    Ok(Integral::new(powers))
}

/// Restores records of one bundle: its coefficient table read in `variables`
/// (the storage variables of an `N`-slot context, [`storage_variables`]) with
/// the context's index variables `indices`.
pub struct Restore<'a, S, const N: usize> {
    pub table: &'a S,
    pub variables: &'a [PolyVariable],
    pub indices: &'a [usize; N],
}

impl<S: CoefficientSource, const N: usize> Restore<'_, S, N> {
    fn coefficient(&self, id: u32) -> Result<Coefficient, BundleWireError> {
        let variables = self.variables;
        let id = CoefficientId::try_from_index(id as usize).map_err(BundleWireError::binary)?;
        let value = self
            .table
            .coefficient_owned(id)
            .map_err(BundleWireError::binary)?;
        let physical_variables = value.get_variables().as_slice();
        if physical_variables != variables
            && (!cfg!(feature = "capacity-dispatch")
                || !variables.starts_with(physical_variables)
                || variables[physical_variables.len()..]
                    .iter()
                    .any(|v| !matches!(v, PolyVariable::Temporary(_))))
        {
            return Err(BundleWireError::input(
                "candidate coefficient has the wrong indexed variable map",
            ));
        }
        if value.get_variables().as_slice() == variables {
            Ok(value)
        } else {
            let numerator = value
                .numerator
                .rearrange_with_growth(variables)
                .map_err(BundleWireError::input)?;
            let denominator = value
                .denominator
                .rearrange_with_growth(variables)
                .map_err(BundleWireError::input)?;
            use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
            Ok(Coefficient::from_num_den(
                numerator,
                denominator,
                &symbolica::domains::integer::Z,
                true,
            ))
        }
    }

    fn polynomial(&self, id: u32) -> Result<CoefficientPolynomial, BundleWireError> {
        let value = self.coefficient(id)?;
        if !value.denominator.is_one() {
            return Err(BundleWireError::input(
                "candidate equation is not a polynomial",
            ));
        }
        Ok(value.numerator)
    }

    /// The case of `record` in `sector`: its fixed face, cut by its equations
    /// if affine.
    pub fn case(
        &self,
        record: &CaseRecord,
        sector: &[bool; N],
    ) -> Result<Case<N>, BundleWireError> {
        validate_case(record, N)?;
        let mut fixed = [None; N];
        for (&axis, &value) in record.fixed_axes.iter().zip(&record.fixed_values) {
            fixed[axis] = Some(value);
        }
        let face = CoordinateCase::new(fixed)
            .map_err(|e| BundleWireError::native(&e, BundleWireErrorKind::Input))?;
        if !face.is_in_sector(sector) {
            return Err(BundleWireError::input(
                "candidate fixed face is outside sector",
            ));
        }
        if record.kind == "coordinate" {
            return Ok(face.into());
        }
        let equations = record
            .equations
            .iter()
            .map(|&id| self.polynomial(id))
            .collect::<Result<Vec<_>, _>>()?;
        match AffineCase::from_coordinate(&face, &equations, self.indices, sector)
            .map_err(|e| BundleWireError::native(&e, BundleWireErrorKind::Input))?
        {
            AffineIntersection::Affine(case) if case.face().fixed() == &fixed => Ok(case.into()),
            _ => Err(BundleWireError::input(
                "saved affine case does not reconstruct its declared fixed face",
            )),
        }
    }

    /// The rule of `record` in `sector`, with the default dispatch policy;
    /// storage padding slots are fixed at zero.
    pub fn rule(
        &self,
        record: &RuleRecord,
        sector: &[bool; N],
    ) -> Result<SectorRule<N>, BundleWireError> {
        validate_rule(record, record.target.values.len())?;
        if !crate::fits_storage(record.target.values.len(), N) {
            return Err(BundleWireError::input(
                "candidate rule exceeds storage capacity",
            ));
        }
        validate_rule_ids(record, self.table.coefficient_count())?;
        let mut padded_case = record.case.clone();
        for axis in record.target.values.len()..N {
            padded_case.fixed_axes.push(axis);
            padded_case.fixed_values.push(0);
        }
        let case = self.case(&padded_case, sector)?;
        let target = integral(&record.target)?;
        if target != case.integral() {
            return Err(BundleWireError::input(
                "candidate target is not canonical for its case",
            ));
        }
        let rhs = record
            .rhs
            .iter()
            .map(|term| {
                Ok(Term {
                    integral: integral(&term.integral)?,
                    coefficient: self.coefficient(term.coefficient)?,
                })
            })
            .collect::<Result<_, BundleWireError>>()?;
        let sources = record
            .sources
            .iter()
            .map(|source| {
                Ok(SeedSource {
                    basis_row: source.basis_row,
                    seed: Seed {
                        integral: integral(&source.integral)?,
                        shifts: crate::storage_array(&source.shifts, 0)
                            .expect("validated seed arity"),
                    },
                })
            })
            .collect::<Result<_, BundleWireError>>()?;
        let branches = record
            .exclusions
            .iter()
            .map(|branch| {
                branch
                    .iter()
                    .map(|&id| self.polynomial(id))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<_, BundleWireError>>()?;
        Ok(SectorRule {
            dispatch_policy: Default::default(),
            candidate: RuleCandidate {
                case,
                target,
                rhs,
                sources,
                stats: SearchStats::default(),
                hidden_poles: Vec::new(),
            },
            exceptions: ExceptionalConditions { branches },
        })
    }

    /// The saved solution of sector `ordinal` of `program`: its rules with
    /// their dispatch policies, its residuals, and its order under the
    /// program's `ordering` restricted to `physical_arity` slots. The solution
    /// carries the generation policy `(max_numerator_rank, finite_case_policy)`.
    pub fn sector(
        &self,
        program: &ProgramRecord,
        ordinal: usize,
        ordering: &OrderingPolicy,
        physical_arity: usize,
        max_numerator_rank: Option<u32>,
        finite_case_policy: FiniteCasePolicy,
    ) -> Result<([bool; N], SectorSolution<N>), BundleWireError> {
        let record = program
            .sectors
            .get(ordinal)
            .ok_or_else(|| BundleWireError::input("no candidate sector of this ordinal"))?;
        let sector: [bool; N] = crate::storage_array(&record.sector, false)
            .ok_or_else(|| BundleWireError::input("candidate reconstruction arity"))?;
        let rules = (record.rules.iter().enumerate())
            .map(|(rule_ordinal, record)| {
                let mut rule = self.rule(record, &sector)?;
                rule.dispatch_policy = program.dispatch_policy(ordinal, rule_ordinal);
                Ok(rule)
            })
            .collect::<Result<Vec<_>, BundleWireError>>()?;
        let finite_residuals = (record.finite_residuals.iter())
            .map(integral)
            .collect::<Result<Vec<_>, _>>()?;
        let order = IntegralOrder::from_persisted_policy(sector, ordering)
            .and_then(|order| order.with_physical_arity(physical_arity))
            .map_err(|error| BundleWireError::native(&error, BundleWireErrorKind::Input))?;
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
    }
}
