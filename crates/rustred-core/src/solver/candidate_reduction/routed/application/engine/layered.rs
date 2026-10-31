//! Layered Route transport of the Ordered adapter (see
//! [`OrderedRouteLayering`]): a Route key with several numerator factors is
//! transported one factor per step through *partially transported* keys.
//!
//! A partial key `(S, t, r)` of source sector `S` stands for the integral
//! `D^-t * prod_{a: r_a < 0} (c_a + sum_l L_al D_l)^-r_a` in the owner's
//! denominators, where `t` are the owner powers obtained so far and `r` the
//! source numerator powers whose factors are not yet expanded (the affine
//! forms of `S`'s route). A Route key `R` of `S` is `(S, base(R), r(R))`. One
//! step expands one factor of `r` (chosen by the layering policy, a function
//! of `(S, r)` only) and emits `(S, t - e, r')` for every monomial `D^e` of the
//! factor's exact expansion; the last factor emits the Route and Apply
//! children of the whole transport. Partial keys of the same sector merge on
//! the frontier, so the later factors are expanded once for the merged
//! weight. The coalesced product is the same polynomial as the whole
//! transport, hence the final weights are exactly the historical ones.
//!
//! Pruning: a partial key whose owner support has only installed zero
//! subsectors (all of its subsets are zero sectors) is dropped, since every
//! final child it could reach has a subset of its support (expanding factors
//! only lowers powers) and would be dropped as a zero-sector child. Layering
//! requires an application without source conditions, so dropped and merged
//! endpoints retain no guards (their validation retains nothing).
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use super::*;
use crate::sector::ComplexityKey;
use crate::sector::symmetry::integral_transport::Endpoint;
use crate::solver::candidate_reduction::routed::CandidateOwnerRoute;
use crate::solver::candidate_reduction::routed::OrderedRouteLayering;
use crate::solver::candidate_reduction::routed::trace::count;

/// A partially transported Route key (see the module docs).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct PartialRoute<const N: usize> {
    /// Source sector (the route).
    pub(super) source: [bool; N],
    /// Owner powers obtained so far (first `N`), then the source numerator
    /// powers whose factors are not yet expanded (last `N`; zero elsewhere).
    pub(super) powers: Box<[i64]>,
}

impl<const N: usize> PartialRoute<N> {
    pub(super) fn target(&self) -> &[i64] {
        &self.powers[..N]
    }

    pub(super) fn remaining(&self) -> &[i64] {
        &self.powers[N..]
    }

    /// Total numerator power not yet expanded; strictly decreases along
    /// every partial step, which orders partial keys inside their phase.
    pub(super) fn remaining_power(&self) -> u64 {
        self.remaining().iter().map(|power| power.unsigned_abs()).sum()
    }
}

/// Key of the Ordered routed adapter: a Route or Apply node, or a partially
/// transported Route key.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum OrderedNode<const N: usize> {
    Node(Node<N>),
    Partial(PartialRoute<N>),
}

/// Scheduling priority of [`OrderedNode`]: partial keys sort below every
/// Route key of their phase, larger remaining numerator power first.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum RoutedPriority {
    Partial(u64),
    Key(ComplexityKey),
}

type Expansion = Arc<[Endpoint]>;

/// Per-request layering plan: policy, zero pruning and the memoized factor
/// expansions (pure functions of route, source axis and power).
pub(super) struct RouteLayers<const N: usize> {
    layering: OrderedRouteLayering,
    /// Owner supports (bit masks) all of whose subsets are installed zero
    /// sectors.
    hereditary_zero: HashSet<u64>,
    expansions: RwLock<HashMap<([bool; N], usize, u64), Expansion>>,
}

fn mask_of<const N: usize>(powers: &[i64]) -> u64 {
    powers
        .iter()
        .take(N)
        .enumerate()
        .filter(|&(_, &power)| power > 0)
        .fold(0, |bits, (position, _)| bits | 1u64 << position)
}

impl<const N: usize> RouteLayers<N> {
    /// The plan of one request. Layering is off for applications with
    /// source conditions (their endpoint validation can retain guards) and
    /// for arities above 64.
    pub(super) fn new(application: &RoutedCandidateApplication<N>) -> Self {
        let shared = &application.context.shared;
        let layering = if N > 64 || !shared.source_conditions.is_empty() {
            OrderedRouteLayering::Off
        } else {
            application.ordered_route_layering
        };
        let mut hereditary_zero = HashSet::new();
        if layering != OrderedRouteLayering::Off {
            let mut zero: Vec<u64> = shared
                .zero_sectors
                .iter()
                .map(|sector| {
                    sector
                        .iter()
                        .enumerate()
                        .filter(|&(_, &bit)| bit)
                        .fold(0u64, |bits, (position, _)| bits | 1u64 << position)
                })
                .collect();
            zero.sort_by_key(|mask| mask.count_ones());
            for mask in zero {
                let mut rest = mask;
                let mut hereditary = true;
                while rest != 0 {
                    let bit = rest & rest.wrapping_neg();
                    rest &= rest - 1;
                    if !hereditary_zero.contains(&(mask & !bit)) {
                        hereditary = false;
                        break;
                    }
                }
                if hereditary {
                    hereditary_zero.insert(mask);
                }
            }
        }
        Self {
            layering,
            hereditary_zero,
            expansions: RwLock::new(HashMap::new()),
        }
    }

