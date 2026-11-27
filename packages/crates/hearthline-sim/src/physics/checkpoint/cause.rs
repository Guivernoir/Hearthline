use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::NumericalCause;

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field {
    Kind,
    Iterations,
    ResidualRaw,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Kind {
    Overflow,
    DivisionByZero,
    InvalidDecimal,
    InvalidGeometry,
    InvalidCoefficient,
    InsufficientResolution,
    InvalidState,
    BufferLength,
    NonPositiveDefinite,
    NonConvergence,
}

// Internally tagged enum derives buffer an arbitrary map before validating it.
// This visitor accepts any field order while retaining only three typed values.
impl<'de> Deserialize<'de> for NumericalCause {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CauseVisitor;
        impl<'de> Visitor<'de> for CauseVisitor {
            type Value = NumericalCause;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a numerical cause with bounded typed fields")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut kind = None;
                let mut iterations = None;
                let mut residual_raw = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Kind => {
                            if kind.is_some() {
                                return Err(de::Error::duplicate_field("kind"));
                            }
                            kind = Some(map.next_value()?);
                        }
                        Field::Iterations => {
                            if iterations.is_some() {
                                return Err(de::Error::duplicate_field("iterations"));
                            }
                            iterations = Some(map.next_value()?);
                        }
                        Field::ResidualRaw => {
                            if residual_raw.is_some() {
                                return Err(de::Error::duplicate_field("residual_raw"));
                            }
                            residual_raw = Some(map.next_value()?);
                        }
                    }
                }
                let kind = kind.ok_or_else(|| de::Error::missing_field("kind"))?;
                if matches!(kind, Kind::NonConvergence) {
                    return Ok(NumericalCause::NonConvergence {
                        iterations: iterations
                            .ok_or_else(|| de::Error::missing_field("iterations"))?,
                        residual_raw: residual_raw
                            .ok_or_else(|| de::Error::missing_field("residual_raw"))?,
                    });
                }
                if iterations.is_some() || residual_raw.is_some() {
                    return Err(de::Error::custom(
                        "only non-convergence causes accept iteration and residual fields",
                    ));
                }
                Ok(match kind {
                    Kind::Overflow => NumericalCause::Overflow,
                    Kind::DivisionByZero => NumericalCause::DivisionByZero,
                    Kind::InvalidDecimal => NumericalCause::InvalidDecimal,
                    Kind::InvalidGeometry => NumericalCause::InvalidGeometry,
                    Kind::InvalidCoefficient => NumericalCause::InvalidCoefficient,
                    Kind::InsufficientResolution => NumericalCause::InsufficientResolution,
                    Kind::InvalidState => NumericalCause::InvalidState,
                    Kind::BufferLength => NumericalCause::BufferLength,
                    Kind::NonPositiveDefinite => NumericalCause::NonPositiveDefinite,
                    Kind::NonConvergence => unreachable!("handled before unit variants"),
                })
            }
        }
        deserializer.deserialize_map(CauseVisitor)
    }
}
