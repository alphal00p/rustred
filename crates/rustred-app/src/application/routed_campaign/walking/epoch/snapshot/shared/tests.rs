use super::*;

fn filled(len: usize) -> Pages<u32> {
    let mut pages = Pages::default();
    for value in 0..len as u32 {
        pages
            .try_push(value, &mut Copies::default(), &mut || Ok(()))
            .unwrap();
    }
    pages
}

fn values(pages: &Pages<u32>) -> Vec<u32> {
    let mut all = Vec::new();
    pages
        .for_each_page(|page| {
            assert!(!page.is_empty() && page.len() <= PAGE);
            all.extend_from_slice(page);
            Ok(())
        })
        .unwrap();
    assert_eq!(all.len(), pages.len());
    for (at, value) in all.iter().enumerate() {
        assert_eq!(pages.get(at), Some(value));
    }
    assert_eq!(pages.get(pages.len()), None);
    all
}

fn page_pointer(pages: &Pages<u32>, index: usize) -> *const Node<u32> {
    let mut node = pages.root.as_deref().unwrap();
    let mut height = pages.height;
    while let Node::Branch(children) = node {
        let shift = PAGE_BITS + BRANCH_BITS * (height - 1);
        node = &children[(index >> shift) & (BRANCH - 1)];
        height -= 1;
    }
    node
}

#[test]
fn pages_cross_leaf_and_two_branch_boundaries_in_order() {
    for len in [
        0,
        1,
        PAGE - 1,
        PAGE,
        PAGE + 1,
        PAGE * BRANCH,
        PAGE * BRANCH + 1,
    ] {
        let pages = filled(len);
        assert_eq!(values(&pages), (0..len as u32).collect::<Vec<_>>());
    }
    let pages = filled(PAGE * BRANCH * BRANCH + 1);
    assert_eq!(pages.height, 3);
    assert_eq!(values(&pages), (0..pages.len() as u32).collect::<Vec<_>>());
}

#[test]
fn updates_copy_only_changed_paths_and_keep_held_roots_exact() {
    let original = filled(PAGE * BRANCH + 5);
    let mut changed = original.clone();
    let mut work = Copies::default();
    changed.try_set(3, 900, &mut work, &mut || Ok(())).unwrap();
    assert_eq!(work.nodes, 3);
    assert_eq!(work.values, PAGE);
    assert_ne!(page_pointer(&original, 3), page_pointer(&changed, 3));
    assert_eq!(page_pointer(&original, PAGE), page_pointer(&changed, PAGE));
    assert_eq!(
        page_pointer(&original, PAGE * BRANCH),
        page_pointer(&changed, PAGE * BRANCH)
    );
    let once = work;
    changed.try_set(4, 901, &mut work, &mut || Ok(())).unwrap();
    assert_eq!(work, once, "a private changed page is not recopied");
    changed.try_push(902, &mut work, &mut || Ok(())).unwrap();
    assert_eq!(original.get(3), Some(&3));
    assert_eq!(original.get(4), Some(&4));
    assert_eq!(original.len(), PAGE * BRANCH + 5);
    assert_eq!(changed.get(3), Some(&900));
    assert_eq!(changed.get(4), Some(&901));
    assert_eq!(changed.get(original.len()), Some(&902));
}

#[test]
fn every_fallible_changed_path_checkpoint_keeps_logical_authority() {
    for len in [PAGE, PAGE + 1, PAGE * BRANCH, PAGE * BRANCH + 3] {
        let original = filled(len);
        for append in [false, true] {
            let mut observed_failure = false;
            let mut observed_success = false;
            for fail_at in 0..10 {
                let mut changed = original.clone();
                let before = values(&changed);
                let mut calls = 0;
                let mut checkpoint = || {
                    let current = calls;
                    calls += 1;
                    if current == fail_at {
                        Err("injected preparation refusal")
                    } else {
                        Ok(())
                    }
                };
                let result = if append {
                    changed.try_push(777, &mut Copies::default(), &mut checkpoint)
                } else {
                    changed.try_set(0, 777, &mut Copies::default(), &mut checkpoint)
                };
                assert_eq!(values(&original), before);
                if result.is_err() {
                    observed_failure = true;
                    assert_eq!(values(&changed), before);
                } else {
                    observed_success = true;
                    let mut expected = before;
                    if append {
                        expected.push(777);
                    } else {
                        expected[0] = 777;
                    }
                    assert_eq!(values(&changed), expected);
                }
            }
            assert!(observed_failure && observed_success);
        }
    }
}

#[test]
fn out_of_range_update_and_page_visitor_refusal_are_read_only() {
    let mut pages = filled(PAGE * 2 + 1);
    let before = values(&pages);
    assert_eq!(
        pages.try_set(pages.len(), 9, &mut Copies::default(), &mut || Ok(())),
        Err("snapshot shared page update range")
    );
    let mut calls = 0;
    assert_eq!(
        pages.for_each_page(|_| {
            calls += 1;
            if calls == 2 { Err("stop") } else { Ok(()) }
        }),
        Err("stop")
    );
    assert_eq!(calls, 2);
    assert_eq!(values(&pages), before);
}
