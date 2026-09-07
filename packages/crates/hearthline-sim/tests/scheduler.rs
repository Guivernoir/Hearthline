use std::sync::Arc;

use hearthline_model::{ComponentId, PartitionMessage, Text};
use hearthline_sim::{
    CellId, CellIdentity, CellRegistration, ConduitConfig, ConduitOverflow, SchedulerError,
    SchedulerStateSnapshot, SiteId, StableScheduler,
};

fn id(value: &str) -> Text<64> {
    Text::try_new(value).expect("test identifier")
}

fn heartbeat(counter: u64) -> PartitionMessage {
    PartitionMessage::Heartbeat {
        source: ComponentId::new("test-source").expect("component ID"),
        counter,
    }
}

fn two_cell_scheduler(overflow: ConduitOverflow) -> (StableScheduler, SiteId, CellId, CellId) {
    let site = id("factory-a");
    let source = id("source-cell");
    let destination = id("destination-cell");
    let mut scheduler = StableScheduler::new();
    for cell in [&source, &destination] {
        scheduler
            .register_cell(CellRegistration {
                site: site.clone(),
                cell: cell.clone(),
                component_demand: 8,
                link_demand: 8,
                operational: true,
            })
            .expect("cell registration");
    }
    scheduler
        .add_conduit(ConduitConfig {
            id: "source-to-destination".into(),
            source_site: site.clone(),
            source_cell: source.clone(),
            destination_site: site.clone(),
            destination_cell: destination.clone(),
            latency_us: 10,
            queue_capacity: 4,
            reviewed_burst: 3,
            overflow,
        })
        .expect("reviewed conduit");
    scheduler.seal().expect("scheduler seal");
    (scheduler, site, source, destination)
}

fn fill(scheduler: &mut StableScheduler, site: &SiteId, source: &CellId, destination: &CellId) {
    for counter in 0..4 {
        scheduler
            .send(site, source, site, destination, heartbeat(counter))
            .expect("message within capacity");
    }
}

#[test]
fn every_conduit_saturation_policy_is_explicit_and_measured() {
    let (mut reject, site, source, destination) = two_cell_scheduler(ConduitOverflow::RejectNewest);
    fill(&mut reject, &site, &source, &destination);
    assert!(matches!(
        reject.send(&site, &source, &site, &destination, heartbeat(4)),
        Err(SchedulerError::Backpressure { limit: 4, .. })
    ));
    assert_eq!(reject.conduits()[0].metrics().rejected, 1);

    let (mut drop_oldest, site, source, destination) =
        two_cell_scheduler(ConduitOverflow::DropOldest);
    fill(&mut drop_oldest, &site, &source, &destination);
    drop_oldest
        .send(&site, &source, &site, &destination, heartbeat(4))
        .expect("drop-oldest admits latest value");
    assert_eq!(drop_oldest.conduits()[0].metrics().dropped, 1);
    assert_eq!(
        drop_oldest
            .pop_ready(10)
            .expect("delivery")
            .unwrap()
            .sequence,
        1
    );

    let (mut coalesce, site, source, destination) =
        two_cell_scheduler(ConduitOverflow::CoalesceLatest);
    fill(&mut coalesce, &site, &source, &destination);
    coalesce
        .send(&site, &source, &site, &destination, heartbeat(4))
        .expect("same-class message coalesces");
    assert_eq!(coalesce.conduits()[0].metrics().coalesced, 1);
    assert_eq!(coalesce.pending_len(), 4);

    let (mut stop, site, source, destination) = two_cell_scheduler(ConduitOverflow::StopSimulation);
    fill(&mut stop, &site, &source, &destination);
    assert!(matches!(
        stop.send(&site, &source, &site, &destination, heartbeat(4)),
        Err(SchedulerError::Saturated { limit: 4, .. })
    ));
    assert!(stop.is_halted());
    assert_eq!(stop.conduits()[0].metrics().stopped, 1);
    assert!(matches!(
        stop.recover_from_saturation(),
        Err(SchedulerError::RecoveryBlocked { pending: 4 })
    ));
    while stop.pop_ready(10).expect("drain after halt").is_some() {}
    stop.recover_from_saturation().expect("explicit recovery");
    assert!(!stop.is_halted());
    assert_eq!(stop.metrics().recoveries, 1);
}

