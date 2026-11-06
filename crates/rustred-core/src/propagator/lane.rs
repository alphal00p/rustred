//! Lanes and their roles (owner: M2-I0 rr-iface; decided per DAG node by M2-T3
//! in `decide.rs`). Roles are slots and loops of the node's presentation; the
//! meaning of each role is pinned by the contract checkers of `presentation.rs`.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OwnerLane {
    Zero,
    SeriesParallel,
    Insertion,
    Carpet,
    Triangle,
    /// M3.
    Diamond,
    Custom,
    /// Reserved.
    Product,
}

/// `lines[0]` carries `+-k_l`, `lines[1]` carries `+-(k_l - P)`, `l = loop_index`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BubbleRoles {
    pub lines: [u16; 2],
    pub loop_index: u8,
}

/// `r = (p - q)^2`, `third = p^2` (absent: `p` is only the blob's external
/// momentum), `p = k_l`, `l = outer_loop`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CarpetRoles {
    pub r: u16,
    pub third: Option<u16>,
    pub outer_loop: u8,
}

/// `b = k^2`, `a[i] = (k + p_i)^2`, `c[i] = p_i^2`, `k = k_l`, `l = k_loop`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TriangleRoles {
    pub b: u16,
    pub a: [u16; 2],
    pub c: [u16; 2],
    pub k_loop: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LaneRoles {
    None,
    Insertion(BubbleRoles),
    Carpet(CarpetRoles),
    Triangle(TriangleRoles),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LaneDecision {
    pub lane: OwnerLane,
    pub roles: LaneRoles,
    /// The trivial node: a terminal, no step.
    pub trivial: bool,
}
