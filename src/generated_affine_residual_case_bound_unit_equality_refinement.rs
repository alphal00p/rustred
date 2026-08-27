//! Authority-bound refinement of one generated-affine equality boundary.
//!
//! The underlying unit-pivot compiler is deliberately source-neutral. This
//! topology-neutral bridge retains the typed premises predecessor, replays its
//! exact source authority, resolves ordered `EqualZero` predicates by borrow,
//! and binds every unit-compiler classification back to that predecessor.
//!
//! Every outcome retains the original equality certificate. `ProvedEmpty` is
//! diagnostic only: this owner cannot prune, publish a rule, or infer a master.
//! Admission of this owner into a long-lived publication resident, including
//! its full transitive retained-byte charge, is the next residence gate.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use crate::generated_affine_residual_case_inventory::{
    GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityError,
    GeneratedAffineResidualCaseSourceView,
};
use crate::generated_affine_residual_case_premises::{
    GeneratedAffineResidualCaseEqualityRefinementCertificate,
    GeneratedAffineResidualCasePremisesError,
};
use crate::generated_affine_residual_case_unit_equality_refinement::{
    GeneratedAffineResidualCaseUnitEqualityRefinementCertificate,
    GeneratedAffineResidualCaseUnitEqualityRefinementError,
    GeneratedAffineResidualCaseUnitEqualityRefinementLimits,
    GeneratedAffineResidualCaseUnitEqualityRefinementOutcome,
    GeneratedAffineResidualCaseUnitEqualityRefinementUnsupported,
    compile_generated_affine_residual_case_unit_equality_refinement_with_borrowed_predicates,
};
use crate::parametric_coefficient::ResidualAffineCompactMapView;
use crate::{
    IntegralFamily, ParametricCoefficientContext, ParametricPolynomial,
    SymbolicPolynomialPredicateKind,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_CASE_BOUND_UNIT_EQUALITY_REFINEMENT_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-case-bound-unit-equality-refinement-v1";

/// Exact adapter-local work. Nested premises/unit certificates retain their
/// own algebra and resource statistics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseBoundUnitEqualityRefinementStats {
    premises_replays: usize,
    source_case_authentications: usize,
    source_group_authentications: usize,
    equality_predicate_resolutions: usize,
    /// One compilation during construction or one retained-certificate replay.
    unit_refinement_actions: usize,
}

impl GeneratedAffineResidualCaseBoundUnitEqualityRefinementStats {
    pub(crate) const fn premises_replays(self) -> usize {
        self.premises_replays
    }

    pub(crate) const fn source_case_authentications(self) -> usize {
        self.source_case_authentications
    }

    pub(crate) const fn source_group_authentications(self) -> usize {
        self.source_group_authentications
    }

    pub(crate) const fn equality_predicate_resolutions(self) -> usize {
        self.equality_predicate_resolutions
    }

    pub(crate) const fn unit_refinement_actions(self) -> usize {
        self.unit_refinement_actions
    }
}

#[derive(Debug)]
pub(crate) enum GeneratedAffineResidualCaseBoundUnitEqualityRefinementError {
    SchemaMismatch,
    WrongCaseBinding,
    WrongGroupBinding,
    MalformedGeometry,
    SourcePredicateBinding,
    ResourceCountOverflow { resource: &'static str },
    ReplayMismatch,
    Premises(GeneratedAffineResidualCasePremisesError),
    Authority(GeneratedAffineResidualCaseAuthorityError),
    Unit(GeneratedAffineResidualCaseUnitEqualityRefinementError),
    SymbolicaPanic,
}

impl fmt::Display for GeneratedAffineResidualCaseBoundUnitEqualityRefinementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("bound unit-equality refinement schema mismatch")
            }
            Self::WrongCaseBinding => {
                formatter.write_str("bound unit-equality refinement case binding mismatch")
            }
            Self::WrongGroupBinding => {
                formatter.write_str("bound unit-equality refinement group binding mismatch")
            }
            Self::MalformedGeometry => {
                formatter.write_str("bound unit-equality refinement geometry is malformed")
            }
            Self::SourcePredicateBinding => {
                formatter.write_str("bound unit-equality refinement predicate binding mismatch")
            }
            Self::ResourceCountOverflow { resource } => write!(
                formatter,
                "bound unit-equality refinement {resource} count overflowed usize"
            ),
            Self::ReplayMismatch => {
                formatter.write_str("bound unit-equality refinement did not replay")
            }
            Self::Premises(error) => error.fmt(formatter),
            Self::Authority(error) => error.fmt(formatter),
            Self::Unit(error) => error.fmt(formatter),
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked inside bound unit-equality refinement")
            }
        }
    }
}

