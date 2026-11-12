//! coef-form: check that two generated programs (e.g. an imported 5L table
//! with expanded and with factored coefficients) carry the same program:
//! the same section tags in the same order, every section other than the
//! coefficient table byte-identical, and coefficient tables that decode to
//! identical coefficients (same ids, same values, same variables).
//!
//! coef_form_compare <a.rrbin> <b.rrbin>
use std::time::Instant;

use rustred::persistence::{
    BinaryIoLimits, CoefficientId, DecodedCoefficientTable, SectionTag, inspect_program,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let limits = BinaryIoLimits::default();
    let a_bytes = std::fs::read(&args[1]).unwrap();
    let b_bytes = std::fs::read(&args[2]).unwrap();
    let a = inspect_program(&a_bytes, limits).unwrap();
    let b = inspect_program(&b_bytes, limits).unwrap();
    assert!(a.kind() == b.kind(), "program kinds differ");
    assert_eq!(
        a.sections().len(),
        b.sections().len(),
        "section counts differ"
    );
    let mut other_sections = 0usize;
    for (sa, sb) in a.sections().iter().zip(b.sections()) {
        assert!(sa.tag == sb.tag, "section tags differ");
        if sa.tag != SectionTag::COEFFICIENTS {
            assert!(sa.bytes == sb.bytes, "a non-coefficient section differs");
            other_sections += 1;
        }
    }
    let decode = |e: &rustred::persistence::ProgramEnvelope<'_>| {
        let t = Instant::now();
        let table = DecodedCoefficientTable::import_generated(
            e.section(SectionTag::SYMBOLICA_STATE).unwrap(),
            e.section(SectionTag::COEFFICIENTS).unwrap(),
            limits,
        )
        .unwrap();
        (table, t.elapsed().as_secs_f64())
    };
    let (ta, sa) = decode(&a);
    let (tb, sb) = decode(&b);
    assert_eq!(ta.len(), tb.len(), "coefficient counts differ");
    for i in 0..ta.len() {
        let id = CoefficientId::try_from_index(i).unwrap();
        let (x, y) = (ta.coefficient(id).unwrap(), tb.coefficient(id).unwrap());
        assert!(
            x == y && x.get_variables() == y.get_variables(),
            "coefficient {i} differs"
        );
    }
    println!(
        "{{\"a\":\"{}\",\"b\":\"{}\",\"bytes_a\":{},\"bytes_b\":{},\"coefficients\":{},\"other_sections_identical\":{other_sections},\"decode_a_s\":{sa},\"decode_b_s\":{sb},\"identical\":true}}",
        args[1],
        args[2],
        a_bytes.len(),
        b_bytes.len(),
        ta.len()
    );
}
