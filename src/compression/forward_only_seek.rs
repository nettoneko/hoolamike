use {
    std::io::{self, Read, Seek, SeekFrom},
    tap::prelude::*,
};

pub struct ForwardOnlySeek<R: Read> {
    end_reached: bool,
    inner: R,
    position: u64,
}

impl<R: Read> ForwardOnlySeek<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: reader,
            position: 0,
            end_reached: false,
        }
    }
}

impl<R: Read> Read for ForwardOnlySeek<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_read = self.inner.read(buf)?;
        self.position += bytes_read as u64;
        Ok(bytes_read)
    }
}

impl<R: Read> ForwardOnlySeek<R> {
    fn skip_to_end(&mut self) -> io::Result<u64> {
        let mut buffer = [0; 1024];
        while let Ok(bytes_read) = self.inner.read(&mut buffer) {
            if bytes_read == 0 {
                self.end_reached = true;
                break;
            }
            self.position += bytes_read as u64;
        }
        Ok(self.position)
    }
}

impl<R: Read> Seek for ForwardOnlySeek<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(new_pos) => {
                if new_pos < self.position {
                    return Err(anyhow::anyhow!("{pos:?} Backward seek is not allowed;").pipe(std::io::Error::other));
                }
                let to_skip = new_pos - self.position;
                io::copy(&mut self.inner.by_ref().take(to_skip), &mut io::sink())?;
                self.position = new_pos;
            }
            SeekFrom::Current(offset) => {
                if offset < 0 {
                    return Err(anyhow::anyhow!("{pos:?} Backward seek is not allowed;").pipe(std::io::Error::other));
                }
                let to_skip = offset as u64;
                io::copy(&mut self.inner.by_ref().take(to_skip), &mut io::sink())?;
                self.position += to_skip;
            }
            SeekFrom::End(0) => {
                self.skip_to_end()?;
            }
            SeekFrom::End(_) => {
                return Err(anyhow::anyhow!("{pos:?} Backward seek is not allowed;").pipe(std::io::Error::other));
            }
        }
        Ok(self.position)
    }
}
