//! The node order of the table header, re-derived from the engine: `integral_order` (rustred-core node/descent.rs) =
//! `OrderDescriptor::spired_uncut(n)` with the header's coordinate priority (rustred-order descriptor.rs), compared by
//! `compare_views` (rustred-order compare.rs) on concrete keys, simpler-first. Program: support count, support weights
//! (all 0), support bits in support-priority order (bit off = simpler), degree row 0 = total excess (active n-1,
//! inactive -n), degree row 1 = excess of the inactive coordinates, then coordinate ties ActiveFirst, both directions
//! Descending (larger excess = simpler). A rule term is LOWER than its target iff it is simpler (the engine's
//! descent check `IntegralOrder::compare(target, term) == Less` with the program reversed).

use std::cmp::Ordering;

#[derive(Clone, Debug)]
pub struct Order {
    pub support_priority: Vec<usize>,
    pub coordinate_priority: Vec<usize>,
}

fn excess(v: i64) -> i128 {
    if v > 0 { v as i128 - 1 } else { -(v as i128) }
}

impl Order {
    /// Less = `a` is simpler (lower) than `b`.
    pub fn cmp_simpler(&self, a: &[i64], b: &[i64]) -> Ordering {
        let n = a.len();
        let ca = a.iter().filter(|&&x| x > 0).count();
        let cb = b.iter().filter(|&&x| x > 0).count();
        if ca != cb {
            return ca.cmp(&cb);
        }
        for &ax in &self.support_priority {
            let c = (a[ax] > 0).cmp(&(b[ax] > 0));
            if c != Ordering::Equal {
                return c;
            }
        }
        let r0a: i128 = a.iter().map(|&x| excess(x)).sum();
        let r0b: i128 = b.iter().map(|&x| excess(x)).sum();
        if r0a != r0b {
            return r0a.cmp(&r0b);
        }
        let r1a: i128 = a.iter().filter(|&&x| x <= 0).map(|&x| excess(x)).sum();
        let r1b: i128 = b.iter().filter(|&&x| x <= 0).map(|&x| excess(x)).sum();
        if r1a != r1b {
            return r1a.cmp(&r1b);
        }
        for pass in 0..2 {
            for &ax in &self.coordinate_priority {
                let active = a[ax] > 0;
                if active != (pass == 0) {
                    continue;
                }
                let c = excess(a[ax]).cmp(&excess(b[ax])).reverse();
                if c != Ordering::Equal {
                    return c;
                }
            }
        }
        debug_assert_eq!(a, b);
        let _ = n;
        Ordering::Equal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spired_shape() {
        let o = Order {
            support_priority: vec![0, 1, 2],
            coordinate_priority: vec![2, 0, 1],
        };
        // fewer active = simpler
        assert_eq!(o.cmp_simpler(&[1, 0, 0], &[1, 1, 0]), Ordering::Less);
        // same support: lower total excess simpler
        assert_eq!(o.cmp_simpler(&[1, 1, 0], &[2, 1, 0]), Ordering::Less);
        assert_eq!(o.cmp_simpler(&[1, 1, -1], &[2, 1, 0]), Ordering::Greater);
        // equal total excess: fewer numerator excess simpler
        assert_eq!(o.cmp_simpler(&[2, 1, 0], &[1, 1, -1]), Ordering::Less);
        // equal rows: active coordinates by priority, larger excess simpler (descending)
        assert_eq!(o.cmp_simpler(&[3, 1, 0], &[2, 2, 0]), Ordering::Less);
        // support bits by priority: bit off simpler
        assert_eq!(o.cmp_simpler(&[0, 1, 0], &[1, 0, 0]), Ordering::Less);
    }
}
