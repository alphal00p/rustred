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
    two_loop_pm("spired-fam1-11", false)
}

pub fn fam1_12() -> Result<IntegralFamily> {
    two_loop_pm("spired-fam1-12", true)
}

fn two_loop_pm(name: &str, second_cut_uses_u2: bool) -> Result<IntegralFamily> {
    // The first four linear denominators are supplied by pm_family. The
    // quadratics remain in the reference's original order, including the
    // three-momentum denominator before the two shifted single-loop squares.
    pm_family(
        name,
        [false, second_cut_uses_u2],
        [
            (0, [-1, 0, 0, 0, 0, 0, 0, 0, 0]),
            (0, [0, 0, -1, 0, 0, 0, 0, 0, 0]),
            (1, [-1, -2, -1, 2, 0, 0, 2, 0, 0]),
            (1, [-1, 0, 0, 2, 0, 0, 0, 0, 0]),
            (1, [0, 0, -1, 0, 0, 0, 2, 0, 0]),
        ],
    )
}

pub fn fam1_111() -> Result<IntegralFamily> {
    three_loop_pm("spired-fam1-111", false)
}

pub fn fam1_112() -> Result<IntegralFamily> {
    three_loop_pm("spired-fam1-112", true)
}

fn three_loop_pm(name: &str, third_cut_uses_u2: bool) -> Result<IntegralFamily> {
    // Coordinates: k1²,k1.k2,k1.k3,k2²,k2.k3,k3², followed by
    // k_i.(q,u1,u2). The order is squares, q-shifted squares, cyclic differences.
    pm_family(
        name,
        [false, false, third_cut_uses_u2],
        [
            (0, [-1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            (0, [0, 0, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            (0, [0, 0, 0, 0, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            (1, [-1, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0]),
            (1, [0, 0, 0, -1, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0]),
            (1, [0, 0, 0, 0, 0, -1, 0, 0, 0, 0, 0, 0, 2, 0, 0]),
            (0, [-1, 2, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            (0, [0, 0, 0, -1, 2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            (0, [-1, 0, 2, 0, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        ],
    )
}

/// Shared PM input kinematics. This helper only assembles example data;
/// cut elimination and all scalar algebra remain in RustRed/Symbolica.
fn pm_family<const L: usize, const N: usize>(
    name: &str,
    cut_uses_u2: [bool; L],
    quadratics: impl IntoIterator<Item = (i64, [i64; N])>,
) -> Result<IntegralFamily> {
    let c = CoefficientContext::try_new(["d", "x"])?;
    let x = c.parameter("x").unwrap();
    let gamma = &(&c.one() + &(&x * &x)) / &(&c.integer(2) * &x);
    let mut denominators = Vec::with_capacity(N);
    // Cut velocities first, then their complementary velocities. The context
    // coordinate order is loop-loop followed by loop-external products.
    let loop_products = L * (L + 1) / 2;
    for complementary in [false, true] {
        for (loop_index, uses_u2) in cut_uses_u2.iter().enumerate() {
            let external = if *uses_u2 ^ complementary { 2 } else { 1 };
            let mut row = vec![c.zero(); N];
            row[loop_products + loop_index * 3 + external] = c.one();
            denominators.push(AffineDenominator::new(c.zero(), row));
        }
    }
    denominators.extend(quadratics.into_iter().map(|(constant, row)| {
        AffineDenominator::new(
            c.integer(constant),
            row.into_iter().map(|value| c.integer(value)).collect(),
        )
    }));
    Ok(IntegralFamily::new(
        name,
        (1..=L).map(|index| format!("k{index}")).collect(),
        vec!["q".into(), "u1".into(), "u2".into()],
        c.clone(),
        c.parameter("d").unwrap(),
        denominators,
        vec![
            vec![c.integer(-1), c.zero(), c.zero()],
            vec![c.zero(), c.one(), gamma.clone()],
            vec![c.zero(), gamma, c.one()],
        ],
        vec![c.zero(); N],
    )?)
}

/// Two-loop boundary fixture with no removed cuts and a noninteger D0 power.
pub fn bc4pm_rad1() -> Result<IntegralFamily> {
    let c = CoefficientContext::try_new(["d", "ep2"])?;
    // Coordinates: k1²,k1.k2,k2²,k1.q,k1.n,k2.q,k2.n.
    let data = [
        (0, [0, 0, 0, 0, 1, 0, 0]),
        (0, [0, 0, 0, 0, 0, 0, 1]),
        (0, [1, 0, 0, 0, 0, 0, 0]),
        (0, [0, 0, 1, 0, 0, 0, 0]),
        (1, [1, 0, 0, -2, 0, 0, 0]),
        (1, [0, 0, 1, 0, 0, -2, 0]),
        (0, [1, -2, 1, 0, 0, 0, 0]),
    ];
    let denominators = data
        .into_iter()
        .map(|(constant, row)| {
            AffineDenominator::new(
                c.integer(constant),
                row.into_iter().map(|v| c.integer(v)).collect(),
            )
        })
        .collect();
    let mut shifts = vec![c.zero(); 7];
    shifts[0] = c.parameter("ep2").unwrap();
    Ok(IntegralFamily::new(
        "spired-bc4PMRad1",
        vec!["k1".into(), "k2".into()],
        vec!["q".into(), "n".into()],
        c.clone(),
        c.parameter("d").unwrap(),
        denominators,
        vec![vec![c.one(), c.zero()], vec![c.zero(), c.one()]],
        shifts,
    )?)
}

/// Multiscale two-loop fixture; s denotes the unspecialized invariant p².
pub fn fam_cosmo() -> Result<IntegralFamily> {
    // C++ prints its explicit scalar slots first and dot[p,p] afterward.
    // Naming that free invariant s preserves the same coefficient priority.
    let c = CoefficientContext::try_new(["d", "M1", "M2", "M3", "s"])?;
    let s = c.parameter("s").unwrap();
    let m1 = c.parameter("M1").unwrap();
    let m2 = c.parameter("M2").unwrap();
    let m3 = c.parameter("M3").unwrap();
    // Coordinates: k1²,k1.k2,k2²,k1.p,k2.p. The local names M3/M5
    // in the C++ source mean its second/third scalar slots, not M3/M5 inputs.
    let data = [
        (m1, [1, 0, 0, 0, 0]),
        (s.clone(), [1, 0, 0, 2, 0]),
        (&s + &m2, [0, 0, 1, 0, 2]),
        (c.zero(), [0, 0, 1, 0, 0]),
        (m3, [1, -2, 1, 0, 0]),
    ];
    let denominators = data
        .into_iter()
        .map(|(constant, row)| {
            AffineDenominator::new(constant, row.into_iter().map(|v| c.integer(v)).collect())
        })
        .collect();
    Ok(IntegralFamily::new(
        "spired-fam-cosmo",
        vec!["k1".into(), "k2".into()],
        vec!["p".into()],
        c.clone(),
        c.parameter("d").unwrap(),
        denominators,
        vec![vec![s]],
        vec![c.zero(); 5],
    )?)
}

#[cfg(test)]
#[path = "spired_families/tests.rs"]
mod tests;
