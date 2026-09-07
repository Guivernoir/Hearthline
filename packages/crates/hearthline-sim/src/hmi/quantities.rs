use hearthline_model::FixedValue;

use crate::ConfigError;

pub(super) fn quantize(value: f64) -> Result<FixedValue, ConfigError> {
    if !value.is_finite() {
        return Err(ConfigError::new("process value must be finite"));
    }
    value
        .to_string()
        .parse::<FixedValue>()
        .map_err(|error| ConfigError::new(format!("cannot quantize process value: {error}")))
}

pub(super) fn project(value: FixedValue) -> f64 {
    value.raw() as f64 / FixedValue::SCALE as f64
}
