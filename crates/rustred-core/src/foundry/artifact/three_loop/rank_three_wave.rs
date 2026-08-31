//! Exact Stage-1 boundary for the first nonzero K6 sector wave.
//!
//! This fixture intentionally publishes nothing. The adjacent closure sweep
//! pins the currently available degree-one replay evidence (path: 9 exact / 4
//! guard-total / 10 uncovered boxes; star: 22 / 12 / 4). Those semantic
//! inputs are not pointer-paired executable owners with `FreshTaskEpoch`
//! authority, and their complements remain nonfinite. The only honest result
//! is therefore one atomic two-sector stop against the shared root snapshot.

use crate::family::IntegralKey;
use crate::foundry::completion::source_discovery::{
    StagedSectorClosureCoordinator, StagedSectorClosureError, StagedSectorClosureLimits,
    StagedSectorClosureOutcome, StagedSectorClosureStop,
};
use crate::foundry::completion::stratum::{
    ImmutableOwnerKind, ImmutableOwnerSnapshot, StratumRegistryLimits,
};
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain};

use super::{canonical_family, derive_k6_terminal_authority};

const ZERO: [i64; 6] = [0, 0, 0, 1, 1, 1];
const PATH: [i64; 6] = [0, 0, 1, 0, 1, 1];
const STAR: [i64; 6] = [0, 0, 1, 1, 0, 1];
const ROOT_OWNER_COUNT: usize = 32;
const EVENTUAL_WAVE_LAYER_COUNT: usize = 2;
const EVENTUAL_SUCCESSOR_OWNER_COUNT: usize = 34;
const PATH_ORBIT_SIZE: usize = 12;
const STAR_ORBIT_SIZE: usize = 4;

fn corner_domain(indices: [i64; 6]) -> SectorInteriorDomain {
    let sector = Mask::try_from_indices(&indices).unwrap();
    SectorInteriorDomain::try_new(
        sector.clone(),
        sector.active_bits().iter().map(|&active| {
            if active {
                InteriorBounds::new(1, 1)
            } else {
                InteriorBounds::new(0, 0)
            }
        }),
    )
    .unwrap()
}

