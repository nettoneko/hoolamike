use {
    super::IoHook,
    std::io::{self, Write},
};

#[extension_traits::extension(pub trait WriteHookExt)]
impl<T: Write> T
where
    Self: Sized,
{
    fn hook_write<F: Fn(usize)>(self, hook_read: F) -> IoHook<T, F> {
        IoHook {
            inner: self,
            callback: hook_read,
        }
    }
}

impl<W, F> Write for IoHook<W, F>
where
    W: Write,
    F: Fn(usize),
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes_written = self.inner.write(buf)?;
        (self.callback)(bytes_written);
        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
