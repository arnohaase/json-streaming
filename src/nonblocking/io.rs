use async_trait::async_trait;
use core::error::Error;
use crate::blocking::{BlockingRead, BlockingWrite};

/// [NonBlockingWrite] is the library's abstraction for non-blocking write I/O.
///
/// It is similar to `tokio::io::AsyncWrite`, and there is a blanket implementation of 
///  [NonBlockingWrite] for any implementation of `AsyncWrite`. The reason for introducing 
///  [NonBlockingWrite] is that it decouples json-streaming from tokio and allows it to be used
///  with other async frameworks.
///
/// Note that json-streaming writes data to a [NonBlockingWrite] in many small chunks without any 
///  I/O buffering. It is the client's responsibility to add buffering for improved performance 
///  where desired.
#[async_trait]
pub trait NonBlockingWrite {
    type Error: Error;

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error>;
}

#[cfg(all(test, not(feature="tokio")))]
#[async_trait]
impl NonBlockingWrite for Vec<u8> {
    type Error = std::io::Error;

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.extend_from_slice(buf);
        Ok(())
    }
}

#[cfg(feature = "tokio")]
/// Blanket implementation that allows any [tokio::io::AsyncWrite] implementation to be used 
///  seamlessly as [NonBlockingWrite].
#[async_trait]
impl <W: tokio::io::AsyncWrite + Unpin + Send> NonBlockingWrite for W {
    type Error = std::io::Error;

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        tokio::io::AsyncWriteExt::write_all(self, buf).await
    }
}


#[async_trait]
/// [NonBlockingRead] is the library's abstraction for non-blocking read I/O.
///
/// It is similar to `tokio:io::AsyncRead`, and there is a blanket implementation of 
///  [NonBlockingRead] for any implementation of `AsyncRead`. The reason for introducing 
///  [NonBlockingRead] is to decouple json-streaming from tokio and allow it to be used with
///  other async frameworks.
///
/// Note that json-streaming reads data from a [NonBlockingRead] in many small chunks without any 
///  I/O buffering. It is the client's responsibility to add buffering for improved performance 
///  where desired.
pub trait NonBlockingRead {
    type Error: Error;

    async fn read(&mut self) -> Result<Option<u8>, Self::Error>;
}

#[cfg(all(test, not(feature = "tokio")))]
#[async_trait]
impl NonBlockingRead for std::io::Cursor<Vec<u8>> {
    type Error = std::io::Error;

    async fn read(&mut self) -> Result<Option<u8>, Self::Error> {
        let mut result = [0u8; 1];
        let num_read = std::io::Read::read(self, &mut result)?;
        if num_read == 1 {
            Ok(Some(result[0]))
        }
        else {
            Ok(None)
        }
    }
}

#[cfg(feature = "tokio")]
/// Blanket implementation that allows any [tokio::io::AsyncRead] implementation to be used 
///  seamlessly as [NonBlockingRead].
#[async_trait]
impl <R: tokio::io::AsyncRead + Unpin + Send> NonBlockingRead for R {
    type Error = std::io::Error;

    async fn read(&mut self) -> Result<Option<u8>, Self::Error> {
        let mut result = [0u8; 1];
        let num_read = tokio::io::AsyncReadExt::read(self, &mut result).await?;
        if num_read == 1 {
            Ok(Some(result[0]))
        }
        else {
            Ok(None)
        }
    }
}
