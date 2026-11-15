//! NEED READ OFF A MAP (POLES_OBJECTIVE 2 (B): "what it costs is read off M"). For a map `X = M_X L` (lane poles'
//! master-map reader, `valuation::basis::read_master_map`; rows X = the basis whose need is asked, e.g. the published
//! masters P = A L of the reconstruction records, columns L) with L REGULAR on the band: a band key K = c_L(K) L has
//! coefficients `c_L(K) M_X^-1` on X, so pole_i(K) <= max_j pole((M_X^-1)[j][i]) (an UPPER bound), with EQUALITY when
//! L is contained in the band (K = L_j gives row j of M_X^-1). need(X_i) = base + that maximum. The exact orders come
//! from lane poles' exact check (`check_master_map`); nothing is recomputed here.

use rustred_campaign_core::regbasis::need::{Availability, NeedTable, need_table};
use serde_json::{Value, json};

use crate::valuation::basis::{MapCheck, MasterMap, check_master_map};
use crate::valuation::{D0, Valuation};

/// The need table of the rows of `map` (X), read off M_X^-1. `exact_if_contained`: the record states whether L was
/// certified regular and contained in the band (then the table is exact, else an upper bound).
pub fn need_from_map(
    map: &MasterMap,
    d0: &D0,
    base: i64,
    avail: Option<&dyn Availability>,
    exact_if_contained: bool,
) -> Result<(NeedTable, MapCheck, Value), String> {
    let chk = check_master_map(map, d0);
    let inv = chk
        .inverse_orders
        .as_ref()
        .ok_or_else(|| format!("map not invertible over Q(d): {:?}", chk.reason))?;
    let m_regular = chk
        .entry_orders
        .iter()
        .flatten()
        .all(|v| !matches!(v, Valuation::Order(o) if *o < 0));
    let n = map.rows.len();
    let poles: Vec<Option<u32>> = (0..n)
        .map(|i| {
            let mut mx: Option<u32> = None;
            for row in inv {
                if let Valuation::Order(o) = row[i] {
                    let p = if o < 0 { (-o) as u32 } else { 0 };
                    mx = Some(mx.map_or(p, |m| m.max(p)));
                }
            }
            mx
        })
        .collect();
    let keys = map
        .rows
        .iter()
        .map(|r| rustred_campaign_core::key::parse_key(r).ok())
        .collect::<Vec<_>>();
    let established = if exact_if_contained {
        "read off M_X^-1: exact (L certified regular on the band and contained in it)"
    } else {
        "read off M_X^-1: upper bound (exact when L is regular on the band and contained in it)"
    };
    let t = need_table("X", base, &map.rows, &keys, &poles, avail, established);
    let extra = json!({"M_X_regular": m_regular, "det_order": chk.det_order, "verdict": chk.verdict, "order": chk.order,
                       "rank_at_d0": chk.rank_at_d0, "null_directions": chk.null_directions, "L": map.cols});
    Ok((t, chk, extra))
}
