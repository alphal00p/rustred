use super::*;
use crate::application::routed_campaign::walking::queue::{Domain, Phase, Queue};
use std::num::NonZeroUsize;
use std::sync::atomic::AtomicBool;

fn policy() -> SchedulingPolicy {
    SchedulingPolicy::TransferUnreserved {
        lookahead: NonZeroUsize::MIN,
    }
}
fn domain(lower: u64, upper: Option<u64>, rank: Option<u32>) -> Domain<2> {
    Domain {
        phase: Phase::Apply,
        owner: [true, false],
        lower: vec![lower, 0],
        upper: vec![upper, Some(4)],
        rank,
        powers: Default::default(),
    }
}
fn queue(max_domains: usize) -> Queue<2> {
    let mut q = Queue::with_policy(max_domains, None, policy()).unwrap();
    let mut reserved = domain(0, Some(0), Some(0));
    reserved.phase = Phase::Route;
    q.admit(reserved).unwrap();
    q
}

#[test]
fn exact_native_semantic_retirement_installs_forward_responsibility_and_keeps_raw_id() {
    let mut q = queue(8);
    let narrow = domain(2, Some(6), Some(100));
    assert_eq!(q.admit(narrow.clone()), Ok((1, true)));
    let mut broad = domain(0, None, Some(5));
    broad.upper[1] = None;
    assert_eq!(q.admit(broad), Ok((2, true)));
    assert_eq!(q.containment_semantic_retirements, 1);
    assert_eq!(q.delegation.as_ref().unwrap().delegated_to(1), Some(2));
    assert_eq!(q.admit(narrow.clone()), Ok((1, false)));
    assert_eq!(q.domains[1].as_ref(), &narrow);
    assert_eq!(q.delegation.as_ref().unwrap().len(), q.domains.len());
    assert_eq!(q.next, 0);
}

#[test]
fn phase_owner_and_reserved_obligations_never_cross_transfer() {
    let mut q = queue(8);
    let narrow = domain(2, Some(6), Some(100));
    q.admit(narrow).unwrap();
    let mut other = domain(0, None, None);
    other.owner = [false, true];
    q.admit(other).unwrap();
    let mut route = domain(0, None, None);
    route.phase = Phase::Route;
    q.admit(route).unwrap();
    assert_eq!(q.delegation.as_ref().unwrap().transfer_count(), 0);
    assert_eq!(q.delegation.as_ref().unwrap().delegated_to(0), None);
}

#[test]
fn failed_counter_preflight_never_admits_ledger_or_installs_alias() {
    let mut q = queue(8);
    q.admit(domain(2, Some(6), Some(100))).unwrap();
    let before = q.delegation.as_ref().unwrap().resolve().unwrap();
    q.containment_semantic_retirements = usize::MAX;
    assert!(q.admit(domain(0, None, Some(5))).is_err());
    assert_eq!(q.domains.len(), 2);
    assert_eq!(q.delegation.as_ref().unwrap().resolve().unwrap(), before);
    assert_eq!(q.containment_retired_candidates, 0);
}

#[test]
fn domain_cap_failure_keeps_all_old_responsibilities() {
    let mut q = queue(2);
    q.admit(domain(2, Some(6), Some(100))).unwrap();
    let before = q.delegation.as_ref().unwrap().resolve().unwrap();
    assert_eq!(
        q.admit(domain(0, None, Some(5))),
        Err("scheduled domain allowance")
    );
    assert_eq!(q.delegation.as_ref().unwrap().resolve().unwrap(), before);
}

#[test]
fn prepared_commit_revalidates_retired_candidate_and_never_publishes_speculative_aliases() {
    let mut q = queue(8);
    q.admit(domain(2, Some(6), Some(100))).unwrap();
    let query = domain(3, Some(4), Some(3));
    let stopped = AtomicBool::new(false);
    let prepared = q.prepare_admission(query, &stopped);
    assert_eq!(q.delegation.as_ref().unwrap().transfer_count(), 0);
    q.admit(domain(0, None, Some(5))).unwrap();
    assert_eq!(q.delegation.as_ref().unwrap().delegated_to(1), Some(2));
    assert_eq!(q.admit_prepared(prepared), Ok((2, false)));
    assert_eq!(q.delegation.as_ref().unwrap().len(), 3);
}

#[test]
fn default_and_finite_cap_policies_do_not_allocate_a_ledger() {
    for cap in [None, Some(100)] {
        let q = Queue::<2>::with_policy(8, cap, SchedulingPolicy::InspectAll).unwrap();
        assert!(q.delegation.is_none());
    }
    assert!(Queue::<2>::with_policy(8, Some(100), policy()).is_err());
}
