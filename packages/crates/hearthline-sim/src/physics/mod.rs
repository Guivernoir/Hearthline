//! Isolated numerical proof sessions. Canonical plant/HMI execution is unchanged.

mod buffers;
mod checkpoint;
pub mod clock;
mod flow;
mod material;
mod mechanics;
pub mod population;
pub mod schedule;
pub mod worker;
use buffers::{ProofBuffers, TransportBuffers};
use flow::FlowBuffers;
pub use flow::{FlowCheckpoint, FlowStepCheckpoint};
use material::VariableDryingBuffers;
pub use material::{DiffusionCheckpoint, ThermalCheckpoint};
pub use mechanics::MechanicalCheckpoint;
use mechanics::SpecimenMechanics;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use hearthline_config::physics::PhysicsProofConfig;
use hearthline_config::physics::geometry::SolidGeometryDocument;
use hearthline_config::physics::material::DryingMaterialDocument;
use hearthline_physics::drying::{self, DryingWorkspace};
use hearthline_physics::flow::{FlowAdvanceError, FlowAdvanceReport};
use hearthline_physics::transport::{Boundary, SolveReport, TransportSystem};
use hearthline_physics::{NUMERICAL_CONTRACT, PhysicsError, Scalar};
use hearthline_project::physics::{CompiledPhysicsProof, CompiledProofCase};
use serde::{Deserialize, Serialize};

