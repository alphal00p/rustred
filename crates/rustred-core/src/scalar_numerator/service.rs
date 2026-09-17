use std::sync::Arc;

use symbolica::atom::{Atom, FunctionBuilder, Symbol, SymbolAttribute, UserData};
use symbolica::prelude::PolyVariable;

use crate::family::{IntegralFamily, IntegralKey, ScalarProductCoordinate};
use crate::foundry::artifact::{ClosedArtifact, CommonMassHomogeneityProof};

use super::error::{
    ScalarNumeratorError, ScalarProductHeadViolation, check_limit, checked_add, checked_mul,
};
use super::model::{ScalarNumeratorLimits, ScalarNumeratorLowering};
use super::syntax::{census_atom, contains_head};

/// Artifact-bound lowering service for scalarized common-mass vacuum numerators.
pub struct ScalarNumeratorService<'artifact> {
    artifact: &'artifact ClosedArtifact,
    prepared: PreparedScalarProducts,
}

/// Exact scalar-product lowering on an independently admitted unit-mass vacuum
/// family. This service has no reduction, sector-coverage or closure authority.
/// Resulting keys must still pass the caller's separate reduction boundary.
pub struct FamilyScalarNumeratorService<'family> {
    family: &'family IntegralFamily,
    prepared: PreparedScalarProducts,
}

pub(super) struct PreparedScalarProducts {
    pub(super) dot_head: Symbol,
    pub(super) loop_momenta: Vec<Atom>,
    pub(super) scalar_products: Vec<Atom>,
    pub(super) scalar_product_variables: Arc<Vec<PolyVariable>>,
    pub(super) limits: ScalarNumeratorLimits,
}

impl<'artifact> ScalarNumeratorService<'artifact> {
    /// Bind an artifact to a passive symmetric scalar-product head and loop labels.
    ///
    /// The head must carry Symbolica's `Symmetric` attribute, optionally with
    /// `Linear`, so reversed loop-loop coordinates have one polynomial form.
    pub fn try_new(
        artifact: &'artifact ClosedArtifact,
        dot_head: Symbol,
        loop_momenta: Vec<Atom>,
        limits: ScalarNumeratorLimits,
    ) -> Result<Self, ScalarNumeratorError> {
        validate_dot_head(dot_head)?;
        let family = artifact.family();
        if artifact.common_mass_homogeneity()
            != Some(CommonMassHomogeneityProof::UniformVacuumMassSquared)
        {
            return Err(ScalarNumeratorError::UnsupportedArtifact {
                detail: "no uniform common-mass homogeneity proof is installed",
            });
        }
        if family.external_count() != 0 {
            return Err(ScalarNumeratorError::UnsupportedArtifact {
                detail: "the common-mass scalar lane currently requires a vacuum family",
            });
        }
        Ok(Self {
            artifact,
            prepared: PreparedScalarProducts::try_new(family, dot_head, loop_momenta, limits)?,
        })
    }

    pub const fn artifact(&self) -> &'artifact ClosedArtifact {
        self.artifact
    }

    pub const fn limits(&self) -> ScalarNumeratorLimits {
        self.prepared.limits
    }

    /// Lower an already scalarized numerator without performing tensor projection.
    pub fn lower(
        &self,
        numerator: &Atom,
        base_integral: &IntegralKey,
    ) -> Result<ScalarNumeratorLowering, ScalarNumeratorError> {
        self.prepared
            .lower_impl(self.artifact.family(), numerator, base_integral, |key| {
                super::lowering::validate_root_key(self.artifact, key)
            })
    }
}

impl<'family> FamilyScalarNumeratorService<'family> {
    /// Bind the same polynomial lowering engine without requiring a closing
    /// artifact. Eligibility is checked independently against the shared
    /// unit-mass, d-only, single-scale vacuum family policy. The authenticated
    /// family's existing inverse basis is reused, never reconstructed here.
    pub fn try_new(
        family: &'family IntegralFamily,
        dot_head: Symbol,
        loop_momenta: Vec<Atom>,
        limits: ScalarNumeratorLimits,
    ) -> Result<Self, ScalarNumeratorError> {
        validate_dot_head(dot_head)?;
        crate::foundry::artifact::validate_unit_mass_family(family)
            .map_err(ScalarNumeratorError::FamilyEligibility)?;
        Ok(Self {
            family,
            prepared: PreparedScalarProducts::try_new(family, dot_head, loop_momenta, limits)?,
        })
    }

    pub const fn family(&self) -> &'family IntegralFamily {
        self.family
    }

    pub const fn limits(&self) -> ScalarNumeratorLimits {
        self.prepared.limits
    }

    /// Lower scalarized syntax to family-bound keys. Only arity and checked
    /// index arithmetic are admitted here, not a certified root/sector scope.
    /// Common-mass powers and scalar spectators retain the artifact lane's
    /// exact meaning. This method does not apply any reduction formula.
    pub fn lower(
        &self,
        numerator: &Atom,
        base_integral: &IntegralKey,
    ) -> Result<ScalarNumeratorLowering, ScalarNumeratorError> {
        self.prepared
            .lower_impl(self.family, numerator, base_integral, |_| Ok(()))
    }
}

