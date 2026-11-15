//! The order in which seeds are offered for dropping (spec data, interfaces/seedmin.md 4).

use super::policy::SeedMinPolicy;
use super::{Offset, add};

/// L1 norm.
fn l1(o: &[i32]) -> u64 {
    o.iter().map(|x| x.unsigned_abs() as u64).sum()
}

/// The drop order of `list` (indices into `list`): `distance` = descending L1 distance (of the centre-relative offset,
/// or of `centre + o` with `distance_from: key`), ties: later in the list first; `outermost_first` = reverse list
/// order. Deterministic.
pub fn drop_order(list: &[Offset], centre: &[i32], policy: &SeedMinPolicy) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..list.len()).collect();
    match policy.order.as_str() {
        "outermost_first" => idx.reverse(),
        _ => {
            let dist = |i: usize| -> u64 {
                if policy.distance_from == "key" {
                    l1(&add(centre, &list[i]))
                } else {
                    l1(&list[i])
                }
            };
            idx.sort_by(|&a, &b| dist(b).cmp(&dist(a)).then(b.cmp(&a)));
        }
    }
    idx
}

/// The start list in the policy's ROW ORDER (TRICKS 17 (2)): `given` keeps it; `distance_asc` / `distance_desc` sort
/// stably by the L1 distance (per `distance_from`).
pub fn row_ordered(list: &[Offset], centre: &[i32], policy: &SeedMinPolicy) -> Vec<Offset> {
    let dist = |o: &Offset| -> u64 {
        if policy.distance_from == "key" {
            l1(&add(centre, o))
        } else {
            l1(o)
        }
    };
    let mut v = list.to_vec();
    match policy.row_order.as_str() {
        "distance_asc" => v.sort_by_key(dist),
        "distance_desc" => v.sort_by_key(|o| std::cmp::Reverse(dist(o))),
        _ => {}
    }
    v
}

/// `list` restricted to the offsets in `keep` (list order kept).
pub fn restrict(list: &[Offset], keep: &[Offset]) -> Vec<Offset> {
    let set: std::collections::BTreeSet<&Offset> = keep.iter().collect();
    list.iter().filter(|o| set.contains(o)).cloned().collect()
}

/// Union of lists in the order of `base` first (offsets of `base` present in any part), then offsets absent from
/// `base` in part order (first occurrence wins).
pub fn union_in_order(base: &[Offset], parts: &[Vec<Offset>]) -> Vec<Offset> {
    let all: std::collections::BTreeSet<&Offset> = parts.iter().flatten().collect();
    let mut out: Vec<Offset> = base.iter().filter(|o| all.contains(o)).cloned().collect();
    let mut seen: std::collections::BTreeSet<Offset> = out.iter().cloned().collect();
    for p in parts {
        for o in p {
            if seen.insert(o.clone()) {
                out.push(o.clone());
            }
        }
    }
    out
}

/// `list` without `o`.
pub fn without(list: &[Offset], o: &[i32]) -> Vec<Offset> {
    list.iter().filter(|x| x.as_slice() != o).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_order_is_outermost_first_with_late_ties_first() {
        let p = SeedMinPolicy::default();
        let list = vec![
            vec![0, 0],
            vec![1, 0],
            vec![0, -1],
            vec![1, -1],
            vec![-1, 0],
        ];
        // distances 0,1,1,2,1 -> [3] then the 1s latest first [4,2,1] then [0]
        assert_eq!(drop_order(&list, &[0, 0], &p), vec![3, 4, 2, 1, 0]);
        let mut q = p.clone();
        q.order = "outermost_first".into();
        assert_eq!(drop_order(&list, &[0, 0], &q), vec![4, 3, 2, 1, 0]);
        // distance from the key: centre [-1, 0] shifts the distances
        let mut k = p.clone();
        k.distance_from = "key".into();
        // key-relative: [-1,0]=1, [0,0]=0, [-1,-1]=2, [0,-1]=1, [-2,0]=2
        assert_eq!(drop_order(&list, &[-1, 0], &k), vec![4, 2, 3, 0, 1]);
        assert_eq!(
            union_in_order(&list, &[vec![vec![1, -1], vec![5, 5]], vec![vec![0, 0]]]),
            vec![vec![0, 0], vec![1, -1], vec![5, 5]]
        );
        assert_eq!(
            restrict(&list, &[vec![-1, 0], vec![1, 0]]),
            vec![vec![1, 0], vec![-1, 0]]
        );
    }
}
