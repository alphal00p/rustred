//! Non-authoritative ordering-portfolio telemetry for the `K = 6` K4 family.
//!
//! This test-only module keeps coordinate-order proposals outside the foundry
//! scheduler and every evidence/owner boundary.  It proves the finite census
//! needed by a future modular portfolio, but deliberately cannot publish a
//! rule, a solved layer, or a closed artifact.  Symbolica's graph canonizer is
//! used only as an independent proposal/deduplication cross-check; the exact
//! authenticated [`Canonicalizer`] remains the symmetry authority.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use symbolica::graph::Graph;

use crate::sector::CoordinatePriority;
use crate::sector::symmetry::{
    CanonicalizationError, Canonicalizer, CoordinatePriorityActionLimits,
    CoordinatePriorityQuotient,
};

use super::{canonical_family, canonical_s4};

const ARITY: usize = 6;
const RAW_PRIORITY_COUNT: usize = 720;
const GROUP_ORDER: usize = 24;
const GLOBAL_PRIORITY_CLASS_COUNT: usize = 30;

const PATH: [bool; ARITY] = [false, false, true, false, true, true];
const STAR: [bool; ARITY] = [false, false, true, true, false, true];
const GENERIC_CHART: [usize; ARITY] = [1, 2, 3, 4, 5, 6];

const PATH_IMAGE_COUNT: usize = 12;
const PATH_ROUTE_MULTIPLICITY: usize = 2;
const STAR_IMAGE_COUNT: usize = 4;
const STAR_ROUTE_MULTIPLICITY: usize = 6;
const STRUCTURAL_IMAGE_COUNT: usize = PATH_IMAGE_COUNT + STAR_IMAGE_COUNT;
const DECORATED_IMAGE_COUNT: usize = GROUP_ORDER + GROUP_ORDER;
const STRUCTURAL_PARTITION_COUNT: usize = GLOBAL_PRIORITY_CLASS_COUNT * STRUCTURAL_IMAGE_COUNT;
const DECORATED_MODULAR_CASE_COUNT: usize = GLOBAL_PRIORITY_CLASS_COUNT * DECORATED_IMAGE_COUNT;

/// K4 edge endpoints in the exact denominator-slot convention of
/// `canonical_family`: `k1`, `k2`, `k3`, `k3-k1`, `k1-k2`, `k2-k3`.
const K4_EDGES: [(usize, usize); ARITY] = [(0, 1), (0, 2), (0, 3), (1, 3), (1, 2), (2, 3)];

/// The quotient promises lexicographically ordered exact representatives, so
/// pinning all 30 is a stable regression boundary rather than a hash snapshot.
const PINNED_GLOBAL_REPRESENTATIVES: [[usize; ARITY]; GLOBAL_PRIORITY_CLASS_COUNT] = [
    [0, 1, 2, 3, 4, 5],
    [0, 1, 2, 3, 5, 4],
    [0, 1, 2, 4, 3, 5],
    [0, 1, 2, 4, 5, 3],
    [0, 1, 2, 5, 3, 4],
    [0, 1, 2, 5, 4, 3],
    [0, 1, 3, 2, 4, 5],
    [0, 1, 3, 2, 5, 4],
    [0, 1, 3, 4, 2, 5],
    [0, 1, 3, 4, 5, 2],
    [0, 1, 3, 5, 2, 4],
    [0, 1, 3, 5, 4, 2],
    [0, 1, 4, 2, 3, 5],
    [0, 1, 4, 2, 5, 3],
    [0, 1, 4, 3, 2, 5],
    [0, 1, 4, 3, 5, 2],
    [0, 1, 4, 5, 2, 3],
    [0, 1, 4, 5, 3, 2],
    [0, 1, 5, 2, 3, 4],
    [0, 1, 5, 2, 4, 3],
    [0, 1, 5, 3, 2, 4],
    [0, 1, 5, 3, 4, 2],
    [0, 1, 5, 4, 2, 3],
    [0, 1, 5, 4, 3, 2],
    [0, 2, 3, 4, 5, 1],
    [0, 2, 3, 5, 4, 1],
    [0, 2, 4, 3, 5, 1],
    [0, 2, 4, 5, 3, 1],
    [0, 2, 5, 3, 4, 1],
    [0, 2, 5, 4, 3, 1],
];

#[derive(Clone, Copy, Debug)]
struct OrderingPortfolioLimits {
    priority_action: CoordinatePriorityActionLimits,
    max_group_routes: usize,
    max_structural_images: usize,
    max_structural_partitions: usize,
    max_decorated_cases: usize,
}