    /// The route of a Route key that takes the layered transport: layering
    /// is on, its support is not an owner, it has a route and at least two
    /// numerator factors. A function of the key only.
    pub(super) fn layered_route<'a>(
        &self,
        application: &'a RoutedCandidateApplication<N>,
        node: &Node<N>,
    ) -> Option<(&'a CandidateOwnerRoute, [bool; N])> {
        if self.layering == OrderedRouteLayering::Off {
            return None;
        }
        let Node::Route(key) = node else {
            return None;
        };
        if key.powers().iter().filter(|&&power| power < 0).count() < 2 {
            return None;
        }
        let source = support::<N>(key);
        if application.owners.contains(&source) {
            return None;
        }
        let route = application.routes.get(&source)?;
        (route.transport.target_family().denominator_count() == N).then_some((route, source))
    }

    fn expansion(
        &self,
        application: &RoutedCandidateApplication<N>,
        source: [bool; N],
        route: &CandidateOwnerRoute,
        axis: usize,
        power: u64,
    ) -> Result<Expansion, CandidateRoutedError> {
        let id = (source, axis, power);
        if let Some(found) = self
            .expansions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&id)
        {
            return Ok(found.clone());
        }
        let expansion: Expansion = route
            .transport
            .factor_expansion(axis, power, application.limits.expansion)?
            .into();
        Ok(self
            .expansions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(id)
            .or_insert(expansion)
            .clone())
    }

    /// The factor expanded next, a function of the source sector and the
    /// remaining powers only.
    fn next_factor(
        &self,
        application: &RoutedCandidateApplication<N>,
        source: [bool; N],
        route: &CandidateOwnerRoute,
        remaining: &[i64],
    ) -> Result<usize, CandidateRoutedError> {
        let mut factors = remaining
            .iter()
            .enumerate()
            .filter(|&(_, &power)| power < 0)
            .map(|(axis, &power)| (axis, power.unsigned_abs()));
        let chosen = match self.layering {
            OrderedRouteLayering::Off | OrderedRouteLayering::Index => factors.next(),
            OrderedRouteLayering::SmallestPower => {
                factors.min_by_key(|&(axis, power)| (power, axis))
            }
            OrderedRouteLayering::LargestPower => {
                factors.min_by_key(|&(axis, power)| (std::cmp::Reverse(power), axis))
            }
            OrderedRouteLayering::FewestTerms => {
                let mut best = None;
                for (axis, power) in factors {
                    let terms = self
                        .expansion(application, source, route, axis, power)?
                        .len();
                    if best.is_none_or(|(t, p, a, _)| (terms, power, axis) < (t, p, a)) {
                        best = Some((terms, power, axis, ()));
                    }
                }
                best.map(|(_, power, axis, ())| (axis, power))
            }
        };
        chosen.map(|(axis, _)| axis).ok_or_else(|| {
            CandidateRoutedError::InvalidInput("a partial Route key has no factor left".into())
        })
    }
}

/// `left + right`, with room for `extra` more powers.
fn add_powers(left: &[i64], right: &[i64], extra: usize) -> Result<Vec<i64>, CandidateRoutedError> {
    let mut sum = Vec::with_capacity(left.len() + extra);
    for (&a, &b) in left.iter().zip(right) {
        sum.push(a.checked_add(b).ok_or(CandidateRoutedError::ResourceLimit {
            resource: "layered transport power",
            requested: usize::MAX,
            limit: i64::MAX as usize,
        })?);
    }
    Ok(sum)
}

