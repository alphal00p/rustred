//! Zero-sector censuses for `proof-symrows` tables (lane symcert). A producer that PRUNED zero-sector terms from a
//! relation (they vanish) makes the kernel drop the same columns (`--drop-zero-sectors`), modulo the census written
//! into the table; the census is data of the request with its producer named: an engine table's own `zero_sectors`
//! record (preferred: the engine's census), or the family's zero criterion over every subset of the chart's lines
//! (our algebra, named as such). Generic: masks are strings over every slot; the line set is the chart (data).

use super::ZeroSectors;
use serde_json::Value;
use std::io::BufRead;

/// The census of an engine table's `zero_sectors` record line.
pub fn from_record_line(line: &str, producer: &str) -> Result<ZeroSectors, String> {
    let v: Value = serde_json::from_str(line).map_err(|e| format!("zero_sectors record: {e}"))?;
    if v["type"] != "zero_sectors" {
        return Err("not a zero_sectors record".into());
    }
    let masks: Vec<String> = v["masks"]
        .as_array()
        .ok_or("zero_sectors without masks")?
        .iter()
        .map(|m| m.as_str().map(str::to_string).ok_or("mask is not a text"))
        .collect::<Result<_, _>>()?;
    Ok(ZeroSectors {
        producer: producer.to_string(),
        scope: v["scope"].as_str().unwrap_or("unknown").to_string(),
        masks,
    })
}

/// The first `zero_sectors` record of a table file (read up to the first rule record), if any.
pub fn table_record(path: &std::path::Path) -> Result<Option<String>, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    for l in std::io::BufReader::new(f).lines() {
        let l = l.map_err(|e| format!("{}: {e}", path.display()))?;
        if l.starts_with("{\"type\":\"rule\"") {
            return Ok(None);
        }
        if l.contains("\"type\":\"zero_sectors\"") {
            return Ok(Some(l));
        }
    }
    Ok(None)
}

/// The census of a zero predicate over every subset of the chart's lines: a subset is zero iff `zero(key)` holds for
/// the key with its lines at power 1 and every other slot 0. `max_lines` bounds the enumeration (2^lines subsets).
pub fn from_predicate(
    chart: &[bool],
    zero: &dyn Fn(&[i64]) -> bool,
    producer: &str,
    max_lines: usize,
) -> Result<ZeroSectors, String> {
    let lines: Vec<usize> = (0..chart.len()).filter(|&i| chart[i]).collect();
    if lines.len() > max_lines {
        return Err(format!(
            "{} chart lines (enumeration bound {max_lines})",
            lines.len()
        ));
    }
    let mut masks = vec![];
    for bits in 0u64..(1u64 << lines.len()) {
        let mut key = vec![0i64; chart.len()];
        for (j, &i) in lines.iter().enumerate() {
            if bits >> j & 1 == 1 {
                key[i] = 1;
            }
        }
        if zero(&key) {
            masks.push(super::mask_of(&key));
        }
    }
    masks.sort();
    Ok(ZeroSectors {
        producer: producer.to_string(),
        scope: "chart_subsets".into(),
        masks,
    })
}

/// Compare two censuses on the subsets of the chart (second route of a census): masks only in `a`, only in `b`.
pub fn compare(a: &ZeroSectors, b: &ZeroSectors, chart: &[bool]) -> (Vec<String>, Vec<String>) {
    let inside = |m: &str| m.chars().zip(chart).all(|(c, &line)| c == '0' || line);
    let sa: std::collections::BTreeSet<&str> = a
        .masks
        .iter()
        .map(String::as_str)
        .filter(|m| inside(m))
        .collect();
    let sb: std::collections::BTreeSet<&str> = b
        .masks
        .iter()
        .map(String::as_str)
        .filter(|m| inside(m))
        .collect();
    (
        sa.difference(&sb).map(|s| s.to_string()).collect(),
        sb.difference(&sa).map(|s| s.to_string()).collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicate_census_and_comparison() {
        let chart = [true, true, true, false];
        // toy criterion: a sector is zero iff it lacks line 0 or line 1
        let z = |k: &[i64]| k[0] <= 0 || k[1] <= 0;
        let c = from_predicate(&chart, &z, "toy", 8).unwrap();
        assert_eq!(c.masks.len(), 6); // 8 subsets minus {0,1} and {0,1,2}
        assert!(c.masks.contains(&"0010".to_string()));
        assert!(!c.masks.contains(&"1100".to_string()));
        let rec = format!(
            "{{\"type\":\"zero_sectors\",\"scope\":\"downset\",\"masks\":{}}}",
            serde_json::to_string(&c.masks[1..]).unwrap()
        );
        let e = from_record_line(&rec, "engine").unwrap();
        let (only_a, only_b) = compare(&c, &e, &chart);
        assert_eq!((only_a.len(), only_b.len()), (1, 0));
        assert!(from_predicate(&chart, &z, "toy", 2).is_err());
    }
}