pub const PHYSICS_REPLAY_SCHEMA: &str = "0.1.0";
pub const PHYSICS_REPLAY_DOCUMENT_LIMIT: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicsStepError {
    Numerical(PhysicsError),
    Flow(FlowAdvanceError),
}
impl From<PhysicsError> for PhysicsStepError {
    fn from(value: PhysicsError) -> Self {
        Self::Numerical(value)
    }
}
impl std::fmt::Display for PhysicsStepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Numerical(error) => error.fmt(f),
            Self::Flow(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for PhysicsStepError {}

#[derive(Clone, Copy, Debug)]
pub enum PhysicsStepReport {
    Transport(SolveReport),
    Flow(FlowAdvanceReport),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicsCheckpoint {
    pub step: u32,
    pub time_us: u64,
    pub first_field_raw: Vec<i64>,
    pub temperature_k_raw: Vec<i64>,
    pub balance_error_raw: [i64; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mechanics: Option<MechanicalCheckpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<FlowCheckpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffusion: Option<DiffusionCheckpoint>,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicsReplay {
    pub schema_version: String,
    pub numerical_contract: String,
    pub model_digest: String,
    pub config: PhysicsProofConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<SolidGeometryDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<DryingMaterialDocument>,
    pub initial: PhysicsCheckpoint,
    pub checkpoints: Vec<PhysicsCheckpoint>,
    pub final_state: PhysicsCheckpoint,
}

pub struct PhysicsProofSession {
    model: Arc<CompiledPhysicsProof>,
    field: Vec<Scalar>,
    temperature: Vec<Scalar>,
    first_trial: Vec<Scalar>,
    temperature_trial: Vec<Scalar>,
    buffers: ProofBuffers,
    step: u32,
    balance_error: [Scalar; 2],
    mechanics: Option<SpecimenMechanics>,
    variable_drying: Option<VariableDryingBuffers>,
    diffusion: Option<DiffusionCheckpoint>,
}

impl PhysicsProofSession {
    pub fn new(model: Arc<CompiledPhysicsProof>) -> Result<Self, String> {
        let n = model.cell_count();
        let mut session = Self {
            variable_drying: model
                .material()
                .map(|_| VariableDryingBuffers::new(n))
                .transpose()?,
            diffusion: None,
            mechanics: model.mechanics().map(SpecimenMechanics::new).transpose()?,
            buffers: if model.flow().is_some() {
                ProofBuffers::Flow(FlowBuffers::new(&model)?)
            } else {
                ProofBuffers::Transport(TransportBuffers::new(n)?)
            },
            field: field(n, Scalar::ZERO)?,
            temperature: field(n, Scalar::ONE)?,
            first_trial: field(n, Scalar::ZERO)?,
            temperature_trial: field(n, Scalar::ZERO)?,
            model,
            step: 0,
            balance_error: [Scalar::ZERO; 2],
        };
        session.initialize().map_err(|e| e.to_string())?;
        if let ProofBuffers::Transport(buffers) = &mut session.buffers {
            material::initialize_caloric_boundary(&session.model, buffers)
                .map_err(|e| e.to_string())?;
        }
        Ok(session)
    }

    fn initialize(&mut self) -> Result<(), PhysicsError> {
        let ProofBuffers::Transport(buffers) = &mut self.buffers else {
            let ProofBuffers::Flow(buffers) = &self.buffers else {
                return Err(PhysicsError::InvalidState);
            };
            let model = self.model.flow().ok_or(PhysicsError::InvalidState)?;
            for (cell, &state) in buffers.state.iter().enumerate() {
                [self.field[cell], self.temperature[cell]] = model.readings(state)?;
            }
            return Ok(());
        };
        match self.model.case() {
            CompiledProofCase::CompressibleReceiver => return Err(PhysicsError::InvalidState),
            CompiledProofCase::Receiver {
                storage,
                temperature,
                pressure,
                receiver_pressure,
                conductance,
            } => {
                self.field.fill(pressure);
                self.temperature.fill(temperature);
                *self.field.last_mut().ok_or(PhysicsError::InvalidGeometry)? = receiver_pressure;
                buffers.storage.fill(storage);
                buffers.conductance = [conductance; 3];
            }
            CompiledProofCase::Drying {
                moisture,
                equilibrium,
                temperature,
                ambient,
                density,
                diffusion,
                surface_transfer,
                heat_capacity,
                conductivity,
                heat_transfer,
                ..
            } => {
                self.field.fill(moisture);
                self.temperature.fill(temperature);
                let grid = self.model.grid();
                let dry_mass = grid.volume()?.checked_mul(density)?;
                buffers.storage.fill(dry_mass);
                buffers
                    .thermal_storage
                    .fill(dry_mass.checked_mul(heat_capacity)?);
                buffers.conductance = grid.conductance(diffusion.checked_mul(density)?)?;
                buffers.thermal_conductance = grid.conductance(conductivity)?;
                let shape = grid.shape();
                let spacing = self
                    .model
                    .config()
                    .spacing_m
                    .each_ref()
                    .map(|s| s.parse::<Scalar>());
                let spacing = [
                    spacing[0].as_ref().copied().map_err(|e| *e)?,
                    spacing[1].as_ref().copied().map_err(|e| *e)?,
                    spacing[2].as_ref().copied().map_err(|e| *e)?,
                ];
                for cell in 0..self.model.cell_count() {
                    let coordinates = [
                        cell % shape[0],
                        (cell / shape[0]) % shape[1],
                        cell / (shape[0] * shape[1]),
                    ];
                    let mut area = Scalar::ZERO;
                    for axis in 0..3 {
                        let faces = match self.model.solid_domain() {
                            Some(domain) => {
                                let faces = domain
                                    .exposed_faces(cell)
                                    .ok_or(PhysicsError::InvalidGeometry)?;
                                i64::from(faces[2 * axis]) + i64::from(faces[2 * axis + 1])
                            }
                            None => {
                                i64::from(coordinates[axis] == 0)
                                    + i64::from(coordinates[axis] + 1 == shape[axis])
                            }
                        };
                        area = area.checked_add(
                            grid.volume()?
                                .checked_div(spacing[axis])?
                                .checked_mul(Scalar::integer(faces)?)?,
                        )?;
                    }
                    buffers.boundary[cell] = Boundary {
                        conductance: surface_transfer.checked_mul(density)?.checked_mul(area)?,
                        ambient: equilibrium,
                    };
                    buffers.thermal_boundary[cell] = Boundary {
                        conductance: area.checked_mul(heat_transfer)?,
                        ambient,
                    };
                    if area > Scalar::ZERO
                        && ((surface_transfer > Scalar::ZERO
                            && buffers.boundary[cell].conductance == Scalar::ZERO)
                            || (heat_transfer > Scalar::ZERO
                                && buffers.thermal_boundary[cell].conductance == Scalar::ZERO))
                    {
                        return Err(PhysicsError::InvalidCoefficient);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn step(&mut self) -> Result<PhysicsStepReport, PhysicsStepError> {
        if self.step >= self.model.config().steps {
            return Err(PhysicsError::InvalidState.into());
        }
        let ProofBuffers::Transport(buffers) = &mut self.buffers else {
            let ProofBuffers::Flow(buffers) = &mut self.buffers else {
                return Err(PhysicsError::InvalidState.into());
            };
            let model = self.model.flow().ok_or(PhysicsError::InvalidState)?;
            let (report, balance) =
                buffers.trial(model, &mut self.first_trial, &mut self.temperature_trial)?;
            buffers.commit(Some(report.into()), balance);
            self.field.copy_from_slice(&self.first_trial);
            self.temperature.copy_from_slice(&self.temperature_trial);
            self.balance_error = [balance[0], balance[4]];
            self.step += 1;
            return Ok(PhysicsStepReport::Flow(report));
        };
        let system = TransportSystem::on_grid(
            self.model.transport_grid()?,
            &buffers.storage,
            buffers.conductance,
            &buffers.boundary,
            self.model.limits(),
        )?;
        // Trial fields make the entire coupled step atomic, including ledgers.
        self.first_trial.copy_from_slice(&self.field);
        self.temperature_trial.copy_from_slice(&self.temperature);
        let mut diffusion = None;
        let (first, second) = match self.model.case() {
            CompiledProofCase::CompressibleReceiver => {
                return Err(PhysicsError::InvalidState.into());
            }
            CompiledProofCase::Receiver { .. } => {
                buffers.heat_sources.fill(Scalar::ZERO);
                let report = system.solve(
                    self.model.timestep(),
                    &mut self.first_trial,
                    &buffers.heat_sources,
                    &mut buffers.linear,
                )?;
                if self.first_trial.iter().any(|p| *p <= Scalar::ZERO) {
                    return Err(PhysicsError::InvalidState.into());
                }
                (report, None)
            }
            CompiledProofCase::Drying { latent_heat, .. } => {
                if let Some(variable) = &mut self.variable_drying {
                    let report = variable.trial(
                        &self.model,
                        buffers,
                        [&self.field, &self.temperature],
                        [&mut self.first_trial, &mut self.temperature_trial],
                    )?;
                    diffusion = Some(report.into());
                    (report.moisture.transport, Some(report.thermal))
                } else {
                    let thermal = TransportSystem::on_grid(
                        self.model.transport_grid()?,
                        &buffers.thermal_storage,
                        buffers.thermal_conductance,
                        &buffers.thermal_boundary,
                        self.model.limits(),
                    )?;
                    let mut workspace = DryingWorkspace {
                        moisture_trial: &mut self.first_trial,
                        temperature_trial: &mut self.temperature_trial,
                        linear: &mut buffers.linear,
                        heat_sources: &mut buffers.heat_sources,
                    };
                    let report = drying::trial_step(
                        &system,
                        &thermal,
                        self.model.timestep(),
                        latent_heat,
                        &self.field,
                        &self.temperature,
                        &mut workspace,
                    )?;
                    (report.moisture, Some(report.thermal))
                }
            }
        };
        let balance = [
            self.balance_error[0].checked_add(first.balance_error)?,
            self.balance_error[1].checked_add(second.map_or(Scalar::ZERO, |r| r.balance_error))?,
        ];
        let mechanical_report = match (
            self.model.mechanics(),
            &mut self.mechanics,
            self.model.case(),
        ) {
            (
                Some(model),
                Some(state),
                CompiledProofCase::Drying {
                    moisture,
                    temperature,
                    ..
                },
            ) => Some(state.trial(
                model,
                &self.first_trial,
                &self.temperature_trial,
                moisture,
                temperature,
            )?),
            (None, None, _) => None,
            _ => return Err(PhysicsError::InvalidState.into()),
        };
        if let (Some(state), Some(report)) = (&mut self.mechanics, mechanical_report) {
            state.commit(report);
        }
        self.field.copy_from_slice(&self.first_trial);
        self.temperature.copy_from_slice(&self.temperature_trial);
        self.balance_error = balance;
        self.diffusion = diffusion;
        self.step += 1;
        Ok(PhysicsStepReport::Transport(first))
    }

    pub fn checkpoint(&self) -> PhysicsCheckpoint {
        let mut checkpoint = PhysicsCheckpoint {
            step: self.step,
            time_us: u64::from(self.step) * self.model.timestep_us(),
            first_field_raw: self.field.iter().map(|v| v.raw()).collect(),
            temperature_k_raw: self.temperature.iter().map(|v| v.raw()).collect(),
            balance_error_raw: self.balance_error.map(Scalar::raw),
            mechanics: self.mechanics.as_ref().map(SpecimenMechanics::checkpoint),
            flow: match &self.buffers {
                ProofBuffers::Flow(buffers) => Some(buffers.checkpoint()),
                _ => None,
            },
            digest: String::new(),
            diffusion: self.diffusion,
        };
        checkpoint.digest = checkpoint_digest(&checkpoint, self.model.digest());
        checkpoint
    }

    pub fn restore(&mut self, checkpoint: &PhysicsCheckpoint) -> Result<(), String> {
        let n = self.model.cell_count();
        if checkpoint.first_field_raw.len() != n
            || checkpoint.temperature_k_raw.len() != n
            || checkpoint.step > self.model.config().steps
            || checkpoint.time_us != u64::from(checkpoint.step) * self.model.timestep_us()
            || checkpoint.digest.as_bytes()
                != checkpoint_digest_bytes(checkpoint, self.model.digest())
            || checkpoint.flow.is_some() != self.model.flow().is_some()
        {
            return Err("invalid physics checkpoint shape, clock or digest".into());
        }
        if checkpoint.temperature_k_raw.iter().any(|v| *v <= 0)
            || checkpoint.first_field_raw.iter().any(|v| {
                *v < 0
                    || (*v == 0 && !matches!(self.model.case(), CompiledProofCase::Drying { .. }))
            })
        {
            return Err("physics checkpoint outside the constitutive domain".into());
        }
        material::validate_checkpoint(&self.model, checkpoint)?;
        if let (ProofBuffers::Flow(buffers), Some(flow)) = (&mut self.buffers, &checkpoint.flow) {
            if checkpoint.mechanics.is_some()
                || checkpoint.balance_error_raw
                    != [flow.balance_error_raw[0], flow.balance_error_raw[4]]
            {
                return Err("flow checkpoint model or balance ledger mismatch".into());
            }
            buffers.validate_restore(
                &self.model,
                flow,
                &checkpoint.first_field_raw,
                &checkpoint.temperature_k_raw,
                checkpoint.step,
            )?;
            buffers.commit(flow.last_step, flow.balance_error_raw.map(Scalar::from_raw));
        }
        match (
            self.model.mechanics(),
            &mut self.mechanics,
            &checkpoint.mechanics,
            self.model.case(),
        ) {
            (
                Some(model),
                Some(state),
                Some(checkpoint_mechanics),
                CompiledProofCase::Drying {
                    moisture,
                    temperature,
                    ..
                },
            ) => {
                let report = state.validate_restore(
                    model,
                    checkpoint_mechanics,
                    &checkpoint.first_field_raw,
                    &checkpoint.temperature_k_raw,
                    [moisture, temperature],
                    checkpoint.step,
                )?;
                state.commit(report);
            }
            (None, None, None, _) => (),
            _ => return Err("physics checkpoint mechanical model mismatch".into()),
        }
        for (field, raw) in self.field.iter_mut().zip(&checkpoint.first_field_raw) {
            *field = Scalar::from_raw(*raw);
        }
        for (field, raw) in self
            .temperature
            .iter_mut()
            .zip(&checkpoint.temperature_k_raw)
        {
            *field = Scalar::from_raw(*raw);
        }
        self.step = checkpoint.step;
        self.diffusion = checkpoint.diffusion;
        self.balance_error = checkpoint.balance_error_raw.map(Scalar::from_raw);
        Ok(())
    }
}

impl PhysicsReplay {
    pub fn record(config: PhysicsProofConfig) -> Result<Self, String> {
        Self::record_with_geometry(config, None)
    }

    pub fn record_with_geometry(
        config: PhysicsProofConfig,
        geometry: Option<SolidGeometryDocument>,
    ) -> Result<Self, String> {
        Self::record_with_resources(config, geometry, None)
    }

    pub fn record_with_resources(
        config: PhysicsProofConfig,
        geometry: Option<SolidGeometryDocument>,
        material: Option<DryingMaterialDocument>,
    ) -> Result<Self, String> {
        validate_recording_capacity(&config, geometry.as_ref(), material.as_ref())?;
        let model = Arc::new(CompiledPhysicsProof::compile_with_resources(
            config.clone(),
            geometry.clone(),
            material.clone(),
        )?);
        let mut session = PhysicsProofSession::new(model.clone())?;
        let initial = session.checkpoint();
        let mut checkpoints = Vec::new();
        for step in 1..=config.steps {
            session
                .step()
                .map_err(|e| format!("physics step {step}: {e}"))?;
            if step == config.steps / 2 {
                checkpoints.push(session.checkpoint());
            }
        }
        Ok(Self {
            schema_version: PHYSICS_REPLAY_SCHEMA.into(),
            numerical_contract: NUMERICAL_CONTRACT.into(),
            model_digest: model.digest().into(),
            config,
            geometry,
            material,
            initial,
            checkpoints,
            final_state: session.checkpoint(),
        })
    }

    pub fn verify(&self) -> Result<(), String> {
        if self.schema_version != PHYSICS_REPLAY_SCHEMA
            || self.numerical_contract != NUMERICAL_CONTRACT
        {
            return Err("unsupported physics replay or numerical contract".into());
        }
        validate_recording_capacity(&self.config, self.geometry.as_ref(), self.material.as_ref())?;
        let model = Arc::new(CompiledPhysicsProof::compile_with_resources(
            self.config.clone(),
            self.geometry.clone(),
            self.material.clone(),
        )?);
        if self.model_digest != model.digest() {
            return Err("physics replay model digest mismatch".into());
        }
        if self.final_state.step != self.config.steps {
            return Err("physics replay does not cover its declared horizon".into());
        }
        let mut previous = 0;
        for checkpoint in &self.checkpoints {
            if checkpoint.step <= previous || checkpoint.step >= self.config.steps {
                return Err("physics checkpoints must be ordered inside the run horizon".into());
            }
            previous = checkpoint.step;
        }
        let mut session = PhysicsProofSession::new(model)?;
        compare(&self.initial, &session.checkpoint())?;
        let mut checkpoints = self.checkpoints.iter().peekable();
        for step in 1..=self.config.steps {
            session
                .step()
                .map_err(|e| format!("physics step {step}: {e}"))?;
            if checkpoints.peek().is_some_and(|c| c.step == step) {
                compare(
                    checkpoints.next().ok_or("missing physics checkpoint")?,
                    &session.checkpoint(),
                )?;
            }
        }
        compare(&self.final_state, &session.checkpoint())
    }
}

fn validate_recording_capacity(
    config: &PhysicsProofConfig,
    geometry: Option<&SolidGeometryDocument>,
    material: Option<&DryingMaterialDocument>,
) -> Result<(), String> {
    // Three accepted-boundary snapshots, worst-case signed decimal integers,
    // delimiters and bounded checkpoint metadata. This adapter is intentionally
    // not the future streaming factory archive.
    let values = CompiledPhysicsProof::planned_storage_with_material(config, geometry, material)?
        .checkpoint_field_bytes
        / 8;
    let config_bytes = serde_json::to_vec(config).map_err(|e| e.to_string())?.len() as u64;
    let geometry_bytes = match geometry {
        Some(document) => serde_json::to_vec(document)
            .map_err(|e| e.to_string())?
            .len() as u64,
        None => 0,
    };
    let material_bytes = match material {
        Some(document) => serde_json::to_vec(document)
            .map_err(|e| e.to_string())?
            .len() as u64,
        None => 0,
    };
    let estimate = values
        .checked_mul(3 * 22)
        .and_then(|bytes| bytes.checked_add(4096))
        .and_then(|bytes| bytes.checked_add(config_bytes))
        .and_then(|bytes| bytes.checked_add(geometry_bytes))
        .and_then(|bytes| bytes.checked_add(material_bytes))
        .ok_or("physics replay size overflow")?;
    if estimate > PHYSICS_REPLAY_DOCUMENT_LIMIT {
        return Err(format!(
            "physics replay estimate {estimate} bytes exceeds the standalone document limit {PHYSICS_REPLAY_DOCUMENT_LIMIT}; streaming factory archives are not implemented"
        ));
    }
    Ok(())
}

fn field<T: Clone>(count: usize, value: T) -> Result<Vec<T>, String> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|e| format!("physics buffer allocation: {e}"))?;
    result.resize(count, value);
    Ok(result)
}

fn checkpoint_digest(checkpoint: &PhysicsCheckpoint, model_digest: &str) -> String {
    let digest = checkpoint_digest_bytes(checkpoint, model_digest);
    std::str::from_utf8(&digest)
        .expect("hexadecimal digest")
        .into()
}

fn checkpoint_digest_bytes(checkpoint: &PhysicsCheckpoint, model_digest: &str) -> [u8; 64] {
    let fields = (
        NUMERICAL_CONTRACT,
        model_digest,
        checkpoint.step,
        checkpoint.time_us,
        &checkpoint.first_field_raw,
        &checkpoint.temperature_k_raw,
        checkpoint.balance_error_raw,
    );
    if let Some(diffusion) = checkpoint.diffusion {
        checkpoint::hash_json(&(fields, &checkpoint.mechanics, diffusion))
    } else {
        match (&checkpoint.mechanics, &checkpoint.flow) {
            (mechanics, Some(flow)) => checkpoint::hash_json(&(fields, mechanics, flow)),
            (Some(mechanics), None) => checkpoint::hash_json(&(fields, mechanics)),
            (None, None) => checkpoint::hash_json(&fields),
        }
    }
    .expect("integer-only physics checkpoint serialization")
}

fn compare(expected: &PhysicsCheckpoint, actual: &PhysicsCheckpoint) -> Result<(), String> {
    match (&expected.flow, &actual.flow) {
        (Some(a), Some(b)) => {
            if a.conserved_raw.len() != b.conserved_raw.len() {
                return Err(format!("step {}: flow field shape diverged", actual.step));
            }
            for (cell, (a, b)) in a.conserved_raw.iter().zip(&b.conserved_raw).enumerate() {
                for (component, (a, b)) in a.iter().zip(b).enumerate() {
                    if a != b {
                        return Err(format!(
                            "step {} cell {cell} flow component {component}: expected raw {a}, actual raw {b}",
                            actual.step
                        ));
                    }
                }
            }
        }
        (None, None) => (),
        _ => return Err(format!("step {}: flow fields missing", actual.step)),
    }
    for (name, a, b) in [
        (
            "first_field",
            &expected.first_field_raw,
            &actual.first_field_raw,
        ),
        (
            "temperature_k",
            &expected.temperature_k_raw,
            &actual.temperature_k_raw,
        ),
    ] {
        if a.len() != b.len() {
            return Err(format!("step {}: {name} shape diverged", actual.step));
        }
        for (cell, (a, b)) in a.iter().zip(b).enumerate() {
            if a != b {
                return Err(format!(
                    "step {} cell {cell} field {name}: expected raw {a}, actual raw {b}",
                    actual.step
                ));
            }
        }
    }
    match (&expected.mechanics, &actual.mechanics) {
        (Some(expected), Some(actual_mechanics)) => {
            for (name, a, b, width) in [
                (
                    "displacement",
                    &expected.displacement_raw,
                    &actual_mechanics.displacement_raw,
                    3,
                ),
                (
                    "stress",
                    &expected.stress_raw,
                    &actual_mechanics.stress_raw,
                    6,
                ),
            ] {
                if a.len() != b.len() {
                    return Err(format!(
                        "step {}: mechanical {name} shape diverged",
                        actual.step
                    ));
                }
                for (index, (a, b)) in a.iter().zip(b).enumerate() {
                    if a != b {
                        return Err(format!(
                            "step {} mechanical {name} entity {} component {}: expected raw {a}, actual raw {b}",
                            actual.step,
                            index / width,
                            index % width
                        ));
                    }
                }
            }
        }
        (None, None) => (),
        _ => return Err(format!("step {}: mechanical fields missing", actual.step)),
    }
    if let Some(field) = material::first_report_difference(expected.diffusion, actual.diffusion) {
        return Err(format!("step {}: {field} diverged", actual.step));
    }
    if expected != actual {
        return Err(format!(
            "step {}: physics clock, balance ledger or digest diverged",
            actual.step
        ));
    }
    Ok(())
}
