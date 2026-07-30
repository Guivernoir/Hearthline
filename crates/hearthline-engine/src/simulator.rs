use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use hearthline_model::{ComponentId, EthernetFrame, PortId};

use crate::{DropReason, Effect, NetworkIngress, SimulatedComponent, SimulationEvent};

#[derive(Clone, Debug, PartialEq)]
pub struct TraceEntry {
    pub time_ms: u64,
    pub component: ComponentId,
    pub effect: Effect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationError {
    DuplicateComponent(ComponentId),
    UnknownComponent(ComponentId),
    UnknownPort {
        component: ComponentId,
        port: PortId,
    },
    PortAlreadyConnected {
        component: ComponentId,
        port: PortId,
    },
    EventLimit {
        limit: usize,
    },
}

impl Display for SimulationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateComponent(component) => {
                write!(formatter, "component {component} already exists")
            }
            Self::UnknownComponent(component) => write!(formatter, "unknown component {component}"),
            Self::UnknownPort { component, port } => {
                write!(formatter, "component {component} has no port {port}")
            }
            Self::PortAlreadyConnected { component, port } => {
                write!(
                    formatter,
                    "component {component} port {port} is already connected"
                )
            }
            Self::EventLimit { limit } => {
                write!(formatter, "simulation exceeded the {limit} event limit")
            }
        }
    }
}

impl Error for SimulationError {}

#[derive(Clone, Debug)]
struct QueuedEvent {
    component: ComponentId,
    event: SimulationEvent,
}

pub struct Simulator {
    components: BTreeMap<ComponentId, Box<dyn SimulatedComponent>>,
    links: BTreeMap<(ComponentId, PortId), (ComponentId, PortId)>,
    immediate: VecDeque<QueuedEvent>,
    delayed: BTreeMap<(u64, u64), QueuedEvent>,
    next_sequence: u64,
    time_ms: u64,
    trace: Vec<TraceEntry>,
}

impl Default for Simulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Simulator {
    pub fn new() -> Self {
        Self {
            components: BTreeMap::new(),
            links: BTreeMap::new(),
            immediate: VecDeque::new(),
            delayed: BTreeMap::new(),
            next_sequence: 0,
            time_ms: 0,
            trace: Vec::new(),
        }
    }

    pub fn add(
        &mut self,
        component: impl SimulatedComponent + 'static,
    ) -> Result<(), SimulationError> {
        let id = component.id().clone();
        if self.components.contains_key(&id) {
            return Err(SimulationError::DuplicateComponent(id));
        }
        self.components.insert(id, Box::new(component));
        Ok(())
    }

    pub fn connect(
        &mut self,
        left_component: &ComponentId,
        left_port: &PortId,
        right_component: &ComponentId,
        right_port: &PortId,
    ) -> Result<(), SimulationError> {
        self.ensure_port(left_component, left_port)?;
        self.ensure_port(right_component, right_port)?;
        self.ensure_available(left_component, left_port)?;
        self.ensure_available(right_component, right_port)?;
        self.links.insert(
            (left_component.clone(), left_port.clone()),
            (right_component.clone(), right_port.clone()),
        );
        self.links.insert(
            (right_component.clone(), right_port.clone()),
            (left_component.clone(), left_port.clone()),
        );
        Ok(())
    }

    pub fn inject_network(
        &mut self,
        component: &ComponentId,
        ingress: &PortId,
        frame: EthernetFrame,
    ) -> Result<(), SimulationError> {
        self.ensure_port(component, ingress)?;
        self.immediate.push_back(QueuedEvent {
            component: component.clone(),
            event: SimulationEvent::Network(NetworkIngress {
                port: ingress.clone(),
                frame,
            }),
        });
        Ok(())
    }

    pub fn inject(
        &mut self,
        component: &ComponentId,
        event: SimulationEvent,
    ) -> Result<(), SimulationError> {
        if !self.components.contains_key(component) {
            return Err(SimulationError::UnknownComponent(component.clone()));
        }
        self.immediate.push_back(QueuedEvent {
            component: component.clone(),
            event,
        });
        Ok(())
    }

    pub fn run(&mut self, event_limit: usize) -> Result<&[TraceEntry], SimulationError> {
        let mut processed = 0;
        while let Some(queued) = self.next_event() {
            if processed >= event_limit {
                self.trace.push(TraceEntry {
                    time_ms: self.time_ms,
                    component: queued.component,
                    effect: Effect::Drop(DropReason::QueueLimit),
                });
                return Err(SimulationError::EventLimit { limit: event_limit });
            }
            processed += 1;
            let component = self
                .components
                .get_mut(&queued.component)
                .ok_or_else(|| SimulationError::UnknownComponent(queued.component.clone()))?;
            let effects = component.handle(queued.event);
            for effect in effects {
                self.trace.push(TraceEntry {
                    time_ms: self.time_ms,
                    component: queued.component.clone(),
                    effect: effect.clone(),
                });
                if let Effect::Transmit {
                    egress,
                    frame,
                    delay_ms,
                    ..
                } = effect
                {
                    if let Some((target, ingress)) =
                        self.links.get(&(queued.component.clone(), egress.clone()))
                    {
                        self.schedule(
                            self.time_ms.saturating_add(delay_ms),
                            QueuedEvent {
                                component: target.clone(),
                                event: SimulationEvent::Network(NetworkIngress {
                                    port: ingress.clone(),
                                    frame,
                                }),
                            },
                        );
                    } else {
                        self.trace.push(TraceEntry {
                            time_ms: self.time_ms,
                            component: queued.component.clone(),
                            effect: Effect::Drop(DropReason::PortDown(egress)),
                        });
                    }
                }
            }
        }
        Ok(&self.trace)
    }

