use super::*;
use symbolica::domains::SelfRing;

use crate::algebra::Coefficient;
use crate::algebra::matrix::{SymbolicaCoefficientMatrixLimits, multiply_coefficient_matrices};
use crate::sector::symmetry::{CoefficientMatrix, DenominatorAction};

pub(super) fn identity_map(
    family: &IntegralFamily,
    limits: TerminalSectorSymmetryLimits,
) -> Result<MomentumMap, Error> {
    let size = family.loop_count() + family.external_count();
    let context = family.coefficient_context();
    let unit = (0..size)
        .map(|row| {
            (0..size)
                .map(|column| {
                    if row == column {
                        context.one()
                    } else {
                        context.zero()
                    }
                })
                .collect()
        })
        .collect();
    from_augmented(family, unit, limits)
}

/// The block matrix `[[A, B], [0, C]]` of `k = A k' + B p'`, `p = C p'` over
/// the loop and then the external momenta. Composition of maps is the
/// product of these matrices; for a vacuum family they are `A`.
fn augmented(map: &MomentumMap, family: &IntegralFamily) -> Vec<Vec<Coefficient>> {
    let loops = family.loop_count();
    let externals = family.external_count();
    let context = family.coefficient_context();
    let entry = |matrix: &CoefficientMatrix, row, column| {
        matrix
            .get(row, column)
            .expect("checked momentum matrix")
            .clone()
    };
    (0..loops + externals)
        .map(|row| {
            (0..loops + externals)
                .map(|column| match (row < loops, column < loops) {
                    (true, true) => entry(map.loop_linear(), row, column),
                    (true, false) => entry(map.loop_external(), row, column - loops),
                    (false, true) => context.zero(),
                    (false, false) => entry(map.external_linear(), row - loops, column - loops),
                })
                .collect()
        })
        .collect()
}

/// Split an augmented matrix back into a momentum map. Its external rows
/// cannot involve loop momenta.
fn from_augmented(
    family: &IntegralFamily,
    rows: Vec<Vec<Coefficient>>,
    limits: TerminalSectorSymmetryLimits,
) -> Result<MomentumMap, Error> {
    let loops = family.loop_count();
    let externals = family.external_count();
    let max_entries = limits.verification.max_matrix_entries;
    if rows[loops..]
        .iter()
        .any(|row| row[..loops].iter().any(|value| !value.is_zero()))
    {
        return Err(Error::InvalidWitness(
            "composed sector symmetry map mixes external and loop momenta",
        ));
    }
    let block = |rows: &[Vec<Coefficient>], columns: std::ops::Range<usize>| {
        CoefficientMatrix::try_new_with_max_entries(
            rows.len(),
            columns.len(),
            rows.iter()
                .flat_map(|row| row[columns.clone()].to_vec())
                .collect::<Vec<_>>(),
            max_entries,
        )
        .map_err(algebra)
    };
    Ok(MomentumMap::new(
        block(&rows[..loops], 0..loops)?,
        block(&rows[..loops], loops..loops + externals)?,
        block(&rows[loops..], loops..loops + externals)?,
    ))
}

/// Native matrix product in substitution order: source=A middle,
/// middle=B target, hence source=(A B) target. No group enumeration occurs.
pub(super) fn compose_maps(
    family: &IntegralFamily,
    first: &MomentumMap,
    second: &MomentumMap,
    limits: TerminalSectorSymmetryLimits,
) -> Result<MomentumMap, Error> {
    let verification = limits.verification;
    let native = SymbolicaCoefficientMatrixLimits {
        diagnostics_enabled: verification.diagnostics_enabled,
        exact_algebra: verification.exact_algebra,
        max_single_matrix_entries: verification.max_symbolica_single_matrix_entries,
        max_live_matrix_entries: verification.max_symbolica_live_matrix_entries,
        max_exact_operations: verification.max_exact_operations,
        max_input_retained_bytes: verification.max_symbolica_input_retained_bytes,
        max_output_retained_bytes: verification.max_symbolica_output_retained_bytes,
    };
    let product = multiply_coefficient_matrices(
        family.coefficient_context(),
        &augmented(first, family),
        &augmented(second, family),
        native,
    )
    .map_err(algebra)?
    .0;
    from_augmented(family, product, limits)
}

