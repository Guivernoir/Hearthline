mod connector;
mod frontend;
mod repository;
mod schema;
mod support;

pub use connector::{ConnectorDropReason, ConnectorTransit, SimulatedConnector};
pub use frontend::{FrontendConnection, FrontendConnectionEndpoint};
pub use repository::{ConnectionRepository, LoadedConnection};
pub use schema::{
    CONNECTION_SCHEMA_VERSION, ConnectionConfig, ConnectionDirection, ConnectionEndpoint,
    ConnectionEndpoints, ConnectionProperties, ConnectorPortProfile, TransportKind,
};

use connector::{endpoint_port, negotiated_duplex, validate_endpoint, validate_endpoint_port};
use support::{collect_yaml_paths, default_capacity, default_true};
