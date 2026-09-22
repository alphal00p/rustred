//! Inclusion reuse for one immutable program snapshot, not solved-state reuse.
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Domain<const N: usize> {
    pub owner: [bool; N],
    pub lower: Vec<u64>,
    pub upper: Vec<Option<u64>>,
    pub rank: Option<u32>,
}

impl<const N: usize> Domain<N> {
    fn contains(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.rank.is_none_or(|r| other.rank.is_some_and(|s| s <= r))
            && self.lower.iter().zip(&other.lower).all(|(a, b)| a <= b)
            && self
                .upper
                .iter()
                .zip(&other.upper)
                .all(|(a, b)| a.is_none_or(|a| b.is_some_and(|b| b <= a)))
    }
}

pub(super) struct Queue<const N: usize> {
    pub domains: Vec<Domain<N>>,
    pub next: usize,
    pub deduplicated: usize,
    pub containment_checks: usize,
    by_owner: BTreeMap<[bool; N], Vec<usize>>,
    max_domains: usize,
    max_checks: usize,
}

impl<const N: usize> Queue<N> {
    pub fn new(max_domains: usize, max_checks: usize) -> Self {
        Self {
            domains: Vec::new(),
            next: 0,
            deduplicated: 0,
            containment_checks: 0,
            by_owner: BTreeMap::new(),
            max_domains,
            max_checks,
        }
    }

    /// A pending containing domain can suppress another scheduling request, but
    /// every admitted domain still has to finish before worklist exhaustion.
    /// The queue is never shared between different snapshots or rank policies.
    pub fn admit(&mut self, domain: Domain<N>) -> Result<(usize, bool), &'static str> {
        if let Some(ids) = self.by_owner.get(&domain.owner) {
            for &id in ids {
                if self.containment_checks == self.max_checks {
                    return Err("domain containment check allowance");
                }
                self.containment_checks += 1;
                if self.domains[id].contains(&domain) {
                    self.deduplicated += 1;
                    return Ok((id, false));
                }
            }
        }
        if self.domains.len() == self.max_domains {
            return Err("scheduled domain allowance");
        }
        let id = self.domains.len();
        self.domains
            .try_reserve(1)
            .map_err(|_| "domain allocation")?;
        self.by_owner.entry(domain.owner).or_default().push(id);
        self.domains.push(domain);
        Ok((id, true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn domain(rank: Option<u32>) -> Domain<2> {
        Domain {
            owner: [true, false],
            lower: vec![0, 0],
            upper: vec![None, None],
            rank,
        }
    }
    #[test]
    fn pending_inclusion_is_scheduling_reuse_not_completion() {
        let mut queue = Queue::new(3, 20);
        assert_eq!(queue.admit(domain(Some(10))), Ok((0, true)));
        let mut child = domain(Some(10));
        child.lower[0] = 3;
        assert_eq!(queue.admit(child), Ok((0, false)));
        assert_eq!(queue.next, 0);
        assert_eq!(queue.admit(domain(Some(11))), Ok((1, true)));
        assert_eq!(queue.admit(domain(None)), Ok((2, true)));
        assert_eq!(queue.admit(domain(Some(12))), Ok((2, false)));
    }
    #[test]
    fn literal_owner_and_unbounded_tail_are_not_approximated() {
        let mut queue = Queue::new(3, 20);
        let mut finite = domain(Some(10));
        finite.upper[0] = Some(u64::MAX);
        assert_eq!(queue.admit(finite), Ok((0, true)));
        assert_eq!(queue.admit(domain(Some(10))), Ok((1, true)));
        let mut other = domain(Some(10));
        other.owner = [false, true];
        assert_eq!(queue.admit(other), Ok((2, true)));
    }
    #[test]
    fn resource_failures_do_not_schedule_or_drop_work() {
        let mut queue = Queue::new(1, 1);
        assert!(queue.admit(domain(Some(10))).is_ok());
        assert_eq!(
            queue.admit(domain(Some(11))),
            Err("scheduled domain allowance")
        );
        assert_eq!(
            queue.admit(domain(Some(10))),
            Err("domain containment check allowance")
        );
        assert_eq!(queue.domains.len(), 1);
        assert_eq!(queue.next, 0);
    }
}
