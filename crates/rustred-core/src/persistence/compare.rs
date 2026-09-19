use super::{BinaryIoError, BinaryIoLimits, DecodedCoefficientTable, SectionTag, inspect_program};
use crate::algebra::Coefficient;

/// Context ownership is stricter than Symbolica's algebraic equality: native
/// polynomial equality deliberately equates zero/constants on different maps.
/// Keep the ordered maps explicit before delegating value equality to Symbolica.
pub(crate) fn same_native_coefficient(left: &Coefficient, right: &Coefficient) -> bool {
    left.numerator.variables() == right.numerator.variables()
        && left.denominator.variables() == right.denominator.variables()
        && left == right
}

/// Compare generated programs without treating ambient Symbolica state bytes
/// as a mathematical identity. Structural sections and ordered native values
/// must agree; this comparison does not establish replay or closure authority.
///
/// Both inputs must satisfy the trusted-generated native payload contract of
/// [`DecodedCoefficientTable::import_generated`]. Native imports can extend
/// process-global Symbolica state.
pub fn equivalent_generated_programs(
    left: &[u8],
    right: &[u8],
    limits: BinaryIoLimits,
) -> Result<bool, BinaryIoError> {
    let left = inspect_program(left, limits)?;
    let right = inspect_program(right, limits)?;
    if left.kind() != right.kind() || left.sections().len() != right.sections().len() {
        return Ok(false);
    }
    for (left, right) in left.sections().iter().zip(right.sections()) {
        if left.tag != right.tag {
            return Ok(false);
        }
        if left.tag != SectionTag::SYMBOLICA_STATE
            && left.tag != SectionTag::COEFFICIENTS
            && left.bytes != right.bytes
        {
            return Ok(false);
        }
    }
    let load = |envelope: &super::ProgramEnvelope<'_>| {
        DecodedCoefficientTable::import_generated(
            envelope
                .section(SectionTag::SYMBOLICA_STATE)
                .ok_or(BinaryIoError::Invalid("missing native state"))?,
            envelope
                .section(SectionTag::COEFFICIENTS)
                .ok_or(BinaryIoError::Invalid("missing coefficient table"))?,
            limits,
        )
    };
    let left = load(&left)?;
    let right = load(&right)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for index in 0..left.len() {
        let id = super::CoefficientId::try_from_index(index)?;
        if !same_native_coefficient(left.coefficient(id)?, right.coefficient(id)?) {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::{Coefficient, CoefficientContext, IndexedCoefficientContext};
    use crate::persistence::{
        BinaryProgramKind, BinarySection, CoefficientTableBuilder, encode_program,
    };

    fn program(kind: BinaryProgramKind, coefficients: &[Coefficient], payload: &[u8]) -> Vec<u8> {
        let mut builder = CoefficientTableBuilder::new(Default::default());
        for coefficient in coefficients {
            builder.intern(coefficient).unwrap();
        }
        let table = builder.finish().unwrap();
        encode_program(
            kind,
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
                    bytes: payload,
                },
            ],
            Default::default(),
        )
        .unwrap()
    }

    #[test]
    fn equality_preserves_kind_structure_order_values_maps_and_unused_entries() {
        let context = CoefficientContext::try_new(["native_compare_d"]).unwrap();
        let a = context.integer(1);
        let b = context.integer(2);
        let indexed = IndexedCoefficientContext::try_new(&context, "native-compare", 2).unwrap();
        let original = program(
            BinaryProgramKind::Certified,
            &[a.clone(), b.clone()],
            b"proof",
        );
        assert!(equivalent_generated_programs(&original, &original, Default::default()).unwrap());
        for (case, other) in [
            program(
                BinaryProgramKind::Candidates,
                &[a.clone(), b.clone()],
                b"proof",
            ),
            program(
                BinaryProgramKind::Certified,
                &[a.clone(), b.clone()],
                b"changed",
            ),
            program(
                BinaryProgramKind::Certified,
                &[b.clone(), a.clone()],
                b"proof",
            ),
            program(BinaryProgramKind::Certified, &[a.clone()], b"proof"),
            program(
                BinaryProgramKind::Certified,
                &[a.clone(), b.clone(), context.integer(3)],
                b"proof",
            ),
            program(
                BinaryProgramKind::Certified,
                &[a, indexed.integer(2).raw().clone()],
                b"proof",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            assert!(
                !equivalent_generated_programs(&original, &other, Default::default()).unwrap(),
                "semantic mismatch case {case} was accepted"
            );
        }
    }

    #[test]
    fn zero_and_constants_never_erase_variable_map_ownership() {
        let empty = CoefficientContext::try_new(Vec::<String>::new()).unwrap();
        let base =
            CoefficientContext::try_new(["native_compare_map_d", "native_compare_map_s"]).unwrap();
        let reversed =
            CoefficientContext::try_new(["native_compare_map_s", "native_compare_map_d"]).unwrap();
        let other =
            CoefficientContext::try_new(["native_compare_map_d", "native_compare_map_q"]).unwrap();
        for number in [0, 1, -3] {
            let values = [
                empty.integer(number),
                base.integer(number),
                reversed.integer(number),
                other.integer(number),
            ];
            // Confirm this fixture exercises native equality's constant case.
            assert_eq!(values[1], values[2]);
            let programs: Vec<_> = values
                .iter()
                .map(|value| {
                    program(
                        BinaryProgramKind::Certified,
                        std::slice::from_ref(value),
                        b"proof",
                    )
                })
                .collect();
            for (left, left_program) in programs.iter().enumerate() {
                for (right, right_program) in programs.iter().enumerate() {
                    assert_eq!(
                        equivalent_generated_programs(
                            left_program,
                            right_program,
                            Default::default()
                        )
                        .unwrap(),
                        left == right,
                        "constant {number}, maps {left}/{right}"
                    );
                }
            }
        }
    }

    #[test]
    fn ambient_native_resources_are_not_program_identity() {
        let context = CoefficientContext::try_new(["native_compare_ambient_d"]).unwrap();
        let value = context.integer(7);
        let before = program(BinaryProgramKind::Certified, &[value.clone()], b"proof");
        let unrelated = CoefficientContext::try_new([
            "native_compare_unrelated_d",
            "native_compare_unrelated_x",
        ])
        .unwrap();
        let _ = program(
            BinaryProgramKind::Candidates,
            &[unrelated.integer(11)],
            b"other",
        );
        let after = program(BinaryProgramKind::Certified, &[value], b"proof");
        assert!(equivalent_generated_programs(&before, &after, Default::default()).unwrap());
        // Do not require bytes to differ: another parallel test may already
        // have registered the same resources. Semantic equality is the gate.
    }
}