#[test]
fn scheduler_orders_by_time_site_cell_conduit_and_sequence() {
    let source_site = id("site-z");
    let destination_site_a = id("site-a");
    let destination_site_b = id("site-b");
    let source = id("source");
    let cell_a = id("cell-a");
    let cell_b = id("cell-b");
    let mut scheduler = StableScheduler::new();
    for (site, cell) in [
        (&source_site, &source),
        (&destination_site_b, &cell_b),
        (&destination_site_a, &cell_a),
    ] {
        scheduler
            .register_cell(CellRegistration {
                site: site.clone(),
                cell: cell.clone(),
                component_demand: 1,
                link_demand: 1,
                operational: true,
            })
            .expect("cell");
    }
    for (name, site, cell) in [
        ("to-b", &destination_site_b, &cell_b),
        ("to-a", &destination_site_a, &cell_a),
    ] {
        scheduler
            .add_conduit(ConduitConfig {
                id: name.into(),
                source_site: source_site.clone(),
                source_cell: source.clone(),
                destination_site: site.clone(),
                destination_cell: cell.clone(),
                latency_us: 5,
                queue_capacity: 4,
                reviewed_burst: 3,
                overflow: ConduitOverflow::RejectNewest,
            })
            .expect("conduit");
    }
    scheduler.seal().expect("seal");
    scheduler
        .send(
            &source_site,
            &source,
            &destination_site_b,
            &cell_b,
            heartbeat(0),
        )
        .expect("send b");
    scheduler
        .send(
            &source_site,
            &source,
            &destination_site_a,
            &cell_a,
            heartbeat(1),
        )
        .expect("send a");

    assert_eq!(
        scheduler
            .pop_ready(5)
            .expect("delivery")
            .unwrap()
            .destination_site,
        destination_site_a
    );
    assert_eq!(
        scheduler
            .pop_ready(5)
            .expect("delivery")
            .unwrap()
            .destination_site,
        destination_site_b
    );
}

#[test]
fn dynamic_composition_has_no_project_wide_cell_ceiling() {
    let mut scheduler = StableScheduler::new();
    for factory in 0..3 {
        for cell in 0..30 {
            scheduler
                .register_cell(CellRegistration {
                    site: id(&format!("factory-{factory:02}")),
                    cell: id(&format!("cell-{cell:02}")),
                    component_demand: 17,
                    link_demand: 16,
                    operational: true,
                })
                .expect("isolated cell");
        }
    }
    scheduler.seal().expect("seal");
    assert_eq!(scheduler.cells().len(), 90);
}

#[test]
fn execution_and_snapshot_restore_do_not_allocate_after_seal() {
    let (mut scheduler, site, source, destination) =
        two_cell_scheduler(ConduitOverflow::RejectNewest);
    let messages = (0..4).map(heartbeat).collect::<Vec<_>>();

    let execution = allocation_counter::measure(|| {
        for message in messages {
            scheduler
                .send(&site, &source, &site, &destination, message)
                .expect("preallocated send");
        }
        while scheduler.pop_ready(10).expect("delivery").is_some() {}
    });
    assert_eq!(
        execution.count_total, 0,
        "sealed execution allocated: {execution:?}"
    );

    let snapshot = SchedulerStateSnapshot::capture(&scheduler).expect("snapshot");
    let restored = snapshot.restore().expect("snapshot restore");
    assert_eq!(restored.metrics(), scheduler.metrics());
    assert_eq!(restored.pending_len(), 0);
}

