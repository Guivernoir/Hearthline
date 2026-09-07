use std::path::PathBuf;

use hearthline_config::{ConfigRepository, HmiAction};
use hearthline_operator::PlantOperatorGateway;
use hearthline_sim::PlantRuntimeStore;

fn appliances() -> ConfigRepository {
    ConfigRepository::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config/appliances"),
    )
    .expect("canonical appliances")
}

#[test]
fn operator_gateway_borrows_authoritative_plant_state() {
    assert_eq!(
        std::mem::size_of::<PlantOperatorGateway<'_>>(),
        std::mem::size_of::<usize>() * 2
    );
    let appliances = appliances();
    let mut plant = PlantRuntimeStore::default();
    let mut gateway = PlantOperatorGateway::new(&appliances, &mut plant);
    let projection = gateway
        .projection("area-02-machine-pc-01")
        .expect("operator projection");
    assert_eq!(projection.id, "area-02-machine-pc-01");
    let report = gateway
        .submit(
            "area-02-machine-pc-01",
            HmiAction::AcknowledgeAlarm {
                alarm_id: "missing-alarm".into(),
            },
        )
        .expect("typed command report");
    assert!(report.snapshot.sequence > projection.sequence);
}
