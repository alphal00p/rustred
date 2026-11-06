//! Closed-form kernels of the Insertion, SeriesParallel and Carpet lanes (owner:
//! M2-T4 rr-kernel): [`gamma`] splits Gamma functions into tags and exact
//! rationals, [`bubble`] is the one-loop bubble, [`carpet`] the outer loop of a
//! carpet. The steps are `propagator::{insertion, series_parallel, carpet}`.
//!
//! # Signs: the kernels are evaluated natively in Minkowski variables
//!
//! Convention C2 of `docs/p_integrals.md` offers two equivalent forms. This
//! module implements the second one: every scalar product of a kernel is the
//! Minkowski product of the presentation, `(Y.Y')_M = -(Y.Y')_E`, so the rewrite
//! into child slots is [`Presentation::express_scalar_product`] verbatim, its
//! constants included (`q.q = -1`).
//!
//! The two forms agree monomial by monomial. Take a parent key with integer
//! parts `n_j` and any output monomial, a product of `s` scalar products of the
//! Euclidean formula. Writing each as `-c_i + Sum_k r_ik D_k^E` for the Minkowski
//! rewrite `c_i + Sum_k r_ik D_k`, the Wick ratio `(-1)^{Sum n_parent - Sum n_child}`
//! of C2 cancels the sign of every selected slot and leaves `(-1)^s` on the
//! Minkowski expansion. What remains per kernel:
//!
//! - Bubble with lines `alpha`, `beta` and rank `n`. The parent contributes
//!   `(-1)^{n1 + n2 + n}`, the merged line of integer power
//!   `n1 + n2 - 2 - sigma` its own sign, the `n - sigma` scalar products
//!   `(-1)^{n - sigma}`: an even total. The Minkowski kernel has no sign at all,
//!   with `(P^2)^sigma` as one more scalar product per order.
//! - Carpet with removed lines `r`, `third`. The parent contributes
//!   `(-1)^{n_r + n_3 + n}`, the `n - sigma - j` scalar products
//!   `(-1)^{n - sigma - j}`, eq. 18 itself `(-1)^j`: the Minkowski kernel carries
//!   `(-1)^{n_r + n_3 + sigma}`.
//! - A line absorbed into `q`, and the line at the trivial node, of integer
//!   power `n`: `(-1)^n`, nothing else, by the eps-line rule of C1. For the merged
//!   line that is `(-1)^{n1 + n2}` in front of `(P^2)^sigma = (-1)^sigma`.
//!
//! # Tags
//!
//! A kernel's tag depends on the eps units alone. Steps add it to the key's
//! absolute tag: every child and every terminal of [`PStepOut`] carries an
//! absolute tag, checked by [`absolute_tag`].
//!
//! # Conditions
//!
//! A kernel adds no `PStepOut::conditions`: the denominators of its weights are
//! products of linear factors in `d`, never the zero polynomial, and they stay
//! inside the `Q(d)` weight, so a sample on a pole fails as a division there.
//!
//! # Versions
//!
//! `P_INSERTION_KERNEL_VERSION` and `P_CARPET_KERNEL_VERSION` name what a step says
//! on a key. The test `the_kernel_versions_are_pinned` of `tests/p_library/kernel.rs`
//! holds a digest of the steps on fixed keys next to each version, so a semantic
//! change without a bump fails there.
//!
//! [`Presentation::express_scalar_product`]: crate::propagator::presentation::Presentation::express_scalar_product
//! [`PStepOut`]: crate::propagator::step::PStepOut
pub mod bubble;
pub mod carpet;
pub mod gamma;

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use smallvec::SmallVec;
use symbolica::prelude::Rational;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};

use super::P_GAMMA_TAG_WIDTH;
use super::census::SignedAffineLoopMap;
use super::error::PError;
use super::library::{PLibrary, POwnerSpec};
use super::presentation::{AffineSlotForm, EdgeKind, EdgeMap, Presentation};
use super::step::{PChild, PStage, PStepOut};
use super::tag::GammaTag;
use gamma::{algebra, rational_coefficient};

/// Most entries of one kernel memo: beyond it, values are computed and not kept.
///
/// It counts entries, not bytes, per step: a step has one memo of weights and one
/// of numerator tables, a table per ISP exponent vector, whose size grows with the
/// rank and the arity of the child. No limit is hit at 3L; at 4L and 5L the
/// resident size is what to watch, not this constant.
pub(crate) const MEMO_CAPACITY: usize = 1 << 16;

/// Hits and misses of a kernel memo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoCounters {
    pub hits: u64,
    pub misses: u64,
}

impl std::ops::Add for MemoCounters {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            hits: self.hits + other.hits,
            misses: self.misses + other.misses,
        }
    }
}

