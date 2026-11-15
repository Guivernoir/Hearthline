use std::io::{self, BufRead, Write};

use hearthline_physics::{PhysicsError, Scalar};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::PhysicsProofSession;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum NumericalCause {
    Overflow,
    DivisionByZero,
    InvalidDecimal,
    InvalidGeometry,
    InvalidCoefficient,
    InsufficientResolution,
    InvalidState,
    BufferLength,
    NonPositiveDefinite,
    NonConvergence { iterations: u32, residual_raw: i64 },
}

impl From<PhysicsError> for NumericalCause {
    fn from(error: PhysicsError) -> Self {
        match error {
            PhysicsError::Overflow => Self::Overflow,
            PhysicsError::DivisionByZero => Self::DivisionByZero,
            PhysicsError::InvalidDecimal => Self::InvalidDecimal,
            PhysicsError::InvalidGeometry => Self::InvalidGeometry,
            PhysicsError::InvalidCoefficient => Self::InvalidCoefficient,
            PhysicsError::InsufficientResolution => Self::InsufficientResolution,
            PhysicsError::InvalidState => Self::InvalidState,
            PhysicsError::BufferLength => Self::BufferLength,
            PhysicsError::NonPositiveDefinite => Self::NonPositiveDefinite,
            PhysicsError::NonConvergence {
                iterations,
                residual,
            } => Self::NonConvergence {
                iterations,
                residual_raw: residual.raw(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldRange {
    pub minimum_raw: i64,
    pub maximum_raw: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NumericalFieldRanges {
    pub first_field: FieldRange,
    pub temperature_k: FieldRange,
}

impl PhysicsProofSession {
    pub(in crate::physics) fn field_ranges(&self) -> NumericalFieldRanges {
        fn range(field: &[Scalar]) -> FieldRange {
            let initial = field[0].raw();
            field.iter().fold(
                FieldRange {
                    minimum_raw: initial,
                    maximum_raw: initial,
                },
                |range, value| FieldRange {
                    minimum_raw: range.minimum_raw.min(value.raw()),
                    maximum_raw: range.maximum_raw.max(value.raw()),
                },
            )
        }
        NumericalFieldRanges {
            first_field: range(&self.field),
            temperature_k: range(&self.temperature),
        }
    }
}

pub(super) struct HashWriter<W> {
    pub output: W,
    hash: Sha256,
}

impl<W> HashWriter<W> {
    pub fn new(output: W) -> Self {
        Self {
            output,
            hash: Sha256::new(),
        }
    }

    pub fn finish(self) -> (W, [u8; 64]) {
        (self.output, hex_digest(self.hash.finalize().into()))
    }
}

impl<W: Write> Write for HashWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.output.write(buffer)?;
        self.hash.update(&buffer[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

pub(super) fn hash_json(value: &impl Serialize) -> Result<[u8; 64], serde_json::Error> {
    let mut writer = HashWriter::new(io::sink());
    serde_json::to_writer(&mut writer, value)?;
    Ok(writer.finish().1)
}

pub(super) fn hex_digest(digest: [u8; 32]) -> [u8; 64] {
    let mut output = [0; 64];
    let digits = b"0123456789abcdef";
    for (index, value) in digest.into_iter().enumerate() {
        output[index * 2] = digits[usize::from(value >> 4)];
        output[index * 2 + 1] = digits[usize::from(value & 15)];
    }
    output
}

pub(super) fn read_line(
    input: &mut impl BufRead,
    output: &mut Vec<u8>,
    maximum: usize,
) -> Result<(), String> {
    output.clear();
    loop {
        let available = input.fill_buf().map_err(|e| e.to_string())?;
        if available.is_empty() {
            return Err("truncated physics snapshot".into());
        }
        let available = &available[..available.len().min(maximum - output.len() + 1)];
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |index| index + 1);
        if take > maximum - output.len() {
            return Err("physics snapshot frame exceeds its bounded size".into());
        }
        output.extend_from_slice(&available[..take]);
        input.consume(take);
        if newline.is_some() {
            return Ok(());
        }
    }
}
