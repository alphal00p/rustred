//! Lane sizegen: the generate allow-list carries the seed-pose rung's trigger (a Decimal), and nothing else changed shape.

use rustred_campaign_contract::spec::{ArgShape, JobKind};

#[test]
fn generate_allows_the_seed_pose_trigger_as_a_decimal() {
    let r = JobKind::Generate
        .allowed_args()
        .iter()
        .find(|r| r.flag == "--seed-pose-trigger")
        .expect("on the generate allow-list");
    assert_eq!(r.shape, ArgShape::Decimal);
    assert!(r.shape.check("20000").is_ok());
    assert!(r.shape.check("2e4").is_ok() || r.shape.check("20000.0").is_ok());
    assert!(r.shape.check("abc").is_err());
    for k in [JobKind::SolveRecipe, JobKind::Descent] {
        assert!(
            !k.allowed_args()
                .iter()
                .any(|r| r.flag == "--seed-pose-trigger"),
            "{k:?}"
        );
    }
}
