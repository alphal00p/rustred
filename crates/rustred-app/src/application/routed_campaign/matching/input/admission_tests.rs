use super::*;
use serde_json::json;

fn document(count: usize) -> String {
    json!({"schema":"rustred.owner-domain-queries.json.v2","queries":
        (0..count).map(|i| json!({"id":format!("query-{i:064}"),"owner":"1","lower":[0],
            "upper":[null],"max_numerator_rank":null})).collect::<Vec<_>>()})
    .to_string()
}

#[test]
fn byte_admission_is_exact_including_utf8_and_precedes_json_parsing() {
    let text = document(1).replace("query-", "quéry-");
    assert!(parse(&text, 1, 1, text.len()).is_ok());
    assert!(
        parse(&text, 1, 1, text.len() - 1)
            .unwrap_err()
            .to_string()
            .contains("byte allowance")
    );
    let invalid = "not json";
    assert!(
        parse(invalid, 1, 1, 1)
            .unwrap_err()
            .to_string()
            .contains("byte allowance")
    );
    for (count, bytes) in [(0, 1), (1, 0), (0, 0)] {
        assert!(
            parse("", 1, count, bytes)
                .unwrap_err()
                .to_string()
                .contains("positive")
        );
    }
}

#[test]
fn explicit_count_is_not_an_implicit_ten_thousand_cap_or_allocation_size() {
    let text = document(10_001);
    assert_eq!(parse(&text, 1, 10_001, text.len()).unwrap().len(), 10_001);
    assert!(
        parse(&text, 1, 10_000, text.len())
            .unwrap_err()
            .to_string()
            .contains("query count")
    );
    assert!(parse(&text, 1, 10_001, 1024 * 1024).is_err());
    // Huge logical allowances must not reserve huge arrays for a tiny input.
    assert_eq!(
        parse(&document(1), 1, usize::MAX, usize::MAX)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn malformed_last_query_is_not_admitted_as_a_valid_prefix() {
    let mut doc: Value = serde_json::from_str(&document(10_001)).unwrap();
    doc["queries"][10_000]["upper"] = json!([-1]);
    let text = doc.to_string();
    assert!(
        parse(&text, 1, 10_001, text.len())
            .unwrap_err()
            .to_string()
            .contains("upper bounds")
    );
}
