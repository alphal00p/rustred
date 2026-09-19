//! Bounded routing proposals for full-rank `L+1`-line vacuum supports.
//!
//! A primitive circuit determines a rational routing class, not an integer
//! lattice class. We try one deterministic, power-preserving correspondence
//! to the least existing member and independently verify its integral,
//! unit-Jacobian momentum map. A missed correspondence is harmless: the raw
//! terminal is retained. No factorial permutation search or oracle is used.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use symbolica::prelude::PolyVariable;

use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::OrderingPolicy;

use super::{
    ProductSkipReason as Skip, TerminalAliasError as Error, TerminalAliasPlan,
    VerifiedTerminalAlias,
};

mod proposal;
mod verify;

#[cfg(test)]
mod audit_tests;
#[cfg(test)]
mod tests;

use proposal::{Candidate, Signature, Support};

impl TerminalAliasPlan {
    /// Prepare exact unit-coefficient vacuum routing aliases.
    ///
    /// Includes the independent-tadpole lane and a disjoint `L+1`-active-line
    /// lane. All inactive indices must be zero, masses one, analytic shifts
    /// zero and active denominators integer momentum squares minus one.
    /// The unique primitive momentum circuit and powers propose one routing;
    /// integer entries, determinant ±1 and the generic exact symmetry verifier
    /// establish every accepted alias. A basis need not itself be unimodular.
    ///
    /// Equal circuit signatures alone do not prove integer-lattice equivalence.
    /// A failed deterministic correspondence is left unchanged rather than
    /// searching permutations. Thus this is an optional finite-terminal
    /// simplification, never a completeness or master-minimality claim.
    pub fn vacuum_routing_equivalences(
        family: &IntegralFamily,
        raw: &BTreeSet<IntegralKey>,
        ordering: OrderingPolicy,
    ) -> Result<Self, Error> {
        let mut plan = Self::independent_tadpole_products(family, raw, ordering)?;
        if family.external_count() != 0 || family.power_shifts().iter().any(|s| !s.is_zero()) {
            return Ok(plan);
        }
        let Some(active_count) = family.loop_count().checked_add(1) else {
            return Err(Error::InvalidCircuitWitness);
        };
        let variables = Arc::new(
            (0..family.loop_count())
                .map(PolyVariable::Temporary)
                .collect(),
        );
        let mut momenta = vec![None; family.denominator_count()];
        let mut supports: BTreeMap<Vec<usize>, Result<Support, Skip>> = BTreeMap::new();
        let mut groups: BTreeMap<Signature, Vec<Candidate>> = BTreeMap::new();
        for key in raw {
            if key.powers().iter().any(|&n| n < 0) {
                continue;
            }
            let slots: Vec<_> = key
                .powers()
                .iter()
                .enumerate()
                .filter_map(|(slot, &n)| (n > 0).then_some(slot))
                .collect();
            if slots.len() != active_count {
                continue;
            }
            // Replace the first lane's active-count disposition, rather than
            // reporting an admitted circuit as an unsupported product too.
            if let Some(count) = plan.statistics.skipped.get_mut(&Skip::ActiveLineCount) {
                *count -= 1;
                if *count == 0 {
                    plan.statistics.skipped.remove(&Skip::ActiveLineCount);
                }
            }
            if !supports.contains_key(&slots) {
                plan.statistics.analyzed_corank_one_supports += 1;
                let support = proposal::support(
                    family,
                    &slots,
                    &variables,
                    &mut momenta,
                    &mut plan.statistics,
                )?;
                supports.insert(slots.clone(), support);
            }
            match supports.get(&slots).expect("support initialized") {
                Ok(support) => {
                    let (signature, candidate) =
                        proposal::candidate(family, key, support, ordering)?;
                    plan.statistics.eligible_corank_one += 1;
                    groups.entry(signature).or_default().push(candidate);
                }
                Err(reason) => plan.statistics.skip(*reason),
            }
        }
        for mut group in groups.into_values() {
            group.sort_by(|a, b| a.complexity.cmp(&b.complexity));
            let representative = &group[0];
            for source in &group[1..] {
                match verify::alias(family, source, representative)? {
                    Ok(witness) => {
                        plan.canonical.remove(&source.key);
                        plan.aliases.insert(
                            source.key.clone(),
                            VerifiedTerminalAlias {
                                representative: representative.key.clone(),
                                witness: Arc::new(witness),
                            },
                        );
                    }
                    Err(reason) => plan.statistics.skip(reason),
                }
            }
        }
        plan.statistics.verified_aliases = plan.aliases.len();
        plan.statistics.canonical_terminals = plan.canonical.len();
        Ok(plan)
    }
}
