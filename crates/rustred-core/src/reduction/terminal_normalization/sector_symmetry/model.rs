use super::*;

/// Cooperative controls apply only while building the finite immutable plan.
#[derive(Default)]
pub struct TerminalSectorSymmetryControl<'a> {
    pub cancelled: Option<&'a dyn Fn() -> bool>,
    pub progress: Option<&'a dyn Fn(TerminalSectorSymmetryProgress)>,
}

impl TerminalSectorSymmetryControl<'_> {
    pub fn checkpoint(&self) -> Result<(), Error> {
        if self.cancelled.is_some_and(|cancelled| cancelled()) {
            return Err(Error::InvalidWitness(
                "sector symmetry preparation cancelled",
            ));
        }
        Ok(())
    }

    pub fn report(&self, progress: TerminalSectorSymmetryProgress) {
        if let Some(report) = self.progress {
            report(progress);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalSectorSymmetryProgress {
    pub processed_keys: usize,
    pub discovered_keys: usize,
    pub analyzed_supports: usize,
    pub verified_generators: usize,
    /// Distinct complete images in the current original-source orbit.
    pub orbit_images: usize,
    pub orbit_terms: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalSectorSymmetryStatistics {
    pub input_keys: usize,
    pub closure_keys: usize,
    pub analyzed_supports: usize,
    pub verified_generators: usize,
    pub equation_targets: usize,
    pub equations: usize,
    pub output_terms: usize,
    /// Original-source Reynolds averages that cancel to zero, counted before
    /// merging the independently authorized weak supplement.
    pub trivial_equations: usize,
    /// Each unsupported or rank-deficient support is counted once. These
    /// diagnostics make no completeness or independent-master assertion.
    pub skipped: BTreeMap<TerminalSectorSymmetrySkipReason, usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminalSectorSymmetrySkipReason {
    RankDeficientSupport,
    MomentumGeometry(super::super::ProductSkipReason),
}

/// Generator rows are finite exact identities without a full-group span
/// claim. Polynomial orbits average the complete image orbit. Default
/// preparation retains the original weak cohort under either policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerminalSectorSymmetryPolicy {
    #[default]
    GeneratorRelations,
    PolynomialOrbits,
}

/// A family-qualified, exactly verified active-denominator route.
/// A transported foreign key never becomes a common-family equation by
/// removing its family identity. Use `compose_round_trip` to return it.
#[derive(Debug)]
pub struct TerminalSectorSymmetryRoute {
    pub(super) source_family: Arc<IntegralFamily>,
    pub(super) target_family: Arc<IntegralFamily>,
    pub(super) prepared: integral_transport::Prepared,
}

/// Retained source/target route chain and its independently verified
/// common-family composition. Native replay requires every route owner.
#[derive(Debug)]
pub struct TerminalSectorSymmetryRoundTrip {
    pub(super) routes: Vec<Arc<TerminalSectorSymmetryRoute>>,
    pub(super) map: Arc<VerifiedMap>,
}

impl TerminalSectorSymmetryRoundTrip {
    pub fn routes(&self) -> &[Arc<TerminalSectorSymmetryRoute>] {
        &self.routes
    }
    pub fn verified_map(&self) -> &VerifiedMap {
        &self.map
    }

    pub fn witness(
        &self,
        source: &IntegralKey,
        limits: TerminalSectorSymmetryLimits,
    ) -> Result<TerminalSectorSymmetryWitness, Error> {
        let first = self
            .routes
            .first()
            .ok_or(Error::InvalidWitness("empty sector symmetry round trip"))?;
        let root = Mask::try_from_indices(source.powers()).map_err(algebra)?;
        if &root != first.source_root() {
            return Err(Error::InvalidWitness(
                "sector symmetry round-trip source support differs",
            ));
        }
        TerminalSectorSymmetryWitness::from_verified(
            first.source_family.clone(),
            source,
            self.map.clone(),
            limits,
        )
    }
}

/// A homogeneous same-family equation with its verified complete map.
#[derive(Clone, Debug)]
pub struct TerminalSectorSymmetryWitness {
    pub(super) family: Arc<String>,
    pub(super) source: IntegralKey,
    pub(super) map: Arc<VerifiedMap>,
    pub(super) equation: TerminalEquation,
}

impl TerminalSectorSymmetryWitness {
    pub fn family_fingerprint(&self) -> &str {
        &self.family
    }
    pub fn source(&self) -> &IntegralKey {
        &self.source
    }
    pub fn verified_map(&self) -> &VerifiedMap {
        &self.map
    }
    pub fn equation(&self) -> &TerminalEquation {
        &self.equation
    }

    pub fn replay(
        &self,
        family: Arc<IntegralFamily>,
        limits: TerminalSectorSymmetryLimits,
    ) -> Result<(), Error> {
        if family.fingerprint() != self.family_fingerprint() {
            return Err(Error::InvalidWitness("sector symmetry family mismatch"));
        }
        let fresh = Self::verify(family, &self.source, self.map.momentum().clone(), limits)?;
        if fresh.equation != self.equation {
            return Err(Error::InvalidWitness(
                "sector symmetry full identity replay differs",
            ));
        }
        Ok(())
    }
}

/// An exact complete-polynomial image average for one original integral.
/// Polynomial-orbit policy uses a full orbit; generator policy uses one map
/// per witness. Maps are shared; expanded images are not retained here.
#[derive(Clone, Debug)]
pub struct TerminalSectorSymmetryOrbitWitness {
    pub(super) family: Arc<String>,
    pub(super) source: IntegralKey,
    pub(super) maps: Vec<Arc<VerifiedMap>>,
    pub(super) equation: TerminalEquation,
}

impl TerminalSectorSymmetryOrbitWitness {
    pub fn family_fingerprint(&self) -> &str {
        &self.family
    }
    pub fn source(&self) -> &IntegralKey {
        &self.source
    }
    pub fn verified_maps(&self) -> &[Arc<VerifiedMap>] {
        &self.maps
    }
    pub fn equation(&self) -> &TerminalEquation {
        &self.equation
    }

    /// Replay row authority from every exact full-integral image. Provider
    /// replay additionally establishes the selected source cohort. Only the
    /// polynomial-orbit policy establishes closure under the retained group.
    pub fn replay(
        &self,
        family: Arc<IntegralFamily>,
        limits: TerminalSectorSymmetryLimits,
    ) -> Result<(), Error> {
        if family.fingerprint() != self.family_fingerprint() {
            return Err(Error::InvalidWitness(
                "sector symmetry orbit family mismatch",
            ));
        }
        let fresh = closure::replay_average(&family, &self.source, &self.maps, limits)?;
        if fresh != self.equation {
            return Err(Error::InvalidWitness(
                "sector symmetry orbit average replay differs",
            ));
        }
        Ok(())
    }
}

/// Policy-selected complete-polynomial identities on original keys, with
/// auxiliary columns. Generator cohorts do not assert a full group span.
/// Endpoint monomials never become additional symmetry sources implicitly.
#[derive(Debug)]
pub struct TerminalSectorSymmetryEquations {
    pub(super) policy: TerminalSectorSymmetryPolicy,
    pub(super) family_fingerprint: Arc<String>,
    pub(super) input_keys: BTreeSet<IntegralKey>,
    pub(super) inventory: BTreeSet<IntegralKey>,
    pub(super) routes: BTreeMap<Mask, Arc<VerifiedMap>>,
    pub(super) generators: BTreeMap<Mask, Vec<Arc<VerifiedMap>>>,
    pub(super) rows: BTreeMap<IntegralKey, Vec<TerminalEquation>>,
    pub(super) witnesses: BTreeMap<IntegralKey, Vec<TerminalSectorSymmetryOrbitWitness>>,
    /// Original-source weak rows preserve the established finite-projection
    /// opportunities alongside Reynolds rows. Their old authority is reused.
    pub(super) legacy: Option<Arc<super::super::TerminalNumeratorSymmetryEquations>>,
    pub(super) statistics: TerminalSectorSymmetryStatistics,
}

impl TerminalSectorSymmetryEquations {
    pub fn prepare(
        family: Arc<IntegralFamily>,
        keys: &BTreeSet<IntegralKey>,
        limits: TerminalSectorSymmetryLimits,
    ) -> Result<Self, Error> {
        Self::prepare_with_control(
            family,
            keys,
            limits,
            &TerminalSectorSymmetryControl::default(),
        )
    }

    pub fn prepare_with_control(
        family: Arc<IntegralFamily>,
        keys: &BTreeSet<IntegralKey>,
        limits: TerminalSectorSymmetryLimits,
        control: &TerminalSectorSymmetryControl<'_>,
    ) -> Result<Self, Error> {
        closure::prepare(family, keys, limits, control)
    }

    pub fn prepare_with_policy(
        family: Arc<IntegralFamily>,
        keys: &BTreeSet<IntegralKey>,
        limits: TerminalSectorSymmetryLimits,
        policy: TerminalSectorSymmetryPolicy,
    ) -> Result<Self, Error> {
        Self::prepare_with_policy_and_control(
            family,
            keys,
            limits,
            policy,
            &TerminalSectorSymmetryControl::default(),
        )
    }

    pub fn prepare_with_policy_and_control(
        family: Arc<IntegralFamily>,
        keys: &BTreeSet<IntegralKey>,
        limits: TerminalSectorSymmetryLimits,
        policy: TerminalSectorSymmetryPolicy,
        control: &TerminalSectorSymmetryControl<'_>,
    ) -> Result<Self, Error> {
        closure::prepare_with_policy(family, keys, limits, control, policy)
    }

    /// Average complete integral images under supplied self-support generators.
    /// Cross-support routes use `prepare_with_orbit_maps` explicitly.
    pub fn prepare_with_maps(
        family: Arc<IntegralFamily>,
        keys: &BTreeSet<IntegralKey>,
        maps: BTreeMap<Mask, Vec<MomentumMap>>,
        limits: TerminalSectorSymmetryLimits,
        control: &TerminalSectorSymmetryControl<'_>,
    ) -> Result<Self, Error> {
        let mut verified = BTreeMap::new();
        for (root, maps) in maps {
            let mut retained = Vec::new();
            for map in maps {
                control.checkpoint()?;
                retained.push(Arc::new(
                    symmetry::verify(&family, &family, map, limits.verification)
                        .map_err(algebra)?,
                ));
            }
            verified.insert(root, retained);
        }
        closure::prepare_with_generators(family, keys, verified, limits, control)
    }

    /// Supply source-to-representative routes separately from representative
    /// self generators. All maps pass the same full exact verification gates.
    pub fn prepare_with_orbit_maps(
        family: Arc<IntegralFamily>,
        keys: &BTreeSet<IntegralKey>,
        routes: BTreeMap<Mask, MomentumMap>,
        generators: BTreeMap<Mask, Vec<MomentumMap>>,
        limits: TerminalSectorSymmetryLimits,
        control: &TerminalSectorSymmetryControl<'_>,
    ) -> Result<Self, Error> {
        let mut verified_routes = BTreeMap::new();
        let mut verified_generators = BTreeMap::new();
        for (root, map) in routes {
            control.checkpoint()?;
            verified_routes.insert(
                root,
                Arc::new(
                    symmetry::verify(&family, &family, map, limits.verification)
                        .map_err(algebra)?,
                ),
            );
        }
        for (root, maps) in generators {
            let mut verified = Vec::new();
            for map in maps {
                control.checkpoint()?;
                verified.push(Arc::new(
                    symmetry::verify(&family, &family, map, limits.verification)
                        .map_err(algebra)?,
                ));
            }
            verified_generators.insert(root, verified);
        }
        closure::prepare_with_orbit_supports(
            family,
            keys,
            verified_routes,
            verified_generators,
            limits,
            control,
        )
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn policy(&self) -> TerminalSectorSymmetryPolicy {
        self.policy
    }
    pub fn input_keys(&self) -> &BTreeSet<IntegralKey> {
        &self.input_keys
    }
    pub fn inventory(&self) -> &BTreeSet<IntegralKey> {
        &self.inventory
    }
    pub fn equations(&self, key: &IntegralKey) -> &[TerminalEquation] {
        self.rows.get(key).map(Vec::as_slice).unwrap_or(&[])
    }
    pub fn witnesses(&self, key: &IntegralKey) -> &[TerminalSectorSymmetryOrbitWitness] {
        self.witnesses.get(key).map(Vec::as_slice).unwrap_or(&[])
    }
    pub fn statistics(&self) -> &TerminalSectorSymmetryStatistics {
        &self.statistics
    }
    pub fn legacy_supplement(&self) -> Option<&super::super::TerminalNumeratorSymmetryEquations> {
        self.legacy.as_deref()
    }
}