impl std::error::Error for GeneratedAffineResidualCaseBoundUnitEqualityRefinementError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Premises(error) => Some(error),
            Self::Authority(error) => Some(error),
            Self::Unit(error) => Some(error),
            _ => None,
        }
    }
}

impl From<GeneratedAffineResidualCasePremisesError>
    for GeneratedAffineResidualCaseBoundUnitEqualityRefinementError
{
    fn from(value: GeneratedAffineResidualCasePremisesError) -> Self {
        Self::Premises(value)
    }
}

impl From<GeneratedAffineResidualCaseAuthorityError>
    for GeneratedAffineResidualCaseBoundUnitEqualityRefinementError
{
    fn from(value: GeneratedAffineResidualCaseAuthorityError) -> Self {
        Self::Authority(value)
    }
}

impl From<GeneratedAffineResidualCaseUnitEqualityRefinementError>
    for GeneratedAffineResidualCaseBoundUnitEqualityRefinementError
{
    fn from(value: GeneratedAffineResidualCaseUnitEqualityRefinementError) -> Self {
        Self::Unit(value)
    }
}

/// Move-only failed attempt that returns ownership of the exact predecessor.
pub(crate) struct GeneratedAffineResidualCaseBoundUnitEqualityRefinementFailure {
    error: GeneratedAffineResidualCaseBoundUnitEqualityRefinementError,
    equality: GeneratedAffineResidualCaseEqualityRefinementCertificate,
}

impl GeneratedAffineResidualCaseBoundUnitEqualityRefinementFailure {
    pub(crate) const fn error(
        &self,
    ) -> &GeneratedAffineResidualCaseBoundUnitEqualityRefinementError {
        &self.error
    }

    pub(crate) fn into_equality(self) -> GeneratedAffineResidualCaseEqualityRefinementCertificate {
        self.equality
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseBoundUnitEqualityRefinementFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseBoundUnitEqualityRefinementFailure")
            .field("error", &self.error)
            .field("private_equality", &"<redacted>")
            .finish()
    }
}

struct BoundInput {
    schema: &'static str,
    equality: GeneratedAffineResidualCaseEqualityRefinementCertificate,
    limits: GeneratedAffineResidualCaseUnitEqualityRefinementLimits,
    stats: GeneratedAffineResidualCaseBoundUnitEqualityRefinementStats,
}

impl BoundInput {
    fn authority(&self) -> &Arc<GeneratedAffineResidualCaseAuthority> {
        self.equality.bound_unit_equality_refinement_authority()
    }

    fn replay_classification(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<
        GeneratedAffineResidualCaseUnitEqualityRefinementOutcome,
        GeneratedAffineResidualCaseBoundUnitEqualityRefinementError,
    > {
        if self.schema != GENERATED_AFFINE_RESIDUAL_CASE_BOUND_UNIT_EQUALITY_REFINEMENT_V1_SCHEMA {
            return Err(
                GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::SchemaMismatch,
            );
        }
        let (classification, stats) =
            classify_bound_input(family, context, &self.equality, self.limits)?;
        if stats != self.stats {
            return Err(
                GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::ReplayMismatch,
            );
        }
        Ok(classification)
    }
}

impl fmt::Debug for BoundInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoundInput")
            .field("schema", &self.schema)
            .field("case_ordinal", &self.equality.case_ordinal())
            .field("group_ordinal", &self.equality.group_ordinal())
            .field(
                "equality_predicate_count",
                &self.equality.equality_predicate_ordinals().len(),
            )
            .field("stats", &self.stats)
            .field("private_equality", &"<redacted>")
            .finish()
    }
}

/// Source-bound successful literal-unit refinement.
pub(crate) struct GeneratedAffineResidualCaseBoundUnitEqualityRefinementCertificate {
    input: BoundInput,
    unit: GeneratedAffineResidualCaseUnitEqualityRefinementCertificate,
}

impl GeneratedAffineResidualCaseBoundUnitEqualityRefinementCertificate {
    pub(crate) fn authority(&self) -> &Arc<GeneratedAffineResidualCaseAuthority> {
        self.input.authority()
    }

    pub(crate) fn equality_refinement(
        &self,
    ) -> &GeneratedAffineResidualCaseEqualityRefinementCertificate {
        &self.input.equality
    }

    pub(crate) const fn unit_refinement(
        &self,
    ) -> &GeneratedAffineResidualCaseUnitEqualityRefinementCertificate {
        &self.unit
    }

    pub(crate) const fn stats(
        &self,
    ) -> GeneratedAffineResidualCaseBoundUnitEqualityRefinementStats {
        self.input.stats
    }