#[test]
fn saturation_errors_do_not_allocate_after_seal() {
    for overflow in [
        ConduitOverflow::RejectNewest,
        ConduitOverflow::StopSimulation,
    ] {
        let (mut scheduler, site, source, destination) = two_cell_scheduler(overflow);
        fill(&mut scheduler, &site, &source, &destination);
        let message = heartbeat(5);
        let mut result = None;
        let measured = allocation_counter::measure(|| {
            result = Some(scheduler.send(&site, &source, &site, &destination, message));
        });
        assert!(result.expect("measured result").is_err());
        assert_eq!(
            measured.count_total, 0,
            "{overflow:?} saturation allocated: {measured:?}"
        );
    }

    let (mut scheduler, site, source, destination) =
        two_cell_scheduler(ConduitOverflow::CoalesceLatest);
    fill(&mut scheduler, &site, &source, &destination);
    let message = PartitionMessage::Telemetry {
        source: ComponentId::new("test-source").expect("component ID"),
        sequence: 5,
        payload: "different-message-class".into(),
    };
    let mut result = None;
    let measured = allocation_counter::measure(|| {
        result = Some(scheduler.send(&site, &source, &site, &destination, message));
    });
    assert!(matches!(
        result.expect("measured result"),
        Err(SchedulerError::Backpressure { .. })
    ));
    assert_eq!(
        measured.count_total, 0,
        "coalesce miss allocated: {measured:?}"
    );
}

#[test]
fn unavailable_destination_can_recover_without_rebuilding_the_session() {
    let (mut scheduler, site, source, destination) =
        two_cell_scheduler(ConduitOverflow::RejectNewest);
    scheduler
        .send(&site, &source, &site, &destination, heartbeat(0))
        .expect("send");
    scheduler
        .set_operational(&site, &destination, false)
        .expect("fault destination");
    assert!(
        scheduler
            .pop_ready(10)
            .expect("unavailable delivery")
            .is_some()
    );
    assert_eq!(scheduler.metrics().unavailable, 1);
    scheduler
        .set_operational(&site, &destination, true)
        .expect("recover destination");
    scheduler
        .send(&site, &source, &site, &destination, heartbeat(1))
        .expect("send after recovery");
    assert!(
        scheduler
            .pop_ready(20)
            .expect("recovered delivery")
            .is_some()
    );
    assert_eq!(scheduler.metrics().delivered, 1);
}

#[test]
fn scheduler_rejects_invalid_lifecycle_topology_time_and_capacity_operations() {
    let site = id("factory");
    let source = id("source");
    let destination = id("destination");
    let missing = id("missing");
    let registration = |cell: &CellId| CellRegistration {
        site: site.clone(),
        cell: cell.clone(),
        component_demand: 1,
        link_demand: 1,
        operational: true,
    };
    let config = |name: &str| ConduitConfig {
        id: name.into(),
        source_site: site.clone(),
        source_cell: source.clone(),
        destination_site: site.clone(),
        destination_cell: destination.clone(),
        latency_us: 5,
        queue_capacity: 4,
        reviewed_burst: 3,
        overflow: ConduitOverflow::RejectNewest,
    };

    let mut scheduler = StableScheduler::default();
    assert!(matches!(
        scheduler.pop_ready(0),
        Err(SchedulerError::NotSealed)
    ));
    scheduler.register_cell(registration(&source)).unwrap();
    assert!(matches!(
        scheduler.register_cell(registration(&source)),
        Err(SchedulerError::DuplicateCell(_, _))
    ));
    assert!(matches!(
        scheduler.add_conduit(config("unknown-destination")),
        Err(SchedulerError::UnknownCell(_, _))
    ));
    scheduler.register_cell(registration(&destination)).unwrap();

    for invalid in [
        ConduitConfig {
            id: "".into(),
            ..config("valid")
        },
        ConduitConfig {
            queue_capacity: 0,
            ..config("zero-capacity")
        },
        ConduitConfig {
            reviewed_burst: 0,
            ..config("zero-burst")
        },
        ConduitConfig {
            reviewed_burst: 4,
            ..config("no-reserve")
        },
        ConduitConfig {
            destination_cell: source.clone(),
            ..config("loopback")
        },
    ] {
        assert!(matches!(
            scheduler.add_conduit(invalid),
            Err(SchedulerError::InvalidConduit(_))
        ));
    }

    scheduler.add_conduit(config("route")).unwrap();
    assert!(matches!(
        scheduler.add_conduit(config("route")),
        Err(SchedulerError::DuplicateConduit(_))
    ));
    scheduler.seal().unwrap();
    assert!(matches!(
        scheduler.seal(),
        Err(SchedulerError::AlreadySealed)
    ));
    assert!(matches!(
        scheduler.register_cell(registration(&missing)),
        Err(SchedulerError::AlreadySealed)
    ));
    assert!(matches!(
        scheduler.set_operational(&site, &missing, false),
        Err(SchedulerError::UnknownCell(_, _))
    ));

    scheduler.set_operational(&site, &source, false).unwrap();
    assert!(matches!(
        scheduler.send(&site, &source, &site, &destination, heartbeat(0)),
        Err(SchedulerError::CellUnavailable(_))
    ));
    scheduler.set_operational(&site, &source, true).unwrap();
    assert!(matches!(
        scheduler.send(&site, &source, &site, &missing, heartbeat(0)),
        Err(SchedulerError::UnknownCell(_, _))
    ));
    assert!(matches!(
        scheduler.send(&site, &destination, &site, &source, heartbeat(0)),
        Err(SchedulerError::UnknownRoute { .. })
    ));
    scheduler
        .send(&site, &source, &site, &destination, heartbeat(0))
        .unwrap();
    assert!(scheduler.pop_ready(4).unwrap().is_none());
    assert!(scheduler.pop_ready(5).unwrap().is_some());
    assert!(matches!(
        scheduler.pop_ready(4),
        Err(SchedulerError::TimeRegression { .. })
    ));
    scheduler.recover_from_saturation().unwrap();
}

