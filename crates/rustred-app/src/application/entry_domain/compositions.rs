//! Stars-and-bars encoding around Symbolica's public combination iterator.
//! No independent combinatorial, integer or polynomial algebra kernel.
use symbolica::combinatorics::CombinationIterator;

pub(super) struct Compositions {
    total: u32,
    dimensions: usize,
    bars: Option<CombinationIterator>,
    single: bool,
}

impl Compositions {
    pub(super) fn new(dimensions: usize, total: u32) -> Self {
        Self {
            total,
            dimensions,
            bars: (dimensions > 1)
                .then(|| CombinationIterator::new(total as usize + dimensions - 1, dimensions - 1)),
            single: dimensions == 1 || dimensions == 0 && total == 0,
        }
    }
}

impl Iterator for Compositions {
    type Item = Vec<u32>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(bars) = &mut self.bars {
            let positions = bars.next()?;
            let mut previous = 0;
            let mut powers = Vec::with_capacity(self.dimensions);
            for &position in positions {
                powers.push((position - previous) as u32);
                previous = position + 1;
            }
            powers.push((self.total as usize + self.dimensions - 1 - previous) as u32);
            Some(powers)
        } else if std::mem::take(&mut self.single) {
            Some(if self.dimensions == 0 {
                Vec::new()
            } else {
                vec![self.total]
            })
        } else {
            None
        }
    }
}
