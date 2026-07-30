use core::borrow::Borrow;
use core::fmt::{self, Display, Formatter};
use core::ops::Deref;
use core::str::FromStr;
use heapless::String as FixedBuffer;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapacityError;

impl Display for CapacityError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("value exceeds fixed storage capacity")
    }
}

impl core::error::Error for CapacityError {}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Text<const N: usize>(FixedBuffer<N>);

impl<const N: usize> Text<N> {
    pub fn try_new(value: &str) -> Result<Self, CapacityError> {
        FixedBuffer::try_from(value)
            .map(Self)
            .map_err(|_| CapacityError)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn push_str(&mut self, value: &str) -> Result<(), CapacityError> {
        self.0.push_str(value).map_err(|_| CapacityError)
    }
}

impl<const N: usize> From<&str> for Text<N> {
    fn from(value: &str) -> Self {
        Self::try_new(value).expect("fixed text literal exceeds capacity")
    }
}

impl<const N: usize> FromStr for Text<N> {
    type Err = CapacityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl<const N: usize> AsRef<str> for Text<N> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> Borrow<str> for Text<N> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> Deref for Text<N> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<const N: usize> Display for Text<N> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl<const N: usize> fmt::Write for Text<N> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.push_str(value).map_err(|_| fmt::Error)
    }
}

impl<const N: usize> Serialize for Text<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de, const N: usize> Deserialize<'de> for Text<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TextVisitor<const N: usize>;

        impl<const N: usize> Visitor<'_> for TextVisitor<N> {
            type Value = Text<N>;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                write!(formatter, "a UTF-8 string no longer than {N} bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Text::try_new(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(TextVisitor::<N>)
    }
}