fn poisoned() -> PError {
    PError::Unsupported("a kernel memo lock is poisoned".to_owned())
}

pub(crate) fn read_memo<K: Eq + Hash, V: Clone>(
    memo: &RwLock<HashMap<K, V>>,
    key: &K,
) -> Result<Option<V>, PError> {
    Ok(memo.read().map_err(|_| poisoned())?.get(key).cloned())
}

pub(crate) fn write_memo<K: Eq + Hash, V: Clone>(
    memo: &RwLock<HashMap<K, V>>,
    key: K,
    value: &V,
    capacity: usize,
) -> Result<(), PError> {
    let mut memo = memo.write().map_err(|_| poisoned())?;
    if memo.len() < capacity {
        memo.insert(key, value.clone());
    }
    Ok(())
}

/// The key's tag plus a kernel's relative tag, checked: an absolute tag is
/// encoded after every add (`GammaTag::add`).
pub(crate) fn absolute_tag(tag: &GammaTag, relative: &GammaTag) -> Result<GammaTag, PError> {
    let absolute = tag.add(relative);
    let mut slots = Vec::with_capacity(P_GAMMA_TAG_WIDTH);
    absolute.encode_dense(&mut slots)?;
    Ok(absolute)
}

/// The child row of a parent momentum without the removed momentum, through the
/// edge map: on an Insertion edge the external column is the parent's `q`, on a
/// Carpet edge the parent's outer loop (see [`EdgeMap`]).
pub(crate) fn child_row(
    map: &SignedAffineLoopMap,
    kind: EdgeKind,
    integrated: usize,
    momentum: &[i64],
) -> Result<Vec<i64>, PError> {
    let external = momentum.len() - 1;
    let dropped = match kind {
        EdgeKind::Insertion => integrated,
        EdgeKind::Carpet => external,
    };
    if momentum[dropped] != 0 {
        return Err(PError::Contract(format!(
            "the momentum {momentum:?} depends on what the edge removes"
        )));
    }
    let mut row: Vec<i64> = (0..external)
        .filter(|&column| column != integrated)
        .map(|column| momentum[column])
        .collect();
    row.push(match kind {
        EdgeKind::Insertion => momentum[external],
        EdgeKind::Carpet => momentum[integrated],
    });
    map.map_momentum(&row)
}

/// A rule edge of an owner with its child presentation.
pub type OwnerEdge = (EdgeMap, Arc<Presentation>);

/// The owner's presentation with its edge maps and their child presentations:
/// what a step constructor needs from a library.
pub(crate) fn owner_edges(
    library: &PLibrary,
    owner: &POwnerSpec,
) -> Result<(Arc<Presentation>, Vec<OwnerEdge>), PError> {
    let missing = |what: String| PError::MalformedLibrary(format!("owner names {what}"));
    let presentation = |id: u32| {
        library
            .presentations()
            .get(id as usize)
            .cloned()
            .ok_or_else(|| missing(format!("presentation {id}")))
    };
    let parent = presentation(owner.presentation.0)?;
    let edges = owner
        .edge_maps
        .iter()
        .map(|&index| {
            let map = library
                .edge_maps()
                .get(index)
                .ok_or_else(|| missing(format!("edge map {index}")))?;
            Ok((map.clone(), presentation(map.child.0)?))
        })
        .collect::<Result<_, PError>>()?;
    Ok((parent, edges))
}

/// The numerators of `slots`: their powers, negated. An ISP at a positive power
/// has no kernel.
pub(crate) fn numerator_powers(powers: &[i64], slots: &[usize]) -> Result<Vec<u32>, PError> {
    slots
        .iter()
        .map(|&slot| {
            u32::try_from(-powers[slot]).map_err(|_| {
                PError::Unsupported(format!(
                    "ISP slot {slot} at the positive power {}",
                    powers[slot]
                ))
            })
        })
        .collect()
}

type Exponents = SmallVec<[u32; 10]>;

/// A polynomial in the slots of one child presentation, with `Rational`
/// coefficients; exponents count factors `D_i`, so they lower the child's powers.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SlotPolynomial {
    terms: HashMap<Exponents, Rational>,
}

impl SlotPolynomial {
    fn one(arity: usize) -> Self {
        Self {
            terms: HashMap::from([(SmallVec::from_elem(0, arity), Rational::one())]),
        }
    }

    fn from_form(form: &AffineSlotForm, arity: usize) -> Self {
        let mut terms = HashMap::with_capacity(form.terms.len() + 1);
        let zero: Exponents = SmallVec::from_elem(0, arity);
        if !form.constant.is_zero() {
            terms.insert(zero.clone(), form.constant.clone());
        }
        for (slot, value) in &form.terms {
            let mut exponents = zero.clone();
            exponents[usize::from(*slot)] = 1;
            terms.insert(exponents, value.clone());
        }
        Self { terms }
    }

