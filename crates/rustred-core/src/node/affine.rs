//! Affine faces: cases whose free axes are coupled by linear equations `A n = b` (exception branches of a parent
//! rule whose coefficients have the linear factor `l(n) = a.n - b` in a denominator).
//!
//! Findings (notes/affine-faces.md, oracle + native probes on the 5L p-integral nodes):
//! * a rule for the face needs a pivot TANGENT to it (`A delta = 0`); seeds need not be;
//! * on many affine faces NO coordinate priority has a depth-1 relation (T159, T210, T255: 0 of ~200 orders): the
//!   face is the zero set of a factor of the parent's pivot coefficient, so every depth-1 relation at the face
//!   point loses its pivot there;
//! * at depth 2 every order has a tangent pivot (the case point itself), but the SIZE is set by the order: the
//!   legacy identity order gives relations of 2,000-5,000 rows / 1,400-3,000 RHS terms (the shipped 1.6-5.5M monomial
//!   rules of T255, T210's degree-44 refusal), the boundary-first order R1 of the case ([`super::predict::bfi_case`])
//!   relations of 1-30 rows / 5-20 RHS terms (native: 23-471 monomials);
//! * the small relations live in the radius-1 ball around ONE displaced point `T + u`, `u = +-e_x` along an
//!   equation-coupled free axis (the layer `l = +-1` next to the face, where the parent's relations are regular):
//!   48 seeds instead of the 346 of the full radius-2 ball. Under a bad order no displacement helps;
//! * a unimodular re-posing of the index lattice adapted to the face (tangent basis + normal as new axes, radius 1
//!   in the new axes) does NOT give a depth-1 relation: the face is not an ordinary corner in any lattice basis.
//!
//! [`diagnose`] evaluates a case under one coordinate priority: depth 1 at the point, then each displaced ball.

use serde::{Deserialize, Serialize};

use super::predict::{Depth1, OracleCase, OracleFamily, parse_equation};

/// Relations at or below these sizes count as small (native: <= ~1e3 monomials on the 5L nodes measured).
pub const SMALL_TRACE: usize = 256;
pub const SMALL_RHS: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShiftProbe {
    /// Displacement of the seed ball centre (original slots).
    pub shift: Vec<i32>,
    pub result: Depth1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffineDiagnosis {
    /// Free axes with a nonzero coefficient in some equation.
    pub coupled: Vec<usize>,
    pub depth1: Depth1,
    pub shifted: Vec<ShiftProbe>,
    /// Index into `shifted` of the smallest relation (rows + RHS), if any.
    pub best: Option<usize>,
}

impl AffineDiagnosis {
    /// The smallest relation found (rows + RHS): the depth-1 one or the best displaced ball. A depth-1 relation can
    /// be the LARGER one (T255 #1854 under R1: depth 1 202 rows / 224 RHS, displaced ball 18 / 5).
    pub fn smallest(&self) -> Option<Depth1> {
        let d2 = self.best.map(|b| self.shifted[b].result);
        match (self.depth1.rule.then_some(self.depth1), d2) {
            (Some(a), Some(b)) => Some(if b.trace + b.rhs < a.trace + a.rhs {
                b
            } else {
                a
            }),
            (a, b) => a.or(b),
        }
    }

    /// A depth-2 relation within [`SMALL_TRACE`] / [`SMALL_RHS`] exists (or a depth-1 one).
    pub fn small(&self) -> bool {
        self.smallest()
            .is_some_and(|d| d.trace <= SMALL_TRACE && d.rhs <= SMALL_RHS)
    }
}

/// Free axes coupled by the equations of a case.
pub fn coupled_axes(fixed: &[Option<i64>], equations: &[String]) -> Result<Vec<usize>, String> {
    let n = fixed.len();
    let mut out = Vec::new();
    for e in equations {
        let (c, _) = parse_equation(e, n)?;
        for i in 0..n {
            if c[i] != 0 && fixed[i].is_none() && !out.contains(&i) {
                out.push(i);
            }
        }
    }
    out.sort_unstable();
    Ok(out)
}

/// The displacements `+-e_x` of the coupled axes.
pub fn shifts(n: usize, coupled: &[usize]) -> Vec<Vec<i32>> {
    let mut out = Vec::new();
    for &x in coupled {
        for s in [1, -1] {
            let mut u = vec![0i32; n];
            u[x] = s;
            out.push(u);
        }
    }
    out
}

/// Prepared oracle cases of one affine case: the point and each displaced ball (rows do not depend on the order).
pub struct AffinePrepared {
    pub coupled: Vec<usize>,
    point: OracleCase,
    shifted: Vec<(Vec<i32>, OracleCase)>,
}

impl AffinePrepared {
    pub fn new(
        fam: &OracleFamily,
        fixed: &[Option<i64>],
        equations: &[String],
        seed: u64,
    ) -> Result<Self, String> {
        let coupled = coupled_axes(fixed, equations)?;
        let point = OracleCase::new(fam, fixed, equations, seed)?;
        let shifted = shifts(fam.n, &coupled)
            .into_iter()
            .map(|u| OracleCase::new_centred(fam, fixed, equations, seed, Some(&u)).map(|c| (u, c)))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            coupled,
            point,
            shifted,
        })
    }

    /// Smallest relation of the displaced balls only (skips the depth-1 test).
    pub fn best_shifted(&self, prio: &[usize]) -> Option<(usize, Depth1)> {
        let mut best: Option<(usize, Depth1)> = None;
        for (k, (_, c)) in self.shifted.iter().enumerate() {
            let r = c.depth1(prio);
            if r.rule
                && best
                    .as_ref()
                    .is_none_or(|(_, b)| (r.trace + r.rhs, r.rhs) < (b.trace + b.rhs, b.rhs))
            {
                best = Some((k, r));
            }
        }
        best
    }

    pub fn diagnose(&self, prio: &[usize]) -> AffineDiagnosis {
        let depth1 = self.point.depth1(prio);
        let shifted: Vec<ShiftProbe> = self
            .shifted
            .iter()
            .map(|(u, c)| ShiftProbe {
                shift: u.clone(),
                result: c.depth1(prio),
            })
            .collect();
        let best = shifted
            .iter()
            .enumerate()
            .filter(|(_, p)| p.result.rule)
            .min_by_key(|(_, p)| (p.result.trace + p.result.rhs, p.result.rhs))
            .map(|(k, _)| k);
        AffineDiagnosis {
            coupled: self.coupled.clone(),
            depth1,
            shifted,
            best,
        }
    }
}

