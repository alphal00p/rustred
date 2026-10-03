//! Finite coefficient-column assembly; polynomial multiplication is Symbolica.
use super::*;
use symbolica::prelude::{MultivariatePolynomial, PolyVariable};

type Polynomial = MultivariatePolynomial<Field, u16>;
pub(super) struct Preflight {
    pub directions: usize,
    n: usize,
    degree: usize,
    protected: Vec<usize>,
    unknowns: usize,
    columns: usize,
    input_nonzeros: usize,
    coordinate_cells: usize,
}
impl Preflight {
    pub fn report(&self) -> Value {
        json!({"denominators":self.n,"ordinary_directions":self.directions,"degree":self.degree,"protected_axes":self.protected,"unknowns":self.unknowns,"constraint_columns_upper_bound":self.columns,"augmented_width_upper_bound":self.columns+self.unknowns,"input_nonzeros_upper_bound_before_identity":self.input_nonzeros,"monomial_coordinate_cells":self.coordinate_cells,"preflight_completed_before_matrix_allocation":true})
    }
}
pub(super) fn preflight(f: &IntegralFamily, r: &Value) -> Result<Preflight> {
    let n = f.denominator_count();
    let degree = number(r, "max_weight_degree")?;
    require(degree <= 1, "degree policy exceeded")?;
    admit(r, "max_denominators", n)?;
    let directions = mul(f.loop_count(), f.contraction_momenta().len())?;
    admit(r, "max_ordinary_rows", directions)?;
    let protected = r["protected_axes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap() as usize)
        .collect::<Vec<_>>();
    let monos = if degree == 0 { 1 } else { add(n, 1)? };
    let unknowns = mul(directions, monos)?;
    let one_axis = if degree == 0 {
        n
    } else {
        add(n, mul(n, n - 1)? / 2)?
    };
    let columns = mul(protected.len(), one_axis)?;
    let input_nonzeros = mul(mul(unknowns, protected.len())?, add(n, 1)?)?;
    let all_columns = if degree == 0 {
        add(n, 1)?
    } else {
        mul(add(n, 1)?, add(n, 2)?)? / 2
    };
    // Enumeration buffers, retained column/unknown keys and native affine
    // derivative exponents. Native scratch is separately outer-RSS bounded.
    let affine_cells = mul(mul(directions, protected.len())?, add(n, 1)?)?;
    let coordinate_cells = mul(
        n,
        add(
            add(monos, all_columns)?,
            add(add(columns, unknowns)?, affine_cells)?,
        )?,
    )?;
    admit(r, "max_unknowns", unknowns)?;
    admit(r, "max_constraint_columns", columns)?;
    admit(r, "max_augmented_columns", add(columns, unknowns)?)?;
    admit(r, "max_input_nonzeros", add(input_nonzeros, unknowns)?)?;
    admit(r, "max_polynomial_operations", input_nonzeros)?;
    admit(r, "max_coordinate_cells", coordinate_cells)?;
    admit(
        r,
        "max_conditions",
        mul(mul(directions, protected.len())?, add(n, 1)?)?,
    )?;
    Ok(Preflight {
        directions,
        n,
        degree,
        protected,
        unknowns,
        columns,
        input_nonzeros,
        coordinate_cells,
    })
}

pub(super) fn monomials(n: usize, degree: usize) -> Result<Vec<Vec<u16>>> {
    require(degree <= 2, "monomial enumeration exceeds admitted degree")?;
    let mut out = vec![vec![0; n]];
    if degree > 0 {
        for i in 0..n {
            let mut m = vec![0; n];
            m[i] = 1;
            out.push(m);
        }
    }
    if degree > 1 {
        for i in 0..n {
            for j in i..n {
                let mut m = vec![0; n];
                m[i] += 1;
                m[j] += 1;
                out.push(m);
            }
        }
    }
    Ok(out)
}
pub(super) struct Unknown {
    pub row_id: RowId,
    pub ordinal: usize,
    pub alpha: Vec<u16>,
}
pub(super) struct System {
    pub matrix: Matrix,
    pub unknowns: Vec<Unknown>,
    pub columns: Value,
    pub report: Value,
}

fn affine(
    template: &Polynomial,
    expansion: &rustred::family::DenominatorExpansion,
    base: &CoefficientContext,
    r: &Value,
) -> Result<Polynomial> {
    let n = expansion.denominator_coefficients().len();
    let mut p = template.constant(expansion.constant().clone());
    checked(base.validate_with_limits(expansion.constant(), ExactAlgebraLimits::default()))?;
    for (i, c) in expansion.denominator_coefficients().iter().enumerate() {
        checked(base.validate_with_limits(c, ExactAlgebraLimits::default()))?;
        if !c.is_zero() {
            let mut exp = vec![0; n];
            exp[i] = 1;
            p.append_monomial(c.clone(), &exp);
        }
    }
    admit(r, "max_polynomial_terms", p.nterms())?;
    Ok(p)
}

pub(super) fn assemble(
    f: &IntegralFamily,
    ids: &BTreeMap<RowId, usize>,
    p: &Preflight,
    r: &Value,
) -> Result<System> {
    require(ids.len() == p.directions, "native inventory size mismatch")?;
    let base = f.coefficient_context();
    let vars = Arc::new((0..p.n).map(PolyVariable::Temporary).collect());
    let template = Polynomial::new(&field(), None, vars);
    let monos = monomials(p.n, p.degree)?;
    let all_columns = monomials(p.n, p.degree + 1)?;
    let mut columns = BTreeMap::new();
    for &axis in &p.protected {
        for alpha in &all_columns {
            if alpha[axis] == 0 {
                let next = checked(u32::try_from(columns.len()))?;
                columns.insert((axis, alpha.clone()), next);
            }
        }
    }
    require(
        columns.len() == p.columns,
        "constraint enumeration differs from preflight",
    )?;
    let mut derivatives = BTreeMap::new();
    let mut guards = Vec::new();
    let mut terms = 0usize;
    for (row, _) in ids {
        let RowId::OrdinaryIbp {
            contraction_momentum,
            differentiated_loop,
        } = row
        else {
            return Err("inventory contains nonordinary row".into());
        };
        let &q = f
            .contraction_momenta()
            .get(*contraction_momentum)
            .ok_or("native contraction ID out of range")?;
        require(
            *differentiated_loop < f.loop_count(),
            "native differentiated loop out of range",
        )?;
        for &axis in &p.protected {
            let expansion = checked(f.derivative_contraction(axis, *differentiated_loop, q))?;
            for (at, c) in std::iter::once(expansion.constant())
                .chain(expansion.denominator_coefficients())
                .enumerate()
            {
                admit(r, "max_conditions", add(guards.len(), 1)?)?;
                guards.push(json!({"kind":"native_derivative_coefficient_denominator","row_id":row.stable_string(),"protected_axis":axis,"affine_slot":at,"nonzero":c.denominator.to_string()}));
                terms = add(terms, add(c.numerator.nterms(), c.denominator.nterms())?)?;
                admit(r, "max_coefficient_terms", terms)?;
            }
            derivatives.insert((row.clone(), axis), affine(&template, expansion, base, r)?);
        }
    }
    admit(r, "max_conditions", guards.len())?;
    admit(r, "max_coefficient_terms", terms)?;
    let mut matrix = Matrix::new(0, checked(u32::try_from(columns.len()))?, field());
    let mut unknowns = Vec::new();
    let mut operations = 0usize;
    for alpha in monos {
        for (row, &ordinal) in ids {
            let mut mono = template.zero();
            mono.append_monomial(base.one(), &alpha);
            let mut entries = BTreeMap::new();
            for &axis in &p.protected {
                let derivative = &derivatives[&(row.clone(), axis)];
                operations = add(operations, derivative.nterms())?;
                admit(r, "max_polynomial_operations", operations)?;
                let product = &mono * derivative;
                require(
                    product.variables() == template.variables(),
                    "outer denominator variable map changed",
                )?;
                admit(r, "max_polynomial_terms", product.nterms())?;
                for (c, exp) in product.coefficients.iter().zip(product.exponents_iter()) {
                    checked(base.validate_with_limits(c, ExactAlgebraLimits::default()))?;
                    if exp[axis] == 0 && !c.is_zero() {
                        let col = *columns
                            .get(&(axis, exp.to_vec()))
                            .ok_or("native product exceeds admitted degree")?;
                        require(
                            entries.insert(col, c.clone()).is_none(),
                            "duplicate native monomial",
                        )?;
                    }
                }
            }
            admit(
                r,
                "max_input_nonzeros",
                add(matrix.nvalues(), entries.len())?,
            )?;
            matrix.add_row(
                entries.values().cloned().collect(),
                entries.keys().copied().collect(),
            );
            unknowns.push(Unknown {
                row_id: row.clone(),
                ordinal,
                alpha: alpha.clone(),
            });
        }
    }
    require(
        unknowns.len() == p.unknowns,
        "unknown enumeration differs from preflight",
    )?;
    let mut column_report = vec![Value::Null; columns.len()];
    for ((axis, alpha), &index) in &columns {
        column_report[index as usize] =
            json!({"column":index,"protected_axis":axis,"monomial":alpha});
    }
    let report = json!({"native_polynomial_monomial_products":operations,"constraint_nonzeros":matrix.nvalues(),"derivative_input_coefficient_terms":terms,"pre_cancellation_derivative_guards":guards,"h_variables_eliminated_by_Dj_zero":true,"custom_polynomial_arithmetic":false});
    Ok(System {
        matrix,
        unknowns,
        columns: json!(column_report),
        report,
    })
}