    fn mul(&self, other: &Self) -> Self {
        let mut terms: HashMap<Exponents, Rational> = HashMap::new();
        for (left, x) in &self.terms {
            for (right, y) in &other.terms {
                let exponents: Exponents = left.iter().zip(right).map(|(a, b)| a + b).collect();
                let value = x * y;
                match terms.entry(exponents) {
                    Entry::Occupied(mut entry) => {
                        let sum = entry.get() + &value;
                        entry.insert(sum);
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(value);
                    }
                }
            }
        }
        terms.retain(|_, value| !value.is_zero());
        Self { terms }
    }

    fn scale(&mut self, factor: &Rational) {
        for value in self.terms.values_mut() {
            *value = &*value * factor;
        }
    }

    fn add_assign(&mut self, other: &Self) {
        for (exponents, value) in &other.terms {
            match self.terms.entry(exponents.clone()) {
                Entry::Occupied(mut entry) => {
                    let sum = entry.get() + value;
                    entry.insert(sum);
                }
                Entry::Vacant(entry) => {
                    entry.insert(value.clone());
                }
            }
        }
        self.terms.retain(|_, value| !value.is_zero());
    }
}

/// One child monomial of a [`NumeratorTable`]: for each order `s` with a nonzero
/// polynomial, the rational multiplying the kernel weight of that order.
#[derive(Clone, Debug, PartialEq)]
struct TableMonomial {
    exponents: Box<[u32]>,
    orders: SmallVec<[(u32, Rational); 2]>,
}

/// `Sum_s W_s Q_s` for one numerator: `W_s` are the kernel weights, `Q_s` the
/// child polynomials, by child monomial in sorted order.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NumeratorTable {
    monomials: Vec<TableMonomial>,
}

/// The scalar products of a kernel output as polynomials in the child's slots.
///
/// Variables `x_s` are the numerators the kernel acts on. The polynomial of
/// order `s` is `factor^s` times the sum of [`bubble::box_pairings`] of order `s`
/// (contracted pairs `contractions`, every other `x_s` to `residuals[s]`), times
/// the product of `free[i]^{m_i}`, the numerators that the kernel leaves alone.
pub(crate) struct NumeratorRewrite {
    arity: usize,
    contractions: Vec<SlotPolynomial>,
    residuals: Vec<SlotPolynomial>,
    free: Vec<SlotPolynomial>,
    order_factor: Option<SlotPolynomial>,
    tables: RwLock<HashMap<Box<[u32]>, Arc<NumeratorTable>>>,
    hits: AtomicU64,
    misses: AtomicU64,
}

fn triangular(variables: usize, s: usize, t: usize) -> usize {
    s * variables - s * (s + 1) / 2 + t
}

impl NumeratorRewrite {
    /// Tabulate `child`'s forms: `contraction(a, b)` for the variables, one
    /// residual per variable, one form per free numerator, the order factor.
    pub(crate) fn new(
        child: &Presentation,
        variables: &[Vec<i64>],
        residual: &[i64],
        free: &[(Vec<i64>, Vec<i64>)],
        order_factor: Option<(&[i64], &[i64])>,
    ) -> Result<Self, PError> {
        let arity = child.slots.len();
        let form = |a: &[i64], b: &[i64]| {
            Ok::<_, PError>(SlotPolynomial::from_form(
                &child.express_scalar_product(a, b)?,
                arity,
            ))
        };
        let mut contractions = Vec::new();
        for (s, left) in variables.iter().enumerate() {
            for right in &variables[s..] {
                contractions.push(form(left, right)?);
            }
        }
        Ok(Self {
            arity,
            contractions,
            residuals: variables
                .iter()
                .map(|variable| form(variable, residual))
                .collect::<Result<_, _>>()?,
            free: free
                .iter()
                .map(|(left, right)| form(left, right))
                .collect::<Result<_, _>>()?,
            order_factor: order_factor.map(|(a, b)| form(a, b)).transpose()?,
            tables: RwLock::default(),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        })
    }

