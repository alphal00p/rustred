//! NEW (campaign, re-implemented, not copied): streaming evaluator of rule coefficients modulo several primes at once.
//!
//! The engine's `expr.rs` builds a tree; a 123 MB coefficient line (T162 fs-c130, 5.82 M monomials) must not. This
//! evaluator walks the bytes once and evaluates every monomial in all contexts (prime, point) as it goes; memory is
//! O(number of contexts). Grammar (STRICT; anything else is refused with the byte position, never guessed):
//!
//! ```text
//! coef   := ws [sgn] side_p [ '/' side_q ] ws          -- [sgn] only directly before a parenthesis
//! side_p := '(' poly ')'   |  poly                     -- a bare poly followed by '/' must be ONE monomial
//! side_q := '(' poly ')'   |  monomial
//! poly   := [+-] mono ( [+-] mono )*                   -- expanded sum; no nested parentheses
//! mono   := factor ( '*' factor )*
//! factor := uint | var [ '^' uint ]                    -- uint: decimal, any length; exponent <= MAX_EXPONENT
//! var    := 'd' | 'n' digits                           -- n<k> without leading zero, k < arity
//! ```
//! Whitespace (space, tab, newline, CR) is allowed between tokens. The value of a coefficient at a context is
//! (num, den) with den = 1 when there is no '/'; a zero den is reported to the caller (pole), never inverted here.

use std::fmt;

/// Largest accepted exponent (as `expr.rs`, which refuses |e| >= 100_000; here 10_000).
pub const MAX_EXPONENT: u64 = 10_000;

/// One evaluation context: a prime and the values of n0..n{N-1} followed by d (length arity + 1), all reduced mod p.
#[derive(Clone, Debug)]
pub struct Ctx {
    pub p: u64,
    pub vals: Vec<u64>,
}

/// Syntactic size and degrees of one side (numerator or denominator).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct SideStats {
    pub terms: usize,
    /// max total degree of a monomial (n and d)
    pub deg: u32,
    /// max degree in d of a monomial
    pub degd: u32,
    /// max degree in the n variables of a monomial
    pub degn: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct CoeffStats {
    pub num: SideStats,
    pub den: SideStats,
    pub has_den: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoeffError {
    pub pos: usize,
    pub msg: String,
}

impl fmt::Display for CoeffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "coefficient syntax at byte {}: {}", self.pos, self.msg)
    }
}

impl std::error::Error for CoeffError {}

#[inline]
fn mulmod(a: u64, b: u64, p: u64) -> u64 {
    ((a as u128 * b as u128) % p as u128) as u64
}
#[inline]
fn addmod(a: u64, b: u64, p: u64) -> u64 {
    ((a as u128 + b as u128) % p as u128) as u64
}
fn powmod(mut a: u64, mut e: u64, p: u64) -> u64 {
    let mut r = 1 % p;
    a %= p;
    while e > 0 {
        if e & 1 == 1 {
            r = mulmod(r, a, p);
        }
        a = mulmod(a, a, p);
        e >>= 1;
    }
    r
}

struct Lex<'a> {
    s: &'a [u8],
    i: usize,
    arity: usize,
}

enum Factor {
    Num(usize, usize), // byte span of the digits
    Var(usize, u64),   // variable (arity = d), exponent
}