    pub(crate) const fn limits(&self) -> GeneratedAffineResidualCaseUnitEqualityRefinementLimits {
        self.input.limits
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualCaseBoundUnitEqualityRefinementError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.input.schema
                != GENERATED_AFFINE_RESIDUAL_CASE_BOUND_UNIT_EQUALITY_REFINEMENT_V1_SCHEMA
            {
                return Err(
                    GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::SchemaMismatch,
                );
            }
            let authenticated = authenticate_bound_input(family, context, &self.input.equality)?;
            let [source_ordinal] = self.input.equality.equality_predicate_ordinals() else {
                return Err(
                    GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::ReplayMismatch,
                );
            };
            let polynomial = resolve_equal_zero_predicate(authenticated.source, *source_ordinal)?;
            // Both operands were authenticated under the retained atom-row
            // shape/term/exponent/bit limits. The next long-lived residence
            // gate must still include this exact payload scan in its aggregate
            // replay-work and peak-memory census.
            if polynomial != self.unit.equality()
                || self.input.limits != self.unit.limits()
                || self.input.stats != wrapper_stats(1, 1)
            {
                return Err(
                    GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::ReplayMismatch,
                );
            }
            self.unit.replay(context, authenticated.parent_geometry)?;
            Ok(())
        }))
        .map_err(|_| GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::SymbolicaPanic)?
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseBoundUnitEqualityRefinementCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseBoundUnitEqualityRefinementCertificate")
            .field("input", &self.input)
            .field("unit_schema", &self.unit.schema())
            .field("pivot_free_ordinal", &self.unit.pivot_free_ordinal())
            .field(
                "pivot_ambient_position",
                &self.unit.pivot_ambient_position(),
            )
            .field("private_unit_certificate", &"<redacted>")
            .finish()
    }
}

pub(crate) struct GeneratedAffineResidualCaseBoundUnitEqualityAlreadySatisfied {
    input: BoundInput,
}

/// Diagnostic only; intentionally carries no pruning/publication capability.
pub(crate) struct GeneratedAffineResidualCaseBoundUnitEqualityProvedEmptyDiagnostic {
    input: BoundInput,
}

impl GeneratedAffineResidualCaseBoundUnitEqualityProvedEmptyDiagnostic {
    pub(crate) const fn is_branch_pruning_authority(&self) -> bool {
        false
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) const fn infers_master(&self) -> bool {
        false
    }
}

pub(crate) struct GeneratedAffineResidualCaseBoundUnitEqualityUnsupported {
    input: BoundInput,
    reason: GeneratedAffineResidualCaseUnitEqualityRefinementUnsupported,
}

impl GeneratedAffineResidualCaseBoundUnitEqualityUnsupported {
    pub(crate) const fn reason(
        &self,
    ) -> &GeneratedAffineResidualCaseUnitEqualityRefinementUnsupported {
        &self.reason
    }
}

pub(crate) enum GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome {
    Refined(GeneratedAffineResidualCaseBoundUnitEqualityRefinementCertificate),
    AlreadySatisfied(GeneratedAffineResidualCaseBoundUnitEqualityAlreadySatisfied),
    ProvedEmpty(GeneratedAffineResidualCaseBoundUnitEqualityProvedEmptyDiagnostic),
    Unsupported(GeneratedAffineResidualCaseBoundUnitEqualityUnsupported),
}

impl GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome {
    fn input(&self) -> &BoundInput {
        match self {
            Self::Refined(value) => &value.input,
            Self::AlreadySatisfied(value) => &value.input,
            Self::ProvedEmpty(value) => &value.input,
            Self::Unsupported(value) => &value.input,
        }
    }

    pub(crate) fn authority(&self) -> &Arc<GeneratedAffineResidualCaseAuthority> {
        self.input().authority()
    }

    pub(crate) fn equality_refinement(
        &self,
    ) -> &GeneratedAffineResidualCaseEqualityRefinementCertificate {
        &self.input().equality
    }

    pub(crate) const fn is_branch_pruning_authority(&self) -> bool {
        false
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) const fn infers_master(&self) -> bool {
        false
    }

