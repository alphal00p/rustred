//! RECIPE RULES (R48; interface `interfaces/acceptance.md` section 10): a domain + node order + solve recipe, NO
//! coefficients, accepted only on a CERTIFIED KEY SET, per key, from the answers of the ENGINE's deterministic solve
//! (a child job; this module writes the request and reads the answers: no solving here).

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, json};

use super::prover::keys_digest;
use super::rule::sha256_tag;
use super::{AcceptError, NodeView, Stop, composed_stop_domain};
use crate::closure::bodies::{Sectors, TermClass};
use crate::cover::Dispatcher;
use crate::domain::{CaseDomain, DomainOptions};
use crate::key::Key;
use crate::record::{ReadOptions, RecordReader, RhsMode, TableLine};

pub const RECIPE_SCHEMA: &str = "rustred.campaign.recipe.v1";
pub const SOLVE_REQUEST_SCHEMA: &str = "rustred.campaign.recipe-solve-request.v1";
pub const SOLVE_ANSWER_SCHEMA: &str = "rustred.campaign.recipe-solve.v1";

/// A recipe rule: canonical body, digest and domain.
#[derive(Clone, Debug)]
pub struct Recipe {
    /// The coordinator's versioned DETERMINISM CONTRACT id (seed order, pivot rule, primes, elimination
    /// convention); with the engine binary digest part of the certification's identity.
    pub contract: String,
    pub body: Value,
    pub canonical: String,
    pub digest: String,
    pub domain: CaseDomain,
}

impl Recipe {
    /// `domain` = rule-like fields (`chart`, `fixed`, `equations`, `exceptions`, `target`); `order_digest` = the node
    /// comparator's identity; `solve` = the solve recipe (seed setting fields, elimination convention, symmetry rows).
    pub fn new(
        domain: &Value,
        order_digest: &str,
        solve: &Value,
        contract: &str,
        frame: &crate::key::NodeFrame,
    ) -> Result<Recipe, String> {
        let pick = |k: &str| domain.get(k).cloned().unwrap_or(Value::Null);
        let dom = json!({"chart": pick("chart"), "fixed": pick("fixed"), "equations": pick("equations"),
            "exceptions": pick("exceptions"), "target": pick("target")});
        let body = json!({"schema": RECIPE_SCHEMA, "domain": dom, "order": order_digest, "recipe": solve,
            "determinism_contract": contract, "version": 1});
        let canonical = crate::schema::canon::canonical_json(&body, false)?;
        let mut line = dom.clone();
        line["type"] = json!("rule");
        line["rhs"] = json!([]);
        if line["equations"].is_null() {
            line["equations"] = json!([]);
        }
        let text = serde_json::to_vec(&line).map_err(|e| e.to_string())?;
        let rec =
            match RecordReader::new(text.as_slice(), ReadOptions { rhs: RhsMode::Skip }).next() {
                Some(Ok(TableLine::Rule(r))) => *r,
                other => {
                    return Err(format!(
                        "recipe domain is not a rule-like record: {other:?}"
                    ));
                }
            };
        let domain = CaseDomain::from_rule_record(&rec, frame, DomainOptions::default())
            .map_err(|e| e.to_string())?;
        Ok(Recipe {
            contract: contract.to_string(),
            digest: sha256_tag(canonical.as_bytes()),
            body,
            canonical,
            domain,
        })
    }
}

/// One answer line of the engine's solve.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SolveAnswer {
    pub key: Key,
    pub run: u32,
    pub ok: bool,
    pub failure: Option<String>,
    pub rhs_keys: Vec<Key>,
    pub relation_digest: Option<String>,
    pub regular: String,
    pub exact_check: String,
    pub cost_ms: u64,
    pub binary_sha256: Option<String>,
    pub determinism_contract: Option<String>,
    /// The line as read (for the relation-set file).
    pub line: String,
}

/// The answers of one or more solve jobs of ONE recipe.
#[derive(Clone, Debug, Default)]
pub struct SolveAnswers {
    pub recipe_digest: String,
    map: BTreeMap<(Key, u32), SolveAnswer>,
}

