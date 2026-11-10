use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use super::{ClockCommand, CommandReply, WorkerAccessError, WorkerProjection};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CommandTicket(pub u64);

#[derive(Clone, Copy)]
pub(super) struct Request {
    pub ticket: CommandTicket,
    pub expected_revision: u64,
    pub command: ClockCommand,
}

#[derive(Clone, Copy)]
enum Slot {
    Free,
    Pending(Request),
    Executing(CommandTicket),
    Replied(CommandReply),
}

pub(super) struct Mailbox {
    slots: Box<[Slot]>,
    next_ticket: u64,
    pub projection: WorkerProjection,
}

pub(super) struct Shared {
    pub mailbox: Mutex<Mailbox>,
    pub wake: Condvar,
    pub shutdown: AtomicBool,
    pub ended: AtomicBool,
}

pub(super) fn allocation_bytes(capacity: usize) -> Result<u64, String> {
    let bytes = capacity
        .checked_mul(size_of::<Slot>())
        .and_then(|n| n.checked_add(size_of::<Shared>()))
        .and_then(|n| n.checked_add(2 * size_of::<usize>()))
        .ok_or("worker mailbox allocation overflow")?;
    u64::try_from(bytes).map_err(|e| e.to_string())
}

impl Shared {
    pub fn new(capacity: usize, projection: WorkerProjection) -> Result<Self, String> {
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(capacity)
            .map_err(|e| e.to_string())?;
        slots.resize(capacity, Slot::Free);
        Ok(Self {
            mailbox: Mutex::new(Mailbox {
                slots: slots.into_boxed_slice(),
                next_ticket: 1,
                projection,
            }),
            wake: Condvar::new(),
            shutdown: AtomicBool::new(false),
            ended: AtomicBool::new(false),
        })
    }

    pub fn stopped(&self) -> bool {
        self.shutdown.load(Ordering::Acquire) || self.ended.load(Ordering::Acquire)
    }
}

impl Mailbox {
    pub fn take_next(&mut self) -> Option<Request> {
        let index = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| match slot {
                Slot::Pending(request) => Some((i, request.ticket)),
                _ => None,
            })
            .min_by_key(|(_, ticket)| *ticket)
            .map(|(index, _)| index)?;
        let Slot::Pending(request) = self.slots[index] else {
            unreachable!()
        };
        self.slots[index] = Slot::Executing(request.ticket);
        Some(request)
    }

    pub fn has_pending(&self) -> bool {
        self.slots
            .iter()
            .any(|slot| matches!(slot, Slot::Pending(_)))
    }

    pub fn publish(&mut self, mut projection: WorkerProjection, reply: Option<CommandReply>) {
        if let Some(reply) = reply {
            for slot in &mut self.slots {
                if matches!(slot, Slot::Executing(ticket) if *ticket == reply.ticket) {
                    *slot = Slot::Replied(reply);
                    break;
                }
            }
        }
        projection.outstanding_commands = self.projection.outstanding_commands;
        projection.command_high_water = self.projection.command_high_water;
        self.projection = projection;
    }
}

/// Clones share one bounded mailbox. Unread replies retain their slots, so
/// neither requests nor acknowledgments can grow without a configured bound.
#[derive(Clone)]
pub struct PhysicsWorkerClient {
    pub(super) shared: Arc<Shared>,
}

impl PhysicsWorkerClient {
    pub fn try_submit(
        &self,
        expected_revision: u64,
        command: ClockCommand,
    ) -> Result<CommandTicket, WorkerAccessError> {
        let mut mailbox = self
            .shared
            .mailbox
            .lock()
            .map_err(|_| WorkerAccessError::Poisoned)?;
        if self.shared.stopped() {
            return Err(WorkerAccessError::Stopped);
        }
        let index = mailbox
            .slots
            .iter()
            .position(|slot| matches!(slot, Slot::Free))
            .ok_or(WorkerAccessError::MailboxFull)?;
        let next = mailbox
            .next_ticket
            .checked_add(1)
            .ok_or(WorkerAccessError::SequenceOverflow)?;
        let ticket = CommandTicket(mailbox.next_ticket);
        mailbox.slots[index] = Slot::Pending(Request {
            ticket,
            expected_revision,
            command,
        });
        mailbox.next_ticket = next;
        mailbox.projection.outstanding_commands += 1;
        mailbox.projection.command_high_water = mailbox
            .projection
            .command_high_water
            .max(mailbox.projection.outstanding_commands);
        drop(mailbox);
        self.shared.wake.notify_all();
        Ok(ticket)
    }

    pub fn try_take_reply(
        &self,
        ticket: CommandTicket,
    ) -> Result<Option<CommandReply>, WorkerAccessError> {
        let mut mailbox = self
            .shared
            .mailbox
            .lock()
            .map_err(|_| WorkerAccessError::Poisoned)?;
        for slot in &mut mailbox.slots {
            match *slot {
                Slot::Replied(reply) if reply.ticket == ticket => {
                    *slot = Slot::Free;
                    mailbox.projection.outstanding_commands -= 1;
                    return Ok(Some(reply));
                }
                Slot::Pending(Request {
                    ticket: pending, ..
                })
                | Slot::Executing(pending)
                    if pending == ticket =>
                {
                    return if self.shared.ended.load(Ordering::Acquire) {
                        Err(WorkerAccessError::Stopped)
                    } else {
                        Ok(None)
                    };
                }
                _ => (),
            }
        }
        Err(WorkerAccessError::UnknownTicket)
    }

    pub fn projection(&self) -> Result<WorkerProjection, WorkerAccessError> {
        let mailbox = self
            .shared
            .mailbox
            .lock()
            .map_err(|_| WorkerAccessError::Poisoned)?;
        Ok(self.read_projection(&mailbox))
    }

    /// Waits at most one second per call. Spurious wakes return the latest
    /// projection; callers recheck their condition and deadline.
    pub fn wait_for_change(
        &self,
        generation: u64,
        timeout: Duration,
    ) -> Result<WorkerProjection, WorkerAccessError> {
        let mailbox = self
            .shared
            .mailbox
            .lock()
            .map_err(|_| WorkerAccessError::Poisoned)?;
        if mailbox.projection.generation != generation || self.shared.ended.load(Ordering::Acquire)
        {
            return Ok(self.read_projection(&mailbox));
        }
        let (mailbox, _) = self
            .shared
            .wake
            .wait_timeout(mailbox, timeout.min(Duration::from_secs(1)))
            .map_err(|_| WorkerAccessError::Poisoned)?;
        Ok(self.read_projection(&mailbox))
    }

    fn read_projection(&self, mailbox: &Mailbox) -> WorkerProjection {
        let mut projection = mailbox.projection;
        projection.terminated = self.shared.ended.load(Ordering::Acquire);
        projection
    }
}

pub(super) struct CloseOnExit<'a>(pub &'a Shared);
impl Drop for CloseOnExit<'_> {
    fn drop(&mut self) {
        // Also wakes clients if the worker unwinds. Joining reports the panic.
        let _mailbox = self.0.mailbox.lock();
        self.0.ended.store(true, Ordering::Release);
        self.0.wake.notify_all();
    }
}
