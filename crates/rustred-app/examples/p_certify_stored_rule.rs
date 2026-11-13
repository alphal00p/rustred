//! Opt-in exact certificates for a STORED written parametric row.
//!
//! This example never publishes a candidate and never uses an archived matrix
//! as coefficient authority. Ordinary IBPs are differentiated afresh from the
//! compiled family geometry. Every original column is retained: there is no
//! zero-sector census, quotient, or zero-unit-row shortcut.
//!
//! For R_T=1, P=I-e_T R_rest has inverse I+e_T R_rest. We reconstruct weights
//! of e_T in B=A P with the public projected source-weight API, then separately
//! multiply those weights by ORIGINAL A and require exact equality to R.
//! Membership is S2 only; input/witness poles, written regularity, guards,
//! source applicability, layers, descent and closure remain separate.

#![recursion_limit = "512"]

#[cfg(not(feature = "reconstruction"))]
fn main() {
    eprintln!("p_certify_stored_rule requires feature reconstruction");
    std::process::exit(2);
}

#[cfg(feature = "reconstruction")]
mod app {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs::{self, File};
    use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    use rustred::algebra::{
        Coefficient, ExactAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext,
    };
    use rustred::family::{
        ContractionMomentum as Momentum, IntegralFamily, ScalarProductCoordinate,
    };
    use rustred::identity::{
        CompletedIbpSourceRows, IndexShift, IntegralShift, ParametricIbpGenerator,
        TranslatedSourceRequest,
    };
    use rustred::solver::{
        ProjectedSourceWeightEvent, ProjectedSourceWeightLimits,
        reconstruct_projected_source_weights,
    };
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use symbolica::atom::AtomCore;
    use symbolica::domains::finite_field::{FiniteFieldCore, ToFiniteField, Zp64};
    use symbolica::domains::rational_polynomial::RationalPolynomialField;
    use symbolica::domains::{Field, Ring, SelfRing};
    use symbolica::prelude::{IntegerRing, Q, Rational, Z};
    use symbolica::tensors::sparse::{LuLMode, SparseMatrix, SparseRowReducer};

    type Result<T> = std::result::Result<T, String>;
    type RawRow = BTreeMap<Vec<i64>, IndexedCoefficient>;
    type Row = BTreeMap<IndexShift, IndexedCoefficient>;
    fn err(e: impl std::fmt::Display) -> String {
        e.to_string()
    }
    fn require(ok: bool, message: &str) -> Result<()> {
        if ok { Ok(()) } else { Err(message.into()) }
    }