impl SolveAnswers {
    pub fn new(recipe_digest: &str) -> Self {
        SolveAnswers {
            recipe_digest: recipe_digest.into(),
            map: BTreeMap::new(),
        }
    }
    /// Read an answer file of THIS recipe (interim schema of interface 10.2). No silent drop: a line of another schema
    /// or of another recipe is an error, as are conflicting lines.
    pub fn load(&mut self, text: &str) -> Result<usize, String> {
        let mut n = 0;
        for (no, l) in text.lines().enumerate() {
            if l.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(l).map_err(|e| format!("line {}: {e}", no + 1))?;
            if v["schema"] != SOLVE_ANSWER_SCHEMA {
                return Err(format!("line {}: schema {} is not {SOLVE_ANSWER_SCHEMA}", no + 1, v["schema"]));
            }
            if v["recipe_digest"] != self.recipe_digest.as_str() {
                return Err(format!(
                    "line {}: answer of recipe {}, this file answers {}",
                    no + 1,
                    v["recipe_digest"],
                    self.recipe_digest
                ));
            }
            let key: Key = serde_json::from_value(v["key"].clone())
                .map_err(|e| format!("line {}: key {e}", no + 1))?;
            let rhs: Vec<Key> = serde_json::from_value(v["rhs_keys"].clone()).unwrap_or_default();
            let a = SolveAnswer {
                key: key.clone(),
                run: v["run"].as_u64().unwrap_or(1) as u32,
                ok: v["status"] == "ok",
                failure: (!v["failure"].is_null()).then(|| v["failure"].to_string()),
                rhs_keys: rhs,
                relation_digest: v["relation_digest"].as_str().map(str::to_string),
                regular: v["regular"].as_str().unwrap_or("unknown").into(),
                exact_check: v["exact_check"].as_str().unwrap_or("not_run").into(),
                cost_ms: v["cost_ms"].as_u64().unwrap_or(0),
                binary_sha256: v["binary_sha256"].as_str().map(str::to_string),
                determinism_contract: v["determinism_contract"].as_str().map(str::to_string),
                line: crate::schema::canon::canonical_json(&v, false)
                    .unwrap_or_else(|_| l.to_string()),
            };
            let id = (key, a.run);
            if let Some(old) = self.map.get(&id)
                && old.relation_digest != a.relation_digest
            {
                // two answers of the SAME run disagreeing is an inconsistent file (determinism uses run 2)
                return Err(format!("line {}: conflicting answers for {:?}", no + 1, id));
            }
            self.map.insert(id, a);
            n += 1;
        }
        Ok(n)
    }
    /// Read the ENGINE verb's sealed `results.jsonl` (interfaces/coord-recipe-solve.md v0, section 7) as run `run`
    /// (two independent jobs = runs 1 and 2). `engine_recipe_digest` = the verb's recipe digest (BLAKE3 of its recipe
    /// file; must equal every record's), `binary` = the binary identity from the job's OUTCOME. Mapping: verdict
    /// SOLVED -> ok; `rhs` I(..) texts -> rhs keys (keys only; descent is re-checked by the chain); `relation_digest`
    /// -> `blake3:<hex>`; IRREGULAR -> regular "pole"; `proof.ok` -> exact check pass / fail (null -> not_run);
    /// `determinism_id` -> the determinism contract; `seconds` -> cost (volatile, cost only). Returns records read.
    /// SEALED (audit-accept-A M6): the last record must be the `end` record (`records` = the relation count, same recipe
    /// digest); a `partial` end, a missing end, a record after the end, any other record type, or two answers for one
    /// (key, run) with different relation digests are errors.
    pub fn load_engine_results(
        &mut self,
        text: &str,
        run: u32,
        engine_recipe_digest: &str,
        binary: &str,
    ) -> Result<usize, String> {
        let mut n = 0;
        let mut sealed = false;
        let mut staged: Vec<((Key, u32), SolveAnswer)> = Vec::new();
        for (no, l) in text.lines().enumerate() {
            if l.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(l).map_err(|e| format!("line {}: {e}", no + 1))?;
            if sealed {
                return Err(format!("line {}: a record after the end record", no + 1));
            }
            match v["type"].as_str() {
                Some("relation") => {}
                Some("end") => {
                    if v["records"].as_u64() != Some(n as u64) {
                        return Err(format!(
                            "line {}: end record counts {}, {n} relation records read",
                            no + 1,
                            v["records"]
                        ));
                    }
                    if !v["recipe_digest"].is_null() && v["recipe_digest"] != engine_recipe_digest {
                        return Err(format!("line {}: end record of another recipe", no + 1));
                    }
                    sealed = true;
                    continue;
                }
                Some("partial") => {
                    return Err(format!(
                        "line {}: partial end record (interrupted job): not a sealed result",
                        no + 1
                    ));
                }
                other => return Err(format!("line {}: unexpected record type {other:?}", no + 1)),
            }
            if v["recipe_digest"] != engine_recipe_digest {
                return Err(format!(
                    "line {}: record of recipe {} (expected {engine_recipe_digest})",
                    no + 1,
                    v["recipe_digest"]
                ));
            }
            let key: Key = serde_json::from_value(v["key"].clone())
                .map_err(|e| format!("line {}: key {e}", no + 1))?;
            let verdict = v["verdict"].as_str().unwrap_or("ERROR");
            let mut rhs = Vec::new();
            for t in v["rhs"].as_array().into_iter().flatten() {
                let k = t[0]
                    .as_str()
                    .and_then(crate::assemble::parse_key_text)
                    .ok_or_else(|| format!("line {}: unreadable rhs key {t}", no + 1))?;
                rhs.push(k);
            }
            let canon =
                crate::schema::canon::canonical_json(&v, true).unwrap_or_else(|_| l.to_string());
            let a = SolveAnswer {
                key: key.clone(),
                run,
                ok: verdict == "SOLVED",
                failure: (verdict != "SOLVED").then(|| {
                    format!(
                        "{verdict}: {}",
                        v["failure"]["message"].as_str().unwrap_or("")
                    )
                }),
                rhs_keys: rhs,
                relation_digest: v["relation_digest"].as_str().map(|h| format!("blake3:{h}")),
                regular: if verdict == "IRREGULAR" {
                    "pole".into()
                } else if verdict == "SOLVED" {
                    "regular".into()
                } else {
                    "unknown".into()
                },
                exact_check: match v["proof"]["ok"].as_bool() {
                    Some(true) => "pass".into(),
                    Some(false) => "fail".into(),
                    None => "not_run".into(),
                },
                cost_ms: (v["seconds"].as_f64().unwrap_or(0.0) * 1000.0) as u64,
                binary_sha256: Some(binary.to_string()),
                determinism_contract: v["determinism_id"].as_str().map(str::to_string),
                line: canon,
            };
            staged.push(((key, run), a));
            n += 1;
        }
        if !sealed {
            return Err("results without an end record (not sealed)".into());
        }
        // conflicts are checked before anything is kept (the answers stay as they were on an error)
        let mut seen: BTreeMap<(Key, u32), Option<String>> = BTreeMap::new();
        for (id, a) in &staged {
            let prev = self
                .map
                .get(id)
                .map(|o| o.relation_digest.clone())
                .or_else(|| seen.get(id).cloned());
            if let Some(p) = prev
                && p != a.relation_digest
            {
                return Err(format!("conflicting answers for {id:?}"));
            }
            seen.insert(id.clone(), a.relation_digest.clone());
        }
        for (id, a) in staged {
            self.map.insert(id, a);
        }
        Ok(n)
    }
    /// Every run-2 answer held (for the determinism check over ALL of them, audit-accept-B M6).
    pub fn run2_keys(&self) -> Vec<Key> {
        self.map
            .keys()
            .filter(|(_, r)| *r == 2)
            .map(|(k, _)| k.clone())
            .collect()
    }
    pub fn get(&self, key: &[i32], run: u32) -> Option<&SolveAnswer> {
        self.map.get(&(key.to_vec(), run))
    }
}

