use rustred::{
    FourLoopComponentTransport, FourLoopComponentTransportConfig, FourLoopNextInventory,
    FourLoopNextInventoryConfig, FourLoopThreeLoopClosure, FourLoopThreeLoopClosureConfig,
};

fn main() {
    let inventory = FourLoopNextInventory::build(FourLoopNextInventoryConfig::default()).unwrap();
    let transport =
        FourLoopComponentTransport::build(&inventory, FourLoopComponentTransportConfig::default())
            .unwrap();
    let closure =
        FourLoopThreeLoopClosure::build(&transport, FourLoopThreeLoopClosureConfig::default())
            .unwrap();
    println!("status={:?}", closure.status());
    println!("stats={:?}", closure.stats());
    println!("service_stats={:?}", closure.service().stats());
    println!("pipeline_stats={:?}", closure.service().pipeline_stats());
    println!(
        "manifest_checksum={:016x}",
        closure.service().manifest_checksum()
    );
    println!("service_checksum={:016x}", closure.service().checksum());
    println!("closure_checksum={:016x}", closure.checksum());
}
