use super::*;

pub(super) fn context(scope: &str, arity: usize) -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(
        &CoefficientContext::new(std::iter::empty::<&str>()),
        scope,
        arity,
    )
    .unwrap()
}

pub(super) fn ordering(mask: &[bool], limits: InvolutiveLimits) -> OreOrderingAdapter {
    OreOrderingAdapter::try_new(
        OrderingPolicy::default(),
        Mask::try_new(mask.iter().copied()).unwrap(),
        limits,
    )
    .unwrap()
}

pub(super) fn completed_ordering(
    mask: &[bool],
    completed: &CompletedIbpSourceRows,
    limits: InvolutiveLimits,
) -> OreOrderingAdapter {
    OreOrderingAdapter::try_new_for_completed(
        OrderingPolicy::default(),
        Mask::try_new(mask.iter().copied()).unwrap(),
        completed,
        limits,
    )
    .unwrap()
}

pub(super) fn shift(values: &[u64], limits: InvolutiveLimits) -> ForwardShift {
    ForwardShift::try_new(values.iter().copied(), limits).unwrap()
}

pub(super) fn build_mixed_relation(
    context: &IndexedCoefficientContext,
    reverse: bool,
) -> ParametricRelation {
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let numerator = context.add(&n0, &n1).unwrap();
    let denominator = context.add(&n1, &context.integer(2)).unwrap();
    let first = context.div(&numerator, &denominator).unwrap();
    let second = context.sub(&n0, &n1).unwrap();
    let mut terms: Vec<([i64; 2], IndexedCoefficient)> = vec![([-2, 1], first), ([1, -3], second)];
    if reverse {
        terms.reverse();
    }
    let mut builder = RelationBuilder::new(
        Arc::new("ordinary-chart-lift-test-family".to_owned()),
        RowId::Derived {
            label: Arc::from("mixed-source"),
        },
        context,
    );
    for (values, coefficient) in terms {
        builder
            .add_term(
                context,
                IndexShift::try_new(values, 2).unwrap(),
                coefficient,
                RelationLimits::default(),
            )
            .unwrap();
    }
    builder.finish()
}

pub(super) fn build_multi_guard_relation(
    context: &IndexedCoefficientContext,
    reverse: bool,
) -> ParametricRelation {
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let first = context
        .div(
            &context.one(),
            &context.add(&n0, &context.integer(2)).unwrap(),
        )
        .unwrap();
    let second = context
        .div(
            &context.one(),
            &context.add(&n1, &context.integer(3)).unwrap(),
        )
        .unwrap();
    let mut terms: Vec<([i64; 2], IndexedCoefficient)> = vec![([-1, 2], first), ([2, -1], second)];
    if reverse {
        terms.reverse();
    }
    let mut builder = RelationBuilder::new(
        Arc::new("ordinary-chart-lift-test-family".to_owned()),
        RowId::Derived {
            label: Arc::from("multi-guard-source"),
        },
        context,
    );
    for (values, coefficient) in terms {
        builder
            .add_term(
                context,
                IndexShift::try_new(values, 2).unwrap(),
                coefficient,
                RelationLimits::default(),
            )
            .unwrap();
    }
    builder.finish()
}

pub(super) fn lift_relation(
    relation: &ParametricRelation,
    source_ordinal: usize,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: OrdinaryChartLiftLimits,
) -> Result<LiftedOrdinarySource, OrdinaryChartLiftError> {
    preflight_relation(relation, source_ordinal, ordering, context, limits)?;
    build_lifted_source(
        relation,
        source_ordinal,
        ordering,
        context,
        limits.involutive,
    )
}

pub(super) fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct InputSymbolicMetrics {
    pub(super) terms: usize,
    pub(super) exponent_cells: usize,
    pub(super) retained_bytes: usize,
    pub(super) max_polynomial_terms: usize,
    pub(super) max_exponent: u16,
    pub(super) max_integer_bits: usize,
    pub(super) max_row_terms: usize,
    pub(super) max_row_exponent_cells: usize,
    pub(super) max_row_retained_bytes: usize,
}

impl InputSymbolicMetrics {
    fn observe_polynomial(&mut self, polynomial: &CoefficientPolynomial, wrapper_bytes: usize) {
        self.terms += polynomial.coefficients.len();
        self.exponent_cells += polynomial.exponents.len();
        self.retained_bytes += wrapper_bytes
            + polynomial.coefficients.len() * std::mem::size_of::<Integer>()
            + polynomial.exponents.len() * std::mem::size_of::<u16>();
        self.max_polynomial_terms = self.max_polynomial_terms.max(polynomial.coefficients.len());
        self.max_exponent = self
            .max_exponent
            .max(polynomial.exponents.iter().copied().max().unwrap_or(0));
        for coefficient in &polynomial.coefficients {
            let bits = match coefficient {
                Integer::Single(value) => {
                    u64::from(i64::BITS - value.unsigned_abs().leading_zeros())
                }
                Integer::Double(value) => {
                    u64::from(i128::BITS - value.unsigned_abs().leading_zeros())
                }
                Integer::Large(value) => u64::from(value.significant_bits()),
            };
            let bits = usize::try_from(bits).unwrap();
            self.max_integer_bits = self.max_integer_bits.max(bits);
            if matches!(coefficient, Integer::Large(_)) {
                self.retained_bytes += bits.div_ceil(8);
            }
        }
    }
}

pub(super) fn input_symbolic_metrics(relations: &[ParametricRelation]) -> InputSymbolicMetrics {
    let mut batch = InputSymbolicMetrics::default();
    for relation in relations {
        let row_start = batch.terms;
        let row_cell_start = batch.exponent_cells;
        let row_byte_start = batch.retained_bytes;
        for coefficient in relation.terms().values() {
            batch.observe_polynomial(
                &coefficient.raw().numerator,
                std::mem::size_of::<IndexedCoefficient>(),
            );
            batch.observe_polynomial(&coefficient.raw().denominator, 0);
        }
        for condition in relation.nonzero_conditions() {
            batch.observe_polynomial(
                condition.polynomial().raw(),
                std::mem::size_of::<IndexedPolynomial>()
                    + std::mem::size_of::<Arc<IndexedPolynomial>>(),
            );
        }
        batch.max_row_terms = batch.max_row_terms.max(batch.terms - row_start);
        batch.max_row_exponent_cells = batch
            .max_row_exponent_cells
            .max(batch.exponent_cells - row_cell_start);
        batch.max_row_retained_bytes = batch
            .max_row_retained_bytes
            .max(batch.retained_bytes - row_byte_start);
    }
    batch
}
