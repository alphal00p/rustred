//! The modular lift over Q shared by every modular weight arithmetic.
//!
//! An [`ImageSource`] evaluates one request modulo 64-bit primes. For a set
//! of primes it returns, per prime, the reduced image `numerator /
//! denominator` in `Z_p[d]` (monic denominator) of every terminal coefficient,
//! together with a structural signature: the source's `Structure` (for
//! example the terminal key set and the guard list) and the degrees of every
//! image. [`lift`] then
//!
//! - draws reconstruction primes from a seeded stream and consumes their
//!   images strictly in stream order (images computed ahead of need are kept
//!   for later rounds), so every decision is independent of the thread count;
//! - groups images by signature; the group holding a strict majority of all
//!   usable images leads, the others are unlucky primes (an unusable prime,
//!   reported as `None`, is skipped);
//! - lifts each coefficient of the leading group by CRT and maximal-quotient
//!   rational reconstruction; a coefficient is stable once the candidate
//!   built from the earlier images reproduces a further image (or two
//!   consecutive candidates agree);
//! - once every coefficient of the leading group is stable, checks the
//!   candidates on `verification_primes` fresh primes from a disjoint stream,
//!   never used to build them; a disagreement re-opens the lift;
//! - fails closed after `max_primes` consumed primes (reconstruction,
//!   verification and unusable ones).
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::finite_field::{FiniteFieldCore, PrimeIteratorU64, Zp64};
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::prelude::{Integer, Rational, Z};

use super::executor::Executor;
use super::field::{self, Fe, Poly};
use super::{ModularStatistics, error};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::reduction::ReductionError;
use crate::reduction::ordered_reduction::arithmetic::ModularCommon;

/// Reduced image of one terminal coefficient modulo a prime `p`.
///
/// Coefficients are ascending standard representatives in `0..p`. The
/// denominator is monic, numerator and denominator are coprime, and the
/// numerator has no trailing zero. Zero is the empty numerator over `[1]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Image {
    pub numerator: Vec<u64>,
    pub denominator: Vec<u64>,
}

impl Image {
    /// `(numerator degree, denominator degree)`, `None` for zero.
    pub fn degrees(&self) -> Option<(usize, usize)> {
        (!self.numerator.is_empty()).then(|| (self.numerator.len() - 1, self.denominator.len() - 1))
    }

    /// Numerator coefficients followed by the non-leading denominator
    /// coefficients: the values lifted over Q.
    fn values(&self) -> impl Iterator<Item = u64> + '_ {
        let lower = self.denominator.len().saturating_sub(1);
        self.numerator
            .iter()
            .chain(&self.denominator[..lower])
            .copied()
    }
}

/// Images of every terminal coefficient of one request at one prime, in the
/// order of the structure's terminals.
pub(super) struct PrimeImages<S> {
    pub structure: Arc<S>,
    pub terminals: Vec<Image>,
}

/// Outcome of checking the lifted candidates at one fresh prime.
pub(super) enum Verification<S> {
    /// The source cannot use this prime; another one is drawn.
    Unusable,
    /// Full images; the driver compares the signature and every coefficient.
    Image(PrimeImages<S>),
    /// The source compared the candidates itself (e.g. at fresh sample
    /// points of the prime): the terminals whose candidate disagreed.
    Checked { disagreeing: Vec<usize> },
}

/// Per-prime evaluation of one Ordered request (see the module docs).
pub(super) trait ImageSource {
    /// Weight-independent part of the signature of an image set; images
    /// with different structures are never combined.
    type Structure: PartialEq + Send + Sync;
    type Error: From<ReductionError>;

    /// Images at distinct primes, one entry per prime in order; `None` marks
    /// a prime this source cannot use (e.g. it divides a constant
    /// denominator). Called with primes never used before.
    fn images(
        &mut self,
        primes: &[u64],
        executor: &Executor,
        statistics: &mut ModularStatistics,
    ) -> Result<Vec<Option<PrimeImages<Self::Structure>>>, Self::Error>;

    /// Primes per round worth computing together on `threads` workers.
    /// Extra images are kept for later rounds, never discarded early.
    fn round_capacity(&self, threads: usize) -> usize {
        threads
    }

