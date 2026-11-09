use hearthline_physics::Scalar;
use hearthline_physics::transport::Boundary;

use super::{field, flow::FlowBuffers};

pub(super) enum ProofBuffers {
    Transport(TransportBuffers),
    Flow(FlowBuffers),
}

pub(super) struct TransportBuffers {
    pub storage: Vec<Scalar>,
    pub thermal_storage: Vec<Scalar>,
    pub boundary: Vec<Boundary>,
    pub thermal_boundary: Vec<Boundary>,
    pub linear: Vec<Scalar>,
    pub heat_sources: Vec<Scalar>,
    pub conductance: [Scalar; 3],
    pub thermal_conductance: [Scalar; 3],
}

impl TransportBuffers {
    pub fn new(n: usize) -> Result<Self, String> {
        Ok(Self {
            storage: field(n, Scalar::ONE)?,
            thermal_storage: field(n, Scalar::ONE)?,
            boundary: field(n, Boundary::default())?,
            thermal_boundary: field(n, Boundary::default())?,
            linear: field(n, Scalar::ZERO)?,
            heat_sources: field(n, Scalar::ZERO)?,
            conductance: [Scalar::ZERO; 3],
            thermal_conductance: [Scalar::ZERO; 3],
        })
    }
}