impl TerminalSectorSymmetryRoute {
    /// Find one canonical route between two supplied full-rank vacuum
    /// supports. Graph matching proposes it; the exact native verifier and
    /// active-denominator transport gate establish the returned identity.
    pub fn discover(
        source: Arc<IntegralFamily>,
        target: Arc<IntegralFamily>,
        source_root: Mask,
        target_root: Mask,
        limits: TerminalSectorSymmetryLimits,
        control: &TerminalSectorSymmetryControl<'_>,
    ) -> Result<Option<Self>, Error> {
        let Some(source_support) =
            discovery::prepare_support(&source, &source_root, limits, control)?
        else {
            return Ok(None);
        };
        let Some(target_support) =
            discovery::prepare_support(&target, &target_root, limits, control)?
        else {
            return Ok(None);
        };
        let Some(map) =
            discovery::match_supports(&source_support, &target_support, limits, control)?
        else {
            return Ok(None);
        };
        let prepared = integral_transport::compile(
            &source,
            target.clone(),
            map,
            source_root,
            target_root,
            limits.expansion,
        )?;
        Ok(Some(Self {
            source_family: source,
            target_family: target,
            prepared,
        }))
    }

    /// Verify `k_source = A k_target` and bind an exact active-line bijection.
    /// Both support masks and both family owners are retained by this route.
    pub fn verify(
        source: Arc<IntegralFamily>,
        target: Arc<IntegralFamily>,
        source_root: Mask,
        target_root: Mask,
        map: MomentumMap,
        limits: TerminalSectorSymmetryLimits,
    ) -> Result<Self, Error> {
        eligible(&source)?;
        eligible(&target)?;
        let verified = Arc::new(
            symmetry::verify(&source, &target, map, limits.verification).map_err(algebra)?,
        );
        let prepared = integral_transport::compile(
            &source,
            target.clone(),
            verified,
            source_root,
            target_root,
            limits.expansion,
        )?;
        Ok(Self {
            source_family: source,
            target_family: target,
            prepared,
        })
    }

    pub fn source_family_fingerprint(&self) -> &str {
        self.source_family.fingerprint()
    }
    pub fn target_family_fingerprint(&self) -> &str {
        self.target_family.fingerprint()
    }
    pub fn source_root(&self) -> &Mask {
        self.prepared.source_root()
    }
    pub fn target_root(&self) -> &Mask {
        self.prepared.target_root()
    }
    pub fn verified_map(&self) -> &VerifiedMap {
        self.prepared.verified_map()
    }

    /// Return an explicitly family-qualified exact combination.
    pub fn transport(
        &self,
        source: &IntegralKey,
        limits: TerminalSectorSymmetryLimits,
    ) -> Result<integral_transport::TransportedIntegral, Error> {
        validate_key(&self.source_family, source)?;
        Ok(self.prepared.transport(source, limits.expansion)?)
    }

    /// Compose a supplied family-qualified route chain returning to its owner.
    /// Native Symbolica multiplies the momentum matrices in substitution order;
    /// the resulting common-family map is independently verified again.
    /// A foreign endpoint or a mismatched intermediate owner is an error.
    pub fn compose_round_trip(
        routes: &[Arc<Self>],
        limits: TerminalSectorSymmetryLimits,
    ) -> Result<Arc<VerifiedMap>, Error> {
        let first = routes
            .first()
            .ok_or(Error::InvalidWitness("empty sector symmetry route chain"))?;
        let last = routes.last().expect("nonempty chain");
        if first.source_family_fingerprint() != last.target_family_fingerprint() {
            return Err(Error::InvalidWitness(
                "sector symmetry route does not return to its source family",
            ));
        }
        let matrix_limits = SymbolicaCoefficientMatrixLimits {
            diagnostics_enabled: limits.verification.diagnostics_enabled,
            exact_algebra: limits.verification.exact_algebra,
            max_single_matrix_entries: limits.verification.max_symbolica_single_matrix_entries,
            max_live_matrix_entries: limits.verification.max_symbolica_live_matrix_entries,
            max_exact_operations: limits.verification.max_exact_operations,
            max_input_retained_bytes: limits.verification.max_symbolica_input_retained_bytes,
            max_output_retained_bytes: limits.verification.max_symbolica_output_retained_bytes,
        };
        let family = &first.source_family;
        let mut product = augmented(first.verified_map().momentum(), family);
        for pair in routes.windows(2) {
            let previous = &pair[0];
            let next = &pair[1];
            if previous.target_family_fingerprint() != next.source_family_fingerprint()
                || previous.target_root() != next.source_root()
            {
                return Err(Error::InvalidWitness(
                    "sector symmetry route chain ownership or support mismatch",
                ));
            }
            product = multiply_coefficient_matrices(
                family.coefficient_context(),
                &product,
                &augmented(next.verified_map().momentum(), family),
                matrix_limits,
            )
            .map_err(algebra)?
            .0;
        }
        let map = from_augmented(family, product, limits)?;
        let verified = Arc::new(
            symmetry::verify(
                &first.source_family,
                &first.source_family,
                map,
                limits.verification,
            )
            .map_err(algebra)?,
        );
        integral_transport::compile(
            &first.source_family,
            first.source_family.clone(),
            verified.clone(),
            first.source_root().clone(),
            last.target_root().clone(),
            limits.expansion,
        )?;
        Ok(verified)
    }