    /// Fewest primes worth requesting in one round (within the capacity);
    /// by default rounds double from a single image.
    fn round_minimum(&self, _threads: usize) -> usize {
        1
    }

    /// Check `candidates` (one per terminal of `structure`) at fresh primes.
    /// The default computes full images, which the driver compares.
    fn verify(
        &mut self,
        primes: &[u64],
        _structure: &Self::Structure,
        _candidates: &[&Candidate],
        executor: &Executor,
        statistics: &mut ModularStatistics,
    ) -> Result<Vec<Verification<Self::Structure>>, Self::Error> {
        Ok(self
            .images(primes, executor, statistics)?
            .into_iter()
            .map(|image| image.map_or(Verification::Unusable, Verification::Image))
            .collect())
    }
}

/// A coefficient lifted over Q.
#[derive(Clone, Debug)]
pub(super) struct Candidate {
    /// `None` for zero.
    degrees: Option<(usize, usize)>,
    /// Numerator coefficients (ascending), then the non-leading coefficients
    /// of the monic denominator.
    rationals: Vec<Rational>,
    /// The same function in the coefficient context's normal form.
    pub coefficient: Coefficient,
}

impl Candidate {
    fn new(
        context: &CoefficientContext,
        degrees: Option<(usize, usize)>,
        rationals: Vec<Rational>,
    ) -> Option<Self> {
        let Some((numerator_degree, denominator_degree)) = degrees else {
            return Some(Self {
                degrees,
                rationals,
                coefficient: context.zero(),
            });
        };
        let mut lcm = Integer::one();
        for r in &rationals {
            lcm = lcm.lcm(r.denominator_ref());
        }
        let scale = |r: &Rational| -> Integer {
            let (quotient, _) = lcm.quot_rem(r.denominator_ref());
            r.numerator_ref() * &quotient
        };
        let template = &context.template().numerator;
        let mut numerator = template.zero();
        for (degree, r) in rationals[..=numerator_degree].iter().enumerate() {
            if !r.is_zero() {
                numerator.append_monomial(scale(r), &exponent(context, degree)?);
            }
        }
        let mut denominator = template.zero();
        for (degree, r) in rationals[numerator_degree + 1..].iter().enumerate() {
            if !r.is_zero() {
                denominator.append_monomial(scale(r), &exponent(context, degree)?);
            }
        }
        denominator.append_monomial(lcm.clone(), &exponent(context, denominator_degree)?);
        let coefficient = Coefficient::from_num_den(numerator, denominator, &Z, true);
        Some(Self {
            degrees,
            rationals,
            coefficient,
        })
    }

    /// The candidate modulo `field` as `(numerator, monic denominator)`;
    /// `None` when the prime divides a denominator of its coefficients.
    pub fn reduce(&self, field: &Zp64) -> Option<(Poly, Poly)> {
        let mut values = Vec::with_capacity(self.rationals.len() + 1);
        for r in &self.rationals {
            let denominator = field::reduce(field, r.denominator_ref());
            if field.is_zero(&denominator) {
                return None;
            }
            values.push(field.div(&field::reduce(field, r.numerator_ref()), &denominator));
        }
        let Some((numerator_degree, _)) = self.degrees else {
            return Some((vec![], vec![field.one()]));
        };
        let mut denominator = values.split_off(numerator_degree + 1);
        denominator.push(field.one());
        Some((values, denominator))
    }

    /// Whether the candidate reduces to `image` modulo `prime`.
    fn agrees(&self, prime: u64, image: &Image) -> bool {
        if self.degrees != image.degrees() {
            return false;
        }
        let field = Zp64::new(prime);
        self.rationals.iter().zip(image.values()).all(|(r, v)| {
            let n = field::reduce(&field, r.numerator_ref());
            let d = field::reduce(&field, r.denominator_ref());
            !field.is_zero(&d) && n == field.mul(&field.to_element(v), &d)
        })
    }
}

fn exponent(context: &CoefficientContext, degree: usize) -> Option<Vec<u16>> {
    if context.variables().is_empty() {
        (degree == 0).then(Vec::new)
    } else {
        u16::try_from(degree).ok().map(|d| vec![d])
    }
}

