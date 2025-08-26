use core::error::Error;


pub trait BlockingWrite {
    type Error: Error;

    fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error>;
}

#[cfg(not(feature = "no-std"))]
impl <W: std::io::Write> BlockingWrite for W {
    type Error = std::io::Error;

    fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        W::write_all(self, buf)
    }
}


pub trait BlockingRead {
    type Error: Error;

    fn read(&mut self) -> Result<Option<u8>, Self::Error>;
}

#[cfg(not(feature = "no-std"))]
/// Blanket implementation for [std::io::Read] - implementation should preferably use an internal
///  read buffer because access is fine-grained
impl <R: std::io::Read> BlockingRead for R {
    type Error = std::io::Error;

    fn read(&mut self) -> Result<Option<u8>, Self::Error> {
        let mut result = [0u8; 1];
        let num_read = R::read(self, &mut result)?;
        if num_read == 1 {
            Ok(Some(result[0]))
        }
        else {
            Ok(None)
        }
    }
}