/// Depth-1 and displaced-ball depth-2 relations of one case under the coordinate priority `prio`.
pub fn diagnose(
    fam: &OracleFamily,
    fixed: &[Option<i64>],
    equations: &[String],
    prio: &[usize],
) -> Result<AffineDiagnosis, String> {
    Ok(AffinePrepared::new(fam, fixed, equations, 1)?.diagnose(prio))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// two-loop family with q: 3 lines, 2 ISPs (same as node::predict tests)
    const FAM: &str = "I(\n  name(T),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1-k2+q)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";

    #[test]
    fn coupled_axes_and_shifts() {
        let fixed = [None, None, Some(1), Some(0), None];
        let eq = vec!["2-n1+n0".to_string()];
        assert_eq!(coupled_axes(&fixed, &eq).unwrap(), vec![0, 1]);
        assert_eq!(
            shifts(5, &[0, 1]),
            vec![
                vec![1, 0, 0, 0, 0],
                vec![-1, 0, 0, 0, 0],
                vec![0, 1, 0, 0, 0],
                vec![0, -1, 0, 0, 0]
            ]
        );
        assert!(coupled_axes(&fixed, &[]).unwrap().is_empty());
    }

    #[test]
    fn displaced_balls_contain_the_point_and_find_tangent_pivots() {
        let f = OracleFamily::parse(FAM).unwrap();
        let fixed = [None, None, Some(1), Some(0), Some(0)];
        let eq = vec!["-n1+n0".to_string()];
        let p = AffinePrepared::new(&f, &fixed, &eq, 1).unwrap();
        assert_eq!(p.shifted.len(), 4);
        let d = p.diagnose(&[0, 1, 2, 3, 4]);
        assert_eq!(d.coupled, vec![0, 1]);
        // the generic two-loop face n0 = n1 reduces (depth 1 or a displaced ball)
        assert!(d.smallest().is_some());
        // an un-displaced centre gives the depth-1 oracle back
        let c0 = OracleCase::new_centred(&f, &fixed, &eq, 1, None).unwrap();
        let c1 = OracleCase::new(&f, &fixed, &eq, 1).unwrap();
        assert_eq!(c0.rows(), c1.rows());
        assert_eq!(c0.depth1(&[0, 1, 2, 3, 4]), c1.depth1(&[0, 1, 2, 3, 4]));
    }
}
