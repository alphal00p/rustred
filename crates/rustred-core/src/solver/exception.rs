//! Exact exceptional equations extracted once from a canonical solved row.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use symbolica::domains::InternalOrdering;
use symbolica::prelude::{Factorize, Integer};

use crate::algebra::CoefficientPolynomial;

use super::RuleCandidate;

/// Index-case equalities over the generic coefficient field Q(parameters).
///
/// The outer vector is OR; equations within each branch are AND. An empty
/// outer vector means false, while an empty branch would mean true. Equations
/// retain the candidate's complete variable map, with zero exponents in every
/// non-index variable. Affine and nonlinear equations remain exact equations;
/// this type makes no claim about their integer solutions or sector geometry.
///
/// These are the index exceptions of the normalized candidate RHS only.
/// Parameter-only poles are deliberately excluded: division can introduce a
/// factor such as `d - 4` even when `SourceSystem::conditions()` is empty.
/// Such poles remain visible in the candidate's exact RHS denominators and
/// must be respected when specializing parameters. Inherited source
/// conditions are additional, separate obligations; they do not cover every
/// parameter pole introduced by the solved rule.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExceptionalConditions {
    pub branches: Vec<Vec<CoefficientPolynomial>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExceptionError {
    InvalidInput(&'static str),
    NativeAlgebra,
}

impl fmt::Display for ExceptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(detail) => write!(f, "invalid exception-extraction input: {detail}"),
            Self::NativeAlgebra => {
                write!(f, "native algebra failed while extracting rule exceptions")
            }
        }
    }
}

impl std::error::Error for ExceptionError {}

/// Extract SpIRed's denominator and inactive-index activation exceptions.
///
/// This is a cold operation on a final canonical candidate. It does not
/// authenticate or wrap every coefficient, specialize source rows, or decide
/// affine/nonlinear geometry. Native Symbolica owns factorization, coefficient
/// collection, content removal, and boundary substitution.
///
/// As for solver-produced candidates, coefficients must already have their
/// fixed-coordinate substitutions and canonical target translation applied.
/// Merely attaching a fixed `CoordinateCase` to a hand-built candidate does
/// not specialize its coefficients. Returned equations retain their meaning
/// on that case; intersection and geometric simplification belong to its owner.
pub fn extract_exceptions<const N: usize>(
    candidate: &RuleCandidate<N>,
    indices: &[usize; N],
    sector: &[bool; N],
) -> Result<ExceptionalConditions, ExceptionError> {
    extract_exceptions_two_sided(candidate, indices, sector, &[false; N])
}

/// [`extract_exceptions`] with two-sided eps axes (plan M2 sec.3.4).
///
/// On a two-sided axis every integer power lies in one support and the
/// order is the injective `e(n)` (`rustred_order::two_sided_excess`); the raw
/// sign of a symbolic power is only a chart. A rule is ordered for a
/// generic symbolic power, so every chart crossing is an exceptional branch
/// solved on its own (fixed) case, where the order is exact:
/// - chart `n <= 0` (inactive): an RHS shift `+s` reaches `n + s >= 1` for
///   `n = 0, -1, .., 1-s`; these are the existing activation branches, and a
///   numeric RHS power `> 0` on such an axis is a legal chart crossing;
/// - chart `n >= 1` (active), the mirror: an RHS shift `-s` reaches
///   `n - s <= 0` for `n = 1, .., s`, so these values become branches too.
///
/// A branch is skipped where the term's coefficient vanishes, as for the
/// activation branches. With no two-sided axis this is exactly
/// [`extract_exceptions`].
pub fn extract_exceptions_two_sided<const N: usize>(
    candidate: &RuleCandidate<N>,
    indices: &[usize; N],
    sector: &[bool; N],
    two_sided: &[bool; N],
) -> Result<ExceptionalConditions, ExceptionError> {
    preflight(candidate, indices, sector, two_sided)?;
    if candidate.rhs.is_empty() && candidate.hidden_poles.is_empty() {
        // No coefficient variable map exists in an empty RHS and there are
        // no denominators or terms capable of activating another sector.
        return Ok(ExceptionalConditions::default());
    }
    catch_unwind(AssertUnwindSafe(|| {
        extract(candidate, indices, sector, two_sided)
    }))
    .map_err(|_| ExceptionError::NativeAlgebra)
}

