use async_trait::async_trait;
use core::error::Error;

#[async_trait]
pub trait NonBlockingWrite {
    type Error: Error;

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error>;
}

#[cfg(test)]
#[async_trait]
impl NonBlockingWrite for Vec<u8> {
    type Error = std::io::Error;

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.extend_from_slice(buf);
        Ok(())
    }
}

// #[cfg(not(feature = "no-std"))] TODO
// impl <W: std::io::Write> BlockingWrite for W {
//     type Error = std::io::Error;
//
//     fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
//         W::write_all(self, buf)
//     }
// }

#[async_trait]
pub trait NonBlockingRead {
    type Error: Error;

    async fn read(&mut self) -> Result<Option<u8>, Self::Error>;
}

#[cfg(test)]
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

// #[cfg(not(feature = "no-std"))] TODO
// /// Blanket implementation for [std::io::Read] - implementation should preferably use an internal
// ///  read buffer because access is fine-grained
// impl <R: std::io::Read> BlockingRead for R {
//     type Error = std::io::Error;
//
//     fn read(&mut self) -> Result<Option<u8>, Self::Error> {
//         let mut result = [0u8; 1];
//         let num_read = R::read(self, &mut result)?;
//         if num_read == 1 {
//             Ok(Some(result[0]))
//         }
//         else {
//             Ok(None)
//         }
//     }
// }