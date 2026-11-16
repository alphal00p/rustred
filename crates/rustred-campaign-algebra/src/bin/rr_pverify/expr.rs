//! The table's scalar expressions (integer literals, `n<i>`, `d`, `+ - * /`, `^` / `**` with constant integer exponents,
//! parentheses; Python precedence, `^` right associative, `-a^2 = -(a^2)`), re-implemented from the ENGINE's grammar
//! (rustred-core node/replay/expr.rs at f9132a56), with EXACT evaluation (Symbolica Integer / Rational) instead of the
//! engine's two-prime modular evaluation, and an exact conversion to Symbolica polynomials.

use std::sync::Arc;

use symbolica::atom::Symbol;
use symbolica::domains::integer::{Integer, IntegerRing, Z};
use symbolica::domains::rational::{Q, Rational, RationalField};
use symbolica::poly::PolyVariable;
use symbolica::poly::polynomial::MultivariatePolynomial;

pub type ZPoly = MultivariatePolynomial<IntegerRing, u16>;
pub type QPoly = MultivariatePolynomial<RationalField, u16>;

#[derive(Clone, Debug)]
pub enum Node {
    Num(Integer),
    /// `n<i>` = Var(i); `d` = Var(n).
    Var(usize),
    Neg(Box<Node>),
    Pow(Box<Node>, i64),
    /// (negated, term)
    Sum(Vec<(bool, Node)>),
    /// (is_division, factor); the first factor is a multiplication
    Prod(Vec<(bool, Node)>),
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub text: String,
    pub root: Node,
    pub n: usize,
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    n: usize,
}

impl Parser<'_> {
    fn peek(&mut self) -> Option<u8> {
        while self.i < self.s.len() && (self.s[self.i] as char).is_whitespace() {
            self.i += 1;
        }
        self.s.get(self.i).copied()
    }
    fn expr(&mut self) -> Result<Node, String> {
        let first = self.term()?;
        let mut terms = vec![(false, first)];
        while let Some(c @ (b'+' | b'-')) = self.peek() {
            self.i += 1;
            terms.push((c == b'-', self.term()?));
        }
        Ok(if terms.len() == 1 && !terms[0].0 {
            terms.pop().unwrap().1
        } else {
            Node::Sum(terms)
        })
    }
    fn term(&mut self) -> Result<Node, String> {
        let mut f = vec![(false, self.unary()?)];
        loop {
            match self.peek() {
                Some(b'*') if self.s.get(self.i + 1) != Some(&b'*') => {
                    self.i += 1;
                    f.push((false, self.unary()?));
                }
                Some(b'/') => {
                    self.i += 1;
                    f.push((true, self.unary()?));
                }
                _ => break,
            }
        }
        Ok(if f.len() == 1 {
            f.pop().unwrap().1
        } else {
            Node::Prod(f)
        })
    }
    fn unary(&mut self) -> Result<Node, String> {
        match self.peek() {
            Some(b'-') => {
                self.i += 1;
                Ok(Node::Neg(Box::new(self.unary()?)))
            }
            Some(b'+') => {
                self.i += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Result<Node, String> {
        let base = self.atom()?;
        let is_pow = match self.peek() {
            Some(b'^') => {
                self.i += 1;
                true
            }
            Some(b'*') if self.s.get(self.i + 1) == Some(&b'*') => {
                self.i += 2;
                true
            }
            _ => false,
        };
        if !is_pow {
            return Ok(base);
        }
        let e = self.unary()?;
        let (num, den) = const_value(&e)?;
        if den != 1 || num.abs() >= 100_000 {
            return Err("unsupported exponent".into());
        }
        Ok(Node::Pow(Box::new(base), num as i64))
    }
    fn atom(&mut self) -> Result<Node, String> {
        match self.peek() {
            Some(b'(') => {
                self.i += 1;
                let e = self.expr()?;
                if self.peek() != Some(b')') {
                    return Err("missing ')'".into());
                }
                self.i += 1;
                Ok(e)
            }
            Some(c) if c.is_ascii_digit() => {
                let st = self.i;
                while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                    self.i += 1;
                }
                if self.i < self.s.len()
                    && (self.s[self.i] == b'.' || self.s[self.i].is_ascii_alphabetic())
                {
                    return Err("noninteger scalar literal".into());
                }
                let t = std::str::from_utf8(&self.s[st..self.i]).unwrap();
                Ok(Node::Num(parse_integer(t)?))
            }
            Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                let st = self.i;
                while self.i < self.s.len()
                    && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_')
                {
                    self.i += 1;
                }
                let name = std::str::from_utf8(&self.s[st..self.i]).unwrap();
                if name == "d" {
                    return Ok(Node::Var(self.n));
                }
                let ok = name.len() > 1
                    && name.starts_with('n')
                    && name[1..].chars().all(|c| c.is_ascii_digit())
                    && (name == "n0" || !name[1..].starts_with('0'));
                if ok
                    && let Ok(k) = name[1..].parse::<usize>()
                    && k < self.n
                {
                    return Ok(Node::Var(k));
                }
                Err(format!("unsupported variable {name}"))
            }
            other => Err(format!(
                "unsupported syntax at byte {} ({:?})",
                self.i,
                other.map(|c| c as char)
            )),
        }
    }
}