/// [`extract_exceptions_two_sided`] through the exact CONTENT route: only the
/// parameter-free content of each guard input is factored (see
/// `content_denominator_branches`). The returned branches describe the same
/// set of index points as the native route, as a different (coarser) OR of
/// ANDs; the activation and two-sided mirror branches are identical. Meant for
/// guard inputs whose native factorization stalls (large multivariate
/// denominators of 4L/5L eps rules); a coarse cofactor branch may be
/// nonlinear and is then left to the case intersection (refined or deferred).
pub fn extract_exceptions_content<const N: usize>(
    candidate: &RuleCandidate<N>,
    indices: &[usize; N],
    sector: &[bool; N],
    two_sided: &[bool; N],
) -> Result<ExceptionalConditions, ExceptionError> {
    preflight(candidate, indices, sector, two_sided)?;
    if candidate.rhs.is_empty() && candidate.hidden_poles.is_empty() {
        return Ok(ExceptionalConditions::default());
    }
    catch_unwind(AssertUnwindSafe(|| {
        extract_with(candidate, indices, sector, two_sided, true)
    }))
    .map_err(|_| ExceptionError::NativeAlgebra)
}

/// How GuardExtraction computes the index exceptions of a found rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuardRoute {
    /// Native factorization of every guard input (the established engine).
    #[default]
    Native,
    /// The exact content route only ([`extract_exceptions_content`]).
    Content,
    /// The native route, tried first in a forked child process that is killed
    /// after `native_timeout`; if the child finished, the parent recomputes
    /// the native result in-process (known to finish, identical bytes to
    /// [`GuardRoute::Native`]); otherwise the content route is used. Where no
    /// child process can be forked (non-Unix, fork failure) the native route
    /// runs in-process without a timeout.
    NativeThenContent { native_timeout: Duration },
}

/// Which route produced a rule's exceptions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardRouteTaken {
    Native,
    Content,
}

/// Outcome of the timed native try of [`GuardRoute::NativeThenContent`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeGuardTry {
    /// No timed try (route `Native` or `Content`).
    NotTried,
    /// The child finished within the timeout.
    Finished,
    /// The child was killed at the timeout.
    TimedOut,
    /// The child died otherwise (signal, panic, allocation failure).
    ChildFailed,
    /// No child could be created; the native route ran in-process.
    Unavailable,
}

/// Report of one routed guard extraction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuardReport {
    pub route: GuardRouteTaken,
    pub native_try: NativeGuardTry,
    /// Wall seconds of the timed native try (0 when not tried).
    pub try_seconds: f64,
    /// Wall seconds of the extraction that produced the result.
    pub seconds: f64,
}

/// [`extract_exceptions_two_sided`] under a [`GuardRoute`]. With
/// [`GuardRoute::Native`] the result is exactly that of
/// [`extract_exceptions_two_sided`].
pub fn extract_exceptions_routed<const N: usize>(
    candidate: &RuleCandidate<N>,
    indices: &[usize; N],
    sector: &[bool; N],
    two_sided: &[bool; N],
    route: GuardRoute,
) -> Result<(ExceptionalConditions, GuardReport), ExceptionError> {
    let native = || extract_exceptions_two_sided(candidate, indices, sector, two_sided);
    let content = || extract_exceptions_content(candidate, indices, sector, two_sided);
    let timed = |route: GuardRouteTaken,
                 native_try: NativeGuardTry,
                 try_seconds: f64,
                 run: &dyn Fn() -> Result<ExceptionalConditions, ExceptionError>| {
        let start = Instant::now();
        run().map(|conditions| {
            (
                conditions,
                GuardReport {
                    route,
                    native_try,
                    try_seconds,
                    seconds: start.elapsed().as_secs_f64(),
                },
            )
        })
    };
    match route {
        GuardRoute::Native => timed(
            GuardRouteTaken::Native,
            NativeGuardTry::NotTried,
            0.0,
            &native,
        ),
        GuardRoute::Content => timed(
            GuardRouteTaken::Content,
            NativeGuardTry::NotTried,
            0.0,
            &content,
        ),
        GuardRoute::NativeThenContent { native_timeout } => {
            // Invalid input fails the same way on either route: check it
            // before paying for a child.
            preflight(candidate, indices, sector, two_sided)?;
            let start = Instant::now();
            let outcome = child::finishes_within(native_timeout, || native().is_ok());
            let try_seconds = start.elapsed().as_secs_f64();
            match outcome {
                NativeGuardTry::Finished | NativeGuardTry::Unavailable => {
                    timed(GuardRouteTaken::Native, outcome, try_seconds, &native)
                }
                NativeGuardTry::TimedOut | NativeGuardTry::ChildFailed => {
                    timed(GuardRouteTaken::Content, outcome, try_seconds, &content)
                }
                NativeGuardTry::NotTried => unreachable!("a timed try reports its outcome"),
            }
        }
    }
}

