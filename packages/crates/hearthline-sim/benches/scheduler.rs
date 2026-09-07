use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use hearthline_model::{ComponentId, PartitionMessage, Text};
use hearthline_sim::{
    CellRegistration, ConduitConfig, ConduitId, ConduitOverflow, StableScheduler,
};

fn text(value: &str) -> Text<64> {
    Text::try_new(value).expect("benchmark identifier")
}

fn runtime() -> StableScheduler {
    let site = text("factory");
    let mut scheduler = StableScheduler::new();
    for index in 0..75 {
        scheduler
            .register_cell(CellRegistration {
                site: site.clone(),
                cell: text(&format!("cell-{index:02}")),
                component_demand: 16,
                link_demand: 18,
                operational: true,
            })
            .expect("cell registration");
    }
    for index in 0..75 {
        let conduit = format!("conduit-{index:02}");
        scheduler
            .add_conduit(ConduitConfig {
                id: ConduitId::try_new(&conduit).expect("conduit identifier"),
                source_site: site.clone(),
                source_cell: text(&format!("cell-{index:02}")),
                destination_site: site.clone(),
                destination_cell: text(&format!("cell-{:02}", (index + 1) % 75)),
                latency_us: 100,
                queue_capacity: 32,
                reviewed_burst: 24,
                overflow: ConduitOverflow::RejectNewest,
            })
            .expect("conduit registration");
    }
    scheduler.seal().expect("sealed scheduler");
    scheduler
}

fn benchmark_seventy_five_cells(criterion: &mut Criterion) {
    criterion.bench_function("seventy_five_cell_scan", |bencher| {
        bencher.iter_batched(
            runtime,
            |mut scheduler| {
                let site = text("factory");
                for scan in 0..100_u64 {
                    for index in 0..75 {
                        scheduler
                            .send(
                                &site,
                                &text(&format!("cell-{index:02}")),
                                &site,
                                &text(&format!("cell-{:02}", (index + 1) % 75)),
                                PartitionMessage::Heartbeat {
                                    source: ComponentId::new(&format!("cell-{index:02}-plc"))
                                        .expect("component"),
                                    counter: scan,
                                },
                            )
                            .expect("scheduled heartbeat");
                    }
                    while scheduler
                        .pop_ready(scan.saturating_mul(100).saturating_add(100))
                        .expect("monotonic scheduler")
                        .is_some()
                    {}
                }
                black_box(scheduler.metrics().clone())
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(runtime_benches, benchmark_seventy_five_cells);
criterion_main!(runtime_benches);