/// Determinism check of the spec (R48 reproducibility).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum Determinism {
    None,
    /// Every key's twin solve (count of sampled keys).
    Count(usize),
    /// Fraction in per mille of the certified candidates.
    PerMille(u32),
}

/// Inputs of one recipe certification.
pub struct RecipeInput<'a> {
    pub node: NodeView<'a>,
    pub before: Option<&'a Dispatcher>,
    pub after: Option<&'a Dispatcher>,
    pub recipe: &'a Recipe,
    /// The keys the recipe is to be certified on (e.g. its served keys of the node's closure).
    pub keys: &'a [Key],
    pub determinism: Determinism,
    /// Digest of the engine binary the certification is bound to (None = take the answers' binary, recorded).
    pub expected_binary: Option<String>,
}

/// Why a key is not certified.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenReason {
    NotDispatched(String),
    SolveFailed(String),
    ExactCheck(String),
    Irregular(String),
    NotDescending { rhs: Key },
    /// The answer is not a usable relation (an empty right-hand side, an rhs key of the wrong length; audit-accept-B m2).
    Malformed(String),
}

/// The request file of a solve job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SolveRequest {
    pub request: String,
    pub keys_file: String,
    pub keys_digest: String,
}

/// Verdict of a recipe certification.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum RecipeVerdict {
    Certified {
        recipe_digest: String,
        certified: Vec<Key>,
        certified_digest: String,
        relation_set_digest: String,
        /// canonical JSONL of the certified run-1 answers, sorted by key (the relation-set file)
        relation_set: String,
        open: Vec<(Key, OpenReason)>,
        binary_sha256: String,
        determinism_contract: String,
        cost_ms: u64,
        determinism_checked: usize,
    },
    Rejected {
        what: String,
    },
    Undecided {
        what: String,
    },
    NeedsExternal {
        requests: Vec<SolveRequest>,
    },
}

