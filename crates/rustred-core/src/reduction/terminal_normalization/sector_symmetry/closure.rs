//! Whole-polynomial Reynolds orbits for the original requested integrals.

use super::*;
use crate::algebra::Coefficient;
use crate::reduction::terminal_relations::TerminalRelationRow;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(Default)]
struct MapRegistry {
    maps: HashMap<u64, Vec<Arc<VerifiedMap>>>,
    prepared: HashMap<(Mask, u64), Vec<Arc<integral_transport::Prepared>>>,
}

impl MapRegistry {
    fn intern(&mut self, map: Arc<VerifiedMap>) -> Arc<VerifiedMap> {
        let bucket = self.maps.entry(map_hash(map.momentum())).or_default();
        if let Some(existing) = bucket.iter().find(|old| {
            old.momentum() == map.momentum()
                && old.source_family_fingerprint() == map.source_family_fingerprint()
                && old.target_family_fingerprint() == map.target_family_fingerprint()
        }) {
            return existing.clone();
        }
        bucket.push(map.clone());
        map
    }

    fn verify(
        &mut self,
        family: &Arc<IntegralFamily>,
        map: MomentumMap,
        limits: TerminalSectorSymmetryLimits,
    ) -> Result<Arc<VerifiedMap>, Error> {
        if let Some(existing) = self.maps.get(&map_hash(&map)).and_then(|bucket| {
            bucket.iter().find(|old| {
                old.momentum() == &map
                    && old.source_family_fingerprint() == family.fingerprint()
                    && old.target_family_fingerprint() == family.fingerprint()
            })
        }) {
            return Ok(existing.clone());
        }
        Ok(self.intern(Arc::new(
            symmetry::verify(family, family, map, limits.verification).map_err(algebra)?,
        )))
    }

    fn prepare(
        &mut self,
        family: &Arc<IntegralFamily>,
        root: &Mask,
        map: Arc<VerifiedMap>,
        limits: TerminalSectorSymmetryLimits,
    ) -> Result<Arc<integral_transport::Prepared>, Error> {
        let bucket = self
            .prepared
            .entry((root.clone(), map_hash(map.momentum())))
            .or_default();
        if let Some(old) = bucket
            .iter()
            .find(|old| old.verified_map().momentum() == map.momentum())
        {
            return Ok(old.clone());
        }
        let prepared = Arc::new(transport::prepare_map(family, root, map, limits)?);
        bucket.push(prepared.clone());
        Ok(prepared)
    }
}

fn map_hash(map: &MomentumMap) -> u64 {
    let mut hash = DefaultHasher::new();
    for matrix in [
        map.loop_linear(),
        map.loop_external(),
        map.external_linear(),
    ] {
        matrix.rows().hash(&mut hash);
        matrix.columns().hash(&mut hash);
        matrix.entries().hash(&mut hash);
    }
    hash.finish()
}

fn image_hash(image: &TerminalRelationRow) -> u64 {
    let mut hash = DefaultHasher::new();
    image.hash(&mut hash);
    hash.finish()
}