#[test]
fn rank_three_frontier_is_one_atomic_nonfinite_wave_on_the_shared_root_authority() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = canonical_family().unwrap();
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())
            .unwrap();
    let registry = StratumRegistryLimits::default();
    let root =
        ImmutableOwnerSnapshot::try_from_terminal_authority(authority.clone(), registry).unwrap();
    assert_eq!(root.owner_count(), ROOT_OWNER_COUNT);
    assert_eq!(root.closed_layer_count(), 0);

    let parent = Mask::try_new([true; 6]).unwrap();
    let zero = corner_domain(ZERO);
    let zero_witness = root
        .owner_for(&parent, OrderingPolicy::default(), &zero)
        .expect("the rank-three zero orbit must already terminate at S0");
    assert_eq!(zero_witness.kind(), ImmutableOwnerKind::ZeroSector);
    assert!(root.verifies_witness(&parent, OrderingPolicy::default(), &zero, zero_witness,));

    let canonicalizer = authority.canonicalizer().unwrap();
    let path_orbit = canonicalizer
        .orbit(&IntegralKey::try_new(PATH).unwrap())
        .unwrap();
    let star_orbit = canonicalizer
        .orbit(&IntegralKey::try_new(STAR).unwrap())
        .unwrap();
    assert_eq!(path_orbit.orbit_size(), PATH_ORBIT_SIZE);
    assert_eq!(star_orbit.orbit_size(), STAR_ORBIT_SIZE);
    for image in path_orbit.images().iter().chain(star_orbit.images()) {
        assert!(
            root.authenticates_explicit_terminal(image.integral())
                .unwrap()
        );
    }

    let path = Mask::try_from_indices(&PATH).unwrap();
    let star = Mask::try_from_indices(&STAR).unwrap();
    let mut wave = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        root.clone(),
        [
            (star.clone(), OrderingPolicy::default()),
            (path.clone(), OrderingPolicy::default()),
        ],
        StagedSectorClosureLimits::default(),
    )
    .unwrap();
    assert_eq!(wave.sector_count(), EVENTUAL_WAVE_LAYER_COUNT);
    assert!(wave.predecessor_snapshot().same_authority_as(&root));

    // These scalar corners are already exact terminals of the retained S0
    // authority. They remove finite missing-point obligations but cannot turn
    // the presently unowned infinite complements into closure.
    assert!(
        authority
            .parent_terminals()
            .contains(&IntegralKey::try_new(PATH).unwrap())
    );
    assert!(
        authority
            .parent_terminals()
            .contains(&IntegralKey::try_new(STAR).unwrap())
    );
    assert!(
        wave.try_insert_terminal(
            &path,
            OrderingPolicy::default(),
            IntegralKey::try_new(PATH).unwrap(),
        )
        .unwrap()
    );
    assert!(
        wave.try_insert_terminal(
            &star,
            OrderingPolicy::default(),
            IntegralKey::try_new(STAR).unwrap(),
        )
        .unwrap()
    );
    assert_eq!(wave.owner_count(), 0);
    assert_eq!(wave.terminal_count(), 2);

    let StagedSectorClosureOutcome::Stopped(stops) = wave.try_finish().unwrap() else {
        panic!("rank-three semantic evidence cannot publish executable layers")
    };
    assert_eq!(stops.len(), EVENTUAL_WAVE_LAYER_COUNT);
    assert_eq!(stops[0].evidence().sector(), &path);
    assert_eq!(stops[1].evidence().sector(), &star);
    for stop in &*stops {
        let StagedSectorClosureStop::NonFinite(evidence) = stop else {
            panic!("an owner-free sector orthant must stop as nonfinite")
        };
        assert_eq!(evidence.owner_count(), 0);
        assert_eq!(evidence.terminal_count(), 1);
        assert_eq!(evidence.uncovered_box_count(), 1);
    }

    // Failure is transactional: no partial path or star layer exists. Once
    // both exact covers close, the same boundary will append two owners in one
    // wave and symmetry routing will expose 12 + 4 sector images.
    assert_eq!(root.owner_count(), ROOT_OWNER_COUNT);
    assert_eq!(root.closed_layer_count(), 0);
    assert_eq!(
        ROOT_OWNER_COUNT + EVENTUAL_WAVE_LAYER_COUNT,
        EVENTUAL_SUCCESSOR_OWNER_COUNT
    );
    assert_eq!(PATH_ORBIT_SIZE + STAR_ORBIT_SIZE, 16);
    assert!(root.try_verify(registry).unwrap());
}

#[test]
fn rank_three_wave_coordinate_limits_are_aggregate_not_per_sector() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = canonical_family().unwrap();
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())
            .unwrap();
    let root = ImmutableOwnerSnapshot::try_from_terminal_authority(
        authority,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let path = Mask::try_from_indices(&PATH).unwrap();
    let star = Mask::try_from_indices(&STAR).unwrap();

    let mut frontier_limits = StagedSectorClosureLimits::default();
    frontier_limits.max_frontier_coordinate_cells = 11;
    assert!(matches!(
        StagedSectorClosureCoordinator::try_new(
            generator.context(),
            root.clone(),
            [
                (path.clone(), OrderingPolicy::default()),
                (star.clone(), OrderingPolicy::default()),
            ],
            frontier_limits,
        ),
        Err(StagedSectorClosureError::ResourceLimit {
            resource: "staged sector-closure frontier coordinate cells",
            requested: 12,
            limit: 11,
        })
    ));

    let mut terminal_limits = StagedSectorClosureLimits::default();
    terminal_limits.max_staged_terminal_coordinate_cells = 11;
    let mut wave = StagedSectorClosureCoordinator::try_new(
        generator.context(),
        root.clone(),
        [
            (path.clone(), OrderingPolicy::default()),
            (star.clone(), OrderingPolicy::default()),
        ],
        terminal_limits,
    )
    .unwrap();
    assert!(
        wave.try_insert_terminal(
            &path,
            OrderingPolicy::default(),
            IntegralKey::try_new(PATH).unwrap(),
        )
        .unwrap()
    );
    assert!(matches!(
        wave.try_insert_terminal(
            &star,
            OrderingPolicy::default(),
            IntegralKey::try_new(STAR).unwrap(),
        ),
        Err(StagedSectorClosureError::ResourceLimit {
            resource: "staged sector-closure terminal coordinate cells",
            requested: 12,
            limit: 11,
        })
    ));
    assert_eq!(wave.terminal_count(), 1);
    assert_eq!(wave.terminal_coordinate_cells(), 6);
    assert_eq!(root.closed_layer_count(), 0);
}
