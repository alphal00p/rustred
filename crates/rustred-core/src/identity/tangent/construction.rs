use crate::family::IntegralFamily;
use crate::identity::{IntegralShift, RowId, TranslatedSourceRequest};

use super::error::{add, limit, mul, reserve};
use super::polynomial::PolynomialWork;
use super::{
    TangentPolynomial, TangentSourceError as Error, TangentSourceLimits, TangentSourcePlan,
    TangentSourceSpec, WeightedTangentSource,
};

impl TangentSourcePlan {
    /// Construct signed native 2x2 minors and exact monomial source requests.
    ///
    /// Multiplying a vector component by `D^alpha` means translating its
    /// original ordinary source by `recenter-alpha`, not multiplying a reduced
    /// RHS. The native translated source automatically retains the product rule.
    pub fn try_new(
        family: &IntegralFamily,
        spec: &TangentSourceSpec,
        limits: TangentSourceLimits,
    ) -> Result<Self, Error> {
        let arity = family.denominator_count();
        limit("denominator variables", arity, limits.max_denominators)?;
        if spec.differentiated_loop >= family.loop_count() {
            return Err(Error::InvalidInput {
                detail: "differentiated loop is absent",
            });
        }
        if spec.recenter.len() != arity {
            return Err(Error::InvalidInput {
                detail: "source recenter has wrong arity",
            });
        }
        if spec.protected_denominators[0] == spec.protected_denominators[1]
            || spec.protected_denominators.iter().any(|&i| i >= arity)
        {
            return Err(Error::InvalidInput {
                detail: "two distinct existing protected denominators are required",
            });
        }
        let mut ordinals = [0; 3];
        for (j, q) in spec.contractions.iter().enumerate() {
            if spec.contractions[..j].contains(q) {
                return Err(Error::InvalidInput {
                    detail: "contraction directions must be distinct",
                });
            }
            let index = family
                .contraction_momenta()
                .iter()
                .position(|c| c == q)
                .ok_or(Error::InvalidInput {
                    detail: "contraction momentum is absent",
                })?;
            ordinals[j] = add(
                "ordinary source ordinal",
                mul("ordinary source ordinal", index, family.loop_count())?,
                spec.differentiated_loop,
            )?;
        }
        let mut work = PolynomialWork::new(family, limits)?;
        let mut matrix = Vec::new();
        reserve(&mut matrix, 6, "protected derivative matrix")?;
        for &axis in &spec.protected_denominators {
            for &q in &spec.contractions {
                matrix.push(work.affine(family.derivative_contraction(
                    axis,
                    spec.differentiated_loop,
                    q,
                )?)?);
            }
        }
        let mut vector = Vec::new();
        reserve(&mut vector, 3, "tangent vector")?;
        for j in 0..3 {
            let columns: Vec<_> = (0..3).filter(|&k| k != j).collect();
            vector.push(work.minor(
                [
                    &matrix[columns[0]],
                    &matrix[columns[1]],
                    &matrix[3 + columns[0]],
                    &matrix[3 + columns[1]],
                ],
                j % 2 != 0,
            )?);
        }
        if let Some(coordinate) = spec.multiplier {
            let expansion =
                family.scalar_product_expansion(family.coordinate_index(coordinate)?)?;
            let multiplier = work.affine(&expansion)?;
            for component in &mut vector {
                *component = work.multiply(component, &multiplier)?;
            }
        }
        if vector.iter().all(|p| p.is_zero()) {
            return Err(Error::DegenerateProtectedSystem);
        }
        for row in 0..2 {
            let mut sum = work.template.zero();
            for j in 0..3 {
                let product = work.multiply(&matrix[row * 3 + j], &vector[j])?;
                sum = work.sum(&sum, &product)?;
            }
            if !sum.is_zero() {
                return Err(Error::TangencyVerificationFailed);
            }
        }
        let count = vector
            .iter()
            .try_fold(0, |a, p| add("selected source requests", a, p.nterms()))?;
        limit(
            "selected source requests",
            count,
            limits.max_selected_sources,
        )?;
        limit(
            "source request coordinates",
            mul("source request coordinates", count, arity)?,
            limits.max_retained_coordinate_cells,
        )?;
        let mut contributions = Vec::new();
        reserve(&mut contributions, count, "weighted source requests")?;
        for (j, component) in vector.iter().enumerate() {
            let contraction_momentum = ordinals[j] / family.loop_count();
            for (coefficient, exponents) in component
                .coefficients
                .iter()
                .zip(component.exponents_iter())
            {
                let mut offset = Vec::new();
                reserve(&mut offset, arity, "source shift")?;
                for (&center, &exponent) in spec.recenter.values().iter().zip(exponents) {
                    offset.push(center.checked_sub(i64::from(exponent)).ok_or(
                        Error::ResourceOverflow {
                            resource: "signed source shift",
                        },
                    )?);
                }
                contributions.push(WeightedTangentSource {
                    request: TranslatedSourceRequest::new(
                        ordinals[j],
                        IntegralShift::try_new(offset)?,
                    ),
                    row_id: RowId::OrdinaryIbp {
                        contraction_momentum,
                        differentiated_loop: spec.differentiated_loop,
                    },
                    weight: coefficient.clone(),
                });
            }
        }
        contributions.sort_unstable_by(|a, b| a.request.cmp(&b.request));
        if contributions
            .windows(2)
            .any(|p| p[0].request == p[1].request)
        {
            return Err(Error::InvalidInput {
                detail: "native polynomial monomials repeated a source request",
            });
        }
        let mut it = vector.into_iter();
        let vector = [
            TangentPolynomial {
                raw: it.next().expect("three constructed components"),
            },
            TangentPolynomial {
                raw: it.next().expect("three constructed components"),
            },
            TangentPolynomial {
                raw: it.next().expect("three constructed components"),
            },
        ];
        Ok(Self {
            family: family.fingerprint_owner(),
            arity,
            spec: spec.clone(),
            vector,
            contributions,
            limits,
        })
    }
}