pub(super) fn prepare(
    family: Arc<IntegralFamily>,
    keys: &BTreeSet<IntegralKey>,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<TerminalSectorSymmetryEquations, Error> {
    prepare_with_policy(
        family,
        keys,
        limits,
        control,
        TerminalSectorSymmetryPolicy::PolynomialOrbits,
    )
}

pub(super) fn prepare_with_policy(
    family: Arc<IntegralFamily>,
    keys: &BTreeSet<IntegralKey>,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
    policy: TerminalSectorSymmetryPolicy,
) -> Result<TerminalSectorSymmetryEquations, Error> {
    eligible(&family)?;
    for key in keys {
        validate_key(&family, key)?;
    }
    let mut registry = MapRegistry::default();
    let identity = registry.verify(&family, transport::identity_map(&family, limits)?, limits)?;
    let roots = keys
        .iter()
        .map(|key| Mask::try_from_indices(key.powers()).map_err(algebra))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut cache = discovery::DiscoveryCache::new(family.clone());
    let mut classes: HashMap<
        discovery::CanonicalSupportKey,
        Vec<(Mask, Arc<discovery::PreparedSupport>)>,
    > = HashMap::new();
    let mut routes = BTreeMap::new();
    let mut generators = BTreeMap::new();
    let mut skipped = BTreeMap::new();
    for root in roots {
        control.checkpoint()?;
        if let Some(support) = cache.prepare_support(&root, limits, control)? {
            let representatives = classes.entry(support.canonical_key()).or_default();
            let mut matched = None;
            for (canonical, representative) in representatives.iter() {
                if let Some(route) =
                    discovery::match_supports(&support, representative, limits, control)?
                {
                    matched = Some((canonical.clone(), registry.intern(route)));
                    break;
                }
            }
            if let Some((_, route)) = matched {
                routes.insert(root.clone(), route);
            } else {
                let maps = discovery::discover_prepared(&support, limits, control)?
                    .into_iter()
                    .map(|map| registry.intern(map))
                    .collect();
                routes.insert(root.clone(), identity.clone());
                generators.insert(root.clone(), maps);
                representatives.push((root.clone(), support));
            }
        } else {
            routes.insert(root.clone(), identity.clone());
            generators.insert(root.clone(), Vec::new());
            if let Some(reason) = cache.skip_reason(&root) {
                *skipped.entry(reason).or_default() += 1;
            }
        }
        control.report(TerminalSectorSymmetryProgress {
            processed_keys: 0,
            discovered_keys: keys.len(),
            analyzed_supports: routes.len(),
            verified_generators: generators.values().map(Vec::len).sum(),
            orbit_images: 0,
            orbit_terms: 0,
        });
    }
    let mut result = build(
        family.clone(),
        keys,
        routes,
        generators,
        limits,
        control,
        &mut registry,
        policy,
    )?;
    result.statistics.skipped = skipped;
    control.checkpoint()?;
    let legacy = Arc::new(super::super::TerminalNumeratorSymmetryEquations::prepare(
        family, keys, limits,
    )?);
    control.checkpoint()?;
    attach_legacy(&mut result, legacy)?;
    report(&result, keys.len(), 0, 0, control);
    control.checkpoint()?;
    Ok(result)
}

pub(super) fn prepare_with_generators(
    family: Arc<IntegralFamily>,
    keys: &BTreeSet<IntegralKey>,
    generators: BTreeMap<Mask, Vec<Arc<VerifiedMap>>>,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<TerminalSectorSymmetryEquations, Error> {
    let mut registry = MapRegistry::default();
    let identity = registry.verify(&family, transport::identity_map(&family, limits)?, limits)?;
    let roots = keys
        .iter()
        .map(|key| Mask::try_from_indices(key.powers()).map_err(algebra))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let routes = roots
        .iter()
        .map(|root| (root.clone(), identity.clone()))
        .collect();
    let mut generators = generators;
    for root in roots {
        generators.entry(root).or_default();
    }
    build(
        family,
        keys,
        routes,
        generators,
        limits,
        control,
        &mut registry,
        TerminalSectorSymmetryPolicy::PolynomialOrbits,
    )
}

pub(super) fn prepare_with_orbit_supports(
    family: Arc<IntegralFamily>,
    keys: &BTreeSet<IntegralKey>,
    routes: BTreeMap<Mask, Arc<VerifiedMap>>,
    generators: BTreeMap<Mask, Vec<Arc<VerifiedMap>>>,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<TerminalSectorSymmetryEquations, Error> {
    prepare_with_orbit_supports_policy(
        family,
        keys,
        routes,
        generators,
        limits,
        control,
        TerminalSectorSymmetryPolicy::PolynomialOrbits,
    )
}

pub(super) fn prepare_with_orbit_supports_policy(
    family: Arc<IntegralFamily>,
    keys: &BTreeSet<IntegralKey>,
    routes: BTreeMap<Mask, Arc<VerifiedMap>>,
    generators: BTreeMap<Mask, Vec<Arc<VerifiedMap>>>,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
    policy: TerminalSectorSymmetryPolicy,
) -> Result<TerminalSectorSymmetryEquations, Error> {
    build(
        family,
        keys,
        routes,
        generators,
        limits,
        control,
        &mut MapRegistry::default(),
        policy,
    )
}

fn build(
    family: Arc<IntegralFamily>,
    keys: &BTreeSet<IntegralKey>,
    routes: BTreeMap<Mask, Arc<VerifiedMap>>,
    generators: BTreeMap<Mask, Vec<Arc<VerifiedMap>>>,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
    registry: &mut MapRegistry,
    policy: TerminalSectorSymmetryPolicy,
) -> Result<TerminalSectorSymmetryEquations, Error> {
    eligible(&family)?;
    control.checkpoint()?;
    for key in keys {
        validate_key(&family, key)?;
    }
    let roots = keys
        .iter()
        .map(|key| Mask::try_from_indices(key.powers()).map_err(algebra))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if routes.keys().cloned().collect::<BTreeSet<_>>() != roots {
        return Err(Error::InvalidWitness(
            "sector symmetry route domain differs from original supports",
        ));
    }
    let mut canonical_roots = BTreeSet::new();
    let mut prepared_routes = BTreeMap::new();
    let mut shared_routes = BTreeMap::new();
    for (root, map) in routes {
        if map.source_family_fingerprint() != family.fingerprint()
            || map.target_family_fingerprint() != family.fingerprint()
        {
            return Err(Error::InvalidWitness("foreign sector symmetry orbit route"));
        }
        let map = registry.intern(map);
        let prepared = registry.prepare(&family, &root, map.clone(), limits)?;
        canonical_roots.insert(prepared.target_root().clone());
        prepared_routes.insert(root.clone(), prepared);
        shared_routes.insert(root, map);
    }
    if generators.keys().cloned().collect::<BTreeSet<_>>() != canonical_roots {
        return Err(Error::InvalidWitness(
            "sector symmetry generator domain differs from canonical supports",
        ));
    }
    let mut prepared_generators = BTreeMap::new();
    let mut shared_generators = BTreeMap::new();
    for (root, maps) in generators {
        if policy == TerminalSectorSymmetryPolicy::PolynomialOrbits
            && !maps.is_empty()
            && !discovery::verify_finite_root(&family, &root, limits, control)?
        {
            return Err(Error::InvalidWitness(
                "sector symmetry orbit generators require full-rank active momentum geometry",
            ));
        }
        let mut unique: Vec<Arc<VerifiedMap>> = Vec::new();
        let mut prepared = Vec::new();
        for map in maps {
            control.checkpoint()?;
            if map.source_family_fingerprint() != family.fingerprint()
                || map.target_family_fingerprint() != family.fingerprint()
            {
                return Err(Error::InvalidWitness(
                    "foreign sector symmetry orbit generator",
                ));
            }
            let map = registry.intern(map);
            if unique.iter().any(|old| old.momentum() == map.momentum()) {
                return Err(Error::InvalidWitness(
                    "duplicate sector symmetry orbit generator",
                ));
            }
            let transport = registry.prepare(&family, &root, map.clone(), limits)?;
            if transport.target_root() != &root {
                return Err(Error::InvalidWitness(
                    "sector symmetry orbit generator is not a self-support map",
                ));
            }
            prepared.push(transport);
            unique.push(map);
        }
        prepared_generators.insert(root.clone(), prepared);
        shared_generators.insert(root, unique);
    }
    let mut plan = TerminalSectorSymmetryEquations {
        policy,
        family_fingerprint: family.fingerprint_owner(),
        input_keys: keys.clone(),
        inventory: keys.clone(),
        routes: shared_routes,
        generators: shared_generators,
        rows: BTreeMap::new(),
        witnesses: BTreeMap::new(),
        legacy: None,
        statistics: TerminalSectorSymmetryStatistics {
            input_keys: keys.len(),
            closure_keys: keys.len(),
            analyzed_supports: roots.len(),
            verified_generators: prepared_generators.values().map(Vec::len).sum(),
            ..Default::default()
        },
    };
    for (processed, source) in keys.iter().enumerate() {
        control.checkpoint()?;
        let root = Mask::try_from_indices(source.powers()).map_err(algebra)?;
        let route = &prepared_routes[&root];
        let canonical = route.target_root();
        if policy == TerminalSectorSymmetryPolicy::GeneratorRelations {
            let initial = plan.routes[&root].clone();
            let mut candidates = vec![initial.clone()];
            for generator in &plan.generators[canonical] {
                control.checkpoint()?;
                let momentum = transport::compose_maps(
                    &family,
                    initial.momentum(),
                    generator.momentum(),
                    limits,
                )?;
                let composite = registry.verify(&family, momentum, limits)?;
                if !candidates
                    .iter()
                    .any(|old| old.momentum() == composite.momentum())
                {
                    candidates.push(composite);
                }
            }
            let mut rows = Vec::new();
            let mut proofs = Vec::new();
            for (index, map) in candidates.into_iter().enumerate() {
                control.checkpoint()?;
                let prepared = registry.prepare(&family, &root, map.clone(), limits)?;
                if prepared.target_root() != canonical {
                    return Err(Error::InvalidWitness(
                        "sector symmetry generator cohort leaves its canonical support",
                    ));
                }
                let result = prepared.transport(source, limits.expansion)?;
                let equation = transport::complete_equation(&family, source, &result, limits)?;
                plan.inventory.extend(equation.terms.keys().cloned());
                if equation.terms.is_empty() {
                    plan.statistics.trivial_equations += 1;
                } else if !rows.contains(&equation) {
                    plan.statistics.equations =
                        plan.statistics
                            .equations
                            .checked_add(1)
                            .ok_or(Error::InvalidWitness(
                                "sector symmetry generator equation count overflow",
                            ))?;
                    plan.statistics.output_terms = plan
                        .statistics
                        .output_terms
                        .checked_add(equation.terms.len())
                        .ok_or(Error::InvalidWitness(
                            "sector symmetry generator output count overflow",
                        ))?;
                    rows.push(equation.clone());
                }
                proofs.push(TerminalSectorSymmetryOrbitWitness {
                    family: family.fingerprint_owner(),
                    source: source.clone(),
                    maps: vec![map],
                    equation,
                });
                report(&plan, processed, index + 1, result.terms().len(), control);
            }
            if !rows.is_empty() {
                plan.statistics.equation_targets += 1;
                plan.rows.insert(source.clone(), rows);
            }
            plan.witnesses.insert(source.clone(), proofs);
            plan.statistics.closure_keys = plan.inventory.len();
            report(&plan, processed + 1, 0, 0, control);
            continue;
        }
        let first = image_terms(&route.transport(source, limits.expansion)?);
        let (maps, equation) = orbit(
            &family,
            source,
            first,
            plan.routes[&root].clone(),
            &root,
            canonical,
            &plan.generators[canonical],
            limits,
            control,
            registry,
            &plan,
            processed,
        )?;
        plan.inventory.extend(equation.terms.keys().cloned());
        if equation.terms.is_empty() {
            plan.statistics.trivial_equations += 1;
        } else {
            plan.statistics.equation_targets += 1;
            plan.statistics.equations += 1;
            plan.statistics.output_terms = plan
                .statistics
                .output_terms
                .checked_add(equation.terms.len())
                .ok_or(Error::InvalidWitness(
                    "sector symmetry average output count overflow",
                ))?;
            plan.rows.insert(source.clone(), vec![equation.clone()]);
        }
        plan.witnesses.insert(
            source.clone(),
            vec![TerminalSectorSymmetryOrbitWitness {
                family: family.fingerprint_owner(),
                source: source.clone(),
                maps,
                equation,
            }],
        );
        plan.statistics.closure_keys = plan.inventory.len();
        report(&plan, processed + 1, 0, 0, control);
    }
    control.checkpoint()?;
    Ok(plan)
}

/// Combine original-source weak rows with the exact orbit averages. Reynolds
/// preserves the pure-group wanted relation space, but a finite saved-IBP
/// projection need not commute with that average; retaining this cohort keeps
/// the previously available projection consequences without endpoint orbits.
pub(super) fn attach_legacy(
    plan: &mut TerminalSectorSymmetryEquations,
    legacy: Arc<super::super::TerminalNumeratorSymmetryEquations>,
) -> Result<(), Error> {
    if plan.legacy.is_some() {
        return Err(Error::InvalidWitness(
            "sector symmetry legacy supplement already attached",
        ));
    }
    if legacy.family_fingerprint() != plan.family_fingerprint()
        || legacy.input_keys() != plan.input_keys()
    {
        return Err(Error::InvalidWitness(
            "sector symmetry legacy supplement ownership or original inputs differ",
        ));
    }
    for source in &plan.input_keys {
        for row in legacy.equations(source) {
            if row.terms.is_empty() {
                continue;
            }
            let rows = plan.rows.entry(source.clone()).or_default();
            if !rows.contains(row) {
                rows.push(row.clone());
            }
        }
    }
    plan.inventory = plan.input_keys.clone();
    plan.statistics.equation_targets = plan.rows.len();
    plan.statistics.equations = 0;
    plan.statistics.output_terms = 0;
    for rows in plan.rows.values() {
        plan.statistics.equations =
            plan.statistics
                .equations
                .checked_add(rows.len())
                .ok_or(Error::InvalidWitness(
                    "sector symmetry combined equation count overflow",
                ))?;
        for row in rows {
            plan.inventory.extend(row.terms.keys().cloned());
            plan.statistics.output_terms = plan
                .statistics
                .output_terms
                .checked_add(row.terms.len())
                .ok_or(Error::InvalidWitness(
                    "sector symmetry combined output count overflow",
                ))?;
        }
    }
    plan.statistics.closure_keys = plan.inventory.len();
    plan.legacy = Some(legacy);
    Ok(())
}

fn orbit(
    family: &Arc<IntegralFamily>,
    source: &IntegralKey,
    first: TerminalRelationRow,
    initial: Arc<VerifiedMap>,
    source_root: &Mask,
    canonical_root: &Mask,
    generators: &[Arc<VerifiedMap>],
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
    registry: &mut MapRegistry,
    plan: &TerminalSectorSymmetryEquations,
    processed: usize,
) -> Result<(Vec<Arc<VerifiedMap>>, TerminalEquation), Error> {
    let mut images = vec![first];
    let mut maps = vec![initial];
    let mut index = HashMap::<u64, Vec<usize>>::from([(image_hash(&images[0]), vec![0])]);
    let mut cursor = 0;
    let mut orbit_terms = images[0].len();
    while cursor < images.len() {
        for generator in generators {
            control.checkpoint()?;
            // Apply the same representative generator to the WHOLE image via
            // its exact composite source map. Expanding the original source
            // once preserves all affine cancellations without monomial BFS.
            let momentum = transport::compose_maps(
                family,
                maps[cursor].momentum(),
                generator.momentum(),
                limits,
            )?;
            let map = registry.verify(family, momentum, limits)?;
            let prepared = registry.prepare(family, source_root, map.clone(), limits)?;
            if prepared.target_root() != canonical_root {
                return Err(Error::InvalidWitness(
                    "sector symmetry composite leaves its canonical support",
                ));
            }
            let image = image_terms(&prepared.transport(source, limits.expansion)?);
            let hash = image_hash(&image);
            if index
                .get(&hash)
                .is_some_and(|bucket| bucket.iter().any(|&i| images[i] == image))
            {
                continue;
            }
            index.entry(hash).or_default().push(images.len());
            orbit_terms = orbit_terms
                .checked_add(image.len())
                .ok_or(Error::InvalidWitness(
                    "sector symmetry orbit term count overflow",
                ))?;
            images.push(image);
            maps.push(map);
            report(plan, processed, images.len(), orbit_terms, control);
        }
        cursor += 1;
        report(plan, processed, images.len(), orbit_terms, control);
    }
    let equation = average_equation(family, source, &images, limits)?;
    Ok((maps, equation))
}

fn image_terms(transported: &integral_transport::TransportedIntegral) -> TerminalRelationRow {
    transported
        .terms()
        .iter()
        .map(|endpoint| (endpoint.key().clone(), endpoint.coefficient().clone()))
        .collect()
}

fn average_equation(
    family: &IntegralFamily,
    source: &IntegralKey,
    images: &[TerminalRelationRow],
    limits: TerminalSectorSymmetryLimits,
) -> Result<TerminalEquation, Error> {
    if images.is_empty() {
        return Err(Error::InvalidWitness(
            "empty sector symmetry whole-polynomial orbit",
        ));
    }
    let context = family.coefficient_context();
    let factor = context
        .try_div(
            &context.integer(-1),
            &context.unsigned_integer(images.len() as u128),
            limits.expansion.exact_algebra,
        )
        .map_err(algebra)?;
    let mut terms = BTreeMap::from([(source.clone(), context.one())]);
    for image in images {
        for (key, value) in image {
            let coefficient = context
                .try_mul(&factor, value, limits.expansion.exact_algebra)
                .map_err(algebra)?;
            accumulate(&mut terms, key, coefficient, family, limits)?;
        }
    }
    Ok(TerminalEquation {
        terms,
        nonzero_conditions: Vec::new(),
    })
}

fn accumulate(
    row: &mut TerminalRelationRow,
    key: &IntegralKey,
    value: Coefficient,
    family: &IntegralFamily,
    limits: TerminalSectorSymmetryLimits,
) -> Result<(), Error> {
    if value.is_zero() {
        return Ok(());
    }
    let value = match row.get(key) {
        Some(old) => family
            .coefficient_context()
            .try_add(old, &value, limits.expansion.exact_algebra)
            .map_err(algebra)?,
        None => value,
    };
    if value.is_zero() {
        row.remove(key);
    } else {
        row.insert(key.clone(), value);
    }
    Ok(())
}

pub(super) fn replay_average(
    family: &Arc<IntegralFamily>,
    source: &IntegralKey,
    maps: &[Arc<VerifiedMap>],
    limits: TerminalSectorSymmetryLimits,
) -> Result<TerminalEquation, Error> {
    let mut fresh_maps = Vec::new();
    for map in maps {
        fresh_maps.push(Arc::new(
            symmetry::verify(family, family, map.momentum().clone(), limits.verification)
                .map_err(algebra)?,
        ));
    }
    replay_verified_average(family, source, &fresh_maps, limits)
}

/// Registry maps were independently verified once by the native decoder.
/// Replay each original numerator without re-verifying a shared map per key.
pub(super) fn replay_verified_average(
    family: &Arc<IntegralFamily>,
    source: &IntegralKey,
    maps: &[Arc<VerifiedMap>],
    limits: TerminalSectorSymmetryLimits,
) -> Result<TerminalEquation, Error> {
    validate_key(family, source)?;
    let root = Mask::try_from_indices(source.powers()).map_err(algebra)?;
    let mut images: Vec<TerminalRelationRow> = Vec::new();
    let mut index = HashMap::<u64, Vec<usize>>::new();
    for map in maps {
        let prepared = transport::prepare_map(family, &root, map.clone(), limits)?;
        let result = prepared.transport(source, limits.expansion)?;
        let image = image_terms(&result);
        let hash = image_hash(&image);
        if index
            .get(&hash)
            .is_some_and(|bucket| bucket.iter().any(|&i| images[i] == image))
        {
            return Err(Error::InvalidWitness(
                "duplicate sector symmetry complete orbit image",
            ));
        }
        index.entry(hash).or_default().push(images.len());
        images.push(image);
    }
    average_equation(family, source, &images, limits)
}

fn report(
    plan: &TerminalSectorSymmetryEquations,
    processed_keys: usize,
    orbit_images: usize,
    orbit_terms: usize,
    control: &TerminalSectorSymmetryControl<'_>,
) {
    control.report(TerminalSectorSymmetryProgress {
        processed_keys,
        discovered_keys: plan.inventory.len(),
        analyzed_supports: plan.statistics.analyzed_supports,
        verified_generators: plan.statistics.verified_generators,
        orbit_images,
        orbit_terms,
    });
}
