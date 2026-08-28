//! Exact one-loop massive-vacuum tadpole reduction.
//!
//! For the positive-Euclidean denominator `D=k^2+m2`, the native total
//! derivative at power `a-1` gives
//!
//! ```text
//! T(a) / T(1) = product_(n=1)^(a-1) (2*n-d)/(2*n*m2).
//! ```
//!
//! Non-positive powers are scaleless in dimensional regularization.  The
//! reducer deliberately works in a caller-supplied [`CoefficientContext`], so
//! a higher-loop factorization can compose its result without translating
//! Symbolica variable maps.

use std::fmt;

use crate::legacy_oracle_support::coefficient_degree::{
    coefficient_variable_degrees, symbolica_coefficient_degree_is_representable,
};
use crate::{Coefficient, CoefficientContext, SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT};

/// Resource bounds for one direct tadpole reduction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OneLoopTadpoleConfig {
    /// Maximum number of recurrence factors `a-1`.
    pub max_recurrence_steps: usize,
    /// Maximum exact coefficient operations charged by the direct product.
    /// Each recurrence factor is conservatively charged four operations.
    pub max_coefficient_operations: usize,
    /// Conservative bound on dense polynomial term updates hidden inside the
    /// high-level rational operations.  For `s` recurrence factors RustRed
    /// charges `4*s*(s+1)`, covering both sides of every linear-factor
    /// multiplication before cancellation. This is not a bound on polynomial
    /// GCD cost or integer bit complexity; the separate hard recurrence cap
    /// keeps those finite on the direct-product path.
    pub max_dense_term_operations: u128,
    /// Caller-selected coefficient-degree ceiling.  Symbolica's `u16` hard
    /// exponent ceiling is always enforced as well.
    pub max_coefficient_degree: u128,
}

impl Default for OneLoopTadpoleConfig {
    fn default() -> Self {
        Self {
            max_recurrence_steps: 256,
            max_coefficient_operations: 1_024,
            max_dense_term_operations: 300_000,
            max_coefficient_degree: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        }
    }
}

/// Exact work retained for one one-loop reduction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OneLoopTadpoleStats {
    recurrence_steps: usize,
    coefficient_operations: usize,
    dense_term_operation_bound: u128,
    coefficient_degree_bound: u128,
}

impl OneLoopTadpoleStats {
    pub const fn recurrence_steps(self) -> usize {
        self.recurrence_steps
    }

    pub const fn coefficient_operations(self) -> usize {
        self.coefficient_operations
    }

    pub const fn dense_term_operation_bound(self) -> u128 {
        self.dense_term_operation_bound
    }

    pub const fn coefficient_degree_bound(self) -> u128 {
        self.coefficient_degree_bound
    }
}

/// A replayable ratio `T(power)/T(1)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OneLoopTadpoleReduction {
    power: i32,
    coefficient: Coefficient,
    stats: OneLoopTadpoleStats,
}

impl OneLoopTadpoleReduction {
    pub const fn power(&self) -> i32 {
        self.power
    }

    /// Exact ratio to the unit-power tadpole.  It is zero for a scaleless
    /// non-positive power.
    pub const fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub const fn stats(&self) -> OneLoopTadpoleStats {
        self.stats
    }

    #[doc(hidden)]
    pub fn with_coefficient_for_replay(&self, coefficient: Coefficient) -> Self {
        let mut candidate = self.clone();
        candidate.coefficient = coefficient;
        candidate
    }

    #[doc(hidden)]
    pub fn with_power_for_replay(&self, power: i32) -> Self {
        let mut candidate = self.clone();
        candidate.power = power;
        candidate
    }

    #[doc(hidden)]
    pub fn with_coefficient_operations_for_replay(&self, operations: usize) -> Self {
        let mut candidate = self.clone();
        candidate.stats.coefficient_operations = operations;
        candidate
    }
}

/// Reusable direct reducer in one authenticated coefficient context.
#[derive(Clone, Debug)]
pub struct OneLoopTadpoleReducer {
    coefficients: CoefficientContext,
    config: OneLoopTadpoleConfig,
    dimension_parameter: String,
    mass_parameter: String,
    dimension: Coefficient,
    mass: Coefficient,
}

