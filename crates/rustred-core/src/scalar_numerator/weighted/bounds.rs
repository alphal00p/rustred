//! Conservative admission for native rational collection, not a CAS kernel.
use crate::scalar_numerator::ScalarNumeratorLimits;
use crate::scalar_numerator::error::{
    ScalarNumeratorError as Error, check_limit, checked_add, checked_mul,
};
use symbolica::atom::AtomView;
use symbolica::prelude::Rational;

#[derive(Clone, Copy)]
struct Shape {
    numerator: usize,
    denominator: usize,
    degree: usize,
}

pub(crate) struct Work {
    used: usize,
    limits: ScalarNumeratorLimits,
}
impl Work {
    pub fn new(limits: ScalarNumeratorLimits) -> Self {
        Self { used: 0, limits }
    }
    fn charge(&mut self, count: usize) -> Result<(), Error> {
        self.used = checked_add("weighted collection work", self.used, count)?;
        check_limit(
            "weighted collection work",
            self.used,
            self.limits.exact_algebra.max_term_operations,
        )
    }
    pub fn preflight(&mut self, atom: AtomView<'_>) -> Result<(), Error> {
        shape(atom, self.limits, self).map(|_| ())
    }
}

fn multiply(
    left: Shape,
    right: Shape,
    limits: ScalarNumeratorLimits,
    work: &mut Work,
) -> Result<Shape, Error> {
    let shape = Shape {
        numerator: checked_mul(
            "weighted numerator term bound",
            left.numerator,
            right.numerator,
        )?,
        denominator: checked_mul(
            "weighted denominator term bound",
            left.denominator,
            right.denominator,
        )?,
        degree: checked_add("weighted polynomial degree", left.degree, right.degree)?,
    };
    work.charge(checked_add(
        "weighted product work",
        shape.numerator,
        shape.denominator,
    )?)?;
    admit(shape, limits)
}
fn admit(shape: Shape, limits: ScalarNumeratorLimits) -> Result<Shape, Error> {
    check_limit(
        "weighted numerator term bound",
        shape.numerator,
        limits.max_polynomial_terms,
    )?;
    check_limit(
        "weighted denominator term bound",
        shape.denominator,
        limits.max_polynomial_terms,
    )?;
    check_limit(
        "weighted polynomial degree",
        shape.degree,
        limits.max_scalar_product_degree,
    )?;
    Ok(shape)
}
fn shape(
    atom: AtomView<'_>,
    limits: ScalarNumeratorLimits,
    work: &mut Work,
) -> Result<Shape, Error> {
    work.charge(1)?;
    match atom {
        AtomView::Num(_) => admit(
            Shape {
                numerator: 1,
                denominator: 1,
                degree: 0,
            },
            limits,
        ),
        AtomView::Var(_) | AtomView::Fun(_) => admit(
            Shape {
                numerator: 1,
                denominator: 1,
                degree: 1,
            },
            limits,
        ),
        AtomView::Mul(product) => {
            let mut bound = Shape {
                numerator: 1,
                denominator: 1,
                degree: 0,
            };
            for factor in product.iter() {
                let next = shape(factor, limits, work)?;
                bound = multiply(bound, next, limits, work)?;
            }
            Ok(bound)
        }
        AtomView::Add(sum) => {
            let mut bound = Shape {
                numerator: 0,
                denominator: 1,
                degree: 0,
            };
            for term in sum.iter() {
                let next = shape(term, limits, work)?;
                bound = admit(
                    Shape {
                        numerator: checked_add(
                            "weighted sum term bound",
                            checked_mul(
                                "weighted sum term bound",
                                bound.numerator,
                                next.denominator,
                            )?,
                            checked_mul(
                                "weighted sum term bound",
                                next.numerator,
                                bound.denominator,
                            )?,
                        )?,
                        denominator: checked_mul(
                            "weighted sum denominator bound",
                            bound.denominator,
                            next.denominator,
                        )?,
                        degree: checked_add(
                            "weighted sum degree bound",
                            bound.degree,
                            next.degree,
                        )?,
                    },
                    limits,
                )?;
                work.charge(checked_add(
                    "weighted sum work",
                    bound.numerator,
                    bound.denominator,
                )?)?;
            }
            Ok(bound)
        }
        AtomView::Pow(power) => {
            let Ok(exponent) = Rational::try_from(power.get_exp()) else {
                return admit(
                    Shape {
                        numerator: 1,
                        denominator: 1,
                        degree: 1,
                    },
                    limits,
                );
            };
            if !exponent.is_integer() {
                return admit(
                    Shape {
                        numerator: 1,
                        denominator: 1,
                        degree: 1,
                    },
                    limits,
                );
            }
            let exponent = exponent
                .numerator()
                .to_i64()
                .ok_or(Error::ScalarProductExponentOverflow)?;
            let mut magnitude = exponent.unsigned_abs();
            let mut base = shape(power.get_base(), limits, work)?;
            if exponent < 0 {
                std::mem::swap(&mut base.numerator, &mut base.denominator);
            }
            let mut result = Shape {
                numerator: 1,
                denominator: 1,
                degree: 0,
            };
            while magnitude != 0 {
                if magnitude & 1 != 0 {
                    result = multiply(result, base, limits, work)?;
                }
                magnitude >>= 1;
                if magnitude != 0 {
                    base = multiply(base, base, limits, work)?;
                }
            }
            Ok(result)
        }
    }
}