#[test]
fn scheduler_snapshot_validation_and_error_diagnostics_cover_every_contract() {
    let (mut scheduler, site, source, destination) =
        two_cell_scheduler(ConduitOverflow::RejectNewest);
    scheduler
        .send(&site, &source, &site, &destination, heartbeat(0))
        .unwrap();
    let snapshot = SchedulerStateSnapshot::capture(&scheduler).unwrap();

    let mut oversized = snapshot.clone();
    let envelope = oversized.conduits[0].pending[0].clone();
    while oversized.conduits[0].pending.len() <= 4 {
        oversized.conduits[0].pending.push(envelope.clone());
    }
    assert!(matches!(
        oversized.restore(),
        Err(SchedulerError::InvalidSnapshot(_))
    ));

    let mut inconsistent = snapshot;
    inconsistent.conduits[0].metrics.high_water = 0;
    assert!(matches!(
        inconsistent.restore(),
        Err(SchedulerError::InvalidSnapshot(_))
    ));

    let errors = [
        SchedulerError::AlreadySealed,
        SchedulerError::NotSealed,
        SchedulerError::DuplicateCell(Box::new(site.clone()), Box::new(source.clone())),
        SchedulerError::UnknownCell(Box::new(site.clone()), Box::new(source.clone())),
        SchedulerError::CellUnavailable(Arc::new(CellIdentity {
            site: site.clone(),
            cell: source.clone(),
        })),
        SchedulerError::DuplicateConduit(Box::new("link".into())),
        SchedulerError::InvalidConduit("invalid".into()),
        SchedulerError::UnknownRoute {
            source: Arc::new(CellIdentity {
                site: site.clone(),
                cell: source.clone(),
            }),
            destination: Arc::new(CellIdentity {
                site: site.clone(),
                cell: destination.clone(),
            }),
        },
        SchedulerError::Backpressure {
            conduit: Arc::new("link".into()),
            limit: 4,
        },
        SchedulerError::Saturated {
            conduit: Arc::new("link".into()),
            limit: 4,
        },
        SchedulerError::SimulationStopped,
        SchedulerError::RecoveryBlocked { pending: 1 },
        SchedulerError::TimeRegression {
            current: 2,
            requested: 1,
        },
        SchedulerError::SequenceExhausted,
        SchedulerError::InvalidSnapshot("invalid".into()),
    ];
    for error in errors {
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn stopped_simulation_refuses_new_messages_until_explicit_recovery() {
    let (mut scheduler, site, source, destination) =
        two_cell_scheduler(ConduitOverflow::StopSimulation);
    fill(&mut scheduler, &site, &source, &destination);
    assert!(matches!(
        scheduler.send(&site, &source, &site, &destination, heartbeat(5)),
        Err(SchedulerError::Saturated { .. })
    ));
    assert!(matches!(
        scheduler.send(&site, &source, &site, &destination, heartbeat(6)),
        Err(SchedulerError::SimulationStopped)
    ));
}