impl PreparedScalarProducts {
    fn try_new(
        family: &IntegralFamily,
        dot_head: Symbol,
        loop_momenta: Vec<Atom>,
        limits: ScalarNumeratorLimits,
    ) -> Result<Self, ScalarNumeratorError> {
        if loop_momenta.len() != family.loop_count() {
            return Err(ScalarNumeratorError::WrongLoopMomentumCount {
                expected: family.loop_count(),
                actual: loop_momenta.len(),
            });
        }
        let duplicate_checks = checked_mul(
            "loop-momentum label equality checks",
            loop_momenta.len(),
            loop_momenta.len().saturating_sub(1),
        )? / 2;
        check_limit(
            "loop-momentum label equality checks",
            duplicate_checks,
            limits.max_loop_momentum_label_checks,
        )?;
        let mut momentum_label_nodes = 0usize;
        for (second, momentum) in loop_momenta.iter().enumerate() {
            let nodes = census_atom(
                momentum.as_view(),
                "scalar-numerator momentum label nodes",
                limits.max_momentum_label_nodes,
                limits,
            )?;
            momentum_label_nodes = checked_add(
                "scalar-numerator momentum label nodes",
                momentum_label_nodes,
                nodes,
            )?;
            check_limit(
                "scalar-numerator momentum label nodes",
                momentum_label_nodes,
                limits.max_momentum_label_nodes,
            )?;
            if let Some(first) = loop_momenta[..second]
                .iter()
                .position(|candidate| candidate == momentum)
            {
                return Err(ScalarNumeratorError::DuplicateLoopMomentum { first, second });
            }
            if contains_head(momentum.as_view(), dot_head) {
                return Err(ScalarNumeratorError::ScalarProductHeadInLoopMomentum {
                    momentum: second,
                });
            }
        }

        let mut scalar_products = Vec::new();
        scalar_products
            .try_reserve_exact(family.coordinates().len())
            .map_err(|_| ScalarNumeratorError::AllocationFailure {
                resource: "scalar-product atoms",
                requested: family.coordinates().len(),
            })?;
        let mut variables = Vec::new();
        variables
            .try_reserve_exact(family.coordinates().len())
            .map_err(|_| ScalarNumeratorError::AllocationFailure {
                resource: "scalar-product polynomial variables",
                requested: family.coordinates().len(),
            })?;
        for coordinate in family.coordinates() {
            let ScalarProductCoordinate::LoopLoop { left, right } = *coordinate else {
                return Err(ScalarNumeratorError::UnsupportedArtifact {
                    detail: "the vacuum family contains a loop-external coordinate",
                });
            };
            let scalar_product = FunctionBuilder::new(dot_head)
                .add_arg(loop_momenta[left].clone())
                .add_arg(loop_momenta[right].clone())
                .finish();
            variables.push(
                PolyVariable::try_from(scalar_product.clone()).map_err(|detail| {
                    ScalarNumeratorError::NonPolynomialScalarProducts { detail }
                })?,
            );
            scalar_products.push(scalar_product);
        }

        Ok(Self {
            dot_head,
            loop_momenta,
            scalar_products,
            scalar_product_variables: Arc::new(variables),
            limits,
        })
    }
}

fn validate_dot_head(dot_head: Symbol) -> Result<(), ScalarNumeratorError> {
    let violation = if dot_head.get_wildcard_level() != 0 {
        Some(ScalarProductHeadViolation::Wildcard)
    } else if dot_head.is_builtin() {
        Some(ScalarProductHeadViolation::BuiltIn)
    } else if !dot_head.is_exportable() {
        Some(ScalarProductHeadViolation::CustomBehavior)
    } else if !dot_head.get_aliases().is_empty() {
        Some(ScalarProductHeadViolation::Aliases)
    } else if !dot_head.get_tags().is_empty() {
        Some(ScalarProductHeadViolation::Tags)
    } else if !matches!(dot_head.get_data(), UserData::None) {
        Some(ScalarProductHeadViolation::UserData)
    } else {
        let attributes = dot_head.get_attributes();
        let symmetric = attributes.as_slice() == [SymbolAttribute::Symmetric];
        let linear = attributes.as_slice() == [SymbolAttribute::Symmetric, SymbolAttribute::Linear];
        if symmetric || linear {
            None
        } else {
            Some(ScalarProductHeadViolation::Attributes)
        }
    };
    match violation {
        Some(violation) => Err(ScalarNumeratorError::InvalidScalarProductHead { violation }),
        None => Ok(()),
    }
}
