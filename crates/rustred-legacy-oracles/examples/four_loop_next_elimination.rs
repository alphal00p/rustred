//! Build the exact generic-Q(d) certificate for the frozen four-loop shell.

use std::error::Error;

use rustred_legacy_oracles::{
    FourLoopComponentTransport, FourLoopComponentTransportConfig, FourLoopCornerShellCertificate,
    FourLoopCornerShellConfig, FourLoopNextClosedRows, FourLoopNextClosedRowsConfig,
    FourLoopNextCornerCrossAuth, FourLoopNextElimination, FourLoopNextEliminationConfig,
    FourLoopNextInventory, FourLoopNextInventoryConfig, FourLoopT1S2Closure,
    FourLoopT1S2ClosureConfig, FourLoopThreeLoopClosure, FourLoopThreeLoopClosureConfig,
};

fn main() -> Result<(), Box<dyn Error>> {
    let inventory = FourLoopNextInventory::build(FourLoopNextInventoryConfig::default())?;
    // Audit every upstream certificate explicitly. The later adapter replay
    // rebuilds the closed parent but intentionally does not invoke all four
    // upstream replay methods on our behalf.
    inventory.replay()?;
    let transport =
        FourLoopComponentTransport::build(&inventory, FourLoopComponentTransportConfig::default())?;
    transport.replay()?;
    let t1s2 = FourLoopT1S2Closure::build(&transport, FourLoopT1S2ClosureConfig::default())?;
    t1s2.replay()?;
    let three_loop =
        FourLoopThreeLoopClosure::build(&transport, FourLoopThreeLoopClosureConfig::default())?;
    three_loop.replay()?;
    let closed = FourLoopNextClosedRows::build(
        &inventory,
        &transport,
        &t1s2,
        &three_loop,
        FourLoopNextClosedRowsConfig::default(),
    )?;
    let certificate =
        FourLoopNextElimination::build(&closed, FourLoopNextEliminationConfig::default())?;
    let corner = FourLoopCornerShellCertificate::build(FourLoopCornerShellConfig::default())?;
    let cross_auth = FourLoopNextCornerCrossAuth::compose(&corner, &certificate)?;
    cross_auth.replay()?;

    println!("{certificate}");
    println!("composed_replay=ok");
    println!(
        "projected_source_checksum=0x{:016x} exact_checksum=0x{:016x} adapter_checksum=0x{:016x}",
        certificate.exact_engine().source_checksum(),
        certificate.exact_engine().checksum(),
        certificate.checksum(),
    );
    let modular = certificate.modular_discovery();
    println!(
        "modular_source_checksum=0x{:016x} column_catalog_checksum=0x{:016x} discovery_checksum=0x{:016x}",
        modular.source_checksum(),
        modular.column_catalog_checksum(),
        modular.checksum(),
    );
    for report in modular.images() {
        let image = report.image();
        println!(
            "modular_image p={} d={} rank={} free={} matrix_checksum=0x{:016x} pivot_checksum=0x{:016x} fill={:#?}",
            image.prime(),
            image.dimension(),
            report.rank(),
            report.free_columns(),
            report.matrix_checksum(),
            report.pivot_checksum(),
            report.fill(),
        );
    }
    println!(
        "exact_engine_stats={:#?}",
        certificate.exact_engine().stats()
    );
    println!("adapter_stats={:#?}", certificate.stats());
    println!("conditions={:#?}", certificate.conditions());
    println!("corner_cross_auth={cross_auth}");
    println!("corner_cross_auth_stats={:#?}", cross_auth.stats());
    Ok(())
}