impl Lex<'_> {
    fn err<T>(&self, msg: impl Into<String>) -> Result<T, CoeffError> {
        Err(CoeffError {
            pos: self.i,
            msg: msg.into(),
        })
    }
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn digits(&mut self) -> (usize, usize) {
        let a = self.i;
        while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
            self.i += 1;
        }
        (a, self.i)
    }
    fn small_uint(&mut self, what: &str) -> Result<u64, CoeffError> {
        let (a, b) = self.digits();
        if a == b {
            return self.err(format!("expected {what}"));
        }
        if b - a > 6 {
            return self.err(format!("{what} too large"));
        }
        let v: u64 = std::str::from_utf8(&self.s[a..b])
            .ok()
            .and_then(|t| t.parse().ok())
            .ok_or_else(|| CoeffError {
                pos: a,
                msg: format!("bad {what}"),
            })?;
        Ok(v)
    }
    fn factor(&mut self) -> Result<Factor, CoeffError> {
        match self.peek() {
            Some(c) if c.is_ascii_digit() => {
                let (a, b) = self.digits();
                if self.i < self.s.len() && (self.s[self.i].is_ascii_alphabetic() || self.s[self.i] == b'.') {
                    return self.err("noninteger or malformed literal");
                }
                Ok(Factor::Num(a, b))
            }
            Some(b'd') => {
                self.i += 1;
                self.after_var(self.arity)
            }
            Some(b'n') => {
                self.i += 1;
                let (a, b) = self.digits();
                if a == b {
                    return self.err("variable n without index");
                }
                if b - a > 1 && self.s[a] == b'0' {
                    return self.err("variable index with leading zero");
                }
                let k: usize = std::str::from_utf8(&self.s[a..b])
                    .ok()
                    .and_then(|t| t.parse().ok())
                    .filter(|&k| k < self.arity)
                    .ok_or_else(|| CoeffError {
                        pos: a,
                        msg: format!("variable index out of range (arity {})", self.arity),
                    })?;
                self.after_var(k)
            }
            Some(c) => self.err(format!("unexpected {:?}", c as char)),
            None => self.err("unexpected end"),
        }
    }
    fn after_var(&mut self, var: usize) -> Result<Factor, CoeffError> {
        if self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_') {
            return self.err("unknown identifier");
        }
        if self.peek() == Some(b'^') {
            self.i += 1;
            self.ws();
            let e = self.small_uint("exponent")?;
            if e > MAX_EXPONENT {
                return self.err("exponent too large");
            }
            Ok(Factor::Var(var, e))
        } else {
            Ok(Factor::Var(var, 1))
        }
    }

    /// One signed monomial added into `acc` (per context). Returns (total degree, d degree, n degree).
    fn monomial(&mut self, neg: bool, ctxs: &[Ctx], acc: &mut [u64]) -> Result<(u32, u32, u32), CoeffError> {
        let mut val: Vec<u64> = ctxs.iter().map(|c| 1 % c.p).collect();
        let (mut deg, mut degd) = (0u64, 0u64);
        loop {
            match self.factor()? {
                Factor::Num(a, b) => {
                    for (v, c) in val.iter_mut().zip(ctxs) {
                        let mut x = 0u64;
                        for &ch in &self.s[a..b] {
                            x = addmod(mulmod(x, 10, c.p), (ch - b'0') as u64, c.p);
                        }
                        *v = mulmod(*v, x, c.p);
                    }
                }
                Factor::Var(k, e) => {
                    deg += e;
                    if k == self.arity {
                        degd += e;
                    }
                    for (v, c) in val.iter_mut().zip(ctxs) {
                        *v = mulmod(*v, powmod(c.vals[k], e, c.p), c.p);
                    }
                }
            }
            if self.peek() == Some(b'*') {
                self.i += 1;
                if self.peek() == Some(b'*') {
                    return self.err("'**' is not accepted (use '^')");
                }
            } else {
                break;
            }
        }
        for ((a, v), c) in acc.iter_mut().zip(&val).zip(ctxs) {
            *a = if neg {
                (*a + (c.p - *v)) % c.p
            } else {
                addmod(*a, *v, c.p)
            };
        }
        let cap = |x: u64| u32::try_from(x).unwrap_or(u32::MAX);
        Ok((cap(deg), cap(degd), cap(deg - degd)))
    }

    /// A polynomial up to ')' / '/' / end (not consumed). `stats` accumulates.
    fn poly(&mut self, ctxs: &[Ctx], acc: &mut [u64], st: &mut SideStats) -> Result<(), CoeffError> {
        let mut first = true;
        loop {
            let neg = match self.peek() {
                Some(b'+') => {
                    self.i += 1;
                    false
                }
                Some(b'-') => {
                    self.i += 1;
                    true
                }
                Some(b')') | Some(b'/') | None if !first => return Ok(()),
                _ if first => false,
                Some(b'(') => return self.err("nested parentheses / implicit product"),
                Some(c) => return self.err(format!("expected + or -, found {:?}", c as char)),
                None => return Ok(()), // not reached: `None if !first` above, `_ if first` before
            };
            if self.peek() == Some(b'(') {
                return self.err("nested parentheses");
            }
            let (deg, degd, degn) = self.monomial(neg, ctxs, acc)?;
            st.terms += 1;
            st.deg = st.deg.max(deg);
            st.degd = st.degd.max(degd);
            st.degn = st.degn.max(degn);
            first = false;
        }
    }

    /// side := '(' poly ')' | bare poly. Returns whether it was parenthesized.
    fn side(&mut self, ctxs: &[Ctx], acc: &mut [u64], st: &mut SideStats) -> Result<bool, CoeffError> {
        if self.peek() == Some(b'(') {
            self.i += 1;
            self.poly(ctxs, acc, st)?;
            if self.peek() != Some(b')') {
                return self.err("missing ')'");
            }
            self.i += 1;
            Ok(true)
        } else {
            self.poly(ctxs, acc, st)?;
            Ok(false)
        }
    }
}

