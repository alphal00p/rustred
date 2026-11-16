//! Exact small rationals (i128, every operation checked: an overflow is an explicit failure, never a wrong value) and
//! affine forms `c + sum a_i n_i` over them.

use std::cmp::Ordering;

use crate::expr::{Node, gcd};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rat {
    pub n: i128,
    pub d: i128,
}

fn ck(x: Option<i128>) -> i128 {
    x.expect("exact i128 rational overflow (explicit failure, no value is produced)")
}

impl Rat {
    pub const ZERO: Rat = Rat { n: 0, d: 1 };
    pub const ONE: Rat = Rat { n: 1, d: 1 };
    pub fn new(n: i128, d: i128) -> Rat {
        assert!(d != 0, "rational with zero denominator");
        let g = gcd(n, d).max(1);
        let (n, d) = (n / g, d / g);
        if d < 0 {
            Rat { n: -n, d: -d }
        } else {
            Rat { n, d }
        }
    }
    pub fn int(n: i128) -> Rat {
        Rat { n, d: 1 }
    }
    pub fn is_zero(&self) -> bool {
        self.n == 0
    }
    pub fn add(self, o: Rat) -> Rat {
        if self.d == 1 && o.d == 1 {
            return Rat::int(ck(self.n.checked_add(o.n)));
        }
        let g = gcd(self.d, o.d);
        let l = ck((self.d / g).checked_mul(o.d));
        let a = ck(self.n.checked_mul(l / self.d));
        let b = ck(o.n.checked_mul(l / o.d));
        Rat::new(ck(a.checked_add(b)), l)
    }
    pub fn neg(self) -> Rat {
        Rat {
            n: -self.n,
            d: self.d,
        }
    }
    pub fn sub(self, o: Rat) -> Rat {
        self.add(o.neg())
    }
    pub fn mul(self, o: Rat) -> Rat {
        if self.d == 1 && o.d == 1 {
            return Rat::int(ck(self.n.checked_mul(o.n)));
        }
        let g1 = gcd(self.n, o.d).max(1);
        let g2 = gcd(o.n, self.d).max(1);
        Rat::new(
            ck((self.n / g1).checked_mul(o.n / g2)),
            ck((self.d / g2).checked_mul(o.d / g1)),
        )
    }
    pub fn div(self, o: Rat) -> Rat {
        assert!(!o.is_zero(), "division by zero");
        self.mul(Rat::new(o.d, o.n))
    }
    pub fn is_int(&self) -> bool {
        self.d == 1
    }
}

impl PartialOrd for Rat {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Rat {
    fn cmp(&self, o: &Self) -> Ordering {
        ck(self.n.checked_mul(o.d)).cmp(&ck(o.n.checked_mul(self.d)))
    }
}

/// Affine form `c + sum_i a[i] n_i` (d-free), dense over `n` variables.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Lin {
    pub c: Rat,
    pub a: Vec<Rat>,
}

impl Lin {
    pub fn zero(n: usize) -> Lin {
        Lin {
            c: Rat::ZERO,
            a: vec![Rat::ZERO; n],
        }
    }
    pub fn constant(n: usize, c: Rat) -> Lin {
        Lin {
            c,
            a: vec![Rat::ZERO; n],
        }
    }
    pub fn var(n: usize, i: usize) -> Lin {
        let mut l = Lin::zero(n);
        l.a[i] = Rat::ONE;
        l
    }
    pub fn is_const(&self) -> bool {
        self.a.iter().all(Rat::is_zero)
    }
    pub fn add(&self, o: &Lin) -> Lin {
        Lin {
            c: self.c.add(o.c),
            a: self.a.iter().zip(&o.a).map(|(x, y)| x.add(*y)).collect(),
        }
    }
    pub fn scale(&self, s: Rat) -> Lin {
        Lin {
            c: self.c.mul(s),
            a: self.a.iter().map(|x| x.mul(s)).collect(),
        }
    }
    pub fn vars(&self) -> Vec<usize> {
        (0..self.a.len())
            .filter(|&i| !self.a[i].is_zero())
            .collect()
    }
    /// Exact affine form of a d-free node, None if not affine (or uses d).
    pub fn from_node(x: &Node, n: usize) -> Option<Lin> {
        Some(match x {
            Node::Num(v) => Lin::constant(n, Rat::int(v.to_i64()? as i128)),
            Node::Var(v) => {
                if *v >= n {
                    return None;
                }
                Lin::var(n, *v)
            }
            Node::Neg(a) => Lin::from_node(a, n)?.scale(Rat::int(-1)),
            Node::Pow(a, e) => {
                let b = Lin::from_node(a, n)?;
                match *e {
                    0 => Lin::constant(n, Rat::ONE),
                    1 => b,
                    e if b.is_const() && e > 0 => {
                        let mut r = Rat::ONE;
                        for _ in 0..e {
                            r = r.mul(b.c);
                        }
                        Lin::constant(n, r)
                    }
                    _ => return None,
                }
            }
            Node::Sum(ts) => {
                let mut acc = Lin::zero(n);
                for (neg, t) in ts {
                    let l = Lin::from_node(t, n)?;
                    acc = acc.add(&if *neg { l.scale(Rat::int(-1)) } else { l });
                }
                acc
            }
            Node::Prod(fs) => {
                let mut acc = Lin::constant(n, Rat::ONE);
                for (div, f) in fs {
                    let l = Lin::from_node(f, n)?;
                    if *div {
                        if !l.is_const() || l.c.is_zero() {
                            return None;
                        }
                        acc = acc.scale(Rat::ONE.div(l.c));
                    } else if acc.is_const() {
                        acc = l.scale(acc.c);
                    } else if l.is_const() {
                        acc = acc.scale(l.c);
                    } else {
                        return None;
                    }
                }
                acc
            }
        })
    }
    /// `c + a n_v` with integer c, a, a != 0 (the fast exact test of a one-variable literal).
    pub fn single_var_i128(&self) -> Option<(i128, usize, i128)> {
        let vs = self.vars();
        if vs.len() != 1 || !self.c.is_int() || !self.a[vs[0]].is_int() {
            return None;
        }
        Some((self.c.n, vs[0], self.a[vs[0]].n))
    }
    pub fn eval(&self, pt: &[i64]) -> Rat {
        let mut s = self.c;
        for (i, a) in self.a.iter().enumerate() {
            if !a.is_zero() {
                s = s.add(a.mul(Rat::int(pt[i] as i128)));
            }
        }
        s
    }
}
