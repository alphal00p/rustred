//! Whitelisted scalar expressions of the replay (integer literals, `n<i>`, `d`, `+ - * / ^`, parentheses, integer
//! exponents), with Python precedence (`-a^2 = -(a^2)`, `^` right associative). Own parser; evaluation mod p, exact
//! linear-form extraction and a conservative syntactic degree. Mirrors `Expr` of p5l-replay.v1.

use super::arith::{self, Q};

#[derive(Clone, Debug)]
pub enum Node {
    /// Decimal digits (arbitrary length, reduced mod p on evaluation).
    Num(String),
    /// `n<i>` = Var(i); `d` = Var(n).
    Var(usize),
    Neg(Box<Node>),
    /// `^` only (base, exponent).
    Bin(char, Box<Node>, Box<Node>),
    /// Flat sum: (negated, term). Flat so that long sums never deepen the tree (deep-scalar regression).
    Sum(Vec<(bool, Node)>),
    /// Flat product: ('*' | '/', factor); the first factor's op is '*'.
    Prod(Vec<(char, Node)>),
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub text: String,
    pub root: Node,
    /// Number of index variables `n0..n{n-1}`; `d` is variable `n`.
    pub n: usize,
}

/// Errors of the replay that make a verdict UNKNOWN.
pub type Unknown = String;

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    n: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn expr(&mut self) -> Result<Node, Unknown> {
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
    fn term(&mut self) -> Result<Node, Unknown> {
        let mut factors = vec![('*', self.unary()?)];
        loop {
            match self.peek() {
                Some(b'*') if self.s.get(self.i + 1) != Some(&b'*') => {
                    self.i += 1;
                    factors.push(('*', self.unary()?));
                }
                Some(b'/') => {
                    self.i += 1;
                    factors.push(('/', self.unary()?));
                }
                _ => break,
            }
        }
        Ok(if factors.len() == 1 {
            factors.pop().unwrap().1
        } else {
            Node::Prod(factors)
        })
    }
    fn unary(&mut self) -> Result<Node, Unknown> {
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
    fn power(&mut self) -> Result<Node, Unknown> {
        let base = self.atom()?;
        match self.peek() {
            Some(b'^') => {
                self.i += 1;
                Ok(Node::Bin('^', Box::new(base), Box::new(self.unary()?)))
            }
            Some(b'*') if self.s.get(self.i + 1) == Some(&b'*') => {
                self.i += 2;
                Ok(Node::Bin('^', Box::new(base), Box::new(self.unary()?)))
            }
            _ => Ok(base),
        }
    }
    fn atom(&mut self) -> Result<Node, Unknown> {
        match self.peek() {
            Some(b'(') => {
                self.i += 1;
                let e = self.expr()?;
                if self.peek() != Some(b')') {
                    return Err("unsupported polynomial syntax: missing ')'".into());
                }
                self.i += 1;
                Ok(e)
            }
            Some(c) if c.is_ascii_digit() => {
                let start = self.i;
                while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                    self.i += 1;
                }
                if self.i < self.s.len()
                    && (self.s[self.i] == b'.' || self.s[self.i].is_ascii_alphabetic())
                {
                    return Err("noninteger scalar literal".into());
                }
                Ok(Node::Num(
                    String::from_utf8_lossy(&self.s[start..self.i]).into_owned(),
                ))
            }
            Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                let start = self.i;
                while self.i < self.s.len()
                    && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_')
                {
                    self.i += 1;
                }
                let name = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
                if name == "d" {
                    return Ok(Node::Var(self.n));
                }
                let ok = name.len() > 1
                    && name.starts_with('n')
                    && name[1..].chars().all(|c| c.is_ascii_digit())
                    && (name == "n0" || !name[1..].starts_with('0'));
                if ok {
                    let k: usize = name[1..]
                        .parse()
                        .map_err(|_| format!("unsupported variable {name}"))?;
                    if k < self.n {
                        return Ok(Node::Var(k));
                    }
                }
                Err(format!("unsupported variable {name}"))
            }
            other => Err(format!(
                "unsupported polynomial syntax at byte {} ({:?})",
                self.i,
                other.map(|c| c as char)
            )),
        }
    }
}

