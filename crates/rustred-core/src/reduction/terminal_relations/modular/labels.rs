//! Temporary, injective column transport for the native projected adapter.
//!
//! Its lexical label ordering preserves the frozen numerical elimination
//! order. Labels carry no physics and never enter a published certificate;
//! exact images are transported back before full physical-column validation.

use super::*;

pub(super) struct ColumnLabels {
    labels: BTreeMap<IntegralKey, IndexShift>,
    physical: BTreeMap<IndexShift, IntegralKey>,
}

impl ColumnLabels {
    pub(super) fn new(
        session: &TerminalRelationSession,
        image: &discovery::DiscoveryImage,
        selected: &[&SourceRow],
        harder: &BTreeSet<IntegralKey>,
        target: &IntegralKey,
    ) -> Result<Self, TerminalRelationError> {
        // Include exact support, not sampled support: a coefficient vanishing
        // at discovery must not erase its physical auxiliary obligation.
        let universe: BTreeSet<_> = selected
            .iter()
            .flat_map(|source| source.terms.keys())
            .chain(harder)
            .chain(std::iter::once(target))
            .cloned()
            .collect();
        let mut unknown = Vec::new();
        let mut known = Vec::new();
        let mut terminals = Vec::new();
        for key in universe {
            if session.terminals.contains(&key) {
                terminals.push(key);
            } else if let Some(&offset) = image.column_offsets.get(&key) {
                known.push((offset, key));
            } else {
                unknown.push(key);
            }
        }
        // New auxiliaries prepend just as in the discovery reducer. Known
        // distance-from-end offsets remain stable under such prepending.
        elimination::sort_keys(&mut unknown);
        known.sort_by(|(left, _), (right, _)| right.cmp(left));
        elimination::sort_keys(&mut terminals);
        let arity = session.family.denominator_count();
        if arity == 0 {
            return Err(invalid(
                "modular column labels require nonempty integral arity",
            ));
        }
        let mut labels = BTreeMap::new();
        let mut physical = BTreeMap::new();
        for (ordinal, key) in unknown
            .into_iter()
            .chain(known.into_iter().map(|(_, key)| key))
            .chain(terminals)
            .enumerate()
        {
            let ordinal = i64::try_from(ordinal)
                .map_err(|_| invalid("modular column label ordinal exceeds native i64"))?;
            let label = IndexShift::try_new(
                std::iter::once(ordinal).chain(std::iter::repeat_n(0, arity - 1)),
                arity,
            )
            .map_err(algebra)?;
            labels.insert(key.clone(), label.clone());
            physical.insert(label, key);
        }
        Ok(Self { labels, physical })
    }

    pub(super) fn label(&self, key: &IntegralKey) -> Result<&IndexShift, TerminalRelationError> {
        self.labels
            .get(key)
            .ok_or_else(|| invalid("physical column missing from modular label universe"))
    }

    pub(super) fn physical(
        &self,
        label: &IndexShift,
    ) -> Result<&IntegralKey, TerminalRelationError> {
        self.physical.get(label).ok_or_else(|| {
            invalid("native projected image contains an unknown modular column label")
        })
    }
}
