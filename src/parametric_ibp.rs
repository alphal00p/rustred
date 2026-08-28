//! Fully parametric integration-by-parts and Lorentz-invariance identities.
//!
//! The implementation follows LiteRed's `GenerateIBP` convention.  It emits
//! reusable relations in symbolic integral indices, never concrete seed
//! equations, and applies no sector, symmetry, or zero-sector rewriting.

use std::fmt;
use std::sync::Arc;

use crate::algebra::{IndexedAlgebraError, IndexedCoefficient, IndexedCoefficientContext};
use crate::family::{
    CoefficientLocation, ContractionMomentum, IntegralFamily, IntegralFamilyError,
    ScalarProductCoordinate,
};
use crate::identity::{
    IdentityConditionError, IdentityConditionSource, ParametricNonZeroCondition, RowId,
};
use crate::parametric_relation::{
    IndexShift, IndexSpace, ParametricRelation, ParametricRelationError, RelationLimits,
};

/// Resource policy for coefficient translations used while constructing LI
/// identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ParametricIbpConfig {
    pub relation_limits: RelationLimits,
}

/// Typed failures from generic parametric IBP/LI generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricIbpError {
    BaseContextMismatch,
    WrongIndexArity {
        expected: usize,
        actual: usize,
    },
    RowCountOverflow {
        loops: usize,
        externals: usize,
    },
    RowOrdinalOutOfRange {
        batch: &'static str,
        ordinal: usize,
        rows: usize,
    },
    WrongSourceRowCount {
        batch: &'static str,
        expected: usize,
        actual: usize,
    },
    SourceRowLayoutMismatch {
        position: usize,
        expected: &'static str,
        actual: &'static str,
    },
    SourceRowScopeMismatch {
        batch: &'static str,
        position: usize,
    },
    SourceRowOrdinalMismatch {
        batch: &'static str,
        position: usize,
        actual: usize,
    },
    CompletedSourceScopeMismatch,
    IdentityCondition(IdentityConditionError),
    Coefficient(IndexedAlgebraError),
    Relation(ParametricRelationError),
    Family(IntegralFamilyError),
}

impl fmt::Display for ParametricIbpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseContextMismatch => formatter.write_str(
                "the indexed coefficient context does not extend the family's exact base map",
            ),
            Self::WrongIndexArity { expected, actual } => write!(
                formatter,
                "the indexed coefficient context has {actual} indices, expected {expected}"
            ),
            Self::RowCountOverflow { loops, externals } => write!(
                formatter,
                "the IBP/LI row count for {loops} loops and {externals} external momenta overflowed usize"
            ),
            Self::RowOrdinalOutOfRange {
                batch,
                ordinal,
                rows,
            } => write!(
                formatter,
                "{batch} row ordinal {ordinal} is outside the prepared row count {rows}"
            ),
            Self::WrongSourceRowCount {
                batch,
                expected,
                actual,
            } => write!(
                formatter,
                "{batch} completion received {actual} rows, expected {expected}"
            ),
            Self::SourceRowLayoutMismatch {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "source row at completion position {position} uses {actual} layout, expected {expected}"
            ),
            Self::SourceRowScopeMismatch { batch, position } => write!(
                formatter,
                "{batch} row at completion position {position} has a foreign semantic source scope"
            ),
            Self::SourceRowOrdinalMismatch {
                batch,
                position,
                actual,
            } => write!(
                formatter,
                "{batch} completion position {position} received row ordinal {actual}"
            ),
            Self::CompletedSourceScopeMismatch => formatter
                .write_str("completed IBP source rows use a foreign family or coefficient context"),
            Self::IdentityCondition(error) => error.fmt(formatter),
            Self::Coefficient(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Family(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParametricIbpError {}

impl From<IndexedAlgebraError> for ParametricIbpError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::Coefficient(value)
    }
}

impl From<IdentityConditionError> for ParametricIbpError {
    fn from(value: IdentityConditionError) -> Self {
        Self::IdentityCondition(value)
    }
}

impl From<ParametricRelationError> for ParametricIbpError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl From<IntegralFamilyError> for ParametricIbpError {
    fn from(value: IntegralFamilyError) -> Self {
        Self::Family(value)
    }
}

/// Generated relations with their exact authenticated `K(n)` context.
#[derive(Clone, Debug)]
pub struct ParametricIbpRelations {
    family_fingerprint: Arc<str>,
    context: IndexedCoefficientContext,
    ordinary_ibp: Vec<ParametricRelation>,
    lorentz_invariance: Vec<ParametricRelation>,
}

impl ParametricIbpRelations {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn context(&self) -> &IndexedCoefficientContext {
        &self.context
    }

    /// The `L*(L+E)` ordinary rows in contraction-major, loop-minor order.
    pub fn ordinary_ibp(&self) -> &[ParametricRelation] {
        &self.ordinary_ibp
    }

    /// The `E*(E-1)/2` LI rows in lexicographic external-pair order.
    pub fn lorentz_invariance(&self) -> &[ParametricRelation] {
        &self.lorentz_invariance
    }

    /// LiteRed's `IBPLI` order: all ordinary rows followed by all LI rows.
    pub fn ibp_li(&self) -> impl Iterator<Item = &ParametricRelation> {
        self.ordinary_ibp
            .iter()
            .chain(self.lorentz_invariance.iter())
    }

    pub fn into_parts(
        self,
    ) -> (
        IndexedCoefficientContext,
        Vec<ParametricRelation>,
        Vec<ParametricRelation>,
    ) {
        (self.context, self.ordinary_ibp, self.lorentz_invariance)
    }
}

/// A topology- and loop-count-independent generator for one complete family.
#[derive(Clone, Debug)]
pub struct ParametricIbpGenerator<'family> {
    family: &'family IntegralFamily,
    source_scope: IbpSourceScope,
    context: IndexedCoefficientContext,
    index_space: IndexSpace,
    positive_units: Vec<IndexShift>,
    negative_units: Vec<IndexShift>,
    config: ParametricIbpConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IbpSourceLayout {
    CompleteOrdinary,
    ExternalOnly,
}

#[derive(Debug)]
enum PreparedIbpSource {
    CompleteOrdinary { dimension: IndexedCoefficient },
    ExternalOnly,
}

impl PreparedIbpSource {
    const fn layout(&self) -> IbpSourceLayout {
        match self {
            Self::CompleteOrdinary { .. } => IbpSourceLayout::CompleteOrdinary,
            Self::ExternalOnly => IbpSourceLayout::ExternalOnly,
        }
    }
}