    pub(crate) fn stats(&self) -> GeneratedAffineResidualCaseBoundUnitEqualityRefinementStats {
        self.input().stats
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualCaseBoundUnitEqualityRefinementError> {
        match self {
            Self::Refined(value) => value.replay(family, context),
            Self::AlreadySatisfied(value) => replay_nonrefined(
                &value.input,
                family,
                context,
                ExpectedNonrefined::AlreadySatisfied,
            ),
            Self::ProvedEmpty(value) => replay_nonrefined(
                &value.input,
                family,
                context,
                ExpectedNonrefined::ProvedEmpty,
            ),
            Self::Unsupported(value) => replay_nonrefined(
                &value.input,
                family,
                context,
                ExpectedNonrefined::Unsupported(&value.reason),
            ),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refined(value) => formatter.debug_tuple("Refined").field(value).finish(),
            Self::AlreadySatisfied(value) => formatter
                .debug_tuple("AlreadySatisfied")
                .field(&value.input)
                .finish(),
            Self::ProvedEmpty(value) => formatter
                .debug_tuple("ProvedEmptyDiagnostic")
                .field(&value.input)
                .finish(),
            Self::Unsupported(value) => formatter
                .debug_struct("Unsupported")
                .field("reason", &value.reason)
                .field("input", &value.input)
                .finish(),
        }
    }
}

pub(crate) struct GeneratedAffineResidualCaseBoundUnitEqualityRefinementCompiler;

impl GeneratedAffineResidualCaseBoundUnitEqualityRefinementCompiler {
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        equality: GeneratedAffineResidualCaseEqualityRefinementCertificate,
        limits: GeneratedAffineResidualCaseUnitEqualityRefinementLimits,
    ) -> Result<
        GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome,
        GeneratedAffineResidualCaseBoundUnitEqualityRefinementFailure,
    > {
        let attempt = catch_unwind(AssertUnwindSafe(|| {
            classify_bound_input(family, context, &equality, limits)
        }));
        match attempt {
            Ok(Ok((classification, stats))) => {
                Ok(bind_classification(equality, limits, stats, classification))
            }
            Ok(Err(error)) => Err(
                GeneratedAffineResidualCaseBoundUnitEqualityRefinementFailure { error, equality },
            ),
            Err(_) => Err(
                GeneratedAffineResidualCaseBoundUnitEqualityRefinementFailure {
                    error:
                        GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::SymbolicaPanic,
                    equality,
                },
            ),
        }
    }
}

fn bind_classification(
    equality: GeneratedAffineResidualCaseEqualityRefinementCertificate,
    limits: GeneratedAffineResidualCaseUnitEqualityRefinementLimits,
    stats: GeneratedAffineResidualCaseBoundUnitEqualityRefinementStats,
    classification: GeneratedAffineResidualCaseUnitEqualityRefinementOutcome,
) -> GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome {
    let input = BoundInput {
        schema: GENERATED_AFFINE_RESIDUAL_CASE_BOUND_UNIT_EQUALITY_REFINEMENT_V1_SCHEMA,
        equality,
        limits,
        stats,
    };
    match classification {
        GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::Refined(unit) => {
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::Refined(
                GeneratedAffineResidualCaseBoundUnitEqualityRefinementCertificate { input, unit },
            )
        }
        GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::AlreadySatisfied => {
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::AlreadySatisfied(
                GeneratedAffineResidualCaseBoundUnitEqualityAlreadySatisfied { input },
            )
        }
        GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::ProvedEmpty => {
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::ProvedEmpty(
                GeneratedAffineResidualCaseBoundUnitEqualityProvedEmptyDiagnostic { input },
            )
        }
        GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::Unsupported(reason) => {
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::Unsupported(
                GeneratedAffineResidualCaseBoundUnitEqualityUnsupported { input, reason },
            )
        }
    }
}

struct AuthenticatedBoundInput<'source> {
    parent_geometry: ResidualAffineCompactMapView<'source>,
    source: GeneratedAffineResidualCaseSourceView<'source>,
}

fn authenticate_bound_input<'source>(
    family: &IntegralFamily,
    context: &'source ParametricCoefficientContext,
    equality: &'source GeneratedAffineResidualCaseEqualityRefinementCertificate,
) -> Result<
    AuthenticatedBoundInput<'source>,
    GeneratedAffineResidualCaseBoundUnitEqualityRefinementError,
> {
    let authority = equality.bound_unit_equality_refinement_authority();
    equality.replay(family, context, authority)?;
    let case = authority.authenticated_source_neutral_case_view(context)?;
    let group = authority.authenticated_source_neutral_group_view(context)?;
    if equality.case_ordinal() != authority.case_ordinal()
        || equality.case_ordinal() != case.ordinal()
    {
        return Err(GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::WrongCaseBinding);
    }
    if equality.group_ordinal() != authority.group_ordinal()
        || equality.group_ordinal() != case.group_ordinal()
        || equality.group_ordinal() != group.ordinal()
    {
        return Err(GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::WrongGroupBinding);
    }
    let matrix_entries = group
        .ambient_arity()
        .checked_mul(group.free_positions().len())
        .ok_or(GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::MalformedGeometry)?;
    if group.ambient_arity() != context.index_count()
        || case.constants().len() != group.ambient_arity()
        || group.compact_linear_coefficients().len() != matrix_entries
    {
        return Err(GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::MalformedGeometry);
    }
    Ok(AuthenticatedBoundInput {
        parent_geometry: ResidualAffineCompactMapView::new(
            context.fingerprint(),
            group.ambient_arity(),
            case.constants(),
            group.free_positions(),
            group.compact_linear_coefficients(),
        ),
        source: case.source(),
    })
}

