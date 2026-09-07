use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::{self, Display, Formatter};

use hearthline_sim::ModelRevision;

use crate::{OperatorCommand, OperatorCommandEnvelope, OperatorCommandError, OperatorProjection};

const DEFAULT_COMMAND_CAPACITY: usize = 32;
const DEFAULT_REVIEWED_BURST: usize = 24;

pub struct OperatorSession {
    id: String,
    operator: String,
    revision: ModelRevision,
    permissions: BTreeSet<String>,
    commands: VecDeque<OperatorCommandEnvelope>,
    command_capacity: usize,
    next_sequence: u64,
    projection: Option<OperatorProjection>,
}

impl OperatorSession {
    pub fn new(
        id: impl Into<String>,
        operator: impl Into<String>,
        revision: ModelRevision,
        permissions: impl IntoIterator<Item = String>,
    ) -> Self {
        Self::with_capacity(
            id,
            operator,
            revision,
            permissions,
            DEFAULT_COMMAND_CAPACITY,
            DEFAULT_REVIEWED_BURST,
        )
        .expect("reviewed default operator queue")
    }

    pub fn with_capacity(
        id: impl Into<String>,
        operator: impl Into<String>,
        revision: ModelRevision,
        permissions: impl IntoIterator<Item = String>,
        command_capacity: usize,
        reviewed_burst: usize,
    ) -> Result<Self, OperatorSessionError> {
        if reviewed_burst == 0
            || reviewed_burst > command_capacity
            || reviewed_burst.saturating_mul(4) > command_capacity.saturating_mul(3)
        {
            return Err(OperatorSessionError::InvalidCapacity {
                capacity: command_capacity,
                reviewed_burst,
            });
        }
        Ok(Self {
            id: id.into(),
            operator: operator.into(),
            revision,
            permissions: permissions.into_iter().collect(),
            commands: VecDeque::with_capacity(command_capacity),
            command_capacity,
            next_sequence: 0,
            projection: None,
        })
    }

    pub fn submit(
        &mut self,
        current_revision: &str,
        submitted_at_us: u64,
        command: OperatorCommand,
    ) -> Result<u64, OperatorCommandError> {
        if self.revision.digest != current_revision {
            return Err(OperatorCommandError::StaleRevision {
                expected: self.revision.digest.clone(),
                actual: current_revision.into(),
            });
        }
        let required = required_permission(&command);
        if !self.permissions.contains(required) {
            return Err(OperatorCommandError::PermissionDenied(required.into()));
        }
        if self.commands.len() >= self.command_capacity {
            return Err(OperatorCommandError::QueueFull {
                limit: self.command_capacity,
            });
        }
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(OperatorCommandError::SequenceExhausted)?;
        self.commands.push_back(OperatorCommandEnvelope {
            sequence,
            submitted_at_us,
            operator: self.operator.clone(),
            model_revision: self.revision.digest.clone(),
            command,
            context: BTreeMap::new(),
        });
        Ok(sequence)
    }

    pub fn pop_command(&mut self) -> Option<OperatorCommandEnvelope> {
        self.commands.pop_front()
    }
    pub fn pending_commands(&self) -> usize {
        self.commands.len()
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn revision(&self) -> &ModelRevision {
        &self.revision
    }
    pub fn projection(&self) -> Option<&OperatorProjection> {
        self.projection.as_ref()
    }
    pub fn update_projection(&mut self, projection: OperatorProjection) {
        self.projection = Some(projection);
    }
}

#[derive(Default)]
pub struct OperatorSessionStore {
    sessions: BTreeMap<String, OperatorSession>,
}

impl OperatorSessionStore {
    pub fn insert(&mut self, session: OperatorSession) -> Result<(), OperatorSessionError> {
        if self.sessions.insert(session.id.clone(), session).is_some() {
            return Err(OperatorSessionError::DuplicateSession);
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&OperatorSession> {
        self.sessions.get(id)
    }
    pub fn get_mut(&mut self, id: &str) -> Option<&mut OperatorSession> {
        self.sessions.get_mut(id)
    }
    pub fn remove(&mut self, id: &str) -> Option<OperatorSession> {
        self.sessions.remove(id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperatorSessionError {
    InvalidCapacity {
        capacity: usize,
        reviewed_burst: usize,
    },
    DuplicateSession,
}

impl Display for OperatorSessionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCapacity {
                capacity,
                reviewed_burst,
            } => write!(
                formatter,
                "operator queue capacity {capacity} does not preserve 25% reserve for burst {reviewed_burst}"
            ),
            Self::DuplicateSession => formatter.write_str("operator session already exists"),
        }
    }
}

impl std::error::Error for OperatorSessionError {}

fn required_permission(command: &OperatorCommand) -> &'static str {
    match command {
        OperatorCommand::Start { .. } | OperatorCommand::Stop { .. } => "operate",
        OperatorCommand::Reset { .. } => "reset",
        OperatorCommand::SetMode { .. } | OperatorCommand::ManualOutput { .. } => "manual-control",
        OperatorCommand::SetParameter { .. } => "configure-parameters",
    }
}
