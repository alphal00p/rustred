//! Exercise dynamic admission in a separately compiled application host.
use rustred_app::{AppErrorKind, FamilySolveRequest, family_solve};

const TADPOLE: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "user_supplied_tadpole"
loop_momenta = ["k1"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "D1"
expression = "k1^2-m"
[target]
powers = [1]
numerator = "1"
"#;

#[test]
fn omitted_campaign_arity_returns_an_input_error_instead_of_panicking() {
    let result = family_solve(FamilySolveRequest::new(TADPOLE));
    if rustred::compiled_runtime_arities().contains(&1) {
        let result = result.unwrap();
        assert_eq!(result.arity, 1);
        assert_eq!(result.family_name, "user_supplied_tadpole");
    } else {
        let error = result.unwrap_err();
        assert_eq!(error.kind(), AppErrorKind::Input);
        assert!(error.to_string().contains("not compiled"));
    }
}