impl<const N: usize> RoutedCandidateApplication<N> {
    /// The children of one layered step of the partial state
    /// `(source, target, remaining)`: one factor expanded. Zero-sector final
    /// children are kept here (validated); the caller drops them like the
    /// whole transport's.
    pub(super) fn expand_route_layer(
        &self,
        layers: &RouteLayers<N>,
        source: [bool; N],
        route: &CandidateOwnerRoute,
        target: &[i64],
        remaining: &[i64],
        conditions: &mut Vec<Coefficient>,
        request: &mut Request<'_, '_, N>,
    ) -> Result<BTreeMap<OrderedNode<N>, Coefficient>, CandidateRoutedError> {
        charge(
            &mut request.stats.transport_calls,
            1,
            self.limits.diagnostic_limit(self.limits.max_transport_calls),
            "transport calls",
        )?;
        request.observe_activity(self, CandidateRoutedActivity::Transport, None);
        let axis = layers.next_factor(self, source, route, remaining)?;
        let power = remaining[axis].unsigned_abs();
        let expansion = layers.expansion(self, source, route, axis, power)?;
        let operations = request
            .stats
            .transport_operations
            .saturating_add(expansion.len());
        let endpoints = request
            .stats
            .transport_endpoints
            .saturating_add(expansion.len());
        for (resource, requested, limit) in [
            (
                "aggregate routed operations",
                operations,
                self.limits.max_transport_operations,
            ),
            (
                "aggregate routed endpoints",
                endpoints,
                self.limits.max_transport_endpoints,
            ),
        ] {
            if self.limits.diagnostics_enabled && requested > limit {
                return Err(CandidateRoutedError::ResourceLimit {
                    resource,
                    requested,
                    limit,
                });
            }
        }
        request.stats.transport_operations = operations;
        request.stats.transport_endpoints = endpoints;
        let mut rest = remaining.to_vec();
        rest[axis] = 0;
        let last = rest.iter().all(|&power| power == 0);
        let owner: [bool; N] = std::array::from_fn(|i| {
            route
                .owner_sector
                .active_bits()
                .get(i)
                .copied()
                .unwrap_or(false)
        });
        let context = &self.context;
        let shared = &context.shared;
        let base = CandidateEvaluator {
            context: &shared.context,
            root_sector: [true; N],
            ordering: &Default::default(),
            rules: &[],
            whole_piece_alternatives: &[],
            source_conditions: &shared.source_conditions,
            zero_sectors: &shared.zero_sectors,
            limits: context.limits,
        };
        let mut terms = BTreeMap::new();
        for endpoint in expansion.iter() {
            let powers = add_powers(target, endpoint.key().powers(), if last { 0 } else { N })?;
            if last {
                let key = IntegralKey::try_new(powers)
                    .map_err(|error| CandidateRoutedError::InvalidInput(format!("{error:?}")))?;
                base.validate_target_with_conditions(&key, &mut |condition| {
                    retain_polynomial(conditions, condition)
                })?;
                let child_support = support::<N>(&key);
                let child = if child_support == owner {
                    Node::Apply(owner, key)
                } else if child_support.iter().zip(owner).all(|(&c, o)| !c || o)
                    && count(&child_support) < count(&source)
                {
                    Node::Route(key)
                } else {
                    return Err(CandidateRoutedError::UnsupportedSupportTransition {
                        target: IntegralKey::try_new(source.map(i64::from)).map_err(|error| {
                            CandidateRoutedError::InvalidInput(format!("{error:?}"))
                        })?,
                        child: key,
                    });
                };
                terms.insert(OrderedNode::Node(child), endpoint.coefficient().clone());
            } else {
                if layers.hereditary_zero.contains(&mask_of::<N>(&powers)) {
                    continue;
                }
                let mut packed = powers;
                packed.extend_from_slice(&rest);
                terms.insert(
                    OrderedNode::Partial(PartialRoute {
                        source,
                        powers: packed.into_boxed_slice(),
                    }),
                    endpoint.coefficient().clone(),
                );
            }
        }
        Ok(terms)
    }

    /// The layered step of a Route key that takes it (see
    /// [`RouteLayers::layered_route`]): its first factor.
    pub(super) fn layered_route_step(
        &self,
        layers: &RouteLayers<N>,
        key: &IntegralKey,
        route: &CandidateOwnerRoute,
        source: [bool; N],
        conditions: &mut Vec<Coefficient>,
        request: &mut Request<'_, '_, N>,
    ) -> Result<BTreeMap<OrderedNode<N>, Coefficient>, CandidateRoutedError> {
        let base = route
            .transport
            .layered_base(key, self.limits.expansion)?;
        let remaining: Vec<i64> = key.powers().iter().map(|&power| power.min(0)).collect();
        self.expand_route_layer(
            layers,
            source,
            route,
            base.powers(),
            &remaining,
            conditions,
            request,
        )
    }

    /// The step of a partial key: its next factor.
    pub(super) fn partial_route_step(
        &self,
        layers: &RouteLayers<N>,
        partial: &PartialRoute<N>,
        conditions: &mut Vec<Coefficient>,
        request: &mut Request<'_, '_, N>,
    ) -> Result<BTreeMap<OrderedNode<N>, Coefficient>, CandidateRoutedError> {
        let route = self.routes.get(&partial.source).ok_or_else(|| {
            CandidateRoutedError::InvalidInput("a partial Route key lost its route".into())
        })?;
        self.expand_route_layer(
            layers,
            partial.source,
            route,
            partial.target(),
            partial.remaining(),
            conditions,
            request,
        )
    }
}