fn parse_integer(t: &str) -> Result<Integer, String> {
    if t.len() <= 18 {
        return Ok(Integer::from(t.parse::<i64>().map_err(|e| e.to_string())?));
    }
    t.parse::<Integer>().map_err(|_| format!("bad integer {t}"))
}

/// Constant value of an exponent expression as an exact fraction of i128 (the engine's `const_int`).
fn const_value(n: &Node) -> Result<(i128, i128), String> {
    fn norm(a: i128, b: i128) -> Result<(i128, i128), String> {
        if b == 0 {
            return Err("exponent division by zero".into());
        }
        let g = gcd(a.abs(), b.abs()).max(1);
        let (a, b) = (a / g, b / g);
        Ok(if b < 0 { (-a, -b) } else { (a, b) })
    }
    match n {
        Node::Num(x) => Ok((x.to_i64().ok_or("exponent too large")? as i128, 1)),
        Node::Var(_) => Err("nonconstant exponent".into()),
        Node::Neg(a) => {
            let (p, q) = const_value(a)?;
            Ok((-p, q))
        }
        Node::Pow(..) => Err("nonconstant exponent".into()),
        Node::Sum(ts) => {
            let mut acc = (0i128, 1i128);
            for (neg, t) in ts {
                let (p, q) = const_value(t)?;
                let p = if *neg { -p } else { p };
                acc = norm(
                    acc.0.checked_mul(q).ok_or("ovf")? + p.checked_mul(acc.1).ok_or("ovf")?,
                    acc.1.checked_mul(q).ok_or("ovf")?,
                )?;
            }
            Ok(acc)
        }
        Node::Prod(fs) => {
            let mut acc = (1i128, 1i128);
            for (div, f) in fs {
                let (p, q) = const_value(f)?;
                acc = if *div {
                    norm(acc.0 * q, acc.1 * p)?
                } else {
                    norm(acc.0 * p, acc.1 * q)?
                };
            }
            Ok(acc)
        }
    }
}

pub fn gcd(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.abs()
}

impl Expr {
    pub fn parse(text: &str, n: usize) -> Result<Expr, String> {
        if text.contains("...") || text.contains('…') {
            return Err("truncated polynomial".into());
        }
        let mut p = Parser {
            s: text.as_bytes(),
            i: 0,
            n,
        };
        let root = p.expr()?;
        if p.peek().is_some() {
            return Err(format!("trailing input at byte {}", p.i));
        }
        Ok(Expr {
            text: text.to_string(),
            root,
            n,
        })
    }