    #[derive(Clone, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct Limits {
        pub max_input_bytes: usize,
        pub max_raw_rows: usize,
        pub max_columns: usize,
        pub max_coefficient_terms: usize,
        pub max_degree: u16,
        pub max_probes: usize,
        pub max_attempts: usize,
        pub max_primes: usize,
        pub max_cached_images: usize,
        pub max_cached_values: usize,
        pub max_weight_slots: usize,
        pub max_scout_points: usize,
    }
    impl Limits {
        fn arithmetic(&self) -> ExactAlgebraLimits {
            ExactAlgebraLimits {
                diagnostics_enabled: true,
                max_exponent: u16::MAX,
                max_polynomial_terms: self.max_coefficient_terms,
                max_term_operations: self.max_coefficient_terms.saturating_mul(16),
            }
        }
        fn projected(&self) -> ProjectedSourceWeightLimits {
            ProjectedSourceWeightLimits {
                arithmetic: self.arithmetic(),
                max_rows: self.max_raw_rows,
                max_columns: self.max_columns,
                max_input_nonzeros: self.max_coefficient_terms,
                max_coefficient_terms: self.max_coefficient_terms,
                max_degree: self.max_degree,
                max_probes: self.max_probes,
                max_attempts: self.max_attempts,
                max_primes: self.max_primes,
                max_cached_images: self.max_cached_images,
                max_cached_values: self.max_cached_values,
                max_weight_slots: self.max_weight_slots,
            }
        }
        fn validate(&self) -> Result<()> {
            require(
                self.max_input_bytes > 0
                    && self.max_raw_rows > 0
                    && self.max_columns > 0
                    && self.max_coefficient_terms > 0
                    && self.max_degree > 0
                    && self.max_probes > 0
                    && self.max_attempts > 0
                    && self.max_primes >= 2
                    && self.max_cached_images > 0
                    && self.max_cached_values > 0
                    && self.max_weight_slots > 0
                    && self.max_scout_points > 0,
                "invalid explicit resource limits",
            )
        }
    }
    #[derive(Clone, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct Origin {
        pub table: PathBuf,
        pub index: usize,
        pub byte_offset: u64,
        pub record_bytes: usize,
    }
    #[derive(Clone, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct Request {
        pub schema: String,
        pub family_file: PathBuf,
        pub origin: Origin,
        pub archive_frame: Option<PathBuf>,
        pub limits: Limits,
    }
    #[derive(Clone)]
    struct Case {
        fixed: Vec<Option<i64>>,
        equations: Vec<String>,
    }
    struct Template {
        differentiated_loop: usize,
        contraction: Momentum,
        divergence: bool,
        // derivative D_i.v: constant plus a linear combination of D_j.
        derivative: Vec<Vec<Coefficient>>,
    }
    struct Prepared {
        family: IntegralFamily,
        context: IndexedCoefficientContext,
        templates: Vec<Template>,
        completed: CompletedIbpSourceRows,
        external_indices: Vec<String>,
        variables: Vec<String>,
        family_text: String,
        family_digest: String,
    }
    fn count(v: &IndexedCoefficient) -> usize {
        v.raw().numerator.nterms() + v.raw().denominator.nterms()
    }
    fn coefficient_profile<'a>(values: impl IntoIterator<Item = &'a IndexedCoefficient>) -> Value {
        let mut nonzeros = 0usize;
        let mut terms = 0usize;
        let mut numerator_degree = 0u32;
        let mut denominator_degree = 0u32;
        for value in values {
            nonzeros += 1;
            terms += count(value);
            for (poly, maximum) in [
                (&value.raw().numerator, &mut numerator_degree),
                (&value.raw().denominator, &mut denominator_degree),
            ] {
                for powers in poly.exponents.chunks(poly.nvars().max(1)) {
                    *maximum = (*maximum).max(powers.iter().map(|v| u32::from(*v)).sum());
                }
            }
        }
        json!({"nonzeros":nonzeros,"numerator_plus_denominator_terms":terms,
            "max_numerator_total_degree":numerator_degree,"max_denominator_total_degree":denominator_degree})
    }
    fn charge(total: &mut usize, add: usize, maximum: usize) -> Result<()> {
        *total = total
            .checked_add(add)
            .ok_or("coefficient-term count overflow")?;
        require(
            *total <= maximum,
            "actual aggregate coefficient-term budget exceeded",
        )
    }
    fn poly_text(p: &rustred::algebra::CoefficientPolynomial, names: &[String]) -> String {
        if p.is_zero() {
            return "0".into();
        }
        p.coefficients
            .iter()
            .enumerate()
            .map(|(i, coefficient)| {
                let mut term = format!("({coefficient})");
                for (j, exponent) in p.exponents[i * p.nvars()..(i + 1) * p.nvars()]
                    .iter()
                    .enumerate()
                {
                    if *exponent != 0 {
                        term.push_str(&format!("*({})^{exponent}", names[j]));
                    }
                }
                term
            })
            .collect::<Vec<_>>()
            .join("+")
    }
    fn text(v: &IndexedCoefficient, names: &[String]) -> String {
        format!(
            "({})/({})",
            poly_text(&v.raw().numerator, names),
            poly_text(&v.raw().denominator, names)
        )
    }
    fn rewrite_indices(input: &str, names: &[String]) -> Result<String> {
        require(input.is_ascii(), "non-ASCII coefficient syntax unsupported")?;
        let b = input.as_bytes();
        let mut i = 0;
        let mut out = String::new();
        while i < b.len() {
            if b[i].is_ascii_alphabetic() || b[i] == b'_' {
                let start = i;
                i += 1;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                let token = &input[start..i];
                if token.starts_with('n')
                    && token.len() > 1
                    && token[1..].bytes().all(|c| c.is_ascii_digit())
                {
                    let index: usize = token[1..].parse().map_err(err)?;
                    let name = names.get(index).ok_or("index token out of range")?;
                    out.push_str(&format!("({name})"));
                } else {
                    out.push_str(token);
                }
            } else {
                out.push(char::from(b[i]));
                i += 1;
            }
        }
        Ok(out)
    }
    impl Prepared {
        fn parse(&self, value: &str, limits: &Limits) -> Result<IndexedCoefficient> {
            require(
                value.len() <= limits.max_input_bytes,
                "coefficient input byte limit",
            )?;
            self.context
                .parse_expression_with_limits(
                    &rewrite_indices(value, &self.external_indices)?,
                    limits.arithmetic(),
                )
                .map_err(err)
        }
        fn roundtrip(&self, value: &IndexedCoefficient, limits: &Limits) -> Result<()> {
            let restored = self.parse(&text(value, &self.variables), limits)?;
            require(
                restored.raw() == value.raw(),
                "non-injective coefficient variable transport",
            )
        }
    }

    fn scalar_expansion(
        f: &IntegralFamily,
        a: Momentum,
        b: Momentum,
        limits: &Limits,
    ) -> Result<Vec<Coefficient>> {
        let c = f.coefficient_context();
        let n = f.denominator_count();
        let mut out = vec![c.zero(); n + 1];
        if let (Momentum::External(i), Momentum::External(j)) = (a, b) {
            out[n] = f.external_gram()[i][j].clone();
            return Ok(out);
        }
        let coordinate = match (a, b) {
            (Momentum::Loop(i), Momentum::Loop(j)) => ScalarProductCoordinate::LoopLoop {
                left: i.min(j),
                right: i.max(j),
            },
            (Momentum::Loop(i), Momentum::External(j))
            | (Momentum::External(j), Momentum::Loop(i)) => ScalarProductCoordinate::LoopExternal {
                loop_index: i,
                external_index: j,
            },
            _ => unreachable!(),
        };
        let position = f
            .coordinates()
            .iter()
            .position(|x| *x == coordinate)
            .ok_or("missing scalar-product coordinate")?;
        for j in 0..n {
            out[j] = f.inverse_basis()[position][j].clone();
            let term = c
                .try_mul(&out[j], f.denominators()[j].constant(), limits.arithmetic())
                .map_err(err)?;
            out[n] = c
                .try_sub(&out[n], &term, limits.arithmetic())
                .map_err(err)?;
        }
        Ok(out)
    }
    fn prepare(family_text: String, limits: &Limits) -> Result<Prepared> {
        let family = rustred::node::driver::compile_family(&family_text)?;
        require(
            family.loop_count() > 0 && family.denominator_count() > 0,
            "empty family",
        )?;
        require(
            family.power_shifts().iter().all(|x| x.is_zero()),
            "EPS/shifted powers are unsupported",
        )?;
        let context = IndexedCoefficientContext::try_new(
            family.coefficient_context(),
            &format!("stored-rule-certificate-v1:{}", family.fingerprint()),
            family.denominator_count(),
        )
        .map_err(err)?;
        let external_indices = (0..family.denominator_count())
            .map(|i| {
                context
                    .index(i)
                    .map(|x| x.to_expression().to_canonical_string())
                    .map_err(err)
            })
            .collect::<Result<Vec<_>>>()?;
        let mut variables = family.coefficient_context().parameter_names().to_vec();
        require(
            !variables
                .iter()
                .any(|x| x.starts_with('n') && x[1..].bytes().all(|c| c.is_ascii_digit())),
            "ambiguous family parameter/index names",
        )?;
        variables.extend((0..family.denominator_count()).map(|i| format!("n{i}")));
        let mut templates = Vec::new();
        let c = family.coefficient_context();
        for l in 0..family.loop_count() {
            let contractions = (0..family.external_count())
                .rev()
                .map(Momentum::External)
                .chain((0..family.loop_count()).map(Momentum::Loop));
            for v in contractions {
                let mut derivative = Vec::new();
                for denominator in family.denominators() {
                    let mut expansion = vec![c.zero(); family.denominator_count() + 1];
                    for (coordinate, coefficient) in
                        family.coordinates().iter().zip(denominator.coefficients())
                    {
                        let momenta: Vec<_> = match *coordinate {
                            ScalarProductCoordinate::LoopLoop { left, right } => {
                                let mut m = Vec::new();
                                if left == l {
                                    m.push(Momentum::Loop(right));
                                }
                                if right == l {
                                    m.push(Momentum::Loop(left));
                                }
                                m
                            }
                            ScalarProductCoordinate::LoopExternal {
                                loop_index,
                                external_index,
                            } => {
                                if loop_index == l {
                                    vec![Momentum::External(external_index)]
                                } else {
                                    vec![]
                                }
                            }
                        };
                        for momentum in momenta {
                            for (out, factor) in expansion
                                .iter_mut()
                                .zip(scalar_expansion(&family, momentum, v, limits)?)
                            {
                                let term = c
                                    .try_mul(coefficient, &factor, limits.arithmetic())
                                    .map_err(err)?;
                                *out = c.try_add(out, &term, limits.arithmetic()).map_err(err)?;
                            }
                        }
                    }
                    derivative.push(expansion);
                }
                templates.push(Template {
                    differentiated_loop: l,
                    contraction: v,
                    divergence: v == Momentum::Loop(l),
                    derivative,
                });
            }
        }
        // Public translated sources are used ONLY to obtain checked lattice
        // key objects. Their coefficients never enter the independently built A.
        let g = ParametricIbpGenerator::try_new(&family).map_err(err)?;
        let batch = g.prepare_ordinary_ibp().map_err(err)?;
        let rows = (0..batch.len()).map(|i| batch.generate(i)).collect();
        let completed = batch.complete(rows).map_err(err)?;
        let family_digest = blake3::hash(family_text.as_bytes()).to_hex().to_string();
        let result = Prepared {
            family,
            context,
            templates,
            completed,
            external_indices,
            variables,
            family_text,
            family_digest,
        };
        for i in 0..result.family.denominator_count() {
            result.roundtrip(&result.context.index(i).map_err(err)?, limits)?;
        }
        Ok(result)
    }

    fn evaluate_polynomial(
        p: &rustred::algebra::CoefficientPolynomial,
        substitutions: &[IndexedCoefficient],
        c: &IndexedCoefficientContext,
        limits: &Limits,
    ) -> Result<IndexedCoefficient> {
        require(p.nvars() == substitutions.len(), "polynomial map mismatch")?;
        let mut sum = c.zero();
        for (i, integer) in p.coefficients.iter().enumerate() {
            let mut term = c
                .parse_expression_with_limits(&integer.to_string(), limits.arithmetic())
                .map_err(err)?;
            for (value, exponent) in substitutions
                .iter()
                .zip(&p.exponents[i * p.nvars()..(i + 1) * p.nvars()])
            {
                let mut power = *exponent;
                let mut factor = value.clone();
                while power > 0 {
                    if power & 1 == 1 {
                        term = c
                            .mul_with_limits(&term, &factor, limits.arithmetic())
                            .map_err(err)?;
                    }
                    power >>= 1;
                    if power > 0 {
                        factor = c
                            .mul_with_limits(&factor, &factor, limits.arithmetic())
                            .map_err(err)?;
                    }
                }
            }
            sum = c
                .add_with_limits(&sum, &term, limits.arithmetic())
                .map_err(err)?;
        }
        Ok(sum)
    }
    fn substitute(
        value: &IndexedCoefficient,
        substitutions: &[IndexedCoefficient],
        p: &Prepared,
        limits: &Limits,
        conditions: &mut BTreeSet<String>,
    ) -> Result<IndexedCoefficient> {
        let numerator =
            evaluate_polynomial(&value.raw().numerator, substitutions, &p.context, limits)?;
        let denominator =
            evaluate_polynomial(&value.raw().denominator, substitutions, &p.context, limits)?;
        require(
            !denominator.is_zero(),
            "coefficient denominator vanishes identically on full Case",
        )?;
        if !denominator.raw().is_constant() {
            conditions.insert(text(&denominator, &p.variables));
        }
        p.context
            .div_with_limits(&numerator, &denominator, limits.arithmetic())
            .map_err(err)
    }
    fn case_substitution(
        case: &Case,
        p: &Prepared,
        limits: &Limits,
    ) -> Result<Vec<IndexedCoefficient>> {
        let n = p.context.index_count();
        let offset = p.family.coefficient_context().parameter_names().len();
        require(case.fixed.len() == n, "Case arity mismatch")?;
        let mut equations = case.equations.clone();
        for (i, value) in case.fixed.iter().enumerate() {
            if let Some(v) = value {
                equations.push(format!("n{i}-({v})"));
            }
        }
        let mut reducer = SparseRowReducer::new((n + 1).try_into().map_err(err)?, Q, LuLMode::None);
        for equation in &equations {
            let value = p.parse(equation, limits)?;
            require(
                value.raw().denominator.is_constant(),
                "nonconstant Case denominator unsupported",
            )?;
            let poly = &value.raw().numerator;
            let mut row = BTreeMap::<u32, Rational>::new();
            for (i, coefficient) in poly.coefficients.iter().enumerate() {
                let powers = &poly.exponents[i * poly.nvars()..(i + 1) * poly.nvars()];
                let total: u32 = powers.iter().map(|x| u32::from(*x)).sum();
                require(total <= 1, "nonlinear Case equations unsupported")?;
                let column = if total == 0 {
                    n
                } else {
                    let position = powers
                        .iter()
                        .position(|x| *x != 0)
                        .ok_or("empty affine monomial")?;
                    require(
                        position >= offset,
                        "parameter-dependent Case equation unsupported",
                    )?;
                    position - offset
                };
                let integer: i64 = coefficient
                    .to_string()
                    .parse()
                    .map_err(|_| "Case integer coefficient outside i64")?;
                let entry = row
                    .entry(column.try_into().map_err(err)?)
                    .or_insert(Rational::from(0));
                *entry = &*entry + &Rational::from(integer);
            }
            let (ids, values): (Vec<_>, Vec<_>) =
                row.into_iter().filter(|(_, v)| !Q.is_zero(v)).unzip();
            if reducer.add_row(&values, &ids) == Some(n as u32) {
                return Err("inconsistent full AND Case".into());
            }
        }
        let mut substitutions = Vec::new();
        for name in p.family.coefficient_context().parameter_names() {
            substitutions.push(
                p.context
                    .lift(
                        &p.family
                            .coefficient_context()
                            .parameter(name)
                            .ok_or("unknown base parameter")?,
                    )
                    .map_err(err)?,
            );
        }
        substitutions.extend(
            (0..n)
                .map(|i| p.context.index(i))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(err)?,
        );
        let u = reducer.u();
        for r in (0..u.nrows() as usize).rev() {
            let range = u.row_ptrs()[r]..u.row_ptrs()[r + 1];
            let ids = &u.col_idcs()[range.clone()];
            let values = &u.values()[range];
            if ids.is_empty() {
                continue;
            }
            let pivot = ids[0] as usize;
            require(pivot < n, "inconsistent Case pivot")?;
            let mut value = p.context.zero();
            for (&column, scalar) in ids.iter().zip(values).skip(1) {
                let scalar = p.parse(&scalar.to_string(), limits)?;
                let v = if column as usize == n {
                    p.context.one()
                } else {
                    substitutions[offset + column as usize].clone()
                };
                let term = p
                    .context
                    .mul_with_limits(&scalar, &v, limits.arithmetic())
                    .map_err(err)?;
                value = p
                    .context
                    .sub_with_limits(&value, &term, limits.arithmetic())
                    .map_err(err)?;
            }
            let pivot_value = p.parse(&values[0].to_string(), limits)?;
            substitutions[offset + pivot] = p
                .context
                .div_with_limits(&value, &pivot_value, limits.arithmetic())
                .map_err(err)?;
        }
        for equation in equations {
            require(
                substitute(
                    &p.parse(&equation, limits)?,
                    &substitutions,
                    p,
                    limits,
                    &mut BTreeSet::new(),
                )?
                .is_zero(),
                "Case substitution did not preserve every AND equation",
            )?;
        }
        Ok(substitutions)
    }

    fn check_recenter(
        case: &Case,
        recenter: &[i64],
        substitutions: &[IndexedCoefficient],
        p: &Prepared,
        limits: &Limits,
    ) -> Result<()> {
        let offset = p.variables.len() - p.context.index_count();
        let mut shifted = substitutions.to_vec();
        for (i, displacement) in recenter.iter().enumerate() {
            shifted[offset + i] = p
                .context
                .add_with_limits(
                    &shifted[offset + i],
                    &p.context.integer(*displacement),
                    limits.arithmetic(),
                )
                .map_err(err)?;
            require(
                case.fixed[i].is_none() || *displacement == 0,
                "recenter changes a fixed Case coordinate",
            )?;
        }
        for equation in &case.equations {
            require(
                substitute(
                    &p.parse(equation, limits)?,
                    &shifted,
                    p,
                    limits,
                    &mut BTreeSet::new(),
                )?
                .is_zero(),
                "recenter does not preserve the full affine AND Case",
            )?;
        }
        Ok(())
    }

    fn constant_integer(value: &IndexedCoefficient) -> Result<i64> {
        let n = &value.raw().numerator;
        let d = &value.raw().denominator;
        require(
            n.is_constant() && d.is_constant(),
            "integral power is not Case-base plus an integer",
        )?;
        if n.is_zero() {
            return Ok(0);
        }
        let a: i64 = n.coefficients[0].to_string().parse().map_err(err)?;
        let b: i64 = d.coefficients[0].to_string().parse().map_err(err)?;
        require(
            b != 0 && a % b == 0,
            "nonintegral integral-power displacement",
        )?;
        a.checked_div(b)
            .ok_or_else(|| "integral displacement overflow".into())
    }
    fn integral_parts(input: &str, arity: usize) -> Result<Vec<&str>> {
        require(
            input.starts_with("I(") && input.ends_with(')'),
            "unsupported integral syntax",
        )?;
        let values: Vec<_> = input[2..input.len() - 1]
            .split(',')
            .map(str::trim)
            .collect();
        require(values.len() == arity, "integral arity mismatch")?;
        Ok(values)
    }
    fn integral_offsets(
        input: &str,
        recenter: &[i64],
        substitutions: &[IndexedCoefficient],
        p: &Prepared,
        limits: &Limits,
    ) -> Result<Vec<i64>> {
        let n = p.context.index_count();
        let offset = p.variables.len() - n;
        require(recenter.len() == n, "recenter arity mismatch")?;
        integral_parts(input, n)?
            .into_iter()
            .enumerate()
            .map(|(i, expression)| {
                let raw = p
                    .context
                    .add_with_limits(
                        &p.parse(expression, limits)?,
                        &p.context.integer(recenter[i]),
                        limits.arithmetic(),
                    )
                    .map_err(err)?;
                let value = substitute(&raw, substitutions, p, limits, &mut BTreeSet::new())?;
                let displacement = p
                    .context
                    .sub_with_limits(&value, &substitutions[offset + i], limits.arithmetic())
                    .map_err(err)?;
                constant_integer(&displacement)
            })
            .collect()
    }
    fn add_raw(
        row: &mut RawRow,
        key: Vec<i64>,
        value: IndexedCoefficient,
        p: &Prepared,
        limits: &Limits,
    ) -> Result<()> {
        let value = if let Some(old) = row.remove(&key) {
            p.context
                .add_with_limits(&old, &value, limits.arithmetic())
                .map_err(err)?
        } else {
            value
        };
        if !value.is_zero() {
            row.insert(key, value);
        }
        Ok(())
    }
    #[derive(Clone, Serialize)]
    struct SourceDescriptor {
        original_ordinal: usize,
        stored_seed_ordinal: usize,
        raw_seed: String,
        written_frame_offset: Vec<i64>,
        differentiated_loop: usize,
        contraction: String,
    }
    fn sources(
        seeds: &[String],
        recenter: &[i64],
        substitutions: &[IndexedCoefficient],
        p: &Prepared,
        limits: &Limits,
        conditions: &mut BTreeSet<String>,
    ) -> Result<(
        Vec<RawRow>,
        Vec<SourceDescriptor>,
        BTreeSet<Vec<i64>>,
        Vec<Vec<i64>>,
    )> {
        let c = &p.context;
        let n = c.index_count();
        let index_offset = p.variables.len() - n;
        require(
            seeds
                .len()
                .checked_mul(p.templates.len())
                .ok_or("source-row count overflow")?
                <= limits.max_raw_rows,
            "raw source-row budget exceeded",
        )?;
        let mut rows = Vec::new();
        let mut descriptors = Vec::new();
        let mut universe = BTreeSet::new();
        let mut seed_offsets = Vec::new();
        for (seed_ordinal, seed) in seeds.iter().enumerate() {
            let offset = integral_offsets(seed, recenter, substitutions, p, limits)?;
            seed_offsets.push(offset.clone());
            let powers = (0..n)
                .map(|i| {
                    c.add_with_limits(
                        &substitutions[index_offset + i],
                        &c.integer(offset[i]),
                        limits.arithmetic(),
                    )
                    .map_err(err)
                })
                .collect::<Result<Vec<_>>>()?;
            for template in &p.templates {
                let mut row = RawRow::new();
                if template.divergence {
                    universe.insert(offset.clone());
                    let d = c
                        .lift_with_limits(p.family.dimension(), limits.arithmetic())
                        .map_err(err)?;
                    add_raw(&mut row, offset.clone(), d, p, limits)?;
                }
                for (i, expansion) in template.derivative.iter().enumerate() {
                    for (j, factor) in expansion.iter().enumerate() {
                        if factor.is_zero() {
                            continue;
                        }
                        let mut key = offset.clone();
                        key[i] = key[i].checked_add(1).ok_or("source shift overflow")?;
                        if j < n {
                            key[j] = key[j].checked_sub(1).ok_or("source shift overflow")?;
                        }
                        universe.insert(key.clone());
                        let factor = c
                            .lift_with_limits(factor, limits.arithmetic())
                            .map_err(err)?;
                        let value = c
                            .neg_with_limits(
                                &c.mul_with_limits(&powers[i], &factor, limits.arithmetic())
                                    .map_err(err)?,
                                limits.arithmetic(),
                            )
                            .map_err(err)?;
                        if !value.raw().denominator.is_constant() {
                            conditions.insert(poly_text(&value.raw().denominator, &p.variables));
                        }
                        add_raw(&mut row, key, value, p, limits)?;
                    }
                }
                descriptors.push(SourceDescriptor {
                    original_ordinal: rows.len(),
                    stored_seed_ordinal: seed_ordinal,
                    raw_seed: seed.clone(),
                    written_frame_offset: offset.clone(),
                    differentiated_loop: template.differentiated_loop,
                    contraction: match template.contraction {
                        Momentum::Loop(i) => format!("loop:{i}"),
                        Momentum::External(i) => format!("external:{i}"),
                    },
                });
                rows.push(row);
            }
        }
        require(
            universe.len() <= limits.max_columns,
            "original column-universe budget exceeded",
        )?;
        Ok((rows, descriptors, universe, seed_offsets))
    }
    fn public_keys(
        seed_offsets: &[Vec<i64>],
        p: &Prepared,
    ) -> Result<BTreeMap<Vec<i64>, IndexShift>> {
        let g = ParametricIbpGenerator::try_new(&p.family).map_err(err)?;
        let mut requests = Vec::new();
        let count = p.templates.len();
        for seed in seed_offsets {
            for ordinal in 0..count {
                requests.push(TranslatedSourceRequest::new(
                    ordinal,
                    IntegralShift::try_new(seed.iter().copied()).map_err(err)?,
                ));
            }
        }
        let batch = g
            .translate_selected_completed_source_rows(&p.completed, requests, Default::default())
            .map_err(err)?;
        let mut keys = BTreeMap::new();
        for source in batch.sources() {
            for key in source.terms().keys() {
                keys.insert(key.values().to_vec(), key.clone());
            }
        }
        Ok(keys)
    }
    fn convert_row(row: &RawRow, keys: &BTreeMap<Vec<i64>, IndexShift>) -> Result<Row> {
        row.iter().map(|(k,v)| Ok((keys.get(k).ok_or("UNKNOWN: public key transport does not contain an independently rebuilt column")?.clone(), v.clone()))).collect()
    }
    fn transform(
        row: &Row,
        target: &IndexShift,
        written: &Row,
        p: &Prepared,
        limits: &Limits,
    ) -> Result<Row> {
        let mut out = row.clone();
        let a = row.get(target).cloned().unwrap_or_else(|| p.context.zero());
        if a.is_zero() {
            return Ok(out);
        }
        for (key, value) in written {
            if key == target {
                continue;
            }
            let prior = out.remove(key).unwrap_or_else(|| p.context.zero());
            let product = p
                .context
                .mul_with_limits(&a, value, limits.arithmetic())
                .map_err(err)?;
            let value = p
                .context
                .sub_with_limits(&prior, &product, limits.arithmetic())
                .map_err(err)?;
            if !value.is_zero() {
                out.insert(key.clone(), value);
            }
        }
        Ok(out)
    }
    fn splitmix(value: &mut u64) -> u64 {
        *value = value.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = *value;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn scout(
        rows: &[Row],
        columns: &[IndexShift],
        target: &IndexShift,
        p: &Prepared,
        limits: &Limits,
    ) -> Result<(Vec<usize>, Value)> {
        let registry: BTreeMap<_, _> = columns
            .iter()
            .enumerate()
            .map(|(i, k)| (k.clone(), i as u32))
            .collect();
        let target_column = registry[target];
        let field = Zp64::new((1u64 << 61) - 1);
        let mut random = 0x53544f5245445f52;
        let mut visited = 0usize;
        for pass in 0..limits.max_scout_points {
            let point: Vec<_> = (0..p.variables.len())
                .map(|_| field.to_element(1 + splitmix(&mut random) % (field.get_prime() - 1)))
                .collect();
            let mut reducer = SparseRowReducer::new(
                (columns.len() + 1).try_into().map_err(err)?,
                field.clone(),
                LuLMode::None,
            );
            let mut accepted = Vec::new();
            let mut unusable = false;
            for (ordinal, row) in rows.iter().enumerate() {
                visited += 1;
                let mut data = Vec::new();
                for (key, coefficient) in row {
                    let numerator = coefficient.raw().numerator.evaluate_with_coeff_map(
                        |v| v.to_finite_field(&field),
                        &point,
                        &field,
                    );
                    let denominator = coefficient.raw().denominator.evaluate_with_coeff_map(
                        |v| v.to_finite_field(&field),
                        &point,
                        &field,
                    );
                    if field.is_zero(&denominator) {
                        unusable = true;
                        break;
                    }
                    let value = field.div(&numerator, &denominator);
                    if !field.is_zero(&value) {
                        data.push((registry[key], value));
                    }
                }
                if unusable {
                    break;
                }
                data.sort_unstable_by_key(|x| x.0);
                let (ids, values): (Vec<_>, Vec<_>) = data.into_iter().unzip();
                if let Some(pivot) = reducer.add_row(&values, &ids) {
                    accepted.push(ordinal);
                    if pivot == target_column {
                        return Ok((
                            accepted,
                            json!({"native_scout_pass":pass,"source_rows_visited":visited,"prime":field.get_prime(),"full_column_count":columns.len(),"scope":"proposal only; exact original product remains mandatory"}),
                        ));
                    }
                }
            }
        }
        Err(format!(
            "UNKNOWN_SCOUT_NO_TARGET: native re-solved {visited} transformed source rows; selected finite source space only, not global nonmembership"
        ))
    }
    fn validate_original_product(
        rows: &[Row],
        weights: &BTreeMap<usize, IndexedCoefficient>,
        columns: &[IndexShift],
        written: &Row,
        p: &Prepared,
        limits: &Limits,
    ) -> Result<()> {
        let registry: BTreeMap<_, _> = columns
            .iter()
            .enumerate()
            .map(|(i, k)| (k.clone(), i as u32))
            .collect();
        let field = RationalPolynomialField::<IntegerRing, u16>::new(Z);
        let mut sources =
            SparseMatrix::new(0, columns.len().try_into().map_err(err)?, field.clone());
        for row in rows {
            let mut data: Vec<_> = row
                .iter()
                .map(|(k, v)| (registry[k], v.raw().clone()))
                .collect();
            data.sort_unstable_by_key(|x| x.0);
            let (ids, values) = data.into_iter().unzip();
            sources.add_row(values, ids);
        }
        let mut w = SparseMatrix::new(0, rows.len().try_into().map_err(err)?, field.clone());
        let (ids, values) = weights
            .iter()
            .map(|(i, v)| Ok((u32::try_from(*i).map_err(err)?, v.raw().clone())))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .unzip();
        w.add_row(values, ids);
        let actual = &w * &sources;
        for value in actual.values() {
            let sealed = p
                .context
                .admit_native_result_with_limits(value.clone(), limits.arithmetic())
                .map_err(err)?;
            p.roundtrip(&sealed, limits)?;
        }
        let mut expected = SparseMatrix::new(0, columns.len().try_into().map_err(err)?, field);
        let mut data: Vec<_> = written
            .iter()
            .map(|(k, v)| (registry[k], v.raw().clone()))
            .collect();
        data.sort_unstable_by_key(|x| x.0);
        let (ids, values) = data.into_iter().unzip();
        expected.add_row(values, ids);
        require(
            actual == expected,
            "UNKNOWN_ORIGINAL_PRODUCT_MISMATCH: all original columns checked",
        )
    }

    fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(err)?
            .take((maximum as u64).saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(err)?;
        require(bytes.len() <= maximum, "input byte limit exceeded")?;
        Ok(bytes)
    }
    fn origin(request: &Request) -> Result<(Value, Value, Value)> {
        let mut file = File::open(&request.origin.table).map_err(err)?;
        let mut header_bytes = Vec::new();
        BufReader::new((&file).take(request.limits.max_input_bytes as u64 + 1))
            .read_until(b'\n', &mut header_bytes)
            .map_err(err)?;
        require(
            header_bytes.len() <= request.limits.max_input_bytes,
            "native header byte limit",
        )?;
        require(
            header_bytes.last() == Some(&b'\n'),
            "native header must include its LF delimiter",
        )?;
        require(
            request.origin.record_bytes <= request.limits.max_input_bytes,
            "native rule byte limit",
        )?;
        if request.origin.byte_offset > 0 {
            file.seek(SeekFrom::Start(request.origin.byte_offset - 1))
                .map_err(err)?;
            let mut byte = [0];
            file.read_exact(&mut byte).map_err(err)?;
            require(byte[0] == b'\n', "native address is not a record boundary")?;
        }
        file.seek(SeekFrom::Start(request.origin.byte_offset))
            .map_err(err)?;
        let mut bytes = vec![0; request.origin.record_bytes];
        file.read_exact(&mut bytes).map_err(err)?;
        require(
            bytes.last() == Some(&b'\n'),
            "native rule record must include its LF boundary",
        )?;
        let header: Value = serde_json::from_slice(&header_bytes).map_err(err)?;
        let rule: Value = serde_json::from_slice(&bytes).map_err(err)?;
        require(
            header["type"] == "header"
                && rule["type"] == "rule"
                && rule["index"].as_u64() == Some(request.origin.index as u64),
            "native header/index/type mismatch",
        )?;
        Ok((
            header,
            rule,
            json!({"algorithm":"BLAKE3","native_header":blake3::hash(&header_bytes).to_hex().to_string(),"native_written_record_including_LF":blake3::hash(&bytes).to_hex().to_string(),"origin":request.origin,
                "native_header_byte_offset":0,"native_header_bytes":header_bytes.len(),"delimiter":"LF","native_header_includes_LF":true,"written_record_includes_LF":true}),
        ))
    }
    fn strings(value: &Value, name: &str) -> Result<Vec<String>> {
        value
            .as_array()
            .ok_or_else(|| format!("missing array {name}"))?
            .iter()
            .map(|x| {
                x.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("non-string {name}"))
            })
            .collect()
    }

    fn certify(
        p: &Prepared,
        header: &Value,
        rule: &Value,
        limits: &Limits,
        profile_only: bool,
    ) -> Result<Value> {
        let start = Instant::now();
        limits.validate()?;
        require(
            header["family_source"].as_str() == Some(p.family_text.as_str()),
            "presented family.sym differs from native header.family_source",
        )?;
        require(
            header["mask"] == rule["chart"],
            "native chart/header mismatch",
        )?;
        require(
            !header["two_sided_axes"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v.as_bool() == Some(true))),
            "two-sided/EPS axes unsupported",
        )?;
        let fixed = rule["fixed"]
            .as_array()
            .ok_or("missing full fixed Case")?
            .iter()
            .map(|x| {
                if x.is_null() {
                    Ok(None)
                } else {
                    x.as_i64()
                        .map(Some)
                        .ok_or_else(|| "invalid fixed Case coordinate".into())
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let case = Case {
            fixed,
            equations: strings(&rule["equations"], "equations")?,
        };
        let recenter = rule["recenter"]
            .as_array()
            .ok_or("missing explicit recenter")?
            .iter()
            .map(|x| {
                x.as_i64()
                    .ok_or_else(|| "invalid recenter coordinate".into())
            })
            .collect::<Result<Vec<_>>>()?;
        require(
            recenter.len() == p.context.index_count(),
            "recenter arity mismatch",
        )?;
        require(
            rule["raw_pivot_encoded"]
                .as_array()
                .is_some_and(|a| a.len() == recenter.len()),
            "missing raw-pivot frame binding",
        )?;
        let substitutions = case_substitution(&case, p, limits)?;
        check_recenter(&case, &recenter, &substitutions, p, limits)?;
        let mut written_conditions = BTreeSet::new();
        let mut source_conditions = BTreeSet::new();
        for condition in p.family.domain().conditions() {
            source_conditions.insert(poly_text(condition.polynomial(), &p.variables));
        }
        let n = p.context.index_count();
        let zero = vec![0; n];
        let target_offset = integral_offsets(
            rule["target"].as_str().ok_or("missing written target")?,
            &zero,
            &substitutions,
            p,
            limits,
        )?;
        let mut written_raw = RawRow::from([(target_offset.clone(), p.context.one())]);
        for term in rule["rhs"].as_array().ok_or("missing written RHS")? {
            let a = term.as_array().ok_or("invalid RHS term")?;
            require(a.len() == 2, "invalid RHS pair")?;
            let integral = a[0].as_str().ok_or("invalid RHS integral")?;
            let value = p.parse(a[1].as_str().ok_or("invalid RHS coefficient")?, limits)?;
            p.roundtrip(&value, limits)?;
            let value = substitute(&value, &substitutions, p, limits, &mut written_conditions)?;
            let key = integral_offsets(integral, &zero, &substitutions, p, limits)?;
            add_raw(
                &mut written_raw,
                key,
                p.context
                    .neg_with_limits(&value, limits.arithmetic())
                    .map_err(err)?,
                p,
                limits,
            )?;
        }
        require(
            written_raw
                .get(&target_offset)
                .is_some_and(|v| v.raw().is_one()),
            "nonunit written pivot: no silent normalization or division on a Case layer",
        )?;
        let seeds = strings(&rule["seeds"], "stored raw seed integrals")?;
        let (raw_rows, descriptors, mut raw_universe, seed_offsets) = sources(
            &seeds,
            &recenter,
            &substitutions,
            p,
            limits,
            &mut source_conditions,
        )?;
        let mut terms = 0;
        for row in &raw_rows {
            for value in row.values() {
                charge(&mut terms, count(value), limits.max_coefficient_terms)?;
            }
        }
        for value in written_raw.values() {
            charge(&mut terms, count(value), limits.max_coefficient_terms)?;
        }
        let source_profile = coefficient_profile(raw_rows.iter().flat_map(BTreeMap::values));
        let written_profile = coefficient_profile(written_raw.values());
        eprintln!(
            "{}",
            json!({"phase":"independent_sources_prepared","raw_source_rows":raw_rows.len(),"structural_columns":raw_universe.len(),
            "source_coefficients":source_profile,"written_coefficients":written_profile,"shared_host_seconds":start.elapsed().as_secs_f64()})
        );
        // A nonzero written column absent from every freshly rebuilt source is
        // a finite-pool miss. Never obtain a synthetic key or assert invalidity
        // modulo zero sectors, other seeds, or a larger family source space.
        let nonzero_source_columns: BTreeSet<_> =
            raw_rows.iter().flat_map(|r| r.keys().cloned()).collect();
        require(
            written_raw
                .keys()
                .all(|k| nonzero_source_columns.contains(k)),
            "UNKNOWN_WRITTEN_COLUMN_ABSENT_FROM_SELECTED_ORIGINAL_SOURCE_POOL",
        )?;
        raw_universe.extend(written_raw.keys().cloned());
        require(
            raw_universe.len() <= limits.max_columns,
            "full original column budget exceeded",
        )?;
        let keys = public_keys(&seed_offsets, p)?;
        let universe = raw_universe
            .iter()
            .map(|k| {
                keys.get(k)
                    .cloned()
                    .ok_or_else(|| "UNKNOWN_PUBLIC_KEY_TRANSPORT_INCOMPLETE".into())
            })
            .collect::<Result<BTreeSet<_>>>()?;
        let target = keys
            .get(&target_offset)
            .ok_or("target missing from public genuine key dictionary")?
            .clone();
        let written = convert_row(&written_raw, &keys)?;
        let original = raw_rows
            .iter()
            .map(|r| convert_row(r, &keys))
            .collect::<Result<Vec<_>>>()?;
        let transformed = original
            .iter()
            .map(|r| transform(r, &target, &written, p, limits))
            .collect::<Result<Vec<_>>>()?;
        for row in &transformed {
            for value in row.values() {
                charge(&mut terms, count(value), limits.max_coefficient_terms)?;
            }
        }
        let profiles = json!({"original_A":source_profile,"written_R":written_profile,
            "transformed_B":coefficient_profile(transformed.iter().flat_map(BTreeMap::values))});
        let mut columns: Vec<_> = universe.iter().filter(|k| **k != target).cloned().collect();
        columns.push(target.clone());
        let frame_seconds = start.elapsed().as_secs_f64();
        eprintln!(
            "{}",
            json!({"phase":"frame_prepared","raw_source_rows":original.len(),"columns":columns.len(),"retained_coefficient_terms":terms,"coefficient_profiles":profiles,"shared_host_seconds":frame_seconds})
        );
        let scout_start = Instant::now();
        let (selected, scout_report) = scout(&transformed, &columns, &target, p, limits)?;
        let scout_seconds = scout_start.elapsed().as_secs_f64();
        eprintln!(
            "{}",
            json!({"phase":"scout_finished","selected_rows":selected.len(),"shared_host_seconds":scout_seconds})
        );
        if profile_only {
            return Ok(
                json!({"schema":"rustred.stored-rule-profile.v1","status":"PROFILE_ONLY_NO_CERTIFICATE","S2_credit":false,
                "family_blake3":p.family_digest,"family_fingerprint":p.family.fingerprint(),"stored_native_rule":rule,
                "full_original_Case":{"fixed":case.fixed,"equations":case.equations},"raw_source_rows":original.len(),
                "raw_source_nonzeros":original.iter().map(BTreeMap::len).sum::<usize>(),"full_original_column_count":columns.len(),
                "selected_source_rows":selected.len(),"selected_original_ordinals":selected,"scout":scout_report,
                "aggregate_retained_coefficient_terms":terms,"zero_erased_columns":[],"zero_axiom_rows":0,"quotient_used":false,
                "coefficient_profiles":profiles,"native_header":header,
                "zero_census_generated_or_used":false,"archived_matrix_coefficients_used":false,
                "reconstruction_executed":false,"original_all_column_exact_product":false,"guard_transfer":false,
                "limits":limits,"timings_shared_host_seconds":{"frame_and_source":frame_seconds,"scout":scout_seconds,"total":start.elapsed().as_secs_f64()}}),
            );
        }
        let selected_b = selected
            .iter()
            .map(|i| transformed[*i].clone())
            .collect::<Vec<_>>();
        let selected_a = selected
            .iter()
            .map(|i| original[*i].clone())
            .collect::<Vec<_>>();
        let forbidden = universe.iter().filter(|k| **k != target).cloned().collect();
        let recon_start = Instant::now();
        let mut events = Vec::new();
        let proposal = reconstruct_projected_source_weights(&p.context,&selected_b,&target,&forbidden,limits.projected(),|event| {
            let value = match event {
                ProjectedSourceWeightEvent::FramePrepared { rows,columns,target_column,original_variables,active_variables } => json!({"event":"frame","rows":rows,"columns":columns,"target_column":target_column,"original_variables":original_variables,"active_variables":active_variables}),
                ProjectedSourceWeightEvent::Coefficient { column,probes,primes } => json!({"event":"coefficient","column":column,"probes":probes,"primes":primes}),
                ProjectedSourceWeightEvent::CoefficientPasses { column,oracle_probes,native_passes,primes } => json!({"event":"coefficient_passes","column":column,"oracle_probes":oracle_probes,"native_passes":native_passes,"primes":primes}),
                ProjectedSourceWeightEvent::WeightsStarted { rows,lower_nonzeros } => json!({"event":"weights_started","rows":rows,"lower_nonzeros":lower_nonzeros}),
                ProjectedSourceWeightEvent::WeightsFinished { nonzero_weights } => json!({"event":"weights_finished","nonzero_weights":nonzero_weights}),
                ProjectedSourceWeightEvent::ExactProductStarted { rows,columns } => json!({"event":"transformed_exact_product_started","rows":rows,"columns":columns}),
                ProjectedSourceWeightEvent::ExactProductFinished { output_terms } => json!({"event":"transformed_exact_product_finished","output_terms":output_terms}),
            }; eprintln!("{value}"); events.push(value);
        }).map_err(|e| format!("UNKNOWN_RECONSTRUCTION: {e}"))?;
        let reconstruction_seconds = recon_start.elapsed().as_secs_f64();
        require(
            proposal.image.len() == 1
                && proposal
                    .image
                    .get(&target)
                    .is_some_and(|v| v.raw().is_one()),
            "transformed full-image result is not exactly e_T",
        )?;
        let mut witness_conditions = BTreeSet::new();
        for value in proposal.weights.values() {
            charge(&mut terms, count(value), limits.max_coefficient_terms)?;
            p.roundtrip(value, limits)?;
            if !value.raw().denominator.is_constant() {
                witness_conditions.insert(poly_text(&value.raw().denominator, &p.variables));
            }
        }
        let product_start = Instant::now();
        validate_original_product(
            &selected_a,
            &proposal.weights,
            &columns,
            &written,
            p,
            limits,
        )?;
        let product_seconds = product_start.elapsed().as_secs_f64();
        let weights = proposal.weights.iter().map(|(i,w)| json!({"selected_ordinal":i,"original_source":descriptors[selected[*i]],"coefficient":text(w,&p.variables)})).collect::<Vec<_>>();
        let source_rows = selected.iter().map(|i| json!({"descriptor":descriptors[*i],"terms":raw_rows[*i].iter().map(|(k,v)| json!([k,text(v,&p.variables)])).collect::<Vec<_>>()})).collect::<Vec<_>>();
        let encoded_substitutions = substitutions
            .iter()
            .skip(p.variables.len() - n)
            .map(|v| text(v, &p.variables))
            .collect::<Vec<_>>();
        Ok(json!({
            "schema":"rustred.stored-rule-certificate.v1","status":"S2_ONLY_EXACT_STORED_MEMBERSHIP",
            "app_source_blake3":blake3::hash(include_str!("p_certify_stored_rule.rs").as_bytes()).to_hex().to_string(),
            "family_blake3":p.family_digest,"family_fingerprint":p.family.fingerprint(),"family_source":p.family_text,
            "source_coefficient_authority":"APP_OWNED_ORDINARY_IBP_DERIVATIVE_FROM_FAMILY_GEOMETRY",
            "public_generator_role":"integer key dictionary only; generated coefficients never enter A",
            "archived_matrix_coefficients_used":false,"zero_erased_columns":[],"zero_axiom_rows":0,"zero_census_generated_or_used":false,"quotient_used":false,
            "stored_native_rule":rule,"native_header":header,"native_header_order":{"producer_node_order":header["node_order"],"applied_integral_order":header["order"]},"chart":rule["chart"],
            "full_original_Case":{"fixed":case.fixed,"equations":case.equations},"affine_substitution":encoded_substitutions,
            "raw_seed_recenter_applied_once":recenter,"written_row_recenter_applied":false,
            "target_offset":target_offset,"written_signed_row":written_raw.iter().map(|(k,v)| json!([k,text(v,&p.variables)])).collect::<Vec<_>>(),
            "full_original_columns":columns.iter().map(|k| k.values()).collect::<Vec<_>>(),"full_original_column_count":columns.len(),
            "raw_source_rows":raw_rows.len(),"raw_source_nonzeros":raw_rows.iter().map(BTreeMap::len).sum::<usize>(),
            "selected_source_rows":selected.len(),"all_source_descriptors":descriptors,"selected_original_source_rows":source_rows,
            "exhibited_weights":weights,"reconstructed_weight_slots":proposal.prefix_rows,"aggregate_retained_coefficient_terms":terms,
            "transformed_product_validated_by_public_API":true,"original_all_column_exact_product":true,
            "checked_original_columns":columns.len(),"column_transform":{"P":"I-e_T*R_rest","P_inverse":"I+e_T*R_rest","R_T_exactly_one":true,"R_rest_T_exactly_zero":true,"nilpotent_square_exactly_zero":true},
            "coefficient_profiles":profiles,"witness_coefficient_profile":coefficient_profile(proposal.weights.values()),
            "variable_map_roundtrip_checked":true,"pivot_normalized_without_division":true,
            "input_source_nonzero_conditions":source_conditions,"witness_denominator_obligations":witness_conditions,
            "written_coefficient_nonzero_conditions":written_conditions,"existing_literal_full_AND_guards":rule["exceptions"],
            "existing_guard_epoch":header.get("guard_version").cloned().unwrap_or_else(||json!("UNKNOWN_NOT_SERIALIZED")),"guard_version_upgraded":false,"guards_source_pole_layers_closed":false,
            "guard_transfer":false,"parent_node_or_table_closure":false,"descent_checked":false,
            "scout":scout_report,"events":events,"limits":limits,
            "timings_shared_host_seconds":{"frame_and_source":frame_seconds,"reconstruction":reconstruction_seconds,"original_exact_product":product_seconds,"total":start.elapsed().as_secs_f64()}
        }))
    }

    fn execute(
        request: &Request,
        cache: &mut BTreeMap<String, Prepared>,
        profile_only: bool,
    ) -> Result<Value> {
        require(
            request.schema == "rustred.stored-rule-certificate-request.v1",
            "unsupported request schema",
        )?;
        request.limits.validate()?;
        let family_bytes = read_bounded(&request.family_file, request.limits.max_input_bytes)?;
        let family_text = String::from_utf8(family_bytes).map_err(err)?;
        let digest = blake3::hash(family_text.as_bytes()).to_hex().to_string();
        let cache_hit = cache.contains_key(&digest);
        if !cache_hit {
            cache.insert(digest.clone(), prepare(family_text, &request.limits)?);
        }
        let (header, rule, provenance) = origin(request)?;
        let mut result = certify(
            &cache[&digest],
            &header,
            &rule,
            &request.limits,
            profile_only,
        )?;
        result["app_source_blake3"] = json!(
            blake3::hash(include_str!("p_certify_stored_rule.rs").as_bytes())
                .to_hex()
                .to_string()
        );
        result["input_provenance"] = provenance;
        result["family_file"] = json!(request.family_file);
        result["immutable_family_base_IBP_cache_hit"] = json!(cache_hit);
        result["archive_frame_reference_only"] = json!(request.archive_frame);
        result["archive_frame_read_for_mathematical_authority"] = json!(false);
        Ok(result)
    }

    pub fn run() -> Result<()> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        require(
            args.len() == 4
                && (args[0] == "--request" || args[0] == "--profile-request")
                && args[2] == "--out",
            "usage: p_certify_stored_rule --request|--profile-request REQUEST.json --out REPORT.json (a JSON array is a batch)",
        )?;
        let profile_only = args[0] == "--profile-request";
        let bytes = read_bounded(Path::new(&args[1]), 16 * 1024 * 1024)?;
        let input: Value = serde_json::from_slice(&bytes).map_err(err)?;
        let requests: Vec<Request> = if input.is_array() {
            serde_json::from_value(input).map_err(err)?
        } else {
            vec![serde_json::from_value(input).map_err(err)?]
        };
        let mut cache = BTreeMap::new();
        let mut output = Vec::new();
        let mut native_panicked = false;
        for request in &requests {
            if native_panicked {
                output.push(json!({"schema":"rustred.stored-rule-certificate.v1","status":"UNKNOWN_NOT_RUN_AFTER_NATIVE_PANIC","origin":request.origin,"S2_credit":false,"guard_transfer":false}));
                continue;
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                execute(request, &mut cache, profile_only)
            }));
            output.push(match result {
                Ok(Ok(v))=>v,
                Ok(Err(e))=>json!({"schema":"rustred.stored-rule-certificate.v1","status":"UNKNOWN_NO_CERTIFICATE","reason":e,"origin":request.origin,"S2_credit":false,"guard_transfer":false,"global_nonmembership_claim":false}),
                Err(_)=>{native_panicked=true;json!({"schema":"rustred.stored-rule-certificate.v1","status":"UNKNOWN_NATIVE_PANIC","origin":request.origin,"S2_credit":false,"guard_transfer":false})},
            });
        }
        let all = output.iter().all(|v| {
            v["status"]
                == if profile_only {
                    "PROFILE_ONLY_NO_CERTIFICATE"
                } else {
                    "S2_ONLY_EXACT_STORED_MEMBERSHIP"
                }
        });
        let result = if output.len() == 1 {
            output.remove(0)
        } else {
            json!({"schema":"rustred.stored-rule-certificate-batch.v1","results":output})
        };
        fs::write(&args[3], serde_json::to_vec_pretty(&result).map_err(err)?).map_err(err)?;
        if !all {
            std::process::exit(1);
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const TADPOLE: &str = "I(loops(k),externals(),dimension(d),prop(D1,k^2+1,1))";
        const BUBBLE: &str = "I(loops(k),externals(q),dimension(d),prop(D1,k^2+1,1),prop(D2,(k-q)^2+2,1),gram(q,q,1))";
        fn limits() -> Limits {
            Limits {
                max_input_bytes: 65536,
                max_raw_rows: 64,
                max_columns: 128,
                max_coefficient_terms: 100000,
                max_degree: 8,
                max_probes: 2000,
                max_attempts: 3,
                max_primes: 4,
                max_cached_images: 128,
                max_cached_values: 100000,
                max_weight_slots: 64,
                max_scout_points: 3,
            }
        }
        fn header(family: &str, mask: &str) -> Value {
            let priority = (0..mask.len()).collect::<Vec<_>>();
            json!({"type":"header","family_source":family,"mask":mask,"guard_version":2,
                "fixture_kind":"ANALYTIC_FRONTEND_CONTROL",
                "node_order":{"name":"explicit_tiny_control","slot_permutation":null,"coordinate_priority":null,"source_order":"Input"},
                "order":{"support_priority":priority,"coordinate_priority":priority,"two_sided":[]}})
        }
        fn rule(rhs: Value) -> Value {
            json!({"type":"rule","index":0,"chart":"1","fixed":[null],"equations":[],
                "recenter":[0],"raw_pivot_encoded":[128],"target":"I(n0)","rhs":rhs,
                "seeds":["I(n0-1)","I(n0-2)"],"exceptions":[]})
        }
        fn assert_equal(a: &IndexedCoefficient, b: &IndexedCoefficient) {
            assert_eq!(a.raw(), b.raw());
        }

        #[test]
        fn independently_differentiates_massive_tadpole() {
            let l = limits();
            let p = prepare(TADPOLE.into(), &l).unwrap();
            let case = Case {
                fixed: vec![None],
                equations: vec![],
            };
            let s = case_substitution(&case, &p, &l).unwrap();
            let (rows, descriptors, _, _) =
                sources(&["I(n0-1)".into()], &[0], &s, &p, &l, &mut BTreeSet::new()).unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].len(), 2);
            assert_eq!(descriptors[0].differentiated_loop, 0);
            assert_equal(&rows[0][&vec![-1]], &p.parse("d-2*n0+2", &l).unwrap());
            assert_equal(&rows[0][&vec![0]], &p.parse("2*n0-2", &l).unwrap());
            let (translated, _, _, _) =
                sources(&["I(n0-2)".into()], &[1], &s, &p, &l, &mut BTreeSet::new()).unwrap();
            assert_eq!(
                rows[0].keys().collect::<Vec<_>>(),
                translated[0].keys().collect::<Vec<_>>()
            );
            for (key, value) in &rows[0] {
                assert_equal(value, &translated[0][key]);
            }
        }

        #[test]
        fn independent_massless_bubble_handcheck_shifts_both_keys_and_powers() {
            let l = limits();
            let p=prepare("I(loops(k),externals(q),dimension(d),prop(D1,k^2,1),prop(D2,(k-q)^2,1),gram(q,q,1))".into(),&l).unwrap();
            let s = case_substitution(
                &Case {
                    fixed: vec![None, None],
                    equations: vec![],
                },
                &p,
                &l,
            )
            .unwrap();
            for (seed, recenter) in [
                ("I(n0,n1)", vec![0, 0]),
                ("I(n0,n1-1)", vec![0, 0]),
                ("I(n0,n1-2)", vec![0, 1]),
            ] {
                let (rows, descriptors, _, _) =
                    sources(&[seed.into()], &recenter, &s, &p, &l, &mut BTreeSet::new()).unwrap();
                let i = descriptors
                    .iter()
                    .position(|d| d.contraction == "loop:0")
                    .unwrap();
                let shifted = seed != "I(n0,n1)";
                let offset = if shifted { -1 } else { 0 };
                let power = if shifted { "n1-1" } else { "n1" };
                assert_eq!(rows[i].len(), 3);
                assert_equal(
                    &rows[i][&vec![0, offset]],
                    &p.parse(&format!("d-2*n0-({power})"), &l).unwrap(),
                );
                assert_equal(
                    &rows[i][&vec![-1, offset + 1]],
                    &p.parse(&format!("-({power})"), &l).unwrap(),
                );
                assert_equal(&rows[i][&vec![0, offset + 1]], &p.parse(power, &l).unwrap());
            }
        }

        #[test]
        fn independent_external_and_loop_derivatives_match_public_generator_up_to_content() {
            for (family_text, seed, n) in [
                (BUBBLE, "I(n0-1,n1-2)", 2),
                (
                    "I(loops(k,l),externals(),dimension(d),prop(D1,k^2+1,1),prop(D2,l^2+2,1),prop(D3,(k-l)^2+3,1))",
                    "I(n0-1,n1-2,n2)",
                    3,
                ),
            ] {
                let l = limits();
                let p = prepare(family_text.into(), &l).unwrap();
                let case = Case {
                    fixed: vec![None; n],
                    equations: vec![],
                };
                let s = case_substitution(&case, &p, &l).unwrap();
                let (raw, _, _, offsets) = sources(
                    &[seed.into()],
                    &vec![0; n],
                    &s,
                    &p,
                    &l,
                    &mut BTreeSet::new(),
                )
                .unwrap();
                let g = ParametricIbpGenerator::try_new(&p.family).unwrap();
                let requests: Vec<_> = (0..p.templates.len())
                    .map(|i| {
                        TranslatedSourceRequest::new(
                            i,
                            IntegralShift::try_new(offsets[0].iter().copied()).unwrap(),
                        )
                    })
                    .collect();
                let batch = g
                    .translate_selected_completed_source_rows(
                        &p.completed,
                        requests,
                        Default::default(),
                    )
                    .unwrap();
                let normalize = |row: &RawRow| {
                    let first = row.values().next().unwrap();
                    row.iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                p.context.div_with_limits(v, first, l.arithmetic()).unwrap(),
                            )
                        })
                        .collect::<RawRow>()
                };
                let expected = raw.iter().map(normalize).collect::<Vec<_>>();
                let actual = batch
                    .sources()
                    .iter()
                    .map(|source| {
                        let row = source
                            .terms()
                            .iter()
                            .map(|(k, v)| {
                                (
                                    k.values().to_vec(),
                                    p.parse(&text(v, &p.variables), &l).unwrap(),
                                )
                            })
                            .collect::<RawRow>();
                        normalize(&row)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(expected.len(), actual.len());
                for a in &expected {
                    assert!(actual.iter().any(|b| {
                        a.len() == b.len()
                            && a.iter()
                                .all(|(k, v)| b.get(k).is_some_and(|w| v.raw() == w.raw()))
                    }));
                }
            }
        }

        #[test]
        fn certifies_two_distinct_same_case_written_rows_and_resolves_invalid_coefficient() {
            let l = limits();
            let p = prepare(TADPOLE.into(), &l).unwrap();
            let h = header(TADPOLE, "1");
            let first = rule(json!([["I(n0-1)", "-(d-2*n0+2)/(2*n0-2)"]]));
            let second = rule(json!([
                ["I(n0-1)", "-(d-2*n0+2)/(2*n0-2)-(2*n0-4)"],
                ["I(n0-2)", "-(d-2*n0+4)"]
            ]));
            assert_ne!(first["rhs"], second["rhs"]);
            for r in [&first, &second] {
                let proof = certify(&p, &h, r, &l, false).unwrap();
                assert_eq!(proof["status"], "S2_ONLY_EXACT_STORED_MEMBERSHIP");
                assert_eq!(proof["original_all_column_exact_product"], true);
                assert_eq!(proof["stored_native_rule"]["rhs"], r["rhs"]);
                assert_eq!(proof["zero_census_generated_or_used"], false);
                assert!(!proof["exhibited_weights"].as_array().unwrap().is_empty());
            }
            let invalid = rule(json!([["I(n0-1)", "-(d-2*n0+2)/(2*n0-2)+1"]]));
            // An independent exact determinant establishes that this particular
            // mutant is outside the two-row, three-column source span. The
            // subsequent scout still re-solves the mutant from scratch.
            let s = case_substitution(
                &Case {
                    fixed: vec![None],
                    equations: vec![],
                },
                &p,
                &l,
            )
            .unwrap();
            let (a, _, _, _) = sources(
                &["I(n0-1)".into(), "I(n0-2)".into()],
                &[0],
                &s,
                &p,
                &l,
                &mut BTreeSet::new(),
            )
            .unwrap();
            let rhs = p.parse(invalid["rhs"][0][1].as_str().unwrap(), &l).unwrap();
            let r = p.context.neg_with_limits(&rhs, l.arithmetic()).unwrap();
            let delta = p
                .context
                .sub_with_limits(
                    &p.context
                        .mul_with_limits(&a[0][&vec![0]], &r, l.arithmetic())
                        .unwrap(),
                    &a[0][&vec![-1]],
                    l.arithmetic(),
                )
                .unwrap();
            let determinant = p
                .context
                .mul_with_limits(&a[1][&vec![-2]], &delta, l.arithmetic())
                .unwrap();
            assert_equal(&determinant, &p.parse("-(2*n0-2)*(d-2*n0+4)", &l).unwrap());
            assert!(!determinant.is_zero());
            let error = certify(&p, &h, &invalid, &l, false).unwrap_err();
            assert!(error.contains("UNKNOWN_SCOUT_NO_TARGET"), "{error}");
            assert!(
                error.contains("native re-solved 6 transformed source rows"),
                "{error}"
            );
        }

        #[test]
        fn full_and_case_and_recenter_fail_closed() {
            let l = limits();
            let p = prepare(BUBBLE.into(), &l).unwrap();
            let case = Case {
                fixed: vec![None, None],
                equations: vec!["n0-n1-1".into()],
            };
            let s = case_substitution(&case, &p, &l).unwrap();
            assert!(check_recenter(&case, &[1, 1], &s, &p, &l).is_ok());
            assert!(check_recenter(&case, &[1, 0], &s, &p, &l).is_err());
            assert!(
                case_substitution(
                    &Case {
                        fixed: vec![Some(1), Some(1)],
                        equations: case.equations
                    },
                    &p,
                    &l
                )
                .is_err()
            );
            assert!(
                case_substitution(
                    &Case {
                        fixed: vec![None, None],
                        equations: vec!["n0*n1-1".into()]
                    },
                    &p,
                    &l
                )
                .is_err()
            );
            assert!(p.parse("n0+\u{03b1}", &l).is_err());
            assert!(
                certify(&p, &header(TADPOLE, "11"), &rule(json!([])), &l, false)
                    .unwrap_err()
                    .contains("family")
            );
        }

        #[test]
        fn exact_native_record_binding_and_archive_is_not_coefficient_authority() {
            let l = limits();
            let directory =
                std::env::temp_dir().join(format!("stored-rule-control-{}", std::process::id()));
            fs::create_dir_all(&directory).unwrap();
            let family = directory.join("family.sym");
            fs::write(&family, TADPOLE).unwrap();
            let mut h = serde_json::to_vec(&header(TADPOLE, "1")).unwrap();
            h.push(b'\n');
            let r = rule(json!([["I(n0-1)", "-(d-2*n0+2)/(2*n0-2)"]]));
            let mut body = serde_json::to_vec(&r).unwrap();
            body.push(b'\n');
            let table = directory.join("control-native.jsonl");
            let mut file = h.clone();
            file.extend(&body);
            fs::write(&table, file).unwrap();
            let request = Request {
                schema: "rustred.stored-rule-certificate-request.v1".into(),
                family_file: family,
                origin: Origin {
                    table,
                    index: 0,
                    byte_offset: h.len() as u64,
                    record_bytes: body.len(),
                },
                archive_frame: Some(directory.join("intentionally-absent-poison-matrix")),
                limits: l,
            };
            let proof = execute(&request, &mut BTreeMap::new(), false).unwrap();
            assert_eq!(
                proof["archive_frame_read_for_mathematical_authority"],
                false
            );
            assert_eq!(
                proof["input_provenance"]["native_written_record_including_LF"],
                blake3::hash(&body).to_hex().to_string()
            );
            let mut bad = request.clone();
            bad.origin.byte_offset += 1;
            assert!(origin(&bad).unwrap_err().contains("record boundary"));
            bad = request.clone();
            bad.origin.index = 1;
            assert!(origin(&bad).unwrap_err().contains("mismatch"));
            fs::remove_dir_all(directory).unwrap();
        }
    }
}

#[cfg(feature = "reconstruction")]
fn main() {
    if let Err(error) = app::run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
