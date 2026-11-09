#![no_std]

//! Numerical proof kernels, not a qualified factory CFD or material model.
//! Storage is supplied by the host; successful steps alone publish new state.

pub mod drying;
pub mod flow;
pub mod gas;
pub mod material;
pub mod mechanics;
pub mod scalar;
pub mod transport;

pub use scalar::Scalar;

pub const NUMERICAL_CONTRACT: &str = "hearthline-physics/1:decimal-9:nearest-even";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicsError {
    Overflow,
    DivisionByZero,
    InvalidDecimal,
    InvalidGeometry,
    InvalidCoefficient,
    InsufficientResolution,
    InvalidState,
    BufferLength,
    NonPositiveDefinite,
    NonConvergence { iterations: u32, residual: Scalar },
}

impl core::fmt::Display for PhysicsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Overflow => f.write_str("physical arithmetic overflow"),
            Self::DivisionByZero => f.write_str("physical division by zero"),
            Self::InvalidDecimal => {
                f.write_str("expected an exact decimal with at most nine fractional digits")
            }
            Self::InvalidGeometry => f.write_str("invalid or unrepresentable physical geometry"),
            Self::InvalidCoefficient => f.write_str("invalid physical coefficient or solver limit"),
            Self::InsufficientResolution => f.write_str("physical timestep or coefficient is below the numerical resolution; review nondimensional scales"),
            Self::InvalidState => f.write_str("physical state outside the model domain"),
            Self::BufferLength => f.write_str("physical field or workspace has the wrong length"),
            Self::NonPositiveDefinite => f.write_str(
                "mechanical system is singular, indefinite, or below numerical resolution",
            ),
            Self::NonConvergence {
                iterations,
                residual,
            } => write!(
                f,
                "solver did not converge after {iterations} sweeps (residual raw={})",
                residual.raw()
            ),
        }
    }
}

impl core::error::Error for PhysicsError {}