impl OneLoopTadpoleReducer {
    pub const SCHEMA: &'static str = "rustred-one-loop-tadpole-v1";

    /// Construct the standard `Q(d,m2)` service in a fresh context.
    pub fn build(config: OneLoopTadpoleConfig) -> Result<Self, OneLoopTadpoleError> {
        Self::new(CoefficientContext::new(["d", "m2"]), "d", "m2", config)
    }

    /// Construct the service in the exact caller-supplied coefficient context.
    pub fn new(
        coefficients: CoefficientContext,
        dimension_parameter: impl Into<String>,
        mass_parameter: impl Into<String>,
        config: OneLoopTadpoleConfig,
    ) -> Result<Self, OneLoopTadpoleError> {
        let dimension_parameter = dimension_parameter.into();
        let mass_parameter = mass_parameter.into();
        if dimension_parameter == mass_parameter {
            return Err(OneLoopTadpoleError::ParameterAlias {
                name: dimension_parameter,
            });
        }
        let dimension = coefficients
            .parameter(&dimension_parameter)
            .ok_or_else(|| OneLoopTadpoleError::MissingParameter {
                name: dimension_parameter.clone(),
            })?;
        let mass = coefficients.parameter(&mass_parameter).ok_or_else(|| {
            OneLoopTadpoleError::MissingParameter {
                name: mass_parameter.clone(),
            }
        })?;
        if config.max_coefficient_degree > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
            return Err(OneLoopTadpoleError::ResourceLimit {
                resource: "configured coefficient degree",
                requested: config.max_coefficient_degree,
                limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            });
        }
        Ok(Self {
            coefficients,
            config,
            dimension_parameter,
            mass_parameter,
            dimension,
            mass,
        })
    }

    pub const fn coefficients(&self) -> &CoefficientContext {
        &self.coefficients
    }

    pub const fn config(&self) -> OneLoopTadpoleConfig {
        self.config
    }

    pub fn dimension_parameter(&self) -> &str {
        &self.dimension_parameter
    }

    pub fn mass_parameter(&self) -> &str {
        &self.mass_parameter
    }

    pub const fn dimension(&self) -> &Coefficient {
        &self.dimension
    }

    pub const fn mass(&self) -> &Coefficient {
        &self.mass
    }

    /// Validate the complete request before constructing a recurrence factor.
    pub fn preflight(&self, power: i32) -> Result<OneLoopTadpoleStats, OneLoopTadpoleError> {
        if power <= 1 {
            return Ok(OneLoopTadpoleStats::default());
        }
        let recurrence_steps = usize::try_from(i64::from(power) - 1).map_err(|_| {
            OneLoopTadpoleError::ArithmeticOverflow {
                resource: "tadpole recurrence steps",
            }
        })?;
        if recurrence_steps > self.config.max_recurrence_steps {
            return Err(OneLoopTadpoleError::ResourceLimit {
                resource: "tadpole recurrence steps",
                requested: recurrence_steps as u128,
                limit: self.config.max_recurrence_steps as u128,
            });
        }
        let coefficient_operations =
            recurrence_steps
                .checked_mul(4)
                .ok_or(OneLoopTadpoleError::ArithmeticOverflow {
                    resource: "tadpole coefficient operations",
                })?;
        if coefficient_operations > self.config.max_coefficient_operations {
            return Err(OneLoopTadpoleError::ResourceLimit {
                resource: "tadpole coefficient operations",
                requested: coefficient_operations as u128,
                limit: self.config.max_coefficient_operations as u128,
            });
        }
        let steps = recurrence_steps as u128;
        let dense_term_operation_bound = steps
            .checked_add(1)
            .and_then(|next| steps.checked_mul(next))
            .and_then(|pairs| pairs.checked_mul(4))
            .ok_or(OneLoopTadpoleError::ArithmeticOverflow {
                resource: "tadpole dense term operations",
            })?;
        if dense_term_operation_bound > self.config.max_dense_term_operations {
            return Err(OneLoopTadpoleError::ResourceLimit {
                resource: "tadpole dense term operations",
                requested: dense_term_operation_bound,
                limit: self.config.max_dense_term_operations,
            });
        }
        let coefficient_degree_bound = self.coefficient_degree_bound(recurrence_steps as u128);
        if coefficient_degree_bound > self.config.max_coefficient_degree
            || !symbolica_coefficient_degree_is_representable(coefficient_degree_bound)
        {
            return Err(OneLoopTadpoleError::ResourceLimit {
                resource: "tadpole coefficient degree",
                requested: coefficient_degree_bound,
                limit: self
                    .config
                    .max_coefficient_degree
                    .min(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT),
            });
        }
        Ok(OneLoopTadpoleStats {
            recurrence_steps,
            coefficient_operations,
            dense_term_operation_bound,
            coefficient_degree_bound,
        })
    }

    /// Reduce one integer power to the unit-power tadpole.
    pub fn reduce_power(&self, power: i32) -> Result<OneLoopTadpoleReduction, OneLoopTadpoleError> {
        let stats = self.preflight(power)?;
        if power <= 0 {
            return Ok(OneLoopTadpoleReduction {
                power,
                coefficient: self.coefficients.zero(),
                stats,
            });
        }
        let mut coefficient = self.coefficients.one();
        for n in 1..i64::from(power) {
            let two_n = self.coefficients.integer(2 * n);
            coefficient = &(&coefficient * &(&two_n - &self.dimension)) / &(&two_n * &self.mass);
        }
        Ok(OneLoopTadpoleReduction {
            power,
            coefficient,
            stats,
        })
    }

    /// Independently recompute a retained reduction and its native recurrence
    /// identity.  This rejects coefficient, power, or work-stat tampering.
    pub fn replay(&self, reduction: &OneLoopTadpoleReduction) -> Result<(), OneLoopTadpoleError> {
        let rebuilt = self.reduce_power(reduction.power)?;
        if rebuilt != *reduction {
            return Err(OneLoopTadpoleError::ReplayMismatch {
                power: reduction.power,
            });
        }
        if reduction.power > 1 {
            let previous = self.reduce_power(reduction.power - 1)?;
            let n = i64::from(reduction.power - 1);
            let two_n = self.coefficients.integer(2 * n);
            let residual = &(&reduction.coefficient * &(&two_n * &self.mass))
                - &(&previous.coefficient * &(&two_n - &self.dimension));
            if !residual.is_zero() {
                return Err(OneLoopTadpoleError::RecurrenceMismatch {
                    power: reduction.power,
                });
            }
        }
        Ok(())
    }

    fn coefficient_degree_bound(&self, steps: u128) -> u128 {
        coefficient_variable_degrees(&self.dimension)
            .into_iter()
            .zip(coefficient_variable_degrees(&self.mass))
            .map(
                |(
                    (dimension_numerator, dimension_denominator),
                    (mass_numerator, mass_denominator),
                )| {
                    let factor_numerator = dimension_numerator
                        .max(dimension_denominator)
                        .saturating_add(mass_denominator);
                    let factor_denominator = dimension_denominator.saturating_add(mass_numerator);
                    steps.saturating_mul(factor_numerator.max(factor_denominator))
                },
            )
            .max()
            .unwrap_or(0)
    }
}

/// Typed context, resource, arithmetic, and replay failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OneLoopTadpoleError {
    MissingParameter {
        name: String,
    },
    ParameterAlias {
        name: String,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    ArithmeticOverflow {
        resource: &'static str,
    },
    ReplayMismatch {
        power: i32,
    },
    RecurrenceMismatch {
        power: i32,
    },
}

impl fmt::Display for OneLoopTadpoleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingParameter { name } => {
                write!(
                    formatter,
                    "one-loop tadpole context has no parameter {name}"
                )
            }
            Self::ParameterAlias { name } => write!(
                formatter,
                "one-loop dimension and mass parameters both name {name}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "one-loop {resource} request {requested} exceeds limit {limit}"
            ),
            Self::ArithmeticOverflow { resource } => {
                write!(formatter, "one-loop {resource} counter overflow")
            }
            Self::ReplayMismatch { power } => {
                write!(
                    formatter,
                    "one-loop reduction at power {power} does not replay"
                )
            }
            Self::RecurrenceMismatch { power } => write!(
                formatter,
                "one-loop native recurrence has nonzero residual at power {power}"
            ),
        }
    }
}

impl std::error::Error for OneLoopTadpoleError {}
