use std::io::{self, Write};

use serde::Serialize;
use sha2::{Digest, Sha256};

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
