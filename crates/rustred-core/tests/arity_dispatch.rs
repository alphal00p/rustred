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
