// Read-only decoder of a CP5 index section (bincode 2 standard, varint) plus
// the nodes flag section: live-candidate census and "inspected then covered".
use std::fs;

struct R<'a> {
    b: &'a [u8],
    p: usize,
}
impl<'a> R<'a> {
    fn u8(&mut self) -> u8 {
        let v = self.b[self.p];
        self.p += 1;
        v
    }
    fn var(&mut self) -> u128 {
        let t = self.u8();
        match t {
            0..=250 => t as u128,
            251 => {
                let v = u16::from_le_bytes(self.b[self.p..self.p + 2].try_into().unwrap());
                self.p += 2;
                v as u128
            }
            252 => {
                let v = u32::from_le_bytes(self.b[self.p..self.p + 4].try_into().unwrap());
                self.p += 4;
                v as u128
            }
            253 => {
                let v = u64::from_le_bytes(self.b[self.p..self.p + 8].try_into().unwrap());
                self.p += 8;
                v as u128
            }
            254 => {
                let v = u128::from_le_bytes(self.b[self.p..self.p + 16].try_into().unwrap());
                self.p += 16;
                v
            }
            _ => panic!("bad varint tag at {}", self.p),
        }
    }
    fn opt(&mut self) -> Option<u128> {
        match self.u8() {
            0 => None,
            1 => Some(self.var()),
            x => panic!("bad option {x} at {}", self.p),
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let idx = fs::read(&a[1]).unwrap();
    let nodes = fs::read(&a[2]).unwrap();
    let flags = &nodes[32..];
    let n = flags.len();
    let mut live = vec![false; n];
    let mut r = R { b: &idx, p: 32 };
    let buckets = r.var() as usize;
    let mut bucket_live: Vec<usize> = Vec::new();
    let mut bucket_groups: Vec<usize> = Vec::new();
    let mut total_blocks = 0usize;
    let mut total_slots = 0usize;
    let mut live_total = 0usize;
    let mut ids_lane = 0usize;
    let mut orthants = 0usize;
    let mut groups_hist = std::collections::BTreeMap::<usize, usize>::new();
    let mut id_span_sum: f64 = 0.0;
    let mut id_span_blocks = 0usize;
    for _ in 0..buckets {
        let _phase = r.var();
        let olen = r.var() as usize;
        r.p += olen; // Vec<bool>
        let nids = r.var() as usize;
        for _ in 0..nids {
            r.var();
        }
        ids_lane += nids;
        let ngroups = r.var() as usize;
        let mut blive = 0usize;
        for _ in 0..ngroups {
            // Signature
            match r.var() {
                0 => {}
                1 => {
                    for _ in 0..2 {
                        match r.var() {
                            0 => {
                                r.var();
                            }
                            1 => {}
                            x => panic!("upper {x}"),
                        }
                    }
                    match r.var() {
                        0 => {}
                        1 => {
                            r.var();
                        }
                        x => panic!("lower {x}"),
                    }
                }
                x => panic!("signature {x} at {}", r.p),
            }
            let nblocks = r.var() as usize;
            for _ in 0..nblocks {
                let mut ids = [0usize; 32];
                for i in 0..32 {
                    ids[i] = r.var() as usize;
                }
                let len = r.var() as usize;
                let env = r.var() as usize;
                for _ in 0..env {
                    r.var();
                    r.var();
                    r.opt();
                    r.opt();
                }
                total_blocks += 1;
                total_slots += len;
                for &id in &ids[..len] {
                    if id < n {
                        live[id] = true;
                    }
                }
                if len >= 2 {
                    id_span_sum += (ids[len - 1] - ids[0]) as f64 / (len - 1) as f64;
                    id_span_blocks += 1;
                }
            }
            let glive = r.var() as usize;
            blive += glive;
        }
        let alive = r.var() as usize;
        assert_eq!(alive, blive);
        if r.opt().is_some() {
            orthants += 1;
        }
        live_total += blive;
        bucket_live.push(blive);
        bucket_groups.push(ngroups);
        *groups_hist.entry(ngroups).or_default() += 1;
    }
    assert_eq!(r.p, idx.len(), "trailing bytes");
    let (mut insp, mut sealed, mut closed) = (0usize, 0usize, 0usize);
    let (mut insp_notlive, mut insp_live, mut deleg, mut pend_live, mut pend_retired) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    for (id, &f) in flags.iter().enumerate() {
        let s = f & 1 != 0;
        let i = f & 2 != 0;
        if i {
            insp += 1;
        }
        if s {
            sealed += 1;
        }
        if f & 4 != 0 {
            closed += 1;
        }
        match (i, s, live[id]) {
            (true, _, false) => insp_notlive += 1,
            (true, _, true) => insp_live += 1,
            (false, true, _) => deleg += 1,
            (false, false, true) => pend_live += 1,
            (false, false, false) => pend_retired += 1,
        }
    }
    bucket_live.sort_unstable_by(|a, b| b.cmp(a));
    let top: usize = bucket_live.iter().take(1).sum();
    let top10: usize = bucket_live.iter().take(10).sum();
    bucket_groups.sort_unstable_by(|a, b| b.cmp(a));
    let gsum: usize = bucket_groups.iter().sum();
    println!("nodes {n} buckets {buckets} live_candidates {live_total} ids_lane {ids_lane} orthant_buckets {orthants}");
    println!("blocks {total_blocks} slots_used {total_slots} fill {:.3}", total_slots as f64 / (32.0 * total_blocks as f64));
    println!("mean_id_gap_within_block {:.0}", id_span_sum / id_span_blocks as f64);
    println!("top_bucket_live_share {:.3} top10 {:.3}", top as f64 / live_total as f64, top10 as f64 / live_total as f64);
    println!("groups total {gsum} max_per_bucket {} median {}", bucket_groups[0], bucket_groups[bucket_groups.len() / 2]);
    println!("flags: inspected {insp} sealed {sealed} closed {closed}");
    println!("inspected_not_live(covered after inspection) {insp_notlive} inspected_live {insp_live} delegated(sealed,not inspected) {deleg} pending_live {pend_live} pending_not_live {pend_retired}");
}
