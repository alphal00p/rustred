//! Dense reduced weights (`denominators=dense`): one numerator and one monic
//! denominator per lane, normalized by a polynomial gcd after every product
//! and sum. A straightforward reference for the partial-fraction
//! representation; its cost grows with the square of the weight degrees.
use super::super::lift::Image;
use super::super::{CoefRef, error};
use super::Representation;
use super::basis::{Basis, LaneTables};
use super::lane::{Lane, Raw};
use crate::reduction::ReductionError;

/// Reduced `(numerator, monic denominator)` per lane; empty is zero.
#[derive(Clone, Debug, Default)]
pub(super) struct DenseWeight(Vec<(Raw, Raw)>);

pub(super) struct Dense<'a> {
    lanes: &'a [LaneTables],
    basis: &'a Basis,
    max_degree: usize,
}

impl<'a> Dense<'a> {
    pub fn new(lanes: &'a [LaneTables], basis: &'a Basis, max_degree: usize) -> Self {
        Self {
            lanes,
            basis,
            max_degree,
        }
    }

    /// Coprime numerator and monic denominator.
    fn normalize(
        &self,
        lane: usize,
        mut numerator: Raw,
        mut denominator: Raw,
    ) -> Result<(Raw, Raw), ReductionError> {
        let arithmetic = &self.lanes[lane].lane;
        Lane::trim(&mut numerator);
        if numerator.is_empty() {
            return Ok((vec![], vec![arithmetic.one()]));
        }
        let gcd = arithmetic.gcd(&numerator, &denominator);
        if gcd.len() > 1 {
            numerator = arithmetic.divrem_monic(&numerator, &gcd).0;
            denominator = arithmetic.divrem_monic(&denominator, &gcd).0;
        }
        let lead = arithmetic.make_monic(&mut denominator);
        for c in numerator.iter_mut() {
            *c = arithmetic.mul(*c, lead);
        }
        if numerator.len() > self.max_degree + 1 || denominator.len() > self.max_degree + 1 {
            return Err(error(format!(
                "a modular_polynomial weight exceeds max_degree={}",
                self.max_degree
            )));
        }
        Ok((numerator, denominator))
    }

    /// The coefficient `c` in one lane, unreduced.
    fn value(&self, lane: usize, c: CoefRef) -> (Raw, Raw) {
        let tables = &self.lanes[lane];
        let arithmetic = &tables.lane;
        if c.is_constant() {
            return (vec![tables.constants[c.index()]], vec![arithmetic.one()]);
        }
        let mut denominator = vec![arithmetic.one()];
        for &(factor, exponent) in &self.basis.coefficients[c.index()].factors {
            let monic = &tables.basis.monic[factor as usize];
            for _ in 0..exponent {
                denominator = arithmetic.poly_mul(&denominator, monic);
            }
        }
        (tables.numerators[c.index()].clone(), denominator)
    }

    fn lane_product(
        &self,
        lane: usize,
        active: &(Raw, Raw),
        c: CoefRef,
    ) -> Result<(Raw, Raw), ReductionError> {
        let arithmetic = &self.lanes[lane].lane;
        let (numerator, denominator) = self.value(lane, c);
        self.normalize(
            lane,
            arithmetic.poly_mul(&active.0, &numerator),
            arithmetic.poly_mul(&active.1, &denominator),
        )
    }
}

impl Representation for Dense<'_> {
    type Weight = DenseWeight;

    fn coefficient(&mut self, c: CoefRef) -> Result<DenseWeight, ReductionError> {
        (0..self.lanes.len())
            .map(|lane| {
                let (numerator, denominator) = self.value(lane, c);
                self.normalize(lane, numerator, denominator)
            })
            .collect::<Result<_, _>>()
            .map(DenseWeight)
    }

    fn activate(&mut self, _: &mut DenseWeight) {}

    fn product(&mut self, active: &DenseWeight, c: CoefRef) -> Result<DenseWeight, ReductionError> {
        if active.0.is_empty() {
            return Ok(DenseWeight::default());
        }
        active
            .0
            .iter()
            .enumerate()
            .map(|(lane, value)| self.lane_product(lane, value, c))
            .collect::<Result<_, _>>()
            .map(DenseWeight)
    }

    fn add_product(
        &mut self,
        destination: &mut DenseWeight,
        active: &DenseWeight,
        c: CoefRef,
    ) -> Result<(), ReductionError> {
        let product = self.product(active, c)?;
        if destination.0.is_empty() {
            *destination = product;
            return Ok(());
        }
        if product.0.is_empty() {
            return Ok(());
        }
        for (lane, (sum, term)) in destination.0.iter_mut().zip(product.0).enumerate() {
            let arithmetic = &self.lanes[lane].lane;
            let numerator = arithmetic.poly_add(
                &arithmetic.poly_mul(&sum.0, &term.1),
                &arithmetic.poly_mul(&term.0, &sum.1),
            );
            *sum = self.normalize(lane, numerator, arithmetic.poly_mul(&sum.1, &term.1))?;
        }
        Ok(())
    }

    fn is_zero(&self, weight: &DenseWeight) -> bool {
        weight.0.iter().all(|(numerator, _)| numerator.is_empty())
    }

    fn words(&self, weight: &DenseWeight) -> usize {
        weight.0.iter().map(|(n, d)| n.len() + d.len()).sum()
    }

    fn image(&mut self, weight: &DenseWeight, lane: usize) -> Image {
        let arithmetic = &self.lanes[lane].lane;
        match weight.0.get(lane) {
            Some((numerator, denominator)) => Image {
                numerator: arithmetic.standard_poly(numerator),
                denominator: arithmetic.standard_poly(denominator),
            },
            None => Image {
                numerator: vec![],
                denominator: vec![1],
            },
        }
    }
}