impl Default for OrderingPortfolioLimits {
    fn default() -> Self {
        Self {
            priority_action: CoordinatePriorityActionLimits::default(),
            max_group_routes: GROUP_ORDER,
            max_structural_images: STRUCTURAL_IMAGE_COUNT,
            max_structural_partitions: STRUCTURAL_PARTITION_COUNT,
            max_decorated_cases: DECORATED_MODULAR_CASE_COUNT,
        }
    }
}

#[derive(Debug)]
enum OrderingPortfolioError {
    Canonicalization(CanonicalizationError),
    WrongArity {
        expected: usize,
        actual: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for OrderingPortfolioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonicalization(error) => error.fmt(formatter),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "K4 ordering portfolio has arity {actual}, expected {expected}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "K4 ordering-portfolio {resource} overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "K4 ordering-portfolio {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for K4 ordering-portfolio {resource}"
            ),
            Self::Invariant { detail } => {
                write!(
                    formatter,
                    "K4 ordering-portfolio invariant failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for OrderingPortfolioError {}

impl From<CanonicalizationError> for OrderingPortfolioError {
    fn from(value: CanonicalizationError) -> Self {
        Self::Canonicalization(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DecoratedTask {
    sector: [bool; ARITY],
    chart: [usize; ARITY],
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StructuralRouteBucket {
    image: [bool; ARITY],
    routes: [usize; GROUP_ORDER],
    route_count: usize,
}

impl StructuralRouteBucket {
    fn new(image: [bool; ARITY], route: usize) -> Self {
        let mut routes = [usize::MAX; GROUP_ORDER];
        routes[0] = route;
        Self {
            image,
            routes,
            route_count: 1,
        }
    }

    fn try_push(&mut self, route: usize) -> Result<(), OrderingPortfolioError> {
        let target =
            self.routes
                .get_mut(self.route_count)
                .ok_or(OrderingPortfolioError::Invariant {
                    detail: "one structural route bucket exceeded the K4 group order",
                })?;
        *target = route;
        self.route_count += 1;
        Ok(())
    }

    fn routes(&self) -> &[usize] {
        &self.routes[..self.route_count]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SectorPortfolio {
    fixed_sector: [bool; ARITY],
    route_buckets: Vec<StructuralRouteBucket>,
    decorated_tasks: Vec<DecoratedTask>,
}

impl SectorPortfolio {
    fn try_new(
        canonicalizer: &Canonicalizer,
        fixed_sector: [bool; ARITY],
        limits: OrderingPortfolioLimits,
    ) -> Result<Self, OrderingPortfolioError> {
        admit_limit(
            "group routes",
            canonicalizer.group_order(),
            limits.max_group_routes,
        )?;

        let mut buckets: Vec<StructuralRouteBucket> = Vec::new();
        try_reserve_exact(
            &mut buckets,
            canonicalizer.group_order(),
            "structural route buckets",
        )?;
        let mut decorated_tasks = Vec::new();
        try_reserve_exact(
            &mut decorated_tasks,
            canonicalizer.group_order(),
            "decorated task images",
        )?;

        for (route, source_for_target) in canonicalizer.group_elements().enumerate() {
            let image = transport_fixed(&fixed_sector, source_for_target);
            if let Some(bucket) = buckets.iter_mut().find(|bucket| bucket.image == image) {
                bucket.try_push(route)?;
            } else {
                buckets.push(StructuralRouteBucket::new(image, route));
            }
            decorated_tasks.push(DecoratedTask {
                sector: image,
                chart: transport_fixed(&GENERIC_CHART, source_for_target),
            });
        }

        buckets.sort_by_key(|bucket| bucket.image);
        decorated_tasks.sort_unstable();
        decorated_tasks.dedup();
        if decorated_tasks.len() != canonicalizer.group_order() {
            return Err(OrderingPortfolioError::Invariant {
                detail: "generic chart did not break every structural stabilizer",
            });
        }

        let route_count = buckets
            .iter()
            .try_fold(0usize, |count, bucket| {
                count.checked_add(bucket.route_count)
            })
            .ok_or(OrderingPortfolioError::ResourceCountOverflow {
                resource: "structural route count",
            })?;
        if route_count != canonicalizer.group_order() {
            return Err(OrderingPortfolioError::Invariant {
                detail: "structural route buckets do not partition the group",
            });
        }

        Ok(Self {
            fixed_sector,
            route_buckets: buckets,
            decorated_tasks,
        })
    }

    fn stabilizer_routes(&self) -> impl Iterator<Item = usize> + '_ {
        self.route_buckets
            .iter()
            .find(|bucket| bucket.image == self.fixed_sector)
            .into_iter()
            .flat_map(|bucket| bucket.routes().iter().copied())
    }
}

#[derive(Debug)]
struct OrderingPortfolioCensus {
    quotient: CoordinatePriorityQuotient,
    path: SectorPortfolio,
    star: SectorPortfolio,
    structural_partition_count: usize,
    decorated_modular_case_count: usize,
}

impl OrderingPortfolioCensus {
    fn try_new(
        canonicalizer: &Canonicalizer,
        limits: OrderingPortfolioLimits,
    ) -> Result<Self, OrderingPortfolioError> {
        if canonicalizer.arity() != ARITY {
            return Err(OrderingPortfolioError::WrongArity {
                expected: ARITY,
                actual: canonicalizer.arity(),
            });
        }
        admit_limit(
            "group routes",
            canonicalizer.group_order(),
            limits.max_group_routes,
        )?;
        // The fixture cardinalities are known before any quotient or task
        // payload is materialized, so caller caps fail at the true boundary.
        admit_limit(
            "structural images",
            STRUCTURAL_IMAGE_COUNT,
            limits.max_structural_images,
        )?;
        admit_limit(
            "structural partitions",
            STRUCTURAL_PARTITION_COUNT,
            limits.max_structural_partitions,
        )?;
        admit_limit(
            "decorated modular cases",
            DECORATED_MODULAR_CASE_COUNT,
            limits.max_decorated_cases,
        )?;

        let quotient = canonicalizer.coordinate_priority_quotient(limits.priority_action)?;
        let path = SectorPortfolio::try_new(canonicalizer, PATH, limits)?;
        let star = SectorPortfolio::try_new(canonicalizer, STAR, limits)?;

        let structural_images = checked_sum(
            "structural images",
            path.route_buckets.len(),
            star.route_buckets.len(),
        )?;
        admit_limit(
            "structural images",
            structural_images,
            limits.max_structural_images,
        )?;
        let structural_partition_count = checked_product(
            "structural partitions",
            quotient.class_count(),
            structural_images,
        )?;
        admit_limit(
            "structural partitions",
            structural_partition_count,
            limits.max_structural_partitions,
        )?;

        let decorated_images = checked_sum(
            "decorated task images",
            path.decorated_tasks.len(),
            star.decorated_tasks.len(),
        )?;
        let decorated_modular_case_count = checked_product(
            "decorated modular cases",
            quotient.class_count(),
            decorated_images,
        )?;
        admit_limit(
            "decorated modular cases",
            decorated_modular_case_count,
            limits.max_decorated_cases,
        )?;

        Ok(Self {
            quotient,
            path,
            star,
            structural_partition_count,
            decorated_modular_case_count,
        })
    }
}

#[test]
fn k4_global_priority_quotient_is_exact_deterministic_and_pinned() {
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let limits = OrderingPortfolioLimits::default();
    let first = OrderingPortfolioCensus::try_new(&canonicalizer, limits).unwrap();
    let second = OrderingPortfolioCensus::try_new(&canonicalizer, limits).unwrap();

    assert_eq!(first.quotient.arity(), ARITY);
    assert_eq!(first.quotient.priority_count(), RAW_PRIORITY_COUNT);
    assert_eq!(first.quotient.group_order(), GROUP_ORDER);
    assert_eq!(first.quotient.class_count(), GLOBAL_PRIORITY_CLASS_COUNT);
    assert!(
        first
            .quotient
            .classes()
            .iter()
            .all(|class| class.orbit_size() == GROUP_ORDER)
    );
    let representatives = first
        .quotient
        .representatives()
        .map(|priority| priority.rank_by_slot().try_into().unwrap())
        .collect::<Vec<[usize; ARITY]>>();
    assert_eq!(representatives, PINNED_GLOBAL_REPRESENTATIVES);
    assert_eq!(first.quotient, second.quotient);
}

#[test]
fn path_and_star_routes_have_exact_structural_and_decorated_census() {
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let census =
        OrderingPortfolioCensus::try_new(&canonicalizer, OrderingPortfolioLimits::default())
            .unwrap();

    assert_eq!(census.path.route_buckets.len(), PATH_IMAGE_COUNT);
    assert!(
        census
            .path
            .route_buckets
            .iter()
            .all(|bucket| bucket.routes().len() == PATH_ROUTE_MULTIPLICITY)
    );
    assert_eq!(census.star.route_buckets.len(), STAR_IMAGE_COUNT);
    assert!(
        census
            .star
            .route_buckets
            .iter()
            .all(|bucket| bucket.routes().len() == STAR_ROUTE_MULTIPLICITY)
    );

    let path_stabilizer = census.path.stabilizer_routes().collect::<BTreeSet<_>>();
    let star_stabilizer = census.star.stabilizer_routes().collect::<BTreeSet<_>>();
    assert_eq!(path_stabilizer.len(), PATH_ROUTE_MULTIPLICITY);
    assert_eq!(star_stabilizer.len(), STAR_ROUTE_MULTIPLICITY);
    let joint = path_stabilizer
        .intersection(&star_stabilizer)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(joint.len(), 1);
    assert_eq!(
        canonicalizer.group_elements().nth(joint[0]).unwrap(),
        &[0, 1, 2, 3, 4, 5]
    );

    let path_fixed_classes = fixed_sector_priority_classes(
        &canonicalizer,
        &census.quotient,
        &path_stabilizer.iter().copied().collect::<Vec<_>>(),
        OrderingPortfolioLimits::default().priority_action,
    )
    .unwrap();
    assert_eq!(
        path_fixed_classes.len(),
        RAW_PRIORITY_COUNT / PATH_ROUTE_MULTIPLICITY
    );
    assert!(
        path_fixed_classes
            .values()
            .all(|&multiplicity| multiplicity == PATH_ROUTE_MULTIPLICITY)
    );
    let star_fixed_classes = fixed_sector_priority_classes(
        &canonicalizer,
        &census.quotient,
        &star_stabilizer.iter().copied().collect::<Vec<_>>(),
        OrderingPortfolioLimits::default().priority_action,
    )
    .unwrap();
    assert_eq!(
        star_fixed_classes.len(),
        RAW_PRIORITY_COUNT / STAR_ROUTE_MULTIPLICITY
    );
    assert!(
        star_fixed_classes
            .values()
            .all(|&multiplicity| multiplicity == STAR_ROUTE_MULTIPLICITY)
    );

    assert_eq!(census.path.decorated_tasks.len(), GROUP_ORDER);
    assert_eq!(census.star.decorated_tasks.len(), GROUP_ORDER);
    assert_eq!(
        census.structural_partition_count,
        GLOBAL_PRIORITY_CLASS_COUNT * (PATH_IMAGE_COUNT + STAR_IMAGE_COUNT)
    );
    assert_eq!(
        census.structural_partition_count,
        STRUCTURAL_PARTITION_COUNT
    );
    assert_eq!(
        census.decorated_modular_case_count,
        GLOBAL_PRIORITY_CLASS_COUNT * (GROUP_ORDER + GROUP_ORDER)
    );
    assert_eq!(
        census.decorated_modular_case_count,
        DECORATED_MODULAR_CASE_COUNT
    );
}

#[test]
fn inverse_transport_and_diagonal_coverage_are_complete() {
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let limits = OrderingPortfolioLimits::default();
    let census = OrderingPortfolioCensus::try_new(&canonicalizer, limits).unwrap();
    let inverse_routes = try_inverse_routes(&canonicalizer, limits).unwrap();

    for class in census.quotient.classes() {
        for priority in class.images() {
            for route in 0..canonicalizer.group_order() {
                let image = canonicalizer
                    .transport_coordinate_priority(priority, route, limits.priority_action)
                    .unwrap();
                let round_trip = canonicalizer
                    .transport_coordinate_priority(
                        &image,
                        inverse_routes[route],
                        limits.priority_action,
                    )
                    .unwrap();
                assert_eq!(&round_trip, priority);
            }
        }
    }

    for (fixed, portfolio) in [(PATH, &census.path), (STAR, &census.star)] {
        for (route, source_for_target) in canonicalizer.group_elements().enumerate() {
            let inverse = canonicalizer
                .group_elements()
                .nth(inverse_routes[route])
                .unwrap();
            assert_eq!(
                transport_fixed(&transport_fixed(&fixed, source_for_target), inverse,),
                fixed
            );
            assert_eq!(
                transport_fixed(&transport_fixed(&GENERIC_CHART, source_for_target), inverse,),
                GENERIC_CHART
            );
        }
        assert_diagonal_coverage(
            &canonicalizer,
            &census.quotient,
            portfolio,
            &inverse_routes,
            limits.priority_action,
        );
    }
}

#[test]
fn symbolica_graph_canonization_independently_matches_the_authenticated_quotient() {
    let uncolored = k4_graph([0; ARITY]);
    assert_eq!(
        uncolored.canonize().automorphism_group_size.to_string(),
        GROUP_ORDER.to_string()
    );

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let quotient = canonicalizer
        .coordinate_priority_quotient(CoordinatePriorityActionLimits::default())
        .unwrap();
    let mut graph_form_to_representative = BTreeMap::new();

    for class in quotient.classes() {
        let representative = class.canonical().rank_by_slot();
        let representative_form = k4_graph(representative.try_into().unwrap()).canonize();
        assert_eq!(representative_form.automorphism_group_size.to_string(), "1");
        for priority in class.images() {
            let graph_form = k4_graph(priority.rank_by_slot().try_into().unwrap()).canonize();
            assert_eq!(graph_form.automorphism_group_size.to_string(), "1");
            assert_eq!(graph_form.graph, representative_form.graph);
        }
        assert!(
            graph_form_to_representative
                .insert(
                    representative_form.graph,
                    representative.try_into().unwrap(),
                )
                .is_none(),
            "two authenticated priority classes collapsed to one colored graph form"
        );
    }

    assert_eq!(
        graph_form_to_representative.len(),
        GLOBAL_PRIORITY_CLASS_COUNT
    );
    assert_eq!(
        graph_form_to_representative
            .values()
            .copied()
            .collect::<BTreeSet<_>>(),
        PINNED_GLOBAL_REPRESENTATIVES.into_iter().collect()
    );
}

#[test]
fn portfolio_limits_fail_before_materializing_the_fixed_k4_census() {
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let baseline = OrderingPortfolioLimits::default();
    for (limits, resource, requested, limit) in [
        (
            OrderingPortfolioLimits {
                max_group_routes: GROUP_ORDER - 1,
                ..baseline
            },
            "group routes",
            GROUP_ORDER,
            GROUP_ORDER - 1,
        ),
        (
            OrderingPortfolioLimits {
                max_structural_images: STRUCTURAL_IMAGE_COUNT - 1,
                ..baseline
            },
            "structural images",
            STRUCTURAL_IMAGE_COUNT,
            STRUCTURAL_IMAGE_COUNT - 1,
        ),
        (
            OrderingPortfolioLimits {
                max_structural_partitions: STRUCTURAL_PARTITION_COUNT - 1,
                ..baseline
            },
            "structural partitions",
            STRUCTURAL_PARTITION_COUNT,
            STRUCTURAL_PARTITION_COUNT - 1,
        ),
        (
            OrderingPortfolioLimits {
                max_decorated_cases: DECORATED_MODULAR_CASE_COUNT - 1,
                ..baseline
            },
            "decorated modular cases",
            DECORATED_MODULAR_CASE_COUNT,
            DECORATED_MODULAR_CASE_COUNT - 1,
        ),
    ] {
        assert!(matches!(
            OrderingPortfolioCensus::try_new(&canonicalizer, limits),
            Err(OrderingPortfolioError::ResourceLimit {
                resource: actual_resource,
                requested: actual_requested,
                limit: actual_limit,
            }) if actual_resource == resource
                && actual_requested == requested
                && actual_limit == limit
        ));
    }
}

fn assert_diagonal_coverage(
    canonicalizer: &Canonicalizer,
    quotient: &CoordinatePriorityQuotient,
    portfolio: &SectorPortfolio,
    inverse_routes: &[usize],
    action_limits: CoordinatePriorityActionLimits,
) {
    let mut fixed_priority_multiplicity = BTreeMap::<CoordinatePriority, usize>::new();
    let mut cases_by_structural_image = BTreeMap::<[bool; ARITY], usize>::new();
    for representative in quotient.representatives() {
        for (route, source_for_target) in canonicalizer.group_elements().enumerate() {
            // A run using representative r on g.task maps back to g^-1.r on
            // the fixed task.  Retaining every route (rather than one route
            // per structural image) is mandatory for a generic chart/probe.
            let fixed_priority = canonicalizer
                .transport_coordinate_priority(representative, inverse_routes[route], action_limits)
                .unwrap();
            *fixed_priority_multiplicity
                .entry(fixed_priority)
                .or_default() += 1;
            *cases_by_structural_image
                .entry(transport_fixed(&portfolio.fixed_sector, source_for_target))
                .or_default() += 1;
        }
    }

    assert_eq!(fixed_priority_multiplicity.len(), RAW_PRIORITY_COUNT);
    assert!(
        fixed_priority_multiplicity
            .values()
            .all(|&multiplicity| multiplicity == 1)
    );
    assert_eq!(
        fixed_priority_multiplicity.values().sum::<usize>(),
        RAW_PRIORITY_COUNT
    );
    assert_eq!(
        cases_by_structural_image.len(),
        portfolio.route_buckets.len()
    );
    for bucket in &portfolio.route_buckets {
        assert_eq!(
            cases_by_structural_image.get(&bucket.image),
            Some(&(GLOBAL_PRIORITY_CLASS_COUNT * bucket.routes().len()))
        );
    }
}

fn fixed_sector_priority_classes(
    canonicalizer: &Canonicalizer,
    quotient: &CoordinatePriorityQuotient,
    stabilizer_routes: &[usize],
    action_limits: CoordinatePriorityActionLimits,
) -> Result<BTreeMap<[usize; ARITY], usize>, OrderingPortfolioError> {
    let mut classes = BTreeMap::new();
    for priority in quotient.classes().iter().flat_map(|class| class.images()) {
        let mut representative = [usize::MAX; ARITY];
        for &route in stabilizer_routes {
            let image =
                canonicalizer.transport_coordinate_priority(priority, route, action_limits)?;
            let ranks: [usize; ARITY] =
                image
                    .rank_by_slot()
                    .try_into()
                    .map_err(|_| OrderingPortfolioError::Invariant {
                        detail: "authenticated K4 priority changed arity during transport",
                    })?;
            representative = representative.min(ranks);
        }
        *classes.entry(representative).or_default() += 1;
    }
    Ok(classes)
}

fn try_inverse_routes(
    canonicalizer: &Canonicalizer,
    limits: OrderingPortfolioLimits,
) -> Result<Vec<usize>, OrderingPortfolioError> {
    admit_limit(
        "inverse group routes",
        canonicalizer.group_order(),
        limits.max_group_routes,
    )?;
    let mut inverse_routes = Vec::new();
    try_reserve_exact(
        &mut inverse_routes,
        canonicalizer.group_order(),
        "inverse group routes",
    )?;
    for source_for_target in canonicalizer.group_elements() {
        let mut inverse = Vec::new();
        try_reserve_exact(&mut inverse, canonicalizer.arity(), "one inverse route")?;
        inverse.resize(canonicalizer.arity(), usize::MAX);
        for (target, &source) in source_for_target.iter().enumerate() {
            if source >= inverse.len() || inverse[source] != usize::MAX {
                return Err(OrderingPortfolioError::Invariant {
                    detail: "authenticated group route is not a permutation",
                });
            }
            inverse[source] = target;
        }
        let inverse_route = canonicalizer
            .group_elements()
            .position(|candidate| candidate == inverse)
            .ok_or(OrderingPortfolioError::Invariant {
                detail: "authenticated finite group does not contain an inverse route",
            })?;
        inverse_routes.push(inverse_route);
    }
    Ok(inverse_routes)
}

fn k4_graph(edge_colors: [usize; ARITY]) -> Graph<(), usize> {
    let mut graph = Graph::new();
    for _ in 0..4 {
        graph.add_node(());
    }
    for (slot, &(source, target)) in K4_EDGES.iter().enumerate() {
        graph
            .add_edge(source, target, false, edge_colors[slot])
            .expect("the fixed K4 endpoints name four constructed vertices");
    }
    graph
}

fn transport_fixed<T: Copy>(input: &[T; ARITY], source_for_target: &[usize]) -> [T; ARITY] {
    debug_assert_eq!(source_for_target.len(), ARITY);
    std::array::from_fn(|target| input[source_for_target[target]])
}

fn checked_sum(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, OrderingPortfolioError> {
    left.checked_add(right)
        .ok_or(OrderingPortfolioError::ResourceCountOverflow { resource })
}

fn checked_product(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, OrderingPortfolioError> {
    left.checked_mul(right)
        .ok_or(OrderingPortfolioError::ResourceCountOverflow { resource })
}

fn admit_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), OrderingPortfolioError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(OrderingPortfolioError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

fn try_reserve_exact<T>(
    values: &mut Vec<T>,
    requested: usize,
    resource: &'static str,
) -> Result<(), OrderingPortfolioError> {
    values
        .try_reserve_exact(requested)
        .map_err(|_| OrderingPortfolioError::AllocationFailure {
            resource,
            requested,
        })
}
