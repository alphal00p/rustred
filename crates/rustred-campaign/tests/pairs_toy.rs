//! Lane gencheck round 6 J3: the (key, image) d = 4 pair measure (`regularity-pairs`) on a toy owner table: a key sent to an
//! image whose first-match rule has a d = 4 pole is WORSE; the reverse is better; a declared-terminal image is regular; a key no
//! rule serves is not compared.
use rustred_campaign::emission::pairs::{Pair, measure};
use serde_json::json;

#[test]
fn pairs_worse_better_terminal_and_unserved() {
    let base = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into());
    let d = std::path::PathBuf::from(base).join(format!("pairs-toy-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let t = d.join("T.table.jsonl");
    let h = json!({"type":"header","schema":"table_dump.v1","family":"TOY","mask":"110","denominators":3,
                   "node_order":{"name":"id","slot_permutation":null,"coordinate_priority":null,"source_order":"Input"}});
    let r0 = json!({"type":"rule","chart":"110","index":0,"case":"Coordinate(A)","fixed":[1,null,null],"equations":[],"target":"I(1,n1,n2)",
                    "rhs":[["I(0,n1,n2)","1/(d-4)"]],"exceptions":[],"dispatch":"Partition","witness_loci":[]});
    let r1 = json!({"type":"rule","chart":"110","index":1,"case":"Coordinate(B)","fixed":[null,null,null],"equations":[],"target":"I(n0,n1,n2)",
                    "rhs":[["I(n0-1,n1,n2)","1"]],"exceptions":[["n0"]],"dispatch":"Partition","witness_loci":[]});
    std::fs::write(&t, format!("{h}\n{r0}\n{r1}\n")).unwrap();
    let p = |k: [i64; 3], i: [i64; 3], term: bool| Pair {
        key: k.to_vec(),
        image: i.to_vec(),
        sign: 1,
        image_is_terminal: term,
    };
    let pairs = vec![
        p([2, 1, 0], [1, 2, 0], false),
        p([1, 3, 0], [3, 1, 0], false),
        p([1, 4, 0], [0, 4, 0], true),
        p([0, 1, 0], [1, 0, 0], false),
    ];
    let (s, rows) = measure(&t, &pairs, 2, None).unwrap();
    eprintln!("{s:#}");
    assert_eq!(rows[0]["status"], "compared");
    assert_eq!(
        (
            rows[0]["old_pole"].as_u64(),
            rows[0]["new_pole"].as_u64(),
            rows[0]["worse"].as_bool()
        ),
        (Some(0), Some(1), Some(true))
    );
    assert_eq!(
        (
            rows[1]["old_pole"].as_u64(),
            rows[1]["new_pole"].as_u64(),
            rows[1]["worse"].as_bool()
        ),
        (Some(1), Some(0), Some(false))
    );
    assert_eq!(rows[2]["status"], "image_terminal");
    assert_eq!(
        rows[3]["status"], "key_unserved",
        "n0 = 0 is outside both domains"
    );
    assert_eq!(s["keys_with_a_worse_image"], 1);
    assert_eq!(s["counts"]["regular_to_singular"], 1);
    assert_eq!(s["counts"]["singular_to_regular"], 1);
    let _ = std::fs::remove_dir_all(&d);
}