    pub fn uses_d(&self) -> bool {
        fn go(x: &Node, n: usize) -> bool {
            match x {
                Node::Num(_) => false,
                Node::Var(v) => *v == n,
                Node::Neg(a) | Node::Pow(a, _) => go(a, n),
                Node::Sum(t) | Node::Prod(t) => t.iter().any(|(_, y)| go(y, n)),
            }
        }
        go(&self.root, self.n)
    }

    pub fn has_division(&self) -> bool {
        fn go(x: &Node) -> bool {
            match x {
                Node::Num(_) | Node::Var(_) => false,
                Node::Neg(a) => go(a),
                Node::Pow(a, e) => *e < 0 || go(a),
                Node::Sum(t) => t.iter().any(|(_, y)| go(y)),
                Node::Prod(t) => t.iter().any(|(div, y)| *div || go(y)),
            }
        }
        go(&self.root)
    }

    /// Exact value at an integer point and rational `d`; Err on a zero divisor (the engine's "sample pole").
    pub fn eval(&self, pt: &[i64], d: &Rational) -> Result<Rational, String> {
        eval_node(&self.root, pt, d, self.n)
    }

    /// Exact INTEGER value of a division-free expression at an integer point, `d` = an integer.
    pub fn eval_int(&self, pt: &[i64], d: i64) -> Result<Integer, String> {
        eval_int_node(&self.root, pt, d, self.n)
    }

    /// The divisors of the expression as written: every right operand of '/' and every base of a negative power.
    pub fn divisors(&self) -> Vec<Node> {
        fn go(x: &Node, out: &mut Vec<Node>) {
            match x {
                Node::Num(_) | Node::Var(_) => {}
                Node::Neg(a) => go(a, out),
                Node::Pow(a, e) => {
                    if *e < 0 {
                        out.push((**a).clone());
                    }
                    go(a, out)
                }
                Node::Sum(t) => t.iter().for_each(|(_, y)| go(y, out)),
                Node::Prod(t) => {
                    for (div, y) in t {
                        if *div {
                            out.push(y.clone());
                        }
                        go(y, out)
                    }
                }
            }
        }
        let mut v = vec![];
        go(&self.root, &mut v);
        v
    }
}

fn eval_node(x: &Node, pt: &[i64], d: &Rational, n: usize) -> Result<Rational, String> {
    Ok(match x {
        Node::Num(a) => Rational::from(a),
        Node::Var(v) => {
            if *v == n {
                d.clone()
            } else {
                Rational::from(pt[*v])
            }
        }
        Node::Neg(a) => -eval_node(a, pt, d, n)?,
        Node::Pow(a, e) => {
            let b = eval_node(a, pt, d, n)?;
            if *e >= 0 {
                b.pow(*e as u64)
            } else {
                if b.is_zero() {
                    return Err("sample pole (negative power of zero)".into());
                }
                b.inv().pow((-*e) as u64)
            }
        }
        Node::Sum(ts) => {
            let mut acc = Rational::zero();
            for (neg, t) in ts {
                let v = eval_node(t, pt, d, n)?;
                acc = if *neg { acc - v } else { acc + v };
            }
            acc
        }
        Node::Prod(fs) => {
            let mut acc = Rational::one();
            for (div, f) in fs {
                let v = eval_node(f, pt, d, n)?;
                if *div {
                    if v.is_zero() {
                        return Err("sample pole (division by zero)".into());
                    }
                    acc /= v;
                } else {
                    acc *= v;
                }
            }
            acc
        }
    })
}

