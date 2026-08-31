use crate::family::IntegralKey;
use crate::foundry::artifact::three_loop::derive_k6_terminal_authority;
use crate::sector::Mask;

#[test]
fn sealed_k3_times_k1_factorization_compiles_both_parent_terminals() {
    let authority = derive_k6_terminal_authority().unwrap();
    let embeddings = authority.factorization_rules()[0].master_embeddings();
    assert_eq!(embeddings.len(), 2);
    assert_eq!(
        embeddings
            .iter()
            .map(|embedding| (
                embedding.raw_parent_master().powers(),
                embedding.parent_terminal().powers(),
            ))
            .collect::<Vec<_>>(),
        [
            (&[0, 0, 1, 0, 1, 1][..], &[0, 0, 1, 0, 1, 1][..]),
            (&[0, 0, 1, 1, 1, 1][..], &[0, 0, 1, 1, 1, 1][..]),
        ]
    );
    assert!(embeddings.iter().all(|embedding| {
        authority
            .parent_terminals()
            .contains(embedding.parent_terminal())
    }));
}

#[test]
fn sealed_k1_cubed_factorizations_own_both_spanning_tree_orbits() {
    let authority = derive_k6_terminal_authority().unwrap();
    for (rule_ordinal, sector, target) in [
        (1, [0, 0, 1, 1, 0, 1], [0, 0, 2, 3, 0, 4]),
        (2, [0, 0, 1, 0, 1, 1], [0, 0, 2, 0, 3, 4]),
    ] {
        let factorization = &authority.factorization_rules()[rule_ordinal];
        assert_eq!(
            factorization.application_domain().sector(),
            &Mask::try_from_indices(&sector).unwrap()
        );
        assert_eq!(factorization.master_embeddings().len(), 1);
        assert_eq!(
            factorization.master_embeddings()[0]
                .raw_parent_master()
                .powers(),
            sector
        );
        assert_eq!(
            factorization.master_embeddings()[0]
                .parent_terminal()
                .powers(),
            sector
        );
        assert!(
            factorization
                .application_domain()
                .bounds()
                .iter()
                .zip(target)
                .all(|(&bounds, power)| bounds.contains(power))
        );
        assert!(
            authority
                .parent_terminals()
                .contains(&IntegralKey::try_new(sector).unwrap())
        );
    }
}