impl IbpSourceLayout {
    const fn name(self) -> &'static str {
        match self {
            Self::CompleteOrdinary => "ordinary IBP source",
            Self::ExternalOnly => "external-contraction IBP source",
        }
    }

    const fn source_offset(self, loops: usize) -> Option<usize> {
        match self {
            Self::CompleteOrdinary => loops.checked_mul(loops),
            Self::ExternalOnly => Some(0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct IbpSourceScope {
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
}

/// Immutable ordinary or external-only IBP source work prepared once for
/// deterministic ordinal execution by an application-owned executor.
#[derive(Debug)]
pub struct PreparedIbpSourceBatch<'generator, 'family> {
    generator: &'generator ParametricIbpGenerator<'family>,
    scope: IbpSourceScope,
    source: PreparedIbpSource,
    powers: Vec<IndexedCoefficient>,
    rows: usize,
}

/// One sealed generated source row. Only a prepared batch can construct it;
/// completion validates its semantic scope, layout, and stable ordinal.
#[derive(Debug)]
pub struct GeneratedIbpSourceRow {
    scope: IbpSourceScope,
    layout: IbpSourceLayout,
    ordinal: usize,
    relation: ParametricRelation,
}

/// A single validated ordered IBP source barrier accepted by LI preparation.
#[derive(Debug)]
pub struct CompletedIbpSourceRows {
    scope: IbpSourceScope,
    layout: IbpSourceLayout,
    relations: Vec<ParametricRelation>,
}

impl CompletedIbpSourceRows {
    pub fn len(&self) -> usize {
        self.relations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.relations.is_empty()
    }

    pub fn into_relations(self) -> Vec<ParametricRelation> {
        self.relations
    }
}

impl PreparedIbpSourceBatch<'_, '_> {
    pub const fn len(&self) -> usize {
        self.rows
    }

    pub const fn is_empty(&self) -> bool {
        self.rows == 0
    }

    /// Generate one row at its stable layout-specific ordinal.
    pub fn generate(&self, ordinal: usize) -> Result<GeneratedIbpSourceRow, ParametricIbpError> {
        let layout = self.source.layout();
        if ordinal >= self.rows {
            return Err(ParametricIbpError::RowOrdinalOutOfRange {
                batch: layout.name(),
                ordinal,
                rows: self.rows,
            });
        }
        let relation = match &self.source {
            PreparedIbpSource::CompleteOrdinary { dimension } => self
                .generator
                .generate_ordinary_row(ordinal, dimension, &self.powers)?,
            PreparedIbpSource::ExternalOnly => self
                .generator
                .generate_external_source_row(ordinal, &self.powers)?,
        };
        Ok(GeneratedIbpSourceRow {
            scope: self.scope.clone(),
            layout,
            ordinal,
            relation,
        })
    }

    /// Validate one concrete ordered execution transcript and seal its source
    /// relations for LI preparation. A real `Vec` length is checked before
    /// consuming results, whose order selects the lowest-ordinal failure.
    pub fn complete(
        self,
        rows: Vec<Result<GeneratedIbpSourceRow, ParametricIbpError>>,
    ) -> Result<CompletedIbpSourceRows, ParametricIbpError> {
        let layout = self.source.layout();
        if rows.len() != self.rows {
            return Err(ParametricIbpError::WrongSourceRowCount {
                batch: layout.name(),
                expected: self.rows,
                actual: rows.len(),
            });
        }
        let mut relations = Vec::with_capacity(self.rows);
        for (position, row) in rows.into_iter().enumerate() {
            let row = row?;
            if row.layout != layout {
                return Err(ParametricIbpError::SourceRowLayoutMismatch {
                    position,
                    expected: layout.name(),
                    actual: row.layout.name(),
                });
            }
            if row.scope != self.scope {
                return Err(ParametricIbpError::SourceRowScopeMismatch {
                    batch: layout.name(),
                    position,
                });
            }
            if row.ordinal != position {
                return Err(ParametricIbpError::SourceRowOrdinalMismatch {
                    batch: layout.name(),
                    position,
                    actual: row.ordinal,
                });
            }
            relations.push(row.relation);
        }
        Ok(CompletedIbpSourceRows {
            scope: self.scope,
            layout,
            relations,
        })
    }
}

/// Immutable LI work prepared from one completed IBP source barrier.
#[derive(Debug)]
pub struct PreparedLorentzInvarianceBatch<'generator, 'family, 'ordinary> {
    generator: &'generator ParametricIbpGenerator<'family>,
    ordinary: &'ordinary [ParametricRelation],
    source_offset: usize,
    external_pairs: Vec<(usize, usize)>,
}

impl PreparedLorentzInvarianceBatch<'_, '_, '_> {
    pub fn len(&self) -> usize {
        self.external_pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.external_pairs.is_empty()
    }

    /// Generate one LI row at its lexicographic external-pair ordinal.
    pub fn generate(&self, ordinal: usize) -> Result<ParametricRelation, ParametricIbpError> {
        let &(first_external, second_external) =
            self.external_pairs
                .get(ordinal)
                .ok_or(ParametricIbpError::RowOrdinalOutOfRange {
                    batch: "Lorentz-invariance",
                    ordinal,
                    rows: self.external_pairs.len(),
                })?;
        self.generator.generate_li_row(
            self.ordinary,
            self.source_offset,
            first_external,
            second_external,
        )
    }
}

impl<'family> ParametricIbpGenerator<'family> {
    pub fn try_new(family: &'family IntegralFamily) -> Result<Self, ParametricIbpError> {
        Self::try_new_with_config(family, ParametricIbpConfig::default())
    }

    pub fn try_new_with_config(
        family: &'family IntegralFamily,
        config: ParametricIbpConfig,
    ) -> Result<Self, ParametricIbpError> {
        let family_fingerprint: Arc<str> = family.fingerprint().into();
        // The full semantic fingerprint is encoded losslessly by the context
        // constructor.  Thus two distinct family definitions never alias an
        // index-variable identity merely because their display names agree.
        let scope = format!("ordinary-ibp|{family_fingerprint}");
        let context = IndexedCoefficientContext::try_new(
            family.coefficient_context(),
            &scope,
            family.denominator_count(),
        )?;
        Self::try_with_context_and_fingerprint(family, family_fingerprint, context, config)
    }

    /// Construct a generator with a caller-owned exact `K(n)` identity.
    ///
    /// This is useful when relations from several generation stages must use
    /// one shared index scope.  Both the base map and index arity are checked.
    pub fn try_with_context(
        family: &'family IntegralFamily,
        context: IndexedCoefficientContext,
        config: ParametricIbpConfig,
    ) -> Result<Self, ParametricIbpError> {
        let family_fingerprint = family.fingerprint().into();
        Self::try_with_context_and_fingerprint(family, family_fingerprint, context, config)
    }

    fn try_with_context_and_fingerprint(
        family: &'family IntegralFamily,
        family_fingerprint: Arc<str>,
        context: IndexedCoefficientContext,
        config: ParametricIbpConfig,
    ) -> Result<Self, ParametricIbpError> {
        if !family
            .coefficient_context()
            .has_same_variable_map(context.base())
        {
            return Err(ParametricIbpError::BaseContextMismatch);
        }
        let arity = family.denominator_count();
        if context.index_count() != arity {
            return Err(ParametricIbpError::WrongIndexArity {
                expected: arity,
                actual: context.index_count(),
            });
        }
        checked_generated_row_counts(family.loop_count(), family.external_count())?;
        let index_space = IndexSpace::try_new(arity)?;
        let positive_units = (0..arity)
            .map(|position| index_space.unit(position, 1))
            .collect::<Result<Vec<_>, _>>()?;
        let negative_units = (0..arity)
            .map(|position| index_space.unit(position, -1))
            .collect::<Result<Vec<_>, _>>()?;
        let source_scope = IbpSourceScope {
            family_fingerprint,
            context_fingerprint: context.fingerprint().into(),
        };
        Ok(Self {
            family,
            source_scope,
            context,
            index_space,
            positive_units,
            negative_units,
            config,
        })
    }

    pub fn family(&self) -> &IntegralFamily {
        self.family
    }

    pub fn context(&self) -> &IndexedCoefficientContext {
        &self.context
    }

    pub fn config(&self) -> ParametricIbpConfig {
        self.config
    }

    /// Prepare the `L*(L+E)` independent ordinary rows for deterministic
    /// ordinal execution. The returned batch owns every shared coefficient
    /// translation needed by its rows and performs no scheduling itself.
    pub fn prepare_ordinary_ibp(
        &self,
    ) -> Result<PreparedIbpSourceBatch<'_, 'family>, ParametricIbpError> {
        let (ordinary_count, _) =
            checked_generated_row_counts(self.family.loop_count(), self.family.external_count())?;
        let (dimension, powers) = self.prepare_ordinary_coefficients()?;
        Ok(PreparedIbpSourceBatch {
            generator: self,
            scope: self.source_scope.clone(),
            source: PreparedIbpSource::CompleteOrdinary { dimension },
            powers,
            rows: ordinary_count,
        })
    }

    /// Prepare only the `L*E` external-contraction ordinary rows needed as
    /// sources for LI-only generation.
    pub fn prepare_external_ibp_sources(
        &self,
    ) -> Result<PreparedIbpSourceBatch<'_, 'family>, ParametricIbpError> {
        let loops = self.family.loop_count();
        let externals = self.family.external_count();
        let rows = loops
            .checked_mul(externals)
            .ok_or(ParametricIbpError::RowCountOverflow { loops, externals })?;
        let powers = self.prepare_ordinary_powers()?;
        Ok(PreparedIbpSourceBatch {
            generator: self,
            scope: self.source_scope.clone(),
            source: PreparedIbpSource::ExternalOnly,
            powers,
            rows,
        })
    }

    fn prepare_ordinary_coefficients(
        &self,
    ) -> Result<(IndexedCoefficient, Vec<IndexedCoefficient>), ParametricIbpError> {
        let dimension = self.context.lift(self.family.dimension())?;
        let powers = self.prepare_ordinary_powers()?;
        Ok((dimension, powers))
    }

    fn prepare_ordinary_powers(&self) -> Result<Vec<IndexedCoefficient>, ParametricIbpError> {
        let powers = (0..self.family.denominator_count())
            .map(|denominator| {
                let index = self.context.index(denominator)?;
                let power_shift = self
                    .context
                    .lift(&self.family.power_shifts()[denominator])?;
                self.context
                    .add_with_limits(
                        &index,
                        &power_shift,
                        self.config.relation_limits.arithmetic.exact_algebra,
                    )
                    .map_err(ParametricIbpError::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(powers)
    }

    /// Generate the `L*(L+E)` raw ordinary IBPs serially.
    pub fn generate_ordinary_ibp(&self) -> Result<Vec<ParametricRelation>, ParametricIbpError> {
        Ok(self.generate_ordinary_completed()?.into_relations())
    }

    fn generate_ordinary_completed(&self) -> Result<CompletedIbpSourceRows, ParametricIbpError> {
        let batch = self.prepare_ordinary_ibp()?;
        let rows = (0..batch.len())
            .map(|ordinal| batch.generate(ordinal))
            .collect();
        batch.complete(rows)
    }

    fn generate_external_sources_completed(
        &self,
    ) -> Result<CompletedIbpSourceRows, ParametricIbpError> {
        let batch = self.prepare_external_ibp_sources()?;
        let rows = (0..batch.len())
            .map(|ordinal| batch.generate(ordinal))
            .collect();
        batch.complete(rows)
    }

    fn generate_ordinary_row(
        &self,
        ordinal: usize,
        dimension: &IndexedCoefficient,
        powers: &[IndexedCoefficient],
    ) -> Result<ParametricRelation, ParametricIbpError> {
        let loops = self.family.loop_count();
        debug_assert!(loops > 0);
        let contraction_index = ordinal / loops;
        let differentiated_loop = ordinal % loops;
        let contraction = self.family.contraction_momenta()[contraction_index];
        let row_id = RowId::OrdinaryIbp {
            contraction_momentum: contraction_index,
            differentiated_loop,
        };
        let mut row = self.empty_relation(row_id)?;
        if contraction == ContractionMomentum::Loop(differentiated_loop) {
            row.add_term_with_limits(
                &self.context,
                self.index_space.zero(),
                dimension.clone(),
                self.config.relation_limits,
            )?;
        }

        self.add_ordinary_derivative_terms(&mut row, differentiated_loop, contraction, powers)?;
        Ok(row)
    }

    fn generate_external_source_row(
        &self,
        ordinal: usize,
        powers: &[IndexedCoefficient],
    ) -> Result<ParametricRelation, ParametricIbpError> {
        let loops = self.family.loop_count();
        debug_assert!(loops > 0);
        let external = ordinal / loops;
        let differentiated_loop = ordinal % loops;
        let contraction_index =
            loops
                .checked_add(external)
                .ok_or(ParametricIbpError::RowCountOverflow {
                    loops,
                    externals: self.family.external_count(),
                })?;
        let mut row = self.empty_relation(RowId::OrdinaryIbp {
            contraction_momentum: contraction_index,
            differentiated_loop,
        })?;
        self.add_ordinary_derivative_terms(
            &mut row,
            differentiated_loop,
            ContractionMomentum::External(external),
            powers,
        )?;
        Ok(row)
    }

    fn add_ordinary_derivative_terms(
        &self,
        row: &mut ParametricRelation,
        differentiated_loop: usize,
        contraction: ContractionMomentum,
        powers: &[IndexedCoefficient],
    ) -> Result<(), ParametricIbpError> {
        for (denominator, power) in powers.iter().enumerate() {
            let derivative = self.family.derivative_contraction(
                denominator,
                differentiated_loop,
                contraction,
            )?;
            self.add_negative_derivative_term(
                row,
                self.positive_units[denominator].clone(),
                power,
                derivative.constant(),
            )?;
            for (target, coefficient) in derivative.denominator_coefficients().iter().enumerate() {
                let shift =
                    self.positive_units[denominator].checked_add(&self.negative_units[target])?;
                self.add_negative_derivative_term(row, shift, power, coefficient)?;
            }
        }
        Ok(())
    }

    /// Generate ordinary and LI rows with their shared authenticated context.
    pub fn generate(&self) -> Result<ParametricIbpRelations, ParametricIbpError> {
        let ordinary = self.generate_ordinary_completed()?;
        let lorentz_invariance = self.generate_li_from_sources(&ordinary)?;
        Ok(self.relations(ordinary.into_relations(), lorentz_invariance))
    }

    fn relations(
        &self,
        ordinary_ibp: Vec<ParametricRelation>,
        lorentz_invariance: Vec<ParametricRelation>,
    ) -> ParametricIbpRelations {
        ParametricIbpRelations {
            family_fingerprint: self.source_scope.family_fingerprint.clone(),
            context: self.context.clone(),
            ordinary_ibp,
            lorentz_invariance,
        }
    }

    /// Generate only the LI rows.  Ordinary external-contraction rows are
    /// derived first, exactly as in LiteRed, and are not returned.
    pub fn generate_lorentz_invariance(
        &self,
    ) -> Result<Vec<ParametricRelation>, ParametricIbpError> {
        let (_, li_count) =
            checked_generated_row_counts(self.family.loop_count(), self.family.external_count())?;
        if li_count == 0 {
            return Ok(Vec::new());
        }
        let sources = self.generate_external_sources_completed()?;
        self.generate_li_from_sources(&sources)
    }

    fn generate_li_from_sources(
        &self,
        sources: &CompletedIbpSourceRows,
    ) -> Result<Vec<ParametricRelation>, ParametricIbpError> {
        let batch = self.prepare_lorentz_invariance(sources)?;
        (0..batch.len())
            .map(|ordinal| batch.generate(ordinal))
            .collect()
    }

    /// Prepare LI rows from one completed ordinary or external-only source
    /// barrier. Completion already authenticated every row, so this boundary
    /// compares the semantic family/context scope once and does not replay the
    /// relation slice.
    pub fn prepare_lorentz_invariance<'generator, 'ordinary>(
        &'generator self,
        sources: &'ordinary CompletedIbpSourceRows,
    ) -> Result<PreparedLorentzInvarianceBatch<'generator, 'family, 'ordinary>, ParametricIbpError>
    {
        if sources.scope != self.source_scope {
            return Err(ParametricIbpError::CompletedSourceScopeMismatch);
        }
        let loops = self.family.loop_count();
        let externals = self.family.external_count();
        let source_offset = sources
            .layout
            .source_offset(loops)
            .ok_or(ParametricIbpError::RowCountOverflow { loops, externals })?;
        let (_, li_count) = checked_generated_row_counts(loops, externals)?;
        let mut pairs = Vec::with_capacity(li_count);
        for first_external in 0..externals {
            for second_external in first_external + 1..externals {
                pairs.push((first_external, second_external));
            }
        }
        debug_assert_eq!(pairs.len(), li_count);
        Ok(PreparedLorentzInvarianceBatch {
            generator: self,
            ordinary: &sources.relations,
            source_offset,
            external_pairs: pairs,
        })
    }

    fn generate_li_row(
        &self,
        ordinary: &[ParametricRelation],
        source_offset: usize,
        first_external: usize,
        second_external: usize,
    ) -> Result<ParametricRelation, ParametricIbpError> {
        let row_id = RowId::LorentzInvariance {
            first_external,
            second_external,
        };
        let mut row = self.empty_relation(row_id.clone())?;
        for differentiated_loop in 0..self.family.loop_count() {
            // M_ba: X_{i b} B_{a i}
            let source_a = self.external_ordinary_row(
                ordinary,
                source_offset,
                first_external,
                differentiated_loop,
            )?;
            let coordinate_b =
                self.family
                    .coordinate_index(ScalarProductCoordinate::LoopExternal {
                        loop_index: differentiated_loop,
                        external_index: second_external,
                    })?;
            let multiplier_b = self.family.scalar_product_expansion(coordinate_b)?;
            self.add_weighted_translation(
                &mut row,
                source_a,
                multiplier_b.constant(),
                multiplier_b.denominator_coefficients(),
                false,
                &row_id,
            )?;

            // -M_ab: -X_{i a} B_{b i}
            let source_b = self.external_ordinary_row(
                ordinary,
                source_offset,
                second_external,
                differentiated_loop,
            )?;
            let coordinate_a =
                self.family
                    .coordinate_index(ScalarProductCoordinate::LoopExternal {
                        loop_index: differentiated_loop,
                        external_index: first_external,
                    })?;
            let multiplier_a = self.family.scalar_product_expansion(coordinate_a)?;
            self.add_weighted_translation(
                &mut row,
                source_b,
                multiplier_a.constant(),
                multiplier_a.denominator_coefficients(),
                true,
                &row_id,
            )?;
        }
        Ok(row)
    }

    fn external_ordinary_row<'rows>(
        &self,
        ordinary: &'rows [ParametricRelation],
        source_offset: usize,
        external: usize,
        differentiated_loop: usize,
    ) -> Result<&'rows ParametricRelation, ParametricIbpError> {
        let row = external
            .checked_mul(self.family.loop_count())
            .and_then(|offset| source_offset.checked_add(offset))
            .and_then(|offset| offset.checked_add(differentiated_loop))
            .and_then(|position| ordinary.get(position))
            .ok_or(ParametricIbpError::RowCountOverflow {
                loops: self.family.loop_count(),
                externals: self.family.external_count(),
            })?;
        Ok(row)
    }

    fn add_negative_derivative_term(
        &self,
        row: &mut ParametricRelation,
        shift: IndexShift,
        power: &IndexedCoefficient,
        derivative_coefficient: &crate::algebra::Coefficient,
    ) -> Result<(), ParametricIbpError> {
        if derivative_coefficient.is_zero() {
            return Ok(());
        }
        let derivative = self.context.lift(derivative_coefficient)?;
        let product = self.context.mul_with_limits(
            power,
            &derivative,
            self.config.relation_limits.arithmetic.exact_algebra,
        )?;
        let coefficient = self.context.neg_with_limits(
            &product,
            self.config.relation_limits.arithmetic.exact_algebra,
        )?;
        row.add_term_with_limits(
            &self.context,
            shift,
            coefficient,
            self.config.relation_limits,
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn add_weighted_translation(
        &self,
        target: &mut ParametricRelation,
        source: &ParametricRelation,
        constant: &crate::algebra::Coefficient,
        denominator_coefficients: &[crate::algebra::Coefficient],
        negate: bool,
        row_id: &RowId,
    ) -> Result<(), ParametricIbpError> {
        self.add_one_weighted_translation(
            target,
            source,
            self.index_space.zero(),
            constant,
            negate,
            row_id,
        )?;
        for (denominator, coefficient) in denominator_coefficients.iter().enumerate() {
            self.add_one_weighted_translation(
                target,
                source,
                self.negative_units[denominator].clone(),
                coefficient,
                negate,
                row_id,
            )?;
        }
        Ok(())
    }

    fn add_one_weighted_translation(
        &self,
        target: &mut ParametricRelation,
        source: &ParametricRelation,
        translation: IndexShift,
        base_factor: &crate::algebra::Coefficient,
        negate: bool,
        row_id: &RowId,
    ) -> Result<(), ParametricIbpError> {
        if base_factor.is_zero() {
            return Ok(());
        }
        let translated = source.translated(
            &self.context,
            &translation,
            row_id.clone(),
            self.config.relation_limits,
        )?;
        let mut factor = self.context.lift(base_factor)?;
        if negate {
            factor = self.context.neg_with_limits(
                &factor,
                self.config.relation_limits.arithmetic.exact_algebra,
            )?;
        }
        target.add_scaled_with_limits(
            &self.context,
            &translated,
            &factor,
            self.config.relation_limits,
        )?;
        Ok(())
    }

    fn empty_relation(&self, row_id: RowId) -> Result<ParametricRelation, ParametricIbpError> {
        let mut relation = ParametricRelation::new(
            self.source_scope.family_fingerprint.clone(),
            row_id,
            &self.context,
        );
        // Preserve the complete family domain before any fraction-field
        // cancellation.  Tautological nonzero constants are intentionally
        // omitted by ParametricRelation.
        for condition in self.family.domain().conditions() {
            let lifted = self.context.lift_base_polynomial(condition.polynomial())?;
            let sources = condition.sources().iter().cloned().map(|location| {
                if location == CoefficientLocation::BasisDeterminantNumerator {
                    IdentityConditionSource::FamilyBasisDeterminantNumerator
                } else {
                    IdentityConditionSource::FamilyInputCoefficientDenominator { location }
                }
            });
            let lifted = ParametricNonZeroCondition::try_new_with_limits(
                &self.context,
                lifted,
                sources,
                self.config.relation_limits.arithmetic.exact_algebra,
                self.config.relation_limits.identity_conditions,
            )?;
            relation.add_nonzero_condition_with_limits(
                &self.context,
                lifted,
                self.config.relation_limits,
            )?;
        }
        Ok(relation)
    }
}

/// Return the exact ordinary-IBP and LI row census without constructing any
/// symbolic row.  Resource-bounded callers use this preflight before entering
/// the generator's allocation and exact-algebra work.
pub(crate) fn checked_generated_row_counts(
    loops: usize,
    externals: usize,
) -> Result<(usize, usize), ParametricIbpError> {
    let contractions = loops
        .checked_add(externals)
        .ok_or(ParametricIbpError::RowCountOverflow { loops, externals })?;
    let ordinary = loops
        .checked_mul(contractions)
        .ok_or(ParametricIbpError::RowCountOverflow { loops, externals })?;
    let li = if externals < 2 {
        0
    } else {
        let predecessor = externals - 1;
        let (left, right) = if externals % 2 == 0 {
            (externals / 2, predecessor)
        } else {
            (externals, predecessor / 2)
        };
        left.checked_mul(right)
            .ok_or(ParametricIbpError::RowCountOverflow { loops, externals })?
    };
    Ok((ordinary, li))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AffineDenominator, algebra::Coefficient, algebra::CoefficientContext};

    fn coefficient_for<'a>(
        relation: &'a crate::ConcreteRelation,
        powers: &[i64],
    ) -> Option<&'a Coefficient> {
        relation
            .terms()
            .iter()
            .find_map(|(key, coefficient)| (key.powers() == powers).then_some(coefficient))
    }

    fn assert_coefficient_eq(left: &Coefficient, right: &Coefficient) {
        assert!((left - right).is_zero(), "left={left}, right={right}");
    }

    fn identity_denominators(
        context: &CoefficientContext,
        constants: Vec<Coefficient>,
    ) -> Vec<AffineDenominator> {
        let size = constants.len();
        constants
            .into_iter()
            .enumerate()
            .map(|(row, constant)| {
                AffineDenominator::new(
                    constant,
                    (0..size)
                        .map(|column| {
                            if row == column {
                                context.one()
                            } else {
                                context.zero()
                            }
                        })
                        .collect(),
                )
            })
            .collect()
    }

    fn coordinate_family(name: &str, loops: usize, externals: usize) -> IntegralFamily {
        let context = CoefficientContext::new(["d"]);
        let arity = loops * (loops + 1) / 2 + loops * externals;
        let external_gram = (0..externals)
            .map(|row| {
                (0..externals)
                    .map(|column| {
                        if row == column {
                            context.one()
                        } else {
                            context.zero()
                        }
                    })
                    .collect()
            })
            .collect();
        IntegralFamily::new(
            name,
            (0..loops).map(|loop_| format!("k{loop_}")).collect(),
            (0..externals)
                .map(|external| format!("p{external}"))
                .collect(),
            context.clone(),
            context.parameter("d").unwrap(),
            identity_denominators(&context, vec![context.integer(-1); arity]),
            external_gram,
            vec![context.zero(); arity],
        )
        .unwrap()
    }

    #[test]
    fn sentinel_topology_neutral_source_counts_cover_one_two_and_six_loops() {
        for (loops, ordinary_count) in [(1, 3), (2, 8)] {
            let family = coordinate_family(&format!("li-sentinel-l{loops}"), loops, 2);
            let generated = ParametricIbpGenerator::try_new(&family)
                .unwrap()
                .generate()
                .unwrap();

            assert_eq!(generated.ordinary_ibp().len(), ordinary_count);
            assert_eq!(generated.lorentz_invariance().len(), 1);
            assert_eq!(
                generated.lorentz_invariance()[0].row_id(),
                &RowId::LorentzInvariance {
                    first_external: 0,
                    second_external: 1,
                }
            );
            assert!(generated.ibp_li().all(|row| {
                row.arity() == family.denominator_count()
                    && row.family_fingerprint() == family.fingerprint_ref()
            }));
        }

        let family = coordinate_family("ordinary-source-sentinel-l6-k21", 6, 0);
        assert_eq!(family.denominator_count(), 21);
        let rows = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate_ordinary_ibp()
            .unwrap();
        assert_eq!(rows.len(), 36);
        for (ordinal, row) in rows.iter().enumerate() {
            assert_eq!(row.arity(), 21);
            assert_eq!(
                row.row_id(),
                &RowId::OrdinaryIbp {
                    contraction_momentum: ordinal / 6,
                    differentiated_loop: ordinal % 6,
                }
            );
        }
    }

    #[test]
    fn li_only_with_zero_or_one_external_returns_before_source_generation() {
        for externals in [0, 1] {
            let family = coordinate_family(
                &format!("li-empty-source-barrier-e{externals}"),
                2,
                externals,
            );
            let generator = ParametricIbpGenerator::try_new(&family).unwrap();
            assert!(generator.generate_lorentz_invariance().unwrap().is_empty());
        }
    }

    #[test]
    fn one_loop_tadpole_is_a_fully_parametric_recurrence() {
        let base = CoefficientContext::new(["d", "m2", "nu"]);
        let d = base.parameter("d").unwrap();
        let m2 = base.parameter("m2").unwrap();
        let nu = base.parameter("nu").unwrap();
        let family = IntegralFamily::new(
            "one-loop-tadpole-parametric",
            vec!["k".into()],
            Vec::new(),
            base.clone(),
            d.clone(),
            vec![AffineDenominator::new(m2.clone(), vec![base.one()])],
            Vec::new(),
            vec![nu.clone()],
        )
        .unwrap();
        let generator = ParametricIbpGenerator::try_new(&family).unwrap();
        let generated = generator.generate().unwrap();

        assert_eq!(generated.ordinary_ibp().len(), 1);
        assert!(generated.lorentz_invariance().is_empty());
        assert_eq!(
            generated.ordinary_ibp()[0].row_id(),
            &RowId::OrdinaryIbp {
                contraction_momentum: 0,
                differentiated_loop: 0,
            }
        );
        let concrete = generated.ordinary_ibp()[0]
            .specialize(generated.context(), &[3], RelationLimits::default())
            .unwrap();
        assert_eq!(concrete.terms().len(), 2);
        let shifted_power = &base.integer(3) + &nu;
        let expected_same = &d - &(&base.integer(2) * &shifted_power);
        let expected_raised = &(&base.integer(2) * &m2) * &shifted_power;
        assert_coefficient_eq(coefficient_for(&concrete, &[3]).unwrap(), &expected_same);
        assert_coefficient_eq(coefficient_for(&concrete, &[4]).unwrap(), &expected_raised);

        // Sector signs are determined by the raw index, but a power shift is
        // still present in the coefficient at n=0.  Raw generation must not
        // use the concrete zero-index shortcut of the legacy vacuum code.
        let at_zero = generated.ordinary_ibp()[0]
            .specialize(generated.context(), &[0], RelationLimits::default())
            .unwrap();
        assert_eq!(at_zero.terms().len(), 2);
        assert_coefficient_eq(
            coefficient_for(&at_zero, &[0]).unwrap(),
            &(&d - &(&base.integer(2) * &nu)),
        );
        assert_coefficient_eq(
            coefficient_for(&at_zero, &[1]).unwrap(),
            &(&(&base.integer(2) * &m2) * &nu),
        );
    }

    #[test]
    fn one_loop_li_has_litered_sign_and_weighted_denominator_shifts() {
        let base = CoefficientContext::new(["d", "s00", "s11", "c1", "c2", "nu0", "nu1", "nu2"]);
        let s00 = base.parameter("s00").unwrap();
        let s11 = base.parameter("s11").unwrap();
        let c1 = base.parameter("c1").unwrap();
        let c2 = base.parameter("c2").unwrap();
        let nu1 = base.parameter("nu1").unwrap();
        let nu2 = base.parameter("nu2").unwrap();
        let family = IntegralFamily::new(
            "one-loop-two-leg-li",
            vec!["k".into()],
            vec!["p0".into(), "p1".into()],
            base.clone(),
            base.parameter("d").unwrap(),
            identity_denominators(&base, vec![base.zero(), c1.clone(), c2.clone()]),
            vec![
                vec![s00.clone(), base.zero()],
                vec![base.zero(), s11.clone()],
            ],
            vec![base.parameter("nu0").unwrap(), nu1.clone(), nu2.clone()],
        )
        .unwrap();
        let generated = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate()
            .unwrap();

        assert_eq!(generated.ordinary_ibp().len(), 3);
        assert_eq!(generated.lorentz_invariance().len(), 1);
        assert_eq!(
            generated.lorentz_invariance()[0].row_id(),
            &RowId::LorentzInvariance {
                first_external: 0,
                second_external: 1,
            }
        );
        let concrete = generated.lorentz_invariance()[0]
            .specialize(generated.context(), &[2, 3, 4], RelationLimits::default())
            .unwrap();
        assert_eq!(concrete.terms().len(), 4);
        let n1 = &base.integer(3) + &nu1;
        let n2 = &base.integer(4) + &nu2;
        assert_coefficient_eq(
            coefficient_for(&concrete, &[2, 4, 4]).unwrap(),
            &(&(&c2 * &s00) * &n1),
        );
        assert_coefficient_eq(
            coefficient_for(&concrete, &[2, 4, 3]).unwrap(),
            &(-(&s00 * &n1)),
        );
        assert_coefficient_eq(
            coefficient_for(&concrete, &[2, 3, 5]).unwrap(),
            &(-(&(&c1 * &s11) * &n2)),
        );
        assert_coefficient_eq(
            coefficient_for(&concrete, &[2, 2, 5]).unwrap(),
            &(&s11 * &n2),
        );
    }

    #[test]
    fn two_loop_rows_are_q_major_and_li_pairs_are_lexicographic() {
        let base = CoefficientContext::new(["d", "s00", "s01", "s02", "s11", "s12", "s22", "nu"]);
        let family = IntegralFamily::new(
            "two-loop-three-leg-structure",
            vec!["k0".into(), "k1".into()],
            vec!["p0".into(), "p1".into(), "p2".into()],
            base.clone(),
            base.parameter("d").unwrap(),
            identity_denominators(&base, vec![base.zero(); 9]),
            vec![
                vec![
                    base.parameter("s00").unwrap(),
                    base.parameter("s01").unwrap(),
                    base.parameter("s02").unwrap(),
                ],
                vec![
                    base.parameter("s01").unwrap(),
                    base.parameter("s11").unwrap(),
                    base.parameter("s12").unwrap(),
                ],
                vec![
                    base.parameter("s02").unwrap(),
                    base.parameter("s12").unwrap(),
                    base.parameter("s22").unwrap(),
                ],
            ],
            vec![
                base.parameter("nu").unwrap(),
                base.zero(),
                base.zero(),
                base.zero(),
                base.zero(),
                base.zero(),
                base.zero(),
                base.zero(),
                base.zero(),
            ],
        )
        .unwrap();
        let generator = ParametricIbpGenerator::try_new(&family).unwrap();
        let ordinary_batch = generator.prepare_ordinary_ibp().unwrap();
        assert_eq!(ordinary_batch.len(), 10);
        assert!(matches!(
            ordinary_batch.generate(10),
            Err(ParametricIbpError::RowOrdinalOutOfRange {
                batch: "ordinary IBP source",
                ordinal: 10,
                rows: 10,
            })
        ));
        let ordinary_rows = (0..ordinary_batch.len())
            .map(|ordinal| ordinary_batch.generate(ordinal))
            .collect();
        let ordinary = ordinary_batch.complete(ordinary_rows).unwrap();
        let li_batch = generator.prepare_lorentz_invariance(&ordinary).unwrap();
        assert_eq!(li_batch.len(), 3);
        assert!(matches!(
            li_batch.generate(3),
            Err(ParametricIbpError::RowOrdinalOutOfRange {
                batch: "Lorentz-invariance",
                ordinal: 3,
                rows: 3,
            })
        ));
        let lorentz_invariance = (0..li_batch.len())
            .map(|ordinal| li_batch.generate(ordinal))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        drop(li_batch);
        let generated = generator.relations(ordinary.into_relations(), lorentz_invariance);

        let ids = generated
            .ordinary_ibp()
            .iter()
            .map(|row| row.row_id().clone())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                RowId::OrdinaryIbp {
                    contraction_momentum: 0,
                    differentiated_loop: 0,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 0,
                    differentiated_loop: 1,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 1,
                    differentiated_loop: 0,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 1,
                    differentiated_loop: 1,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 2,
                    differentiated_loop: 0,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 2,
                    differentiated_loop: 1,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 3,
                    differentiated_loop: 0,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 3,
                    differentiated_loop: 1,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 4,
                    differentiated_loop: 0,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 4,
                    differentiated_loop: 1,
                },
            ]
        );
        assert_eq!(
            generated
                .lorentz_invariance()
                .iter()
                .map(|row| row.row_id().clone())
                .collect::<Vec<_>>(),
            vec![
                RowId::LorentzInvariance {
                    first_external: 0,
                    second_external: 1,
                },
                RowId::LorentzInvariance {
                    first_external: 0,
                    second_external: 2,
                },
                RowId::LorentzInvariance {
                    first_external: 1,
                    second_external: 2,
                },
            ]
        );
        assert_eq!(generated.ibp_li().count(), 13);
        assert!(
            generated
                .ordinary_ibp()
                .iter()
                .chain(generated.lorentz_invariance())
                .all(|row| row.arity() == 9 && row.family_fingerprint() == family.fingerprint())
        );
    }

    #[test]
    fn source_completion_seals_layout_scope_and_order_without_generator_identity() {
        let family = coordinate_family("sealed-source-validation", 1, 2);
        let generator = ParametricIbpGenerator::try_new(&family).unwrap();
        let equivalent_generator = ParametricIbpGenerator::try_new(&family).unwrap();

        // A separately prepared generator with the same semantic scope is a
        // valid source; pointer identity is deliberately irrelevant.
        let target = generator.prepare_ordinary_ibp().unwrap();
        let equivalent = equivalent_generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..equivalent.len())
            .map(|ordinal| equivalent.generate(ordinal))
            .collect();
        let completed = target.complete(rows).unwrap();
        assert!(generator.prepare_lorentz_invariance(&completed).is_ok());

        let short = generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..short.len() - 1)
            .map(|ordinal| short.generate(ordinal))
            .collect();
        assert!(matches!(
            short.complete(rows),
            Err(ParametricIbpError::WrongSourceRowCount {
                batch: "ordinary IBP source",
                expected: 3,
                actual: 2,
            })
        ));

        let reordered = generator.prepare_ordinary_ibp().unwrap();
        let mut rows = (0..reordered.len())
            .map(|ordinal| reordered.generate(ordinal))
            .collect::<Vec<_>>();
        rows.swap(0, 1);
        assert!(matches!(
            reordered.complete(rows),
            Err(ParametricIbpError::SourceRowOrdinalMismatch {
                batch: "ordinary IBP source",
                position: 0,
                actual: 1,
            })
        ));

        let wrong_layout = generator.prepare_ordinary_ibp().unwrap();
        let ordinary_source = equivalent_generator.prepare_ordinary_ibp().unwrap();
        let external_source = equivalent_generator.prepare_external_ibp_sources().unwrap();
        let mut rows = (0..ordinary_source.len())
            .map(|ordinal| ordinary_source.generate(ordinal))
            .collect::<Vec<_>>();
        rows[0] = external_source.generate(0);
        assert!(matches!(
            wrong_layout.complete(rows),
            Err(ParametricIbpError::SourceRowLayoutMismatch {
                position: 0,
                expected: "ordinary IBP source",
                actual: "external-contraction IBP source",
            })
        ));

        let foreign_family = coordinate_family("foreign-li-source", 1, 2);
        let foreign_generator = ParametricIbpGenerator::try_new(&foreign_family).unwrap();
        let target = generator.prepare_ordinary_ibp().unwrap();
        let foreign_batch = foreign_generator.prepare_ordinary_ibp().unwrap();
        let foreign_rows = (0..foreign_batch.len())
            .map(|ordinal| foreign_batch.generate(ordinal))
            .collect();
        assert!(matches!(
            target.complete(foreign_rows),
            Err(ParametricIbpError::SourceRowScopeMismatch {
                batch: "ordinary IBP source",
                position: 0,
            })
        ));

        let foreign_batch = foreign_generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..foreign_batch.len())
            .map(|ordinal| foreign_batch.generate(ordinal))
            .collect();
        let foreign_completed = foreign_batch.complete(rows).unwrap();
        assert!(matches!(
            generator.prepare_lorentz_invariance(&foreign_completed),
            Err(ParametricIbpError::CompletedSourceScopeMismatch)
        ));
    }

    #[test]
    fn li_only_source_batch_contains_exactly_external_contractions() {
        let family = coordinate_family("dense-external-sources", 2, 3);
        let generator = ParametricIbpGenerator::try_new(&family).unwrap();
        let batch = generator.prepare_external_ibp_sources().unwrap();
        assert_eq!(batch.len(), 6);
        let rows = (0..batch.len())
            .map(|ordinal| batch.generate(ordinal))
            .collect();
        let sources = batch.complete(rows).unwrap();
        assert_eq!(
            sources
                .relations
                .iter()
                .map(|row| row.row_id().clone())
                .collect::<Vec<_>>(),
            vec![
                RowId::OrdinaryIbp {
                    contraction_momentum: 2,
                    differentiated_loop: 0,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 2,
                    differentiated_loop: 1,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 3,
                    differentiated_loop: 0,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 3,
                    differentiated_loop: 1,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 4,
                    differentiated_loop: 0,
                },
                RowId::OrdinaryIbp {
                    contraction_momentum: 4,
                    differentiated_loop: 1,
                },
            ]
        );
        let li_batch = generator.prepare_lorentz_invariance(&sources).unwrap();
        let rows = (0..li_batch.len())
            .map(|ordinal| li_batch.generate(ordinal))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows.iter()
                .map(|row| row.row_id().clone())
                .collect::<Vec<_>>(),
            vec![
                RowId::LorentzInvariance {
                    first_external: 0,
                    second_external: 1,
                },
                RowId::LorentzInvariance {
                    first_external: 0,
                    second_external: 2,
                },
                RowId::LorentzInvariance {
                    first_external: 1,
                    second_external: 2,
                },
            ]
        );
    }

    #[test]
    fn every_row_inherits_input_and_determinant_domain_conditions() {
        let base = CoefficientContext::new(["d", "a", "b", "s", "g"]);
        let family = IntegralFamily::new(
            "conditioned-one-loop-one-leg",
            vec!["k".into()],
            vec!["p".into()],
            base.clone(),
            base.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    base.zero(),
                    vec![base.coefficient_fixture("a/s"), base.one()],
                ),
                AffineDenominator::new(
                    base.zero(),
                    vec![base.parameter("b").unwrap(), base.integer(2)],
                ),
            ],
            vec![vec![base.parameter("g").unwrap()]],
            vec![base.zero(), base.zero()],
        )
        .unwrap();
        let generated = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate()
            .unwrap();
        let determinant = generated
            .context()
            .lift_base_polynomial(family.domain().determinant_nonzero().polynomial())
            .unwrap();
        let input_denominator = generated
            .context()
            .lift_base_polynomial(
                family
                    .domain()
                    .input_denominators()
                    .iter()
                    .find(|condition| !condition.polynomial().is_constant())
                    .unwrap()
                    .polynomial(),
            )
            .unwrap();
        assert_eq!(generated.ordinary_ibp().len(), 2);
        assert!(generated.ordinary_ibp().iter().all(|row| {
            row.nonzero_conditions()
                .iter()
                .any(|condition| condition.polynomial() == &determinant)
                && row
                    .nonzero_conditions()
                    .iter()
                    .any(|condition| condition.polynomial() == &input_denominator)
        }));
        assert!(generated.ordinary_ibp().iter().all(|row| {
            let determinant_condition = row
                .nonzero_conditions()
                .iter()
                .find(|condition| condition.polynomial() == &determinant)
                .unwrap();
            let input_condition = row
                .nonzero_conditions()
                .iter()
                .find(|condition| condition.polynomial() == &input_denominator)
                .unwrap();
            determinant_condition
                .sources()
                .contains(&IdentityConditionSource::FamilyBasisDeterminantNumerator)
                && input_condition.sources().contains(
                    &IdentityConditionSource::FamilyInputCoefficientDenominator {
                        location: crate::CoefficientLocation::DenominatorCoefficient {
                            denominator: 0,
                            coordinate: 0,
                        },
                    },
                )
        }));
    }

    #[test]
    fn custom_context_must_match_family_base_and_arity() {
        let base = CoefficientContext::new(["d"]);
        let family = IntegralFamily::new(
            "context-check",
            vec!["k".into()],
            Vec::new(),
            base.clone(),
            base.one(),
            identity_denominators(&base, vec![base.zero()]),
            Vec::new(),
            vec![base.zero()],
        )
        .unwrap();
        let wrong_base = CoefficientContext::new(["x"]);
        let wrong_context =
            IndexedCoefficientContext::try_new(&wrong_base, "wrong-base", 1).unwrap();
        assert!(matches!(
            ParametricIbpGenerator::try_with_context(
                &family,
                wrong_context,
                ParametricIbpConfig::default()
            ),
            Err(ParametricIbpError::BaseContextMismatch)
        ));

        let wrong_arity = IndexedCoefficientContext::try_new(&base, "wrong-arity", 2).unwrap();
        assert!(matches!(
            ParametricIbpGenerator::try_with_context(
                &family,
                wrong_arity,
                ParametricIbpConfig::default()
            ),
            Err(ParametricIbpError::WrongIndexArity {
                expected: 1,
                actual: 2
            })
        ));
    }

    #[test]
    fn maximal_power_shift_times_parameter_is_a_typed_error_not_a_symbolica_panic() {
        let base = CoefficientContext::new(["x"]);
        let x = base.parameter("x").unwrap();
        let maximal_power = base.coefficient_fixture("x^65535");
        let family = IntegralFamily::new(
            "maximal-power-shift",
            vec!["k".into()],
            Vec::new(),
            base.clone(),
            base.integer(4),
            vec![AffineDenominator::new(x, vec![base.one()])],
            Vec::new(),
            vec![maximal_power],
        )
        .unwrap();

        let error = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate_ordinary_ibp()
            .unwrap_err();
        assert!(matches!(
            error,
            ParametricIbpError::Coefficient(IndexedAlgebraError::ExactAlgebra(
                crate::algebra::ExactAlgebraError::ExponentLimit {
                    operation: crate::algebra::ExactAlgebraOperation::Multiply,
                    requested: 65_536,
                    limit: crate::algebra::SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
                    ..
                }
            ))
        ));
    }
}
