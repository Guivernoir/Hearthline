use std::fmt::Write as _;

use sha2::{Digest, Sha256};

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len().saturating_mul(2));
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing hexadecimal to a String");
    }
    output
}
