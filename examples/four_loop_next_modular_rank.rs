//! Build the exact four-loop next-shell parent matrix and run the default
//! finite-field rank discovery images.
//!
//! The printed ranks and pivot skeletons are discovery evidence only.  They
//! are not exact `Q(d)` reduction claims.

use std::error::Error;

use rustred::{
    FourLoopComponentTransport, FourLoopComponentTransportConfig, FourLoopNextClosedRows,
    FourLoopNextClosedRowsConfig, FourLoopNextInventory, FourLoopNextInventoryConfig,
    FourLoopT1S2Closure, FourLoopT1S2ClosureConfig, FourLoopThreeLoopClosure,
    FourLoopThreeLoopClosureConfig, discover_four_loop_next_modular_rank,
};

fn main() -> Result<(), Box<dyn Error>> {
    let inventory = FourLoopNextInventory::build(FourLoopNextInventoryConfig::default())?;
    let transport =
        FourLoopComponentTransport::build(&inventory, FourLoopComponentTransportConfig::default())?;
    let t1s2 = FourLoopT1S2Closure::build(&transport, FourLoopT1S2ClosureConfig::default())?;
    let three_loop =
        FourLoopThreeLoopClosure::build(&transport, FourLoopThreeLoopClosureConfig::default())?;
    let closed = FourLoopNextClosedRows::build(
        &inventory,
        &transport,
        &t1s2,
        &three_loop,
        FourLoopNextClosedRowsConfig::default(),
    )?;
    let report = discover_four_loop_next_modular_rank(&closed)?;
    println!("{report}");
    Ok(())
}