fn resolve_equal_zero_predicate(
    source: GeneratedAffineResidualCaseSourceView<'_>,
    source_ordinal: usize,
) -> Result<&ParametricPolynomial, GeneratedAffineResidualCaseBoundUnitEqualityRefinementError> {
    let predicate = source.exceptional_predicate(source_ordinal).ok_or(
        GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::SourcePredicateBinding,
    )?;
    if predicate.predicate_ordinal() != source_ordinal
        || predicate.kind() != SymbolicPolynomialPredicateKind::EqualZero
    {
        return Err(
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::SourcePredicateBinding,
        );
    }
    Ok(predicate.polynomial())
}

fn classify_bound_input(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    equality: &GeneratedAffineResidualCaseEqualityRefinementCertificate,
    limits: GeneratedAffineResidualCaseUnitEqualityRefinementLimits,
) -> Result<
    (
        GeneratedAffineResidualCaseUnitEqualityRefinementOutcome,
        GeneratedAffineResidualCaseBoundUnitEqualityRefinementStats,
    ),
    GeneratedAffineResidualCaseBoundUnitEqualityRefinementError,
> {
    let authenticated = authenticate_bound_input(family, context, equality)?;
    let equality_predicate_ordinals = equality.equality_predicate_ordinals();
    let source = authenticated.source;
    let mut equality_predicate_resolutions = 0usize;
    let mut source_binding_failed = false;
    let mut resolution_count_overflowed = false;
    let classification =
        compile_generated_affine_residual_case_unit_equality_refinement_with_borrowed_predicates(
            context,
            authenticated.parent_geometry,
            equality_predicate_ordinals.len(),
            |equality_ordinal| {
                let Some(next_count) = equality_predicate_resolutions.checked_add(1) else {
                    resolution_count_overflowed = true;
                    return None;
                };
                equality_predicate_resolutions = next_count;
                let Some(&source_ordinal) = equality_predicate_ordinals.get(equality_ordinal)
                else {
                    source_binding_failed = true;
                    return None;
                };
                match resolve_equal_zero_predicate(source, source_ordinal) {
                    Ok(polynomial) => Some(polynomial),
                    Err(_) => {
                        source_binding_failed = true;
                        None
                    }
                }
            },
            limits,
        );
    if resolution_count_overflowed {
        return Err(
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::ResourceCountOverflow {
                resource: "equality predicate resolutions",
            },
        );
    }
    if source_binding_failed {
        return Err(
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::SourcePredicateBinding,
        );
    }
    let classification = classification?;
    if equality_predicate_resolutions != equality_predicate_ordinals.len() {
        return Err(
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::SourcePredicateBinding,
        );
    }
    Ok((
        classification,
        wrapper_stats(equality_predicate_resolutions, 1),
    ))
}

const fn wrapper_stats(
    equality_predicate_resolutions: usize,
    unit_refinement_actions: usize,
) -> GeneratedAffineResidualCaseBoundUnitEqualityRefinementStats {
    GeneratedAffineResidualCaseBoundUnitEqualityRefinementStats {
        premises_replays: 1,
        source_case_authentications: 1,
        source_group_authentications: 1,
        equality_predicate_resolutions,
        unit_refinement_actions,
    }
}

enum ExpectedNonrefined<'reason> {
    AlreadySatisfied,
    ProvedEmpty,
    Unsupported(&'reason GeneratedAffineResidualCaseUnitEqualityRefinementUnsupported),
}