/// A computation in a forked child with a wall-clock timeout. Native
/// Symbolica factorization has no cancellation point, so a child process is
/// the only way to bound it without leaking a busy thread.
mod child {
    use super::NativeGuardTry;
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    pub(super) fn finishes_within(
        timeout: Duration,
        work: impl FnOnce() -> bool,
    ) -> NativeGuardTry {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        // SAFETY: the child only runs `work` (exact algebra on memory it
        // inherited copy-on-write) and leaves with `_exit`, skipping atexit
        // handlers and destructors of state shared with the parent. A lock
        // held by another parent thread at the fork can only make the child
        // hang; it is then killed at the timeout like any slow child.
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return NativeGuardTry::Unavailable;
        }
        if pid == 0 {
            let finished = catch_unwind(AssertUnwindSafe(work)).unwrap_or(false);
            unsafe { libc::_exit(if finished { 0 } else { 3 }) };
        }
        let deadline = Instant::now() + timeout;
        let mut pause = Duration::from_millis(1);
        loop {
            let mut status = 0;
            // SAFETY: plain waitpid on our own child.
            let reaped = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
            if reaped == pid {
                return if libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0 {
                    NativeGuardTry::Finished
                } else {
                    NativeGuardTry::ChildFailed
                };
            }
            if reaped < 0 {
                return NativeGuardTry::ChildFailed;
            }
            let now = Instant::now();
            if now >= deadline {
                // SAFETY: our own unreaped child; reap it after the kill.
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                    libc::waitpid(pid, &mut status, 0);
                }
                return NativeGuardTry::TimedOut;
            }
            std::thread::sleep(pause.min(deadline - now));
            pause = (pause * 2).min(Duration::from_millis(100));
        }
    }

    #[cfg(not(unix))]
    pub(super) fn finishes_within(
        _timeout: Duration,
        _work: impl FnOnce() -> bool,
    ) -> NativeGuardTry {
        NativeGuardTry::Unavailable
    }
}

fn preflight<const N: usize>(
    candidate: &RuleCandidate<N>,
    indices: &[usize; N],
    sector: &[bool; N],
    two_sided: &[bool; N],
) -> Result<(), ExceptionError> {
    if candidate.target != candidate.case.integral() || !candidate.case.is_in_sector(sector) {
        return Err(ExceptionError::InvalidInput(
            "target must be canonical for its case and sector",
        ));
    }
    for (axis, position) in indices.iter().enumerate() {
        if indices[..axis].contains(position) {
            return Err(ExceptionError::InvalidInput(
                "index-variable positions must be distinct",
            ));
        }
    }
    let variables = match (candidate.rhs.first(), candidate.hidden_poles.first()) {
        (Some(first), _) => first.coefficient.numerator.variables(),
        (None, Some(pole)) => pole.variables(),
        (None, None) => return Ok(()),
    };
    if candidate
        .hidden_poles
        .iter()
        .any(|pole| pole.variables() != variables || pole.is_zero())
    {
        return Err(ExceptionError::InvalidInput(
            "witness poles must be nonzero and share the RHS variable map",
        ));
    }
    if indices.iter().any(|&position| position >= variables.len()) {
        return Err(ExceptionError::InvalidInput(
            "index-variable position is outside the coefficient variable map",
        ));
    }
    for term in &candidate.rhs {
        for (axis, active) in sector.iter().enumerate() {
            if term.integral[axis].is_symbolic() != candidate.target[axis].is_symbolic() {
                return Err(ExceptionError::InvalidInput(
                    "RHS and target must share their symbolic-coordinate pattern",
                ));
            }
            if !active
                && !two_sided[axis]
                && !term.integral[axis].is_symbolic()
                && term.integral[axis].value() > 0
            {
                return Err(ExceptionError::InvalidInput(
                    "numeric RHS index activates an inactive sector coordinate",
                ));
            }
        }
        for polynomial in [&term.coefficient.numerator, &term.coefficient.denominator] {
            if polynomial.variables() != variables {
                return Err(ExceptionError::InvalidInput(
                    "coefficient parts must share one variable map",
                ));
            }
            if polynomial.coefficients.len().checked_mul(variables.len())
                != Some(polynomial.exponents.len())
            {
                return Err(ExceptionError::InvalidInput(
                    "native coefficient and exponent arrays have inconsistent lengths",
                ));
            }
        }
        if term.coefficient.denominator.is_zero() {
            return Err(ExceptionError::InvalidInput(
                "coefficient denominator is zero",
            ));
        }
    }
    Ok(())
}

