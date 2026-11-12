//! coef-form: re-encode the coefficient section of a generated program
//! (custom owner payload, imported 5L table) in the factored format
//! (rustred::persistence::FACTORED_TABLE_VERSION), every other section
//! byte-identical; checks that the new file decodes to the identical
//! coefficient table.
//!
//! coef_form_convert <in.rrbin> <out.rrbin>
use std::time::Instant;

use rustred::persistence::{
    BinaryIoLimits, BinarySection, CoefficientId, DecodedCoefficientTable, SectionTag,
    encode_factored_table, encode_program, inspect_program,
};

fn table(state: &[u8], atoms: &[u8]) -> (DecodedCoefficientTable, f64) {
    let t = Instant::now();
    let table =
        DecodedCoefficientTable::import_generated(state, atoms, BinaryIoLimits::default()).unwrap();
    (table, t.elapsed().as_secs_f64())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = std::fs::read(&args[1]).unwrap();
    let limits = BinaryIoLimits::default();
    let envelope = inspect_program(&input, limits).unwrap();
    let state = envelope.section(SectionTag::SYMBOLICA_STATE).unwrap();
    let atoms = envelope.section(SectionTag::COEFFICIENTS).unwrap();
    let (old, old_s) = table(state, atoms);
    let coefficients: Vec<_> = (0..old.len())
        .map(|i| {
            old.coefficient(CoefficientId::try_from_index(i).unwrap())
                .unwrap()
                .clone()
        })
        .collect();
    let t = Instant::now();
    let factored = encode_factored_table(&coefficients).unwrap();
    let encode_s = t.elapsed().as_secs_f64();
    let sections: Vec<BinarySection<'_>> = envelope
        .sections()
        .iter()
        .map(|s| BinarySection {
            tag: s.tag,
            bytes: if s.tag == SectionTag::COEFFICIENTS {
                &factored
            } else {
                s.bytes
            },
        })
        .collect();
    let output = encode_program(envelope.kind(), &sections, limits).unwrap();
    let check = inspect_program(&output, limits).unwrap();
    let (new, new_s) = table(
        check.section(SectionTag::SYMBOLICA_STATE).unwrap(),
        check.section(SectionTag::COEFFICIENTS).unwrap(),
    );
    assert_eq!(new.len(), coefficients.len());
    for (i, c) in coefficients.iter().enumerate() {
        let v = new
            .coefficient(CoefficientId::try_from_index(i).unwrap())
            .unwrap();
        assert!(
            v == c && v.get_variables() == c.get_variables(),
            "coefficient {i} differs"
        );
    }
    std::fs::write(&args[2], &output).unwrap();
    println!(
        "{{\"input\":\"{}\",\"bytes_in\":{},\"bytes_out\":{},\"coef_section_in\":{},\"coef_section_out\":{},\"coefficients\":{},\"decode_in_s\":{old_s},\"decode_out_s\":{new_s},\"encode_s\":{encode_s},\"identical\":true}}",
        args[1],
        input.len(),
        output.len(),
        atoms.len(),
        factored.len(),
        coefficients.len()
    );
}
