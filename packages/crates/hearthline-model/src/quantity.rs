use core::fmt::{self, Display, Formatter};
use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};
use core::str::FromStr;

/// Dimensionless fixed-point value used where a signal has no declared unit.
///
/// The representation is signed millionths. Configuration adapters quantize
/// decimal input before constructing this type; deterministic runtime code only
/// operates on the integer representation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FixedValue(i64);

impl FixedValue {
    pub const SCALE: i64 = 1_000_000;
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(Self::SCALE);

    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    pub const fn from_integer(value: i64) -> Self {
        Self(value.saturating_mul(Self::SCALE))
    }

    /// Parses a source literal without allocation for deterministic constants.
    pub const fn from_decimal_literal(source: &str) -> Self {
        let bytes = source.as_bytes();
        let mut index = 0;
        let mut negative = false;
        if !bytes.is_empty() && bytes[0] == b'-' {
            negative = true;
            index = 1;
        }
        let mut whole = 0_i64;
        let mut fraction = 0_i64;
        let mut fraction_digits = 0_u32;
        let mut decimal = false;
        let mut saw_digit = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if byte == b'_' {
                index += 1;
                continue;
            }
            if byte == b'.' && !decimal {
                decimal = true;
                index += 1;
                continue;
            }
            if byte < b'0' || byte > b'9' {
                panic!("invalid fixed-point literal");
            }
            saw_digit = true;
            let digit = (byte - b'0') as i64;
            if decimal {
                if fraction_digits == 6 {
                    panic!("fixed-point literal exceeds six decimal places");
                }
                fraction = fraction.saturating_mul(10).saturating_add(digit);
                fraction_digits += 1;
            } else {
                whole = whole.saturating_mul(10).saturating_add(digit);
            }
            index += 1;
        }
        if !saw_digit {
            panic!("fixed-point literal has no digits");
        }
        let mut padding = fraction_digits;
        while padding < 6 {
            fraction = fraction.saturating_mul(10);
            padding += 1;
        }
        let magnitude = whole.saturating_mul(Self::SCALE).saturating_add(fraction);
        Self(if negative {
            magnitude.saturating_neg()
        } else {
            magnitude
        })
    }

    pub const fn from_ratio(numerator: i64, denominator: i64) -> Result<Self, QuantityError> {
        if denominator == 0 {
            return Err(QuantityError::InvalidScale);
        }
        let scaled = (numerator as i128) * (Self::SCALE as i128) / (denominator as i128);
        if scaled > i64::MAX as i128 || scaled < i64::MIN as i128 {
            Err(QuantityError::Overflow)
        } else {
            Ok(Self(scaled as i64))
        }
    }

    pub const fn from_u64_ratio(numerator: u64, denominator: u64) -> Self {
        if denominator == 0 {
            return Self::ZERO;
        }
        let scaled = (numerator as u128).saturating_mul(Self::SCALE as u128) / denominator as u128;
        if scaled > i64::MAX as u128 {
            Self(i64::MAX)
        } else {
            Self(scaled as i64)
        }
    }

    pub const fn raw(self) -> i64 {
        self.0
    }

    pub const fn abs(self) -> Self {
        Self(self.0.saturating_abs())
    }

    pub const fn round_to_u64(self) -> u64 {
        if self.0 <= 0 {
            return 0;
        }
        self.0.saturating_add(Self::SCALE / 2) as u64 / Self::SCALE as u64
    }

    pub const fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    pub const fn saturating_mul(self, other: Self) -> Self {
        let value = (self.0 as i128) * (other.0 as i128) / (Self::SCALE as i128);
        if value > i64::MAX as i128 {
            Self(i64::MAX)
        } else if value < i64::MIN as i128 {
            Self(i64::MIN)
        } else {
            Self(value as i64)
        }
    }

    pub const fn checked_div(self, other: Self) -> Result<Self, QuantityError> {
        if other.0 == 0 {
            return Err(QuantityError::InvalidScale);
        }
        let value = (self.0 as i128) * (Self::SCALE as i128) / (other.0 as i128);
        if value > i64::MAX as i128 || value < i64::MIN as i128 {
            Err(QuantityError::Overflow)
        } else {
            Ok(Self(value as i64))
        }
    }

    pub const fn saturating_div(self, other: Self) -> Self {
        match self.checked_div(other) {
            Ok(value) => value,
            Err(QuantityError::Overflow) if (self.0 < 0) ^ (other.0 < 0) => Self(i64::MIN),
            Err(QuantityError::Overflow) => Self(i64::MAX),
            Err(QuantityError::InvalidScale | QuantityError::InvalidDecimal) => Self::ZERO,
        }
    }

    pub const fn clamp(self, minimum: Self, maximum: Self) -> Self {
        if self.0 < minimum.0 {
            minimum
        } else if self.0 > maximum.0 {
            maximum
        } else {
            self
        }
    }
}

impl Add for FixedValue {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        self.saturating_add(other)
    }
}

impl AddAssign for FixedValue {
    fn add_assign(&mut self, other: Self) {
        *self = self.saturating_add(other);
    }
}

impl Sub for FixedValue {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        self.saturating_sub(other)
    }
}

impl SubAssign for FixedValue {
    fn sub_assign(&mut self, other: Self) {
        *self = self.saturating_sub(other);
    }
}

impl Mul for FixedValue {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        self.saturating_mul(other)
    }
}

impl MulAssign for FixedValue {
    fn mul_assign(&mut self, other: Self) {
        *self = self.saturating_mul(other);
    }
}

