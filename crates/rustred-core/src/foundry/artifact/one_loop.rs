use std::collections::BTreeSet;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::foundry::parametric::{ParametricRuleLimits, derive_sector_interior_rule};
use crate::identity::ParametricIbpGenerator;
use crate::sector::{Mask, OrderingPolicy};

use super::error::ArtifactError;
use super::install::{ClosingArtifactCandidate, install};
use super::model::{
    ArtifactSchemaVersion, ClosedArtifact, CommonMassHomogeneityProof, ZeroSectorTerminal,
    ZeroTerminalProof,
};

pub(super) const ALGORITHM_ID: &str = "rustred.generated.one-loop-unit-mass-tadpole.v1";

/// Generate the sole ordinary IBP source and seal the genuinely closed
/// one-loop, unit-common-mass tadpole family.
///
/// This is not an authored recurrence table.  The rule is derived from the
/// freshly generated source over `K(n)`, exactly replayed by the parametric
/// foundry, and only then admitted by the closing-artifact installer.
pub fn derive_one_loop_unit_mass_tadpole() -> Result<ClosedArtifact, ArtifactError> {
    // The artifact is genuinely unit mass and therefore lives over Q(d).
    // Common-scale dependence is restored later as a typed homogeneity power,
    // without smuggling an unused mass symbol into this coefficient map.
    let base = CoefficientContext::try_new(["d"])?;
    let dimension = base
        .parameter("d")
        .expect("the just-authenticated coefficient context contains d");
    let one = base.one();
    let minus_one = base
        .try_neg(&one, Default::default())
        .map_err(crate::family::IntegralFamilyError::from)?;
    let zero = base.zero();
    let family = IntegralFamily::new(
        "rustred-one-loop-unit-mass-tadpole-v1",
        vec!["k".to_owned()],
        Vec::new(),
        base,
        dimension,
        // Vakint's authenticated vacuum convention is q^2-m^2.
        vec![AffineDenominator::new(minus_one, vec![one])],
        Vec::new(),
        vec![zero],
    )?;
    let generator = ParametricIbpGenerator::try_new(&family)?;
    let batch = generator.prepare_ordinary_ibp()?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(batch.len())
        .map_err(|_| ArtifactError::InvalidRuleShape {
            detail: "could not reserve the generated ordinary source set",
        })?;
    for ordinal in 0..batch.len() {
        rows.push(batch.generate(ordinal));
    }
    let source_relations = batch.complete(rows)?.into_relations();
    let rule = derive_sector_interior_rule(
        generator.context(),
        &source_relations,
        &[1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?;
    let context = generator.context().clone();
    drop(generator);
    let master = IntegralKey::try_new([1])?;
    let mut masters = BTreeSet::new();
    masters.insert(master);
    let zero_sector = Mask::try_from_indices(&[0])?;
    install(ClosingArtifactCandidate {
        schema: ArtifactSchemaVersion::CURRENT,
        algorithm_id: ALGORITHM_ID,
        arity: 1,
        family,
        context,
        source_relations,
        rules: vec![rule],
        masters,
        zero_sectors: vec![ZeroSectorTerminal::new(
            zero_sector,
            ZeroTerminalProof::ScalelessVacuumPolynomial,
        )],
        common_mass_homogeneity: Some(CommonMassHomogeneityProof::UniformVacuumMassSquared),
    })
}
