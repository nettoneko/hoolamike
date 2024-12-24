use {
    super::IoHook,
    std::{
        pin::Pin,
        task::{Context, Poll},
    },
    tokio::io::{self, AsyncWrite, AsyncWriteExt, ReadBuf},
};

impl<W: tokio::io::AsyncWrite + Unpin, F: Fn(usize)> tokio::io::AsyncWrite for IoHook<W, F> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf).map(|poll| {
            poll.map(|inc| {
                (self.callback)(inc);
                inc
            })
        })
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