fn const_int(node: &Node) -> Result<Q, Unknown> {
    match node {
        Node::Num(s) => Ok(Q::int(
            s.parse::<i128>()
                .map_err(|_| "unsupported exponent".to_string())?,
        )),
        Node::Neg(a) => Ok(const_int(a)?.neg()),
        Node::Bin(op, a, b) => {
            let (a, b) = (const_int(a)?, const_int(b)?);
            match op {
                '+' => a.add(b),
                '-' => a.sub(b),
                '*' => a.mul(b),
                '/' => a.div(b),
                _ => Err("nonconstant exponent".into()),
            }
        }
        Node::Var(_) => Err("nonconstant exponent".into()),
        Node::Sum(ts) => ts.iter().try_fold(Q::ZERO, |acc, (neg, t)| {
            let v = const_int(t)?;
            if *neg { acc.sub(v) } else { acc.add(v) }
        }),
        Node::Prod(fs) => fs.iter().try_fold(Q::ONE, |acc, (op, f)| {
            let v = const_int(f)?;
            if *op == '/' { acc.div(v) } else { acc.mul(v) }
        }),
    }
}

impl Expr {
    pub fn parse(text: &str, n: usize) -> Result<Self, Unknown> {
        if text.contains("...") || text.contains('…') {
            return Err("truncated/non-string polynomial".into());
        }
        let mut p = Parser {
            s: text.as_bytes(),
            i: 0,
            n,
        };
        let root = p.expr()?;
        if p.peek().is_some() {
            return Err(format!(
                "unsupported polynomial syntax: trailing input at byte {}",
                p.i
            ));
        }
        let e = Expr {
            text: text.to_string(),
            root,
            n,
        };
        e.check(&e.root)?;
        Ok(e)
    }

    fn check(&self, node: &Node) -> Result<(), Unknown> {
        match node {
            Node::Num(_) | Node::Var(_) => Ok(()),
            Node::Neg(a) => self.check(a),
            Node::Sum(ts) => ts.iter().try_for_each(|(_, t)| self.check(t)),
            Node::Prod(fs) => fs.iter().try_for_each(|(_, f)| self.check(f)),
            Node::Bin(op, a, b) => {
                self.check(a)?;
                self.check(b)?;
                if *op == '^' {
                    let z = const_int(b)?;
                    if z.d != 1 || z.n.abs() >= 100_000 {
                        return Err("unsupported exponent".into());
                    }
                }
                Ok(())
            }
        }
    }

    /// Evaluate mod `p` at index values `point` and `d`; Err on a zero divisor (sample pole).
    pub fn at_mod(&self, p: u64, point: &[u64], d: u64) -> Result<u64, Unknown> {
        self.eval(&self.root, p, point, d)
    }

    fn eval(&self, node: &Node, p: u64, point: &[u64], d: u64) -> Result<u64, Unknown> {
        Ok(match node {
            Node::Num(s) => s.bytes().fold(0u64, |acc, c| {
                arith::add(p, arith::mul(p, acc, 10), (c - b'0') as u64)
            }),
            Node::Var(k) => {
                if *k == self.n {
                    d
                } else {
                    point[*k] % p
                }
            }
            Node::Neg(a) => arith::neg(p, self.eval(a, p, point, d)?),
            Node::Sum(ts) => {
                let mut acc = 0u64;
                for (neg, t) in ts {
                    let v = self.eval(t, p, point, d)?;
                    acc = if *neg {
                        arith::sub(p, acc, v)
                    } else {
                        arith::add(p, acc, v)
                    };
                }
                acc
            }
            Node::Prod(fs) => {
                let mut acc = 1u64;
                for (op, f) in fs {
                    let v = self.eval(f, p, point, d)?;
                    acc = if *op == '/' {
                        arith::mul(
                            p,
                            acc,
                            arith::inv(p, v)
                                .ok_or_else(|| format!("sample pole: {}", short(&self.text)))?,
                        )
                    } else {
                        arith::mul(p, acc, v)
                    };
                }
                acc
            }
            Node::Bin(op, a, b) => {
                if *op == '^' {
                    let base = self.eval(a, p, point, d)?;
                    let e = const_int(b)?.n;
                    if e >= 0 {
                        arith::pow(p, base, e as u64)
                    } else {
                        arith::pow(
                            p,
                            arith::inv(p, base)
                                .ok_or_else(|| format!("sample pole: {}", short(&self.text)))?,
                            (-e) as u64,
                        )
                    }
                } else {
                    let (x, y) = (self.eval(a, p, point, d)?, self.eval(b, p, point, d)?);
                    match op {
                        '+' => arith::add(p, x, y),
                        '-' => arith::sub(p, x, y),
                        '*' => arith::mul(p, x, y),
                        '/' => arith::mul(
                            p,
                            x,
                            arith::inv(p, y)
                                .ok_or_else(|| format!("sample pole: {}", short(&self.text)))?,
                        ),
                        _ => unreachable!(),
                    }
                }
            }
        })
    }

