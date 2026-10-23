//! Ordered batch union of already validated nonzero conditions.

use super::{Coefficient, TerminalRelationSession};
use std::collections::HashSet;

impl TerminalRelationSession {
    /// Preserve the existing prefix and first incoming occurrence of each
    /// novel condition. Membership borrows both slices; only additions clone
    /// coefficient payloads, and hash iteration never controls output order.
    pub(super) fn merge_nonzero_conditions(
        existing: &mut Vec<Coefficient>,
        incoming: &[Coefficient],
    ) {
        if incoming.is_empty() {
            return;
        }
        let additions = {
            let mut seen: HashSet<&Coefficient> = existing.iter().collect();
            let mut additions = Vec::new();
            for condition in incoming {
                if seen.insert(condition) {
                    additions.push(condition.clone());
                }
            }
            additions
        };
        existing.extend(additions);
    }
}
