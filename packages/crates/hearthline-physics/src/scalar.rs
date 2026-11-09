use core::str::FromStr;

use crate::PhysicsError;

/// Signed decimal billionths. Products/divisions round to nearest, ties to even.
/// Equations must be scaled before constructing their coefficients.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Scalar(i64);

impl Scalar {
    pub const SCALE: i64 = 1_000_000_000;
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(Self::SCALE);

    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> i64 {
        self.0
    }

    pub fn integer(value: i64) -> Result<Self, PhysicsError> {
        value
            .checked_mul(Self::SCALE)
            .map(Self)
            .ok_or(PhysicsError::Overflow)
    }

    pub fn ratio(numerator: i64, denominator: i64) -> Result<Self, PhysicsError> {
        rounded_div(
            i128::from(numerator) * i128::from(Self::SCALE),
            i128::from(denominator),
        )
    }

    /// Preserve small inner products until their ratio is evaluated.
    pub fn ratio_wide(numerator: i128, denominator: i128) -> Result<Self, PhysicsError> {
        rounded_div(
            numerator
                .checked_mul(i128::from(Self::SCALE))
                .ok_or(PhysicsError::Overflow)?,
            denominator,
        )
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, PhysicsError> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or(PhysicsError::Overflow)
    }

    pub fn checked_sub(self, rhs: Self) -> Result<Self, PhysicsError> {
        self.0
            .checked_sub(rhs.0)
            .map(Self)
            .ok_or(PhysicsError::Overflow)
    }

    pub fn checked_mul(self, rhs: Self) -> Result<Self, PhysicsError> {
        rounded_div(
            i128::from(self.0) * i128::from(rhs.0),
            i128::from(Self::SCALE),
        )
    }

    /// One rounded result for a * b / c, without quantizing the product first.
    pub fn checked_mul_div(self, multiplier: Self, divisor: Self) -> Result<Self, PhysicsError> {
        rounded_div(
            i128::from(self.0) * i128::from(multiplier.0),
            i128::from(divisor.0),
        )
    }

    pub(crate) fn from_raw_ratio(numerator: i128, denominator: i128) -> Result<Self, PhysicsError> {
        rounded_div(numerator, denominator)
    }

    pub(crate) fn from_product_sum(sum: i128) -> Result<Self, PhysicsError> {
        rounded_div(sum, i128::from(Self::SCALE))
    }

    pub fn checked_div(self, rhs: Self) -> Result<Self, PhysicsError> {
        rounded_div(
            i128::from(self.0) * i128::from(Self::SCALE),
            i128::from(rhs.0),
        )
    }

    pub fn abs(self) -> Result<Self, PhysicsError> {
        self.0.checked_abs().map(Self).ok_or(PhysicsError::Overflow)
    }

    pub fn sqrt(self) -> Result<Self, PhysicsError> {
        if self.0 < 0 {
            return Err(PhysicsError::InvalidState);
        }
        let scaled = (self.0 as u128) * (Self::SCALE as u128);
        let floor = scaled.isqrt();
        let remainder = scaled - floor * floor;
        let rounded = if remainder > floor { floor + 1 } else { floor };
        i64::try_from(rounded)
            .map(Self)
            .map_err(|_| PhysicsError::Overflow)
    }

    pub fn checked_neg(self) -> Result<Self, PhysicsError> {
        self.0.checked_neg().map(Self).ok_or(PhysicsError::Overflow)
    }
}

fn rounded_div(numerator: i128, denominator: i128) -> Result<Scalar, PhysicsError> {
    if denominator == 0 {
        return Err(PhysicsError::DivisionByZero);
    }
    let mut quotient = numerator
        .checked_div(denominator)
        .ok_or(PhysicsError::Overflow)?;
    let remainder = numerator
        .checked_rem(denominator)
        .ok_or(PhysicsError::Overflow)?
        .unsigned_abs();
    let complement = denominator.unsigned_abs() - remainder;
    if remainder > complement || (remainder == complement && quotient % 2 != 0) {
        let direction = if (numerator < 0) == (denominator < 0) {
            1
        } else {
            -1
        };
        quotient = quotient
            .checked_add(direction)
            .ok_or(PhysicsError::Overflow)?;
    }
    i64::try_from(quotient)
        .map(Scalar)
        .map_err(|_| PhysicsError::Overflow)
}

impl FromStr for Scalar {
    type Err = PhysicsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (negative, value) = value
            .strip_prefix('-')
            .map_or((false, value), |s| (true, s));
        let (whole, fraction) = value
            .split_once('.')
            .map_or((value, None), |(a, b)| (a, Some(b)));
        if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
            return Err(PhysicsError::InvalidDecimal);
        }
        let mut magnitude = whole
            .parse::<i128>()
            .map_err(|_| PhysicsError::Overflow)?
            .checked_mul(i128::from(Self::SCALE))
            .ok_or(PhysicsError::Overflow)?;
        if let Some(fraction) = fraction {
            if fraction.is_empty()
                || fraction.len() > 9
                || !fraction.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(PhysicsError::InvalidDecimal);
            }
            let digits = u32::try_from(fraction.len()).map_err(|_| PhysicsError::InvalidDecimal)?;
            let scaled = fraction
                .parse::<i128>()
                .map_err(|_| PhysicsError::InvalidDecimal)?
                * 10_i128.pow(9 - digits);
            magnitude = magnitude
                .checked_add(scaled)
                .ok_or(PhysicsError::Overflow)?;
        }
        if negative {
            magnitude = magnitude.checked_neg().ok_or(PhysicsError::Overflow)?;
        }
        i64::try_from(magnitude)
            .map(Self)
            .map_err(|_| PhysicsError::Overflow)
    }
}