/// The request for `keys` (`run2` = the determinism sample, solved a second time independently).
pub fn solve_request(
    recipe: &Recipe,
    keys: &[Key],
    run2: &[Key],
    binary: Option<&str>,
) -> SolveRequest {
    let mut kf = String::new();
    for k in keys {
        kf.push_str(&k.iter().map(i32::to_string).collect::<Vec<_>>().join(","));
        kf.push('\n');
    }
    let arity = recipe.domain.arity;
    let kd = keys_digest(arity, keys);
    let req = json!({"schema": SOLVE_REQUEST_SCHEMA, "recipe_digest": recipe.digest, "recipe": recipe.body,
        "keys_file": "KEYS", "keys_digest": kd, "runs": if run2.is_empty() { json!([1]) } else { json!([1, 2]) },
        "run2_keys": run2, "binary_sha256_expected": binary, "determinism_contract": recipe.contract});
    SolveRequest {
        request: format!("{req}\n"),
        keys_file: kf,
        keys_digest: kd,
    }
}

fn sample(cands: &[Key], d: Determinism) -> Vec<Key> {
    let want = match d {
        Determinism::None => 0,
        Determinism::Count(c) => c.min(cands.len()),
        Determinism::PerMille(p) => {
            ((cands.len() as u64 * p as u64).div_ceil(1000) as usize).min(cands.len())
        }
    };
    if want == 0 {
        return vec![];
    }
    let step = cands.len() / want;
    (0..want).map(|i| cands[i * step.max(1)].clone()).collect()
}

