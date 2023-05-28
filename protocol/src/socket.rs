use std::{net::SocketAddr, task::{Poll, Context}, pin::Pin};
use tokio::io::{self, AsyncRead, ReadBuf, AsyncWrite};

pub trait Socket {
    fn poll_recv(&mut self, cx: &mut Context, buf: &mut [u8]) -> Poll<io::Result<usize>>;
	fn poll_send(&mut self, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>>;
	fn local_addr(&self) -> io::Result<SocketAddr>;
}

impl<T: AsyncRead + AsyncWrite + Unpin> Socket for T {
    fn poll_recv(&mut self, cx: &mut std::task::Context, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let mut read_buf = ReadBuf::new(buf);
        match T::poll_read(Pin::new(self), cx, &mut read_buf) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(read_buf.filled().len())),
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => Poll::Pending
        }
    }

    fn poll_send(&mut self, cx: &mut std::task::Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        T::poll_write(Pin::new(self), cx, buf)
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        todo!()
    }
}