    /// Integer point, rational d: mod p (d = num/den reduced mod p).
    pub fn at_point(&self, p: u64, point: &[i64], d: Q) -> Result<u64, Unknown> {
        let pt: Vec<u64> = point
            .iter()
            .map(|&x| arith::from_i128(p, x as i128))
            .collect();
        let dv = d.modp(p).ok_or("dimension pole")?;
        self.at_mod(p, &pt, dv)
    }

    /// Exact zero test at an integer point and rational d (two independent 61-bit primes).
    pub fn is_zero_at(&self, point: &[i64], d: Q) -> Result<bool, Unknown> {
        Ok(self.at_point(arith::P, point, d)? == 0 && self.at_point(arith::P2, point, d)? == 0)
    }

    /// Exact linear form `[const, n0.., n{n-1}, d]` (Err if not affine).
    pub fn linear(&self) -> Result<Vec<Q>, Unknown> {
        self.lin(&self.root)
    }

    fn lin(&self, node: &Node) -> Result<Vec<Q>, Unknown> {
        let width = self.n + 2;
        let constant = |q: Q| {
            let mut v = vec![Q::ZERO; width];
            v[0] = q;
            v
        };
        let is_const = |v: &[Q]| v[1..].iter().all(Q::is_zero);
        Ok(match node {
            Node::Num(s) => constant(Q::int(
                s.parse::<i128>()
                    .map_err(|_| "exact rational overflow".to_string())?,
            )),
            Node::Var(k) => {
                let mut v = vec![Q::ZERO; width];
                v[k + 1] = Q::ONE;
                v
            }
            Node::Neg(a) => self.lin(a)?.into_iter().map(Q::neg).collect(),
            Node::Sum(ts) => {
                let mut acc = vec![Q::ZERO; width];
                for (neg, t) in ts {
                    let v = self.lin(t)?;
                    acc = acc
                        .iter()
                        .zip(&v)
                        .map(|(a, b)| if *neg { a.sub(*b) } else { a.add(*b) })
                        .collect::<Result<_, _>>()?;
                }
                acc
            }
            Node::Prod(fs) => {
                let mut acc = constant(Q::ONE);
                for (op, f) in fs {
                    let y = self.lin(f)?;
                    acc = match op {
                        '*' if is_const(&acc) => {
                            y.iter().map(|b| acc[0].mul(*b)).collect::<Result<_, _>>()?
                        }
                        '*' if is_const(&y) => {
                            acc.iter().map(|a| a.mul(y[0])).collect::<Result<_, _>>()?
                        }
                        '/' if is_const(&y) => {
                            acc.iter().map(|a| a.div(y[0])).collect::<Result<_, _>>()?
                        }
                        _ => return Err("non-affine expression".into()),
                    };
                }
                acc
            }
            Node::Bin(op, a, b) => {
                let (x, y) = (self.lin(a)?, self.lin(b)?);
                match op {
                    '+' => x
                        .iter()
                        .zip(&y)
                        .map(|(a, b)| a.add(*b))
                        .collect::<Result<_, _>>()?,
                    '-' => x
                        .iter()
                        .zip(&y)
                        .map(|(a, b)| a.sub(*b))
                        .collect::<Result<_, _>>()?,
                    '*' if is_const(&x) => {
                        y.iter().map(|b| x[0].mul(*b)).collect::<Result<_, _>>()?
                    }
                    '*' if is_const(&y) => {
                        x.iter().map(|a| a.mul(y[0])).collect::<Result<_, _>>()?
                    }
                    '/' if is_const(&y) => {
                        x.iter().map(|a| a.div(y[0])).collect::<Result<_, _>>()?
                    }
                    '^' => {
                        let e = const_int(b)?.n;
                        if is_const(&x) && e >= 0 {
                            let mut r = Q::ONE;
                            for _ in 0..e {
                                r = r.mul(x[0])?;
                            }
                            constant(r)
                        } else if e == 1 {
                            x
                        } else if e == 0 {
                            constant(Q::ONE)
                        } else {
                            return Err("non-affine expression".into());
                        }
                    }
                    _ => return Err("non-affine expression".into()),
                }
            }
        })
    }