/// Certify a recipe on its key set from the solve answers (interface 10.3).
pub fn accept_recipe(
    input: &RecipeInput,
    answers: &SolveAnswers,
) -> Result<RecipeVerdict, AcceptError> {
    let node = &input.node;
    if answers.recipe_digest != input.recipe.digest {
        return Ok(RecipeVerdict::Undecided {
            what: "answers of another recipe digest".into(),
        });
    }
    let sectors = Sectors {
        lines: node.lines(),
        zero_sectors: node.zero_sectors.clone(),
    };
    let mut keys: Vec<Key> = input.keys.to_vec();
    keys.sort();
    keys.dedup();
    let mut open: Vec<(Key, OpenReason)> = Vec::new();
    let mut cands: Vec<Key> = Vec::new();
    for k in &keys {
        if k.len() != node.arity() {
            return Err(AcceptError::Arity {
                expected: node.arity(),
                got: k.len(),
            });
        }
        match composed_stop_domain(
            node,
            input.before,
            Some(&input.recipe.domain),
            input.after,
            k,
        )? {
            Stop::Served => cands.push(k.clone()),
            other => open.push((k.clone(), OpenReason::NotDispatched(format!("{other:?}")))),
        }
    }
    let run2 = sample(&cands, input.determinism);
    let missing: Vec<Key> = cands
        .iter()
        .filter(|k| answers.get(k, 1).is_none())
        .cloned()
        .collect();
    let missing2: Vec<Key> = run2
        .iter()
        .filter(|k| answers.get(k, 2).is_none())
        .cloned()
        .collect();
    if !missing.is_empty() || !missing2.is_empty() {
        let keys_req: Vec<Key> = if missing.is_empty() {
            missing2.clone()
        } else {
            missing
        };
        return Ok(RecipeVerdict::NeedsExternal {
            requests: vec![solve_request(
                input.recipe,
                &keys_req,
                &missing2,
                input.expected_binary.as_deref(),
            )],
        });
    }
    // audit-accept-A M6: the certification is bound to a STATED engine binary (never taken from the answers)
    let Some(binary) = input.expected_binary.clone() else {
        return Ok(RecipeVerdict::Undecided {
            what: "no expected engine binary: a certification is bound to the binary that solved".into(),
        });
    };
    // audit-accept-B M6: every run-2 answer held for a candidate is a determinism witness, sampled or not
    let mut det_keys: Vec<Key> = run2.clone();
    for k in answers.run2_keys() {
        if cands.contains(&k) && !det_keys.contains(&k) {
            det_keys.push(k);
        }
    }
    for k in det_keys.iter() {
        let (a, b) = (answers.get(k, 1).unwrap(), answers.get(k, 2).unwrap());
        if a.binary_sha256.as_deref() != Some(binary.as_str())
            || b.binary_sha256.as_deref() != Some(binary.as_str())
        {
            return Ok(RecipeVerdict::Undecided {
                what: format!("stale: determinism answers at {k:?} not from binary {binary}"),
            });
        }
        if (a.ok, &a.relation_digest) != (b.ok, &b.relation_digest) {
            return Ok(RecipeVerdict::Rejected {
                what: format!(
                    "determinism: two solves at {k:?} give {:?} and {:?}",
                    a.relation_digest, b.relation_digest
                ),
            });
        }
    }
    let mut certified = Vec::new();
    let mut rel = String::new();
    let mut cost = 0u64;
    'k: for k in &cands {
        let a = answers.get(k, 1).unwrap();
        if a.binary_sha256.as_deref() != Some(binary.as_str())
            || a.determinism_contract.as_deref() != Some(input.recipe.contract.as_str())
        {
            return Ok(RecipeVerdict::Undecided {
                what: format!(
                    "stale: answer at {k:?} from binary {:?} / contract {:?}, expected {binary} / {}",
                    a.binary_sha256, a.determinism_contract, input.recipe.contract
                ),
            });
        }
        if !a.ok || a.relation_digest.is_none() {
            open.push((
                k.clone(),
                OpenReason::SolveFailed(a.failure.clone().unwrap_or_default()),
            ));
            continue;
        }
        if a.exact_check != "pass" {
            open.push((k.clone(), OpenReason::ExactCheck(a.exact_check.clone())));
            continue;
        }
        if a.regular != "regular" {
            open.push((k.clone(), OpenReason::Irregular(a.regular.clone())));
            continue;
        }
        if a.rhs_keys.is_empty() {
            open.push((
                k.clone(),
                OpenReason::Malformed("empty right-hand side (I(k) = 0 needs a zero-sector statement)".into()),
            ));
            continue;
        }
        if let Some(r) = a.rhs_keys.iter().find(|r| r.len() != node.arity()) {
            open.push((
                k.clone(),
                OpenReason::Malformed(format!("rhs key {r:?} of length {}", r.len())),
            ));
            continue;
        }
        for r in &a.rhs_keys {
            let fine = match sectors.classify_masked(r, 0) {
                TermClass::Subsector { .. } | TermClass::ZeroSector { .. } => true,
                TermClass::Top => node.cmp.try_cmp(r, k) == Ok(std::cmp::Ordering::Less),
                TermClass::PositiveNumerator { .. } => false,
            };
            if !fine {
                open.push((k.clone(), OpenReason::NotDescending { rhs: r.clone() }));
                continue 'k;
            }
        }
        cost += a.cost_ms;
        rel.push_str(&a.line);
        rel.push('\n');
        certified.push(k.clone());
    }
    if certified.is_empty() {
        return Ok(RecipeVerdict::Rejected {
            what: format!("no key certified ({} open)", open.len()),
        });
    }
    let determinism_checked = det_keys.iter().filter(|k| certified.contains(k)).count();
    Ok(RecipeVerdict::Certified {
        recipe_digest: input.recipe.digest.clone(),
        certified_digest: keys_digest(node.arity(), &certified),
        relation_set_digest: sha256_tag(rel.as_bytes()),
        relation_set: rel,
        certified,
        open,
        binary_sha256: binary,
        determinism_contract: input.recipe.contract.clone(),
        cost_ms: cost,
        determinism_checked,
    })
}

/// A certified recipe as a dispatch entry: it MATCHES a key only if the key is in its domain AND in its certified
/// set; otherwise dispatch FALLS THROUGH to later entries (coordinator ruling, R48 addendum 2).
pub struct RecipeEntry<'a> {
    pub domain: &'a CaseDomain,
    pub certified: &'a std::collections::BTreeSet<Key>,
}

impl RecipeEntry<'_> {
    pub fn matches(&self, key: &[i32]) -> bool {
        self.certified.contains(key) && self.domain.applies(key).applies()
    }
}
