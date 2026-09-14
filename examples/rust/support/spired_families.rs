//! Supplied benchmark family data, deliberately outside the generic engine.
use rustred::algebra::CoefficientContext;
use rustred::family::{AffineDenominator, IntegralFamily};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub fn vacuum(momenta: &[&[i64]]) -> Result<IntegralFamily> {
    let loops = momenta[0].len();
    let coefficients = CoefficientContext::try_new(["d", "m"])?;
    let mass = coefficients.parameter("m").unwrap();
    let denominators = momenta
        .iter()
        .map(|momentum| {
            let row = (0..loops)
                .flat_map(|i| {
                    (i..loops).map(move |j| momentum[i] * momentum[j] * if i == j { 1 } else { 2 })
                })
                .map(|value| coefficients.integer(value))
                .collect();
            AffineDenominator::new(-mass.clone(), row)
        })
        .collect();
    Ok(IntegralFamily::new(
        "spired-vacuum-sector",
        (0..loops).map(|i| format!("k{i}")).collect(),
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        denominators,
        Vec::new(),
        vec![coefficients.zero(); momenta.len()],
    )?)
}

pub fn fam1_11() -> Result<IntegralFamily> {
    let c = CoefficientContext::try_new(["d", "x"])?;
    let x = c.parameter("x").unwrap();
    let gamma = &(&c.one() + &(&x * &x)) / &(&c.integer(2) * &x);
    let data = [
        (0, [0, 0, 0, 0, 1, 0, 0, 0, 0]),
        (0, [0, 0, 0, 0, 0, 0, 0, 1, 0]),
        (0, [0, 0, 0, 0, 0, 1, 0, 0, 0]),
        (0, [0, 0, 0, 0, 0, 0, 0, 0, 1]),
        (0, [-1, 0, 0, 0, 0, 0, 0, 0, 0]),
        (0, [0, 0, -1, 0, 0, 0, 0, 0, 0]),
        (1, [-1, -2, -1, 2, 0, 0, 2, 0, 0]),
        (1, [-1, 0, 0, 2, 0, 0, 0, 0, 0]),
        (1, [0, 0, -1, 0, 0, 0, 2, 0, 0]),
    ];
    let denominators = data
        .into_iter()
        .map(|(constant, row)| {
            AffineDenominator::new(
                c.integer(constant),
                row.into_iter().map(|value| c.integer(value)).collect(),
            )
        })
        .collect();
    Ok(IntegralFamily::new(
        "spired-fam1-11",
        vec!["k1".into(), "k2".into()],
        vec!["q".into(), "u1".into(), "u2".into()],
        c.clone(),
        c.parameter("d").unwrap(),
        denominators,
        vec![
            vec![c.integer(-1), c.zero(), c.zero()],
            vec![c.zero(), c.one(), gamma.clone()],
            vec![c.zero(), gamma, c.one()],
        ],
        vec![c.zero(); 9],
    )?)
}