    /// The table of `box_powers` and `free_powers`, memoised.
    pub(crate) fn table(
        &self,
        box_powers: &[u32],
        free_powers: &[u32],
    ) -> Result<Arc<NumeratorTable>, PError> {
        let key: Box<[u32]> = box_powers.iter().chain(free_powers).copied().collect();
        if let Some(table) = read_memo(&self.tables, &key)? {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Ok(table);
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let table = Arc::new(self.build(box_powers, free_powers));
        write_memo(&self.tables, key, &table, MEMO_CAPACITY)?;
        Ok(table)
    }

    pub(crate) fn counters(&self) -> MemoCounters {
        MemoCounters {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }

    fn build(&self, box_powers: &[u32], free_powers: &[u32]) -> NumeratorTable {
        let mut powers: HashMap<(usize, u32), Arc<SlotPolynomial>> = HashMap::new();
        let variables = self.residuals.len();
        let residual_base = self.contractions.len();
        let free_base = residual_base + variables;
        let factor_base = free_base + self.free.len();
        let base = |index: usize| -> &SlotPolynomial {
            if index < residual_base {
                &self.contractions[index]
            } else if index < free_base {
                &self.residuals[index - residual_base]
            } else if index < factor_base {
                &self.free[index - free_base]
            } else {
                self.order_factor
                    .as_ref()
                    .expect("the order factor is used only when present")
            }
        };
        let arity = self.arity;
        let mut power = move |index: usize, exponent: u32| -> Arc<SlotPolynomial> {
            let mut known = Arc::new(SlotPolynomial::one(arity));
            for e in 1..=exponent {
                known = match powers.get(&(index, e)) {
                    Some(value) => value.clone(),
                    None => {
                        let value = Arc::new(known.mul(base(index)));
                        powers.insert((index, e), value.clone());
                        value
                    }
                };
            }
            known
        };

        let mut prefix = SlotPolynomial::one(arity);
        for (i, &exponent) in free_powers.iter().enumerate() {
            prefix = prefix.mul(&power(free_base + i, exponent));
        }
        let rank: u32 = box_powers.iter().sum();
        let mut monomials: BTreeMap<Exponents, SmallVec<[(u32, Rational); 2]>> = BTreeMap::new();
        for order in 0..=rank / 2 {
            let mut sum = SlotPolynomial::default();
            for pairing in bubble::box_pairings(box_powers, order) {
                let mut term = prefix.clone();
                for &(s, t, count) in &pairing.pairs {
                    let index = triangular(variables, usize::from(s), usize::from(t));
                    term = term.mul(&power(index, count));
                }
                for (s, &exponent) in pairing.rest.iter().enumerate() {
                    if exponent > 0 {
                        term = term.mul(&power(residual_base + s, exponent));
                    }
                }
                term.scale(&pairing.weight);
                sum.add_assign(&term);
            }
            if self.order_factor.is_some() {
                sum = sum.mul(&power(factor_base, order));
            }
            for (exponents, value) in sum.terms {
                monomials.entry(exponents).or_default().push((order, value));
            }
        }
        NumeratorTable {
            monomials: monomials
                .into_iter()
                .map(|(exponents, orders)| TableMonomial {
                    exponents: exponents.into_vec().into_boxed_slice(),
                    orders,
                })
                .collect(),
        }
    }
}

/// What a table turns into child keys: the child's powers before the
/// numerators, and the sign of the removed lines.
pub(crate) struct ChildKeys<'a> {
    pub context: &'a CoefficientContext,
    pub child: &'a Presentation,
    pub base: Vec<i64>,
    pub negate: bool,
    pub tag: &'a GammaTag,
}

impl ChildKeys<'_> {
    /// One [`PChild`] per child monomial of `table` with a nonzero weight.
    pub(crate) fn emit(
        &self,
        table: &NumeratorTable,
        weights: &[Coefficient],
        limits: ExactAlgebraLimits,
        output: &mut PStepOut,
    ) -> Result<(), PError> {
        for monomial in &table.monomials {
            let mut weight = table_weight(self.context, monomial, weights, limits)?;
            if weight.is_zero() {
                continue;
            }
            if self.negate {
                weight = self.context.try_neg(&weight, limits).map_err(algebra)?;
            }
            let powers = self
                .base
                .iter()
                .zip(&monomial.exponents)
                .map(|(power, &exponent)| power - i64::from(exponent))
                .collect();
            output.children.push(PChild {
                stage: PStage::Route,
                presentation: self.child.id,
                powers,
                tag: self.tag.clone(),
                coefficient: weight,
            });
        }
        Ok(())
    }
}

/// `Sum_s rational_s W_s`, one child weight of a table.
fn table_weight(
    context: &CoefficientContext,
    monomial: &TableMonomial,
    weights: &[Coefficient],
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, PError> {
    let mut total = context.zero();
    for (order, value) in &monomial.orders {
        let weight = &weights[*order as usize];
        if weight.is_zero() {
            continue;
        }
        let scale = rational_coefficient(context, value, limits)?;
        let term = context
            .try_mul_validated(weight, &scale, limits)
            .map_err(algebra)?;
        total = context
            .try_add_validated(&total, &term, limits)
            .map_err(algebra)?;
    }
    Ok(total)
}
