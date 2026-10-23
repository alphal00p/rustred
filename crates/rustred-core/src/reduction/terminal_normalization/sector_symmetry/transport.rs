use super::*;
use symbolica::domains::SelfRing;

use crate::algebra::matrix::{SymbolicaCoefficientMatrixLimits, multiply_coefficient_matrices};
use crate::sector::symmetry::{CoefficientMatrix, DenominatorAction};

pub(super) fn identity_map(
    family: &IntegralFamily,
    limits: TerminalSectorSymmetryLimits,
) -> Result<MomentumMap, Error> {
    let loops = family.loop_count();
    let context = family.coefficient_context();
    Ok(MomentumMap::new(
        CoefficientMatrix::try_new_with_max_entries(
            loops,
            loops,
            (0..loops).flat_map(|left| {
                (0..loops).map(move |right| {
                    if left == right {
                        context.one()
                    } else {
                        context.zero()
                    }
                })
            }),
            limits.verification.max_matrix_entries,
        )
        .map_err(algebra)?,
        CoefficientMatrix::try_new_with_max_entries(
            loops,
            0,
            [],
            limits.verification.max_matrix_entries,
        )
        .map_err(algebra)?,
        CoefficientMatrix::try_new_with_max_entries(
            0,
            0,
            [],
            limits.verification.max_matrix_entries,
        )
        .map_err(algebra)?,
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
    let rows = |matrix: &CoefficientMatrix| {
        (0..matrix.rows())
            .map(|row| {
                (0..matrix.columns())
                    .map(|column| {
                        matrix
                            .get(row, column)
                            .expect("checked momentum matrix")
                            .clone()
                    })
                    .collect()
            })
            .collect::<Vec<Vec<_>>>()
    };
    let verification = limits.verification;
    let native = SymbolicaCoefficientMatrixLimits {
        exact_algebra: verification.exact_algebra,
        max_single_matrix_entries: verification.max_symbolica_single_matrix_entries,
        max_live_matrix_entries: verification.max_symbolica_live_matrix_entries,
        max_exact_operations: verification.max_exact_operations,
        max_input_retained_bytes: verification.max_symbolica_input_retained_bytes,
        max_output_retained_bytes: verification.max_symbolica_output_retained_bytes,
    };
    let product = multiply_coefficient_matrices(
        family.coefficient_context(),
        &rows(first.loop_linear()),
        &rows(second.loop_linear()),
        native,
    )
    .map_err(algebra)?
    .0;
    let loops = family.loop_count();
    Ok(MomentumMap::new(
        CoefficientMatrix::try_new_with_max_entries(
            loops,
            loops,
            product.into_iter().flatten(),
            verification.max_matrix_entries,
        )
        .map_err(algebra)?,
        CoefficientMatrix::try_new_with_max_entries(loops, 0, [], verification.max_matrix_entries)
            .map_err(algebra)?,
        CoefficientMatrix::try_new_with_max_entries(0, 0, [], verification.max_matrix_entries)
            .map_err(algebra)?,
    ))
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
            exact_algebra: limits.verification.exact_algebra,
            max_single_matrix_entries: limits.verification.max_symbolica_single_matrix_entries,
            max_live_matrix_entries: limits.verification.max_symbolica_live_matrix_entries,
            max_exact_operations: limits.verification.max_exact_operations,
            max_input_retained_bytes: limits.verification.max_symbolica_input_retained_bytes,
            max_output_retained_bytes: limits.verification.max_symbolica_output_retained_bytes,
        };
        let rows = |matrix: &CoefficientMatrix| {
            (0..matrix.rows())
                .map(|r| {
                    (0..matrix.columns())
                        .map(|c| matrix.get(r, c).expect("checked matrix shape").clone())
                        .collect()
                })
                .collect::<Vec<Vec<_>>>()
        };
        let mut product = rows(first.verified_map().momentum().loop_linear());
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
                first.source_family.coefficient_context(),
                &product,
                &rows(next.verified_map().momentum().loop_linear()),
                matrix_limits,
            )
            .map_err(algebra)?
            .0;
        }
        let loops = first.source_family.loop_count();
        let map = MomentumMap::new(
            CoefficientMatrix::try_new_with_max_entries(
                loops,
                loops,
                product.into_iter().flatten(),
                limits.verification.max_matrix_entries,
            )
            .map_err(algebra)?,
            CoefficientMatrix::try_new_with_max_entries(
                loops,
                0,
                [],
                limits.verification.max_matrix_entries,
            )
            .map_err(algebra)?,
            CoefficientMatrix::try_new_with_max_entries(
                0,
                0,
                [],
                limits.verification.max_matrix_entries,
            )
            .map_err(algebra)?,
        );
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