fn replay_nonrefined(
    input: &BoundInput,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    expected: ExpectedNonrefined<'_>,
) -> Result<(), GeneratedAffineResidualCaseBoundUnitEqualityRefinementError> {
    catch_unwind(AssertUnwindSafe(|| {
        let replayed = input.replay_classification(family, context)?;
        let matches = match (expected, replayed) {
            (
                ExpectedNonrefined::AlreadySatisfied,
                GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::AlreadySatisfied,
            )
            | (
                ExpectedNonrefined::ProvedEmpty,
                GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::ProvedEmpty,
            ) => true,
            (
                ExpectedNonrefined::Unsupported(expected),
                GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::Unsupported(actual),
            ) => expected == &actual,
            _ => false,
        };
        if matches {
            Ok(())
        } else {
            Err(GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::ReplayMismatch)
        }
    }))
    .map_err(|_| GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::SymbolicaPanic)?
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::*;
    use crate::generated_affine_residual_boolean_cover::{
        GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
    };
    use crate::generated_affine_residual_case_inventory::{
        GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCertificate,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
    };
    use crate::generated_affine_residual_case_premises::{
        GeneratedAffineResidualCasePremisesLimits, GeneratedAffineResidualCasePremisesOutcome,
        compile_generated_affine_residual_case_premises,
    };
    use crate::generated_affine_residual_case_unit_equality_refinement::compile_generated_affine_residual_case_unit_equality_refinement;
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::generated_sector_affine_effective_coverage::{
        GeneratedSectorAffineEffectiveCoverageCompiler,
        GeneratedSectorAffineEffectiveCoverageConfig, GeneratedSectorAffineEffectiveCoverageLimits,
    };
    use crate::generated_sector_affine_effective_residual_queue::{
        GeneratedSectorAffineEffectiveResidualQueueCompiler,
        GeneratedSectorAffineEffectiveResidualQueueLimits,
    };
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedResidualAffineCaseInventoryCompiler,
        GeneratedResidualAffineCaseInventoryLimits, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, IntegralOrderingPolicy, ParametricIbpGenerator,
        SectorMask,
    };

    fn equal_mass_two_loop_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap()
    }

    fn prior_fixture() -> &'static (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualCaseInventoryCertificate>,
    ) {
        static FIXTURE: OnceLock<(
            IntegralFamily,
            ParametricCoefficientContext,
            Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        )> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let family = equal_mass_two_loop_family("bound-unit-shared-generic-prior");
            let context = ParametricIbpGenerator::try_new(&family)
                .unwrap()
                .context()
                .clone();
            let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
            discovery_limits.adaptive.max_search_depth = 0;
            let discovery = GeneratedSectorDiscoveryCompiler::compile(
                &family,
                &context,
                SectorMask::try_from_bit_string("001").unwrap(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                discovery_limits,
            )
            .unwrap();
            let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
            queue_limits.translation_radius = 0;
            queue_limits.max_translation_points = 1;
            let queue = Arc::new(
                GeneratedSectorLiveLeafQueueCompiler::compile(
                    &family,
                    &context,
                    &discovery,
                    queue_limits,
                )
                .unwrap(),
            );
            let old_inventory = Arc::new(
                GeneratedResidualAffineCaseInventoryCompiler::compile(
                    &family,
                    &context,
                    queue,
                    GeneratedResidualAffineCaseInventoryLimits::default(),
                )
                .unwrap(),
            );
            let effective = Arc::new(
                GeneratedSectorAffineEffectiveCoverageCompiler::compile(
                    &family,
                    &context,
                    old_inventory,
                    GeneratedSectorAffineEffectiveCoverageConfig::new(0),
                    GeneratedSectorAffineEffectiveCoverageLimits::default(),
                )
                .unwrap(),
            );
            let prior_queue = Arc::new(
                GeneratedSectorAffineEffectiveResidualQueueCompiler::compile(
                    &family,
                    &context,
                    effective,
                    GeneratedSectorAffineEffectiveResidualQueueLimits::default(),
                )
                .unwrap(),
            );
            let source = GeneratedAffineResidualSourceAuthority::prior_effective(prior_queue);
            let boolean = Arc::new(
                GeneratedAffineResidualBooleanCoverCompiler::compile(
                    &family,
                    &context,
                    source,
                    GeneratedAffineResidualBooleanCoverLimits::default(),
                )
                .unwrap(),
            );
            let inventory = Arc::new(
                GeneratedAffineResidualCaseInventoryCompiler::compile(
                    &family,
                    &context,
                    boolean,
                    GeneratedAffineResidualCaseInventoryLimits::default(),
                )
                .unwrap(),
            );
            (family, context, inventory)
        })
    }

    fn authority(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        inventory: &Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        case_ordinal: usize,
    ) -> Arc<GeneratedAffineResidualCaseAuthority> {
        Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                family,
                context,
                Arc::clone(inventory),
                case_ordinal,
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        )
    }

    fn equality(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> GeneratedAffineResidualCaseEqualityRefinementCertificate {
        match compile_generated_affine_residual_case_premises(
            family,
            context,
            authority,
            GeneratedAffineResidualCasePremisesLimits::default(),
        )
        .unwrap()
        {
            GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(value) => {
                value
            }
            GeneratedAffineResidualCasePremisesOutcome::Ready(_) => {
                panic!("generic prior fixture unexpectedly has no equality boundary")
            }
        }
    }

    fn input_mut(
        outcome: &mut GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome,
    ) -> &mut BoundInput {
        match outcome {
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::Refined(value) => {
                &mut value.input
            }
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::AlreadySatisfied(
                value,
            ) => &mut value.input,
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::ProvedEmpty(value) => {
                &mut value.input
            }
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::Unsupported(value) => {
                &mut value.input
            }
        }
    }

    #[test]
    fn generic_prior_equalities_bind_and_replay_with_explicit_outcome_census() {
        let (family, context, inventory) = prior_fixture();
        assert_eq!(inventory.case_count(), 3);
        let mut counts = [0usize; 4];
        let mut source_equal_zero_positions = Vec::new();
        for case_ordinal in 0..inventory.case_count() {
            let authority = authority(family, context, inventory, case_ordinal);
            let equality = equality(family, context, Arc::clone(&authority));
            let predicate_count = equality.equality_predicate_ordinals().len();
            let [source_ordinal] = equality.equality_predicate_ordinals() else {
                panic!("natural prior fixture must retain exactly one EqualZero predicate")
            };
            let source_case = authority
                .authenticated_source_neutral_case_view(context)
                .unwrap();
            let source_predicate = source_case
                .source()
                .exceptional_predicate(*source_ordinal)
                .unwrap();
            assert_eq!(source_predicate.predicate_ordinal(), *source_ordinal);
            assert_eq!(
                source_predicate.kind(),
                SymbolicPolynomialPredicateKind::EqualZero
            );
            source_equal_zero_positions.push((*source_ordinal, source_predicate.locus_ordinal()));
            let outcome = GeneratedAffineResidualCaseBoundUnitEqualityRefinementCompiler::compile(
                family,
                context,
                equality,
                GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
            )
            .unwrap();
            match &outcome {
                GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::Refined(refined) => {
                    assert_eq!(
                        source_predicate.polynomial(),
                        refined.unit_refinement().equality()
                    );
                    counts[0] += 1;
                }
                GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::AlreadySatisfied(
                    _,
                ) => counts[1] += 1,
                GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::ProvedEmpty(_) => {
                    counts[2] += 1
                }
                GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::Unsupported(_) => {
                    counts[3] += 1
                }
            }
            assert!(Arc::ptr_eq(outcome.authority(), &authority));
            assert_eq!(outcome.stats(), wrapper_stats(predicate_count, 1));
            assert!(!outcome.is_branch_pruning_authority());
            assert!(!outcome.publishes_rule());
            assert!(!outcome.infers_master());
            outcome.replay(family, context).unwrap();
            let debug = format!("{outcome:?}");
            assert!(debug.contains("<redacted>"));
            assert!(!debug.contains("m2"));
        }
        eprintln!(
            "natural bound-unit outcome census [refined, satisfied, empty, unsupported]: \
             {counts:?}; retained EqualZero (predicate, locus) positions: \
             {source_equal_zero_positions:?}"
        );
        assert_eq!(counts, [3, 0, 0, 0]);
    }

    #[test]
    fn failure_owners_return_replayable_predecessors() {
        let (family, context, inventory) = prior_fixture();
        let authority = authority(family, context, inventory, 0);
        let limited_equality = equality(family, context, Arc::clone(&authority));
        let mut limits = GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default();
        limits.max_equal_zero_predicates_inspected = 0;
        let failure = GeneratedAffineResidualCaseBoundUnitEqualityRefinementCompiler::compile(
            family,
            context,
            limited_equality,
            limits,
        )
        .unwrap_err();
        assert!(matches!(
            failure.error(),
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::Unit(
                GeneratedAffineResidualCaseUnitEqualityRefinementError::ResourceLimit {
                    resource: "equal-zero predicates inspected",
                    ..
                }
            )
        ));
        failure
            .into_equality()
            .replay(family, context, &authority)
            .unwrap();

        let mut equality = equality(family, context, Arc::clone(&authority));
        let original_ordinal = equality.equality_predicate_ordinals()[0];
        assert!(equality.replace_equality_predicate_ordinal_for_test(0, usize::MAX));
        let failure = GeneratedAffineResidualCaseBoundUnitEqualityRefinementCompiler::compile(
            family,
            context,
            equality,
            GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
        )
        .unwrap_err();
        assert!(matches!(
            failure.error(),
            GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::Premises(
                GeneratedAffineResidualCasePremisesError::EqualityPredicateMismatch
            )
        ));
        let mut recovered = failure.into_equality();
        assert!(recovered.replace_equality_predicate_ordinal_for_test(0, original_ordinal));
        recovered.replay(family, context, &authority).unwrap();
    }

    #[test]
    fn replay_rejects_foreign_scope_and_wrapper_stat_tamper() {
        let (family, context, inventory) = prior_fixture();
        let equality = equality(family, context, authority(family, context, inventory, 0));
        let mut outcome = GeneratedAffineResidualCaseBoundUnitEqualityRefinementCompiler::compile(
            family,
            context,
            equality,
            GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
        )
        .unwrap();
        let foreign_family = equal_mass_two_loop_family("bound-unit-foreign-family");
        let foreign_context = ParametricIbpGenerator::try_new(&foreign_family)
            .unwrap()
            .context()
            .clone();
        assert!(outcome.replay(&foreign_family, context).is_err());
        assert!(outcome.replay(family, &foreign_context).is_err());
        outcome.replay(family, context).unwrap();
        let original_limits = input_mut(&mut outcome).limits;
        input_mut(&mut outcome)
            .limits
            .max_equal_zero_predicates_inspected += 1;
        assert!(matches!(
            outcome.replay(family, context),
            Err(GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::ReplayMismatch)
        ));
        input_mut(&mut outcome).limits = original_limits;
        outcome.replay(family, context).unwrap();
        input_mut(&mut outcome).stats.premises_replays = 2;
        assert!(matches!(
            outcome.replay(family, context),
            Err(GeneratedAffineResidualCaseBoundUnitEqualityRefinementError::ReplayMismatch)
        ));
    }

    /// Exhaustive typed-wrapper mapping. Natural source-backed Refined
    /// classification/replay is covered above; this test isolates the
    /// allocation-free binding match for every typed outcome.
    #[test]
    fn binding_maps_all_unit_outcomes_without_losing_the_predecessor() {
        let (family, context, inventory) = prior_fixture();
        for branch in 0..4 {
            let authority = authority(family, context, inventory, branch % inventory.case_count());
            let equality = equality(family, context, Arc::clone(&authority));
            let case = authority
                .authenticated_source_neutral_case_view(context)
                .unwrap();
            let group = authority
                .authenticated_source_neutral_group_view(context)
                .unwrap();
            let parent_free_count = group.free_positions().len();
            let parent = ResidualAffineCompactMapView::new(
                context.fingerprint(),
                group.ambient_arity(),
                case.constants(),
                group.free_positions(),
                group.compact_linear_coefficients(),
            );
            let one = context.numerator_condition(&context.one()).unwrap();
            let classification = match branch {
                0 => compile_generated_affine_residual_case_unit_equality_refinement(
                    context,
                    parent,
                    &[],
                    GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
                )
                .unwrap(),
                1 => compile_generated_affine_residual_case_unit_equality_refinement(
                    context,
                    parent,
                    &[one.clone()],
                    GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
                )
                .unwrap(),
                2 => compile_generated_affine_residual_case_unit_equality_refinement(
                    context,
                    parent,
                    &[one.clone(), one],
                    GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
                )
                .unwrap(),
                3 => {
                    let free_position = *group.free_positions().first().unwrap();
                    let affine = context
                        .numerator_condition(&context.index(free_position).unwrap())
                        .unwrap();
                    compile_generated_affine_residual_case_unit_equality_refinement(
                        context,
                        parent,
                        &[affine],
                        GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
                    )
                    .unwrap()
                }
                _ => unreachable!(),
            };
            let outcome = bind_classification(
                equality,
                GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
                GeneratedAffineResidualCaseBoundUnitEqualityRefinementStats::default(),
                classification,
            );
            assert!(Arc::ptr_eq(outcome.authority(), &authority));
            match (branch, outcome) {
                (
                    0,
                    GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::AlreadySatisfied(
                        _,
                    ),
                ) => {}
                (
                    1,
                    GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::ProvedEmpty(
                        diagnostic,
                    ),
                ) => {
                    assert!(!diagnostic.is_branch_pruning_authority());
                    assert!(!diagnostic.publishes_rule());
                    assert!(!diagnostic.infers_master());
                }
                (
                    2,
                    GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::Unsupported(
                        unsupported,
                    ),
                ) => assert!(matches!(
                    unsupported.reason(),
                    GeneratedAffineResidualCaseUnitEqualityRefinementUnsupported::MultipleEqualZeroPredicates {
                        actual: 2
                    }
                )),
                (
                    3,
                    GeneratedAffineResidualCaseBoundUnitEqualityRefinementOutcome::Refined(
                        refined,
                    ),
                ) => assert_eq!(
                    refined.unit_refinement().child_geometry().free_positions().len() + 1,
                    parent_free_count
                ),
                (branch, outcome) => panic!("branch {branch} wrapped as {outcome:?}"),
            }
        }
    }
}
