use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};

pub(super) struct ByteCounter(pub u64);
impl Write for ByteCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| io::Error::other("schedule frame length overflow"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) struct HashingRead<'a, R> {
    pub input: R,
    pub hash: &'a mut Sha256,
}
impl<R: Read> Read for HashingRead<'_, R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let count = self.input.read(output)?;
        self.hash.update(&output[..count]);
        Ok(count)
    }
}
