use core::error::Error;
use core::fmt::{self, Display, Formatter, Write as _};

use heapless::{Deque, Vec as FixedList};
use hearthline_model::{ComponentId, EthernetFrame, Ipv4Packet, PortId, Text};

use crate::{
    DropReason, Effect, Ipv4Egress, MediaLink, NetworkIngress, SimulatedComponent, SimulationEvent,
};

const COMPONENT_CAPACITY: usize = 192;
const LINK_CAPACITY: usize = 256;
const IMMEDIATE_CAPACITY: usize = 64;
const DELAYED_CAPACITY: usize = 64;
const TRACE_CAPACITY: usize = 224;
const SHARED_MEDIA_CAPACITY: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub struct TraceEntry {
    pub time_ms: u64,
    pub time_us: u64,
    pub component: ComponentId,
    pub effect: Effect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationError {
    DuplicateComponent(ComponentId),
    DuplicateConnection(ComponentId),
    UnknownComponent(ComponentId),
    UnknownPort(Text<96>),
    PortAlreadyConnected(Text<96>),
    CapacityExceeded {
        resource: &'static str,
        limit: usize,
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
            Self::DuplicateConnection(connection) => {
                write!(formatter, "connection {connection} already exists")
            }
            Self::UnknownComponent(component) => write!(formatter, "unknown component {component}"),
            Self::UnknownPort(endpoint) => write!(formatter, "unknown port {endpoint}"),
            Self::PortAlreadyConnected(endpoint) => {
                write!(formatter, "port {endpoint} is already connected")
            }
            Self::CapacityExceeded { resource, limit } => {
                write!(
                    formatter,
                    "{resource} exceeds its fixed capacity of {limit}"
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

#[derive(Clone, Debug)]
struct DelayedEvent {
    time_us: u64,
    sequence: u64,
    event: QueuedEvent,
}

pub struct Simulator<'components> {
    components: FixedList<&'components mut dyn SimulatedComponent, COMPONENT_CAPACITY>,
    links: FixedList<&'components mut MediaLink, LINK_CAPACITY>,
    immediate: Deque<QueuedEvent, IMMEDIATE_CAPACITY>,
    delayed: FixedList<DelayedEvent, DELAYED_CAPACITY>,
    next_sequence: u64,
    time_us: u64,
    trace: FixedList<TraceEntry, TRACE_CAPACITY>,
}

impl Default for Simulator<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'components> Simulator<'components> {
    pub const fn new() -> Self {
        Self::with_start_time_us(0)
    }

    pub const fn with_start_time_us(time_us: u64) -> Self {
        Self {
            components: FixedList::new(),
            links: FixedList::new(),
            immediate: Deque::new(),
            delayed: FixedList::new(),
            next_sequence: 0,
            time_us,
            trace: FixedList::new(),
        }
    }

    pub fn add(
        &mut self,
        component: &'components mut impl SimulatedComponent,
    ) -> Result<(), SimulationError> {
        let id = component.id().clone();
        if self.component(&id).is_some() {
            return Err(SimulationError::DuplicateComponent(id));
        }
        self.components
            .push(component)
            .map_err(|_| SimulationError::CapacityExceeded {
                resource: "components",
                limit: COMPONENT_CAPACITY,
            })
    }

    pub fn add_link(&mut self, link: &'components mut MediaLink) -> Result<(), SimulationError> {
        if self
            .links
            .iter()
            .any(|candidate| candidate.id() == link.id())
        {
            return Err(SimulationError::DuplicateConnection(link.id().clone()));
        }
        let (endpoint_a, endpoint_b) = link.endpoints();
        self.ensure_port(&endpoint_a.component, &endpoint_a.port)?;
        self.ensure_port(&endpoint_b.component, &endpoint_b.port)?;
        self.ensure_available(
            &endpoint_a.component,
            &endpoint_a.port,
            link.requires_exclusive_endpoints(),
        )?;
        self.ensure_available(
            &endpoint_b.component,
            &endpoint_b.port,
            link.requires_exclusive_endpoints(),
        )?;
        self.links
            .push(link)
            .map_err(|_| SimulationError::CapacityExceeded {
                resource: "links",
                limit: LINK_CAPACITY,
            })
    }

    pub fn inject_network(
        &mut self,
        component: &ComponentId,
        ingress: &PortId,
        frame: EthernetFrame,
    ) -> Result<(), SimulationError> {
        self.ensure_port(component, ingress)?;
        self.enqueue_immediate(QueuedEvent {
            component: component.clone(),
            event: SimulationEvent::Network(NetworkIngress {
                port: ingress.clone(),
                frame,
                received_at_us: self.time_us,
            }),
        })
    }

    pub fn inject(
        &mut self,
        component: &ComponentId,
        event: SimulationEvent,
    ) -> Result<(), SimulationError> {
        if self.component(component).is_none() {
            return Err(SimulationError::UnknownComponent(component.clone()));
        }
        self.enqueue_immediate(QueuedEvent {
            component: component.clone(),
            event,
        })
    }

    pub fn inject_ipv4(
        &mut self,
        component: &ComponentId,
        packet: Ipv4Packet,
        wire_len_bytes: u16,
    ) -> Result<(), SimulationError> {
        self.inject(
            component,
            SimulationEvent::Ipv4Egress(Ipv4Egress {
                packet,
                wire_len_bytes,
                sent_at_us: self.time_us,
            }),
        )
    }

    pub fn run(&mut self, event_limit: usize) -> Result<&[TraceEntry], SimulationError> {
        let mut processed = 0;
        while let Some(queued) = self.next_event() {
            if processed >= event_limit {
                self.record(TraceEntry {
                    time_ms: self.time_ms(),
                    time_us: self.time_us,
                    component: queued.component,
                    effect: Effect::Drop(DropReason::QueueLimit),
                })?;
                return Err(SimulationError::EventLimit { limit: event_limit });
            }
            processed += 1;
            let component = self
                .component_mut(&queued.component)
                .ok_or_else(|| SimulationError::UnknownComponent(queued.component.clone()))?;
            let effects = component.handle(queued.event);
            for effect in effects {
                self.record(TraceEntry {
                    time_ms: self.time_ms(),
                    time_us: self.time_us,
                    component: queued.component.clone(),
                    effect: effect.clone(),
                })?;
                self.route_effect(&queued.component, effect)?;
            }
        }
        Ok(self.trace.as_slice())
    }

    pub fn trace(&self) -> &[TraceEntry] {
        self.trace.as_slice()
    }

    pub fn clear_trace(&mut self) {
        self.trace.clear();
    }

    pub const fn time_ms(&self) -> u64 {
        self.time_us / 1_000
    }

    pub const fn time_us(&self) -> u64 {
        self.time_us
    }

    fn route_effect(
        &mut self,
        source: &ComponentId,
        effect: Effect,
    ) -> Result<(), SimulationError> {
        let Effect::Transmit {
            egress,
            frame,
            delay_ms,
            ..
        } = effect
        else {
            return Ok(());
        };
        let mut link_indices: FixedList<usize, SHARED_MEDIA_CAPACITY> = FixedList::new();
        for (index, _) in self
            .links
            .iter()
            .enumerate()
            .filter(|(_, link)| link.contains(source, &egress))
        {
            link_indices
                .push(index)
                .map_err(|_| SimulationError::CapacityExceeded {
                    resource: "shared media fan-out",
                    limit: SHARED_MEDIA_CAPACITY,
                })?;
        }
        if link_indices.is_empty() {
            self.record(TraceEntry {
                time_ms: self.time_ms(),
                time_us: self.time_us,
                component: source.clone(),
                effect: Effect::Drop(DropReason::PortDown(egress)),
            })?;
            return Ok(());
        }
        let ready_at_us = self.time_us.saturating_add(delay_ms.saturating_mul(1_000));
        for link_index in link_indices {
            self.route_over_link(link_index, source, &egress, &frame, ready_at_us)?;
        }
        Ok(())
    }

    fn route_over_link(
        &mut self,
        link_index: usize,
        source: &ComponentId,
        egress: &PortId,
        frame: &EthernetFrame,
        ready_at_us: u64,
    ) -> Result<(), SimulationError> {
        let (connection, transit) = {
            let link = &mut self.links[link_index];
            (
                link.id().clone(),
                link.transmit(source, egress, frame, ready_at_us),
            )
        };
        match transit {
            Ok(transit) => {
                self.record(TraceEntry {
                    time_ms: self.time_ms(),
                    time_us: self.time_us,
                    component: source.clone(),
                    effect: Effect::MediaTransit {
                        connection,
                        destination_component: transit.destination_component.clone(),
                        destination_port: transit.destination_port.clone(),
                        wire_bytes: frame.wire_len_bytes,
                        queue_delay_us: transit.queue_delay_us,
                        serialization_us: transit.serialization_us,
                        propagation_us: transit.propagation_us,
                        arrival_us: transit.arrival_us,
                    },
                })?;
                self.schedule(
                    transit.arrival_us,
                    QueuedEvent {
                        component: transit.destination_component,
                        event: SimulationEvent::Network(NetworkIngress {
                            port: transit.destination_port,
                            frame: frame.clone(),
                            received_at_us: transit.arrival_us,
                        }),
                    },
                )
            }
            Err(reason) => self.record(TraceEntry {
                time_ms: self.time_ms(),
                time_us: self.time_us,
                component: source.clone(),
                effect: Effect::Drop(DropReason::Media(reason)),
            }),
        }
    }

    fn ensure_port(&self, component: &ComponentId, port: &PortId) -> Result<(), SimulationError> {
        let appliance = self
            .component(component)
            .ok_or_else(|| SimulationError::UnknownComponent(component.clone()))?;
        if appliance.has_port(port) {
            Ok(())
        } else {
            Err(SimulationError::UnknownPort(endpoint_text(component, port)))
        }
    }

    fn ensure_available(
        &self,
        component: &ComponentId,
        port: &PortId,
        new_link_exclusive: bool,
    ) -> Result<(), SimulationError> {
        if self.links.iter().any(|link| {
            link.contains(component, port)
                && (new_link_exclusive || link.requires_exclusive_endpoints())
        }) {
            Err(SimulationError::PortAlreadyConnected(endpoint_text(
                component, port,
            )))
        } else {
            Ok(())
        }
    }

    fn component(&self, id: &ComponentId) -> Option<&(dyn SimulatedComponent + 'components)> {
        self.components
            .iter()
            .find(|component| component.id() == id)
            .map(|component| &**component)
    }

    fn component_mut(
        &mut self,
        id: &ComponentId,
    ) -> Option<&mut (dyn SimulatedComponent + 'components)> {
        self.components
            .iter_mut()
            .find(|component| component.id() == id)
            .map(|component| &mut **component)
    }

    fn enqueue_immediate(&mut self, event: QueuedEvent) -> Result<(), SimulationError> {
        self.immediate
            .push_back(event)
            .map_err(|_| SimulationError::CapacityExceeded {
                resource: "immediate event queue",
                limit: IMMEDIATE_CAPACITY,
            })
    }

    fn schedule(&mut self, time_us: u64, event: QueuedEvent) -> Result<(), SimulationError> {
        if time_us == self.time_us {
            return self.enqueue_immediate(event);
        }
        let delayed = DelayedEvent {
            time_us,
            sequence: self.next_sequence,
            event,
        };
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let position = self
            .delayed
            .iter()
            .position(|candidate| {
                (candidate.time_us, candidate.sequence) > (delayed.time_us, delayed.sequence)
            })
            .unwrap_or(self.delayed.len());
        self.delayed
            .insert(position, delayed)
            .map_err(|_| SimulationError::CapacityExceeded {
                resource: "delayed event queue",
                limit: DELAYED_CAPACITY,
            })
    }

    fn next_event(&mut self) -> Option<QueuedEvent> {
        if let Some(event) = self.immediate.pop_front() {
            return Some(event);
        }
        if self.delayed.is_empty() {
            return None;
        }
        let event = self.delayed.remove(0);
        self.time_us = event.time_us;
        Some(event.event)
    }

    fn record(&mut self, entry: TraceEntry) -> Result<(), SimulationError> {
        self.trace
            .push(entry)
            .map_err(|_| SimulationError::CapacityExceeded {
                resource: "simulation trace",
                limit: TRACE_CAPACITY,
            })
    }
}

fn endpoint_text(component: &ComponentId, port: &PortId) -> Text<96> {
    let mut endpoint = Text::default();
    write!(&mut endpoint, "{component}:{port}").expect("validated endpoint must fit error context");
    endpoint
}