/// Evaluate a coefficient text at every context: `num[k]`, `den[k]` mod `ctxs[k].p` (`den` = 1 without '/').
pub fn eval_ratio(
    text: &str,
    arity: usize,
    ctxs: &[Ctx],
    num: &mut [u64],
    den: &mut [u64],
) -> Result<CoeffStats, CoeffError> {
    assert!(num.len() == ctxs.len() && den.len() == ctxs.len(), "output length");
    debug_assert!(ctxs.iter().all(|c| c.vals.len() == arity + 1));
    let mut lx = Lex {
        s: text.as_bytes(),
        i: 0,
        arity,
    };
    let mut st = CoeffStats::default();
    num.fill(0);
    // optional sign directly before a parenthesized numerator
    let mut outer_neg = false;
    if let Some(c @ (b'-' | b'+')) = lx.peek() {
        let save = lx.i;
        lx.i += 1;
        if lx.peek() == Some(b'(') {
            outer_neg = c == b'-';
        } else {
            lx.i = save;
        }
    }
    let p_paren = lx.side(ctxs, num, &mut st.num)?;
    if outer_neg {
        for (v, c) in num.iter_mut().zip(ctxs) {
            *v = (c.p - *v) % c.p;
        }
    }
    match lx.peek() {
        None => {
            for (v, c) in den.iter_mut().zip(ctxs) {
                *v = 1 % c.p;
            }
            return Ok(st);
        }
        Some(b'/') => {}
        Some(c) => return lx.err(format!("trailing {:?}", c as char)),
    }
    if !p_paren && st.num.terms != 1 {
        return lx.err("a bare numerator before '/' must be one monomial");
    }
    lx.i += 1;
    st.has_den = true;
    den.fill(0);
    let q_paren = lx.side(ctxs, den, &mut st.den)?;
    if !q_paren && st.den.terms != 1 {
        return lx.err("a bare denominator must be one monomial");
    }
    if let Some(c) = lx.peek() {
        return lx.err(format!("trailing {:?} after the denominator", c as char));
    }
    Ok(st)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay_base::arith::{self, P, Q};
    use crate::replay_base::expr::Expr;

    fn ctx(p: u64, vals: &[i64], d: i64) -> Ctx {
        Ctx {
            p,
            vals: vals
                .iter()
                .chain([&d])
                .map(|&x| arith::from_i128(p, x as i128))
                .collect(),
        }
    }

    fn val(text: &str, arity: usize, c: &Ctx) -> Result<(u64, u64), CoeffError> {
        let (mut n, mut d) = ([0u64], [0u64]);
        eval_ratio(text, arity, std::slice::from_ref(c), &mut n, &mut d)?;
        Ok((n[0], d[0]))
    }

    #[test]
    fn agrees_with_the_copied_expr_parser() {
        let p2 = crate::replay_base::primes::verify_primes(7, &[])[0];
        let texts = [
            "n1/(1-n3-n2+n2*n3)",
            "-1/(1-n3-n2+n2*n3)",
            "(3-d)/(-7-n17-n13+2*d)",
            "(-6+2*d)/(-7-n17-n13+2*d)",
            "1",
            "-(2*n1^2*d - 3)/(n0)",
            "( 12345678901234567890123*n3^3*d^2 - n0 )",
            "n0*n1^2-7",
            "(n2)/2",
        ];
        let pt: Vec<i64> = (0..20).map(|i| 3 * i - 17).collect();
        for t in texts {
            for p in [P, p2] {
                let c = ctx(p, &pt, 29);
                let (a, b) = val(t, 20, &c).unwrap();
                let want = Expr::parse(t, 20).unwrap().at_point(p, &pt, Q::int(29)).unwrap();
                assert_eq!(arith::mul(p, a, arith::inv(p, b).unwrap()), want, "{t}");
            }
        }
    }

    #[test]
    fn refuses_outside_the_grammar() {
        let c = ctx(P, &[1, 2, 3], 5);
        for bad in [
            "n1+1/(n2)",        // bare sum before '/'
            "(n1)/(n2)/(n3)",   // two divisions
            "((n1))",           // nested
            "2*(n1+1)",         // implicit product with a parenthesis
            "n1**2",            // python power
            "n3",               // out of range (arity 3)
            "n01",              // leading zero
            "x+1",              // unknown variable
            "1.5",              // noninteger
            "(n1)/n2+1",        // bare sum denominator
            "n1^100001",        // exponent
            "n1 n2",            // missing operator
            "",                 // empty
            "(n1",              // unbalanced
            "n1/(n2))",         // trailing
            "nd",               // malformed
            "d2",               // identifier
        ] {
            assert!(val(bad, 3, &c).is_err(), "{bad:?} must be refused");
        }
        // accepted forms
        for good in ["-1/(n0)", "(n1)/n2", "n0*n1/(1+d)", "+(n0)", " - ( n0 ) / ( d ) ", "0"] {
            assert!(val(good, 3, &c).is_ok(), "{good:?}");
        }
    }

    #[test]
    fn stats_and_zero_denominator() {
        let c = ctx(P, &[1, 2, 3], 5);
        let (mut n, mut d) = ([0u64], [0u64]);
        let st = eval_ratio("(n0*d^2+n1^3)/(n0-1)", 3, std::slice::from_ref(&c), &mut n, &mut d).unwrap();
        assert_eq!(d[0], 0); // pole reported to the caller
        assert_eq!(st.num, SideStats { terms: 2, deg: 3, degd: 2, degn: 3 });
        assert_eq!(st.den, SideStats { terms: 2, deg: 1, degd: 0, degn: 1 });
        assert!(st.has_den);
    }

    #[test]
    fn many_contexts_one_pass() {
        let ps = crate::replay_base::primes::verify_primes(3, &[]);
        let cs: Vec<Ctx> = (0..6).map(|k| ctx(ps[k % 2], &[k as i64, 2, 9], 11 + k as i64)).collect();
        let t = "(n0^2*d-4*n1+n2*n0)/(d-1)";
        let (mut n, mut d) = (vec![0; 6], vec![0; 6]);
        eval_ratio(t, 3, &cs, &mut n, &mut d).unwrap();
        for (k, c) in cs.iter().enumerate() {
            assert_eq!(val(t, 3, c).unwrap(), (n[k], d[k]));
        }
    }
}