    /// Retain the family-qualified chain as well as the exact composed map.
    pub fn verify_round_trip(
        routes: &[Arc<Self>],
        limits: TerminalSectorSymmetryLimits,
    ) -> Result<TerminalSectorSymmetryRoundTrip, Error> {
        let map = Self::compose_round_trip(routes, limits)?;
        Ok(TerminalSectorSymmetryRoundTrip {
            routes: routes.to_vec(),
            map,
        })
    }
}

impl TerminalSectorSymmetryWitness {
    /// Match active denominators by an exact unit bijection while transporting
    /// the full signed tuple. Positive powers may move to distinct slots.
    pub fn verify(
        family: Arc<IntegralFamily>,
        source: &IntegralKey,
        map: MomentumMap,
        limits: TerminalSectorSymmetryLimits,
    ) -> Result<Self, Error> {
        eligible(&family)?;
        validate_key(&family, source)?;
        let verified = Arc::new(
            symmetry::verify(&family, &family, map, limits.verification).map_err(algebra)?,
        );
        Self::from_verified(family, source, verified, limits)
    }

    pub(super) fn from_verified(
        family: Arc<IntegralFamily>,
        source: &IntegralKey,
        map: Arc<VerifiedMap>,
        limits: TerminalSectorSymmetryLimits,
    ) -> Result<Self, Error> {
        validate_key(&family, source)?;
        let root = Mask::try_from_indices(source.powers()).map_err(algebra)?;
        let prepared = prepare_map(&family, &root, map.clone(), limits)?;
        let transported = prepared.transport(source, limits.expansion)?;
        let equation = complete_equation(&family, source, &transported, limits)?;
        Ok(Self {
            family: family.fingerprint_owner(),
            source: source.clone(),
            map,
            equation,
        })
    }
}

pub(super) fn prepare_map(
    family: &Arc<IntegralFamily>,
    source_root: &Mask,
    map: Arc<VerifiedMap>,
    limits: TerminalSectorSymmetryLimits,
) -> Result<integral_transport::Prepared, Error> {
    if map.source_family_fingerprint() != family.fingerprint()
        || map.target_family_fingerprint() != family.fingerprint()
    {
        return Err(Error::InvalidWitness(
            "foreign-family map cannot enter a homogeneous terminal equation",
        ));
    }
    if source_root.arity() != family.denominator_count() {
        return Err(Error::InvalidWitness(
            "sector symmetry source support arity differs",
        ));
    }
    let mut target_bits = vec![false; family.denominator_count()];
    for (axis, active) in source_root.active_bits().iter().copied().enumerate() {
        if !active {
            continue;
        }
        let DenominatorAction::Monomial { target, scale } = &map.row_actions()[axis] else {
            return Err(Error::InvalidWitness(
                "sector symmetry does not preserve an active denominator",
            ));
        };
        if !scale.is_one() || *target >= target_bits.len() || target_bits[*target] {
            return Err(Error::InvalidWitness(
                "sector symmetry active denominators are not a unit bijection",
            ));
        }
        target_bits[*target] = true;
    }
    let target_root = Mask::try_new(target_bits).map_err(algebra)?;
    Ok(integral_transport::compile(
        family,
        family.clone(),
        map,
        source_root.clone(),
        target_root,
        limits.expansion,
    )?)
}

pub(super) fn complete_equation(
    family: &IntegralFamily,
    source: &IntegralKey,
    transported: &integral_transport::TransportedIntegral,
    limits: TerminalSectorSymmetryLimits,
) -> Result<TerminalEquation, Error> {
    if transported.source_family_fingerprint() != family.fingerprint()
        || transported.target_family_fingerprint() != family.fingerprint()
    {
        return Err(Error::InvalidWitness(
            "foreign-family transport cannot enter a homogeneous terminal equation",
        ));
    }
    let context = family.coefficient_context();
    let mut terms = BTreeMap::from([(source.clone(), context.one())]);
    for endpoint in transported.terms() {
        let coefficient = match terms.get(endpoint.key()) {
            Some(old) => {
                context.try_sub(old, endpoint.coefficient(), limits.expansion.exact_algebra)
            }
            None => context.try_neg(endpoint.coefficient(), limits.expansion.exact_algebra),
        }
        .map_err(algebra)?;
        if coefficient.is_zero() {
            terms.remove(endpoint.key());
        } else {
            terms.insert(endpoint.key().clone(), coefficient);
        }
    }
    // The shared compile gate rejects unresolved symbolic guards. No generic
    // condition is discarded by this supported unconditional rational lane.
    Ok(TerminalEquation {
        terms,
        nonzero_conditions: Vec::new(),
    })
}
