use super::*;
use rustred::family::IntegralKey;

fn key(n: i64) -> ApplicationCacheKey {
    ApplicationCacheKey::route(&IntegralKey::try_new([n]).unwrap())
}

#[test]
fn exact_slot_stage_owner_and_powers_decide_membership() {
    let mut index = KeyIndex::default();
    let sparse = [key(0), key(10)];
    let dense = [key(0), key(5), key(10)];
    index.retain((0, 0), sparse.iter(), usize::MAX).unwrap();
    index.retain((1, 0), dense.iter(), usize::MAX).unwrap();
    assert_eq!(index.contains((0, 0), &key(5)), Some(false));
    assert_eq!(index.contains((1, 0), &key(5)), Some(true));
    assert_eq!(index.contains((0, 1), &key(5)), None);
    let integral = IntegralKey::try_new([5]).unwrap();
    assert_eq!(
        index.contains((1, 0), &ApplicationCacheKey::guarded_root(&integral)),
        Some(false)
    );
    assert_eq!(
        index.contains((1, 0), &ApplicationCacheKey::apply(vec![true], &integral)),
        Some(false)
    );
    let applied = [ApplicationCacheKey::apply(vec![false], &integral)];
    index.retain((2, 0), applied.iter(), usize::MAX).unwrap();
    assert_eq!(index.contains((2, 0), &applied[0]), Some(true));
    assert_eq!(
        index.contains((2, 0), &ApplicationCacheKey::apply(vec![true], &integral)),
        Some(false)
    );
}

#[test]
fn optional_admission_counts_capacity_and_keeps_existing_entries_on_refusal() {
    let keys = [key(0), key(10)];
    let mut index = KeyIndex::default();
    index.retain((0, 0), keys.iter(), 0).unwrap();
    assert_eq!(index.owned_bytes(), 0);
    assert_eq!(index.contains((0, 0), &keys[0]), None);
    let allowance = size_of::<Entry>()
        + keys.len() * size_of::<ApplicationCacheKey>()
        + keys
            .iter()
            .map(ApplicationCacheKey::owned_heap_bytes)
            .sum::<usize>();
    index.retain((0, 0), keys.iter(), allowance).unwrap();
    assert_eq!(index.contains((0, 0), &keys[0]), Some(true));
    assert!(index.owned_bytes() <= allowance);
    let before = index.owned_bytes();
    index.retain((0, 1), keys.iter(), allowance).unwrap();
    assert_eq!(index.owned_bytes(), before);
    assert_eq!(index.contains((0, 1), &keys[0]), None);
    index.retain((0, 0), keys.iter(), 0).unwrap();
    assert_eq!(index.owned_bytes(), before);
}