fn eval_int_node(x: &Node, pt: &[i64], d: i64, n: usize) -> Result<Integer, String> {
    Ok(match x {
        Node::Num(a) => a.clone(),
        Node::Var(v) => Integer::from(if *v == n { d } else { pt[*v] }),
        Node::Neg(a) => -eval_int_node(a, pt, d, n)?,
        Node::Pow(a, e) => {
            if *e < 0 {
                return Err("negative power in an integer evaluation".into());
            }
            eval_int_node(a, pt, d, n)?.pow(*e as u64)
        }
        Node::Sum(ts) => {
            let mut acc = Integer::zero();
            for (neg, t) in ts {
                let v = eval_int_node(t, pt, d, n)?;
                if *neg { acc -= &v } else { acc += &v }
            }
            acc
        }
        Node::Prod(fs) => {
            let mut acc = Integer::one();
            for (div, f) in fs {
                if *div {
                    return Err("division in an integer evaluation".into());
                }
                acc = &acc * &eval_int_node(f, pt, d, n)?;
            }
            acc
        }
    })
}

/// Variables `n0..n{n-1}, d` of every polynomial of the verifier.
pub fn poly_vars(n: usize) -> Arc<Vec<PolyVariable>> {
    let mut v: Vec<PolyVariable> = (0..n)
        .map(|i| PolyVariable::Symbol(Symbol::parse(format!("n{i}"), "rr_pverify").unwrap()))
        .collect();
    v.push(PolyVariable::Symbol(
        Symbol::parse("d", "rr_pverify").unwrap(),
    ));
    Arc::new(v)
}

/// Exact conversion of a division-free expression to an integer polynomial in `n0..n{n-1}, d`.
/// Flat sums of monomials (the expanded form of the table's coefficient sides) are collected, sorted and merged once.
pub fn to_zpoly(x: &Node, n: usize, vars: &Arc<Vec<PolyVariable>>) -> Result<ZPoly, String> {
    let nv = n + 1;
    // fast path: monomial
    fn monomial(x: &Node, nv: usize) -> Option<(Integer, Vec<u16>)> {
        match x {
            Node::Num(a) => Some((a.clone(), vec![0; nv])),
            Node::Var(v) => {
                let mut e = vec![0u16; nv];
                e[*v] = 1;
                Some((Integer::one(), e))
            }
            Node::Neg(a) => monomial(a, nv).map(|(c, e)| (-c, e)),
            Node::Pow(a, p) if *p >= 0 && *p < 60000 => {
                let (c, e) = monomial(a, nv)?;
                let p16 = *p as u16;
                let mut e2 = e.clone();
                for x in e2.iter_mut() {
                    *x = x.checked_mul(p16)?;
                }
                Some((c.pow(*p as u64), e2))
            }
            Node::Prod(fs) => {
                let mut c = Integer::one();
                let mut e = vec![0u16; nv];
                for (div, f) in fs {
                    if *div {
                        return None;
                    }
                    let (c2, e2) = monomial(f, nv)?;
                    c = &c * &c2;
                    for (a, b) in e.iter_mut().zip(e2) {
                        *a = a.checked_add(b)?;
                    }
                }
                Some((c, e))
            }
            _ => None,
        }
    }
    if let Some((c, e)) = monomial(x, nv) {
        let mut p = ZPoly::new(&Z, Some(1), vars.clone());
        p.append_monomial(c, &e);
        return Ok(p);
    }
    match x {
        Node::Sum(ts) => {
            let mut mons: Vec<(Vec<u16>, Integer)> = Vec::with_capacity(ts.len());
            let mut rest: Vec<(bool, &Node)> = vec![];
            for (neg, t) in ts {
                match monomial(t, nv) {
                    Some((c, e)) => mons.push((e, if *neg { -c } else { c })),
                    None => rest.push((*neg, t)),
                }
            }
            mons.sort_by(|a, b| a.0.cmp(&b.0));
            let mut p = ZPoly::new(&Z, Some(mons.len()), vars.clone());
            let mut i = 0;
            while i < mons.len() {
                let mut c = mons[i].1.clone();
                let mut j = i + 1;
                while j < mons.len() && mons[j].0 == mons[i].0 {
                    c += &mons[j].1;
                    j += 1;
                }
                if !c.is_zero() {
                    p.append_monomial(c, &mons[i].0);
                }
                i = j;
            }
            for (neg, t) in rest {
                let q = to_zpoly(t, n, vars)?;
                p = if neg { p - q } else { p + q };
            }
            Ok(p)
        }
        Node::Neg(a) => Ok(-to_zpoly(a, n, vars)?),
        Node::Pow(a, e) => {
            if *e < 0 {
                return Err("negative power in a polynomial".into());
            }
            Ok(to_zpoly(a, n, vars)?.pow(*e as usize))
        }
        Node::Prod(fs) => {
            let mut p = ZPoly::new(&Z, Some(1), vars.clone()).one();
            for (div, f) in fs {
                if *div {
                    return Err("division in a polynomial".into());
                }
                p = &p * &to_zpoly(f, n, vars)?;
            }
            Ok(p)
        }
        Node::Num(_) | Node::Var(_) => unreachable!(),
    }
}

