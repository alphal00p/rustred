//! The order the planner and the gate judge descent with.
//!
//! Production code passes the node's FULL comparator ([`crate::order::NodeComparator`], every tie-break). The trait
//! exists so that the independent graph check can be exercised against a deliberately coarse or broken order in tests
//! (the hand alias cycle of TRICKS item 1 came from edges believed descending under a measure that was not the full
//! order).

use std::cmp::Ordering;

use crate::order::NodeComparator;

/// A total preorder on keys of one arity: `Less` means `a` is LOWER (simpler) than `b` (the orientation of
/// [`NodeComparator::cmp`]). An edge `k -> t` descends iff `cmp_keys(t, k) == Less`; `Equal` never descends.
pub trait KeyOrder {
    fn cmp_keys(&self, a: &[i32], b: &[i32]) -> Ordering;
    /// Short label for reports.
    fn order_label(&self) -> String;
}

impl KeyOrder for NodeComparator {
    fn cmp_keys(&self, a: &[i32], b: &[i32]) -> Ordering {
        // callers check the arity of every key before comparing (`GateError::Arity` / `PlanError::Arity`)
        self.cmp(a, b)
    }
    fn order_label(&self) -> String {
        format!(
            "node comparator {} (full order, every tie-break)",
            self.name()
        )
    }
}

/// An order given by a closure (tests, diagnostics).
pub struct FnOrder<F: Fn(&[i32], &[i32]) -> Ordering> {
    pub f: F,
    pub label: String,
}

impl<F: Fn(&[i32], &[i32]) -> Ordering> KeyOrder for FnOrder<F> {
    fn cmp_keys(&self, a: &[i32], b: &[i32]) -> Ordering {
        (self.f)(a, b)
    }
    fn order_label(&self) -> String {
        self.label.clone()
    }
}

/// How an edge `k -> t` relates to the order.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum EdgeOrder {
    /// `t < k`: descends.
    Lower,
    /// `t == k` under the order (an alias at equal rank, or `t` = `k`): refused.
    Equal,
    /// `t > k`: refused.
    Higher,
}

/// The relation of the edge `key -> term` under `order`.
pub fn edge_order<O: KeyOrder + ?Sized>(order: &O, key: &[i32], term: &[i32]) -> EdgeOrder {
    match order.cmp_keys(term, key) {
        Ordering::Less => EdgeOrder::Lower,
        Ordering::Equal => EdgeOrder::Equal,
        Ordering::Greater => EdgeOrder::Higher,
    }
}
