//! Native finite certificates for complete-polynomial sector symmetry orbits.
//!
//! Exact momentum maps and complete equations are proof data. Cold loading
//! verifies those maps and repeats each original source's whole-polynomial
//! orbit using stored support routes and generators. Final inventory consists
//! only of original keys and nonzero averaged-row columns.

use super::super::{ProductSkipReason, TerminalNumeratorSymmetryEquations};
use super::*;
use crate::algebra::Coefficient;
use crate::persistence::{
    BinaryIoError, BinaryIoLimits, BinaryProgramKind, BinarySection, CoefficientId,
    CoefficientTableBuilder, DecodedCoefficientTable, SectionTag, encode_program, inspect_program,
};
use crate::sector::symmetry::CoefficientMatrix;

const SCHEMA: u32 = 1;
const PROVIDER_SCHEMA: u32 = 3;
const RECIPE: &str = "rustred.terminal-sector-symmetry.complete-integral-generator-orbits-v3";
const ROUND_TRIP_RECIPE: &str = "rustred.terminal-sector-symmetry.round-trip-v1";

#[derive(Clone, bincode::Encode, bincode::Decode)]
struct MatrixRecord {
    rows: u64,
    columns: u64,
    entries: Vec<u32>,
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
struct SupportRouteRecord {
    active: Vec<bool>,
    map: u64,
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
struct RootRecord {
    active: Vec<bool>,
    maps: Vec<u64>,
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
struct WitnessRecord {
    source: u64,
    maps: Vec<u64>,
    terms: Vec<(u64, u32)>,
    conditions: Vec<u32>,
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
struct Record {
    schema: u32,
    recipe: String,
    family: [u8; 32],
    policy: u8,
    keys: Vec<Vec<i64>>,
    inventory: Vec<Vec<i64>>,
    maps: Vec<[MatrixRecord; 3]>,
    routes: Vec<SupportRouteRecord>,
    roots: Vec<RootRecord>,
    witnesses: Vec<WitnessRecord>,
    legacy: Option<Vec<u8>>,
    statistics: [u64; 8],
    skipped: Vec<(u8, Option<u8>, u64)>,
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
struct RouteRecord {
    source_family: [u8; 32],
    target_family: [u8; 32],
    source_root: Vec<bool>,
    target_root: Vec<bool>,
    maps: [MatrixRecord; 3],
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
struct RoundTripRecord {
    schema: u32,
    recipe: String,
    routes: Vec<RouteRecord>,
    maps: [MatrixRecord; 3],
}

impl TerminalSectorSymmetryEquations {
    /// Retain exact maps once globally, then record each original source's
    /// complete-polynomial orbit and its single coalesced averaged equation.
    pub fn encode_native(&self, io: BinaryIoLimits) -> Result<Vec<u8>, BinaryIoError> {
        let mut table = CoefficientTableBuilder::new(io);
        let mut registry = MapRegistry::default();
        for map in self
            .routes
            .values()
            .chain(self.generators.values().flatten())
            .chain(
                self.witnesses
                    .values()
                    .flatten()
                    .flat_map(|witness| witness.verified_maps()),
            )
        {
            if map.source_family_fingerprint() != self.family_fingerprint()
                || map.target_family_fingerprint() != self.family_fingerprint()
            {
                return Err(invalid(
                    "foreign-family map in sector symmetry orbit provider",
                ));
            }
        }
        let inventory: Vec<_> = self.inventory.iter().collect();
        grant("sector symmetry input keys", self.input_keys.len(), io)?;
        grant("sector symmetry inventory keys", inventory.len(), io)?;
        grant("sector symmetry source routes", self.routes.len(), io)?;
        grant("sector symmetry canonical roots", self.generators.len(), io)?;
        if !self.input_keys.is_subset(&self.inventory)
            || self.witnesses.len() != self.input_keys.len()
        {
            return Err(invalid(
                "sector symmetry original orbit proofs differ from inventory",
            ));
        }
        for key in &inventory {
            grant("sector symmetry coordinate entries", key.powers().len(), io)?;
        }
        let routes = self
            .routes
            .iter()
            .map(|(root, map)| {
                grant("sector symmetry route coordinates", root.arity(), io)?;
                Ok(SupportRouteRecord {
                    active: root.active_bits().to_vec(),
                    map: registry.intern(map, io)?,
                })
            })
            .collect::<Result<Vec<_>, BinaryIoError>>()?;
        let roots = self
            .generators
            .iter()
            .map(|(root, maps)| {
                grant("sector symmetry root coordinates", root.arity(), io)?;
                grant("sector symmetry root generators", maps.len(), io)?;
                Ok(RootRecord {
                    active: root.active_bits().to_vec(),
                    maps: maps
                        .iter()
                        .map(|map| registry.intern(map, io))
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect::<Result<Vec<_>, BinaryIoError>>()?;
        let mut witnesses = Vec::new();
        let mut averaged_rows: BTreeMap<IntegralKey, Vec<TerminalEquation>> = BTreeMap::new();
        for (source_id, source) in self.input_keys.iter().enumerate() {
            let proofs = self
                .witnesses
                .get(source)
                .ok_or(invalid("missing sector symmetry source witnesses"))?;
            if proofs.is_empty()
                || (self.policy == TerminalSectorSymmetryPolicy::PolynomialOrbits
                    && proofs.len() != 1)
            {
                return Err(invalid(
                    "sector symmetry witness cohort differs from policy",
                ));
            }
            let root = Mask::try_from_indices(source.powers()).map_err(native)?;
            let route = self
                .routes
                .get(&root)
                .ok_or(invalid("missing sector symmetry original support route"))?;
            for (candidate, witness) in proofs.iter().enumerate() {
                let row = witness.equation();
                if !row.terms.is_empty() {
                    let rows = averaged_rows.entry(source.clone()).or_default();
                    if !rows.contains(row) {
                        rows.push(row.clone());
                    }
                }
                if witness.source() != source
                    || witness.family_fingerprint() != self.family_fingerprint()
                    || witness.verified_maps().is_empty()
                    || (self.policy == TerminalSectorSymmetryPolicy::GeneratorRelations
                        && witness.verified_maps().len() != 1)
                    || (candidate == 0 && witness.verified_maps()[0].momentum() != route.momentum())
                {
                    return Err(invalid(
                        "sector symmetry source witness differs from policy or route",
                    ));
                }
                grant(
                    "sector symmetry witness maps",
                    witness.verified_maps().len(),
                    io,
                )?;
                grant("sector symmetry row entries", row.terms.len(), io)?;
                grant(
                    "sector symmetry conditions",
                    row.nonzero_conditions.len(),
                    io,
                )?;
                let maps = witness
                    .verified_maps()
                    .iter()
                    .map(|map| registry.intern(map, io))
                    .collect::<Result<_, _>>()?;
                let terms = row
                    .terms
                    .iter()
                    .map(|(key, value)| {
                        if value.is_zero() {
                            return Err(invalid("sector symmetry equation contains explicit zero"));
                        }
                        let target = inventory
                            .binary_search(&key)
                            .map_err(|_| invalid("sector symmetry row endpoint lacks inventory"))?;
                        Ok((
                            u64::try_from(target).map_err(native)?,
                            index(table.intern(value)?)?,
                        ))
                    })
                    .collect::<Result<_, BinaryIoError>>()?;
                let conditions = row
                    .nonzero_conditions
                    .iter()
                    .map(|value| index(table.intern(value)?))
                    .collect::<Result<_, _>>()?;
                witnesses.push(WitnessRecord {
                    source: u64::try_from(source_id).map_err(native)?,
                    maps,
                    terms,
                    conditions,
                });
                grant("sector symmetry source witnesses", witnesses.len(), io)?;
            }
        }
        let expected_rows = merge_legacy_rows(
            self.family_fingerprint(),
            &self.input_keys,
            averaged_rows,
            self.legacy.as_deref(),
        )?;
        let expected_inventory = self
            .input_keys
            .iter()
            .cloned()
            .chain(
                expected_rows
                    .values()
                    .flatten()
                    .flat_map(|row| row.terms.keys().cloned()),
            )
            .collect::<BTreeSet<_>>();
        if self.rows != expected_rows || self.inventory != expected_inventory {
            return Err(invalid(
                "sector symmetry merged rows or inventory differ from proofs",
            ));
        }
        let maps = registry
            .maps
            .iter()
            .map(|map| encode_map(map.momentum(), &mut table, io))
            .collect::<Result<_, _>>()?;
        let record = Record {
            schema: PROVIDER_SCHEMA,
            recipe: RECIPE.into(),
            family: family_digest(self.family_fingerprint()),
            policy: encode_policy(self.policy),
            keys: self
                .input_keys
                .iter()
                .map(|key| key.powers().to_vec())
                .collect(),
            inventory: inventory.iter().map(|key| key.powers().to_vec()).collect(),
            maps,
            routes,
            roots,
            witnesses,
            legacy: self
                .legacy
                .as_ref()
                .map(|legacy| legacy.encode_native(io))
                .transpose()?,
            statistics: encode_statistics(&self.statistics)?,
            skipped: encode_skipped(
                &self.statistics.skipped,
                self.generators
                    .values()
                    .filter(|maps| maps.is_empty())
                    .count(),
                io,
            )?,
        };
        encode_record(&record, table.finish()?, io)
    }

    /// Verify shared maps and replay the selected original-source cohort.
    /// Polynomial-orbit policy additionally rederives full orbit closure;
    /// generator policy rederives each route and route-generator identity.
    /// Temporary image monomials never become new source seeds.
    pub fn from_native_bytes(
        bytes: &[u8],
        family: Arc<IntegralFamily>,
        limits: TerminalSectorSymmetryLimits,
        io: BinaryIoLimits,
    ) -> Result<Self, Error> {
        let envelope = inspect_program(bytes, io)?;
        if envelope.kind() != BinaryProgramKind::TerminalNormalization
            || envelope.sections().iter().map(|section| section.tag).ne([
                SectionTag::SYMBOLICA_STATE,
                SectionTag::COEFFICIENTS,
                SectionTag::PROGRAM,
            ])
        {
            return Err(invalid("not a sector symmetry orbit capsule").into());
        }
        // Generated-native payloads use caller byte/count grants, without an
        // unrelated fixed decoded-allocation ceiling.
        let payload = envelope
            .section(SectionTag::PROGRAM)
            .expect("checked section");
        let (record, consumed): (Record, usize) =
            bincode::decode_from_slice(payload, bincode::config::standard()).map_err(native)?;
        if consumed != payload.len() || record.schema != PROVIDER_SCHEMA || record.recipe != RECIPE
        {
            return Err(invalid("unsupported or trailing sector symmetry orbit records").into());
        }
        if record.family != family_digest(family.fingerprint()) {
            return Err(invalid("sector symmetry orbit family identity differs").into());
        }
        let policy = decode_policy(record.policy)?;
        let keys = decode_keys(record.keys, &family, "sector symmetry input keys", io)?;
        let inventory = decode_keys(
            record.inventory,
            &family,
            "sector symmetry inventory keys",
            io,
        )?;
        let input_keys: BTreeSet<_> = keys.iter().cloned().collect();
        let inventory_set: BTreeSet<_> = inventory.iter().cloned().collect();
        if !input_keys.is_subset(&inventory_set) {
            return Err(invalid("sector symmetry originals are absent from inventory").into());
        }
        grant("sector symmetry shared maps", record.maps.len(), io)?;
        grant("sector symmetry source routes", record.routes.len(), io)?;
        grant("sector symmetry canonical roots", record.roots.len(), io)?;
        grant(
            "sector symmetry original orbit witnesses",
            record.witnesses.len(),
            io,
        )?;
        if record.witnesses.len() < keys.len()
            || (policy == TerminalSectorSymmetryPolicy::PolynomialOrbits
                && record.witnesses.len() != keys.len())
        {
            return Err(
                invalid("sector symmetry orbit proof count differs from original keys").into(),
            );
        }
        let table = DecodedCoefficientTable::import_generated(
            envelope
                .section(SectionTag::SYMBOLICA_STATE)
                .expect("checked section"),
            envelope
                .section(SectionTag::COEFFICIENTS)
                .expect("checked section"),
            io,
        )?;
        for i in 0..table.len() {
            family
                .coefficient_context()
                .validate_with_limits(
                    table.coefficient(CoefficientId::try_from_index(i)?)?,
                    io.exact_algebra,
                )
                .map_err(native)?;
        }
        let mut used_coefficients = BTreeSet::new();
        let mut registry = MapRegistry::default();
        for record in record.maps {
            let momentum = decode_map(record, &table, &mut used_coefficients, limits, io)?;
            let verified = Arc::new(
                symmetry::verify(&family, &family, momentum, limits.verification)
                    .map_err(algebra)?,
            );
            let expected = registry.maps.len();
            let id = registry.intern(&verified, io)?;
            if usize::try_from(id).map_err(native)? != expected {
                return Err(invalid("duplicate sector symmetry shared momentum map").into());
            }
        }
        let mut used_maps = vec![false; registry.maps.len()];
        let mut routes = BTreeMap::new();
        let mut previous_root = None;
        for record in record.routes {
            let root = decode_route_root(record.active, &family, io)?;
            if previous_root
                .as_ref()
                .is_some_and(|previous| previous >= &root)
            {
                return Err(
                    invalid("sector symmetry source routes are not ordered uniquely").into(),
                );
            }
            previous_root = Some(root.clone());
            routes.insert(
                root,
                registered_map(record.map, &registry.maps, &mut used_maps)?,
            );
        }
        let original_roots = keys
            .iter()
            .map(|key| Mask::try_from_indices(key.powers()).map_err(native))
            .collect::<Result<BTreeSet<_>, _>>()?;
        if routes.keys().cloned().collect::<BTreeSet<_>>() != original_roots {
            return Err(
                invalid("sector symmetry source routes differ from original supports").into(),
            );
        }
        let mut generators = BTreeMap::new();
        previous_root = None;
        for record in record.roots {
            let root = decode_route_root(record.active, &family, io)?;
            if previous_root
                .as_ref()
                .is_some_and(|previous| previous >= &root)
            {
                return Err(
                    invalid("sector symmetry canonical roots are not ordered uniquely").into(),
                );
            }
            previous_root = Some(root.clone());
            grant("sector symmetry root generators", record.maps.len(), io)?;
            let mut ids = BTreeSet::new();
            let mut maps = Vec::new();
            for id in record.maps {
                if !ids.insert(id) {
                    return Err(invalid("duplicate sector symmetry canonical generator").into());
                }
                maps.push(registered_map(id, &registry.maps, &mut used_maps)?);
            }
            generators.insert(root, maps);
        }
        let mut rows: BTreeMap<IntegralKey, Vec<TerminalEquation>> = BTreeMap::new();
        let mut witnesses: BTreeMap<IntegralKey, Vec<(Vec<Arc<VerifiedMap>>, TerminalEquation)>> =
            BTreeMap::new();
        let mut previous_source = None;
        for record in record.witnesses {
            let source_id = usize::try_from(record.source).map_err(native)?;
            if previous_source.is_some_and(|previous| previous > source_id) {
                return Err(
                    invalid("sector symmetry orbit sources are not ordered completely").into(),
                );
            }
            previous_source = Some(source_id);
            let source = keys.get(source_id).ok_or(invalid(
                "sector symmetry source witness index out of bounds",
            ))?;
            let first = !witnesses.contains_key(source);
            if policy == TerminalSectorSymmetryPolicy::PolynomialOrbits && !first {
                return Err(invalid("duplicate polynomial-orbit source witness").into());
            }
            grant("sector symmetry orbit maps", record.maps.len(), io)?;
            if record.maps.is_empty()
                || (policy == TerminalSectorSymmetryPolicy::GeneratorRelations
                    && record.maps.len() != 1)
            {
                return Err(invalid("empty sector symmetry source orbit").into());
            }
            let mut ids = BTreeSet::new();
            let mut maps = Vec::new();
            for id in record.maps {
                if !ids.insert(id) {
                    return Err(invalid("duplicate sector symmetry source orbit map").into());
                }
                maps.push(registered_map(id, &registry.maps, &mut used_maps)?);
            }
            let root = Mask::try_from_indices(source.powers()).map_err(native)?;
            if first && maps[0].momentum() != routes[&root].momentum() {
                return Err(invalid(
                    "sector symmetry initial orbit map differs from support route",
                )
                .into());
            }
            grant("sector symmetry row entries", record.terms.len(), io)?;
            grant("sector symmetry conditions", record.conditions.len(), io)?;
            let mut terms = BTreeMap::new();
            let mut previous_target = None;
            for (target, id) in record.terms {
                let target = usize::try_from(target).map_err(native)?;
                if previous_target.is_some_and(|previous| previous >= target) {
                    return Err(
                        invalid("sector symmetry averaged row is not uniquely coalesced").into(),
                    );
                }
                previous_target = Some(target);
                let key = inventory.get(target).ok_or(invalid(
                    "sector symmetry averaged endpoint index out of bounds",
                ))?;
                let value = coefficient(&table, &mut used_coefficients, id)?;
                if value.is_zero() {
                    return Err(
                        invalid("sector symmetry averaged row contains explicit zero").into(),
                    );
                }
                terms.insert(key.clone(), value);
            }
            let nonzero_conditions = record
                .conditions
                .into_iter()
                .map(|id| coefficient(&table, &mut used_coefficients, id))
                .collect::<Result<_, _>>()?;
            let equation = TerminalEquation {
                terms,
                nonzero_conditions,
            };
            if closure::replay_verified_average(&family, source, &maps, limits)? != equation {
                return Err(invalid(
                    "sector symmetry averaged row or guards failed original-source map replay",
                )
                .into());
            }
            if !equation.terms.is_empty() {
                let target = rows.entry(source.clone()).or_default();
                if !target.contains(&equation) {
                    target.push(equation.clone());
                }
            }
            witnesses
                .entry(source.clone())
                .or_default()
                .push((maps, equation));
        }
        if witnesses.keys().cloned().collect::<BTreeSet<_>>() != input_keys {
            return Err(
                invalid("sector symmetry witness sources differ from original keys").into(),
            );
        }
        if used_maps.iter().any(|used| !used) {
            return Err(invalid("unused sector symmetry shared momentum maps").into());
        }
        if used_coefficients.len() != table.len() {
            return Err(invalid("unused sector symmetry coefficient records").into());
        }
        let legacy = record
            .legacy
            .map(|payload| {
                TerminalNumeratorSymmetryEquations::from_native_bytes(
                    &payload,
                    family.clone(),
                    limits,
                    io,
                )
                .map(Arc::new)
            })
            .transpose()?;
        let rows = merge_legacy_rows(family.fingerprint(), &input_keys, rows, legacy.as_deref())?;
        let skipped = decode_skipped(
            record.skipped,
            generators.values().filter(|maps| maps.is_empty()).count(),
            io,
        )?;
        let statistics = decode_statistics(record.statistics, skipped)?;
        let certified_routes = routes.clone();
        let certified_generators = generators.clone();
        let mut replayed = closure::prepare_with_orbit_supports_policy(
            family,
            &input_keys,
            routes,
            generators,
            limits,
            &TerminalSectorSymmetryControl::default(),
            policy,
        )?;
        if let Some(legacy) = legacy {
            closure::attach_legacy(&mut replayed, legacy)?;
        }
        // Unsupported-geometry counts are historical discovery metadata.
        replayed.statistics.skipped = statistics.skipped.clone();
        if replayed.policy != policy
            || replayed.inventory != inventory_set
            || replayed.rows != rows
            || replayed.statistics != statistics
            || !support_maps_equal(&replayed.routes, &certified_routes)
            || replayed.generators.len() != certified_generators.len()
            || replayed.generators.iter().any(|(root, maps)| {
                certified_generators
                    .get(root)
                    .is_none_or(|stored| !maps_equal(maps, stored))
            })
        {
            return Err(invalid(
                "sector symmetry averaged rows, supports, counts or inventory failed orbit replay",
            )
            .into());
        }
        if replayed.witnesses.len() != witnesses.len()
            || replayed.witnesses.iter().any(|(source, expected)| {
                let Some(stored) = witnesses.get(source) else {
                    return true;
                };
                expected.len() != stored.len()
                    || expected.iter().zip(stored).any(|(expected, (maps, row))| {
                        expected.source() != source
                            || expected.equation() != row
                            || !maps_equal(expected.verified_maps(), maps)
                    })
            })
        {
            return Err(invalid(
                "sector symmetry complete-integral witness cohort failed exact replay",
            )
            .into());
        }
        Ok(replayed)
    }
}

fn encode_policy(policy: TerminalSectorSymmetryPolicy) -> u8 {
    match policy {
        TerminalSectorSymmetryPolicy::GeneratorRelations => 0,
        TerminalSectorSymmetryPolicy::PolynomialOrbits => 1,
    }
}

fn decode_policy(code: u8) -> Result<TerminalSectorSymmetryPolicy, BinaryIoError> {
    match code {
        0 => Ok(TerminalSectorSymmetryPolicy::GeneratorRelations),
        1 => Ok(TerminalSectorSymmetryPolicy::PolynomialOrbits),
        _ => Err(invalid("unknown sector symmetry relation policy")),
    }
}

fn merge_legacy_rows(
    family_fingerprint: &str,
    keys: &BTreeSet<IntegralKey>,
    mut rows: BTreeMap<IntegralKey, Vec<TerminalEquation>>,
    legacy: Option<&TerminalNumeratorSymmetryEquations>,
) -> Result<BTreeMap<IntegralKey, Vec<TerminalEquation>>, BinaryIoError> {
    if let Some(legacy) = legacy {
        if legacy.family_fingerprint() != family_fingerprint || legacy.input_keys() != keys {
            return Err(invalid(
                "sector symmetry legacy family or original input inventory differs",
            ));
        }
        for source in keys {
            for row in legacy.equations(source) {
                if row.terms.is_empty() {
                    continue;
                }
                let target = rows.entry(source.clone()).or_default();
                if !target.contains(row) {
                    target.push(row.clone());
                }
            }
        }
    }
    Ok(rows)
}

#[derive(Default)]
struct MapRegistry {
    maps: Vec<Arc<VerifiedMap>>,
    by_hash: BTreeMap<u64, Vec<usize>>,
}

impl MapRegistry {
    fn intern(&mut self, map: &Arc<VerifiedMap>, io: BinaryIoLimits) -> Result<u64, BinaryIoError> {
        use std::hash::{Hash, Hasher};
        if self.maps.first().is_some_and(|first| {
            first.source_family_fingerprint() != map.source_family_fingerprint()
                || first.target_family_fingerprint() != map.target_family_fingerprint()
        }) {
            return Err(invalid(
                "foreign-family map in sector symmetry shared registry",
            ));
        }
        let mut hasher = std::hash::DefaultHasher::new();
        for matrix in [
            map.momentum().loop_linear(),
            map.momentum().loop_external(),
            map.momentum().external_linear(),
        ] {
            matrix.rows().hash(&mut hasher);
            matrix.columns().hash(&mut hasher);
            matrix.entries().hash(&mut hasher);
        }
        let hash = hasher.finish();
        if let Some(indices) = self.by_hash.get(&hash) {
            for &index in indices {
                if self.maps[index].momentum() == map.momentum() {
                    return u64::try_from(index).map_err(native);
                }
            }
        }
        let index = self.maps.len();
        grant(
            "sector symmetry shared maps",
            index
                .checked_add(1)
                .ok_or(invalid("sector symmetry map count overflow"))?,
            io,
        )?;
        self.maps.push(map.clone());
        self.by_hash.entry(hash).or_default().push(index);
        u64::try_from(index).map_err(native)
    }
}

fn registered_map(
    id: u64,
    maps: &[Arc<VerifiedMap>],
    used: &mut [bool],
) -> Result<Arc<VerifiedMap>, BinaryIoError> {
    let id = usize::try_from(id).map_err(native)?;
    let map = maps.get(id).ok_or(invalid(
        "sector symmetry momentum-map reference out of bounds",
    ))?;
    used[id] = true;
    Ok(map.clone())
}

fn maps_equal(left: &[Arc<VerifiedMap>], right: &[Arc<VerifiedMap>]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.momentum() == right.momentum())
}

fn support_maps_equal(
    left: &BTreeMap<Mask, Arc<VerifiedMap>>,
    right: &BTreeMap<Mask, Arc<VerifiedMap>>,
) -> bool {
    left.len() == right.len()
        && left.iter().all(|(root, map)| {
            right
                .get(root)
                .is_some_and(|other| map.momentum() == other.momentum())
        })
}

impl TerminalSectorSymmetryRoundTrip {
    /// Retain every family-qualified route and the independently verified
    /// common-family composition in a separate native proof capsule.
    pub fn encode_native(&self, io: BinaryIoLimits) -> Result<Vec<u8>, BinaryIoError> {
        grant("sector symmetry round-trip routes", self.routes.len(), io)?;
        let first = self
            .routes
            .first()
            .ok_or(invalid("empty sector symmetry round trip"))?;
        if self.map.source_family_fingerprint() != first.source_family_fingerprint()
            || self.map.target_family_fingerprint() != first.source_family_fingerprint()
        {
            return Err(invalid(
                "sector symmetry round-trip composition has foreign owner",
            ));
        }
        let mut table = CoefficientTableBuilder::new(io);
        let routes = self
            .routes
            .iter()
            .map(|route| {
                grant(
                    "sector symmetry route source coordinates",
                    route.source_root().arity(),
                    io,
                )?;
                grant(
                    "sector symmetry route target coordinates",
                    route.target_root().arity(),
                    io,
                )?;
                if route.verified_map().source_family_fingerprint()
                    != route.source_family_fingerprint()
                    || route.verified_map().target_family_fingerprint()
                        != route.target_family_fingerprint()
                {
                    return Err(invalid("sector symmetry route map has foreign owner"));
                }
                Ok(RouteRecord {
                    source_family: family_digest(route.source_family_fingerprint()),
                    target_family: family_digest(route.target_family_fingerprint()),
                    source_root: route.source_root().active_bits().to_vec(),
                    target_root: route.target_root().active_bits().to_vec(),
                    maps: encode_map(route.verified_map().momentum(), &mut table, io)?,
                })
            })
            .collect::<Result<Vec<_>, BinaryIoError>>()?;
        let record = RoundTripRecord {
            schema: SCHEMA,
            recipe: ROUND_TRIP_RECIPE.into(),
            routes,
            maps: encode_map(self.map.momentum(), &mut table, io)?,
        };
        encode_record(&record, table.finish()?, io)
    }

    /// Replay every route against both caller-supplied owning families, then
    /// recompose and verify the final common-family momentum map. Registry
    /// membership supplies identity only; exact verification supplies proof.
    pub fn from_native_bytes(
        bytes: &[u8],
        families: &[Arc<IntegralFamily>],
        limits: TerminalSectorSymmetryLimits,
        io: BinaryIoLimits,
    ) -> Result<Self, Error> {
        let envelope = inspect_program(bytes, io)?;
        if envelope.kind() != BinaryProgramKind::TerminalNormalization
            || envelope.sections().iter().map(|section| section.tag).ne([
                SectionTag::SYMBOLICA_STATE,
                SectionTag::COEFFICIENTS,
                SectionTag::PROGRAM,
            ])
        {
            return Err(invalid("not a sector symmetry round-trip capsule").into());
        }
        let payload = envelope
            .section(SectionTag::PROGRAM)
            .expect("checked section");
        let (record, consumed): (RoundTripRecord, usize) =
            bincode::decode_from_slice(payload, bincode::config::standard()).map_err(native)?;
        if consumed != payload.len()
            || record.schema != SCHEMA
            || record.recipe != ROUND_TRIP_RECIPE
            || record.routes.is_empty()
        {
            return Err(
                invalid("unsupported, empty or trailing sector symmetry round trip").into(),
            );
        }
        grant("sector symmetry round-trip families", families.len(), io)?;
        grant("sector symmetry round-trip routes", record.routes.len(), io)?;
        let mut registry: BTreeMap<[u8; 32], Arc<IntegralFamily>> = BTreeMap::new();
        for family in families {
            let digest = family_digest(family.fingerprint());
            if registry
                .get(&digest)
                .is_some_and(|old| old.fingerprint() != family.fingerprint())
            {
                return Err(invalid("ambiguous sector symmetry route family identity").into());
            }
            registry.insert(digest, family.clone());
        }
        let table = DecodedCoefficientTable::import_generated(
            envelope
                .section(SectionTag::SYMBOLICA_STATE)
                .expect("checked section"),
            envelope
                .section(SectionTag::COEFFICIENTS)
                .expect("checked section"),
            io,
        )?;
        let mut used = BTreeSet::new();
        let mut routes = Vec::new();
        for record in record.routes {
            let source = registry
                .get(&record.source_family)
                .ok_or(invalid("missing sector symmetry route source family"))?
                .clone();
            let target = registry
                .get(&record.target_family)
                .ok_or(invalid("missing sector symmetry route target family"))?
                .clone();
            let source_root = decode_route_root(record.source_root, &source, io)?;
            let target_root = decode_route_root(record.target_root, &target, io)?;
            let map = decode_map(record.maps, &table, &mut used, limits, io)?;
            validate_map_coefficients(&map, &source, io)?;
            routes.push(Arc::new(TerminalSectorSymmetryRoute::verify(
                source,
                target,
                source_root,
                target_root,
                map,
                limits,
            )?));
        }
        let final_map = decode_map(record.maps, &table, &mut used, limits, io)?;
        validate_map_coefficients(&final_map, &routes[0].source_family, io)?;
        if used.len() != table.len() {
            return Err(invalid("unused sector symmetry round-trip coefficient records").into());
        }
        let replayed = TerminalSectorSymmetryRoute::verify_round_trip(&routes, limits)?;
        if replayed.verified_map().momentum() != &final_map {
            return Err(
                invalid("sector symmetry round-trip composition failed exact replay").into(),
            );
        }
        Ok(replayed)
    }
}

fn family_digest(fingerprint: &str) -> [u8; 32] {
    *blake3::hash(fingerprint.as_bytes()).as_bytes()
}

fn decode_route_root(
    active: Vec<bool>,
    family: &IntegralFamily,
    io: BinaryIoLimits,
) -> Result<Mask, Error> {
    grant("sector symmetry route coordinates", active.len(), io)?;
    if active.len() != family.denominator_count() {
        return Err(invalid("sector symmetry route root has wrong owner arity").into());
    }
    Ok(Mask::try_new(active).map_err(native)?)
}

fn validate_map_coefficients(
    map: &MomentumMap,
    family: &IntegralFamily,
    io: BinaryIoLimits,
) -> Result<(), BinaryIoError> {
    for matrix in [
        map.loop_linear(),
        map.loop_external(),
        map.external_linear(),
    ] {
        for value in matrix.entries() {
            family
                .coefficient_context()
                .validate_with_limits(value, io.exact_algebra)
                .map_err(native)?;
        }
    }
    Ok(())
}

fn invalid(message: &'static str) -> BinaryIoError {
    BinaryIoError::Invalid(message)
}

fn native(error: impl std::fmt::Display) -> BinaryIoError {
    BinaryIoError::Native(error.to_string())
}

fn grant(
    resource: &'static str,
    requested: usize,
    io: BinaryIoLimits,
) -> Result<(), BinaryIoError> {
    if requested > io.max_collection_entries {
        Err(BinaryIoError::Limit {
            resource,
            requested,
            limit: io.max_collection_entries,
        })
    } else {
        Ok(())
    }
}

fn index(id: CoefficientId) -> Result<u32, BinaryIoError> {
    u32::try_from(id.index()).map_err(native)
}

fn coefficient(
    table: &DecodedCoefficientTable,
    used: &mut BTreeSet<u32>,
    id: u32,
) -> Result<Coefficient, BinaryIoError> {
    let value = table
        .coefficient(CoefficientId::try_from_index(id as usize)?)?
        .clone();
    used.insert(id);
    Ok(value)
}

fn decode_keys(
    records: Vec<Vec<i64>>,
    family: &IntegralFamily,
    resource: &'static str,
    io: BinaryIoLimits,
) -> Result<Vec<IntegralKey>, Error> {
    grant(resource, records.len(), io)?;
    let mut keys = Vec::with_capacity(records.len());
    for powers in records {
        grant("sector symmetry coordinate entries", powers.len(), io)?;
        let key = IntegralKey::try_new(powers).map_err(native)?;
        validate_key(family, &key)?;
        if keys.last().is_some_and(|previous| previous >= &key) {
            return Err(invalid("sector symmetry keys are not ordered uniquely").into());
        }
        keys.push(key);
    }
    Ok(keys)
}

fn encode_map(
    map: &MomentumMap,
    table: &mut CoefficientTableBuilder,
    io: BinaryIoLimits,
) -> Result<[MatrixRecord; 3], BinaryIoError> {
    Ok([
        encode_matrix(map.loop_linear(), table, io)?,
        encode_matrix(map.loop_external(), table, io)?,
        encode_matrix(map.external_linear(), table, io)?,
    ])
}

fn decode_map(
    records: [MatrixRecord; 3],
    table: &DecodedCoefficientTable,
    used: &mut BTreeSet<u32>,
    limits: TerminalSectorSymmetryLimits,
    io: BinaryIoLimits,
) -> Result<MomentumMap, BinaryIoError> {
    let [a, b, c] = records;
    Ok(MomentumMap::new(
        decode_matrix(a, table, used, limits, io)?,
        decode_matrix(b, table, used, limits, io)?,
        decode_matrix(c, table, used, limits, io)?,
    ))
}

fn encode_matrix(
    matrix: &CoefficientMatrix,
    table: &mut CoefficientTableBuilder,
    io: BinaryIoLimits,
) -> Result<MatrixRecord, BinaryIoError> {
    grant("sector symmetry matrix entries", matrix.entries().len(), io)?;
    Ok(MatrixRecord {
        rows: u64::try_from(matrix.rows()).map_err(native)?,
        columns: u64::try_from(matrix.columns()).map_err(native)?,
        entries: matrix
            .entries()
            .iter()
            .map(|value| index(table.intern(value)?))
            .collect::<Result<_, _>>()?,
    })
}

fn decode_matrix(
    record: MatrixRecord,
    table: &DecodedCoefficientTable,
    used: &mut BTreeSet<u32>,
    limits: TerminalSectorSymmetryLimits,
    io: BinaryIoLimits,
) -> Result<CoefficientMatrix, BinaryIoError> {
    grant("sector symmetry matrix entries", record.entries.len(), io)?;
    let entries = record
        .entries
        .into_iter()
        .map(|id| coefficient(table, used, id))
        .collect::<Result<Vec<_>, _>>()?;
    CoefficientMatrix::try_new_with_max_entries(
        usize::try_from(record.rows).map_err(native)?,
        usize::try_from(record.columns).map_err(native)?,
        entries,
        limits.verification.max_matrix_entries,
    )
    .map_err(native)
}

fn encode_record(
    record: &impl bincode::Encode,
    table: crate::persistence::EncodedCoefficientTable,
    io: BinaryIoLimits,
) -> Result<Vec<u8>, BinaryIoError> {
    let program = bincode::encode_to_vec(record, bincode::config::standard()).map_err(native)?;
    encode_program(
        BinaryProgramKind::TerminalNormalization,
        &[
            BinarySection {
                tag: SectionTag::SYMBOLICA_STATE,
                bytes: &table.state,
            },
            BinarySection {
                tag: SectionTag::COEFFICIENTS,
                bytes: &table.atoms,
            },
            BinarySection {
                tag: SectionTag::PROGRAM,
                bytes: &program,
            },
        ],
        io,
    )
}

fn encode_statistics(
    statistics: &TerminalSectorSymmetryStatistics,
) -> Result<[u64; 8], BinaryIoError> {
    let values = [
        statistics.input_keys,
        statistics.closure_keys,
        statistics.analyzed_supports,
        statistics.verified_generators,
        statistics.equation_targets,
        statistics.equations,
        statistics.output_terms,
        statistics.trivial_equations,
    ];
    let mut result = [0; 8];
    for (target, value) in result.iter_mut().zip(values) {
        *target = u64::try_from(value).map_err(native)?;
    }
    Ok(result)
}

fn decode_statistics(
    record: [u64; 8],
    skipped: BTreeMap<TerminalSectorSymmetrySkipReason, usize>,
) -> Result<TerminalSectorSymmetryStatistics, BinaryIoError> {
    let mut counts = [0; 8];
    for (target, value) in counts.iter_mut().zip(record) {
        *target = usize::try_from(value).map_err(native)?;
    }
    Ok(TerminalSectorSymmetryStatistics {
        input_keys: counts[0],
        closure_keys: counts[1],
        analyzed_supports: counts[2],
        verified_generators: counts[3],
        equation_targets: counts[4],
        equations: counts[5],
        output_terms: counts[6],
        trivial_equations: counts[7],
        skipped,
    })
}

fn encode_skipped(
    skipped: &BTreeMap<TerminalSectorSymmetrySkipReason, usize>,
    empty_roots: usize,
    io: BinaryIoLimits,
) -> Result<Vec<(u8, Option<u8>, u64)>, BinaryIoError> {
    grant("sector symmetry diagnostic reasons", skipped.len(), io)?;
    validate_skipped(skipped, empty_roots)?;
    skipped
        .iter()
        .map(|(reason, count)| {
            let (kind, geometry) = match reason {
                TerminalSectorSymmetrySkipReason::RankDeficientSupport => (0, None),
                TerminalSectorSymmetrySkipReason::MomentumGeometry(reason) => {
                    (1, Some(geometry_code(*reason)))
                }
            };
            Ok((kind, geometry, u64::try_from(*count).map_err(native)?))
        })
        .collect()
}

fn decode_skipped(
    records: Vec<(u8, Option<u8>, u64)>,
    empty_roots: usize,
    io: BinaryIoLimits,
) -> Result<BTreeMap<TerminalSectorSymmetrySkipReason, usize>, BinaryIoError> {
    grant("sector symmetry diagnostic reasons", records.len(), io)?;
    let mut skipped = BTreeMap::new();
    for (kind, geometry, count) in records {
        let reason = match (kind, geometry) {
            (0, None) => TerminalSectorSymmetrySkipReason::RankDeficientSupport,
            (1, Some(code)) => {
                TerminalSectorSymmetrySkipReason::MomentumGeometry(geometry_reason(code)?)
            }
            _ => return Err(invalid("unknown sector symmetry diagnostic reason")),
        };
        if skipped
            .insert(reason, usize::try_from(count).map_err(native)?)
            .is_some()
        {
            return Err(invalid("duplicate sector symmetry diagnostic reason"));
        }
    }
    validate_skipped(&skipped, empty_roots)?;
    Ok(skipped)
}

fn validate_skipped(
    skipped: &BTreeMap<TerminalSectorSymmetrySkipReason, usize>,
    empty_roots: usize,
) -> Result<(), BinaryIoError> {
    let mut total = 0usize;
    for &count in skipped.values() {
        if count == 0 {
            return Err(invalid("zero sector symmetry diagnostic count"));
        }
        total = total
            .checked_add(count)
            .ok_or(invalid("sector symmetry diagnostic count overflow"))?;
    }
    if total > empty_roots {
        return Err(invalid(
            "sector symmetry diagnostics exceed empty-generator roots",
        ));
    }
    Ok(())
}

fn geometry_code(reason: ProductSkipReason) -> u8 {
    use ProductSkipReason::*;
    match reason {
        ExternalMomenta => 0,
        AnalyticPowerShifts => 1,
        NumeratorPowers => 2,
        ActiveLineCount => 3,
        NonUnitMass => 4,
        NonIntegerQuadratic => 5,
        NotLinearSquare => 6,
        SingularMomentumBasis => 7,
        NonUnimodularMomentumBasis => 8,
        ConditionalMomentumMap => 9,
        NonIntegralMomentumMap => 10,
        NonUnimodularMomentumMap => 11,
        ParametricPreparationLimit => 12,
    }
}

fn geometry_reason(code: u8) -> Result<ProductSkipReason, BinaryIoError> {
    use ProductSkipReason::*;
    Ok(match code {
        0 => ExternalMomenta,
        1 => AnalyticPowerShifts,
        2 => NumeratorPowers,
        3 => ActiveLineCount,
        4 => NonUnitMass,
        5 => NonIntegerQuadratic,
        6 => NotLinearSquare,
        7 => SingularMomentumBasis,
        8 => NonUnimodularMomentumBasis,
        9 => ConditionalMomentumMap,
        10 => NonIntegralMomentumMap,
        11 => NonUnimodularMomentumMap,
        12 => ParametricPreparationLimit,
        _ => return Err(invalid("unknown sector symmetry geometry diagnostic")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::family::AffineDenominator;

    fn family(mass: i64) -> Arc<IntegralFamily> {
        family_named("native_sector_symmetry", mass)
    }

    fn family_named(name: &str, mass: i64) -> Arc<IntegralFamily> {
        let context = CoefficientContext::new(["d"]);
        Arc::new(
            IntegralFamily::new(
                name,
                vec!["k".into(), "l".into()],
                vec![],
                context.clone(),
                context.parameter("d").unwrap(),
                [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
                    .into_iter()
                    .map(|row| {
                        AffineDenominator::new(
                            context.integer(-mass),
                            row.map(|value| context.integer(value)).into(),
                        )
                    })
                    .collect(),
                vec![],
                vec![context.zero(); 3],
            )
            .unwrap(),
        )
    }

    fn fixture() -> (Arc<IntegralFamily>, Vec<u8>) {
        fixture_with_generator(true)
    }

    fn fixture_with_generator(with_generator: bool) -> (Arc<IntegralFamily>, Vec<u8>) {
        fixture_with_policy(
            with_generator,
            TerminalSectorSymmetryPolicy::PolynomialOrbits,
        )
    }

    fn fixture_with_policy(
        with_generator: bool,
        policy: TerminalSectorSymmetryPolicy,
    ) -> (Arc<IntegralFamily>, Vec<u8>) {
        let family = family(1);
        let keys = BTreeSet::from([
            IntegralKey::try_new([2, 1, -1]).unwrap(),
            IntegralKey::try_new([1, 2, -1]).unwrap(),
        ]);
        let root = Mask::try_new([true, true, false]).unwrap();
        let map = |entries: [i64; 4]| {
            Arc::new(
                symmetry::verify(
                    &family,
                    &family,
                    MomentumMap::new(
                        CoefficientMatrix::try_new(
                            2,
                            2,
                            entries.map(|value| family.coefficient_context().integer(value)),
                        )
                        .unwrap(),
                        CoefficientMatrix::try_new(2, 0, []).unwrap(),
                        CoefficientMatrix::try_new(0, 0, []).unwrap(),
                    ),
                    Default::default(),
                )
                .unwrap(),
            )
        };
        let identity = map([1, 0, 0, 1]);
        let generators = if with_generator {
            vec![map([-1, 0, 0, 1])]
        } else {
            Vec::new()
        };
        let plan = closure::prepare_with_orbit_supports_policy(
            family.clone(),
            &keys,
            BTreeMap::from([(root.clone(), identity)]),
            BTreeMap::from([(root, generators)]),
            Default::default(),
            &TerminalSectorSymmetryControl::default(),
            policy,
        )
        .unwrap();
        assert_eq!(plan.input_keys(), &keys);
        if with_generator {
            assert!(plan.inventory().len() > keys.len());
        }
        (
            family,
            plan.encode_native(BinaryIoLimits::unbounded_resources())
                .unwrap(),
        )
    }

    fn unpack(bytes: &[u8]) -> (Record, crate::persistence::EncodedCoefficientTable) {
        let envelope = inspect_program(bytes, BinaryIoLimits::unbounded_resources()).unwrap();
        let (record, consumed): (Record, usize) = bincode::decode_from_slice(
            envelope.section(SectionTag::PROGRAM).unwrap(),
            bincode::config::standard(),
        )
        .unwrap();
        assert_eq!(
            consumed,
            envelope.section(SectionTag::PROGRAM).unwrap().len()
        );
        (
            record,
            crate::persistence::EncodedCoefficientTable {
                state: envelope
                    .section(SectionTag::SYMBOLICA_STATE)
                    .unwrap()
                    .to_vec(),
                atoms: envelope.section(SectionTag::COEFFICIENTS).unwrap().to_vec(),
            },
        )
    }

    fn zero_id(table: &crate::persistence::EncodedCoefficientTable) -> u32 {
        let decoded = DecodedCoefficientTable::import_generated(
            &table.state,
            &table.atoms,
            BinaryIoLimits::unbounded_resources(),
        )
        .unwrap();
        u32::try_from(
            (0..decoded.len())
                .find(|&id| {
                    decoded
                        .coefficient(CoefficientId::try_from_index(id).unwrap())
                        .unwrap()
                        .is_zero()
                })
                .unwrap(),
        )
        .unwrap()
    }

    fn reject(
        record: &Record,
        table: crate::persistence::EncodedCoefficientTable,
        family: Arc<IntegralFamily>,
    ) {
        let bytes = encode_record(record, table, BinaryIoLimits::unbounded_resources()).unwrap();
        assert!(
            TerminalSectorSymmetryEquations::from_native_bytes(
                &bytes,
                family,
                Default::default(),
                BinaryIoLimits::unbounded_resources()
            )
            .is_err()
        );
    }

    #[test]
    fn native_averaged_orbits_share_maps_and_replay_only_original_sources() {
        let (family, bytes) = fixture();
        let (record, _) = unpack(&bytes);
        assert_eq!(record.schema, PROVIDER_SCHEMA);
        assert_eq!(record.maps.len(), 2);
        assert_eq!(record.witnesses.len(), record.keys.len());
        assert!(
            record
                .witnesses
                .iter()
                .all(|witness| witness.maps.len() == 2)
        );
        let decoded = TerminalSectorSymmetryEquations::from_native_bytes(
            &bytes,
            family,
            Default::default(),
            BinaryIoLimits::unbounded_resources(),
        )
        .unwrap();
        assert!(decoded.legacy.is_none());
        assert!(decoded.inventory().len() > decoded.input_keys().len());
        for key in decoded.input_keys() {
            assert_eq!(decoded.witnesses(key).len(), 1);
            assert_eq!(decoded.witnesses(key)[0].verified_maps().len(), 2);
            assert_eq!(decoded.equations(key).len(), 1);
        }
        for key in decoded.inventory().difference(decoded.input_keys()) {
            assert!(decoded.equations(key).is_empty());
            assert!(decoded.witnesses(key).is_empty());
        }
        assert_eq!(
            decoded
                .encode_native(BinaryIoLimits::unbounded_resources())
                .unwrap(),
            bytes
        );
    }

    #[test]
    fn native_trivial_orbits_still_require_original_source_proofs() {
        let (family, bytes) = fixture_with_generator(false);
        let decoded = TerminalSectorSymmetryEquations::from_native_bytes(
            &bytes,
            family,
            Default::default(),
            BinaryIoLimits::unbounded_resources(),
        )
        .unwrap();
        assert_eq!(decoded.inventory(), decoded.input_keys());
        assert_eq!(decoded.statistics().equations, 0);
        for source in decoded.input_keys() {
            assert_eq!(decoded.witnesses(source).len(), 1);
            assert!(decoded.witnesses(source)[0].equation().terms.is_empty());
            assert!(decoded.equations(source).is_empty());
        }
    }

    #[test]
    fn native_generator_cohort_replays_single_complete_integral_maps() {
        let (family, bytes) =
            fixture_with_policy(true, TerminalSectorSymmetryPolicy::GeneratorRelations);
        let (record, _) = unpack(&bytes);
        assert_eq!(record.policy, 0);
        assert_eq!(record.witnesses.len(), 2 * record.keys.len());
        assert!(
            record
                .witnesses
                .iter()
                .all(|witness| witness.maps.len() == 1)
        );
        assert!(
            record
                .witnesses
                .iter()
                .filter(|witness| witness.terms.is_empty())
                .count()
                >= record.keys.len()
        );
        let decoded = TerminalSectorSymmetryEquations::from_native_bytes(
            &bytes,
            family,
            Default::default(),
            BinaryIoLimits::unbounded_resources(),
        )
        .unwrap();
        assert_eq!(
            decoded.policy(),
            TerminalSectorSymmetryPolicy::GeneratorRelations
        );
        for source in decoded.input_keys() {
            assert_eq!(decoded.witnesses(source).len(), 2);
            assert!(decoded.witnesses(source)[0].equation().terms.is_empty());
            assert!(!decoded.witnesses(source)[1].equation().terms.is_empty());
            assert_eq!(decoded.equations(source).len(), 1);
        }
        assert_eq!(
            decoded
                .encode_native(BinaryIoLimits::unbounded_resources())
                .unwrap(),
            bytes
        );
    }

    #[test]
    fn native_generator_cohort_rejects_changed_policy_and_omitted_candidate() {
        let (family, bytes) =
            fixture_with_policy(true, TerminalSectorSymmetryPolicy::GeneratorRelations);
        for policy in [1, 255] {
            let (mut record, table) = unpack(&bytes);
            record.policy = policy;
            reject(&record, table, family.clone());
        }
        let (mut record, table) = unpack(&bytes);
        let omitted = record.witnesses.remove(1);
        record.statistics[4] -= 1;
        record.statistics[5] -= 1;
        record.statistics[6] -= u64::try_from(omitted.terms.len()).unwrap();
        reject(&record, table, family);
    }

    #[test]
    fn generator_policy_honors_cancellation_before_candidate_work() {
        let family = family(1);
        let cancelled = || true;
        assert!(
            TerminalSectorSymmetryEquations::prepare_with_policy_and_control(
                family,
                &BTreeSet::from([IntegralKey::try_new([2, 1, -1]).unwrap()]),
                Default::default(),
                TerminalSectorSymmetryPolicy::GeneratorRelations,
                &TerminalSectorSymmetryControl {
                    cancelled: Some(&cancelled),
                    progress: None
                },
            )
            .is_err()
        );
    }

    fn legacy_fixture() -> (Arc<IntegralFamily>, Vec<u8>) {
        let (family, bytes) = fixture();
        let mut plan = TerminalSectorSymmetryEquations::from_native_bytes(
            &bytes,
            family.clone(),
            Default::default(),
            BinaryIoLimits::unbounded_resources(),
        )
        .unwrap();
        assert!(plan.legacy.is_none());
        let legacy = Arc::new(
            TerminalNumeratorSymmetryEquations::prepare(
                family.clone(),
                plan.input_keys(),
                Default::default(),
            )
            .unwrap(),
        );
        closure::attach_legacy(&mut plan, legacy).unwrap();
        assert!(
            plan.input_keys()
                .iter()
                .any(|source| plan.equations(source).len() > 1)
        );
        (
            family,
            plan.encode_native(BinaryIoLimits::unbounded_resources())
                .unwrap(),
        )
    }

    #[test]
    fn native_combines_replayed_legacy_rows_with_original_orbit_averages() {
        let (family, bytes) = legacy_fixture();
        let (record, _) = unpack(&bytes);
        assert!(record.legacy.is_some());
        let decoded = TerminalSectorSymmetryEquations::from_native_bytes(
            &bytes,
            family,
            Default::default(),
            BinaryIoLimits::unbounded_resources(),
        )
        .unwrap();
        assert!(decoded.legacy.is_some());
        assert!(
            decoded
                .input_keys()
                .iter()
                .any(|source| decoded.equations(source).len() > 1)
        );
        assert!(
            decoded
                .input_keys()
                .iter()
                .all(|source| decoded.witnesses(source).len() == 1)
        );
        assert_eq!(
            decoded
                .encode_native(BinaryIoLimits::unbounded_resources())
                .unwrap(),
            bytes
        );
    }

    #[test]
    fn native_rejects_missing_foreign_or_wrong_input_legacy_supplements() {
        let (original, bytes) = legacy_fixture();
        let (mut record, table) = unpack(&bytes);
        record.legacy = None;
        reject(&record, table, original.clone());
        for foreign in [true, false] {
            let (mut record, table) = unpack(&bytes);
            let owner = if foreign { family(2) } else { original.clone() };
            let keys = if foreign {
                record
                    .keys
                    .iter()
                    .cloned()
                    .map(|powers| IntegralKey::try_new(powers).unwrap())
                    .collect()
            } else {
                BTreeSet::from([IntegralKey::try_new([1, 1, -1]).unwrap()])
            };
            let legacy =
                TerminalNumeratorSymmetryEquations::prepare(owner, &keys, Default::default())
                    .unwrap();
            record.legacy = Some(
                legacy
                    .encode_native(BinaryIoLimits::unbounded_resources())
                    .unwrap(),
            );
            reject(&record, table, original.clone());
        }
    }

    #[test]
    fn refreshed_envelope_rejects_changed_shared_maps_rows_guards_and_references() {
        let (family, bytes) = fixture();
        for mutation in 0..4 {
            let (mut record, table) = unpack(&bytes);
            match mutation {
                0 => record.maps[0][0].entries.fill(zero_id(&table)),
                1 => record.witnesses[0].terms[0].1 = zero_id(&table),
                2 => record.witnesses[0].conditions.push(zero_id(&table)),
                _ => record.witnesses[0].maps[0] = u64::MAX,
            }
            reject(&record, table, family.clone());
        }
    }

    #[test]
    fn refreshed_envelope_rejects_unproved_inventory_and_changed_originals() {
        let (family, bytes) = fixture();
        let (mut record, table) = unpack(&bytes);
        record.inventory.push(vec![99, 99, 99]);
        record.statistics[1] += 1;
        reject(&record, table, family.clone());
        let (mut record, table) = unpack(&bytes);
        record.keys[0][0] += 1;
        reject(&record, table, family);
    }

    #[test]
    fn native_rejects_omitted_source_proofs_and_nonclosed_recorded_orbits() {
        let (family, bytes) = fixture();
        let (mut record, table) = unpack(&bytes);
        record.witnesses.pop();
        reject(&record, table, family.clone());
        let (mut record, table) = unpack(&bytes);
        // Give the first source a valid trivial average over its identity
        // image, and make rows, counts and inventory agree with that claim.
        // Only replay under the retained generator exposes the missing image.
        let removed_terms = record.witnesses[0].terms.len();
        record.witnesses[0].maps.truncate(1);
        record.witnesses[0].terms.clear();
        record.statistics[4] -= 1;
        record.statistics[5] -= 1;
        record.statistics[6] -= u64::try_from(removed_terms).unwrap();
        record.statistics[7] += 1;
        let mut retained: BTreeSet<usize> = record
            .keys
            .iter()
            .map(|key| record.inventory.binary_search(key).unwrap())
            .collect();
        retained.extend(record.witnesses.iter().flat_map(|witness| {
            witness
                .terms
                .iter()
                .map(|(target, _)| usize::try_from(*target).unwrap())
        }));
        let remap: BTreeMap<_, _> = retained
            .iter()
            .enumerate()
            .map(|(new, &old)| (old, new))
            .collect();
        record.inventory = record
            .inventory
            .into_iter()
            .enumerate()
            .filter_map(|(index, key)| retained.contains(&index).then_some(key))
            .collect();
        for witness in &mut record.witnesses {
            for (target, _) in &mut witness.terms {
                *target = u64::try_from(remap[&usize::try_from(*target).unwrap()]).unwrap();
            }
        }
        record.statistics[1] = u64::try_from(record.inventory.len()).unwrap();
        reject(&record, table, family);
    }

    #[test]
    fn native_rejects_duplicate_registry_and_duplicate_orbit_references() {
        let (family, bytes) = fixture();
        let (mut record, table) = unpack(&bytes);
        record.maps.push(record.maps[0].clone());
        reject(&record, table, family.clone());
        let (mut record, table) = unpack(&bytes);
        let repeated = record.witnesses[0].maps[0];
        record.witnesses[0].maps.push(repeated);
        reject(&record, table, family);
    }

    #[test]
    fn native_family_schema_recipe_and_exact_counts_are_authenticated() {
        let (original, bytes) = fixture();
        let changed = family(2);
        let (mut record, table) = unpack(&bytes);
        record.family = family_digest(changed.fingerprint());
        reject(&record, table, changed);
        for mutation in 0..3 {
            let (mut record, table) = unpack(&bytes);
            match mutation {
                0 => record.schema += 1,
                1 => record.recipe.push_str("-wrong"),
                _ => record.statistics[5] += 1,
            }
            reject(&record, table, original.clone());
        }
    }

    #[test]
    fn native_preserves_bounded_historical_geometry_diagnostics() {
        let (family, bytes) = fixture_with_generator(false);
        let (mut record, table) = unpack(&bytes);
        record.skipped = vec![(0, None, 1)];
        let bytes = encode_record(&record, table, BinaryIoLimits::unbounded_resources()).unwrap();
        let decoded = TerminalSectorSymmetryEquations::from_native_bytes(
            &bytes,
            family,
            Default::default(),
            BinaryIoLimits::unbounded_resources(),
        )
        .unwrap();
        assert_eq!(
            decoded.statistics().skipped,
            BTreeMap::from([(TerminalSectorSymmetrySkipReason::RankDeficientSupport, 1)])
        );
        assert_eq!(
            decoded
                .encode_native(BinaryIoLimits::unbounded_resources())
                .unwrap(),
            bytes
        );
    }

    #[test]
    fn native_rejects_excessive_duplicate_zero_and_unknown_geometry_diagnostics() {
        let (family, bytes) = fixture_with_generator(false);
        for mutation in 0..4 {
            let (mut record, table) = unpack(&bytes);
            let empty_roots = record
                .roots
                .iter()
                .filter(|root| root.maps.is_empty())
                .count();
            record.skipped = match mutation {
                0 => vec![(0, None, u64::try_from(empty_roots).unwrap() + 1)],
                1 => vec![(0, None, 1), (0, None, 1)],
                2 => vec![(0, None, 0)],
                _ => vec![(1, Some(255), 1)],
            };
            reject(&record, table, family.clone());
        }
    }

    #[test]
    fn native_digest_and_trailing_records_are_rejected() {
        let (family, bytes) = fixture();
        let mut corrupt = bytes.clone();
        let middle = corrupt.len() / 2;
        corrupt[middle] ^= 1;
        let mut trailing = bytes;
        trailing.push(0);
        for bytes in [corrupt, trailing] {
            assert!(
                TerminalSectorSymmetryEquations::from_native_bytes(
                    &bytes,
                    family.clone(),
                    Default::default(),
                    BinaryIoLimits::unbounded_resources()
                )
                .is_err()
            );
        }
    }

    fn round_trip_fixture() -> (Vec<Arc<IntegralFamily>>, Vec<u8>) {
        let source = family(1);
        let target = family_named("native_sector_symmetry_foreign", 1);
        assert_ne!(source.fingerprint(), target.fingerprint());
        let root = Mask::try_new([true, true, false]).unwrap();
        let map = |family: &IntegralFamily, entries: [i64; 4]| {
            MomentumMap::new(
                CoefficientMatrix::try_new(
                    2,
                    2,
                    entries.map(|value| family.coefficient_context().integer(value)),
                )
                .unwrap(),
                CoefficientMatrix::try_new(2, 0, []).unwrap(),
                CoefficientMatrix::try_new(0, 0, []).unwrap(),
            )
        };
        let outward = Arc::new(
            TerminalSectorSymmetryRoute::verify(
                source.clone(),
                target.clone(),
                root.clone(),
                root.clone(),
                map(&source, [1, 0, 0, 1]),
                Default::default(),
            )
            .unwrap(),
        );
        let inward = Arc::new(
            TerminalSectorSymmetryRoute::verify(
                target.clone(),
                source.clone(),
                root.clone(),
                root,
                map(&target, [-1, 0, 0, 1]),
                Default::default(),
            )
            .unwrap(),
        );
        let round_trip =
            TerminalSectorSymmetryRoute::verify_round_trip(&[outward, inward], Default::default())
                .unwrap();
        let bytes = round_trip
            .encode_native(BinaryIoLimits::unbounded_resources())
            .unwrap();
        (vec![source, target], bytes)
    }

    fn unpack_round_trip(
        bytes: &[u8],
    ) -> (RoundTripRecord, crate::persistence::EncodedCoefficientTable) {
        let envelope = inspect_program(bytes, BinaryIoLimits::unbounded_resources()).unwrap();
        let (record, consumed): (RoundTripRecord, usize) = bincode::decode_from_slice(
            envelope.section(SectionTag::PROGRAM).unwrap(),
            bincode::config::standard(),
        )
        .unwrap();
        assert_eq!(
            consumed,
            envelope.section(SectionTag::PROGRAM).unwrap().len()
        );
        (
            record,
            crate::persistence::EncodedCoefficientTable {
                state: envelope
                    .section(SectionTag::SYMBOLICA_STATE)
                    .unwrap()
                    .to_vec(),
                atoms: envelope.section(SectionTag::COEFFICIENTS).unwrap().to_vec(),
            },
        )
    }

    #[test]
    fn native_round_trip_chain_replays_both_owners_and_common_family_witness() {
        let (families, bytes) = round_trip_fixture();
        let decoded = TerminalSectorSymmetryRoundTrip::from_native_bytes(
            &bytes,
            &families,
            Default::default(),
            BinaryIoLimits::unbounded_resources(),
        )
        .unwrap();
        assert_eq!(decoded.routes().len(), 2);
        assert_eq!(
            decoded.routes()[0].source_family_fingerprint(),
            families[0].fingerprint()
        );
        assert_eq!(
            decoded.routes()[0].target_family_fingerprint(),
            families[1].fingerprint()
        );
        let witness = decoded
            .witness(
                &IntegralKey::try_new([2, 1, -1]).unwrap(),
                Default::default(),
            )
            .unwrap();
        assert!(!witness.equation().terms.is_empty());
        witness
            .replay(families[0].clone(), Default::default())
            .unwrap();
        assert_eq!(
            decoded
                .encode_native(BinaryIoLimits::unbounded_resources())
                .unwrap(),
            bytes
        );
    }

    #[test]
    fn native_round_trip_chain_requires_every_owning_family() {
        let (families, bytes) = round_trip_fixture();
        for registry in [&families[..1], &families[1..], &families[..0]] {
            assert!(
                TerminalSectorSymmetryRoundTrip::from_native_bytes(
                    &bytes,
                    registry,
                    Default::default(),
                    BinaryIoLimits::unbounded_resources(),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn native_round_trip_chain_rejects_changed_route_owner_roots_maps_and_composition() {
        let (families, bytes) = round_trip_fixture();
        for mutation in 0..4 {
            let (mut record, table) = unpack_round_trip(&bytes);
            match mutation {
                0 => record.routes[1].source_family = record.routes[0].source_family,
                1 => record.routes[0].source_root[2] = true,
                2 => record.routes[0].maps[0].entries.fill(zero_id(&table)),
                _ => record.maps[0].entries.fill(zero_id(&table)),
            }
            let altered =
                encode_record(&record, table, BinaryIoLimits::unbounded_resources()).unwrap();
            assert!(
                TerminalSectorSymmetryRoundTrip::from_native_bytes(
                    &altered,
                    &families,
                    Default::default(),
                    BinaryIoLimits::unbounded_resources(),
                )
                .is_err()
            );
        }
    }
}
