//! Router tests on the three-loop K6 (tetrahedron) vacuum family with six
//! unit-mass lines: zero sectors of the identity router, and the census
//! router's routing of the six equivalent five-line sectors onto one
//! representative with its automorphism rows.

use std::sync::Arc;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};

use super::{CensusRouter, IdentityRouter, SectorRouter};

const N: usize = 6;

/// K6: `k1^2-1, k2^2-1, k3^2-1, (k1-k3)^2-1, (k1-k2)^2-1, (k2-k3)^2-1` over the
/// coordinates `k1.k1, k1.k2, k1.k3, k2.k2, k2.k3, k3.k3`.
fn k6() -> Arc<IntegralFamily> {
    const ROWS: [[i64; 6]; 6] = [
        [1, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0, 1],
        [1, 0, -2, 0, 0, 1],
        [1, -2, 0, 1, 0, 0],
        [0, 0, 0, 1, -2, 1],
    ];
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let denominators = ROWS
        .iter()
        .map(|row| {
            AffineDenominator::new(
                context.integer(-1),
                row.map(|value| context.integer(value)).to_vec(),
            )
        })
        .collect();
    Arc::new(
        IntegralFamily::new(
            "k6",
            ["k1", "k2", "k3"].map(String::from).to_vec(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); N],
        )
        .unwrap(),
    )
}

fn sector(bits: u64) -> [bool; N] {
    std::array::from_fn(|slot| bits >> slot & 1 == 1)
}

fn bits(sector: &[bool; N]) -> u64 {
    sector
        .iter()
        .enumerate()
        .filter(|(_, active)| **active)
        .map(|(slot, _)| 1 << slot)
        .sum()
}

#[test]
fn identity_router_keeps_nonzero_sectors_and_finds_zero_sectors() {
    let family = k6();
    let router = IdentityRouter::new(&family).unwrap();
    // Two one-loop tadpoles leave k3 unintegrated: scaleless, zero.
    assert!(router.is_zero_bits(0b000011).unwrap());
    // Three tadpoles and the full tetrahedron are not zero.
    assert!(!router.is_zero_bits(0b000111).unwrap());
    assert!(!router.is_zero_bits(0b111111).unwrap());
    for bits in [0b000111u64, 0b111111, 0b011111] {
        let sector = sector(bits);
        assert_eq!(
            SectorRouter::<N>::representative(&router, &sector),
            Some(sector)
        );
    }
    assert_eq!(
        SectorRouter::<N>::representative(&router, &sector(0b000011)),
        None
    );
    let n = [2, 1, 1, 1, 1, -1];
    let route = SectorRouter::<N>::route(&router, &n).unwrap();
    assert_eq!(route.len(), 1);
    assert_eq!(route[0].0, n);
    assert_eq!(route[0].1, family.coefficient_context().one());
    assert!(
        SectorRouter::<N>::automorphism_rows(&router, &n)
            .unwrap()
            .is_empty()
    );
    assert!(SectorRouter::<N>::is_identity(&router));
}

#[test]
fn census_router_routes_equivalent_sectors_onto_one_representative() {
    let family = k6();
    let router = CensusRouter::discover(family.clone(), &[true; N]).unwrap();
    // Deleting any one edge of the tetrahedron gives the same graph.
    let five_line: Vec<u64> = (0..N).map(|slot| 0b111111 ^ (1 << slot)).collect();
    let representatives: Vec<[bool; N]> = five_line
        .iter()
        .map(|&bits| SectorRouter::<N>::representative(&router, &sector(bits)).unwrap())
        .collect();
    assert!(representatives.windows(2).all(|pair| pair[0] == pair[1]));
    let representative = bits(&representatives[0]);
    assert!(five_line.contains(&representative));
    // Every corner integral of a five-line sector routes onto one integral of
    // the representative sector with coefficient one.
    let one = family.coefficient_context().one();
    for &bits_ in &five_line {
        let n: [i16; N] = std::array::from_fn(|slot| (bits_ >> slot & 1) as i16);
        let route = SectorRouter::<N>::route(&router, &n).unwrap();
        assert_eq!(route.len(), 1, "{n:?}");
        let (target, coefficient) = &route[0];
        let target_sector: [bool; N] = std::array::from_fn(|slot| target[slot] > 0);
        assert_eq!(bits(&target_sector), representative);
        assert_eq!(coefficient, &one);
    }
    // The top sector is its own class; its automorphisms give exact rows
    // `I - T(I)` at a non-symmetric integral, and none outside representatives.
    let top = [2, 1, 1, 1, 1, 1];
    assert_eq!(
        SectorRouter::<N>::representative(&router, &sector(0b111111)),
        Some(sector(0b111111))
    );
    let rows = SectorRouter::<N>::automorphism_rows(&router, &top).unwrap();
    assert!(!rows.is_empty());
    assert!(SectorRouter::<N>::automorphism_bound(&router, &sector(0b111111)) >= rows.len());
    let non_representative = five_line.iter().find(|&&b| b != representative).unwrap();
    let n: [i16; N] = std::array::from_fn(|slot| (non_representative >> slot & 1) as i16);
    assert!(
        SectorRouter::<N>::automorphism_rows(&router, &n)
            .unwrap()
            .is_empty()
    );
    // Zero sectors route to nothing.
    assert!(
        SectorRouter::<N>::route(&router, &[1, 1, 0, 0, 0, 0])
            .unwrap()
            .is_empty()
    );
    assert!(!SectorRouter::<N>::is_identity(&router));
    // The fingerprint binds family and routing policy, and is reproducible.
    let again = CensusRouter::discover(family.clone(), &[true; N]).unwrap();
    assert_eq!(
        SectorRouter::<N>::fingerprint(&router),
        SectorRouter::<N>::fingerprint(&again)
    );
    let identity = IdentityRouter::new(&family).unwrap();
    assert_ne!(
        SectorRouter::<N>::fingerprint(&router),
        SectorRouter::<N>::fingerprint(&identity)
    );
}
