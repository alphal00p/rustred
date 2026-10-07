//! These macro calls compile outside the library, as a direct solver host would.
mod downstream {
    pub fn callback<const N: usize>(value: &str) -> (usize, &str) {
        (N, value)
    }
}

#[test]
fn explicit_caller_registry_is_independent_and_evaluates_only_one_arm() {
    use downstream::callback;
    assert_eq!(
        rustred::dispatch_arity!(17, [13, 17], callback("direct"), n => (n, "unsupported")),
        (17, "direct")
    );
    assert_eq!(
        rustred::dispatch_arity!(9, [13, 17], callback("direct"), n => (n, "unsupported")),
        (9, "unsupported")
    );
    fn owned<const N: usize>(value: String) -> (usize, String) {
        (N, value)
    }
    let text = String::from("one move");
    assert_eq!(
        rustred::dispatch_arity!(13, [13, 17], owned(text), n => (n, String::new())),
        (13, String::from("one move"))
    );
}

#[test]
fn compiled_registry_and_default_dispatch_agree() {
    use downstream::callback;
    let registry = rustred::compiled_runtime_arities();
    assert!(!registry.is_empty() && registry[0] > 0);
    assert!(registry.windows(2).all(|pair| pair[0] < pair[1]));
    for &arity in registry {
        let mut evaluations = 0;
        let found = rustred::dispatch_arity!({ evaluations += 1; arity }, callback("compiled"), n => (n, "unsupported"));
        assert_eq!(found, (arity, "compiled"));
        assert_eq!(evaluations, 1);
    }
    assert_eq!(
        rustred::dispatch_arity!(0, callback("compiled"), n => (n, "unsupported")),
        (0, "unsupported")
    );
}

#[test]
fn capacity_dispatch_uses_smallest_bucket_and_keeps_supported_arities() {
    use downstream::callback;
    let capacities = rustred::compiled_runtime_capacities();
    for &arity in rustred::compiled_runtime_arities() {
        let expected = capacities
            .iter()
            .copied()
            .find(|capacity| *capacity >= arity)
            .unwrap();
        assert_eq!(
            rustred::dispatch_solver_capacity!(arity, callback("capacity"), n => (n,"unsupported")),
            (expected, "capacity")
        );
    }
    assert_eq!(
        rustred::dispatch_solver_capacity!(0, callback("capacity"), n => (n,"unsupported")),
        (0, "unsupported")
    );
    #[cfg(feature = "capacity-dispatch")]
    {
        let expected = rustred::compiled_runtime_arities()
            .iter()
            .map(|&arity| match arity {
                1..=4 => 4,
                5..=8 => 8,
                9..=16 => 16,
                other => other,
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(capacities, expected.into_iter().collect::<Vec<_>>());
    }
    for arity in 1..=16 {
        if !rustred::compiled_runtime_arities().contains(&arity) {
            assert_eq!(
                rustred::dispatch_solver_capacity!(arity, callback("capacity"), n => (n, "unsupported")),
                (arity, "unsupported")
            );
        }
    }
    #[cfg(not(feature = "capacity-dispatch"))]
    assert_eq!(capacities, rustred::compiled_runtime_arities());
}

#[test]
fn campaign_macro_uses_the_selected_registry_and_its_existing_maximum() {
    macro_rules! values { ($($n:literal),*) => { &[$($n as usize),*] }; }
    let actual: &[usize] = rustred::with_app_runtime_arities!(values);
    let expected = rustred::compiled_runtime_arities()
        .iter()
        .copied()
        .filter(|&arity| arity <= 16)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}