/// CRT accumulation of one terminal coefficient within one signature group.
struct Lift {
    degrees: Option<(usize, usize)>,
    residues: Vec<Integer>,
    modulus: Integer,
    images: usize,
    candidate: Option<Candidate>,
    stable: bool,
}

impl Lift {
    fn new(degrees: Option<(usize, usize)>) -> Self {
        Self {
            degrees,
            residues: vec![],
            modulus: Integer::one(),
            images: 0,
            candidate: None,
            stable: false,
        }
    }

    fn add(&mut self, prime: u64, image: &Image, context: &CoefficientContext) {
        // A candidate built from earlier primes that also matches this image
        // is confirmed by a prime it was not built from.
        let confirmed = self
            .candidate
            .as_ref()
            .is_some_and(|candidate| candidate.agrees(prime, image));
        let modulus = Integer::from(prime);
        if self.images == 0 {
            self.residues = image.values().map(Integer::from).collect();
            self.modulus = modulus;
        } else {
            for (residue, value) in self.residues.iter_mut().zip(image.values()) {
                *residue = Integer::chinese_remainder(
                    Integer::from(value),
                    std::mem::replace(residue, Integer::zero()),
                    modulus.clone(),
                    self.modulus.clone(),
                );
            }
            self.modulus = &self.modulus * &modulus;
        }
        self.images += 1;
        if confirmed {
            self.stable = true;
            return;
        }
        let candidate = self
            .residues
            .iter()
            .map(|residue| {
                Rational::maximal_quotient_reconstruction(residue, &self.modulus, None).ok()
            })
            .collect::<Option<Vec<_>>>()
            .and_then(|rationals| Candidate::new(context, self.degrees, rationals));
        self.stable = self.images >= 2
            && candidate.is_some()
            && candidate.as_ref().map(|c| &c.coefficient)
                == self.candidate.as_ref().map(|c| &c.coefficient);
        self.candidate = candidate;
    }
}

/// Images sharing one signature.
struct Group<S> {
    structure: Arc<S>,
    degrees: Vec<Option<(usize, usize)>>,
    images: usize,
    lifts: Vec<Lift>,
}

impl<S> Group<S> {
    fn ready(&self) -> bool {
        self.lifts.iter().all(|lift| lift.stable)
    }

    fn candidates(&self) -> Vec<&Candidate> {
        self.lifts
            .iter()
            .map(|lift| lift.candidate.as_ref().expect("stable candidate"))
            .collect()
    }
}

/// The verified lift of the leading signature group.
pub(super) struct Lifted<S> {
    pub structure: Arc<S>,
    pub coefficients: Vec<Coefficient>,
}

/// Deterministic stream of distinct 64-bit primes.
struct PrimeStream(PrimeIteratorU64);

impl PrimeStream {
    /// Primes above `base + (random >> shift)`; the streams below use
    /// disjoint ranges below 2^63.
    fn new(seed: u64, salt: u64, base: u64, shift: u32) -> Self {
        let mut rng = StdRng::seed_from_u64(seed ^ salt);
        Self(PrimeIteratorU64::new(base + (rng.random::<u64>() >> shift)))
    }

    fn take(&mut self, count: usize) -> Result<Vec<u64>, ReductionError> {
        (0..count)
            .map(|_| {
                self.0
                    .next()
                    .ok_or_else(|| error("exhausted 64-bit primes"))
            })
            .collect()
    }
}

/// Reconstruction primes: `[2^62, 2^62 + 2^61)` plus the primes consumed.
const LIFT_SALT: u64 = 0x6c69_6674;
/// Verification primes: `[2^62 + 2^61, 2^62 + 2^61 + 2^59)`.
const VERIFICATION_SALT: u64 = 0x7665_7269_6679;

struct Driver<'a, S> {
    context: &'a CoefficientContext,
    common: &'a ModularCommon,
    executor: &'a Executor,
    groups: Vec<Group<S>>,
    /// Usable images absorbed into some group.
    usable: usize,
    /// Primes consumed so far (bounded by `max_primes`).
    consumed: usize,
}