fn extract<const N: usize>(
    candidate: &RuleCandidate<N>,
    indices: &[usize; N],
    sector: &[bool; N],
    two_sided: &[bool; N],
) -> ExceptionalConditions {
    extract_with(candidate, indices, sector, two_sided, false)
}

/// The exact guard inputs of GuardExtraction: the index-containing
/// denominators of nonzero RHS coefficients, primitive with positive leading
/// coefficient, sorted and deduplicated. Scalar content has no zero locus over
/// Q; removing it lets repeated denominators differing by a nonzero integer
/// share one factorization.
fn guard_inputs<const N: usize>(
    candidate: &RuleCandidate<N>,
    indices: &[usize; N],
) -> Vec<CoefficientPolynomial> {
    // guard-fix: the witness poles of the lift (source-weight denominators,
    // direct-hit pivot) are guards exactly like the RHS denominators: on such a
    // locus the retained witness proves nothing even where the RHS is regular.
    let mut denominators: Vec<_> = candidate
        .rhs
        .iter()
        .filter(|term| !term.coefficient.is_zero())
        .map(|term| &term.coefficient.denominator)
        .chain(candidate.hidden_poles.iter())
        .filter(|denominator| contains_index(denominator, indices))
        .map(|denominator| primitive(denominator.clone()))
        .collect();
    sort_unique(&mut denominators);
    denominators
}

/// Native route: factor every guard input; each irreducible index-containing
/// factor `f` gives one AND-branch, the coefficients of `f` in the parameters.
fn native_denominator_branches<const N: usize>(
    inputs: Vec<CoefficientPolynomial>,
    indices: &[usize; N],
    parameter_positions: &[usize],
) -> Vec<Vec<CoefficientPolynomial>> {
    let mut factors = Vec::new();
    for denominator in inputs {
        for (factor, _multiplicity) in denominator.factor() {
            if contains_index(&factor, indices) {
                factors.push(primitive(factor));
            }
        }
    }
    sort_unique(&mut factors);

    let mut branches = Vec::new();
    for factor in factors {
        let coefficients = factor.to_multivariate_polynomial_list(parameter_positions, true);
        let mut branch = Vec::with_capacity(coefficients.len());
        let mut impossible = false;
        for equation in coefficients.into_values() {
            if equation.is_zero() {
                continue;
            }
            if equation.is_constant() {
                // A nonzero coefficient of a parameter monomial prevents the
                // denominator factor from vanishing identically in parameters.
                impossible = true;
                break;
            }
            branch.push(primitive(equation));
        }
        if !impossible {
            sort_unique(&mut branch);
            branches.push(branch);
        }
    }
    branches
}

/// Content route (exact, the same zero set as the native route): for a guard
/// input `P(x, p)` (`x` index variables, `p` parameters), `P(x0, .) == 0` as a
/// polynomial in `p` iff every parameter coefficient `c_k(x0)` vanishes. With
/// `g = gcd_k c_k` and `h_k = c_k / g`, `V(c) = V(g) u V(h)`: where `g(x0) != 0`
/// every `h_k(x0)` must vanish. Only the parameter-free `g` is factored (one
/// branch `[f]` per irreducible index-containing factor `f`); the rest is ONE
/// AND-branch `[h_k]`, omitted when some `h_k` is a nonzero constant (then
/// `V(h)` is empty). The native route instead factors `P` and takes the
/// parameter coefficients of each irreducible factor; since `Q[p]` is an
/// integral domain, `P(x0, .) == 0` iff some factor vanishes identically, so
/// both routes give the same set of index points, as different branch lists.
fn content_denominator_branches<const N: usize>(
    inputs: Vec<CoefficientPolynomial>,
    indices: &[usize; N],
    parameter_positions: &[usize],
) -> Vec<Vec<CoefficientPolynomial>> {
    let mut branches = Vec::new();
    for input in inputs {
        let coefficients: Vec<CoefficientPolynomial> = input
            .to_multivariate_polynomial_list(parameter_positions, true)
            .into_values()
            .filter(|coefficient| !coefficient.is_zero())
            .collect();
        let Some(first) = coefficients.first() else {
            continue;
        };
        let mut content = first.clone();
        for coefficient in &coefficients[1..] {
            if content.is_constant() {
                break;
            }
            content = content.gcd(coefficient);
        }
        if !content.is_constant() {
            for (factor, _multiplicity) in primitive(content.clone()).factor() {
                if contains_index(&factor, indices) {
                    branches.push(vec![primitive(factor)]);
                }
            }
        }
        let cofactors: Vec<CoefficientPolynomial> = if content.is_constant() {
            coefficients
        } else {
            coefficients
                .iter()
                .map(|coefficient| {
                    coefficient
                        .try_div(&content)
                        .expect("the content divides every coefficient")
                })
                .collect()
        };
        if cofactors
            .iter()
            .any(|cofactor| cofactor.is_constant() && !cofactor.is_zero())
        {
            continue;
        }
        let mut branch: Vec<_> = cofactors
            .into_iter()
            .filter(|cofactor| !cofactor.is_zero())
            .map(primitive)
            .collect();
        sort_unique(&mut branch);
        if !branch.is_empty() {
            branches.push(branch);
        }
    }
    branches
}

