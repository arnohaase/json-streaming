use async_trait::async_trait;
use core::error::Error;

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
#[async_trait]
impl <W: tokio::io::AsyncWrite + Unpin + Send> NonBlockingWrite for W {
    type Error = std::io::Error;

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        tokio::io::AsyncWriteExt::write_all(self, buf).await
    }
}


#[async_trait]
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

//TODO unit test feature "tokio"
#[cfg(feature = "tokio")]
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