    pub fn trace(&self) -> &[TraceEntry] {
        &self.trace
    }

    pub fn clear_trace(&mut self) {
        self.trace.clear();
    }

    pub const fn time_ms(&self) -> u64 {
        self.time_ms
    }

    fn ensure_port(&self, component: &ComponentId, port: &PortId) -> Result<(), SimulationError> {
        let appliance = self
            .components
            .get(component)
            .ok_or_else(|| SimulationError::UnknownComponent(component.clone()))?;
        if appliance.has_port(port) {
            Ok(())
        } else {
            Err(SimulationError::UnknownPort {
                component: component.clone(),
                port: port.clone(),
            })
        }
    }

    fn ensure_available(
        &self,
        component: &ComponentId,
        port: &PortId,
    ) -> Result<(), SimulationError> {
        if self.links.contains_key(&(component.clone(), port.clone())) {
            Err(SimulationError::PortAlreadyConnected {
                component: component.clone(),
                port: port.clone(),
            })
        } else {
            Ok(())
        }
    }

    fn schedule(&mut self, time_ms: u64, event: QueuedEvent) {
        if time_ms == self.time_ms {
            self.immediate.push_back(event);
        } else {
            let sequence = self.next_sequence;
            self.next_sequence += 1;
            self.delayed.insert((time_ms, sequence), event);
        }
    }

    fn next_event(&mut self) -> Option<QueuedEvent> {
        if let Some(event) = self.immediate.pop_front() {
            return Some(event);
        }
        let ((time_ms, _), event) = self.delayed.pop_first()?;
        self.time_ms = time_ms;
        Some(event)
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use hearthline_model::{
        ApplicationData, ComponentKind, Ipv4Packet, MacAddress, NetworkPayload, ServiceKind,
        TcpFlags, TcpSegment, Transport, VlanId,
    };

    use super::*;
    use crate::{LinkAppliance, LinkMode, ServiceNode};

    fn id(value: &str) -> ComponentId {
        ComponentId::new(value).expect("test ID")
    }

    fn port(value: &str) -> PortId {
        PortId::new(value).expect("test port")
    }

    #[test]
    fn forwards_through_transparent_cpe_to_service() {
        let cpe_id = id("customer-inet-cpe-01");
        let server_id = id("public-service-01");
        let mut simulator = Simulator::new();
        simulator
            .add(LinkAppliance::new(
                cpe_id.clone(),
                ComponentKind::TransparentCpe,
                [port("customer"), port("access")],
                LinkMode::Transparent,
            ))
            .expect("add CPE");
        simulator
            .add(ServiceNode::new(
                server_id.clone(),
                ComponentKind::ServiceCluster,
                [port("network")],
                [Ipv4Addr::new(192, 0, 2, 10)],
                [ServiceKind::Https],
            ))
            .expect("add service");
        simulator
            .connect(&cpe_id, &port("access"), &server_id, &port("network"))
            .expect("connect");

        simulator
            .inject_network(
                &cpe_id,
                &port("customer"),
                EthernetFrame {
                    source: MacAddress::new([0, 1, 2, 3, 4, 5]),
                    destination: MacAddress::new([0, 1, 2, 3, 4, 6]),
                    vlan: VlanId::new(10).expect("VLAN"),
                    payload: NetworkPayload::Ipv4(Ipv4Packet {
                        source: Ipv4Addr::new(203, 0, 113, 2),
                        destination: Ipv4Addr::new(192, 0, 2, 10),
                        ttl: 64,
                        transport: Transport::Tcp(TcpSegment {
                            source_port: 50_000,
                            destination_port: 443,
                            flags: TcpFlags {
                                syn: true,
                                ..TcpFlags::default()
                            },
                        }),
                        application: ApplicationData::Service(ServiceKind::Https),
                    }),
                },
            )
            .expect("inject");
        let trace = simulator.run(10).expect("simulation");
        assert!(trace.iter().any(|entry| {
            entry.component == server_id
                && matches!(
                    entry.effect,
                    Effect::Deliver {
                        service: ServiceKind::Https,
                        ..
                    }
                )
        }));
    }
}
