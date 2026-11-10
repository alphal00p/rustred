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

/// Most paths of a diamond (M3-T5b): `L + 1 <= 8`, every node up to 7 loops.
pub const DIAMOND_MAX_PATHS: usize = 8;

/// Path `i < paths` of a diamond runs top - `x_i` - bottom: `a[i] = (k_i + p_i)^2`
/// (top to `x_i`), `b[i] = k_i^2` (`x_i` to bottom), `c[i] = p_i^2` (the third line
/// at `x_i`); `spectator` joins top and bottom. Loop `i < paths - 1` is
/// `k_i = mom(b[i])`, then the spectator's loop; `k_last = -Sum k_i - l` runs
/// through the last path. Entries past `paths` are 0, so equal diamonds are equal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DiamondRoles {
    a: [u16; DIAMOND_MAX_PATHS],
    b: [u16; DIAMOND_MAX_PATHS],
    c: [u16; DIAMOND_MAX_PATHS],
    paths: u8,
    pub spectator: Option<u16>,
}

impl DiamondRoles {
    /// `None` unless `a`, `b`, `c` have one common length in `2..=DIAMOND_MAX_PATHS`.
    pub fn new(a: &[u16], b: &[u16], c: &[u16], spectator: Option<u16>) -> Option<Self> {
        let paths = a.len();
        if !(2..=DIAMOND_MAX_PATHS).contains(&paths) || b.len() != paths || c.len() != paths {
            return None;
        }
        let padded = |lines: &[u16]| {
            let mut padded = [0; DIAMOND_MAX_PATHS];
            padded[..paths].copy_from_slice(lines);
            padded
        };
        Some(Self {
            a: padded(a),
            b: padded(b),
            c: padded(c),
            paths: u8::try_from(paths).ok()?,
            spectator,
        })
    }

    pub fn a(&self) -> &[u16] {
        &self.a[..usize::from(self.paths)]
    }

    pub fn b(&self) -> &[u16] {
        &self.b[..usize::from(self.paths)]
    }

    pub fn c(&self) -> &[u16] {
        &self.c[..usize::from(self.paths)]
    }

    pub fn paths(&self) -> usize {
        usize::from(self.paths)
    }

    /// The loops of the diamond, `L + S`: loops `0..L + S` of its presentation.
    pub fn loops(&self) -> usize {
        self.paths() - 1 + usize::from(self.spectator.is_some())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LaneRoles {
    None,
    Insertion(BubbleRoles),
    Carpet(CarpetRoles),
    Triangle(TriangleRoles),
    /// M3-T5b.
    Diamond(DiamondRoles),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LaneDecision {
    pub lane: OwnerLane,
    pub roles: LaneRoles,
    /// The trivial node: a terminal, no step.
    pub trivial: bool,
}
