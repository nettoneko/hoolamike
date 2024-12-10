use {std::io::Read, validate_hash::ValidateHashReader, validate_size::ValidateSizeReader};

pub mod validate_size {
    use std::io::{Error, ErrorKind, Read, Result};

    pub struct ValidateSizeReader<R> {
        inner: R,
        expected: u64,
        total: u64,
        finished: bool,
    }

    impl<R: Read> ValidateSizeReader<R> {
        pub fn new(inner: R, expected: u64) -> Self {
            ValidateSizeReader {
                inner,
                expected,
                total: 0,
                finished: false,
            }
        }
    }

    impl<R: Read> Read for ValidateSizeReader<R> {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            if self.finished {
                return Ok(0);
            }

            let n = self.inner.read(buf)?;
            self.total += n as u64;

            if n == 0 {
                // EOF reached. Validate the size.
                self.finished = true;
                if self.total != self.expected {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        format!("ValidateSizeReader: expected {} bytes, got {}", self.expected, self.total),
                    ));
                }
            }

            Ok(n)
        }
    }
}

pub mod validate_hash {
    use {
        crate::install_modlist::download_cache::to_base_64_from_u64,
        std::{
            hash::Hasher,
            io::{Error, ErrorKind, Read, Result},
        },
        tap::prelude::*,
    };

    pub struct ValidateHashReader<R> {
        inner: R,
        finished: bool,
        expected_hash: u64,
        hash_state: xxhash_rust::xxh64::Xxh64,
    }

    impl<R: Read> ValidateHashReader<R> {
        pub fn new(inner: R, expected_hash: u64) -> Self {
            ValidateHashReader {
                inner,
                finished: false,
                hash_state: xxhash_rust::xxh64::Xxh64::new(0),
                expected_hash,
            }
        }
    }

    impl<R: Read> Read for ValidateHashReader<R> {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            if self.finished {
                return Ok(0);
            }
            let n = self.inner.read(buf)?;
            self.hash_state.update(&buf[..n]);

            if n == 0 {
                // EOF reached. Validate the size.
                self.finished = true;
                let hash = self.hash_state.finish();
                if self.expected_hash != hash {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        format!(
                            "ValidateHashReader: expected {}, got {}",
                            to_base_64_from_u64(self.expected_hash),
                            to_base_64_from_u64(hash)
                        ),
                    ));
                }
            }

            Ok(n)
        }
    }
}

#[extension_traits::extension(pub trait ReadExt)]
impl<R: Read> R {
    fn and_validate_hash(self, expected_hash: u64) -> ValidateHashReader<R> {
        ValidateHashReader::new(self, expected_hash)
    }
    fn and_validate_size(self, expected_size: u64) -> ValidateSizeReader<R> {
        ValidateSizeReader::new(self, expected_size)
    }
}
