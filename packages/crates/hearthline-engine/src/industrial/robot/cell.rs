use heapless::Deque;
use hearthline_model::Text;

pub const ROBOT_CELL_QUEUE_CAPACITY: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RobotCellStage {
    Idle,
    Approach,
    Pickup,
    GripConfirm,
    Handoff,
    Release,
    Retreat,
    Return,
    Faulted,
}

impl RobotCellStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Approach => "approach",
            Self::Pickup => "pickup",
            Self::GripConfirm => "grip-confirm",
            Self::Handoff => "handoff",
            Self::Release => "release",
            Self::Retreat => "retreat",
            Self::Return => "return",
            Self::Faulted => "faulted",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RobotCellRequestStatus {
    Granted,
    Queued,
    AlreadyPending,
    Full,
    Invalid,
}

#[derive(Clone, Debug)]
pub struct RobotCellArbiter {
    active: Option<Text<64>>,
    queue: Deque<Text<64>, ROBOT_CELL_QUEUE_CAPACITY>,
    stage: RobotCellStage,
    completed: u64,
}

impl Default for RobotCellArbiter {
    fn default() -> Self {
        Self {
            active: None,
            queue: Deque::new(),
            stage: RobotCellStage::Idle,
            completed: 0,
        }
    }
}

impl RobotCellArbiter {
    pub fn request(&mut self, mould: &str) -> RobotCellRequestStatus {
        let Ok(mould) = Text::try_new(mould) else {
            return RobotCellRequestStatus::Invalid;
        };
        if self.active.as_ref() == Some(&mould) || self.queue.iter().any(|item| item == &mould) {
            return RobotCellRequestStatus::AlreadyPending;
        }
        if self.active.is_none() {
            self.active = Some(mould);
            self.stage = RobotCellStage::Approach;
            return RobotCellRequestStatus::Granted;
        }
        if self.queue.push_back(mould).is_err() {
            RobotCellRequestStatus::Full
        } else {
            RobotCellRequestStatus::Queued
        }
    }

    pub fn set_stage(&mut self, stage: RobotCellStage) {
        if self.active.is_some() {
            self.stage = stage;
        }
    }

    pub fn complete_active(&mut self) -> Option<Text<64>> {
        let completed = self.advance_active()?;
        self.completed = self.completed.saturating_add(1);
        Some(completed)
    }

    pub fn clear_fault(&mut self) {
        if self.stage == RobotCellStage::Faulted {
            self.stage = if self.active.is_some() {
                RobotCellStage::Approach
            } else {
                RobotCellStage::Idle
            };
        }
    }

    pub fn cancel(&mut self, mould: &str) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.as_str() == mould)
        {
            self.advance_active();
            return;
        }
        let mut retained = Deque::new();
        while let Some(candidate) = self.queue.pop_front() {
            if candidate.as_str() != mould {
                let _ = retained.push_back(candidate);
            }
        }
        self.queue = retained;
    }

    pub fn active(&self) -> Option<&str> {
        self.active.as_ref().map(Text::as_str)
    }

    pub const fn stage(&self) -> RobotCellStage {
        self.stage
    }

    pub fn queue(&self) -> impl Iterator<Item = &str> {
        self.queue.iter().map(Text::as_str)
    }

    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    pub const fn completed(&self) -> u64 {
        self.completed
    }

    fn advance_active(&mut self) -> Option<Text<64>> {
        let active = self.active.take()?;
        self.active = self.queue.pop_front();
        self.stage = if self.active.is_some() {
            RobotCellStage::Approach
        } else {
            RobotCellStage::Idle
        };
        Some(active)
    }
}
