#![no_main]

use hearthline_model::{ComponentId, PartitionMessage, Text};
use hearthline_sim::{
    CellRegistration, ConduitConfig, ConduitOverflow, SchedulerStateSnapshot, StableScheduler,
};
use libfuzzer_sys::fuzz_target;

fn partition(value: &str) -> Text<64> {
    Text::try_new(value).expect("static partition id")
}

fn runtime() -> StableScheduler {
    let site = partition("factory");
    let mut runtime = StableScheduler::new();
    for cell in ["source", "destination"] {
        runtime
            .register_cell(CellRegistration {
                site: site.clone(),
                cell: partition(cell),
                component_demand: 9,
                link_demand: 8,
                operational: true,
            })
            .expect("cell registration");
    }
    runtime
        .add_conduit(ConduitConfig {
            id: "source-to-destination".into(),
            source_site: site.clone(),
            source_cell: partition("source"),
            destination_site: site,
            destination_cell: partition("destination"),
            latency_us: 5,
            queue_capacity: 32,
            reviewed_burst: 24,
            overflow: ConduitOverflow::CoalesceLatest,
        })
        .expect("conduit");
    runtime.seal().expect("sealed scheduler");
    runtime
}

fuzz_target!(|data: &[u8]| {
    let mut runtime = runtime();
    let site = partition("factory");
    let mut now_us = 0_u64;
    for (index, byte) in data.iter().copied().take(4_096).enumerate() {
        match byte % 6 {
            0 | 1 => {
                let _ = runtime.send(
                    &site,
                    &partition("source"),
                    &site,
                    &partition("destination"),
                    PartitionMessage::Heartbeat {
                        source: ComponentId::new("source-plc").expect("static component id"),
                        counter: index as u64,
                    },
                );
            }
            2 => {
                let _ = runtime.set_operational(&site, &partition("source"), byte & 0x08 != 0);
            }
            3 => {
                let _ = runtime.set_operational(&site, &partition("destination"), byte & 0x08 != 0);
            }
            4 => {
                now_us = now_us.saturating_add(u64::from(byte >> 3));
                while runtime
                    .pop_ready(now_us)
                    .expect("monotonic fuzz clock")
                    .is_some()
                {}
            }
            _ => {
                let snapshot =
                    SchedulerStateSnapshot::capture(&runtime).expect("self-produced snapshot");
                runtime = snapshot.restore().expect("self-produced snapshot restores");
            }
        }
        assert!(runtime.pending_len() <= 32);
    }
});