impl Div for FixedValue {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        self.saturating_div(other)
    }
}

impl Neg for FixedValue {
    type Output = Self;

    fn neg(self) -> Self {
        Self(self.0.saturating_neg())
    }
}

#[macro_export]
macro_rules! fixed {
    ($value:literal) => {
        $crate::FixedValue::from_decimal_literal(stringify!($value))
    };
    (-$value:literal) => {
        $crate::FixedValue::from_decimal_literal(concat!("-", stringify!($value)))
    };
}

impl Display for FixedValue {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let magnitude = self.0.unsigned_abs();
        let whole = magnitude / Self::SCALE as u64;
        let fraction = magnitude % Self::SCALE as u64;
        if self.0 < 0 {
            formatter.write_str("-")?;
        }
        write!(formatter, "{whole}.{fraction:06}")
    }
}

impl FromStr for FixedValue {
    type Err = QuantityError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let (negative, source) = source
            .strip_prefix('-')
            .map_or((false, source), |value| (true, value));
        let (whole, fraction) = source.split_once('.').unwrap_or((source, ""));
        if whole.is_empty()
            || fraction.len() > 6
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(QuantityError::InvalidDecimal);
        }
        let whole = whole
            .parse::<i64>()
            .map_err(|_| QuantityError::InvalidDecimal)?;
        let fraction = if fraction.is_empty() {
            0
        } else {
            let digits = fraction
                .parse::<i64>()
                .map_err(|_| QuantityError::InvalidDecimal)?;
            digits
                .checked_mul(10_i64.pow(6 - fraction.len() as u32))
                .ok_or(QuantityError::Overflow)?
        };
        let magnitude = whole
            .checked_mul(Self::SCALE)
            .and_then(|value| value.checked_add(fraction))
            .ok_or(QuantityError::Overflow)?;
        Ok(Self(if negative { -magnitude } else { magnitude }))
    }
}

pub trait QuantityUnit {
    const SYMBOL: &'static str;
    const SCALE: i64;
}

pub struct Quantity<U: QuantityUnit> {
    raw: i64,
    unit: PhantomData<U>,
}

impl<U: QuantityUnit> Quantity<U> {
    pub const ZERO: Self = Self::from_raw(0);

    pub const fn from_raw(raw: i64) -> Self {
        Self {
            raw,
            unit: PhantomData,
        }
    }

    pub const fn raw(self) -> i64 {
        self.raw
    }

    pub const fn checked_add(self, other: Self) -> Result<Self, QuantityError> {
        match self.raw.checked_add(other.raw) {
            Some(raw) => Ok(Self::from_raw(raw)),
            None => Err(QuantityError::Overflow),
        }
    }

    pub const fn checked_sub(self, other: Self) -> Result<Self, QuantityError> {
        match self.raw.checked_sub(other.raw) {
            Some(raw) => Ok(Self::from_raw(raw)),
            None => Err(QuantityError::Overflow),
        }
    }

    pub const fn saturating_add(self, other: Self) -> Self {
        Self::from_raw(self.raw.saturating_add(other.raw))
    }

    pub const fn saturating_sub(self, other: Self) -> Self {
        Self::from_raw(self.raw.saturating_sub(other.raw))
    }

    pub const fn clamp(self, minimum: Self, maximum: Self) -> Self {
        if self.raw < minimum.raw {
            minimum
        } else if self.raw > maximum.raw {
            maximum
        } else {
            self
        }
    }
}

impl<U: QuantityUnit> Clone for Quantity<U> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<U: QuantityUnit> Copy for Quantity<U> {}

impl<U: QuantityUnit> Default for Quantity<U> {
    fn default() -> Self {
        Self::ZERO
    }
}

impl<U: QuantityUnit> PartialEq for Quantity<U> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<U: QuantityUnit> Eq for Quantity<U> {}

impl<U: QuantityUnit> PartialOrd for Quantity<U> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<U: QuantityUnit> Ord for Quantity<U> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.raw.cmp(&other.raw)
    }
}

impl<U: QuantityUnit> fmt::Debug for Quantity<U> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}", self.raw, U::SYMBOL)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuantityError {
    Overflow,
    InvalidScale,
    InvalidDecimal,
}

impl Display for QuantityError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => formatter.write_str("fixed-point quantity overflow"),
            Self::InvalidScale => formatter.write_str("fixed-point quantity has an invalid scale"),
            Self::InvalidDecimal => formatter.write_str("invalid fixed-point decimal"),
        }
    }
}

macro_rules! unit {
    ($name:ident, $alias:ident, $symbol:literal, $scale:expr) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $name;

        impl QuantityUnit for $name {
            const SYMBOL: &'static str = $symbol;
            const SCALE: i64 = $scale;
        }

        pub type $alias = Quantity<$name>;
    };
}

unit!(Millibar, Pressure, "mbar", 1_000);
unit!(MilliCelsius, Temperature, "mdegC", 1_000);
unit!(MilliliterPerMinute, Flow, "mL/min", 1_000);
unit!(Gram, Mass, "g", 1_000);
unit!(PartPerMillion, Concentration, "ppm", 1_000_000);
unit!(Micrometer, Position, "um", 1_000_000);
unit!(Millidegree, Angle, "mdeg", 1_000);
unit!(BasisPoint, Percentage, "bp", 10_000);
unit!(Microsecond, Duration, "us", 1_000_000);
unit!(MicrometerPerSecond, LinearSpeed, "um/s", 1);
unit!(MillidegreePerSecond, AngularSpeed, "mdeg/s", 1);