impl<S: PartialEq + Send + Sync> Driver<'_, S> {
    /// The group with the most images (earliest on ties) if it holds a strict
    /// majority of all usable images and every coefficient is stable.
    fn leader(&self) -> Option<usize> {
        let (index, group) = self
            .groups
            .iter()
            .enumerate()
            .max_by(|(i, a), (j, b)| a.images.cmp(&b.images).then(j.cmp(i)))?;
        (2 * group.images > self.usable && group.ready()).then_some(index)
    }

    fn leading_images(&self) -> usize {
        self.groups.iter().map(|g| g.images).max().unwrap_or(0)
    }

    fn exhausted(&self) -> ReductionError {
        error(format!(
            "modular reconstruction did not stabilize within {} primes \
             ({} usable images in {} signature groups, the largest with {})",
            self.common.max_primes,
            self.usable,
            self.groups.len(),
            self.leading_images()
        ))
    }

    fn consume(&mut self, count: usize) -> Result<(), ReductionError> {
        if self.consumed + count > self.common.max_primes {
            return Err(self.exhausted());
        }
        self.consumed += count;
        Ok(())
    }

    /// Absorb a usable image into its signature group; returns the group.
    fn absorb(
        &mut self,
        prime: u64,
        image: PrimeImages<S>,
        statistics: &mut ModularStatistics,
    ) -> usize {
        let degrees: Vec<_> = image.terminals.iter().map(Image::degrees).collect();
        let index = match self.groups.iter().position(|group| {
            (Arc::ptr_eq(&group.structure, &image.structure)
                || *group.structure == *image.structure)
                && group.degrees == degrees
        }) {
            Some(index) => index,
            None => {
                self.groups.push(Group {
                    structure: image.structure,
                    lifts: degrees.iter().map(|d| Lift::new(*d)).collect(),
                    degrees,
                    images: 0,
                });
                self.groups.len() - 1
            }
        };
        let started = Instant::now();
        let group = &mut self.groups[index];
        group.images += 1;
        let context = self.context;
        let terminals = &image.terminals;
        self.executor.map_mut(&mut group.lifts, |t, lift| {
            lift.add(prime, &terminals[t], context)
        });
        statistics.lift_seconds += started.elapsed().as_secs_f64();
        self.usable += 1;
        index
    }

    /// Check the leader on fresh primes; false re-opens the lift.
    fn verify<I: ImageSource<Structure = S>>(
        &mut self,
        leader: usize,
        source: &mut I,
        primes: &mut PrimeStream,
        statistics: &mut ModularStatistics,
    ) -> Result<bool, I::Error> {
        let mut agreed = 0;
        while agreed < self.common.verification_primes {
            let missing = self.common.verification_primes - agreed;
            self.consume(missing)?;
            let fresh = primes.take(missing)?;
            statistics.verification_primes += missing;
            let results = {
                let group = &self.groups[leader];
                source.verify(
                    &fresh,
                    &group.structure,
                    &group.candidates(),
                    self.executor,
                    statistics,
                )?
            };
            if results.len() != fresh.len() {
                return Err(error("image source returned a wrong number of verifications").into());
            }
            let mut rejected = false;
            for (prime, result) in fresh.into_iter().zip(results) {
                let disagreeing = match result {
                    Verification::Unusable => {
                        statistics.unusable_primes += 1;
                        continue;
                    }
                    Verification::Checked { disagreeing } => disagreeing,
                    Verification::Image(image) => {
                        let group = &self.groups[leader];
                        let same = (Arc::ptr_eq(&group.structure, &image.structure)
                            || *group.structure == *image.structure)
                            && image.terminals.len() == group.lifts.len()
                            && image
                                .terminals
                                .iter()
                                .zip(&group.degrees)
                                .all(|(image, degrees)| image.degrees() == *degrees);
                        let disagreeing: Vec<usize> = if same {
                            group
                                .lifts
                                .iter()
                                .zip(&image.terminals)
                                .enumerate()
                                .filter(|(_, (lift, image))| {
                                    !lift
                                        .candidate
                                        .as_ref()
                                        .is_some_and(|c| c.agrees(prime, image))
                                })
                                .map(|(t, _)| t)
                                .collect()
                        } else {
                            vec![]
                        };
                        if !same || !disagreeing.is_empty() {
                            // A usable fresh image that refutes the leader is
                            // also a further reconstruction image (or vote).
                            let index = self.absorb(prime, image, statistics);
                            for &t in &disagreeing {
                                self.groups[index].lifts[t].stable = false;
                            }
                            rejected = true;
                            statistics.verification_failures += disagreeing.len().max(1);
                            continue;
                        }
                        disagreeing
                    }
                };
                if disagreeing.is_empty() {
                    agreed += 1;
                } else {
                    statistics.verification_failures += disagreeing.len();
                    for t in disagreeing {
                        self.groups[leader].lifts[t].stable = false;
                    }
                    rejected = true;
                }
            }
            if rejected {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Lift the images of `source` over Q (see the module docs).
pub(super) fn lift<I: ImageSource>(
    source: &mut I,
    context: &CoefficientContext,
    common: &ModularCommon,
    executor: &Executor,
    statistics: &mut ModularStatistics,
) -> Result<Lifted<I::Structure>, I::Error> {
    let mut lift_primes = PrimeStream::new(common.seed, LIFT_SALT, 1 << 62, 3);
    let mut verification_primes =
        PrimeStream::new(common.seed, VERIFICATION_SALT, (1 << 62) + (1 << 61), 5);
    let mut driver = Driver {
        context,
        common,
        executor,
        groups: vec![],
        usable: 0,
        consumed: 0,
    };
    // Images computed ahead of need, consumed strictly in prime order.
    let mut pending: VecDeque<(u64, Option<PrimeImages<I::Structure>>)> = VecDeque::new();
    let leader = loop {
        if let Some(leader) = driver.leader() {
            if driver.verify(leader, source, &mut verification_primes, statistics)? {
                break leader;
            }
            continue;
        }
        if pending.is_empty() {
            // Double the images of the leading group, within what the
            // threads can compute together; never beyond the prime budget.
            let budget = common.max_primes.saturating_sub(driver.consumed);
            if budget == 0 {
                return Err(driver.exhausted().into());
            }
            let count = driver
                .leading_images()
                .max(source.round_minimum(executor.threads()).max(1))
                .min(source.round_capacity(executor.threads()).max(1))
                .min(budget);
            let primes = lift_primes.take(count)?;
            let images = source.images(&primes, executor, statistics)?;
            if images.len() != primes.len() {
                return Err(error("image source returned a wrong number of images").into());
            }
            statistics.rounds += 1;
            pending.extend(primes.into_iter().zip(images));
        }
        let (prime, image) = pending.pop_front().expect("pending image");
        driver.consume(1)?;
        match image {
            None => statistics.unusable_primes += 1,
            Some(image) => {
                driver.absorb(prime, image, statistics);
                statistics.primes += 1;
            }
        }
    };
    statistics.discarded_images += pending.len();
    let Driver { groups, usable, .. } = driver;
    let group = groups.into_iter().nth(leader).expect("leading group");
    statistics.unlucky_images += usable - group.images;
    let mut coefficients = Vec::with_capacity(group.lifts.len());
    for lift in group.lifts {
        let candidate = lift.candidate.expect("verified candidate");
        if let Some((n, d)) = candidate.degrees {
            statistics.max_numerator_degree = statistics.max_numerator_degree.max(n);
            statistics.max_denominator_degree = statistics.max_denominator_degree.max(d);
        }
        for c in candidate
            .coefficient
            .numerator
            .coefficients
            .iter()
            .chain(candidate.coefficient.denominator.coefficients.iter())
        {
            statistics.max_coefficient_bits = statistics
                .max_coefficient_bits
                .max(c.abs().significant_bits() as usize);
        }
        coefficients.push(candidate.coefficient);
    }
    Ok(Lifted {
        structure: group.structure,
        coefficients,
    })
}

/// Convert a reduced Montgomery-form image to standard representatives.
pub(super) fn image_from(field: &Zp64, numerator: &[Fe], denominator: &[Fe]) -> Image {
    Image {
        numerator: numerator.iter().map(|c| field::to_u64(field, c)).collect(),
        denominator: denominator
            .iter()
            .map(|c| field::to_u64(field, c))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reduction::ordered_reduction::modular::field::DenseRational;

    /// Images of fixed rational functions, with scripted bad behaviour by
    /// image index (in the order the driver requests them).
    struct Scripted {
        functions: Vec<DenseRational>,
        produced: usize,
        unusable: Vec<usize>,
        /// Leading numerator coefficient dropped (an unlucky prime).
        degree_drop: Vec<usize>,
        /// A different structure (e.g. a spurious zero erasure).
        odd_structure: Vec<usize>,
        /// Unrelated values at every prime.
        garbage: bool,
        verifications: usize,
        /// `round_minimum`, and the sizes of the image requests.
        minimum: usize,
        requests: Vec<usize>,
    }

    impl Scripted {
        fn new(context: &CoefficientContext, functions: &[&str]) -> Self {
            Self {
                functions: functions
                    .iter()
                    .map(|text| DenseRational::from_coefficient(&context.coefficient_fixture(text)))
                    .collect(),
                produced: 0,
                unusable: vec![],
                degree_drop: vec![],
                odd_structure: vec![],
                garbage: false,
                verifications: 0,
                minimum: 1,
                requests: vec![],
            }
        }

        fn image(&self, prime: u64, function: &DenseRational, index: usize) -> Image {
            let f = Zp64::new(prime);
            let reduce = |values: &[Integer]| -> Poly {
                let mut poly: Poly = values.iter().map(|c| field::reduce(&f, c)).collect();
                field::trim(&f, &mut poly);
                poly
            };
            let mut numerator = reduce(&function.numerator);
            let mut denominator = reduce(&function.denominator);
            let lead = f.inv(denominator.last().unwrap());
            for c in numerator.iter_mut().chain(denominator.iter_mut()) {
                *c = f.mul(&*c, &lead);
            }
            if self.garbage {
                numerator[0] = f.to_element((prime ^ index as u64) % 1_000_003);
            }
            if self.degree_drop.contains(&index) {
                numerator.pop();
            }
            image_from(&f, &numerator, &denominator)
        }
    }

    impl ImageSource for Scripted {
        type Structure = u8;
        type Error = ReductionError;

        fn images(
            &mut self,
            primes: &[u64],
            _: &Executor,
            _: &mut ModularStatistics,
        ) -> Result<Vec<Option<PrimeImages<u8>>>, ReductionError> {
            self.requests.push(primes.len());
            Ok(primes
                .iter()
                .map(|&prime| {
                    let index = self.produced;
                    self.produced += 1;
                    (!self.unusable.contains(&index)).then(|| PrimeImages {
                        structure: Arc::new(u8::from(self.odd_structure.contains(&index))),
                        terminals: self
                            .functions
                            .iter()
                            .map(|function| self.image(prime, function, index))
                            .collect(),
                    })
                })
                .collect())
        }

        fn round_minimum(&self, _: usize) -> usize {
            self.minimum
        }

        fn verify(
            &mut self,
            primes: &[u64],
            _: &u8,
            _: &[&Candidate],
            executor: &Executor,
            statistics: &mut ModularStatistics,
        ) -> Result<Vec<Verification<u8>>, ReductionError> {
            self.verifications += primes.len();
            Ok(self
                .images(primes, executor, statistics)?
                .into_iter()
                .map(|image| image.map_or(Verification::Unusable, Verification::Image))
                .collect())
        }
    }

    const FUNCTIONS: [&str; 3] = [
        "(123456789012345678901*d^2-5*d+7)/(7*d+3)",
        "-11/13",
        "d^3/(2*d^2-9)",
    ];

    fn run(
        source: &mut Scripted,
        common: ModularCommon,
    ) -> Result<(Lifted<u8>, ModularStatistics), ReductionError> {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut statistics = ModularStatistics::default();
        let executor = Executor::new(common.threads)?;
        lift(source, &context, &common, &executor, &mut statistics)
            .map(|lifted| (lifted, statistics))
    }

    fn expected() -> Vec<Coefficient> {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        FUNCTIONS
            .iter()
            .map(|text| context.coefficient_fixture(text))
            .collect()
    }

    #[test]
    fn majority_rejects_unlucky_and_structurally_different_images() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut source = Scripted::new(&context, &FUNCTIONS);
        source.unusable = vec![1];
        source.degree_drop = vec![0];
        source.odd_structure = vec![3];
        let (lifted, statistics) = run(&mut source, ModularCommon::default()).unwrap();
        assert_eq!(lifted.coefficients, expected());
        assert_eq!(*lifted.structure, 0);
        // Images 0 and 3 are outvoted, image 1 is unusable.
        assert_eq!(statistics.unlucky_images, 2);
        assert_eq!(statistics.unusable_primes, 1);
        assert_eq!(statistics.verification_primes, 1);
        assert_eq!(statistics.max_coefficient_bits, 67);
    }

    #[test]
    fn a_refuting_verification_reopens_the_lift() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut clean = Scripted::new(&context, &FUNCTIONS);
        let (_, reference) = run(&mut clean, ModularCommon::default()).unwrap();
        // The first verification image has another structure.
        let mut source = Scripted::new(&context, &FUNCTIONS);
        source.odd_structure = vec![reference.primes];
        let (lifted, statistics) = run(&mut source, ModularCommon::default()).unwrap();
        assert_eq!(lifted.coefficients, expected());
        assert_eq!(statistics.verification_failures, 1);
        assert_eq!(statistics.verification_primes, 2);
        assert_eq!(source.verifications, 2);
    }

    #[test]
    fn inconsistent_images_fail_closed_at_the_prime_budget() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut source = Scripted::new(&context, &FUNCTIONS);
        source.garbage = true;
        let error = run(
            &mut source,
            ModularCommon {
                max_primes: 12,
                ..Default::default()
            },
        )
        .err()
        .expect("never stable");
        assert!(matches!(error, ReductionError::ModularWeights { .. }));
        assert!(
            error
                .to_string()
                .contains("did not stabilize within 12 primes"),
            "{error}"
        );
        assert_eq!(source.produced, 12);
    }

    #[test]
    fn rounds_double_within_the_source_capacity() {
        const LARGE: [&str; 2] = [
            "(1234567890123456789012345678901234567890123456789012345678901234567890123456789*d+1)\
             /(98765432109876543210987654321098765432109876543210987654321*d-3)",
            "1/3",
        ];
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut sequential = Scripted::new(&context, &LARGE);
        let (reference, one) = run(&mut sequential, ModularCommon::default()).unwrap();
        let mut parallel = Scripted::new(&context, &LARGE);
        let (lifted, four) = run(
            &mut parallel,
            ModularCommon {
                threads: 4,
                ..Default::default()
            },
        )
        .unwrap();
        let expected: Vec<_> = LARGE
            .iter()
            .map(|text| context.coefficient_fixture(text))
            .collect();
        assert_eq!(reference.coefficients, expected);
        assert_eq!(lifted.coefficients, expected);
        // The same images are consumed; more threads only compute ahead.
        assert!(one.primes > 4, "{}", one.primes);
        assert_eq!(
            (one.primes, one.verification_primes, one.unlucky_images),
            (four.primes, four.verification_primes, four.unlucky_images)
        );
        assert_eq!((one.rounds, one.discarded_images), (one.primes, 0));
        assert!(four.rounds < one.rounds);
        assert_eq!(parallel.produced, four.primes + four.discarded_images + 1);
    }

    #[test]
    fn a_round_minimum_fills_the_first_round_without_changing_the_result() {
        const LARGE: [&str; 1] =
            ["(123456789012345678901234567890123456789*d+1)/(98765432109876543210*d-3)"];
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut doubling = Scripted::new(&context, &LARGE);
        let (reference, one) = run(
            &mut doubling,
            ModularCommon {
                threads: 4,
                ..Default::default()
            },
        )
        .unwrap();
        let mut filled = Scripted::new(&context, &LARGE);
        filled.minimum = 4;
        let (lifted, four) = run(
            &mut filled,
            ModularCommon {
                threads: 4,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(lifted.coefficients, reference.coefficients);
        assert_eq!(
            (one.primes, one.verification_primes),
            (four.primes, four.verification_primes)
        );
        // Rounds double from one image, or start at the minimum; the
        // capacity (the thread count) bounds both; the last request is the
        // verification prime.
        assert_eq!(doubling.requests[..3], [1, 1, 2]);
        assert_eq!(filled.requests[0], 4);
        assert!(
            filled.requests[..filled.requests.len() - 1]
                .iter()
                .all(|&n| n == 4)
        );
        assert!(four.rounds < one.rounds);
    }
}
