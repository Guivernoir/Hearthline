//! Count the expanded YAML before allocating typed vectors or cloned aliases.

use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

// String's YAML deserializer also accepts numeric scalars. Disallow that
// coercion: the generic size visitor cannot see a numeric scalar's raw length.
pub(super) fn text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    struct TextVisitor;
    impl Visitor<'_> for TextVisitor {
        type Value = String;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a YAML string; quote physical decimal values")
        }
        fn visit_str<E: de::Error>(self, value: &str) -> Result<String, E> {
            Ok(value.to_owned())
        }
    }
    deserializer.deserialize_any(TextVisitor)
}

struct Text(String);
impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        text(deserializer).map(Self)
    }
}
pub(super) fn texts<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[String; 3], D::Error> {
    <[Text; 3]>::deserialize(deserializer).map(|values| values.map(|value| value.0))
}
pub(super) fn optional_text<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<Text>::deserialize(deserializer).map(|value| value.map(|value| value.0))
}

pub(super) fn validate(source: &str, limit: usize) -> Result<(), String> {
    let mut remaining = limit;
    Node {
        remaining: &mut remaining,
        depth: 0,
    }
    .deserialize(serde_yaml_ng::Deserializer::from_str(source))
    .map_err(|error| error.to_string())
}

struct Node<'a> {
    remaining: &'a mut usize,
    depth: u8,
}

impl Node<'_> {
    fn consume<E: de::Error>(&mut self, bytes: usize) -> Result<(), E> {
        *self.remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or_else(|| E::custom("expanded connected gas source exceeds its size budget"))?;
        Ok(())
    }
}

impl<'de> DeserializeSeed<'de> for Node<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(mut self, deserializer: D) -> Result<(), D::Error> {
        if self.depth >= 32 {
            return Err(de::Error::custom(
                "connected gas YAML exceeds 32 nesting levels",
            ));
        }
        self.consume::<D::Error>(32)?;
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Node<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded, untagged connected gas YAML")
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: de::Error>(mut self, value: &str) -> Result<(), E> {
        self.consume(value.len())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        while sequence
            .next_element_seed(Node {
                remaining: self.remaining,
                depth: self.depth + 1,
            })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while map
            .next_key_seed(Node {
                remaining: self.remaining,
                depth: self.depth + 1,
            })?
            .is_some()
        {
            map.next_value_seed(Node {
                remaining: self.remaining,
                depth: self.depth + 1,
            })?;
        }
        Ok(())
    }
}
