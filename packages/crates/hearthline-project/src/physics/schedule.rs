//! Immutable numerical-instance ordering. Physical connections and controller
//! tasks must be compiled separately before this can represent a coupled plant.

use std::sync::Arc;

use hearthline_model::ComponentId;
use sha2::{Digest, Sha256};

use super::CompiledPhysicsProof;

pub const PHYSICS_SCHEDULE_CONTRACT: &str =
    "hearthline-physics-schedule/1:time-site-cell-instance:committed-event-prefix";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PhysicsInstanceIdentity {
    pub site: ComponentId,
    pub cell: ComponentId,
    pub instance: ComponentId,
}

impl PhysicsInstanceIdentity {
    pub fn new(site: &str, cell: &str, instance: &str) -> Result<Self, String> {
        Ok(Self {
            site: ComponentId::new(site).map_err(|e| e.to_string())?,
            cell: ComponentId::new(cell).map_err(|e| e.to_string())?,
            instance: ComponentId::new(instance).map_err(|e| e.to_string())?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct PhysicsInstanceRegistration {
    pub identity: PhysicsInstanceIdentity,
    pub model: Arc<CompiledPhysicsProof>,
}

#[derive(Clone, Debug)]
pub struct CompiledPhysicsInstance {
    pub identity: PhysicsInstanceIdentity,
    pub profile: usize,
    pub slot: usize,
}

#[derive(Debug)]
pub struct CompiledPhysicsSchedule {
    profiles: Box<[Arc<CompiledPhysicsProof>]>,
    instances: Box<[CompiledPhysicsInstance]>,
    counts: Box<[usize]>,
    digest: String,
    allocation_budget_bytes: u64,
    event_capacity: usize,
    total_events: u64,
    horizon_us: u64,
}

impl CompiledPhysicsSchedule {
    pub fn compile(
        mut registrations: Vec<PhysicsInstanceRegistration>,
        allocation_budget_bytes: u64,
    ) -> Result<Self, String> {
        if registrations.is_empty() || allocation_budget_bytes == 0 {
            return Err("physics schedule requires instances and a nonzero budget".into());
        }
        registrations.sort_unstable_by(|a, b| a.identity.cmp(&b.identity));
        if registrations
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
        {
            return Err("duplicate qualified physics instance identity".into());
        }
        let event_capacity = registrations
            .len()
            .checked_mul(4)
            .map(|n| n.div_ceil(3))
            .ok_or("physics schedule event capacity overflow")?;
        let mut profiles: Vec<_> = registrations.iter().map(|r| Arc::clone(&r.model)).collect();
        profiles.sort_unstable_by(|a, b| a.digest().cmp(b.digest()));
        profiles.dedup_by(|a, b| a.digest() == b.digest());
        let mut counts = vec![0; profiles.len()];
        let mut instances = Vec::with_capacity(registrations.len());
        let mut hash = Sha256::new();
        hash.update(PHYSICS_SCHEDULE_CONTRACT);
        hash.update(allocation_budget_bytes.to_be_bytes());
        let mut total_events = 0_u64;
        let mut horizon_us = 0_u64;
        for registration in registrations {
            let model = &registration.model;
            let profile = profiles
                .binary_search_by(|p| p.digest().cmp(model.digest()))
                .map_err(|_| "missing compiled physics profile")?;
            let horizon = model
                .timestep_us()
                .checked_mul(u64::from(model.config().steps))
                .ok_or("physics schedule horizon overflow")?;
            horizon_us = horizon_us.max(horizon);
            total_events = total_events
                .checked_add(u64::from(model.config().steps))
                .ok_or("physics schedule event count overflow")?;
            for id in [
                &registration.identity.site,
                &registration.identity.cell,
                &registration.identity.instance,
            ] {
                hash.update((id.as_str().len() as u64).to_be_bytes());
                hash.update(id.as_str());
            }
            hash.update(model.digest());
            instances.push(CompiledPhysicsInstance {
                identity: registration.identity,
                profile,
                slot: counts[profile],
            });
            counts[profile] += 1;
        }
        Ok(Self {
            profiles: profiles.into_boxed_slice(),
            instances: instances.into_boxed_slice(),
            counts: counts.into_boxed_slice(),
            digest: crate::source::sha256_hex(hash.finalize().as_slice()),
            allocation_budget_bytes,
            event_capacity,
            total_events,
            horizon_us,
        })
    }

    pub fn profiles(&self) -> &[Arc<CompiledPhysicsProof>] {
        &self.profiles
    }
    pub fn instances(&self) -> &[CompiledPhysicsInstance] {
        &self.instances
    }
    pub fn profile_counts(&self) -> &[usize] {
        &self.counts
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub const fn allocation_budget_bytes(&self) -> u64 {
        self.allocation_budget_bytes
    }
    pub const fn event_capacity(&self) -> usize {
        self.event_capacity
    }
    pub const fn total_events(&self) -> u64 {
        self.total_events
    }
    pub const fn horizon_us(&self) -> u64 {
        self.horizon_us
    }

    /// Schedule-owned allocations, including one Arc control block. Numerical
    /// profile payloads are reported separately by the population memory plans.
    pub fn allocation_bytes(&self) -> Result<u64, String> {
        size_of_val(self.profiles.as_ref())
            .checked_add(size_of_val(self.instances.as_ref()))
            .and_then(|n| n.checked_add(size_of_val(self.counts.as_ref())))
            .and_then(|n| n.checked_add(self.digest.capacity()))
            .and_then(|n| n.checked_add(size_of::<Self>() + 2 * size_of::<usize>()))
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| "physics schedule metadata allocation overflow".into())
    }
}