pub fn zpoly_to_q(p: &ZPoly) -> QPoly {
    p.map_coeff(|c| Rational::from(c), Q)
}

/// A coefficient written as N / D (D = 1 when there is no division), both sides division-free.
pub fn split_ratio(x: &Node) -> Option<(Node, Node)> {
    let one = Node::Num(Integer::one());
    match x {
        Node::Prod(fs) => {
            let mut num = vec![];
            let mut den = vec![];
            for (div, f) in fs {
                if *div {
                    den.push((false, f.clone()));
                } else {
                    num.push((false, f.clone()));
                }
            }
            let mk = |v: Vec<(bool, Node)>| match v.len() {
                0 => one.clone(),
                1 => v.into_iter().next().unwrap().1,
                _ => Node::Prod(v),
            };
            Some((mk(num), mk(den)))
        }
        Node::Neg(a) => {
            let (p, q) = split_ratio(a)?;
            Some((Node::Neg(Box::new(p)), q))
        }
        _ => Some((x.clone(), one)),
    }
}

/// Does a node contain any division (or negative power)?
pub fn node_has_division(x: &Node) -> bool {
    match x {
        Node::Num(_) | Node::Var(_) => false,
        Node::Neg(a) => node_has_division(a),
        Node::Pow(a, e) => *e < 0 || node_has_division(a),
        Node::Sum(t) => t.iter().any(|(_, y)| node_has_division(y)),
        Node::Prod(t) => t.iter().any(|(div, y)| *div || node_has_division(y)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_and_eval() {
        let e = Expr::parse("-n1^2+3*n0*d-(n2-1)/(n3)", 4).unwrap();
        let v = e.eval(&[2, 3, 5, 2], &Rational::from((37, 7))).unwrap();
        // -9 + 3*2*37/7 - 4/2 = -11 + 222/7 = 145/7
        assert_eq!(v, Rational::from((145, 7)));
        assert!(e.eval(&[2, 3, 5, 0], &Rational::from(1)).is_err());
        // the engine's const_int refuses a power inside an exponent: so does this grammar
        assert!(Expr::parse("2^3^2", 1).is_err());
        let p = Expr::parse("2^(1+2)", 1).unwrap();
        assert_eq!(p.eval(&[0], &Rational::zero()).unwrap(), Rational::from(8));
        let q = Expr::parse("-2^2", 1).unwrap();
        assert_eq!(q.eval(&[0], &Rational::zero()).unwrap(), Rational::from(-4));
        assert!(Expr::parse("n01", 3).is_err());
        assert!(Expr::parse("n3", 3).is_err());
        assert!(Expr::parse("1.5", 3).is_err());
    }

    #[test]
    fn poly_conversion() {
        let vars = poly_vars(2);
        let e = Expr::parse("n0*n0 + 2*n0*n1 + n1^2 - (n0+n1)^2 + d", 2).unwrap();
        let p = to_zpoly(&e.root, 2, &vars).unwrap();
        let f = Expr::parse("d", 2).unwrap();
        let q = to_zpoly(&f.root, 2, &vars).unwrap();
        assert_eq!(p, q);
    }
}
