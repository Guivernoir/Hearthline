use std::fmt::Write as _;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MODEL_SOURCE_SCHEMA_VERSION: &str = "0.1.0";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelSourceKind {
    Project,
    Blueprint,
    Instance,
    Appliance,
    Connection,
    Scenario,
    Process,
    ControlProgram,
    CapacityPolicy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSourceDocument {
    pub schema_version: String,
    pub path: PathBuf,
    pub kind: ModelSourceKind,
    pub source: String,
}

impl ModelSourceDocument {
    pub fn new(path: PathBuf, kind: ModelSourceKind, source: String) -> Self {
        Self {
            schema_version: MODEL_SOURCE_SCHEMA_VERSION.into(),
            path,
            kind,
            source,
        }
    }

    pub fn digest(&self) -> String {
        let mut digest = Sha256::new();
        digest.update(self.path.to_string_lossy().as_bytes());
        digest.update([0]);
        digest.update(self.source.as_bytes());
        sha256_hex(digest.finalize().as_slice())
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing hexadecimal to a String");
    }
    output
}
