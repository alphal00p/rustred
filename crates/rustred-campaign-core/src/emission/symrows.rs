//! THE SYMMETRY-RELATION CHECK of an emitted record (emission contract section 15; round 5 H0 GAP-1, lane t266's request). A
//! record that states it is the image of its target under a symmetry element (`origin: {kind: "symmetry_relation", element:
//! e}`) is proven by `rr_proof --mode instance-span --symmetry-group G --symrows R --symrows-only --emit-certs C`: R is ONE
//! expanded symmetry row at the record's key (`{element: e, offset: 0^arity}`; the kernel derives the line permutation and
//! the numerator decomposition from the element's matrix and checks unimodularity), the IBP rows are not used, and C is the
//! exact certificate (weights of the symmetry row) BOUND to the record bytes (`record_sha256`). Without the rows the kernel
//! refuses a seedless record (`MISSING_SEEDS`). PURE: the request, and the binding of output + certificate + request to a
//! record body.
use super::body::sha256_hex;
use serde_json::{Value, json};

/// The symmetry element a record names (`origin.kind == "symmetry_relation"`), if any.
pub fn element(body: &str) -> Option<u64> {
    let v: Value = serde_json::from_str(body).ok()?;
    if v["origin"]["kind"].as_str() != Some("symmetry_relation") {
        return None;
    }
    v["origin"]["element"].as_u64()
}

/// The symrows request of a record (one line): element `e` at the record's own key (offset 0 in every slot).
pub fn request(body: &str, e: u64) -> Result<String, String> {
    let v: Value = serde_json::from_str(body).map_err(|x| format!("record: {x}"))?;
    let arity = v["fixed"]
        .as_array()
        .map(Vec::len)
        .ok_or("a symmetry relation must be a point record (no `fixed` key)")?;
    Ok(json!({"element": e, "id": format!("element {e}"), "offset": vec![0; arity]}).to_string())
}

/// The output of a symrows-only check, its certificate and its request, BOUND to `body`: the output says the rows were
/// given and used alone; the certificate was made on these record bytes, at this rule and key, with a symmetry row of
/// the stated element; the request names that element at offset 0.
pub fn verify(out: &str, cert: &[u8], req: &[u8], body: &str) -> Result<(), String> {
    let e = element(body).ok_or("the record names no symmetry element")?;
    let rec: Value = serde_json::from_str(body).map_err(|x| format!("record: {x}"))?;
    let o: Value = serde_json::from_str(out).map_err(|x| format!("span output: {x}"))?;
    if o["symrows"]["only"].as_bool() != Some(true) || o["symrows"]["given"].as_u64().unwrap_or(0) == 0 {
        return Err(format!("the check did not run on the symmetry rows alone (symrows {})", o["symrows"]));
    }
    let sha = sha256_hex(body.as_bytes());
    if o["record_sha256"].as_str() != Some(sha.as_str()) {
        return Err("the check output was made on another record line".into());
    }
    let r: Value = serde_json::from_slice(req).map_err(|x| format!("symrows request: {x}"))?;
    let arity = rec["fixed"].as_array().map_or(0, Vec::len);
    let zero = r["offset"].as_array().is_some_and(|a| a.len() == arity && a.iter().all(|x| x.as_i64() == Some(0)));
    if r["element"].as_u64() != Some(e) || !zero {
        return Err(format!("the symrows request is not element {e} at the record's key: {r}"));
    }
    let c: Value = serde_json::from_slice(cert).map_err(|x| format!("certificate: {x}"))?;
    if c["record_sha256"].as_str() != Some(sha.as_str()) {
        return Err("the certificate was made on another record line".into());
    }
    if c["rule"] != rec["index"] {
        return Err(format!("the certificate is of rule {}, the record is {}", c["rule"], rec["index"]));
    }
    if c["key"] != rec["fixed"] {
        return Err(format!("the certificate is at key {}, the record's key is {}", c["key"], rec["fixed"]));
    }
    // weights rows: [seed index, source row m, W]; m < 0 = a symmetry row (a given symrows row is m = -1000000 - j, as in
    // lane t266's K2 certificate); the certificate must use at least one symmetry row and no IBP row (m >= 0)
    let w = c["weights"].as_array().ok_or("certificate without weights")?;
    let ms: Vec<i64> = w.iter().filter_map(|x| x[1].as_i64()).collect();
    if ms.is_empty() || ms.len() != w.len() || ms.iter().any(|m| *m >= 0) {
        return Err(format!("the certificate's weights are not on symmetry rows alone: {ms:?}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_symmetry_relation_binds_output_certificate_and_request_to_its_bytes() {
        let body = json!({"type":"rule","chart":"110","index":0,"fixed":[1,2,0],"target":"I(1,2,0)","rhs":[["I(2,1,0)","1"]],
            "origin":{"kind":"symmetry_relation","element":3}})
        .to_string();
        assert_eq!(element(&body), Some(3));
        let req = request(&body, 3).unwrap();
        let sha = sha256_hex(body.as_bytes());
        let out = json!({"mode":"instance-span","record_sha256":sha,"symrows":{"given":1,"only":true}}).to_string();
        let cert = json!({"rule":0,"record_sha256":sha,"key":[1,2,0],"weights":[[0,-1000000,["1"]]]}).to_string();
        assert_eq!(verify(&out, cert.as_bytes(), req.as_bytes(), &body), Ok(()));
        // another record's certificate, the IBP rows used, another element, rows not alone: refused
        let other = cert.replace(&sha, &"0".repeat(64));
        assert!(verify(&out, other.as_bytes(), req.as_bytes(), &body).is_err());
        let ibp = json!({"rule":0,"record_sha256":sha,"key":[1,2,0],"weights":[[0,4,["1"]]]}).to_string();
        assert!(verify(&out, ibp.as_bytes(), req.as_bytes(), &body).is_err());
        let req2 = request(&body, 2).unwrap();
        assert!(verify(&out, cert.as_bytes(), req2.as_bytes(), &body).is_err());
        let mixed = json!({"mode":"instance-span","record_sha256":sha,"symrows":{"given":1,"only":false}}).to_string();
        assert!(verify(&mixed, cert.as_bytes(), req.as_bytes(), &body).is_err());
        assert_eq!(element(&body.replace("symmetry_relation", "recipe")), None);
    }
}
