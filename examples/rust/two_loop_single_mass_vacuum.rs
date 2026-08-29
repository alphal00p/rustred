use std::error::Error;

use rustred::{
    identity::{ParametricIbpGenerator, ParametricRelation},
    input::{Compiler, Limits, LoweringLimits},
};

const FAMILY: &str = r#"
I(
  name(equal_mass_sunset),
  loops(k1,k2),
  externals(),
  parameters(d,m2),
  dimension(d),
  prop(D1,k1^2-m2,1),
  prop(D2,k2^2-m2,1),
  prop(D3,(k1+k2)^2-m2,1)
)
"#;

const EXPECTED_ROWS: [&str; 4] = [
    "ordinary-ibp:0:0",
    "ordinary-ibp:0:1",
    "ordinary-ibp:1:0",
    "ordinary-ibp:1:1",
];

fn main() -> Result<(), Box<dyn Error>> {
    let compiler = Compiler::new(Limits::default())?;
    let project = compiler.compile_compact(FAMILY, None)?;
    let lowered = project.into_lowered(LoweringLimits::default())?;
    let generator = ParametricIbpGenerator::try_new(lowered.family())?;

    // A prepared batch exposes independent stable ordinals. A larger caller
    // may schedule these rows itself; four tiny rows are clearest sequentially.
    let batch = generator.prepare_ordinary_ibp()?;
    assert_eq!(batch.len(), EXPECTED_ROWS.len());
    let rows = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect::<Vec<_>>();
    let relations = batch.complete(rows)?.into_relations();

    let actual_rows = relations
        .iter()
        .map(|relation| relation.row_id().stable_string())
        .collect::<Vec<_>>();
    assert_eq!(actual_rows, EXPECTED_ROWS);

    println!("# sum(coefficient * I(n + shift)) = 0");
    for relation in &relations {
        println!(
            "{}: {}",
            relation.row_id().stable_string(),
            equation(relation)
        );
    }
    Ok(())
}

fn equation(relation: &ParametricRelation) -> String {
    let terms = relation
        .terms()
        .iter()
        .map(|(shift, coefficient)| {
            format!(
                "({}) * {}",
                coefficient.to_expression(),
                shifted_integral(shift.values())
            )
        })
        .collect::<Vec<_>>();
    format!("{} = 0", terms.join(" + "))
}

fn shifted_integral(shift: &[i64]) -> String {
    let indices = shift
        .iter()
        .enumerate()
        .map(|(position, displacement)| match displacement {
            0 => format!("n{position}"),
            positive if *positive > 0 => format!("n{position}+{positive}"),
            negative => format!("n{position}{negative}"),
        })
        .collect::<Vec<_>>();
    format!("I({})", indices.join(","))
}
