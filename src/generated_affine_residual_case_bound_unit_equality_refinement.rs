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
use crate::solver::closure::case_inventory::{
    GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityError,
    GeneratedAffineResidualCaseSourceView,
};
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
