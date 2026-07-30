use core::error::Error;
use core::fmt::{self, Display, Formatter, Write as _};

use heapless::{Deque, Vec as FixedList};
use hearthline_model::{ComponentId, EthernetFrame, PortId, Text};

use crate::{DropReason, Effect, NetworkIngress, SimulatedComponent, SimulationEvent};

const COMPONENT_CAPACITY: usize = 192;
const LINK_CAPACITY: usize = 256;
const IMMEDIATE_CAPACITY: usize = 16;
const DELAYED_CAPACITY: usize = 16;
const TRACE_CAPACITY: usize = 128;

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
    time_ms: u64,
    sequence: u64,
    event: QueuedEvent,
}

#[derive(Clone, Debug)]
struct LinkBinding {
    left_component: ComponentId,
    left_port: PortId,
    right_component: ComponentId,
    right_port: PortId,
}

impl LinkBinding {
    fn contains(&self, component: &ComponentId, port: &PortId) -> bool {
        (&self.left_component == component && &self.left_port == port)
            || (&self.right_component == component && &self.right_port == port)
    }

    fn peer(&self, component: &ComponentId, port: &PortId) -> Option<(&ComponentId, &PortId)> {
        if &self.left_component == component && &self.left_port == port {
            Some((&self.right_component, &self.right_port))
        } else if &self.right_component == component && &self.right_port == port {
            Some((&self.left_component, &self.left_port))
        } else {
            None
        }
    }
}

pub struct Simulator<'components> {
    components: FixedList<&'components mut dyn SimulatedComponent, COMPONENT_CAPACITY>,
    links: FixedList<LinkBinding, LINK_CAPACITY>,
    immediate: Deque<QueuedEvent, IMMEDIATE_CAPACITY>,
    delayed: FixedList<DelayedEvent, DELAYED_CAPACITY>,
    next_sequence: u64,
    time_ms: u64,
    trace: FixedList<TraceEntry, TRACE_CAPACITY>,
}

impl Default for Simulator<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'components> Simulator<'components> {
    pub const fn new() -> Self {
        Self {
            components: FixedList::new(),
            links: FixedList::new(),
            immediate: Deque::new(),
            delayed: FixedList::new(),
            next_sequence: 0,
            time_ms: 0,
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
        self.links
            .push(LinkBinding {
                left_component: left_component.clone(),
                left_port: left_port.clone(),
                right_component: right_component.clone(),
                right_port: right_port.clone(),
            })
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

    pub fn run(&mut self, event_limit: usize) -> Result<&[TraceEntry], SimulationError> {
        let mut processed = 0;
        while let Some(queued) = self.next_event() {
            if processed >= event_limit {
                self.record(TraceEntry {
                    time_ms: self.time_ms,
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
                    time_ms: self.time_ms,
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
        self.time_ms
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
        let peer = self
            .links
            .iter()
            .find_map(|link| link.peer(source, &egress))
            .map(|(component, port)| (component.clone(), port.clone()));
        if let Some((component, port)) = peer {
            self.schedule(
                self.time_ms.saturating_add(delay_ms),
                QueuedEvent {
                    component,
                    event: SimulationEvent::Network(NetworkIngress { port, frame }),
                },
            )
        } else {
            self.record(TraceEntry {
                time_ms: self.time_ms,
                component: source.clone(),
                effect: Effect::Drop(DropReason::PortDown(egress)),
            })
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
    ) -> Result<(), SimulationError> {
        if self.links.iter().any(|link| link.contains(component, port)) {
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

    fn schedule(&mut self, time_ms: u64, event: QueuedEvent) -> Result<(), SimulationError> {
        if time_ms == self.time_ms {
            return self.enqueue_immediate(event);
        }
        let delayed = DelayedEvent {
            time_ms,
            sequence: self.next_sequence,
            event,
        };
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let position = self
            .delayed
            .iter()
            .position(|candidate| {
                (candidate.time_ms, candidate.sequence) > (delayed.time_ms, delayed.sequence)
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
        self.time_ms = event.time_ms;
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