fn extract_with<const N: usize>(
    candidate: &RuleCandidate<N>,
    indices: &[usize; N],
    sector: &[bool; N],
    two_sided: &[bool; N],
    content_route: bool,
) -> ExceptionalConditions {
    let template = candidate
        .rhs
        .first()
        .map(|term| &term.coefficient.numerator)
        .or(candidate.hidden_poles.first())
        .expect("callers return early without RHS terms and witness poles");
    let parameter_positions: Vec<_> = (0..template.nvars())
        .filter(|position| !indices.contains(position))
        .collect();

    let inputs = guard_inputs(candidate, indices);
    let mut branches = if content_route {
        content_denominator_branches(inputs, indices, &parameter_positions)
    } else {
        native_denominator_branches(inputs, indices, &parameter_positions)
    };

    for term in &candidate.rhs {
        for (axis, (&position, active)) in indices.iter().zip(sector).enumerate() {
            let power = term.integral[axis];
            if *active || !power.is_symbolic() || power.value() <= 0 {
                continue;
            }
            for boundary in 0..power.value() {
                let value = Integer::from(-boundary);
                if term
                    .coefficient
                    .numerator
                    .replace(position, &value)
                    .is_zero()
                {
                    // Even when its denominator vanishes here, denominator
                    // exceptions above still retain the corresponding pole.
                    continue;
                }
                let variable = template
                    .variable(&template.variables()[position])
                    .expect("preflight checked the variable-map position");
                let equation = &variable + &template.constant(Integer::from(boundary));
                branches.push(vec![equation]);
            }
        }
    }
    // Mirror branches of a two-sided axis in its chart n >= 1 (see
    // `extract_exceptions_two_sided`): a shift -s leaves the chart at n <= s.
    for term in &candidate.rhs {
        for (axis, (&position, active)) in indices.iter().zip(sector).enumerate() {
            let power = term.integral[axis];
            if !two_sided[axis] || !*active || !power.is_symbolic() || power.value() >= 0 {
                continue;
            }
            for value in 1..=-power.value() {
                let value = Integer::from(value);
                if term
                    .coefficient
                    .numerator
                    .replace(position, &value)
                    .is_zero()
                {
                    continue;
                }
                let variable = template
                    .variable(&template.variables()[position])
                    .expect("preflight checked the variable-map position");
                let equation = &variable - &template.constant(value);
                branches.push(vec![equation]);
            }
        }
    }
    branches.sort_unstable_by(InternalOrdering::internal_cmp);
    branches.dedup();
    ExceptionalConditions { branches }
}

fn contains_index<const N: usize>(
    polynomial: &CoefficientPolynomial,
    indices: &[usize; N],
) -> bool {
    if indices.is_empty() || polynomial.nvars() == 0 {
        return false;
    }
    polynomial
        .exponents_iter()
        .any(|exponents| indices.iter().any(|&position| exponents[position] != 0))
}

fn primitive(polynomial: CoefficientPolynomial) -> CoefficientPolynomial {
    debug_assert!(!polynomial.is_zero());
    let polynomial = polynomial.make_primitive();
    if polynomial.lcoeff() < 0 {
        -polynomial
    } else {
        polynomial
    }
}

fn sort_unique(polynomials: &mut Vec<CoefficientPolynomial>) {
    polynomials.sort_unstable_by(InternalOrdering::internal_cmp);
    polynomials.dedup();
}

#[cfg(test)]
mod tests;