    /// Conservative syntactic degree (p5l-replay.v1 `Expr.degree`): variable denominators are refused.
    pub fn degree(&self) -> Result<usize, Unknown> {
        fn deg(node: &Node) -> Result<usize, Unknown> {
            Ok(match node {
                Node::Num(_) => 0,
                Node::Var(_) => 1,
                Node::Neg(a) => deg(a)?,
                Node::Sum(ts) => ts
                    .iter()
                    .map(|(_, t)| deg(t))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .max()
                    .unwrap_or(0),
                Node::Prod(fs) => {
                    let mut total = 0;
                    for (op, f) in fs {
                        let x = deg(f)?;
                        if *op == '/' {
                            if x != 0 {
                                return Err("variable denominator in a required polynomial".into());
                            }
                        } else {
                            total += x;
                        }
                    }
                    total
                }
                Node::Bin(op, a, b) => {
                    let (x, y) = (deg(a)?, deg(b)?);
                    match op {
                        '+' | '-' => x.max(y),
                        '*' => x + y,
                        '/' => {
                            if y != 0 {
                                return Err("variable denominator in a required polynomial".into());
                            }
                            x
                        }
                        _ => {
                            let e = const_int(b)?;
                            if e.d != 1 || e.n < 0 {
                                return Err("negative/noninteger polynomial exponent".into());
                            }
                            x * e.n as usize
                        }
                    }
                }
            })
        }
        deg(&self.root)
    }

    /// True if `d` occurs.
    pub fn has_d(&self) -> bool {
        fn walk(node: &Node, n: usize) -> bool {
            match node {
                Node::Var(k) => *k == n,
                Node::Num(_) => false,
                Node::Neg(a) => walk(a, n),
                Node::Bin(_, a, b) => walk(a, n) || walk(b, n),
                Node::Sum(ts) => ts.iter().any(|(_, t)| walk(t, n)),
                Node::Prod(fs) => fs.iter().any(|(_, f)| walk(f, n)),
            }
        }
        walk(&self.root, self.n)
    }
}

fn short(s: &str) -> String {
    s.chars().take(120).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::replay::arith::P;

    #[test]
    fn parse_eval_precedence() {
        let e = Expr::parse("-n1^2+3*(d-n0)/2", 2).unwrap();
        // n0=1, n1=2, d=5: -4 + 3*4/2 = 2
        assert_eq!(e.at_point(P, &[1, 2], Q::int(5)).unwrap(), 2);
        let e = Expr::parse("2^-1", 1).unwrap();
        assert_eq!(arith::mul(P, e.at_mod(P, &[0], 0).unwrap(), 2), 1);
        assert!(Expr::parse("n2", 2).is_err());
        assert!(Expr::parse("x+1", 2).is_err());
        assert!(Expr::parse("1.5", 2).is_err());
        assert!(
            Expr::parse("1/(n0-1)", 1)
                .unwrap()
                .at_point(P, &[1], Q::int(3))
                .is_err()
        );
    }

    #[test]
    fn linear_degree_and_zero() {
        let e = Expr::parse("1/2*n0-n1+3*d-4", 2).unwrap();
        let l = e.linear().unwrap();
        assert_eq!(
            l,
            vec![Q::int(-4), Q::new(1, 2).unwrap(), Q::int(-1), Q::int(3)]
        );
        assert_eq!(e.degree().unwrap(), 1);
        assert!(Expr::parse("n0*n1", 2).unwrap().linear().is_err());
        assert_eq!(Expr::parse("n0*n1^2", 2).unwrap().degree().unwrap(), 3);
        assert!(
            Expr::parse("n0-n1", 2)
                .unwrap()
                .is_zero_at(&[3, 3], Q::int(1))
                .unwrap()
        );
        assert!(
            !Expr::parse("n0-n1", 2)
                .unwrap()
                .is_zero_at(&[3, 4], Q::int(1))
                .unwrap()
        );
        assert!(Expr::parse("n0+d", 1).unwrap().has_d());
    }

    /// Astra's scalar regressions (replay parser v1.1): a deep nested scalar that overflowed the Python recursion
    /// limit, and a 168k-character coefficient; expected values from the accepted modular evaluator.
    #[test]
    fn deep_and_large_scalar_regressions() {
        for text in [
            include_str!("../testdata/scalar_deep_T206_r1132.json"),
            include_str!("../testdata/scalar_large_T211_r2510.json"),
        ] {
            let x: serde_json::Value = serde_json::from_str(text).unwrap();
            let e = Expr::parse(x["coefficient"].as_str().unwrap(), 20).unwrap();
            let point: Vec<i64> = x["point"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect();
            for (ds, want) in x["dimensions"]
                .as_array()
                .unwrap()
                .iter()
                .zip(x["expected_values"].as_array().unwrap())
            {
                let got = e
                    .at_point(P, &point, Q::parse(ds.as_str().unwrap()).unwrap())
                    .unwrap();
                assert_eq!(got, want.as_u64().unwrap(), "{} at d={ds}", x["integral"]);
            }
        }
    }
}
